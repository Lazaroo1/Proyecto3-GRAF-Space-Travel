//! La columna de ceniza del Volcan de Fuego.
//!
//! El Fuego no echa humo parejo: cada explosion (las mismas que encienden
//! el crater y tiran bombas en `fuego::fire`) suelta una **bocanada** que
//! sube rapido, se frena, se infla y el viento se la lleva de lado. Las
//! bocanadas de las ultimas explosiones forman la columna: una fila de
//! coliflores cada vez mas grandes y palidas que se tuerce con el viento.
//!
//! Abajo, un **tallo** de gas que sale siempre de la boca y se abre al
//! subir une el crater con las bocanadas.
//!
//! Las bocanadas y la esfera que las envuelve se calculan una vez por frame
//! (`Plume::new`): la pasada de volumenes corre para todos los pixeles de
//! la pantalla y la mayoria solo hace la prueba contra la esfera.
//!
//! Es un volumen: en la pasada de volumenes cada pixel cuyo rayo cruza la
//! esfera que envuelve la columna la marcha de adelante hacia atras (rayo
//! recto, en unidades del mundo). En cada muestra la densidad es la de la
//! bocanada que mas tapa: una esfera difusa cuyo borde se come un ruido
//! fbm que sube con ella (los bordes de coliflor). La luz es la de la
//! estrella sobre el lado de cada bocanada que le da (una esfera iluminada,
//! sin marchar sombras), mas el cielo; justo despues de cada explosion la
//! base brilla al rojo con la lava.

use crate::math::blackbody::planckian_rgb;
use crate::math::{Perlin, Vec3, smoothstep};

use super::fuego;

/// Cuantas bocanadas siguen en el aire (las de las ultimas explosiones).
const PUFFS: usize = 8;
/// Segundos que dura una bocanada antes de deshacerse.
const LIFE: f32 = 32.0;
/// Lo alto que sube una bocanada (rapido al principio) y lo que sigue
/// subiendo despues, por segundo.
const RISE: f32 = 15.0;
const DRIFT_UP: f32 = 0.9;
/// Lo que se la lleva el viento, por segundo.
const WIND: f32 = 1.3;
/// Alto del tallo de gas sobre la boca.
const STEM: f32 = 14.0;
/// Opacidad por unidad recorrida, a densidad 1.
const KAPPA: f32 = 1.1;
/// Paso de la marcha y cuantas muestras como mucho.
const STEP: f32 = 2.4;
const MAX_STEPS: usize = 32;
/// Color de la ceniza (lineal, en la escala de luz de la version chapina).
const ASH: Vec3 = Vec3::new(0.022, 0.02, 0.0185);

#[derive(Clone, Copy, Debug)]
pub struct Plume {
    /// La boca del crater, relativa a la camara.
    mouth: Vec3,
    /// Vertical del crater (unitaria).
    up: Vec3,
    /// Hacia la estrella y la luz que llega.
    sun_dir: Vec3,
    light: Vec3,
    time: f32,
    /// Las bocanadas en el aire (relativas a la boca) y la esfera que
    /// envuelve todo (relativa a la camara).
    puffs: [Puff; PUFFS],
    middle: Vec3,
    reach: f32,
}

/// Una bocanada ya ubicada.
#[derive(Clone, Copy, Debug)]
struct Puff {
    center: Vec3,
    radius: f32,
    /// Cuanta ceniza tiene (se deshace con la edad).
    amount: f32,
    /// Lo caliente de la base, recien salida.
    heat: f32,
    /// Desfase del ruido de cada una.
    seed: f32,
}

impl Plume {
    /// La columna en el instante `time`: la boca relativa a la camara, su
    /// vertical y el viento (unitarios), hacia la estrella y la luz que le
    /// llega.
    pub fn new(mouth: Vec3, up: Vec3, wind: Vec3, sun_dir: Vec3, light: Vec3, time: f32) -> Self {
        let puffs = Self::puffs(up, wind, time);
        // La esfera que envuelve todas las bocanadas y el tallo.
        let (mut low, mut high) = (Vec3::splat(-4.0), up * STEM + Vec3::splat(4.0));
        (low, high) = (low.min(high), low.max(high));
        for p in puffs.iter().filter(|p| p.amount > 0.0) {
            low = low.min(p.center - Vec3::splat(p.radius * 1.3));
            high = high.max(p.center + Vec3::splat(p.radius * 1.3));
        }
        Self {
            mouth,
            up,
            sun_dir,
            light,
            time,
            puffs,
            middle: mouth + (low + high) * 0.5,
            reach: (high - low).length() * 0.5,
        }
    }

    /// Las bocanadas en el aire, relativas a la boca.
    fn puffs(up: Vec3, wind: Vec3, t: f32) -> [Puff; PUFFS] {
        let mut out = [Puff {
            center: Vec3::ZERO,
            radius: 1.0,
            amount: 0.0,
            heat: 0.0,
            seed: 0.0,
        }; PUFFS];
        let latest = fuego::burst_index(t);
        for (j, puff) in out.iter_mut().enumerate() {
            let k = latest - j as f32;
            let (born, strength) = fuego::burst(k);
            let age = t - born;
            if !(0.0..LIFE).contains(&age) {
                continue;
            }
            let rise = RISE * (0.6 + 0.6 * strength) * (1.0 - (-age / 4.0).exp()) + DRIFT_UP * age;
            let drift = WIND * age + 0.02 * age * age;
            *puff = Puff {
                center: up * (1.5 + rise) + wind * drift,
                radius: 3.0 + 2.6 * age.powf(0.6) * (0.7 + 0.5 * strength),
                amount: strength * smoothstep(0.0, 0.6, age) * smoothstep(LIFE, LIFE * 0.45, age),
                heat: (-age / 1.2).exp() * strength,
                seed: k * 3.7,
            };
        }
        out
    }

    /// Mezcla la columna sobre `color` en la direccion `dir` (unitaria)
    /// antes de `depth`.
    pub fn apply(&self, color: Vec3, dir: Vec3, depth: f32, noise: &Perlin) -> Vec3 {
        if sphere(self.middle, self.reach, dir, depth).is_none() {
            return color;
        }
        // Las bocanadas que cruza este rayo y el tramo donde hay algo: asi
        // no se marcha el vacio entre ellas ni se revisan las que no toca.
        let mut near = [0usize; PUFFS];
        let mut count = 0;
        let (mut t0, mut t1) = (f32::MAX, 0.0f32);
        for (i, p) in self.puffs.iter().enumerate().filter(|(_, p)| p.amount > 0.0) {
            if let Some((a, b)) = sphere(self.mouth + p.center, p.radius * 1.4, dir, depth) {
                near[count] = i;
                count += 1;
                (t0, t1) = (t0.min(a), t1.max(b));
            }
        }
        let stem_reach = STEM * 0.5 + (1.1 + 0.28 * STEM) * 1.4;
        let stem = sphere(self.mouth + self.up * (STEM * 0.5), stem_reach, dir, depth);
        if let Some((a, b)) = stem {
            (t0, t1) = (t0.min(a), t1.max(b));
        }
        if t1 <= t0 {
            return color;
        }
        let near = &near[..count];
        let steps = (((t1 - t0) / STEP).ceil() as usize).clamp(6, MAX_STEPS);
        let dt = (t1 - t0) / steps as f32;
        let sky = self.light * 0.22 + Vec3::new(0.012, 0.016, 0.024);
        let glow = planckian_rgb(1050.0);
        let mut transmittance = 1.0f32;
        let mut light = Vec3::ZERO;
        for k in 0..steps {
            let x = dir * (t0 + (k as f32 + 0.5) * dt) - self.mouth;
            let (density, lit, heat) = self.density(near, stem.is_some(), x, noise);
            if density <= 0.0 {
                continue;
            }
            let alpha = 1.0 - (-density * KAPPA * dt).exp();
            let source = ASH * (self.light * lit + sky) + glow * (heat * 0.6);
            light += source * (alpha * transmittance);
            transmittance *= 1.0 - alpha;
            if transmittance < 0.02 {
                break;
            }
        }
        color * transmittance + light
    }

    /// Densidad en `x` (relativo a la boca), cuanto le da la estrella ahi
    /// (0 a 1) y lo caliente.
    #[inline]
    fn density(&self, near: &[usize], with_stem: bool, x: Vec3, noise: &Perlin) -> (f32, f32, f32) {
        let height = x.dot(self.up);
        // El tallo: un cono que se abre al subir y se deshace arriba.
        let mut stem = None;
        if with_stem && (-1.0..STEM).contains(&height) {
            let radius = 1.1 + 0.28 * height.max(0.0);
            let r = (x - self.up * height).length() / radius;
            if r < 1.4 {
                stem = Some((r, radius, 0.85 * smoothstep(STEM, STEM * 0.4, height)));
            }
        }
        // La bocanada que mas tapa aqui (sin ruido todavia).
        let mut best: Option<(&Puff, f32)> = None;
        for p in near.iter().map(|&i| &self.puffs[i]) {
            let r = (x - p.center).length() / p.radius;
            if r < 1.4 && best.is_none_or(|(_, b)| r < b) {
                best = Some((p, r));
            }
        }
        let (center, r, radius, amount, heat, seed) = match (best, stem) {
            (Some((_, r)), Some((rs, radius, a))) if rs < r => (self.up * height, rs, radius, a, 0.35, 0.0),
            (Some((p, r)), _) => (p.center, r, p.radius, p.amount, p.heat, p.seed),
            (None, Some((rs, radius, a))) => (self.up * height, rs, radius, a, 0.35, 0.0),
            (None, None) => return (0.0, 0.0, 0.0),
        };
        // El borde de coliflor: un ruido que sube con el humo. Adentro de la
        // bocanada todo es ceniza y no hace falta.
        let q = (x - center) / radius;
        let billow = if r < 0.3 {
            0.0
        } else {
            let rising = Vec3::new(seed, seed * 0.7, 0.0) - self.up * (self.time * 0.12);
            noise.fbm(q * 2.6 + x * 0.08 + rising, 2)
        };
        let edge = r + 0.42 * billow;
        let density = smoothstep(1.0, 0.78, edge) * amount;
        if density <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        // El lado que le da la estrella; adentro, mas oscuro.
        let facing = q.dot(self.sun_dir) / r.max(0.2) + 0.3 * billow;
        let lit = smoothstep(-0.6, 0.8, facing) * (0.3 + 0.7 * smoothstep(0.25, 1.0, r));
        let base = smoothstep(5.0, 0.0, height);
        (density, lit, heat * base * density)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La boca 200 unidades enfrente de la camara, con la vertical en +y.
    fn column(time: f32) -> Plume {
        let sun = Vec3::new(0.0, 1.0, 1.0).normalize();
        Plume::new(Vec3::new(0.0, 0.0, -200.0), Vec3::Y, Vec3::X, sun, Vec3::ONE, time)
    }

    #[test]
    fn the_column_only_covers_what_is_behind_it() {
        let noise = Perlin::new(1);
        let sky = Vec3::splat(0.5);
        for time in [3.0, 40.0, 117.0] {
            let plume = column(time);
            // Un rayo hacia el otro lado no la cruza.
            assert_eq!(plume.apply(sky, Vec3::Z, f32::INFINITY, &noise), sky);
            // Justo arriba de la boca el tallo tapa casi todo.
            let stem = Vec3::new(0.0, 6.0, -200.0).normalize();
            let through = plume.apply(sky, stem, f32::INFINITY, &noise);
            assert!(through.x < sky.x * 0.5, "{time}: {through:?}");
            // Lo que esta antes de la columna se ve igual.
            assert_eq!(plume.apply(sky, stem, 150.0, &noise), sky);
        }
    }
}

/// Donde el rayo `dir` (unitario, desde la camara) entra y sale de la
/// esfera de centro `center` (relativo a la camara) y radio `radius`, antes
/// de `depth`.
#[inline]
fn sphere(center: Vec3, radius: f32, dir: Vec3, depth: f32) -> Option<(f32, f32)> {
    let b = center.dot(dir);
    let disc = b * b - (center.length_squared() - radius * radius);
    if disc <= 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let (t0, t1) = ((b - root).max(0.0), (b + root).min(depth));
    (t1 > t0).then_some((t0, t1))
}
