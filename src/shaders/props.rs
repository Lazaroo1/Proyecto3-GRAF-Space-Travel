//! Shaders de los objetos de la superficie.
//!
//! Los naufragios llevan **mapeo de normales** (las placas del casco). La normal del mapa vive
//! en el espacio tangente de la cara; `Fragment::perturb` la lleva al mundo
//! con la base TBN interpolada. El nivel de mipmap sale de cuanto mide un
//! pixel sobre la superficie: de lejos se lee el promedio, que no parpadea.

use super::{Fragment, Uniforms, detail, pbr, rings};
use crate::math::blackbody::planckian_rgb;
use crate::math::{Vec3, smoothstep};
use crate::scene::props::{CORAL_TOP, PropKind, SPIRE_TIERS, SPIRE_TIP, WRECK_UV_SIZE};
use std::f32::consts::TAU;

/// Datos de una instancia para su shader.
#[derive(Clone, Copy, Debug)]
pub struct PropStyle {
    pub kind: PropKind,
    /// Escala de la instancia: los mapas de normales eligen su mipmap con ella.
    pub scale: f32,
    /// Centro del planeta relativo a la camara y radio de la esfera que hace
    /// sombra (la que pasa por el suelo bajo el objeto).
    pub planet: Vec3,
    pub occluder: f32,
    /// Radio angular del disco de la estrella visto desde el objeto.
    pub disc: f32,
}

/// Centro del sombrero de los hongos (en el modelo) y altura del anillo que
/// brilla en el tallo.
const CAP_CENTER: Vec3 = Vec3::new(0.69, 8.6, 0.0);

/// Lo que todos los objetos necesitan: el fragmento, los uniforms, la
/// escala de la instancia y la luz de la estrella que les llega.
struct Ctx<'a> {
    f: &'a Fragment,
    u: &'a Uniforms<'a>,
    scale: f32,
    /// Luz directa de la estrella: cero cuando se oculto tras el horizonte.
    sun: Vec3,
}

pub fn shade(style: &PropStyle, f: &Fragment, u: &Uniforms) -> Vec3 {
    let l = f.light_dir(u);
    // Sombra de los anillos, igual que en el terreno de Cementerio.
    let rings = u
        .ring
        .as_ref()
        .map_or(1.0, |ring| rings::transmittance(ring, f.pos, l, u.noise));
    let c = Ctx {
        f,
        u,
        scale: style.scale,
        sun: u.sun_light * (planet_shadow(style, f.pos, l) * rings),
    };
    match style.kind {
        PropKind::Spire | PropKind::Ruin => spire(&c),
        PropKind::Mushroom => mushroom(&c),
        PropKind::Coral => coral(&c),
        PropKind::Spike => spike(&c),
        PropKind::Wreck => wreck(&c),
        PropKind::Boulder => boulder(&c),
    }
}

/// Luz directa con un reflejo Blinn-Phong, para una normal dada. `wrap`
/// deja que la luz "envuelva" la superficie mas alla del terminador, como
/// en lo que deja pasar un poco de luz (la carne de un hongo); 0 es Lambert.
#[inline]
fn lit(c: &Ctx, albedo: Vec3, n: Vec3, specular: f32, shininess: f32, wrap: f32) -> Vec3 {
    let rough = (2.0 / (shininess + 2.0)).powf(0.25).clamp(0.24, 0.85);
    let _ = wrap;
    pbr::shade_light(c.f, c.u, albedo, rough, specular * 0.06, n, 1.0, c.sun)
}

/// Cuanta luz de la estrella llega al punto `pos` sin que el planeta la
/// tape. El rayo hacia la estrella pasa a una distancia `d` del centro: si es
/// menor que el radio, el planeta lo corta. El borde se suaviza con el tamano
/// del disco de la estrella proyectado a esa distancia (la penumbra). Cada
/// pixel lo calcula: la punta de una aguja sigue al sol cuando su base ya
/// esta de noche.
#[inline]
fn planet_shadow(style: &PropStyle, pos: Vec3, to_light: Vec3) -> f32 {
    let p = pos - style.planet;
    let along = p.dot(to_light);
    if along >= 0.0 {
        return 1.0;
    }
    let closest = (p.length_squared() - along * along).max(0.0).sqrt();
    let soft = style.disc * -along + 0.05;
    smoothstep(style.occluder - soft, style.occluder + soft, closest)
}

/// Cuanto mide un pixel sobre la superficie, en unidades del modelo. Visto
/// de lado, un pixel cubre mas superficie: se divide entre el coseno del
/// angulo de vision, con un tope para no irse al infinito en los bordes.
#[inline]
fn pixel_footprint(c: &Ctx) -> f32 {
    let ndv = c.f.normal.dot(c.f.view_dir()).abs().max(0.25);
    c.f.pos.length() * c.u.pixel_angle / (ndv * c.scale)
}

/// Aguja gotica: piedra oscura azulada, escarcha en lo que mira hacia
/// arriba (cornisas y remates) y en cada cara del fuste una ventana de luz
/// fria. La luz sube por la aguja con cada latido, del pie a la punta, como
/// una senal hacia donde va a nacer Gargantua; despues del colapso se queda
/// encendida.
fn spire(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let p = f.local;
    let up = f.object_to_world.mul_vec(Vec3::Y);
    let facing_up = f.normal.dot(up);
    let grain = u.noise.fbm(Vec3::new(p.x * 1.5, p.y * 0.35, p.z * 1.5), 3) * 0.5 + 0.5;
    let frost = smoothstep(0.35, 0.8, facing_up + 0.25 * (grain - 0.5))
        + 0.25 * smoothstep(2.5, 0.0, p.y);
    let stone = Vec3::new(0.030, 0.036, 0.048) * (0.65 + 0.7 * grain);
    let ice = Vec3::new(0.50, 0.62, 0.78);
    let albedo = stone.lerp(ice, frost.min(1.0));

    // Ventanas: una ranura vertical en el centro de cada cara del fuste.
    let mut window = 0.0;
    let radius = p.x.hypot(p.z);
    for &(bottom, top, r0, r1) in &SPIRE_TIERS {
        if p.y > bottom && p.y < top {
            let along = (p.y - bottom) / (top - bottom);
            let wall = r0 + (r1 - r0) * along;
            if (radius - wall * 0.92).abs() < 0.25 {
                let face = (p.z.atan2(p.x) / (TAU / 8.0)).rem_euclid(1.0);
                let slit = smoothstep(0.09, 0.04, (face - 0.5).abs());
                window = slit * smoothstep(0.0, 0.12, along) * smoothstep(1.0, 0.8, along);
            }
        }
    }
    // La punta brilla como un faro.
    let tip = smoothstep(SPIRE_TIP - 3.0, SPIRE_TIP, p.y);
    let pulse = u.beat_at(p.y * 0.045);
    let after = (u.biolum - 1.0).max(0.0) * 0.5;
    let light = (0.12 + 1.5 * pulse + after) * u.gains.requiem;
    let cold = Vec3::new(0.30, 0.62, 1.0);
    let emission = cold * ((window * 1.4 + tip * 0.8) * light);
    let cold = Ctx {
        sun: super::requiem::cold_light(c.sun),
        ..*c
    };
    lit(&cold, albedo, f.normal, 0.3 + 0.5 * frost.min(1.0), 60.0, 0.0) + emission
}

/// Hongo bioluminiscente: tallo palido y translucido con un anillo que
/// brilla, sombrero oscuro de terciopelo con puntos de luz que laten, y por
/// debajo laminillas que resplandecen.
fn mushroom(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let cap = f.color.z > f.color.y;
    let footprint = pixel_footprint(c);
    let tex = detail::rock(f, u, 0.22, footprint, 0.46);
    let cells = detail::sample(f, &u.textures.organic, 0.19, footprint, 0.2);
    let n = (tex.normal * 0.65 + cells.normal * 0.35).normalize();
    let tissue = tex.color * Vec3::new(0.28, 0.34, 0.27) * (0.55 + 0.45 * cells.height);
    let rel = f.local - CAP_CENTER;
    let radius = rel.x.hypot(rel.z);
    let phi = rel.z.atan2(rel.x);
    let broken = u.noise.fbm(f.local * 0.8, 3) * 0.5 + 0.5;
    let glow = if cap {
        let edge = smoothstep(4.72, 5.05, radius) * 0.20;
        let gills = (phi * 42.0 + radius * 1.3 + broken * 2.0).sin() * 0.5 + 0.5;
        let under = if rel.y < 0.10 {
            gills.powi(4) * 0.55 * broken
        } else {
            cells.glow.powi(3) * 0.35 * broken
        };
        Vec3::new(0.018, 0.55, 0.42) * (under + edge * broken)
    } else {
        Vec3::new(0.012, 0.42, 0.30) * (cells.glow.powi(3) * 0.30 * broken)
    };
    pbr::shade_light(
        f,
        u,
        tissue,
        0.34 + 0.22 * tex.rough,
        0.04,
        n,
        tex.ao * (0.35 + 0.65 * cells.height),
        c.sun,
    ) + glow * u.biolum * (0.7 + 0.3 * u.beat)
}

fn coral(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let height = (f.local.y / CORAL_TOP).clamp(0.0, 1.0);
    let tex = detail::rock(f, u, 0.25, pixel_footprint(c), 0.6);
    let tips = 1.0 - f.color.y;
    let cell = detail::sample(f, &u.textures.organic, 0.25, pixel_footprint(c), 0.2);
    let n = (tex.normal * 0.75 + cell.normal * 0.25).normalize();
    let albedo = tex.color * Vec3::new(0.24, 0.31, 0.24) * (0.5 + 0.5 * cell.height);
    let lit = pbr::shade_light(
        f,
        u,
        albedo,
        0.38 + 0.25 * tex.rough,
        0.03,
        n,
        tex.ao,
        c.sun,
    );
    let grain = u.noise.fbm(f.local * 2.0, 3) * 0.5 + 0.5;
    let glow = Vec3::new(0.012, 0.48, 0.24).lerp(Vec3::new(0.01, 0.25, 0.45), height)
        * (tips * (0.3 + 0.7 * grain) * (0.75 + 0.25 * u.beat_at(height * 0.6))
            + cell.glow.powi(7) * 0.025);
    lit + glow * u.biolum
}

/// Espina de obsidiana con lava todavia viva en la base.
fn spike(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let albedo = Vec3::new(0.02, 0.016, 0.016);
    let heat = smoothstep(1.8, -0.2, f.local.y);
    let flicker = u
        .noise
        .noise(f.local * 0.7 + Vec3::new(0.0, u.time * 0.4, 0.0))
        * 0.5
        + 0.5;
    let glow = planckian_rgb(1250.0 + 500.0 * flicker) * (heat * (1.0 + 2.0 * flicker));
    lit(c, albedo, f.normal, 0.35, 160.0, 0.0) + glow
}

/// Casco de una nave naufraga: metal con placas (mapa de normales), oxido y
/// quemaduras hacia el extremo partido.
fn wreck(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let footprint = pixel_footprint(c) / WRECK_UV_SIZE;
    let texel = u.textures.hull.sample(f.uv, footprint);
    let n = f.perturb(texel.normal, 1.0);
    let rust = smoothstep(0.5, 0.75, u.noise.fbm(f.local * 0.2, 3) * 0.5 + 0.5);
    let burn = smoothstep(0.0, 8.0, f.local.x);
    let mut albedo = Vec3::new(0.09, 0.095, 0.105).lerp(Vec3::new(0.2, 0.08, 0.035), rust * 0.8);
    albedo = albedo.lerp(Vec3::new(0.02, 0.018, 0.015), burn * 0.75) * texel.height.clamp(0.8, 1.0);
    // Las paredes de adentro (color de vertice oscuro) casi no reciben luz.
    albedo = albedo * f.color.x;
    lit(c, albedo, n, 0.6 * (1.0 - rust), 60.0, 0.0)
}

fn boulder(c: &Ctx) -> Vec3 {
    let tex = detail::rock(c.f, c.u, 0.35, pixel_footprint(c), 0.85);
    pbr::shade_light(
        c.f,
        c.u,
        tex.color * 0.20,
        tex.rough.max(0.5),
        0.04,
        tex.normal,
        tex.ao,
        c.sun,
    )
}
