//! Shaders de los objetos de la superficie.
//!
//! Dos llevan **mapeo de normales**: los monolitos (un ojo tallado en la
//! piedra) y los naufragios (las placas del casco). La normal del mapa vive
//! en el espacio tangente de la cara; `Fragment::perturb` la lleva al mundo
//! con la base TBN interpolada. El nivel de mipmap sale de cuanto mide un
//! pixel sobre la superficie: de lejos se lee el promedio, que no parpadea.

use super::{heartbeat, Fragment, Uniforms};
use crate::config::HEARTBEAT_PERIOD;
use crate::math::blackbody::planckian_rgb;
use crate::math::{smoothstep, Vec2, Vec3};
use crate::scene::props::{PropKind, WRECK_UV_SIZE};

/// Lado del glifo en las caras anchas del monolito (que miden 2.4 x 15), en
/// unidades del modelo, y altura de la cara (en v) donde empieza.
const GLYPH_SIZE: f32 = 2.4;
const GLYPH_BOTTOM: f32 = 0.6;
const GLYPH_ASPECT: f32 = 15.0 / GLYPH_SIZE;
/// Centro del sombrero de los hongos (en el modelo) y altura del anillo que
/// brilla en el tallo.
const CAP_CENTER: Vec3 = Vec3::new(0.69, 6.1, 0.0);
const STEM_RING: f32 = 5.3;

/// Lo que todos los objetos necesitan: el fragmento, los uniforms, la
/// escala de la instancia y la luz de la estrella que les llega.
struct Ctx<'a> {
    f: &'a Fragment,
    u: &'a Uniforms<'a>,
    scale: f32,
    /// Luz directa de la estrella: cero cuando se oculto tras el horizonte.
    sun: Vec3,
}

pub fn shade(kind: PropKind, scale: f32, light: f32, f: &Fragment, u: &Uniforms) -> Vec3 {
    let c = Ctx {
        f,
        u,
        scale,
        sun: u.sun_light * light,
    };
    match kind {
        PropKind::Monolith => monolith(&c),
        PropKind::Obelisk => obelisk(&c),
        PropKind::Mushroom => mushroom(&c),
        PropKind::Coral => coral(&c),
        PropKind::Spike => spike(&c),
        PropKind::Wreck => wreck(&c),
    }
}

/// Luz directa con un reflejo Blinn-Phong, para una normal dada. `wrap`
/// deja que la luz "envuelva" la superficie mas alla del terminador, como
/// en lo que deja pasar un poco de luz (la carne de un hongo); 0 es Lambert.
#[inline]
fn lit(c: &Ctx, albedo: Vec3, n: Vec3, specular: f32, shininess: f32, wrap: f32) -> Vec3 {
    let l = c.f.light_dir(c.u);
    let ndl = n.dot(l);
    let diffuse = ((ndl + wrap) / (1.0 + wrap)).max(0.0);
    let spec = if ndl > 0.0 && specular > 0.0 {
        let half = (l + c.f.view_dir()).normalize();
        specular * n.dot(half).max(0.0).powf(shininess)
    } else {
        0.0
    };
    // El termino ambiente usa la luz completa: es el cielo, que sigue
    // iluminando un rato despues de la puesta.
    c.sun * (albedo * diffuse + Vec3::splat(spec)) + albedo * c.u.sun_light * 0.02
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
            let texel = u.textures.eye_glyph.sample(Vec2::new(f.uv.x, gv), footprint);
            n = f.perturb(texel.normal, 1.0);
            glow = texel.glow;
            cavity = texel.height.clamp(0.4, 1.0);
        }
    }
    // Vetas de la piedra: un poco mas clara en bandas verticales irregulares.
    let veins = u.noise.fbm(Vec3::new(f.local.x * 2.0, f.local.y * 0.25, f.local.z * 2.0), 3) * 0.5 + 0.5;
    let albedo = Vec3::new(0.012, 0.010, 0.016) * ((0.7 + 0.8 * veins) * cavity);
    let beat = heartbeat(u.time as f64, HEARTBEAT_PERIOD) as f32;
    let emission = Vec3::new(1.0, 0.07, 0.12) * (glow * (0.5 + 1.6 * beat));
    lit(c, albedo, n, 0.6, 120.0, 0.0) + emission
}

/// Obelisco de piedra gastada con bandas grabadas cada dos unidades.
fn obelisk(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let wear = u.noise.fbm(f.local * 0.8, 3) * 0.5 + 0.5;
    let band = smoothstep(0.08, 0.0, (f.local.y * 0.5).fract().min(1.0 - (f.local.y * 0.5).fract()));
    let albedo = Vec3::new(0.16, 0.15, 0.17) * (0.7 + 0.4 * wear) * (1.0 - 0.5 * band);
    lit(c, albedo, f.normal, 0.08, 16.0, 0.0)
}

/// Hongo bioluminiscente: tallo palido y translucido con un anillo que
/// brilla, sombrero oscuro de terciopelo con puntos de luz que laten, y por
/// debajo laminillas que resplandecen.
fn mushroom(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let is_cap = f.color.z > f.color.y;
    let ndv = f.normal.dot(f.view_dir()).max(0.0);
    if !is_cap {
        // Vetas a lo largo del tallo.
        let streaks = u.noise.noise(Vec3::new(f.local.x * 3.0, f.local.y * 0.3, f.local.z * 3.0)) * 0.5 + 0.5;
        let albedo = Vec3::new(0.42, 0.40, 0.48) * (0.75 + 0.35 * streaks);
        let ring = smoothstep(0.45, 0.0, (f.local.y - STEM_RING).abs());
        let glow = Vec3::new(0.15, 0.85, 0.75) * (ring * 0.8);
        return lit(c, albedo, f.normal, 0.05, 20.0, 0.5) + glow * u.biolum;
    }
    let rel = f.local - CAP_CENTER;
    let pixel = pixel_footprint(c);
    let glow = if rel.y < 0.05 {
        // Por debajo: laminillas radiales. Su separacion crece con el radio;
        // cuando un pixel ya no las separa, se ve su brillo promedio.
        let radius = rel.x.hypot(rel.z);
        let spacing = std::f32::consts::TAU * radius / 40.0;
        let contrast = smoothstep(spacing, spacing * 0.3, pixel);
        let lines = 0.5 + 0.5 * (rel.z.atan2(rel.x) * 40.0).sin() * contrast;
        let fade = smoothstep(0.3, 1.2, radius);
        Vec3::new(0.95, 0.2, 0.75) * (lines * fade * 1.1)
    } else {
        // Arriba: puntos de luz en las celdas de un Worley. Cada punto late a
        // su ritmo (la segunda distancia de la celda le da una fase).
        let (f1, f2) = u.noise.cells(f.local * 1.4);
        let dot = smoothstep(0.32, 0.16, f1);
        let pulse = 0.6 + 0.4 * (u.time * 1.3 + f2 * 9.0).sin();
        Vec3::new(0.2, 1.0, 0.85) * (dot * pulse * 1.4)
    };
    let albedo = Vec3::new(0.10, 0.035, 0.13);
    // Terciopelo: la luz se junta en el borde de la silueta.
    let sheen = Vec3::new(0.5, 0.2, 0.6) * ((1.0 - ndv).powi(3) * 0.35);
    lit(c, albedo, f.normal, 0.15, 24.0, 0.3) + sheen * c.sun + glow * u.biolum
}

/// Coral: oscuro en la base, rosado arriba, con las puntas encendidas.
fn coral(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let height = (f.local.y / 12.0).clamp(0.0, 1.0);
    let albedo = Vec3::new(0.08, 0.02, 0.09).lerp(Vec3::new(0.45, 0.14, 0.32), height);
    let tips = smoothstep(0.6, 0.95, height);
    let pulse = heartbeat((u.time - height * 0.6) as f64, HEARTBEAT_PERIOD) as f32;
    let glow = Vec3::new(0.95, 0.3, 0.7).lerp(Vec3::new(0.2, 0.95, 0.9), height) * (tips * (0.3 + 1.2 * pulse));
    lit(c, albedo, f.normal, 0.15, 24.0, 0.3) + glow * u.biolum
}

/// Espina de obsidiana con lava todavia viva en la base.
fn spike(c: &Ctx) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let albedo = Vec3::new(0.02, 0.016, 0.016);
    let heat = smoothstep(1.8, -0.2, f.local.y);
    let flicker = u.noise.noise(f.local * 0.7 + Vec3::new(0.0, u.time * 0.4, 0.0)) * 0.5 + 0.5;
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
    lit(c, albedo, n, 0.6 * (1.0 - rust), 60.0, 0.0)
}
