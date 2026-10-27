//! Shaders de los objetos de la superficie.
//!
//! Los naufragios llevan **mapeo de normales** (las placas del casco). La normal del mapa vive
//! en el espacio tangente de la cara; `Fragment::perturb` la lleva al mundo
//! con la base TBN interpolada. El nivel de mipmap sale de cuanto mide un
//! pixel sobre la superficie: de lejos se lee el promedio, que no parpadea.

use super::{Fragment, Uniforms, detail, pbr, rings};
use crate::math::blackbody::planckian_rgb;
use crate::math::{Vec2, Vec3, smoothstep};
use crate::scene::props::{
    CORAL_TOP, CRYSTAL_TOP, PropKind, SPIRE_TIERS, SPIRE_TIP, VENT_TOP, WRECK_UV_SIZE,
};
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
    /// Numero al azar de la instancia (0 a 1): cada geiser hace erupcion a
    /// su ritmo.
    pub seed: f32,
}

/// Que tan cerca esta la nave de `pos` (relativo a la camara): 0 a `far`
/// unidades o mas, 1 a `close` o menos.
#[inline]
pub fn near_ship(u: &Uniforms, pos: Vec3, far: f32, close: f32) -> f32 {
    smoothstep(far, close, (pos - u.ship).length())
}

/// Actividad de un geiser (0 a 1): cada uno hace erupcion solo de vez en
/// cuando, a su ritmo, y despierta del todo cuando la nave se le acerca.
pub fn vent_activity(time: f32, seed: f32, ship_distance: f32) -> f32 {
    let cycle = (time * (0.05 + 0.04 * seed) + seed * 7.3).fract();
    let burst = smoothstep(0.0, 0.04, cycle) * smoothstep(0.32, 0.12, cycle);
    let near = smoothstep(230.0, 70.0, ship_distance);
    (0.12 + 0.88 * burst).max(near)
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
    let rings: f32 = u
        .rings
        .iter()
        .map(|ring| rings::transmittance(ring, f.pos, l, u.noise))
        .product();
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
        PropKind::Beacon => beacon(&c, style),
        PropKind::Vent => vent(&c, style),
        PropKind::Crystal => crystal(&c, style),
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
    // Despierta cuando la nave se acerca: la luz se enciende primero en lo
    // que le queda mas cerca y sube por la aguja.
    let awake = near_ship(u, f.pos, 260.0, 40.0);
    let light = (0.12 + 1.5 * pulse + after + awake * (2.5 + 1.5 * pulse)) * u.gains.requiem;
    let cold = Vec3::new(0.30, 0.62, 1.0);
    let emission = cold * ((window * 1.4 + tip * 0.8) * light);
    let cold = Ctx {
        sun: super::requiem::cold_light(c.sun),
        ..*c
    };
    lit(&cold, albedo, f.normal, 0.3 + 0.5 * frost.min(1.0), 60.0, 0.0) + emission
}

/// La baliza: metal oscuro y una lampara roja que destella cada 2.4 s,
/// como si siguiera transmitiendo la historia del exodo.
fn beacon(c: &Ctx, _style: &PropStyle) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let wear = u.noise.fbm(f.local * 0.6, 3) * 0.5 + 0.5;
    let albedo = Vec3::new(0.10, 0.10, 0.11).lerp(Vec3::new(0.22, 0.12, 0.07), wear * 0.6);
    let lamp = f.color.x > 0.5 && f.color.y < 0.5;
    let flash = beacon_flash(u.time, near_ship(u, f.pos, 300.0, 60.0));
    let emission = if lamp {
        Vec3::new(1.0, 0.06, 0.03) * (0.3 + 12.0 * flash)
    } else {
        Vec3::ZERO
    };
    lit(c, albedo, f.normal, 0.5, 40.0, 0.0) + emission * u.gains.cementerio
}

/// Destello de la baliza: sube de golpe y se apaga en medio segundo. Con
/// la nave cerca, entre destello y destello titila rapido: te detecto.
pub fn beacon_flash(time: f32, near: f32) -> f32 {
    let phase = (time / BEACON_PERIOD).fract() * BEACON_PERIOD;
    let quick = (time / 0.45).fract() * 0.45;
    (-phase / 0.18).exp().max(near * (-quick / 0.08).exp() * 0.6)
}

/// Geiser de Pira: roca negra que se calienta hacia la boca, grietas que
/// brillan y la lava de la boca, todo segun su actividad.
fn vent(c: &Ctx, style: &PropStyle) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let footprint = pixel_footprint(c);
    let tex = detail::rock(f, u, 0.3, footprint, 0.8);
    let top = f.object_to_world.mul_vec(Vec3::Y * (VENT_TOP * style.scale));
    let mouth = f.pos - f.object_to_world.mul_vec(f.local * style.scale) + top;
    let activity = vent_activity(u.time, style.seed, (mouth - u.ship).length());
    let pool = f.color.x > 0.5 && f.color.y > 0.5;
    if pool {
        let churn = u.noise.fbm(f.local * 2.5 + Vec3::new(0.0, u.time * 0.8, 0.0), 3) * 0.5 + 0.5;
        let temperature = 1300.0 + 900.0 * activity * (0.6 + 0.4 * churn);
        return planckian_rgb(temperature) * ((1.5 + 6.0 * activity) * (0.6 + 0.4 * churn) * u.gains.pira);
    }
    let heat = smoothstep(3.5, VENT_TOP, f.local.y);
    let cracks = 1.0 - u.noise.noise(f.local * 1.1).abs();
    let crack = smoothstep(0.965, 0.995, cracks) * heat * heat;
    let albedo = tex.color * Vec3::new(0.06, 0.05, 0.048);
    let lit = pbr::shade_light(f, u, albedo, 0.75, 0.05, tex.normal, tex.ao, c.sun);
    let glow = planckian_rgb(1200.0 + 500.0 * activity)
        * ((crack * 2.0 + heat.powi(4) * 0.25) * (0.3 + activity) * u.gains.pira);
    lit + glow
}

/// Cada cuanto destella la baliza (y sale una onda).
pub const BEACON_PERIOD: f32 = 2.4;

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
    // Ventanillas en filas a lo largo del casco: cuando la nave pasa cerca
    // se encienden, parpadeando como si les quedara un poco de energia.
    let awake = near_ship(u, f.pos, 160.0, 30.0);
    let mut glow = Vec3::ZERO;
    if awake > 0.0 && f.color.x > 0.5 {
        let grid = Vec2::new(f.uv.x * 6.0, f.uv.y * 3.0);
        let id = Vec2::new(grid.x.floor(), grid.y.floor());
        let cell = Vec2::new(grid.x - id.x, grid.y - id.y);
        // Solo unas pocas ventanillas siguen vivas.
        let alive = ((id.x * 12.9898 + id.y * 78.233).sin() * 43_758.547).fract().abs() < 0.22;
        let window = if alive {
            smoothstep(0.22, 0.14, (cell.x - 0.5).abs()) * smoothstep(0.14, 0.08, (cell.y - 0.5).abs())
        } else {
            0.0
        };
        let flicker = (u.time * 9.0 + id.x * 3.1 + id.y * 7.7).sin() * (u.time * 2.3 + id.x).sin();
        let on = smoothstep(-0.3, 0.6, flicker);
        let tint = if (id.x + id.y) % 3.0 < 1.0 {
            Vec3::new(1.0, 0.35, 0.12)
        } else {
            Vec3::new(0.65, 0.85, 1.0)
        };
        glow = tint * (window * on * awake * 1.6 * u.gains.cementerio);
    }
    lit(c, albedo, n, 0.6 * (1.0 - rust), 60.0, 0.0) + glow
}

/// Racimo de cristal de Parpado: liso y violeta, con brillos de la estrella
/// en las caras y el borde encendido (Fresnel). Adentro tiene una luz que
/// late despacio, mas hacia las puntas, y que se aviva con la nave cerca.
fn crystal(c: &Ctx, style: &PropStyle) -> Vec3 {
    let (f, u) = (c.f, c.u);
    let v = f.view_dir();
    let n = f.normal;
    let ndv = n.dot(v).abs();
    // Cristal oscuro y pulido: casi todo lo que se ve es lo que refleja
    // (Vigia y el cielo, con Fresnel) y los brillos de la estrella en sus
    // caras, cada una con su inclinacion.
    let fresnel = 0.06 + 0.94 * (1.0 - ndv).powi(4);
    let r = n * (2.0 * n.dot(v)) - v;
    let mirror = super::parpado::environment(f, r, u) * (fresnel * 0.9);
    let height = smoothstep(0.0, CRYSTAL_TOP, f.local.y);
    let base = pbr::shade_light(f, u, Vec3::new(0.05, 0.02, 0.1), 0.08, 0.0, n, 1.0, c.sun);
    // Adentro, vetas de luz que laten despacio y suben hacia las puntas; se
    // ven mas por los bordes (la luz corre por dentro del cristal y sale de
    // lado). Con la nave cerca se encienden.
    let near = near_ship(u, f.pos, 150.0, 20.0);
    let pulse = 0.5 + 0.5 * (u.time * 1.3 + style.seed * TAU + f.local.y * 0.35).sin();
    let veins = smoothstep(0.88, 0.98, 1.0 - u.noise.noise(f.local * 2.6 + Vec3::new(0.0, u.time * 0.25, style.seed * 9.0)).abs());
    let edge = 0.3 + 0.7 * (1.0 - ndv) * (1.0 - ndv);
    let inner = Vec3::new(0.5, 0.12, 1.0).lerp(Vec3::new(1.0, 0.3, 0.75), height)
        * (height * height * (0.04 + 0.06 * pulse) + near * (0.15 + 0.55 * pulse) * height * veins)
        * edge;
    base + mirror + inner * u.gains.vigia
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
