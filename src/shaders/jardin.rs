//! Terreno vivo: quitina erosionada, membranas hundidas y luz interna.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::{Perlin, Vec3, smoothstep};
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
    let albedo = stone.color * Vec3::new(0.26, 0.34, 0.29) * (0.35 + grain * 0.65);
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
    lit + glow
}
