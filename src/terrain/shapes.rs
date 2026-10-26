//! Funciones de altura de los planetas.
//!
//! Todas reciben una direccion unitaria en el espacio del planeta y devuelven
//! la altura en unidades del mundo mas unos atributos para el shader. Son
//! deterministas: la misma semilla da exactamente el mismo planeta.

use super::{HeightField, TerrainSample};
use crate::math::{hash_u32, smoothstep, Perlin, Vec3};

/// Campo de crateres: el espacio se divide en celdas cubicas y cada celda
/// tiene, con cierta probabilidad, un crater con centro y tamano al azar.
///
/// Un punto solo necesita revisar su celda y las 26 vecinas: el radio de
/// influencia de un crater nunca pasa de una celda. Es la misma idea que el
/// ruido celular (Worley), con un perfil de crater en vez de una distancia.
pub struct Craters {
    /// Celdas por unidad de radio del planeta.
    pub frequency: f32,
    /// Probabilidad de que una celda tenga crater.
    pub chance: f32,
    /// Radios minimo y maximo, en celdas (el maximo no debe pasar de 0.6).
    pub min_radius: f32,
    pub max_radius: f32,
    /// Profundidad del crater mas grande, en unidades del mundo.
    pub depth: f32,
    pub seed: u32,
}

impl Craters {
    /// Altura que agregan los crateres en `dir` y cuanto borde de crater hay
    /// ahi (0 a 1, para el shader).
    pub fn sample(&self, dir: Vec3) -> (f32, f32) {
        let q = dir * self.frequency;
        let base = (q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32);
        let mut height = 0.0;
        let mut rim = 0.0f32;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let cell = (base.0 + dx, base.1 + dy, base.2 + dz);
                    let h = cell_hash(cell, self.seed);
                    if unit(h) >= self.chance {
                        continue;
                    }
                    let h1 = hash_u32(h);
                    let h2 = hash_u32(h1);
                    let h3 = hash_u32(h2);
                    let h4 = hash_u32(h3);
                    let center = Vec3::new(
                        cell.0 as f32 + unit(h1),
                        cell.1 as f32 + unit(h2),
                        cell.2 as f32 + unit(h3),
                    );
                    // Mas crateres chicos que grandes.
                    let size = unit(h4);
                    let r = self.min_radius + (self.max_radius - self.min_radius) * size * size;
                    let d = (q - center).length() / r;
                    if d >= 1.6 {
                        continue;
                    }
                    // Cuenco parabolico con fondo plano y un anillo elevado
                    // en el borde que se desvanece hacia afuera.
                    let ring = (-((d - 1.0) / 0.22).powi(2)).exp();
                    let shape = (d * d - 1.0).clamp(-0.6, 0.0) + 0.35 * ring;
                    let scale = r / self.max_radius;
                    height += shape * self.depth * scale;
                    rim = rim.max(ring * scale);
                }
            }
        }
        (height, rim)
    }

    /// Altura minima y maxima que pueden sumar (dos crateres encimados).
    pub fn range(&self) -> (f32, f32) {
        (-1.2 * self.depth, 0.7 * self.depth)
    }
}

#[inline]
fn cell_hash(cell: (i32, i32, i32), seed: u32) -> u32 {
    hash_u32(
        (cell.0 as u32).wrapping_mul(0x8DA6_B343)
            ^ (cell.1 as u32).wrapping_mul(0xD816_3841)
            ^ (cell.2 as u32).wrapping_mul(0xCB1A_B31F)
            ^ seed,
    )
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Requiem: mundo helado y muerto. Los mares se congelaron en llanuras
/// planas; tierra adentro se levantan cordilleras de crestas afiladas y
/// todo esta marcado por crateres.
///
/// Atributos: `x` hielo (llanuras congeladas), `y` nieve en las alturas,
/// `z` borde de crater.
pub struct RequiemField {
    noise: Perlin,
    craters: Craters,
    small: Craters,
}

impl RequiemField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            craters: Craters {
                frequency: 7.0,
                chance: 0.35,
                min_radius: 0.2,
                max_radius: 0.6,
                depth: 4.0,
                seed: seed ^ 0x51ED,
            },
            small: Craters {
                frequency: 24.0,
                chance: 0.25,
                min_radius: 0.15,
                max_radius: 0.5,
                depth: 1.2,
                seed: seed ^ 0xC0FE,
            },
        }
    }
}

impl HeightField for RequiemField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let continent = n.fbm(dir * 1.3 + Vec3::new(3.1, -7.2, 1.7), 4);
        let land = smoothstep(-0.02, 0.14, continent);
        // Crestas al cubo: picos afilados y valles anchos, solo tierra adentro.
        let ridges = n.ridged(dir * 3.4 + Vec3::new(-4.4, 2.2, 9.1), 5);
        let mountains = ridges * ridges * ridges * smoothstep(0.08, 0.4, continent);
        let detail = n.fbm(dir * 16.0 + Vec3::new(1.3, 5.5, -2.7), 3);
        let (big, big_rim) = self.craters.sample(dir);
        let (small, small_rim) = self.small.sample(dir);
        let relief = land * (1.5 + 1.2 * detail) + mountains * 13.0;
        // El hielo rellena a medias los crateres de las llanuras.
        let craters = (big + small) * (0.35 + 0.65 * land);
        let height = relief + craters + (1.0 - land) * 0.15 * detail;
        TerrainSample {
            height,
            attrs: Vec3::new(1.0 - land, smoothstep(6.0, 10.0, relief), big_rim.max(small_rim)),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        let (lo_a, hi_a) = self.craters.range();
        let (lo_b, hi_b) = self.small.range();
        (lo_a + lo_b - 0.3, 1.5 + 1.2 + 13.0 + hi_a + hi_b + 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requiem_stays_inside_its_declared_range_and_is_deterministic() {
        let a = RequiemField::new(7);
        let b = RequiemField::new(7);
        let (lo, hi) = a.height_range();
        for i in 0..4000 {
            let t = i as f32 * 0.37;
            let dir = Vec3::new(t.sin() * (t * 0.31).cos(), (t * 0.77).cos(), t.cos() * (t * 0.13).sin()).normalize();
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
        }
    }

    #[test]
    fn craters_are_continuous() {
        let craters = Craters { frequency: 7.0, chance: 0.5, min_radius: 0.2, max_radius: 0.6, depth: 4.0, seed: 3 };
        for i in 0..20_000 {
            let t = i as f32 * 0.0013;
            let dir = Vec3::new(t.cos(), (t * 3.1).sin() * 0.4, t.sin()).normalize();
            let next = Vec3::new((t + 1e-4).cos(), ((t + 1e-4) * 3.1).sin() * 0.4, (t + 1e-4).sin()).normalize();
            let (a, _) = craters.sample(dir);
            let (b, _) = craters.sample(next);
            assert!((a - b).abs() < 0.05, "salto de {a} a {b}");
        }
    }
}
