//! PNG minimo sin dependencias: bloques deflate sin comprimir.
//!
//! Sirve para capturas y para verificar el render sin abrir la ventana. Los
//! archivos pesan lo mismo que la imagen cruda, pero cualquier visor los abre.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

/// Guarda una imagen RGBA8 por filas como PNG RGB.
pub fn write_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> io::Result<()> {
    let (w, h) = (width as usize, height as usize);
    assert_eq!(rgba.len(), w * h * 4, "la imagen no mide {width}x{height}");

    // Cada fila empieza con su tipo de filtro (0 = ninguno).
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for row in rgba.chunks_exact(w * 4) {
        raw.push(0);
        for px in row.chunks_exact(4) {
            raw.extend_from_slice(&px[..3]);
        }
    }

    // zlib: cabecera, bloques "stored" de hasta 65535 bytes y Adler-32.
    let mut zlib = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    zlib.extend_from_slice(&[0x78, 0x01]);
    let blocks = raw.chunks(65535).len();
    for (i, block) in raw.chunks(65535).enumerate() {
        zlib.push(u8::from(i + 1 == blocks));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 64);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // 8 bits por canal, RGB, compresion y filtro estandar, sin entrelazado.
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &header);
    write_chunk(&mut png, b"IDAT", &zlib);
    write_chunk(&mut png, b"IEND", &[]);
    fs::write(path, png)
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

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    // 5552 es el mayor bloque que no desborda `b` antes del modulo.
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }
}
