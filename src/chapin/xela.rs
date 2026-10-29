//! La Luna de Xelaju: la luna de los Cuchumatanes (solo existe en la
//! version chapina). Palida y quieta, como la de la cancion.
//!
//! - **Mares y crateres**: el color sale de la foto de la Luna (LROC, en
//!   `assets/textures`), mas gris y palida; el relieve, de dos campos de
//!   crateres (`Craters`, el mismo de Cementerio) sobre lomas suaves.
//! - **Luz de luna**: el regolito no se ve como un plato de Lambert. Con
//!   **Lommel-Seeliger** (`mu0 / (mu0 + mu)`, con `mu0` el coseno hacia la
//!   luz y `mu` hacia la camara) el disco lleno se ve parejo hasta el borde,
//!   como la Luna de verdad, y el terminador sigue marcando los crateres.
//! - **El huipil**: franjas de un tejido de Quetzaltenango grabadas en la
//!   luna: rombos con su punto, grecas de dientes y lineas, en una grilla de
//!   puntadas (como un brocado). Cada franja tiene un numero entero de
//!   puntadas por vuelta (cierra sin costura) y cada puntada es un cuadrito
//!   redondeado. Brillan de noche; de dia apenas se ven como hilo grabado.
//!   Cuando una puntada mide menos que un par de pixeles se dibuja su
//!   promedio: de lejos la franja brilla pareja y no parpadea.

use std::f32::consts::TAU;
use std::sync::Arc;

use super::{Chapin, Spec};
use crate::math::{Perlin, Vec3, smoothstep};
use crate::shaders::{Fragment, Uniforms, detail, pbr};
use crate::terrain::shapes::Craters;
use crate::terrain::{HeightField, TerrainSample};

pub fn spec() -> Spec {
    Spec {
        label: "Luna de Xelaju",
        material: Chapin::Xela,
        atmosphere: None,
        rings: None,
        motes: None,
    }
}

/// El suelo de la luna. Atributos: `x` borde de crater.
pub struct Moon {
    noise: Perlin,
    craters: Craters,
    small: Craters,
}

impl Moon {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            craters: Craters {
                frequency: 5.0,
                chance: 0.5,
                min_radius: 0.2,
                max_radius: 0.6,
                depth: 1.6,
                seed: seed ^ 0x3E1,
            },
            small: Craters {
                frequency: 15.0,
                chance: 0.45,
                min_radius: 0.15,
                max_radius: 0.55,
                depth: 0.5,
                seed: seed ^ 0xA7A,
            },
        }
    }
}

impl HeightField for Moon {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let swell = self.noise.fbm(dir * 2.5 + Vec3::new(1.7, -3.1, 0.6), 4);
        let (big, big_rim) = self.craters.sample(dir);
        let (small, small_rim) = self.small.sample(dir);
        TerrainSample {
            height: 0.8 * swell + big + small,
            attrs: Vec3::new(big_rim.max(small_rim), 0.0, 0.0),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        let (lo_a, hi_a) = self.craters.range();
        let (lo_b, hi_b) = self.small.range();
        (-0.8 + lo_a + lo_b, 0.8 + hi_a + hi_b)
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(Moon::new(seed))
}

/// Una franja del huipil: latitud del centro (radianes), puntadas por
/// vuelta, filas y el motivo.
struct Band {
    latitude: f32,
    around: f32,
    rows: f32,
    motif: fn(i32, i32) -> Option<usize>,
    /// Fraccion de puntadas encendidas (el promedio de lejos).
    fill: f32,
}

/// Hilos: morado, magenta, oro y turquesa.
const THREAD: [Vec3; 4] = [
    Vec3::new(0.42, 0.12, 0.9),
    Vec3::new(1.0, 0.16, 0.5),
    Vec3::new(1.0, 0.66, 0.2),
    Vec3::new(0.2, 0.85, 0.8),
];

/// La franja ancha: lineas de oro arriba y abajo, puntos de magenta, y una
/// cadena de rombos morados con un punto turquesa al centro.
fn diamonds(i: i32, j: i32) -> Option<usize> {
    let c = i.rem_euclid(8) - 4;
    match j {
        0 | 10 => Some(2),
        2 | 8 if i % 2 == 0 => Some(1),
        3..=7 => {
            let d = c.abs() + (j - 5).abs();
            if d == 2 || (d == 4 && c.abs() < 4) {
                Some(0)
            } else if d == 0 {
                Some(3)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Las franjas angostas: una greca de dientes entre dos lineas.
fn zigzag(i: i32, j: i32) -> Option<usize> {
    let tooth = 1 + (i.rem_euclid(6) - 3).abs().min(2);
    match j {
        0 | 4 => Some(1),
        _ if j == tooth => Some(3),
        _ => None,
    }
}

const BANDS: [Band; 3] = [
    Band { latitude: 0.0, around: 168.0, rows: 11.0, motif: diamonds, fill: 0.27 },
    Band { latitude: 0.62, around: 120.0, rows: 5.0, motif: zigzag, fill: 0.53 },
    Band { latitude: -0.62, around: 120.0, rows: 5.0, motif: zigzag, fill: 0.53 },
];

/// El tejido en la direccion `p` (unitaria, en el espacio de la luna):
/// cuanto brilla y de que color. `pixel` es lo que mide un pixel en
/// radianes sobre la luna.
fn huipil(p: Vec3, pixel: f32) -> Vec3 {
    let latitude = p.y.clamp(-1.0, 1.0).asin();
    let longitude = p.z.atan2(p.x) / TAU + 0.5;
    for band in &BANDS {
        // Puntadas cuadradas: a lo alto miden lo mismo que a lo largo en el
        // centro de la franja.
        let step = TAU / band.around * band.latitude.cos();
        let row = (latitude - band.latitude) / step + band.rows * 0.5;
        if !(0.0..band.rows).contains(&row) {
            continue;
        }
        let column = longitude * band.around;
        let (i, j) = (column.floor(), row.floor());
        let size = step / pixel;
        let resolved = smoothstep(1.5, 3.0, size);
        let mut color = Vec3::ZERO;
        if let Some(thread) = (band.motif)(i as i32, j as i32) {
            // Cuadrito redondeado con un hueco entre puntadas.
            let (fx, fy) = (column - i - 0.5, row - j - 0.5);
            let soft = (0.5 / size).max(0.04);
            color = THREAD[thread] * smoothstep(soft, -soft, fx.abs().max(fy.abs()) - 0.36);
        }
        let average = (THREAD[0] + THREAD[1] + THREAD[2] + THREAD[3]) * (0.25 * band.fill * 0.55);
        return average.lerp(color, resolved);
    }
    Vec3::ZERO
}

const DUST: Vec3 = Vec3::new(0.7, 0.71, 0.74);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let radius = f.local.length().max(1.0);
    let p = f.local / radius;
    let footprint = f.pos.length() * u.pixel_angle;
    // La foto de la Luna, gris y palida.
    let photo = u.textures.images.moon.sphere(p, footprint / (TAU * radius));
    let grey = photo.lerp(Vec3::splat(photo.luminance()), 0.75);
    let albedo = (grey * 1.35).min(Vec3::splat(0.9)) * DUST;
    let rock = detail::rock(f, u, 0.35, footprint, 0.35);
    let n = rock.normal;
    let l = f.light_dir(u);
    let v = f.view_dir();
    // Lommel-Seeliger: el disco lleno parejo hasta el borde.
    // Lambert ya multiplica por mu0: con la luz dividida entre mu0 + mu
    // queda Lommel-Seeliger (el 2.3 iguala el brillo de frente).
    let (mu0, mu) = (n.dot(l).max(0.0), n.dot(v).max(0.0));
    let light = u.sun_light.lerp(Vec3::splat(u.sun_light.luminance()), 0.5);
    let lunar = light * (2.3 * std::f32::consts::PI / 3.6 / (mu0 + mu).max(0.05));
    let lit = pbr::shade_light(f, u, albedo, 0.95, 0.0, n, rock.ao, lunar);
    // El huipil: brilla de noche; de dia es hilo grabado.
    let ground = f.object_to_world.mul_vec(p);
    let night = smoothstep(0.12, -0.2, ground.dot(l));
    let thread = huipil(p, footprint / radius);
    let etched = 1.0 - 0.3 * thread.luminance().min(1.0) * (1.0 - night);
    let glow = thread * ((0.03 + 0.5 * night) * u.gains.argos);
    lit * etched + glow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_moon_stays_in_range_and_is_deterministic() {
        let (a, b) = (Moon::new(9), Moon::new(9));
        let (lo, hi) = a.height_range();
        for i in 0..4000 {
            let t = i as f32 * 0.37;
            let dir = Vec3::new(t.sin() * (t * 0.31).cos(), (t * 0.77).cos(), t.cos() * (t * 0.13).sin()).normalize();
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
        }
    }

    /// El tejido cierra la vuelta sin costura y fuera de las franjas no hay
    /// nada.
    #[test]
    fn the_huipil_closes_around_and_stays_in_its_bands() {
        for band in &BANDS {
            let around = band.around as i32;
            for j in 0..band.rows as i32 {
                for i in 0..8 {
                    assert_eq!((band.motif)(i, j), (band.motif)(i + around, j), "{i} {j}");
                }
            }
        }
        assert_eq!(huipil(Vec3::Y, 0.001), Vec3::ZERO);
        assert_eq!(huipil(Vec3::new(0.0, 0.35, 1.0).normalize(), 0.001), Vec3::ZERO);
    }
}
