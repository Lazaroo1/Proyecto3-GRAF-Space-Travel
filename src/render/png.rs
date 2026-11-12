//! PNG propio sin dependencias: escribe y lee RGB de 8 bits.
//!
//! **Escribir** (capturas, fotos y las imagenes de `media/`): cada fila va
//! con el filtro de PNG que la deja mas facil de comprimir (ninguno, resta
//! con el de la izquierda, con el de arriba, con el promedio o con Paeth: se
//! queda el que da la menor suma de diferencias, la heuristica que
//! recomienda la especificacion) y todo pasa por deflate (`zlib.rs`). Antes
//! se guardaba en bloques sin comprimir: una captura de 1280 x 720 pesaba
//! 2.7 MB y ahora pesa como la tercera parte.
//!
//! **Leer** (las texturas de `assets/`): RGB o RGBA de 8 bits sin
//! entrelazado, de este programa o de cualquier otro.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

use super::zlib;

/// Guarda una imagen RGBA8 por filas como PNG RGB.
pub fn write_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> io::Result<()> {
    let (w, h) = (width as usize, height as usize);
    assert_eq!(rgba.len(), w * h * 4, "la imagen no mide {width}x{height}");
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    write_rgb(path, width, height, &rgb)
}

/// Guarda una imagen RGB8 por filas como PNG.
pub fn write_rgb(path: &Path, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
    fs::write(path, encode_rgb(width, height, rgb))
}

/// Un PNG RGB8 en memoria.
pub fn encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    assert_eq!(rgb.len(), w * h * 3, "la imagen no mide {width}x{height}");
    let stride = w * 3;
    let mut raw = Vec::with_capacity((stride + 1) * h);
    let zero = vec![0u8; stride];
    let mut candidate = vec![0u8; stride];
    let mut best = vec![0u8; stride];
    for y in 0..h {
        let row = &rgb[y * stride..(y + 1) * stride];
        let up = if y > 0 { &rgb[(y - 1) * stride..y * stride] } else { &zero[..] };
        let mut best_kind = 0u8;
        let mut best_cost = u64::MAX;
        for kind in 0..5u8 {
            filter_row(kind, row, up, 3, &mut candidate);
            // La suma de las diferencias vistas con signo: lo que mas se
            // acerca a cero comprime mejor.
            let cost: u64 = candidate.iter().map(|&b| (b as i8).unsigned_abs() as u64).sum();
            if cost < best_cost {
                best_cost = cost;
                best_kind = kind;
                best.copy_from_slice(&candidate);
            }
        }
        raw.push(best_kind);
        raw.extend_from_slice(&best);
    }
    let mut png = Vec::with_capacity(raw.len() / 2 + 64);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // 8 bits por canal, RGB, compresion y filtro estandar, sin entrelazado.
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &header);
    write_chunk(&mut png, b"IDAT", &zlib::compress(&raw));
    write_chunk(&mut png, b"IEND", &[]);
    png
}

/// El predictor de Paeth: el vecino (izquierda, arriba o arriba a la
/// izquierda) mas cercano a `a + b - c`.
#[inline]
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Filtra `row` (con `up` la fila de arriba ya sin filtrar) con el filtro
/// `kind`, a `bpp` bytes por pixel.
fn filter_row(kind: u8, row: &[u8], up: &[u8], bpp: usize, out: &mut [u8]) {
    for i in 0..row.len() {
        let a = if i >= bpp { row[i - bpp] } else { 0 };
        let c = if i >= bpp { up[i - bpp] } else { 0 };
        let b = up[i];
        let pred = match kind {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((a as u16 + b as u16) / 2) as u8,
            _ => paeth(a, b, c),
        };
        out[i] = row[i].wrapping_sub(pred);
    }
}

/// Lee un PNG de 8 bits por canal (RGB, RGBA o gris) sin entrelazado y
/// devuelve su ancho, su alto y los pixeles en RGB.
pub fn read_rgb(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    decode_rgb(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn decode_rgb(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("no es un PNG".into());
    }
    let (mut width, mut height, mut channels) = (0u32, 0u32, 0usize);
    let mut idat = Vec::new();
    let mut at = 8;
    while at + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = &bytes[at + 4..at + 8];
        let data = bytes.get(at + 8..at + 8 + len).ok_or("PNG cortado")?;
        match kind {
            b"IHDR" => {
                if data.len() < 13 {
                    return Err("IHDR corto".into());
                }
                width = u32::from_be_bytes(data[0..4].try_into().unwrap());
                height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                let (depth, color, interlace) = (data[8], data[9], data[12]);
                channels = match color {
                    0 => 1,
                    2 => 3,
                    4 => 2,
                    6 => 4,
                    _ => return Err(format!("tipo de color {color} no soportado")),
                };
                if depth != 8 || interlace != 0 {
                    return Err("solo PNG de 8 bits sin entrelazado".into());
                }
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + len;
    }
    if width == 0 || height == 0 || channels == 0 {
        return Err("PNG sin imagen".into());
    }
    let raw = zlib::decompress(&idat)?;
    let (w, h) = (width as usize, height as usize);
    let stride = w * channels;
    if raw.len() < (stride + 1) * h {
        return Err("PNG con menos datos que su tamano".into());
    }
    let mut pixels = vec![0u8; stride * h];
    let zero = vec![0u8; stride];
    for y in 0..h {
        let kind = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (done, rest) = pixels.split_at_mut(y * stride);
        let up = if y > 0 { &done[(y - 1) * stride..] } else { &zero[..] };
        let row = &mut rest[..stride];
        for i in 0..stride {
            let a = if i >= channels { row[i - channels] } else { 0 };
            let c = if i >= channels { up[i - channels] } else { 0 };
            let b = up[i];
            let pred = match kind {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err(format!("filtro {kind} invalido")),
            };
            row[i] = src[i].wrapping_add(pred);
        }
    }
    let rgb = match channels {
        3 => pixels,
        4 => pixels.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(),
        2 => pixels.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0]]).collect(),
        _ => pixels.iter().flat_map(|&v| [v, v, v]).collect(),
    };
    Ok((width, height, rgb))
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        std::array::from_fn(|n| {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            c
        })
    });
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(zlib::adler32(b"Wikipedia"), 0x11E6_0398);
    }

    /// Una imagen guardada vuelve igual y pesa menos que cruda.
    #[test]
    fn an_image_round_trips_and_shrinks() {
        let (w, h) = (97u32, 61u32);
        let rgb: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                [(x * 2) as u8, (y * 4) as u8, ((x ^ y) * 3) as u8]
            })
            .collect();
        let png = encode_rgb(w, h, &rgb);
        assert!(png.len() < rgb.len() / 2, "{} de {}", png.len(), rgb.len());
        let (rw, rh, back) = decode_rgb(&png).unwrap();
        assert_eq!((rw, rh), (w, h));
        assert_eq!(back, rgb);
    }
}
