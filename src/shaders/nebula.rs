//! La capa de gas que la estrella suelta en el estertor: una cascara
//! esferica que se expande y se enfria.
//!
//! No se marcha un volumen: cada rayo corta la esfera en a lo mas dos
//! puntos, y ahi se evaluan los filamentos (donde un ruido cruza por cero,
//! `1 - |ruido|` cerca de 1, deformado por otro ruido). La luz es la que
//! emite el gas a lo largo del tramo que el rayo pasa dentro de la cascara:
//! su grosor dividido entre el coseno con la normal. Por eso el borde, que
//! se ve de canto, brilla mas, como en las nebulosas de verdad.

use crate::math::{Perlin, Vec3, smoothstep};

#[derive(Clone, Copy, Debug)]
pub struct Nebula {
    /// Centro relativo a la camara.
    pub center: Vec3,
    pub radius: f32,
    /// Brillo: sube al soltarse y se apaga al enfriarse.
    pub glow: f32,
    /// Del naranja recien soltado (0) al rojo y magenta (1).
    pub cooled: f32,
    /// Segundos desde que se solto: los filamentos se retuercen despacio.
    pub age: f32,
}

impl Nebula {
    /// Cuanto dura a la vista, en segundos.
    const LIFE: f32 = 70.0;

    /// La cascara `age` segundos despues de soltarse, alrededor de una
    /// estrella de radio `base` en `center`.
    pub fn at(age: f32, center: Vec3, base: f32) -> Option<Self> {
        if !(0.0..Self::LIFE).contains(&age) {
            return None;
        }
        Some(Self {
            center,
            radius: base * (1.04 + 0.32 * age),
            glow: smoothstep(0.0, 2.0, age) * (-age / 20.0).exp(),
            cooled: smoothstep(0.0, 16.0, age),
            age,
        })
    }

    /// Luz de la cascara en la direccion `dir` (unitaria) antes de `depth`.
    pub fn light(&self, dir: Vec3, depth: f32, noise: &Perlin) -> Vec3 {
        let b = dir.dot(self.center);
        let c = self.center.length_squared() - self.radius * self.radius;
        let disc = b * b - c;
        if disc <= 0.0 {
            return Vec3::ZERO;
        }
        let root = disc.sqrt();
        let young = Vec3::new(1.5, 0.5, 0.12);
        let old = Vec3::new(1.0, 0.09, 0.18);
        let color = young.lerp(old, self.cooled);
        let mut light = Vec3::ZERO;
        for hit in [b - root, b + root] {
            if hit <= 0.0 || hit >= depth {
                continue;
            }
            let normal = (dir * hit - self.center) / self.radius;
            // Tramo dentro de la cascara, en grosores.
            let path = (1.0 / normal.dot(dir).abs().max(0.1)).min(8.0);
            let q = normal * 2.0 + Vec3::splat(self.age * 0.01);
            let warp = noise.noise(q * 0.9);
            let ridge = |x: f32, k: i32| (1.0 - x.abs()).powi(k);
            let filaments = ridge(noise.noise(q * 1.5 + Vec3::splat(warp * 1.2)), 9)
                + 0.45 * ridge(noise.noise(q * 4.2 + Vec3::splat(warp * 2.0)), 12);
            // Grumos grandes: hay regiones casi vacias y otras densas.
            let clumps = smoothstep(-0.25, 0.45, noise.noise(q * 0.55 + Vec3::new(9.0, 3.0, 1.0)));
            let haze = (0.5 + 0.5 * noise.fbm(q * 1.4 + Vec3::new(4.0, 1.0, 7.0), 3)).powi(2);
            let density = clumps * (filaments * 0.8 + haze * 0.35);
            light += color * (density * path * self.glow * 0.14);
        }
        light
    }
}
