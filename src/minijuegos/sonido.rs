//! Sonidos grabados para los minijuegos: un lector de WAV propio (PCM de 8,
//! 16, 24 o 32 bits, o flotante de 32) que mezcla los canales a uno y
//! remuestrea (lineal) a la frecuencia del juego.
//!
//! Un WAV es un RIFF: la cabecera `RIFF....WAVE` y despues trozos con su
//! nombre de cuatro letras y su largo. `fmt ` dice el formato (1 entero,
//! 3 flotante, 0xFFFE extensible con el formato en su subtipo), los canales,
//! la frecuencia y los bits por muestra; `data` trae las muestras
//! intercaladas por canal, en little endian.

use std::path::Path;

/// Las muestras de `path` en mono, de -1 a 1, a `rate` por segundo, o nada
/// si no esta o no se entiende.
pub fn load_wav(path: &Path, rate: u32) -> Option<Vec<f32>> {
    decode_wav(&std::fs::read(path).ok()?, rate)
}

pub fn decode_wav(bytes: &[u8], rate: u32) -> Option<Vec<f32>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (mut format, mut channels, mut source_rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    let mut data: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32_at(bytes, at + 4) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        if id == b"fmt " && body.len() >= 16 {
            format = u16_at(body, 0);
            channels = u16_at(body, 2);
            source_rate = u32_at(body, 4);
            bits = u16_at(body, 14);
            if format == 0xFFFE && body.len() >= 26 {
                format = u16_at(body, 24);
            }
        } else if id == b"data" {
            data = Some(body);
        }
        at += 8 + len + (len & 1);
    }
    let data = data?;
    if channels == 0 || source_rate == 0 {
        return None;
    }
    let width = (bits as usize).div_ceil(8);
    let frame = width * channels as usize;
    if width == 0 || frame == 0 {
        return None;
    }
    let sample = |b: &[u8]| -> Option<f32> {
        Some(match (format, bits) {
            (1, 8) => (b[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
            (1, 24) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            _ => return None,
        })
    };
    let mut mono = Vec::with_capacity(data.len() / frame);
    for f in data.chunks_exact(frame) {
        let mut sum = 0.0;
        for c in f.chunks_exact(width) {
            sum += sample(c)?;
        }
        mono.push(sum / channels as f32);
    }
    if source_rate == rate || mono.len() < 2 {
        return Some(mono);
    }
    // Lineal: a esta frecuencia, para un claxon basta.
    let step = source_rate as f64 / rate as f64;
    let n = ((mono.len() - 1) as f64 / step) as usize + 1;
    Some(
        (0..n)
            .map(|i| {
                let x = i as f64 * step;
                let k = x as usize;
                let t = (x - k as f64) as f32;
                let (a, b) = (mono[k], mono[(k + 1).min(mono.len() - 1)]);
                a + (b - a) * t
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un WAV estereo de 16 bits a 22 050 Hz se lee mezclado y remuestreado.
    #[test]
    fn a_stereo_wav_is_mixed_and_resampled() {
        let rate = 22_050u32;
        let frames = 2205usize;
        let mut data = Vec::new();
        for i in 0..frames {
            let v = ((i as f32 / frames as f32) * 20_000.0) as i16;
            data.extend_from_slice(&v.to_le_bytes());
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        let out = decode_wav(&wav, 44_100).expect("se lee");
        assert!((out.len() as i64 - 2 * frames as i64).abs() <= 2, "{}", out.len());
        assert!((out[out.len() / 2] - 0.305).abs() < 0.01, "{}", out[out.len() / 2]);
        assert!(decode_wav(b"no es un wav", 44_100).is_none());
    }

    /// El claxon de don Meyer que va en el repositorio se lee (unos 3 s).
    #[test]
    fn the_horn_in_assets_is_read() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sonidos/claxon_meyer.wav");
        let out = load_wav(&path, 44_100).expect("se lee");
        assert!(out.len() > 2 * 44_100, "{}", out.len());
        assert!(out.iter().all(|s| s.abs() <= 1.0));
    }
}
