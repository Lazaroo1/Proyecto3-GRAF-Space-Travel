//! Tonemap: de color HDR lineal a RGBA8 en sRGB.
//!
//! El render trabaja en HDR: la estrella brilla decenas de veces mas que un
//! planeta iluminado. La curva ACES comprime ese rango con un hombro suave
//! hacia el blanco, en vez de recortar. Despues se codifica en sRGB, la curva
//! de la pantalla, con un tramado minimo que evita bandas en los degradados
//! oscuros del espacio.
//!
//! # Conservar el tono
//! ACES aplicado canal por canal satura primero el canal mas fuerte y lava los
//! colores intensos hacia el blanco: la supergigante roja saldria amarilla.
//! Como en el Proyecto 2, la curva se aplica al canal maximo y el color se
//! escala entero, asi conserva su tono; solo los brillos extremos se mezclan
//! con ACES por canal y se vuelven blancos, como un nucleo sobreexpuesto.
//!
//! # Tabla de sRGB
//! sRGB tiene un tramo lineal junto al negro (pendiente 12.92). Gracias a eso
//! una tabla indexada directamente por el valor lineal no hace bandas: un
//! escalon de la tabla mueve la salida menos de un nivel de 8 bits incluso en
//! lo mas oscuro, y el tramado lo disimula. Con una gamma 2.2 pura, el primer
//! escalon saltaria cinco niveles.

use std::sync::OnceLock;

use super::bloom::MipLevel;
use super::{TILE, TILE_PIXELS};
use crate::math::Vec3;

const LUT_SIZE: usize = 4096;

/// Tabla de la curva sRGB en `[0, 255]`, indexada por el valor lineal.
fn srgb_lut() -> &'static [f32; LUT_SIZE] {
    static LUT: OnceLock<[f32; LUT_SIZE]> = OnceLock::new();
    LUT.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / (LUT_SIZE - 1) as f32;
            let s = if c <= 0.003_130_8 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            s * 255.0
        })
    })
}

/// Ajuste de Narkowicz a la curva ACES (portado del Proyecto 2).
#[inline]
fn aces(x: f32) -> f32 {
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// ACES sobre el canal maximo, con mezcla a ACES por canal en los brillos.
#[inline]
fn tonemap(color: Vec3) -> Vec3 {
    let color = color.max(Vec3::ZERO);
    let peak = color.max_element();
    if peak <= 1.0e-6 {
        return Vec3::ZERO;
    }
    let mapped_peak = aces(peak);
    let hue = color * (mapped_peak / peak);
    // Casi todos los pixeles quedan por debajo del hombro: solo los muy
    // brillantes pagan las tres curvas por canal.
    if mapped_peak <= 0.75 {
        return hue;
    }
    let t = (mapped_peak - 0.75) * 4.0;
    let white = t * t * (3.0 - 2.0 * t) * 0.6;
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
    // `v` ya viene en [0, 1] del tonemap.
    let index = (v * (LUT_SIZE - 1) as f32) as usize;
    (lut[index.min(LUT_SIZE - 1)] + dither).min(255.0) as u8
}

/// Parametros del pase final.
pub struct PostParams<'a> {
    pub width: usize,
    pub height: usize,
    pub tiles_x: usize,
    pub exposure: f32,
    pub bloom: &'a MipLevel,
    pub bloom_intensity: f32,
}

/// Fila `y` del bloom ampliada al ancho completo, con interpolacion bilineal:
/// primero se mezclan en vertical las dos filas del nivel (una vez por fila),
/// despues cada pixel solo mezcla dos valores vecinos.
fn bloom_row(bloom: &MipLevel, y: usize, height: usize, out: &mut [Vec3], column: &mut Vec<Vec3>) {
    let (bw, bh) = (bloom.width, bloom.height);
    let fy = ((y as f32 + 0.5) / height as f32 * bh as f32 - 0.5).max(0.0);
    let y0 = (fy as usize).min(bh - 1);
    let y1 = (y0 + 1).min(bh - 1);
    let ty = fy - y0 as f32;
    column.clear();
    column.extend(
        bloom.pixels[y0 * bw..(y0 + 1) * bw]
            .iter()
            .zip(&bloom.pixels[y1 * bw..(y1 + 1) * bw])
            .map(|(a, b)| a.lerp(*b, ty)),
    );
    let width = out.len();
    let scale = bw as f32 / width as f32;
    for (x, o) in out.iter_mut().enumerate() {
        let fx = ((x as f32 + 0.5) * scale - 0.5).max(0.0);
        let x0 = (fx as usize).min(bw - 1);
        let x1 = (x0 + 1).min(bw - 1);
        *o = column[x0].lerp(column[x1], fx - x0 as f32);
    }
}

/// Convierte a RGBA8 las filas desde `first_row` que caben en `out`, sumando
/// el halo del bloom antes del tonemap. Recorre cada fila por tramos de tile,
/// que en el buffer HDR son contiguos.
pub fn tonemap_rows(first_row: usize, out: &mut [u8], hdr: &[Vec3], p: &PostParams) {
    let lut = srgb_lut();
    let width = p.width;
    let rows = out.len() / (width * 4);
    let mut glow = vec![Vec3::ZERO; width];
    let mut column = Vec::with_capacity(p.bloom.width);
    for ly in 0..rows {
        let y = first_row + ly;
        bloom_row(p.bloom, y, p.height, &mut glow, &mut column);
        let out_row = &mut out[ly * width * 4..(ly + 1) * width * 4];
        let tile_row = (y / TILE) * p.tiles_x;
        let in_tile = (y % TILE) * TILE;
        for tx in 0..p.tiles_x {
            let x0 = tx * TILE;
            let n = TILE.min(width - x0);
            let start = (tile_row + tx) * TILE_PIXELS + in_tile;
            let src = &hdr[start..start + n];
            let halo = &glow[x0..x0 + n];
            let dst = &mut out_row[x0 * 4..(x0 + n) * 4];
            for (i, ((color, light), px)) in src.iter().zip(halo).zip(dst.chunks_exact_mut(4)).enumerate() {
                let mapped = tonemap((*color + *light * p.bloom_intensity) * p.exposure);
                let d = dither(x0 + i, y);
                px[0] = encode(mapped.x, d, lut);
                px[1] = encode(mapped.y, d, lut);
                px[2] = encode(mapped.z, d, lut);
                px[3] = 255;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_table_has_no_big_steps_in_the_dark() {
        let lut = srgb_lut();
        assert_eq!(lut[0], 0.0);
        assert!((lut[LUT_SIZE - 1] - 255.0).abs() < 1e-3);
        // Ningun escalon de la tabla salta mas de un nivel de 8 bits.
        assert!(lut.windows(2).all(|w| w[1] - w[0] < 1.0));
    }

    #[test]
    fn tonemap_keeps_hue_below_the_shoulder() {
        let c = tonemap(Vec3::new(0.6, 0.3, 0.1));
        assert!((c.y / c.x - 0.5).abs() < 1e-5);
        assert!((c.z / c.x - 1.0 / 6.0).abs() < 1e-5);
        assert_eq!(tonemap(Vec3::ZERO), Vec3::ZERO);
    }
}
