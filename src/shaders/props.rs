//! Shaders de los objetos de la superficie.
//!
//! Dos llevan **mapeo de normales**: los monolitos (un ojo tallado en la
//! piedra) y los naufragios (las placas del casco). La normal del mapa vive
//! en el espacio tangente de la cara; `Fragment::perturb` la lleva al mundo
//! con la base TBN interpolada. El nivel de mipmap sale de cuanto mide un
//! pixel sobre la superficie: de lejos se lee el promedio, que no parpadea.

use super::{Fragment, Uniforms, detail, pbr, rings};
use crate::math::blackbody::planckian_rgb;
use crate::math::{Vec2, Vec3, smoothstep};
use crate::scene::props::{CORAL_TOP, PropKind, WRECK_UV_SIZE};

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

/// Lado del glifo en las caras anchas del monolito (que miden 2.4 x 15), en
/// unidades del modelo, y altura de la cara (en v) donde empieza.
const GLYPH_SIZE: f32 = 2.4;
const GLYPH_BOTTOM: f32 = 0.6;
const GLYPH_ASPECT: f32 = 15.0 / GLYPH_SIZE;
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
        PropKind::Monolith => monolith(&c),
        PropKind::Obelisk => obelisk(&c),
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
/// pixel lo calcula: la punta de un monolito sigue al sol cuando su base ya
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

/// Monolito de obsidiana. En las caras anchas de la losa (marcadas con el
/// color de vertice azul) hay un ojo tallado que late con la estrella.
fn monolith(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let mut n = f.normal;
    let mut glow = 0.0;
    // Lo tallado recibe menos luz del cielo y de los rebotes: mas oscuro.
    let mut cavity = 1.0;
    if f.color.z > 0.5 {
        let gv = (f.uv.y - GLYPH_BOTTOM) * GLYPH_ASPECT;
        if (0.0..1.0).contains(&gv) {
            let footprint = pixel_footprint(c) / GLYPH_SIZE;
            let texel = u
                .textures
                .eye_glyph
                .sample(Vec2::new(f.uv.x, gv), footprint);
            n = f.perturb(texel.normal, 1.0);
            glow = texel.glow;
            cavity = texel.height.clamp(0.4, 1.0);
        }
    }
    // Vetas de la piedra: un poco mas clara en bandas verticales irregulares.
    let veins = u.noise.fbm(
        Vec3::new(f.local.x * 2.0, f.local.y * 0.25, f.local.z * 2.0),
        3,
    ) * 0.5
        + 0.5;
    let albedo = Vec3::new(0.012, 0.010, 0.016) * ((0.7 + 0.8 * veins) * cavity);
    let emission = Vec3::new(1.0, 0.07, 0.12) * (glow * (0.5 + 1.6 * u.beat) * u.gains.requiem);
    lit(c, albedo, n, 0.6, 120.0, 0.0) + emission
}

/// Obelisco de piedra gastada con bandas grabadas cada dos unidades.
fn obelisk(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let wear = u.noise.fbm(f.local * 0.8, 3) * 0.5 + 0.5;
    // Bandas de 0.3 unidades cada 2: de lejos se ve su promedio (8%).
    let phase = (f.local.y * 0.5).fract();
    let sharp = smoothstep(0.08, 0.0, phase.min(1.0 - phase));
    let band = 0.08 + (sharp - 0.08) * smoothstep(0.5, 0.15, pixel_footprint(c));
    let albedo = Vec3::new(0.16, 0.15, 0.17) * (0.7 + 0.4 * wear) * (1.0 - 0.5 * band);
    lit(c, albedo, f.normal, 0.08, 16.0, 0.0)
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
