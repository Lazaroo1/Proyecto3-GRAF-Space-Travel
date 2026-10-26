//! Tonemap: de color HDR lineal a RGBA8 en gamma.
//!
//! El render trabaja en HDR: la estrella brilla decenas de veces mas que un
//! planeta iluminado. La curva ACES comprime ese rango con un hombro suave
//! hacia el blanco, en vez de recortar. Despues se aplica la gamma de la
//! pantalla y un tramado minimo que evita bandas en los degradados oscuros
//! del espacio.
//!
//! # Conservar el tono
//! ACES aplicado canal por canal satura primero el canal mas fuerte y lava
//! los colores intensos hacia el blanco: la supergigante roja saldria
//! amarilla. Como en el Proyecto 2, la curva se aplica al canal maximo y el
//! color se escala entero, asi conserva su tono; solo los brillos extremos se
//! mezclan con ACES por canal y se vuelven blancos, como un nucleo
//! sobreexpuesto.

use std::sync::OnceLock;

use super::{TILE, TILE_PIXELS};
use crate::math::{smoothstep, Vec3};

const LUT_SIZE: usize = 4096;

/// Tabla de gamma indexada por `sqrt(v)`: asi tiene mucha mas resolucion en
/// los tonos oscuros, donde la curva de gamma es mas empinada.
fn gamma_lut() -> &'static [f32; LUT_SIZE] {
    static LUT: OnceLock<[f32; LUT_SIZE]> = OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|i| {
            let s = i as f32 / (LUT_SIZE - 1) as f32;
            (s * s).powf(1.0 / 2.2) * 255.0
        })
    })
}

/// Ajuste de Narkowicz a la curva ACES (portado del Proyecto 2).
#[inline]
fn aces(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// ACES sobre el canal maximo, con mezcla a ACES por canal en los brillos.
#[inline]
fn tonemap(color: Vec3) -> Vec3 {
    let color = color.max(Vec3::ZERO);
    let peak = color.max_element();
    if peak <= 0.0 {
        return Vec3::ZERO;
    }
    let mapped_peak = aces(peak);
    let hue = color * (mapped_peak / peak);
    // Casi todos los pixeles quedan por debajo del hombro: solo los muy
    // brillantes pagan las tres curvas por canal.
    let white = smoothstep(0.75, 1.0, mapped_peak) * 0.6;
    if white <= 0.0 {
        return hue;
    }
    let channels = Vec3::new(aces(color.x), aces(color.y), aces(color.z));
    hue.lerp(channels, white)
}

/// Tramado fijo en `[0, 1)`: redondeo estocastico sin parpadeo entre frames.
#[inline]
fn dither(x: usize, y: usize) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

#[inline]
fn encode(v: f32, dither: f32, lut: &[f32; LUT_SIZE]) -> u8 {
    let index = (v.sqrt() * (LUT_SIZE - 1) as f32) as usize;
    (lut[index.min(LUT_SIZE - 1)] + dither).min(255.0) as u8
}

/// Convierte la banda `band` (una fila de tiles) a RGBA8 por filas.
pub fn tonemap_band(
    band: usize,
    out: &mut [u8],
    hdr: &[Vec3],
    width: usize,
    tiles_x: usize,
    exposure: f32,
) {
    let lut = gamma_lut();
    let rows = out.len() / (width * 4);
    for ly in 0..rows {
        let y = band * TILE + ly;
        for x in 0..width {
            let tile = band * tiles_x + x / TILE;
            let color = tonemap(hdr[tile * TILE_PIXELS + ly * TILE + x % TILE] * exposure);
            let d = dither(x, y);
            let o = (ly * width + x) * 4;
            out[o] = encode(color.x, d, lut);
            out[o + 1] = encode(color.y, d, lut);
            out[o + 2] = encode(color.z, d, lut);
            out[o + 3] = 255;
        }
    }
}
