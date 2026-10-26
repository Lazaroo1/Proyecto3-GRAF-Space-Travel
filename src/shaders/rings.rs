//! Anillos de Cementerio: un disco analitico, no una malla.
//!
//! Por cada pixel se intersecta el rayo de la camara con el plano de los
//! anillos y se compara con la profundidad del visibility buffer: si el
//! anillo esta mas cerca que la superficie, se mezcla encima con su densidad
//! como opacidad; si esta detras, la superficie lo tapa. Asi los anillos son
//! semitransparentes sin ordenar triangulos.
//!
//! La densidad depende solo de la distancia al centro: bandas de ruido 1D,
//! bordes suaves y una division oscura. Con la misma funcion se calculan las
//! dos sombras: la de los anillos sobre el planeta y la del planeta sobre los
//! anillos.

use super::Uniforms;
use crate::math::{smoothstep, Perlin, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct RingInfo {
    /// Centro del planeta, relativo a la camara.
    pub center: Vec3,
    /// Normal del plano de los anillos.
    pub normal: Vec3,
    pub inner: f32,
    pub outer: f32,
    /// Radio del planeta, para su sombra sobre los anillos.
    pub planet_radius: f32,
}

const DUST: Vec3 = Vec3::new(0.62, 0.55, 0.48);
const DARK_DUST: Vec3 = Vec3::new(0.30, 0.26, 0.24);

/// Densidad de los anillos (0 a ~0.9) a la distancia `r` del centro.
pub fn density(ring: &RingInfo, r: f32, noise: &Perlin) -> f32 {
    let s = (r - ring.inner) / (ring.outer - ring.inner);
    if !(0.0..=1.0).contains(&s) {
        return 0.0;
    }
    let edges = smoothstep(0.0, 0.04, s) * smoothstep(1.0, 0.9, s);
    let division = 1.0 - 0.9 * smoothstep(0.02, 0.0, (s - 0.58).abs());
    let bands = noise.noise(Vec3::new(s * 37.0, 0.5, 0.5)) * 0.5 + 0.5;
    let fine = noise.noise(Vec3::new(s * 160.0, 1.5, 0.5)) * 0.5 + 0.5;
    (edges * division * (0.25 + 0.55 * bands + 0.2 * fine)).min(0.92)
}

/// Fraccion de luz que pasa a traves de los anillos desde `point` en la
/// direccion `to_light` (ambos relativos a la camara).
pub fn transmittance(ring: &RingInfo, point: Vec3, to_light: Vec3, noise: &Perlin) -> f32 {
    let denom = to_light.dot(ring.normal);
    if denom.abs() < 1e-6 {
        return 1.0;
    }
    let t = (ring.center - point).dot(ring.normal) / denom;
    if t <= 0.0 {
        return 1.0;
    }
    let hit = point + to_light * t;
    1.0 - density(ring, (hit - ring.center).length(), noise)
}

/// Mezcla los anillos sobre `color` si el rayo `dir` (unitario) los cruza
/// antes de `depth`.
pub fn composite(color: Vec3, dir: Vec3, depth: f32, ring: &RingInfo, u: &Uniforms) -> Vec3 {
    let denom = dir.dot(ring.normal);
    if denom.abs() < 1e-6 {
        return color;
    }
    let t = ring.center.dot(ring.normal) / denom;
    if t <= 0.0 || t >= depth {
        return color;
    }
    let hit = dir * t;
    let r = (hit - ring.center).length();
    let alpha = density(ring, r, u.noise);
    if alpha <= 0.0 {
        return color;
    }
    let to_sun = (u.sun_pos - hit).normalize();
    // Sombra del planeta: el rayo del punto hacia la estrella pega en la esfera.
    let oc = hit - ring.center;
    let b = oc.dot(to_sun);
    let c = oc.length_squared() - ring.planet_radius * ring.planet_radius;
    let shadowed = b < 0.0 && b * b - c > 0.0;
    let s = (r - ring.inner) / (ring.outer - ring.inner);
    let albedo = DARK_DUST.lerp(DUST, smoothstep(0.1, 0.9, alpha) * (0.7 + 0.3 * s));
    // El polvo dispersa hacia adelante: brilla mas a contraluz.
    let forward = dir.dot(to_sun).max(0.0).powi(6) * 1.5;
    let light = if shadowed { 0.02 } else { 0.35 + forward };
    color.lerp(albedo * u.sun_light * light, alpha)
}
