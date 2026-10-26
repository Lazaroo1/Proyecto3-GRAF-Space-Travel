//! Planeta rocoso con el terreno pintado por ruido fractal.
//!
//! La altura sale de ruido evaluado en la posicion **del objeto**, asi que el
//! patron gira con el planeta. Por ahora la esfera es lisa y el relieve solo
//! existe en el color; la deformacion real de la malla viene despues.

use super::{Fragment, Uniforms};
use crate::math::{smoothstep, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct PlanetStyle {
    /// Desplaza el ruido: cada planeta tiene un terreno distinto.
    pub seed: Vec3,
    pub frequency: f32,
    /// Altura (en `[-1, 1]`) por debajo de la cual estan las llanuras bajas.
    pub sea_level: f32,
    pub deep: Vec3,
    pub shallow: Vec3,
    pub shore: Vec3,
    pub lowland: Vec3,
    pub highland: Vec3,
    pub peak: Vec3,
    pub polar: Vec3,
    pub atmosphere: Vec3,
    /// Brillo especular de las llanuras bajas (agua o hielo).
    pub sea_gloss: f32,
}

impl PlanetStyle {
    /// Requiem: mundo helado y muerto. Llanuras de hielo donde hubo mares,
    /// basalto oscuro y escarcha en las cumbres.
    pub const REQUIEM: Self = Self {
        seed: Vec3::new(13.7, -4.1, 8.9),
        frequency: 1.9,
        sea_level: -0.05,
        deep: Vec3::new(0.32, 0.4, 0.5),
        shallow: Vec3::new(0.62, 0.72, 0.8),
        shore: Vec3::new(0.42, 0.42, 0.44),
        lowland: Vec3::new(0.2, 0.19, 0.2),
        highland: Vec3::new(0.12, 0.11, 0.12),
        peak: Vec3::new(0.85, 0.88, 0.92),
        polar: Vec3::new(0.9, 0.93, 0.97),
        atmosphere: Vec3::new(0.35, 0.28, 0.5),
        sea_gloss: 0.35,
    };
}

pub fn shade(f: &Fragment, u: &Uniforms, s: &PlanetStyle) -> Vec3 {
    let p = f.local.normalize();
    let height = u.noise.fbm(p * s.frequency + s.seed, 6);
    let n = f.normal;
    let v = f.view_dir();
    let l = f.light_dir(u);
    let ndl = n.dot(l);

    let (mut albedo, gloss) = if height < s.sea_level {
        let depth = smoothstep(s.sea_level - 0.35, s.sea_level, height);
        (s.deep.lerp(s.shallow, depth * depth), s.sea_gloss)
    } else {
        let e = (height - s.sea_level) / (1.0 - s.sea_level);
        let mut c = s.shore.lerp(s.lowland, smoothstep(0.0, 0.08, e));
        c = c.lerp(s.highland, smoothstep(0.18, 0.45, e));
        c = c.lerp(s.peak, smoothstep(0.5, 0.75, e));
        (c, 0.0)
    };
    // Casquetes polares con borde irregular.
    let latitude = p.y.abs() + 0.08 * u.noise.noise(p * 9.0 + s.seed);
    albedo = albedo.lerp(s.polar, smoothstep(0.72, 0.8, latitude));

    let diffuse = albedo * u.sun_light * ndl.max(0.0);
    let specular = if ndl > 0.0 && gloss > 0.0 {
        let half = (l + v).normalize();
        u.sun_light * (gloss * n.dot(half).max(0.0).powf(60.0))
    } else {
        Vec3::ZERO
    };
    let ambient = albedo * 0.012;
    // Borde atmosferico: cerca del limbo se atraviesa mas atmosfera, y solo
    // brilla del lado de dia (con un poco de luz pasando el terminador).
    let rim = (1.0 - n.dot(v).max(0.0)).powi(3);
    let day = smoothstep(-0.2, 0.35, ndl);
    diffuse + specular + ambient + s.atmosphere * (rim * day * 0.9)
}
