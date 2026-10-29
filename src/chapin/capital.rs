//! La Capital (la version chapina de Cementerio): la Ciudad de Guatemala
//! sobre su meseta, partida por barrancos verdes y bajo el smog. En El
//! Reformador, la torre de hierro sobre la 7a Avenida, con camionetas y
//! edificios alrededor; en su anillo, camionetas y tuc-tucs dan vueltas como
//! un trafico en orbita.
//!
//! # El relieve
//! - **La meseta**: lomas suaves; donde un ruido grande sube, cerros con
//!   crestas (ruido con crestas) donde no llega la ciudad.
//! - **Barrancos**: donde un ruido cruza por cero, un perfil con fondo
//!   angosto, paredes empinadas y la orilla redondeada (una `smoothstep` de
//!   la distancia al cero). Un ruido mas fino hace ramales menos hondos.
//! - **El Volcan de Agua**: un cono con flancos concavos, crater y canales
//!   que bajan (ruido con crestas que solo cambia con el rumbo desde la
//!   cima).
//! - **El llano del Reformador**: alrededor del lugar todo se aplana.
//!
//! # La ciudad (en el shader)
//! - **Zonas**: celdas de Voronoi (Worley) sobre el planeta; cada zona tiene
//!   su cuadricula, girada a su modo y con cuadras de otro tamano, como las
//!   zonas de la capital. La frontera entre dos zonas (la distancia al plano
//!   bisector de sus puntos) es una calzada. La zona del Reformador pesa mas
//!   (un diagrama de potencia): su cuadricula es la de la 7a Avenida.
//! - **Cuadras y lotes**: con `floor` y `fract` sobre las coordenadas de la
//!   zona salen la calle, la banqueta y el lote; cada lote es un hash: su
//!   altura y su techo (losa con tinaco, lamina, teja o pintado).
//! - **Sombras de los edificios**: se camina unos pasos hacia la estrella
//!   por la cuadricula y se compara la altura de cada lote con la del rayo.
//! - **De noche**: postes de luz de sodio con su charco en el asfalto,
//!   ventanas que se encienden y apagan y faros por las calzadas. Todo es
//!   funcion del tiempo y de hashes. De lejos se dibuja el promedio.
//!
//! # Los objetos
//! La Torre del Reformador (cuatro patas de hierro que se abren en arcos
//! sobre la avenida, celosia de diagonales, campana y luz arriba), las
//! camionetas (bus escolar pintado a la chapina, con parrilla y bultos) y
//! edificios de block con tinaco y varillas. Se colocan con la misma
//! cuadricula del shader: los edificios en los lotes, las camionetas en la
//! avenida.
//!
//! # El anillo
//! Celdas en polares: cada carril gira a su velocidad y cada celda puede
//! tener una camioneta o un tuc-tuc (formas con distancia con signo), con
//! faros adelante y calaveras atras. De lejos, su promedio.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::math::{DVec3, Perlin, Vec3, hash_u32, smoothstep};
use crate::scene::Mesh;
use crate::scene::Rings;
use crate::scene::builder::MeshBuilder;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter, clear_site, upright_rotation};
use crate::shaders::rings::{RingInfo, RingStyle};
use crate::shaders::{AtmosphereParams, Fragment, PropStyle, Uniforms, detail, limb_haze, pbr};
use crate::terrain::{HeightField, TerrainSample};

pub fn spec() -> Spec {
    Spec {
        label: "La Capital",
        material: Chapin::Capital,
        // Smog: humo y polvo (mucho Mie) en un aire pardo y bajo.
        atmosphere: Some(AtmosphereParams {
            thickness: 14.0,
            scale_height: 4.0,
            rayleigh: Vec3::new(0.0042, 0.0037, 0.0031),
            mie: 0.006,
            mie_g: 0.72,
            intensity: 1.4,
        }),
        rings: Some(Rings {
            inner: 1.7,
            outer: 3.1,
            style: RingStyle::Buses,
        }),
        motes: Some((Motes::Smog, 100.0)),
    }
}

// El relieve ----------------------------------------------------------------

/// Radio del planeta: pasa angulos a unidades del mundo.
const RADIUS: f32 = 140.0;
/// El Reformador (la vertical del lugar), hacia donde corre la 7a Avenida y
/// la derecha, en el espacio del planeta. A los 40 s la estrella esta a unos
/// 20 grados, atras a la izquierda: luz de tarde de lado sobre la torre, y
/// el anillo pasa como un arco enfrente.
const UP: Vec3 = Vec3::new(0.595_794_9, -0.784_393_2, -0.172_498_5);
const FORWARD: Vec3 = Vec3::new(0.662_007_1, 0.601_245, -0.447_494_1);
const RIGHT: Vec3 = Vec3::new(-0.454_725_2, -0.152_419_5, -0.877_492_6);
/// Radio angular del llano del Reformador: plano hasta el primero, se junta
/// con el valle hasta el segundo.
const PLAZA: (f32, f32) = (0.36, 0.5);
/// El Volcan de Agua visto desde el lugar: a que angulo y con que rumbo (0
/// al frente, positivo a la derecha); su radio angular y su altura.
const AGUA: (f32, f32) = (0.65, -0.45);
const AGUA_SPAN: f32 = 0.32;
const AGUA_HEIGHT: f32 = 32.0;
/// Hondura de los barrancos.
const GORGE_DEPTH: f32 = 14.0;
/// Lo mas bajo y lo mas alto del relieve.
const LOWEST: f32 = -16.5;
const HIGHEST: f32 = 34.5;

/// El valle de la capital. Atributos: `x` ciudad, `y` barranco (1 en el
/// fondo, 0 en la orilla), `z` el volcan.
pub struct Valle {
    noise: Perlin,
    agua: Vec3,
    /// Altura del llano del Reformador.
    plaza: f32,
}

impl Valle {
    pub fn new(seed: u32) -> Self {
        let (angle, heading) = AGUA;
        let toward = FORWARD * heading.cos() + RIGHT * heading.sin();
        let mut valle = Self {
            noise: Perlin::new(seed),
            agua: (UP * angle.cos() + toward * angle.sin()).normalize(),
            plaza: 0.0,
        };
        valle.plaza = valle.relief(UP).height;
        valle
    }

    /// El relieve sin el llano.
    fn relief(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let warp = Vec3::new(
            n.noise(dir * 2.6 + Vec3::new(3.1, 0.0, 0.0)),
            n.noise(dir * 2.6 + Vec3::new(0.0, 7.7, 0.0)),
            n.noise(dir * 2.6 + Vec3::new(0.0, 0.0, -5.3)),
        ) * 0.08;
        let q = dir + warp;
        // Cerca del lugar los barrancos y los cerros se acaban.
        let from_site = dir.dot(UP).clamp(-1.0, 1.0).acos();
        let open = smoothstep(PLAZA.0, PLAZA.1 + 0.2, from_site);
        // El volcan: flancos concavos, crater y canales segun el rumbo.
        let cos_a = dir.dot(self.agua);
        let a = cos_a.clamp(-1.0, 1.0).acos() / AGUA_SPAN;
        let skirt = smoothstep(1.3, 0.85, a);
        let mut volcano = 0.0;
        if a < 1.0 {
            let side = dir - self.agua * cos_a;
            let side = side / side.length().max(1e-6);
            let gullies = n.ridged(side * 6.0 + Vec3::splat(a * 1.5), 2);
            let flank = (1.0 - a).powf(1.7);
            let crater = 2.5 * smoothstep(0.08, 0.0, a);
            volcano = AGUA_HEIGHT * flank - crater - 6.0 * gullies * a * (1.0 - a);
        }
        // La meseta y los cerros donde no hay ciudad.
        let rolling = n.fbm(q * 3.0 + Vec3::new(-2.2, 4.4, 1.7), 3);
        let wild = smoothstep(0.3, 0.55, n.fbm(q * 1.5 + Vec3::new(6.3, -3.9, 2.8), 3)) * open * (1.0 - skirt);
        let hills = wild * (3.0 + 8.0 * (n.fbm(q * 3.5 + Vec3::new(1.4, 8.8, -6.1), 3) * 0.5 + 0.5));
        // Barrancos: distancia al cero de un ruido, en anchos del barranco.
        // Los ramales solo salen en algunas partes.
        let main = n.noise(q * 1.6 + Vec3::new(5.5, -1.1, 3.3)).abs() / 0.075
            + 3.0 * smoothstep(-0.05, -0.4, n.noise(q * 1.1 + Vec3::new(-6.2, 1.9, 4.6)));
        let branch = n.noise(q * 3.6 + Vec3::new(-3.7, 2.9, 7.1)).abs() / 0.07
            + 4.0 * smoothstep(0.15, -0.2, n.noise(q * 1.3 + Vec3::new(2.0, 6.5, -4.4)));
        let fade = open * (1.0 - skirt);
        let gorge = (1.0 - smoothstep(0.1, 1.0, main)).max(0.6 * (1.0 - smoothstep(0.1, 1.0, branch))) * fade;
        let urban = (1.0 - smoothstep(0.1, 0.45, wild)) * smoothstep(0.95, 1.35, a) * (1.0 - smoothstep(0.0, 0.2, gorge));
        TerrainSample {
            height: 1.6 * rolling + hills + volcano - GORGE_DEPTH * gorge,
            attrs: Vec3::new(urban, gorge, skirt),
        }
    }
}

impl HeightField for Valle {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let mut s = self.relief(dir);
        let angle = dir.dot(UP).clamp(-1.0, 1.0).acos();
        if angle < PLAZA.1 {
            let flat = smoothstep(PLAZA.1, PLAZA.0, angle);
            s.height += (self.plaza - s.height) * flat;
            s.attrs = s.attrs.lerp(Vec3::new(1.0, 0.0, 0.0), flat);
        }
        s.height = s.height.clamp(LOWEST, HIGHEST);
        s
    }

    fn height_range(&self) -> (f32, f32) {
        // Abajo: la loma mas baja y el barranco mas hondo. Arriba: el volcan
        // (los cerros no llegan a el) y la loma mas alta.
        (LOWEST, HIGHEST)
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(Valle::new(seed))
}

// La ciudad -------------------------------------------------------------------

/// Lado de la celda de las zonas, en unidades del mundo.
const ZONE: f32 = 46.0;
/// Hasta donde manda la zona del Reformador (el peso de su punto).
const SITE_ZONE: f32 = 70.0;
/// Numero de la zona del Reformador.
const SITE_ID: u32 = 0x7A09;
/// Cuadras del Reformador, ancho de sus calles y de la 7a Avenida.
const SITE_BLOCK: f32 = 12.0;
const STREET: f32 = 2.6;
const AVENIDA: f32 = 4.8;
/// Medio ancho de las calzadas entre zonas.
const CALZADA: f32 = 2.2;
/// Alto de un piso y espacio entre postes de luz.
const STOREY: f32 = 0.9;
const LAMP_STEP: f32 = 3.2;

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Una zona de la ciudad: su numero, los ejes de su cuadricula en el
/// espacio del planeta (`ax` a lo ancho, `ay` a lo largo de las avenidas),
/// la distancia al borde con la zona vecina y la direccion de ese borde.
#[derive(Clone, Copy)]
struct Zone {
    id: u32,
    ax: Vec3,
    ay: Vec3,
    edge: f32,
    along: Vec3,
}

/// La zona del punto `p` (en el espacio del planeta): Worley con la
/// distancia de potencia (`|p - c|^2 - peso`), asi la del Reformador abarca
/// mas. La frontera con la segunda mas cercana es un plano: la distancia a
/// el sale de la diferencia de las distancias.
fn zone(p: Vec3) -> Zone {
    let q = p / ZONE;
    let base = (q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32);
    let site = UP * RADIUS;
    let mut best = ((p - site).length_squared() - SITE_ZONE * SITE_ZONE, site, SITE_ID);
    let mut second = (f32::MAX, site);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy, cz) = (base.0 + dx, base.1 + dy, base.2 + dz);
                let h = hash_u32(
                    (cx as u32).wrapping_mul(0x8DA6_B343)
                        ^ (cy as u32).wrapping_mul(0xD816_3841)
                        ^ (cz as u32).wrapping_mul(0xCB1A_B31F)
                        ^ 0x20E5,
                );
                let (h2, h3) = (hash_u32(h), hash_u32(h ^ 0x9E37));
                let c = Vec3::new(cx as f32 + unit(h), cy as f32 + unit(h2), cz as f32 + unit(h3)) * ZONE;
                let d = (p - c).length_squared();
                if d < best.0 {
                    second = (best.0, best.1);
                    best = (d, c, h3);
                } else if d < second.0 {
                    second = (d, c);
                }
            }
        }
    }
    let toward = second.1 - best.1;
    let edge = (second.0 - best.0) / (2.0 * toward.length().max(1e-3));
    let along = (best.1 + second.1).normalize().cross(toward).normalize();
    let (ax, ay) = if best.2 == SITE_ID {
        (RIGHT, FORWARD)
    } else {
        let (e1, e2) = crate::scene::builder::perpendiculars(best.1.normalize());
        let (s, c) = (unit(hash_u32(best.2)) * TAU).sin_cos();
        (e1 * c + e2 * s, e2 * c - e1 * s)
    };
    Zone { id: best.2, ax, ay, edge, along }
}

/// La cuadricula de una zona: largo de las cuadras (a lo ancho y a lo
/// largo), ancho de las calles y donde empieza.
#[derive(Clone, Copy)]
struct Grid {
    block: (f32, f32),
    street: f32,
    origin: (f32, f32),
    id: u32,
}

/// Lo que hay en un punto de la cuadricula.
#[derive(Clone, Copy)]
struct Spot {
    /// Distancia con signo a la orilla de la cuadra (negativa en la calle).
    curb: f32,
    /// De la calle mas cercana: distancia a su centro, posicion a lo largo,
    /// medio ancho y su numero.
    across: f32,
    along: f32,
    half: f32,
    street: u32,
    block: u32,
    lot: u32,
    /// Distancia al borde del lote.
    seam: f32,
    /// Altura de lo construido en el lote (cero en el patio).
    height: f32,
    patio: bool,
    /// Posicion dentro del lote: hacia adentro desde la calle y a lo largo.
    inner: (f32, f32),
    /// Fondo y frente del lote.
    size: (f32, f32),
}

/// Que hay en cada lote.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Roof {
    /// Losa de concreto con su tinaco.
    Slab,
    /// Lamina acanalada, nueva o con oxido.
    Sheet,
    /// Teja de barro.
    Tile,
    /// Losa pintada.
    Painted,
    /// Patio con arboles.
    Garden,
}

impl Zone {
    fn grid(&self) -> Grid {
        if self.id == SITE_ID {
            // La torre queda en el cruce de la 7a Avenida (x = 0) con una calle.
            return Grid {
                block: (SITE_BLOCK, SITE_BLOCK),
                street: STREET,
                origin: (0.0, 0.0),
                id: SITE_ID,
            };
        }
        let h = |k: u32| unit(hash_u32(self.id ^ k.wrapping_mul(0x2C1B_3C6D)));
        let block = (8.5 + 4.0 * h(1), 9.5 + 6.0 * h(2));
        Grid {
            block,
            street: 1.5 + 0.6 * h(3),
            origin: (h(4) * block.0, h(5) * block.1),
            id: self.id,
        }
    }
}

impl Grid {
    /// Medio ancho de la calle numero `k` a lo ancho (la 7a Avenida es mas
    /// ancha).
    #[inline]
    fn half_x(&self, k: f32) -> f32 {
        if self.id == SITE_ID && k == 0.0 { AVENIDA * 0.5 } else { self.street * 0.5 }
    }

    fn at(&self, x: f32, y: f32) -> Spot {
        let (bx, by) = self.block;
        let (gx, gy) = ((x - self.origin.0) / bx, (y - self.origin.1) / by);
        let (ix, iy) = (gx.floor(), gy.floor());
        let (fx, fy) = ((gx - ix) * bx, (gy - iy) * by);
        let (left, right) = (self.half_x(ix), self.half_x(ix + 1.0));
        let half_y = self.street * 0.5;
        let curb_x = (fx - left).min(bx - fx - right);
        let curb_y = (fy - half_y).min(by - fy - half_y);
        let block = hash_u32((ix as i32 as u32).wrapping_mul(0x8DA6_B343) ^ (iy as i32 as u32).wrapping_mul(0xD816_3841) ^ self.id);
        // La calle mas cercana: una avenida (a lo largo) o una calle.
        let (across, along, half, street) = if curb_x < curb_y {
            let near_left = fx < bx * 0.5;
            let k = if near_left { ix } else { ix + 1.0 };
            let across = if near_left { fx } else { bx - fx };
            (across, y, self.half_x(k), hash_u32(k as i32 as u32 ^ self.id ^ 0xA7))
        } else {
            let k = if fy < by * 0.5 { iy } else { iy + 1.0 };
            (fy.min(by - fy), x, half_y, hash_u32(k as i32 as u32 ^ self.id ^ 0xC4))
        };
        // Dos filas de lotes espalda con espalda, cada una de frente a una
        // avenida; a lo largo, lotes del mismo frente.
        let (w, d) = (bx - left - right, by - 2.0 * half_y);
        let (u0, v0) = (fx - left, fy - half_y);
        let row = u0 > w * 0.5;
        let front = 2.2 + 1.3 * unit(block);
        let count = (d / front).floor().max(1.0);
        let front = d / count;
        let k = (v0 / front).floor().clamp(0.0, count - 1.0);
        let lot = hash_u32(block ^ (k as u32).wrapping_mul(0x9E37_79B9) ^ row as u32);
        let seam = (u0 - w * 0.5).abs().min(v0 - k * front).min((k + 1.0) * front - v0);
        let inner = (if row { w - u0 } else { u0 }, v0 - k * front);
        // Atras de la casa queda el patio, a ras del suelo; los edificios
        // altos llenan su lote.
        let height = lot_height(lot, block, self.id);
        let built = if height > 4.0 * STOREY { 1.0 } else { 0.5 + 0.45 * unit(hash_u32(lot ^ 0xBAC)) } * w * 0.5;
        let patio = inner.0 > built;
        Spot {
            curb: curb_x.min(curb_y),
            across,
            along,
            half,
            street,
            block,
            lot,
            seam: seam.min((inner.0 - built).abs()),
            height: if patio { 0.0 } else { height },
            patio,
            inner,
            size: (w * 0.5, front),
        }
    }
}

/// Que tan alto es lo construido en un lote: casi todo de uno a tres
/// pisos, a veces un edificio.
#[inline]
fn lot_height(lot: u32, block: u32, zone: u32) -> f32 {
    if zone == SITE_ID || park(block) {
        return 0.0;
    }
    let tall = unit(hash_u32(lot ^ 0x7A11));
    let pick = unit(hash_u32(lot ^ 0x5701));
    let storeys = if tall < 0.05 { 5.0 + (pick * 9.0).floor() } else { 1.0 + (pick * pick * 3.2).floor() };
    storeys * STOREY
}

/// Una cuadra entera de parque.
#[inline]
fn park(block: u32) -> bool {
    unit(hash_u32(block ^ 0xBA4C)) < 0.06
}

#[inline]
fn roof(lot: u32, height: f32) -> Roof {
    let r = unit(hash_u32(lot ^ 0x2007));
    if height > 4.0 * STOREY {
        return if r < 0.7 { Roof::Slab } else { Roof::Painted };
    }
    match r {
        r if r < 0.42 => Roof::Slab,
        r if r < 0.68 => Roof::Sheet,
        r if r < 0.8 => Roof::Tile,
        r if r < 0.9 => Roof::Painted,
        _ => Roof::Garden,
    }
}

// El shader del suelo ---------------------------------------------------------

const ASPHALT: Vec3 = Vec3::new(0.045, 0.045, 0.05);
const SIDEWALK: Vec3 = Vec3::new(0.3, 0.28, 0.25);
const CONCRETE: Vec3 = Vec3::new(0.38, 0.37, 0.34);
const ZINC: Vec3 = Vec3::new(0.4, 0.42, 0.44);
const RUST: Vec3 = Vec3::new(0.24, 0.11, 0.05);
const CLAY: Vec3 = Vec3::new(0.32, 0.12, 0.065);
const TANK: Vec3 = Vec3::new(0.02, 0.02, 0.022);
const GRASS: Vec3 = Vec3::new(0.08, 0.12, 0.04);
const PASTURE: Vec3 = Vec3::new(0.17, 0.16, 0.07);
const CANOPY: Vec3 = Vec3::new(0.018, 0.05, 0.02);
const CANOPY_LIGHT: Vec3 = Vec3::new(0.05, 0.11, 0.035);
const TALPETATE: Vec3 = Vec3::new(0.42, 0.35, 0.25);
const ASH: Vec3 = Vec3::new(0.16, 0.14, 0.13);
const RIVER: Vec3 = Vec3::new(0.025, 0.03, 0.022);
/// Pinturas de las casas y los edificios.
const PAINTS: [Vec3; 10] = [
    Vec3::new(0.62, 0.56, 0.45),
    Vec3::new(0.6, 0.6, 0.58),
    Vec3::new(0.2, 0.42, 0.48),
    Vec3::new(0.6, 0.3, 0.33),
    Vec3::new(0.62, 0.45, 0.14),
    Vec3::new(0.33, 0.5, 0.36),
    Vec3::new(0.5, 0.2, 0.1),
    Vec3::new(0.28, 0.36, 0.55),
    Vec3::new(0.0, 0.42, 0.4),
    Vec3::new(0.62, 0.36, 0.18),
];
/// Luz de sodio de los postes, de las ventanas y de los faros.
const SODIUM: Vec3 = Vec3::new(1.0, 0.42, 0.1);
const WINDOW: Vec3 = Vec3::new(1.0, 0.75, 0.45);
const NEON: Vec3 = Vec3::new(0.7, 0.85, 1.0);
const HEADLIGHT: Vec3 = Vec3::new(1.0, 0.9, 0.75);
const TAILLIGHT: Vec3 = Vec3::new(1.0, 0.05, 0.02);
/// Cuanto brilla un poste y el charco que deja en el asfalto.
const LAMP_GLOW: f32 = 24.0;
const POOL_GLOW: f32 = 2.0;

/// Lo que deja cada material del suelo para la luz: su color, la normal (en
/// el mundo), aspereza, oclusion, la sombra de los edificios que dibuja el
/// shader y la luz propia.
#[derive(Clone, Copy)]
struct Ground {
    albedo: Vec3,
    normal: Vec3,
    rough: f32,
    ao: f32,
    lit: f32,
    glow: Vec3,
}

impl Ground {
    fn lerp(self, o: Ground, t: f32) -> Ground {
        Ground {
            albedo: self.albedo.lerp(o.albedo, t),
            normal: self.normal.lerp(o.normal, t),
            rough: self.rough + (o.rough - self.rough) * t,
            ao: self.ao + (o.ao - self.ao) * t,
            lit: self.lit + (o.lit - self.lit) * t,
            glow: self.glow.lerp(o.glow, t),
        }
    }
}

/// Lo que todos los materiales del suelo necesitan saber.
struct Ctx<'a> {
    f: &'a Fragment,
    u: &'a Uniforms<'a>,
    /// Lo que mide un pixel en el suelo.
    footprint: f32,
    /// La luz de la estrella en el espacio del planeta.
    light: Vec3,
    /// Vertical en el espacio del planeta.
    up: Vec3,
    /// Que tan de noche es (0 de dia, 1 de noche): prende las luces.
    night: f32,
}

/// Del mundo al espacio del planeta (la transpuesta del giro).
#[inline]
fn to_local(f: &Fragment, v: Vec3) -> Vec3 {
    let m = &f.object_to_world.m;
    Vec3::new(
        m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
        m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
        m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
    )
}

/// Una linea de ancho `width` a distancia `d` cuando un pixel mide `aa`: si
/// es mas delgada que un pixel se ensancha y baja.
#[inline]
fn line(d: f32, width: f32, aa: f32) -> f32 {
    smoothstep(width + aa, 0.0, d) * (width / (width + aa)).min(1.0)
}

/// Un punto de luz de radio `size` a distancia `d`: cuando el pixel es mas
/// grande que el punto se abre y baja, con la misma energia.
#[inline]
fn spot_light(d: f32, size: f32, aa: f32) -> f32 {
    smoothstep(size + aa, 0.0, d) * (size / (size + aa)).powi(2)
}

/// Que tan de noche es con la estrella a `height` (el seno de su altura):
/// las luces prenden al atardecer.
#[inline]
fn dusk(height: f32) -> f32 {
    smoothstep(0.16, -0.04, height)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (urban, gorge, volcano) = (f.color.x, f.color.y, f.color.z);
    let p = f.local;
    let up = p.normalize();
    let light = to_local(f, f.light_dir(u));
    let c = Ctx {
        f,
        u,
        footprint: f.pos.length() * u.pixel_angle,
        light,
        up,
        night: dusk(up.dot(light)),
    };
    // Las casitas de las paredes de los barrancos siguen la cuadricula de
    // la zona, como la ciudad.
    let shacks = smoothstep(0.03, 0.1, gorge) * smoothstep(0.45, 0.25, gorge) * smoothstep(0.6, 0.4, f.uv.y);
    let z = (urban > 0.01 || shacks > 0.0).then(|| zone(p));
    let mut ground = match z {
        Some(z) if urban > 0.01 => {
            let town = city(&c, &z);
            if urban < 0.99 { hills(&c, volcano).lerp(town, urban) } else { town }
        }
        _ => hills(&c, volcano),
    };
    let wall = smoothstep(0.02, 0.15, gorge);
    if wall > 0.0 {
        ground = ground.lerp(barranco(&c, z.as_ref(), gorge, shacks), wall);
    }
    let lit = pbr::shade_light(f, u, ground.albedo, ground.rough, 0.0, ground.normal, ground.ao, u.sun_light * ground.lit);
    lit + ground.glow * u.gains.cementerio + limb_haze(f, Vec3::new(0.35, 0.28, 0.2), 0.35, f.normal.dot(f.light_dir(u)))
}

/// La ciudad: calles, banquetas y lotes de su zona, con la sombra de los
/// edificios; de noche, postes, ventanas y faros. De lejos, el promedio.
fn city(c: &Ctx, z: &Zone) -> Ground {
    let (f, u, fp) = (c.f, c.u, c.footprint);
    let p = f.local;
    let n = u.noise;
    let grid = z.grid();
    let (x, y) = (p.dot(z.ax), p.dot(z.ay));
    let s = grid.at(x, y);
    // Cada zona con su tono: unas mas de lamina y teja, otras de concreto.
    let zone_tint = Vec3::ONE.lerp(Vec3::new(1.1, 0.95, 0.82), unit(hash_u32(z.id ^ 0x71)));
    let grime = n.fbm(p * 0.35, 2) * 0.5 + 0.5;

    // La calle: asfalto con parches, la banqueta y una linea al centro de
    // las avenidas.
    let street = smoothstep(fp * 0.5, -fp * 0.5, s.curb);
    let walk = smoothstep(-fp * 0.5, fp * 0.5, s.curb) * smoothstep(0.35 + fp * 0.5, 0.35 - fp * 0.5, s.curb);
    let patch = smoothstep(0.3, 0.7, n.noise(p * 0.6) * 0.5 + 0.5);
    let mut road = ASPHALT * (0.8 + 0.5 * patch);
    let avenue = s.half > STREET * 0.5;
    if avenue {
        let dash = smoothstep(0.3, 0.35, ((s.along / 1.2).fract() - 0.5).abs());
        road = road.lerp(Vec3::new(0.5, 0.42, 0.15), line(s.across, 0.05, fp) * (1.0 - dash));
    }
    let (lot, lot_rough, lot_tilt, lot_ao) = lot_surface(c, &s, z.ax, z.ay);
    let seam = 1.0 - 0.6 * line(s.seam, 0.05, fp);
    let paved = street.max(walk);
    let mut albedo = (lot * zone_tint).lerp(SIDEWALK * (0.85 + 0.3 * grime), walk).lerp(road, street);
    let mut rough = lot_rough + (0.9 - lot_rough) * paved;
    let mut ao = lot_ao * if s.curb > 0.0 { seam } else { 1.0 };
    let tilt = lot_tilt * (1.0 - paved);
    // Calzadas entre zonas: anchas, con camellon de grama.
    let calzada = smoothstep(CALZADA + fp * 0.5, CALZADA - fp * 0.5, z.edge);
    if calzada > 0.0 {
        let median = smoothstep(0.35 + fp * 0.5, 0.35 - fp * 0.5, z.edge);
        albedo = albedo.lerp((ASPHALT * (0.85 + 0.4 * patch)).lerp(GRASS, median), calzada);
        rough += (0.9 - rough) * calzada;
        ao += (1.0 - ao) * calzada;
    }

    // Sombras de los edificios: unos pasos por la cuadricula hacia la
    // estrella; si un lote es mas alto que el rayo, tapa.
    let mut lit = 1.0;
    let sun_up = c.up.dot(c.light);
    let (lx, ly) = (c.light.dot(z.ax), c.light.dot(z.ay));
    let flat = (lx * lx + ly * ly).sqrt();
    if sun_up > 0.0 && flat > 0.05 && fp < 1.5 {
        let rise = sun_up / flat;
        let (dx, dy) = (lx / flat, ly / flat);
        let here = if s.curb > 0.0 { s.height } else { 0.0 };
        let mut shadow: f32 = 0.0;
        for step in [0.45, 1.1, 2.2, 3.8] {
            let o = grid.at(x + dx * step, y + dy * step);
            let tall = if o.curb > 0.0 { o.height } else { 0.0 };
            shadow = shadow.max(smoothstep(0.0, 0.25, tall - here - step * rise));
        }
        lit = 1.0 - 0.8 * shadow * smoothstep(1.5, 0.8, fp);
    }

    let mut glow = Vec3::ZERO;
    if c.night > 0.0 {
        glow = night_lights(c, &s, z, albedo, paved, calzada);
    }

    // De lejos: el promedio de la zona y el de sus luces (la energia de un
    // poste y su charco repartida en la calle que alumbra).
    let resolved = smoothstep(2.2, 0.8, fp);
    let street_share = (grid.street / grid.block.0 + grid.street / grid.block.1).min(0.6);
    let average = CONCRETE.lerp(RUST, 0.25) * 0.85 * zone_tint;
    let average = average.lerp(ASPHALT, street_share);
    let per_lamp = LAMP_GLOW * 0.94 * 0.12 * 0.12 + POOL_GLOW * PI * 1.4 * ASPHALT.x;
    let far_glow = (SODIUM * (street_share * 0.45 * per_lamp / (LAMP_STEP * grid.street)) + WINDOW * (0.016 + 0.012 * unit(z.id ^ 0x4E))) * c.night;
    let normal = (f.normal + f.object_to_world.mul_vec(tilt)).normalize();
    Ground {
        albedo: average.lerp(albedo, resolved),
        normal: f.normal.lerp(normal, resolved),
        rough: 0.85 + (rough - 0.85) * resolved,
        ao: 1.0 + (ao - 1.0) * resolved,
        lit: 1.0 + (lit - 1.0) * resolved,
        glow: far_glow.lerp(glow, resolved),
    }
}

/// Las luces de la ciudad de noche: un poste de sodio cada `LAMP_STEP` en
/// una banqueta de cada calle (unas calles mas alumbradas que otras, unos
/// postes fundidos) con su charco en el asfalto; las ventanas de cada lote,
/// que prenden y apagan a su ritmo; y los faros que corren por las calzadas.
fn night_lights(c: &Ctx, s: &Spot, z: &Zone, albedo: Vec3, paved: f32, calzada: f32) -> Vec3 {
    let (u, fp) = (c.u, c.footprint);
    let k = (s.along / LAMP_STEP).round();
    let side = if s.street & 1 == 0 { 1.0 } else { -1.0 };
    let d = (s.across * side - (s.half - 0.15)).hypot(s.along - k * LAMP_STEP);
    let id = hash_u32(s.street ^ (k as i32 as u32).wrapping_mul(0x9E37_79B9));
    let bright = if s.half > STREET * 0.5 { 1.0 } else { 0.15 + 0.55 * unit(s.street) };
    let alive = (unit(hash_u32(id)) > 0.08) as u8 as f32 * bright;
    let twinkle = 0.85 + 0.15 * (u.time * (2.0 + 3.0 * unit(id)) + unit(id >> 3) * TAU).sin();
    let lamp = spot_light(d, 0.12, fp) * twinkle * alive;
    let pool = (-d * d / 1.4).exp() * alive * paved;
    // Ventanas: en la orilla de cada lote que da a la calle.
    let period = 20.0 + 40.0 * unit(s.lot ^ 0x11);
    let on = unit(hash_u32(s.lot ^ ((u.time / period + unit(s.lot)).floor() as u32))) < 0.5;
    let facade = smoothstep(0.5, 0.3, s.inner.0) * smoothstep(0.05, 0.15, s.inner.0) * (s.curb > 0.0) as u8 as f32;
    let pane = (s.inner.1 / 0.55).floor();
    let lit = unit(hash_u32(s.lot ^ (pane as i32 as u32).wrapping_mul(0x2C1B_3C6D))) < 0.5;
    let panes = smoothstep(0.22, 0.12, ((s.inner.1 / 0.55).fract() - 0.5).abs()) * lit as u8 as f32;
    let resolved = smoothstep(0.3, 0.1, fp);
    let windows = if on && s.height > 0.0 { facade * (0.12 + resolved * (panes - 0.12)) } else { 0.0 };
    let tone = if unit(s.lot ^ 0x3E) < 0.3 { NEON } else { WINDOW };
    // Y el resplandor de lo que no se alcanza a ver: patios, rotulos, casas.
    let haze = WINDOW * (0.006 + 0.012 * unit(z.id ^ 0x4E)) * (s.curb > 0.0) as u8 as f32;
    let mut glow = SODIUM * (lamp * LAMP_GLOW + albedo.x * pool * POOL_GLOW) + tone * (windows * 0.5) + haze;
    // Faros: carros que avanzan por cada lado del camellon, blancos de
    // frente y rojos de espalda.
    if calzada > 0.0 {
        let along = c.f.local.dot(z.along) / 2.2 + u.time * 2.5;
        let slot = along.floor();
        let car = hash_u32(z.id ^ (slot as i32 as u32).wrapping_mul(0x51ED_270B) ^ ((z.edge > 1.2) as u32));
        if unit(car) < 0.45 {
            let lane = if z.edge > 1.2 { 1.5 } else { 0.8 };
            let t = (along - slot - 0.5) * 2.2;
            let dy = z.edge - lane;
            let head = spot_light((t + 0.35).hypot(dy), 0.08, fp);
            let tail = spot_light((t - 0.35).hypot(dy), 0.08, fp);
            glow += (HEADLIGHT * head * 20.0 + TAILLIGHT * tail * 12.0) * calzada;
        }
    }
    glow * c.night
}

/// El techo de un lote (o su patio, junto al Reformador, donde los edificios
/// son de verdad): color, aspereza, inclinacion de la normal (en el espacio
/// del planeta) y oclusion.
fn lot_surface(c: &Ctx, s: &Spot, ax: Vec3, ay: Vec3) -> (Vec3, f32, Vec3, f32) {
    let (f, u, fp) = (c.f, c.u, c.footprint);
    let n = u.noise;
    let p = f.local;
    let grime = n.fbm(p * 0.5 + Vec3::splat(4.0), 2) * 0.5 + 0.5;
    let pick = |k: u32| unit(hash_u32(s.lot ^ k));
    if park(s.block) {
        let (albedo, normal, ao) = canopy(c, -0.1);
        return (GRASS.lerp(albedo, smoothstep(0.3, 0.6, grime)), 0.9, to_local(f, normal - f.normal), ao);
    }
    if s.height == 0.0 {
        // Patios: concreto y tierra, con grama y a veces un arbol.
        let dirt = smoothstep(0.4, 0.7, n.noise(p * 0.3) * 0.5 + 0.5);
        let albedo = CONCRETE.lerp(TALPETATE * 0.7, dirt).lerp(GRASS, 0.5 * smoothstep(0.55, 0.8, grime)) * 0.6;
        if s.patio && pick(7) < 0.4 {
            let (leaves, normal, ao) = canopy(c, 0.15);
            let tree = smoothstep(0.75, 0.55, (s.inner.0 - s.size.0 * 0.8).hypot(s.inner.1 - s.size.1 * 0.5) / s.size.1);
            return (albedo.lerp(leaves, tree), 0.9, to_local(f, (normal - f.normal) * tree), 1.0 - 0.3 * tree * (1.0 - ao));
        }
        return (albedo, 0.9, Vec3::ZERO, 0.85);
    }
    // Las acanaladuras y las hileras de teja se apagan antes de que un pixel
    // cubra una onda.
    let fine = smoothstep(0.06, 0.025, fp);
    let (lu, lv) = s.inner;
    match roof(s.lot, s.height) {
        Roof::Slab | Roof::Painted => {
            let base = if roof(s.lot, s.height) == Roof::Slab {
                CONCRETE * (0.7 + 0.4 * pick(1))
            } else {
                PAINTS[(hash_u32(s.lot ^ 0x9A) % PAINTS.len() as u32) as usize].lerp(CONCRETE, 0.45)
            };
            // El tinaco negro en una esquina y el pretil en el borde.
            let (tx, ty) = (lu - 0.35 - 0.3 * pick(2) * s.size.0, lv - 0.4 - 0.4 * pick(3) * s.size.1);
            let tank = (spot_light(tx.hypot(ty), 0.2, fp) * 4.0).min(1.0);
            let parapet = line(s.seam.min(lu), 0.07, fp);
            let albedo = (base * (0.75 + 0.35 * grime) * (1.0 + 0.3 * parapet)).lerp(TANK, tank);
            (albedo, 0.85, Vec3::ZERO, 1.0 - 0.3 * tank)
        }
        Roof::Sheet => {
            // Lamina acanalada: la normal ondula de un lado a otro.
            let rusty = smoothstep(0.35, 0.75, pick(4) * 0.7 + grime * 0.5);
            let albedo = ZINC.lerp(RUST, rusty) * (0.75 + 0.3 * pick(5));
            let wave = (lv * TAU / 0.18).sin() * fine;
            (albedo, 0.45 + 0.4 * rusty, ay * (wave * 0.25), 1.0)
        }
        Roof::Tile => {
            // Teja: dos aguas desde la cumbrera, con hileras.
            let side = if lu < s.size.0 * 0.5 { -1.0 } else { 1.0 };
            let rows = 1.0 - 0.3 * fine * smoothstep(0.35, 0.5, ((lu / 0.22).fract() - 0.5).abs());
            let albedo = CLAY * (0.75 + 0.4 * pick(6)) * rows * (0.8 + 0.3 * grime);
            (albedo, 0.8, ax * (side * 0.35), 1.0)
        }
        Roof::Garden => {
            let (albedo, normal, ao) = canopy(c, 0.1);
            (albedo, 0.9, to_local(f, normal - f.normal), ao)
        }
    }
}

/// Copas de arboles: cada celda de Worley es una copa, clara al centro y
/// oscura entre copas, con hojas del mapa de normales. De lejos, manchas.
/// Devuelve color, normal (en el mundo) y oclusion.
fn canopy(c: &Ctx, tone: f32) -> (Vec3, Vec3, f32) {
    const CROWN: f32 = 1.4;
    let (f, u, fp) = (c.f, c.u, c.footprint);
    let patch = u.noise.fbm(f.local * 0.08, 3) * 0.5 + 0.5;
    let base = CANOPY.lerp(CANOPY_LIGHT, (patch * 0.9 + tone).clamp(0.0, 1.0));
    let near = smoothstep(0.8, 0.3, fp / CROWN);
    if near <= 0.0 {
        return (base * 0.85, c.f.normal, 0.8);
    }
    let (f1, f2) = u.noise.cells(f.local / CROWN);
    let dome = 1.0 - (f1 / 0.9).min(1.0).powi(2);
    let gap = smoothstep(0.0, 0.3, f2 - f1);
    let leaves = detail::sample(f, &u.textures.organic, 0.7, fp, 0.3);
    let albedo = base * (0.85 + near * ((0.55 + 0.6 * dome) * (0.55 + 0.45 * gap) * (0.8 + 0.3 * leaves.height) - 0.85));
    let normal = f.normal.lerp(leaves.normal, near);
    (albedo, normal, 0.8 + near * (0.45 + 0.55 * gap - 0.8))
}

/// Los barrancos: monte verde en las paredes, derrumbes de tierra blanca
/// (talpetate), el rio sucio en el fondo y casitas de lamina colgadas de la
/// parte alta de las paredes.
fn barranco(c: &Ctx, z: Option<&Zone>, gorge: f32, shacks: f32) -> Ground {
    let (f, u, fp) = (c.f, c.u, c.footprint);
    let n = u.noise;
    let p = f.local;
    let slope = f.uv.y;
    let (mut albedo, mut normal, ao) = canopy(c, 0.0);
    // Las hojas casi no reflejan: sin brillo en las paredes vistas de lado.
    let mut rough = 1.0;
    // Derrumbes en lo mas empinado, con chorreones.
    let scar = smoothstep(0.7, 0.9, slope + 0.25 * n.noise(p * 0.35));
    let streaks = n.noise(p * 0.8 - c.up * (p.dot(c.up) * 0.6)) * 0.5 + 0.5;
    albedo = albedo.lerp(TALPETATE * (0.6 + 0.5 * streaks), scar);
    normal = normal.lerp(f.normal, scar);
    // El rio: agua oscura y quieta.
    let river = smoothstep(0.92, 0.97, gorge);
    albedo = albedo.lerp(RIVER, river);
    rough += (0.12 - rough) * river;
    normal = normal.lerp(f.normal, river);
    let mut glow = Vec3::ZERO;
    if let Some(z) = z {
        // Casitas: una cuadricula fina girada en la zona, con callejones.
        let here = shacks * smoothstep(0.4, 0.6, n.noise(p * 0.12 + Vec3::splat(2.0)) * 0.5 + 0.5) * (1.0 - scar);
        if here > 0.0 {
            let (sx, sy) = (p.dot(z.ax + z.ay), p.dot(z.ay - z.ax));
            let (gx, gy) = (sx / 1.05, sy / 0.85);
            let id = hash_u32((gx.floor() as i32 as u32).wrapping_mul(0x8DA6_B343) ^ (gy.floor() as i32 as u32).wrapping_mul(0xD816_3841) ^ 0x5AC);
            let border = (gx.fract().min(1.0 - gx.fract()) * 1.05).min(gy.fract().min(1.0 - gy.fract()) * 0.85);
            let roof = match id % 6 {
                0 => ZINC,
                1 => PAINTS[(id >> 8) as usize % PAINTS.len()] * 0.8,
                2 => CONCRETE * 0.8,
                _ => RUST.lerp(ZINC, unit(id >> 4) * 0.6),
            };
            let built = (unit(id) < 0.75) as u8 as f32 * here;
            let gap = 1.0 - 0.75 * line(border, 0.07, fp);
            let resolved = smoothstep(0.6, 0.2, fp);
            albedo = albedo.lerp(roof * gap, built * resolved).lerp(RUST.lerp(ZINC, 0.4) * 0.75, here * 0.6 * (1.0 - resolved));
            normal = normal.lerp(f.normal, built);
            if unit(id >> 4) < 0.2 {
                glow = WINDOW * (built * 0.12 * c.night);
            }
        }
    }
    Ground { albedo, normal, rough, ao, lit: 1.0, glow }
}

/// Fuera de la ciudad: monte con potreros en las laderas suaves, y en el
/// volcan, cafetales abajo y ceniza arriba.
fn hills(c: &Ctx, volcano: f32) -> Ground {
    let (f, u, fp) = (c.f, c.u, c.footprint);
    let n = u.noise;
    let p = f.local;
    let altitude = f.uv.x;
    let slope = f.uv.y;
    let (forest, normal, ao) = canopy(c, 0.05 * volcano);
    let open = smoothstep(0.55, 0.42, n.fbm(p * 0.06 + Vec3::splat(9.0), 3) * 0.5 + 0.5 + 0.4 * slope);
    let pasture = PASTURE.lerp(GRASS, n.noise(p * 0.2) * 0.5 + 0.5);
    let mut albedo = forest.lerp(pasture, open);
    let mut normal = normal.lerp(f.normal, open);
    // Arriba del volcan: ceniza con canales.
    let bare = smoothstep(0.7, 0.82, altitude + 0.05 * n.noise(p * 0.2)) * volcano;
    if bare > 0.0 {
        let rock = detail::rock(f, u, 0.15, fp, 0.8);
        albedo = albedo.lerp(ASH * (0.6 + 0.8 * rock.color.luminance()), bare);
        normal = normal.lerp(rock.normal, bare);
    }
    Ground { albedo, normal: normal.normalize(), rough: 1.0, ao: ao + (1.0 - ao) * open, lit: 1.0, glow: Vec3::ZERO }
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

// Los objetos -----------------------------------------------------------------

/// Nada se reparte al azar: lo que hay en la capital esta en El Reformador.
pub fn rules() -> Vec<Scatter> {
    Vec::new()
}

pub fn site(_default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    Some(("El Reformador", UP.as_dvec3(), FORWARD.as_dvec3(), place))
}

/// La direccion del planeta que cae en `(x, y)` de la cuadricula del
/// Reformador (la misma cuenta del shader: `p . eje`, con `p` en el suelo).
fn on_grid(field: &dyn HeightField, x: f64, y: f64) -> (DVec3, f64) {
    let (up, right, forward) = (UP.as_dvec3(), RIGHT.as_dvec3(), FORWARD.as_dvec3());
    let mut height = 0.0;
    let mut dir = up;
    for _ in 0..3 {
        let r = RADIUS as f64 + height;
        let (a, b) = (x / r, y / r);
        dir = (up * (1.0 - a * a - b * b).max(0.0).sqrt() + right * a + forward * b).normalize();
        height = field.sample(dir.as_vec3()).height as f64;
    }
    (dir, height)
}

/// Un objeto en `(x, y)` de la cuadricula, con su frente (+Z del modelo)
/// hacia `facing` (en ejes de la cuadricula: x a la derecha, y adelante).
fn put(field: &dyn HeightField, radius: f64, kind: PropKind, x: f64, y: f64, facing: (f64, f64), scale: f32, sink: f64) -> Instance {
    let (dir, height) = on_grid(field, x, y);
    let toward = RIGHT.as_dvec3() * facing.0 + FORWARD.as_dvec3() * facing.1;
    let toward = (toward - dir * toward.dot(dir)).normalize();
    let base = upright_rotation(dir, 0.0);
    let (x0, z0) = (base.rotate(DVec3::X), base.rotate(DVec3::Z));
    let yaw = toward.dot(x0).atan2(toward.dot(z0));
    Instance {
        kind,
        position: dir * (radius + height - sink * scale as f64),
        rotation: upright_rotation(dir, yaw),
        scale,
    }
}

/// Hasta donde hay edificios de verdad alrededor del Reformador.
const DOWNTOWN: f64 = 62.0;
/// La cuadra de la terminal de camionetas y el tamano de las camionetas.
const TERMINAL: (i32, i32) = (-2, 1);
const BUS_SCALE: f32 = 1.1;

/// El Reformador: la torre en el cruce de la 7a Avenida, edificios pegados
/// uno al otro en las orillas de las cuadras, una terminal de camionetas y
/// camionetas haciendo cola por la avenida y por las calles.
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    clear_site(instances, radius, frame.0, DOWNTOWN + 10.0);
    instances.push(put(field, radius, PropKind::Torre, 0.0, 0.0, (0.0, 1.0), 1.0, 0.05));
    let block = SITE_BLOCK as f64;
    let half_street = STREET as f64 * 0.5;
    let random = |h: u32| unit(h) as f64;
    let reach = (DOWNTOWN / block).ceil() as i32;
    for i in -reach..reach {
        for j in -reach..reach {
            let (x0, y0) = (i as f64 * block, j as f64 * block);
            // La orilla de la cuadra (la 7a Avenida es mas ancha).
            let left = x0 + if i == 0 { AVENIDA as f64 * 0.5 } else { half_street };
            let right = x0 + block - if i + 1 == 0 { AVENIDA as f64 * 0.5 } else { half_street };
            let (bottom, top) = (y0 + half_street, y0 + block - half_street);
            let center = ((left + right) * 0.5, (bottom + top) * 0.5);
            if center.0.hypot(center.1) > DOWNTOWN {
                continue;
            }
            let h = hash_u32(seed ^ (i as u32).wrapping_mul(0x9E37_79B9) ^ (j as u32).wrapping_mul(0x85EB_CA6B));
            if (i, j) == TERMINAL {
                // La terminal: camionetas estacionadas en bateria.
                for k in 0..5 {
                    let x = left + 1.0 + k as f64 * 1.6;
                    instances.push(put(field, radius, PropKind::Camioneta, x, center.1 + 0.6 * random(hash_u32(h ^ k)), (0.12, 1.0), BUS_SCALE, 0.0));
                }
                continue;
            }
            // Cada edificio: posicion, frente, escala y su medio fondo (el
            // modelo mide 5.1 de frente y 2.8 de fondo).
            let mut lots: Vec<(f64, f64, (f64, f64), f32)> = Vec::new();
            for (k, (x, facing)) in [(left, -1.0), (right, 1.0)].into_iter().enumerate() {
                let hk = hash_u32(h ^ k as u32 * 0x3C6E);
                let inward = -facing;
                if unit(hk) < 0.2 {
                    // Uno solo, mas alto, en todo el lado.
                    let s = 1.25 + 0.3 * unit(hk >> 4);
                    lots.push((x + inward * 1.42 * s as f64, center.1, (facing, 0.0), s));
                } else {
                    for slot in [-1.0, 1.0] {
                        let s = 0.72 + 0.2 * unit(hash_u32(hk ^ (slot > 0.0) as u32));
                        lots.push((x + inward * 1.42 * s as f64, center.1 + slot * 2.35, (facing, 0.0), s));
                    }
                }
            }
            for (k, (y, facing)) in [(bottom, -1.0), (top, 1.0)].into_iter().enumerate() {
                let s = 0.6 + 0.2 * unit(hash_u32(h ^ (k as u32 + 7)));
                lots.push((center.0, y - facing * 1.42 * s as f64, (0.0, facing), s));
            }
            for (k, &(x, y, facing, s)) in lots.iter().enumerate() {
                // La plaza de la torre queda libre y falta uno que otro.
                if x.hypot(y) < 12.0 || unit(hash_u32(h ^ k as u32 ^ 0xE1)) < 0.06 {
                    continue;
                }
                instances.push(put(field, radius, PropKind::Edificio, x, y, facing, s, 0.05));
            }
        }
    }
    // Camionetas en cola por la 7a Avenida, en los dos sentidos (una pasa
    // bajo los arcos de la torre), y algunas en las calles.
    for k in -6..16 {
        for lane in [-1.0, 1.0] {
            let h = hash_u32(seed ^ (k as u32).wrapping_mul(0x51ED) ^ (lane > 0.0) as u32 ^ 0xB05);
            if unit(h) < 0.3 {
                continue;
            }
            let y = k as f64 * 4.4 + 0.8 * random(hash_u32(h));
            instances.push(put(field, radius, PropKind::Camioneta, lane * 1.2, y, (0.0, lane), BUS_SCALE, 0.0));
        }
    }
    for k in 0..14u32 {
        let h = hash_u32(seed ^ k.wrapping_mul(0x3C6E) ^ 0xCA11);
        let street = (unit(h) * 8.0).floor() as f64 - 3.0;
        let x = (random(hash_u32(h)) - 0.5) * 2.0 * DOWNTOWN * 0.8;
        // Fuera de los cruces.
        if ((x / block).fract().abs() - 0.5).abs() > 0.32 {
            continue;
        }
        let side = if h & 1 == 0 { 1.0 } else { -1.0 };
        instances.push(put(field, radius, PropKind::Camioneta, x, street * block + side * 0.65, (side, 0.0), BUS_SCALE, 0.0));
    }
}

// Los modelos -----------------------------------------------------------------

/// Marcas de material en el color de vertice (`x`); `y` y `z` llevan datos.
const IRON: f32 = 0.0;
const BELL: f32 = 1.0 / 8.0;
const LAMP: f32 = 2.0 / 8.0;
const PAINT: f32 = 3.0 / 8.0;
const CHROME: f32 = 4.0 / 8.0;
const RUBBER: f32 = 5.0 / 8.0;
const CARGO: f32 = 6.0 / 8.0;
const WALL: f32 = 7.0 / 8.0;
const TRIM: f32 = 1.0;

/// La marca de material de un fragmento.
#[inline]
fn material(f: &Fragment) -> u32 {
    (f.color.x * 8.0 + 0.5) as u32
}

/// Una viga de seccion cuadrada de `a` a `c`.
fn beam(b: &mut MeshBuilder, a: Vec3, c: Vec3, half: f32) {
    let axis = (c - a).normalize();
    let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let side = axis.cross(helper).normalize();
    b.cuboid((a + c) * 0.5, [side, axis, side.cross(axis)], Vec3::new(half, (c - a).length() * 0.5, half));
}

/// Una caja alineada con los ejes del modelo.
fn bx(b: &mut MeshBuilder, center: Vec3, half: Vec3) {
    b.cuboid(center, [Vec3::X, Vec3::Y, Vec3::Z], half);
}

pub fn mesh(kind: PropKind, detail: usize) -> Mesh {
    match kind {
        PropKind::Torre => tower(detail),
        PropKind::Camioneta => bus(detail),
        _ => building(detail),
    }
}

/// Alto del fuste de la torre, de sus dos plataformas y de la punta del
/// faro. La de verdad mide unos 72 m: aqui una unidad son unos 3 m.
const SHAFT: f32 = 19.5;
const PLATFORM: f32 = 5.0;
const PLATFORM_2: f32 = 11.5;
const TOWER_TOP: f32 = 22.3;

/// Media anchura de la torre a la altura `y`. Como la torre Eiffel, en la
/// que se inspira: una curva exponencial que se abre mucho abajo (las patas
/// quedan a los lados de la 7a Avenida y los arcos pasan sobre ella) y se
/// cierra despacio hacia arriba.
fn tower_half(y: f32) -> f32 {
    0.5 + 3.7 * (-y / 5.0).exp()
}

/// La Torre del Reformador: cuatro patas de acero que se abren abajo en
/// arcos sobre la avenida y la calle, dos plataformas, el fuste de celosia
/// con diagonales en cruz, el campanario con su campana y el faro arriba.
fn tower(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(Vec3::new(IRON, 0.0, 0.0));
    let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let leg = |k: usize, y: f32| {
        let r = tower_half(y);
        Vec3::new(corners[k].0 * r, y, corners[k].1 * r)
    };
    // Las patas: vigas que siguen la curva, con mas tramos abajo, donde
    // mas se dobla.
    let steps = [6, 10, 16][detail];
    let heights: Vec<f32> = (0..=steps)
        .map(|i| SHAFT * (i as f32 / steps as f32).powf(1.6))
        .collect();
    for k in 0..4 {
        for w in heights.windows(2) {
            let thick = 0.26 - 0.16 * w[0] / SHAFT;
            beam(&mut b, leg(k, w[0] - 0.05), leg(k, w[1] + 0.05), thick);
        }
    }
    // Los arcos de abajo, uno por cara, con un arco de adentro y rayos.
    let segments = [6, 10, 16][detail];
    let spring = 0.6;
    for face in 0..4 {
        let (a, c) = (corners[face], corners[(face + 1) % 4]);
        let point = |t: f32, inset: f32| {
            let theta = t * PI;
            let y = spring + (PLATFORM - 0.2 - inset - spring) * theta.sin();
            let r = tower_half(y);
            let s = theta.cos() * (tower_half(spring) - inset);
            // Entre la pata `a` y la `c`, sobre la cara.
            let mid = Vec3::new((a.0 + c.0) * 0.5 * r, y, (a.1 + c.1) * 0.5 * r);
            let along = Vec3::new((a.0 - c.0) * 0.5, 0.0, (a.1 - c.1) * 0.5);
            mid + along * s
        };
        for i in 0..segments {
            let (t0, t1) = (i as f32 / segments as f32, (i + 1) as f32 / segments as f32);
            beam(&mut b, point(t0, 0.0), point(t1, 0.0), 0.13);
            if detail >= 1 {
                beam(&mut b, point(t0, 0.45), point(t1, 0.45), 0.08);
                if i % 2 == 0 {
                    beam(&mut b, point(t0, 0.0), point(t0, 0.45), 0.05);
                }
            }
        }
    }
    // Las plataformas: marcos cuadrados con baranda.
    for (level, thick) in [(PLATFORM, 0.18), (PLATFORM_2, 0.12)] {
        for k in 0..4 {
            let (a, c) = (leg(k, level), leg((k + 1) % 4, level));
            let out = (a + c) * 0.5;
            let push = Vec3::new(out.x, 0.0, out.z).normalize() * 0.25;
            beam(&mut b, a + push, c + push, thick);
            if detail >= 1 {
                beam(&mut b, a + push * 1.6 + Vec3::Y * 0.6, c + push * 1.6 + Vec3::Y * 0.6, 0.04);
            }
        }
    }
    // El fuste: travesanos en cada piso y diagonales en cruz.
    let bays = [5, 8, 11][detail];
    for i in 0..bays {
        let (y0, y1) = (
            PLATFORM + (SHAFT - PLATFORM) * i as f32 / bays as f32,
            PLATFORM + (SHAFT - PLATFORM) * (i + 1) as f32 / bays as f32,
        );
        for k in 0..4 {
            let next = (k + 1) % 4;
            beam(&mut b, leg(k, y1), leg(next, y1), 0.07);
            if detail >= 1 {
                beam(&mut b, leg(k, y0), leg(next, y1), 0.045);
            }
            if detail >= 2 {
                beam(&mut b, leg(next, y0), leg(k, y1), 0.045);
            }
        }
    }
    // El campanario: losa, cuatro postes, techo en punta y el mastil.
    bx(&mut b, Vec3::new(0.0, SHAFT + 0.08, 0.0), Vec3::new(0.8, 0.08, 0.8));
    for (sx, sz) in corners {
        beam(&mut b, Vec3::new(sx * 0.5, SHAFT + 0.15, sz * 0.5), Vec3::new(sx * 0.5, SHAFT + 1.4, sz * 0.5), 0.05);
    }
    let apex = Vec3::new(0.0, SHAFT + 2.1, 0.0);
    let eave = SHAFT + 1.4;
    for k in 0..4 {
        let (a, c) = (corners[k], corners[(k + 1) % 4]);
        let (pa, pc) = (Vec3::new(a.0 * 0.66, eave, a.1 * 0.66), Vec3::new(c.0 * 0.66, eave, c.1 * 0.66));
        // Que la cara mire hacia afuera.
        if (pc - pa).cross(apex - pa).dot(pa + pc) > 0.0 {
            b.triangle(pa, pc, apex);
        } else {
            b.triangle(pc, pa, apex);
        }
    }
    b.frustum(apex, Vec3::Y, TOWER_TOP - apex.y - 0.15, 0.06, 0.035, 4, false, (false, false));
    b.color(Vec3::new(BELL, 0.0, 0.0));
    let sides = [6, 8, 12][detail];
    b.frustum(Vec3::new(0.0, SHAFT + 0.45, 0.0), Vec3::Y, 0.55, 0.36, 0.16, sides, true, (true, false));
    b.ellipsoid(Vec3::new(0.0, SHAFT + 1.0, 0.0), Vec3::new(0.16, 0.12, 0.16), 3, sides, true);
    b.color(Vec3::new(LAMP, 0.0, 0.0));
    b.ellipsoid(Vec3::new(0.0, TOWER_TOP, 0.0), Vec3::splat(0.16), [3, 4, 5][detail], sides, false);
    b.build()
}

/// Largo, ancho y alto de una camioneta (de la carroceria), en el modelo.
const BUS_HALF: Vec3 = Vec3::new(0.42, 0.4, 1.42);
const BUS_FLOOR: f32 = 0.32;

/// Una camioneta extraurbana: un bus escolar con trompa, defensa y parrilla
/// cromadas, llantas, la parrilla de arriba con bultos y la escalera atras.
/// Mira hacia +Z.
fn bus(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let body_z = -0.2;
    b.color(Vec3::new(PAINT, 0.0, 0.0));
    bx(&mut b, Vec3::new(0.0, BUS_FLOOR + BUS_HALF.y, body_z), BUS_HALF);
    // El techo redondeado: una tapa un poco mas angosta.
    bx(&mut b, Vec3::new(0.0, BUS_FLOOR + 2.0 * BUS_HALF.y + 0.03, body_z), Vec3::new(0.37, 0.04, 1.39));
    // La trompa.
    bx(&mut b, Vec3::new(0.0, 0.55, 1.45), Vec3::new(0.34, 0.22, 0.24));
    b.color(Vec3::new(CHROME, 0.0, 0.0));
    bx(&mut b, Vec3::new(0.0, 0.55, 1.7), Vec3::new(0.24, 0.17, 0.015));
    bx(&mut b, Vec3::new(0.0, 0.32, 1.72), Vec3::new(0.43, 0.05, 0.04));
    bx(&mut b, Vec3::new(0.0, 0.36, -1.65), Vec3::new(0.43, 0.05, 0.04));
    // Llantas y chasis.
    b.color(Vec3::new(RUBBER, 0.0, 0.0));
    bx(&mut b, Vec3::new(0.0, 0.26, body_z), Vec3::new(0.36, 0.07, 1.3));
    let sides = [4, 6, 10][detail];
    for z in [1.2, -0.95] {
        for side in [-1.0f32, 1.0] {
            b.frustum(Vec3::new(side * 0.3, 0.2, z), Vec3::X * side, 0.13, 0.2, 0.2, sides, detail > 0, (false, true));
        }
    }
    // Faros adelante y calaveras atras (el shader las prende).
    b.color(Vec3::new(LAMP, 0.0, 0.0));
    for side in [-1.0f32, 1.0] {
        bx(&mut b, Vec3::new(side * 0.27, 0.66, 1.695), Vec3::new(0.055, 0.045, 0.012));
        bx(&mut b, Vec3::new(side * 0.34, 0.52, -1.625), Vec3::new(0.04, 0.07, 0.012));
    }
    if detail >= 1 {
        // La parrilla del techo y los bultos.
        b.color(Vec3::new(TRIM, 0.0, 0.0));
        let roof = BUS_FLOOR + 2.0 * BUS_HALF.y + 0.07;
        for side in [-1.0f32, 1.0] {
            bx(&mut b, Vec3::new(side * 0.36, roof + 0.06, body_z), Vec3::new(0.012, 0.06, 1.3));
        }
        for z in [-1.45, 1.05] {
            bx(&mut b, Vec3::new(0.0, roof + 0.06, z), Vec3::new(0.36, 0.06, 0.012));
        }
        let pieces = [(-1.1, 0.2, 0.18), (-0.7, -0.12, 0.14), (-0.3, 0.1, 0.22), (0.2, -0.15, 0.16), (0.65, 0.12, 0.12)];
        for (k, &(z, x, h)) in pieces.iter().enumerate() {
            b.color(Vec3::new(CARGO, k as f32 / 5.0, 0.0));
            bx(&mut b, Vec3::new(x, roof + h, z), Vec3::new(0.16, h, 0.17));
        }
    }
    if detail >= 2 {
        // La escalera de atras y los espejos.
        b.color(Vec3::new(TRIM, 0.0, 0.0));
        for side in [-1.0f32, 1.0] {
            bx(&mut b, Vec3::new(side * 0.13, 0.82, -1.64), Vec3::new(0.012, 0.42, 0.012));
            beam(&mut b, Vec3::new(side * 0.42, 0.95, 1.2), Vec3::new(side * 0.55, 0.85, 1.3), 0.01);
            bx(&mut b, Vec3::new(side * 0.56, 0.78, 1.3), Vec3::new(0.012, 0.08, 0.04));
        }
        for k in 0..5 {
            bx(&mut b, Vec3::new(0.0, 0.5 + k as f32 * 0.17, -1.64), Vec3::new(0.13, 0.01, 0.012));
        }
    }
    b.build()
}

/// Un edificio de block: un cuerpo de cuatro pisos y uno de dos pegado a
/// la par (con varillas para el piso que falta), tinaco en la terraza y un
/// rotulo sobre la tienda. Mira hacia +Z; los dos cuerpos van centrados.
fn building(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(Vec3::new(WALL, 0.0, 0.0));
    bx(&mut b, Vec3::new(-1.05, 2.0 * STOREY, 0.0), Vec3::new(1.5, 2.0 * STOREY, 1.4));
    b.color(Vec3::new(WALL, 0.0, 1.0));
    bx(&mut b, Vec3::new(1.5, STOREY, 0.05), Vec3::new(1.05, STOREY, 1.35));
    if detail >= 1 {
        // El tinaco.
        b.color(Vec3::new(RUBBER, 0.0, 0.0));
        b.frustum(Vec3::new(-1.9, 4.0 * STOREY, -0.7), Vec3::Y, 0.45, 0.28, 0.25, [5, 6, 10][detail], true, (false, true));
        // El rotulo de la tienda.
        b.color(Vec3::new(CHROME, 0.0, 0.0));
        bx(&mut b, Vec3::new(1.5, 0.95, 1.43), Vec3::new(0.9, 0.15, 0.03));
    }
    if detail >= 2 {
        // Pretil de la terraza, losas de los balcones y varillas.
        b.color(Vec3::new(WALL, 0.0, 0.0));
        for (c, h) in [
            (Vec3::new(-1.05, 4.0 * STOREY + 0.08, 1.37), Vec3::new(1.5, 0.08, 0.03)),
            (Vec3::new(-1.05, 4.0 * STOREY + 0.08, -1.37), Vec3::new(1.5, 0.08, 0.03)),
            (Vec3::new(-2.52, 4.0 * STOREY + 0.08, 0.0), Vec3::new(0.03, 0.08, 1.4)),
        ] {
            bx(&mut b, c, h);
        }
        for floor in 1..4 {
            bx(&mut b, Vec3::new(-1.05, floor as f32 * STOREY + 0.02, 1.55), Vec3::new(0.6, 0.03, 0.15));
        }
        b.color(Vec3::new(TRIM, 0.0, 0.0));
        for (x, z) in [(0.55, 1.25), (2.45, 1.25), (0.55, -1.15), (2.45, -1.15)] {
            bx(&mut b, Vec3::new(x, 2.0 * STOREY + 0.25, z), Vec3::new(0.012, 0.25, 0.012));
            bx(&mut b, Vec3::new(x + 0.05, 2.0 * STOREY + 0.22, z), Vec3::new(0.012, 0.22, 0.012));
        }
    }
    b.build()
}

/// Una direccion del mundo en los ejes del modelo.
#[inline]
fn to_model(f: &Fragment, v: Vec3) -> Vec3 {
    to_local(f, v)
}

/// Cuanto mide un pixel sobre la superficie, en unidades del modelo.
#[inline]
fn model_footprint(f: &Fragment, u: &Uniforms, scale: f32) -> f32 {
    let ndv = f.normal.dot(f.view_dir()).abs().max(0.25);
    f.pos.length() * u.pixel_angle / (ndv * scale)
}

/// Que tan de noche es donde esta el objeto (por su vertical).
#[inline]
fn prop_night(f: &Fragment, u: &Uniforms) -> f32 {
    let up = f.object_to_world.mul_vec(Vec3::Y);
    smoothstep(0.22, 0.0, up.dot(f.light_dir(u)))
}

pub fn shade_prop(kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    match kind {
        PropKind::Torre => tower_shade(f, u, style, sun),
        PropKind::Camioneta => bus_shade(f, u, style, sun),
        _ => building_shade(f, u, style, sun),
    }
}

/// La torre: acero galvanizado gris, mas oscuro donde la intemperie lo
/// mancha; la campana de bronce y el faro de arriba que destella. De noche,
/// reflectores desde abajo.
fn tower_shade(f: &Fragment, u: &Uniforms, _style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    match material(f) {
        1 => pbr::shade_light(f, u, Vec3::new(0.55, 0.36, 0.16), 0.35, 1.0, f.normal, 1.0, sun),
        2 => {
            let flash = (-((u.time / 1.6).fract() * 1.6) / 0.25).exp();
            Vec3::new(1.0, 0.1, 0.04) * ((0.6 + 14.0 * flash) * u.gains.cementerio)
        }
        _ => {
            let weather = u.noise.fbm(p * Vec3::new(1.5, 0.4, 1.5), 3) * 0.5 + 0.5;
            let paint = Vec3::new(0.36, 0.37, 0.39).lerp(Vec3::new(0.2, 0.21, 0.22), smoothstep(0.45, 0.75, weather));
            let lit = pbr::shade_light(f, u, paint, 0.45, 0.6, f.normal, 1.0, sun);
            let night = prop_night(f, u);
            let flood = (1.0 - p.y / TOWER_TOP).max(0.0).powf(1.5);
            lit + paint * Vec3::new(1.0, 0.8, 0.6) * (night * flood * 3.0 * u.gains.cementerio)
        }
    }
}

/// Colores de las camionetas.
const BUS_PAINTS: [Vec3; 10] = [
    Vec3::new(0.7, 0.02, 0.02),
    Vec3::new(0.02, 0.12, 0.6),
    Vec3::new(0.0, 0.4, 0.12),
    Vec3::new(0.85, 0.55, 0.0),
    Vec3::new(0.4, 0.02, 0.5),
    Vec3::new(0.0, 0.42, 0.5),
    Vec3::new(0.85, 0.2, 0.0),
    Vec3::new(0.75, 0.75, 0.72),
    Vec3::new(0.02, 0.02, 0.025),
    Vec3::new(0.85, 0.05, 0.3),
];

/// Una franja de `y0` a `y1` con bordes de un pixel.
#[inline]
fn band(y: f32, y0: f32, y1: f32, aa: f32) -> f32 {
    smoothstep(y0 - aa, y0 + aa, y) - smoothstep(y1 - aa, y1 + aa, y)
}

/// Camioneta: tres colores al azar, franjas que ondulan por los lados, las
/// ventanas en fila, cromo adelante y el letrero de la ruta sobre el
/// parabrisas.
fn bus_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    let id = (style.seed * 16_777_216.0) as u32;
    let pick = |k: u32| BUS_PAINTS[(hash_u32(id ^ k) % BUS_PAINTS.len() as u32) as usize];
    let fp = model_footprint(f, u, style.scale);
    let nm = to_model(f, f.normal);
    match material(f) {
        4 => pbr::shade_light(f, u, Vec3::new(0.75, 0.75, 0.78), 0.2, 1.0, f.normal, 1.0, sun),
        5 => pbr::shade_light(f, u, Vec3::splat(0.03), 0.8, 0.0, f.normal, 1.0, sun),
        8 => pbr::shade_light(f, u, Vec3::splat(0.05), 0.6, 0.6, f.normal, 1.0, sun),
        6 => {
            let tone = BUS_PAINTS[(f.color.y * 5.0 + id as f32) as usize % BUS_PAINTS.len()];
            let cargo = tone.lerp(Vec3::new(0.3, 0.22, 0.12), 0.5);
            pbr::shade_light(f, u, cargo, 0.9, 0.0, f.normal, 1.0, sun)
        }
        2 => {
            let front = p.z > 0.0;
            let night = prop_night(f, u);
            let glow = if front { Vec3::new(1.0, 0.85, 0.6) * (1.0 + 6.0 * night) } else { Vec3::new(1.0, 0.05, 0.02) * (0.6 + 3.0 * night) };
            glow * u.gains.cementerio
        }
        _ => {
            let (a, b, c) = (pick(1), pick(2), pick(3));
            let side = nm.x.abs() > 0.7;
            let wave = 0.05 * (p.z * 3.0 + unit(id) * TAU).sin();
            let mut paint = a;
            paint = paint.lerp(b, band(p.y, 0.5 + wave, 0.62 + wave, fp));
            paint = paint.lerp(c, band(p.y, 0.64 + wave, 0.68 + wave, fp));
            paint = paint.lerp(Vec3::splat(0.03), band(p.y, 0.0, 0.42, fp));
            let mut rough = 0.35;
            let mut metal = 0.0;
            // Las ventanas: una fila con parales.
            let glass = if side && p.z < 1.2 {
                let pane = band(((p.z + 2.0) / 0.32).fract(), 0.12, 0.95, fp / 0.32);
                band(p.y, 0.74, 0.99, fp) * pane
            } else if nm.z > 0.7 && p.z > 1.15 && p.z < 1.3 {
                band(p.y, 0.74, 1.02, fp) * band(p.x.abs(), 0.0, 0.38, fp)
            } else if nm.z < -0.7 {
                band(p.y, 0.74, 1.0, fp) * band(p.x.abs(), 0.04, 0.36, fp)
            } else {
                0.0
            };
            // El techo blanco y el letrero de la ruta adelante.
            if nm.y > 0.7 && p.y > 1.0 {
                paint = Vec3::new(0.7, 0.7, 0.68);
            }
            let sign = if nm.z > 0.7 && p.z > 1.15 { band(p.y, 1.03, 1.1, fp) * band(p.x.abs(), 0.0, 0.3, fp) } else { 0.0 };
            if glass > 0.0 {
                paint = paint.lerp(Vec3::splat(0.02), glass);
                rough += (0.05 - rough) * glass;
                metal += 0.3 * glass;
            }
            let chrome_trim = band(p.y, 0.475, 0.49, fp);
            paint = paint.lerp(Vec3::splat(0.7), chrome_trim);
            let lit = pbr::shade_light(f, u, paint, rough, metal, f.normal, 1.0, sun);
            lit + Vec3::new(1.0, 0.9, 0.6) * (sign * (0.3 + 2.0 * prop_night(f, u)) * u.gains.cementerio)
        }
    }
}

/// Block de concreto sin repellar.
const BLOCK: Vec3 = Vec3::new(0.3, 0.29, 0.27);
/// Vidrio de las ventanas.
const GLASS: Vec3 = Vec3::new(0.025, 0.03, 0.035);

/// Edificio: cada cuerpo es de un dueno distinto. Pintado hasta un piso y
/// block sin repellar arriba; las paredes de los lados son ciegas (pegan con
/// el vecino) y a veces llevan un anuncio pintado. Ventanas de un estilo
/// por edificio (separacion, ancho, alto y marco), con barrotes abajo; en
/// la tienda, cortina de metal o el mostrador abierto. Chorreones bajo las
/// ventanas y tierra al pie. De noche, unas ventanas encendidas.
fn building_shade(f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    let annex = f.color.z > 0.5;
    let id = hash_u32((style.seed * 16_777_216.0) as u32 ^ annex as u32 * 0x51ED);
    let pick = |k: u32| unit(hash_u32(id ^ k.wrapping_mul(0x2C1B_3C6D)));
    let fp = model_footprint(f, u, style.scale);
    let nm = to_model(f, f.normal);
    let night = prop_night(f, u);
    let grime = u.noise.fbm(p * 1.3 + Vec3::splat(style.seed * 30.0), 3) * 0.5 + 0.5;
    match material(f) {
        // El tinaco, de plastico negro.
        5 => return pbr::shade_light(f, u, Vec3::splat(0.025), 0.45, 0.0, f.normal, 1.0, sun),
        // Las varillas, oxidadas.
        8 => return pbr::shade_light(f, u, Vec3::new(0.12, 0.05, 0.03), 0.7, 0.4, f.normal, 1.0, sun),
        // El rotulo de la tienda: un color con una franja; de noche alumbra.
        4 => {
            let sign = PAINTS[(hash_u32(id ^ 0x5E) % PAINTS.len() as u32) as usize] * 1.2;
            let stripe = band((p.y - 0.8) / 0.3, 0.4, 0.6, fp / 0.3);
            let face = sign.lerp(Vec3::splat(0.75), stripe);
            return pbr::shade_light(f, u, face, 0.4, 0.0, f.normal, 1.0, sun) + face * (night * 0.6 * u.gains.cementerio);
        }
        _ => {}
    }
    if nm.y > 0.5 {
        // La terraza: losa con manchas.
        let albedo = CONCRETE * (0.6 + 0.35 * grime);
        return pbr::shade_light(f, u, albedo, 0.9, 0.0, f.normal, 1.0, sun);
    }
    let storey = p.y / STOREY;
    let level = storey.floor();
    // Hasta que piso esta pintado.
    let painted = if pick(1) < 0.25 { 0.0 } else { 1.0 + (pick(2) * if annex { 2.0 } else { 4.0 }).floor() };
    let paint = PAINTS[(hash_u32(id ^ 0x9A) % PAINTS.len() as u32) as usize];
    // Las juntas del block (cuatrapeado), solo de cerca.
    let row = p.y / 0.065;
    let course = ((p.x + p.z) / 0.13 + 0.5 * (row.floor() % 2.0)).fract();
    let joint = (row.fract().min(1.0 - row.fract()) * 0.065).min(course.min(1.0 - course) * 0.13);
    let mortar = line(joint, 0.006, fp) * smoothstep(0.03, 0.01, fp);
    let wall = if level < painted { paint * (0.85 + 0.2 * grime) } else { BLOCK * (0.8 + 0.3 * grime) * (1.0 - 0.25 * mortar) };
    let mut albedo = wall;
    let mut rough = 0.9;
    let mut glow = Vec3::ZERO;
    let blind = nm.x.abs() > 0.7;
    if blind {
        // Pared ciega: a veces un anuncio pintado, de un color con letras
        // que de lejos son una franja.
        if pick(3) < 0.4 {
            let (ax, ay) = (p.z / 2.4 + 0.5, (p.y - STOREY * 0.6) / (STOREY * 1.6));
            let ad = band(ax, 0.1, 0.9, fp / 2.4) * band(ay, 0.0, 1.0, fp / 1.4);
            let color = BUS_PAINTS[(hash_u32(id ^ 0xAD) % BUS_PAINTS.len() as u32) as usize];
            let letters = band(ay, 0.35, 0.65, fp / 1.4) * (0.6 + 0.4 * ((p.z * 9.0).sin() * smoothstep(0.03, 0.01, fp)));
            albedo = albedo.lerp(color.lerp(Vec3::splat(0.8), letters), ad);
        }
    } else {
        // Ventanas: el estilo es del edificio; centradas en cada cuerpo.
        let back = nm.z < -0.5;
        let spacing = [0.62, 0.78, 0.95][(pick(4) * 3.0) as usize] * if back { 1.4 } else { 1.0 };
        let width = (0.4 + 0.25 * pick(5)) * if back { 0.7 } else { 1.0 };
        let height = 0.36 + 0.2 * pick(6);
        let center = if annex { 1.5 } else { -1.05 };
        let across = (p.x - center) / spacing + 0.5;
        let column = across.floor();
        let cx = across - column;
        let cy = storey - level;
        let (x0, x1, y0, y1) = (0.5 - width * 0.5, 0.5 + width * 0.5, 0.3, 0.3 + height);
        let (ax, ay) = (fp / spacing, fp / STOREY);
        let window = band(cx, x0, x1, ax) * band(cy, y0, y1, ay);
        let frame = (band(cx, x0 - 0.05, x1 + 0.05, ax) * band(cy, y0 - 0.04, y1 + 0.03, ay) - window).max(0.0);
        let sill = band(cx, x0 - 0.07, x1 + 0.07, ax) * band(cy, y0 - 0.08, y0 - 0.03, ay);
        // Chorreones debajo de cada ventana.
        let streak = band(cx, x0, x1, ax) * smoothstep(y0, 0.0, cy) * smoothstep(0.35, 0.75, u.noise.noise(Vec3::new(p.x * 14.0, p.y * 1.5, p.z * 14.0)) * 0.5 + 0.5);
        albedo = albedo * (1.0 - 0.25 * streak);
        let shop = level < 1.0 && nm.z > 0.5;
        if shop && annex {
            // La tienda: cortina de metal bajada, o abierta con su mostrador.
            let door = band((p.x - 1.5) / 1.8 + 0.5, 0.1, 0.9, fp / 1.8) * band(cy, 0.0, 0.78, ay);
            let open = pick(7) < 0.55;
            let inside = if open {
                let shelves = smoothstep(0.4, 0.6, (cy * 6.0).fract()) * (0.5 + 0.5 * (p.x * 17.0).sin().abs());
                Vec3::new(0.05, 0.04, 0.03).lerp(PAINTS[(hash_u32(id ^ (p.x * 6.0) as i32 as u32) % PAINTS.len() as u32) as usize] * 0.5, shelves * smoothstep(0.04, 0.015, fp))
            } else {
                Vec3::new(0.22, 0.23, 0.24) * (0.85 + 0.15 * (p.y * 70.0).sin() * smoothstep(0.03, 0.01, fp))
            };
            albedo = albedo.lerp(inside, door);
            rough += (0.6 - rough) * door;
            if open {
                glow = Vec3::new(1.0, 0.8, 0.55) * (door * 0.5 * night);
            }
        } else if shop {
            // La puerta de la casa y su ventana con barrotes.
            let door = band(cx, 0.3, 0.7, ax) * band(cy, 0.0, 0.8, ay) * (column == 0.0) as u8 as f32;
            albedo = albedo.lerp(Vec3::new(0.08, 0.05, 0.035), door);
            albedo = albedo.lerp(GLASS, window * (1.0 - door)).lerp(Vec3::splat(0.04), frame);
        } else {
            let wid = hash_u32(id ^ (column as i32 as u32).wrapping_mul(0x9E37_79B9) ^ (level as u32) << 8 ^ back as u32);
            let curtain = PAINTS[(wid >> 4) as usize % PAINTS.len()];
            let lit = unit(hash_u32(wid ^ ((u.time / (25.0 + 30.0 * unit(wid))).floor() as u32))) < 0.45;
            let bars = if level < 2.0 { line(((cx - x0) / width * 5.0).fract().min(1.0 - ((cx - x0) / width * 5.0).fract()), 0.08, ax * 5.0 / width) } else { 0.0 };
            let trim = if pick(8) < 0.5 { Vec3::splat(0.55) } else { Vec3::splat(0.04) };
            albedo = albedo.lerp(GLASS, window).lerp(trim, frame).lerp(paint.lerp(Vec3::splat(0.6), 0.5), sill * 0.8).lerp(Vec3::splat(0.03), bars * window);
            rough += (0.12 - rough) * window;
            if lit {
                let tone = if unit(wid >> 9) < 0.25 { NEON } else { WINDOW.lerp(curtain, 0.35) };
                glow = tone * (window * (1.0 - bars) * night * 1.4);
            }
        }
    }
    // Tierra al pie de la pared.
    albedo = albedo * (0.7 + 0.3 * smoothstep(0.0, 0.3, p.y));
    pbr::shade_light(f, u, albedo, rough, 0.0, f.normal, 1.0, sun) + glow * u.gains.cementerio
}

// El anillo: camionetas y tuc-tucs --------------------------------------------

/// Lado de las celdas a lo ancho del anillo y a lo largo (en el radio de
/// adentro), en unidades del mundo.
const LANE: f32 = 5.0;
const SLOT: f32 = 14.0;
/// Cobertura media donde mas trafico hay.
const TRAFFIC: f32 = 0.42;
/// Color promedio del trafico visto de lejos.
const JAM: Vec3 = Vec3::new(0.42, 0.3, 0.22);

/// Donde hay trafico a lo ancho del anillo (0 a 1): carriles con lineas
/// vacias entre ellos y tramos mas llenos.
fn traffic(s: f32, noise: &Perlin) -> f32 {
    if !(0.0..=1.0).contains(&s) {
        return 0.0;
    }
    let edges = smoothstep(0.0, 0.05, s) * smoothstep(1.0, 0.93, s);
    let lane = (s * 7.0).fract();
    let gap = smoothstep(0.0, 0.1, lane) * smoothstep(1.0, 0.9, lane);
    let busy = noise.noise(Vec3::new(s * 6.0, 2.5, 0.5)) * 0.5 + 0.5;
    edges * gap * (0.4 + 0.6 * busy)
}

/// Densidad del anillo (0 a ~0.9) a la fraccion `s` de su ancho (0 adentro, 1 afuera).
pub fn ring_density(s: f32, noise: &Perlin) -> f32 {
    traffic(s, noise) * TRAFFIC
}

/// Una camioneta o un tuc-tuc en su sistema (`x` a lo largo, adelante
/// positivo; `y` de lado), en unidades del mundo: cuanto tapa, su color y
/// sus luces.
fn vehicle(x: f32, y: f32, id: u32, aa: f32, time: f32) -> (f32, Vec3, Vec3) {
    let pick = |k: u32| BUS_PAINTS[(hash_u32(id ^ k) % BUS_PAINTS.len() as u32) as usize];
    let tuk = unit(hash_u32(id ^ 0x70C)) < 0.3;
    let (half_l, half_w) = if tuk { (1.8, 1.1) } else { (5.5, 1.5) };
    let r = if tuk { 0.9 } else { 0.4 };
    let qx = x.abs() - half_l + r;
    let qy = y.abs() - half_w + r;
    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
    let cover = smoothstep(aa, -aa, d);
    if cover <= 0.0 {
        return (0.0, JAM, Vec3::ZERO);
    }
    let body = pick(1);
    let mut color = body;
    if !tuk {
        // Techo con franjas, la trompa adelante y bultos en la parrilla.
        color = color.lerp(pick(2), band(y.abs(), 0.7, 1.0, aa));
        color = color.lerp(Vec3::splat(0.75), smoothstep(half_l - 1.4, half_l - 1.4 + aa, x) * band(y.abs(), 0.0, 1.2, aa) * 0.5);
        let bundle = hash_u32(id ^ ((x + 6.0) / 1.3).floor() as i32 as u32);
        let cargo = band(y.abs(), 0.0, 0.8, aa) * band(x, -4.6, 3.0, aa) * (unit(bundle) < 0.5) as u8 as f32;
        color = color.lerp(pick(bundle).lerp(Vec3::new(0.3, 0.22, 0.12), 0.4), cargo);
    } else {
        color = Vec3::new(0.05, 0.05, 0.05).lerp(body, smoothstep(0.2, -0.2, x));
    }
    // Faros adelante, calaveras atras; algunos frenan.
    let lamp = 0.35;
    let lights_y = half_w - 0.45;
    let head = spot_light((x - half_l + 0.2).hypot(y.abs() - lights_y), lamp, aa);
    let tail = spot_light((x + half_l - 0.2).hypot(y.abs() - lights_y), lamp, aa);
    let brake = 0.4 + 0.6 * smoothstep(0.3, 0.6, (time * 0.7 + unit(id) * 9.0).sin());
    let lights = Vec3::new(1.0, 0.85, 0.6) * (head * 14.0) + Vec3::new(1.0, 0.04, 0.02) * (tail * 10.0 * brake);
    (cover, color, lights)
}

/// Lo que se ve del anillo en `local` (el punto en su plano, relativo al planeta): color, opacidad,
/// luz propia y cuanto dispersa hacia adelante.
pub fn ring(ring: &RingInfo, local: Vec3, alpha: f32, s: f32, footprint: f32, u: &Uniforms) -> (Vec3, f32, Vec3, f32) {
    let far = (JAM, alpha * 1.2, Vec3::ZERO, 1.2);
    let resolved = smoothstep(0.35, 0.12, footprint / SLOT);
    if resolved <= 0.0 {
        return far;
    }
    let (a, b) = (local.dot(ring.axis_a), local.dot(ring.axis_b));
    let r = a.hypot(b);
    // Cada carril gira en bloque, a su velocidad.
    let lane = (r / LANE).floor();
    let lane_id = hash_u32(lane as i32 as u32 ^ 0x1A7E);
    let middle = 0.5 * (ring.inner + ring.outer);
    let spin = u.time * 420.0 / middle.powf(1.5) * (0.6 + 0.8 * unit(lane_id));
    let per_turn = (TAU * ring.inner / SLOT).round();
    let y = ((b.atan2(a) + spin) / TAU).rem_euclid(1.0) * per_turn + unit(lane_id >> 4);
    let slot = y.floor();
    let arc = TAU * r / per_turn;
    let id = hash_u32(lane_id ^ (slot as u32 % per_turn as u32).wrapping_mul(0x9E37_79B9));
    if unit(id) > traffic(s, u.noise) * 1.6 {
        return (far.0, alpha + (0.0 - alpha) * resolved, Vec3::ZERO, 1.2);
    }
    // Un poco corrida y girada dentro de su celda; el frente va hacia donde
    // avanza (el angulo baja con el tiempo).
    let ox = (y - slot - 0.5) * arc;
    let oy = (r / LANE - lane - 0.5) * LANE + (unit(id >> 3) - 0.5) * 0.6;
    let (sn, cs) = ((unit(id >> 6) - 0.5) * 0.12).sin_cos();
    let (vx, vy) = (-(ox * cs - oy * sn), ox * sn + oy * cs);
    let (cover, color, lights) = vehicle(vx, vy, id, footprint, u.time);
    (
        JAM.lerp(color, resolved),
        alpha + (cover - alpha) * resolved,
        lights * (resolved * u.gains.cementerio),
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

    /// El valle se queda dentro de su rango (el culling por horizonte
    /// depende de eso) y la misma semilla da el mismo valle.
    #[test]
    fn the_valley_stays_in_range_and_is_deterministic() {
        let (a, b) = (Valle::new(7), Valle::new(7));
        let (lo, hi) = a.height_range();
        let near = (0..600).map(|i| {
            let t = i as f32 * 0.61;
            (UP + Vec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * (0.002 * i as f32)).normalize()
        });
        for dir in directions().chain(near).chain(std::iter::once(a.agua)) {
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
            assert_eq!(s.attrs, b.sample(dir).attrs);
        }
    }

    /// El lugar mira hacia donde dice y sus ejes son los del shader.
    #[test]
    fn the_site_frame_is_orthonormal() {
        assert!((UP.length() - 1.0).abs() < 1e-5);
        assert!(UP.dot(FORWARD).abs() < 1e-5 && UP.dot(RIGHT).abs() < 1e-5);
        assert!((FORWARD.cross(UP) - RIGHT).length() < 1e-5);
    }

    /// La torre esta en el cruce, los edificios en las cuadras (no en la
    /// calle) y las camionetas en la calle, todo parado en el suelo.
    #[test]
    fn buildings_stand_on_lots_and_buses_on_streets() {
        let field = Valle::new(11);
        let (_, normal, forward, place) = site((DVec3::Y, DVec3::Z)).unwrap();
        let frame = (normal, forward, forward.cross(normal));
        let mut instances = Vec::new();
        place(&mut instances, RADIUS as f64, &field, 3, frame);
        let grid = zone(UP * RADIUS).grid();
        let towers = instances.iter().filter(|p| p.kind == PropKind::Torre).count();
        assert_eq!(towers, 1);
        let (mut buildings, mut buses) = (0, 0);
        for i in &instances {
            let dir = i.position.normalize();
            let ground = RADIUS as f64 + field.sample(dir.as_vec3()).height as f64;
            assert!((i.position.length() - ground).abs() < 0.2);
            assert!(i.rotation.rotate(DVec3::Y).dot(dir) > 0.999);
            let p = (dir * ground).as_vec3();
            let spot = grid.at(p.dot(RIGHT), p.dot(FORWARD));
            match i.kind {
                PropKind::Edificio => {
                    buildings += 1;
                    // Mas alla de la banqueta (que llega a 0.35).
                    assert!(spot.curb > 0.35, "edificio en la calle: {}", spot.curb);
                }
                PropKind::Camioneta if spot.curb < 0.0 => buses += 1,
                _ => {}
            }
        }
        assert!(buildings > 100 && buses > 10, "{buildings} {buses}");
    }

    #[test]
    fn traffic_stays_inside_the_ring() {
        let noise = Perlin::new(5);
        assert_eq!(ring_density(-0.01, &noise), 0.0);
        assert_eq!(ring_density(1.01, &noise), 0.0);
        let most = (0..100).map(|i| ring_density(i as f32 / 100.0, &noise)).fold(0.0, f32::max);
        assert!(most > 0.1 && most <= TRAFFIC, "{most}");
    }
}
