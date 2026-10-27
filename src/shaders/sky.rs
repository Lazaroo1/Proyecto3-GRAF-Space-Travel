//! Cielo: estrellas y nebulosa horneadas en un cubemap al arrancar, mas la
//! corona de la estrella calculada en cada frame.
//!
//! # Estrellas procedurales
//! Las estrellas no vienen de una imagen. La direccion se proyecta sobre un
//! cubo, cada cara se divide en celdas y un hash de la celda decide si hay una
//! estrella, donde esta dentro de la celda, cuanto brilla y de que color es
//! (cuerpo negro). Tres capas con celdas de distinto tamano dan densidad sin
//! que se note la repeticion.
//!
//! # Por que hornearlas
//! Evaluar las tres capas en cada pixel costaba unos 230 ciclos: con el cielo
//! cubriendo casi toda la pantalla era la mitad del shading. Horneadas en un
//! cubemap de 1024x1024 por cara, el cielo cuesta una lectura bilineal por
//! pixel. Ademas el agujero negro, al desviar la luz, necesitara leer el cielo
//! en cualquier direccion: un cubemap es justo eso.
//!
//! Como el cielo solo depende de la direccion, esta en el infinito: no se
//! mueve al trasladar la camara, solo al girarla.

use std::sync::OnceLock;

use super::Uniforms;
use crate::math::blackbody::planckian_rgb;
use crate::math::{Perlin, Vec3, smoothstep};
use crate::parallel;

/// (celdas por lado de cada cara, probabilidad de estrella, brillo).
const LAYERS: [(f32, f32, f32); 3] = [(85.0, 0.12, 0.8), (180.0, 0.12, 0.42), (360.0, 0.16, 0.18)];
/// Fondo apenas morado: el negro puro se ve plano.
const BACKGROUND: Vec3 = Vec3::new(0.0004, 0.0003, 0.0008);

/// Cielo horneado: seis caras cuadradas, cada texel en RGBE de 32 bits
/// (tres mantisas de 8 bits con un exponente comun), que guarda HDR en una
/// cuarta parte de la memoria de tres `f32`.
pub struct SkyCube {
    size: usize,
    texels: Vec<u32>,
}

impl SkyCube {
    /// Hornea el cielo en paralelo. Cada texel evalua las estrellas en su
    /// direccion, con el tamano minimo de un texel para que no titilen.
    pub fn bake(size: usize, seed: u32) -> Self {
        let noise = Perlin::new(seed ^ 0x5EB0);
        let texel_angle = std::f32::consts::FRAC_PI_2 / size as f32;
        let mut texels = vec![0u32; 6 * size * size];
        parallel::chunks_mut(&mut texels, size, size, |row, out| {
            let face = row / size;
            let y = row % size;
            for (x, texel) in out.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let dir = face_direction(face, u, v);
                *texel = encode_rgbe(stars(dir, texel_angle) + nebula(dir, &noise) + BACKGROUND);
            }
        });
        Self { size, texels }
    }

    /// Color en la direccion `dir` (unitaria o no), con interpolacion bilineal
    /// dentro de la cara.
    pub fn sample(&self, dir: Vec3) -> Vec3 {
        let (face, u, v) = cube_face(dir);
        let n = self.size;
        let x = ((u * 0.5 + 0.5) * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let y = ((v * 0.5 + 0.5) * n as f32 - 0.5).clamp(0.0, (n - 1) as f32);
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(n - 1), (y0 + 1).min(n - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let base = face as usize * n * n;
        let t = |x: usize, y: usize| decode_rgbe(self.texels[base + y * n + x]);
        let top = t(x0, y0).lerp(t(x1, y0), fx);
        let bottom = t(x0, y1).lerp(t(x1, y1), fx);
        top.lerp(bottom, fy)
    }
}

/// Cielo del pixel: el cubemap mas la corona de la estrella. `dir` no
/// necesita estar normalizada: el cubemap divide por el eje mayor y la
/// corona normaliza solo si el pixel esta cerca de la estrella.
pub fn shade(dir: Vec3, u: &Uniforms) -> Vec3 {
    u.sky.sample(dir) + corona(dir, u) * u.star_glow
}

/// Cara del cubo (el eje de mayor magnitud) y coordenadas en ella, en [-1, 1].
#[inline]
fn cube_face(d: Vec3) -> (u32, f32, f32) {
    let a = d.abs();
    let (face, cu, cv, major) = if a.x >= a.y && a.x >= a.z {
        (u32::from(d.x < 0.0), d.y, d.z, a.x)
    } else if a.y >= a.z {
        (2 + u32::from(d.y < 0.0), d.x, d.z, a.y)
    } else {
        (4 + u32::from(d.z < 0.0), d.x, d.y, a.z)
    };
    let inv = 1.0 / major;
    (face, cu * inv, cv * inv)
}

/// Inversa de `cube_face`: la direccion del punto `(u, v)` de una cara.
fn face_direction(face: usize, u: f32, v: f32) -> Vec3 {
    let sign = if face % 2 == 0 { 1.0 } else { -1.0 };
    match face / 2 {
        0 => Vec3::new(sign, u, v),
        1 => Vec3::new(u, sign, v),
        _ => Vec3::new(u, v, sign),
    }
    .normalize()
}

fn stars(dir: Vec3, pixel_angle: f32) -> Vec3 {
    let (face, cu, cv) = cube_face(dir);
    let (fu, fv) = (cu * 0.5 + 0.5, cv * 0.5 + 0.5);
    let mut color = Vec3::ZERO;
    for (layer, &(cells, density, brightness)) in LAYERS.iter().enumerate() {
        let gx = fu * cells;
        let gy = fv * cells;
        let (cx, cy) = (gx.floor(), gy.floor());
        let h = hash(face + layer as u32 * 8, cx as u32, cy as u32);
        if unit(h) > density {
            continue;
        }
        let h2 = mix(h);
        let h3 = mix(h2);
        let h4 = mix(h3);
        let h5 = mix(h4);
        // La estrella queda lejos del borde de su celda: su brillo no se corta.
        let sx = cx + 0.35 + 0.3 * unit(h2);
        let sy = cy + 0.35 + 0.3 * unit(h3);
        let dist2 = (gx - sx) * (gx - sx) + (gy - sy) * (gy - sy);
        // Una estrella menor que un pixel parpadearia al girar la camara: se
        // ensancha hasta medio pixel y se atenua para conservar su brillo.
        let cell_angle = 2.0 / cells;
        let size = 0.012 + 0.016 * unit(h4);
        let sigma = size.max(0.5 * pixel_angle / cell_angle);
        let energy = (size / sigma) * (size / sigma);
        // Casi todas tenues y unas pocas brillantes, como en un cielo real.
        let peak = brightness * (0.08 + 2.8 * unit(h5).powi(14)) * energy;
        color += star_color(unit(mix(h5))).lerp(Vec3::ONE, 0.5)
            * (peak * (-dist2 / (2.0 * sigma * sigma)).exp());
    }
    color
}

/// Nebulosa roja y morada a lo largo de una banda del cielo, con vetas
/// oscuras de polvo: ruido fractal, muy tenue, solo para dar atmosfera.
fn nebula(dir: Vec3, noise: &Perlin) -> Vec3 {
    let band_normal = Vec3::new(0.25, 0.92, -0.3).normalize();
    let latitude = dir.dot(band_normal);
    let band = (-(latitude / 0.19).powi(2)).exp();
    if band < 0.01 {
        return Vec3::ZERO;
    }
    let gas = noise.fbm(dir * 5.2, 6) * 0.5 + 0.5;
    let dust = smoothstep(
        0.42,
        0.66,
        noise.fbm(dir * 13.0 + Vec3::new(7.0, 1.0, 3.0), 5) * 0.5 + 0.5,
    );
    let tint = noise.fbm(dir * 1.3 + Vec3::new(-3.0, 5.0, 2.0), 3) * 0.5 + 0.5;
    let color =
        Vec3::new(0.17, 0.29, 0.48).lerp(Vec3::new(0.36, 0.18, 0.30), smoothstep(0.35, 0.7, tint));
    let lane = 1.0 - 0.8 * (-(latitude / 0.055 + gas * 0.9).powi(2)).exp();
    color * (band * gas.powi(2) * (1.0 - 0.94 * dust) * lane * 0.16)
}

/// Resplandor alrededor del disco de la estrella: cae rapido junto al borde y
/// deja una cola larga y tenue.
///
/// La distancia angular se mide con la cuerda, `sqrt(2 (1 - cos))`, en vez
/// de `acos`: para angulos pequenos son casi iguales, y lejos del disco la
/// corona ya es despreciable.
pub fn corona(dir: Vec3, u: &Uniforms) -> Vec3 {
    let dist2 = u.sun_pos.length_squared();
    let r2 = u.sun_radius * u.sun_radius;
    if dist2 <= r2 {
        return Vec3::ZERO;
    }
    let cos = dir.dot(u.sun_pos) / (dir.length_squared() * dist2).sqrt();
    let disc2 = r2 / dist2;
    // Cuerda al cuadrado en radios del disco. Es continua en toda la esfera:
    // cortar la corona en el hemisferio opuesto dejaria un borde recto en el
    // cielo, porque su cola tenue todavia llega hasta ahi. Mas alla de 40
    // radios ya no se nota y se ahorran la raiz y la exponencial.
    let x2 = (2.0 * (1.0 - cos) / disc2).max(1.0);
    if x2 > 1600.0 {
        return Vec3::ZERO;
    }
    let x = x2.sqrt();
    let rim = 0.24 * (-(x - 1.0) * 12.0).exp();
    let halo = 0.035 * (-(x - 1.0) * 3.5).exp() + 0.002 / x2;
    u.sun_light * Vec3::new(1.0, 0.18, 0.045) * (rim + halo)
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

/// RGBE: tres mantisas de 8 bits que comparten el exponente del canal mayor.
fn encode_rgbe(c: Vec3) -> u32 {
    let c = c.max(Vec3::ZERO);
    let peak = c.max_element();
    if peak < 1e-9 {
        return 0;
    }
    let exponent = peak.log2().floor() as i32 + 1;
    let scale = 256.0 / 2f32.powi(exponent);
    let m = |v: f32| ((v * scale) as u32).min(255);
    m(c.x) | (m(c.y) << 8) | (m(c.z) << 16) | (((exponent + 128) as u32) << 24)
}

#[inline]
fn decode_rgbe(texel: u32) -> Vec3 {
    let e = (texel >> 24) as i32;
    if e == 0 {
        return Vec3::ZERO;
    }
    // 2^(e - 128 - 8), armado directo en los bits del exponente de un f32.
    let scale = f32::from_bits(((e - 128 - 8 + 127) as u32) << 23);
    Vec3::new(
        (texel & 0xFF) as f32 + 0.5,
        ((texel >> 8) & 0xFF) as f32 + 0.5,
        ((texel >> 16) & 0xFF) as f32 + 0.5,
    ) * scale
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Costo de un pixel de cielo en un solo hilo. Correr con
    /// `cargo test --release -- --ignored --nocapture medir_`.
    #[test]
    #[ignore]
    fn medir_cielo() {
        let sky = SkyCube::bake(crate::config::SKY_SIZE, 1);
        let noise = Perlin::new(1);
        let textures = super::super::Textures::generate();
        let u = Uniforms {
            shadow: None,
            points: &[],
            time: 0.0,
            beat: 0.0,
            beat_phase: 0.0,
            beat_rate: 0.5,
            heart: 1.0,
            star_glow: 1.0,
            flares: 0.0,
            nebula: None,
            tail: None,
            vapor: None,
            black_hole: None,
            sun_pos: Vec3::new(1000.0, 200.0, -3000.0),
            sun_radius: 300.0,
            sun_light: Vec3::new(1.5, 0.8, 0.4),
            noise: &noise,
            sky: &sky,
            textures: &textures,
            pixel_angle: 0.002,
            eye_dir: Vec3::Z,
            eye_near: 0.0,
            ship: Vec3::ZERO,
            melt: 0.0,
            ring: None,
            atmospheres: &[],
            sky_view: None,
            biolum: 1.0,
            gains: Default::default(),
            debug: Default::default(),
        };
        let n = 2_000_000;
        let dirs: Vec<Vec3> = (0..n)
            .map(|i| {
                let t = i as f32 * 0.000_37;
                Vec3::new(t.sin(), (t * 0.7).cos() * 0.5, t.cos())
            })
            .collect();
        let time = |label: &str, f: &dyn Fn(Vec3) -> Vec3| {
            let start = std::time::Instant::now();
            let mut acc = Vec3::ZERO;
            for d in &dirs {
                acc += f(*d);
            }
            std::hint::black_box(acc);
            let ns = start.elapsed().as_secs_f64() * 1e9 / n as f64;
            println!("{label:<10} {ns:6.1} ns por pixel");
        };
        time("cubemap", &|d| sky.sample(d));
        time("corona", &|d| corona(d, &u));
        time("cielo", &|d| shade(d, &u));
    }

    #[test]
    fn rgbe_round_trip_keeps_hdr_within_one_percent() {
        for c in [
            Vec3::new(0.001, 0.0005, 0.002),
            Vec3::new(3.0, 1.5, 0.2),
            Vec3::new(40.0, 30.0, 10.0),
        ] {
            let back = decode_rgbe(encode_rgbe(c));
            let error = (back - c).length() / c.length();
            assert!(error < 0.01, "{c:?} -> {back:?}");
        }
        assert_eq!(decode_rgbe(encode_rgbe(Vec3::ZERO)), Vec3::ZERO);
    }

    #[test]
    fn face_direction_inverts_cube_face() {
        for face in 0..6 {
            for (u, v) in [(-0.9, 0.3), (0.0, 0.0), (0.5, -0.7)] {
                let dir = face_direction(face, u, v);
                let (f, cu, cv) = cube_face(dir);
                assert_eq!(f as usize, face);
                assert!((cu - u).abs() < 1e-5 && (cv - v).abs() < 1e-5);
            }
        }
    }
}
