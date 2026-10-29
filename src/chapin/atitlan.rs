//! Atitlan (la version chapina de Vigia): un mundo lago, azul hondo, con tres
//! volcanes que salen del agua (Atitlan, Toliman con sus dos cimas y San
//! Pedro), cerros verdes con pueblos en la orilla y, en Panajachel, un muelle
//! con lanchas. Por las tardes sopla el Xocomil y pica el lago.
//!
//! # El relieve
//! - **El lago**: una elipse frente a Panajachel. La distancia a su orilla
//!   (`inland`, en unidades del mundo, negativa adentro) decide todo: bajo el
//!   agua el fondo baja como `1 - e^(q/30)`; en tierra hay un delta plano y
//!   luego las paredes de la caldera, con crestas de ruido. Lejos del lago,
//!   continentes de ruido fractal con la misma forma de orilla.
//! - **Los volcanes**: cada uno es un perfil concavo `H (1 - s)^1.7` (`s` es
//!   la distancia a la cima entre el radio de la falda): empinado arriba y
//!   tendido abajo, como los estratovolcanes. Los **barrancos** bajan por la
//!   falda: un ruido muestreado en un circulo alrededor de la cima (asi es
//!   periodico en el angulo) talla un valle donde cruza por cero, hondo a
//!   media falda y nada en la cima ni en el pie. Arriba, un crater. Los
//!   volcanes se juntan entre si y con el suelo con un maximo suave, como
//!   el collado entre Atitlan y Toliman.
//! - **El muelle** de Panajachel es parte del relieve: una franja plana sobre
//!   el agua. Lo que se ve es lo que se choca.
//! - Atributos: `x` hondura del agua (negativa en tierra), `y` pueblo, `z`
//!   cuanto se esta cerca de la cima de un volcan.
//!
//! # El agua
//! - **El Xocomil**: un ciclo de `CYCLE` segundos, funcion pura del tiempo:
//!   calma de manana, sube el viento, sopla y se calma. Rachas de ruido lo
//!   hacen desigual sobre el lago.
//! - **Olas**: el oleaje es un ruido estirado de este a oeste (las crestas
//!   quedan de traves al viento, que sopla del sur) que corre con el tiempo.
//!   El mismo oleaje mueve los vertices del agua (`displace`), asi las
//!   lanchas suben y bajan con el; su pendiente, por diferencias finitas,
//!   inclina la normal. Encima, rizos finos que se apagan cuando un pixel
//!   cubre varios (si no, parpadean).
//! - **Espejo**: Fresnel de Schlick entre el azul del agua honda (o el fondo
//!   en la orilla) y el reflejo. El rayo reflejado se intersecta con un cono
//!   por volcan (una ecuacion cuadratica): si pega, se ve el volcan al reves
//!   en el agua, iluminado con la normal del cono; si no, el cielo. En calma
//!   el lago es un espejo; con el Xocomil el reflejo se rompe, salen crestas
//!   blancas y el destello del sol se vuelve un camino ancho.
//!
//! # La tierra
//! Arena volcanica negra en la orilla; milpas en parcelas (celdas de
//! Worley, cada una con su cultivo y un lindero oscuro), bosque en las
//! laderas empinadas y en la franja de neblina de los volcanes, casi hasta
//! arriba; solo la punta es pedregal de roca cafe y ceniza.
//! Los pueblos son celdas de Worley mas chicas: cada casa con su techo de
//! lamina o de teja, inclinado desde el centro para que el sol lo marque.
//!
//! # Las lanchas
//! Cascos de madera facetados (secciones unidas por cuadrilateros), con
//! techo sobre postes y motor fuera de borda. El shader las pinta con
//! franjas de colores que siguen la borda; cada una con su combinacion.

use std::f32::consts::PI;
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::math::{DVec3, Perlin, Vec3, hash_u32, smoothstep};
use crate::scene::Mesh;
use crate::scene::builder::MeshBuilder;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter, at_site, clear_site, upright_rotation};
use crate::shaders::{AtmosphereParams, Fragment, PropStyle, Uniforms, detail, limb_haze, pbr};
use crate::terrain::{HeightField, TerrainSample};

pub fn spec() -> Spec {
    Spec {
        label: "Atitlan",
        material: Chapin::Atitlan,
        // Aire limpio de altiplano: azul y no muy grueso.
        atmosphere: Some(AtmosphereParams {
            thickness: 30.0,
            scale_height: 8.0,
            rayleigh: Vec3::new(0.0025, 0.0065, 0.016),
            mie: 0.0018,
            mie_g: 0.76,
            intensity: 2.6,
        }),
        rings: None,
        motes: Some((Motes::Spray, 200.0)),
    }
}

// El lugar y los volcanes ---------------------------------------------------

/// Radio para el que esta pensado el relieve: pasa angulos a unidades del
/// mundo.
const RADIUS: f32 = 540.0;
/// Panajachel, en el espacio del planeta, y hacia donde se mira: a los
/// volcanes, casi al norte del planeta. A los 40 s es de manana: el sol entra
/// por la derecha, un poco por detras, a unos 27 grados; desde la orbita
/// (vista 45) los volcanes quedan arriba a la izquierda, cerca del
/// terminador, con sombras largas.
const PANAJACHEL: Vec3 = Vec3::new(0.972789, 0.104528, -0.206773);
const TOWARD: Vec3 = Vec3::new(-0.044949, 0.960634, 0.274155);

/// Ejes del lugar: vertical, al frente (hacia los volcanes) y a la derecha.
fn axes() -> (Vec3, Vec3, Vec3) {
    let up = PANAJACHEL.normalize();
    let forward = (TOWARD - up * TOWARD.dot(up)).normalize();
    (up, forward, forward.cross(up))
}

/// La direccion del punto que esta a `x` a la derecha y `z` al frente de
/// Panajachel (como `at_site`).
fn site_dir(x: f32, z: f32) -> Vec3 {
    let (up, forward, right) = axes();
    (up + right * (x / RADIUS) + forward * (z / RADIUS)).normalize()
}

/// Un volcan: su cima a `x` a la derecha y `z` al frente de Panajachel, su
/// altura sobre el lago, el radio de su falda y la hondura del crater.
struct Volcano {
    x: f32,
    z: f32,
    height: f32,
    base: f32,
    crater: f32,
}

const VOLCANOES: [Volcano; 4] = [
    // San Pedro, solo, a la derecha.
    Volcano { x: 140.0, z: 170.0, height: 74.0, base: 118.0, crater: 0.0 },
    // Toliman y su segunda cima.
    Volcano { x: -22.0, z: 192.0, height: 82.0, base: 135.0, crater: 5.0 },
    Volcano { x: -4.0, z: 172.0, height: 74.0, base: 105.0, crater: 0.0 },
    // Atitlan, el mas alto, detras de Toliman.
    Volcano { x: -88.0, z: 272.0, height: 125.0, base: 225.0, crater: 4.0 },
];

/// Curvatura del perfil de los volcanes: `H (1 - s)^CONCAVE`.
const CONCAVE: f32 = 1.7;
/// Radio del barranco en el circulo de ruido: cuantos barrancos hay.
const RAVINES: f32 = 3.2;

/// El lago: una elipse con centro a `x`, `z` del lugar, semiejes y la
/// direccion del eje mayor (en `x`, `z`). Panajachel queda sobre el agua,
/// junto a la orilla de la izquierda; Toliman y San Pedro salen del lago y
/// Atitlan queda detras, en tierra.
const LAKE: (f32, f32, f32, f32) = (125.0, 40.0, 220.0, 140.0);
const LAKE_AXIS: (f32, f32) = (0.7071, -0.7071);
/// Hondura maxima del agua.
const DEPTH: f32 = 16.0;
/// Altura de las paredes de la caldera y de los cerros.
const WALL: f32 = 38.0;

/// El muelle sale de la orilla de la izquierda: su arranque (en tierra), su
/// direccion (unitaria, en `x`, `z`), su largo y su medio ancho; en la punta
/// una cabeza mas ancha. Su piso queda a `DOCK_TOP` sobre el agua.
const DOCK_START: (f32, f32) = (-30.0, 10.0);
const DOCK_DIR: (f32, f32) = (0.9363, 0.3511);
const DOCK_LENGTH: f32 = 36.0;
const DOCK_HALF: f32 = 2.0;
const DOCK_HEAD: (f32, f32) = (6.0, 5.0);
const DOCK_TOP: f32 = 0.9;

/// El punto `(x, z)` del lugar en los ejes del muelle: a lo largo (desde el
/// arranque) y de lado.
#[inline]
fn dock_coords(x: f32, z: f32) -> (f32, f32) {
    let (px, pz) = (x - DOCK_START.0, z - DOCK_START.1);
    let (dx, dz) = DOCK_DIR;
    (px * dx + pz * dz, pz * dx - px * dz)
}

/// Cuanto cubre el muelle el punto `(x, z)` del lugar (1 en el piso).
fn dock(x: f32, z: f32) -> f32 {
    let (along, side) = dock_coords(x, z);
    let ends = smoothstep(-0.4, 0.0, along) * smoothstep(DOCK_LENGTH + 0.4, DOCK_LENGTH, along);
    let stem = smoothstep(DOCK_HALF + 0.4, DOCK_HALF, side.abs());
    let head = smoothstep(DOCK_HEAD.0 + 0.4, DOCK_HEAD.0, side.abs())
        * smoothstep(DOCK_LENGTH - DOCK_HEAD.1 - 0.4, DOCK_LENGTH - DOCK_HEAD.1, along);
    ends * stem.max(head)
}

// El relieve ----------------------------------------------------------------

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(AtitlanField::new(seed))
}

/// Un volcan ya ubicado: la direccion de su cima y dos ejes de su plano
/// tangente (para el angulo de los barrancos).
struct Cone {
    center: Vec3,
    ta: Vec3,
    tb: Vec3,
    height: f32,
    base: f32,
    crater: f32,
}

/// El relieve de Atitlan.
pub struct AtitlanField {
    noise: Perlin,
    up: Vec3,
    forward: Vec3,
    right: Vec3,
    lake: Vec3,
    /// El eje mayor del lago.
    major: Vec3,
    cones: Vec<Cone>,
}

/// Maximo suave: como `max`, pero redondea la esquina en `k`.
#[inline]
fn smax(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.max(b) + h * h * k * 0.25
}

/// La altura segun cuanto se entra a tierra desde la orilla (`q`, negativo
/// bajo el agua) y que tanto suben los cerros (`hills`, 0 a 1). `delta`
/// aleja las paredes de la orilla (el llano de Panajachel).
fn shore(q: f32, hills: f32, delta: f32) -> f32 {
    if q < 0.0 {
        return -DEPTH * (1.0 - (q / 30.0).exp());
    }
    let start = 12.0 + 70.0 * delta;
    // Un borde de arena que sube rapido (asi la orilla queda nitida) y luego
    // el llano.
    0.35 * q.min(3.0) + 0.07 * (q - 3.0).clamp(0.0, 60.0) + WALL * smoothstep(start, start + 80.0, q) * hills
}

/// Lo mas bajo y lo mas alto que puede quedar el relieve.
const LOWEST: f32 = -DEPTH;
const HIGHEST: f32 = 135.0;
/// La hondura que se guarda en tierra (la roca asoma de mas).
const DRY: f32 = 4.0;

impl AtitlanField {
    pub fn new(seed: u32) -> Self {
        let (up, forward, right) = axes();
        let cones = VOLCANOES
            .iter()
            .map(|v| {
                let center = site_dir(v.x, v.z);
                let ta = (forward - center * forward.dot(center)).normalize();
                Cone {
                    center,
                    ta,
                    tb: center.cross(ta),
                    height: v.height,
                    base: v.base,
                    crater: v.crater,
                }
            })
            .collect();
        Self {
            noise: Perlin::new(seed),
            up,
            forward,
            right,
            lake: site_dir(LAKE.0, LAKE.1),
            major: right * LAKE_AXIS.0 + forward * LAKE_AXIS.1,
            cones,
        }
    }

    /// Cuanto se entra a tierra desde la orilla del lago (negativo adentro).
    fn inland(&self, dir: Vec3) -> f32 {
        let v = (dir - self.lake) * RADIUS;
        let along = v.dot(self.major);
        let (a, b) = (LAKE.2, LAKE.3);
        // Lo que no va por el eje mayor cuenta para el menor (tambien lo que
        // sale del plano): del otro lado del planeta no hay otro lago.
        let e2 = along * along / (a * a) + (v.length_squared() - along * along) / (b * b);
        (e2.sqrt() - 1.0) * b
    }

    /// El suelo sin los volcanes: el lago y su caldera cerca, continentes
    /// lejos.
    fn ground(&self, dir: Vec3) -> f32 {
        let n = &self.noise;
        let near_site = (dir - self.up).length() * RADIUS;
        // La orilla del lago se tuerce con ruido, menos en Panajachel.
        let wobble = 30.0 * n.fbm(dir * 7.0 + Vec3::new(2.2, -5.1, 1.7), 3) * smoothstep(30.0, 120.0, near_site);
        let lake = self.inland(dir) + wobble;
        // Continentes: donde un ruido grande pasa de un umbral.
        let warp = Vec3::new(
            n.noise(dir * 2.1 + Vec3::new(5.3, 0.0, 0.0)),
            n.noise(dir * 2.1 + Vec3::new(0.0, -8.7, 0.0)),
            n.noise(dir * 2.1 + Vec3::new(0.0, 0.0, 3.9)),
        ) * 0.18;
        let land = (n.fbm((dir + warp) * 1.6, 5) - 0.24) * 700.0;
        // Alrededor del lago siempre hay tierra: la caldera.
        let land = land.max(50.0 * smoothstep(260.0, 120.0, lake + 60.0 * n.noise(dir * 5.0)));
        let w = smoothstep(120.0, 70.0, lake);
        let q = (land + (lake - land) * w).clamp(-250.0, 300.0);
        let hills = 0.35 + 0.9 * n.ridged(dir * 7.0 + Vec3::new(-3.1, 4.4, 0.6), 4);
        let delta = smoothstep(110.0, 50.0, near_site);
        shore(q, hills, delta)
    }

    /// Un volcan en `dir`: su altura y que tan cerca de la cima (1 en la
    /// cima, 0 en el pie), o nada si `dir` queda fuera de su falda.
    fn cone(&self, c: &Cone, dir: Vec3) -> Option<(f32, f32)> {
        let off = dir - c.center;
        let s = off.length() * RADIUS / c.base;
        if s >= 1.0 {
            return None;
        }
        // La falda baja hasta el fondo del lago: el volcan sale del agua.
        let body = c.height * (1.0 - s).powf(CONCAVE) - DEPTH * smoothstep(0.3, 1.0, s);
        // Barrancos: ruido en un circulo alrededor de la cima (periodico en
        // el angulo); el valle va donde cruza por cero y serpentea al bajar.
        let (a, b) = (off.dot(c.ta), off.dot(c.tb));
        let r = (a * a + b * b).sqrt().max(1e-6);
        let ring = Vec3::new(a / r * RAVINES, b / r * RAVINES, s * 3.0 + c.height * 0.1);
        let ravine = self.noise.noise(ring).abs();
        let carve = (1.0 - smoothstep(0.0, 0.18, ravine)) * c.height * 0.08 * (4.0 * s * (1.0 - s));
        let crater = c.crater * smoothstep(0.07, 0.035, s);
        // Coladas y lomas en la falda.
        let lumps = 3.0 * s * self.noise.fbm(dir * 30.0, 2);
        Some((body - carve - crater + lumps, 1.0 - s))
    }

    /// Los volcanes juntos: altura y cercania a la cima del que manda.
    fn volcanoes(&self, dir: Vec3) -> Option<(f32, f32)> {
        let mut top: Option<(f32, f32)> = None;
        for c in &self.cones {
            if let Some((h, s)) = self.cone(c, dir) {
                top = Some(match top {
                    None => (h, s),
                    Some((t, ts)) => (smax(t, h, 12.0), if h > t { s } else { ts }),
                });
            }
        }
        top
    }

    /// Pueblos: en manchas en lo bajo junto al agua, y `town` (Panajachel)
    /// donde no lo tape el agua.
    fn village(&self, dir: Vec3, height: f32, town: f32) -> f32 {
        let low = smoothstep(0.4, 1.2, height) * smoothstep(12.0, 5.0, height);
        let patches = if low > 0.0 {
            smoothstep(0.2, 0.45, self.noise.noise(dir * 16.0 + Vec3::new(7.7, 1.9, -4.2)))
        } else {
            0.0
        };
        (patches * low).max(town * smoothstep(0.3, 0.8, height))
    }
}

impl HeightField for AtitlanField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let ground = self.ground(dir);
        let (mut height, summit) = match self.volcanoes(dir) {
            Some((h, s)) => (smax(ground, h, 8.0), s * smoothstep(-6.0, 6.0, h - ground)),
            None => (ground, 0.0),
        };
        // Cerca del lugar: el muelle y el pueblo de Panajachel.
        let mut town = 0.0;
        if dir.dot(self.up) > 0.9 {
            let (x, z) = (dir.dot(self.right) * RADIUS, dir.dot(self.forward) * RADIUS);
            // El muelle sube del fondo hasta su piso en un borde angosto.
            height += (DOCK_TOP - height) * dock(x, z);
            // El pueblo, en la orilla de la izquierda.
            town = smoothstep(70.0, 40.0, (x + 70.0).hypot(z - 5.0));
        }
        let village = self.village(dir, height, town);
        let height = height.clamp(LOWEST, HIGHEST);
        TerrainSample {
            height: height.max(0.0),
            attrs: Vec3::new((-height).clamp(-DRY, DEPTH), village * (1.0 - summit), summit),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        // El agua es plana: lo mas bajo que se ve es su nivel.
        (0.0, HIGHEST)
    }
}

// El shader del planeta -------------------------------------------------------

// Colores (lineales).
const WATER_DEEP: Vec3 = Vec3::new(0.008, 0.045, 0.16);
const WATER_SHALLOW: Vec3 = Vec3::new(0.02, 0.06, 0.07);
const SAND: Vec3 = Vec3::new(0.07, 0.062, 0.055);
const FOREST: Vec3 = Vec3::new(0.018, 0.05, 0.02);
const FOREST_LIGHT: Vec3 = Vec3::new(0.05, 0.1, 0.035);
const ASH: Vec3 = Vec3::new(0.085, 0.075, 0.066);
const BASALT: Vec3 = Vec3::new(0.09, 0.085, 0.08);
/// Pedregal de las cimas: roca cafe y ceniza.
const SCREE: Vec3 = Vec3::new(0.068, 0.054, 0.044);
const FOAM: Vec3 = Vec3::new(0.8, 0.85, 0.88);
/// Cultivos de las parcelas: milpa verde, milpa seca, cafe, tierra arada.
const CROPS: [Vec3; 6] = [
    Vec3::new(0.045, 0.095, 0.03),
    Vec3::new(0.085, 0.09, 0.045),
    Vec3::new(0.022, 0.055, 0.024),
    Vec3::new(0.065, 0.055, 0.042),
    Vec3::new(0.06, 0.105, 0.034),
    Vec3::new(0.035, 0.075, 0.032),
];
/// Techos: lamina oxidada, lamina gris, teja, lamina pintada.
const ROOFS: [Vec3; 6] = [
    Vec3::new(0.2, 0.1, 0.06),
    Vec3::new(0.27, 0.27, 0.28),
    Vec3::new(0.28, 0.09, 0.05),
    Vec3::new(0.08, 0.12, 0.2),
    Vec3::new(0.36, 0.34, 0.31),
    Vec3::new(0.17, 0.16, 0.15),
];
/// El azul del borde del planeta visto desde lejos.
const LIMB: Vec3 = Vec3::new(0.2, 0.45, 0.95);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    // Desde otro cuerpo (las pozas de Semuc la reflejan) llega la esfera
    // lisa, sin atributos: ahi todo es lago hondo.
    let attrs = if f.color == Vec3::ZERO { Vec3::new(DEPTH, 0.0, 0.0) } else { f.color };
    let (depth, village, summit) = (attrs.x, attrs.y, attrs.z);
    let footprint = f.pos.length() * u.pixel_angle;
    // La orilla se mezcla en una franja del ancho de un pixel: sin escalones.
    let band = (footprint * 1.5).max(0.02);
    let wet = smoothstep(-band, band, depth);
    let color = if wet >= 1.0 {
        water(f, u, depth, footprint)
    } else {
        let dry = land(f, u, village, summit, footprint);
        if wet <= 0.0 {
            dry
        } else {
            dry.lerp(water(f, u, depth.max(0.0), footprint), wet)
        }
    };
    color + limb_haze(f, LIMB, 0.35, f.normal.dot(f.light_dir(u)))
}

/// Del espacio del planeta al mundo.
#[inline]
fn to_world(f: &Fragment, v: Vec3) -> Vec3 {
    f.object_to_world.mul_vec(v)
}

/// Del mundo al espacio del planeta (la transpuesta de la rotacion).
#[inline]
fn to_local(f: &Fragment, v: Vec3) -> Vec3 {
    let m = &f.object_to_world.m;
    Vec3::new(
        m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
        m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
        m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
    )
}

/// Luz del sol que llega a un punto con normal `n` (con su sombra).
fn sunlight(f: &Fragment, u: &Uniforms, n: Vec3) -> Vec3 {
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, n));
    u.sun_light * (n.dot(f.light_dir(u)).max(0.0) * shadow * 3.6 / PI)
}

// El Xocomil y las olas -------------------------------------------------------

/// Duracion del ciclo del Xocomil, en segundos.
const CYCLE: f32 = 150.0;

/// La fuerza del Xocomil en el instante `t` (0 calma, 1 en su punto): la
/// manana en calma, el viento sube, sopla un rato y se calma.
pub fn xocomil(t: f32) -> f32 {
    let phase = (t / CYCLE).fract();
    smoothstep(0.35, 0.5, phase) * smoothstep(0.92, 0.75, phase)
}

/// El oleaje: cuanto sube el agua en `p` (espacio del planeta) en el
/// instante `t`. El Xocomil sopla de norte a sur del planeta (el eje es +Y):
/// de los volcanes hacia Panajachel. Un ruido estirado de este a oeste deja
/// las crestas de traves al viento, y corre con el.
fn swell(p: Vec3, t: f32, noise: &Perlin) -> f32 {
    let q = Vec3::new(p.x, p.y * 2.4, p.z) * 0.12 + Vec3::new(0.0, t * 0.5, 0.0);
    (0.06 + 0.4 * xocomil(t)) * noise.noise(q)
}

/// Agua del lago: el oleaje, el azul hondo, el reflejo y el sol.
fn water(f: &Fragment, u: &Uniforms, depth: f32, footprint: f32) -> Vec3 {
    let t = u.time;
    let n = u.noise;
    let p = f.local;
    let up = p.normalize();
    let east = Vec3::Y.cross(up);
    let east = if east.length_squared() > 1e-6 { east.normalize() } else { Vec3::X };
    let north = up.cross(east);
    // Rachas: el Xocomil no sopla parejo.
    let gust = n.noise(p * 0.012 + Vec3::new(t * 0.03, 0.0, -t * 0.02)) * 0.5 + 0.5;
    let wind = xocomil(t) * (0.55 + 0.6 * gust) + 0.05 * smoothstep(0.55, 0.8, gust);
    // La pendiente del oleaje, por diferencias finitas.
    let e = 0.4;
    let w0 = swell(p, t, n);
    let we = swell(p + east * e, t, n);
    let wn = swell(p + north * e, t, n);
    let mut tilt = -(east * (we - w0) + north * (wn - w0)) / e;
    // Rizos finos que corren con el viento; se apagan donde un pixel cubre
    // varios.
    let fine = smoothstep(0.6, 0.12, footprint);
    if fine > 0.0 {
        let q = Vec3::new(p.x, p.y * 1.8, p.z) * 0.9 + Vec3::new(0.0, t * 1.3, 0.0);
        let ca = n.noise(q);
        let cb = n.noise(q * 1.9 + Vec3::new(4.2, -t * 0.7, 1.3));
        tilt += (east * ca + north * cb) * ((0.015 + 0.2 * wind) * fine);
    }
    let normal = to_world(f, (up + tilt).normalize());
    let v = f.view_dir();
    let ndv = normal.dot(v).max(1e-3);
    let fresnel = 0.02 + 0.98 * (1.0 - ndv).powi(5);
    let reflected = reflection(f, u, normal * (2.0 * ndv) - v);
    // Lo que se ve adentro: azul hondo, o el fondo de arena junto a la orilla.
    let sun = sunlight(f, u, f.normal);
    let ambient = Vec3::new(0.03, 0.05, 0.08) + u.sun_light * 0.01;
    // Bajo el agua el rojo de la estrella casi no cuenta: el ojo se
    // acostumbra y el azul del lago sale de la luminancia de la luz.
    let light = sun.lerp(Vec3::splat(sun.luminance()), 0.6) * 0.6 + ambient;
    let shallow = smoothstep(1.6, 0.1, depth);
    let inside = WATER_DEEP.lerp(WATER_SHALLOW, shallow) * light;
    let mut color = inside * (1.0 - fresnel) + reflected * fresnel;
    // Crestas blancas con el Xocomil.
    let crest = smoothstep(0.25, 0.5, w0 / (0.06 + 0.4 * xocomil(t)) + 0.4 * n.noise(p * 0.7 - up * t));
    let foam = crest * smoothstep(0.35, 0.9, wind) * 0.8;
    color = color.lerp(FOAM * (sun + ambient), foam);
    let rough = 0.03 + 0.2 * wind + footprint * 0.4;
    color + glint(f, u, normal, rough) * (1.0 - foam)
}

/// Destello del sol en el agua: Blinn-Phong normalizado, mas ancho con
/// viento o cuando el pixel cubre muchas olas.
fn glint(f: &Fragment, u: &Uniforms, n: Vec3, rough: f32) -> Vec3 {
    let l = f.light_dir(u);
    let h = (l + f.view_dir()).normalize();
    let shine = 2.0 / (rough * rough).max(1e-3);
    let nl = n.dot(l).max(0.0);
    let spec = (shine + 8.0) / (8.0 * PI) * n.dot(h).max(0.0).powf(shine);
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, f.normal));
    u.sun_light * (spec * nl * shadow * 0.05 * u.gains.vigia)
}

/// El cielo en la direccion `dir` (del mundo).
fn sky(u: &Uniforms, dir: Vec3) -> Vec3 {
    let stars = u.sky.sample(dir);
    match u.sky_view {
        Some(view) => view.sample(dir).apply(stars),
        None => stars,
    }
}

/// Lo que refleja el agua en la direccion `dir` (del mundo): un volcan si el
/// rayo le pega a su cono, si no el cielo.
fn reflection(f: &Fragment, u: &Uniforms, dir: Vec3) -> Vec3 {
    let sky = sky(u, dir);
    let origin = f.local;
    let d = to_local(f, dir);
    let mut nearest: Option<(f32, Vec3, f32, &Volcano)> = None;
    for v in &VOLCANOES {
        let axis = site_dir(v.x, v.z);
        let apex = axis * (RADIUS + v.height);
        if let Some((t, along)) = cone_hit(origin - apex, d, axis, v) {
            if nearest.is_none_or(|(best, ..)| t < best) {
                nearest = Some((t, axis, along, v));
            }
        }
    }
    let Some((t, axis, along, v)) = nearest else { return sky };
    // El volcan de lejos: verde abajo, ceniza arriba, con la luz del sol
    // sobre la normal del cono.
    let hit = origin + d * t;
    let radial = hit - axis * hit.dot(axis);
    let radial = radial * (1.0 / radial.length().max(1e-6));
    let (sin_a, cos_a) = cone_angle(v);
    let normal = to_world(f, radial * cos_a + axis * sin_a);
    let high = 1.0 - along / v.height;
    let albedo = FOREST_LIGHT.lerp(SCREE, smoothstep(0.82, 0.92, high));
    let light = u.sun_light * (normal.dot(f.light_dir(u)).max(0.0) * 3.6 / PI) + Vec3::new(0.03, 0.045, 0.07);
    // Mientras mas lejos, mas se pierde en la bruma.
    (albedo * light).lerp(sky, 1.0 - (-t / 700.0).exp())
}

/// Seno y coseno del medio angulo del cono que aproxima a un volcan: pasa
/// por la cima y por el punto de su perfil a media falda.
fn cone_angle(v: &Volcano) -> (f32, f32) {
    let tan = 0.5 * v.base / (v.height * (1.0 - 0.5f32.powf(CONCAVE)));
    let cos = 1.0 / (1.0 + tan * tan).sqrt();
    (tan * cos, cos)
}

/// Interseccion del rayo `o + t d` (con `o` relativo a la cima) con el cono
/// del volcan `v`, que abre hacia abajo alrededor de `axis`. Devuelve `t` y
/// cuanto baja el punto desde la cima, si queda sobre el agua.
fn cone_hit(o: Vec3, d: Vec3, axis: Vec3, v: &Volcano) -> Option<(f32, f32)> {
    let (_, cos) = cone_angle(v);
    let c2 = cos * cos;
    // Un punto `w` esta en el cono si `(w . -axis)^2 = cos^2 |w|^2`.
    let (dv, ov) = (-d.dot(axis), -o.dot(axis));
    let a = dv * dv - c2;
    let b = ov * dv - c2 * o.dot(d);
    let c = ov * ov - c2 * o.length_squared();
    let disc = b * b - a * c;
    if disc < 0.0 || a.abs() < 1e-8 {
        return None;
    }
    let root = disc.sqrt();
    let mut best: Option<(f32, f32)> = None;
    for t in [(-b - root) / a, (-b + root) / a] {
        let along = ov + t * dv;
        // Adelante, en la mitad de abajo del cono y sobre el agua.
        if t > 0.0 && along > 0.0 && along < v.height && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, along));
        }
    }
    best
}

// La tierra -----------------------------------------------------------------

/// Arena, milpas, bosque, el pedregal de las cimas, pueblos y el muelle.
fn land(f: &Fragment, u: &Uniforms, village: f32, summit: f32, footprint: f32) -> Vec3 {
    let n = u.noise;
    let p = f.local;
    let height = f.uv.x * HIGHEST;
    let slope = f.uv.y;
    let rock = detail::rock(f, u, 0.25, footprint, 0.8);
    let grain = 0.6 + 0.8 * rock.color.luminance();

    // Cerca del lugar: el muelle.
    let (up, forward, right) = axes();
    let dir = p.normalize();
    let (mut deck, mut along, mut side) = (0.0, 0.0, 0.0);
    if dir.dot(up) > 0.99 {
        let (x, z) = (dir.dot(right) * RADIUS, dir.dot(forward) * RADIUS);
        deck = dock(x, z);
        (along, side) = dock_coords(x, z);
    }

    // Arena negra en la orilla.
    let beach = smoothstep(1.0, 0.35, height + 0.5 * n.noise(p * 0.4)) * (1.0 - deck);
    // Arriba de los volcanes: pedregal, solo en la punta (el bosque sube
    // casi hasta arriba); en la franja de neblina y en lo empinado: bosque;
    // lo demas, parcelas.
    let bare = smoothstep(0.83, 0.91, summit + 0.05 * n.noise(p * 0.06));
    let cliff = smoothstep(0.45, 0.65, slope);
    let cloud_forest = smoothstep(0.4, 0.55, summit) * (1.0 - bare);
    let woods = smoothstep(0.1, 0.35, n.fbm(p * 0.03, 3) + slope * 0.9).max(cloud_forest).max(village * 0.85);
    let (crop, hedge) = parcels(p, footprint);
    let mut albedo = crop.lerp(FOREST, hedge);
    let mut normal = f.normal.lerp(rock.normal, 0.3 + 0.7 * cliff.max(bare)).normalize();
    let mut cavity = rock.ao;
    if woods > 0.01 {
        let (leaf, leaf_normal, shade) = canopy(f, u, footprint);
        albedo = albedo.lerp(leaf, woods);
        normal = normal.lerp(leaf_normal, woods).normalize();
        cavity *= 1.0 + (shade - 1.0) * woods;
    }
    let scree = SCREE.lerp(ASH, smoothstep(-0.3, 0.5, n.fbm(p * 0.12, 2))) * grain;
    albedo = albedo.lerp(scree, bare).lerp(BASALT * grain, cliff * (1.0 - beach));
    albedo = albedo.lerp(SAND * grain, beach);

    // Los pueblos.
    if village > 0.02 {
        let (roof, roof_normal, cover, shadow) = houses(f, u, p, footprint);
        let town = smoothstep(0.02, 0.5, village);
        let cover = cover * town;
        albedo = albedo.lerp(roof, cover) * (1.0 + (shadow - 1.0) * town);
        normal = normal.lerp(roof_normal, cover).normalize();
    }
    if deck > 0.0 {
        // Tablones de traves, con rendijas oscuras.
        let gap = smoothstep(0.08, 0.02, ((along * 2.5).fract() - 0.5).abs() - 0.38) * smoothstep(0.3, 0.05, footprint);
        let wood = Vec3::new(0.2, 0.12, 0.07) * (0.8 + 0.3 * n.noise(Vec3::new(side * 0.4, along * 2.5, 0.0)));
        albedo = albedo.lerp(wood * (1.0 - 0.7 * gap), deck);
        normal = normal.lerp(f.normal, deck).normalize();
    }
    let rough = 0.9 - 0.3 * beach;
    pbr::shade(f, u, albedo, rough, 0.0, normal, cavity)
}

/// Copas de arboles: un Worley donde cada punto es una copa; la normal sale
/// del centro de la copa (una cupula) y cada una tiene su verde. Devuelve
/// el color, la normal y la sombra entre copas. De lejos, el promedio.
fn canopy(f: &Fragment, u: &Uniforms, footprint: f32) -> (Vec3, Vec3, f32) {
    const CROWN: f32 = 2.2;
    // Las copas aparecen cuando miden mas de unos pixeles.
    let near = smoothstep(CROWN * 0.12, CROWN * 0.05, footprint);
    let patch = u.noise.fbm(f.local * 0.05, 2) * 0.5 + 0.5;
    let average = FOREST.lerp(FOREST_LIGHT, 0.25 + 0.3 * patch);
    if near <= 0.0 {
        return (average, f.normal, 1.0);
    }
    let (offset, edge, id) = nearest(f.local / CROWN);
    let up = f.local.normalize();
    let side = to_world(f, offset - up * offset.dot(up));
    let normal = (f.normal + side * (1.2 * near)).normalize();
    let tone = unit(hash_u32(id));
    let leaf = FOREST.lerp(FOREST_LIGHT, (0.15 + 0.6 * tone + 0.3 * (patch - 0.5)).clamp(0.0, 1.0));
    let gap = 1.0 - near * smoothstep(0.3, 0.0, edge) * 0.7;
    (average.lerp(leaf, near), normal, gap)
}

/// El punto de Worley mas cercano a `p`: vector desde el punto, distancia
/// al borde con la celda vecina (`f2 - f1`) y el hash de su celda.
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
                        ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F),
                );
                let h2 = hash_u32(h);
                let h3 = hash_u32(h2);
                let point = cell + Vec3::new(unit(h), unit(h2), unit(h3)) * 0.8 + Vec3::splat(0.1);
                let delta = p - point;
                let d = delta.length_squared();
                if d < best.1 {
                    second = best.1;
                    best = (delta, d, h3);
                } else if d < second {
                    second = d;
                }
            }
        }
    }
    (best.0, second.sqrt() - best.1.sqrt(), best.2)
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Las parcelas: cada celda de Worley con su cultivo, y el lindero (1 en la
/// orilla de la parcela). De lejos, el promedio.
fn parcels(p: Vec3, footprint: f32) -> (Vec3, f32) {
    const SIZE: f32 = 6.0;
    let near = smoothstep(SIZE * 0.3, SIZE * 0.08, footprint);
    let average = Vec3::new(0.05, 0.08, 0.032);
    if near <= 0.0 {
        return (average, 0.0);
    }
    let (_, edge, id) = nearest(p / SIZE);
    let crop = CROPS[(id % CROPS.len() as u32) as usize];
    let hedge = smoothstep(0.06, 0.0, edge) * smoothstep(0.25, 0.05, footprint / SIZE);
    (average.lerp(crop, near), hedge * 0.7)
}

/// Lado de las celdas de la grilla del pueblo.
const LOT: f32 = 1.7;

/// Un solar del pueblo en `q` (en celdas): la grilla son cubos del espacio
/// del planeta y se usan los dos ejes que mas cortan la superficie (las
/// caras de los cubos la cortan en calles rectas). Devuelve cuanto cae
/// adentro de la casa (0 en la calle y en los patios), el hash de la celda,
/// los dos ejes y la posicion en ellos (0 a 1). Cada casa tiene su tamano.
fn lot(q: Vec3, up: Vec3, soft: f32) -> (f32, u32, (Vec3, f32), (Vec3, f32)) {
    let cell = q.floor();
    let fr = q - cell;
    let a = up.abs();
    let (ea, eb) = if a.x >= a.y && a.x >= a.z {
        ((Vec3::Y, fr.y), (Vec3::Z, fr.z))
    } else if a.y >= a.z {
        ((Vec3::Z, fr.z), (Vec3::X, fr.x))
    } else {
        ((Vec3::X, fr.x), (Vec3::Y, fr.y))
    };
    let id = hash_u32(
        (cell.x as i32 as u32).wrapping_mul(0x8DA6_B343)
            ^ (cell.y as i32 as u32).wrapping_mul(0xD816_3841)
            ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F),
    );
    if (id >> 20) % 5 < 2 {
        // Un patio.
        return (0.0, id, ea, eb);
    }
    let margin = |salt: u32| 0.08 + 0.2 * unit(hash_u32(id ^ salt));
    let edge = |v: f32, m: f32| smoothstep(m - soft, m + soft, v.min(1.0 - v));
    (edge(ea.1, margin(0x51)) * edge(eb.1, margin(0xA3)), id, ea, eb)
}

/// Las casas de un pueblo: cada solar con su techo de dos aguas (la
/// cumbrera sobre uno de los ejes, al azar) y la sombra que la casa echa a
/// la calle, del lado contrario al sol. Devuelve el techo, su normal, cuanto
/// cubre la casa y la sombra. De lejos, el promedio.
fn houses(f: &Fragment, u: &Uniforms, p: Vec3, footprint: f32) -> (Vec3, Vec3, f32, f32) {
    let near = smoothstep(LOT * 0.7, LOT * 0.2, footprint);
    let average = Vec3::new(0.16, 0.11, 0.08);
    if near <= 0.0 {
        return (average, f.normal, 0.45, 1.0);
    }
    let up = p.normalize();
    let q = p / LOT;
    let soft = (footprint / LOT).max(0.01);
    let (inside, id, ea, eb) = lot(q, up, soft);
    // Dos aguas: la normal se inclina a un lado u otro de la cumbrera.
    let (across, v) = if id & 1 == 0 { ea } else { eb };
    let across = to_world(f, across - up * across.dot(up));
    let pitch = smoothstep(-soft, soft, v - 0.5) * 2.0 - 1.0;
    let normal = (f.normal + across * (0.6 * pitch)).normalize();
    let roof = ROOFS[(id % ROOFS.len() as u32) as usize] * (0.75 + 0.5 * unit(hash_u32(id)));
    // La sombra: si hacia el sol (a lo que mide la sombra de una casa de
    // medio metro) hay una casa, la calle queda a la sombra.
    let sun = to_local(f, f.light_dir(u));
    let rise = sun.dot(up).max(0.15);
    let toward = (sun - up * sun.dot(up)) * (0.5 / (rise * LOT));
    let (blocker, ..) = lot(q + toward, up, soft);
    let shadow = 1.0 - 0.6 * blocker * (1.0 - inside) * near;
    (
        average.lerp(roof, near),
        f.normal.lerp(normal, near * inside).normalize(),
        0.45 + (inside - 0.45) * near,
        shadow,
    )
}

// El oleaje mueve los vertices ------------------------------------------------

pub const DEFORMS: bool = true;

/// El vertex shader: el agua (y lo que flota en ella, un poco hundido) sube
/// y baja con el oleaje. La tierra no se mueve: el agua esta justo en el
/// radio y la tierra arriba.
pub fn displace(local: Vec3, _normal: Vec3, time: f32, noise: &Perlin) -> Vec3 {
    let r = local.length();
    let h = r - RADIUS;
    let water = smoothstep(0.05, 0.0, h) * smoothstep(-0.9, -0.6, h);
    if water <= 0.0 {
        return Vec3::ZERO;
    }
    local * (swell(local, time, noise) * water / r)
}

// Los objetos -----------------------------------------------------------------

/// Lanchas en el agua poco honda junto a las orillas de todo el planeta.
pub fn rules() -> Vec<Scatter> {
    vec![Scatter {
        kind: PropKind::Lancha,
        cells: 48,
        scale: (0.85, 1.15),
        sink: 0.0,
        upright: true,
        accept: |density, s, _| {
            smoothstep(0.8, 1.6, s.attrs.x) * smoothstep(9.0, 4.0, s.attrs.x) * smoothstep(0.5, 0.75, density) * 0.5
        },
    }]
}

pub fn site(_default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    let (up, forward, _) = axes();
    Some(("Panajachel", up.as_dvec3(), forward.as_dvec3(), place))
}

/// Las lanchas amarradas al muelle: a que lado (en medios anchos del
/// muelle; 0 es la punta), a que distancia del arranque y si la proa mira al
/// lago.
const MOORED: [(f32, f32, bool); 7] = [
    (-1.0, 12.0, true),
    (-1.0, 20.0, false),
    (-1.0, 27.0, true),
    (1.0, 10.0, true),
    (1.0, 18.0, true),
    (1.0, 25.0, false),
    (0.0, DOCK_LENGTH + 1.3, true),
];

/// El giro (alrededor de la vertical `dir`) que deja la proa (+Z del modelo)
/// mirando hacia `facing`, como lo arma `upright_rotation`.
fn yaw_toward(dir: DVec3, facing: DVec3) -> f64 {
    let base = upright_rotation(dir, 0.0);
    facing.dot(base.rotate(DVec3::X)).atan2(facing.dot(base.rotate(DVec3::Z)))
}

/// Panajachel: lanchas amarradas al muelle y otras regadas por la bahia.
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    let (_, forward, right) = frame;
    clear_site(instances, radius, frame.0, 150.0);
    let lancha = |instances: &mut Vec<Instance>, x: f32, z: f32, facing: DVec3| {
        let dir = (frame.0 + (right * x as f64 + forward * z as f64) / radius).normalize();
        let yaw = yaw_toward(dir, facing);
        instances.push(at_site(radius, field, frame, PropKind::Lancha, x as f64, z as f64, 1.0, yaw, 0.0));
    };
    let (dx, dz) = DOCK_DIR;
    let out = right * dx as f64 + forward * dz as f64;
    let across = forward * dx as f64 - right * dz as f64;
    for &(side, along, lake) in &MOORED {
        let offset = if side == 0.0 { 0.0 } else { side * (DOCK_HALF + 1.3) };
        let x = DOCK_START.0 + dx * along - dz * offset;
        let z = DOCK_START.1 + dz * along + dx * offset;
        // En la punta, de traves; a los lados, a lo largo del muelle.
        let facing = match (side == 0.0, lake) {
            (true, _) => across,
            (false, true) => out,
            (false, false) => -out,
        };
        lancha(instances, x, z, facing);
    }
    // En la bahia, cada una para su lado, solo donde hay agua y lejos del
    // muelle.
    let mut k = 0u32;
    let mut placed = 0;
    while placed < 14 && k < 96 {
        let h = hash_u32(seed ^ k.wrapping_mul(0x9E37_79B9) ^ 0x1A4C);
        k += 1;
        let x = -60.0 + 160.0 * unit(h);
        let z = -10.0 + 130.0 * unit(hash_u32(h));
        let (along, side) = dock_coords(x, z);
        let near_dock = along > -10.0 && along < DOCK_LENGTH + 12.0 && side.abs() < 12.0;
        if near_dock || field.sample(site_dir(x, z)).attrs.x < 2.0 {
            continue;
        }
        let a = unit(hash_u32(h ^ 7)) as f64 * std::f64::consts::TAU;
        lancha(instances, x, z, right * a.cos() + forward * a.sin());
        placed += 1;
    }
}

// La lancha -------------------------------------------------------------------

/// Largo de la lancha: de la popa (`z0`) a la proa (`z1`), en el modelo.
const STERN: f32 = -3.3;
const BOW: f32 = 3.9;

// Colores de vertice: marcan el material.
const HULL: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const TRIM: Vec3 = Vec3::new(1.0, 1.0, 0.0);
const WOOD: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const ROOF: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const MOTOR: Vec3 = Vec3::new(0.0, 0.0, 0.0);

/// La forma del casco a una fraccion `u` del largo (0 popa, 1 proa): medio
/// ancho, hondura de la quilla y altura de la borda (sube en la proa).
fn hull_section(u: f32) -> (f32, f32, f32) {
    let width = 0.85 * (0.8 + 0.2 * smoothstep(0.0, 0.35, u)) * (1.0 - ((u - 0.45) / 0.55).max(0.0).powf(1.8));
    let keel = 0.38 * (1.0 - 0.6 * ((u - 0.6) / 0.4).max(0.0).powf(1.5));
    let sheer = 0.42 + 0.38 * ((u - 0.55) / 0.45).max(0.0).powi(2);
    (width, keel, sheer)
}

/// La altura de la borda en el punto `z` del modelo.
fn sheer_at(z: f32) -> f32 {
    hull_section(((z - STERN) / (BOW - STERN)).clamp(0.0, 1.0)).2
}

pub fn mesh(_kind: PropKind, detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let sections = [4, 6, 9][detail];
    // Cada seccion: borda, pantoque y quilla, de babor a estribor; y el
    // borde de la cubierta, un poco adentro y abajo de la borda.
    let ring = |k: usize| {
        let u = k as f32 / (sections - 1) as f32;
        let z = STERN + (BOW - STERN) * u;
        let (w, d, s) = hull_section(u);
        let hull = [
            Vec3::new(-w, s, z),
            Vec3::new(-w * 0.86, -d * 0.35, z),
            Vec3::new(0.0, -d, z),
            Vec3::new(w * 0.86, -d * 0.35, z),
            Vec3::new(w, s, z),
        ];
        let inner = (w - 0.08).max(0.0);
        (hull, [Vec3::new(-inner, s - 0.16, z), Vec3::new(inner, s - 0.16, z)])
    };
    for k in 0..sections - 1 {
        let ((a, da), (c, dc)) = (ring(k), ring(k + 1));
        b.color(HULL);
        for i in 0..4 {
            b.quad(a[i], a[i + 1], c[i + 1], c[i]);
        }
        // La borda por dentro y la cubierta.
        b.color(TRIM);
        b.quad(a[0], c[0], dc[0], da[0]);
        b.quad(da[1], dc[1], c[4], a[4]);
        b.color(WOOD);
        b.quad(da[0], dc[0], dc[1], da[1]);
    }
    // El espejo de popa.
    let (s, _) = ring(0);
    b.color(HULL);
    b.triangle(s[2], s[0], s[4]);
    b.triangle(s[2], s[1], s[0]);
    b.triangle(s[2], s[4], s[3]);
    // El techo sobre postes.
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let (roof_z, roof_half) = (-0.5, 1.9);
    b.color(ROOF);
    b.cuboid(Vec3::new(0.0, 1.78, roof_z), axes, Vec3::new(0.9, 0.04, roof_half));
    b.color(WOOD);
    let posts: &[f32] = [&[-1.0, 1.0][..], &[-1.0, 1.0][..], &[-1.0, 0.0, 1.0][..]][detail];
    for &side in &[-1.0f32, 1.0] {
        for &along in posts {
            let z = roof_z + along * (roof_half - 0.15);
            b.cuboid(Vec3::new(side * 0.72, 1.1, z), axes, Vec3::new(0.035, 0.66, 0.035));
        }
    }
    // El motor fuera de borda.
    b.color(MOTOR);
    b.cuboid(Vec3::new(0.0, 0.62, STERN - 0.2), axes, Vec3::new(0.17, 0.24, 0.2));
    if detail >= 1 {
        b.cuboid(Vec3::new(0.0, 0.1, STERN - 0.24), axes, Vec3::new(0.05, 0.3, 0.07));
    }
    if detail >= 2 {
        // Bancas de traves bajo el techo.
        b.color(WOOD);
        for z in [-1.7, -0.6, 0.5, 1.5] {
            let (w, _, s) = hull_section((z - STERN) / (BOW - STERN));
            b.cuboid(Vec3::new(0.0, s - 0.02, z), axes, Vec3::new(w - 0.1, 0.03, 0.12));
        }
    }
    b.build()
}

/// Cuanto mide un pixel sobre la superficie, en unidades del modelo.
#[inline]
fn model_footprint(f: &Fragment, u: &Uniforms, scale: f32) -> f32 {
    let ndv = f.normal.dot(f.view_dir()).abs().max(0.25);
    f.pos.length() * u.pixel_angle / (ndv * scale)
}

/// Pinturas de las lanchas: el casco y dos franjas.
const PAINTS: [(Vec3, Vec3, Vec3); 5] = [
    (Vec3::new(0.7, 0.7, 0.68), Vec3::new(0.03, 0.12, 0.45), Vec3::new(0.6, 0.04, 0.03)),
    (Vec3::new(0.05, 0.18, 0.5), Vec3::new(0.75, 0.55, 0.02), Vec3::new(0.7, 0.7, 0.68)),
    (Vec3::new(0.7, 0.7, 0.68), Vec3::new(0.02, 0.3, 0.08), Vec3::new(0.75, 0.55, 0.02)),
    (Vec3::new(0.55, 0.05, 0.04), Vec3::new(0.7, 0.7, 0.68), Vec3::new(0.03, 0.12, 0.45)),
    (Vec3::new(0.02, 0.32, 0.38), Vec3::new(0.7, 0.7, 0.68), Vec3::new(0.6, 0.04, 0.03)),
];

pub fn shade_prop(_kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    // El color de vertice llega interpolado: se compara por umbrales.
    let (red, green, blue) = (f.color.x > 0.5, f.color.y > 0.5, f.color.z > 0.5);
    let (albedo, rough) = if red && !green {
        let (body, first, second) = PAINTS[(style.seed * PAINTS.len() as f32) as usize % PAINTS.len()];
        // Las franjas siguen la borda: se miden desde ella hacia abajo.
        let below = sheer_at(p.z) - p.y;
        let soft = model_footprint(f, u, style.scale).max(0.01);
        let band = |a: f32, b: f32| smoothstep(a - soft, a + soft, below) * smoothstep(b + soft, b - soft, below);
        let mut paint = body.lerp(first, band(0.04, 0.13)).lerp(second, band(0.17, 0.24));
        // Bajo el agua, pintura oscura.
        paint = paint.lerp(Vec3::new(0.1, 0.03, 0.02), smoothstep(0.06 + soft, 0.06 - soft, p.y));
        (paint, 0.35)
    } else if red {
        (Vec3::new(0.5, 0.48, 0.44), 0.5)
    } else if green {
        let grain = u.noise.noise(Vec3::new(p.x * 9.0, p.y * 9.0, p.z * 1.5)) * 0.5 + 0.5;
        (Vec3::new(0.2, 0.12, 0.06) * (0.7 + 0.5 * grain), 0.75)
    } else if blue {
        let tarp = [Vec3::new(0.05, 0.14, 0.42), Vec3::new(0.5, 0.06, 0.04), Vec3::new(0.55, 0.53, 0.5)];
        (tarp[(style.seed * 7.0) as usize % tarp.len()], 0.6)
    } else {
        (Vec3::new(0.02, 0.02, 0.022), 0.4)
    };
    pbr::shade_light(f, u, albedo, rough, 0.0, f.normal, 1.0, sun)
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

    /// El relieve se queda dentro de su rango (el culling por horizonte
    /// depende de eso) y la misma semilla da el mismo planeta, tambien
    /// alrededor de Panajachel y de los volcanes.
    #[test]
    fn the_lake_world_stays_in_range_and_is_deterministic() {
        let (a, b) = (AtitlanField::new(7), AtitlanField::new(7));
        let (lo, hi) = a.height_range();
        let around = (0..2000).map(|i| {
            let t = i as f32 * 0.61;
            site_dir(150.0 * t.sin() * (t * 0.13).cos(), 120.0 + 170.0 * (t * 1.3).cos())
        });
        for dir in directions().chain(around) {
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
            assert_eq!(s.attrs, b.sample(dir).attrs);
        }
    }

    /// Panajachel esta en la orilla: tierra atras, el muelle sobre el agua y
    /// el lago al frente; los volcanes salen muy por encima de todo.
    #[test]
    fn panajachel_looks_across_the_lake_at_the_volcanoes() {
        let field = AtitlanField::new(3);
        let at = |x: f32, z: f32| field.sample(site_dir(x, z));
        assert!(at(-90.0, 0.0).attrs.x < 0.0, "a la izquierda, Panajachel");
        for z in [20.0, 50.0, 80.0] {
            assert!(at(20.0, z).attrs.x > 0.5, "al frente hay lago ({z})");
        }
        let (dx, dz) = DOCK_DIR;
        let mid = (DOCK_START.0 + dx * 20.0, DOCK_START.1 + dz * 20.0);
        assert!((at(mid.0, mid.1).height - DOCK_TOP).abs() < 0.05);
        assert!(at(mid.0 - dz * 5.0, mid.1 + dx * 5.0).attrs.x > 0.3, "junto al muelle hay agua");
        for v in &VOLCANOES {
            let top = at(v.x, v.z).height;
            assert!(top > v.height - 12.0, "cima {top}");
        }
        // Los ejes del lugar son ortonormales.
        let (up, forward, right) = axes();
        assert!(up.dot(forward).abs() < 1e-6 && up.dot(right).abs() < 1e-6 && (right.length() - 1.0).abs() < 1e-6);
    }

    /// Las lanchas flotan: todas quedan sobre agua y al nivel del lago, y el
    /// oleaje las mueve poco; la tierra no se mueve.
    #[test]
    fn the_boats_float_and_only_water_moves() {
        let field = AtitlanField::new(5);
        let (_, up, forward, place) = site((DVec3::Y, DVec3::Z)).unwrap();
        let frame = (up, forward, forward.cross(up));
        let mut instances = Vec::new();
        place(&mut instances, RADIUS as f64, &field, 9, frame);
        assert!(instances.len() >= MOORED.len() + 8);
        let noise = Perlin::new(1);
        for boat in &instances {
            let dir = boat.position.normalize().as_vec3();
            assert!(field.sample(dir).attrs.x > 0.0, "una lancha en tierra");
            assert!((boat.position.length() - RADIUS as f64).abs() < 1e-3);
            let lift = displace(boat.position.as_vec3(), dir, 100.0, &noise);
            assert!(lift.length() > 0.0 && lift.length() < 0.5);
        }
        let hill = dir_on_land(&field) * (RADIUS + 3.0);
        assert_eq!(displace(hill, hill.normalize(), 100.0, &noise), Vec3::ZERO);
    }

    fn dir_on_land(field: &AtitlanField) -> Vec3 {
        directions().find(|&d| field.sample(d).height > 3.0).unwrap()
    }

    /// El Xocomil: en calma a la hora de las vistas fijas, sopla mas tarde y
    /// nunca pasa de 1.
    #[test]
    fn the_xocomil_comes_in_the_afternoon() {
        assert_eq!(xocomil(40.0), 0.0);
        assert!((0..300).map(|t| xocomil(t as f32)).fold(0.0, f32::max) > 0.99);
        assert!((0..3000).all(|t| (0.0..=1.0).contains(&xocomil(t as f32 * 0.1))));
    }

    /// Un rayo horizontal hacia la cima de un volcan le pega a su cono.
    #[test]
    fn a_ray_toward_a_volcano_hits_its_cone() {
        let v = &VOLCANOES[3];
        let axis = site_dir(v.x, v.z);
        let apex = axis * (RADIUS + v.height);
        let origin = PANAJACHEL.normalize() * RADIUS;
        let target = axis * (RADIUS + v.height * 0.5);
        let d = (target - origin).normalize();
        let (t, along) = cone_hit(origin - apex, d, axis, v).unwrap();
        assert!(t > 0.0 && along > 0.0 && along < v.height);
        assert!(cone_hit(origin - apex, -d, axis, v).is_none());
    }
}
