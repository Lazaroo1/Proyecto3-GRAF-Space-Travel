//! Los Cuchumatanes (la version chapina de Argos): la sierra no volcanica
//! mas alta de Centroamerica, con helada y pinos; en un llano, las estelas
//! de Quirigua; en su anillo, los barriletes gigantes de Sumpango.
//!
//! # El relieve
//! - **Altiplanicie**: un ruido grande decide donde la sierra sube a la
//!   meseta. La subida pasa por una funcion de **gradas** (`terrace`): planos
//!   con una pared corta entre uno y otro, como los estratos de caliza que
//!   deja la erosion. Abajo, crestas (ruido con crestas) y lomas.
//! - **Karst**: en la meseta, dolinas (hoyos redondos donde el ruido celular
//!   de Worley queda cerca de un punto) y lapiaz (crestas finas de roca).
//! - **Canones**: donde un ruido cruza por cero, un perfil en V redondeada
//!   corta la meseta, mas hondo donde la meseta es mas alta.
//! - **El llano de Quirigua**: alrededor del lugar el relieve se aplana hacia
//!   la altura que tiene ahi, con un cinturon de bosque en la orilla.
//!
//! # El shader
//! Zacate dorado en la meseta y verde abajo, bosque de pino oscuro con copas
//! (que de cerca deja ver el suelo de agujas), caliza palida en las paredes.
//! La **helada** se forma en las hondonadas cuando la estrella esta baja: el
//! lado de la manana se sabe por el giro del planeta (la superficie avanza
//! hacia la estrella: `(eje x arriba) . luz > 0`), asi que la helada aguanta
//! la manana hasta que el sol sube y en la tarde solo aparece al ocultarse.
//! Neblina en el fondo de los canones. La luz de la estrella llega menos
//! roja: con todo el rojo el bosque se veia cafe.
//!
//! # Los objetos
//! Pinos de pisos (conos lisos con el borde irregular) y las estelas: un
//! bloque simple; las tallas son del shader. Cada cara tiene una funcion de
//! **altura del relieve** hecha de formas con distancia con signo (el
//! gobernante con su tocado y su barra ceremonial al frente y atras, y
//! columnas de glifos con numeros de barras y puntos a los lados). La
//! normal sale de diferencias finitas de esa altura con un paso del tamano
//! de un pixel, y los bordes se suavizan con ese mismo tamano: de lejos la
//! talla se funde en su promedio y no parpadea.
//!
//! # Los barriletes
//! En el plano del anillo, celdas en polares que giran en bloque. Cada
//! celda puede tener un barrilete redondo: anillos con motivos de papel de
//! china (gajos, dientes, petalos, rombos), una estrella al centro, lineas
//! negras entre papeles y flecos en el borde. Se revisan las celdas
//! vecinas y gana el de encima (se pueden traslapar). A contraluz el papel
//! deja pasar la luz como un vitral y se ve la sombra de las canas. Si un
//! barrilete mide menos que unos pixeles se dibuja su promedio.

use std::f32::consts::TAU;
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::math::{DVec3, Perlin, Vec3, hash_u32, smoothstep};
use crate::scene::Mesh;
use crate::scene::Rings;
use crate::scene::builder::MeshBuilder;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter, at_site, clear_site, upright_rotation};
use crate::shaders::rings::{RingInfo, RingStyle};
use crate::shaders::{AtmosphereParams, Fragment, PropStyle, Uniforms, detail, limb_haze, pbr};
use crate::terrain::{HeightField, TerrainSample};

pub fn spec() -> Spec {
    Spec {
        label: "Cuchumatanes",
        material: Chapin::Cuchumatanes,
        // Aire de montana: delgado, frio y limpio.
        atmosphere: Some(AtmosphereParams {
            thickness: 8.0,
            scale_height: 2.5,
            rayleigh: Vec3::new(0.0025, 0.006, 0.014),
            mie: 0.002,
            mie_g: 0.75,
            intensity: 1.2,
        }),
        rings: Some(Rings {
            inner: 1.6,
            outer: 2.4,
            style: RingStyle::Kites,
        }),
        motes: Some((Motes::Frost, 120.0)),
    }
}

// El relieve ----------------------------------------------------------------

/// Donde estan las estelas de Quirigua (en el espacio del planeta) y hacia
/// donde se mira al llegar. Cerca del polo: la estrella nunca sube mucho y
/// la luz rasante marca las tallas a cualquier hora. A los 40 s la estrella
/// queda baja, atras a la izquierda, y la Luna de Xelaju baja enfrente.
const QUIRIGUA: Vec3 = Vec3::new(-0.3261, 0.9452, -0.0189);
const TOWARD: Vec3 = Vec3::new(-0.1534, -0.0726, -0.9855);
/// Radio angular del llano: plano hasta el primero, se junta con la sierra
/// hasta el segundo.
const PLAZA: (f32, f32) = (0.2, 0.36);

/// La sierra. Atributos: `x` meseta (zacate alto), `y` hondonada (dolinas y
/// fondo de los canones: ahi se junta la helada), `z` bosque.
pub struct Sierra {
    noise: Perlin,
    /// Altura del llano de las estelas.
    plaza: f32,
}

/// Gradas: `t` (0 a 1) sube en `steps` planos con una pared corta entre uno
/// y otro.
#[inline]
fn terrace(t: f32, steps: f32) -> f32 {
    let s = t * steps;
    let floor = s.floor();
    (floor + smoothstep(0.55, 0.85, s - floor)) / steps
}

impl Sierra {
    pub fn new(seed: u32) -> Self {
        let mut sierra = Self {
            noise: Perlin::new(seed),
            plaza: 0.0,
        };
        sierra.plaza = sierra.relief(QUIRIGUA.normalize()).height;
        sierra
    }

    /// El relieve sin el llano.
    fn relief(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let warp = Vec3::new(
            n.noise(dir * 2.0 + Vec3::new(4.1, 0.0, 0.0)),
            n.noise(dir * 2.0 + Vec3::new(0.0, 9.3, 0.0)),
            n.noise(dir * 2.0 + Vec3::new(0.0, 0.0, 6.7)),
        ) * 0.1;
        let q = dir + warp;
        // Donde sube la altiplanicie, y en gradas.
        let table = smoothstep(-0.1, 0.12, n.fbm(q * 1.6 + Vec3::new(2.7, -1.3, 5.1), 4));
        let ridges = n.ridged(q * 3.2 + Vec3::new(-4.4, 2.2, 0.9), 4);
        let hills = n.fbm(q * 6.0 + Vec3::new(1.9, 7.7, -3.3), 3);
        let lowland = 1.0 + 7.0 * ridges * ridges + 1.5 * hills;
        let upland = 15.0 + 1.6 * hills + 4.0 * ridges.powi(4);
        let mut height = lowland + (upland - lowland) * terrace(table, 4.0);
        // Karst: dolinas y lapiaz, solo en la meseta.
        let (f1, _) = n.cells(q * 24.0);
        let doline = smoothstep(0.45, 0.1, f1) * table;
        let outcrop = smoothstep(0.78, 0.96, n.ridged(q * 15.0 + Vec3::new(3.3, -8.1, 2.4), 2)) * table;
        // Canones: V redondeada donde el ruido cruza por cero.
        let river = (n.noise(q * 2.3 + Vec3::new(-7.2, 3.9, 1.4)).abs() / 0.07).min(1.0);
        let gorge = (1.0 - river * river).powi(2);
        height += 1.3 * outcrop - 1.8 * doline - (4.0 + 7.0 * table) * gorge;
        // Bosque en manchas, mas abajo que en la meseta y nunca en la roca.
        let woods = n.fbm(q * 7.0 + Vec3::new(5.5, 1.1, -6.6), 3) * 0.5 + 0.5 - 0.22 * table;
        let forest = smoothstep(0.38, 0.5, woods) * (1.0 - outcrop) * (1.0 - 0.6 * gorge);
        TerrainSample {
            height,
            attrs: Vec3::new(table, doline.max(gorge), forest),
        }
    }
}

impl HeightField for Sierra {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let mut s = self.relief(dir);
        let angle = dir.dot(QUIRIGUA.normalize()).clamp(-1.0, 1.0).acos();
        if angle < PLAZA.1 {
            // El llano: casi plano, de zacate, con bosque en la orilla.
            let flat = smoothstep(PLAZA.1, PLAZA.0, angle);
            let swell = self.noise.fbm(dir * 9.0, 2) * 0.6;
            s.height += (self.plaza + swell - s.height) * flat;
            // El zacate llega hasta una orilla irregular (nunca adentro de
            // donde estan las estelas): un circulo perfecto se notaba desde
            // la orbita.
            let reach = (self.noise.fbm(dir * 5.0 + Vec3::new(3.7, 1.1, 8.2), 2) * 0.5 + 0.5) * 0.16;
            let grass = smoothstep(PLAZA.1 + reach - 0.1, PLAZA.0 + reach - 0.02, angle);
            s.attrs = s.attrs.lerp(Vec3::new(1.0, 0.0, 0.0), grass);
            let edge = smoothstep(0.2, 0.27, angle) * smoothstep(0.4, 0.3, angle);
            s.attrs.z = s.attrs.z.max(edge);
        }
        s
    }

    fn height_range(&self) -> (f32, f32) {
        // Abajo: lomas (-1.5 * 0.92), dolina y el canon mas hondo. Arriba:
        // la meseta con sus cerros y el lapiaz. El llano queda entre los dos.
        (-15.0, 23.0)
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(Sierra::new(seed))
}

// El shader del suelo ---------------------------------------------------------

const GRASS_GREEN: Vec3 = Vec3::new(0.10, 0.16, 0.045);
const GRASS_GOLD: Vec3 = Vec3::new(0.34, 0.27, 0.09);
const CANOPY: Vec3 = Vec3::new(0.022, 0.05, 0.028);
const CROWN: Vec3 = Vec3::new(0.05, 0.095, 0.045);
const NEEDLES: Vec3 = Vec3::new(0.075, 0.055, 0.035);
const LIMESTONE: Vec3 = Vec3::new(0.52, 0.5, 0.45);
const FROST: Vec3 = Vec3::new(0.82, 0.88, 0.95);
const MIST: Vec3 = Vec3::new(0.6, 0.66, 0.75);

/// La luz de la estrella sobre la sierra, menos roja.
#[inline]
fn mountain_light(sun: Vec3) -> Vec3 {
    sun.lerp(Vec3::splat(sun.luminance()), 0.4)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (plateau, hollow, forest) = (f.color.x, f.color.y, f.color.z);
    let n = u.noise;
    let p = f.local;
    let footprint = f.pos.length() * u.pixel_angle;
    let up = f.object_to_world.mul_vec(p.normalize());
    let l = f.light_dir(u);
    let steep = smoothstep(0.16, 0.42, 1.0 - f.normal.dot(up));
    let altitude = f.uv.x;
    let rock = detail::rock(f, u, 0.15, footprint, 0.9);

    // Zacate: dorado arriba, verde abajo, en manchas.
    let patches = n.fbm(p * 0.05, 3) * 0.5 + 0.5;
    let tufts = n.noise(p * 1.3) * smoothstep(0.4, 0.1, footprint);
    let grass = GRASS_GREEN.lerp(GRASS_GOLD, smoothstep(0.25, 0.8, patches * 0.7 + plateau * 0.45))
        * (0.8 + 0.25 * tufts);
    // Bosque: copas de lejos; de cerca, el suelo de agujas bajo los pinos.
    let crowns = n.noise(p * 0.55 + Vec3::splat(3.0)) * smoothstep(1.2, 0.4, footprint);
    let canopy = CANOPY.lerp(CROWN, (0.5 + 0.6 * crowns).clamp(0.0, 1.0));
    let canopy = canopy.lerp(NEEDLES, smoothstep(0.12, 0.03, footprint) * 0.7);
    let cover = smoothstep(0.25, 0.6, forest) * (1.0 - steep);
    let mut albedo = grass.lerp(canopy, cover);
    // Caliza en las paredes y donde asoma.
    let stone = LIMESTONE * (0.55 + 0.6 * rock.color.luminance());
    albedo = albedo.lerp(stone, steep.max(smoothstep(0.85, 1.0, plateau) * smoothstep(0.62, 0.8, rock.color.luminance()) * 0.6));

    // La helada: en las hondonadas y lo alto, con la estrella baja.
    let axis = f.object_to_world.mul_vec(Vec3::Y);
    let sun_height = up.dot(l);
    let morning = smoothstep(-0.08, 0.08, axis.cross(up).dot(l));
    let cold = smoothstep(0.06, -0.12, sun_height)
        + (smoothstep(0.45, 0.08, sun_height) - smoothstep(0.06, -0.12, sun_height)) * morning;
    let breakup = n.noise(p * 0.35) * 0.5 + 0.5;
    let frost = (cold * (hollow * 0.9 + 0.3 * smoothstep(0.7, 0.9, altitude))
        * smoothstep(0.25, 0.6, breakup + 0.35 * hollow))
        .max(smoothstep(0.86, 0.95, altitude) * smoothstep(0.4, 0.6, breakup) * 0.8)
        * (1.0 - cover * 0.6);
    albedo = albedo.lerp(FROST, frost);

    let normal = f.normal.lerp(rock.normal, 0.35 + 0.65 * steep).normalize();
    let rough = 0.92 - 0.35 * frost;
    let light = mountain_light(u.sun_light);
    let mut color = pbr::shade_light(f, u, albedo, rough, 0.0, normal, rock.ao, light);
    // Neblina en el fondo de los canones.
    let drift = n.fbm(p * 0.04 + Vec3::new(u.time * 0.01, 0.0, u.time * 0.006), 3) * 0.5 + 0.5;
    let mist = smoothstep(0.42, 0.22, altitude) * smoothstep(0.35, 0.75, drift) * 0.45;
    let haze = MIST * light * (0.06 + 0.3 * smoothstep(-0.1, 0.3, sun_height));
    color = color.lerp(haze, mist);
    color + limb_haze(f, Vec3::new(0.25, 0.45, 0.8), 0.3, f.normal.dot(l))
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

// Los objetos -----------------------------------------------------------------

/// Pinos en el bosque, nunca en las paredes de caliza.
pub fn rules() -> Vec<Scatter> {
    vec![Scatter {
        kind: PropKind::Pino,
        cells: 70,
        scale: (0.75, 1.45),
        sink: 0.4,
        upright: true,
        accept: |density, s, slope| {
            smoothstep(0.35, 0.7, s.attrs.z) * smoothstep(1.0, 0.45, slope) * smoothstep(0.35, 0.6, density)
        },
    }]
}

pub fn site(_default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    let normal = QUIRIGUA.normalize().as_dvec3();
    let toward = TOWARD.as_dvec3();
    let forward = (toward - normal * toward.dot(normal)).normalize();
    Some(("Quirigua", normal, forward, place))
}

/// Las estelas del llano: a `x` a la derecha y `z` al frente del lugar, su
/// escala, y cuanto se giran hacia la estrella (que a la hora de la vista
/// queda a la izquierda): la luz rasante marca la talla del frente.
const STELAE: [(f64, f64, f32); 7] = [
    // La estela E, la mas alta.
    (0.0, 8.0, 1.4),
    (-9.0, 17.0, 1.0),
    (10.0, 15.0, 1.05),
    (-17.0, 26.0, 0.95),
    (17.0, 24.0, 1.1),
    (-6.0, 32.0, 0.8),
    (6.0, 36.0, 0.9),
];

/// Quirigua: estelas en un llano de zacate, mirando hacia quien llega.
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    let (n, forward, right) = frame;
    clear_site(instances, radius, n, 50.0);
    // Quien llega mira desde atras del lugar.
    let viewer = -forward * 40.0;
    for (k, &(x, z, scale)) in STELAE.iter().enumerate() {
        let dir = (n + (right * x + forward * z) / radius).normalize();
        let base = upright_rotation(dir, 0.0);
        let (x0, z0) = (base.rotate(DVec3::X), base.rotate(DVec3::Z));
        let offset = right * x + forward * z;
        let to_viewer = (viewer - offset - dir * (viewer - offset).dot(dir)).normalize();
        // Girada hacia la izquierda (la estrella), con un poco de azar.
        let jitter = (hash_u32(seed ^ k as u32 ^ 0x57E1) >> 8) as f64 / 16_777_216.0 - 0.5;
        let turn = -0.55 + 0.3 * jitter;
        let facing = to_viewer * turn.cos() + dir.cross(to_viewer) * turn.sin();
        let yaw = facing.dot(x0).atan2(facing.dot(z0));
        instances.push(at_site(radius, field, frame, PropKind::Estela, x, z, scale, yaw, 0.08));
    }
}

/// Medio ancho, alto y medio grueso del fuste de una estela, en el modelo.
const STELA: Vec3 = Vec3::new(0.85, 10.0, 0.68);
/// Altura de la punta de los pinos, en el modelo.
const PINE_TOP: f32 = 10.0;

// Colores de vertice: marcan el material.
const CARVED: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const PLAIN: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const BARK: Vec3 = Vec3::new(1.0, 1.0, 0.0);

pub fn mesh(kind: PropKind, detail: usize) -> Mesh {
    match kind {
        PropKind::Estela => stela(detail),
        _ => pine(detail),
    }
}

/// Una estela: un fuste de piedra (enterrado un poco) sobre una losa. Las
/// tallas las pone el shader.
fn stela(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    b.color(CARVED);
    b.cuboid(Vec3::new(0.0, (STELA.y - 1.0) * 0.5, 0.0), axes, Vec3::new(STELA.x, (STELA.y + 1.0) * 0.5, STELA.z));
    b.color(PLAIN);
    b.cuboid(Vec3::new(0.0, 0.05, 0.0), axes, Vec3::new(STELA.x * 1.9, 0.3, STELA.z * 2.4));
    if detail >= 1 {
        // La corona: un bloque apenas mas angosto encima.
        b.color(CARVED);
        b.cuboid(Vec3::new(0.0, STELA.y + 0.12, 0.0), axes, Vec3::new(STELA.x * 0.92, 0.12, STELA.z * 0.9));
    }
    if detail >= 2 {
        // Piedras sueltas al pie.
        b.color(PLAIN);
        for k in 0..4u32 {
            let h = hash_u32(0x57E1A ^ k.wrapping_mul(0x9E37));
            let unit = |shift: u32| ((h >> shift) & 0xFF) as f32 / 255.0;
            let a = (k as f32 + 0.5 * unit(0)) / 4.0 * TAU;
            let r = 2.1 + 0.6 * unit(8);
            let (s, c) = (a * 1.7).sin_cos();
            b.cuboid(
                Vec3::new(a.cos() * r, 0.15, a.sin() * r),
                [Vec3::new(c, 0.0, s), Vec3::Y, Vec3::new(-s, 0.0, c)],
                Vec3::new(0.3 + 0.2 * unit(16), 0.2 + 0.12 * unit(24), 0.25 + 0.15 * unit(4)),
            );
        }
    }
    b.build()
}

/// Un pino: tronco y pisos de ramas (conos lisos que se traslapan), con el
/// borde de cada piso irregular. El azul del color de vertice lleva a que
/// altura de la copa esta cada piso.
fn pine(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(BARK);
    b.frustum(Vec3::new(0.0, -0.8, 0.0), Vec3::Y, 3.6, 0.42, 0.24, [4, 5, 6][detail], detail > 0, (false, false));
    let tiers = [2, 4, 6][detail];
    let sides = [5, 7, 10][detail];
    let bottom = 1.9;
    let spacing = (PINE_TOP - bottom) / (tiers as f32 + 0.6);
    for k in 0..tiers {
        let t = k as f32 / tiers as f32;
        let base = bottom + spacing * k as f32;
        let radius = 2.6 * (1.0 - t * 0.82);
        let top = if k + 1 == tiers { PINE_TOP } else { base + spacing * 1.9 };
        b.color(Vec3::new(0.0, 0.0, t));
        let tip = if k + 1 == tiers { 0.0 } else { radius * 0.18 };
        b.frustum(Vec3::new(0.0, base, 0.0), Vec3::Y, top - base, radius, tip, sides, true, (detail > 0, false));
    }
    let mut mesh = b.build();
    // Ramas: el borde de los pisos se abolla y se cuelga un poco.
    let noise = Perlin::new(0x914E);
    mesh.deform(|p| {
        let reach = (p.x * p.x + p.z * p.z).sqrt();
        if p.y < 1.5 {
            return p;
        }
        let bump = noise.noise(Vec3::new(p.x * 1.3, p.y * 0.7, p.z * 1.3));
        let out = 1.0 + 0.16 * bump;
        Vec3::new(p.x * out, p.y - 0.25 * reach * (0.5 + 0.5 * bump), p.z * out)
    });
    mesh
}

pub fn shade_prop(kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let light = mountain_light(sun);
    match kind {
        PropKind::Estela => stela_shade(f, u, style, light),
        _ => pine_shade(f, u, style, light),
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

/// Pino: agujas verde oscuro en racimos, mas claras en las puntas de cada
/// piso y arriba; dejan pasar un poco de luz. Cada arbol con su tono (unos
/// mas azules, como los cipreses; otros mas amarillos, como los pinos).
fn pine_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, light: Vec3) -> Vec3 {
    let p = f.local;
    if f.color.y > 0.5 {
        let streaks = u.noise.noise(Vec3::new(p.x * 6.0, p.y * 0.8, p.z * 6.0)) * 0.5 + 0.5;
        let bark = Vec3::new(0.09, 0.06, 0.045) * (0.6 + 0.6 * streaks);
        return pbr::shade_light(f, u, bark, 0.85, 0.0, f.normal, 1.0, light);
    }
    let footprint = model_footprint(f, u, style.scale);
    let fine = smoothstep(0.5, 0.15, footprint);
    let e = 0.15;
    let q = p * 2.2;
    let h0 = u.noise.noise(q);
    let bump = Vec3::new(u.noise.noise(q + Vec3::X * e) - h0, u.noise.noise(q + Vec3::Y * e) - h0, u.noise.noise(q + Vec3::Z * e) - h0) / e;
    let n = (f.normal - f.object_to_world.mul_vec(bump) * (0.35 * fine)).normalize();
    let reach = (p.x * p.x + p.z * p.z).sqrt();
    let tier = f.color.z;
    let tips = smoothstep(0.4, 1.0, reach / (2.6 * (1.0 - tier * 0.82)));
    let tone = Vec3::new(0.02, 0.055, 0.035).lerp(Vec3::new(0.045, 0.06, 0.02), style.seed);
    let albedo = tone * (0.75 + 0.5 * tips + 0.3 * tier + 0.35 * h0 * fine);
    let lit = pbr::shade_light(f, u, albedo, 0.8, 0.0, n, 0.55 + 0.45 * tips, light);
    // Luz que pasa entre las agujas por el lado de la sombra.
    let back = (-n.dot(f.light_dir(u))).max(0.0);
    lit + albedo * light * (back * 0.25 * tips)
}

// La talla de las estelas -----------------------------------------------------

/// Distancia con signo a una caja centrada en `c` con medias medidas `h` y
/// esquinas redondeadas `r`.
#[inline]
fn rounded_box(x: f32, y: f32, c: (f32, f32), h: (f32, f32), r: f32) -> f32 {
    let qx = (x - c.0).abs() - h.0 + r;
    let qy = (y - c.1).abs() - h.1 + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// Distancia con signo a un segmento de `a` a `b` con grosor `r`.
#[inline]
fn capsule(x: f32, y: f32, a: (f32, f32), b: (f32, f32), r: f32) -> f32 {
    let (px, py) = (x - a.0, y - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let t = ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0);
    (px - bx * t).hypot(py - by * t) - r
}

#[inline]
fn circle(x: f32, y: f32, c: (f32, f32), r: f32) -> f32 {
    (x - c.0).hypot(y - c.1) - r
}

/// Relieve redondeado de una forma: 1 adentro, 0 afuera, con un borde que
/// baja en `soft` (nunca menos que un pixel).
#[inline]
fn raised(d: f32, soft: f32) -> f32 {
    smoothstep(soft, -soft, d)
}

/// La cara ancha: el gobernante de pie, de frente, con la barra ceremonial
/// en los brazos y un tocado enorme de plumas. `x` de lado a lado y `y`
/// hacia arriba, en unidades del modelo. Devuelve la altura del relieve: 1
/// es la cara de la piedra, 0 el fondo tallado.
fn ruler(x: f32, y: f32, soft: f32, noise: &Perlin) -> f32 {
    let w = STELA.x;
    let ax = x.abs();
    // El marco: la piedra queda entera en el borde y abajo.
    let field = raised(rounded_box(x, y, (0.0, 5.25), (w - 0.07, 4.65), 0.05), soft);
    // El fondo esta lleno de volutas (curvas donde un ruido cruza por cero).
    let scroll = 1.0 - noise.noise(Vec3::new(x * 2.6, y * 2.6, 3.7)).abs();
    let mut h = 0.35 * smoothstep(0.8, 0.9, scroll);
    // Pies, piernas, el faldellin con sus franjas y el cinturon con mascaras.
    let feet = rounded_box(ax, y, (0.3, 0.85), (0.22, 0.22), 0.08);
    let legs = capsule(ax, y, (0.27, 1.1), (0.3, 3.0), 0.16);
    let apron = rounded_box(x, y, (0.0, 2.9), (0.22, 1.5), 0.08);
    let bands = 0.15 * smoothstep(0.35, 0.5, ((y * 2.5).fract() - 0.5).abs() * 2.0);
    h = h.max(0.8 * raised(feet.min(legs), soft)).max((0.9 - bands) * raised(apron, soft));
    let belt = rounded_box(x, y, (0.0, 4.45), (0.66, 0.2), 0.06);
    h = h.max(raised(belt, soft));
    let masks = circle(ax, y, (0.0, 4.45), 0.14).min(circle(ax, y, (0.42, 4.45), 0.12));
    h = h.max(0.75 + 0.25 * raised(masks, soft) - 0.2 * raised(masks + 0.06, soft));
    // El torso con el pectoral, los brazos y la barra con cabezas de serpiente.
    let torso = rounded_box(x, y, (0.0, 5.25), (0.46, 0.65), 0.18);
    let arms = capsule(ax, y, (0.5, 5.9), (0.42, 5.25), 0.13);
    h = h.max(0.85 * raised(torso.min(arms), soft));
    let bar = rounded_box(x, y, (0.0, 5.2), (0.62, 0.11), 0.05).min(circle(ax, y, (0.66, 5.2), 0.17));
    h = h.max(raised(bar, soft));
    let pectoral = circle(x, y, (0.0, 5.65), 0.2);
    h = h.max(0.9 + 0.1 * raised(pectoral, soft) - 0.25 * raised(pectoral + 0.08, soft));
    // La cabeza con orejeras y la cara.
    let head = ((x / 0.3).powi(2) + ((y - 6.5) / 0.4).powi(2)).sqrt() - 1.0;
    let spools = circle(ax, y, (0.4, 6.45), 0.12);
    h = h.max(raised(head * 0.3, soft)).max(0.9 * raised(spools, soft));
    let eyes = rounded_box(ax, y, (0.12, 6.58), (0.07, 0.03), 0.02);
    let mouth = rounded_box(x, y, (0.0, 6.3), (0.1, 0.025), 0.02);
    h -= 0.3 * raised(eyes.min(mouth), soft) * raised(head * 0.3, soft);
    // El tocado: bandas, una mascara al centro y un abanico de plumas.
    let crown = rounded_box(x, y, (0.0, 7.45), (0.74, 0.5), 0.15);
    let crown_bands = 0.12 * smoothstep(0.3, 0.45, ((y * 3.0).fract() - 0.5).abs() * 2.0);
    h = h.max((0.85 - crown_bands) * raised(crown, soft));
    let mask = circle(x, y, (0.0, 7.5), 0.3);
    h = h.max(raised(mask, soft) - 0.35 * raised(circle(ax, y, (0.12, 7.56), 0.06), soft));
    let (fx, fy) = (x, y - 7.9);
    let angle = fx.atan2(fy);
    let reach = fx.hypot(fy);
    let fan = smoothstep(1.25, 0.9, angle.abs()) * raised(reach - 1.75, soft) * smoothstep(0.35, 0.5, reach);
    let feathers = 1.0 - 0.3 * smoothstep(0.25, 0.5, ((angle * 4.5).fract() - 0.5).abs() * 2.0);
    h = h.max(0.8 * fan * feathers);
    // La talla queda adentro del marco; el resto es la piedra entera.
    1.0 + (h - 1.0) * field
}

/// Una cara angosta: dos columnas de glifos en cartuchos. Cada glifo sale
/// de su numero: una cabeza con un prefijo, un numero de barras y puntos
/// con su signo, o cuatro signos chicos.
fn glyphs(x: f32, y: f32, soft: f32) -> f32 {
    let w = STELA.z;
    let field = raised(rounded_box(x, y, (0.0, 5.25), (w - 0.07, 4.65), 0.05), soft);
    let cell = (w - 0.07) / 1.0;
    let (cx, cy) = ((x / cell + 1.0).floor(), ((y - 0.6) / 0.62).floor());
    if !(0.0..=1.0).contains(&cx) || !(0.0..14.0).contains(&cy) {
        return 1.0 + (0.0 - 1.0) * field;
    }
    let center = ((cx - 0.5) * cell, 0.6 + (cy + 0.5) * 0.62);
    let (lx, ly) = (x - center.0, y - center.1);
    let block = rounded_box(lx, ly, (0.0, 0.0), (cell * 0.43, 0.27), 0.07);
    let mut h = 0.9 * raised(block, soft);
    let id = hash_u32((cx as u32) ^ (cy as u32).wrapping_mul(0x9E37_79B9) ^ 0x61F);
    let inner = match id % 3 {
        // Cabeza de perfil con un prefijo delgado a la izquierda.
        0 => circle(lx, ly, (0.05, -0.01), 0.17).min(rounded_box(lx, ly, (-0.19, 0.0), (0.04, 0.17), 0.03)),
        // Numero: barras (cinco) y puntos (uno) a la izquierda y el signo.
        1 => {
            let bars = 1 + (id >> 4) % 3;
            let mut d = rounded_box(lx, ly, (0.08, 0.0), (0.13, 0.15), 0.05);
            for k in 0..bars {
                d = d.min(rounded_box(lx, ly, (-0.12 - 0.07 * k as f32, 0.0), (0.025, 0.17), 0.015));
            }
            d
        }
        // Cuatro signos chicos.
        _ => {
            let (qx, qy) = (lx.abs() - 0.11, ly.abs() - 0.12);
            rounded_box(qx, qy, (0.0, 0.0), (0.085, 0.09), 0.04)
        }
    };
    // Los signos quedan en alto, separados del cartucho por un surco.
    h -= 0.3 * raised(inner.abs() - 0.012, soft) * raised(block, soft);
    1.0 + (h - 1.0) * field
}

/// Las caras de una estela.
#[derive(Clone, Copy)]
enum Face {
    Broad,
    Narrow,
    Top,
}

/// Que tan hondo es el relieve de las estelas, en unidades del modelo.
const CARVE: f32 = 0.09;

/// Estela de Quirigua: arenisca cafe rojiza con liquen, tallada en alto
/// relieve. La normal del relieve sale de diferencias finitas de su altura,
/// con un paso del tamano de un pixel; los surcos se oscurecen.
fn stela_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, light: Vec3) -> Vec3 {
    let p = f.local;
    let footprint = model_footprint(f, u, style.scale);
    let weather = u.noise.fbm(p * 1.3 + Vec3::splat(style.seed * 40.0), 3) * 0.5 + 0.5;
    let lichen = smoothstep(0.55, 0.75, u.noise.fbm(p * 2.1 + Vec3::splat(9.0 + style.seed * 30.0), 3) * 0.5 + 0.5 + 0.25 * smoothstep(2.5, 0.0, p.y));
    let sandstone = Vec3::new(0.36, 0.22, 0.14).lerp(Vec3::new(0.46, 0.32, 0.22), weather);
    if f.color.x < 0.5 {
        let tex = detail::rock(f, u, 0.6, footprint * style.scale, 0.7);
        let albedo = sandstone * 0.8 * (0.6 + 0.6 * tex.color.luminance());
        return pbr::shade_light(f, u, albedo, 0.9, 0.0, tex.normal, tex.ao, light);
    }
    // En que cara cae: el eje dominante de la normal en el modelo. Las
    // anchas llevan al gobernante (la de atras, de espejo); las angostas,
    // glifos; arriba no hay talla.
    let nm = to_model(f, f.normal);
    let a = nm.abs();
    let soft = (footprint * 0.8).max(0.012);
    let (face, tu) = if a.z > a.x.max(a.y) {
        (Face::Broad, Vec3::X * nm.z.signum())
    } else if a.x > a.y {
        (Face::Narrow, Vec3::Z * -nm.x.signum())
    } else {
        (Face::Top, Vec3::X)
    };
    let relief = |x: f32, y: f32| match face {
        Face::Broad => ruler(x, y, soft, u.noise),
        Face::Narrow => glyphs(x, y, soft),
        Face::Top => 1.0,
    };
    let (x, y) = (p.dot(tu), p.y);
    let h = relief(x, y);
    // Diferencias finitas con el paso de un pixel: la pendiente del relieve
    // (que tan hondo es: `CARVE` unidades del modelo) inclina la normal.
    let dx = (relief(x + soft, y) - h) / soft;
    let dy = (relief(x, y + soft) - h) / soft;
    let slope = (tu * dx + Vec3::Y * dy) * CARVE;
    let normal = (f.normal - f.object_to_world.mul_vec(slope)).normalize();
    let (cavity, depth) = (0.45 + 0.55 * h, h);
    let albedo = sandstone.lerp(Vec3::new(0.07, 0.08, 0.05), lichen * 0.65)
        * (0.7 + 0.3 * depth)
        * (0.85 + 0.3 * weather);
    pbr::shade_light(f, u, albedo, 0.88, 0.0, normal, cavity, light)
}

// Los barriletes de Sumpango -------------------------------------------------

/// Lado de la celda de cada barrilete en el plano del anillo, en unidades
/// del mundo.
const KITE_CELL: f32 = 30.0;
/// Hasta donde llegan los flecos, en radios del barrilete.
const FRINGE: f32 = 1.17;
/// Cobertura media de los barriletes donde mas hay: la densidad que ve la
/// sombra del anillo y el promedio de lejos.
const KITE_COVER: f32 = 0.34;

/// Papel de china.
const PAPER: [Vec3; 9] = [
    Vec3::new(0.85, 0.02, 0.32),
    Vec3::new(0.95, 0.68, 0.02),
    Vec3::new(0.95, 0.24, 0.01),
    Vec3::new(0.0, 0.55, 0.5),
    Vec3::new(0.03, 0.14, 0.75),
    Vec3::new(0.38, 0.03, 0.62),
    Vec3::new(0.06, 0.55, 0.08),
    Vec3::new(0.78, 0.01, 0.03),
    Vec3::new(0.86, 0.84, 0.78),
];
/// Color promedio de un barrilete visto de lejos.
const CONFETTI: Vec3 = Vec3::new(0.5, 0.3, 0.25);
const INK: Vec3 = Vec3::new(0.015, 0.012, 0.012);

/// Donde vuelan los barriletes a lo ancho del anillo (0 a 1).
fn lanes(s: f32, noise: &Perlin) -> f32 {
    let edges = smoothstep(0.0, 0.08, s) * smoothstep(1.0, 0.9, s);
    let bands = noise.noise(Vec3::new(s * 5.0, 4.5, 0.5)) * 0.5 + 0.5;
    edges * (0.5 + 0.5 * bands)
}

/// Densidad del anillo (0 a ~0.9) a la fraccion `s` de su ancho (0 adentro, 1 afuera).
pub fn ring_density(s: f32, noise: &Perlin) -> f32 {
    lanes(s, noise) * KITE_COVER
}

/// Una linea de ancho `width` a distancia `d` cuando un pixel mide `aa`: si
/// es mas delgada que un pixel se ensancha y baja (como las grietas de Pira).
#[inline]
fn ink_line(d: f32, width: f32, aa: f32) -> f32 {
    smoothstep(width + aa, 0.0, d) * (width / (width + aa)).min(1.0)
}

/// Un motivo en una franja del barrilete: `t` va de 0 a 1 a lo ancho de la
/// franja y `a` de 0 a 1 en cada repeticion. Devuelve si cae en el primer
/// papel y la distancia al borde entre papeles (en fracciones de la franja
/// `band` y de la repeticion `arc`, unidades del mundo).
fn motif(kind: u32, t: f32, a: f32, band: f32, arc: f32) -> (bool, f32) {
    match kind {
        // Gajos.
        0 => (a < 0.5, (a - 0.5).abs().min(a).min(1.0 - a) * arc),
        // Dientes.
        1 => {
            let tooth = 1.0 - (2.0 * a - 1.0).abs();
            let steep = 2.0 * band / arc;
            (t < tooth, (t - tooth).abs() * band / (1.0 + steep * steep).sqrt())
        }
        // Petalos.
        2 => {
            let x = 2.0 * a - 1.0;
            let petal = 0.9 * (1.0 - x * x).max(0.0).sqrt();
            (t < petal, (t - petal).abs() * band * 0.7)
        }
        // Rombos.
        _ => {
            let d = (2.0 * a - 1.0).abs() + (2.0 * t - 1.0).abs();
            (d < 1.0, (1.0 - d).abs() * 0.5 * band.min(arc) * 0.7)
        }
    }
}

/// Un barrilete en su sistema: `(ox, oy)` desde su centro en unidades del
/// mundo. Devuelve cuanto tapa, el color del papel, cuanto deja pasar la luz
/// (el papel sin tinta ni canas) y su color promedio.
fn kite(ox: f32, oy: f32, radius: f32, id: u32, footprint: f32, time: f32) -> (f32, Vec3, f32) {
    let unit = |k: u32| (hash_u32(id ^ k.wrapping_mul(0x2C1B_3C6D)) >> 8) as f32 / 16_777_216.0;
    let paper = |k: u32| PAPER[(hash_u32(id ^ (k + 31)) % PAPER.len() as u32) as usize];
    let rho = ox.hypot(oy) / radius;
    let spin = (unit(6) - 0.5) * 0.3 * time;
    let phi = (oy.atan2(ox) + spin).rem_euclid(TAU) / TAU;
    let aa = footprint;
    // Los flecos: tiras de papel de dos colores con las puntas disparejas.
    if rho > 1.0 {
        let strips = 64.0 + 8.0 * (unit(7) * 4.0).floor();
        let k = (phi * strips).floor();
        let a = phi * strips - k;
        let width = TAU * rho * radius / strips;
        let length = 1.08 + 0.09 * ((hash_u32(id ^ k as u32) >> 8) as f32 / 16_777_216.0);
        let gap = ink_line((a - 0.5).abs().min(a).min(1.0 - a) * width, 0.08 * width, aa);
        let cover = smoothstep(length, length - 0.02 - aa / radius, rho) * (1.0 - gap);
        let color = if (k as u32 / 3) % 2 == 0 { paper(0) } else { paper(1) };
        return (cover, color, cover);
    }
    // El marco de afuera, tres franjas con motivos y la estrella del centro.
    let center = 0.2 + 0.08 * unit(1);
    let edges = [center, center + 0.24, center + 0.44, 0.88, 1.0];
    let line = 0.012 * radius;
    let (color, mut edge) = if rho < center {
        let points = 5.0 + (unit(2) * 4.0).floor();
        let a = (phi * points).fract();
        let star = 0.45 + 0.55 * (1.0 - (2.0 * a - 1.0).abs()).powi(2);
        let r = rho / center;
        let navel = r < 0.22;
        let color = if navel {
            paper(5)
        } else if r < star {
            paper(2)
        } else {
            paper(3)
        };
        let d = ((r - star).abs() * 0.6).min((r - 0.22).abs()) * center * radius;
        (color, d)
    } else if rho > edges[3] {
        (paper(0), (rho - edges[3]).abs().min(1.0 - rho) * radius)
    } else {
        let band = if rho < edges[1] { 0 } else if rho < edges[2] { 1 } else { 2 };
        let (r0, r1) = (edges[band], edges[band + 1]);
        let t = (rho - r0) / (r1 - r0);
        let repeats = [12.0, 16.0, 24.0][band] * (1.0 + (unit(10 + band as u32) * 2.0).floor());
        let a = (phi * repeats).fract();
        let kind = hash_u32(id ^ (band as u32 + 3)) % 4;
        let (first, d) = motif(kind, t, a, (r1 - r0) * radius, TAU * rho * radius / repeats);
        let pair = 4 + band as u32 * 2;
        let color = if first { paper(pair) } else { paper(pair + 1) };
        (color, d.min((rho - r0).abs().min(r1 - rho) * radius))
    };
    edge = edge.max(0.0);
    let ink = ink_line(edge, line, aa);
    // Las canas del armazon: rayos y dos aros. Solo se ven a contraluz.
    let spokes = 8.0;
    let s = (phi * spokes).fract();
    let cane = (s.min(1.0 - s) * TAU * rho * radius / spokes)
        .min((rho - 0.62).abs() * radius)
        .min((1.0 - rho).abs() * radius);
    let frame = ink_line(cane, 0.025 * radius, aa);
    let through = (1.0 - ink) * (1.0 - 0.85 * frame);
    (1.0, color.lerp(INK, ink), through)
}

/// Lo que se ve del anillo en `local` (el punto en su plano, relativo al planeta): color, opacidad,
/// luz propia y cuanto dispersa hacia adelante.
pub fn ring(ring: &RingInfo, local: Vec3, alpha: f32, _s: f32, footprint: f32, u: &Uniforms) -> (Vec3, f32, Vec3, f32) {
    // Luz que atraviesa el papel: la estrella del otro lado del plano, y
    // sin la sombra del planeta.
    let hit = ring.center + local;
    let to_sun = (u.sun_pos - hit).normalize();
    let view = (-hit).normalize();
    let behind = (-to_sun.dot(ring.normal) * view.dot(ring.normal).signum()).max(0.0);
    let b = local.dot(to_sun);
    let lit = if b < 0.0 {
        let miss = (local.length_squared() - b * b).max(0.0).sqrt();
        smoothstep(ring.planet_radius * 0.97, ring.planet_radius * 1.03, miss)
    } else {
        1.0
    };
    let shine = mountain_light(u.sun_light) * (behind.sqrt() * lit * 0.9 * u.gains.argos);
    let far = (CONFETTI, alpha, CONFETTI * shine * (alpha * 0.6), 1.2);
    let resolved = smoothstep(0.3, 0.1, footprint / KITE_CELL);
    if resolved <= 0.0 {
        return far;
    }
    // Celdas en polares que giran en bloque, a la velocidad del radio medio.
    let middle = 0.5 * (ring.inner + ring.outer);
    let spin = u.time * 450.0 / middle.powf(1.5);
    let (a, bb) = (local.dot(ring.axis_a), local.dot(ring.axis_b));
    let r = a.hypot(bb);
    let per_turn = (TAU * middle / KITE_CELL).round();
    let period = per_turn as i32;
    let x = r / KITE_CELL;
    let y = ((bb.atan2(a) + spin) / TAU).rem_euclid(1.0) * per_turn;
    // Lo que mide una celda a lo largo de la vuelta en este radio.
    let arc = TAU * r / per_turn;
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let mut best: Option<(f32, f32, f32, u32)> = None;
    let mut top = -1.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (ix + dx, iy + dy);
            let id = hash_u32((cx as u32).wrapping_mul(0x8DA6_B343) ^ (cy.rem_euclid(period) as u32).wrapping_mul(0xD816_3841) ^ 0x5A1E);
            let unit = |k: u32| (hash_u32(id ^ k) >> 8) as f32 / 16_777_216.0;
            let radius = KITE_CELL * (0.2 + 0.17 * unit(4));
            let (px, py) = (cx as f32 + 0.3 + 0.4 * unit(1), cy as f32 + 0.3 + 0.4 * unit(2));
            let (ox, oy) = ((x - px) * KITE_CELL, (y - py) * arc);
            if ox.hypot(oy) > radius * FRINGE {
                continue;
            }
            let layer = unit(5);
            if layer <= top {
                continue;
            }
            let s = (px * KITE_CELL - ring.inner) / (ring.outer - ring.inner);
            if unit(3) > lanes(s, u.noise) * 0.95 {
                continue;
            }
            top = layer;
            best = Some((ox, oy, radius, id));
        }
    }
    let (cover, color, through) = match best {
        Some((ox, oy, radius, id)) => kite(ox, oy, radius, id, footprint, u.time),
        None => (0.0, CONFETTI, 0.0),
    };
    let glow = color * shine * (through * cover);
    (
        CONFETTI.lerp(color, resolved),
        alpha + (cover - alpha) * resolved,
        far.2.lerp(glow, resolved),
        1.2,
    )
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

    /// La sierra se queda dentro de su rango (el culling por horizonte
    /// depende de eso) y la misma semilla da la misma sierra.
    #[test]
    fn the_sierra_stays_in_range_and_is_deterministic() {
        let (a, b) = (Sierra::new(7), Sierra::new(7));
        let (lo, hi) = a.height_range();
        // Tambien alrededor del llano.
        let plaza = (0..400).map(|i| {
            let t = i as f32 * 0.61;
            (QUIRIGUA.normalize() + Vec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * (0.002 * i as f32)).normalize()
        });
        for dir in directions().chain(plaza) {
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
            assert_eq!(s.attrs, b.sample(dir).attrs);
        }
    }

    /// Las estelas estan de pie en el llano de zacate, sin bosque, y el
    /// lugar mira hacia donde dice.
    #[test]
    fn the_stelae_stand_on_the_grassy_plaza() {
        let field = Sierra::new(11);
        let (_, normal, forward, place) = site((DVec3::Y, DVec3::Z)).unwrap();
        assert!(normal.dot(forward).abs() < 1e-9);
        let frame = (normal, forward, forward.cross(normal));
        let mut instances = Vec::new();
        place(&mut instances, 200.0, &field, 3, frame);
        let stelae: Vec<_> = instances.iter().filter(|p| p.kind == PropKind::Estela).collect();
        assert_eq!(stelae.len(), STELAE.len());
        for stela in stelae {
            let dir = stela.position.normalize();
            let s = field.sample(dir.as_vec3());
            assert!(s.attrs.x > 0.9 && s.attrs.z < 0.1, "{:?}", s.attrs);
            let ground = 200.0 + s.height as f64;
            assert!((stela.position.length() - ground).abs() < 0.5);
            assert!(stela.rotation.rotate(DVec3::Y).dot(dir) > 0.999);
        }
    }

    #[test]
    fn kites_stay_inside_the_ring_and_dense_lanes_cast_more_shadow() {
        let noise = Perlin::new(5);
        assert_eq!(ring_density(-0.01, &noise), 0.0);
        assert_eq!(ring_density(1.01, &noise), 0.0);
        let mid = (10..90).map(|i| ring_density(i as f32 / 100.0, &noise)).fold(0.0, f32::max);
        assert!(mid > 0.1 && mid <= KITE_COVER, "{mid}");
    }
}
