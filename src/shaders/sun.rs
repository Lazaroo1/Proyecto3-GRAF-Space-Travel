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
use crate::math::blackbody::planckian_rgb;
use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct SunStyle {
    /// Temperatura de la fotosfera en kelvin: decide el color.
    pub temperature: f32,
    /// Brillo de la superficie (HDR).
    pub intensity: f32,
}

/// Latido en `[0, 1]` aproximadamente: dos golpes por periodo (lub-dub).
pub fn heartbeat(t: f64, period: f64) -> f64 {
    let phase = (t / period).rem_euclid(1.0);
    let beat = |center: f64, width: f64, height: f64| {
        let x = (phase - center) / width;
        height * (-x * x).exp()
    };
    beat(0.1, 0.045, 1.0) + beat(0.3, 0.06, 0.55)
}

pub fn shade(f: &Fragment, u: &Uniforms, s: &SunStyle) -> Vec3 {
    let mu = f.normal.dot(f.view_dir()).max(0.0);
    let p = f.local.normalize();
    let t = u.time;

    let warp = Vec3::new(
        u.noise.noise(p * 1.7 + Vec3::new(0.0, t * 0.035, 0.0)),
        u.noise.noise(p * 1.7 + Vec3::new(5.2, -t * 0.03, 1.3)),
        u.noise.noise(p * 1.7 + Vec3::new(-3.1, 2.4, t * 0.025)),
    );
    let cells = u.noise.fbm(p * 3.2 + warp * 0.9, 4);
    let granules = u.noise.fbm(p * 14.0 + warp * 0.4 + Vec3::splat(t * 0.02), 3);
    let heat = (0.5 + 0.5 * (cells * 0.85 + granules * 0.25)).clamp(0.0, 1.0);

    let pulse = u.beat;
    let limb = 0.25 + 0.75 * mu.powf(0.6);
    let temperature =
        s.temperature * (0.82 + 0.3 * heat) * (0.88 + 0.12 * mu) * (1.0 + 0.04 * pulse);
    let brightness = s.intensity * (0.35 + 1.1 * heat * heat) * limb * (1.0 + 0.25 * pulse);
    planckian_rgb(temperature) * brightness
}
