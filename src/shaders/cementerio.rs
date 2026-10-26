//! Cementerio: roca muerta marcada por los impactos del exodo.
//!
//! Piedra gris con manchas de oxido, eyecta clara alrededor de los crateres
//! y zanjas quemadas donde se arrastraron las naves. En las zanjas brillan
//! fragmentos de chatarra: microfacetas al azar que solo destellan cuando
//! reflejan la estrella hacia la camara. Los anillos le hacen sombra.

use super::rings;
use super::{Fragment, Uniforms};
use crate::math::{hash_u32, smoothstep, Vec3};

const STONE: Vec3 = Vec3::new(0.21, 0.19, 0.18);
const RUST: Vec3 = Vec3::new(0.34, 0.15, 0.07);
const EJECTA: Vec3 = Vec3::new(0.40, 0.37, 0.34);
const SCORCH: Vec3 = Vec3::new(0.05, 0.045, 0.045);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (rim, gouge, _base) = (f.color.x, f.color.y, f.color.z);
    let n = f.normal;
    let v = f.view_dir();
    let l = f.light_dir(u);
    let ndl = n.dot(l);

    let rust = smoothstep(0.55, 0.75, u.noise.fbm(f.local * 0.03, 3) * 0.5 + 0.5);
    let mut albedo = STONE.lerp(RUST, rust * 0.7);
    albedo = albedo.lerp(EJECTA, rim * 0.6);
    albedo = albedo.lerp(SCORCH, gouge * 0.85);

    let shadow = match &u.ring {
        Some(ring) if ndl > 0.0 => rings::transmittance(ring, f.pos, l, u.noise),
        _ => 1.0,
    };
    let lit = albedo * u.sun_light * (ndl.max(0.0) * shadow) + albedo * 0.012;

    // Chatarra: cada celda de ~0.7 unidades tiene una normal al azar; si
    // refleja la estrella hacia la camara, destella.
    let footprint = f.pos.length() * u.pixel_angle;
    let mut glint = Vec3::ZERO;
    if gouge > 0.1 && footprint < 0.6 && ndl > 0.0 {
        let cell = (f.local * 1.4).floor();
        let h = hash_u32(
            (cell.x as i32 as u32).wrapping_mul(0x8DA6_B343)
                ^ (cell.y as i32 as u32).wrapping_mul(0xD816_3841)
                ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F),
        );
        let jitter = Vec3::new(
            (h & 0xFF) as f32 / 255.0 - 0.5,
            ((h >> 8) & 0xFF) as f32 / 255.0 - 0.5,
            ((h >> 16) & 0xFF) as f32 / 255.0 - 0.5,
        );
        let facet = (n + jitter * 1.2).normalize();
        let half = (l + v).normalize();
        glint = u.sun_light * (facet.dot(half).max(0.0).powi(64) * gouge * shadow * 3.0 * u.gains.cementerio);
    }
    lit + glint
}
