//! Requiem: mundo helado y muerto bajo la luz roja de la estrella.
//!
//! El terreno ya trae en cada vertice su altura, su pendiente y mascaras de
//! material (hielo, nieve, borde de crater): el shader las mezcla y agrega
//! detalle por pixel. Ese detalle se desvanece con la distancia: cuando un
//! pixel cubre mas terreno que el tamano del detalle, el ruido solo
//! parpadearia al moverse.

use super::{limb_haze, Fragment, Uniforms};
use crate::math::{smoothstep, Vec3};

const BASALT: Vec3 = Vec3::new(0.075, 0.07, 0.08);
const REGOLITH: Vec3 = Vec3::new(0.21, 0.20, 0.21);
const EJECTA: Vec3 = Vec3::new(0.32, 0.30, 0.30);
const SNOW: Vec3 = Vec3::new(0.80, 0.83, 0.88);
const ICE_DEEP: Vec3 = Vec3::new(0.24, 0.35, 0.50);
const ICE_PALE: Vec3 = Vec3::new(0.50, 0.61, 0.74);
const ATMOSPHERE: Vec3 = Vec3::new(0.30, 0.22, 0.48);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let height = f.uv.x;
    let slope = f.uv.y;
    let (ice, snow, rim) = (f.color.x, f.color.y, f.color.z);

    // Detalle por pixel con desvanecimiento por distancia.
    let footprint = f.pos.length() * u.pixel_angle;
    let fade = smoothstep(3.0, 0.6, footprint);
    let detail = if fade > 0.0 {
        u.noise.fbm(f.local * 0.45, 2) * fade
    } else {
        0.0
    };

    // Roca: basalto en las paredes, regolito en lo plano, eyecta en los bordes.
    let flat = 1.0 - smoothstep(0.08, 0.3, slope);
    let mut rock = BASALT.lerp(REGOLITH, flat * (0.55 + 0.45 * height));
    rock = rock.lerp(EJECTA, rim * 0.7);
    rock *= 1.0 + 0.35 * detail;

    // Hielo de los mares congelados: mas profundo donde hay grietas.
    let cracks = smoothstep(0.1, 0.0, detail.abs()) * fade;
    let frozen = ICE_PALE.lerp(ICE_DEEP, 0.35 + 0.4 * cracks);

    // La nieve solo se queda en lo plano.
    let snow_cover = (snow * flat).clamp(0.0, 1.0);
    let mut albedo = rock.lerp(SNOW, snow_cover);
    albedo = albedo.lerp(frozen, ice);

    let n = f.normal;
    let v = f.view_dir();
    let l = f.light_dir(u);
    let ndl = n.dot(l);
    let diffuse = albedo * u.sun_light * ndl.max(0.0);
    // El hielo y la nieve brillan con reflejos de la estrella.
    let gloss = ice * 0.45 + snow_cover * 0.15;
    let specular = if ndl > 0.0 && gloss > 0.0 {
        let half = (l + v).normalize();
        u.sun_light * (gloss * u.gains.requiem * n.dot(half).max(0.0).powf(80.0))
    } else {
        Vec3::ZERO
    };
    // Luz de las estrellas en el lado de noche: casi nada, pero no negro puro.
    let ambient = albedo * 0.012;
    // Atmosfera tenue: brilla en el borde del lado de dia.
    diffuse + specular + ambient + limb_haze(f, ATMOSPHERE, 0.8, ndl)
}
