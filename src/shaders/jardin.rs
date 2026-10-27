//! Terreno vivo: quitina erosionada, membranas hundidas y luz interna.
//!
//! - **Arterias**: las curvas donde un ruido de Perlin vale cero forman
//!   sobre la esfera una red cerrada (la misma que levanta el terreno). Se
//!   dibujan por pixel con antialiasing: cuando un pixel cubre mas que el
//!   ancho de la linea, la linea se ensancha y se atenua en proporcion, asi
//!   conserva su brillo total y no parpadea desde la orbita.
//! - **Pulso**: las arterias laten con el ritmo de la estrella, pero cada
//!   lugar recibe el latido con un retraso que cambia suave sobre la
//!   superficie: se ve una onda que recorre el planeta. Brillan mas de noche
//!   y cuando la estrella se apaga.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::{Perlin, Vec3, smoothstep};
use crate::terrain::shapes::{ARTERY_FREQUENCY, artery};

const VEIN_CYAN: Vec3 = Vec3::new(0.10, 0.95, 0.80);
const VEIN_VIOLET: Vec3 = Vec3::new(0.55, 0.25, 0.95);

/// Mascara de una linea donde `value` se acerca a 1, con `width` de ancho y
/// `aa` de desenfoque (los dos en unidades de `value`). Al crecer `aa` la
/// linea se ensancha y baja su pico: su brillo total se mantiene.
#[inline]
fn line(value: f32, width: f32, aa: f32) -> f32 {
    smoothstep(1.0 - width - aa, 1.0 - width + aa, value) * (width / (width + aa)).min(1.0)
}
use std::f32::consts::TAU;
pub fn breathe(local: Vec3, time: f32, noise: &Perlin) -> f32 {
    ((time * TAU / 7.0).sin() * 0.5 + 0.5)
        * 0.7
        * (0.3 + 0.7 * (noise.noise(local * 0.02) * 0.5 + 0.5))
}
pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let stone = detail::rock(f, u, 0.16, footprint, 0.7);
    let tissue = detail::sample(f, &u.textures.organic, 0.035, footprint, 0.32);
    let grain = u.noise.fbm(f.local * 0.4, 3) * 0.5 + 0.5;
    let fold = f.color.z;
    let pore = f.color.y;
    // Tejido verde azulado: de la roca solo se toma el relieve de su brillo.
    let albedo =
        Vec3::new(0.20, 0.36, 0.31) * (stone.color.luminance() * (0.35 + grain * 0.65) * 1.1);
    let cavity = (stone.ao * (0.45 + 0.55 * tissue.height) * (1.0 - 0.55 * pore)).clamp(0.08, 1.0);
    let n = (stone.normal * 0.7 + tissue.normal * 0.3).normalize();
    let rough = 0.29 + 0.34 * stone.rough;
    let lit = pbr::shade(f, u, albedo, rough, 0.08, n, cavity);
    // La luz aparece bajo una piel opaca, con bordes rotos y zonas apagadas.
    let network = tissue.glow.powi(7) * smoothstep(0.38, 0.72, grain);
    let pulse = 0.65 + 0.35 * u.beat_at(fold * 1.4);
    let glow = Vec3::new(0.012, 0.42, 0.26)
        * (network * (0.05 + 0.10 * fold) + pore * 0.055)
        * u.biolum
        * pulse;
    // Arterias que laten, con el pulso viajando por el planeta.
    let dir = f.local.normalize();
    let step = footprint / f.local.length().max(1.0) * 1.5;
    let a = artery(u.noise, dir);
    let halo = smoothstep(0.97, 1.0, a);
    let arteries = line(a, 0.008, (step * ARTERY_FREQUENCY).max(0.003)) + halo * halo * 0.08;
    // Capilares: una red mas fina que se ramifica entre las arterias.
    const CAPILLARY_FREQUENCY: f32 = 8.0;
    let capillary = 1.0 - u.noise.noise(dir * CAPILLARY_FREQUENCY + Vec3::new(4.0, 9.0, -2.0)).abs();
    let arteries = arteries + 0.45 * line(capillary, 0.02, (step * CAPILLARY_FREQUENCY).max(0.004));
    let delay = (u.noise.noise(dir * 1.2) * 0.5 + 0.5) * 2.5;
    let beat = u.beat_at(delay);
    let night = smoothstep(0.15, -0.35, f.normal.dot(f.light_dir(u)));
    let vein = VEIN_CYAN.lerp(VEIN_VIOLET, smoothstep(0.45, 0.55, fold));
    let veins = vein * (arteries * (0.25 + 1.6 * beat) * (0.35 + 0.65 * night) * u.biolum);
    lit + glow + veins * u.gains.jardin
}
