//! Requiem: un mundo helado. Los crateres salen del mapa de la Luna (NASA
//! LROC) usado solo como relieve; el color es propio: roca azul oscura,
//! escarcha casi en todo y mares de hielo lisos y brillantes.
//!
//! Lo vivo: bajo los mares de hielo corre una red de grietas con luz fria
//! (donde dos celdas de Voronoi se tocan) que respira despacio, mas visible
//! de noche, y despues del colapso se enciende como las agujas.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::{Vec3, smoothstep};

const ROCK: Vec3 = Vec3::new(0.085, 0.10, 0.13);
const FROST: Vec3 = Vec3::new(0.72, 0.80, 0.92);
const ICE: Vec3 = Vec3::new(0.24, 0.42, 0.62);
const CRACK_LIGHT: Vec3 = Vec3::new(0.14, 0.48, 1.0);

/// Tan lejos de la estrella, su luz llega tenue y casi sin color: el hielo
/// se lee frio y no anaranjado. Tambien la usan las agujas.
pub fn cold_light(sun: Vec3) -> Vec3 {
    let gray = Vec3::new(0.82, 0.9, 1.0) * sun.luminance();
    sun.lerp(gray, 0.75)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let photo = u
        .textures
        .images
        .moon
        .sphere(f.local, footprint / f.local.length().max(1.0) * 0.35)
        .luminance();
    let rock = detail::rock(f, u, 0.13, footprint, 0.55);
    let ice = f.color.x;
    let snow = f.color.y;
    // Escarcha en lo plano y en la nieve; la roca asoma en las paredes.
    let up = f.local.normalize();
    let flat = smoothstep(0.82, 0.97, f.normal.dot(up));
    let frost = (0.35 + 0.65 * snow).min(1.0) * (0.4 + 0.6 * flat) * smoothstep(0.08, 0.3, photo);
    let ground = ROCK * (0.6 + 1.6 * photo) * (0.55 + rock.color.luminance());
    let mut albedo = ground.lerp(FROST * (0.75 + 0.5 * photo), frost);
    // Grietas del hielo: la frontera entre celdas de Voronoi.
    let cells = u.noise.cells(f.local * 0.035);
    let aa = (footprint * 0.035).max(0.002);
    let seams = smoothstep(0.012 + aa, 0.002, cells.1 - cells.0) * ice;
    albedo = albedo.lerp(ICE, ice * 0.85) * (1.0 - seams * 0.6);
    let lit = pbr::shade_light(
        f,
        u,
        albedo,
        0.8 - 0.62 * ice - 0.2 * frost,
        0.02,
        rock.normal,
        rock.ao,
        cold_light(u.sun_light),
    );
    // La luz de las grietas respira despacio, cada lugar a su ritmo.
    let breath = 0.5 + 0.5 * (u.time * 0.4 + cells.0 * 40.0).sin();
    let night = smoothstep(0.1, -0.25, f.normal.dot(f.light_dir(u)));
    let after = (u.biolum - 1.0).max(0.0) * 0.5;
    let glow = seams * (0.05 + 0.12 * breath) * (0.35 + 0.65 * night + after);
    lit + CRACK_LIGHT * (glow * u.gains.requiem)
}
