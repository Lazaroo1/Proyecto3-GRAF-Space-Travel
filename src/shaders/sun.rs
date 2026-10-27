//! Supergigante roja moribunda: una superficie de plasma que hierve, el
//! borde mas oscuro y rojo, y un corazon que late.
//!
//! # Granulacion
//! Tres escalas de celdas de conveccion: unas pocas enormes (como en
//! Betelgeuse), granulos medianos y un moteado fino. Los bordes de cada
//! celda (donde un ruido cruza por cero, `1 - |ruido|` cerca de 1) son el
//! plasma frio que baja: mas oscuro y rojo, nunca negro. Antes de muestrear,
//! el punto se desplaza con otro ruido que cambia con el tiempo (domain
//! warping): las celdas no se trasladan en bloque, se arrastran.
//!
//! # Latido
//! Cada latido sale de un punto de la superficie y la recorre como una onda
//! de brillo; ademas todo el disco se aviva con la curva de doble golpe.
//!
//! # Limbo y llamaradas
//! En el borde la linea de vision atraviesa capas mas altas y frias: mas
//! oscuro y mas rojo. En el estertor aparecen manchas que estallan de luz.

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
    let n = u.noise;
    let warp = Vec3::new(
        n.noise(p * 3.0 + Vec3::new(0.0, t * 0.025, 0.0)),
        n.noise(p * 3.0 + Vec3::new(17.0, 2.0, t * 0.02)),
        n.noise(p * 3.0 + Vec3::new(t * 0.02, 31.0, 8.0)),
    );
    let q = p + warp * 0.11;
    let smooth = crate::math::smoothstep;
    // Granulos: gotas de plasma caliente separadas por canales frios.
    let granule = smooth(-0.28, 0.32, n.noise(q * 17.0 + warp * 0.8));
    let fine = smooth(-0.3, 0.3, n.noise(q * 46.0 + warp * 1.4 + Vec3::splat(t * 0.03)));
    // Las celdas gigantes y sus bordes, mas frios.
    let giant = 1.0 - n.noise(q * 4.5 + Vec3::splat(t * 0.004)).abs();
    let giant = giant * giant * giant * giant;
    // Regiones calientes grandes que van y vienen.
    let hot = n.fbm(q * 1.6 + Vec3::new(t * 0.006, 0.0, 0.0), 3);
    let mut heat = 0.42 + 0.28 * hot + 0.26 * granule + 0.12 * fine - 0.22 * giant;

    // La onda del latido: nace en un punto y cruza la estrella.
    let origin = Vec3::new(0.48, 0.62, -0.62).normalize();
    let along = 1.0 - p.dot(origin);
    let front = u.beat_phase.fract() * 2.6;
    let ring = (-((along - front) * 6.0).powi(2)).exp() * (1.0 - u.beat_phase.fract());
    heat += 0.14 * ring * u.heart;

    // Llamaradas del estertor: parches que estallan de luz.
    if u.flares > 0.0 {
        let burst = n.noise(q * 3.2 + Vec3::new(0.0, t * 0.35, 0.0));
        heat += u.flares * crate::math::smoothstep(0.25, 0.6, burst) * 0.45;
    }
    let heat = heat.clamp(0.0, 1.3);

    let mu = f.normal.dot(f.view_dir()).max(0.0);
    let limb = 0.05 + 0.95 * mu.powf(1.1);
    // Color de cuerpo negro entre 1700 y 2700 K: a 3400 K el sRGB lo veria
    // color durazno; asi se lee como la brasa naranja de una gigante roja.
    let temperature =
        (s.temperature * (0.50 + 0.36 * heat) * (0.70 + 0.30 * mu)).clamp(1500.0, 9000.0);
    let energy = (0.05 + 0.75 * heat * heat * heat) * limb;
    planckian_rgb(temperature) * (energy * s.intensity * u.gains.sun * (1.0 + 0.16 * u.beat))
}
