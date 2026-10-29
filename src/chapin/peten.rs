//! Peten (la version chapina de Jardin): la selva de las tierras bajas, con
//! rios que serpentean, los lagos del centro (el Peten Itza con la isla de
//! Flores) y Tikal: el Templo del Gran Jaguar saliendo sobre los arboles.
//!
//! # El relieve
//! - **Llanura y karst**: lomas suaves y, donde un ruido grande lo dice,
//!   cerros redondos de caliza (ruido celular, como los conos de Semuc).
//! - **El agua**: rios y lagos estan todos al mismo nivel. La orilla es una
//!   distancia con signo en unidades del mundo: un rio es donde un ruido
//!   cruza por cero (con el espacio torcido por otro ruido, para que
//!   serpentee), y la distancia es el valor entre su pendiente; un lago es
//!   una elipse con la orilla rota por ruido. Cerca del agua la tierra baja
//!   en un valle hasta la orilla.
//! - **Tikal**: un llano con la Gran Plaza despejada en medio de la selva.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::config;
use crate::math::{DVec3, Perlin, Vec3, hash_u32, smoothstep};
use crate::scene::Mesh;
use crate::scene::builder::{MeshBuilder, perpendiculars};
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter, at_site, upright_rotation};
use crate::shaders::{AtmosphereParams, Fragment, PropStyle, Uniforms, detail, limb_haze, pbr};
use crate::terrain::{HeightField, TerrainSample};

pub fn spec() -> Spec {
    Spec {
        label: "Peten",
        material: Chapin::Peten,
        // Aire de selva: humedo (mucho Mie) y un cielo azul verdoso.
        atmosphere: Some(AtmosphereParams {
            thickness: 45.0,
            scale_height: 15.0,
            rayleigh: Vec3::new(0.0009, 0.0062, 0.014),
            mie: 0.0012,
            mie_g: 0.8,
            intensity: 3.0,
        }),
        rings: None,
        motes: Some((Motes::Fireflies, 120.0)),
    }
}

// El lugar y el agua ---------------------------------------------------------

/// Radio para el que esta pensado el relieve: pasa angulos a unidades.
const RADIUS: f32 = 400.0;
/// Tikal, en el espacio del planeta, y hacia donde se mira al llegar. A los
/// 40 s la estrella esta a unos 32 grados, atras a la izquierda: alumbra el
/// frente del templo, deja en sombra su lado derecho y las sombras de la
/// selva no alcanzan a taparlo.
const TIKAL: Vec3 = Vec3::new(0.1226, 0.4903, -0.8629);
const TOWARD: Vec3 = Vec3::new(0.9482, -0.3146, -0.044);
/// Radio angular del llano de Tikal: plano hasta el primero, se junta con
/// la selva hasta el segundo.
const LLANO: (f32, f32) = (0.28, 0.45);
/// La Gran Plaza despejada: medio ancho, medio largo y cuanto se corre
/// hacia adelante, en unidades del mundo (ejes del lugar).
const PLAZA: (f32, f32, f32) = (40.0, 67.0, 5.0);
/// Donde queda el Templo del Gran Jaguar, al frente de la plaza, y su escala.
const TEMPLE_AHEAD: f64 = 54.0;
const TEMPLE_SCALE: f32 = 1.15;

/// Los lagos: centro, medio largo y medio ancho (unidades del mundo). Van
/// de oeste a este, como la cadena de lagos del centro de Peten: el primero
/// es el Peten Itza, con Flores en su brazo del sur.
const LAKES: [(Vec3, f32, f32); 3] = [
    (Vec3::new(-0.2698, 0.3975, -0.8771), 58.0, 17.0),
    (Vec3::new(-0.4805, 0.4219, -0.7689), 22.0, 9.0),
    (Vec3::new(-0.1027, 0.1601, -0.9818), 30.0, 11.0),
];
/// La isla de Flores: donde queda en el Peten Itza (a lo largo y a lo
/// ancho, en medios ejes) y su radio.
const FLORES: (f32, f32, f32) = (-0.15, -0.55, 4.6);
/// Rios por radio del planeta.
const RIVERS: f32 = 1.35;
/// Nivel del agua y altura de la orilla.
const WATER: f32 = 0.0;
const BANK: f32 = 0.6;

/// Ejes del lugar: vertical, al frente y a la derecha.
fn site_axes() -> (Vec3, Vec3, Vec3) {
    let up = TIKAL.normalize();
    let forward = (TOWARD - up * TOWARD.dot(up)).normalize();
    (up, forward, forward.cross(up))
}

/// Ejes de un lago: el largo va de oeste a este (alrededor del eje de giro,
/// `y`) y el ancho de norte a sur.
#[inline]
fn lake_axes(center: Vec3) -> (Vec3, Vec3, Vec3) {
    let c = center.normalize();
    let east = Vec3::Y.cross(c).normalize();
    (c, east, c.cross(east))
}

/// Distancia aproximada (con signo) a una elipse de medios ejes `a` y `b`:
/// el valor de `k = |(x/a, y/b)|` menos uno entre su pendiente.
#[inline]
fn ellipse(x: f32, y: f32, a: f32, b: f32) -> f32 {
    let k = ((x / a).powi(2) + (y / b).powi(2)).sqrt();
    let grad = ((x / (a * a)).powi(2) + (y / (b * b)).powi(2)).sqrt().max(1e-6);
    k * (k - 1.0) / grad
}

/// El agua mas cercana a un punto.
#[derive(Clone, Copy, Debug)]
struct Shore {
    /// Distancia con signo a la orilla, en unidades del mundo: negativa en
    /// el agua.
    distance: f32,
    /// 1 si es un lago, 0 si es un rio.
    lake: f32,
    /// Cuanto pesa el valle (0 lejos de cualquier rio o lago).
    flow: f32,
}

/// Los rios: donde un ruido cruza por cero. Otro ruido tuerce el espacio
/// para que el cauce serpentee, y uno grande decide donde hay rio: nace
/// angosto y se ensancha. La distancia es el valor entre la pendiente
/// (sobre la superficie, por radian), asi el ancho queda en unidades.
fn river(n: &Perlin, dir: Vec3) -> (f32, f32) {
    let away = smoothstep(0.988, 0.978, dir.dot(TIKAL.normalize()));
    let flow = smoothstep(-0.15, 0.25, n.noise(dir * 1.1 + Vec3::new(8.8, -3.0, 1.5))) * away;
    if flow <= 0.0 {
        return (f32::MAX, 0.0);
    }
    let w = dir * 4.5;
    let warp = Vec3::new(
        n.noise(w + Vec3::new(3.1, 0.0, 0.0)),
        n.noise(w + Vec3::new(0.0, 7.7, 0.0)),
        n.noise(w + Vec3::new(0.0, 0.0, 5.3)),
    ) * 0.09;
    let q = (dir + warp) * RIVERS + Vec3::new(-6.2, 2.9, 4.4);
    let v = n.noise(q);
    let (t1, t2) = perpendiculars(dir);
    let e = 0.002;
    let g1 = (n.noise(q + t1 * (RIVERS * e)) - v) / e;
    let g2 = (n.noise(q + t2 * (RIVERS * e)) - v) / e;
    let slope = (g1 * g1 + g2 * g2).sqrt().max(0.3);
    let half = (2.0 + 4.5 * flow) * flow;
    (v.abs() / slope * RADIUS - half, flow)
}

/// Los lagos: elipses con la orilla rota por ruido, y la isla de Flores.
fn lakes(n: &Perlin, dir: Vec3) -> f32 {
    let mut best = f32::MAX;
    for (k, &(center, a, b)) in LAKES.iter().enumerate() {
        let (c, east, north) = lake_axes(center);
        let offset = dir - c;
        if offset.length_squared() * RADIUS * RADIUS > (a + 50.0).powi(2) {
            continue;
        }
        let (x, y) = (offset.dot(east) * RADIUS, offset.dot(north) * RADIUS);
        let wobble = 4.0 * n.noise(dir * 30.0 + Vec3::splat(k as f32 * 3.7)) + 1.5 * n.noise(dir * 90.0);
        let mut d = ellipse(x, y, a, b) + wobble;
        if k == 0 {
            // El brazo del sur, con Flores en medio.
            let (fx, fy) = (FLORES.0 * a, FLORES.1 * b);
            let arm = ellipse(x - fx, y - fy - 0.35 * b, 0.32 * a, 0.75 * b) + wobble * 0.5;
            let island = (x - fx).hypot(y - fy) - FLORES.2 + n.noise(dir * 200.0) * 0.6;
            d = d.min(arm).max(-island);
        }
        best = best.min(d);
    }
    best
}

fn shore(n: &Perlin, dir: Vec3) -> Shore {
    let (river, flow) = river(n, dir);
    let lake = lakes(n, dir);
    Shore {
        distance: river.min(lake),
        lake: if lake < river { 1.0 } else { 0.0 },
        flow: flow.max(smoothstep(80.0, 40.0, lake)),
    }
}

// El relieve -------------------------------------------------------------------

/// La selva. Atributos: `x` distancia a la orilla (unidades, negativa en el
/// agua), `y` selva (1) o claro (0), `z` bajo (hondonada que se inunda).
pub struct PetenField {
    noise: Perlin,
    /// Rios y lagos: con la semilla global, la misma que reciben los
    /// shaders (como las arterias de Jardin), asi el shader los redibuja.
    water: Perlin,
    /// Altura del llano de Tikal.
    plaza: f32,
}

impl PetenField {
    pub fn new(seed: u32) -> Self {
        let mut field = Self {
            noise: Perlin::new(seed),
            water: Perlin::new(config::NOISE_SEED),
            plaza: 0.0,
        };
        field.plaza = field.land(TIKAL.normalize()).0;
        field
    }

    /// La tierra sin agua: llanura, cerros de karst y bajos.
    fn land(&self, dir: Vec3) -> (f32, f32) {
        let n = &self.noise;
        let plain = 1.8 + 1.2 * n.fbm(dir * 7.0 + Vec3::new(2.2, -4.1, 0.7), 3);
        // Cerros redondos de caliza donde un ruido grande lo dice.
        let hills = smoothstep(0.05, 0.35, n.fbm(dir * 1.6 + Vec3::new(-3.3, 5.5, 1.2), 2));
        let (f1, _) = n.cells(dir * 22.0 + Vec3::new(4.7, 1.1, -2.9));
        let cone = (1.0 - (f1 / 0.9).powi(2)).max(0.0);
        let tall = 9.0 + 6.0 * n.noise(dir * 5.0 + Vec3::new(0.3, 8.1, -1.7));
        let karst = hills * cone * cone.sqrt() * tall;
        // Bajos: hondonadas planas que se inundan con la lluvia.
        let bajo = smoothstep(0.15, 0.4, n.noise(dir * 3.4 + Vec3::new(6.6, 0.4, -5.2))) * (1.0 - hills);
        (plain + karst - 0.5 * bajo, bajo)
    }
}

/// La Gran Plaza en los ejes del lugar: distancia con signo a su borde.
#[inline]
fn plaza_edge(x: f32, z: f32) -> f32 {
    let (hx, hz, shift) = PLAZA;
    let r = 14.0;
    let qx = x.abs() - hx + r;
    let qz = (z - shift).abs() - hz + r;
    qx.max(0.0).hypot(qz.max(0.0)) + qx.max(qz).min(0.0) - r
}

/// Lo que esta a `dir` en los ejes del lugar (unidades del mundo).
#[inline]
fn at_tikal(dir: Vec3) -> (f32, f32) {
    let (_, forward, right) = site_axes();
    (dir.dot(right) * RADIUS, dir.dot(forward) * RADIUS)
}

impl HeightField for PetenField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let (land, bajo) = self.land(dir);
        let water = shore(&self.water, dir);
        let s = water.distance;
        let mut height = if s < 0.0 {
            WATER
        } else {
            // El valle: la tierra baja hasta la orilla.
            let bank = BANK * smoothstep(0.0, 5.0, s);
            let valley = smoothstep(2.0, 45.0, s).max(1.0 - water.flow);
            bank + (land - bank) * valley
        };
        let mut cover = smoothstep(2.0, 6.0, s);
        let angle = dir.dot(TIKAL.normalize()).clamp(-1.0, 1.0).acos();
        if angle < LLANO.1 {
            let flat = smoothstep(LLANO.1, LLANO.0, angle);
            let swell = 0.4 * self.noise.fbm(dir * 40.0, 2);
            height += (self.plaza + swell - height) * flat;
            let (x, z) = at_tikal(dir);
            let ragged = 8.0 * self.noise.noise(dir * 35.0) + 3.0 * self.noise.noise(dir * 110.0);
            cover = cover.min(smoothstep(0.0, 9.0, plaza_edge(x, z) + ragged));
        }
        TerrainSample {
            height,
            attrs: Vec3::new(s.clamp(-12.0, 12.0), cover, bajo),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        // Abajo, el agua y los bajos (la llanura nunca baja de 0.6); arriba,
        // los cerros de karst mas altos sobre la llanura.
        (-0.5, 3.0 + 15.0)
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(PetenField::new(seed))
}

// El shader del suelo ---------------------------------------------------------

/// Verdes de las copas: oscuro de selva madura, medio, amarillento de hojas
/// nuevas y azulado; y los arboles en flor (cortez amarillo, matilisguate
/// rosado).
const GREENS: [Vec3; 4] = [
    Vec3::new(0.008, 0.035, 0.010),
    Vec3::new(0.018, 0.075, 0.014),
    Vec3::new(0.050, 0.110, 0.012),
    Vec3::new(0.010, 0.055, 0.030),
];
const BLOOMS: [Vec3; 2] = [Vec3::new(0.40, 0.30, 0.04), Vec3::new(0.35, 0.14, 0.20)];
const BAJO: Vec3 = Vec3::new(0.05, 0.065, 0.035);
const GRASS: Vec3 = Vec3::new(0.05, 0.085, 0.022);
const SCRUB: Vec3 = Vec3::new(0.02, 0.045, 0.012);
/// La tierra blanca de caliza de los caminos (sascab).
const SASCAB: Vec3 = Vec3::new(0.24, 0.22, 0.17);
const MUD: Vec3 = Vec3::new(0.12, 0.10, 0.065);
const LIMESTONE: Vec3 = Vec3::new(0.5, 0.48, 0.42);
/// Agua: lo que dispersa (cafe verdoso en los rios, que cargan lodo; azul
/// verdoso en los lagos de caliza) y cuanto apaga cada color por unidad de
/// camino.
const RIVER_TINT: Vec3 = Vec3::new(0.022, 0.028, 0.014);
const LAKE_TINT: Vec3 = Vec3::new(0.008, 0.05, 0.055);
const ABSORB: Vec3 = Vec3::new(0.5, 0.18, 0.15);
/// Lado de la celda de cada copa, en unidades del mundo.
const CROWN: f32 = 5.0;

/// La luz de la estrella sobre la selva, un poco menos roja: con todo el
/// rojo el verde se ve cafe.
#[inline]
fn jungle_light(sun: Vec3) -> Vec3 {
    sun.lerp(Vec3::splat(sun.luminance()), 0.35)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let dir = f.local.normalize();
    let water = shore(u.noise, dir);
    // La orilla se mezcla en una franja del ancho de un pixel.
    let band = (footprint * 1.2).max(0.2);
    let wet = smoothstep(band, -band, water.distance);
    let l = f.light_dir(u);
    let light = jungle_light(u.sun_light);
    let mut color = Vec3::ZERO;
    if wet < 1.0 {
        color += ground(f, u, water.distance, footprint, light) * (1.0 - wet);
    }
    if wet > 0.0 {
        color += lake_or_river(f, u, water, footprint, light) * wet;
    }
    color + limb_haze(f, Vec3::new(0.2, 0.45, 0.5), 0.35, f.normal.dot(l))
}

/// El suelo: la selva, los bajos, la plaza, las orillas y la caliza de los
/// cerros.
fn ground(f: &Fragment, u: &Uniforms, s: f32, footprint: f32, light: Vec3) -> Vec3 {
    let n = u.noise;
    let p = f.local;
    let (cover, bajo) = (f.color.y, f.color.z);
    let slope = f.uv.y;
    let mottle = n.fbm(p * 0.09 + Vec3::splat(4.0), 2) * 0.5 + 0.5;
    let mut albedo = Vec3::ZERO;
    let mut normal = f.normal;
    let mut cavity = 1.0;
    if cover > 0.01 {
        let (leaf, dome, gap) = canopy(f, n, footprint, bajo);
        albedo += leaf * cover;
        normal = normal.lerp(dome, cover).normalize();
        cavity = 1.0 - (1.0 - gap) * cover;
    }
    if cover < 0.99 {
        // La plaza: grama con caminos de tierra blanca; hacia la orilla de
        // la selva, monte tupido.
        let edge = smoothstep(0.0, 0.5, cover);
        let blades = n.noise(p * 1.7) * smoothstep(0.4, 0.1, footprint);
        let grass = GRASS * (0.7 + 0.6 * mottle + 0.25 * blades);
        let (x, z) = at_tikal(p.normalize());
        let worn = trails(x, z, n, footprint);
        let floor = grass.lerp(SASCAB * (0.85 + 0.3 * mottle), worn);
        albedo += floor.lerp(SCRUB, edge) * (1.0 - cover);
    }
    // Una orilla de lodo junto al agua.
    albedo = albedo.lerp(MUD * (0.8 + 0.4 * mottle), smoothstep(1.5, 0.3, s));
    let rock = smoothstep(0.5, 0.75, slope);
    if rock > 0.0 {
        let stone = detail::rock(f, u, 0.2, footprint, 0.8);
        albedo = albedo.lerp(LIMESTONE * (0.5 + 0.8 * stone.color.luminance() / 0.155), rock);
        normal = normal.lerp(stone.normal, rock).normalize();
    }
    pbr::shade_light(f, u, albedo, 0.9, 0.0, normal, cavity, light)
}

/// Los caminos de la plaza: uno derecho hasta el pie de la escalinata, uno
/// de lado a lado y la tierra pisada frente al templo. 0 en la grama, 1 en
/// la tierra; el borde se suaviza con el tamano del pixel.
fn trails(x: f32, z: f32, n: &Perlin, footprint: f32) -> f32 {
    let wobble = 3.5 * n.noise(Vec3::new(x * 0.02, 0.5, z * 0.02));
    let foot = TEMPLE_AHEAD as f32 - STAIR.1 * TEMPLE_SCALE;
    let main = if z < foot { (x + wobble).abs() } else { (x + wobble).hypot(z - foot) };
    // Uno que se abre hacia la izquierda, a la acropolis.
    let bend = (z - 4.0 - 0.012 * x * x + wobble).abs() + smoothstep(0.0, 4.0, x) * 50.0;
    let soft = footprint.max(0.25);
    let broken = smoothstep(-0.25, 0.25, n.noise(Vec3::new(x * 0.07, 3.0, z * 0.07)));
    let path = smoothstep(0.8 + soft, 0.8 - soft, main.min(bend)) * broken * (0.35 + 0.35 * n.noise(Vec3::new(x * 0.5, 1.0, z * 0.5)));
    let trodden = smoothstep(10.0, 4.0, x.hypot(z - foot)) * (0.5 + 0.4 * n.noise(Vec3::new(x * 0.3, 2.0, z * 0.3)));
    // De lejos no se ven.
    path.max(trodden).max(0.0) * smoothstep(1.2, 0.5, footprint)
}

/// Las copas de la selva vistas desde arriba: cada punto de un Worley
/// propio es una copa, una cupula con su verde (unas pocas en flor), con
/// sombra entre una y otra. Cada celda tiene su peso (Voronoi pesado), asi
/// las copas no son todas del mismo tamano. Cuando una copa mide pocos
/// pixeles queda su promedio. Devuelve el color, la normal y la sombra
/// entre copas.
fn canopy(f: &Fragment, n: &Perlin, footprint: f32, bajo: f32) -> (Vec3, Vec3, f32) {
    let p = f.local;
    // Manchas de selva madura (oscura), guamil (mas verde) y hojas nuevas,
    // a dos escalas.
    let region = n.fbm(p * 0.006 + Vec3::splat(1.7), 3) * 0.5 + 0.5;
    let patch = n.fbm(p * 0.03, 3) * 0.5 + 0.5;
    let tone = (region * 1.4 + patch * 0.6 - 0.75).clamp(0.0, 1.0);
    let mut base = GREENS[0].lerp(GREENS[1], smoothstep(0.1, 0.55, tone)).lerp(GREENS[2], smoothstep(0.6, 0.95, tone) * 0.7);
    base = base.lerp(GREENS[3], smoothstep(0.55, 0.8, 1.0 - patch) * 0.4);
    // En los bajos las copas son mas bajas y grises.
    base = base.lerp(BAJO, smoothstep(0.2, 0.8, bajo) * 0.7);
    let near = smoothstep(0.55, 0.2, footprint / CROWN);
    if near <= 0.0 {
        return (base, f.normal, 0.8);
    }
    let (offset, edge, id) = nearest(p / CROWN);
    let up = p.normalize();
    let relief = smoothstep(0.4, 0.15, footprint / CROWN);
    let side = f.object_to_world.mul_vec(offset - up * offset.dot(up));
    let normal = (f.normal + side * (1.5 * relief)).normalize();
    let mut leaf = base.lerp(GREENS[(id >> 8) as usize % 4], 0.35) * (0.7 + 0.6 * unit(id >> 4));
    if id % 151 < 2 {
        // Un arbol en flor: solo cuando la copa mide varios pixeles.
        leaf = leaf.lerp(BLOOMS[(id % 151) as usize], smoothstep(0.3, 0.12, footprint / CROWN));
    }
    let gap = 1.0 - smoothstep(0.3, 0.0, edge) * 0.75 * relief;
    (base.lerp(leaf, near), normal, gap)
}

/// El punto de Worley mas cercano a `p` (vector desde el punto, en celdas),
/// la distancia al borde con la celda vecina (`f2 - f1`) y el hash de su
/// celda. Cada punto pesa distinto: la distancia se divide entre su peso y
/// unos ocupan mas que otros.
fn nearest(p: Vec3) -> (Vec3, f32, u32) {
    let base = p.floor();
    let mut best = (Vec3::ZERO, f32::MAX, 0);
    let mut second = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                let h = hash_u32(
                    (cell.x as i32 as u32).wrapping_mul(0x8DA6_B343)
                        ^ (cell.y as i32 as u32).wrapping_mul(0xD816_3841)
                        ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F)
                        ^ 0x7E7E,
                );
                let (h2, h3) = (hash_u32(h), hash_u32(h ^ 0x5A5A));
                let point = cell + Vec3::new(unit(h), unit(h2), unit(h3));
                let delta = p - point;
                let weight = 0.7 + 0.6 * unit(h3 >> 3);
                let d = delta.length_squared() / (weight * weight);
                if d < best.1 {
                    second = best.1;
                    best = (delta / weight, d, h3);
                } else if d < second {
                    second = d;
                }
            }
        }
    }
    (best.0, second.sqrt() - best.1.sqrt(), best.2)
}

/// Lo que se ve reflejado en el agua en la direccion `dir`: el cielo de la
/// atmosfera sobre las estrellas.
fn sky(u: &Uniforms, dir: Vec3) -> Vec3 {
    let stars = u.sky.sample(dir);
    match u.sky_view {
        Some(view) => view.sample(dir).apply(stars),
        None => stars,
    }
}

/// Rios y lagos: el fondo se ve a traves del agua (Beer-Lambert: el rojo se
/// apaga primero), Fresnel de Schlick entre eso y el reflejo del cielo, y el
/// destello de la estrella en los rizos.
fn lake_or_river(f: &Fragment, u: &Uniforms, water: Shore, footprint: f32, light: Vec3) -> Vec3 {
    let n = u.noise;
    let t = u.time;
    let p = f.local;
    let up = p.normalize();
    let (ta, tb) = perpendiculars(up);
    // Rizos: se apagan donde un pixel cubre varias ondas.
    let calm = smoothstep(0.6, 0.1, footprint);
    let wa = n.noise(p * 0.8 + Vec3::new(t * 0.25, 0.0, t * 0.15)) * calm;
    let wb = n.noise(p * 0.8 + Vec3::new(5.2, t * 0.2, -3.1)) * calm;
    let tilt = (ta * wa + tb * wb) * 0.06;
    let normal = f.object_to_world.mul_vec((up + tilt).normalize());
    let v = f.view_dir();
    let ndv = normal.dot(v).max(1e-3);
    let fresnel = 0.02 + 0.98 * (1.0 - ndv).powi(5);
    let reflected = sky(u, normal * (2.0 * ndv) - v);
    // Lo que se ve adentro: el fondo (lodo o caliza) cada vez mas tapado
    // por el color del agua mientras mas hondo.
    let depth = (-water.distance * 1.2).clamp(0.0, 10.0);
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, f.normal));
    let sun = light * (f.normal.dot(f.light_dir(u)).max(0.0) * shadow * 3.6 / PI);
    let ambient = Vec3::new(0.05, 0.07, 0.08);
    let floor = MUD.lerp(Vec3::new(0.12, 0.14, 0.08), water.lake);
    // El rio carga lodo y su fondo se pierde enseguida; el lago es claro.
    let through = exp3(ABSORB * -(depth * (5.0 - 3.5 * water.lake)));
    let tint = RIVER_TINT.lerp(LAKE_TINT, water.lake);
    let inside = (floor * through + tint * (Vec3::ONE - through)) * (sun + ambient);
    let body = inside * (1.0 - fresnel) + reflected * fresnel;
    body + glint(f, u, normal, 0.06 + (footprint * 0.15).min(0.25), light)
}

/// Destello de la estrella en el agua: Blinn-Phong normalizado, mas ancho
/// cuando el pixel cubre muchos rizos.
fn glint(f: &Fragment, u: &Uniforms, n: Vec3, rough: f32, light: Vec3) -> Vec3 {
    let l = f.light_dir(u);
    let h = (l + f.view_dir()).normalize();
    let shine = 2.0 / (rough * rough).max(1e-3);
    let nl = n.dot(l).max(0.0);
    let spec = (shine + 8.0) / (8.0 * PI) * n.dot(h).max(0.0).powf(shine);
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, f.normal));
    light * (spec * nl * shadow * 0.08 * u.gains.jardin)
}

/// `e^v` por canal.
#[inline]
fn exp3(v: Vec3) -> Vec3 {
    Vec3::new(v.x.exp(), v.y.exp(), v.z.exp())
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

// Los objetos -----------------------------------------------------------------

/// Selva en toda la tierra seca: muchos arboles y algunas ceibas, nunca en
/// el agua, la orilla ni los claros.
pub fn rules() -> Vec<Scatter> {
    vec![
        Scatter {
            kind: PropKind::Arbol,
            cells: 58,
            scale: (0.7, 1.25),
            sink: 0.5,
            upright: true,
            accept: |density, s, slope| {
                smoothstep(4.0, 8.0, s.attrs.x) * smoothstep(0.7, 1.0, s.attrs.y) * smoothstep(0.8, 0.35, slope) * smoothstep(0.35, 0.6, density) * 0.5
            },
        },
        Scatter {
            kind: PropKind::Ceiba,
            cells: 26,
            scale: (0.85, 1.15),
            sink: 0.8,
            upright: true,
            accept: |density, s, slope| {
                smoothstep(6.0, 10.0, s.attrs.x) * smoothstep(0.8, 1.0, s.attrs.y) * smoothstep(0.5, 0.2, slope) * smoothstep(0.45, 0.7, density) * 0.4
            },
        },
    ]
}

pub fn site(_default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    let (up, forward, _) = site_axes();
    Some(("Tikal", up.as_dvec3(), forward.as_dvec3(), place))
}

/// Giro para que el frente del modelo (`+z`) mire hacia `facing` en `dir`.
fn facing_yaw(dir: DVec3, facing: DVec3) -> f64 {
    let base = upright_rotation(dir, 0.0);
    let (x0, z0) = (base.rotate(DVec3::X), base.rotate(DVec3::Z));
    facing.dot(x0).atan2(facing.dot(z0))
}

/// Tikal: la Gran Plaza despejada, el Templo del Gran Jaguar al frente
/// mirando hacia quien llega, una ceiba grande a un lado y la selva tupida
/// alrededor (mas rala lejos: de lado los arboles se tapan unos a otros).
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    let (n, forward, right) = frame;
    let local = |p: DVec3| {
        let d = p.normalize() - n;
        ((d.dot(right) * radius) as f32, (d.dot(forward) * radius) as f32)
    };
    instances.retain(|p| {
        let (x, z) = local(p.position);
        plaza_edge(x, z) > 3.0
    });
    let temple_dir = (n + forward * (TEMPLE_AHEAD / radius)).normalize();
    instances.push(at_site(radius, field, frame, PropKind::Templo, 0.0, TEMPLE_AHEAD, TEMPLE_SCALE, facing_yaw(temple_dir, -forward), 0.3));
    instances.push(at_site(radius, field, frame, PropKind::Ceiba, 50.0, -4.0, 1.05, 0.7, 0.8));
    // La selva alrededor: una grilla con un punto al azar por celda.
    let cell = 7.5;
    let reach = 230.0;
    let cells = (reach / cell) as i32;
    for j in -cells..cells {
        for i in -cells..cells {
            let h = hash_u32(seed ^ 0x7E3A ^ (i as u32).wrapping_mul(0x9E37_79B9) ^ (j as u32).wrapping_mul(0x85EB_CA6B));
            let x = (i as f64 + unit(h) as f64) * cell;
            let z = (j as f64 + unit(hash_u32(h)) as f64) * cell;
            let distance = x.hypot(z);
            let edge = plaza_edge(x as f32, z as f32);
            if distance > reach || edge < 2.0 {
                continue;
            }
            let keep = 1.0 - 0.55 * smoothstep(30.0, 200.0, edge);
            if unit(hash_u32(h ^ 0x51)) > keep {
                continue;
            }
            let dir = (n + (right * x + forward * z) / radius).normalize();
            if field.sample(dir.as_vec3()).attrs.x < 5.0 {
                continue;
            }
            // Las ceibas, lejos de la plaza: su sombra no alcanza al templo.
            let big = edge > 35.0 && unit(hash_u32(h ^ 0xCE1B)) < 0.035;
            let (kind, scale, sink) = if big {
                (PropKind::Ceiba, 0.85 + 0.25 * unit(hash_u32(h ^ 3)), 0.8)
            } else {
                (PropKind::Arbol, 0.7 + 0.6 * unit(hash_u32(h ^ 5)), 0.5)
            };
            let yaw = unit(hash_u32(h ^ 9)) as f64 * std::f64::consts::TAU;
            instances.push(at_site(radius, field, frame, kind, x, z, scale, yaw, sink));
        }
    }
}

// Los modelos -------------------------------------------------------------------

/// El Templo del Gran Jaguar, en unidades del modelo (el frente es `+z`):
/// medio ancho y medio fondo de la base y de la cima, alto de los nueve
/// cuerpos.
const BASE: (f32, f32) = (14.5, 12.5);
const SUMMIT: (f32, f32) = (7.0, 6.0);
const PYRAMID: f32 = 30.0;
const TERRACES: usize = 9;
/// Fraccion de cada cuerpo que es talud; arriba va el tablero.
const TALUD: f32 = 0.72;
/// La escalinata: medio ancho y donde empieza al pie.
const STAIR: (f32, f32) = (4.2, 17.0);
/// Alto de cada escalon de la escalinata.
const RISER: f32 = 0.42;
/// El templo de arriba: muros, techo y la cresteria.
const SHRINE: (f32, f32) = (6.0, 4.2);
const SHRINE_Z: f32 = -0.6;
const WALLS: (f32, f32) = (31.0, 36.0);
const ROOF: f32 = 38.2;
const COMB: (f32, f32, f32) = (38.2, 43.5, 47.0);
const COMB_Z: f32 = -2.0;

/// Altura del tronco de las ceibas y tamano de los racimos de hojas.
const CEIBA_TRUNK: f32 = 30.0;
const LEAF_CLUSTER: f32 = 0.7;

// Colores de vertice: marcan el material.
const STONE: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const STEPS: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const SHRINE_WALL: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const CREST: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const BARK: Vec3 = Vec3::new(1.0, 1.0, 0.0);
const LEAVES: Vec3 = Vec3::new(0.0, 1.0, 1.0);

pub fn mesh(kind: PropKind, detail: usize) -> Mesh {
    match kind {
        PropKind::Templo => temple(detail),
        PropKind::Ceiba => ceiba(detail),
        _ => tree(detail),
    }
}

/// Un cuerpo de base rectangular centrado en `z = cz`: paredes de medias
/// medidas `h0` en `y0` a `h1` en `y1`, con tapa arriba si `top`.
fn block(b: &mut MeshBuilder, y0: f32, y1: f32, h0: (f32, f32), h1: (f32, f32), cz: f32, top: bool) {
    let c0 = |sx: f32, sz: f32| Vec3::new(sx * h0.0, y0, cz + sz * h0.1);
    let c1 = |sx: f32, sz: f32| Vec3::new(sx * h1.0, y1, cz + sz * h1.1);
    b.quad(c0(-1.0, 1.0), c0(1.0, 1.0), c1(1.0, 1.0), c1(-1.0, 1.0));
    b.quad(c0(1.0, 1.0), c0(1.0, -1.0), c1(1.0, -1.0), c1(1.0, 1.0));
    b.quad(c0(1.0, -1.0), c0(-1.0, -1.0), c1(-1.0, -1.0), c1(1.0, -1.0));
    b.quad(c0(-1.0, -1.0), c0(-1.0, 1.0), c1(-1.0, 1.0), c1(-1.0, -1.0));
    if top {
        b.quad(c1(-1.0, 1.0), c1(1.0, 1.0), c1(1.0, -1.0), c1(-1.0, -1.0));
    }
}

/// Medias medidas de la base del cuerpo `k` (0 abajo, 9 la cima).
fn terrace_half(k: f32) -> (f32, f32) {
    let t = k / TERRACES as f32;
    (BASE.0 + (SUMMIT.0 - BASE.0) * t, BASE.1 + (SUMMIT.1 - BASE.1) * t)
}

/// El Templo I: nueve cuerpos escalonados (talud abajo y un tablero que
/// sobresale arriba), la escalinata al frente, el templo con su puerta y la
/// cresteria. Los escalones, la puerta y los tableros de la cresteria son
/// del shader.
fn temple(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    let rise = PYRAMID / TERRACES as f32;
    for k in 0..TERRACES {
        let y0 = k as f32 * rise;
        let (hx, hz) = terrace_half(k as f32);
        if detail == 0 {
            block(&mut b, y0, y0 + rise, (hx, hz), (hx - 0.5, hz - 0.5), 0.0, true);
        } else {
            // Talud y, encima, el tablero con su moldura.
            let talud = y0 + rise * TALUD;
            block(&mut b, y0, talud, (hx, hz), (hx - 0.55, hz - 0.55), 0.0, false);
            block(&mut b, talud, y0 + rise, (hx - 0.4, hz - 0.4), (hx - 0.4, hz - 0.4), 0.0, true);
        }
    }
    // La escalinata: una rampa empinada del pie a la cima.
    b.color(STEPS);
    let (w, foot) = STAIR;
    let (ty, tz) = (PYRAMID, SUMMIT.1);
    b.quad(Vec3::new(-w, 0.0, foot), Vec3::new(w, 0.0, foot), Vec3::new(w, ty, tz), Vec3::new(-w, ty, tz));
    b.color(STONE);
    b.triangle(Vec3::new(-w, 0.0, foot), Vec3::new(-w, ty, tz), Vec3::new(-w, 0.0, tz));
    b.triangle(Vec3::new(w, 0.0, foot), Vec3::new(w, 0.0, tz), Vec3::new(w, ty, tz));
    // El templo: basamento, muros con la puerta, el friso inclinado y la
    // cresteria en dos cuerpos sobre la mitad de atras.
    let (sx, sz) = SHRINE;
    block(&mut b, PYRAMID, WALLS.0, (sx + 0.6, sz + 0.6), (sx + 0.6, sz + 0.6), SHRINE_Z, true);
    b.color(SHRINE_WALL);
    block(&mut b, WALLS.0, WALLS.1, (sx, sz), (sx, sz), SHRINE_Z, false);
    b.color(STONE);
    block(&mut b, WALLS.1, ROOF, (sx + 0.25, sz + 0.25), (sx - 0.5, sz - 0.5), SHRINE_Z, true);
    b.color(CREST);
    if detail == 0 {
        block(&mut b, COMB.0, COMB.2, (4.6, 1.5), (3.2, 1.0), COMB_Z, true);
    } else {
        block(&mut b, COMB.0, COMB.1, (4.6, 1.5), (4.2, 1.3), COMB_Z, true);
        block(&mut b, COMB.1, COMB.2, (3.6, 1.2), (3.2, 1.0), COMB_Z, true);
    }
    b.build()
}

/// La ceiba: tronco liso y recto con contrafuertes (aletas delgadas al
/// pie), ramas casi horizontales y una copa ancha y plana de sombrilla.
fn ceiba(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(BARK);
    b.frustum(Vec3::new(0.0, -1.5, 0.0), Vec3::Y, CEIBA_TRUNK + 1.5, 1.9, 1.1, [6, 9, 12][detail], true, (false, false));
    let fins = [3, 5, 6][detail];
    for k in 0..fins {
        let a = (k as f32 + 0.3) / fins as f32 * TAU;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos()) * 0.35;
        let reach = 4.5 + 1.5 * ((k * 7) % 3) as f32 / 2.0;
        let (inner, outer, high) = (out * 1.4 - Vec3::Y, out * (1.4 + reach) - Vec3::Y, out * 1.5 + Vec3::Y * 7.0);
        b.triangle(inner + side, outer + side, high + side);
        b.triangle(inner - side, high - side, outer - side);
        b.quad(outer + side, outer - side, high - side, high + side);
    }
    let top = Vec3::new(0.0, CEIBA_TRUNK, 0.0);
    let limbs = [4, 6, 6][detail];
    let segments = [3, 4, 6][detail];
    let (rings, around) = [(3, 6), (4, 8), (5, 10)][detail];
    b.color(LEAVES);
    b.ellipsoid(top + Vec3::Y * 5.0, Vec3::new(9.5, 4.0, 9.5), rings, around, false);
    for k in 0..limbs {
        let a = (k as f32 + 0.15) / limbs as f32 * TAU;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let reach = 11.0 + 3.0 * ((k * 5) % 3) as f32 / 2.0;
        b.color(BARK);
        let path = [top - Vec3::Y * 3.0, top + out * (reach * 0.45) + Vec3::Y * 1.8, top + out * reach + Vec3::Y * 3.0];
        b.tube(&path, &[0.9, 0.55, 0.3], segments);
        b.color(LEAVES);
        b.ellipsoid(top + out * reach + Vec3::Y * 4.0, Vec3::new(7.2, 3.2, 7.2), rings, around, false);
        if detail > 0 {
            b.ellipsoid(top + out * (reach * 0.55) + Vec3::Y * 5.6, Vec3::new(5.0, 2.8, 5.0), rings, around, false);
        }
    }
    let mut mesh = b.build();
    lumpy(&mut mesh, 0xCE1B, CEIBA_TRUNK + 1.0, 1.0);
    mesh
}

/// Un arbol de la selva: tronco recto, una copa de racimos de hojas y, mas
/// abajo, el monte que crece a su sombra (asi la orilla de la selva es una
/// pared verde y no una fila de troncos). Con menos detalle los racimos se
/// juntan en menos elipsoides del mismo tamano total.
fn tree(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(BARK);
    b.frustum(Vec3::new(0.0, -1.0, 0.0), Vec3::Y, 14.0, 0.55, 0.3, [4, 5, 6][detail], detail > 0, (false, false));
    b.color(LEAVES);
    let clumps: &[(Vec3, Vec3)] = match detail {
        0 => &[(Vec3::new(0.0, 11.5, 0.0), Vec3::new(5.6, 8.0, 5.6))],
        1 => &[
            (Vec3::new(0.0, 15.4, 0.0), Vec3::new(4.8, 3.6, 4.8)),
            (Vec3::new(2.6, 12.6, 1.6), Vec3::new(3.6, 3.0, 3.6)),
            (Vec3::new(-2.6, 12.8, -1.6), Vec3::new(3.7, 3.0, 3.7)),
            (Vec3::new(0.4, 5.6, -0.6), Vec3::new(4.3, 4.0, 4.3)),
        ],
        _ => &[
            (Vec3::new(0.0, 15.8, 0.0), Vec3::new(4.6, 3.2, 4.6)),
            (Vec3::new(3.0, 13.6, 1.4), Vec3::new(3.2, 2.6, 3.2)),
            (Vec3::new(-2.7, 13.8, -1.8), Vec3::new(3.4, 2.7, 3.4)),
            (Vec3::new(-0.9, 13.2, 3.1), Vec3::new(2.9, 2.3, 2.9)),
            (Vec3::new(1.4, 14.0, -3.0), Vec3::new(2.8, 2.3, 2.8)),
            (Vec3::new(1.8, 8.0, 0.8), Vec3::new(3.3, 3.0, 3.3)),
            (Vec3::new(-1.6, 4.6, -1.0), Vec3::new(3.7, 3.5, 3.7)),
        ],
    };
    let (rings, around) = [(3, 6), (4, 7), (5, 8)][detail];
    for &(center, radii) in clumps {
        b.ellipsoid(center, radii, rings, around, false);
    }
    let mut mesh = b.build();
    lumpy(&mut mesh, 0xA4B0, 3.0, 0.7);
    mesh
}

/// Abolla las copas: lo que esta arriba de `from` y lejos del eje se mueve
/// hacia afuera o hacia adentro segun un ruido (el tronco queda derecho).
fn lumpy(mesh: &mut Mesh, seed: u32, from: f32, amount: f32) {
    let noise = Perlin::new(seed);
    mesh.deform(|p| {
        let reach = p.x.hypot(p.z);
        let weight = smoothstep(from - 1.0, from + 1.0, p.y) * smoothstep(0.7, 2.2, reach);
        if weight <= 0.0 {
            return p;
        }
        let bump = noise.noise(p * 0.45) + 0.5 * noise.noise(p * 1.1 + Vec3::splat(7.0));
        let out = Vec3::new(p.x / reach, 0.0, p.z / reach) * 0.6 + Vec3::Y * 0.4;
        p + out * (bump * amount * weight)
    });
}

// Los shaders de los objetos -----------------------------------------------------

pub fn shade_prop(kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let light = jungle_light(sun);
    match kind {
        // En la piedra la luz llega aun menos roja: que se vea gris.
        PropKind::Templo => temple_shade(f, u, style, light.lerp(Vec3::splat(light.luminance()), 0.3)),
        _ => tree_shade(kind, f, u, style, light),
    }
}

/// Una direccion del mundo en los ejes del modelo (la inversa de su giro).
#[inline]
fn to_model(f: &Fragment, v: Vec3) -> Vec3 {
    let m = &f.object_to_world.m;
    Vec3::new(
        m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
        m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
        m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
    )
}

/// Cuanto mide un pixel sobre la superficie, en unidades del modelo.
#[inline]
fn model_footprint(f: &Fragment, u: &Uniforms, scale: f32) -> f32 {
    let ndv = f.normal.dot(f.view_dir()).abs().max(0.25);
    f.pos.length() * u.pixel_angle / (ndv * scale)
}

/// Caliza gris con chorreones negros de liquen que bajan por las paredes,
/// musgo en las cornisas y una linea de sombra bajo cada tablero.
fn temple_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, light: Vec3) -> Vec3 {
    let p = f.local;
    let n = u.noise;
    let footprint = model_footprint(f, u, style.scale);
    let nm = to_model(f, f.normal);
    let rock = detail::rock(f, u, 0.5, footprint * style.scale, 0.6);
    let weather = n.fbm(p * 0.3, 3) * 0.5 + 0.5;
    // Chorreones: ruido estirado en vertical, solo en las paredes.
    let wall = smoothstep(0.75, 0.3, nm.y.abs());
    let rise = PYRAMID / TERRACES as f32;
    let k = (p.y / rise).fract();
    let runs = n.fbm(Vec3::new(p.x * 0.7, p.y * 0.1, p.z * 0.7), 3) + 0.15 * n.noise(p * 2.3);
    let streaks = smoothstep(-0.1, 0.35, runs + 0.3 * (k - 0.5)) * wall;
    let moss = smoothstep(0.55, 0.9, nm.y) * smoothstep(0.3, 0.6, n.fbm(p * 0.8, 2) * 0.5 + 0.5);
    let grit = 0.6 + 0.6 * rock.color.luminance() / 0.155;
    let mut albedo = Vec3::new(0.34, 0.34, 0.32) * (0.7 + 0.5 * weather) * grit;
    albedo = albedo.lerp(Vec3::new(0.04, 0.042, 0.038), streaks * 0.75);
    albedo = albedo.lerp(Vec3::new(0.05, 0.075, 0.025), moss);
    let mut normal = rock.normal;
    let mut cavity = rock.ao;
    if f.color.x > 0.5 {
        // Escalones: de frente se ve el peralte, arriba la huella.
        let step = (p.y / RISER).fract();
        let resolved = smoothstep(RISER * 0.8, RISER * 0.25, footprint);
        let tread = smoothstep(0.7, 0.8, step);
        let up = f.object_to_world.mul_vec(Vec3::Y);
        let front = f.object_to_world.mul_vec(Vec3::Z);
        let stepped = front.lerp(up, tread).normalize();
        normal = normal.lerp(stepped, resolved).normalize();
        cavity *= 1.0 - 0.5 * resolved * smoothstep(0.15, 0.0, step);
    } else if p.y < PYRAMID {
        // Bajo la moldura de cada tablero queda una franja en sombra, y en
        // el rincon donde un cuerpo se apoya en el de abajo.
        let under = smoothstep(TALUD - 0.12, TALUD, k) * smoothstep(TALUD + 0.02, TALUD, k);
        let corner = smoothstep(0.12, 0.0, k);
        cavity *= 1.0 - (0.6 * under + 0.45 * corner) * wall;
    }
    if f.color.y > 0.5 && nm.z > 0.5 && p.x.abs() < 1.2 && p.y < WALLS.0 + 3.4 {
        // La puerta: adentro esta oscuro.
        albedo = Vec3::splat(0.008);
    }
    pbr::shade_light(f, u, albedo, 0.85, 0.0, normal, cavity, light)
}

/// Copas y troncos de los arboles y las ceibas.
fn tree_shade(kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, light: Vec3) -> Vec3 {
    let p = f.local;
    let n = u.noise;
    if f.color.x > 0.5 {
        // Corteza: la de la ceiba gris verdosa y lisa; la de los demas,
        // cafe oscura. Vetas a lo largo y manchas de liquen.
        let streaks = n.noise(Vec3::new(p.x * 3.0, p.y * 0.3, p.z * 3.0)) * 0.5 + 0.5;
        let lichen = smoothstep(0.1, 0.5, n.fbm(p * 0.7 + Vec3::splat(style.seed * 20.0), 2));
        let bark = if kind == PropKind::Ceiba {
            Vec3::new(0.12, 0.125, 0.10).lerp(Vec3::new(0.08, 0.1, 0.05), lichen * 0.6)
        } else {
            Vec3::new(0.07, 0.06, 0.045).lerp(Vec3::new(0.05, 0.07, 0.03), lichen * 0.5)
        } * (0.7 + 0.5 * streaks);
        return pbr::shade_light(f, u, bark, 0.8, 0.0, f.normal, 1.0, light);
    }
    let footprint = model_footprint(f, u, style.scale);
    // Racimos de hojas: un Worley chico en el espacio del modelo; cada
    // racimo es una cupula con huecos oscuros entre uno y otro.
    let near = smoothstep(0.5, 0.18, footprint / LEAF_CLUSTER);
    let (mut normal, mut gap, mut shade) = (f.normal, 1.0, 1.0);
    if near > 0.0 {
        let (offset, edge, id) = nearest(p / LEAF_CLUSTER + Vec3::splat(style.seed * 40.0));
        let bumps = f.object_to_world.mul_vec(offset * 0.8 + Vec3::new(n.noise(p * 2.1), n.noise(p * 2.1 + Vec3::splat(9.0)), n.noise(p * 2.1 + Vec3::splat(17.0))) * 0.5);
        normal = (f.normal + bumps * near).normalize();
        gap = 1.0 - smoothstep(0.3, 0.0, edge) * 0.55 * near;
        shade = 1.0 + (unit(id) - 0.5) * 0.45 * near;
    }
    // Cada arbol con su verde; adentro y abajo de la copa llega menos luz.
    let tone = GREENS[(style.seed * 4.0) as usize % 4].lerp(GREENS[1], 0.3) * 1.4;
    let clumps = n.noise(p * 0.3 + Vec3::splat(style.seed * 13.0)) * 0.5 + 0.5;
    let albedo = tone * shade * (0.75 + 0.5 * clumps);
    let top = if kind == PropKind::Ceiba { CEIBA_TRUNK + 8.0 } else { 18.0 };
    let inside = 0.45 + 0.55 * smoothstep(top * 0.3, top, p.y) * (0.6 + 0.4 * to_model(f, normal).y.max(0.0));
    let lit = pbr::shade_light(f, u, albedo, 0.7, 0.0, normal, gap * inside, light);
    // Luz que pasa entre las hojas por el lado de la sombra.
    let back = (-normal.dot(f.light_dir(u))).max(0.0);
    lit + albedo * light * (back * 0.35 * gap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directions() -> impl Iterator<Item = Vec3> {
        (0..4000).map(|i| {
            let t = i as f32 * 0.37;
            Vec3::new(t.sin() * (t * 0.31).cos(), (t * 0.77).cos(), t.cos() * (t * 0.13).sin()).normalize()
        })
    }

    /// La selva se queda dentro de su rango (el culling por horizonte
    /// depende de eso) y la misma semilla da la misma selva.
    #[test]
    fn the_jungle_stays_in_range_and_is_deterministic() {
        let (a, b) = (PetenField::new(7), PetenField::new(7));
        let (lo, hi) = a.height_range();
        let near = (0..600).map(|i| {
            let t = i as f32 * 0.61;
            let around = if i % 2 == 0 { TIKAL } else { LAKES[0].0 };
            (around.normalize() + Vec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * (0.0015 * i as f32)).normalize()
        });
        for dir in directions().chain(near) {
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
            assert_eq!(s.attrs, b.sample(dir).attrs);
        }
    }
}
