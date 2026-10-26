//! Supergigante roja moribunda: celdas de conveccion gigantes que hierven,
//! oscurecimiento del limbo y un latido.
//!
//! # Celdas de conveccion
//! En una supergigante como Betelgeuse la superficie no tiene millones de
//! granulos como el Sol: tiene unas pocas celdas enormes de plasma caliente
//! que sube y frio que baja. Aqui son ruido fractal de baja frecuencia con
//! **domain warping**: antes de muestrear el ruido, el punto se desplaza con
//! otro ruido que cambia con el tiempo. Las celdas no se trasladan en bloque:
//! se deforman y se arrastran como plasma.
//!
//! # Limbo
//! En el borde del disco la linea de vision atraviesa capas mas altas y frias
//! de la estrella, asi que se ve mas oscuro y mas rojo.
//!
//! # Latido
//! El brillo, la temperatura y el radio (en `scene/system.rs`) siguen la
//! misma curva de doble golpe, como un corazon.

use super::{Fragment, Uniforms};
use crate::math::Vec3;
use crate::math::blackbody::planckian_rgb;

#[derive(Clone, Copy, Debug)]
pub struct SunStyle {
    /// Temperatura de la fotosfera en kelvin: decide el color.
    pub temperature: f32,
    /// Brillo de la superficie (HDR).
    pub intensity: f32,
}

/// Latido en `[0, 1]` aproximadamente: dos golpes por periodo (lub-dub).
pub fn heartbeat(t: f64, period: f64) -> f64 {
    heartbeat_phase(t / period)
}

/// El mismo latido en la fase `phase`, medida en latidos. El ciclo de OCASO
/// acelera el corazon, asi que la fase es la integral de la frecuencia y no
/// `t / periodo`.
pub fn heartbeat_phase(phase: f64) -> f64 {
    let phase = phase.rem_euclid(1.0);
    let beat = |center: f64, width: f64, height: f64| {
        let x = (phase - center) / width;
        height * (-x * x).exp()
    };
    beat(0.1, 0.045, 1.0) + beat(0.3, 0.06, 0.55)
}

pub fn shade(f: &Fragment, u: &Uniforms, s: &SunStyle) -> Vec3 {
    let p = f.local.normalize();
    let t = u.time;
    let warp = Vec3::new(
        u.noise.noise(p * 4.0 + Vec3::new(0.0, t * 0.03, 0.0)),
        u.noise.noise(p * 4.0 + Vec3::new(17.0, 2.0, t * 0.02)),
        u.noise.noise(p * 4.0 + Vec3::new(t * 0.02, 31.0, 8.0)),
    );
    let convection = u.noise.fbm(p * 9.0 + warp * 2.2, 5);
    let fine = u
        .noise
        .fbm(p * 90.0 + warp * 3.0 + Vec3::splat(t * 0.01), 4);
    let granules = 1.0 - u.noise.noise(p * 155.0 + warp * 1.5).abs();
    let spots = crate::math::smoothstep(0.16, 0.42, u.noise.noise(p * 7.0 + warp)) * 0.85;
    let heat = ((0.53 + convection * 0.95 + fine * 0.5) * (1.0 - spots)).clamp(0.0, 1.0);
    let mu = f.normal.dot(f.view_dir()).max(0.0);
    let limb = 0.18 + 0.82 * mu.powf(0.45);
    let temperature = (s.temperature * (0.28 + heat * 0.65)).clamp(1200.0, 7000.0);
    let energy = (0.025 + heat.powi(4) * 2.0) * (0.45 + granules * 0.85);
    planckian_rgb(temperature) * (energy * s.intensity * u.gains.sun * limb * (1.0 + 0.16 * u.beat))
}
