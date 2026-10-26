//! Cielo: estrellas procedurales y la corona de la estrella.
//!
//! Las estrellas no son una textura. La direccion del pixel se proyecta sobre
//! un cubo, cada cara se divide en celdas y un hash de la celda decide si hay
//! una estrella, donde esta dentro de la celda, cuanto brilla y de que color
//! es. Tres capas con celdas de distinto tamano dan densidad sin que se note
//! una repeticion. Como solo depende de la direccion, el cielo esta en el
//! infinito: no se mueve al trasladar la camara, solo al girarla.

use std::sync::OnceLock;

use super::Uniforms;
use crate::math::blackbody::planckian_rgb;
use crate::math::Vec3;

/// (celdas por lado de cada cara, probabilidad de estrella, brillo).
const LAYERS: [(f32, f32, f32); 3] = [(40.0, 0.22, 1.0), (80.0, 0.16, 0.55), (150.0, 0.12, 0.3)];
/// Fondo apenas morado: el negro puro se ve plano.
const BACKGROUND: Vec3 = Vec3::new(0.0004, 0.0003, 0.0008);

pub fn shade(dir: Vec3, u: &Uniforms) -> Vec3 {
    let mut color = BACKGROUND;
    for (layer, &(cells, density, brightness)) in LAYERS.iter().enumerate() {
        color += star_layer(dir, cells, density, brightness, layer as u32, u.pixel_angle);
    }
    color + corona(dir, u)
}

fn star_layer(
    d: Vec3,
    cells: f32,
    density: f32,
    brightness: f32,
    layer: u32,
    pixel_angle: f32,
) -> Vec3 {
    // Cara del cubo: el eje de mayor magnitud.
    let a = d.abs();
    let (face, u, v, major) = if a.x >= a.y && a.x >= a.z {
        (u32::from(d.x < 0.0), d.y, d.z, a.x)
    } else if a.y >= a.z {
        (2 + u32::from(d.y < 0.0), d.x, d.z, a.y)
    } else {
        (4 + u32::from(d.z < 0.0), d.x, d.y, a.z)
    };
    let gx = (u / major * 0.5 + 0.5) * cells;
    let gy = (v / major * 0.5 + 0.5) * cells;
    let (cx, cy) = (gx.floor(), gy.floor());

    let h = hash(face + layer * 8, cx as u32, cy as u32);
    if unit(h) > density {
        return Vec3::ZERO;
    }
    let h2 = mix(h);
    let h3 = mix(h2);
    let h4 = mix(h3);
    let h5 = mix(h4);
    // La estrella queda lejos del borde de su celda: su brillo no se corta.
    let sx = cx + 0.35 + 0.3 * unit(h2);
    let sy = cy + 0.35 + 0.3 * unit(h3);
    let dist2 = (gx - sx) * (gx - sx) + (gy - sy) * (gy - sy);

    // Tamano propio y tamano de un pixel, en celdas. Una estrella menor que
    // un pixel parpadearia al girar la camara; se ensancha y se atenua para
    // conservar su brillo total.
    let cell_angle = 2.0 / cells;
    let size = 0.025 + 0.025 * unit(h4);
    let sigma = size.max(0.5 * pixel_angle / cell_angle);
    let energy = (size / sigma) * (size / sigma);
    // Casi todas tenues y unas pocas brillantes, como en un cielo real.
    let peak = brightness * (0.05 + 7.0 * unit(h5).powi(10)) * energy;
    star_color(unit(mix(h5))) * (peak * (-dist2 / (2.0 * sigma * sigma)).exp())
}

/// Resplandor alrededor del disco de la estrella: cae rapido junto al borde y
/// deja una cola larga y tenue.
///
/// La distancia angular se mide con la cuerda, `sqrt(2 (1 - cos))`, en vez
/// de `acos`: para angulos pequenos son casi iguales, y lejos del disco la
/// corona ya es despreciable. Se ahorra un `acos` y un `asin` por pixel.
fn corona(dir: Vec3, u: &Uniforms) -> Vec3 {
    let dist2 = u.sun_pos.length_squared();
    let r2 = u.sun_radius * u.sun_radius;
    if dist2 <= r2 {
        return Vec3::ZERO;
    }
    let cos_angle = dir.dot(u.sun_pos) / dist2.sqrt();
    // Radio angular del disco al cuadrado (seno, casi igual al angulo).
    let disc2 = r2 / dist2;
    let x2 = (2.0 * (1.0 - cos_angle) / disc2).max(1.0);
    let x = x2.sqrt();
    u.sun_light * (0.35 * (-(x - 1.0) * 4.0).exp() + 0.025 / x2)
}

/// Color de estrella: mas frias (rojizas) que calientes (azules).
fn star_color(r: f32) -> Vec3 {
    static TABLE: OnceLock<[Vec3; 32]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let s = i as f32 / 31.0;
            planckian_rgb(2600.0 + 9400.0 * s * s)
        })
    });
    table[((r * 31.0) as usize).min(31)]
}

#[inline]
fn hash(a: u32, b: u32, c: u32) -> u32 {
    mix(a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D))
}

/// Mezclador de bits (lowbias32 de Chris Wellons).
#[inline]
fn mix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}
