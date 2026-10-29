//! Los Cuchumatanes (la version chapina de Argos): la sierra no volcanica
//! mas alta de Centroamerica, con helada y pinos; en un llano, la iglesia de
//! San Andres Xecul; en su anillo, los barriletes gigantes de Sumpango.
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
//! - **El llano de Xecul**: alrededor del lugar el relieve se aplana hacia
//!   la altura que tiene ahi, con un cinturon de bosque en la orilla.
//!
//! # El shader
//! En el pueblo de Xecul, la plaza empedrada (lajas con juntas) frente a la
//! iglesia y calles de tierra alrededor, en los mismos ejes con que se
//! colocan las casas. Afuera, zacate dorado en la meseta y verde abajo, bosque de pino oscuro con copas
//! (que de cerca deja ver el suelo de agujas), caliza palida en las paredes.
//! La **helada** se forma en las hondonadas cuando la estrella esta baja: el
//! lado de la manana se sabe por el giro del planeta (la superficie avanza
//! hacia la estrella: `(eje x arriba) . luz > 0`), asi que la helada aguanta
//! la manana hasta que el sol sube y en la tarde solo aparece al ocultarse.
//! Neblina en el fondo de los canones. La luz de la estrella llega menos
//! roja: con todo el rojo el bosque se veia cafe.
//!
//! # Los objetos
//! El pueblo: hileras de casas (las de Antigua) alrededor de la plaza, el
//! mercado de puestos frente a la iglesia, pinos y camionetas. Pinos de
//! pisos (conos lisos con el borde irregular) y la iglesia de San Andres
//! Xecul: una nave con techo de teja y cupula de gajos, la fachada
//! alta con dos remates y dos torres con su cupula. Todo lo que hace famosa
//! a la fachada lo pinta el shader con formas de distancia con signo en los
//! ejes del modelo: columnas con franjas de colores en espiral, cornisas
//! blancas con una linea roja, angeles con alas en cada tablero, la
//! enredadera con uvas y mazorcas que corre bajo las cornisas, el medallon
//! del remate y la puerta en arco. Los bordes se suavizan con el tamano de
//! un pixel: de lejos se funde en su promedio y no parpadea.
//!
//! Los **puestos del mercado** (tambien en Antigua): cuatro postes, una
//! mesa con un corte tipico colgando al frente, montones de fruta y verdura
//! y un toldo de lona inclinado hacia atras. El shader le da a cada puesto
//! su lona (azul, roja, amarilla, verde o a rayas), al corte franjas de
//! colores con un zigzag y a cada monton lo que vende (tomate, naranja,
//! banano, aguacate, guisquil, cebolla morada), con los granos en celdas.
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

/// Donde esta la iglesia de San Andres Xecul (en el espacio del planeta) y hacia
/// donde se mira al llegar. Cerca del polo: la estrella nunca sube mucho y
/// la luz rasante marca las tallas a cualquier hora. A los 40 s la estrella
/// queda baja, atras a la izquierda, y la Luna de Xelaju baja enfrente.
const XECUL: Vec3 = Vec3::new(-0.3261, 0.9452, -0.0189);
const TOWARD: Vec3 = Vec3::new(-0.1534, -0.0726, -0.9855);
/// Radio angular del llano: plano hasta el primero, se junta con la sierra
/// hasta el segundo.
const PLAZA: (f32, f32) = (0.2, 0.36);
/// El pueblo, en el plano del lugar (x a la derecha, z hacia la iglesia):
/// medio ancho de la plaza empedrada, de donde a donde va, el ancho de las
/// calles que la rodean y donde empiezan las casas de los lados.
const SQUARE: (f32, f32, f32) = (15.0, -16.0, 12.0);
const LANE: f32 = 3.2;
const HOUSES: f32 = SQUARE.0 + LANE + 0.4;

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
        sierra.plaza = sierra.relief(XECUL.normalize()).height;
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
        let angle = dir.dot(XECUL.normalize()).clamp(-1.0, 1.0).acos();
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
    // El pueblo: la plaza empedrada y las calles de tierra.
    let (paving, street) = town_ground(p, footprint, n);
    if paving.1 > 0.0 {
        albedo = albedo.lerp(paving.0, paving.1);
    }
    if street.1 > 0.0 {
        albedo = albedo.lerp(street.0, street.1);
    }
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

/// Ejes del pueblo de Xecul: vertical, hacia la iglesia y a la derecha.
fn town_axes() -> (Vec3, Vec3, Vec3) {
    let up = XECUL.normalize();
    let forward = (TOWARD - up * TOWARD.dot(up)).normalize();
    (up, forward, forward.cross(up))
}

const FLAGSTONE: Vec3 = Vec3::new(0.3, 0.27, 0.24);
const DIRT: Vec3 = Vec3::new(0.16, 0.11, 0.07);

/// El suelo del pueblo en `p` (espacio del planeta): el color y cuanto
/// cubre la plaza empedrada, y lo mismo de las calles de tierra.
fn town_ground(p: Vec3, footprint: f32, n: &Perlin) -> ((Vec3, f32), (Vec3, f32)) {
    let (up, forward, right) = town_axes();
    let none = ((Vec3::ZERO, 0.0), (Vec3::ZERO, 0.0));
    if p.normalize().dot(up) < 0.97 {
        return none;
    }
    let (x, z) = (p.dot(right), p.dot(forward));
    let aa = footprint.max(0.02);
    let (half, z0, z1) = SQUARE;
    // La plaza: un rectangulo hasta la iglesia.
    let square = (x.abs() - half).max(z0 - z).max(z - z1 - 6.0);
    let on_square = smoothstep(aa, -aa, square);
    // Lajas: juntas en una cuadricula, cada laja con su tono.
    let (gx, gz) = (x / 1.4, z / 0.9 + 0.5 * (x / 1.4).floor());
    let joint = (gx - gx.round()).abs().min((gz - gz.round()).abs());
    let mortar = smoothstep(0.06 + aa, 0.02, joint) * smoothstep(0.4, 0.1, aa);
    let tone = n.noise(Vec3::new(gx.floor() * 3.1, gz.floor() * 2.3, 0.7)) * 0.5 + 0.5;
    let flag = FLAGSTONE * (0.75 + 0.4 * tone) * (1.0 - 0.45 * mortar);
    // Las calles: a los lados y enfrente de la plaza, la de entrada y la
    // de atras de las casas de los lados.
    let sides = ((x.abs() - half - LANE * 0.5).abs() - LANE * 0.5).max(z0 - LANE - z).max(z - 46.0);
    let front = ((z - z0 + LANE * 0.5).abs() - LANE * 0.5).max(x.abs() - half - LANE);
    let entry = (x.abs() - 2.5).max(z - z0).max(-60.0 - z);
    let back = ((x.abs() - HOUSES - 16.0).abs() - 2.5).max(z0 - 4.0 - z).max(z - 46.0);
    let road = sides.min(front).min(entry).min(back);
    let on_road = smoothstep(aa, -aa, road) * (1.0 - on_square);
    let ruts = n.fbm(Vec3::new(x * 0.6, z * 0.15, 4.2), 2) * 0.5 + 0.5;
    let dirt = DIRT * (0.7 + 0.5 * ruts);
    ((flag, on_square), (dirt, on_road))
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
    let normal = XECUL.normalize().as_dvec3();
    let toward = TOWARD.as_dvec3();
    let forward = (toward - normal * toward.dot(normal)).normalize();
    Some(("San Andres Xecul", normal, forward, place))
}

/// Que tan adelante del lugar esta la fachada de la iglesia.
const CHURCH_AHEAD: f64 = 14.0;

/// San Andres Xecul: la iglesia al fondo de la plaza, con la fachada hacia
/// quien llega (que mira desde atras del lugar); el mercado en la plaza,
/// casas alrededor, pinos y camionetas.
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    let (n, forward, _) = frame;
    clear_site(instances, radius, n, 55.0);
    town(instances, radius, field, seed, frame);
    let dir = (n + forward * (CHURCH_AHEAD / radius)).normalize();
    let base = upright_rotation(dir, 0.0);
    let (x0, z0) = (base.rotate(DVec3::X), base.rotate(DVec3::Z));
    let toward = (-forward - dir * (-forward).dot(dir)).normalize();
    let yaw = toward.dot(x0).atan2(toward.dot(z0));
    instances.push(at_site(radius, field, frame, PropKind::Iglesia, 0.0, CHURCH_AHEAD, 1.0, yaw, 0.3));
}

/// Un numero de 0 a 1 para el elemento `k` de algo con semilla `seed`.
#[inline]
fn chance(seed: u32, k: u32) -> f64 {
    (hash_u32(seed ^ k.wrapping_mul(0x9E37_79B9)) >> 8) as f64 / 16_777_216.0
}

/// El pueblo alrededor de la plaza.
fn town(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    use super::fuego::{Lot, place_lots, row};
    let (half, z0, z1) = (SQUARE.0 as f64, SQUARE.1 as f64, SQUARE.2 as f64);
    let houses = HOUSES as f64;
    let mut lots = Vec::new();
    for side in [-1.0, 1.0] {
        let s = seed ^ (side > 0.0) as u32 * 0x5151;
        // Las casas de los lados de la plaza y, detras, otra hilera de
        // espaldas a una calle mas.
        row(&mut lots, (side * houses, z0 - 4.0), (side * houses, 46.0), (-side, 0.0), s ^ 0x1);
        row(&mut lots, (side * (houses + 13.0), z0 - 4.0), (side * (houses + 13.0), 46.0), (side, 0.0), s ^ 0x2);
        row(&mut lots, (side * (houses + 19.0), z0 - 4.0), (side * (houses + 19.0), 40.0), (-side, 0.0), s ^ 0x3);
        // Del lado de quien llega, a los lados de la calle de entrada.
        row(&mut lots, (side * 4.0, z0 - LANE as f64 - 0.5), (side * (houses + 10.0), z0 - LANE as f64 - 0.5), (0.0, 1.0), s ^ 0x4);
        // El mercado: filas de puestos a los lados del paso central, de
        // frente al paso.
        for (r, z) in [-11.0, -6.0, -1.0, 4.0].into_iter().enumerate() {
            for (k, x) in [4.5, 8.0, 11.5].into_iter().enumerate() {
                let h = s ^ (r as u32) << 4 ^ (k as u32) << 8;
                if chance(h, 0) < 0.12 {
                    continue;
                }
                lots.push(Lot { kind: PropKind::Puesto, x: side * x, z, scale: 1.2, facing: (-side, 0.0) });
            }
        }
        // Pinos entre las casas y el bosque.
        for i in 0..14u32 {
            let h = s ^ 0x9140 ^ i << 6;
            let x = side * (houses + 24.0 + 12.0 * chance(h, 0));
            let z = z0 - 10.0 + 70.0 * chance(h, 1);
            let way = chance(h, 2) * std::f64::consts::TAU;
            lots.push(Lot { kind: PropKind::Pino, x, z, scale: (0.8 + 0.5 * chance(h, 3)) as f32, facing: (way.cos(), way.sin()) });
        }
        // Una camioneta en la calle de la plaza.
        let way = if side > 0.0 { 1.0 } else { -1.0 };
        lots.push(Lot { kind: PropKind::Camioneta, x: side * (half + LANE as f64 * 0.5), z: -2.0 + 10.0 * chance(s, 9), scale: 2.0, facing: (0.0, way) });
    }
    // Detras de la iglesia, casas de espaldas a la sierra.
    row(&mut lots, (-houses, 48.0), (houses, 48.0), (0.0, -1.0), seed ^ 0xBAC);
    let _ = z1;
    place_lots(instances, radius, field, frame, &lots, 1.0);
}

/// La iglesia, en el modelo (la fachada mira hacia +z): medio ancho de la
/// fachada, su alto, el largo de la nave y el alto de sus paredes.
const FACADE: (f32, f32) = (7.0, 15.0);
const NAVE: (f32, f32) = (20.0, 10.0);
/// Las torres: cuanto se separan del centro, su medio ancho y su alto.
const TOWER: (f32, f32, f32) = (8.6, 1.6, 17.0);

/// Altura de la punta de los pinos, en el modelo.
const PINE_TOP: f32 = 10.0;

// Colores de vertice: marcan el material.
const BARK: Vec3 = Vec3::new(1.0, 1.0, 0.0);
/// En la iglesia: la fachada pintada, las paredes lisas, la teja, las
/// cupulas, lo blanco y la piedra de las gradas.
const FRONT: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const WALL: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const TILE: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const DOME: Vec3 = Vec3::new(1.0, 0.0, 1.0);
const WHITE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
const STEPS: Vec3 = Vec3::new(0.0, 1.0, 1.0);

pub fn mesh(kind: PropKind, detail: usize) -> Mesh {
    match kind {
        PropKind::Iglesia => church(detail),
        PropKind::Puesto => stall(detail),
        _ => pine(detail),
    }
}

/// Un puesto, en el modelo (mira hacia +z): medio ancho y medio fondo, alto
/// de la mesa y de los postes de adelante (los de atras son mas altos).
const STALL: (f32, f32) = (1.1, 0.85);
const TABLE: f32 = 0.6;
const POSTS: f32 = 1.6;
/// Materiales del puesto (por color de vertice).
const POST: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const BOARD: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const CLOTH: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const GOODS: Vec3 = Vec3::new(1.0, 1.0, 0.0);
const TARP: Vec3 = Vec3::new(1.0, 0.0, 1.0);

fn stall(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let (hx, hz) = STALL;
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let back = POSTS + 0.35;
    b.color(POST);
    for (x, z) in [(-hx, -hz), (hx, -hz), (-hx, hz), (hx, hz)] {
        let top = if z < 0.0 { back } else { POSTS };
        b.cuboid(Vec3::new(x, top * 0.5, z), axes, Vec3::new(0.04, top * 0.5, 0.04));
    }
    // La mesa y el corte que cuelga al frente.
    b.color(BOARD);
    b.cuboid(Vec3::new(0.0, TABLE - 0.03, 0.1), axes, Vec3::new(hx - 0.06, 0.03, hz * 0.6));
    if detail >= 1 {
        b.color(CLOTH);
        b.cuboid(Vec3::new(0.0, TABLE * 0.5, 0.1 + hz * 0.6 + 0.01), axes, Vec3::new(hx - 0.06, TABLE * 0.5 - 0.02, 0.01));
    }
    // Los montones de lo que se vende, en dos filas.
    b.color(GOODS);
    let per_row = [2, 3, 4][detail];
    let rows = if detail == 0 { 1 } else { 2 };
    let (rings, segments) = ([2, 3, 4][detail], [5, 7, 9][detail]);
    let width = (2.0 * hx - 0.3) / per_row as f32;
    for row in 0..rows {
        for k in 0..per_row {
            let x = -hx + 0.15 + width * (k as f32 + 0.5);
            let z = if rows == 1 { 0.1 } else { -0.15 + 0.45 * row as f32 };
            b.ellipsoid(Vec3::new(x, TABLE, z), Vec3::new(width * 0.42, 0.14, 0.2), rings, segments, true);
        }
    }
    // El toldo: una lona inclinada, mas alta atras, que sobresale.
    b.color(TARP);
    let slope = Vec3::new(0.0, back - POSTS, -2.0 * hz).normalize();
    let up = Vec3::X.cross(slope);
    let length = (2.0 * hz + 0.5) * 0.5;
    let middle = Vec3::new(0.0, (back + POSTS) * 0.5 + 0.03, 0.0) + slope * 0.0;
    b.cuboid(middle, [Vec3::X, up, slope], Vec3::new(hx + 0.18, 0.015, length));
    b.build()
}

/// La iglesia: la nave con su techo de dos aguas y su cupula, la fachada
/// mas alta que la nave con dos remates escalonados y una cruz, dos torres
/// con cupula y las gradas del atrio. El detalle se pinta en el shader.
fn church(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let (w, h) = FACADE;
    let (length, wall) = NAVE;
    let (rings, segments) = [(3, 8), (4, 12), (6, 16)][detail];
    // La nave, detras de la fachada.
    b.color(WALL);
    b.cuboid(Vec3::new(0.0, wall * 0.5 - 0.5, -length * 0.5), axes, Vec3::new(w - 0.6, wall * 0.5 + 0.5, length * 0.5));
    // El techo de dos aguas.
    b.color(TILE);
    let (eave, ridge, half) = (wall, wall + 3.0, w - 0.2);
    let back = -length - 0.3;
    b.quad(
        Vec3::new(-half, eave, 0.0),
        Vec3::new(-half, eave, back),
        Vec3::new(0.0, ridge, back),
        Vec3::new(0.0, ridge, 0.0),
    );
    b.quad(
        Vec3::new(half, eave, back),
        Vec3::new(half, eave, 0.0),
        Vec3::new(0.0, ridge, 0.0),
        Vec3::new(0.0, ridge, back),
    );
    b.color(WALL);
    b.triangle(Vec3::new(-half, eave, back), Vec3::new(half, eave, back), Vec3::new(0.0, ridge, back));
    // La cupula sobre el crucero, en su tambor.
    b.frustum(Vec3::new(0.0, wall, -length * 0.7), Vec3::Y, 3.6, 3.4, 3.4, segments, true, (false, false));
    b.color(DOME);
    b.ellipsoid(Vec3::new(0.0, wall + 3.6, -length * 0.7), Vec3::new(3.5, 3.2, 3.5), rings, segments, true);
    // La fachada y sus remates.
    b.color(FRONT);
    b.cuboid(Vec3::new(0.0, h * 0.5 - 0.5, 0.6), axes, Vec3::new(w, h * 0.5 + 0.5, 0.6));
    b.cuboid(Vec3::new(0.0, h + 1.5, 0.5), axes, Vec3::new(w * 0.58, 1.5, 0.5));
    b.cuboid(Vec3::new(0.0, h + 3.8, 0.45), axes, Vec3::new(w * 0.28, 0.8, 0.45));
    b.color(WHITE);
    // Cornisas que sobresalen.
    for y in [5.0, 10.0, h] {
        b.cuboid(Vec3::new(0.0, y, 0.7), axes, Vec3::new(w + 0.25, 0.22, 0.75));
    }
    if detail >= 1 {
        // La cruz de arriba.
        b.cuboid(Vec3::new(0.0, h + 5.6, 0.45), axes, Vec3::new(0.12, 1.0, 0.12));
        b.cuboid(Vec3::new(0.0, h + 5.9, 0.45), axes, Vec3::new(0.5, 0.12, 0.12));
    }
    // Las torres con su cupula y su linterna.
    let (side, tower, top) = TOWER;
    for x in [-side, side] {
        b.color(FRONT);
        b.cuboid(Vec3::new(x, top * 0.5 - 0.5, 0.0), axes, Vec3::new(tower, top * 0.5 + 0.5, tower));
        b.color(WHITE);
        b.cuboid(Vec3::new(x, top, 0.0), axes, Vec3::new(tower + 0.2, 0.2, tower + 0.2));
        b.color(DOME);
        b.ellipsoid(Vec3::new(x, top + 0.2, 0.0), Vec3::new(tower * 0.95, 1.9, tower * 0.95), rings, segments, true);
        if detail >= 2 {
            b.color(WHITE);
            b.frustum(Vec3::new(x, top + 2.0, 0.0), Vec3::Y, 1.0, 0.3, 0.05, 6, false, (false, false));
        }
    }
    // Las gradas del atrio.
    b.color(STEPS);
    b.cuboid(Vec3::new(0.0, -0.1, 3.4), axes, Vec3::new(side + tower + 1.5, 0.45, 2.6));
    if detail >= 1 {
        b.cuboid(Vec3::new(0.0, 0.3, 2.0), axes, Vec3::new(side + tower + 0.5, 0.35, 1.2));
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
        PropKind::Iglesia => church_shade(f, u, style, light),
        PropKind::Puesto => stall_shade(f, u, style, sun),
        _ => pine_shade(f, u, style, light),
    }
}

/// Lonas de los toldos: la azul de siempre, roja, amarilla, verde y
/// anaranjada.
const TARPS: [Vec3; 5] = [
    Vec3::new(0.03, 0.16, 0.55),
    Vec3::new(0.6, 0.04, 0.035),
    Vec3::new(0.75, 0.48, 0.02),
    Vec3::new(0.04, 0.32, 0.1),
    Vec3::new(0.8, 0.22, 0.02),
];
/// Lo que se vende: tomate, naranja, banano, aguacate, guisquil y cebolla
/// morada.
const PRODUCE: [Vec3; 6] = [
    Vec3::new(0.6, 0.03, 0.02),
    Vec3::new(0.8, 0.28, 0.01),
    Vec3::new(0.75, 0.6, 0.06),
    Vec3::new(0.03, 0.08, 0.02),
    Vec3::new(0.25, 0.42, 0.08),
    Vec3::new(0.3, 0.04, 0.2),
];
/// Hilos del corte tipico.
const THREADS: [Vec3; 6] = [RED, BLUE, PURPLE, GREEN, YELLOW, ROSE];

fn stall_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    let code = |v: Vec3| (v.x > 0.5, v.y > 0.5, v.z > 0.5);
    let is = |material: Vec3| code(f.color) == code(material);
    let id = (style.seed * 16_777_216.0) as u32;
    let pick = |k: u32| hash_u32(id ^ k.wrapping_mul(0x9E37_79B9));
    let soft = (model_footprint(f, u, style.scale) * 0.7).max(0.01);
    let (albedo, rough) = if is(TARP) {
        let tarp = TARPS[(pick(1) % TARPS.len() as u32) as usize];
        // Unas a rayas con blanco.
        let striped = pick(2) % 3 == 0;
        let stripe = if striped { smoothstep(-soft, soft, (p.x * 2.5).fract() - 0.5) } else { 0.0 };
        let worn = u.noise.fbm(p * 3.0 + Vec3::splat(style.seed * 20.0), 2) * 0.5 + 0.5;
        (tarp.lerp(LIME, stripe) * (0.75 + 0.35 * worn), 0.7)
    } else if is(CLOTH) {
        // Franjas de colores con un zigzag, como un corte.
        let row = (p.y * 18.0).floor();
        let thread = THREADS[(hash_u32(id ^ row as i32 as u32) % THREADS.len() as u32) as usize];
        let zig = ((p.x * 6.0).fract() - 0.5).abs() * 2.0;
        let band = ((p.y * 6.0).fract() - zig * 0.5).abs();
        (thread.lerp(LIME, smoothstep(0.08 + soft, 0.08, band) * 0.8), 0.95)
    } else if is(GOODS) {
        // Cada monton es de una cosa; los granos son celdas de Worley.
        let pile = ((p.x + STALL.0) * 1.6).floor() + 3.0 * (p.z > 0.05) as u8 as f32;
        let fruit = PRODUCE[(hash_u32(id ^ pile as i32 as u32 ^ 0xF00D) % PRODUCE.len() as u32) as usize];
        let (f1, f2) = u.noise.cells(p * 9.0);
        let grain = smoothstep(0.05, 0.2, f2 - f1) * smoothstep(0.04, 0.012, soft);
        (fruit * (0.55 + 0.45 * (1.0 - f1).max(0.0)) * (1.0 - 0.4 * (1.0 - grain) * smoothstep(0.04, 0.012, soft)), 0.45)
    } else {
        let streaks = u.noise.noise(Vec3::new(p.x * 9.0, p.y * 2.0, p.z * 9.0)) * 0.5 + 0.5;
        (WOOD * (0.8 + 0.6 * streaks), 0.85)
    };
    let lit = pbr::shade_light(f, u, albedo, rough, 0.0, f.normal, 1.0, sun);
    if is(TARP) {
        // La lona deja pasar la luz: vista desde abajo se enciende.
        let through = (-f.normal.dot(f.light_dir(u))).max(0.0);
        return lit + albedo * sun * (through * 0.35);
    }
    lit
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

// La fachada de la iglesia -----------------------------------------------------

/// Distancia con signo a una caja centrada en `c` con medias medidas `h` y
/// esquinas redondeadas `r`.
#[inline]
fn rounded_box(x: f32, y: f32, c: (f32, f32), h: (f32, f32), r: f32) -> f32 {
    let qx = (x - c.0).abs() - h.0 + r;
    let qy = (y - c.1).abs() - h.1 + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}


#[inline]
fn circle(x: f32, y: f32, c: (f32, f32), r: f32) -> f32 {
    (x - c.0).hypot(y - c.1) - r
}


/// Lo amarillo de la iglesia (cal pintada) y los colores de la fachada.
const YELLOW: Vec3 = Vec3::new(0.82, 0.55, 0.05);
const LIME: Vec3 = Vec3::new(0.82, 0.8, 0.74);
const ROSE: Vec3 = Vec3::new(0.85, 0.32, 0.38);
const RED: Vec3 = Vec3::new(0.7, 0.05, 0.05);
const BLUE: Vec3 = Vec3::new(0.06, 0.24, 0.62);
const SKY_BLUE: Vec3 = Vec3::new(0.3, 0.55, 0.85);
const GREEN: Vec3 = Vec3::new(0.08, 0.42, 0.16);
const PURPLE: Vec3 = Vec3::new(0.32, 0.08, 0.42);
const WOOD: Vec3 = Vec3::new(0.12, 0.05, 0.02);
const TERRACOTTA: Vec3 = Vec3::new(0.55, 0.2, 0.08);

/// Una forma pintada en la fachada: cubre `color` donde la distancia con
/// signo `d` es negativa, con el borde suavizado un pixel (`soft`).
#[inline]
fn paint(base: Vec3, color: Vec3, d: f32, soft: f32) -> Vec3 {
    base.lerp(color, smoothstep(soft, -soft, d))
}

/// El frente de la fachada en `(x, y)` (unidades del modelo, `x` de lado a
/// lado, `y` hacia arriba): todo lo que la hace famosa.
fn facade(x: f32, y: f32, soft: f32) -> Vec3 {
    let (w, h) = FACADE;
    let mut c = YELLOW;
    // Columnas con franjas en espiral: rojo, blanco, azul y verde.
    for column in [-6.4f32, -4.0, -1.8, 1.8, 4.0, 6.4] {
        let d = (x - column).abs() - 0.38;
        if d < soft && y < h {
            let band = ((y + (x - column) * 0.8) * 1.6).rem_euclid(4.0) as u32;
            let stripe = [RED, LIME, BLUE, GREEN][band as usize % 4];
            c = paint(c, stripe, d, soft);
        }
    }
    // Las cornisas: blancas con una linea roja.
    for level in [5.0f32, 10.0, h] {
        c = paint(c, LIME, (y - level).abs() - 0.3, soft);
        c = paint(c, RED, (y - level - 0.38).abs() - 0.06, soft);
    }
    // La enredadera bajo cada cornisa: un tallo verde que ondula con hojas,
    // uvas moradas y de vez en cuando una mazorca amarilla.
    for level in [5.0f32, 10.0, h] {
        let v = y - (level - 0.85 + 0.22 * (x * 2.2).sin());
        c = paint(c, GREEN, v.abs() - 0.07, soft);
        let leaf = (x * 2.2 / std::f32::consts::PI).round() * std::f32::consts::PI / 2.2;
        c = paint(c, GREEN, circle(x, y, (leaf + 0.25, level - 0.6), 0.16), soft);
        let k = (x * 1.1).floor();
        let grape = (x * 1.1).fract() - 0.5;
        if (k as i32).rem_euclid(3) == 0 {
            c = paint(c, YELLOW * 1.2, rounded_box(grape, y - (level - 1.15), (0.0, 0.0), (0.12, 0.28), 0.1), soft);
        } else {
            for (gx, gy) in [(-0.12f32, 0.0f32), (0.12, 0.0), (0.0, -0.2)] {
                c = paint(c, PURPLE, circle(grape, y - (level - 1.1), (gx, gy), 0.1), soft);
            }
        }
    }
    // Un angel en cada tablero entre columnas: cabeza, tunica de color, alas
    // blancas abiertas. Los tableros del centro llevan la puerta y la ventana.
    let panels = [(-5.2f32, BLUE), (-2.9, ROSE), (2.9, SKY_BLUE), (5.2, ROSE)];
    for &(cx, robe) in &panels {
        for (story, cy) in [(0u32, 2.2f32), (1, 7.2), (2, 12.2)] {
            let robe = if story == 1 { GREEN } else { robe };
            let wings = circle(x, y, (cx - 0.45, cy + 0.6), 0.45).min(circle(x, y, (cx + 0.45, cy + 0.6), 0.45));
            c = paint(c, LIME, wings, soft);
            c = paint(c, robe, rounded_box(x, y, (cx, cy), (0.32 + 0.15 * (cy + 0.5 - y).max(0.0), 0.75), 0.15), soft);
            c = paint(c, ROSE * 1.1 + Vec3::splat(0.1), circle(x, y, (cx, cy + 1.05), 0.26), soft);
            c = paint(c, YELLOW * 1.3, (circle(x, y, (cx, cy + 1.15), 0.4)).abs() - 0.04, soft);
        }
    }
    // La puerta en arco con su marco blanco, y la ventana del coro.
    let door = rounded_box(x, y, (0.0, 1.6), (1.15, 2.2), 0.0).min(circle(x, y, (0.0, 3.8), 1.15));
    c = paint(c, LIME, door - 0.3, soft);
    c = paint(c, WOOD, door, soft);
    c = paint(c, WOOD * 0.6, (x.abs() - 0.04).max(door), soft);
    let window = rounded_box(x, y, (0.0, 7.2), (0.6, 0.9), 0.0).min(circle(x, y, (0.0, 8.1), 0.6));
    c = paint(c, LIME, window - 0.2, soft);
    c = paint(c, Vec3::new(0.02, 0.03, 0.05), window, soft);
    // El medallon del remate: azul con un sol blanco.
    let medal = circle(x, y, (0.0, h + 1.5), 1.1);
    c = paint(c, LIME, medal - 0.15, soft);
    c = paint(c, BLUE, medal, soft);
    c = paint(c, LIME, circle(x, y, (0.0, h + 1.5), 0.45), soft);
    // El borde de la fachada: una franja roja.
    c = paint(c, RED, w - 0.2 - x.abs(), soft);
    c
}

/// Las torres por el frente: campanario con su arco oscuro arriba, una
/// columna de franjas a cada lado y un angel.
fn tower_front(x: f32, y: f32, soft: f32) -> Vec3 {
    let (_, tower, top) = TOWER;
    let mut c = YELLOW;
    for edge in [-1.0f32, 1.0] {
        let d = (x - edge * (tower - 0.3)).abs() - 0.22;
        let band = ((y * 1.6).rem_euclid(3.0)) as usize;
        c = paint(c, [RED, LIME, BLUE][band], d, soft);
    }
    let bell = rounded_box(x, y, (0.0, top - 3.2), (0.7, 1.0), 0.0).min(circle(x, y, (0.0, top - 2.2), 0.7));
    c = paint(c, LIME, bell - 0.15, soft);
    c = paint(c, Vec3::new(0.02, 0.02, 0.02), bell, soft);
    c = paint(c, ROSE, circle(x, y, (0.0, 9.0), 0.5), soft);
    c = paint(c, LIME, (y - 5.0).abs() - 0.25, soft);
    c = paint(c, LIME, (y - 10.0).abs() - 0.25, soft);
    c
}

/// La iglesia de San Andres Xecul: amarilla y pintada por delante, lisa a
/// los lados; teja de barro, cupulas de gajos rojos y blancos, cal blanca y
/// piedra en las gradas.
fn church_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, light: Vec3) -> Vec3 {
    let p = f.local;
    // El color de vertice llega interpolado (1.0000001 no es 1): se compara
    // cada canal contra la mitad.
    let code = |v: Vec3| (v.x > 0.5, v.y > 0.5, v.z > 0.5);
    let c = code(f.color);
    let is = |material: Vec3| c == code(material);
    let footprint = model_footprint(f, u, style.scale);
    let soft = (footprint * 0.7).max(0.02);
    let grime = u.noise.fbm(p * 0.6, 3) * 0.5 + 0.5;
    let (albedo, rough) = if is(FRONT) && to_model(f, f.normal).z > 0.7 {
        let in_tower = p.x.abs() > TOWER.0 - TOWER.1 - 0.05;
        let x = if in_tower { p.x - TOWER.0 * p.x.signum() } else { p.x };
        let art = if in_tower { tower_front(x, p.y, soft) } else { facade(x, p.y, soft) };
        (art * (0.85 + 0.2 * grime), 0.75)
    } else if is(FRONT) || is(WALL) {
        (YELLOW * (0.75 + 0.3 * grime), 0.85)
    } else if is(TILE) {
        let channel = ((p.x * 3.0).fract() - 0.5).abs() * 2.0;
        (TERRACOTTA * (0.6 + 0.4 * channel) * (0.85 + 0.3 * grime), 0.75)
    } else if is(DOME) {
        // Gajos rojos y blancos alrededor del eje de cada cupula.
        let center = if p.x.abs() > TOWER.0 * 0.5 { TOWER.0 * p.x.signum() } else { 0.0 };
        let z = if center == 0.0 { p.z + NAVE.0 * 0.7 } else { p.z };
        let gore = (0.5 + 0.5 * (z.atan2(p.x - center) * 8.0).cos()).powf(0.5);
        (RED.lerp(LIME, smoothstep(0.45, 0.55, gore)), 0.45)
    } else if is(WHITE) {
        (LIME * (0.85 + 0.2 * grime), 0.8)
    } else {
        let tex = detail::rock(f, u, 0.5, footprint * style.scale, 0.6);
        return pbr::shade_light(f, u, Vec3::new(0.32, 0.3, 0.28) * (0.6 + 0.6 * tex.color.luminance()), 0.9, 0.0, tex.normal, tex.ao, light);
    };
    // La sombra se busca un poco afuera de la pared, y mas con la luz
    // rasante: en las caras grandes y lisas el mapa de sombras hacia acne.
    let grazing = 1.0 - f.normal.dot(f.light_dir(u)).abs();
    let lifted = Fragment {
        pos: f.pos + f.normal * ((0.08 + 0.003 * f.pos.length()) * (1.0 + 3.0 * grazing)),
        ..*f
    };
    pbr::shade_light(&lifted, u, albedo, rough, 0.0, f.normal, 1.0, light)
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
            (XECUL.normalize() + Vec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * (0.002 * i as f32)).normalize()
        });
        for dir in directions().chain(plaza) {
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
            assert_eq!(s.attrs, b.sample(dir).attrs);
        }
    }

    /// La iglesia esta de pie en el llano de zacate, sin bosque, con la
    /// fachada hacia quien llega.
    #[test]
    fn the_church_stands_on_the_grassy_plaza_facing_the_visitor() {
        let field = Sierra::new(11);
        let (_, normal, forward, place) = site((DVec3::Y, DVec3::Z)).unwrap();
        assert!(normal.dot(forward).abs() < 1e-9);
        let frame = (normal, forward, forward.cross(normal));
        let mut instances = Vec::new();
        place(&mut instances, 200.0, &field, 3, frame);
        let churches: Vec<_> = instances.iter().filter(|p| p.kind == PropKind::Iglesia).collect();
        assert_eq!(churches.len(), 1);
        let church = churches[0];
        let dir = church.position.normalize();
        let s = field.sample(dir.as_vec3());
        assert!(s.attrs.x > 0.9 && s.attrs.z < 0.1, "{:?}", s.attrs);
        assert!(church.rotation.rotate(DVec3::Y).dot(dir) > 0.999);
        // El frente del modelo (+z) mira hacia atras del lugar.
        assert!(church.rotation.rotate(DVec3::Z).dot(-forward) > 0.99);
    }

    /// Los puestos del mercado quedan sobre la plaza empedrada y las casas
    /// alrededor: ninguna en la plaza ni en la iglesia.
    #[test]
    fn the_market_is_on_the_square_and_the_houses_stay_around_it() {
        let field = Sierra::new(11);
        let (_, normal, forward, place) = site((DVec3::Y, DVec3::Z)).unwrap();
        let frame = (normal, forward, forward.cross(normal));
        let mut instances = Vec::new();
        place(&mut instances, 200.0, &field, 3, frame);
        let noise = Perlin::new(1);
        let (mut stalls, mut houses) = (0, 0);
        for i in &instances {
            let p = (i.position.normalize() * 200.0).as_vec3();
            let (x, z) = (p.dot(frame.2.as_vec3()), p.dot(forward.as_vec3()));
            match i.kind {
                PropKind::Puesto => {
                    stalls += 1;
                    let ((_, paved), _) = town_ground(p, 0.05, &noise);
                    assert!(paved > 0.99, "puesto fuera de la plaza en {x} {z}");
                }
                PropKind::Casa => {
                    houses += 1;
                    let in_square = x.abs() < SQUARE.0 && z > SQUARE.1 && z < SQUARE.2;
                    let in_church = x.abs() < 12.0 && z > 12.0 && z < 36.0;
                    assert!(!in_square && !in_church, "casa en {x} {z}");
                }
                _ => {}
            }
        }
        assert!(stalls > 12 && houses > 30, "{stalls} puestos, {houses} casas");
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
