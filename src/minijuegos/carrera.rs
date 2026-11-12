//! La carrera del Periferico: el minijuego de la capital (version chapina).
//! Cuatro naves, una de cada clase (la chela, el tuk-tuk, la camioneta y el
//! colectivo): la que se maneja es la que se trae y las otras tres son las
//! rivales. Tres vueltas a una carretera que flota en orbita sobre la
//! capital de noche, con trafico, tumulos, rebufo, pito y el humo negro de
//! la camioneta.
//!
//! - **Pseudo-3D** (como los juegos de carreras de antes): la pista es una
//!   lista de tramos de 4 m, cada uno con su curva (cuanto gira el rumbo) y
//!   su altura. Desde la camara se recorren hacia adelante sumando el rumbo
//!   (`theta += curva`) y el corrimiento lateral (`x += tramo * theta`), y
//!   cada borde se proyecta con `pantalla = centro + x * f / z`. Se pintan
//!   de cerca a lejos guardando la fila mas alta ya cubierta: lo que queda
//!   abajo de ella esta detras de una loma. Cada fila guarda que punto de la
//!   pista muestra (la distancia interpolada en `1 / z`, correcta en
//!   perspectiva), y con eso se pinta por pixel en paralelo: asfalto,
//!   rayas, tumulos, la meta y la luz de los postes.
//! - **Asfalto mojado**: cada fila sabe cuantos metros de pista abarca
//!   (mas lo que se corre en el frame) y todo lo pintado a lo largo (rayas,
//!   juntas cian, tachas, tumulos, el bordillo celeste y blanco) se filtra
//!   en caja sobre eso: de lejos se promedia en vez de saltar de fila en
//!   fila, y con velocidad se estira (las tachas se vuelven trazadoras).
//!   Lo que cambia en metros a lo ancho (la luz de los postes con su cono,
//!   el charco de luz de cada nave con su sombra de contacto y sus
//!   calaveras, lo mojado, el neon) va en una tabla por fila que cada
//!   pixel interpola. Lo mojado refleja tres cosas: la estela de todo lo
//!   que emitio en el frame anterior (cortinas de color que bajan hacia la
//!   camara), una raya ambar bajo cada poste y el cielo con Fresnel.
//! - **Las naves son sprites horneados** al abrir el juego (`hornear`) con
//!   los modelos de Blender: 15 angulos para verlas desde cualquier lado y 9
//!   de cerca para la propia (cada 2.5 grados, mezclados: gira sin saltos).
//!   Se eligen por el angulo entre el rumbo de la nave y el rayo de la
//!   camara, con el mip que toca a su tamano (de lejos no centellean) y, de
//!   cerca, con un barrido de movimiento. Sus LEDs van aparte y se animan
//!   por clase (las ambar corren, las magentas respiran, los stops se
//!   encienden al frenar); los postes y el neon de las barandas las
//!   alumbran; se ladean con un resorte en las curvas (el tuk-tuk sobre la
//!   rueda de afuera) y rebotan en los tumulos. La camioneta echa humo negro
//!   por sus chimeneas, cada clase tiene su turbo y los roces, choques y
//!   tumulos sacan chispas.
//! - **El mundo en metros**: la pista tambien guarda su trazo en el plano
//!   (x al este, z al norte) y la camara sabe donde esta y a que altura
//!   (400 m sobre la ciudad). Cada columna de la pantalla es un rayo
//!   pinhole exacto: su rumbo es `rumbo + atan(xoff / f)` y su elevacion
//!   `atan((horizonte - y) / hypot(f, xoff))`, asi el limbo y el cielo de
//!   las orillas se curvan como en una camara de verdad.
//! - **La capital abajo** (`ciudad`): cada pixel bajo el limbo es un rayo
//!   contra una esfera de radio 160 km. Donde pega se lee la ciudad
//!   horneada con el mip que corresponde al tamano del pixel en el suelo, y
//!   se funde en la neblina del color del horizonte. Corre con paralaje
//!   real al avanzar y al doblar.
//! - **El cielo** es el de la version chapina: el cubemap se pasa una vez a
//!   un panorama (rumbo y altura), regradeado a plateado-azul, y se mezcla
//!   en modo pantalla con un gradiente horneado sobre el limbo: el domo de
//!   sodio de la ciudad, el airglow verde pegado al limbo y el indigo de
//!   arriba. Enfrente de la recta de meta estan el Agua, el Acatenango, el
//!   Fuego (en erupcion: penacho, rios de lava y bombas, y da la salida) y
//!   el Pacaya, como siluetas por columna. Hay estrellas fugaces.
//! - **La luz** (`posproceso`): cada pixel guarda en su alfa cuanto emite
//!   (la baranda de neon, las luces fuertes de la ciudad, la lava y las
//!   bombas, la estrella fugaz, los postes, el semaforo y los focos de las
//!   naves, que el horneado marca por la clase LED de cada pieza). Despues
//!   de las filas, lo que emite se junta (con su tono) en una piramide de
//!   bloom y se suma, y la imagen se gradua: vineta, sombras indigo, altas
//!   al sodio, curva S y grano. Con la velocidad la camara se abre (un
//!   resorte en la distancia focal) y salen lineas hacia la orilla; con el
//!   turbo, mas lineas y aberracion cromatica. El mapa va encima de todo.
//! - **La escenografia chapina**: la meta pasa bajo la Torre del Reformador
//!   (celosia de hierro con sus focos LED, que es tambien el semaforo: se
//!   llena de rojo por tercios y se pone verde), tuneles de arcos de
//!   foquitos de feria que tinen el asfalto, barriletes gigantes de Sumpango
//!   a contraluz (horneados en `escenario`, con flecos y colas en vivo) y
//!   cohetes que suben de la capital y revientan en el cielo. Cada cosa
//!   encima de la pista sabe de que fila a que fila cae y las demas filas
//!   ni la miran.
//! - **La fisica** es de arcade a 120 pasos por segundo: aceleracion que se
//!   apaga cerca de la velocidad maxima, la curva empuja hacia afuera con
//!   `v^2 kappa`, el volante con el manejo de cada nave, choques entre
//!   rectangulos con masa, y fuera del asfalto se frena.
//! - **La IA** busca el lado de adentro de las curvas, frena lo justo para
//!   no pasar de `sqrt(agarre / kappa)`, esquiva el trafico y los tumulos, y
//!   gasta el turbo en las rectas. Un poco de liga: se apura si va lejos
//!   detras de quien juega y afloja si va muy adelante.

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

use super::ciudad::{self, Ciudad, unit};
use super::escenario::{self, KITE_EXTENT, PAPEL};
use super::hornear::{self, Finish, Lut, Pick, Sprite, Texel};
use super::lienzo::{self, cover, emission, halo, over, write_emit};
use super::malla::{self, Hit, Indexed};
use super::posproceso::{self, Posproceso};
use super::{Align, Font, Keys, Minigame, Overlay, Paint, Records};
use crate::audio::SAMPLE_RATE;
use crate::math::{Vec3, fast_atan2, fast_exp, hash_u32, smoothstep};
use crate::parallel;
use crate::render::transition;
use crate::scene::{Mesh, Surface};

/// Largo de un tramo, ancho de un carril y medio ancho del asfalto (m).
const SEGMENT: f32 = 4.0;
const LANE: f32 = 4.0;
const HALF_ROAD: f32 = 8.0;
/// Donde van las barandas de luz, a cada lado.
const RAIL: f32 = 9.4;
const LAPS: u32 = 3;
/// Tramos que se dibujan hacia adelante (~920 m).
const DRAW: usize = 230;
/// Distancia focal en altos de pantalla y la fila del horizonte.
const FOCAL: f32 = 0.95;
const HORIZON: f32 = 0.42;
/// Cuanto empuja la curva hacia afuera y el agarre que usa la IA.
const CENTRIFUGAL: f32 = 0.2;
/// Lo que la IA cree que aguanta una nave en curva, por su manejo: la
/// velocidad maxima en una curva es `sqrt(GRIP * manejo / kappa)`.
const GRIP: f32 = 2.8;
/// Paso de la fisica.
const STEP: f32 = 1.0 / 120.0;
/// Un poste de luz cada tanto, de un lado y del otro, contados en cada
/// vuelta (ver `lamp`): ninguno a menos de `LAMP_CLEAR` m de la meta, donde
/// estan las patas de la torre.
const LAMP_EVERY: f32 = 48.0;
const LAMP_CLEAR: f32 = 12.0;
/// Los arcos de foquitos: uno cada 16 m contados en cada vuelta (corridos
/// 8 m para no caer encima de un poste: la vuelta no mide un multiplo de 16)
/// en estos tramos, `[desde, hasta)` en metros de la vuelta: antes de la
/// meta, la primera loma, la recta de los primeros tumulos y la cresta de
/// la segunda loma. A menos de 24 m de la meta no hay: sus focos de colores
/// se revolverian con el semaforo de la torre.
const ARCH_EVERY: f32 = 16.0;
const ARCH_OFFSET: f32 = 8.0;
const ARCH_ZONES: [(f32, f32); 5] = [(2296.0, 2352.0), (24.0, 120.0), (320.0, 488.0), (632.0, 736.0), (1040.0, 1184.0)];
/// Los colores de los foquitos de feria: rosa, celeste, amarillo, verde y
/// rojo.
const ARCH_PAL: [Vec3; 5] = [Vec3::new(1.0, 0.2, 0.6), Vec3::new(0.15, 0.85, 1.0), Vec3::new(1.0, 0.8, 0.2), Vec3::new(0.2, 1.0, 0.4), Vec3::new(1.0, 0.15, 0.1)];
/// La semielipse de cada arco (m): medio ancho, alto y cuantos focos.
const ARCH_A: f32 = RAIL + 0.3;
const ARCH_B: f32 = 10.5;
const ARCH_BULBS: usize = 41;
/// Los barriletes: uno de 12 m cada 200 m (alternando lado) y el festival,
/// uno cada 48 m alternando lado antes de la meta.
const KITE_EVERY: f32 = 200.0;
const FESTIVAL: (f32, f32) = (1752.0, 2296.0);
const FESTIVAL_EVERY: f32 = 48.0;
/// Lo alto de la Torre del Reformador (hasta la punta del faro).
const TOWER_TOP: f32 = 66.0;
/// Hasta cuantos cohetes en el aire, cuanto vive cada uno despues de
/// reventar (s), cuanto tarda en subir y cuantas chispas tiene.
const MAX_BURSTS: usize = 8;
const BURST_LIFE: f32 = 3.5;
const RISE: f32 = 0.9;
const SPARKS: u32 = 70;
const MAX_SPARK_SEGS: usize = 600;
/// Rumbo de los angulos horneados: de -70 a 70 grados, cada 10.
const YAWS: usize = 15;
const YAW_STEP: f32 = 10.0 * PI / 180.0;
/// Los de la nave propia: de -19.5 a 19.5, cada 1.5 (se toma el mas
/// cercano; mezclar dos vecinos la dejaba doble, ver `view`). Le alcanza
/// para verse de lado en las curvas cerradas.
const PLAYER_YAWS: usize = 27;
const PLAYER_YAW_STEP: f32 = 1.5 * PI / 180.0;
/// Las chispas de la pista: cuantas a la vez como mucho.
const MAX_TRACK_SPARKS: usize = 600;
/// El humo de las chimeneas: cuantos soplos a la vez y cuanto vive cada uno.
const MAX_PUFFS: usize = 80;
/// Cuantas naves de trafico (una de cada tres, lata si hay).
const TRAFFIC: u32 = 18;
const PUFF_LIFE: f32 = 2.2;
const RATE: f32 = SAMPLE_RATE as f32;

/// Las clases de nave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Chela,
    TukTuk,
    Camioneta,
    Colectivo,
}

/// Como es cada clase en la carrera: medidas (m) y manejo.
#[derive(Clone, Copy, Debug)]
struct Spec {
    length: f32,
    width: f32,
    height: f32,
    /// Velocidad maxima (m/s), aceleracion (m/s^2), cuanto se mueve de lado
    /// a fondo (m/s) y su masa en los choques.
    top: f32,
    accel: f32,
    handling: f32,
    mass: f32,
    /// Cuanto flota sobre el asfalto (la chela vuela).
    hover: f32,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Chela, Kind::TukTuk, Kind::Camioneta, Kind::Colectivo];

    /// La clase de la nave chapina `index` (0 a 5 botellas, 6 tuk-tuk, 7
    /// camioneta, 8 colectivo).
    pub fn of_ship(index: usize) -> Self {
        match index {
            0..=5 => Kind::Chela,
            6 => Kind::TukTuk,
            7 => Kind::Camioneta,
            _ => Kind::Colectivo,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    /// Cuanto mide en la carrera (m).
    pub fn length(self) -> f32 {
        self.spec().length
    }

    fn spec(self) -> Spec {
        match self {
            Kind::Chela => Spec { length: 4.2, width: 1.5, height: 1.5, top: 63.0, accel: 12.5, handling: 10.0, mass: 1.0, hover: 0.6 },
            Kind::TukTuk => Spec { length: 3.35, width: 1.76, height: 2.4, top: 55.0, accel: 17.0, handling: 12.5, mass: 0.6, hover: 0.0 },
            Kind::Camioneta => Spec { length: 9.6, width: 2.9, height: 3.9, top: 76.0, accel: 10.5, handling: 8.6, mass: 2.6, hover: 0.0 },
            Kind::Colectivo => Spec { length: 4.64, width: 2.0, height: 1.84, top: 66.0, accel: 12.0, handling: 10.0, mass: 1.2, hover: 0.0 },
        }
    }

    /// Lo que se dice de cada una en la largada.
    fn blurb(self) -> &'static str {
        match self {
            Kind::Chela => "pareja en todo; flota y no le hacen los túmulos",
            Kind::TukTuk => "arranca como bala y dobla de lado, pero corre poco",
            Kind::Camioneta => "la más rápida en recta, lenta para arrancar y para doblar; echa humo negro",
            Kind::Colectivo => "rápido y ágil: el más parejo de los carros",
        }
    }

    fn record(self) -> &'static str {
        match self {
            Kind::Chela => "carrera_chela",
            Kind::TukTuk => "carrera_tuk_tuk",
            Kind::Camioneta => "carrera_camioneta",
            Kind::Colectivo => "carrera_colectivo",
        }
    }

    /// Como se ladea en las curvas: cuanto (rad a fondo y a toda
    /// velocidad) y el resorte, `(K, C)` de `roll'' = K (meta - roll) - C
    /// roll'`. Apenas: ladear el sprite entero se veia como si la nave se
    /// fuera de lado (sobre todo la camioneta); el giro se ve con el rumbo.
    /// El tuk-tuk es alto y angosto: se ladea mas y rapido, con
    /// `zeta = C / (2 sqrt K) ~ 0.47` (se pasa y rebota); la camioneta, lenta
    /// y blanda (`omega = 5`, `zeta = 0.3`: se mece); el resto, firme.
    fn roll_spring(self) -> (f32, f32, f32) {
        match self {
            Kind::TukTuk => (0.03, 90.0, 9.0),
            Kind::Camioneta => (0.0, 25.0, 3.0),
            Kind::Colectivo => (0.0, 60.0, 7.0),
            Kind::Chela => (0.0, 60.0, 7.0),
        }
    }

    /// El color de su punto en el mapa.
    fn color(self) -> Vec3 {
        match self {
            Kind::Chela => Vec3::new(1.0, 0.75, 0.2),
            Kind::TukTuk => Vec3::new(1.0, 0.25, 0.55),
            Kind::Camioneta => Vec3::new(0.3, 0.55, 1.0),
            Kind::Colectivo => Vec3::new(0.75, 0.2, 0.3),
        }
    }
}

/// El modelo de una clase: su malla, como se sombrea, cuanto mide una
/// unidad del modelo en metros y su nombre.
pub struct Model<'a> {
    pub mesh: &'a Mesh,
    /// La malla para verla en 3D de cerca (la baja de los vehiculos: a esa
    /// distancia no se distingue de la media y cuesta la mitad).
    pub near: &'a Mesh,
    pub finish: Finish,
    pub meters: f32,
    pub name: &'static str,
}

// La pista ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
struct Segment {
    /// Cuanto gira el rumbo en este tramo (radianes, + a la derecha).
    curve: f32,
    /// Altura al empezar el tramo (m).
    y: f32,
    /// Carriles con tumulo (bit 0 el de la izquierda).
    bumps: u8,
}

struct Track {
    segments: Vec<Segment>,
    /// El rumbo al empezar cada tramo (sin contar vueltas).
    headings: Vec<f32>,
    /// El dibujo de la pista para el mapa, de 0 a 1.
    map: Vec<(f32, f32)>,
    /// El trazo en metros al empezar cada tramo (y al final de la vuelta):
    /// x al este y z al norte (el rumbo 0 va hacia +z y el positivo dobla
    /// hacia +x). Es el rumbo integrado sin corregir: la vuelta no cierra
    /// en posicion, se corre `drift` metros, y cada vuelta pasa sobre otra
    /// parte de la capital (que se repite). Asi la ciudad corre debajo a la
    /// velocidad y en la direccion de verdad.
    world: Vec<(f32, f32)>,
    drift: (f32, f32),
}

/// Arma la pista con secciones: entrada, tramo y salida de cada curva (que
/// se suavizan) y lo que sube o baja.
struct Builder {
    segments: Vec<Segment>,
    y: f32,
}

impl Builder {
    fn add(&mut self, enter: usize, hold: usize, leave: usize, curve: f32, rise: f32) {
        let total = enter + hold + leave;
        let start = self.y;
        for n in 0..total {
            let c = if n < enter {
                curve * smoothstep(0.0, 1.0, n as f32 / enter as f32)
            } else if n < enter + hold {
                curve
            } else {
                curve * smoothstep(1.0, 0.0, (n - enter - hold) as f32 / leave.max(1) as f32)
            };
            let t = n as f32 / total as f32;
            let y = start + rise * (0.5 - 0.5 * (t * PI).cos());
            self.segments.push(Segment { curve: c, y, bumps: 0 });
        }
        self.y = start + rise;
    }

    fn straight(&mut self, n: usize) {
        self.add(0, n, 0, 0.0, 0.0);
    }
}

impl Track {
    fn new() -> Self {
        let mut b = Builder { segments: Vec::new(), y: 0.0 };
        b.straight(30);
        b.add(12, 26, 12, 2.0, 0.0);
        b.add(14, 14, 14, 0.0, 16.0);
        b.add(8, 20, 8, -1.5, -16.0);
        // La recta de los primeros tumulos, despues de la bajada.
        let bumps_a = b.segments.len();
        b.straight(26);
        b.add(8, 16, 8, 3.0, 0.0);
        b.add(8, 16, 8, -3.0, 0.0);
        b.add(10, 10, 10, 0.0, 10.0);
        b.add(10, 10, 10, 0.0, -10.0);
        b.add(12, 40, 12, 2.5, 8.0);
        b.add(8, 20, 8, 3.5, -8.0);
        let bumps_b = b.segments.len();
        b.straight(30);
        b.add(12, 30, 12, 2.0, 0.0);
        b.add(8, 14, 8, -2.0, 5.0);
        b.add(12, 28, 12, 2.5, -5.0);
        let y = b.y;
        b.add(0, 20, 0, 0.0, -y);
        let mut segments = b.segments;
        // Una vuelta gira el rumbo una vuelta completa (la pista rodea a la
        // capital): las curvas se escalan para que sumen 2 pi.
        let total: f32 = segments.iter().map(|s| s.curve).sum();
        for s in &mut segments {
            s.curve *= TAU / total;
        }
        // Los tumulos: en la recta despues de la bajada y en la de la
        // vuelta, en unos carriles y no en otros.
        for (at, mask) in [(bumps_a + 7, 0b0011u8), (bumps_a + 17, 0b1100), (bumps_b + 8, 0b0110), (bumps_b + 20, 0b1011)] {
            if let Some(s) = segments.get_mut(at) {
                s.bumps = mask;
            }
        }
        let mut headings = Vec::with_capacity(segments.len());
        let mut heading = 0.0;
        for s in &segments {
            headings.push(heading);
            heading += s.curve;
        }
        // El mapa: el rumbo integrado; lo que no cierra se reparte.
        let mut points = Vec::with_capacity(segments.len() + 1);
        let (mut x, mut z) = (0.0f32, 0.0f32);
        for (s, &h) in segments.iter().zip(&headings) {
            points.push((x, z));
            let mid = h + s.curve * 0.5;
            x += SEGMENT * mid.sin();
            z += SEGMENT * mid.cos();
        }
        let n = points.len() as f32;
        let (ex, ez) = (x, z);
        let mut world = points.clone();
        world.push((ex, ez));
        for (i, p) in points.iter_mut().enumerate() {
            p.0 -= ex * i as f32 / n;
            p.1 -= ez * i as f32 / n;
        }
        let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
        for &(x, z) in &points {
            lo = (lo.0.min(x), lo.1.min(z));
            hi = (hi.0.max(x), hi.1.max(z));
        }
        let size = (hi.0 - lo.0).max(hi.1 - lo.1);
        let map = points
            .iter()
            .map(|&(x, z)| (0.5 + (x - (lo.0 + hi.0) * 0.5) / size, 0.5 - (z - (lo.1 + hi.1) * 0.5) / size))
            .collect();
        Self { segments, headings, map, world, drift: (ex, ez) }
    }

    /// Donde queda en el plano (m) el punto `s` del centro de la pista,
    /// contando las vueltas (cada una corre el trazo `drift`).
    fn world_at(&self, s: f32) -> (f32, f32) {
        let i = self.index(s);
        let f = (s / SEGMENT).rem_euclid(1.0);
        let laps = (s / self.length()).floor();
        let (a, b) = (self.world[i], self.world[i + 1]);
        (a.0 + (b.0 - a.0) * f + laps * self.drift.0, a.1 + (b.1 - a.1) * f + laps * self.drift.1)
    }

    fn length(&self) -> f32 {
        self.segments.len() as f32 * SEGMENT
    }

    fn index(&self, s: f32) -> usize {
        ((s / SEGMENT).floor() as i64).rem_euclid(self.segments.len() as i64) as usize
    }

    fn segment(&self, s: f32) -> &Segment {
        &self.segments[self.index(s)]
    }

    /// Altura de la pista en `s`.
    fn height(&self, s: f32) -> f32 {
        let i = self.index(s);
        let next = &self.segments[(i + 1) % self.segments.len()];
        let t = (s / SEGMENT).rem_euclid(1.0);
        self.segments[i].y + (next.y - self.segments[i].y) * t
    }

    /// Curvatura en `s` (1/m).
    fn curvature(&self, s: f32) -> f32 {
        self.segment(s).curve / SEGMENT
    }

    /// El rumbo en `s`, contando las vueltas.
    fn heading(&self, s: f32) -> f32 {
        let i = self.index(s);
        let laps = (s / self.length()).floor();
        let t = (s / SEGMENT).rem_euclid(1.0);
        laps * TAU + self.headings[i] + self.segments[i].curve * t
    }

    /// La curvatura promedio de `from` a `from + span` metros.
    fn curvature_ahead(&self, from: f32, span: f32) -> f32 {
        let steps = (span / SEGMENT).max(1.0) as usize;
        (0..steps).map(|k| self.curvature(from + k as f32 * SEGMENT)).sum::<f32>() / steps as f32
    }

    /// La curvatura mas fuerte (en valor absoluto) en los proximos metros.
    fn sharpest_ahead(&self, from: f32, span: f32) -> f32 {
        let steps = (span / SEGMENT).max(1.0) as usize;
        (0..steps).map(|k| self.curvature(from + k as f32 * SEGMENT).abs()).fold(0.0, f32::max)
    }
}

// Las naves ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Player,
    Rival,
    Traffic,
}

#[derive(Clone, Debug)]
struct Car {
    kind: Kind,
    role: Role,
    name: &'static str,
    /// Distancia recorrida (m, contando vueltas), lado (m) y velocidad (m/s).
    s: f32,
    x: f32,
    v: f32,
    /// Hacia donde gira (-1 a 1), suavizado: el sprite se ladea con el.
    steer: f32,
    /// Turbo guardado (0 a 1) y segundos que le quedan al que se gasta.
    turbo: f32,
    boost: f32,
    /// A que lado quiere ir (la IA y el trafico) y cuando cambia de carril.
    target: f32,
    lane_timer: f32,
    /// Que tan bien maneja la IA (0.9 a 1).
    skill: f32,
    /// Cuando termino y su mejor vuelta.
    finished: Option<f32>,
    lap_start: f32,
    best_lap: Option<f32>,
    /// Tiempo hasta el siguiente soplo de humo (la camioneta) y desde el
    /// ultimo golpe.
    smoke: f32,
    bumped: f32,
    /// Las calaveras (0 a 1): suben a 12/s al frenar y bajan a 5/s.
    brake_light: f32,
    /// Si acelera en este paso (el humo sale solo acelerando).
    throttle: bool,
    /// El ladeo (rad, + a la derecha de la pantalla) y su velocidad: un
    /// resorte amortiguado hacia lo que pide la curva.
    roll: f32,
    roll_v: f32,
    /// Lo que salta (m) y su velocidad: otro resorte que patean los
    /// tumulos y las crestas.
    bounce: f32,
    bounce_v: f32,
    /// Lo que se junta para la siguiente chispa de su turbo (el tuk-tuk).
    spark_due: f32,
    /// Si en vez de su clase se dibuja como una de las latas (su indice en
    /// las que se pasaron a `Carrera::new`): maneja como la chela (flota)
    /// pero se ve como lata.
    can: Option<u8>,
}

impl Car {
    fn new(kind: Kind, role: Role, name: &'static str, s: f32, x: f32) -> Self {
        Self {
            kind,
            role,
            name,
            s,
            x,
            v: 0.0,
            steer: 0.0,
            turbo: 0.0,
            boost: 0.0,
            target: x,
            lane_timer: 0.0,
            skill: 1.0,
            finished: None,
            lap_start: 0.0,
            best_lap: None,
            smoke: 0.0,
            bumped: 10.0,
            brake_light: 0.0,
            throttle: false,
            roll: 0.0,
            roll_v: 0.0,
            bounce: 0.0,
            bounce_v: 0.0,
            spark_due: 0.0,
            can: None,
        }
    }

    fn lap(&self, length: f32) -> u32 {
        (self.s.max(0.0) / length) as u32
    }
}

/// Un soplo de humo negro de las chimeneas de una camioneta: donde va en
/// la pista (`s`, `x`, y su altura `y`, m), cuanto lleva, a que velocidad
/// sigue a lo largo (sale con la camioneta y el aire lo frena), su radio
/// al nacer, como va girado su ruido y su semilla.
#[derive(Clone, Copy, Debug)]
struct Puff {
    s: f32,
    x: f32,
    y: f32,
    age: f32,
    vs: f32,
    r0: f32,
    rot: f32,
    seed: f32,
    /// Si sale de la nave propia (su humo va tenue: sale delante de la
    /// camara y taparia la pista).
    own: bool,
}

impl Puff {
    /// El radio (m): `r0 + 1.4 edad^0.7`, crece rapido y despues despacio.
    fn radius(&self) -> f32 {
        self.r0 + 1.4 * self.age.max(0.0).powf(0.7)
    }
}

/// Una chispa en la pista (m, con su velocidad en m/s): la del roce con la
/// baranda, los choques, los tumulos y el turbo del tuk-tuk. Cae con la
/// gravedad y rebota en el asfalto. Con edad negativa todavia no sale.
#[derive(Clone, Copy, Debug)]
struct TrackSpark {
    s: f32,
    x: f32,
    y: f32,
    vs: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    color: Vec3,
    /// Cuanto devuelve al rebotar.
    bounce: f32,
}

/// Donde van en una nave (m, desde su centro a lo largo con `+z` hacia
/// atras, `x` a la derecha y `y` desde el suelo) las chimeneas (de cada
/// lado, si tiene) y el escape.
#[derive(Clone, Debug, Default)]
struct Rig {
    stacks: Vec<Vec3>,
    exhaust: Vec3,
}

/// Un aviso en la pantalla.
#[derive(Clone, Copy, Debug)]
struct Message {
    text: &'static str,
    age: f32,
    color: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Antes de arrancar: quien corre y las teclas.
    Ready,
    /// El semaforo: segundos desde que empezo.
    Countdown(f32),
    Racing,
    /// Termino quien juega: segundos desde entonces.
    Finished(f32),
}

/// Un sonido de una vez.
#[derive(Clone, Copy, Debug)]
enum Sound {
    Thud,
    Bump,
    Horn,
    Beep(f32, f32),
    Whoosh,
    /// El pum de un cohete: llega `.0` segundos despues (lo que tarda en
    /// subir mas lo que tarda el sonido) y la palmera truena dos veces.
    Pum(f32, bool),
}

/// Un cohete: hacia donde revienta (rumbo de mundo y elevacion, rad),
/// cuando salio (reloj de pared; revienta `RISE` despues), su semilla y su
/// clase: 0 palmera dorada, 1 navideno (rojo y verde), 2 patrio (celeste
/// y blanco), 3 de papel de china y 4 crepitante.
#[derive(Clone, Copy, Debug)]
struct Burst {
    yaw: f32,
    pitch: f32,
    born: f32,
    seed: u32,
    kind: u8,
}

impl Burst {
    /// Cuanto lleva reventado (negativo mientras sube).
    fn age(&self, wall: f32) -> f32 {
        wall - self.born - RISE
    }

    /// La chispa `i` a los `t` segundos de reventar, sin estado (todo sale
    /// de la semilla): su corrimiento en rumbo y elevacion desde el centro
    /// del cohete. La direccion es un punto al azar en una esfera vista de
    /// frente, `(sin th cos ph, sin ph)` con `sin ph` parejo en `[-1, 1]`
    /// (las chispas se juntan hacia el centro como en una esfera de
    /// verdad). Se abre como `rho = R (1 - e^(-3 t))` (el aire la frena) y
    /// cae `0.015 t^2` (el doble la palmera, que se deshace en sauce).
    fn spark(&self, i: u32, t: f32) -> (f32, f32) {
        let h = hash_u32(self.seed ^ i.wrapping_mul(0x9E37_79B9));
        let th = unit(h) * TAU;
        let sin_ph = unit(hash_u32(h ^ 0x51ED)) * 2.0 - 1.0;
        let cos_ph = (1.0 - sin_ph * sin_ph).max(0.0).sqrt();
        let reach = 0.10 + 0.08 * unit(hash_u32(self.seed ^ 0xB0B));
        let rho = reach * (1.0 - (-3.0 * t.max(0.0)).exp());
        let fall = if self.kind == 0 { 0.03 } else { 0.015 } * t * t;
        (th.sin() * cos_ph * rho, sin_ph * rho - fall)
    }

    /// El color de la chispa `i` (luz lineal).
    fn color(&self, i: u32) -> Vec3 {
        match self.kind {
            0 => Vec3::new(1.0, 0.72, 0.3),
            1 => if i % 2 == 0 { Vec3::new(1.0, 0.15, 0.1) } else { Vec3::new(0.2, 1.0, 0.35) },
            2 => if i % 2 == 0 { Vec3::new(0.3, 0.65, 1.0) } else { Vec3::new(1.0, 1.0, 1.0) },
            3 => PAPEL[(i % 6) as usize],
            _ => Vec3::new(0.95, 0.92, 1.0),
        }
    }

    /// Lo que brilla la chispa `i` a los `t` segundos: `e^(-1.3 t)`, y la
    /// crepitante destella al doble en uno de cada cuatro pasos de 1/18 s.
    fn bright(&self, i: u32, t: f32) -> f32 {
        let b = (-1.3 * t.max(0.0)).exp();
        if self.kind == 4 && hash_u32(self.seed ^ i.wrapping_mul(0x2545_F491) ^ ((t * 18.0) as u32).wrapping_mul(0x85EB_CA77)) & 3 == 0 { 2.0 * b } else { b }
    }
}

/// Un trazo de chispa en la pantalla: la capsula de la cola `a` a la cabeza
/// `b`, su color ya por su brillo, cuanto emite, su radio (px), lo que
/// brilla la cola y la caja hasta donde llega su halo.
#[derive(Clone, Copy, Debug)]
struct SparkSeg {
    a: (f32, f32),
    b: (f32, f32),
    color: Vec3,
    emit: f32,
    w: f32,
    /// Cuanto queda de luz en la cola (`a`): de ahi crece a 1 en la cabeza.
    tail: f32,
    lo: (f32, f32),
    hi: (f32, f32),
}

/// Un barrilete en la vuelta: donde (m), de que lado, cuanto mide (m), su
/// diseno y su numero.
#[derive(Clone, Copy, Debug)]
struct KiteSpot {
    s: f32,
    side: f32,
    size: f32,
    design: u8,
    id: u32,
}

/// El sintetizador del juego: el motor de quien juega, el viento y los
/// golpes.
#[derive(Default)]
struct Synth {
    phase: f32,
    sub: f32,
    noise: u32,
    low: f32,
    wind: f32,
    /// Lo que pide el juego: vueltas del motor (0 a 1), acelerador,
    /// velocidad (0 a 1) y la clase.
    rpm: f32,
    throttle: f32,
    speed: f32,
    rpm_now: f32,
    /// El ruido grave de los cohetes (un pasabajos de un polo).
    boom: f32,
    events: Vec<(Sound, f32)>,
}

pub struct Carrera {
    phase: Phase,
    track: Track,
    cars: Vec<Car>,
    player_kind: Kind,
    /// El nombre de cada clase (la chela es la botella que se trae).
    names: [&'static str; 4],
    /// Tiempo de carrera (desde el verde).
    clock: f32,
    accumulator: f32,
    /// Sprites de cada clase (15 angulos) y de la nave propia (9).
    sets: Vec<Vec<Sprite>>,
    player_set: Vec<Sprite>,
    /// Las mismas mallas (en el orden de `sets`) para dibujar en 3D las que
    /// pasan cerca (ver `NEAR_SOLID`).
    solids: Vec<hornear::Solid>,
    /// Los buffers de las mallas de un frame, para el siguiente.
    mesh_pool: std::sync::Mutex<Vec<Indexed>>,
    /// El color del charco de luz de cada clase (el de sus LEDs).
    tint: [Vec3; 4],
    panorama: Panorama,
    /// La capital horneada, el gradiente del cielo sobre el limbo (lineal y
    /// ya con tono) y lo que pasa en el: el Fuego y las estrellas fugaces.
    ciudad: Ciudad,
    sky_lin_lut: Vec<Vec3>,
    sky_lut: Vec<Vec3>,
    eruption: Eruption,
    meteor: Option<Meteor>,
    next_meteor: f32,
    /// Los barriletes horneados (cada diseno con sus mips) y donde van.
    kites: Vec<Vec<Sprite>>,
    kite_spots: Vec<KiteSpot>,
    /// Los cohetes en el aire, cuando sale el proximo de ambiente y
    /// cuantos han salido (la semilla).
    bursts: Vec<Burst>,
    next_burst: f32,
    burst_count: u32,
    /// Cuando se dio el verde (reloj de pared).
    green_at: Option<f32>,
    minimap: Vec<[f32; 4]>,
    map_size: usize,
    puffs: Vec<Puff>,
    /// Las chimeneas y el escape de cada clase, y desde que altura (rad)
    /// se horneo la nave propia.
    rigs: [Rig; 4],
    player_elevation: f32,
    /// Desde que altura se hornearon las otras, por clase.
    rival_elevation: [f32; 4],
    /// Las chispas en la pista y lo que se junta para la siguiente del
    /// roce con la baranda (salen 120 por segundo; las del turbo llevan su
    /// cuenta en cada nave, `Car::spark_due`).
    sparks: Vec<TrackSpark>,
    spark_acc: f32,
    /// El contador del azar de las chispas y el de los soplos de humo.
    rng: u32,
    puff_count: u32,
    /// Donde roza la nave propia cada baranda (`s`) y cuanto queda del
    /// destello blanco del neon (baja 2.5 por segundo).
    scrape: [(f32, f32); 2],
    width: usize,
    height: usize,
    /// Sacudida de la camara y humo en la pantalla.
    shake: f32,
    smoke_screen: f32,
    /// La apertura de mas de la camara con la velocidad y el turbo (un
    /// resorte: `fov` y su velocidad `fov_v`) y el turbo suavizado para el
    /// posproceso (sube a 4/s y baja a 2/s).
    fov: f32,
    fov_v: f32,
    boost_s: f32,
    /// El brillo, el acabado y el humo en la pantalla.
    post: Posproceso,
    /// Rebufo: si se esta cargando (para el aviso).
    drafting: bool,
    horn_cooldown: f32,
    messages: Vec<Message>,
    records: Records,
    new_record: bool,
    autopilot: bool,
    done: bool,
    synth: Synth,
    wall: f32,
    /// El paso de los `update` (s), suavizado: lo que se corre la pista en
    /// un frame emborrona las rayas a lo largo. Con un pasabajos de 4/s,
    /// `dt' += (dt - dt') (1 - e^(-4 dt))`, el temblor del vsync (16.4 a
    /// 16.9 ms) no mueve el borron de un frame al otro.
    frame_dt: f32,
    /// La entrada del frame (la fisica la usa en cada paso).
    steer_in: f32,
    throttle_in: bool,
    brake_in: bool,
}

impl Carrera {
    /// Abre la carrera: `models` son las cuatro clases en el orden de
    /// `Kind::ALL`; `sky` da el color del cielo en cada direccion.
    pub fn new(models: [Model; 4], cans: &[Model], player: Kind, sky: &(dyn Fn(Vec3) -> Vec3 + Sync), width: usize, height: usize, records: Records) -> Self {
        let s = height as f32 / 720.0;
        let focal = FOCAL * height as f32;
        // Los angulos de las otras, y desde que altura se ven: la que tiene
        // la camara de quien juega sobre el centro de cada una a la distancia
        // donde la nave pasa del sprite al 3D (ver `rival_elevation`).
        let yaws: Vec<f32> = (0..YAWS).map(|k| (k as f32 - (YAWS / 2) as f32) * YAW_STEP).collect();
        let rival_elevation: [f32; 4] = std::array::from_fn(|k| rival_elevation(player, Kind::ALL[k]));
        let mut jobs = Vec::new();
        for (k, m) in models.iter().enumerate() {
            for &yaw in &yaws {
                jobs.push(hornear::Job { mesh: m.mesh, finish: m.finish, yaw, elevation: rival_elevation[k], meters: m.meters, pixels_per_meter: 46.0 * s, eye: 0.0, rise: 0.0 });
            }
        }
        // La propia, de cerca: tal como la ve la camara, a su tamano en
        // pantalla.
        let spec = player.spec();
        let (back, up) = camera_offset(spec);
        let model = &models[player.index()];
        let elevation = ((up - spec.height * 0.45) / back).atan();
        // La perspectiva del horneado: la camara a `1.5 (back - largo / 2)`
        // de la cola (de verdad esta a `back - largo / 2`, pero tan de cerca
        // la cola salia el doble y la camioneta se salia de la pantalla; un
        // poco mas lejos es como un lente largo), con el punto de fuga en
        // el centro de la nave (en el horizonte, el techo se alargaba de mas
        // con la vista desde arriba del horneado).
        let eye = 1.5 * (back - 0.5 * spec.length).max(1.0);
        let rise = 0.0;
        for k in 0..PLAYER_YAWS {
            let yaw = (k as f32 - (PLAYER_YAWS / 2) as f32) * PLAYER_YAW_STEP;
            jobs.push(hornear::Job { mesh: model.mesh, finish: model.finish, yaw, elevation, meters: model.meters, pixels_per_meter: focal / back, eye, rise });
        }
        // Las latas del trafico, vistas como las rivales.
        for m in cans {
            for &yaw in &yaws {
                jobs.push(hornear::Job { mesh: m.mesh, finish: m.finish, yaw, elevation: rival_elevation[Kind::Chela.index()], meters: m.meters, pixels_per_meter: 46.0 * s, eye: 0.0, rise: 0.0 });
            }
        }
        let mut baked = hornear::bake_all(&jobs);
        let mut can_sprites = baked.split_off(models.len() * YAWS + PLAYER_YAWS);
        let player_set = baked.split_off(models.len() * YAWS);
        let mut sets: Vec<Vec<Sprite>> = Vec::new();
        while !baked.is_empty() {
            let rest = baked.split_off(YAWS);
            sets.push(std::mem::replace(&mut baked, rest));
        }
        // Despues de las cuatro clases, las latas.
        while !can_sprites.is_empty() {
            let rest = can_sprites.split_off(YAWS);
            sets.push(std::mem::replace(&mut can_sprites, rest));
        }
        let tint = std::array::from_fn(|k| if Kind::ALL[k] == Kind::Chela { Vec3::new(1.0, 0.7, 0.3) } else { led_tint(&sets[k][YAWS / 2]) });
        let solids = models.iter().chain(cans.iter()).map(|m| hornear::Solid::new(m.near, m.finish, m.meters)).collect();
        let rigs = std::array::from_fn(|k| rig_of(&models[k]));
        let track = Track::new();
        let kites = escenario::barriletes();
        let kite_spots = kite_spots(&track);
        let map_size = (190.0 * s) as usize;
        let minimap = draw_map(&track, map_size);
        let names = models.map(|m| m.name);
        let mut game = Self {
            phase: Phase::Ready,
            cars: Vec::new(),
            player_kind: player,
            names,
            clock: 0.0,
            accumulator: 0.0,
            sets,
            player_set,
            solids,
            mesh_pool: std::sync::Mutex::new(Vec::new()),
            tint,
            panorama: Panorama::new(sky),
            ciudad: Ciudad::new(),
            sky_lin_lut: (0..SKY_LUT).map(|i| sky_lin(sky_eps(i))).collect(),
            sky_lut: (0..SKY_LUT).map(|i| tone(sky_lin(sky_eps(i)))).collect(),
            eruption: Eruption::new(),
            meteor: None,
            next_meteor: 3.0,
            kites,
            kite_spots,
            bursts: Vec::new(),
            next_burst: 0.0,
            burst_count: 0,
            green_at: None,
            minimap,
            map_size,
            puffs: Vec::new(),
            rigs,
            player_elevation: elevation,
            rival_elevation,
            sparks: Vec::new(),
            spark_acc: 0.0,
            rng: 0x51A7,
            puff_count: 0,
            scrape: [(0.0, 0.0); 2],
            width,
            height,
            shake: 0.0,
            smoke_screen: 0.0,
            fov: 0.0,
            fov_v: 0.0,
            boost_s: 0.0,
            post: Posproceso::new(width, height),
            drafting: false,
            horn_cooldown: 0.0,
            messages: Vec::new(),
            records,
            new_record: false,
            autopilot: false,
            done: false,
            synth: Synth { noise: 0x5EED, ..Synth::default() },
            wall: 0.0,
            frame_dt: 1.0 / 60.0,
            steer_in: 0.0,
            throttle_in: false,
            brake_in: false,
            track,
        };
        game.reset();
        game
    }

    /// La parrilla: quien juega atras de las tres rivales y el trafico
    /// repartido por la pista.
    fn reset(&mut self) {
        let names = self.names;
        let length = self.track.length();
        let mut cars = vec![Car::new(self.player_kind, Role::Player, "Vos", -26.0, LANE * 0.5)];
        let rivals: Vec<Kind> = Kind::ALL.into_iter().filter(|&k| k != self.player_kind).collect();
        for (k, &kind) in rivals.iter().enumerate() {
            let mut car = Car::new(kind, Role::Rival, names[kind.index()], -6.0 - 9.0 * k as f32, [-LANE * 1.5, LANE * 1.5, -LANE * 0.5][k]);
            car.skill = [0.97, 0.95, 0.99][k];
            cars.push(car);
        }
        // El trafico: 18 naves, y una de cada tres es una de las latas (la
        // Four Loko y la Jager, alternando) que flota como la chela.
        let cans = self.sets.len().saturating_sub(Kind::ALL.len());
        for k in 0..TRAFFIC {
            let h = hash_u32(0x7EAF ^ k.wrapping_mul(0x9E37));
            let can = (cans > 0 && k % 3 == 2).then(|| ((k / 3) as usize % cans) as u8);
            let kind = if can.is_some() { Kind::Chela } else { Kind::ALL[(h % 4) as usize] };
            let lane = ((h >> 4) % 4) as f32;
            let s = 120.0 + k as f32 * (length - 160.0) / TRAFFIC as f32;
            let mut car = Car::new(kind, Role::Traffic, "", s, -HALF_ROAD + LANE * (lane + 0.5));
            car.can = can;
            car.v = 16.0 + 10.0 * ((h >> 8) & 0xFF) as f32 / 255.0;
            car.skill = car.v;
            car.lane_timer = 4.0 + ((h >> 16) & 0xFF) as f32 / 20.0;
            cars.push(car);
        }
        self.cars = cars;
        self.clock = 0.0;
        self.puffs.clear();
        self.sparks.clear();
        self.scrape = [(0.0, 0.0); 2];
        // El azar de las chispas y del humo vuelve a empezar: cada carrera
        // sale igual.
        self.spark_acc = 0.0;
        self.rng = 0x51A7;
        self.puff_count = 0;
        self.messages.clear();
        self.new_record = false;
        self.shake = 0.0;
        self.smoke_screen = 0.0;
        self.fov = 0.0;
        self.fov_v = 0.0;
        self.boost_s = 0.0;
        self.bursts.clear();
        self.green_at = None;
        // Los pum de los cohetes que aun no llegan: sin cohetes, tampoco
        // su trueno (al salir al menu seguirian tronando hasta 2 s).
        self.synth.events.retain(|(sound, _)| !matches!(sound, Sound::Pum(..)));
    }

    /// Lanza un cohete que revienta en `(yaw, pitch)` dentro de `explode_in`
    /// segundos (si es menos de lo que tarda en subir, ya va subiendo). Si
    /// ya hay `MAX_BURSTS` en el aire se va el mas viejo. El pum llega de
    /// 0.3 a 0.8 s despues de reventar (lo lejos que esta).
    fn spawn_burst(&mut self, yaw: f32, pitch: f32, kind: u8, explode_in: f32) {
        self.burst_count = self.burst_count.wrapping_add(1);
        let seed = hash_u32(self.burst_count.wrapping_mul(0x2C1B_3C6D) ^ 0xC0E7);
        if self.bursts.len() >= MAX_BURSTS {
            self.bursts.remove(0);
        }
        self.bursts.push(Burst { yaw, pitch, born: self.wall + explode_in - RISE, seed, kind });
        let delay = explode_in.max(0.0) + 0.3 + 0.5 * unit(hash_u32(seed ^ 0xD1A));
        self.sound(Sound::Pum(delay, kind == 0));
    }

    /// Cohetes alrededor de un rumbo: `count` que revientan de `at` en
    /// adelante, uno cada `every` segundos, repartidos `spread` rad a cada
    /// lado y de `pitch.0` a `pitch.1` de alto, con clases al azar.
    fn volley(&mut self, count: u32, at: f32, every: f32, spread: f32, pitch: (f32, f32)) {
        let heading = self.camera().heading;
        for k in 0..count {
            let h = hash_u32(self.burst_count.wrapping_add(k).wrapping_mul(0x9E37_79B9) ^ 0x7ED);
            let r = |j: u32| unit(hash_u32(h ^ j.wrapping_mul(0x85EB_CA6B)));
            let side = if count > 1 { k as f32 / (count - 1) as f32 * 2.0 - 1.0 } else { 0.0 };
            let yaw = heading + side * spread + (r(1) - 0.5) * 0.12;
            self.spawn_burst(yaw, pitch.0 + (pitch.1 - pitch.0) * r(2), (r(3) * 5.0) as u8 % 5, at + every * k as f32);
        }
    }

    /// Que la IA maneje la nave propia (pruebas y capturas).
    pub fn set_autopilot(&mut self, on: bool) {
        self.autopilot = on;
    }

    /// Si la camara va adentro de un tunel de arcos con arcos adelante (de
    /// la entrada a 48 m de la salida): para las fotos.
    pub fn in_tunnel(&self) -> bool {
        let here = self.camera().s.rem_euclid(self.track.length());
        ARCH_ZONES.iter().any(|&(a, b)| (a - 8.0..b - 48.0).contains(&here))
    }

    /// Si la camara va por el festival de barriletes con barriletes
    /// adelante de los dos lados: para las fotos.
    pub fn in_festival(&self) -> bool {
        let here = self.camera().s.rem_euclid(self.track.length());
        (FESTIVAL.0 + 40.0..FESTIVAL.1 - 240.0).contains(&here)
    }

    /// Cuantos metros faltan para el proximo tumulo delante de la nave
    /// propia (el centro de su tramo), para la foto de los tumulos.
    pub fn bump_ahead(&self) -> Option<f32> {
        let s = self.player().s;
        let length = self.track.length();
        let here = s.rem_euclid(length);
        self.track
            .segments
            .iter()
            .enumerate()
            .filter(|(_, seg)| seg.bumps != 0)
            .map(|(i, _)| ((i as f32 + 0.5) * SEGMENT - here).rem_euclid(length))
            .min_by(f32::total_cmp)
    }

    /// Empieza el semaforo.
    pub fn start(&mut self) {
        self.reset();
        self.phase = Phase::Countdown(0.0);
    }

    fn player(&self) -> &Car {
        &self.cars[0]
    }

    /// Un aviso al centro: queda de ultimo (es el que se ve; si ya estaba,
    /// vuelve a empezar).
    fn message(&mut self, text: &'static str, color: [u8; 4]) {
        self.messages.retain(|m| m.text != text);
        self.messages.push(Message { text, age: 0.0, color });
    }

    fn sound(&mut self, sound: Sound) {
        self.synth.events.push((sound, 0.0));
    }

    /// Posicion de cada nave en la carrera (0 la primera): las que ya
    /// terminaron por su tiempo, las demas por lo que llevan.
    fn standings(&self) -> Vec<usize> {
        let mut racers: Vec<usize> = (0..self.cars.len()).filter(|&i| self.cars[i].role != Role::Traffic).collect();
        racers.sort_by(|&a, &b| {
            let (ca, cb) = (&self.cars[a], &self.cars[b]);
            match (ca.finished, cb.finished) {
                (Some(ta), Some(tb)) => ta.total_cmp(&tb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => cb.s.total_cmp(&ca.s),
            }
        });
        racers
    }

    fn place(&self) -> usize {
        self.standings().iter().position(|&i| i == 0).unwrap_or(0)
    }

    /// Un paso de fisica para todas.
    fn physics(&mut self) {
        let length = self.track.length();
        let racing = self.phase == Phase::Racing || matches!(self.phase, Phase::Finished(_));
        let total = self.cars.len();
        let player_s = self.cars[0].s;
        let leader = self.cars.iter().filter(|c| c.role != Role::Traffic).map(|c| c.s).fold(f32::MIN, f32::max);
        for i in 0..total {
            let car = self.cars[i].clone();
            let spec = car.kind.spec();
            let (steer, throttle, brake, boost) = match car.role {
                Role::Player if !self.autopilot && car.finished.is_none() => (self.steer_in, self.throttle_in, self.brake_in, false),
                Role::Traffic => (((car.target - car.x) * 0.6).clamp(-1.0, 1.0), car.v < car.skill, false, false),
                _ => {
                    // La IA (y quien juega con piloto automatico o ya en la meta).
                    let rubber = if car.role == Role::Rival {
                        let gap = car.s - player_s;
                        1.0 + (-gap / 300.0).clamp(-0.05, 0.06)
                    } else {
                        1.0
                    };
                    self.drive(i, rubber)
                }
            };
            // Las calaveras siguen al freno (tambien en la largada).
            let light = &mut self.cars[i].brake_light;
            *light = if brake { (*light + 12.0 * STEP).min(1.0) } else { (*light - 5.0 * STEP).max(0.0) };
            self.cars[i].throttle = throttle && racing;
            if !racing {
                // En la largada nadie se mueve (el trafico tampoco).
                continue;
            }
            let car = &mut self.cars[i];
            if boost && car.turbo > 0.3 {
                car.boost = 1.0 + 2.2 * car.turbo;
                car.turbo = 0.0;
            }
            let kappa = self.track.curvature(car.s);
            let offroad = car.x.abs() > HALF_ROAD;
            let boosting = car.boost > 0.0;
            let mut top = spec.top * if boosting { 1.25 } else { 1.0 };
            if offroad {
                top *= 0.5;
            }
            if throttle {
                let push = if boosting { 1.6 } else { 1.0 };
                car.v += spec.accel * push * (1.0 - car.v / top).clamp(-1.0, 1.0) * STEP;
            } else {
                car.v -= 3.0 * STEP;
            }
            if brake {
                car.v -= 26.0 * STEP;
            }
            if car.v > top {
                car.v -= (car.v - top) * 1.5 * STEP;
            }
            car.v = car.v.max(0.0);
            car.boost = (car.boost - STEP).max(0.0);
            // De lado: el volante contra lo que empuja la curva.
            let grip = (car.v / spec.top).min(1.0);
            car.steer += (steer - car.steer) * (1.0 - (-10.0 * STEP).exp());
            car.x += steer * spec.handling * (0.35 + 0.65 * grip) * STEP;
            car.x -= kappa * car.v * car.v * CENTRIFUGAL * STEP;
            // El ladeo: un resorte amortiguado hacia
            // `meta = -volante (v / top) ladeo` (hacia afuera de la curva),
            // con Euler semi-implicito: primero la velocidad y con ella el
            // angulo.
            let (lean, stiff, damp) = car.kind.roll_spring();
            let goal = -car.steer * (car.v / spec.top).clamp(0.0, 1.2) * lean;
            car.roll_v += (stiff * (goal - car.roll) - damp * car.roll_v) * STEP;
            car.roll += car.roll_v * STEP;
            // Las barandas: la propia, si roza con velocidad, saca chispas
            // (120 por segundo) y pone blanco el neon donde roza.
            let limit = RAIL - spec.width * 0.5;
            let mut scrape = None;
            if car.x.abs() > limit {
                car.x = car.x.clamp(-limit, limit);
                car.v *= 1.0 - 0.6 * STEP;
                if car.role == Role::Player && car.v > 10.0 {
                    scrape = Some((car.s, car.x.signum(), car.v, spec.length));
                }
            }
            let before = car.s;
            car.s += car.v * STEP;
            car.bumped += STEP;
            // La cresta: al cambiar la pendiente de un tramo al siguiente,
            // `bounce' -= dpendiente v 0.05` (en una cresta la pendiente baja
            // y la nave se despega un poco).
            let crossed = self.track.index(before) != self.track.index(car.s);
            if crossed {
                let slope = |s: f32| (self.track.height(s + 0.5 * SEGMENT) - self.track.height(s - 0.5 * SEGMENT)) / SEGMENT;
                let after = self.cars[i].s;
                let change = slope(after) - slope(before);
                let car = &mut self.cars[i];
                car.bounce_v -= change * car.v * 0.05;
            }
            // Los tumulos: si se pasa uno en su carril con velocidad.
            let mut bumped_at = None;
            if crossed && spec.hover == 0.0 {
                let segment = *self.track.segment(self.cars[i].s);
                let car = &mut self.cars[i];
                let lane = (((car.x + HALF_ROAD) / LANE).floor() as i32).clamp(0, 3);
                if segment.bumps & (1 << lane) != 0 && car.v > 18.0 {
                    car.v *= 0.62;
                    car.bounce_v += 1.8;
                    bumped_at = Some((car.s, car.x, car.v));
                    if car.role == Role::Player {
                        self.shake = 1.0;
                        self.message("¡Túmulo!", [255, 214, 90, 255]);
                        self.sound(Sound::Bump);
                    }
                }
            }
            // El rebote: `bounce'' = -140 bounce - 9 bounce'` (unos 1.9
            // rebotes por segundo que se apagan en medio segundo).
            let car = &mut self.cars[i];
            car.bounce_v += (-140.0 * car.bounce - 9.0 * car.bounce_v) * STEP;
            car.bounce += car.bounce_v * STEP;
            let near = (car.s - player_s).abs() < 260.0;
            if let (Some((s, x, v)), true) = (bumped_at, near) {
                // Las llantas contra el tumulo: 24 chispas naranjas y otras
                // 24 un poco despues (las de atras).
                let y = self.track.height(s) + 0.15;
                for delay in [0.0, 0.12] {
                    self.spawn_sparks(24, (s, x, y), (v * 0.55, 0.0, 2.2), (4.0, 2.5, 1.5), Vec3::new(1.0, 0.5, 0.15) * 2.5, (0.3, 0.6), 0.35, delay);
                }
            }
            if let Some((s, side, v, length)) = scrape {
                self.spark_acc += 120.0 * STEP;
                let count = self.spark_acc.floor();
                self.spark_acc -= count;
                let y = self.track.height(s) + 0.5;
                for _ in 0..count as u32 {
                    let along = s + (self.random() - 0.5) * length * 0.8;
                    self.spawn_sparks(1, (along, side * (RAIL - 0.1), y), (v * 0.65, -side * 1.5, 1.8), (3.0, 1.2, 1.4), Vec3::new(1.0, 0.85, 0.5) * 2.5, (0.35, 0.7), 0.35, 0.0);
                }
                let k = (side > 0.0) as usize;
                self.scrape[k] = (s, 1.0);
            }
            // El turbo del tuk-tuk: chispas que saltan del escape, muchas y
            // cortas (90 por segundo que viven de 0.12 a 0.35 s: unas 20 a
            // la vez), cada nave con su cuenta.
            let car = &mut self.cars[i];
            if car.kind == Kind::TukTuk && car.boost > 0.0 && near {
                car.spark_due += 90.0 * STEP;
                let count = car.spark_due.floor();
                car.spark_due -= count;
                let (s0, x0, v) = (car.s, car.x, car.v);
                let ex = self.rigs[Kind::TukTuk.index()].exhaust;
                let s = s0 - ex.z;
                let y = self.track.height(s) + ex.y;
                self.spawn_sparks(count as u32, (s, x0 + ex.x, y), (v * 0.6, 0.0, 2.0), (2.5, 1.8, 1.5), Vec3::new(1.0, 0.7, 0.3) * 2.0, (0.12, 0.35), 0.4, 0.0);
            }
            let car = &mut self.cars[i];
            // Vueltas y meta.
            if car.role != Role::Traffic && car.finished.is_none() {
                let lap_now = car.lap(length);
                if lap_now > (before.max(0.0) / length) as u32 && before >= 0.0 {
                    let lap_time = self.clock - car.lap_start;
                    car.best_lap = Some(car.best_lap.map_or(lap_time, |b: f32| b.min(lap_time)));
                    car.lap_start = self.clock;
                    if lap_now >= LAPS {
                        car.finished = Some(self.clock);
                    } else if car.role == Role::Player && lap_now == LAPS - 1 {
                        self.message("¡Última vuelta!", [255, 204, 120, 255]);
                    }
                }
            }
            // El trafico da la vuelta para siempre (un poco atras del lider, para que no se acabe).
            let car = &mut self.cars[i];
            if car.role == Role::Traffic && car.s < leader - 200.0 {
                car.s += length;
            }
        }
        if !racing {
            return;
        }
        self.collide();
        self.draft();
        self.traffic_lanes();
        self.exhaust();
        self.move_sparks();
    }

    /// Un numero al azar de 0 a 1 (un contador por un hash).
    fn random(&mut self) -> f32 {
        self.rng = self.rng.wrapping_add(1);
        unit(hash_u32(self.rng.wrapping_mul(0x9E37_79B9) ^ 0x5EA7))
    }

    /// Echa `n` chispas desde `at` (`s`, `x`, `y`) con la velocidad `vel`
    /// (`vs`, `vx`, `vy`) mas `spread` al azar de cada lado, del color
    /// `color`, que viven de `life.0` a `life.1` s, devuelven `bounce` al
    /// rebotar y salen dentro de `delay` s. Si ya hay `MAX_TRACK_SPARKS` se
    /// van las mas viejas.
    #[allow(clippy::too_many_arguments)]
    fn spawn_sparks(&mut self, n: u32, at: (f32, f32, f32), vel: (f32, f32, f32), spread: (f32, f32, f32), color: Vec3, life: (f32, f32), bounce: f32, delay: f32) {
        for _ in 0..n {
            let r = [self.random(), self.random(), self.random(), self.random()];
            let spark = TrackSpark {
                s: at.0,
                x: at.1,
                y: at.2,
                vs: vel.0 + spread.0 * (2.0 * r[0] - 1.0),
                vx: vel.1 + spread.1 * (2.0 * r[1] - 1.0),
                vy: vel.2 + spread.2 * r[2],
                age: -delay,
                life: life.0 + (life.1 - life.0) * r[3],
                color,
                bounce,
            };
            if self.sparks.len() >= MAX_TRACK_SPARKS {
                self.sparks.remove(0);
            }
            self.sparks.push(spark);
        }
    }

    /// Las chispas: tiro parabolico (`g = 9.8`) con un poco de aire
    /// (`e^(-1.5 dt)` a lo largo y de lado), y al tocar el asfalto rebotan,
    /// `vy = -rebote vy`, y se frenan. Las que aun no salen esperan.
    fn move_sparks(&mut self) {
        let drag = (-1.5 * STEP).exp();
        for p in &mut self.sparks {
            if p.age < 0.0 {
                p.age += STEP;
                continue;
            }
            p.vy -= 9.8 * STEP;
            p.vs *= drag;
            p.vx *= drag;
            p.s += p.vs * STEP;
            p.x += p.vx * STEP;
            p.y += p.vy * STEP;
            let ground = self.track.height(p.s);
            if p.y < ground {
                p.y = ground;
                p.vy = -p.bounce * p.vy;
                p.vs *= 0.8;
                p.vx *= 0.8;
            }
            p.age += STEP;
        }
        self.sparks.retain(|p| p.age < p.life);
    }

    /// Lo que hace la IA de la nave `i`: volante, acelerador, freno y turbo.
    fn drive(&self, i: usize, rubber: f32) -> (f32, bool, bool, bool) {
        let car = &self.cars[i];
        let spec = car.kind.spec();
        let track = &self.track;
        // El lado de adentro de la curva que viene.
        let ahead = track.curvature_ahead(car.s + 20.0, 60.0);
        let mut target = (ahead * 900.0).clamp(-5.0, 5.0);
        // Esquivar lo que va adelante y mas lento.
        let blocked = |x: f32| {
            self.cars.iter().enumerate().any(|(j, o)| {
                j != i && o.s > car.s && o.s - car.s < 35.0 + (car.v - o.v).max(0.0) * 1.2 && (o.x - x).abs() < (spec.width + o.kind.spec().width) * 0.5 + 0.6 && o.v < car.v + 2.0
            })
        };
        if blocked(target) || blocked(car.x) {
            let options = [-6.0, -2.0, 2.0, 6.0];
            if let Some(&free) = options
                .iter()
                .filter(|&&x| !blocked(x))
                .min_by(|a, b| (*a - car.x).abs().total_cmp(&(*b - car.x).abs()))
            {
                target = free;
            }
        }
        // Los tumulos que vienen: buscar un carril sin tumulo.
        if spec.hover == 0.0 {
            for k in 1..14 {
                let segment = track.segment(car.s + k as f32 * SEGMENT);
                if segment.bumps != 0 {
                    let lane = |x: f32| (((x + HALF_ROAD) / LANE).floor() as i32).clamp(0, 3);
                    if segment.bumps & (1 << lane(target)) != 0 {
                        if let Some(free) = (0..4).filter(|l| segment.bumps & (1 << l) == 0).min_by_key(|&l| (l - lane(car.x)).abs()) {
                            target = -HALF_ROAD + LANE * (free as f32 + 0.5);
                        }
                    }
                    break;
                }
            }
        }
        let steer = ((target - car.x) * 0.45).clamp(-1.0, 1.0);
        // Lo que aguanta la curva mas fuerte que viene.
        let sharp = track.sharpest_ahead(car.s, 25.0 + car.v * 0.8);
        let corner = if sharp > 1e-5 { (GRIP * spec.handling / sharp).sqrt() } else { f32::MAX };
        let want = (spec.top * car.skill * rubber).min(corner * (0.9 + 0.1 * car.skill));
        let throttle = car.v < want;
        let brake = car.v > want + 4.0;
        // El turbo en las rectas.
        let straight = track.sharpest_ahead(car.s, 160.0) < 0.002;
        (steer, throttle, brake, straight && car.turbo > 0.6)
    }

    /// Los choques: rectangulos que se separan segun su masa; el de atras
    /// pierde velocidad.
    fn collide(&mut self) {
        let n = self.cars.len();
        for a in 0..n {
            for b in a + 1..n {
                let (ca, cb) = (&self.cars[a], &self.cars[b]);
                let (sa, sb) = (ca.kind.spec(), cb.kind.spec());
                let ds = cb.s - ca.s;
                let dx = cb.x - ca.x;
                let reach_s = (sa.length + sb.length) * 0.5;
                let reach_x = (sa.width + sb.width) * 0.5;
                if ds.abs() >= reach_s || dx.abs() >= reach_x {
                    continue;
                }
                let (overlap_s, overlap_x) = (reach_s - ds.abs(), reach_x - dx.abs());
                let total = sa.mass + sb.mass;
                let (wa, wb) = (sb.mass / total, sa.mass / total);
                let player = a == 0 || b == 0;
                if overlap_x < overlap_s * 0.6 {
                    // De lado: se empujan.
                    let push = overlap_x * dx.signum();
                    self.cars[a].x -= push * wa;
                    self.cars[b].x += push * wb;
                } else {
                    // Por atras: el de atras frena hasta la velocidad del de
                    // adelante y rebota un poco.
                    let (front, back) = if ds > 0.0 { (b, a) } else { (a, b) };
                    let vf = self.cars[front].v;
                    let back_car = &mut self.cars[back];
                    back_car.v = (vf - 3.0).max(0.0).min(back_car.v);
                    back_car.s -= overlap_s;
                    self.cars[front].v += 1.5;
                }
                // Las chispas del golpe, en el punto de contacto (a lo mas
                // una vez cada 0.4 s por pareja y cerca de la camara).
                let fresh = self.cars[a].bumped > 0.4 && self.cars[b].bumped > 0.4 && (self.cars[a].s - self.cars[0].s).abs() < 200.0;
                if player && self.cars[0].bumped > 0.4 {
                    self.cars[0].bumped = 0.0;
                    self.shake = 0.8;
                    self.message("¡Pum!", [255, 130, 100, 255]);
                    self.sound(Sound::Thud);
                }
                if fresh {
                    let (ca, cb) = (&self.cars[a], &self.cars[b]);
                    let s = (ca.s + cb.s) * 0.5;
                    let x = (ca.x + cb.x) * 0.5;
                    let v = (ca.v + cb.v) * 0.5;
                    let y = self.track.height(s) + 0.6;
                    self.cars[a].bumped = 0.0;
                    self.cars[b].bumped = 0.0;
                    self.spawn_sparks(40, (s, x, y), (v * 0.7, 0.0, 2.5), (5.0, 4.0, 2.0), Vec3::new(1.0, 0.8, 0.45) * 2.5, (0.3, 0.6), 0.35, 0.0);
                }
            }
        }
    }

    /// El rebufo: ir pegado atras de otra nave carga el turbo.
    fn draft(&mut self) {
        let n = self.cars.len();
        let mut player_drafting = false;
        for i in 0..n {
            if self.cars[i].role == Role::Traffic || self.cars[i].v < 20.0 {
                continue;
            }
            let (s, x) = (self.cars[i].s, self.cars[i].x);
            let behind = (0..n).any(|j| j != i && (4.0..28.0).contains(&(self.cars[j].s - s)) && (self.cars[j].x - x).abs() < 1.8);
            if behind {
                self.cars[i].turbo = (self.cars[i].turbo + 0.32 * STEP).min(1.0);
                player_drafting |= i == 0;
            }
        }
        self.drafting = player_drafting;
    }

    /// El trafico cambia de carril de vez en cuando (y si le pitan).
    fn traffic_lanes(&mut self) {
        for i in 0..self.cars.len() {
            if self.cars[i].role != Role::Traffic {
                continue;
            }
            self.cars[i].lane_timer -= STEP;
            if self.cars[i].lane_timer <= 0.0 {
                let h = hash_u32((self.clock * 1000.0) as u32 ^ i as u32 * 0x51ED);
                self.move_aside(i, h);
                self.cars[i].lane_timer = 5.0 + (h >> 8 & 0xFF) as f32 / 25.0;
            }
        }
    }

    /// Cambia el carril de la nave `i` a uno vecino libre.
    fn move_aside(&mut self, i: usize, h: u32) {
        let car = &self.cars[i];
        let lane = (((car.target + HALF_ROAD) / LANE).floor() as i32).clamp(0, 3);
        let mut options: Vec<i32> = [lane - 1, lane + 1].into_iter().filter(|l| (0..4).contains(l)).collect();
        if h & 1 == 1 {
            options.reverse();
        }
        let free = options.into_iter().find(|&l| {
            let x = -HALF_ROAD + LANE * (l as f32 + 0.5);
            !self.cars.iter().enumerate().any(|(j, o)| j != i && (o.s - car.s).abs() < 14.0 && (o.x - x).abs() < 2.5)
        });
        if let Some(l) = free {
            self.cars[i].target = -HALF_ROAD + LANE * (l as f32 + 0.5);
        }
    }

    /// El pito: el trafico de adelante en el carril se hace a un lado.
    fn honk(&mut self) {
        if self.horn_cooldown > 0.0 {
            return;
        }
        self.horn_cooldown = 0.8;
        self.sound(Sound::Horn);
        let (s, x) = (self.cars[0].s, self.cars[0].x);
        let mut moved = false;
        for i in 1..self.cars.len() {
            let o = &self.cars[i];
            if o.role == Role::Traffic && (0.0..80.0).contains(&(o.s - s)) && (o.x - x).abs() < 3.0 {
                self.move_aside(i, i as u32);
                self.cars[i].v += 3.0;
                moved = true;
            }
        }
        if moved {
            self.message("¡Abran cancha!", [180, 230, 255, 255]);
        }
    }

    /// El humo negro de las chimeneas de las camionetas que aceleran (cerca
    /// de la camara): un soplo cada 0.05 s (0.025 con turbo, a bocanadas),
    /// alternando chimenea, que sale con `0.8 v` a lo largo. El aire lo
    /// frena, `vs *= e^(-1.8 dt)`, y sube cada vez mas rapido,
    /// `y' = 0.9 + 0.6 edad` (el humo caliente). Vive 2.2 s.
    fn exhaust(&mut self) {
        let player_s = self.cars[0].s;
        for i in 0..self.cars.len() {
            let car = &mut self.cars[i];
            if car.kind != Kind::Camioneta || !car.throttle || car.v < 3.0 || !(-40.0..260.0).contains(&(car.s - player_s)) {
                continue;
            }
            car.smoke -= STEP;
            if car.smoke > 0.0 {
                continue;
            }
            car.smoke = if car.boost > 0.0 { 0.025 } else { 0.05 };
            let (s0, x0, v, own) = (car.s, car.x, car.v, i == 0);
            self.puff_count = self.puff_count.wrapping_add(1);
            let rig = &self.rigs[Kind::Camioneta.index()];
            let stack = if rig.stacks.is_empty() { Vec3::new(0.8, 2.4, 0.55 * Kind::Camioneta.length()) } else { rig.stacks[self.puff_count as usize % rig.stacks.len()] };
            let s = s0 - stack.z;
            let r = [self.random(), self.random(), self.random()];
            let puff = Puff { s, x: x0 + stack.x, y: self.track.height(s) + stack.y, age: 0.0, vs: v * 0.8, r0: 0.6 + 0.15 * r[0], rot: TAU * r[1], seed: 64.0 * r[2], own };
            if self.puffs.len() >= MAX_PUFFS {
                self.puffs.remove(0);
            }
            self.puffs.push(puff);
        }
        let drag = (-1.8 * STEP).exp();
        for puff in &mut self.puffs {
            puff.vs *= drag;
            puff.s += puff.vs * STEP;
            puff.y += (0.9 + 0.6 * puff.age) * STEP;
            puff.age += STEP;
        }
        self.puffs.retain(|p| p.age < PUFF_LIFE);
        // Si la camara pasa por el humo, se le nubla la vista: un soplo
        // denso (de menos de 1.6 s) que esta a menos de 2.5 m delante de la
        // camara y con el ojo adentro de su nucleo (`hypot(dx, dy) < 0.8
        // radio` en el corte de frente). (Midiendo solo a lo ancho y desde
        // la nave, el humo de la camioneta del carril de al lado nublaba la
        // vista entera.)
        let cam = self.camera();
        let dense = self.puffs.iter().any(|p| (0.0..2.5).contains(&(p.s - cam.s)) && (p.x - cam.x).hypot(p.y - cam.y) < 0.8 * p.radius() && p.age < 1.6);
        if dense && self.cars[0].kind != Kind::Camioneta {
            if self.smoke_screen < 0.3 {
                self.message("¡Humo negro!", [200, 200, 200, 255]);
            }
            self.smoke_screen = 1.0;
        }
    }
}

/// Las chimeneas y el escape de una nave, de su malla en metros (`Rig`):
///
/// - las chimeneas son las piezas de metal del 35 % de adelante que pasan
///   del 80 % del alto: de cada lado (por el signo de `x`) el vertice mas
///   alto y, si empatan, el mas afuera (la boca del tubo);
/// - el escape, el centro de los vertices de metal del 15 % de atras bajo
///   el 25 % del alto (el tubo bajo la defensa). Si no hay (la botella), el
///   centro de la cara de atras a un cuarto del alto.
fn rig_of(model: &Model) -> Rig {
    let mesh = model.mesh;
    let m = model.meters;
    let (lo, hi) = mesh.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    if mesh.positions.is_empty() {
        return Rig::default();
    }
    let length = (hi.z - lo.z) * m;
    let top = (hi.y - lo.y) * m;
    let mid_x = (lo.x + hi.x) * 0.5;
    let mid_z = (lo.z + hi.z) * 0.5;
    let to_m = |p: Vec3| Vec3::new((p.x - mid_x) * m, (p.y - lo.y) * m, (p.z - mid_z) * m);
    let metal = |i: usize| model.finish == Finish::Vehicle && mesh.uvs.get(i).is_some_and(|uv| Surface::from_code(uv.x) == Surface::Metal);
    let mut stacks = [None::<Vec3>; 2];
    let (mut sum, mut count) = (Vec3::ZERO, 0.0f32);
    for (i, &p) in mesh.positions.iter().enumerate() {
        if !metal(i) {
            continue;
        }
        let q = to_m(p);
        if q.z < -0.5 * length + 0.35 * length && q.y > 0.8 * top {
            let side = (q.x > 0.0) as usize;
            let better = stacks[side].is_none_or(|b| q.y > b.y + 1e-3 || ((q.y - b.y).abs() <= 1e-3 && q.x.abs() > b.x.abs()));
            if better {
                stacks[side] = Some(q);
            }
        }
        if q.y < 0.25 * top && q.z > 0.5 * length - 0.15 * length {
            sum += q;
            count += 1.0;
        }
    }
    let exhaust = if count > 0.0 { sum * (1.0 / count) } else { Vec3::new(0.0, 0.25 * top, 0.5 * length) };
    Rig { stacks: stacks.into_iter().flatten().collect(), exhaust }
}

/// Donde va la camara: cuanto atras y cuanto arriba de la nave.
fn camera_offset(spec: Spec) -> (f32, f32) {
    (4.5 + 0.55 * spec.length, 1.6 + 0.75 * spec.height)
}

/// Un numero de tiempo como 1:02.35.
fn clock(t: f32) -> String {
    let t = t.max(0.0);
    format!("{}:{:05.2}", (t / 60.0) as u32, t % 60.0)
}

fn ordinal(n: usize) -> String {
    format!("{n}º")
}

impl Minigame for Carrera {
    fn demo_start(&mut self) {
        self.set_autopilot(true);
        self.start();
    }

    /// Se corre con la musica del vuelo (K, U / I y - / + siguen sirviendo).
    fn keeps_music(&self) -> bool {
        true
    }

    fn update(&mut self, dt: f64, keys: &Keys) {
        let dt = (dt as f32).min(0.1);
        self.wall += dt;
        self.frame_dt += (dt - self.frame_dt) * (1.0 - (-4.0 * dt).exp());
        self.steer_in = keys.steer;
        self.throttle_in = keys.throttle;
        self.brake_in = keys.brake;
        match self.phase {
            Phase::Ready => {
                if keys.escape {
                    self.done = true;
                } else if keys.enter {
                    self.start();
                }
            }
            Phase::Countdown(t) => {
                let next = t + dt;
                // Tres rojas, un segundo cada una, y la verde.
                for beat in 0..4 {
                    let at = beat as f32;
                    if t < at && next >= at {
                        self.sound(if beat == 3 { Sound::Beep(880.0, 0.45) } else { Sound::Beep(440.0, 0.15) });
                    }
                }
                self.phase = if next >= 3.0 { Phase::Racing } else { Phase::Countdown(next) };
                if self.phase == Phase::Racing {
                    // Con el verde, el Fuego da la salida y revientan seis
                    // cohetes alrededor de la torre.
                    self.eruption.next = self.wall;
                    self.green_at = Some(self.wall);
                    self.volley(6, 0.4, 0.12, 0.42, (0.12, 0.3));
                    self.next_burst = self.wall + 5.0;
                    self.message("¡Dale!", [140, 255, 150, 255]);
                    // Salida perfecta: acelerando justo en el verde.
                    if keys.throttle && t > 2.6 {
                        self.cars[0].turbo = 0.6;
                        self.message("¡Salida perfecta!", [255, 214, 120, 255]);
                    }
                }
                if keys.escape {
                    self.phase = Phase::Ready;
                }
            }
            Phase::Racing | Phase::Finished(_) => {
                if keys.escape {
                    self.phase = Phase::Ready;
                    self.reset();
                    return;
                }
                if keys.horn {
                    self.honk();
                }
                if keys.boost && self.cars[0].turbo > 0.3 && self.cars[0].finished.is_none() {
                    let car = &mut self.cars[0];
                    car.boost = 1.0 + 2.2 * car.turbo;
                    car.turbo = 0.0;
                    self.message("¡Turbo!", [140, 220, 255, 255]);
                    self.sound(Sound::Whoosh);
                }
                if let Phase::Finished(t) = self.phase {
                    self.phase = Phase::Finished(t + dt);
                    if keys.enter && t > 1.0 {
                        self.start();
                        return;
                    }
                }
            }
        }
        if matches!(self.phase, Phase::Racing | Phase::Finished(_)) {
            self.clock += dt;
        }
        let length = self.track.length();
        let lap_before = self.cars[0].lap(length);
        self.accumulator += dt;
        while self.accumulator >= STEP {
            self.physics();
            self.accumulator -= STEP;
        }
        self.fireworks(lap_before < self.cars[0].lap(length) && self.cars[0].s > 1.0);
        if self.phase == Phase::Racing && self.cars[0].finished.is_some() {
            self.phase = Phase::Finished(0.0);
            let place = self.place();
            if place == 0 {
                self.message("¡Ganaste!", [255, 214, 120, 255]);
            }
            let time = self.cars[0].finished.unwrap_or(self.clock);
            let key = self.player_kind.record();
            let best = self.records.get(key);
            if best.is_none_or(|b| ((time * 1000.0) as i64) < b) {
                self.records.set(key, (time * 1000.0) as i64);
                self.new_record = true;
            }
        }
        // Lo que se ve y se oye.
        self.sky_update(dt);
        self.shake = (self.shake - dt * 2.5).max(0.0);
        self.smoke_screen = (self.smoke_screen - dt * 1.1).max(0.0);
        for scrape in &mut self.scrape {
            scrape.1 = (scrape.1 - 2.5 * dt).max(0.0);
        }
        self.horn_cooldown = (self.horn_cooldown - dt).max(0.0);
        // La camara se abre con la velocidad y mas con el turbo: un resorte
        // amortiguado hacia `0.10 (v / top)^2 + 0.12 turbo`, con
        // `fov'' = 60 (meta - fov) - 9 fov'`: subamortiguado, con
        // `zeta = 9 / (2 sqrt 60) ~ 0.58`, se pasa ~11 % y llega en ~0.5 s
        // (se siente como un empujon). Euler semi-implicito: primero la
        // velocidad y con ella la posicion.
        let (v, top, boosting) = (self.cars[0].v, self.cars[0].kind.spec().top, self.cars[0].boost > 0.0);
        let target = 0.10 * (v / top).clamp(0.0, 1.3).powi(2) + if boosting { 0.12 } else { 0.0 };
        (self.fov, self.fov_v) = fov_spring(self.fov, self.fov_v, target, dt);
        self.boost_s = if boosting { (self.boost_s + 4.0 * dt).min(1.0) } else { (self.boost_s - 2.0 * dt).max(0.0) };
        for m in &mut self.messages {
            m.age += dt;
        }
        self.messages.retain(|m| m.age < 1.4);
        if self.drafting && self.phase == Phase::Racing {
            self.message("Rebufo: cargando turbo", [150, 210, 255, 220]);
        }
        let player = &self.cars[0];
        let spec = player.kind.spec();
        let speed = (player.v / spec.top).clamp(0.0, 1.3);
        // Cinco marchas: las vueltas del motor suben en cada una y caen al
        // cambiar.
        let gear = (speed * 5.0).min(4.999);
        self.synth.rpm = 0.25 + 0.75 * gear.fract().max(if gear < 1.0 { 0.0 } else { 0.25 });
        self.synth.throttle = if self.throttle_in || self.autopilot { 1.0 } else { 0.35 };
        self.synth.speed = speed;
    }

    fn audio(&mut self, out: &mut [f32]) {
        let kind = self.player_kind;
        let (base, drive) = match kind {
            Kind::TukTuk => (62.0, 260.0),
            Kind::Camioneta => (30.0, 95.0),
            Kind::Colectivo => (44.0, 170.0),
            Kind::Chela => (52.0, 210.0),
        };
        let synth = &mut self.synth;
        let dt = 1.0 / RATE;
        for sample in out.iter_mut() {
            synth.rpm_now += (synth.rpm - synth.rpm_now) * 0.0015;
            let freq = base + drive * synth.rpm_now;
            synth.phase = (synth.phase + freq * dt).fract();
            synth.sub = (synth.sub + freq * 0.5 * dt).fract();
            let mut x = synth.noise;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            synth.noise = x;
            let white = (x >> 8) as f32 / 8_388_608.0 - 1.0;
            // El motor: cada clase con su timbre.
            let engine = match kind {
                // Dos tiempos: pulsos angostos con zumbido.
                Kind::TukTuk => (if synth.phase < 0.22 { 1.0 } else { -0.28 }) + 0.25 * white,
                // Diesel: diente de sierra grave y un sub.
                Kind::Camioneta => (2.0 * synth.phase - 1.0) * 0.8 + (if synth.sub < 0.5 { 0.5 } else { -0.5 }) + 0.2 * white,
                Kind::Colectivo => (2.0 * synth.phase - 1.0) * 0.9 + 0.3 * (synth.sub * TAU).sin(),
                // La chela: el motor con burbujas.
                Kind::Chela => (2.0 * synth.phase - 1.0) * 0.7 + if white > 0.985 { 2.5 } else { 0.0 },
            };
            synth.low += (engine - synth.low) * 0.12;
            let level = 0.07 + 0.09 * synth.throttle;
            // El viento sube con la velocidad.
            synth.wind += (white - synth.wind) * (0.01 + 0.08 * synth.speed);
            let mut mix = synth.low * level + synth.wind * 0.25 * synth.speed * synth.speed;
            // La envolvente de los pum de los cohetes: su ruido sale de un
            // solo pasabajos compartido.
            let mut boom = 0.0;
            for (sound, age) in &mut synth.events {
                let t = *age;
                mix += match *sound {
                    Sound::Thud => ((t * 55.0 * TAU).sin() * 0.8 + white * 0.5) * (-t * 14.0).exp(),
                    Sound::Bump => {
                        let hit = |t0: f32| if t >= t0 { ((t - t0) * 70.0 * TAU).sin() * (-(t - t0) * 18.0).exp() } else { 0.0 };
                        (hit(0.0) + hit(0.12)) * 0.7
                    }
                    Sound::Horn => {
                        let (a, b) = match kind {
                            Kind::Camioneta => (330.0, 415.0),
                            Kind::TukTuk => (610.0, 640.0),
                            Kind::Colectivo => (440.0, 554.0),
                            Kind::Chela => (523.0, 659.0),
                        };
                        let f = if t < 0.18 || t > 0.3 { a } else { b };
                        let square = |f: f32| if (t * f).fract() < 0.5 { 1.0 } else { -1.0 };
                        (square(f) * 0.12 + square(f * 1.5) * 0.05) * smoothstep(0.55, 0.45, t)
                    }
                    Sound::Beep(f, len) => (t * f * TAU).sin() * 0.2 * smoothstep(len, len - 0.03, t),
                    Sound::Whoosh => white * 0.35 * (t * 4.0).min(1.0) * (-t * 2.2).exp(),
                    // Ruido grave que cae en 0.4 s (`e^(-t / 0.4)`) y un
                    // golpe de 48 Hz; la palmera truena otra vez 0.22 s
                    // despues.
                    Sound::Pum(delay, double) => {
                        let hit = |t0: f32| if t >= t0 { (-(t - t0) / 0.4).exp() } else { 0.0 };
                        let thump = |t0: f32| if t >= t0 { ((t - t0) * 48.0 * TAU).sin() * (-(t - t0) * 9.0).exp() } else { 0.0 };
                        let second = if double { 0.8 } else { 0.0 };
                        boom += hit(delay) + second * hit(delay + 0.22);
                        (thump(delay) + second * thump(delay + 0.22)) * 0.3
                    }
                };
                *age += dt;
            }
            // Un polo en ~170 Hz: `y += (x - y) 0.025` a 44.1 kHz.
            synth.boom += (white * boom.min(2.0) - synth.boom) * 0.025;
            mix += synth.boom * 2.2;
            *sample = mix / (1.0 + mix.abs());
        }
        synth.events.retain(|(sound, age)| *age < if matches!(sound, Sound::Pum(..)) { 4.0 } else { 1.2 });
    }

    /// Las filas en paralelo (con la emision de cada pixel en su alfa) y
    /// despues el posproceso: el brillo, el acabado, la velocidad y el
    /// humo, con el mapa encima.
    fn draw(&mut self, rgba: &mut [u8]) {
        // Las pasadas del minijuego van con 3/4 de los hilos (ver `parallel::light`).
        let _light = crate::parallel::light();
        // Las filas, y al terminar cada una sus sumas para el brillo.
        let mut sums = self.post.take_sums();
        let hud = {
            let view = self.view();
            sums.paint(rgba, |y, row| view.row(y, row));
            view.into_hud()
        };
        let player = &self.cars[0];
        let params = posproceso::Params {
            boost_s: self.boost_s,
            speed: player.v / player.kind.spec().top,
            smoke: self.smoke_screen,
            wall: self.wall,
            ..Default::default()
        };
        self.post.prepare(sums, &params);
        let game = &*self;
        game.post.compose(rgba, &params, |y, row| hud.row(game, y, row));
    }

    fn overlay(&self) -> Overlay {
        let mut o = Overlay::default();
        let s = self.height as f32 / 720.0;
        let (w, h) = (self.width as f32, self.height as f32);
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 210];
        let accent = [255, 204, 120, 255];
        let player = self.player();
        let length = self.track.length();
        match self.phase {
            Phase::Ready => {
                let (pw, ph) = (680.0 * s, 392.0 * s);
                let (px, py) = ((w - pw) * 0.5, 44.0 * s);
                o.card(px, py, pw, ph);
                o.sign("La carrera del Periférico", w * 0.5, py + 42.0 * s, 52.0 * s, Paint::Sun, Align::Center);
                o.text("Tres vueltas en órbita sobre la capital, de noche", w * 0.5, py + 108.0 * s, Font::Body, dim, Align::Center);
                let mut y = py + 146.0 * s;
                for car in self.cars.iter().filter(|c| c.role != Role::Traffic) {
                    let mine = car.role == Role::Player;
                    if mine {
                        o.bar(px + 22.0 * s, y - 5.0 * s, pw - 44.0 * s, 46.0 * s, 1.0, [250, 200, 70, 36], false);
                    }
                    let who = if mine { format!("Vos · {}", self.player_name()) } else { car.name.to_string() };
                    o.text(who, px + 40.0 * s, y, Font::Bold, if mine { accent } else { cream }, Align::Left);
                    o.text(car.kind.blurb(), px + 40.0 * s, y + 20.0 * s, Font::Small, dim, Align::Left);
                    y += 46.0 * s;
                }
                let record = match self.records.get(self.player_kind.record()) {
                    Some(ms) => format!("Tu récord con esta nave: {}", clock(ms as f32 / 1000.0)),
                    None => "Sin récord con esta nave: estrenalo".to_string(),
                };
                o.text(record, w * 0.5, py + ph - 66.0 * s, Font::Bold, accent, Align::Center);
                o.keys("[W][A][S][D] manejar   [Shift] turbo   [H] pito   [Enter] arrancar   [Esc] salir", w * 0.5, py + ph - 40.0 * s, Align::Center);
            }
            Phase::Countdown(t) => {
                let text = match t as u32 {
                    0 => "3",
                    1 => "2",
                    _ => "1",
                };
                let pulse = 1.0 - t.fract();
                o.sign(text, w * 0.5, h * 0.17, (120.0 + 40.0 * pulse) * s, Paint::Rose, Align::Center);
            }
            _ => {}
        }
        if matches!(self.phase, Phase::Racing | Phase::Finished(_)) || matches!(self.phase, Phase::Countdown(_)) {
            // La posicion, la vuelta y el tiempo, arriba a la izquierda.
            o.panel(16.0 * s, 16.0 * s, 236.0 * s, 136.0 * s);
            let place = self.place() + 1;
            o.sign(ordinal(place), 34.0 * s, 30.0 * s, 62.0 * s, if place == 1 { Paint::Sun } else { Paint::Cream }, Align::Left);
            o.text("de 4", 122.0 * s, 62.0 * s, Font::Bold, dim, Align::Left);
            let lap = (player.lap(length) + 1).min(LAPS);
            o.text(format!("Vuelta {lap} de {LAPS}"), 34.0 * s, 98.0 * s, Font::Bold, cream, Align::Left);
            let best = player.best_lap.map_or(String::new(), |b| format!("   ·   mejor {}", clock(b)));
            o.text(format!("{}{best}", clock(self.clock)), 34.0 * s, 120.0 * s, Font::Small, dim, Align::Left);
            // El velocimetro y el turbo, arriba a la derecha.
            let boosting = player.boost > 0.0;
            o.gauge(w - 100.0 * s, 100.0 * s, 168.0 * s, player.v * 3.6, 340.0, "km/h", boosting);
            let turbo_color = if boosting { [140, 220, 255, 255] } else if player.turbo > 0.3 { [250, 200, 70, 255] } else { [200, 190, 176, 255] };
            o.keys("[Shift] turbo", w - 100.0 * s, 192.0 * s, Align::Center);
            o.bar(w - 170.0 * s, 222.0 * s, 140.0 * s, 9.0 * s, if boosting { 1.0 } else { player.turbo }, turbo_color, false);
            // Los nombres de las rivales sobre sus naves, si no se enciman con
            // los paneles, el aviso u otro nombre (gana la mas cercana).
            let mut taken = vec![(0.0, 0.0, 268.0 * s, 168.0 * s), (w - 210.0 * s, 0.0, w, 240.0 * s)];
            if let Some(m) = self.messages.last() {
                let half = m.text.chars().count() as f32 * 11.0 * s;
                taken.push((w * 0.5 - half, h * 0.27 - 8.0 * s, w * 0.5 + half, h * 0.27 + 50.0 * s));
            }
            let mut labels: Vec<(f32, String, f32, f32)> = self
                .standings()
                .iter()
                .enumerate()
                .filter(|&(_, &i)| self.cars[i].role == Role::Rival)
                .filter_map(|(place, &i)| {
                    let car = &self.cars[i];
                    let (x, y) = self.label_at(car)?;
                    Some((car.s - player.s, format!("{} {}", ordinal(place + 1), car.name), x, y))
                })
                .collect();
            labels.sort_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
            for (_, text, x, y) in labels {
                let half = text.chars().count() as f32 * 3.8 * s + 4.0 * s;
                let rect = (x - half, y - 2.0 * s, x + half, y + 20.0 * s);
                if taken.iter().any(|t| rect.0 < t.2 && rect.2 > t.0 && rect.1 < t.3 && rect.3 > t.1) {
                    continue;
                }
                taken.push(rect);
                o.text(text, x, y, Font::Small, [255, 240, 220, 230], Align::Center);
            }
        }
        // El aviso, al centro (no encima de los resultados): solo el ultimo
        // (varios a la vez se apilaban y se mezclaban).
        let results = matches!(self.phase, Phase::Finished(t) if t > 1.0);
        if let Some(m) = self.messages.last().filter(|_| !results) {
            let alpha = (1.0 - (m.age / 1.4).powi(3)).clamp(0.0, 1.0);
            let mut color = m.color;
            color[3] = (color[3] as f32 * alpha) as u8;
            o.text(m.text, w * 0.5, h * 0.27 - m.age * 10.0 * s, Font::Big(32.0 * s), color, Align::Center);
        }
        if let Phase::Finished(t) = self.phase {
            if t > 1.0 {
                self.results(&mut o);
            }
        }
        if matches!(self.phase, Phase::Racing) {
            o.keys("[Esc] salir de la carrera", w - 16.0 * s, h - 34.0 * s, Align::Right);
        }
        o
    }

    fn done(&self) -> bool {
        self.done
    }
}

impl Carrera {
    fn player_name(&self) -> &'static str {
        self.names[self.player_kind.index()]
    }

    /// El Fuego y las estrellas fugaces: corren con el reloj de pared, en
    /// todas las fases.
    fn sky_update(&mut self, dt: f32) {
        let cam = self.camera();
        let limb = cam.limb();
        let wall = self.wall;
        let e = &mut self.eruption;
        e.flash *= (-dt / 0.4).exp();
        e.glow = 0.6 + 0.2 * value_noise(wall * 3.0, 0.0) + 3.0 * e.flash;
        if wall >= e.next {
            e.explode(wall, limb);
        }
        for f in &mut e.flows {
            *f = (*f - 0.02 * dt).max(0.35);
        }
        // Las bombas: tiro parabolico en (rumbo, elevacion), hasta que caen
        // en la ladera. Se avanza con la parabola exacta,
        // `p += v dt - g dt^2 / 2` y `v -= g dt`, que da la misma trayectoria
        // a cualquier fps (y la misma que usa la estela en `view`).
        // Las que todavia no salen (edad negativa) esperan en el crater.
        for b in &mut e.bombs {
            let flight = if b.age >= 0.0 { dt } else { (b.age + dt).max(0.0) };
            b.age += dt;
            b.psi += b.vpsi * flight;
            b.p += b.vp * flight - 0.5 * BOMB_GRAVITY * flight * flight;
            b.vp -= BOMB_GRAVITY * flight;
        }
        e.bombs.retain(|b| b.age < 4.0 && (b.vp > 0.0 || b.p > volcano_profile(b.psi).0 - limb));
        // Una estrella fugaz cada 5 a 15 s, enfrente mas o menos.
        if self.meteor.is_some_and(|m| wall - m.born > m.dur) {
            self.meteor = None;
        }
        if self.meteor.is_none() && wall > self.next_meteor {
            let h = hash_u32(self.next_meteor.to_bits() ^ 0x5747);
            let r = |k: u32| unit(hash_u32(h ^ k.wrapping_mul(0x9E37_79B9)));
            let len = 0.14 + 0.12 * r(4);
            let side = if r(5) < 0.5 { -1.0 } else { 1.0 };
            self.meteor = Some(Meteor {
                born: wall,
                yaw: cam.heading + (r(1) - 0.5) * 1.4,
                pitch: 0.15 + 0.25 * r(2),
                dyaw: side * len * 0.8,
                dpitch: -len * 0.6,
                dur: 0.4 + 0.4 * r(3),
            });
            self.next_meteor = wall + 5.0 + 10.0 * r(6);
        }
    }

    /// Los cohetes: uno de ambiente cada 4 a 9 s durante la carrera (a
    /// +-0.7 rad del rumbo y de 0.06 a 0.3 rad de alto), cuatro cuando
    /// quien juega cruza la meta, y se van los que ya se apagaron.
    fn fireworks(&mut self, lap_done: bool) {
        let wall = self.wall;
        self.bursts.retain(|b| b.age(wall) < BURST_LIFE);
        if lap_done {
            self.volley(4, 0.25, 0.3, 0.5, (0.1, 0.28));
        }
        if self.phase == Phase::Racing && wall >= self.next_burst {
            let h = hash_u32(self.next_burst.to_bits() ^ 0xF17E);
            let r = |k: u32| unit(hash_u32(h ^ k.wrapping_mul(0x9E37_79B9)));
            let yaw = self.camera().heading + (r(1) * 2.0 - 1.0) * 0.7;
            self.spawn_burst(yaw, 0.06 + 0.24 * r(2), (r(3) * 5.0) as u8 % 5, RISE);
            self.next_burst = wall + 4.0 + 5.0 * r(4);
        }
    }

    /// La luz que cae de los arcos de foquitos en el punto `s` de la pista
    /// (m, contando vueltas): el color del arco mas cercano por
    /// `e^(-ds^2 / 20)` (unos 4.5 m a cada lado). Cero lejos de los
    /// tuneles. El color pasa al siguiente de la paleta tres veces por
    /// segundo con un fundido, `smoothstep(fract(3 t))` en lo que dura cada
    /// uno, y no de golpe (si no la pista y las naves parpadean).
    fn arch_spill(&self, s: f32) -> Vec3 {
        // (Los arcos se cuentan en cada vuelta y no hay cerca de la meta:
        // basta el mas cercano dentro de la vuelta.)
        let length = self.track.length();
        let local = s.rem_euclid(length);
        let n = ((local - ARCH_OFFSET) / ARCH_EVERY).round();
        let at = n * ARCH_EVERY + ARCH_OFFSET;
        let ds = local - at;
        if !arches_at(at) {
            return Vec3::ZERO;
        }
        // Los de junto a la meta con el eco del semaforo.
        let signal = if at.min(length - at) < 48.0 { self.arch_signal() } else { 0 };
        if signal > 0 {
            return if signal == 1 { Vec3::new(1.0, 0.1, 0.08) } else { Vec3::new(0.2, 1.0, 0.35) } * fast_exp(-ds * ds / 20.0);
        }
        let phase = self.wall * 3.0;
        let k = (2.0 * n + phase.floor()) as i64;
        let (a, b) = (ARCH_PAL[k.rem_euclid(5) as usize], ARCH_PAL[(k + 1).rem_euclid(5) as usize]);
        a.lerp(b, smoothstep(0.0, 1.0, fract(phase))) * fast_exp(-ds * ds / 20.0)
    }

    /// El eco del semaforo en los arcos de junto a la meta: 1 rojo con la
    /// cuenta, 2 verde el primer segundo y medio del verde, 0 si no.
    fn arch_signal(&self) -> u8 {
        match self.phase {
            Phase::Countdown(_) => 1,
            _ if self.green_at.is_some_and(|g| self.wall - g < 1.5) => 2,
            _ => 0,
        }
    }

    fn results(&self, o: &mut Overlay) {
        let s = self.height as f32 / 720.0;
        let w = self.width as f32;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 210];
        let accent = [255, 204, 120, 255];
        let (pw, ph) = (540.0 * s, 372.0 * s);
        let (px, py) = ((w - pw) * 0.5, 130.0 * s);
        o.card(px, py, pw, ph);
        let place = self.place();
        let (headline, paint) = match place {
            0 => ("¡Ganaste!", Paint::Sun),
            1 => ("Segundo lugar", Paint::Cream),
            2 => ("Tercer lugar", Paint::Cream),
            _ => ("Último… ¡a la otra!", Paint::Rose),
        };
        o.sign(headline, w * 0.5, py + 42.0 * s, 50.0 * s, paint, Align::Center);
        let mut y = py + 118.0 * s;
        for (k, &i) in self.standings().iter().enumerate() {
            let car = &self.cars[i];
            let time = match car.finished {
                Some(t) => clock(t),
                // Lo que le falta, a su velocidad de ahora.
                None => {
                    let left = LAPS as f32 * self.track.length() - car.s;
                    format!("~{}", clock(self.clock + left / car.v.max(10.0)))
                }
            };
            let mine = car.role == Role::Player;
            if mine {
                o.bar(px + 24.0 * s, y - 4.0 * s, pw - 48.0 * s, 30.0 * s, 1.0, [250, 200, 70, 40], false);
            }
            let name = if mine { "Vos" } else { car.name };
            let color = if mine { accent } else { cream };
            o.text(format!("{}  {name}", ordinal(k + 1)), px + 44.0 * s, y, Font::Bold, color, Align::Left);
            o.text(time, px + pw - 44.0 * s, y, Font::Bold, color, Align::Right);
            y += 34.0 * s;
        }
        let record = if self.new_record {
            "¡Récord nuevo con esta nave!".to_string()
        } else {
            match self.records.get(self.player_kind.record()) {
                Some(ms) => format!("Récord con esta nave: {}", clock(ms as f32 / 1000.0)),
                None => String::new(),
            }
        };
        o.text(record, w * 0.5, py + ph - 78.0 * s, Font::Bold, if self.new_record { accent } else { dim }, Align::Center);
        o.keys("[Enter] otra carrera   [Esc] salir", w * 0.5, py + ph - 44.0 * s, Align::Center);
    }

    /// Donde va el nombre de una nave en la pantalla (arriba de ella), si se
    /// ve y no esta muy lejos.
    fn label_at(&self, car: &Car) -> Option<(f32, f32)> {
        let cam = self.camera();
        let d = car.s - cam.s;
        if !(6.0..160.0).contains(&d) {
            return None;
        }
        let (x, _theta) = self.lateral(cam.s, d);
        let y = self.track.height(car.s) + car.kind.spec().height + 1.2;
        let sx = self.width as f32 * 0.5 + (car.x + x - cam.x) * cam.focal / d;
        let sy = cam.horizon - (y - cam.y) * cam.focal / d;
        (sx > 0.0 && sx < self.width as f32 && sy > 0.0).then_some((sx, sy - 18.0))
    }

    /// Corrimiento lateral y rumbo de la pista a `d` metros adelante de la
    /// camara, relativos a ella.
    fn lateral(&self, from: f32, d: f32) -> (f32, f32) {
        let mut x = 0.0;
        let mut theta = 0.0;
        let mut s = from;
        let end = from + d;
        while s < end {
            let step = (end - s).min(SEGMENT);
            let curve = self.track.curvature(s) * step;
            x += step * (theta + curve * 0.5);
            theta += curve;
            s += step;
        }
        (x, theta)
    }

    /// La camara: atras y arriba de quien juega.
    fn camera(&self) -> Camera {
        let player = self.player();
        let spec = player.kind.spec();
        let (back, up) = camera_offset(spec);
        let s = player.s - back;
        let shake = self.shake * 0.25 * ((self.wall * 37.0).sin() + (self.wall * 23.0).cos());
        let x = player.x * 0.75;
        let y = self.track.height(s).max(self.track.height(player.s) - 1.0) + up + shake;
        let heading = self.track.heading(s);
        // En el plano: el centro de la pista y, a la derecha del rumbo
        // (`(cos h, -sin h)`), el corrimiento de la camara.
        let w = self.track.world_at(s);
        let (sin_h, cos_h) = heading.sin_cos();
        Camera {
            s,
            x,
            y,
            // Con mas apertura la distancia focal se acorta: lo de los lados
            // corre mas rapido hacia afuera.
            focal: FOCAL * self.height as f32 / (1.0 + self.fov),
            horizon: self.height as f32 * HORIZON + shake * 4.0,
            heading,
            world: (w.0 + x * cos_h, w.1 - x * sin_h),
            alt: ALTITUDE + y,
        }
    }

    /// Todo lo que se dibuja en este frame: las filas de la pista y lo que
    /// va encima, ordenado de lejos a cerca.
    fn view(&self) -> View<'_> {
        let cam = self.camera();
        let (w, h) = (self.width, self.height);
        let cx = w as f32 * 0.5;
        let mut rows: Vec<Option<RoadRow>> = vec![None; h];
        // La segunda capa: el tramo mas cercano de los que caen en filas ya
        // cubiertas (detras de una loma, o una curva que vuelve a bajar en
        // la pantalla). Se ve solo donde la pista de adelante no lo tapa.
        let mut far: Vec<Option<RoadRow>> = vec![None; h];
        // Los bordes de los tramos, de cerca a lejos.
        let base = (cam.s / SEGMENT).floor() * SEGMENT;
        let mut edges = Vec::with_capacity(DRAW + 1);
        let mut x = 0.0f32;
        let mut theta = -self.track.curvature(cam.s) * (cam.s - base);
        for k in 0..=DRAW {
            let s = base + k as f32 * SEGMENT;
            let d = s - cam.s;
            edges.push((s, d, x, theta, self.track.height(s)));
            let curve = self.track.segment(s).curve;
            x += SEGMENT * (theta + curve * 0.5);
            theta += curve;
        }
        // Las filas: cada tramo cubre de su borde lejano al cercano, solo lo
        // que queda abajo de lo ya cubierto (lo de mas arriba lo tapa una
        // loma).
        let mut top = h as f32;
        for k in 0..DRAW {
            let (s0, d0, x0, _, y0) = edges[k];
            let (_, d1, x1, _, y1) = edges[k + 1];
            if d1 <= 0.5 {
                continue;
            }
            let d0 = d0.max(0.5);
            let project = |d: f32, x: f32, y: f32| (cx + (x - cam.x) * cam.focal / d, cam.horizon - (y - cam.y) * cam.focal / d);
            let (sx0, sy0) = project(d0, x0, y0);
            let (sx1, sy1) = project(d1, x1, y1);
            if (sy1 - sy0).abs() < 1e-3 {
                continue;
            }
            let row_at = |y: i32| {
                let yf = y as f32 + 0.5;
                let t = ((sy0 - yf) / (sy0 - sy1)).clamp(0.0, 1.0);
                // En perspectiva se interpola 1 / z.
                let inv = (1.0 / d0) * (1.0 - t) + (1.0 / d1) * t;
                let d = 1.0 / inv;
                let u = (d - d0) / (d1 - d0);
                RoadRow { center: sx0 + (sx1 - sx0) * t, per_meter: cam.focal / d, depth: d, s: s0 + u * SEGMENT, ds_dy: 0.0, cut: false }
            };
            if sy1 > sy0 {
                // Un tramo que baja en la pantalla al alejarse: la pista cae
                // detras de una loma mas empinado que la vista. Arriba no se
                // ve, pero en una curva su cinta asoma al lado de la de
                // adelante: va a la segunda capa, asi la pista sigue y los
                // postes no quedan flotando.
                for y in (sy0.max(top).ceil() as i32).max(0)..(sy1.ceil() as i32).min(h as i32) {
                    if far[y as usize].is_none() {
                        far[y as usize] = Some(row_at(y));
                    }
                }
                continue;
            }
            let from = sy1.max(0.0).ceil() as i32;
            let to = sy0.min(top).ceil() as i32;
            for y in from..to.min(h as i32) {
                rows[y as usize] = Some(row_at(y));
            }
            for y in from.max(to)..(sy0.ceil() as i32).min(h as i32) {
                if far[y as usize].is_none() {
                    far[y as usize] = Some(row_at(y));
                }
            }
            top = top.min(sy1);
        }
        // Cuantos metros de pista abarca cada fila (`ds_dy`): la resta con
        // la de arriba, o si no sigue la misma pista (no hay, o el salto
        // pasa de 20 m: un corte en una cresta) con la de abajo. Si ninguna
        // sirve, el de un suelo plano a `hc` metros bajo la camara: de
        // `y = f hc / d`, `|ds / dy| = d^2 / (f hc)`, y la fila queda
        // marcada como corte.
        for layer in [&mut rows, &mut far] {
            for y in 0..h {
                let Some(r) = layer[y] else { continue };
                let step = |n: Option<RoadRow>| n.map(|n| (n.s - r.s).abs()).filter(|&ds| ds <= 20.0);
                let up = if y > 0 { step(layer[y - 1]) } else { None };
                let down = if y + 1 < h { step(layer[y + 1]) } else { None };
                let (ds, cut) = match up.or(down) {
                    Some(ds) => (ds, false),
                    None => {
                        let hc = (cam.y - self.track.height(r.s)).max(0.5);
                        (r.depth * r.depth / (cam.focal * hc), true)
                    }
                };
                if let Some(r) = layer[y].as_mut() {
                    r.ds_dy = ds;
                    r.cut = cut;
                }
            }
        }
        // Lo que va encima.
        let mut items = Vec::new();
        let at = |d: f32| -> Option<(f32, f32, f32)> {
            // Corrimiento, altura de la pista y rumbo a d metros (entre bordes).
            if d <= 0.5 {
                return None;
            }
            let k = ((d + cam.s - base) / SEGMENT).floor();
            if k < 0.0 || k as usize >= DRAW {
                return None;
            }
            let (s0, _, x0, th0, _) = edges[k as usize];
            let local = cam.s + d - s0;
            let curve = self.track.curvature(s0);
            Some((x0 + local * (th0 + curve * local * 0.5), self.track.height(cam.s + d), th0 + curve * local))
        };
        // Algo a `d` metros, `lateral` del centro de la pista y a la altura
        // `y` (m), con su punto en el suelo en la pantalla.
        let place = |d: f32, kind: ItemKind, lateral: f32, x: f32, y: f32| Item::new(d, kind, cx + (x + lateral - cam.x) * cam.focal / d, cam.horizon - (y - cam.y) * cam.focal / d, cam.focal / d);
        // Los postes.
        let length = self.track.length();
        let first = lamp_after(cam.s, length);
        for n in first..first + 64 {
            let (s, side) = lamp(n, length);
            let d = s - cam.s;
            if d > DRAW as f32 * SEGMENT {
                break;
            }
            if let Some((x, y, _)) = at(d) {
                items.push(place(d, ItemKind::Lamp { side }, side * (RAIL + 0.7), x, y));
            }
        }
        // Los arcos de foquitos (desde 1.5 m: mas cerca taparian la vista
        // un frame; y no los que la neblina ya borro). Cada arco esta de
        // frente a la pista; la camara lo ve con el angulo `rel` entre el
        // rumbo de la pista alli y su rayo, `atan(x / d)`: a lo ancho se
        // angosta `max(|cos rel|, 0.15)` (de canto es una elipse angosta y
        // no un circulo entero). Los de junto a la meta (a menos de 48 m)
        // hacen eco del semaforo con la cuenta y el primer segundo y medio
        // del verde: todos sus focos rojos o verdes, asi no revuelven la
        // senal con sus colores.
        let signal = self.arch_signal();
        let lap = (cam.s / length).floor();
        for base_s in [lap * length, (lap + 1.0) * length] {
            let mut k = (((cam.s - base_s - ARCH_OFFSET) / ARCH_EVERY).floor() as i64 + 1).max(0);
            loop {
                let local = k as f32 * ARCH_EVERY + ARCH_OFFSET;
                let d = base_s + local - cam.s;
                if local >= length || d > DRAW as f32 * SEGMENT {
                    break;
                }
                if d > 1.5 && fog_at(d) < 0.98 && arches_at(local) {
                    if let Some((x, y, theta)) = at(d) {
                        let rel = theta - ((x - cam.x) / d).atan();
                        let squash = rel.cos().abs().max(0.15);
                        let signal = if local.min(length - local) < 48.0 { signal } else { 0 };
                        items.push(place(d, ItemKind::Arch { id: k.rem_euclid(5) as u32, squash, signal }, 0.0, x, y));
                    }
                }
                k += 1;
            }
        }
        // Los barriletes de esta vuelta y de la siguiente. Puestos de canto:
        // miran `0.45 rad` hacia la pista, y la camara los ve con el angulo
        // `atan(x / d)`; se angostan con `0.35 + 0.65 |cos(rel)|`.
        let lap = (cam.s / length).floor();
        for base_s in [lap * length, (lap + 1.0) * length] {
            for spot in &self.kite_spots {
                let d = base_s + spot.s - cam.s;
                if fog_at(d) > 0.98 {
                    continue;
                }
                let Some((x, y, _)) = at(d) else { continue };
                let lateral = spot.side * (RAIL + 4.0 + 0.5 * spot.size);
                let rel = ((x + lateral - cam.x) / d).atan() - spot.side * 0.45;
                let kind = ItemKind::Kite { design: spot.design, size: spot.size, id: spot.id, squash: 0.35 + 0.65 * rel.cos().abs(), lift: 0.5 * spot.size + 5.0 };
                items.push(place(d, kind, lateral, x, y));
            }
        }
        // La Torre del Reformador sobre la meta: su semaforo y sus focos
        // siguen la fase.
        let next_line = ((cam.s / length).floor() + 1.0) * length;
        for s in [next_line - length, next_line] {
            let d = s - cam.s;
            if let Some((x, y, _)) = at(d) {
                let (lights, t) = match self.phase {
                    Phase::Ready => (0, 0.0),
                    Phase::Countdown(t) => (1 + t as u8, t),
                    _ => (4, 0.0),
                };
                let green = self.green_at.map_or(f32::MAX, |g| self.wall - g);
                let kind = ItemKind::Tower { lights, t, green, finished: matches!(self.phase, Phase::Finished(_)) };
                items.push(place(d, kind, 0.0, x, y));
            }
        }
        // Cuanto alumbra el poste mas cercano a `s`: `e^(-(ds / 6)^2)`.
        let lamp_near = |s: f32| {
            let n = lamp_after(s - LAMP_EVERY, length);
            (n..n + 3).map(|n| (lamp(n, length).0 - s) * (1.0 / 6.0)).map(|q| fast_exp(-q * q)).fold(0.0f32, f32::max)
        };
        // El humo: cada soplo con su luz del poste y su giro.
        let mut puff_draws = Vec::new();
        for puff in &self.puffs {
            let d = puff.s - cam.s;
            if let Some((x, _, _)) = at(d) {
                let id = puff_draws.len() as u16;
                let (sin, cos) = puff.rot.sin_cos();
                puff_draws.push(PuffDraw { age: puff.age, radius: puff.radius(), sin, cos, seed: puff.seed, lamp: lamp_near(puff.s), weight: if puff.own { 0.4 } else { 1.0 } });
                let mut item = place(d, ItemKind::Puff { id }, puff.x, x, puff.y);
                let r = puff.radius() * item.scale;
                (item.top, item.bottom) = (item.y - r - 1.0, item.y + r + 1.0);
                items.push(item);
            }
        }
        // Las chispas de la pista: una capsula de donde estaba hace 1/45 s
        // (con su velocidad de ahora) a donde esta, cada una con su
        // profundidad (las tapa lo que este mas cerca). La capsula mide como
        // mucho 12 px: las que vuelan hacia la camara se corren mucho en la
        // pantalla en 1/45 s y, sin tope, eran rayas rectas hasta la orilla
        // (como laser, no chispas). Las de a menos de 2 m no se pintan.
        let mut spark_draws = Vec::new();
        let project_track = |s: f32, x: f32, y: f32| -> Option<(f32, f32, f32)> {
            let d = s - cam.s;
            let (xo, _, _) = at(d)?;
            Some((cx + (xo + x - cam.x) * cam.focal / d, cam.horizon - (y - cam.y) * cam.focal / d, d))
        };
        for p in self.sparks.iter().filter(|p| p.age >= 0.0) {
            let tau = 1.0 / 45.0;
            let (Some(head), Some(tail)) = (project_track(p.s, p.x, p.y), project_track(p.s - p.vs * tau, p.x - p.vx * tau, p.y - p.vy * tau)) else { continue };
            if head.2 < 2.0 {
                continue;
            }
            let (tx, ty) = (tail.0 - head.0, tail.1 - head.1);
            let long = tx.hypot(ty);
            let cut = if long > 12.0 { 12.0 / long } else { 1.0 };
            let tail = (head.0 + tx * cut, head.1 + ty * cut, tail.2);
            // Se enfria: del color al rojo, y se apaga `(1 - t)^1.5`.
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let bright = (1.0 - t) * (1.0 - t).sqrt() * (1.0 - fog_at(head.2));
            if bright < 0.02 {
                continue;
            }
            let color = p.color.lerp(Vec3::new(1.0, 0.2, 0.03) * p.color.max_element() * 0.6, t) * bright;
            let w = (0.03 * cam.focal / head.2).clamp(0.6, 1.4);
            let reach = 8.0 * w + 1.0;
            let id = spark_draws.len() as u16;
            spark_draws.push(SparkSeg { a: (tail.0, tail.1), b: (head.0, head.1), color, emit: bright.min(1.0), w, tail: 0.3, lo: (head.0.min(tail.0) - reach, head.1.min(tail.1) - reach), hi: (head.0.max(tail.0) + reach, head.1.max(tail.1) + reach) });
            let mut item = Item::new(head.2, ItemKind::Spark { id }, head.0, head.1, cam.focal / head.2);
            (item.top, item.bottom) = (head.1.min(tail.1) - reach, head.1.max(tail.1) + reach);
            items.push(item);
        }
        // Las naves. Las tablas de sus LEDs: la persecucion de feria (las
        // ambar), `0.5 + 0.5 smoothstep(0.55, 1, 0.5 + 0.5 sin(2 pi (4 f /
        // 64 - 1.6 t)))`, un pulso que corre por la fase `f` (cuatro por
        // vuelta de fase, seis vueltas a lo largo de la nave; con piso de
        // 0.5, asi los focos se apagan a medias y la nave sigue de feria), y
        // la respiracion (magentas, cianes y azules), `0.7 + 0.3 sin(2.1 t +
        // 0.1 f)`. Cada nave las lee corridas por su numero (no van todas al
        // compas); las fijas a 1 y los stops a `0.6 + 1.4 calaveras`. Todo
        // por 0.3 en el trafico (que no le gane a las rivales) y por 1.25 con
        // turbo. La tabla va por el byte del patron tal cual (`patron << 6 |
        // fase`, ver `hornear::Lut`).
        let wall = self.wall;
        let chase: [f32; 64] = std::array::from_fn(|f| 0.5 + 0.5 * smoothstep(0.55, 1.0, 0.5 + 0.5 * (TAU * (f as f32 / 64.0 * 4.0 - wall * 1.6)).sin()));
        let breathe: [f32; 64] = std::array::from_fn(|f| 0.7 + 0.3 * (wall * 2.1 + f as f32 * 0.1).sin());
        let mut luts: Vec<Lut> = Vec::new();
        let mut car_draws: Vec<CarDraw> = Vec::new();
        let mut flames: Vec<FlameDraw> = Vec::new();
        let mut meshes: Vec<MeshDraw> = Vec::new();
        let up = Vec3::new(0.0, 1.0, 0.0);
        let player_v = self.cars[0].v;
        for (i, car) in self.cars.iter().enumerate() {
            let spec = car.kind.spec();
            let d = car.s - cam.s;
            let Some((x, y, theta)) = at(d) else { continue };
            let wx = x + car.x - cam.x;
            let sx = cx + wx * cam.focal / d;
            // La chela flota y se mece: `0.6 + 0.12 sin(2.2 t + i)` de alto y
            // sin ladeo.
            let phase = i as f32;
            let (hover, wobble) = if car.kind == Kind::Chela { (0.6 + 0.12 * (wall * 2.2 + phase).sin(), 0.0) } else { (spec.hover, 0.0) };
            let sy = cam.horizon - (y + hover - cam.y) * cam.focal / d;
            let player = i == 0;
            // La apertura de la camara (`focal / (1 + fov)`) encoge todo; a
            // la nave propia se le devuelve la mitad, `1 + fov / 2`, para
            // que no pierda presencia con el turbo.
            let keep = if player { 1.0 + 0.5 * self.fov.max(0.0) } else { 1.0 };
            let k = cam.focal / d * keep;
            // Las otras, a la escala de su cara mas cercana (la cola, o el
            // costado si se ve de lado): un sprite no tiene perspectiva y,
            // medido desde su centro, la cola de la camioneta (4.8 m mas
            // cerca) salia hasta la mitad de chica de cerca. Asi su cola mide
            // lo que mide en 3D y el paso al 3D no brinca.
            // El rumbo de las otras: cuanto costado se les veria con
            // perspectiva. Un sprite es una foto ortografica: girado lo que
            // da el rayo (`theta - atan(x / z)`) enseña el costado casi tan
            // ancho como la cola, y en perspectiva ese costado se va al punto
            // de fuga y sale angosto; al pasar al 3D la camioneta parecia
            // enderezarse (como si se ladeara). Se toma el costado que da a la
            // camara (la orilla de la nave mas cerca del eje) en la cola y en
            // la nariz, cada una donde va la pista a su profundidad: lo que se
            // separan en la pantalla, `f (x_n / z_n - x_c / z_c)`, es lo que
            // mide el costado en el sprite girado `phi`, `k L sin(phi)`, con
            // `k = f / z_c` la escala de la cola.
            let yaw = if player {
                0.0
            } else {
                let (near_d, far_d) = ((d - 0.5 * spec.length).max(1.0), d + 0.5 * spec.length);
                let lat = |dd: f32| at(dd).map_or(x + theta * (dd - d), |(xx, _, _)| xx) + car.x - cam.x;
                let inner = |v: f32| v - v.signum() * v.abs().min(0.5 * spec.width);
                let spread = inner(lat(far_d)) / far_d - inner(lat(near_d)) / near_d;
                (spread * near_d / spec.length).clamp(-1.0, 1.0).asin()
            };
            let k = if player {
                k
            } else {
                let face = 0.5 * spec.length * yaw.cos().abs() + 0.5 * spec.width * yaw.sin().abs();
                k * d / (d - face).max(0.6 * d)
            };
            // Y anclada por su cola: con perspectiva la cola, mas cerca que el
            // centro, cae mas a un lado y mas abajo que la proyeccion del
            // centro (una camioneta a 16 m y 4 m a un lado: 60 px). El sprite
            // se corre para que su cola (en el horneado,
            // `(-L/2 sin(rumbo), L/2 cos(rumbo) sin(altura))` del ancla, en
            // metros) caiga donde cae la de verdad; asi coincide con su malla
            // al fundirse con ella.
            let (sx, sy) = if player {
                (sx, sy)
            } else {
                let half = 0.5 * spec.length;
                let tail_d = (d - half).max(1.0);
                let (tx, ty, _) = at(tail_d).unwrap_or((x - theta * half, y, theta));
                let tail = (cx + (tx + car.x - cam.x) * cam.focal / tail_d, cam.horizon - (ty + hover - cam.y) * cam.focal / tail_d);
                let e = self.rival_elevation[car.kind.index()];
                (tail.0 + half * yaw.sin() * k, tail.1 - half * yaw.cos() * e.sin() * k)
            };
            // El rumbo: el angulo en pasos de horneado, `kf`, cae entre dos
            // sprites y se toma el mas cercano. (Mezclar los dos vecinos no
            // sirve: solo coincide lo que se alinea, y lo demas sale doble,
            // como la botella con dos cuellos.) La propia tiene un sprite
            // cada 1.25 grados, asi el salto de uno al siguiente es chico; las
            // demas, cada 10, pero se ven de lejos.
            // La propia sigue la pista: el rumbo de la pista donde va menos
            // el angulo del rayo (en las curvas se le ve el costado). El
            // volante no la gira: vista desde arriba, una nave girada se ve
            // chueca, como ladeada, y apachar la flecha la inclinaba.
            let (set, kf, count) = if player {
                let yaw = theta - (wx / d).atan();
                (usize::MAX, yaw / PLAYER_YAW_STEP + (PLAYER_YAWS / 2) as f32, PLAYER_YAWS)
            } else {
                (car.can.map_or(car.kind.index(), |c| Kind::ALL.len() + c as usize), yaw / YAW_STEP + (YAWS / 2) as f32, YAWS)
            };
            let kf = if kf.is_finite() { kf.clamp(0.0, (count - 1) as f32) } else { (count / 2) as f32 };
            let sprites: &[Sprite] = if player { &self.player_set } else { self.sets.get(set).map_or(&[], |v| v.as_slice()) };
            let k0 = (kf.round() as usize).min(count - 1);
            let Some(sprite) = sprites.get(k0) else { continue };
            let scale = k / sprite.scale;
            // La cola de una nave larga gira alrededor del centro: del rumbo
            // `k0` al de verdad, `kf`, se corre `L/2 sin(rumbo)` (px por
            // metro del ancla). El sprite se corre a lo ancho para que su
            // cola caiga donde cae la del rumbo de verdad: la cola sigue el
            // giro sin saltos y solo lo de adelante salta, poquito.
            let step = if player { PLAYER_YAW_STEP } else { YAW_STEP };
            let mid = (count / 2) as f32;
            let length = spec.length;
            let tail = |k: f32| -0.5 * length * ((k - mid) * step).sin() * sprite.scale;
            let ax = sprite.anchor.0 - (tail(kf) - tail(k0 as f32));
            // El mip: `log2(1 / escala)` (cuantos pixeles del sprite caen en
            // uno de la pantalla) menos 1: un nivel mas fino que lo justo,
            // que el bilineal aguanta casi sin centellear, asi las de lejos
            // salen nitidas (con 0.6 se veian borrosas junto a las de antes,
            // sin mips) y la propia, apenas encogida por la apertura, se
            // queda en el nivel 0.
            let lod = if scale > 0.0 { ((1.0 / scale).log2() - 1.0).max(0.0) } else { 0.0 };
            // Las que pasan pegadas a la camara se desvanecen solo a menos de
            // 1.6 m (antes de 5.5: pasaban como fantasmas).
            let near = if player { 1.0 } else { smoothstep(0.8, 1.6, d) };
            // El barrido: de cerca (el sprite agrandado mas de 2.2 veces, asi
            // que siempre por la recta de `draw_car`), la que pasa (o a la
            // que se pasa) se emborrona a lo ancho lo que se mueve respecto a
            // la camara, `0.02 |dv|` pixeles del sprite (hasta 6). Mas lo
            // que esconde los pixeles del sprite agrandado,
            // `0.25 (escala - 2)` (hasta 1.5), y nada si no llega a un pixel
            // de la pantalla.
            let smear = if !player && scale > 2.2 { ((car.v - player_v).abs() * 0.02).max((0.25 * (scale - 2.0)).min(1.5)).min(6.0) } else { 0.0 };
            let smear = if smear * scale < 1.0 { 0.0 } else { smear.min(6.0) };
            let roll = car.roll + wobble;
            let (sin, cos) = roll.sin_cos();
            // El tuk-tuk gira sobre la rueda de afuera (levanta la de adentro):
            // el pivote se va a `0.45 ancho` del lado al que se ladea.
            let pivot = if car.kind == Kind::TukTuk { roll.signum() * spec.width * 0.45 * smoothstep(0.06, 0.14, roll.abs()) * k } else { 0.0 };
            let rail = (fast_exp(-(car.x + RAIL) / 3.0), fast_exp(-(RAIL - car.x) / 3.0));
            let off = (i * 13) % 64;
            let gain = if car.role == Role::Traffic { 0.3 } else { 1.0 } * if car.boost > 0.0 { 1.25 } else { 1.0 };
            let stop = 0.6 + 1.4 * car.brake_light;
            luts.push(std::array::from_fn(|b| {
                let f = b & 63;
                gain * match b >> 6 {
                    0 => 1.0,
                    1 => chase[(f + off) % 64],
                    2 => breathe[(f + off) % 64],
                    _ => stop,
                }
            }));
            let cd = CarDraw {
                set,
                k: k0,
                ax,
                length,
                spill: self.arch_spill(car.s),
                smear,
                sin,
                cos,
                pivot,
                lift: car.bounce * k,
                stretch: 1.0 / (1.0 + 0.04 * car.bounce_v.clamp(-5.0, 5.0)),
                lamp: lamp_near(car.s),
                rail,
                neon: std::array::from_fn(|i| {
                    let tx = i as f32 / RELIGHT_TAPS as f32;
                    let g = NEON[0] * (RELIGHT_NEON * rail.0 * smoothstep(0.6, 0.0, tx)) + NEON[1] * (RELIGHT_NEON * rail.1 * smoothstep(0.4, 1.0, tx));
                    Vec3::new(fast_exp(-g.x), fast_exp(-g.y), fast_exp(-g.z))
                }),
                near,
                lod,
                pick: sprite.pick(lod),
                // De muy lejos (escala bajo 0.12, mas de unos 60 m) se suma
                // un poco de la luz del mip siguiente, hasta 0.3, para que
                // las luces no se apaguen al promediarse; solo sobre el
                // cuerpo (por su alfa), asi no agranda la silueta. (Con dos
                // niveles arriba y hasta 2.4 desde 0.25, las de 30 a 120 m
                // quedaban como manchas rosas con forma de caja.)
                far_pick: sprite.pick(lod + 1.0),
                far: if scale < 0.12 { 0.3 * smoothstep(0.12, 0.06, scale) } else { 0.0 },
                solid: near >= 1.0 && (player || k >= 6.0),
                mesh: None,
                mix: 1.0,
            };
            // Las otras de cerca van en 3D: su malla con la camara de la
            // carrera (la misma proyeccion que la pista), ver `MeshDraw`.
            let mut cd = cd;
            let solid_index = car.can.map_or(car.kind.index(), |c| Kind::ALL.len() + c as usize);
            if !player && d < FAR_SOLID && solid_index < self.solids.len() {
                let origin = Vec3::new(x + car.x, y + hover + car.bounce, d);
                let tall = self.solids[solid_index].tall;
                // La camara del horneado mira el centro de la nave.
                let view_dir = (origin + up * (0.45 * tall) - Vec3::new(cam.x, cam.y, 0.0)).normalize();
                let toward = -view_dir;
                let right_b = toward.cross(up).normalize();
                let up_b = right_b.cross(toward);
                cd.mesh = Some(meshes.len() as u16);
                cd.mix = smoothstep(NEAR_SOLID, FAR_SOLID, d);
                cd.solid = false;
                if cd.mix <= 0.0 {
                    cd.smear = 0.0;
                }
                let raster = self.mesh_pool.lock().ok().and_then(|mut pool| pool.pop()).unwrap_or_default();
                // La pista a lo largo de la nave, cada medio metro: donde
                // pasa su centro, su altura y su rumbo a cada profundidad.
                let reach = 0.6 * spec.length + 1.0;
                let road = (0..=((2.0 * reach / ROAD_STEP) as usize))
                    .map(|n| {
                        let dd = d - reach + n as f32 * ROAD_STEP;
                        at(dd).unwrap_or((x + theta * (dd - d), y, theta))
                    })
                    .collect();
                meshes.push(MeshDraw {
                    raster,
                    solid: solid_index,
                    origin,
                    axes: [right_b, up_b, toward],
                    span: (0.0, 1.0),
                    halo: None,
                    road,
                    road_from: -reach,
                    lateral: car.x,
                    lift: hover + car.bounce,
                });
            }
            let id = car_draws.len() as u16;
            car_draws.push(cd);
            let mut item = Item::new(d, ItemKind::Car { id }, sx, sy, k);
            (item.top, item.bottom) = self.car_rows(&item, &cd);
            items.push(item);
            // El turbo, en el escape: un cono de llama a lo largo del chorro,
            // que sale hacia atras (hacia la camara). Se pinta justo delante
            // del sprite: el escape es la cola, lo mas cerca de la camara
            // (pintado antes que el sprite lo tapaba la defensa entero).
            if car.boost > 0.0 {
                let ex = self.rigs[car.kind.index()].exhaust;
                // Donde cae el escape en el sprite: con el rumbo `th` y la
                // altura de la vista `e` del horneado,
                // `x = ex.x cos th - ex.z sin th` y
                // `y = ex.y cos e - (ex.x sin th + ex.z cos th) sin e` sobre el
                // ancla (lo de atras, mas cerca de la camara, baja). Despues,
                // girado con la nave: `(rx, ry)` desde el pivote y de vuelta a
                // la pantalla.
                let th = (kf - mid) * step;
                let e = if player { self.player_elevation } else { self.rival_elevation[car.kind.index()] };
                let (st, ct) = th.sin_cos();
                let (se, ce) = e.sin_cos();
                let up = ex.y * ce - (ex.x * st + ex.z * ct) * se;
                let (rx, ry) = ((ex.x * ct - ex.z * st) * k - pivot, -up * k / cd.stretch - cd.lift);
                let (fx, fy) = (sx + pivot + rx * cos - ry * sin, sy + rx * sin + ry * cos);
                // En 3D, el escape mismo proyectado (ver `MeshDraw`).
                let (fx, fy) = match cd.mesh.and_then(|m| meshes.get(m as usize)) {
                    Some(m) => {
                        let (p, _) = m.place(Vec3::new(ex.x, ex.y, ex.z), Vec3::Y);
                        if p.z > 0.3 { (cx + (p.x - cam.x) * cam.focal / p.z, cam.horizon - (p.y - cam.y) * cam.focal / p.z) } else { (fx, fy) }
                    }
                    None => (fx, fy),
                };
                // La punta: el chorro mide `largo` metros hacia la camara, de
                // la profundidad del escape `de = d - ex.z` a `de - largo`, y
                // en perspectiva se corre lo que cambia `1 / profundidad`:
                // `(x, alto - camara) f (1 / (de - largo) - 1 / de)` (de cerca
                // baja mucho en la pantalla: el cono apunta a quien mira).
                let (reach, r0, r1) = flame_shape(car.kind);
                let de = (d - ex.z).max(1.0);
                let dt = (de - reach).max(0.6 * de).max(1.0);
                let grow = cam.focal * (1.0 / dt - 1.0 / de);
                let wy = y + ex.y + hover - cam.y;
                let tip = (fx + (wx + ex.x) * grow, fy - wy * grow);
                let (r0, r1) = (r0 * cam.focal / de, r1 * cam.focal / dt);
                let id = flames.len() as u16;
                flames.push(FlameDraw { kind: car.kind, power: (car.boost / 0.3).min(1.0), seed: phase * 7.3, length: spec.length, tip, r0, r1 });
                let r = r0.max(r1) * 1.6 + 1.0;
                let mut flame = Item::new(d - 0.01, ItemKind::Flame { id }, fx, fy, k);
                (flame.top, flame.bottom) = (fy.min(tip.1) - r, fy.max(tip.1) + r);
                items.push(flame);
            }
        }
        // Las mallas de las naves cercanas, cada una en su hilo, y de que
        // fila a que fila caen.
        let view3 = malla::View { pos: Vec3::new(cam.x, cam.y, 0.0), focal: cam.focal, cx, horizon: cam.horizon };
        // 1. Sus vertices en la camara con su luz, por vertice (ver
        // `hornear::Solid`), cada malla en paralelo por tramos: la normal
        // del modelo vista desde la camara del horneado (de frente a ella:
        // las mallas tienen caras abiertas).
        for m in &mut meshes {
            let Some(solid) = self.solids.get(m.solid) else { continue };
            let [bx, by, bz] = m.axes;
            let mut raster = std::mem::take(&mut m.raster);
            raster.begin(view3, w, h);
            raster.reserve(solid.positions.len() + 64, solid.indices.len());
            let placed = &*m;
            raster.fill(solid.positions.len(), |i| {
                // A la camara como va la pista (ver `MeshDraw::place`), y la
                // normal vista desde la camara del horneado.
                let (world, n) = placed.place(solid.positions[i], solid.normals[i]);
                let mut nb = Vec3::new(n.dot(bx), n.dot(by), n.dot(bz));
                nb = if nb.length_squared() > 1e-12 { nb.normalize() } else { Vec3::Z };
                if nb.z < 0.0 {
                    nb = -nb;
                }
                let (lit, emit, tag) = solid.vertex(i, nb);
                (world, n, lit, emit, tag)
            });
            m.raster = raster;
        }
        // 2. Sus triangulos.
        parallel::chunks_mut(&mut meshes, 1, 1, |_, slot| {
            let m = &mut slot[0];
            let Some(solid) = self.solids.get(m.solid) else { return };
            for &tri in &solid.indices {
                m.raster.triangle(tri);
            }
            if let Some((_, _, x0, x1)) = m.raster.bounds() {
                m.span = (x0, x1.max(x0 + 1.0));
            }
        });
        self.halos(&mut meshes, cam.focal, w, h);
        for item in &mut items {
            if let ItemKind::Car { id } = item.kind
                && let Some(m) = car_draws.get(id as usize).and_then(|c| c.mesh).and_then(|m| meshes.get(m as usize))
            {
                let sprite_rows = (item.top, item.bottom);
                let blended = car_draws.get(id as usize).is_some_and(|c| c.mix > 0.0);
                (item.top, item.bottom) = match m.raster.bounds() {
                    Some((y0, y1, _, _)) if blended => ((y0 - 1.0).min(sprite_rows.0), (y1 + 1.0).max(sprite_rows.1)),
                    Some((y0, y1, _, _)) => (y0 - 1.0, y1 + 1.0),
                    None if blended => sprite_rows,
                    None => (0.0, -1.0),
                };
                if let Some(halo) = &m.halo {
                    (item.top, item.bottom) = (item.top.min(halo.y0), item.bottom.max(halo.y0 + halo.h as f32 * halo.cell));
                }
            }
        }
        items.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        // De que fila a que fila cae cada cosa: las demas filas ni la miran.
        // (Las naves, el humo, las chispas y el turbo ya lo traen.)
        for item in &mut items {
            if item.top == f32::MIN {
                (item.top, item.bottom) = self.item_rows(item);
            }
        }
        // Las naves que tapan pista opacas: la propia y las demas que ya no
        // se desvanecen y que miden algo en la pantalla (ver
        // `CarDraw::solid`; las barridas tambien, con lo opaco angostado lo
        // que alcanza el barrido, ver `solid_cols`).
        let solid_items = items.iter().filter(|i| matches!(i.kind, ItemKind::Car { id } if car_draws.get(id as usize).is_some_and(|c| c.solid))).copied().collect();
        // El charco de luz de cada nave: las diez mas cercanas a la vista.
        let mut pools: Vec<(f32, Pool)> = self
            .cars
            .iter()
            .filter_map(|car| {
                let d = car.s - cam.s;
                if !(0.5..250.0).contains(&d) {
                    return None;
                }
                let spec = car.kind.spec();
                // La chela flota y no tiene calaveras: su charco es lo unico
                // que la asienta, va mas fuerte y mas recogido (ver `strip`).
                let gain = if car.role == Role::Traffic { 0.5 } else { 1.0 } * if spec.hover > 0.0 { 2.0 } else { 1.0 } * (1.0 - fog_at(d));
                Some((d, Pool { s: car.s, x: car.x, hw: spec.width * 0.5, hl: spec.length * 0.5, tint: self.tint[car.kind.index()], gain, brake: car.brake_light, hover: spec.hover > 0.0 }))
            })
            .collect();
        pools.sort_by(|a, b| a.0.total_cmp(&b.0));
        let pools: Vec<Pool> = pools.into_iter().take(10).map(|(_, p)| p).collect();
        // La raya del reflejo de cada poste en el piso mojado: el espejo de
        // la lampara (a 8.65 m) queda a `-8.65`, y la recta de la camara a
        // el cruza el suelo a la fraccion `t = hc / (hc + 8.65)` del camino
        // (`hc` la altura de la camara sobre la pista alla). Se estira hacia
        // la camara, mas de lejos: `largo = 2 + 0.25 (s - s_camara)`. El
        // reflejo cae en el plano vertical del ojo y la lampara: en la
        // pantalla es una raya vertical bajo la lampara, asi que se guarda
        // la columna de la lampara y cada fila busca su `u` ahi.
        let mut lamp_spec = Vec::new();
        let first = lamp_after(cam.s, length);
        for n in first..first + 16 {
            let (s_l, side) = lamp(n, length);
            let d = s_l - cam.s;
            if d >= 300.0 {
                break;
            }
            let Some((x, _, _)) = at(d) else { continue };
            let hc = (cam.y - self.track.height(s_l)).max(0.5);
            let t = hc / (hc + LAMP_HEIGHT);
            let s_spec = cam.s + d * t;
            lamp_spec.push((s_spec, cx + (x + side * LAMP_REACH - cam.x) * cam.focal / d, 2.0 + 0.25 * (s_spec - cam.s)));
        }

        // Los puntos del mapa.
        let dots = self
            .cars
            .iter()
            .filter(|c| c.role != Role::Traffic)
            .map(|c| {
                let i = self.track.index(c.s);
                // Con el mismo margen con que se dibuja el trazado.
                let (mx, my) = self.track.map[i];
                (map_pad(mx), map_pad(my), c.kind.color(), c.role == Role::Player)
            })
            .collect();
        // Un rayo pinhole por columna: su rumbo y la distancia focal
        // efectiva `k = hypot(f, xoff)` con que se mide la elevacion.
        let cols: Vec<Col> = (0..w)
            .map(|x| {
                let xoff = x as f32 + 0.5 - cx;
                let k = cam.focal.hypot(xoff);
                let yaw = cam.heading + (xoff / cam.focal).atan();
                let (sin_y, cos_y) = yaw.sin_cos();
                Col { k, yaw, sin_y, cos_y, psi: wrap_pi(yaw) }
            })
            .collect();
        let limb = cam.limb();
        // El rumbo de cada columna de la estela (su centro en la pantalla)
        // en el cielo suave.
        let bw = self.post.streak().width;
        let sky_u = (0..bw)
            .map(|i| {
                let x = (((i as f32 + 0.5) * w as f32 / bw as f32) as usize).min(w.saturating_sub(1));
                Panorama::soft_cols(cols.get(x).map_or(cam.heading, |c| c.yaw))
            })
            .collect();
        // La neblina de cada fila: el cielo que queda atras, en la columna
        // del centro (donde se pierde la pista).
        let haze_row = (0..h).map(|y| sky_sample(&self.sky_lut, (((cam.horizon - (y as f32 + 0.5)) / cam.focal).atan() + limb).max(0.0))).collect();
        // Por columna: la silueta, la del Fuego y el destello (que cae con
        // la distancia en rumbo al Fuego, `e^(-|dpsi| / 0.18)`, y llega a
        // cero sin salto: se le resta un umbral chico).
        let flash = (self.eruption.flash - 0.003).max(0.0);
        let vol = cols
            .iter()
            .map(|c| {
                let (top, id) = volcano_profile(c.psi);
                let dpsi = (c.psi - FUEGO.psi).abs();
                let fuego = if dpsi < 0.3 { FUEGO.profile(c.psi) } else { 0.0 };
                // Lo que queda bajo 1e-3 se corta (en el cielo usaria la
                // ruta lenta del tono sin que se note).
                let flash = if flash > 0.0 { (flash * (-dpsi / 0.18).exp() - 1e-3).max(0.0) } else { 0.0 };
                VolCol { top, id, fuego, flash }
            })
            .collect();
        // La estrella fugaz en la pantalla: cabeza, cola y su caja.
        let project = |yaw: f32, pitch: f32| -> Option<(f32, f32)> {
            let rel = wrap_pi(yaw - cam.heading);
            if rel.abs() > 1.3 {
                return None;
            }
            let xs = cx + cam.focal * rel.tan();
            let k = cam.focal.hypot(xs - cx);
            Some((xs, cam.horizon - k * pitch.tan()))
        };
        let meteor = self.meteor.and_then(|m| {
            let age = self.wall - m.born;
            let p = (age / m.dur).clamp(0.0, 1.0);
            let len = m.dyaw.hypot(m.dpitch).max(1e-4);
            // La cola mide 0.08 rad, o lo que lleva recorrido.
            let back = (0.08 / len).min(p);
            let head = project(m.yaw + m.dyaw * p, m.pitch + m.dpitch * p)?;
            let tail = project(m.yaw + m.dyaw * (p - back), m.pitch + m.dpitch * (p - back))?;
            // Holgura de la caja: el halo llega a cero a los 11 px.
            let pad = 12.0;
            Some(MeteorView {
                head,
                tail,
                fade: 1.0 - p,
                lo: (head.0.min(tail.0) - pad, head.1.min(tail.1) - pad),
                hi: (head.0.max(tail.0) + pad, head.1.max(tail.1) + pad),
            })
        });
        // Las bombas del Fuego en la pantalla: una capsula de la posicion
        // de ahora a la de hace 0.09 s (hacia atras en la parabola,
        // `p(t - tau) = p - v tau - g tau^2 / 2`), ordenadas por la fila de
        // arriba de su caja. Se enrojecen al envejecer, para que no lean
        // como cohetes.
        let mut sparks = Vec::new();
        for b in self.eruption.bombs.iter().filter(|b| b.age > 0.0) {
            let color = Vec3::new(2.0, 0.6, 0.14).lerp(Vec3::new(0.6, 0.05, 0.008), b.age.min(1.0));
            let tau = b.age.min(0.09);
            let p = b.p - b.vp * tau - 0.5 * BOMB_GRAVITY * tau * tau;
            if let (Some(head), Some(tail)) = (project(b.psi, b.p), project(b.psi - b.vpsi * tau, p)) {
                let reach = 8.0;
                sparks.push(Spark { head, tail, color, lo: (head.0.min(tail.0) - reach, head.1.min(tail.1) - reach), hi: (head.0.max(tail.0) + reach, head.1.max(tail.1) + reach) });
            }
        }
        sparks.sort_by(|a, b| a.lo.1.total_cmp(&b.lo.1));
        // Los cohetes: cada chispa es una capsula de donde estaba hace
        // 0.07 s (0.15 la palmera) a donde esta, en el mismo rumbo y
        // elevacion que el cielo (`project`). Se reparten en cubetas de 8
        // filas por su caja. El destello de los que acaban de reventar,
        // `sum color 0.012 e^(-7 t)` (hasta 0.02 por canal), alumbra el cielo
        // y la pista. (Con 0.06 el cielo de noche, de ~0.005, se iba a gris
        // cuando revientan varios juntos.)
        let mut fw_segs: Vec<SparkSeg> = Vec::new();
        let mut fw_flash = Vec3::ZERO;
        let push = |segs: &mut Vec<SparkSeg>, a: (f32, f32), b: (f32, f32), color: Vec3, emit: f32, w: f32| {
            if segs.len() < MAX_SPARK_SEGS {
                let reach = 8.0 * w + 1.0;
                segs.push(SparkSeg { a, b, color, emit, w, tail: 0.15, lo: (a.0.min(b.0) - reach, a.1.min(b.1) - reach), hi: (a.0.max(b.0) + reach, a.1.max(b.1) + reach) });
            }
        };
        for burst in &self.bursts {
            let t = burst.age(self.wall);
            if t < 0.0 {
                // Subiendo desde la capital (elevacion -0.35) y frenando:
                // `p = pitch + (-0.35 - pitch) u^2` con `u` lo que le falta.
                let rise = |t: f32| {
                    let u = (-t / RISE).clamp(0.0, 1.0);
                    burst.pitch + (-0.35 - burst.pitch) * u * u
                };
                if let (Some(a), Some(b)) = (project(burst.yaw, rise(t - 0.12)), project(burst.yaw, rise(t))) {
                    push(&mut fw_segs, a, b, Vec3::new(1.0, 0.7, 0.4) * 0.9, 0.8, 0.7);
                }
                continue;
            }
            if t < 0.6 {
                fw_flash += (burst.color(0) + burst.color(1)) * (0.006 * (-7.0 * t).exp());
            }
            let Some(center) = project(burst.yaw, burst.pitch) else { continue };
            // El fogonazo del centro al reventar.
            if t < 0.25 {
                let f = (-12.0 * t).exp();
                push(&mut fw_segs, center, center, Vec3::new(1.0, 0.9, 0.75) * (2.0 * f), f, 1.5 + 2.5 * f);
                // (Un punto: pareja, sin cola.)
                if let Some(last) = fw_segs.last_mut() {
                    last.tail = 1.0;
                }
            }
            // Lo que mide abierto en la pantalla: si no pasa de 2 px, aun no.
            if burst.spark(0, t).0.hypot(burst.spark(0, t).1) * cam.focal < 2.0 && t < 0.1 {
                continue;
            }
            let trail = if burst.kind == 0 { 0.15 } else { 0.07 };
            let w = 0.6 + 0.5 * (-t).exp();
            for i in 0..SPARKS {
                let bright = burst.bright(i, t);
                if bright < 0.02 {
                    continue;
                }
                let (y0, p0) = burst.spark(i, (t - trail).max(0.0));
                let (y1, p1) = burst.spark(i, t);
                if let (Some(a), Some(b)) = (project(burst.yaw + y0, burst.pitch + p0), project(burst.yaw + y1, burst.pitch + p1)) {
                    push(&mut fw_segs, a, b, burst.color(i) * (1.3 * bright), bright.min(1.0), w);
                }
            }
        }
        // Al cielo hasta 0.02 por canal; la pista se aclara aparte, por su
        // propio color (`c (1 + 2 destello)`), hasta un 12 %.
        let fw_flash_road = fw_flash.min(Vec3::splat(0.06));
        let fw_flash = fw_flash.min(Vec3::splat(0.02));
        let mut fw_buckets: Vec<Vec<u16>> = vec![Vec::new(); h.div_ceil(8)];
        for (i, seg) in fw_segs.iter().enumerate() {
            let (b0, b1) = ((seg.lo.1.max(0.0) / 8.0) as usize, (seg.hi.1.max(0.0) / 8.0) as usize);
            for bucket in fw_buckets.iter_mut().take(b1 + 1).skip(b0) {
                bucket.push(i as u16);
            }
        }
        let e = &self.eruption;
        View {
            game: self,
            cam,
            rows,
            far,
            items,
            dots,
            cols,
            limb,
            haze_row,
            vol,
            lut: tone_lut(),
            root_c: (2.0 * R_PLANET * cam.alt + cam.alt * cam.alt).sqrt(),
            lod_bias: (cam.focal * ciudad::METROS).log2(),
            haze_lin: sky_lin(0.0),
            glow: e.glow,
            flows: e.flows,
            meteor,
            sparks,
            pools,
            lamp_spec,
            blur_s: (self.cars[0].v * self.frame_dt).clamp(0.0, 3.0),
            sky_u,
            solid_items,
            fw_segs,
            fw_buckets,
            fw_flash,
            fw_flash_road,
            car_draws,
            meshes,
            luts,
            puff_draws,
            spark_draws,
            flames,
        }
    }

    /// Los halos de las LEDs de las naves en 3D (ver `Halo`): las celdas de
    /// todas se llenan en paralelo, una fila de celdas por trabajo (la fila
    /// de la pantalla de su centro, con lo mas cercano de la malla), y
    /// despues cada halo se difumina como el del horneado.
    fn halos(&self, meshes: &mut [MeshDraw], focal: f32, width: usize, height: usize) {
        // Pixeles por metro de los sprites de las otras.
        let sprite_ppm = 46.0 * height as f32 / 720.0;
        struct Plan {
            mesh: usize,
            cell: f32,
            radius: f32,
            x0: f32,
            y0: f32,
            w: usize,
            h: usize,
        }
        let mut plans = Vec::new();
        for (i, m) in meshes.iter().enumerate() {
            let Some(solid) = self.solids.get(m.solid) else { continue };
            let Some((by0, by1, bx0, bx1)) = m.raster.bounds() else { continue };
            if solid.finish != Finish::Vehicle {
                continue;
            }
            // Pixeles de la pantalla por pixel del sprite a su distancia. El
            // halo es un difuminado: va a la mitad del detalle del sprite
            // (y nunca a mas de una celda por cada 2 x 2 pixeles).
            let scale = focal / m.origin.z.max(0.5) / sprite_ppm;
            let cell = (2.0 * scale).max(2.0);
            let radius = scale / cell;
            // El halo ancho llega a unos 21 pixeles del sprite.
            let margin = ((21.0 * radius).ceil() + 2.0) * cell;
            let x0 = (bx0 - margin).max(-cell);
            let x1 = (bx1 + margin).min(width as f32 + cell);
            let y0 = (by0 - margin).max(-cell);
            let y1 = (by1 + margin).min(height as f32 + cell);
            if !(x1 > x0 && y1 > y0) {
                continue;
            }
            let (w, h) = (((x1 - x0) / cell).ceil() as usize, ((y1 - y0) / cell).ceil() as usize);
            if w * h > 600_000 {
                continue;
            }
            plans.push(Plan { mesh: i, cell, radius, x0, y0, w, h });
        }
        if plans.is_empty() {
            return;
        }
        let mut src: Vec<Vec<Vec3>> = plans.iter().map(|p| vec![Vec3::ZERO; p.w * p.h]).collect();
        let mut alpha: Vec<Vec<f32>> = plans.iter().map(|p| vec![0.0; p.w * p.h]).collect();
        let mut led: Vec<Vec<Vec3>> = plans.iter().map(|p| vec![Vec3::ZERO; p.w * p.h]).collect();
        let tasks: Vec<(usize, usize)> = plans.iter().enumerate().flat_map(|(i, p)| (0..p.h).map(move |j| (i, j))).collect();
        {
            let src_w: Vec<_> = src.iter_mut().map(|v| parallel::DisjointMut::new(v.as_mut_slice())).collect();
            let alpha_w: Vec<_> = alpha.iter_mut().map(|v| parallel::DisjointMut::new(v.as_mut_slice())).collect();
            let led_w: Vec<_> = led.iter_mut().map(|v| parallel::DisjointMut::new(v.as_mut_slice())).collect();
            let meshes = &*meshes;
            parallel::for_each(tasks.len(), |t| {
                let (i, j) = tasks[t];
                let p = &plans[i];
                let m = &meshes[p.mesh];
                let y = (p.y0 + (j as f32 + 0.5) * p.cell).floor();
                if y < 0.0 || y >= height as f32 {
                    return;
                }
                // SAFETY: cada trabajo es una fila de celdas distinta de su
                // halo.
                let (src_row, alpha_row, led_row) = unsafe { (src_w[i].range(j * p.w, p.w), alpha_w[i].range(j * p.w, p.w), led_w[i].range(j * p.w, p.w)) };
                HITS.with(|cell| {
                    let mut scratch = cell.borrow_mut();
                    let hits = &mut scratch.0;
                    if hits.len() < width {
                        hits.resize(width, Hit::default());
                    }
                    let (lo, hi) = m.raster.row_nearest(y as usize, hits);
                    for k in 0..p.w {
                        let x = (p.x0 + (k as f32 + 0.5) * p.cell).floor();
                        if x < lo as f32 || x >= hi as f32 {
                            continue;
                        }
                        let hit = hits[x as usize];
                        if hit.depth == f32::MAX {
                            continue;
                        }
                        alpha_row[k] = 1.0;
                        let f = m.raster.sample(&hit);
                        if f.b.max_element() > 1e-6 {
                            src_row[k] = hornear::halo_source(f.b, f.tag);
                            led_row[k] = Vec3::ONE;
                        }
                    }
                    for hit in &mut hits[lo..hi] {
                        *hit = Hit::default();
                    }
                });
            });
        }
        let mut done: Vec<(usize, Halo)> = plans
            .iter()
            .map(|p| (p.mesh, Halo { cell: p.cell, x0: p.x0, y0: p.y0, w: p.w, h: p.h, img: Vec::new() }))
            .collect();
        parallel::chunks_mut(&mut done, 1, 1, |i, slot| {
            let p = &plans[i];
            if led[i].iter().any(|l| l.x > 0.0) {
                slot[0].1.img = hornear::led_halo(&src[i], &alpha[i], &led[i], p.w, p.h, p.radius);
            }
        });
        for (mesh, halo) in done {
            if !halo.img.is_empty() {
                meshes[mesh].halo = Some(halo);
            }
        }
    }

    /// El sprite de una nave: el de su rumbo (ver `CarDraw::k`).
    fn car_sprite(&self, cd: &CarDraw) -> Option<&Sprite> {
        let set: &[Sprite] = if cd.set == usize::MAX { &self.player_set } else { self.sets.get(cd.set)? };
        set.get(cd.k)
    }

    /// De que fila a que fila cae una nave: las cuatro esquinas del
    /// rectangulo de su sprite (con el barrido) llevadas a la pantalla con
    /// su ladeo, su salto y su estiramiento (ver `CarDraw`), y un pixel de
    /// margen. Lo que se muestrea afuera del rectangulo no se pinta.
    fn car_rows(&self, item: &Item, cd: &CarDraw) -> (f32, f32) {
        let Some(sprite) = self.car_sprite(cd) else { return (0.0, -1.0) };
        let scale = item.scale / sprite.scale;
        if !(scale > 0.0) {
            return (0.0, -1.0);
        }
        let (mut top, mut bottom) = (f32::MAX, f32::MIN);
        for sx in [-cd.smear, sprite.width as f32 + cd.smear] {
            for sy in [0.0, sprite.height as f32] {
                // `rx = (sx - ax) escala - pivote`, `ry = (sy - ay) escala
                // / estiramiento - salto`, y de vuelta a la pantalla,
                // `dy = rx sin + ry cos`.
                let rx = (sx - cd.ax) * scale - cd.pivot;
                let ry = (sy - sprite.anchor.1) * scale / cd.stretch - cd.lift;
                let dy = rx * cd.sin + ry * cd.cos;
                top = top.min(item.y + dy);
                bottom = bottom.max(item.y + dy);
            }
        }
        (top - 1.0, bottom + 1.0)
    }

    /// De que fila a que fila de la pantalla cae algo (con un margen): el
    /// poste, la torre, el arco y el barrilete, lo que miden en metros por
    /// los pixeles por metro.
    fn item_rows(&self, item: &Item) -> (f32, f32) {
        let k = item.scale;
        let y = item.y;
        match item.kind {
            // Las naves, el humo, las chispas y el turbo las sacan al
            // ponerse (ver `view`).
            ItemKind::Car { .. } | ItemKind::Puff { .. } | ItemKind::Spark { .. } | ItemKind::Flame { .. } => (item.top, item.bottom),
            ItemKind::Lamp { .. } => (y - 11.5 * k, y + 0.3 * k),
            ItemKind::Tower { .. } => (y - (TOWER_TOP - 1.0) * k - 4.0 * beacon_spread(k), y + 0.3 * k),
            ItemKind::Arch { .. } => (y - (ARCH_B + 1.2) * k, y + 0.5 * k),
            ItemKind::Kite { size, lift, .. } => (y - (lift + 0.62 * size) * k - 2.0, y + 0.3 * k),
        }
    }
}

/// La tabla de LEDs de una nave es la de su numero.
fn item_lut(item: &Item) -> usize {
    match item.kind {
        ItemKind::Car { id } => id as usize,
        _ => usize::MAX,
    }
}

/// Si en el punto `lap_s` de la vuelta (m) hay tunel de arcos.
fn arches_at(lap_s: f32) -> bool {
    ARCH_ZONES.iter().any(|&(a, b)| (a..b).contains(&lap_s))
}

/// Cuantos postes hay en una vuelta de `length` m: en `k LAMP_EVERY` con
/// `k = 1, 2, ...` mientras quede a `LAMP_CLEAR` m de la meta siguiente
/// (el de `k = 0` caeria en la pata de la torre).
fn lamps_per_lap(length: f32) -> i64 {
    ((length - LAMP_CLEAR) / LAMP_EVERY).floor().max(1.0) as i64
}

/// El poste numero `n` (contando desde la primera vuelta, negativo antes):
/// su `s` contando vueltas y su lado (los pares a la derecha). Se cuentan
/// por vuelta para que caigan en el mismo lugar de cada vuelta (la vuelta
/// no mide un multiplo de 48 m) y lejos de la meta.
fn lamp(n: i64, length: f32) -> (f32, f32) {
    let per = lamps_per_lap(length);
    let (lap, k) = (n.div_euclid(per), n.rem_euclid(per) + 1);
    (lap as f32 * length + k as f32 * LAMP_EVERY, if k % 2 == 0 { 1.0 } else { -1.0 })
}

/// El numero del primer poste despues de `s` (m, contando vueltas).
fn lamp_after(s: f32, length: f32) -> i64 {
    let per = lamps_per_lap(length);
    let lap = (s / length).floor();
    let k = ((s - lap * length) / LAMP_EVERY).floor() as i64 + 1;
    lap as i64 * per + (k - 1).min(per)
}

/// Los barriletes de una vuelta: uno de 12 m cada 200 m, alternando lado,
/// y el festival antes de la meta, uno cada 48 m alternando lado y de 9, 14
/// y 18 m. Todos caen a medio camino entre dos postes (`s = 24` modulo 48:
/// los postes se cuentan por vuelta) y ninguno adentro de un tunel de arcos
/// (ni a 6 m): el que caeria ahi se corre al lugar libre mas cercano, asi
/// sus focos y su cable no le pasan por la cara.
fn kite_spots(track: &Track) -> Vec<KiteSpot> {
    let length = track.length();
    let mid = |s: f32| ((s - 0.5 * LAMP_EVERY) / LAMP_EVERY).round() * LAMP_EVERY + 0.5 * LAMP_EVERY;
    let clear = |s: f32| !ARCH_ZONES.iter().any(|&(a, b)| (a - 6.0..b + 6.0).contains(&s));
    let mut spots = Vec::new();
    let mut id = 0u32;
    let mut k = 0;
    while (k as f32 + 0.5) * KITE_EVERY < length {
        let mut s = mid((k as f32 + 0.5) * KITE_EVERY);
        // El lugar libre mas cercano, de 48 en 48 hacia los dos lados.
        if !clear(s) {
            s = (1..12)
                .flat_map(|j| [s + j as f32 * LAMP_EVERY, s - j as f32 * LAMP_EVERY])
                .find(|&c| (0.0..length).contains(&c) && clear(c))
                .unwrap_or(s);
        }
        if !(FESTIVAL.0 - 20.0..FESTIVAL.1 + 20.0).contains(&s) {
            spots.push(KiteSpot { s, side: if k % 2 == 0 { -1.0 } else { 1.0 }, size: 12.0, design: (id % 4) as u8, id });
            id += 1;
        }
        k += 1;
    }
    let mut s = mid(FESTIVAL.0);
    let mut side = 1.0;
    while s < FESTIVAL.1 {
        spots.push(KiteSpot { s, side, size: [9.0, 14.0, 18.0][id as usize % 3], design: (id % 4) as u8, id });
        id += 1;
        side = -side;
        s += FESTIVAL_EVERY;
    }
    spots
}

/// Lo que mide la torre a la altura `my` (m) a cada lado del centro: el borde de
/// afuera de las patas, `wo = 2.2 + (RAIL + 2.6) max(1 - my / 58, 0)^1.8`
/// (abiertas abajo, rectas arriba), y el arco de adentro,
/// `wi = (RAIL + 0.9) sqrt(1 - (my / 13)^2)` hasta 13 m (0 arriba).
fn tower_outer(my: f32) -> f32 {
    2.2 + (RAIL + 2.6) * (1.0 - my / 58.0).max(0.0).powf(1.8)
}

fn tower_inner(my: f32) -> f32 {
    if my < 13.0 { (RAIL + 0.9) * (1.0 - (my / 13.0).powi(2)).max(0.0).sqrt() } else { 0.0 }
}

/// Los doce focos de cada lado del arco de la torre: parejos en el angulo
/// `phi = (j + 0.5) pi / 24` de la elipse de semiejes `(RAIL + 0.9, 13)`,
/// `(a cos phi, b sin phi)`, calculados una vez.
fn tower_arc_bulbs() -> &'static [(f32, f32); 12] {
    static BULBS: OnceLock<[(f32, f32); 12]> = OnceLock::new();
    BULBS.get_or_init(|| {
        std::array::from_fn(|j| {
            let phi = (j as f32 + 0.5) * std::f32::consts::FRAC_PI_2 / 12.0;
            ((RAIL + 0.9) * phi.cos(), 13.0 * phi.sin())
        })
    })
}

/// Lo que se abre el halo del faro de la torre (px): 1.5 m, entre 6 y 40
/// px (de cerca, un halo de cientos de pixeles solo costaria).
fn beacon_spread(k: f32) -> f32 {
    (1.5 * k).clamp(6.0, 40.0)
}

/// Una lista corta en la pila, para lo que se arma por fila sin pedir
/// memoria al monton: lo que ya no cabe se descarta.
struct Short<T: Copy + Default, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> Short<T, N> {
    fn new() -> Self {
        Self { items: [T::default(); N], len: 0 }
    }

    fn push(&mut self, v: T) {
        if self.len < N {
            self.items[self.len] = v;
            self.len += 1;
        }
    }

    fn as_slice(&self) -> &[T] {
        &self.items[..self.len]
    }

    fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.items[..self.len]
    }
}

#[derive(Clone, Copy, Debug)]
struct Camera {
    s: f32,
    x: f32,
    y: f32,
    focal: f32,
    horizon: f32,
    heading: f32,
    /// Donde esta en el plano (m) y a que altura sobre la ciudad (m).
    world: (f32, f32),
    alt: f32,
}

impl Camera {
    /// Cuanto baja el limbo del planeta bajo el horizonte (rad): desde la
    /// altura `H` sobre una esfera de radio `R`, `cos d = R / (R + H)`, y
    /// para `H << R`, `d = sqrt(2 H / R)` (~0.0707 a 400 m).
    fn limb(&self) -> f32 {
        (2.0 * self.alt.max(1.0) / R_PLANET).sqrt()
    }
}

/// El rayo de una columna de la pantalla.
#[derive(Clone, Copy, Debug)]
struct Col {
    /// Distancia focal efectiva hasta la columna, `hypot(f, xoff)`.
    k: f32,
    /// Rumbo del rayo (con las vueltas), su seno y coseno, y el rumbo de
    /// mundo de -pi a pi (`psi`, 0 hacia la recta de largada).
    yaw: f32,
    sin_y: f32,
    cos_y: f32,
    psi: f32,
}

/// La estrella fugaz en la pantalla.
#[derive(Clone, Copy, Debug)]
struct MeteorView {
    head: (f32, f32),
    tail: (f32, f32),
    fade: f32,
    lo: (f32, f32),
    hi: (f32, f32),
}

/// Una bomba de lava en la pantalla: la capsula de la cola a la cabeza, su
/// color (luz lineal) y la caja hasta donde alumbra.
#[derive(Clone, Copy, Debug)]
struct Spark {
    head: (f32, f32),
    tail: (f32, f32),
    color: Vec3,
    lo: (f32, f32),
    hi: (f32, f32),
}

/// Lo de una fila que comparten sus pixeles.
struct RowCtx<'s> {
    y: usize,
    /// Pixeles sobre el horizonte (negativo abajo).
    yoff: f32,
    /// Las bombas que caen cerca de esta fila.
    sparks: &'s [Spark],
    /// De que columna a cual pueden alumbrar.
    span: (f32, f32),
    /// La neblina de la pista de esta fila y de la de atras (son por
    /// fila: la profundidad no cambia a lo largo de ella).
    fog: f32,
    far_fog: f32,
    /// De que columna a cual va la pista de atras en esta fila (entre las
    /// orillas de sus barandas).
    far_span: (f32, f32),
    /// Cuanto queda del destello del Fuego a esta altura,
    /// `e^(-max(p - cima, 0) / 0.1)` con la elevacion de la columna del
    /// centro (en las orillas cambia poco y es un resplandor suave).
    flash_up: f32,
    /// La pista de esta fila y la de atras, con lo que no cambia a lo
    /// ancho.
    near: Option<Strip>,
    far_strip: Option<Strip>,
    /// La fila en espacio de pantalla, a la resolucion de la estela: la
    /// estela del reflejo (del frame anterior, ya interpolada en vertical)
    /// y el cielo suave que se refleja; y texeles por pixel a lo ancho.
    /// Solo las columnas entre las barandas, desde `screen_lo`.
    screen: Vec<[Vec3; 2]>,
    screen_lo: usize,
    screen_scale: f32,
}

/// Que punto de la pista muestra una fila.
#[derive(Clone, Copy, Debug)]
struct RoadRow {
    /// Donde cae el centro de la pista y cuantos pixeles mide un metro.
    center: f32,
    per_meter: f32,
    depth: f32,
    s: f32,
    /// Cuantos metros de pista abarca la fila a lo largo.
    ds_dy: f32,
    /// Si `ds_dy` salio del suelo plano porque ninguna vecina sigue la
    /// misma pista (un corte en una cresta): ahi no se pinta la junta, que
    /// con ese `ds_dy` aproximado saldria como una franja.
    cut: bool,
}

#[derive(Clone, Copy, Debug)]
enum ItemKind {
    /// Una nave: su numero en `View::car_draws` (y en `View::luts`).
    Car { id: u16 },
    Lamp { side: f32 },
    /// Un barrilete: su diseno, cuanto mide (m), su numero, cuanto se
    /// angosta puesto de canto y a que altura va su centro (m).
    Kite { design: u8, size: f32, id: u32, squash: f32, lift: f32 },
    /// La Torre del Reformador: las luces del semaforo (0 apagado, 1 a 3
    /// rojas, 4 verde), el segundo de la cuenta, cuanto lleva el verde y
    /// si quien juega ya termino.
    Tower { lights: u8, t: f32, green: f32, finished: bool },
    /// Un arco de foquitos: su numero (de 0 a 4, mueve los colores),
    /// cuanto se angosta visto de canto y si hace eco del semaforo (0 no,
    /// 1 rojo, 2 verde).
    Arch { id: u32, squash: f32, signal: u8 },
    /// Un soplo de humo (`View::puff_draws`).
    Puff { id: u16 },
    /// Una chispa de la pista (`View::spark_draws`).
    Spark { id: u16 },
    /// El fuego del turbo de una nave (`View::flames`).
    Flame { id: u16 },
}

/// Lo de una nave que se pinta en este frame.
///
/// Su sprite se muestrea al reves: para cada pixel de la pantalla, su
/// corrimiento `(dx, dy)` desde el pivote `P = (x + pivote, y)` se gira por
/// el ladeo, `rx = dx cos + dy sin`, `ry = -dx sin + dy cos`, y en el
/// sprite es `sx = (rx + pivote) / escala + ancla_x`,
/// `sy = (ry + salto) / escala * estiramiento + ancla_y`.
#[derive(Clone, Copy, Debug)]
struct CarDraw {
    /// Su juego de sprites (`usize::MAX` el propio) y el rumbo.
    set: usize,
    k: usize,
    /// La `x` de su ancla, corrida para que su cola caiga donde cae la del
    /// rumbo de verdad (ver `view`).
    ax: f32,
    /// Su largo (m).
    length: f32,
    /// La luz de los arcos de foquitos que le cae.
    spill: Vec3,
    /// El barrido (pixeles del sprite a cada lado).
    smear: f32,
    /// El seno y el coseno del ladeo, el pivote (px desde su centro, a lo
    /// ancho), lo que salta (px) y el estiramiento vertical del sprite.
    sin: f32,
    cos: f32,
    pivot: f32,
    lift: f32,
    stretch: f32,
    /// Cuanto la alumbra el poste mas cercano y el neon de cada baranda, y
    /// lo que deja pasar el neon, `e^(-neon(tx))`, en 17 puntos a lo ancho
    /// del sprite (ver `draw_car`).
    lamp: f32,
    rail: (f32, f32),
    neon: [Vec3; RELIGHT_TAPS + 1],
    /// Lo que se ve (se desvanece pegada a la camara), el mip (y que
    /// niveles lee, sacado una vez por nave) y cuanto se suma de la luz del
    /// mip siguiente (de muy lejos, ver `view`).
    near: f32,
    lod: f32,
    pick: Pick,
    far_pick: Pick,
    far: f32,
    /// Si tapa la pista: opaca entera donde su sprite (en los mips que
    /// lee, y con lo que alcanzan el barrido y la recta) lo es; sin
    /// desvanecerse.
    solid: bool,
    /// Si va en 3D: su numero en `View::meshes`, y cuanto pesa su sprite
    /// (0 solo la malla, 1 solo el sprite; entre `NEAR_SOLID` y
    /// `FAR_SOLID` se funden).
    mesh: Option<u16>,
    mix: f32,
}

/// Hasta que distancia (m) las otras naves se dibujan en 3D con su malla, y
/// desde cual solo con su sprite (en medio se funden). Un sprite es una
/// foto sin perspectiva, girada a lo que se ve de la nave: de cerca (sobre
/// todo a un lado, donde se le ve el costado) la camioneta, de 9.6 m, salia
/// atravesada en la calle, como si diera la vuelta en la curva. La malla,
/// con la misma camara que la pista, sale con su perspectiva y nitida. De
/// lejos el sprite es mejor: sus LEDs, de unos centimetros, quedan
/// promediadas (2 x 2 muestras y mips) y con su halo, y en la malla un foco
/// de menos de un pixel sale o no sale (centellea). El sprite se agranda
/// desde unos 15 m (46 px por metro): de ahi para aca, la malla. El tramo
/// en que se funden es corto: aunque el sprite esta anclado por la cola y
/// horneado desde la altura de la camara, la malla de cerca se ve desde
/// abajo del techo y el sprite no; fundidos se ve doble.
const NEAR_SOLID: f32 = 16.0;
const FAR_SOLID: f32 = 19.0;

/// Una nave cercana en 3D en este frame: su raster, cual malla es, donde
/// queda (el suelo bajo su centro, en la camara: `x` a la derecha, `y`
/// arriba, `z` la profundidad) y la pista bajo ella (ver `place`); los
/// ejes de la camara del horneado que la ve (derecha, arriba y hacia la
/// camara: la luz sale como en los sprites) y las columnas que cubre en la
/// pantalla (para el neon a lo ancho).
struct MeshDraw {
    raster: Indexed,
    solid: usize,
    origin: Vec3,
    axes: [Vec3; 3],
    span: (f32, f32),
    halo: Option<Halo>,
    /// La pista a lo largo de la nave (ver `place`): donde pasa su centro,
    /// su altura y su rumbo cada `ROAD_STEP` metros desde `road_from` metros
    /// (adelante del centro), y la nave a cuanto de su centro va y cuanto
    /// sube (flota, rebota).
    road: Vec<(f32, f32, f32)>,
    road_from: f32,
    lateral: f32,
    lift: f32,
}

/// Cada cuanto se toma la pista a lo largo de una nave en 3D (m).
const ROAD_STEP: f32 = 0.5;

impl MeshDraw {
    /// La pista a `ahead` metros del centro de la nave (adelante, positivo).
    fn road_at(&self, ahead: f32) -> (f32, f32, f32) {
        let t = ((ahead - self.road_from) / ROAD_STEP).clamp(0.0, (self.road.len() - 1) as f32);
        let i = (t as usize).min(self.road.len().saturating_sub(2));
        let f = t - i as f32;
        let (a, b) = (self.road[i], self.road[(i + 1).min(self.road.len() - 1)]);
        (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f, a.2 + (b.2 - a.2) * f)
    }

    /// Un punto y una normal del modelo (`x` a la derecha, `y` arriba, la
    /// nariz hacia `-z`) en la camara, como va la pista: cada punto a su
    /// profundidad, corrido de lado lo que se corre la pista ahi (el
    /// pseudo-3D no gira la pista: la corre, y sus filas siguen derechas).
    /// Una nave girada rigida con el rumbo de su centro cruzaba esas filas;
    /// asi va en su carril como las rayas. Con `X = x_pista(d - z) + x` y
    /// `Z = d - z`, la normal va con la inversa transpuesta,
    /// `(n.x, n.y, -rumbo n.x - n.z)`.
    #[inline]
    fn place(&self, p: Vec3, n: Vec3) -> (Vec3, Vec3) {
        let (rx, ry, heading) = self.road_at(-p.z);
        (Vec3::new(rx + self.lateral + p.x, ry + self.lift + p.y, self.origin.z - p.z), Vec3::new(n.x, n.y, -heading * n.x - n.z))
    }
}

/// El halo de las LEDs de una nave en 3D, como el de su sprite
/// (`hornear::led_halo`): una imagen de celdas de `cell` pixeles de la
/// pantalla (un pixel del sprite a esa distancia, o uno de la pantalla si
/// la nave se ve mas chica que su sprite) desde `(x0, y0)`. Cada celda sale
/// de lo que la malla deja ver en su centro (las LEDs tapadas no brillan).
struct Halo {
    cell: f32,
    x0: f32,
    y0: f32,
    w: usize,
    h: usize,
    img: Vec<Vec3>,
}

impl Halo {
    /// Las columnas de la pantalla que cubre, `[desde, hasta)`.
    fn cols(&self, width: usize) -> (usize, usize) {
        let a = self.x0.max(0.0) as usize;
        let b = ((self.x0 + self.w as f32 * self.cell).ceil().max(0.0) as usize).min(width);
        (a.min(b), b)
    }

    fn covers_row(&self, yf: f32) -> bool {
        yf >= self.y0 && yf < self.y0 + self.h as f32 * self.cell
    }

    /// La fila de la pantalla `y` del halo, ya interpolada entre sus dos
    /// filas de celdas (con una celda de cero a cada lado): cada pixel
    /// solo interpola a lo ancho (`line_at`).
    fn line(&self, y: f32, out: &mut Vec<Vec3>) {
        let v = (y - self.y0) / self.cell - 0.5;
        let j = v.floor();
        let fv = v - j;
        let row = |j: f32| (j >= 0.0 && j < self.h as f32).then(|| &self.img[j as usize * self.w..(j as usize + 1) * self.w]);
        let (top, bottom) = (row(j), row(j + 1.0));
        out.clear();
        out.push(Vec3::ZERO);
        for k in 0..self.w {
            let a = top.map_or(Vec3::ZERO, |r| r[k]);
            let b = bottom.map_or(Vec3::ZERO, |r| r[k]);
            out.push(a.lerp(b, fv));
        }
        out.push(Vec3::ZERO);
    }

    /// El halo en la columna `x` de una fila de `line`.
    #[inline]
    fn line_at(&self, line: &[Vec3], x: f32) -> Vec3 {
        // La celda `k` esta en `line[k + 1]`.
        let u = ((x - self.x0) / self.cell + 0.5).clamp(0.0, self.w as f32 + 0.999);
        let i = u as usize;
        line[i].lerp(line[i + 1], u - i as f32)
    }
}

/// Un soplo de humo en este frame: su edad, su radio (m), su giro, su
/// semilla, cuanto lo alumbra el poste mas cercano y cuanto pesa (0.4 el
/// de la nave propia).
#[derive(Clone, Copy, Debug)]
struct PuffDraw {
    age: f32,
    radius: f32,
    sin: f32,
    cos: f32,
    seed: f32,
    lamp: f32,
    weight: f32,
}

/// El fuego del turbo de una nave: de que clase es, cuanto queda (se
/// apaga los ultimos 0.3 s), su semilla, el largo de la nave, la punta del
/// cono en la pantalla (la boca es el punto del `Item`) y su radio en la
/// boca y en la punta (px).
#[derive(Clone, Copy, Debug)]
struct FlameDraw {
    kind: Kind,
    power: f32,
    seed: f32,
    /// El largo de su nave (m).
    length: f32,
    tip: (f32, f32),
    r0: f32,
    r1: f32,
}

/// La forma del chorro del turbo de cada clase (m): cuanto mide y su radio
/// en la boca y en la punta. El tuk-tuk, una brasa chica (lo suyo son las
/// chispas).
fn flame_shape(kind: Kind) -> (f32, f32, f32) {
    match kind {
        Kind::Colectivo => (1.7, 0.16, 0.34),
        Kind::Camioneta => (1.5, 0.2, 0.5),
        Kind::Chela => (1.1, 0.16, 0.36),
        Kind::TukTuk => (0.35, 0.08, 0.12),
    }
}

/// Un sprite de nave en `(sx, sy)` (ver `CarDraw`): el cuerpo y la luz de
/// las LEDs animada por su tabla, con el mip de la nave (cero afuera de su
/// rectangulo); de muy lejos, mas la luz del mip siguiente por el alfa del
/// cuerpo (que no agrande la silueta).
#[inline(always)]
fn car_texel(sprite: &Sprite, cd: &CarDraw, lut: &Lut, sx: f32, sy: f32) -> Texel {
    if !(sy >= 0.0 && sy <= sprite.height as f32 && sx >= 0.0 && sx <= sprite.width as f32) {
        return Texel::default();
    }
    let mut t = sprite.sample_pick(&cd.pick, sx, sy, lut);
    if cd.far > 0.0 && t.body[3] > 0.0 {
        let g = sprite.sample_pick(&cd.far_pick, sx, sy, lut);
        let w = cd.far * t.body[3];
        for c in 0..3 {
            t.light[c] += g.light[c] * w;
        }
    }
    t
}

/// Lo que hay en `dx` de una recta muestreada cada `1 / inv_step` desde
/// `start` (ver `draw_car`): la interpolacion lineal de las dos muestras
/// vecinas. Afuera, la de la orilla.
#[inline(always)]
fn line_at(line: &[Texel], start: f32, inv_step: f32, dx: f32) -> Texel {
    let n = line.len();
    if n < 2 {
        return line.first().copied().unwrap_or_default();
    }
    let f = ((dx - start) * inv_step).clamp(0.0, n as f32 - 1.001);
    let i = (f as usize).min(n - 2);
    line[i].lerp(line[i + 1], f - i as f32)
}

/// Desde que agrandamiento una nave se muestrea por la recta de cada fila
/// (ver `draw_car`).
const LINE_SCALE: f32 = 2.0;
/// Cuantas muestras lleva como mucho la recta de `draw_car` (mas, y se
/// muestrea pixel por pixel).
const LINE_MAX: usize = 4096;


thread_local! {
    /// Lo mas cercano de cada pixel de la fila para las naves en 3D
    /// (`draw_mesh`), de cada hilo.
    /// (Y la fila del halo, ver `Halo::line`.)
    static HITS: std::cell::RefCell<(Vec<Hit>, Vec<Vec3>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
    /// La fila antes de una nave que se funde y con su malla (ver
    /// `draw_item_behind`).
    static BLEND: std::cell::RefCell<(Vec<u8>, Vec<u8>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
}

thread_local! {
    /// Las rectas de `draw_car` (la que se usa y la cruda del barrido) de
    /// cada hilo: se reusan de fila en fila.
    static LINES: std::cell::RefCell<(Vec<Texel>, Vec<Texel>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
}

/// Cuanto alumbra a una nave el poste que tiene encima y el neon de la
/// baranda a su lado. Se multiplica por su color ya con tono (no luz
/// lineal): con 1.4 y 0.5 la laca pasaba de roja a rosa palido bajo cada
/// poste, y con 0.6 (y aun con 0.25) el techo azul marino de la camioneta,
/// que mira hacia arriba, pasaba a gris lavanda.
const RELIGHT_LAMP: f32 = 0.15;
const RELIGHT_NEON: f32 = 0.35;

/// El resplandor de la ciudad que alumbra por abajo el humo (con tono).
const CITY_UNDERLIGHT: Vec3 = Vec3::new(0.09, 0.045, 0.025);

/// Desde que altura (rad) se hornean las otras naves: la que tiene la camara
/// de quien juega (`camera_offset`) sobre el centro de la otra clase a la
/// mitad del tramo donde se funden el sprite y el 3D. Con una sola altura
/// (0.16, la de la camara de la camioneta) desde la botella, mas bajita, el
/// sprite enseñaba el techo y la malla no, y al fundirse se veia doble.
fn rival_elevation(player: Kind, other: Kind) -> f32 {
    let (_, up) = camera_offset(player.spec());
    let spec = other.spec();
    let center = spec.hover + 0.45 * spec.height;
    ((up - center) / (0.5 * (NEAR_SOLID + FAR_SOLID))).atan().clamp(0.03, 0.2)
}

/// En cuantos tramos se tabula la re-iluminacion de una nave a lo ancho.
const RELIGHT_TAPS: usize = 16;


/// Algo encima de la pista, con su punto en el suelo en la pantalla.
#[derive(Clone, Copy, Debug)]
struct Item {
    depth: f32,
    kind: ItemKind,
    x: f32,
    y: f32,
    /// Pixeles por metro a su distancia.
    scale: f32,
    /// De que fila a que fila de la pantalla puede caer (`item_rows`).
    top: f32,
    bottom: f32,
}

impl Item {
    /// Uno que cae en cualquier fila (hasta que `item_rows` lo acote).
    fn new(depth: f32, kind: ItemKind, x: f32, y: f32, scale: f32) -> Self {
        Self { depth, kind, x, y, scale, top: f32::MIN, bottom: f32::MAX }
    }
}

struct View<'a> {
    game: &'a Carrera,
    cam: Camera,
    rows: Vec<Option<RoadRow>>,
    /// La pista que asoma detras de la de adelante en la misma fila.
    far: Vec<Option<RoadRow>>,
    items: Vec<Item>,
    dots: Vec<(f32, f32, Vec3, bool)>,
    cols: Vec<Col>,
    /// El angulo del limbo bajo el horizonte, la neblina de cada fila (con
    /// tono) y lo de los volcanes por columna.
    limb: f32,
    haze_row: Vec<Vec3>,
    vol: Vec<VolCol>,
    lut: &'static [f32],
    /// Para el rayo contra el planeta: `sqrt(2 R H + H^2)` y
    /// `log2(f * metros por texel)`.
    root_c: f32,
    lod_bias: f32,
    /// El cielo justo en el limbo (lineal): el borde de la ciudad y el pie
    /// de los volcanes se funden en el.
    haze_lin: Vec3,
    /// El Fuego: brillo del crater y largo de los rios de lava.
    glow: f32,
    flows: [f32; 3],
    meteor: Option<MeteorView>,
    sparks: Vec<Spark>,
    /// Los charcos de luz de las naves cercanas.
    pools: Vec<Pool>,
    /// La raya del reflejo de cada poste: donde cae en la pista (`s`), la
    /// columna de la pantalla de su lampara y su largo (m).
    lamp_spec: Vec<(f32, f32, f32)>,
    /// Lo que se corre la pista en un frame (m, hasta 3): emborrona a lo
    /// largo las rayas, las tachas y el grano.
    blur_s: f32,
    /// Por columna de la estela (un cuarto de la pantalla): de que dos
    /// columnas del cielo suave lee y con que peso.
    sky_u: Vec<(usize, usize, f32)>,
    /// Las naves cercanas (para no pintar la pista que tapan enteras).
    solid_items: Vec<Item>,
    /// Las chispas de los cohetes, sus cubetas de 8 filas y el destello
    /// de los que acaban de reventar (luz lineal).
    fw_segs: Vec<SparkSeg>,
    fw_buckets: Vec<Vec<u16>>,
    fw_flash: Vec3,
    /// El mismo destello para la pista (con su propio tope).
    fw_flash_road: Vec3,
    /// Las naves de este frame, la tabla de sus LEDs (por patron y fase),
    /// el humo, las chispas de la pista y el fuego de los turbos.
    car_draws: Vec<CarDraw>,
    /// Las naves cercanas en 3D (ver `CarDraw::mesh`).
    meshes: Vec<MeshDraw>,
    luts: Vec<Lut>,
    puff_draws: Vec<PuffDraw>,
    spark_draws: Vec<SparkSeg>,
    flames: Vec<FlameDraw>,
}

/// El charco de luz de una nave en la pista: donde va (`s`, `x`), su medio
/// ancho y medio largo (m), el color de sus LEDs, cuanto pesa (la mitad el
/// trafico, y se apaga en la neblina), sus calaveras y si flota (la chela
/// no deja sombra de contacto).
#[derive(Clone, Copy, Debug, Default)]
struct Pool {
    s: f32,
    x: f32,
    hw: f32,
    hl: f32,
    tint: Vec3,
    gain: f32,
    brake: f32,
    hover: bool,
}

/// La altura de la lampara de los postes sobre la pista y a cuanto del
/// centro queda (la punta del brazo).
const LAMP_HEIGHT: f32 = 8.65;
const LAMP_REACH: f32 = 7.4;

const LAMP_COLOR: Vec3 = Vec3::new(1.0, 0.62, 0.25);
/// `h^2` de la lampara de los postes.
const LAMP_H2: f32 = LAMP_HEIGHT * LAMP_HEIGHT;
/// El cono de la lampara en `cos(theta)`: adentro de 0.86 (unos 5 m de
/// radio en el suelo) alumbra entera, afuera de 0.72 solo un cuarto. Con
/// el cono ancho (0.45 a 0.62, 11 m) el charco cubria toda la pista y no
/// tenia borde.
const LAMP_CONE: (f32, f32) = (0.72, 0.86);
/// El asfalto de cada losa (antes de su tono) y la pintura de las rayas:
/// gris tibio, no blanco de karting, y la orilla amarilla.
const SLAB: Vec3 = Vec3::new(0.06, 0.06, 0.07);
const LANE_PAINT: Vec3 = Vec3::new(0.46, 0.45, 0.41);
const EDGE_PAINT: Vec3 = Vec3::new(0.75, 0.55, 0.1);
/// La junta entre losas, cian.
const JOINT: Vec3 = Vec3::new(0.35, 0.8, 1.0);
/// El bordillo: blanco y celeste.
const CURB_WHITE: Vec3 = Vec3::new(0.85, 0.87, 0.9);
const CURB_BLUE: Vec3 = Vec3::new(0.25, 0.55, 0.85);
/// El promedio de la faja del hombro (lo que queda de lejos).
const SASH_AVG: Vec3 = Vec3::new(0.5, 0.18, 0.1);
/// Los tumulos: amarillo y negro.
const BUMP_YELLOW: Vec3 = Vec3::new(0.85, 0.65, 0.05);
const BUMP_BLACK: Vec3 = Vec3::new(0.03, 0.03, 0.03);
/// El rojo de las calaveras en el piso.
const BRAKE_RED: Vec3 = Vec3::new(1.0, 0.08, 0.05);
/// Donde va el aro antigravedad de lo que flota, en la elipse de su charco
/// (ver `surface`).
const RING_AT: f32 = 0.75;
/// Cuanto del cielo refleja la pista mojada (con Fresnel encima).
const SKY_REFLECT: f32 = 0.6;
const NEON: [Vec3; 2] = [Vec3::new(1.0, 0.2, 0.6), Vec3::new(0.15, 0.85, 1.0)];
/// El promedio de los colores del papel de china (los flecos de lejos).
const PAPEL_AVG: Vec3 = Vec3::new(0.617, 0.5, 0.4);

/// El coseno y el seno del angulo de cada foco de un arco,
/// `phi = pi (j + 0.5) / 41`, calculados una vez.
fn arch_bulbs() -> &'static [(f32, f32); ARCH_BULBS] {
    static BULBS: OnceLock<[(f32, f32); ARCH_BULBS]> = OnceLock::new();
    BULBS.get_or_init(|| {
        std::array::from_fn(|j| {
            let phi = PI * (j as f32 + 0.5) / ARCH_BULBS as f32;
            (phi.cos(), phi.sin())
        })
    })
}

impl View<'_> {
    /// Lo de la fila `y` que no cambia de pixel a pixel.
    fn row_ctx<'s>(&self, y: usize, sparks: &'s [Spark], span: (f32, f32)) -> RowCtx<'s> {
        let yoff = self.cam.horizon - (y as f32 + 0.5);
        let fog = |r: Option<RoadRow>| r.map_or(0.0, |r| fog_at(r.depth).max(horizon_fog(-yoff, r.depth)));
        let summit = FUEGO.height - self.limb;
        let pitch = (yoff / self.cam.focal).atan();
        let road = self.rows[y];
        let far = self.far[y];
        let far_span = far.map_or((f32::MAX, f32::MIN), |f| (f.center - (RAIL + 0.35) * f.per_meter, f.center + (RAIL + 0.35) * f.per_meter));
        // El reflejo en la pista mojada. La fila de la estela del frame
        // anterior, interpolada en vertical una vez (por pixel queda solo
        // la interpolacion a lo ancho).
        let mut screen = Vec::new();
        let mut screen_lo = 0;
        let mut screen_scale = 0.0;
        let (mut near, mut far_strip) = (None, None);
        if road.is_some() || far.is_some() {
            // El cielo que se refleja: el rayo de la fila rebota hacia
            // arriba con la elevacion `pm = -pitch`. Fresnel de Schlick con
            // `cos(theta) = sin(pm)` (rasante: casi espejo),
            // `F = 0.02 + 0.98 (1 - sin pm)^5`; y el gradiente del cielo a
            // esa altura sobre el limbo.
            let pm = (-pitch).max(0.0);
            let fres_s = if pitch >= 0.0 { 1.0 } else { 0.02 + 0.98 * (1.0 - pm.sin()).max(0.0).powi(5) };
            let sky_row = sky_sample(&self.game.sky_lut, pm + self.limb);
            // La fila en espacio de pantalla, a la resolucion de la estela
            // (un cuarto): la estela del frame anterior interpolada en
            // vertical y el cielo suave a esa altura (el panorama suave en
            // modo pantalla con el gradiente). Por pixel queda una
            // interpolacion a lo ancho.
            let st = self.game.post.streak();
            if st.width > 0 && st.height > 0 && st.pixels.len() >= st.width * st.height && self.sky_u.len() == st.width {
                let fy = ((y as f32 + 0.5) / self.game.height as f32 * st.height as f32 - 0.5).max(0.0);
                let y0 = (fy as usize).min(st.height - 1);
                let y1 = (y0 + 1).min(st.height - 1);
                let ty = (fy - y0 as f32).min(1.0);
                let (a, b) = (&st.pixels[y0 * st.width..][..st.width], &st.pixels[y1 * st.width..][..st.width]);
                let (v0, v1, tv) = Panorama::soft_rows(pm);
                let pano = &self.game.panorama;
                screen_scale = st.width as f32 / self.game.width as f32;
                // Solo las columnas entre las barandas de las pistas de la
                // fila (de lejos, unas pocas): no se llenan ni se reservan
                // las demas.
                let span = |r: Option<RoadRow>| r.map_or((f32::MAX, f32::MIN), |r| (r.center - (RAIL + 0.5) * r.per_meter, r.center + (RAIL + 0.5) * r.per_meter));
                let (n0, n1) = span(road);
                let (f0, f1) = span(far);
                let lo = ((n0.min(f0) * screen_scale - 1.0).max(0.0) as usize).min(st.width);
                let hi = ((n1.max(f1) * screen_scale + 2.0).max(0.0) as usize).min(st.width);
                screen_lo = lo;
                screen.reserve_exact(hi.saturating_sub(lo));
                for i in lo..hi {
                    let (x0, x1, tx) = self.sky_u[i];
                    let soft = pano.soft_at(x0, x1, tx, v0, v1, tv);
                    // La estela sale del brillo de todo lo que emite a un
                    // cuarto (ya mezclada): se le devuelve color,
                    // `v + 0.5 (v - luma)`, para que el charco de una nave
                    // salga del tono de sus LEDs y no cafe.
                    let v = a[i].lerp(b[i], ty);
                    let luma = 0.2126 * v.x + 0.7152 * v.y + 0.0722 * v.z;
                    let streak = (v * 1.5 - Vec3::splat(0.5 * luma)).max(Vec3::ZERO);
                    screen.push([streak, Vec3::ONE - (Vec3::ONE - soft) * (Vec3::ONE - sky_row)]);
                }
            }
            near = road.map(|r| self.strip(r, fres_s));
            far_strip = far.map(|r| self.strip(r, fres_s));
        }
        RowCtx { y, yoff, sparks, span, fog: fog(road), far_fog: fog(far), far_span, flash_up: (-(pitch - summit).max(0.0) / 0.1).exp(), near, far_strip, screen, screen_lo, screen_scale }
    }

    /// Lo de una fila de pista que no cambia a lo ancho: lo que cubre cada
    /// cosa pintada a lo largo (filtrado en caja sobre lo que abarca la
    /// fila mas lo que se corre la pista en el frame, `w`), el ruido de la
    /// fila, los postes y las naves que la alumbran, y el reflejo.
    fn strip(&self, r: RoadRow, fres_s: f32) -> Strip {
        let game = self.game;
        let s = r.s;
        let px = 1.0 / r.per_meter;
        let blur = self.blur_s;
        let w = (r.ds_dy + blur).max(1e-3);
        // Cada losa de 32 m con su tono, `1 + 0.05 (hash - 0.5)`.
        let slab_i = (s / 32.0).floor();
        let slab = SLAB * (1.0 + 0.05 * ((hash_u32(slab_i as i32 as u32) >> 8) as f32 * (1.0 / 16_777_216.0) - 0.5));
        // La junta cian entre losas: un hilo de 8 cm que destella de cerca
        // y despacio. La linea se filtra solo sobre lo que abarca la fila
        // (`ds_dy`, sin el borron: con el borron en caja la linea y su
        // resplandor salian como una meseta de bordes duros) y el
        // resplandor `e^(-|d| / 0.22)` con una tienda de medio ancho
        // `ds_dy` (ver `exp_tent`: sin bordes de fila en fila). Con la
        // velocidad se apaga, `1 / (1 + (borron / 0.25)^2)` (a 15 m/s la
        // mitad, a 60 m/s un 4 %: el hilo de cerca seria un borron de un
        // metro que casi no tine), y en una fila de corte (con `ds_dy`
        // aproximado) no se pinta.
        let dj = (s + 16.0).rem_euclid(32.0) - 16.0;
        let wj = r.ds_dy.max(1e-3);
        let joint_on = if r.cut {
            0.0
        } else {
            let k = blur / 0.25;
            1.0 / (1.0 + k * k)
        };
        let joint = band(s + 0.04, 32.0, 0.08, wj) * joint_on;
        let joint_glow = exp_tent(dj, 0.22, wj) * joint_on * (0.8 + 0.2 * (game.wall * 3.0 + slab_i).sin());
        // El relieve del bordillo: una loma cada 0.9 m, iluminada de frente
        // y en sombra atras; cuando la fila (con el borron) abarca mas que
        // media loma, se va suave a su promedio 0.83 (sin un salto en una
        // fila fija ni de un frame al otro al acelerar).
        let p = (s / 0.9).rem_euclid(1.0);
        let bumpy = 0.75 + 0.35 * smoothstep(0.0, 0.15, p) * smoothstep(0.45, 0.3, p) - 0.15 * smoothstep(0.6, 0.9, p);
        let relief = bumpy + (0.83 - bumpy) * smoothstep(0.3, 0.6, w);
        // La faja del hombro: rombos de 1.4 m, uno por celda de color. El
        // rombo es ancho entero a la mitad de su celda,
        // `pico = 1 - |fract(s / 1.4) - 0.5| 2`, y angosto en la punta,
        // donde cambia el color: asi el rombo es de un solo color sobre el
        // rojo. El color (amarillo en una celda, verde en la otra) tambien
        // se filtra en caja, `band(s, 2.8, 1.4, w)`: de lejos o rapido pasa
        // a su promedio sin escalones.
        let along = s / 1.4;
        let sash_col = transition::YELLOW.lerp(transition::GREEN, band(s, 2.8, 1.4, w));
        let sash_soft = (px / 0.7).max(w / 1.4);
        // De lejos (menos de ~10 px por metro) o cuando el frame corre mas
        // que un rombo, su promedio; con menos, el rombo solo se suaviza.
        let sash_far = smoothstep(0.06, 0.12, px).max(smoothstep(0.9, 1.4, w));
        // Los tumulos: cada uno cubre `0.3..0.7` de su tramo (1.6 m) con
        // los botones al centro. Se buscan en los tramos que toca la
        // ventana de la fila (de lejos una fila abarca varios tramos, y si
        // solo se mirara el del centro el tumulo se perderia). Para que se
        // lea de lejos, nunca mide menos de 2.5 filas a lo largo
        // (`max(1.6, 2.5 ds_dy)`, hasta 30 m) y los botones menos de una:
        // la cobertura es lo que cae de ese tramo en la ventana `w`.
        let (mut bumps, mut bump_cov, mut bump_q, mut bump_btn) = (0u8, 0.0, 0.0, 0.0);
        let k0 = (s / SEGMENT).floor() as i64;
        let reach = (0.5 * w / SEGMENT).ceil().min(16.0) as i64 + 4;
        let count = game.track.segments.len() as i64;
        for k in k0 - reach..=k0 + reach {
            let mask = game.track.segments[k.rem_euclid(count) as usize].bumps;
            if mask == 0 {
                continue;
            }
            let d = s - (k as f32 + 0.5) * SEGMENT;
            let half = (0.2 * SEGMENT).max(1.25 * r.ds_dy).min(15.0);
            let cov = window_cover(d, half, w);
            if cov > bump_cov {
                bumps = mask;
                bump_cov = cov;
                bump_q = ((d + 0.2 * SEGMENT) / (0.4 * SEGMENT)).clamp(0.0, 1.0);
                bump_btn = window_cover(d, (0.032 * SEGMENT).max(0.5 * r.ds_dy), w);
            }
        }
        let bump_far = smoothstep(0.2, 1.0, w).max(smoothstep(0.1, 0.4, px));
        // La meta: 2.5 m de cuadros de 1 m.
        let length = game.track.length();
        let finish = band(s, length, 2.5, w);
        let finish_fs = if finish > 0.0 { band(s, 2.0, 1.0, w) } else { 0.0 };
        // Los postes que alumbran esta fila (a menos de 40 m a lo largo).
        let mut lamps = [(0.0, 0.0); 3];
        let mut n_lamps = 0;
        let first = lamp_after(s - 40.0, length);
        for n in first..first + 3 {
            let (s_l, side) = lamp(n, length);
            let ds = s - s_l;
            if ds < -40.0 {
                break;
            }
            if ds.abs() <= 40.0 && n_lamps < lamps.len() {
                lamps[n_lamps] = (side * LAMP_REACH, 0.56 * ds * ds + LAMP_H2);
                n_lamps += 1;
            }
        }
        // Las rayas del reflejo de los postes: gaussiana de 0.30 m a lo
        // ancho (nunca menos de 0.7 px, con la energia conservada) y de
        // `largo` a lo largo, `1.1 e^(-t^2)` con `t = ds / largo` (se lleva
        // la luz que el difuso pierde en lo mojado). Caben cuatro por fila:
        // las que pesan mas (con `|t| < 2`, `e^-4 = 0.018`), no las
        // primeras que aparezcan (de lejos califican mas, y las cercanas
        // ahi ya no pesan nada).
        let mut specs = [(0.0, 0.0, 0.0); 4];
        let mut n_specs = 0;
        let sigma = 0.30f32.max(0.7 * px);
        for &(ss, column, len) in &self.lamp_spec {
            let t = (s - ss) / len.max(0.5);
            if t.abs() >= 2.0 {
                continue;
            }
            let amp = 1.1 * fast_exp(-t * t) * (0.30 / sigma);
            let spec = ((column - r.center) / r.per_meter, amp, 1.0 / sigma);
            if n_specs < specs.len() {
                specs[n_specs] = spec;
                n_specs += 1;
            } else if let Some(min) = specs.iter_mut().min_by(|a, b| a.1.total_cmp(&b.1)) {
                if min.1 < amp {
                    *min = spec;
                }
            }
        }
        // Las naves cerca de esta fila a lo largo (con la cola de las
        // calaveras): lo que solo depende de `ds` va aqui.
        let mut pools = [PoolRow::default(); 6];
        let mut n_pools = 0;
        for p in &self.pools {
            let ds = s - p.s;
            if ds.abs() >= p.hl + 8.0 || n_pools == pools.len() {
                continue;
            }
            // La elipse de la luz: `hw + 1.5` por `hl + 2`, y la de lo que
            // flota mas recogida, `hw + 0.7` por `hl + 1` (un charco de luz
            // bajo la botella, como el brillo de antes).
            let (lw, ll) = if p.hover { (p.hw + 0.7, p.hl + 1.0) } else { (p.hw + 1.5, p.hl + 2.0) };
            let ey = ds / ll;
            // La sombra de contacto: la huella de la carroceria, de la cola
            // a la trompa con orillas de 0.8 m (`fy`, a lo largo), mas una
            // franja oscura de 0.6 m en la linea de la cola. Desde la camara
            // de persecucion lo que asoma bajo el sprite es piso que queda
            // debajo de la nave (de la mitad de atras; la cola de una nave
            // larga cae fuera de la pantalla): con la sombra solo cerca del
            // centro, el sprite la tapaba entera y no se veia nunca.
            let fy = smoothstep(-p.hl - 0.4, -p.hl + 0.4, ds) * smoothstep(p.hl + 0.4, p.hl - 0.4, ds);
            let lip = (ds + p.hl + 0.05) * (1.0 / 0.3);
            let lip = 0.35 * fast_exp(-lip * lip);
            let behind = -(ds + p.hl);
            let tail = if !p.hover && (-0.6..8.0).contains(&behind) { (0.15 + 0.5 * p.brake) * p.gain * fast_exp(-behind / 2.5) * smoothstep(-0.6, 0.4, behind) * smoothstep(8.0, 4.0, behind) } else { 0.0 };
            pools[n_pools] = PoolRow {
                x: p.x,
                reach: p.hw + 5.0,
                inv_l: 1.0 / lw,
                ey2: ey * ey,
                tint: p.tint * (0.22 * p.gain),
                shadow: !p.hover,
                inv_c: 1.0 / (p.hw * 0.9).max(0.1),
                fy,
                lip,
                tail,
                tail_x: p.hw * 0.75,
            };
            n_pools += 1;
        }
        // La tabla a lo ancho: la luz de los postes, el charco de las naves
        // (su luz, la sombra de contacto y las calaveras) y lo mojado son
        // suaves a lo ancho (cambian en metros), asi que se calculan cada
        // `paso` metros y por pixel se interpolan: de cerca, una fila de
        // 1280 pixeles hace ~100 muestras y no 1280. El paso es de dos
        // pixeles (de lejos, pocas muestras) y nunca menos de lo que cabe.
        let u0 = -(RAIL + 0.5);
        let step = (2.0 * px).max(TABLE_STEP);
        let n = ((-2.0 * u0 / step).ceil() as usize + 2).clamp(2, TABLE);
        let wet_line = NoiseLine::<8>::new(3.0 + u0 * 0.15 - 0.1, s * 0.045);
        // Las manchas, pegadas a `s` y promediadas a lo largo sobre lo que
        // se corre la pista en el frame (ver `NoiseLine::blurred`).
        let patch = NoiseLine::<8>::blurred(u0 * 0.35 - 0.1, s * 0.12, blur * 0.12);
        let pulse = 0.75 + 0.25 * (s * 0.15 - game.wall * 6.0).sin();
        // (En el monton: la fila se mueve entera de funcion en funcion y
        // con la tabla adentro serian kilobytes por fila.)
        let mut table = Vec::with_capacity(n);
        for k in 0..n {
            let u = u0 + k as f32 * step;
            // La luz de los postes: con `h = 8.65` m de alto y
            // `q = h^2 / (r^2 + h^2) = cos^2`, la iluminancia es
            // `cos^3 = q sqrt(q)` (inverso del cuadrado por el coseno), mas
            // fuerte adentro del cono de la lampara (`LAMP_CONE`): el
            // charco tiene borde. A lo largo pesa
            // `0.56 ds^2`: el charco se alarga con la pista.
            let mut light = 0.0;
            for &(uc, ds2) in &lamps[..n_lamps] {
                let du = u - uc;
                let q = LAMP_H2 / (du * du + ds2);
                let cos = q.sqrt();
                light += q * cos * (0.25 + 0.75 * smoothstep(LAMP_CONE.0, LAMP_CONE.1, cos));
            }
            // Las naves: su luz `tinte 0.22 e^(-2.2 e)` con la elipse
            // `e = (du / (hw + 1.5))^2 + (ds / (hl + 2))^2`; la huella
            // `h = e^(-cx^4) fy` con `cx = du / 0.9 hw` (pareja adentro,
            // suave en las orillas), que oscurece `1 - 0.6 h` y tapa su
            // propia luz (`1 - 0.7 h`: las LEDs van por fuera, abajo de la
            // carroceria no llegan); la franja de la cola
            // `1 - 0.35 e^(-((ds + hl + 0.05) / 0.3)^2) e^(-cx^4)`; y las
            // dos calaveras a `0.75 hw` del centro.
            let (mut add, mut tail, mut shade) = (Vec3::ZERO, 0.0, 1.0);
            for p in &pools[..n_pools] {
                let du = u - p.x;
                if du.abs() > p.reach {
                    continue;
                }
                let lx = du * p.inv_l;
                let mut glow = fast_exp(-2.2 * (lx * lx + p.ey2));
                if p.shadow {
                    let cx = du * p.inv_c;
                    let cx2 = cx * cx;
                    let fx = fast_exp(-cx2 * cx2);
                    let foot = fx * p.fy;
                    shade *= (1.0 - 0.6 * foot) * (1.0 - p.lip * fx);
                    glow *= 1.0 - 0.7 * foot;
                }
                add += p.tint * glow;
                if p.tail > 0.0 {
                    let q = (du.abs() - p.tail_x) * (1.0 / 0.45);
                    tail += p.tail * fast_exp(-q * q);
                }
            }
            // Lo mojado: charcos de unos 7 x 22 m.
            let wet = 0.30 + 0.70 * smoothstep(-0.1, 0.35, wet_line.at(u * 0.15 + 3.0));
            // El neon de la baranda tine la orilla, el bordillo y el hombro:
            // `0.2 pulso e^(-1.6 (RAIL - |u|))` desde 1.5 m antes del
            // bordillo.
            let a = u.abs();
            let neon = if a > HALF_ROAD - 1.5 { NEON[(u > 0.0) as usize] * (pulse * 0.2 * fast_exp(-(RAIL - a).max(0.0) * 1.6)) } else { Vec3::ZERO };
            // Las manchas del asfalto (de metros: caben en la tabla).
            table.push([light, wet, patch.at(u * 0.35), neon.x, neon.y, neon.z, add.x, add.y, add.z, tail, shade, 0.0]);
        }
        // El ruido de la fila: la `y` es la misma en toda la fila, asi que
        // se interpolan una vez las esquinas de cada celda a lo ancho.
        let spill = game.arch_spill(s);
        // El destello blanco del neon donde roza la nave propia,
        // `destello e^(-(ds / 5)^2)`.
        let flare: [f32; 2] = std::array::from_fn(|k| {
            let (at, f) = game.scrape[k];
            let q = (s - at) * 0.2;
            if f > 0.0 { f * fast_exp(-q * q) } else { 0.0 }
        });
        // Los anillos de lo que flota: en la elipse de `1.3 hw` por `1.3 hl`
        // alrededor de la nave.
        let mut rings = [RingRow::default(); 2];
        let mut n_rings = 0;
        for p in self.pools.iter().filter(|p| p.hover) {
            let ds = s - p.s;
            let ll = p.hl * 1.3;
            if ds.abs() >= ll * 1.1 || n_rings == rings.len() {
                continue;
            }
            rings[n_rings] = RingRow { x: p.x, inv_w: 1.0 / (p.hw * 1.3), ey: ds / ll, px: r.per_meter * p.hw, ds, gain: (0.5 * p.gain).min(1.0) };
            n_rings += 1;
        }
        let ripple = game.wall * 1.5;
        let resolved = smoothstep(0.6, 0.15, px);
        let grain_on = px < 0.12;
        let grain_amp = 0.18 * smoothstep(0.12, 0.03, px);
        Strip {
            r,
            px,
            dash: band(s, 12.0, 6.0, w),
            eye: tracer(band(s - 8.92, 12.0, 0.16, w)) * smoothstep(0.35, 0.1, px),
            amber: tracer(band(s - 2.92, 12.0, 0.16, w)) * smoothstep(0.35, 0.1, px),
            curb: band(s, 16.0, 8.0, w),
            relief,
            joint,
            joint_glow,
            slab,
            resolved,
            grain_on,
            grain_amp,
            sash_peak: 1.0 - ((along - along.floor()) - 0.5).abs() * 2.0,
            sash_soft,
            sash_col,
            sash_far,
            bumps,
            bump_cov,
            bump_q,
            bump_btn,
            bump_far,
            finish,
            finish_fs,
            specs,
            n_specs,
            table,
            u0,
            inv_step: 1.0 / step,
            n,
            has_pools: n_pools > 0,
            fres: 0.35 + 0.65 * smoothstep(8.0, 160.0, r.depth),
            fres_s,
            pulse,
            spill,
            lit: spill != Vec3::ZERO || self.fw_flash_road != Vec3::ZERO,
            // El grano, pegado a `s` y promediado a lo largo como las
            // manchas (el promedio de las muestras lo apaga con la
            // velocidad).
            grain: if grain_on { NoiseLine::blurred(17.0 - HALF_ROAD * 6.0 - 0.1, s * 6.0, blur * 6.0) } else { NoiseLine::default() },
            flare,
            rings,
            n_rings,
            ripples: [fract(ripple), fract(ripple + 0.5)],
            caustic: (0.7 * game.wall.sin(), game.wall * 0.6),
        }
    }

    fn row(&self, y: usize, row: &mut [u8]) {
        let game = self.game;
        let yf = y as f32 + 0.5;
        // Las bombas cuya caja toca la fila (estan ordenadas por la fila de
        // arriba de su caja; son pocas, asi que se copian a una lista).
        let to = self.sparks.partition_point(|s| s.lo.1 <= yf);
        let sparks: Vec<Spark> = self.sparks[..to].iter().filter(|s| s.hi.1 >= yf).copied().collect();
        let span = sparks.iter().fold((f32::MAX, f32::MIN), |(lo, hi), s| (lo.min(s.lo.0), hi.max(s.hi.0)));
        let ctx = self.row_ctx(y, &sparks, span);
        // Lo que una nave cercana tapa del todo no se pinta: ella lo
        // sobreescribe entero (color y emision) mas abajo. Ni la pista ni lo
        // que va detras de ella (mas lejos que su centro: se pinta antes).
        let mut cut = [(0usize, 0usize, 0.0f32); 8];
        let mut n = 0;
        for item in &self.solid_items {
            let (a, b) = self.solid_cols(item, y, yf);
            if b > a && n < cut.len() {
                cut[n] = (a, b, item.depth);
                n += 1;
            }
        }
        let cut = &mut cut[..n];
        cut.sort_unstable_by_key(|c| c.0);
        match &ctx.near {
            Some(st) => {
                // Se pintan los huecos entre sus tramos.
                let mut from = 0;
                for &(a, b, _) in cut.iter().chain(std::iter::once(&(game.width, game.width, 0.0))) {
                    for x in from..a.max(from) {
                        let (c, e) = self.road(st, x as f32 + 0.5, &ctx, x);
                        write_emit(&mut row[x * 4..x * 4 + 4], c, e);
                    }
                    from = from.max(b);
                }
            }
            None => {
                for x in 0..game.width {
                    let (c, e) = self.background(x, &ctx);
                    write_emit(&mut row[x * 4..x * 4 + 4], c, e);
                }
            }
        }
        self.fireworks_row(y, yf, &ctx, row);
        for item in &self.items {
            if yf < item.top || yf > item.bottom {
                continue;
            }
            self.draw_item_behind(item, y, yf, row, cut);
        }
    }

    /// Las chispas de los cohetes que caen en la fila, solo sobre el fondo
    /// (afuera de las barandas de la pista de la fila y de la de atras):
    /// la distancia `d` (px) a la capsula de cada una da
    /// `color (cubre(d - w) + 0.45 e^(-d / 2w))`, que se suma, y la emision
    /// con su brillo.
    fn fireworks_row(&self, y: usize, yf: f32, ctx: &RowCtx, row: &mut [u8]) {
        let Some(bucket) = self.fw_buckets.get(y / 8) else { return };
        if bucket.is_empty() {
            return;
        }
        let width = self.game.width;
        let near = self.rows[y].map_or((f32::MAX, f32::MIN), |r| (r.center - (RAIL + 0.35) * r.per_meter, r.center + (RAIL + 0.35) * r.per_meter));
        let far = ctx.far_span;
        for &i in bucket {
            let Some(s) = self.fw_segs.get(i as usize) else { continue };
            if yf < s.lo.1 || yf > s.hi.1 {
                continue;
            }
            let x0 = (s.lo.0.max(0.0) as usize).min(width);
            let x1 = ((s.hi.0.max(0.0) as usize) + 1).min(width);
            let (ax, ay) = s.a;
            let (bx, by) = (s.b.0 - ax, s.b.1 - ay);
            let inv_len2 = 1.0 / (bx * bx + by * by).max(1e-4);
            let (reach, inv_glow) = (8.0 * s.w, 1.0 / (2.0 * s.w));
            for x in x0..x1 {
                let xf = x as f32 + 0.5;
                if (xf > near.0 && xf < near.1) || (xf > far.0 && xf < far.1) {
                    continue;
                }
                let t = (((xf - ax) * bx + (yf - ay) * by) * inv_len2).clamp(0.0, 1.0);
                let d = len(xf - ax - bx * t, yf - ay - by * t);
                if d > reach {
                    continue;
                }
                // Mas luz en la cabeza que en la cola.
                // (Al resplandor se le resta lo que vale en el corte.)
                let v = (cover(d - s.w) + 0.45 * (fast_exp(-d * inv_glow) - FW_CUT).max(0.0)) * (s.tail + (1.0 - s.tail) * t);
                let p = &mut row[x * 4..x * 4 + 4];
                write_emit(p, lienzo::read(p) + s.color * v, emission(p).max(s.emit * v.min(1.0)));
            }
        }
    }

    /// Las columnas `[desde, hasta)` de la fila `y` que la nave `item`
    /// cubre opaca del todo (el bilineal de su sprite lee cuatro pixeles
    /// opacos), o un tramo vacio. Si la pista de la fila esta mas cerca que
    /// la cola de la nave (una loma la tapa), ninguna.
    fn solid_cols(&self, item: &Item, y: usize, yf: f32) -> (usize, usize) {
        let game = self.game;
        let ItemKind::Car { id } = item.kind else { return (0, 0) };
        let Some(cd) = self.car_draws.get(id as usize) else { return (0, 0) };
        if !cd.solid || yf < item.top || yf > item.bottom {
            return (0, 0);
        }
        let Some(sprite) = game.car_sprite(cd) else { return (0, 0) };
        let front = item.depth - 1.0 - 0.5 * cd.length;
        if self.rows[y].is_some_and(|r| r.depth < front) || self.far[y].is_some_and(|r| r.depth < front) {
            return (0, 0);
        }
        let scale = item.scale / sprite.scale;
        if !(scale > 0.0) {
            return (0, 0);
        }
        // Las mismas rectas que `draw_car`: `sx = gx dx + bx + ancla_x`,
        // `sy = gy dx + by + ancla_y`, y lo opaco en todas las filas del
        // sprite que cruza la fila de la pantalla (inclinada con el ladeo).
        let inv = 1.0 / scale;
        let dy = yf - item.y;
        let px = item.x + cd.pivot;
        let (gx, gy) = (cd.cos * inv, -cd.sin * inv * cd.stretch);
        let (bx, by) = ((dy * cd.sin + cd.pivot) * inv, (dy * cd.cos + cd.lift) * inv * cd.stretch);
        // De cerca, la recta de `draw_car` lee hasta un pixel del sprite a
        // cada lado, y con el barrido (que solo hay de cerca) otro mas la
        // recta cruda y lo que alcanza el barrido: lo opaco se angosta eso.
        let shrink = if scale > LINE_SCALE { if cd.smear > 0.0 { cd.smear + 2.02 } else { 1.01 } } else { 0.0 };
        let (ax, ay) = (cd.ax, sprite.anchor.1);
        let (d0, d1) = ((-bx - ax) / gx, (sprite.width as f32 - bx - ax) / gx);
        let (s0, s1) = (gy * d0 + by + ay, gy * d1 + by + ay);
        let (c0, c1) = sprite.solid_rows(s0.min(s1), s0.max(s1), cd.lod);
        let (c0, c1) = (c0 + shrink, c1 - shrink);
        if c1 <= c0 {
            return (0, 0);
        }
        let lo = ((c0 - bx - ax) / gx).max(d0);
        let hi = ((c1 - bx - ax) / gx).min(d1);
        if !(hi > lo) {
            return (0, 0);
        }
        // El pixel `x` (centro en `x + 0.5`) cae adentro si
        // `px + lo <= x + 0.5 < px + hi` (con un pelito de holgura).
        let b = ((px + hi - 0.5 - 1e-3).ceil().max(0.0) as usize).min(game.width);
        let a = ((px + lo - 0.5 + 1e-3).ceil().max(0.0) as usize).min(b);
        (a, b)
    }

    /// Lo que el HUD de CPU necesita despues del posproceso (dueno de sus
    /// datos: la vista se consume aqui, ya no existe cuando se pinta).
    fn into_hud(self) -> HudFrame {
        // Los buffers de las mallas quedan para el siguiente frame.
        if let Ok(mut pool) = self.game.mesh_pool.lock() {
            pool.extend(self.meshes.into_iter().map(|m| m.raster));
        }
        HudFrame { dots: self.dots }
    }

    /// Lo que se ve detras de la pista: la ciudad abajo o el cielo. Da el
    /// color en pantalla y cuanto emite (para el brillo del posproceso).
    fn background(&self, x: usize, ctx: &RowCtx) -> (Vec3, f32) {
        match self.ground(x, ctx.yoff) {
            Some(g) => g,
            None => self.sky(x, ctx),
        }
    }

    /// El rayo de la columna `x` con `yoff` pixeles sobre el horizonte
    /// contra la esfera del planeta, de radio `R` y con la camara a `H`
    /// sobre ella. Con `sin p` lo que baja el rayo, la distancia al suelo
    /// resuelve `t^2 - 2 b t + c = 0` con `b = (R + H) sin p` y
    /// `c = 2 R H + H^2`: `t = b - sqrt(b^2 - c) = c / (b + sqrt(b^2 - c))`
    /// (la segunda forma no resta numeros parecidos en `f32`). Sin raiz
    /// real el rayo pasa por encima del limbo y se ve el cielo. Da `t`, el
    /// punto en el suelo `(gx, gz)` y el seno del angulo rasante,
    /// `sqrt(b^2 - c) / R`.
    #[inline]
    fn ray(&self, x: usize, yoff: f32) -> Option<(f32, f32, f32, f32)> {
        if yoff >= 0.0 {
            return None;
        }
        let c = self.cols[x];
        let inv_hyp = 1.0 / (c.k * c.k + yoff * yoff).sqrt();
        let sinp = -yoff * inv_hyp;
        let b = (R_PLANET + self.cam.alt) * sinp;
        // b^2 - c factorizado: (b - sqrt c)(b + sqrt c), exacto cerca del
        // limbo donde b y sqrt c casi son iguales.
        let root_c = self.root_c;
        let disc = (b - root_c) * (b + root_c);
        if disc <= 0.0 {
            return None;
        }
        let root = disc.sqrt();
        let t = root_c * root_c / (b + root);
        // Lo horizontal del rayo hasta el suelo, a lo largo de su rumbo.
        let th = t * c.k * inv_hyp;
        Some((t, self.cam.world.0 + th * c.sin_y, self.cam.world.1 + th * c.cos_y, root * (1.0 / R_PLANET)))
    }

    /// El nivel de detalle: el pixel mide `t / f` de lado a lado y se
    /// estira `1 / sin(rasante)` a lo largo del rayo. La media geometrica
    /// de los dos lados, `log2(t) - log2(f * metros) - log2(sin) / 2`, se
    /// queda corta de lejos (submuestrea a lo largo del rayo y las luces de
    /// un texel reptan de fila en fila al avanzar), asi que se toma
    /// `max(geometrica, a lo largo - 1)`: sube `-log2(sin) / 2 - 1` cuando
    /// el rasante baja de ~0.25.
    #[inline]
    fn lod(&self, t: f32, graze: f32) -> f32 {
        let half = -0.5 * fast_log2(graze.max(1e-3));
        fast_log2(t) - self.lod_bias + half + (half - 1.0).max(0.0)
    }

    /// La capital bajo la pista, con la neblina violeta de la ciudad, y
    /// cuanto emite: las luces, `clamp((luma - 0.15) 1.2, 0, 0.6)` con la
    /// luma de la luz lineal (lo oscuro no florece; las calles de sodio
    /// dan un brillo ancho y tenue, naranja).
    #[inline]
    fn ground(&self, x: usize, yoff: f32) -> Option<(Vec3, f32)> {
        let (t, gx, gz, graze) = self.ray(x, yoff)?;
        let lod = self.lod(t, graze);
        let city = &self.game.ciudad;
        let mut col = city.sample(gx, gz, lod);
        // De cerca, las luces titilan un poco (por el aire caliente).
        // Cada texel con su fase y su ritmo (de 0.3 a 1.4 vueltas por
        // segundo) sacados de un hash corto, y una onda triangular.
        if lod < 2.0 {
            let (ix, iz) = ((gx * (1.0 / ciudad::METROS)) as i32 as u32, (gz * (1.0 / ciudad::METROS)) as i32 as u32);
            let mut hash = ix.wrapping_mul(0x9E37_79B1) ^ iz.wrapping_mul(0x85EB_CA77);
            hash ^= hash >> 15;
            let phase = (hash & 0xFFFF) as f32 * (1.0 / 65536.0) + self.game.wall * (0.3 + 0.16 * ((hash >> 16) & 7) as f32);
            let wave = (phase - phase.floor()) * 4.0 - 2.0;
            let wave = 1.0 - wave.abs();
            col = col * (1.0 + 0.3 * wave * (2.0 - lod).min(1.0));
        }
        // La neblina: violeta oscuro, `f = 1 - e^(-t / 9000)`. Pegado al
        // limbo (los ultimos ~2 px, donde el rasante tiende a 0) se funde
        // con el cielo del limbo, `0.85 e^(-sin / 0.01)`, y no queda un
        // escalon contra el domo. Mas la mitad del destello del Fuego.
        let rim = if graze < 0.07 { 0.85 * fast_exp(-graze * 100.0) } else { 0.0 };
        let haze = CITY_HAZE.lerp(self.haze_lin, rim) + FLASH_COLOR * (0.5 * self.vol[x].flash);
        let f = 1.0 - fast_exp(-t * (1.0 / 9000.0));
        let lin = col.lerp(haze, f.max(rim));
        let e = ((lin.luminance() - 0.15) * 1.2).clamp(0.0, 0.6);
        Some((tone_fast(self.lut, lin), e))
    }

    /// El cielo arriba del limbo: el panorama mezclado en modo pantalla con
    /// el gradiente (`1 - (1 - a)(1 - b)`, que no lava las estrellas), los
    /// volcanes, el Fuego y la estrella fugaz. Todo lo que brilla (destello,
    /// halo del crater, estrella fugaz y bombas) se suma en lineal al
    /// gradiente antes del tono. Da tambien cuanto emite: lo que brilla
    /// emite segun su intensidad, `min(I, 1)` (la estrella fugaz al doble),
    /// y el cielo mismo no.
    fn sky(&self, x: usize, ctx: &RowCtx) -> (Vec3, f32) {
        let c = self.cols[x];
        let vc = self.vol[x];
        let pitch = fast_atan2(ctx.yoff, c.k);
        let eps = pitch + self.limb;
        let xf = x as f32 + 0.5;
        let yf = self.cam.horizon - ctx.yoff;
        let pano = self.game.panorama.sample(c.yaw, pitch);
        // Luz lineal que se suma al gradiente.
        let mut extra = Vec3::ZERO;
        let dpsi = c.psi - FUEGO.psi;
        let summit = FUEGO.height - self.limb;
        // El destello: por columna y cayendo con la altura sobre la cima,
        // `e^(-max(p - cima, 0) / 0.1)`: un fogonazo sobre el volcan y no
        // un velo en toda la columna.
        if vc.flash > 0.0 {
            extra = extra + FLASH_COLOR * (vc.flash * ctx.flash_up);
        }
        // El Fuego: su caja en rumbo y elevacion (el penacho se va a la
        // izquierda con el viento).
        let fuego = (-0.5..0.16).contains(&dpsi) && pitch > summit - 0.2 && pitch < summit + 0.36;
        let mut halo_c = Vec3::ZERO;
        if fuego {
            let (dx, dy) = (dpsi * c.k, (pitch - summit) * c.k);
            let d2 = dx * dx + dy * dy;
            if d2 < 50.0 * 50.0 {
                halo_c = Vec3::new(2.5, 0.8, 0.15) * (self.glow * fast_exp(-d2.sqrt() / 6.0));
                extra = extra + halo_c;
            }
        }
        let mut emit = halo_c.luminance();
        // La estrella fugaz: distancia a la capsula cabeza-cola, mas brillo
        // en la cabeza. El halo se apaga antes del borde de su caja.
        if let Some(m) = self.meteor {
            if xf > m.lo.0 && xf < m.hi.0 && yf > m.lo.1 && yf < m.hi.1 {
                let (ax, ay) = (m.tail.0, m.tail.1);
                let (bx, by) = (m.head.0 - ax, m.head.1 - ay);
                let t = (((xf - ax) * bx + (yf - ay) * by) / (bx * bx + by * by).max(1e-4)).clamp(0.0, 1.0);
                let d = (xf - ax - bx * t).hypot(yf - ay - by * t);
                let glow = 0.3 * fast_exp(-d / 1.5) * smoothstep(11.0, 9.0, d);
                let light = m.fade * (cover(d - 0.6) + glow) * t;
                extra = extra + Vec3::new(0.85, 0.9, 1.0) * light;
                emit += 2.0 * light;
            }
        }
        // Las bombas: cada una una capsula de la cola a la cabeza, con un
        // nucleo de 0.8 px en la cabeza que se adelgaza hacia la cola y un
        // halo `0.15 max(e^(-d / 1.5) - e^(-8 / 1.5), 0)` que llega a cero
        // en 8 px (sin borde).
        let mut spark = Vec3::ZERO;
        if xf > ctx.span.0 && xf < ctx.span.1 {
            for s in ctx.sparks {
                if xf < s.lo.0 || xf > s.hi.0 {
                    continue;
                }
                let (ax, ay) = s.tail;
                let (bx, by) = (s.head.0 - ax, s.head.1 - ay);
                let t = (((xf - ax) * bx + (yf - ay) * by) / (bx * bx + by * by).max(1e-4)).clamp(0.0, 1.0);
                let d = (xf - ax - bx * t).hypot(yf - ay - by * t);
                if d < 8.0 {
                    let core = cover(d - 0.8 * (0.35 + 0.65 * t));
                    let glow = 0.15 * (fast_exp(-d / 1.5) - SPARK_CUT).max(0.0);
                    spark = spark + s.color * ((core + glow) * (0.25 + 0.75 * t));
                }
            }
            extra = extra + spark;
            emit += spark.luminance();
        }
        // El destello de los cohetes que acaban de reventar.
        extra = extra + self.fw_flash;
        let gradient = if extra.x + extra.y + extra.z > 1e-4 {
            tone_fast(self.lut, sky_sample(&self.game.sky_lin_lut, eps) + extra)
        } else {
            sky_sample(&self.game.sky_lut, eps)
        };
        let mut color = Vec3::ONE - (Vec3::ONE - pano) * (Vec3::ONE - gradient);
        // El penacho del Fuego: se dobla con el viento hacia la izquierda
        // (`x' = x + 0.35 y^1.4`) y se deshace en fbm. Cerca del crater el
        // umbral baja (`0.35 - 0.3 e^(-y / 1.5)`) para que la columna salga
        // entera y se rompa arriba. Ceniza oscura y fria, con el naranja
        // del crater solo abajo (`e^(-1.5 y)`).
        if fuego {
            let (px, py) = (dpsi / 0.04, (pitch - summit) / 0.04);
            let width = 0.6 + 0.35 * py;
            // `py^1.4 = 2^(1.4 log2 py)`, solo donde el penacho puede caer.
            let bend = if py > 0.0 && px < width { 0.35 * fast_exp(1.4 * std::f32::consts::LN_2 * fast_log2(py)) } else { 0.0 };
            let bent = px + bend;
            if width > 0.05 && bent.abs() < width && py < 8.0 {
                let env = smoothstep(width, 0.0, bent.abs());
                let n = fbm2(0.9 * bent, 0.9 * py - 0.25 * self.game.wall) * 0.5 + 0.5;
                let up = py.max(0.0);
                let dens = env * smoothstep(0.35 - 0.3 * fast_exp(-up / 1.5), 0.7, n);
                if dens > 0.0 {
                    let lit = Vec3::new(1.0, 0.3, 0.06) * (0.6 * self.glow * fast_exp(-1.5 * up)) + Vec3::new(0.006, 0.006, 0.01);
                    let a = 0.9 * dens * smoothstep(8.0, 4.0, py);
                    color = over(color, tone_fast(self.lut, lit), a);
                    emit *= 1.0 - a;
                }
            }
        }
        // Los volcanes, encima, con el borde suavizado un pixel.
        if vc.id != 0 {
            let below = (vc.top - eps) * c.k;
            if below > -0.5 {
                let v = VOLCANOES[(vc.id - 1) as usize];
                let u = (c.psi - v.psi) / if c.psi < v.psi { v.left } else { v.right };
                let up = eps.max(0.0);
                let mut body = Vec3::new(0.004, 0.005, 0.011).lerp(self.haze_lin, 0.5 * fast_exp(-up * (1.0 / 0.035)));
                body = body + sky_sample(&self.game.sky_lin_lut, eps) * (1.5 * cover(below - 1.5));
                body = body + Vec3::new(0.03, 0.012, 0.004) * (0.5 * fast_exp(-up * (1.0 / 0.06)));
                body = body * (0.85 + 0.15 * value_noise(u * 40.0, eps * 400.0));
                body = body + halo_c + spark;
                // Los rios de lava del Fuego, solo sobre su ladera, y
                // apareciendo un poco bajo la silueta (no cortados en ella).
                let inside = vc.fuego - eps;
                let mut hot = halo_c + spark;
                if fuego && inside > 0.0 {
                    let lava = self.lava(dpsi / FUEGO.right, (summit - pitch) / FUEGO.height) * smoothstep(0.0, 0.012, inside);
                    body = body + lava;
                    hot = hot + lava;
                }
                color = over(color, tone_fast(self.lut, body), below + 0.5);
                let a = (below + 0.5).clamp(0.0, 1.0);
                emit = emit + (hot.luminance() - emit) * a;
            }
        }
        (color, emit.min(1.0))
    }

    /// Los tres rios de lava del Fuego: `u` de lado (en anchos del cono) y
    /// `v` hacia abajo (0 en la cima, 1 en el limbo). Cada rio nace un poco
    /// bajo el crater, se aparta de el (`a_k smoothstep(0, 0.35, v)`),
    /// serpentea con su propia frecuencia y fase (`0.04 sin(w_k v + f_k)`
    /// mas ruido), se ensancha al bajar (`0.006 + 0.012 v`), se afina en
    /// los ultimos 0.1 de su largo `L_k` (que crece con cada explosion) y
    /// el ruido lo corta en tramos.
    fn lava(&self, u: f32, v: f32) -> Vec3 {
        if !(0.0..1.0).contains(&v) {
            return Vec3::ZERO;
        }
        const RIVERS: [(f32, f32, f32, f32); 3] = [(-0.16, 5.3, 0.4, 0.05), (0.03, 7.4, 2.3, 0.07), (0.17, 8.8, 4.1, 0.04)];
        let mut sum = Vec3::ZERO;
        for (k, (a, freq, phase, start)) in RIVERS.into_iter().enumerate() {
            let kf = k as f32;
            let length = self.flows[k];
            if v < start || v > length {
                continue;
            }
            let center = a * smoothstep(0.0, 0.35, v) + 0.04 * (freq * v + phase).sin() + 0.02 * value_noise(5.0 * v, kf);
            let taper = smoothstep(length, length - 0.1, v) * smoothstep(start, start + 0.04, v);
            let width = (0.006 + 0.012 * v) * (0.3 + 0.7 * taper);
            let across = (u - center) / width;
            if across.abs() > 3.0 {
                continue;
            }
            let gaps = smoothstep(-0.35, 0.05, value_noise(9.0 * v + 3.0 * kf, kf + 20.0));
            let flicker = 0.6 + 0.4 * value_noise(12.0 * v - 0.5 * self.game.wall, kf + 10.0);
            let i = fast_exp(-across * across) * taper * gaps * flicker;
            let hot = Vec3::new(2.5, 0.9, 0.2).lerp(Vec3::new(0.8, 0.08, 0.01), (v / length).min(1.0));
            sum = sum + hot * i;
        }
        sum
    }

    /// Un pixel de la fila de la pista (o lo que se ve a los lados).
    ///
    /// Va siempre en linea dentro del ciclo de la fila; lo que solo
    /// depende de la fila de pista ya viene en `st`. La pista de atras va
    /// aparte, en una funcion que no se mete en linea (son pocos pixeles).
    /// Da el color y cuanto emite.
    #[inline(always)]
    fn road(&self, st: &Strip, x: f32, ctx: &RowCtx, column: usize) -> (Vec3, f32) {
        let r = st.r;
        let u = (x - r.center) / r.per_meter;
        let a = u.abs();
        // Afuera de las barandas: la pista de atras si asoma ahi, o el
        // fondo, con un halo corto del neon (el ancho lo pone el brillo
        // del posproceso; este no emite).
        if a > RAIL + 0.35 {
            let (back, e) = if x > ctx.far_span.0 && x < ctx.far_span.1 { self.far_road(x, ctx) } else { self.background(column, ctx) };
            let side = (u > 0.0) as usize;
            let neon = if st.flare[side] > 0.0 { NEON[side].lerp(Vec3::ONE, 0.6 * st.flare[side]) } else { NEON[side] };
            let glow = neon * (0.45 * halo((a - RAIL - 0.35) * r.per_meter, 6.0 + 0.5 * r.per_meter));
            return (back + glow * (1.0 - ctx.fog), e);
        }
        self.surface(st, u, x, ctx.fog, ctx)
    }

    /// La pista de atras que asoma al lado de la de adelante.
    #[inline(never)]
    fn far_road(&self, x: f32, ctx: &RowCtx) -> (Vec3, f32) {
        match &ctx.far_strip {
            Some(f) => self.surface(f, (x - f.r.center) / f.r.per_meter, x, ctx.far_fog, ctx),
            None => (Vec3::ZERO, 0.0),
        }
    }

    /// La pista misma entre las barandas, a `u` metros del centro (en la
    /// columna `x` de la pantalla), ya con su neblina `fog`, y cuanto emite.
    ///
    /// - **Material**: asfalto con un tono por losa, manchas y grano (que
    ///   se estira a lo largo con la velocidad), carrileras mas lisas donde
    ///   pisan las llantas, rayas, juntas cian que destellan cada 32 m,
    ///   tachas (ojos de gato) y tumulos. Todo lo pintado a lo largo se
    ///   filtra en caja sobre lo que abarca la fila (`band`), asi de lejos
    ///   se promedia en vez de saltar de fila en fila, y con la velocidad se
    ///   estira (las tachas se vuelven trazadoras).
    /// - **Luz**: cada poste deja un charco con borde, `cos^3` (la ley del
    ///   inverso del cuadrado por el coseno) mas un cono, y las naves su
    ///   charco del color de sus LEDs, su sombra de contacto y el rojo de
    ///   las calaveras atras.
    /// - **Mojado**: `wet` sale de un ruido grande (charcos de unos 7 x 22 m)
    ///   y de las carrileras. Se le suman la estela de todo lo que emite
    ///   (del frame anterior), la raya de cada poste y el cielo con Fresnel.
    #[inline(always)]
    fn surface(&self, st: &Strip, u: f32, x: f32, fog: f32, ctx: &RowCtx) -> (Vec3, f32) {
        let r = st.r;
        let a = u.abs();
        let px = st.px;
        let side = (u > 0.0) as usize;
        let haze = self.haze_row[ctx.y];
        if a > RAIL {
            // La baranda de luz: un tubo de neon con su color de pantalla
            // (no el tono de `2.2 NEON`, que se recortaba casi a blanco),
            // mas oscuro en las orillas del tubo, `0.55 + 0.45 (1 - t^2)`
            // con `t` de -1 a 1 a lo ancho, y una linea fina casi blanca al
            // centro (la parte mas caliente del gas). De lejos, cuando el
            // tubo mide pocos pixeles, la linea se apaga y queda el color.
            let pulse = st.pulse;
            let t = (a - RAIL) * (2.0 / 0.35) - 1.0;
            // Donde roza la nave propia, el gas se pone blanco.
            let neon = if st.flare[side] > 0.0 { NEON[side].lerp(Vec3::ONE, 0.6 * st.flare[side]) } else { NEON[side] };
            let mut c = neon * ((0.55 + 0.45 * (1.0 - t * t)) * (0.8 + 0.2 * pulse));
            let tube_px = 0.35 * r.per_meter;
            if tube_px > 1.5 {
                let core = cover(t.abs() * 0.5 * tube_px - (0.06 * tube_px).max(0.4)) * smoothstep(1.5, 6.0, tube_px) * (0.5 + 0.4 * pulse);
                c = c.lerp(neon * 0.25 + Vec3::splat(0.75), core);
            }
            return (c.lerp(haze, fog), 0.9 * pulse * (1.0 - fog));
        }
        // Lo de la tabla a lo ancho de la fila (ver `strip`): la luz de los
        // postes (antes del material: el grano la recoge), el charco de las
        // naves y lo mojado.
        // Por grupos: lo de siempre (luz, mojado y manchas), el neon solo
        // cerca de la baranda y las naves solo si alguna alumbra la fila.
        let f = ((u - st.u0) * st.inv_step).clamp(0.0, st.n as f32 - 1.001);
        let i = (f as usize).min(st.n - 2);
        let t = f - i as f32;
        let (lo, hi) = match (st.table.get(i), st.table.get(i + 1)) {
            (Some(lo), Some(hi)) => (lo, hi),
            _ => (&TABLE_EMPTY, &TABLE_EMPTY),
        };
        let at = |k: usize| lo[k] + (hi[k] - lo[k]) * t;
        let light = at(0);
        let mut e = 0.0;
        let mut tm = 0.0;
        // Cuanto refleja el material: el asfalto mojado todo, el bordillo
        // pintado un poco menos y la faja del hombro (tela) casi nada.
        let mut gloss = 1.0;
        let mut c;
        if a > HALF_ROAD + 0.7 {
            // El hombro: una faja tejida de rombos amarillos y verdes sobre
            // rojo (`tt` de 0 al centro a 1 en las orillas, el rombo cubre
            // donde `tt < pico`), suavizado medio pixel y, de lejos, su
            // promedio.
            let v = (a - HALF_ROAD - 0.7) * (1.0 / 0.7);
            let tt = (v - 0.5).abs() * 2.0;
            let sash = transition::HUIPIL_RED.lerp(st.sash_col, smoothstep(st.sash_peak + st.sash_soft, st.sash_peak - st.sash_soft, tt));
            c = sash.lerp(SASH_AVG, st.sash_far) * 0.6;
            gloss = 0.25;
        } else if a > HALF_ROAD {
            // El bordillo celeste y blanco con relieve; el filo de afuera
            // en sombra.
            let shade = if a > HALF_ROAD + 0.6 { st.relief * 0.6 } else { st.relief };
            c = CURB_WHITE.lerp(CURB_BLUE, st.curb) * (shade * 0.8);
            // (Refleja menos que el asfalto: con la estela entera, el neon
            // de la baranda lavaba el celeste y blanco.)
            gloss = 0.35;
        } else {
            // El asfalto. Las carrileras: a 0.85 m del centro de cada
            // carril, donde pisan las llantas, mas oscuras y lisas.
            // (Con `floor` y no `rem_euclid`: el `%` de flotantes es una
            // llamada a la biblioteca de C, lenta para cada pixel.)
            let w = (u + HALF_ROAD) * (1.0 / LANE);
            let l = (w - w.floor() - 0.5) * LANE;
            tm = smoothstep(0.42, 0.12, (l.abs() - 0.85).abs());
            let patches = at(2);
            let grain = if st.grain_on { st.grain.at(u * 6.0 + 17.0) * st.grain_amp * (1.0 + light) } else { 0.0 };
            c = st.slab * ((1.0 + (0.25 * patches + grain) * st.resolved) * (1.0 - 0.22 * tm));
            // Las rayas entre carriles (6 m de 12, filtradas a lo largo) y
            // la orilla amarilla. A lo ancho, una raya mas angosta que un
            // pixel no cubre mas que su ancho en pixeles.
            let lane = (u * (1.0 / LANE)).round().clamp(-1.0, 1.0) * LANE;
            if st.dash > 0.0 {
                let d = ((u - lane).abs() - 0.12) * r.per_meter;
                c = over(c, LANE_PAINT, cover(d).min(0.24 * r.per_meter) * st.dash);
            }
            let edge = ((a - (HALF_ROAD - 0.35)).abs() - 0.1) * r.per_meter;
            c = over(c, EDGE_PAINT, cover(edge).min(0.2 * r.per_meter));
            // La junta cian que destella: un hilo con un resplandor leve.
            c = over(c, JOINT * 1.1, st.joint * 0.7) + JOINT * (0.05 * st.joint_glow * (1.0 - fog));
            e = 0.3 * st.joint;
            // Los ojos de gato en el hueco de las rayas (blancos) y en las
            // orillas (ambar): 10 cm de ancho y al menos un pixel.
            let half = 0.05f32.max(0.5 * px);
            if st.eye > 0.0 {
                let v = cover(((u - lane).abs() - half) * r.per_meter) * st.eye;
                c = c + Vec3::new(0.9, 0.95, 1.0) * (1.6 * v * (1.0 - fog));
                e += v;
            }
            if st.amber > 0.0 {
                let v = cover(((a - (HALF_ROAD - 0.35)).abs() - half) * r.per_meter) * st.amber;
                c = c + Vec3::new(1.0, 0.7, 0.15) * (1.6 * v * (1.0 - fog));
                e += v;
            }
            // Los tumulos: chevrones amarillos y negros (la cara de
            // adelante iluminada, la de atras en sombra) con botones ambar
            // al centro; de lejos, su promedio y su cobertura a lo largo.
            if st.bump_cov > 0.0 {
                let k = (((u + HALF_ROAD) / LANE).floor() as i32).clamp(0, 3);
                if st.bumps & (1 << k) != 0 {
                    let face = if st.bump_q < 0.5 { 1.35 } else { 0.6 };
                    let chevron = if fract(u * 1.2 + st.bump_q * 2.0) < 0.5 { BUMP_YELLOW * face } else { BUMP_BLACK };
                    let avg = (BUMP_YELLOW * 0.975 + BUMP_BLACK) * 0.5;
                    c = over(c, chevron.lerp(avg, st.bump_far), st.bump_cov);
                    if st.bump_btn > 0.0 {
                        let b = cover(((fract(u * 2.0) - 0.5).abs() * 0.5 - 0.06) * r.per_meter) * st.bump_btn;
                        c = c + Vec3::new(1.0, 0.7, 0.15) * (2.0 * b);
                        e += b;
                    }
                }
            }
            // La meta: cuadros de 1 m, filtrados a lo largo (y a lo ancho
            // cuando un cuadro mide menos de unos pixeles).
            if st.finish > 0.0 {
                let even = fract(u * 0.5) < 0.5;
                let white = if even { st.finish_fs } else { 1.0 - st.finish_fs };
                let white = white + (0.5 - white) * smoothstep(0.3, 1.0, px);
                c = over(c, Vec3::splat(0.02).lerp(Vec3::splat(0.8), white), st.finish);
            }
        }
        // Lo mojado: charcos de unos 7 x 22 m y las carrileras.
        let wet = (at(1) + 0.35 * tm).min(1.0) * gloss;
        // El charco del poste: luz que se suma y que tine lo que alumbra.
        // Lo mojado devuelve menos difuso, `1.6 (1 - 0.5 wet)` (esa luz se
        // va a la raya del reflejo): asi no queda gris de concreto seco.
        c = c + LAMP_COLOR * (light * 0.22) + c * (light * 1.6 * (1.0 - 0.5 * wet));
        // La luz del arco de foquitos mas cercano y el destello de los
        // cohetes. La del arco es un charco: alumbra el asfalto (su luz por
        // el color de lo que toca, pero ese color tomado hasta 0.12, mas un
        // poco que se suma) y no tine de mas las rayas blancas, que con
        // `c spill` saldrian salmon.
        if st.lit {
            c = c + c.min(Vec3::splat(0.12)) * (st.spill * 1.3) + c * (self.fw_flash_road * 2.0) + st.spill * 0.04;
        }
        // La estela de lo que emite, desde el frame anterior, mas fuerte de
        // lejos (Fresnel de la distancia), y el cielo reflejado con su
        // Fresnel (nunca suma mas de 0.8): los dos de la fila en espacio
        // de pantalla, bilineal a lo ancho.
        if !ctx.screen.is_empty() {
            let last = ctx.screen.len() - 1;
            let f = (x * ctx.screen_scale - 0.5 - ctx.screen_lo as f32).max(0.0);
            let i = (f as usize).min(last);
            let (a0, a1) = (&ctx.screen[i], &ctx.screen[(i + 1).min(last)]);
            let t = (f - i as f32).min(1.0);
            let (streak, sky) = (a0[0].lerp(a1[0], t), a0[1].lerp(a1[1], t));
            c = c + streak * (1.4 * wet * st.fres * (1.0 - fog));
            c = c + (sky * (st.fres_s * (0.25 + 0.75 * wet) * SKY_REFLECT)).min(Vec3::splat(0.8));
        }
        // La raya de cada poste en el piso mojado.
        if st.n_specs > 0 {
            let k = wet * (0.3 + 0.7 * st.fres);
            for &(us, amp, inv) in &st.specs[..st.n_specs] {
                let t = (u - us) * inv;
                if t.abs() < 4.0 {
                    let v = amp * fast_exp(-t * t) * k;
                    c = c + LAMP_COLOR * v;
                    e = e.max(0.4 * v);
                }
            }
        }
        // Las naves: la sombra de contacto (antes de su luz, para que los
        // LEDs no la tapen), su charco de luz (en el bordillo, la mitad) y
        // el rojo de las calaveras atras de ellas.
        if st.has_pools && a <= HALF_ROAD + 0.7 {
            let curb = if a > HALF_ROAD { 0.5 } else { 1.0 };
            c = c * at(10) + Vec3::new(at(6), at(7), at(8)) * ((0.6 + 0.4 * wet) * curb) + BRAKE_RED * (at(9) * curb);
        }
        // El anillo antigravedad de lo que flota: con `e` la distancia en la
        // elipse de `1.3 hw` por `1.3 hl`, un aro cian fino en `e = 0.75`
        // (`cubre((|e - 0.75| - 0.012) hw px)`, de un pixel o poco mas, que
        // emite) con un brillo suave aparte, `0.35 e^(-((e - 0.75) / 0.06)^2)`,
        // todo a 0.3; dos ondas finas que salen del centro (`e = fract(1.5 t
        // + j / 2)`, que se apagan al abrirse) y adentro la caustica ambar de
        // la botella, un ruido que corre, `(1 - e) (0.6 + 0.4 ruido)`. (El
        // aro grueso a 0.5 en `e = 0.92` dominaba el tercio de abajo de la
        // pantalla y lo cortaba la orilla: se leia como calcomania.)
        if st.n_rings > 0 && a <= HALF_ROAD {
            for ring in &st.rings[..st.n_rings] {
                let du = u - ring.x;
                let lx = du * ring.inv_w;
                if lx.abs() >= 1.1 {
                    continue;
                }
                let el = len(lx, ring.ey);
                if el >= 1.1 {
                    continue;
                }
                let off = el - RING_AT;
                let band = cover((off.abs() - 0.012) * ring.px);
                let mut aura = band + 0.35 * fast_exp(-(off * off) * (1.0 / (0.06 * 0.06)));
                for &rk in &st.ripples {
                    aura += cover(((el - rk).abs() - 0.012) * ring.px) * (1.0 - rk) * 0.4;
                }
                c = c + Vec3::new(0.4, 0.9, 1.0) * (0.3 * aura * ring.gain);
                if el < 1.0 {
                    let n = value_noise(du * 2.5 + st.caustic.0, ring.ds * 2.5 + st.caustic.1);
                    c = c + Vec3::new(1.0, 0.6, 0.15) * (0.25 * (1.0 - el) * (0.6 + 0.4 * n) * ring.gain);
                }
                e = e.max(0.5 * band * ring.gain);
            }
        }
        // El neon de la baranda tine el bordillo, el hombro y la orilla.
        if a > HALF_ROAD - 1.5 {
            c = c + Vec3::new(at(3), at(4), at(5));
        }
        // La neblina toma el color del cielo de atras: la pista lejana se
        // funde en el horizonte.
        (c.lerp(haze, fog), e.min(1.0) * (1.0 - fog))
    }

    /// Lo que cae en la fila `y` de un objeto encima de la pista. No se ve
    /// donde la pista que muestra la fila esta mas cerca (una loma lo tapa).
    #[cfg(test)]
    fn draw_item(&self, item: &Item, y: usize, yf: f32, row: &mut [u8]) {
        self.draw_item_behind(item, y, yf, row, &[]);
    }

    /// `draw_item` sabiendo que tramos de la fila tapan del todo las naves
    /// opacas (`cut`: columnas `[desde, hasta)` y la profundidad de la
    /// nave): lo que esta detras de una no se pinta (ella lo sobreescribe
    /// despues), solo se ahorra.
    fn draw_item_behind(&self, item: &Item, y: usize, yf: f32, row: &mut [u8], cut: &[(usize, usize, f32)]) {
        let game = self.game;
        let road = self.rows[y];
        let far = self.far[y];
        // Los tramos de las naves opacas mas cerca que esta cosa, en `x` del
        // centro de cada pixel: `[desde + 0.5, hasta + 0.5)`.
        let mut occ = [(0.0f32, 0.0f32); 8];
        let mut n_occ = 0;
        for &(a, b, depth) in cut {
            if depth < item.depth - 1e-3 && n_occ < occ.len() {
                occ[n_occ] = (a as f32 + 0.5, b as f32 + 0.5);
                n_occ += 1;
            }
        }
        let occ = &occ[..n_occ];
        // Lo tapa la pista que esta mas cerca que su punta de adelante (una
        // loma). Para una nave es la cola, medio largo mas cerca que su
        // centro: el sprite visto desde arriba baja en la pantalla hasta
        // la cola, y si se midiera desde el centro la pista de enfrente le
        // cortaria las llantas de atras en recto (la Mamaneta mide 9.6 m).
        // (El fuego del turbo va en la cola de su nave: lo mismo.)
        let front = item.depth
            - 1.0
            - match item.kind {
                ItemKind::Car { id } => self.car_draws.get(id as usize).map_or(0.0, |c| 0.5 * c.length),
                ItemKind::Flame { id } => self.flames.get(id as usize).map_or(0.0, |f| 0.5 * f.length),
                _ => 0.0,
            };
        // Lo que tapa cada pista en esta fila es un tramo de `x` (entre
        // sus barandas, `|x - centro| < (RAIL + 0.4) px por metro`), que
        // se saca una vez por fila y no por pixel.
        let reach = |r: Option<RoadRow>| match r {
            Some(r) if r.depth < front => (r.center - (RAIL + 0.4) * r.per_meter, r.center + (RAIL + 0.4) * r.per_meter),
            _ => (f32::MAX, f32::MIN),
        };
        let (near_road, far_road) = (reach(road), reach(far));
        let hidden = |x: f32| (x > near_road.0 && x < near_road.1) || (x > far_road.0 && x < far_road.1) || occ.iter().any(|&(a, b)| x >= a && x < b);
        // La neblina del horizonte se mide en la fila para lo alto (la
        // torre, los barriletes, los postes y los arcos): con la de su pie,
        // que de lejos cae en el horizonte, se borrarian enteros.
        let tall = !matches!(item.kind, ItemKind::Car { .. } | ItemKind::Puff { .. } | ItemKind::Spark { .. } | ItemKind::Flame { .. });
        let fog = fog_at(item.depth).max(horizon_fog(if tall { yf } else { item.y } - self.cam.horizon, item.depth));
        let haze = self.haze_row[y];
        let k = item.scale;
        // Coordenadas en metros alrededor del punto en el suelo (y hacia arriba).
        let my = (item.y - yf) / k;
        match item.kind {
            ItemKind::Car { id } => {
                if yf < item.top || yf > item.bottom {
                    return;
                }
                if let Some(cd) = self.car_draws.get(id as usize) {
                    match cd.mesh.and_then(|m| self.meshes.get(m as usize)) {
                        Some(m) if cd.mix <= 0.0 => self.draw_mesh(item, cd, m, y, row, &hidden, fog, haze),
                        Some(m) => {
                            // Entre las dos: la fila con la malla y con el
                            // sprite, cada una sobre el mismo fondo, y su
                            // promedio por el peso del sprite.
                            BLEND.with(|cell| {
                                let mut scratch = cell.borrow_mut();
                                let (back, with_mesh) = &mut *scratch;
                                back.clear();
                                back.extend_from_slice(row);
                                self.draw_mesh(item, cd, m, y, row, &hidden, fog, haze);
                                with_mesh.clear();
                                with_mesh.extend_from_slice(row);
                                row.copy_from_slice(back);
                                self.draw_car(item, cd, yf, row, (near_road, far_road), occ, fog, haze);
                                let t = cd.mix;
                                for (p, &q) in row.iter_mut().zip(with_mesh.iter()) {
                                    if *p != q {
                                        *p = (q as f32 + (*p as f32 - q as f32) * t + 0.5) as u8;
                                    }
                                }
                            });
                        }
                        None => self.draw_car(item, cd, yf, row, (near_road, far_road), occ, fog, haze),
                    }
                }
            }
            ItemKind::Spark { id } => {
                if yf < item.top || yf > item.bottom {
                    return;
                }
                if let Some(seg) = self.spark_draws.get(id as usize) {
                    self.draw_spark(seg, yf, row, &hidden);
                }
            }
            ItemKind::Flame { id } => {
                if yf < item.top || yf > item.bottom {
                    return;
                }
                if let Some(f) = self.flames.get(id as usize) {
                    self.draw_flame(item, f, yf, row, &hidden, fog);
                }
            }
            ItemKind::Lamp { side } => {
                // Poste de 9 m con su brazo hacia la pista y la lampara: una
                // caja oscura de 1 x 0.32 m con la cara de abajo encendida.
                // En la cara del palo que mira a la pista, una tira LED del
                // color del neon de su baranda con un pulso que sube; abajo
                // de la lampara, su cono de luz en el aire. En cada fila solo
                // se recorre lo que tiene algo.
                if !(-0.3..11.5).contains(&my) {
                    return;
                }
                let spread = 0.8f32.max(3.0 / k);
                let near_head = ((my - 8.5) * 1.5).abs() < 2.6 * spread || (8.4..9.1).contains(&my);
                // El cono: medio ancho `w = 0.3 + 0.36 (8.5 - my)` alrededor
                // de la lampara (a 2.7 m hacia la pista).
                let cone_w = 0.3 + 0.36 * (8.5 - my);
                let below = if my < 8.5 { 2.7 + cone_w } else { 0.0 };
                // Lo que se recorre con todo, en metros hacia la pista: cerca
                // de la lampara, hasta su resplandor (y su cono); abajo, el
                // palo con su tira y el halo de la tira, que llega a cero a
                // 9 px de ella (de 0.105 a 0.175 m). El resto del cono va
                // aparte, solo luz.
                let px = 1.0 / k;
                let strip_on = (0.6..8.3).contains(&my);
                let strip_reach = if strip_on { 10.0 * px } else { 0.0 };
                let (from, to) = if near_head {
                    ((-0.3f32).min(0.105 - strip_reach), (2.7 + 2.6 * spread).max(below).max(0.175 + strip_reach))
                } else if my <= 9.0 {
                    ((-0.3f32).min(0.105 - strip_reach), 0.3f32.max(0.175 + strip_reach))
                } else {
                    return;
                };
                // De metros hacia la pista a columnas de la pantalla.
                let columns = |from: f32, to: f32| {
                    let (a, b) = (item.x - side * from * k, item.x - side * to * k);
                    ((a.min(b) - 1.0).max(0.0) as usize, ((a.max(b) + 1.0).max(0.0) as usize).min(game.width))
                };
                let (x0, x1) = columns(from, to);
                let neon = NEON[(side > 0.0) as usize];
                // La tira: 7 cm en `mx = 0.14` con un pulso que corre hacia
                // arriba. Mas angosta que un pixel se le quita la luz que
                // sobra (`0.1 / px`), si no de lejos seria una raya gruesa.
                // Su halo es `e^(-d / 3) - e^-3` (llega a cero a los 9 px).
                let strip_col = neon * (0.6 + 0.4 * (my * 1.6 - game.wall * 9.0).sin());
                let thin = (0.1 / px).min(1.0);
                // La cara encendida: al menos 1.2 px de alto.
                let lens_h = 0.05f32.max(0.6 * px);
                // El cono en el aire: `0.07 (1 - q)^2 sqrt(my / 8.5)` con
                // `q = |mx - 2.7| / w` (mas tenue al bajar).
                let cone = if my > 0.0 && my < 8.5 { 0.07 * (my / 8.5).sqrt() * (1.0 - fog) } else { 0.0 };
                let steel_col = Vec3::new(0.06, 0.06, 0.07);
                for x in x0..x1 {
                    let xf = x as f32 + 0.5;
                    if hidden(xf) {
                        continue;
                    }
                    // Hacia la pista es -side.
                    let mx = (xf - item.x) / k * -side;
                    let pole = ((mx.abs() - 0.12).max(my - 9.0).max(-my)) / px;
                    let arm = ((my - 8.85).abs() - 0.1).max((mx - 1.5).abs() - 1.5) / px;
                    let head = ((my - 8.65).abs() - 0.16).max((mx - 2.7).abs() - 0.5) / px;
                    let lens = ((my - 8.49 - lens_h).abs() - lens_h).max((mx - 2.7).abs() - 0.44) / px;
                    let p = &mut row[x * 4..x * 4 + 4];
                    let (mut c, mut e) = (lienzo::read(p), emission(p));
                    if cone > 0.0 {
                        let q = (mx - 2.7).abs() / cone_w;
                        if q < 1.0 {
                            c = c + LAMP_COLOR * (cone * (1.0 - q) * (1.0 - q));
                        }
                    }
                    // El palo y el brazo; el 30 % del palo del lado de la
                    // pista lleva un filo del neon.
                    let steel = cover(pole.min(arm));
                    let edge = if my < 8.7 { 0.25 * smoothstep(0.03, 0.06, mx) } else { 0.0 };
                    c = over(c, (steel_col + neon * edge).lerp(haze, fog), steel);
                    e *= 1.0 - steel;
                    if strip_on {
                        let d = ((mx - 0.14).abs() - 0.035) / px;
                        let on = cover(d) * thin;
                        c = over(c, strip_col * 1.4, on) + strip_col * (0.35 * (halo(d, 3.0) - STRIP_CUT).max(0.0) * thin * (1.0 - fog));
                        e = e.max(on * (1.0 - fog));
                    }
                    // La lampara: la caja oscura y su cara encendida, apenas
                    // mas clara al centro.
                    let house = cover(head);
                    c = over(c, Vec3::new(0.07, 0.07, 0.08).lerp(haze, fog), house);
                    let lit = cover(lens);
                    let hot = smoothstep(0.44, 0.0, (mx - 2.7).abs());
                    c = over(c, Vec3::new(1.0, 0.85, 0.6).lerp(Vec3::new(1.0, 0.95, 0.85), 0.5 * hot), lit);
                    e = (e * (1.0 - house)).max(lit * (1.0 - fog));
                    // El resplandor: gaussiano en metros (y nunca menos de
                    // unos pixeles); a 2.6 anchos ya no se ve. Afuera de la
                    // caja y sobre la cara encendida.
                    let dm = len(mx - 2.7, (my - 8.5) * 1.5) / spread;
                    let glow = (-dm * dm).exp() * (1.0 - fog) * (1.0 - house * (1.0 - lit));
                    c = c + LAMP_COLOR * (0.55 * glow);
                    write_emit(p, c, e.max(0.5 * glow));
                }
                // El cono abajo de la lampara, lejos del palo: solo luz que
                // se suma (de cerca son cientos de pixeles por fila).
                if !near_head && cone > 0.0 {
                    let (c0, c1) = columns((2.7 - cone_w).max(0.3), 2.7 + cone_w);
                    for x in c0..c1 {
                        let xf = x as f32 + 0.5;
                        if (x0..x1).contains(&x) || hidden(xf) {
                            continue;
                        }
                        let q = ((xf - item.x) / k * -side - 2.7).abs() / cone_w;
                        if q < 1.0 {
                            let p = &mut row[x * 4..x * 4 + 4];
                            write_emit(p, lienzo::read(p) + LAMP_COLOR * (cone * (1.0 - q) * (1.0 - q)), emission(p));
                        }
                    }
                }
            }
            ItemKind::Kite { design, size, id, squash, lift } => {
                // Barrilete de Sumpango: la rueda horneada (el mip que toca
                // por su tamano en la pantalla y el siguiente, mezclados),
                // con un bamboleo `0.05 sin(0.9 t + id)` y puesta de canto
                // (se angosta `squash` a lo ancho). En vivo, los flecos de
                // papel que ondean en el anillo `1 < r < 1.12 + 0.035
                // sin(9 a + 5 t + id)`, dos colas que culebrean y la cana
                // que la sostiene desde el suelo. De lejos (menos de ~40 px
                // de diametro) los flecos y las colas se desvanecen
                // (`detalle`) y se lee un solo mip.
                let Some(mips) = game.kites.get(design as usize % game.kites.len().max(1)) else { return };
                let Some(first) = mips.first() else { return };
                let rad = 0.5 * size * k;
                if rad < 0.75 {
                    return;
                }
                let (cx, cy) = (item.x, item.y - lift * k);
                let rx = rad * squash;
                let theta = 0.05 * (game.wall * 0.9 + id as f32).sin();
                let (st, ct) = theta.sin_cos();
                let dy = yf - cy;
                let px = 1.0 / k;
                // Las colas: desde abajo del disco hasta 1.2 veces su tamano,
                // sin pasar de la altura de la pista. `ty` son los metros
                // bajo el disco.
                let ty = (dy - rad) / k;
                let detail = smoothstep(12.0, 20.0, rad);
                let disk = dy.abs() < rad * if detail > 0.0 { 1.2 } else { KITE_EXTENT };
                let pole = (-0.1..=lift).contains(&my);
                let tails = detail > 0.0 && ty > -0.5 && ty < 1.2 * size && my > 0.0;
                if !(disk || pole || tails) {
                    return;
                }
                // El mip: cuantos texeles caen en un pixel a lo ancho (lo
                // mas apretado), `log2(texeles por unidad / pixeles por
                // unidad)`, y la mezcla con el siguiente (sin saltos al
                // acercarse).
                let lod = (first.scale / rx.max(1e-3)).max(1.0).log2();
                let (l0, blend) = if detail < 1.0 { ((lod.round() as usize).min(mips.len() - 1), 0.0) } else { ((lod.floor() as usize).min(mips.len() - 1), lod.fract()) };
                let l1 = (l0 + 1).min(mips.len() - 1);
                let blend = if l1 > l0 { blend } else { 0.0 };
                let sample = |m: &Sprite, u: f32, v: f32| m.sample(m.anchor.0 + u * m.scale, m.anchor.1 + v * m.scale);
                // Las tiras de los flecos, cada `2 pi / 90`: si miden menos
                // de 3 px se van a su promedio (de lejos no titilan).
                let strips = smoothstep(1.5, 4.0, TAU / 90.0 * rx);
                let tail_col = [PAPEL[id as usize % 6] * 0.55, PAPEL[(id as usize + 3) % 6] * 0.55];
                let thin = (0.2 / px).min(1.0);
                let tail_fade = smoothstep(1.2 * size, 0.7 * size, ty) * smoothstep(-0.5, 0.0, ty) * detail;
                let wave = |j: f32| 0.5 * (my * 1.1 + game.wall * 4.0 + j * 1.7).sin();
                let tail_x = [-0.18 * size + wave(0.0), 0.18 * size + wave(1.0)];
                // Lo que se recorre en la fila, alrededor de `cx`: la cuerda
                // del disco con sus flecos (`rx sqrt(1.21^2 - (dy / R)^2)`,
                // mas lo que lo gira el bamboleo), la cana y cada cola.
                let mut ranges = [(0.0f32, 0.0f32); 4];
                let mut n = 0;
                if disk {
                    let q = dy / rad;
                    let outer = if detail > 0.0 { 1.21 } else { KITE_EXTENT };
                    let hw = rx * (outer * outer - q * q).max(0.0).sqrt() + theta.abs() * rad + 2.0;
                    ranges[n] = (-hw, hw);
                    n += 1;
                }
                if pole {
                    ranges[n] = (-(0.075 * k + 2.0), 0.075 * k + 2.0);
                    n += 1;
                }
                if tails && tail_fade > 0.0 {
                    for tx in tail_x {
                        ranges[n] = (tx * k - 0.1 * k - 2.0, tx * k + 0.1 * k + 2.0);
                        n += 1;
                    }
                }
                let ranges = &mut ranges[..n];
                ranges.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                let mut from = 0usize;
                for &(a, b) in ranges.iter() {
                    let x0 = ((cx + a).max(0.0) as usize).max(from).min(game.width);
                    let x1 = ((cx + b).max(0.0) as usize + 1).min(game.width);
                    from = from.max(x1);
                    for x in x0..x1 {
                        let xf = x as f32 + 0.5;
                        if hidden(xf) {
                            continue;
                        }
                        let dx = xf - cx;
                        let p = &mut row[x * 4..x * 4 + 4];
                        let (mut c, mut e) = (lienzo::read(p), emission(p));
                        if pole {
                            let d = (dx.abs() * px - 0.075).max(my - lift) / px;
                            let cov = cover(d) * (0.15 / px).min(1.0);
                            c = over(c, Vec3::new(0.08, 0.05, 0.03).lerp(haze, fog), cov);
                            e *= 1.0 - cov;
                        }
                        if tails && tail_fade > 0.0 {
                            for (j, &tx) in tail_x.iter().enumerate() {
                                let d = ((dx * px - tx).abs() - 0.1) / px;
                                let cov = cover(d) * thin * 0.9 * tail_fade;
                                if cov > 0.0 {
                                    c = over(c, tail_col[j].lerp(haze, fog), cov);
                                    e = e * (1.0 - cov) + 0.25 * cov * (1.0 - fog);
                                }
                            }
                        }
                        if disk {
                            let (ux, uy) = (dx / rx, dy / rad);
                            let (u, v) = (ux * ct + uy * st, -ux * st + uy * ct);
                            let r2 = u * u + v * v;
                            if r2 < 1.21 * 1.21 {
                                let r1 = r2.sqrt();
                                if r1 > 0.97 && detail > 0.0 {
                                    let ang = fast_atan2(-v, u);
                                    let edge = 1.12 + 0.035 * (9.0 * ang + game.wall * 5.0 + id as f32).sin();
                                    let cov = 0.9 * detail * cover((r1 - edge) * rx);
                                    if cov > 0.0 {
                                        let k_strip = ((ang + PI) * (90.0 / TAU)) as usize + id as usize;
                                        let col = PAPEL_AVG.lerp(PAPEL[k_strip % 6], strips) * 0.5;
                                        c = over(c, col.lerp(haze, fog), cov);
                                        e = e * (1.0 - cov) + 0.3 * cov * (1.0 - fog);
                                    }
                                }
                                if r1 < KITE_EXTENT {
                                    let mut s = sample(&mips[l0], u, v);
                                    if blend > 0.0 {
                                        let s1 = sample(&mips[l1], u, v);
                                        s = std::array::from_fn(|i| s[i] + (s1[i] - s[i]) * blend);
                                    }
                                    let a = s[3];
                                    let src = Vec3::new(s[0], s[1], s[2]).lerp(haze * a, fog);
                                    c = src + c * (1.0 - a);
                                    e = (e * (1.0 - a) + 0.45 * src.x.max(src.y).max(src.z) * (1.0 - fog)).min(1.0);
                                }
                            }
                        }
                        write_emit(p, c, e);
                    }
                }
            }
            ItemKind::Tower { lights, t, green, finished } => {
                self.draw_tower(item, (lights, t, green, finished), my, row, &hidden, fog, haze);
            }
            ItemKind::Arch { id, squash, signal } => {
                // Arco de foquitos de feria: 41 focos en la semielipse de
                // medio ancho `a` (angostado `squash` de canto) y alto `b`,
                // colgados de un cable. Cada foco suma
                // `color (1.8 cubre(d - r) + 0.55 h e^(-(2.2 d / g)^2))`: un
                // nucleo de radio `r` (nunca menos de 1.1 px; de lejos la
                // misma energia repartida, `(0.17 / r)^2`) y un halo de
                // radio `g` (0.6 m, entre 4 y 22 px). De lejos (un pixel de
                // mas de 0.12 a 0.2 m) el halo se apaga (`h`): el brillo del
                // posproceso ya lo abre, y asi cada foco recorre unos
                // pixeles y no un circulo de 9. Por fila solo se miran los
                // focos a su alcance de alto, y solo alrededor de cada uno.
                if !(-0.5..ARCH_B + 1.2).contains(&my) {
                    return;
                }
                let width = game.width;
                let px = 1.0 / k;
                let r = 0.17f32.max(1.1 * px);
                let energy = (0.17 / r).powi(2) * (1.0 - fog);
                if energy < 1e-3 {
                    return;
                }
                let g = (4.0 * px).max(0.6).min(22.0 * px);
                let halo_w = 0.55 * smoothstep(0.2, 0.12, px);
                let reach = if halo_w > 0.0 { g } else { r + px };
                let (a, b) = (ARCH_A * squash, ARCH_B);
                // El cable: la banda `|d| < t` con `d = (hypot(mx / a, my / b) - 1) b`
                // (`t = 0.03 m` mas pixel y medio). En la fila cae entre las
                // elipses `hypot = 1 -+ t / b`: de `x_in = a sqrt((1 - t/b)^2 - (my/b)^2)`
                // a `x_out = a sqrt((1 + t/b)^2 - (my/b)^2)` de cada lado, y
                // arriba, donde la de adentro ya no llega, de lado a lado.
                // Lo alumbran los focos: un tono calido tenue que emite
                // un poco (sobre el cielo oscuro un cable negro no se ve).
                let t = 0.03 + 1.5 * px;
                if my > -t && my < b + t {
                    let q = (my / b).powi(2);
                    let x_out = a * ((1.0 + t / b).powi(2) - q).max(0.0).sqrt();
                    let x_in = a * ((1.0 - t / b).powi(2) - q).max(0.0).sqrt();
                    let spans = if x_in > 0.0 { [(-x_out, -x_in), (x_in, x_out)] } else { [(-x_out, x_out), (0.0, 0.0)] };
                    let cable = (Vec3::new(0.32, 0.22, 0.26) * 0.35).lerp(haze, fog);
                    let thin = (0.06 / px).min(1.0);
                    for (lo, hi) in spans {
                        if hi <= lo {
                            continue;
                        }
                        let x0 = ((item.x + lo * k).floor().max(0.0) as usize).min(width);
                        let x1 = ((item.x + hi * k).ceil().max(0.0) as usize + 1).min(width);
                        for x in x0..x1 {
                            let xf = x as f32 + 0.5;
                            if hidden(xf) {
                                continue;
                            }
                            let mx = (xf - item.x) * px;
                            let d = (len(mx / a, my / b) - 1.0) * b;
                            let cov = cover((d.abs() - 0.03) / px) * thin;
                            if cov > 0.0 {
                                let p = &mut row[x * 4..x * 4 + 4];
                                write_emit(p, over(lienzo::read(p), cable, cov), emission(p).max(0.15 * cov * (1.0 - fog)));
                            }
                        }
                    }
                }
                let wall9 = (game.wall * 9.0).floor() as i64;
                let inv_g = 2.2 / g;
                for (j, &(cos, sin)) in arch_bulbs().iter().enumerate() {
                    let (bx, by) = (-a * cos, b * sin);
                    if (my - by).abs() > reach {
                        continue;
                    }
                    let color = match signal {
                        1 => Vec3::new(1.0, 0.1, 0.08),
                        2 => Vec3::new(0.2, 1.0, 0.35),
                        _ => ARCH_PAL[(j as i64 + 2 * id as i64 + wall9).rem_euclid(5) as usize],
                    };
                    let bright = 0.65 + 0.35 * (game.wall * 7.0 - 0.6 * j as f32).sin();
                    let amp = color * (bright * energy);
                    let x0 = ((item.x + (bx - reach) * k).floor().max(0.0) as usize).min(width);
                    let x1 = ((item.x + (bx + reach) * k).ceil().max(0.0) as usize + 1).min(width);
                    for x in x0..x1 {
                        let xf = x as f32 + 0.5;
                        if hidden(xf) {
                            continue;
                        }
                        let dm = len((xf - item.x) * px - bx, my - by);
                        let cov = cover((dm - r) / px);
                        let v = if halo_w > 0.0 {
                            let q = dm * inv_g;
                            1.8 * cov + halo_w * fast_exp(-q * q)
                        } else {
                            1.8 * cov
                        };
                        if v < 1e-3 {
                            continue;
                        }
                        let p = &mut row[x * 4..x * 4 + 4];
                        write_emit(p, lienzo::read(p) + amp * v, emission(p).max(cov * (1.0 - fog)));
                    }
                }
            }
            ItemKind::Puff { id } => {
                let Some(pd) = self.puff_draws.get(id as usize) else { return };
                self.draw_puff(item, pd, yf, row, &hidden);
            }
        }
    }

    /// La fila `yf` de una nave (ver `CarDraw`): su sprite con el mip que
    /// toca, la luz de sus LEDs animada por su tabla, lo que la alumbra y su
    /// emision.
    ///
    /// - **LEDs**: `src = cuerpo + luz`, con la luz ya animada en cada
    ///   esquina del bilineal (ver `hornear::bilinear8`), y emite
    ///   `smoothstep(0.25, 0.85, max(luz))`. Si `src` pasa de 1 se divide
    ///   por su maximo (y no se recorta cada canal): un stop que estalla al
    ///   frenar sigue rojo y una ambar sigue ambar, no se van a blanco.
    /// - **Ladeo y salto**: el pixel se gira alrededor del pivote antes de
    ///   leer el sprite (ver `CarDraw`).
    /// - **Re-iluminacion** (solo el cuerpo, donde hay alfa): el poste mas
    ///   cercano la barre de arriba, `LAMP 0.15 poste (0.3 + 0.7 (1 - ty))`,
    ///   y el neon de cada baranda pinta su costado,
    ///   `NEON 0.35 baranda smoothstep(...)` con `tx` a lo ancho del sprite;
    ///   con tope suave, `cuerpo (1 - e^(-x))`.
    /// - **Barrido**: con `smear` (solo de cerca, por la recta), el promedio
    ///   de cuatro muestras de `sx - smear` a `sx + smear` (ocho si el
    ///   barrido pasa de 4 px de la pantalla: con cuatro las orillas salian
    ///   en peine).
    #[allow(clippy::too_many_arguments)]
    fn draw_car(&self, item: &Item, cd: &CarDraw, yf: f32, row: &mut [u8], roads: ((f32, f32), (f32, f32)), occ: &[(f32, f32)], fog: f32, haze: Vec3) {
        let game = self.game;
        let Some(a) = game.car_sprite(cd) else { return };
        let Some(lut) = self.luts.get(item_lut(item)) else { return };
        let k = item.scale;
        let scale = k / a.scale;
        if !(scale > 0.0) || cd.near <= 0.0 {
            return;
        }
        let inv = 1.0 / scale;
        let (sin, cos) = (cd.sin, cd.cos);
        let dy = yf - item.y;
        // En la fila, `sx` y `sy` son lineales en `dx = x - P.x`:
        // `sx = gx dx + bx + ancla_x`, `sy = gy dx + by + ancla_y`.
        let px = item.x + cd.pivot;
        let (gx, gy) = (cos * inv, -sin * inv * cd.stretch);
        let (bx, by) = ((dy * sin + cd.pivot) * inv, (dy * cos + cd.lift) * inv * cd.stretch);
        let (ax, ay) = (cd.ax, a.anchor.1);
        // Las columnas: donde el sprite cae adentro de su rectangulo (con el
        // barrido) y su fila tiene algo.
        let (w, h) = (a.width as f32, a.height as f32);
        let (mut lo, mut hi) = ((-cd.smear - bx - ax) / gx, (w + cd.smear - bx - ax) / gx);
        if gy.abs() > 1e-6 {
            let (t0, t1) = ((-by - ay) / gy, (h - by - ay) / gy);
            lo = lo.max(t0.min(t1));
            hi = hi.min(t0.max(t1));
        } else if !(0.0..=h).contains(&(by + ay)) {
            return;
        }
        if !(hi > lo) {
            return;
        }
        let (sy0, sy1) = (gy * lo + by + ay, gy * hi + by + ay);
        let (c0, c1) = a.span_rows(sy0.min(sy1), sy0.max(sy1), cd.lod);
        if c1 <= c0 {
            return;
        }
        lo = lo.max((c0 - cd.smear - bx - ax) / gx);
        hi = hi.min((c1 + cd.smear - bx - ax) / gx);
        if !(hi > lo) {
            return;
        }
        let x0 = ((px + lo).floor().max(0.0) as usize).min(game.width);
        let x1 = (((px + hi).ceil().max(0.0) as usize) + 1).min(game.width);
        let (near_road, far_road) = roads;
        let hidden = |x: f32| (x > near_road.0 && x < near_road.1) || (x > far_road.0 && x < far_road.1) || occ.iter().any(|&(a, b)| x >= a && x < b);
        // La re-iluminacion de la fila: el poste por la altura en el sprite
        // (`ty`, casi la misma en toda la fila) y el neon por `tx` a lo
        // ancho. Como `1 - e^(-(poste + neon)) = 1 - e^(-poste) e^(-neon)`,
        // la parte del neon viene tabulada por nave (`CarDraw::neon`) y en
        // la fila solo se saca la del poste.
        let ty = ((by + ay) / h).clamp(0.0, 1.0);
        let lamp = LAMP_COLOR * (RELIGHT_LAMP * cd.lamp * (0.3 + 0.7 * (1.0 - ty)));
        let lamp_keep = Vec3::new(fast_exp(-lamp.x), fast_exp(-lamp.y), fast_exp(-lamp.z));
        // (Lo que suma menos de un 5 % no se calcula.)
        let relight = cd.lamp > 0.1 || cd.rail.0 > 0.1 || cd.rail.1 > 0.1;
        let to_tap = RELIGHT_TAPS as f32 / a.width.max(1) as f32;
        let spill = Vec3::ONE + cd.spill * 0.6;
        let fade = cd.near * (1.0 - fog);
        let smear = cd.smear;
        // De cerca (el sprite agrandado mas del doble) la recta que la fila
        // cruza en el sprite se muestrea una vez por pixel del sprite (cada
        // `step` pixeles de la pantalla) y cada pixel interpola entre las dos
        // muestras vecinas: cada pixel del sprite cubre varios de la
        // pantalla, asi se lee una vez y no varias. El barrido se hace sobre
        // esa recta una vez por fila y a cada pixel le queda una sola
        // interpolacion. Las rectas van en dos buffers de cada hilo que se
        // reusan (nada de memoria nueva por fila).
        let step = 1.0 / gx.abs().max(gy.abs()).max(1e-6);
        let inv_step = 1.0 / step;
        let start = (x0 as f32 + 0.5 - px) - step;
        let n = (((x1 as f32 - 0.5 - px) + step - start) * inv_step).ceil().max(0.0) as usize + 2;
        let smear_px = smear / gx;
        let extra = if smear > 0.0 { (smear_px * inv_step).ceil() as usize + 1 } else { 0 };
        let line = scale > LINE_SCALE && n + 2 * extra <= LINE_MAX;
        // (El barrido solo pasa de 2.2 de escala: siempre va por la recta.)
        debug_assert!(smear == 0.0 || scale > LINE_SCALE);
        LINES.with(|cell| {
            let mut lines = cell.borrow_mut();
            let (mixed, raw_line) = &mut *lines;
            if line {
                let sample_at = |dx: f32| car_texel(a, cd, lut, gx * dx + bx + ax, gy * dx + by + ay);
                mixed.clear();
                if smear <= 0.0 {
                    mixed.extend((0..n).map(|j| sample_at(start + j as f32 * step)));
                } else {
                    // La recta cruda, de `smear_px` antes a `smear_px`
                    // despues, y en cada muestra el promedio de `taps`
                    // puntos parejos de `-smear_px` a `smear_px`.
                    let raw_start = start - extra as f32 * step;
                    raw_line.clear();
                    raw_line.extend((0..n + 2 * extra).map(|j| sample_at(raw_start + j as f32 * step)));
                    let taps = if smear_px.abs() > 4.0 { 8 } else { 4 };
                    let (span, weight) = (2.0 * smear_px / (taps - 1) as f32, 1.0 / taps as f32);
                    mixed.extend((0..n).map(|j| {
                        let mut t = Texel::default();
                        let from = start + j as f32 * step - smear_px;
                        for q in 0..taps {
                            let s = line_at(raw_line, raw_start, inv_step, from + q as f32 * span);
                            for c in 0..4 {
                                t.body[c] += s.body[c] * weight;
                            }
                            for c in 0..3 {
                                t.light[c] += s.light[c] * weight;
                            }
                        }
                        t
                    }));
                }
            }
            for x in x0..x1 {
                let xf = x as f32 + 0.5;
                if hidden(xf) {
                    continue;
                }
                let dx = xf - px;
                let sxa = gx * dx + bx + ax;
                let t = if line { line_at(mixed, start, inv_step, dx) } else { car_texel(a, cd, lut, sxa, gy * dx + by + ay) };
                let alpha = t.body[3];
                let top = t.light[0].max(t.light[1]).max(t.light[2]);
                if alpha <= 0.0 && t.body[0] + t.body[1] + t.body[2] <= 0.0 && top <= 0.0 {
                    continue;
                }
                let base = Vec3::new(t.body[0], t.body[1], t.body[2]);
                let mut src = base * spill;
                if relight && alpha > 0.0 {
                    // El poste de arriba y el neon de los lados, con tope
                    // suave `1 - e^(-x)` sobre lo que se suma.
                    let f = (sxa * to_tap).clamp(0.0, RELIGHT_TAPS as f32 - 0.001);
                    let i = (f as usize).min(RELIGHT_TAPS - 1);
                    let keep = cd.neon[i].lerp(cd.neon[i + 1], f - i as f32) * lamp_keep;
                    src += base * (Vec3::ONE - keep);
                }
                src += Vec3::new(t.light[0], t.light[1], t.light[2]);
                let peak = src.x.max(src.y).max(src.z);
                if peak > 1.0 {
                    src = src * (1.0 / peak);
                }
                // Emite lo que brilla de verdad (los focos), no el halo tenue:
                // `smoothstep(0.25, 0.85, max(luz))` (si no, el brillo del
                // posproceso abre un velo sobre toda la camioneta).
                let q = ((top - 0.25) * (1.0 / 0.6)).clamp(0.0, 1.0);
                let e_src = q * q * (3.0 - 2.0 * q) * fade;
                let a = alpha * cd.near;
                // De lejos se pierde en la neblina.
                let src = (src * cd.near).lerp(haze * a, fog);
                let p = &mut row[x * 4..x * 4 + 4];
                if a >= 1.0 {
                    // Opaca: tapa todo lo de atras (sin leerlo).
                    write_emit(p, src, e_src);
                } else {
                    let back = lienzo::read(p);
                    write_emit(p, src + back * (1.0 - a), e_src + emission(p) * (1.0 - a));
                }
            }
        });
    }

    /// La fila `y` de una nave en 3D (ver `MeshDraw`): lo mas cercano de
    /// su malla en cada pixel se sombrea una vez, como una muestra de su
    /// sprite (`hornear::Solid::texel`, con la normal vista desde la camara
    /// del horneado), y despues todo lo de `draw_car`: la luz de los arcos,
    /// el poste de arriba y el neon de los lados, sus LEDs animadas por su
    /// tabla, lo que emite y la neblina.
    #[allow(clippy::too_many_arguments)]
    fn draw_mesh(&self, item: &Item, cd: &CarDraw, m: &MeshDraw, y: usize, row: &mut [u8], hidden: &dyn Fn(f32) -> bool, fog: f32, haze: Vec3) {
        let game = self.game;
        let Some(solid) = game.solids.get(m.solid) else { return };
        let Some(lut) = self.luts.get(item_lut(item)) else { return };
        if cd.near <= 0.0 {
            return;
        }
        let spill = Vec3::ONE + cd.spill * 0.6;
        let fade = cd.near * (1.0 - fog);
        let relight = cd.lamp > 0.1 || cd.rail.0 > 0.1 || cd.rail.1 > 0.1;
        let to_tap = RELIGHT_TAPS as f32 / (m.span.1 - m.span.0);
        let a = cd.near;
        let halo = m.halo.as_ref();
        HITS.with(|cell| {
            let mut scratch = cell.borrow_mut();
            let (hits, line) = &mut *scratch;
            if hits.len() < game.width {
                hits.resize(game.width, Hit::default());
            }
            let (t0, t1) = m.raster.row_nearest(y, hits);
            // Lo que toca la malla y lo que alcanza su halo.
            let yf = y as f32 + 0.5;
            let halo = halo.filter(|h| h.covers_row(yf));
            if let Some(h) = halo {
                h.line(yf, line);
            }
            let (h0, h1) = halo.map_or((t0, t0), |h| h.cols(game.width));
            let (x0, x1) = if t1 > t0 { (t0.min(h0), t1.max(h1)) } else { (h0, h1) };
            for x in x0..x1 {
                let hit = if x >= t0 && x < t1 { std::mem::take(&mut hits[x]) } else { Hit::default() };
                let xf = x as f32 + 0.5;
                if hidden(xf) {
                    continue;
                }
                let glow = halo.map_or(Vec3::ZERO, |h| h.line_at(line, xf));
                if hit.depth == f32::MAX {
                    // Afuera de la nave: solo su halo, que se suma.
                    let light = glow * lut[0];
                    let top = light.x.max(light.y).max(light.z);
                    if top < 1.0 / 512.0 {
                        continue;
                    }
                    let qe = ((top - 0.25) * (1.0 / 0.6)).clamp(0.0, 1.0);
                    let p = &mut row[x * 4..x * 4 + 4];
                    write_emit(p, light * fade + lienzo::read(p), qe * qe * (3.0 - 2.0 * qe) * fade + emission(p));
                    continue;
                }
                // La luz y la emision interpoladas de sus vertices, con tono.
                let f = m.raster.sample(&hit);
                let height = f.world.y - m.origin.y;
                let (base, led) = hornear::pixel(f.a, f.b, f.tag);
                let light = (led + glow) * lut[f.tag as usize];
                let mut src = base * spill;
                if relight {
                    let lamp = LAMP_COLOR * (RELIGHT_LAMP * cd.lamp * (0.3 + 0.7 * (height / solid.tall).clamp(0.0, 1.0)));
                    let lamp_keep = Vec3::new(fast_exp(-lamp.x), fast_exp(-lamp.y), fast_exp(-lamp.z));
                    let t = ((xf - m.span.0) * to_tap).clamp(0.0, RELIGHT_TAPS as f32 - 0.001);
                    let i = (t as usize).min(RELIGHT_TAPS - 1);
                    let keep = cd.neon[i].lerp(cd.neon[i + 1], t - i as f32) * lamp_keep;
                    src += base * (Vec3::ONE - keep);
                }
                src += light;
                let peak = src.x.max(src.y).max(src.z);
                if peak > 1.0 {
                    src = src * (1.0 / peak);
                }
                let top = light.x.max(light.y).max(light.z);
                let qe = ((top - 0.25) * (1.0 / 0.6)).clamp(0.0, 1.0);
                let e_src = qe * qe * (3.0 - 2.0 * qe) * fade;
                let src = (src * a).lerp(haze * a, fog);
                let p = &mut row[x * 4..x * 4 + 4];
                if a >= 1.0 {
                    write_emit(p, src, e_src);
                } else {
                    let back = lienzo::read(p);
                    write_emit(p, src + back * (1.0 - a), e_src + emission(p) * (1.0 - a));
                }
            }
        });
    }

    /// La fila `yf` de una chispa de la pista: la distancia `d` (px) a su
    /// capsula da `color (cubre(d - w) + 0.45 e^(-d / 2w))` (mas en la
    /// cabeza que en la cola), que se suma, y su emision.
    fn draw_spark(&self, s: &SparkSeg, yf: f32, row: &mut [u8], hidden: &dyn Fn(f32) -> bool) {
        let width = self.game.width;
        let x0 = (s.lo.0.max(0.0) as usize).min(width);
        let x1 = ((s.hi.0.max(0.0) as usize) + 1).min(width);
        let (ax, ay) = s.a;
        let (bx, by) = (s.b.0 - ax, s.b.1 - ay);
        let inv_len2 = 1.0 / (bx * bx + by * by).max(1e-4);
        let (reach, inv_glow) = (8.0 * s.w, 1.0 / (2.0 * s.w));
        for x in x0..x1 {
            let xf = x as f32 + 0.5;
            if hidden(xf) {
                continue;
            }
            let t = (((xf - ax) * bx + (yf - ay) * by) * inv_len2).clamp(0.0, 1.0);
            let d = len(xf - ax - bx * t, yf - ay - by * t);
            if d > reach {
                continue;
            }
            let v = (cover(d - s.w) + 0.45 * (fast_exp(-d * inv_glow) - FW_CUT).max(0.0)) * (s.tail + (1.0 - s.tail) * t);
            let p = &mut row[x * 4..x * 4 + 4];
            write_emit(p, lienzo::read(p) + s.color * v, emission(p).max(s.emit * v.min(1.0)));
        }
    }

    /// La fila `yf` del fuego del turbo: un cono de la boca del escape `A`
    /// (el punto del `Item`) a la punta `B` (ver `view`), con radio
    /// `r(t) = r0 + (r1 - r0) t` a lo largo. Para cada pixel `p`, `t` es su
    /// proyeccion en el eje, `(p - A) . (B - A) / |B - A|^2` recortada de 0 a
    /// 1, y `q = |p - (A + t (B - A))| / r(t)` su distancia al eje en
    /// radios:
    /// `I = nucleo e^(-6 q^2) (1 - t)^2 + orilla e^(-2.5 q^2) (1 - t) (0.55 + 0.45 ruido)`,
    /// luz que se suma y emite. Si la suma pasa de 1 se divide por su
    /// maximo (el nucleo queda amarillo y no blanco quemado). El ruido corre
    /// a lo largo del chorro con el tiempo: la llama tiembla. Cada clase el
    /// suyo:
    ///
    /// - colectivo: blanco con orilla magenta y anillos de choque que
    ///   cruzan el cono a lo largo del eje (`smoothstep(0.7, 1, fract(4 t -
    ///   20 tiempo))`, los diamantes de un escape de cohete), del ancho del
    ///   cono en cada punto;
    /// - camioneta: amarillo con orilla naranja (y bocanadas negras por las
    ///   chimeneas);
    /// - chela: espuma en puntitos que salen por el chorro;
    /// - tuk-tuk: una brasa chica (lo suyo son las chispas).
    fn draw_flame(&self, item: &Item, f: &FlameDraw, yf: f32, row: &mut [u8], hidden: &dyn Fn(f32) -> bool, fog: f32) {
        let game = self.game;
        let big = f.r0.max(f.r1);
        if big < 0.5 {
            return;
        }
        let (ax, ay) = (item.x, item.y);
        let (bx, by) = (f.tip.0 - ax, f.tip.1 - ay);
        let len2 = bx * bx + by * by;
        let inv_len2 = if len2 > 1e-4 { 1.0 / len2 } else { 0.0 };
        let reach = big * 1.6 + 1.0;
        if yf < ay.min(f.tip.1) - reach || yf > ay.max(f.tip.1) + reach {
            return;
        }
        let x0 = ((ax.min(f.tip.0) - reach).max(0.0) as usize).min(game.width);
        let x1 = (((ax.max(f.tip.0) + reach).max(0.0) as usize) + 1).min(game.width);
        let (core, edge) = match f.kind {
            Kind::Colectivo => (Vec3::new(1.0, 0.9, 0.9), Vec3::new(0.8, 0.2, 1.0)),
            Kind::Camioneta => (Vec3::new(1.0, 0.85, 0.45), Vec3::new(1.0, 0.4, 0.05)),
            Kind::Chela => (Vec3::new(1.0, 0.9, 0.7), Vec3::new(1.0, 0.85, 0.55)),
            Kind::TukTuk => (Vec3::new(1.0, 0.7, 0.3) * 0.8, Vec3::new(1.0, 0.35, 0.05) * 0.6),
        };
        let wall = game.wall;
        let power = f.power * (1.0 - fog);
        let dy = yf - ay;
        for x in x0..x1 {
            let xf = x as f32 + 0.5;
            if hidden(xf) {
                continue;
            }
            let dx = xf - ax;
            let t = ((dx * bx + dy * by) * inv_len2).clamp(0.0, 1.0);
            let (ex, ey) = (dx - bx * t, dy - by * t);
            let rr = f.r0 + (f.r1 - f.r0) * t;
            let q2 = (ex * ex + ey * ey) / (rr * rr).max(0.25);
            if q2 >= 2.56 {
                continue;
            }
            // De que lado del eje y a cuantos radios (para el ruido).
            let side = (dx * by - dy * bx).signum() * q2.sqrt();
            let fall = 1.0 - t;
            let flicker = 0.55 + 0.45 * value_noise(t * 5.0 - wall * 25.0 + f.seed, side * 1.5 + wall * 7.0);
            let mut light = core * (fast_exp(-6.0 * q2) * fall * fall) + edge * (fast_exp(-2.5 * q2) * fall * flicker);
            match f.kind {
                Kind::Colectivo => {
                    let ring = smoothstep(0.7, 1.0, fract(t * 4.0 - wall * 20.0)) * fast_exp(-3.0 * q2) * fall;
                    light += Vec3::new(1.0, 0.85, 1.0) * (0.8 * ring);
                }
                Kind::Chela => {
                    // Espuma: puntitos en celdas que corren por el chorro.
                    let (qx, qy) = (t * 7.0 - wall * 7.0 + f.seed, side * 2.0);
                    let (cx, cy) = (qx.floor(), qy.floor());
                    let h = hash_u32((cx as i32 as u32).wrapping_mul(0x9E37_79B9) ^ (cy as i32 as u32).wrapping_mul(0x85EB_CA6B));
                    let (ox, oy) = (0.3 + 0.4 * ((h >> 8) & 0xFF) as f32 / 255.0, 0.3 + 0.4 * ((h >> 16) & 0xFF) as f32 / 255.0);
                    let dot = if (h & 0xFF) < 110 { smoothstep(0.3, 0.15, len(qx - cx - ox, qy - cy - oy)) } else { 0.0 };
                    light = core * (fast_exp(-6.0 * q2) * fall * fall) + edge * (fast_exp(-q2) * dot * fall * 1.4);
                }
                _ => {}
            }
            let light = light * power;
            let top = light.max_element();
            if top < 1e-3 {
                continue;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let sum = lienzo::read(p) + light;
            let peak = sum.max_element();
            write_emit(p, if peak > 1.0 { sum * (1.0 / peak) } else { sum }, emission(p).max(top.min(1.0)));
        }
    }

    /// La fila `yf` de un soplo de humo negro: un disco de radio `R` cuyo
    /// ruido (el del posproceso, girado por su `rot` y corrido por su
    /// semilla) le da volutas. `dens = (0.4 + 0.6 ruido) (1 - d^2)` que nace
    /// en 0.1 s y se va de los 0.5 a los 2.2 s; el color es casi negro,
    /// alumbrado de arriba por el poste cercano (`(0.5 - 0.5 v)`, `v` de -1
    /// arriba a 1 abajo) y de abajo por las LEDs de la camioneta (al nacer)
    /// y por el resplandor naranja de la ciudad. Y la orilla, `d^2`, se
    /// alumbra mas (el poste y la ciudad la atraviesan: es lo que hace leer
    /// el humo negro contra el cielo de noche, que tambien es oscuro).
    /// Encima con `alfa = 0.95 min(1.6 dens, 1)`.
    fn draw_puff(&self, item: &Item, pd: &PuffDraw, yf: f32, row: &mut [u8], hidden: &dyn Fn(f32) -> bool) {
        let game = self.game;
        let radius = pd.radius * item.scale;
        if radius < 0.5 {
            return;
        }
        let dy = (yf - item.y) / radius;
        if dy.abs() >= 1.0 {
            return;
        }
        let age = pd.age;
        // Cerca de la camara (a menos de ~14 m, o con mas de 220 px de
        // radio) se desvanece: el humo de la propia sale adelante y pasa por
        // la camara a la altura del ojo, y de tan cerca taparia media
        // pantalla con cientos de miles de pixeles (ese humo ya lo pinta el
        // de la pantalla).
        let opacity = 0.95 * pd.weight * smoothstep(2.2, 0.5, age) * smoothstep(0.0, 0.1, age) * smoothstep(6.0, 14.0, item.depth) * smoothstep(220.0, 140.0, radius);
        if opacity < 0.004 {
            return;
        }
        let chord = radius * (1.0 - dy * dy).max(0.0).sqrt();
        let x0 = ((item.x - chord).max(0.0) as usize).min(game.width);
        let x1 = (((item.x + chord).max(0.0) as usize) + 1).min(game.width);
        let inv_r = 1.0 / radius;
        let soot = Vec3::new(0.025, 0.022, 0.02);
        let lamp = LAMP_COLOR * (0.35 * pd.lamp);
        let leds = game.tint[Kind::Camioneta.index()] * (0.25 * fast_exp(-2.0 * age));
        let rim_light = LAMP_COLOR * (0.5 * pd.lamp) + CITY_UNDERLIGHT * 2.0 + Vec3::new(0.05, 0.05, 0.08);
        for x in x0..x1 {
            let xf = x as f32 + 0.5;
            if hidden(xf) {
                continue;
            }
            let u = (xf - item.x) * inv_r;
            let d2 = u * u + dy * dy;
            if d2 >= 1.0 {
                continue;
            }
            let (ur, vr) = (u * pd.cos - dy * pd.sin, u * pd.sin + dy * pd.cos);
            let n = game.post.tex(ur * 0.5 + pd.seed, vr * 0.5 + pd.seed * 0.37 + age * 0.3);
            let a = opacity * ((0.4 + 0.6 * n) * (1.0 - d2) * 1.6).min(1.0);
            if a < 0.002 {
                continue;
            }
            let color = soot + lamp * (0.5 - 0.5 * dy) + (leds + CITY_UNDERLIGHT) * (0.5 + 0.5 * dy) + rim_light * (d2 * d2 * (0.6 + 0.4 * n));
            let p = &mut row[x * 4..x * 4 + 4];
            write_emit(p, over(lienzo::read(p), color, a), emission(p) * (1.0 - a));
        }
    }

    /// La fila `yf` (a `my` metros de alto) de la Torre del Reformador sobre
    /// la meta: una torre de celosia de hierro de 66 m con las patas a los
    /// lados de la pista (`tower_outer` y el arco `tower_inner`), tres
    /// plataformas, la caja de la campana arriba con su faro rojo que
    /// parpadea, la manta de la meta a cuadros con orillas de faja entre las
    /// patas y, colgado de ella, el semaforo. Los focos LED que la delinean
    /// (cada 1.5 m en el borde de afuera y en el arco) son tambien el
    /// semaforo: blancos que respiran antes de arrancar, rojos por tercios
    /// de lo que se ve desde la parrilla con la cuenta (las patas de 0 a 6,
    /// de 6 a 12 y de 12 para arriba; el arco, cuatro focos por tercio),
    /// toda verde con un destello al arrancar, una persecucion celeste y
    /// blanca en la carrera y dorada al terminar.
    ///
    /// La celosia son dos familias de diagonales,
    /// `min(|fract((|mx| + my) / 2.4) - 0.5|, |fract((|mx| - my) / 2.4) - 0.5|) 2.4 < 0.09`;
    /// cuando un pixel mide mas de ~12 cm se va a un hierro parejo al 30 %
    /// (si no, la reja de lejos haria moire). Las barras mas angostas que
    /// un pixel se pintan de un pixel con su cobertura (`ancho / px`): asi
    /// de lejos no se rompen en puntos. Y el hierro se va al color de la
    /// neblina con la distancia antes que lo demas (perspectiva aerea).
    ///
    /// Por fila solo se recorren las patas, la corona, la manta, el
    /// semaforo y el faro donde caen; lo que depende solo de la fila (las
    /// plataformas, el color de cada foco, la altura de cada luz del
    /// semaforo, los focos del arco que alcanzan la fila) se saca antes.
    #[allow(clippy::too_many_arguments)]
    fn draw_tower(&self, item: &Item, state: (u8, f32, f32, bool), my: f32, row: &mut [u8], hidden: &dyn Fn(f32) -> bool, fog: f32, haze: Vec3) {
        let game = self.game;
        let (lights, _, green, finished) = state;
        if my < -0.3 {
            return;
        }
        let k = item.scale;
        let px = 1.0 / k;
        let g = 3.0 * px;
        let wo = tower_outer(my);
        let wi = tower_inner(my);
        let body = my < 64.0;
        let spread = beacon_spread(k);
        let beacon_reach = 4.0 * spread * px;
        // El halo de los focos del semaforo, `8 + 0.6 k` px (hasta 26),
        // hasta 3 veces eso (se le resta `e^-3`: llega a cero sin borde).
        let sem_spread = (8.0 + 0.6 * k).min(26.0);
        let sem_reach = 3.0 * sem_spread * px;
        let lattice_far = smoothstep(0.08, 0.16, px);
        let led_r = 0.16f32.max(0.7 * px);
        let led_reach = led_r + 7.0 * px;
        // Una barra de medio ancho `h` a `dist` m de su eje: de un pixel
        // por lo menos, con la cobertura de lo que mide.
        let bar = |dist: f32, h: f32| cover((dist - h.max(0.5 * px)) / px) * (2.0 * h / px).min(1.0);
        // Las plataformas (solo dependen de la fila).
        let platform = [13.0f32, 28.0, 42.0].iter().fold(0.0f32, |acc, f| acc.max(bar((my - f).abs(), 0.35)));
        // El color de un foco: su tercio (de abajo arriba) y la altura
        // `yb` (m) para la persecucion; y cuanto brilla.
        let breathe = 0.6 + 0.4 * (game.wall * 2.0).sin();
        let green_led = Vec3::new(0.2, 1.0, 0.35);
        let led = |tier: u8, yb: f32| -> (Vec3, f32) {
            match lights {
                0 => (Vec3::new(1.0, 0.95, 0.9), breathe),
                1..=3 => (Vec3::new(1.0, 0.1, 0.08), if tier < lights { 1.0 } else { 0.1 }),
                _ if finished => (Vec3::new(1.0, 0.75, 0.3), 1.0),
                _ if green < 1.5 => (green_led, 1.0),
                _ => {
                    let chase = ((yb / 1.5 - game.wall * 6.0).floor() as i64).rem_euclid(2) == 0;
                    (if chase { Vec3::new(0.3, 0.65, 1.0) } else { Vec3::new(1.0, 1.0, 1.0) }, 1.0)
                }
            }
        };
        // Los focos del arco que alcanzan esta fila (de los doce de cada
        // lado, parejos en el angulo de la elipse de semiejes
        // `(RAIL + 0.9, 13)`), con su color: uno a cuatro, nunca un
        // `atan2` por pixel.
        let mut arc: Short<(f32, f32, Vec3, f32), 12> = Short::new();
        if body && my < 13.0 + led_reach {
            for (j, &(bx, by)) in tower_arc_bulbs().iter().enumerate() {
                if (my - by).abs() < led_reach {
                    let (col, on) = led((j / 4) as u8, by);
                    arc.push((bx, by, col.lerp(haze, fog * fog) * on, on));
                }
            }
        }
        // Los tramos de la fila en metros desde el centro.
        let mut spans: Short<(f32, f32), 96> = Short::new();
        let mirror = |spans: &mut Short<(f32, f32), 96>, a: f32, b: f32| {
            spans.push((a, b));
            spans.push((-b, -a));
        };
        if body {
            if lattice_far > 0.0 || platform > 0.0 {
                // De lejos (la celosia es un hierro parejo) o en una
                // plataforma: todo lo que va de las patas.
                if wi > g {
                    mirror(&mut spans, wi - g, wo + g);
                } else {
                    spans.push((-(wo + g), wo + g));
                }
            } else {
                // De cerca, solo donde hay algo: los bordes con el halo de
                // sus focos (7 px), cada diagonal de la celosia (en la fila
                // `|mx| = 2.4 (n + 0.5) -+ my`, con su medio ancho mas un
                // pixel) y los focos del arco. Los huecos de la reja ni se
                // recorren.
                let pad = 1.5 * px;
                mirror(&mut spans, wo - 0.35f32.max(led_reach) - pad, wo + 0.35f32.max(led_reach) + pad);
                if wi > 0.0 {
                    mirror(&mut spans, wi - 0.3 - pad, wi + 0.3 + pad);
                }
                let (lo, hi) = (wi.max(0.0), wo);
                let hw = 0.09 + px;
                for offset in [-my, my] {
                    let first = ((lo - hw - offset) / 2.4 - 0.5).ceil() as i64;
                    let last = ((hi + hw - offset) / 2.4 - 0.5).floor() as i64;
                    for n in first..=last.min(first + 32) {
                        let at = 2.4 * (n as f32 + 0.5) + offset;
                        mirror(&mut spans, (at - hw).max(lo), (at + hw).min(hi));
                    }
                }
                for &(bx, ..) in arc.as_slice() {
                    mirror(&mut spans, bx - led_reach, bx + led_reach);
                }
            }
        }
        if my > 56.0 && my < 65.0 {
            spans.push((-2.6, 2.6));
        }
        if (9.5..11.8).contains(&my) {
            spans.push((-(RAIL + 0.8), RAIL + 0.8));
        }
        let semaphore = lights > 0 && my > 5.4 - sem_reach && my < 10.0f32.max(7.2 + sem_reach);
        if semaphore {
            spans.push((-(0.9 + sem_reach), 0.9 + sem_reach));
        }
        if (my - 65.0).abs() < beacon_reach + 0.4 {
            spans.push((-(beacon_reach + 0.4), beacon_reach + 0.4));
        }
        if spans.len == 0 {
            return;
        }
        // A columnas, ordenados y juntando los que se pisan (en su lugar).
        let width = game.width;
        let mut cols: Short<(usize, usize), 96> = Short::new();
        for &(a, b) in spans.as_slice() {
            let x1 = ((item.x + b * k).ceil().max(0.0) as usize + 1).min(width);
            cols.push((((item.x + a * k).floor().max(0.0) as usize).min(x1), x1));
        }
        let cols = cols.as_mut_slice();
        cols.sort_unstable();
        let mut merged = 0usize;
        for i in 0..cols.len() {
            let (a, b) = cols[i];
            if merged > 0 && a <= cols[merged - 1].1 {
                cols[merged - 1].1 = cols[merged - 1].1.max(b);
            } else {
                cols[merged] = (a, b);
                merged += 1;
            }
        }
        // Lo que es igual en toda la fila.
        // El destello verde al arrancar: el hierro se alumbra de verde.
        let flash = if lights >= 4 && green < 1.5 { 0.4 * (-green * 4.0).exp() * (1.0 - fog) } else { 0.0 };
        let iron_fog = fog.max(0.6 * smoothstep(100.0, 500.0, item.depth));
        let iron_col = (Vec3::new(0.05, 0.05, 0.06) + Vec3::new(0.9, 0.6, 0.35) * (0.4 * (-my / 14.0).exp())).lerp(haze, iron_fog) + green_led * flash;
        // Lo que pesa un foco: su energia `(0.16 / r)^2` repartida en el
        // disco de radio `r` (de lejos, mas que el foco), pero nunca menos
        // de 0.4 de un pixel: de lejos la torre se lee como su silueta de
        // luces. Se mezcla con lo de atras con esa cobertura (si se pintara
        // el color ya oscurecido encima, de lejos serian puntos negros).
        // Las luces atraviesan la neblina mejor que el hierro (se ven
        // puntos aunque la torre ya se pierda): a ellas les toca `fog^2`.
        let led_fog = fog * fog;
        let led_k = (0.16 / led_r).powi(2).max(0.4) * (1.0 - led_fog);
        // Los focos del borde de afuera: uno cada 1.5 m de alto, el de esta
        // fila en `1.5 floor(my / 1.5) + 0.75`.
        let row_bulb = 1.5 * (my / 1.5).floor() + 0.75;
        let (outer_col, outer_on) = led(if row_bulb < 6.0 { 0 } else if row_bulb < 12.0 { 1 } else { 2 }, row_bulb);
        let outer = outer_col.lerp(haze, led_fog) * outer_on;
        let outer_near = (my - row_bulb).abs() < led_reach;
        let beacon_on = if game.wall.rem_euclid(1.0) < 0.5 { 1.0 } else { 0.0 };
        let red = Vec3::new(1.0, 0.1, 0.04);
        // El semaforo: la altura de cada luz desde la fila y su color.
        let sem_green = lights >= 4;
        let sem_col = if sem_green { green_led } else { Vec3::new(1.0, 0.1, 0.08) };
        let sem_dy = [my - 7.2, my - 6.3, my - 5.4];
        let sem_cut = (0.36 + sem_reach) * (0.36 + sem_reach);
        let crown = my > 57.5 && my < 65.0;
        let beacon = (my - 65.0).abs() < beacon_reach + 0.4;
        let banner = (9.9..11.7).contains(&my);
        for &(x0, x1) in cols[..merged].iter() {
            for x in x0..x1 {
                let xf = x as f32 + 0.5;
                if hidden(xf) {
                    continue;
                }
                let mx = (xf - item.x) * px;
                let amx = mx.abs();
                let p = &mut row[x * 4..x * 4 + 4];
                let (mut c, mut e) = (lienzo::read(p), emission(p));
                // El hierro: los bordes de las patas y del arco, la celosia
                // y las plataformas.
                if body && amx < wo + g {
                    let mut iron = bar((amx - wo).abs(), 0.35);
                    if wi > 0.0 {
                        iron = iron.max(bar((amx - wi).abs(), 0.3));
                    }
                    if amx < wo && amx > wi {
                        let lat = (fract((amx + my) / 2.4) - 0.5).abs().min((fract((amx - my) / 2.4) - 0.5).abs()) * 2.4;
                        let sharp = cover((lat - 0.09) / px);
                        iron = iron.max(sharp + (0.3 - sharp) * lattice_far).max(platform);
                    }
                    c = over(c, iron_col, iron);
                    e *= 1.0 - iron;
                }
                // La corona: la caja de la campana (abierta), la campana de
                // bronce alumbrada desde abajo y la aguja del faro.
                if crown && amx < 2.7 {
                    let frame = bar((amx - 0.8).max((my - 61.0).abs() - 3.0).abs(), 0.12);
                    let spire = cover((amx - 0.06f32.max(0.5 * px)).max((my - 64.5).abs() - 0.5) / px) * (0.12 / px).min(1.0);
                    let cov = frame.max(spire);
                    c = over(c, iron_col, cov);
                    e *= 1.0 - cov;
                    let bell = cover((len(mx, my - 61.5) - 0.9) / px);
                    if bell > 0.0 {
                        let shade = 0.55 + 0.6 * smoothstep(0.9, -0.9, my - 61.5);
                        c = over(c, (Vec3::new(0.6, 0.4, 0.15) * shade).lerp(haze, iron_fog), bell);
                        e *= 1.0 - bell;
                    }
                }
                // El faro rojo de la punta, a 1 Hz.
                if beacon {
                    let d = (len(mx, my - 65.0) - 0.35) / px;
                    let bulb = cover(d);
                    c = over(c, red * (0.15 + 0.85 * beacon_on), bulb);
                    let h = if d <= 0.0 { 1.0 } else { fast_exp(-d / spread) };
                    let glow = beacon_on * (h - 0.018).max(0.0) * (1.0 - bulb) * (1.0 - fog);
                    c = c + red * (0.6 * glow);
                    e = (e * (1.0 - bulb)).max(bulb * beacon_on * (1.0 - fog)).max(0.3 * glow);
                }
                // Los focos LED: el del borde de afuera en esta fila y los
                // del arco que la alcanzan. Cada uno, `cubre(d)` con su peso
                // y un halo `e^(-d / 2.5)` que llega a cero en 7 px (se le
                // resta `e^(-2.8)`).
                if body {
                    let mut dot = |d: f32, col: Vec3, on: f32| {
                        let cov = cover(d) * led_k;
                        c = over(c, col, cov) + col * (0.5 * led_k * (fast_exp(-d.max(0.0) / 2.5) - 0.061).max(0.0) * (1.0 - cov));
                        e = e.max(cov * on.min(1.0));
                    };
                    if outer_near && (amx - wo).abs() < led_reach {
                        let d = (len(amx - wo, my - row_bulb) - led_r) / px;
                        if d < 7.0 {
                            dot(d, outer, outer_on);
                        }
                    }
                    for &(bx, by, col, on) in arc.as_slice() {
                        if (amx - bx).abs() < led_reach {
                            let d = (len(amx - bx, my - by) - led_r) / px;
                            if d < 7.0 {
                                dot(d, col, on);
                            }
                        }
                    }
                }
                // El semaforo colgado de la manta: una varilla y la caja con
                // sus tres focos (rojos por segundo de la cuenta, todos
                // verdes al arrancar). El halo alumbra el aire afuera de la
                // caja; adentro solo brillan las lamparas.
                if semaphore && amx < 0.9 + sem_reach {
                    let rod = (amx - 0.06f32.max(0.5 * px)).max((my - 8.9).abs() - 1.1) / px;
                    let housing = cover((amx - 0.75).max((my - 6.3).abs() - 1.5) / px);
                    let cov = housing.max(cover(rod));
                    c = over(c, Vec3::new(0.02, 0.02, 0.025), cov);
                    e *= 1.0 - cov;
                    for (k_light, &dy) in sem_dy.iter().enumerate() {
                        let r2 = mx * mx + dy * dy;
                        if r2 > sem_cut {
                            continue;
                        }
                        let on = sem_green || lights as usize > k_light;
                        let d = (r2.sqrt() - 0.36) / px;
                        let bulb = cover(d);
                        c = over(c, if on { sem_col * 1.2 } else { sem_col * 0.12 }, bulb);
                        e = if on { (e * (1.0 - bulb)).max(bulb) } else { e * (1.0 - bulb) };
                        if on {
                            let h = if d <= 0.0 { 1.0 } else { fast_exp(-d / sem_spread) };
                            c = c + sem_col * (0.5 * (h - 0.05).max(0.0) * (1.0 - fog) * (1.0 - housing));
                        }
                    }
                }
                // La manta de la meta, encima de los focos del arco: cuadros
                // de 1 x 0.55 m con orillas de faja de 0.25 m arriba y abajo
                // (los cuadros de lejos se van a su gris).
                if banner && amx < RAIL + 0.9 {
                    let cov = cover((amx - (RAIL + 0.8)).max((my - 10.8).abs() - 0.8) / px);
                    if cov > 0.0 {
                        let face = if !(10.25..=11.35).contains(&my) {
                            let v = if my < 10.25 { (my - 10.0) / 0.25 * transition::EDGE } else { 1.0 - (11.6 - my) / 0.25 * transition::EDGE };
                            transition::sash((mx + RAIL + 0.8) / (RAIL * 2.0), v.clamp(0.0, 1.0), (px / 0.25 * transition::EDGE).max(0.004))
                        } else {
                            let check = (mx.floor() as i64 + ((my - 10.25) / 0.55).floor() as i64).rem_euclid(2) == 0;
                            let f = if check { 0.85 } else { 0.03 };
                            Vec3::splat(f + (0.44 - f) * smoothstep(0.25, 0.6, px))
                        };
                        c = over(c, (face * 0.9).lerp(haze, fog), cov);
                        e *= 1.0 - cov;
                    }
                }
                write_emit(p, c, e);
            }
        }
    }
}

/// Lo que el HUD pinta en CPU encima de la imagen ya graduada (en el
/// gancho de cada fila del compuesto): por ahora el mapa.
struct HudFrame {
    /// Los puntos del mapa: donde va cada nave (de 0 a 1), su color y si es
    /// la propia.
    dots: Vec<(f32, f32, Vec3, bool)>,
}

impl HudFrame {
    /// La fila `y` de lo que va encima.
    fn row(&self, game: &Carrera, y: usize, row: &mut [u8]) {
        self.minimap(game, y, row);
    }

    /// El mapa de la pista abajo a la izquierda, con un punto por nave.
    fn minimap(&self, game: &Carrera, y: usize, row: &mut [u8]) {
        let size = game.map_size;
        let margin = (16.0 * game.height as f32 / 720.0) as usize;
        let (x0, y0) = (margin, game.height.saturating_sub(size + margin));
        if y < y0 || y >= y0 + size {
            return;
        }
        let ly = y - y0;
        let radius = |player: bool| if player { 5.0 } else { 4.0 } * game.height as f32 / 720.0;
        for lx in 0..size.min(game.width.saturating_sub(x0)) {
            let p = &mut row[(x0 + lx) * 4..(x0 + lx) * 4 + 4];
            let m = game.minimap[ly * size + lx];
            // Lo de atras se oscurece bajo el disco y mas bajo el borde del
            // trazado (un contorno oscuro de unos 2 px): la linea se lee
            // aunque pase sobre una nave clara.
            let mut out = lienzo::read(p) * (1.0 - m[3] * 0.8) + Vec3::new(m[0], m[1], m[2]);
            for &(mx, my, color, player) in &self.dots {
                let (dx, dy) = (lx as f32 + 0.5 - mx * size as f32, ly as f32 + 0.5 - my * size as f32);
                let d = dx.hypot(dy) - radius(player);
                out = over(out, if player { Vec3::ONE } else { color }, cover(d));
                if player {
                    out = over(out, color, cover(d + 2.0));
                }
            }
            lienzo::write(p, out);
        }
    }
}

/// Lo de una fila de pista que no cambia a lo ancho (ver `View::strip`).
#[derive(Clone, Debug)]
struct Strip {
    r: RoadRow,
    /// Metros por pixel a lo ancho.
    px: f32,
    /// Lo que cubre a lo largo (de 0 a 1, filtrado en caja): las rayas de
    /// carril, las tachas blancas y ambar, el celeste del bordillo, la
    /// junta y la meta (con la fraccion de sus cuadros pares).
    dash: f32,
    eye: f32,
    amber: f32,
    curb: f32,
    joint: f32,
    finish: f32,
    finish_fs: f32,
    /// El relieve del bordillo y el resplandor de la junta (ya promediado).
    relief: f32,
    joint_glow: f32,
    /// El tono de la losa, cuanto se resuelven las manchas y el grano.
    slab: Vec3,
    resolved: f32,
    grain_on: bool,
    grain_amp: f32,
    /// La faja del hombro: el pico del rombo, su suavizado, el color del
    /// rombo y cuanto se va a su promedio de lejos.
    sash_peak: f32,
    sash_soft: f32,
    sash_col: Vec3,
    sash_far: f32,
    /// Los tumulos: carriles, cobertura, donde va la fila adentro del
    /// tumulo (0 a 1), los botones y cuanto se va al promedio.
    bumps: u8,
    bump_cov: f32,
    bump_q: f32,
    bump_btn: f32,
    bump_far: f32,
    /// Las rayas del reflejo de los postes: `(u, amplitud, 1 / sigma)`.
    specs: [(f32, f32, f32); 4],
    n_specs: usize,
    /// La tabla a lo ancho: cada `1 / inv_step` metros desde `u0`, `n`
    /// muestras de `[luz de los postes, mojado, manchas, neon (rgb), luz
    /// de las naves (rgb), calaveras, sombra de contacto, -]`.
    table: Vec<[f32; ENTRY]>,
    u0: f32,
    inv_step: f32,
    n: usize,
    /// Si alguna nave alumbra la fila (si no, esas columnas de la tabla
    /// son cero y no se interpolan).
    has_pools: bool,
    /// El Fresnel de la estela (con la distancia) y el del cielo (con el
    /// angulo).
    fres: f32,
    fres_s: f32,
    /// El pulso del neon de la baranda.
    pulse: f32,
    /// La luz del arco de foquitos mas cercano (lineal; cero afuera de los
    /// tuneles).
    spill: Vec3,
    /// Si a la fila le cae la luz de un arco o el destello de un cohete.
    lit: bool,
    /// El grano de la fila.
    grain: NoiseLine<100>,
    /// El destello blanco del neon de cada baranda donde roza la nave
    /// propia.
    flare: [f32; 2],
    /// Los anillos antigravedad de lo que flota cerca de la fila, cuantos,
    /// el radio de sus dos ondas y lo que corre la caustica.
    rings: [RingRow; 2],
    n_rings: usize,
    ripples: [f32; 2],
    caustic: (f32, f32),
}

/// El anillo de lo que flota (la chela) en una fila: su centro a lo ancho
/// (m), `1 / (1.3 hw)`, `ds / (1.3 hl)` de la fila, cuantos pixeles mide una
/// unidad de la elipse (`hw` por pixeles por metro), lo que le toca a lo
/// largo (m) y su fuerza.
#[derive(Clone, Copy, Debug, Default)]
struct RingRow {
    x: f32,
    inv_w: f32,
    ey: f32,
    px: f32,
    ds: f32,
    gain: f32,
}

/// Las muestras de la tabla a lo ancho de una fila de pista y su paso mas
/// fino (m): 20 cm alcanzan para la calavera, la mas angosta (0.45 m).
const TABLE: usize = 100;
const ENTRY: usize = 12;
/// Una muestra vacia (sin luz, seco, sin sombra): por si la tabla no
/// alcanza.
const TABLE_EMPTY: [f32; ENTRY] = [0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
const TABLE_STEP: f32 = 0.2;

/// El charco de una nave en una fila: lo que depende de `ds` ya resuelto.
#[derive(Clone, Copy, Debug, Default)]
struct PoolRow {
    x: f32,
    /// Hasta donde alcanza a lo ancho (m).
    reach: f32,
    /// La luz: `1 / ancho`, `(ds / largo)^2` de su elipse y el color con
    /// su peso.
    inv_l: f32,
    ey2: f32,
    tint: Vec3,
    /// La sombra de contacto: `1 / (0.9 hw)`, cuanto cae la fila adentro
    /// de la huella a lo largo y la fuerza de la franja de la cola.
    shadow: bool,
    inv_c: f32,
    fy: f32,
    lip: f32,
    /// Las calaveras: su fuerza a esta distancia atras y donde van a lo
    /// ancho (`0.75 hw`).
    tail: f32,
    tail_x: f32,
}

/// El ruido de valor a lo largo de una fila: con la `y` fija, el ruido es
/// la interpolacion suave entre los valores de las esquinas enteras de
/// `x` (ya interpoladas en `y`). Se guardan `N` desde `x0` y por pixel solo
/// queda una interpolacion: lo mismo que `value_noise` sin cuatro hashes
/// por pixel.
#[derive(Clone, Copy, Debug)]
struct NoiseLine<const N: usize> {
    x0: f32,
    v: [f32; N],
}

impl<const N: usize> Default for NoiseLine<N> {
    fn default() -> Self {
        Self { x0: 0.0, v: [0.0; N] }
    }
}

impl<const N: usize> NoiseLine<N> {
    fn new(x_lo: f32, y: f32) -> Self {
        let x0 = x_lo.floor();
        let yi = y.floor();
        let fy = y - yi;
        let sy = fy * fy * (3.0 - 2.0 * fy);
        let (xi, yi) = (x0 as i32 as u32, yi as i32 as u32);
        let mut v = [0.0; N];
        for (k, o) in v.iter_mut().enumerate() {
            let x = xi.wrapping_add(k as u32);
            let (a, b) = (lattice(x, yi), lattice(x, yi.wrapping_add(1)));
            *o = a + (b - a) * sy;
        }
        Self { x0, v }
    }

    /// El ruido promediado a lo largo sobre `[y - spread / 2, y + spread /
    /// 2]` (lo que se corre la pista en un frame, el borron) con
    /// `BLUR_TAPS` muestras parejas. Queda pegado a `y` (a la pista): si el
    /// borron cambia un poco (el `dt` tiembla, la nave acelera), las
    /// muestras se corren un poco y el ruido no salta. Estirar la `y`
    /// dividiendola entre la velocidad, en cambio, movia todo el patron con
    /// `s` de miles de metros. Cuesta `BLUR_TAPS` veces la fila, nada por
    /// pixel; quieto, una sola.
    fn blurred(x_lo: f32, y: f32, spread: f32) -> Self {
        if !(spread > 1e-3) {
            return Self::new(x_lo, y);
        }
        let mut out = Self::new(x_lo, y - 0.5 * spread);
        for j in 1..BLUR_TAPS {
            let tap = Self::new(x_lo, y + (j as f32 / (BLUR_TAPS - 1) as f32 - 0.5) * spread);
            for (o, v) in out.v.iter_mut().zip(tap.v) {
                *o += v;
            }
        }
        for o in &mut out.v {
            *o *= 1.0 / BLUR_TAPS as f32;
        }
        out
    }

    #[inline(always)]
    fn at(&self, x: f32) -> f32 {
        let t = (x - self.x0).clamp(0.0, N as f32 - 1.001);
        let i = (t as usize).min(N - 2);
        let f = t - i as f32;
        let s = f * f * (3.0 - 2.0 * f);
        self.v[i] + (self.v[i + 1] - self.v[i]) * s
    }
}

/// Las muestras del ruido emborronado a lo largo (`NoiseLine::blurred`).
const BLUR_TAPS: usize = 4;

/// El valor al azar de la esquina `(x, y)` de la rejilla del ruido, de -1
/// a 1.
#[inline(always)]
fn lattice(x: u32, y: u32) -> f32 {
    (hash_u32(x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B)) >> 8) as f32 / 8_388_608.0 - 1.0
}

/// Una franja periodica filtrada en caja: cuanto de la ventana
/// `[s - w / 2, s + w / 2]` cae en lo encendido de un patron que prende
/// `on` metros de cada `period`. Con `F(x)` lo encendido de 0 a `x`,
/// `F(x) = floor(x / P) on + min(fract(x / P) P, on)`, es
/// `(F(s + w / 2) - F(s - w / 2)) / w`. De cerca (`w` chica) es la franja
/// con su borde; de lejos, su promedio `on / P`: una fila que abarca
/// varios metros no salta entre raya y hueco.
fn band(s: f32, period: f32, on: f32, w: f32) -> f32 {
    let w = w.max(1e-3);
    // Se resta lo entero primero: con `s` de miles de metros, `s +- w / 2`
    // perderia los milimetros.
    let x = s - (s / period).floor() * period;
    let f = |x: f32| {
        let k = (x / period).floor();
        k * on + (x - k * period).clamp(0.0, on)
    };
    ((f(x + 0.5 * w) - f(x - 0.5 * w)) / w).clamp(0.0, 1.0)
}

/// El largo de `(x, y)`. `f32::hypot` cuida el desborde y es una llamada a
/// la biblioteca de C, lenta para cada pixel; aqui los numeros son chicos.
#[inline(always)]
fn len(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// La parte fraccionaria hacia abajo, de 0 a 1 (tambien con negativos).
#[inline(always)]
fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Una tacha emborronada a lo largo: la caja reparte su luz en lo que
/// abarca la fila, `0.16 / w` (con el turbo, una trazadora de 3 m se
/// apagaria a un 5 %). Pero la tacha devuelve los faros, es mucho mas
/// brillante que lo que el pixel muestra (se recorta): emborronada sigue
/// llena en un tercio de su luz, `min(3 c, 1)`.
fn tracer(c: f32) -> f32 {
    (3.0 * c).min(1.0)
}

/// `e^(-|t| / a)` filtrado con una tienda de medio ancho `w` (area 1,
/// `(1 - |t| / w) / w`): un resplandor angosto que una fila promedia en
/// vez de muestrear, y que a diferencia de la caja no deja bordes (la
/// tienda es la caja de ancho `w` aplicada dos veces, y el resultado pasa
/// suave de fila en fila).
///
/// Con `H` la segunda primitiva, `H(x) = a |x| - a^2 (1 - e^(-|x| / a))`,
/// es `(H(d + w) - 2 H(d) + H(d - w)) / w^2`. Si `|d| >= w` la parte
/// `a |x|` se cancela y queda
/// `a^2 e^(-|d| / a) (e^k - 2 + e^-k) / w^2 = (2 a sinh(k / 2) / w)^2 e^(-|d| / a)`
/// con `k = w / a`: con `k` chico la forma del `sinh` (sin restar
/// numeros parecidos) y con `k` grande los tres exponentes, todos `<= 0`
/// (sin desbordar). Con `w` mucho menor que `a`, la exponencial misma.
fn exp_tent(d: f32, a: f32, w: f32) -> f32 {
    let w = w.max(1e-3);
    let ad = d.abs();
    let k = w / a;
    if k < 0.1 {
        return (-ad / a).exp();
    }
    let v = if ad >= w {
        if k < 1.0 {
            let sh = 2.0 * a * (0.5 * k).sinh() / w;
            sh * sh * (-ad / a).exp()
        } else {
            a * a / (w * w) * (((w - ad) / a).exp() - 2.0 * (-ad / a).exp() + (-(ad + w) / a).exp())
        }
    } else {
        let h = |x: f32| {
            let x = x.abs();
            a * (x - a * (1.0 - (-x / a).exp()))
        };
        (h(d + w) - 2.0 * h(d) + h(d - w)) / (w * w)
    };
    v.clamp(0.0, 1.0)
}

/// Cuanto de la ventana `[d - w / 2, d + w / 2]` cae en `[-half, half]`
/// (de 0 a 1): un tramo filtrado en caja, sin periodo.
fn window_cover(d: f32, half: f32, w: f32) -> f32 {
    let w = w.max(1e-3);
    ((d + 0.5 * w).min(half) - (d - 0.5 * w).max(-half)).max(0.0) / w
}

/// El color de lo que emite un sprite: el promedio de la luz de sus LEDs
/// (el `glow`, nucleo y halo), normalizado a maximo 1. Si no tiene LEDs,
/// ambar.
///
/// Cada pixel pesa ademas `0.2 + saturacion` (`(max - min) / max`): las
/// LEDs blancas o casi blancas (faros, el nucleo saturado) no lavan el
/// tinte, y el charco sale del color de las de color (magenta y ambar en la
/// Mamaneta) y no de su promedio cafe.
fn led_tint(sprite: &Sprite) -> Vec3 {
    let mut sum = Vec3::ZERO;
    for g in &sprite.glow {
        let c = Vec3::new(g[0] as f32, g[1] as f32, g[2] as f32) * (1.0 / 255.0);
        let top = c.x.max(c.y).max(c.z);
        if top > 1e-4 {
            let sat = (top - c.x.min(c.y).min(c.z)) / top;
            sum += c * (0.2 + sat);
        }
    }
    let top = sum.x.max(sum.y).max(sum.z);
    if top > 1e-6 { sum * (1.0 / top) } else { Vec3::new(1.0, 0.68, 0.16) }
}

/// Ruido de valor en 2D, de -1 a 1: un numero al azar en cada esquina de la
/// cuadricula, interpolado suave. Mucho mas barato que el Perlin, y para el
/// asfalto alcanza.
#[inline]
pub(super) fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i32 as u32, yi as i32 as u32);
    let corner = |dx: u32, dy: u32| lattice(xi.wrapping_add(dx), yi.wrapping_add(dy));
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

/// Cuanto se pierde en la neblina algo a `depth` metros: empieza a un
/// cuarto de lo que se dibuja y a los nueve decimos ya es el color del
/// horizonte, asi la pista y lo que va encima se funden antes de acabarse.
#[inline]
fn fog_at(depth: f32) -> f32 {
    smoothstep(DRAW as f32 * SEGMENT * 0.25, DRAW as f32 * SEGMENT * 0.9, depth)
}

/// La neblina extra de lo que cae pegado a la linea del horizonte en la
/// pantalla (`dy` pixeles abajo de ella, negativo arriba): ahi el color de
/// atras es el del cielo del horizonte y la pista lejana debe fundirse en
/// el, `smoothstep(14, 2, |dy|)`. Solo de lejos (de 150 a 350 m), para no
/// tocar la pista cercana que cruza el horizonte al subir una loma.
#[inline]
fn horizon_fog(dy: f32, depth: f32) -> f32 {
    smoothstep(14.0, 2.0, dy.abs()) * smoothstep(150.0, 350.0, depth)
}

/// Mapeo de tonos y gamma (de luz lineal a lo que se ve).
pub(super) fn tone(c: Vec3) -> Vec3 {
    let t = |v: f32| (1.0 - (-v.max(0.0) * 1.3).exp()).powf(1.0 / 2.2);
    Vec3::new(t(c.x), t(c.y), t(c.z))
}

/// La tabla de tono se indexa con los bits del `f32`: el exponente y los 7
/// bits altos de la mantisa (`bits >> 16`). Cada entrada cubre un tramo de
/// 1/128 relativo, de `2^-16` a `2^5` (de negro a mucho mas que blanco).
const TONE_FIRST: i32 = 111 << 7;
const TONE_SIZE: usize = 21 << 7;

/// La curva de `tone` tabulada para lo que se pinta por pixel en cada
/// frame (la ciudad, el cielo, los volcanes, las barandas). Indexar por
/// los bits del flotante es un indice logaritmico gratis: sin raiz ni
/// division, y con el mismo detalle relativo en lo oscuro que en lo
/// claro. La entrada 0 es negro.
fn tone_lut() -> &'static [f32] {
    static TONE_LUT: OnceLock<Vec<f32>> = OnceLock::new();
    TONE_LUT.get_or_init(|| {
        (0..TONE_SIZE)
            .map(|i| {
                if i == 0 {
                    return 0.0;
                }
                // El centro del tramo: la mitad de los bits que se tiran.
                let v = f32::from_bits((((i as i32 + TONE_FIRST) as u32) << 16) | 0x8000);
                (1.0 - (-1.3 * v).exp()).powf(1.0 / 2.2)
            })
            .collect()
    })
}

/// `tone` con la tabla.
#[inline]
fn tone_fast(lut: &[f32], c: Vec3) -> Vec3 {
    let at = |v: f32| {
        let i = ((v.max(0.0).to_bits() >> 16) as i32 - TONE_FIRST).clamp(0, TONE_SIZE as i32 - 1);
        lut[i as usize]
    };
    Vec3::new(at(c.x), at(c.y), at(c.z))
}

/// `log2` barato (error < 0.005) para el nivel de detalle: el exponente
/// sale de los bits del `f32` y el `log2` de la mantisa `m` en `[1, 2)` de
/// una parabola, `-0.3448 m^2 + 2.0247 m - 1.6749`.
#[inline]
fn fast_log2(x: f32) -> f32 {
    let bits = x.max(1e-30).to_bits();
    let exponent = ((bits >> 23) & 0xFF) as f32 - 127.0;
    let m = f32::from_bits((bits & 0x007F_FFFF) | 0x3F80_0000);
    exponent + (-0.344_848_43 * m + 2.024_665_8) * m - 1.674_877_6
}

/// Un angulo llevado a `[-pi, pi]`.
#[inline]
fn wrap_pi(a: f32) -> f32 {
    a - (a * (1.0 / TAU)).round() * TAU
}


/// Ruido fractal de cuatro octavas de ruido de valor (de -1 a 1 mas o
/// menos): cada octava al doble de frecuencia y la mitad de amplitud.
fn fbm2(x: f32, y: f32) -> f32 {
    let (mut sum, mut amp, mut f) = (0.0, 0.5, 1.0);
    for k in 0..4 {
        sum += amp * value_noise(x * f + k as f32 * 17.3, y * f - k as f32 * 9.1);
        amp *= 0.5;
        f *= 2.0;
    }
    sum * (1.0 / 0.9375)
}

// El cielo y los volcanes -----------------------------------------------------

/// Radio del planeta (m) y altura de la pista sobre la ciudad (m).
const R_PLANET: f32 = 160_000.0;
const ALTITUDE: f32 = 400.0;
/// La tabla del gradiente del cielo: angulos sobre el limbo de -0.05 a 0.6.
const SKY_LUT: usize = 512;
const SKY_LOW: f32 = -0.05;
const SKY_HIGH: f32 = 0.6;
/// El destello de una explosion del Fuego en el cielo (lineal): naranja
/// rojizo saturado.
const FLASH_COLOR: Vec3 = Vec3::new(0.3, 0.05, 0.004);
/// La neblina de la ciudad (lineal): violeta oscuro, aparte del cielo del
/// limbo, para que la alfombra de luces no se lave a cafe.
const CITY_HAZE: Vec3 = Vec3::new(0.012, 0.006, 0.02);
/// El halo de las bombas llega a cero en 8 px: se le resta `e^(-8 / 1.5)`.
const SPARK_CUT: f32 = 0.004_828;
/// `e^-3`: lo que vale el halo de la tira del poste a los 9 px, donde se
/// corta (se le resta, asi llega a cero sin escalon).
const STRIP_CUT: f32 = 0.049_787;
/// `e^-4`: el resplandor de una chispa de cohete donde se corta (a `8 w`
/// con `e^(-d / 2w)`).
const FW_CUT: f32 = 0.018_316;
/// La gravedad de las bombas de lava, en radianes de elevacion por s^2.
const BOMB_GRAVITY: f32 = 0.12;

/// El angulo de la entrada `i` de la tabla del cielo.
fn sky_eps(i: usize) -> f32 {
    SKY_LOW + (SKY_HIGH - SKY_LOW) * i as f32 / (SKY_LUT - 1) as f32
}

/// El cielo de noche en luz lineal a `eps` radianes sobre el limbo:
/// - del violeta oscuro pegado al limbo al indigo de arriba, con
///   `1 - e^(-eps / 0.18)`;
/// - el domo de la capital: una franja delgada de sodio saturado pegada al
///   limbo (`e^(-eps / 0.012)`), un magenta-violeta que cae con
///   `e^(-eps / 0.05)` y un violeta mas ancho (`e^(-eps / 0.15)`);
/// - el airglow: una linea verde fina (gaussiana de 0.0028 rad) a 0.022
///   sobre el limbo, arriba de la franja de sodio para que no se mezclen.
fn sky_lin(eps: f32) -> Vec3 {
    let up = eps.max(0.0);
    let base = Vec3::new(0.006, 0.0025, 0.011).lerp(Vec3::new(0.0005, 0.0012, 0.0055), 1.0 - (-up / 0.18).exp());
    let g = |center: f32, width: f32| (-((eps - center) / width).powi(2)).exp();
    base + Vec3::new(0.05, 0.6, 0.25) * (0.11 * g(0.022, 0.0028))
        + Vec3::new(1.0, 0.55, 0.2) * (0.06 * (-up / 0.012).exp())
        + Vec3::new(0.55, 0.18, 0.45) * (0.05 * (-up / 0.05).exp())
        + Vec3::new(0.5, 0.15, 0.3) * (0.012 * (-up / 0.15).exp())
}

/// Lee una tabla del cielo en `eps`, interpolando entre entradas.
#[inline]
fn sky_sample(lut: &[Vec3], eps: f32) -> Vec3 {
    let f = ((eps - SKY_LOW) / (SKY_HIGH - SKY_LOW)).clamp(0.0, 1.0) * (SKY_LUT - 1) as f32;
    let i = (f as usize).min(SKY_LUT - 2);
    lut[i].lerp(lut[i + 1], f - i as f32)
}

/// Un volcan como silueta en el horizonte: su rumbo de mundo, su altura
/// sobre el limbo (rad), su ancho a cada lado (rad) y la forma de la cima.
#[derive(Clone, Copy, Debug)]
struct Volcano {
    psi: f32,
    height: f32,
    left: f32,
    right: f32,
    /// La cima redonda (el Pico Mayor del Acatenango) en vez de en punta.
    round: bool,
    /// El Agua tiene el crater plano y el Fuego una muesca en la cima.
    flat: bool,
    notch: bool,
}

impl Volcano {
    /// Altura sobre el limbo en el rumbo `psi`, con `s = |psi - psi0| / w`:
    /// un cono empinado mas una falda ancha y concava,
    /// `e = H (0.55 (1 - s)^1.3 + 0.45 (1 - s / 2.2)^3)`
    /// (con `1 - s^2` en vez de `1 - s` si la cima es redonda), y un fbm de
    /// dos octavas de +-0.01 rad en los flancos (no en la cima ni al pie).
    /// Es 0 afuera de `s = 2.2`.
    fn profile(&self, psi: f32) -> f32 {
        let d = psi - self.psi;
        let w = if d < 0.0 { self.left } else { self.right };
        let s = d.abs() / w;
        if s >= 2.2 {
            return 0.0;
        }
        let cone = if s >= 1.0 {
            0.0
        } else if self.round {
            (1.0 - s * s).powf(1.3)
        } else {
            (1.0 - s).powf(1.3)
        };
        let skirt = 1.0 - s / 2.2;
        let mut e = self.height * (0.55 * cone + 0.45 * skirt * skirt * skirt);
        if self.flat {
            e = e.min(0.975 * self.height);
        }
        if self.notch && d.abs() < 0.008 {
            e -= 0.004 * (1.0 - d.abs() / 0.008);
        }
        let n = value_noise(psi * 60.0, self.psi * 31.0) + 0.5 * value_noise(psi * 130.0, self.psi * 17.0 + 5.0);
        (e + 0.0067 * n * smoothstep(0.08, 0.35, s) * skirt).max(0.0)
    }
}

/// El Pacaya, el Agua, el Fuego y el Acatenango (Pico Mayor redondo y
/// Yepocapa, mas bajo y ancho), en el orden de sus ids (1 a 5) en
/// `vol`. El Fuego queda a la izquierda de la pista en la largada, entre
/// el poste del arco y el semaforo, para que se vea cuando da la salida.
const VOLCANOES: [Volcano; 5] = [
    Volcano { psi: -1.05, height: 0.085, left: 0.10, right: 0.10, round: false, flat: false, notch: false },
    Volcano { psi: -0.62, height: 0.17, left: 0.2, right: 0.16, round: false, flat: true, notch: false },
    FUEGO,
    Volcano { psi: -0.29, height: 0.175, left: 0.10, right: 0.10, round: true, flat: false, notch: false },
    Volcano { psi: -0.355, height: 0.15, left: 0.13, right: 0.13, round: false, flat: false, notch: false },
];
const FUEGO: Volcano = Volcano { psi: -0.2, height: 0.155, left: 0.11, right: 0.11, round: false, flat: false, notch: true };
/// La silla (La Horqueta) que une el Fuego con el Acatenango: un lomo bajo
/// y ancho, `e = H (1 - ((psi - m) / a)^2)^0.6`, que se toma con `max`.
const SADDLE: (f32, f32, f32) = (-0.245, 0.075, 0.12);

/// La silueta en el rumbo `psi`: la altura del volcan mas alto ahi y su
/// id (0 si no hay ninguno). La silla cuenta como Acatenango.
fn volcano_profile(psi: f32) -> (f32, u8) {
    let mut best = (0.0, 0u8);
    for (k, v) in VOLCANOES.iter().enumerate() {
        let e = v.profile(psi);
        if e > best.0 {
            best = (e, k as u8 + 1);
        }
    }
    let (m, a, h) = SADDLE;
    let q = 1.0 - ((psi - m) / a).powi(2);
    if q > 0.0 {
        let e = h * q.powf(0.6) + 0.003 * value_noise(psi * 90.0, 3.0);
        if e > best.0 {
            best = (e, 4);
        }
    }
    best
}

/// Lo que se guarda por columna para el cielo: la silueta (altura sobre el
/// limbo y cual volcan), la silueta del Fuego solo (para sus rios de lava)
/// y el destello de la explosion en ese rumbo.
#[derive(Clone, Copy, Debug)]
struct VolCol {
    top: f32,
    id: u8,
    fuego: f32,
    flash: f32,
}

/// Una bomba de lava del Fuego: rumbo y elevacion (rad), sus velocidades
/// (rad/s) y su edad (s; negativa mientras espera salir del crater).
#[derive(Clone, Copy, Debug)]
struct Bomb {
    psi: f32,
    p: f32,
    vpsi: f32,
    vp: f32,
    age: f32,
}

/// El Fuego en erupcion.
struct Eruption {
    /// Cuando explota la proxima vez (reloj de pared).
    next: f32,
    /// El destello de la ultima explosion (1 al explotar, cae con
    /// `e^(-t / 0.4)`) y el brillo del crater `G`.
    flash: f32,
    glow: f32,
    /// Hasta donde bajan los tres rios de lava (0 la cima, 1 el limbo).
    flows: [f32; 3],
    bombs: Vec<Bomb>,
    count: u32,
}

impl Eruption {
    fn new() -> Self {
        Self { next: 4.0, flash: 0.0, glow: 0.6, flows: [0.55, 0.42, 0.38], bombs: Vec::new(), count: 0 }
    }

    /// Una explosion: destello, de 40 a 60 bombas desde la cima y los rios
    /// de lava un poco mas largos.
    fn explode(&mut self, wall: f32, limb: f32) {
        self.count += 1;
        let h = hash_u32(self.count.wrapping_mul(0x2545_F491) ^ 0xF0E6);
        let r = |k: u32| unit(hash_u32(h ^ k.wrapping_mul(0x9E37_79B9)));
        self.flash = 1.0;
        let summit = FUEGO.height - limb;
        let n = 40 + h % 21;
        for i in 0..n {
            // Salen en medio segundo (mas al principio), mas hacia arriba
            // que de lado: un surtidor y no una cascara de cohete.
            let delay = 0.5 * r(4 * i + 4) * r(4 * i + 4);
            self.bombs.push(Bomb {
                psi: FUEGO.psi + (r(4 * i + 1) - 0.5) * 0.008,
                p: summit,
                vpsi: (r(4 * i + 2) - 0.5) * 0.07,
                vp: 0.05 + 0.09 * r(4 * i + 3),
                age: -delay,
            });
        }
        for f in &mut self.flows {
            *f = (*f + 0.15).min(1.0);
        }
        self.next = wall + 6.0 + 5.0 * r(9999);
    }
}

/// Una estrella fugaz: cuando nacio (reloj de pared), de donde sale
/// (rumbo y elevacion), cuanto recorre en su vida y cuanto dura.
#[derive(Clone, Copy, Debug)]
struct Meteor {
    born: f32,
    yaw: f32,
    pitch: f32,
    dyaw: f32,
    dpitch: f32,
    dur: f32,
}

/// El cielo pasado a un panorama: rumbo de 0 a 2 pi y altura de -1 a 1.3
/// radianes.
struct Panorama {
    width: usize,
    height: usize,
    /// Ya con tono, un byte por canal (6 MB en vez de 18: cabe mejor en la
    /// cache y se lee por pixel).
    texels: Vec<[u8; 4]>,
    /// El mismo cielo a un cuarto y borroso (`SOFT_W x SOFT_H`): lo que
    /// refleja la pista mojada, que no es un espejo.
    soft: Vec<Vec3>,
}

const SOFT_W: usize = 512;
const SOFT_H: usize = 180;

const PITCH_LOW: f32 = -1.0;
/// El nivel de negro del panorama (luminancia lineal).
const PANO_BLACK: f32 = 0.0025;
const PITCH_HIGH: f32 = 1.3;

impl Panorama {
    fn new(sky: &(dyn Fn(Vec3) -> Vec3 + Sync)) -> Self {
        let (width, height) = (2048, 720);
        let mut texels = vec![[0u8; 4]; width * height];
        parallel::chunks_mut(&mut texels, width, 1, |y, row| {
            let pitch = PITCH_HIGH - (y as f32 + 0.5) / height as f32 * (PITCH_HIGH - PITCH_LOW);
            for (x, t) in row.iter_mut().enumerate() {
                let yaw = (x as f32 + 0.5) / width as f32 * TAU;
                let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
                // Regradeado: media saturacion y mas azul, asi el polvo
                // cafe de la Via Lactea queda plateado.
                let luma = Vec3::new(0.2126, 0.7152, 0.0722);
                let c = sky(dir) * 0.9;
                let lum = c.dot(luma);
                let c = (Vec3::splat(lum) + (c - Vec3::splat(lum)) * 0.5) * Vec3::new(0.75, 0.8, 1.15);
                // Nivel de negro: se resta `PANO_BLACK` a la luminancia
                // conservando el tono (`c * max(l - b, 0) / l`), asi el fondo
                // gris del cubemap se vuelve negro y quedan las estrellas y
                // la Via Lactea.
                let lum = c.dot(luma);
                let c = tone(c * ((lum - PANO_BLACK).max(0.0) / lum.max(1e-6) * 1.08));
                let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                *t = [byte(c.x), byte(c.y), byte(c.z), 255];
            }
        });
        // El suave: el promedio de cada bloque de 4 x 4 y despues una caja
        // de radio 2, dos veces a lo ancho (dando la vuelta) y a lo alto
        // (recortando al borde); dos cajas se parecen a una gaussiana.
        let (bx, by) = (width / SOFT_W, height / SOFT_H);
        let mut soft: Vec<Vec3> = (0..SOFT_W * SOFT_H)
            .map(|i| {
                let (sx, sy) = (i % SOFT_W, i / SOFT_W);
                let mut sum = Vec3::ZERO;
                for y in sy * by..(sy + 1) * by {
                    for x in sx * bx..(sx + 1) * bx {
                        let p = texels[y * width + x];
                        sum += Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
                    }
                }
                sum * (1.0 / (255.0 * (bx * by).max(1) as f32))
            })
            .collect();
        let mut tmp = vec![Vec3::ZERO; SOFT_W * SOFT_H];
        for _ in 0..2 {
            for y in 0..SOFT_H {
                for x in 0..SOFT_W {
                    let mut sum = Vec3::ZERO;
                    for dx in 0..5 {
                        sum += soft[y * SOFT_W + (x + SOFT_W + dx - 2) % SOFT_W];
                    }
                    tmp[y * SOFT_W + x] = sum * 0.2;
                }
            }
            for y in 0..SOFT_H {
                for x in 0..SOFT_W {
                    let mut sum = Vec3::ZERO;
                    for dy in 0..5 {
                        let yy = (y + dy).saturating_sub(2).min(SOFT_H - 1);
                        sum += tmp[yy * SOFT_W + x];
                    }
                    soft[y * SOFT_W + x] = sum * 0.2;
                }
            }
        }
        Self { width, height, texels, soft }
    }

    /// Las dos columnas del cielo suave de un rumbo y el peso de la
    /// segunda (bilineal, dando la vuelta: el ancho es potencia de 2).
    fn soft_cols(yaw: f32) -> (usize, usize, f32) {
        let turns = yaw * (1.0 / TAU);
        let u = (turns - turns.floor()) * SOFT_W as f32 - 0.5;
        let x0 = u.floor();
        let fx = (u - x0).clamp(0.0, 1.0);
        let x0 = (x0 as i32 as usize) & (SOFT_W - 1);
        (x0, (x0 + 1) & (SOFT_W - 1), fx)
    }

    /// Las dos filas del cielo suave de una elevacion y el peso de la
    /// segunda.
    fn soft_rows(pitch: f32) -> (usize, usize, f32) {
        let v = ((PITCH_HIGH - pitch) / (PITCH_HIGH - PITCH_LOW)).clamp(0.0, 1.0) * SOFT_H as f32 - 0.5;
        let y0 = v.floor().max(0.0);
        let fy = (v - y0).clamp(0.0, 1.0);
        let y0 = (y0 as usize).min(SOFT_H - 1);
        (y0, (y0 + 1).min(SOFT_H - 1), fy)
    }

    /// El cielo suave entre esas columnas y filas.
    #[inline]
    fn soft_at(&self, x0: usize, x1: usize, fx: f32, y0: usize, y1: usize, fy: f32) -> Vec3 {
        let t = |x: usize, y: usize| self.soft[(y * SOFT_W + x).min(self.soft.len() - 1)];
        let top = t(x0, y0).lerp(t(x1, y0), fx);
        let bottom = t(x0, y1).lerp(t(x1, y1), fx);
        top.lerp(bottom, fy)
    }

    /// Bilineal, con el rumbo dando la vuelta. El ancho es potencia de 2:
    /// el modulo es un `&` (el `%` de flotantes y el de enteros son lentos
    /// para hacerlos en cada pixel).
    fn sample(&self, yaw: f32, pitch: f32) -> Vec3 {
        let turns = yaw * (1.0 / TAU);
        let u = (turns - turns.floor()) * self.width as f32 - 0.5;
        let v = ((PITCH_HIGH - pitch) / (PITCH_HIGH - PITCH_LOW)).clamp(0.0, 1.0) * self.height as f32 - 0.5;
        let (x0, y0) = (u.floor(), v.floor().max(0.0));
        let (fx, fy) = (u - x0, (v - y0).clamp(0.0, 1.0));
        let mask = self.width - 1;
        let x0 = (x0 as i32 as usize) & mask;
        let x1 = (x0 + 1) & mask;
        let y0 = (y0 as usize).min(self.height - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let t = |x: usize, y: usize| {
            let p = self.texels[y * self.width + x];
            Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
        };
        let top = t(x0, y0).lerp(t(x1, y0), fx);
        let bottom = t(x0, y1).lerp(t(x1, y1), fx);
        top.lerp(bottom, fy) * (1.0 / 255.0)
    }
}

/// Un paso del resorte de la apertura de la camara hacia `target`:
/// `fov'' = 60 (meta - fov) - 9 fov'`, con Euler semi-implicito (primero la
/// velocidad y con ella la posicion). Con pasos largos (un tiron de 0.1 s)
/// no se dispara: la apertura se acota a `[-0.1, 0.5]` y la velocidad a
/// `+-5`.
fn fov_spring(fov: f32, fov_v: f32, target: f32, dt: f32) -> (f32, f32) {
    let fov_v = (fov_v + (60.0 * (target - fov) - 9.0 * fov_v) * dt).clamp(-5.0, 5.0);
    ((fov + fov_v * dt).clamp(-0.1, 0.5), fov_v)
}

/// De la coordenada del trazado (de 0 a 1) a la del mapa en pantalla, con
/// un margen de 0.08 por lado: `0.08 + 0.84 v`. La usan el trazado y los
/// puntos de las naves (sin ella los puntos caian corridos hacia la
/// esquina).
#[inline]
fn map_pad(v: f32) -> f32 {
    const PAD: f32 = 0.08;
    PAD + v * (1.0 - 2.0 * PAD)
}

/// El dibujo del mapa una sola vez: la linea de la pista con su borde,
/// premultiplicada (color y alfa).
fn draw_map(track: &Track, size: usize) -> Vec<[f32; 4]> {
    let mut image = vec![[0.0f32; 4]; size * size];
    let points: Vec<(f32, f32)> = track.map.iter().map(|&(x, y)| (map_pad(x) * size as f32, map_pad(y) * size as f32)).collect();
    let n = points.len();
    parallel::chunks_mut(&mut image, size, 1, |y, row| {
        for (x, px) in row.iter_mut().enumerate() {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let mut d = f32::MAX;
            for i in 0..n {
                let (a, b) = (points[i], points[(i + 1) % n]);
                let (ex, ey) = (b.0 - a.0, b.1 - a.1);
                let t = (((p.0 - a.0) * ex + (p.1 - a.1) * ey) / (ex * ex + ey * ey).max(1e-6)).clamp(0.0, 1.0);
                d = d.min((p.0 - a.0 - ex * t).hypot(p.1 - a.1 - ey * t));
            }
            let line = cover(d - 1.6);
            let border = cover(d - 4.0);
            let back = 0.35 * smoothstep(size as f32 * 0.5, size as f32 * 0.45, (p.0 - size as f32 * 0.5).hypot(p.1 - size as f32 * 0.5));
            let alpha = (back + border * 0.6).min(1.0);
            let c = Vec3::new(0.95, 0.85, 0.7) * line + Vec3::new(0.02, 0.02, 0.03) * border * (1.0 - line);
            *px = [c.x, c.y, c.z, alpha];
        }
    });
    image
}

/// Para medir y fotografiar con los modelos de verdad (desde las pruebas
/// de `minijuegos`).
#[cfg(test)]
impl Carrera {
    /// Cuanto cuesta cada clase de cosa encima de la pista (las filas en
    /// paralelo, el minimo de cinco).
    pub(crate) fn bench_items(&self, rgba: &mut [u8]) {
        let w = self.width;
        let name = |k: &ItemKind| match k {
            ItemKind::Car { .. } => 0,
            ItemKind::Lamp { .. } => 1,
            ItemKind::Kite { .. } => 2,
            ItemKind::Tower { .. } => 3,
            ItemKind::Arch { .. } => 4,
            ItemKind::Puff { .. } => 5,
            ItemKind::Spark { .. } => 6,
            ItemKind::Flame { .. } => 7,
        };
        for (skip, label) in [(99, "todo"), (0, "sin naves"), (1, "sin postes"), (2, "sin barriletes"), (4, "sin arcos"), (5, "sin humo"), (6, "sin chispas"), (7, "sin turbo"), (97, "nada encima")] {
            let mut view = self.view();
            view.items.retain(|i| name(&i.kind) != skip && skip != 97);
            if skip == 0 || skip == 97 {
                view.solid_items.clear();
            }
            let par = (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    parallel::chunks_mut(rgba, w * 4, 1, |y, row| view.row(y, row));
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .fold(f64::MAX, f64::min);
            println!("{label:<14} {par:6.2} ms   ({} cosas)", view.items.len());
        }
        let t = std::time::Instant::now();
        for _ in 0..10 {
            std::hint::black_box(self.view());
        }
        println!("view {:6.2} ms", t.elapsed().as_secs_f64() * 100.0);
        // Cada nave sola y cada clase de cosa sola, en un hilo (sin pista:
        // el minimo de 5).
        let view = self.view();
        let mut row = vec![0u8; w * 4];
        for item in view.items.iter().filter(|i| name(&i.kind) == 0) {
            let ItemKind::Car { id } = item.kind else { continue };
            let cd = view.car_draws[id as usize];
            let t = std::time::Instant::now();
            for y in 0..self.height {
                let yf = y as f32 + 0.5;
                if yf >= item.top && yf <= item.bottom {
                    view.draw_item(item, y, yf, &mut row);
                }
            }
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            if ms > 0.05 {
                println!("    nave {id:2} d {:6.1} k {:6.1} lod {:4.2} barrido {:4.2} lejos {:4.2} solida {}  {ms:5.2} ms", item.depth, item.scale, cd.lod, cd.smear, cd.far, cd.solid);
            }
        }
        for (kind, label) in [(0, "naves"), (1, "postes"), (4, "arcos"), (5, "humo"), (6, "chispas"), (7, "turbo")] {
            let one = (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    for y in 0..self.height {
                        let yf = y as f32 + 0.5;
                        for item in view.items.iter().filter(|i| name(&i.kind) == kind && yf >= i.top && yf <= i.bottom) {
                            view.draw_item(item, y, yf, &mut row);
                        }
                    }
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .fold(f64::MAX, f64::min);
            println!("  {label:<10} un hilo {one:6.2} ms");
        }
    }

    /// Para las fotos: el freno y el turbo de quien juega.
    pub(crate) fn force(&mut self, brake: f32, boost: bool) {
        self.cars[0].brake_light = brake;
        if boost {
            self.cars[0].boost = 2.0;
            self.boost_s = 1.0;
        }
    }

    /// Para las fotos: el turbo de todas las rivales.
    pub(crate) fn boost_rivals(&mut self) {
        for car in self.cars.iter_mut().filter(|c| c.role == Role::Rival) {
            car.boost = 2.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;

    /// Una caja como modelo de prueba (4 de largo en z).
    fn box_mesh() -> Mesh {
        let mut mesh = Mesh::default();
        for z in [-2.0, 2.0] {
            for y in [0.0, 1.0] {
                for x in [-0.5, 0.5] {
                    let p = Vec3::new(x, y, z);
                    mesh.positions.push(p);
                    mesh.normals.push((p - Vec3::new(0.0, 0.5, 0.0)).normalize());
                    mesh.colors.push(Vec3::new(0.6, 0.1, 0.1));
                    mesh.uvs.push(Vec2::new(0.0, 0.3));
                }
            }
        }
        mesh.indices = vec![[0, 1, 3], [0, 3, 2], [4, 6, 7], [4, 7, 5], [0, 4, 5], [0, 5, 1], [2, 3, 7], [2, 7, 6], [0, 2, 6], [0, 6, 4], [1, 5, 7], [1, 7, 3]];
        mesh
    }

    fn game(player: Kind) -> Carrera {
        let mesh = box_mesh();
        let names = ["Gallo", "Tuk-Tuk Elena", "La Mamaneta", "Colectivo Q.5"];
        let models = std::array::from_fn(|k| Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        Carrera::new(models, &[], player, &sky, 320, 180, Records::in_memory())
    }

    fn run(game: &mut Carrera, seconds: f32, keys: Keys) {
        let dt = 1.0 / 60.0;
        for _ in 0..(seconds / dt) as usize {
            game.update(dt as f64, &keys);
        }
    }

    #[test]
    fn the_track_closes_on_itself() {
        let track = Track::new();
        let turn: f32 = track.segments.iter().map(|s| s.curve).sum();
        assert!((turn - TAU).abs() < 1e-3, "{turn}");
        let end = track.segments.last().unwrap().y;
        assert!(end.abs() < 1.0, "termina a {end} m");
        assert!(track.segments.iter().any(|s| s.bumps != 0));
        assert!((1800.0..3600.0).contains(&track.length()), "{}", track.length());
        // El mapa cierra y cabe.
        let (a, b) = (track.map[0], *track.map.last().unwrap());
        assert!((a.0 - b.0).hypot(a.1 - b.1) < 0.05);
        assert!(track.map.iter().all(|&(x, y)| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)));
        // Rumbo continuo y con las vueltas.
        assert!((track.heading(track.length()) - TAU).abs() < 1e-3);
    }

    /// El trazo en metros sigue al rumbo: la recta de largada va hacia +z
    /// (rumbo 0) y el trazo cierra.
    #[test]
    fn the_world_follows_the_heading() {
        let track = Track::new();
        let (a, b) = (track.world_at(0.0), track.world_at(40.0));
        assert!((b.0 - a.0).abs() < 1.0, "{a:?} -> {b:?}");
        assert!((b.1 - a.1 - 40.0).abs() < 1.0, "{a:?} -> {b:?}");
        // Continuo al cerrar la vuelta (y antes de la largada).
        for at in [track.length(), 0.0] {
            let (p, q) = (track.world_at(at - 0.01), track.world_at(at + 0.01));
            assert!((p.0 - q.0).hypot(p.1 - q.1) < 0.1, "{p:?} {q:?}");
        }
    }

    /// La tabla de tono da lo mismo que la curva (a un paso de tabla).
    #[test]
    fn the_tone_table_matches_the_curve() {
        let lut = tone_lut();
        for v in [0.0f32, 0.001, 0.02, 0.1, 0.5, 1.0, 3.0, 20.0] {
            let (a, b) = (tone(Vec3::splat(v)).x, tone_fast(lut, Vec3::splat(v)).x);
            assert!((a - b).abs() < 0.005, "{v}: {a} {b}");
        }
        assert!(tone_fast(lut, Vec3::splat(f32::NAN)).x.is_finite());
        for x in [1e-3f32, 0.3, 1.0, 1.7, 8.0, 900.0, 12_000.0] {
            assert!((fast_log2(x) - x.log2()).abs() < 0.01, "{x}");
        }
    }

    /// El limbo esta a ~0.0707 rad bajo el horizonte a 400 m, el cielo
    /// arriba de el tiene el airglow verde y los volcanes asoman enfrente.
    #[test]
    fn the_limb_the_airglow_and_the_volcanoes() {
        let g = game(Kind::Chela);
        let cam = g.camera();
        assert!((cam.limb() - (2.0 * cam.alt / R_PLANET).sqrt()).abs() < 1e-6);
        assert!((0.065..0.08).contains(&cam.limb()));
        // El verde tiene su pico pegado al limbo, arriba de la franja de
        // sodio, y es mas verde que rojo ahi.
        let glow = sky_lin(0.022);
        assert!(glow.y > sky_lin(0.012).y && glow.y > sky_lin(0.032).y);
        assert!(glow.y > glow.x);
        // Pegado al limbo domina el sodio; arriba, el indigo (azul).
        let limb = sky_lin(0.0);
        assert!(limb.x > limb.y && limb.x > limb.z);
        let high = sky_lin(0.5);
        assert!(high.z > high.x && high.z > high.y);
        assert_eq!(volcano_profile(-0.62).1, 2);
        assert_eq!(volcano_profile(FUEGO.psi).1, 3);
        assert_eq!(volcano_profile(-0.29).1, 4);
        assert_eq!(volcano_profile(2.5).1, 0);
        assert!(volcano_profile(-0.62).0 > 0.15);
        // El Fuego y el Acatenango forman un macizo: la silla entre los
        // dos no baja de 0.1 rad.
        let saddle = (0..=20).map(|k| volcano_profile(-0.29 + 0.09 * k as f32 / 20.0).0).fold(f32::MAX, f32::min);
        assert!(saddle > 0.1, "{saddle}");
        // La falda es concava: a medio ancho queda bajo la recta del cono.
        let half = FUEGO.profile(FUEGO.psi + FUEGO.right * 0.5);
        assert!(half < 0.5 * FUEGO.height * 1.05 && half > 0.2 * FUEGO.height, "{half}");
    }

    /// El rayo contra el planeta: un pixel arriba del limbo da cielo y uno
    /// abajo da suelo; abajo al centro `t` es la raiz analitica; el nivel
    /// de detalle crece hacia el limbo; y lo que se ve en la columna de la
    /// derecha queda a la derecha del rumbo (asi, al doblar a la derecha,
    /// la ciudad corre a la izquierda).
    #[test]
    fn the_ground_hits_below_the_limb() {
        let g = game(Kind::Chela);
        let v = g.view();
        let cam = v.cam;
        let x = g.width / 2;
        let k = v.cols[x].k;
        let limb_px = k * v.limb.tan();
        assert!(v.ray(x, -(limb_px - 1.0)).is_none());
        assert!(v.ray(x, 5.0).is_none());
        let near_limb = v.ray(x, -(limb_px + 1.0)).expect("abajo del limbo hay suelo");
        let bottom = -(g.height as f32 - cam.horizon - 0.5);
        let (t, gx, gz, graze) = v.ray(x, bottom).expect("abajo hay suelo");
        let (r, h) = (R_PLANET as f64, cam.alt as f64);
        let sinp = (-bottom as f64) / (k as f64).hypot(bottom as f64);
        let b = (r + h) * sinp;
        let exact = b - (b * b - 2.0 * r * h - h * h).sqrt();
        assert!(((t as f64 - exact) / exact).abs() < 1e-3, "{t} {exact}");
        assert!(v.lod(near_limb.0, near_limb.3) > v.lod(t, graze) + 3.0);
        assert!(graze > near_limb.3);
        // La derecha del rumbo `h` es `(cos h, -sin h)` (x al este, z al norte).
        let (sh, ch) = cam.heading.sin_cos();
        let right = |x: usize| {
            let (_, px, pz, _) = v.ray(x, bottom).expect("suelo");
            (px - cam.world.0) * ch - (pz - cam.world.1) * sh
        };
        assert!(right(g.width - 1) > 1.0 && right(0) < -1.0);
        assert!(((gx - cam.world.0) * ch - (gz - cam.world.1) * sh).abs() < 0.01 * t);
    }

    /// Cuanto cuesta cada parte del fondo a 1280 x 720
    /// (`cargo test --release bench_background -- --ignored --nocapture`).
    #[test]
    #[ignore = "mide tiempos"]
    fn bench_background() {
        let mesh = box_mesh();
        let names = ["Gallo", "Tuk-Tuk Elena", "La Mamaneta", "Colectivo Q.5"];
        let models = std::array::from_fn(|k| Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let mut g = Carrera::new(models, &[], Kind::Camioneta, &sky, w, h, Records::in_memory());
        g.set_autopilot(true);
        g.start();
        let mut rgba = vec![0u8; w * h * 4];
        let min_of = |f: &mut dyn FnMut()| {
            (0..25)
                .map(|_| {
                    let t = std::time::Instant::now();
                    f();
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .fold(f64::MAX, f64::min)
        };
        let mut at = 0.0;
        for moment in [1.6f32, 16.0, 27.0, 44.0] {
            run(&mut g, moment - at, Keys::default());
            at = moment;
            let full = min_of(&mut || g.draw(&mut rgba));
            let view = g.view();
            let ground = min_of(&mut || {
                rgba.chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                    let yoff = view.cam.horizon - (y as f32 + 0.5);
                    for x in 0..w {
                        let c = view.ground(x, yoff).map_or(Vec3::ZERO, |g| g.0);
                        row[x * 4] = (c.x * 255.0) as u8;
                    }
                })
            });
            let sky = min_of(&mut || {
                rgba.chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
                    let ctx = view.row_ctx(y, &[], (0.0, 0.0));
                    for x in 0..w {
                        let c = if ctx.yoff >= -40.0 { view.sky(x, &ctx).0 } else { Vec3::ZERO };
                        row[x * 4] = (c.x * 255.0) as u8;
                    }
                })
            });
            let views = min_of(&mut || {
                let v = g.view();
                std::hint::black_box(&v.rows);
            });
            let rows = min_of(&mut || {
                for (y, row) in rgba.chunks_mut(w * 4).enumerate() {
                    view.row(y, row);
                }
            });
            println!("t {moment:5.1}: frame {full:6.2} ms   filas 1 hilo {rows:6.2}   ground 1 hilo {ground:6.2}   sky 1 hilo {sky:6.2}   view {views:5.2}");
        }
    }

    /// Cuanto cuesta el frame con humo en la pantalla y con turbo, contra
    /// uno normal, a 1280 x 720
    /// (`cargo test --release medir_humo_y_turbo -- --ignored --nocapture`).
    /// Con `OCASO_PNG=carpeta` guarda las tres imagenes.
    #[test]
    #[ignore = "mide tiempos"]
    fn medir_humo_y_turbo() {
        let mesh = box_mesh();
        let names = ["Gallo", "Tuk-Tuk Elena", "La Mamaneta", "Colectivo Q.5"];
        let models = std::array::from_fn(|k| Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let mut g = Carrera::new(models, &[], Kind::Colectivo, &sky, w, h, Records::in_memory());
        g.set_autopilot(true);
        g.start();
        run(&mut g, 16.0, Keys::default());
        let mut rgba = vec![0u8; w * h * 4];
        let mut measure = |g: &mut Carrera, name: &str| {
            // Calentar: el primer dibujo carga todo en frio.
            for _ in 0..3 {
                g.draw(&mut rgba);
            }
            let mut times: Vec<f64> = (0..15)
                .map(|_| {
                    let t = std::time::Instant::now();
                    g.draw(&mut rgba);
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            times.sort_by(f64::total_cmp);
            println!("{name:<8} mediana {:6.2} ms   peor {:6.2} ms", times[7], times[14]);
            if let Ok(dir) = std::env::var("OCASO_PNG") {
                let path = std::path::Path::new(&dir).join(format!("{name}.png"));
                crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
            }
        };
        // Sin turbo (despues de 16 s el autopiloto puede ir con el) para
        // aislar cada efecto.
        g.cars[0].boost = 0.0;
        g.boost_s = 0.0;
        measure(&mut g, "normal");
        // Humo: la pantalla nublada y soplos adelante.
        g.smoke_screen = 1.0;
        let (s, x) = (g.cars[0].s, g.cars[0].x);
        for k in 0..20 {
            let at = s + 4.0 + k as f32 * 1.5;
            g.puffs.push(Puff { s: at, x: x + 0.8, y: g.track.height(at) + 2.4, age: 0.1 * k as f32, vs: 0.0, r0: 0.35, rot: k as f32, seed: k as f32 * 3.1, own: false });
        }
        measure(&mut g, "humo");
        g.smoke_screen = 0.0;
        g.puffs.clear();
        g.cars[0].boost = 2.0;
        g.boost_s = 1.0;
        measure(&mut g, "turbo");
    }

    /// Con el verde el Fuego explota: destello y bombas que despues caen.
    #[test]
    fn the_fuego_gives_the_start() {
        let mut g = game(Kind::Colectivo);
        g.set_autopilot(true);
        g.start();
        run(&mut g, 2.9, Keys::default());
        let before = g.eruption.count;
        run(&mut g, 0.2, Keys::default());
        assert_eq!(g.phase, Phase::Racing);
        assert!(g.eruption.count > before);
        assert!(g.eruption.flash > 0.5 && (40..=60).contains(&g.eruption.bombs.len()));
        let mut rgba = vec![0u8; 320 * 180 * 4];
        g.draw(&mut rgba);
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        run(&mut g, 4.0, Keys::default());
        assert!(g.eruption.bombs.len() < 40, "{}", g.eruption.bombs.len());
    }

    #[test]
    fn nobody_moves_before_the_green_light() {
        let mut g = game(Kind::TukTuk);
        let mut keys = Keys::default();
        keys.throttle = true;
        run(&mut g, 1.0, keys);
        assert_eq!(g.phase, Phase::Ready);
        keys.enter = true;
        g.update(0.01, &keys);
        keys.enter = false;
        let grid: Vec<f32> = g.cars.iter().map(|c| c.s).collect();
        run(&mut g, 2.0, keys);
        assert!(matches!(g.phase, Phase::Countdown(_)));
        assert!(g.cars.iter().zip(&grid).all(|(c, &s)| c.s == s), "alguien se movio en el semaforo");
        run(&mut g, 2.0, keys);
        assert_eq!(g.phase, Phase::Racing);
        assert!(g.player().v > 5.0);
    }

    /// Con el piloto automatico, todas terminan las tres vueltas en tiempos
    /// de carrera, y gana alguna.
    #[test]
    fn the_autopilot_and_the_rivals_finish_the_race() {
        for player in Kind::ALL {
            let mut g = game(player);
            g.set_autopilot(true);
            g.start();
            run(&mut g, 260.0, Keys::default());
            let length = g.track.length();
            let racers: Vec<&Car> = g.cars.iter().filter(|c| c.role != Role::Traffic).collect();
            for car in &racers {
                let time = car.finished.unwrap_or_else(|| panic!("{:?} no termino ({} de {} m)", car.kind, car.s, LAPS as f32 * length));
                let lap = time / LAPS as f32;
                assert!((25.0..90.0).contains(&lap), "{:?}: {lap} s por vuelta", car.kind);
            }
            assert!(matches!(g.phase, Phase::Finished(_)));
            let standings = g.standings();
            assert_eq!(standings.len(), 4);
            let times: Vec<f32> = standings.iter().map(|&i| g.cars[i].finished.unwrap()).collect();
            assert!(times.windows(2).all(|w| w[0] <= w[1]));
            assert!(g.new_record);
        }
    }

    /// Para ajustar el balance: los tiempos de cada clase con el piloto
    /// automatico (`cargo test --release balance -- --ignored --nocapture`).
    #[test]
    #[ignore = "imprime los tiempos de cada clase"]
    fn balance() {
        for player in Kind::ALL {
            let mut g = game(player);
            g.set_autopilot(true);
            g.start();
            run(&mut g, 260.0, Keys::default());
            let line: Vec<String> = g
                .standings()
                .iter()
                .map(|&i| {
                    let c = &g.cars[i];
                    format!("{:?}{} {:.1}s (vuelta {:.1})", c.kind, if c.role == Role::Player { "*" } else { "" }, c.finished.unwrap_or(0.0), c.best_lap.unwrap_or(0.0))
                })
                .collect();
            println!("{}", line.join("  |  "));
        }
    }

    #[test]
    fn a_rear_end_slows_the_one_behind_and_separates_them() {
        let mut g = game(Kind::Colectivo);
        g.phase = Phase::Racing;
        g.cars.truncate(2);
        g.cars[1].s = g.cars[0].s + 2.0;
        g.cars[1].x = g.cars[0].x;
        g.cars[0].v = 50.0;
        g.cars[1].v = 20.0;
        g.collide();
        let gap = g.cars[1].s - g.cars[0].s;
        let reach = (g.cars[0].kind.spec().length + g.cars[1].kind.spec().length) * 0.5;
        assert!(gap >= reach - 1e-3, "{gap} < {reach}");
        assert!(g.cars[0].v <= 20.0);
    }

    /// Un choque saca chispas en el punto de contacto, una vez (no en cada
    /// paso mientras se tocan).
    #[test]
    fn a_crash_throws_sparks_once() {
        let mut g = game(Kind::Colectivo);
        g.phase = Phase::Racing;
        g.cars.truncate(2);
        g.cars[1].s = g.cars[0].s + 2.0;
        g.cars[1].x = g.cars[0].x;
        g.cars[0].v = 50.0;
        g.cars[1].v = 20.0;
        g.collide();
        assert_eq!(g.sparks.len(), 40);
        g.cars[1].s = g.cars[0].s + 2.0;
        g.collide();
        assert_eq!(g.sparks.len(), 40);
    }

    /// Las chimeneas y el escape salen de las piezas de metal: arriba
    /// adelante, una de cada lado y la mas alta, y abajo atras.
    #[test]
    fn the_rig_finds_the_stacks_and_the_exhaust() {
        let mut mesh = box_mesh();
        for p in [Vec3::new(-0.45, 1.3, -1.8), Vec3::new(-0.45, 1.1, -1.8), Vec3::new(0.45, 1.25, -1.7), Vec3::new(0.0, 0.1, 1.9)] {
            mesh.positions.push(p);
            mesh.normals.push(Vec3::Y);
            mesh.colors.push(Vec3::splat(0.8));
            mesh.uvs.push(Vec2::new(1.0, 0.3));
        }
        let model = Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: 2.0, name: "prueba" };
        let rig = rig_of(&model);
        assert_eq!(rig.stacks.len(), 2, "{:?}", rig.stacks);
        let left = rig.stacks.iter().find(|p| p.x < 0.0).copied().unwrap_or_default();
        let right = rig.stacks.iter().find(|p| p.x > 0.0).copied().unwrap_or_default();
        // En metros: la mas alta de la izquierda, adelante (`z` negativo).
        assert!((left.y - 2.6).abs() < 1e-3 && (left.x + 0.9).abs() < 1e-3 && left.z < -3.0, "{left:?}");
        assert!((right.y - 2.5).abs() < 1e-3 && right.z < -3.0, "{right:?}");
        assert!(rig.exhaust.z > 3.0 && rig.exhaust.y < 0.5, "{:?}", rig.exhaust);
        // Sin metal (la botella), el escape al centro de atras.
        let bottle = Model { mesh: &mesh, near: &mesh, finish: Finish::Bottle, meters: 1.0, name: "botella" };
        let rig = rig_of(&bottle);
        assert!(rig.stacks.is_empty() && rig.exhaust.z > 1.9 && rig.exhaust.x.abs() < 1e-4);
    }

    /// El tuk-tuk se ladea hacia afuera de la curva (a la izquierda al
    /// doblar a la derecha) y endereza al soltar; en un tumulo salta (y
    /// salen chispas que caen y rebotan sin atravesar el asfalto), y el
    /// salto se apaga.
    #[test]
    fn the_tuk_tuk_leans_out_and_bounces_on_bumps() {
        let mut g = game(Kind::TukTuk);
        g.start();
        let gas = Keys { throttle: true, ..Keys::default() };
        run(&mut g, 3.1, gas);
        run(&mut g, 3.0, gas);
        run(&mut g, 1.0, Keys { throttle: true, steer: 1.0, ..Keys::default() });
        let roll = g.cars[0].roll;
        assert!(roll < -0.01 && roll > -0.1, "{roll}");
        run(&mut g, 1.5, gas);
        assert!(g.cars[0].roll.abs() < 0.01, "{}", g.cars[0].roll);
        // Justo antes de un tumulo, en su carril y con velocidad.
        let (i, seg) = g.track.segments.iter().enumerate().find(|(_, s)| s.bumps != 0).map(|(i, s)| (i, *s)).expect("hay tumulos");
        let lane = (0..4).find(|l| seg.bumps & (1 << l) != 0).unwrap_or(0);
        g.cars[0].s = i as f32 * SEGMENT - 0.05 + g.track.length() * 1.0;
        g.cars[0].x = -HALF_ROAD + LANE * (lane as f32 + 0.5);
        g.cars[0].v = 40.0;
        g.cars[0].bounce = 0.0;
        g.cars[0].bounce_v = 0.0;
        g.sparks.clear();
        let mut top = 0.0f32;
        for _ in 0..30 {
            g.physics();
            top = top.max(g.cars[0].bounce);
        }
        assert!(top > 0.05, "{top}");
        assert!(g.sparks.len() >= 40, "{}", g.sparks.len());
        for _ in 0..60 {
            g.physics();
            for p in &g.sparks {
                assert!(p.y.is_finite() && p.y >= g.track.height(p.s) - 1e-3);
            }
        }
        for _ in 0..240 {
            g.physics();
        }
        assert!(g.cars[0].bounce.abs() < 0.01 && g.sparks.is_empty(), "{} {}", g.cars[0].bounce, g.sparks.len());
    }

    /// El humo sale de las chimeneas solo acelerando, avanza frenandose,
    /// sube, crece y se va a los 2.2 s.
    #[test]
    fn the_smoke_rises_grows_and_fades() {
        let mut g = game(Kind::Camioneta);
        g.start();
        run(&mut g, 3.1, Keys::default());
        run(&mut g, 1.0, Keys { throttle: true, ..Keys::default() });
        assert!(!g.puffs.is_empty() && g.puffs.len() <= MAX_PUFFS);
        let p = g.puffs[0];
        run(&mut g, 0.5, Keys::default());
        let q = g.puffs.iter().find(|q| q.seed == p.seed).copied().expect("sigue");
        assert!(q.y > p.y && q.radius() > p.radius() && q.s > p.s && q.vs < p.vs);
        // Sin acelerar no sale mas y los que hay se van.
        run(&mut g, 2.5, Keys { brake: true, ..Keys::default() });
        assert!(g.puffs.is_empty(), "{}", g.puffs.len());
    }

    #[test]
    fn honking_moves_the_traffic_in_front_aside() {
        let mut g = game(Kind::Camioneta);
        g.phase = Phase::Racing;
        g.cars.truncate(1);
        let mut car = Car::new(Kind::TukTuk, Role::Traffic, "", g.cars[0].s + 30.0, g.cars[0].x);
        car.v = 10.0;
        g.cars.push(car);
        g.honk();
        assert!((g.cars[1].target - g.cars[0].x).abs() >= LANE - 1e-3);
    }

    /// El resorte de la apertura llega a su meta, se pasa poco, no se
    /// dispara con pasos largos y `start` lo reinicia.
    #[test]
    fn the_fov_spring_settles_and_resets() {
        let (mut fov, mut v, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for _ in 0..120 {
            (fov, v) = fov_spring(fov, v, 0.2, 1.0 / 60.0);
            peak = peak.max(fov);
        }
        assert!((fov - 0.2).abs() < 1e-3, "{fov}");
        // zeta ~ 0.58: se pasa ~11 % (con el paso discreto, un poco mas).
        assert!(peak > 0.2 && peak < 0.2 * 1.2, "{peak}");
        // Con tirones de 0.1 s sigue acotado y finito.
        for k in 0..200 {
            (fov, v) = fov_spring(fov, v, if k % 7 < 3 { 0.22 } else { 0.0 }, 0.1);
            assert!(fov.is_finite() && v.is_finite() && (-0.1..=0.5).contains(&fov) && v.abs() <= 5.0, "{fov} {v}");
        }
        let mut g = game(Kind::Chela);
        (g.fov, g.fov_v, g.boost_s) = (0.3, 1.0, 0.7);
        g.start();
        assert_eq!((g.fov, g.fov_v, g.boost_s), (0.0, 0.0, 0.0));
    }

    /// Lo opaco que se pinta encima (la manta de la meta) borra la emision
    /// de lo que tapa: la baranda o el cielo que florecian atras ya no
    /// florecen.
    #[test]
    fn what_covers_an_emitter_erases_its_emission() {
        let g = game(Kind::Chela);
        let view = g.view();
        // Una fila del cielo (arriba del horizonte, sin pista) que emite.
        let y = 20;
        assert!(view.rows[y].is_none());
        let mut row = vec![0u8; g.width * 4];
        for p in row.chunks_exact_mut(4) {
            write_emit(p, Vec3::splat(0.8), 1.0);
        }
        // La torre a 10 px por metro con el pie 108 px abajo: la fila cae
        // a 10.8 m de alto, en medio de la manta (de 10 a 11.6 m y hasta
        // `RAIL + 0.8` a cada lado); entre -4 y 4 m no hay focos (el arco
        // va a 5.7 m).
        let kind = ItemKind::Tower { lights: 0, t: 0.0, green: f32::MAX, finished: false };
        let item = Item::new(30.0, kind, 160.0, y as f32 + 0.5 + 108.0, 10.0);
        view.draw_item(&item, y, y as f32 + 0.5, &mut row);
        for x in 120..200 {
            assert_eq!(emission(&row[x * 4..x * 4 + 4]), 0.0, "x = {x}");
        }
        // Afuera de la valla sigue emitiendo.
        assert_eq!(emission(&row[10 * 4..10 * 4 + 4]), 1.0);
    }

    /// La franja filtrada: de cerca es la franja con su borde, de lejos su
    /// promedio, y siempre lo mismo que integrar el patron a mano.
    #[test]
    fn the_band_is_the_box_filtered_stripe() {
        // Ventana chica: adentro 1, afuera 0 (rayas de 6 m de cada 12).
        assert!((band(3.0, 12.0, 6.0, 0.01) - 1.0).abs() < 1e-4);
        assert!(band(9.0, 12.0, 6.0, 0.01) < 1e-4);
        // Ventana de varios periodos: el promedio, `on / P`.
        assert!((band(5.3, 12.0, 6.0, 120.0) - 0.5).abs() < 0.03);
        // Con miles de metros no pierde el borde.
        let far = 9000.0 + 12.0 * 7.0;
        assert!((band(far + 3.0, 12.0, 6.0, 0.02) - 1.0).abs() < 1e-3);
        assert!(band(far - 3.0, 12.0, 6.0, 0.02) < 1e-3);
        // Contra la integral numerica, en bordes y ventanas cualquiera.
        for &(s, w) in &[(5.9f32, 0.5f32), (11.7, 1.3), (-2.4, 3.0), (100.1, 0.2), (31.98, 0.1)] {
            let n = 4000;
            let on = (0..n).filter(|&i| (s - 0.5 * w + (i as f32 + 0.5) * w / n as f32).rem_euclid(12.0) < 6.0).count() as f32 / n as f32;
            assert!((band(s, 12.0, 6.0, w) - on).abs() < 2e-3, "s {s} w {w}: {} {on}", band(s, 12.0, 6.0, w));
        }
        // Sin NaN con ventanas nulas o negativas.
        assert!(band(1.0, 4.0, 1.6, 0.0).is_finite() && band(1.0, 4.0, 1.6, -1.0).is_finite());
    }

    /// El resplandor filtrado en tienda: con ventana chica es la
    /// exponencial, con ventana grande reparte su integral `2a` (pico
    /// `2a / w`), siempre lo mismo que convolucionar a mano, sin NaN con
    /// ventanas enormes y sin escalones de una `d` a la siguiente.
    #[test]
    fn the_glow_tent_keeps_its_light() {
        for &d in &[0.0f32, 0.1, -0.3, 1.0] {
            assert!((exp_tent(d, 0.22, 1e-3) - (-d.abs() / 0.22).exp()).abs() < 2e-3, "{d}");
        }
        assert!((exp_tent(0.0, 0.22, 20.0) - 0.44 / 20.0).abs() < 1e-3);
        assert!(exp_tent(5.0, 0.22, 0.1) < 1e-6);
        // Contra la convolucion numerica con la tienda.
        for &(d, w) in &[(0.0f32, 0.05f32), (0.03, 0.05), (0.2, 0.3), (-0.5, 0.3), (1.0, 2.0), (3.0, 2.5), (0.4, 0.4), (-7.0, 6.0)] {
            let n = 20_000;
            let mut sum = 0.0f64;
            for i in 0..n {
                let tt = -w + (i as f32 + 0.5) * 2.0 * w / n as f32;
                let k = (1.0 - tt.abs() / w) / w;
                sum += (k * (-(d - tt).abs() / 0.22).exp()) as f64 * (2.0 * w / n as f32) as f64;
            }
            let v = exp_tent(d, 0.22, w);
            assert!((v as f64 - sum).abs() < 2e-3, "d {d} w {w}: {v} {sum}");
        }
        // Sin NaN ni infinitos con ventanas grandes y lejos.
        for &(d, w) in &[(15.9f32, 60.0f32), (0.0, 1e4), (-16.0, 30.0), (16.0, 0.0)] {
            let v = exp_tent(d, 0.22, w);
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "d {d} w {w}: {v}");
        }
        // Sin escalones: con la ventana de una fila lejana (2.5 m), entre dos
        // `d` a 5 cm no cambia mas que la pendiente lo permite.
        let mut last = exp_tent(-4.0, 0.22, 2.5);
        for i in 1..=160 {
            let v = exp_tent(-4.0 + i as f32 * 0.05, 0.22, 2.5);
            assert!((v - last).abs() < 0.01, "{i}: {last} {v}");
            last = v;
        }
        assert!((0.0..=1.0).contains(&tracer(0.5)) && tracer(0.1) > 0.29 && tracer(0.0) == 0.0);
    }

    /// El tramo filtrado: adentro 1, afuera 0, y una ventana mas ancha que
    /// el tramo cubre su fraccion.
    #[test]
    fn the_window_cover_is_the_overlap() {
        assert!((window_cover(0.0, 1.0, 0.5) - 1.0).abs() < 1e-6);
        assert!(window_cover(3.0, 1.0, 0.5) == 0.0);
        assert!((window_cover(1.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((window_cover(0.0, 1.0, 8.0) - 0.25).abs() < 1e-6);
        assert!(window_cover(0.0, 1.0, 0.0).is_finite());
    }

    /// El asfalto no hierve: dos frames a 70 m/s con `dt` de 1/60 y de
    /// 1/57 s, a unos 3000 m de la salida, dan casi el mismo grano y las
    /// mismas manchas (pegados a `s`). Estirarlos dividiendo `s` entre la
    /// velocidad daba otro ruido en cada frame.
    #[test]
    fn the_asphalt_noise_stays_put_when_dt_shakes() {
        let (b0, b1) = (70.0f32 / 60.0, 70.0f32 / 57.0);
        let (mut grain, mut patch, mut size) = (0.0f32, 0.0f32, 0.0f32);
        for k in 0..40 {
            let s = 3000.0 + k as f32 * 0.37;
            let (g0, g1) = (NoiseLine::<100>::blurred(-2.0, s * 6.0, b0 * 6.0), NoiseLine::<100>::blurred(-2.0, s * 6.0, b1 * 6.0));
            let (p0, p1) = (NoiseLine::<8>::blurred(-2.0, s * 0.12, b0 * 0.12), NoiseLine::<8>::blurred(-2.0, s * 0.12, b1 * 0.12));
            for i in 0..60 {
                let x = -1.5 + i as f32 * 1.3;
                grain += (g0.at(x) - g1.at(x)).abs();
                size += g0.at(x).abs();
                patch += (p0.at(x * 0.1) - p1.at(x * 0.1)).abs();
            }
        }
        let n = 40.0 * 60.0;
        assert!(grain / size < 0.3, "grano {}", grain / size);
        assert!(patch / n < 0.01, "manchas {}", patch / n);
        // Quieto, lo mismo que sin promediar.
        let (a, b) = (NoiseLine::<8>::blurred(0.0, 12.3, 0.0), NoiseLine::<8>::new(0.0, 12.3));
        assert!(a.v == b.v);
    }

    /// El ruido por fila da lo mismo que el ruido de valor.
    #[test]
    fn the_noise_line_matches_the_value_noise() {
        for &y in &[0.3f32, -7.6, 1234.56] {
            let line = NoiseLine::<8>::new(-3.1, y);
            for k in 0..60 {
                let x = -3.0 + k as f32 * 0.1;
                assert!((line.at(x) - value_noise(x, y)).abs() < 1e-4, "x {x} y {y}");
            }
        }
        // Fuera de su tramo se queda en la orilla, sin salirse.
        let line = NoiseLine::<8>::new(0.0, 0.0);
        assert!(line.at(-50.0).is_finite() && line.at(50.0).is_finite());
    }

    /// La pista mojada: la fila de pista trae su tabla a lo ancho completa,
    /// con luz, mojado entre 0.3 y 1 y sombra entre 0.4 y 1, y el pixel de
    /// pista no da NaN en ninguna columna.
    #[test]
    fn the_wet_road_is_finite_and_in_range() {
        let mut g = game(Kind::Camioneta);
        g.set_autopilot(true);
        g.start();
        run(&mut g, 6.0, Keys::default());
        let mut rgba = vec![0u8; 320 * 180 * 4];
        // Dos frames: el segundo ya lee la estela del primero.
        g.draw(&mut rgba);
        g.draw(&mut rgba);
        let view = g.view();
        let mut rows = 0;
        for y in 0..g.height {
            let ctx = view.row_ctx(y, &[], (0.0, 0.0));
            let Some(st) = &ctx.near else { continue };
            rows += 1;
            assert!(st.table.len() >= 2 && st.table.len() == st.n);
            for e in &st.table {
                assert!(e.iter().all(|v| v.is_finite()));
                assert!((0.3..=1.0).contains(&e[1]), "{}", e[1]);
                assert!((0.0..=1.0).contains(&e[10]), "{}", e[10]);
                assert!(e[0] >= 0.0);
            }
            for x in 0..g.width {
                let (c, e) = view.road(st, x as f32 + 0.5, &ctx, x);
                assert!(c.x.is_finite() && c.y.is_finite() && c.z.is_finite() && e.is_finite(), "({x}, {y})");
            }
        }
        assert!(rows > 30);
    }

    /// No pintar la pista que la nave propia tapa no cambia nada: la fila
    /// sale igual que pintando todo y pegando la nave encima.
    #[test]
    fn skipping_what_the_ship_covers_changes_nothing() {
        let mut g = game(Kind::Camioneta);
        g.set_autopilot(true);
        g.start();
        run(&mut g, 4.0, Keys::default());
        let view = g.view();
        let w = g.width;
        let mut skipped = 0;
        // Solo las filas de pista (en las del cielo, las bombas del Fuego
        // dependen de la lista de la fila).
        for y in (0..g.height).filter(|&y| view.rows[y].is_some()) {
            for item in &view.solid_items {
                let (sa, sb) = view.solid_cols(item, y, y as f32 + 0.5);
                skipped += sb - sa;
            }
            let mut fast = vec![0u8; w * 4];
            view.row(y, &mut fast);
            // Todo pintado y despues lo de encima.
            let mut full = vec![0u8; w * 4];
            let yf = y as f32 + 0.5;
            let ctx = view.row_ctx(y, &[], (f32::MAX, f32::MIN));
            for x in 0..w {
                let (c, e) = match &ctx.near {
                    Some(st) => view.road(st, x as f32 + 0.5, &ctx, x),
                    None => view.background(x, &ctx),
                };
                write_emit(&mut full[x * 4..x * 4 + 4], c, e);
            }
            view.fireworks_row(y, yf, &ctx, &mut full);
            for item in &view.items {
                view.draw_item(item, y, yf, &mut full);
            }
            for x in 0..w {
                for k in 0..4 {
                    assert!((fast[x * 4 + k] as i32 - full[x * 4 + k] as i32).abs() <= 1, "({x}, {y}) canal {k}: {} {}", fast[x * 4 + k], full[x * 4 + k]);
                }
            }
        }
        // Y si se salta algo (la caja de prueba es opaca).
        assert!(skipped > 100, "{skipped}");
    }

    /// El color del charco de cada nave sale de sus LEDs: con los
    /// modelos de prueba (sin LEDs) es ambar, y siempre con maximo 1.
    #[test]
    fn every_ship_tints_its_pool() {
        let g = game(Kind::TukTuk);
        for t in g.tint {
            let top = t.x.max(t.y).max(t.z);
            assert!((top - 1.0).abs() < 1e-4 && t.x.min(t.y).min(t.z) >= 0.0, "{t:?}");
        }
    }

    /// El tinte de un sprite con LEDs: sale del color de su luz
    /// normalizado, y las blancas no lo lavan.
    #[test]
    fn the_pool_takes_the_color_of_the_leds() {
        let sprite = |glow: Vec<[u8; 4]>| Sprite { width: glow.len(), height: 1, pixels: vec![[90, 90, 90, 255]; glow.len()], glow, texels: Vec::new(), spans: vec![(0, 0)], solid: vec![(0, 0)], anchor: (0.0, 0.0), scale: 1.0, levels: Vec::new() };
        // Una LED magenta y laca gris que no emite.
        let t = led_tint(&sprite(vec![[200, 20, 120, 0], [0, 0, 0, 0]]));
        let want = Vec3::new(1.0, 0.1, 0.6);
        assert!((t - want).length() < 1e-3, "{t:?}");
        // Magenta y ambar con dos faros blancos: queda entre las dos de
        // color, no gris.
        let t = led_tint(&sprite(vec![[255, 0, 160, 0], [255, 150, 20, 0], [250, 250, 250, 0], [250, 250, 250, 0]]));
        let sat = (t.x - t.x.min(t.y).min(t.z)) / t.x;
        assert!((t.x - 1.0).abs() < 1e-4 && sat > 0.6, "{t:?}");
        // Sin LEDs, ambar.
        let t = led_tint(&sprite(Vec::new()));
        assert!((t - Vec3::new(1.0, 0.68, 0.16)).length() < 1e-4);
    }

    #[test]
    fn the_frame_has_no_holes_before_and_during_the_race() {
        let mut g = game(Kind::Chela);
        let mut rgba = vec![0u8; 320 * 180 * 4];
        g.draw(&mut rgba);
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        super::super::assert_loaded_glyphs(&g.overlay());
        g.set_autopilot(true);
        g.start();
        run(&mut g, 1.5, Keys::default());
        super::super::assert_loaded_glyphs(&g.overlay());
        run(&mut g, 10.5, Keys::default());
        g.draw(&mut rgba);
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        assert!(!g.overlay().items.is_empty());
        super::super::assert_loaded_glyphs(&g.overlay());
        // Hasta los resultados.
        run(&mut g, 250.0, Keys::default());
        assert!(matches!(g.phase, Phase::Finished(t) if t > 1.0));
        super::super::assert_loaded_glyphs(&g.overlay());
        let mut sound = vec![0.0; 4410];
        g.audio(&mut sound);
        assert!(sound.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    /// Los arcos de foquitos: solo en sus tuneles, nunca encima de un poste
    /// (cada 16 m corridos 8, los postes cada 48 en cada vuelta) ni a menos
    /// de 20 m de la meta, y la luz que cae es finita, cero lejos y mas
    /// fuerte bajo el arco.
    #[test]
    fn the_arches_light_their_tunnels() {
        assert!(arches_at(30.0) && arches_at(2300.0) && arches_at(1100.0));
        assert!(!arches_at(200.0) && !arches_at(0.0) && !arches_at(10.0) && !arches_at(2360.0));
        let length = Track::new().length();
        let mut k = 0;
        while (k as f32) * ARCH_EVERY + ARCH_OFFSET < length {
            let s = k as f32 * ARCH_EVERY + ARCH_OFFSET;
            if arches_at(s) {
                let ds = s.rem_euclid(LAMP_EVERY);
                assert!(ds.min(LAMP_EVERY - ds) > 4.0, "arco encima del poste en {s}");
                assert!(s.min(length - s) >= 20.0, "arco junto a la meta en {s}");
            }
            k += 1;
        }
        let g = game(Kind::Chela);
        assert_eq!(g.arch_spill(200.0), Vec3::ZERO);
        let under = g.arch_spill(ARCH_EVERY + ARCH_OFFSET);
        let between = g.arch_spill(1.5 * ARCH_EVERY + ARCH_OFFSET);
        assert!(under.x + under.y + under.z > 0.5, "{under:?}");
        assert!(between.x + between.y + between.z < 0.1 * (under.x + under.y + under.z), "{between:?}");
        for i in 0..400 {
            let v = g.arch_spill(-50.0 + i as f32 * 7.3);
            assert!(v.x.is_finite() && v.y.is_finite() && v.z.is_finite() && v.x >= 0.0 && v.x <= 1.0);
        }
        // Los focos de un arco: la semielipse, de orilla a orilla.
        let bulbs = arch_bulbs();
        assert!(bulbs[0].0 > 0.99 && bulbs[ARCH_BULBS - 1].0 < -0.99 && bulbs[ARCH_BULBS / 2].1 > 0.99);
    }

    /// La torre: las patas abren afuera de las barandas y el arco deja
    /// pasar la pista; arriba es angosta y nunca da NaN.
    #[test]
    fn the_tower_stands_over_the_road() {
        assert!(tower_inner(0.0) > RAIL && tower_outer(0.0) > tower_inner(0.0) + 2.0);
        assert!((tower_outer(0.0) - (2.2 + RAIL + 2.6)).abs() < 1e-4);
        assert_eq!(tower_inner(13.0), 0.0);
        assert!((tower_outer(60.0) - 2.2).abs() < 1e-4);
        let mut last = f32::MAX;
        for i in 0..=140 {
            let my = -2.0 + i as f32 * 0.5;
            let (o, n) = (tower_outer(my), tower_inner(my));
            assert!(o.is_finite() && n.is_finite() && n >= 0.0 && o > n);
            assert!(o <= last + 1e-4);
            last = o;
        }
        assert!(beacon_spread(1.0) >= 6.0 && beacon_spread(20.0) == 30.0 && beacon_spread(400.0) == 40.0);
    }

    /// Un cohete: sale del centro, se abre hasta ~0.18 rad, cae y se
    /// apaga; sus chispas no dan NaN y la crepitante destella.
    #[test]
    fn a_burst_opens_falls_and_fades() {
        for kind in 0..5u8 {
            let b = Burst { yaw: 0.3, pitch: 0.2, born: 10.0, seed: 0xABCD + kind as u32, kind };
            assert!(b.age(10.0 + RISE).abs() < 1e-5);
            let (dy, dp) = b.spark(3, 0.0);
            assert!(dy.abs() < 1e-6 && dp.abs() < 1e-6);
            let mut sparkle = 0;
            for i in 0..SPARKS {
                let (y, p) = b.spark(i, 2.0);
                assert!(y.is_finite() && p.is_finite());
                assert!(y.hypot(p + if kind == 0 { 0.12 } else { 0.06 }) < 0.19, "{kind} {i}: {y} {p}");
                let c = b.color(i);
                assert!(c.x.is_finite() && c.x.max(c.y).max(c.z) > 0.5);
                assert!(b.bright(i, 0.5) < b.bright(i, 0.0) * if kind == 4 { 2.0 } else { 1.0 } + 1e-6);
                sparkle += (b.bright(i, 1.0) > (-1.3f32).exp() * 1.5) as u32;
            }
            assert_eq!(sparkle > 0, kind == 4, "{kind}");
            // Cae: la elevacion promedio baja con el tiempo.
            let mean = |t: f32| (0..SPARKS).map(|i| b.spark(i, t).1).sum::<f32>() / SPARKS as f32;
            assert!(mean(3.0) < mean(1.0) - 0.01);
        }
    }

    /// Con el verde salen seis cohetes, en la carrera salen de ambiente y
    /// se van solos; al cruzar la meta salen cuatro mas. El frame no
    /// queda con huecos y el pum suena sin pasarse.
    #[test]
    fn the_rockets_go_up_with_the_green_and_at_each_lap() {
        let mut g = game(Kind::Colectivo);
        g.set_autopilot(true);
        g.start();
        run(&mut g, 3.05, Keys::default());
        assert_eq!(g.phase, Phase::Racing);
        assert_eq!(g.bursts.len(), 6);
        assert!(g.green_at.is_some());
        let mut rgba = vec![0u8; 320 * 180 * 4];
        run(&mut g, 0.6, Keys::default());
        g.draw(&mut rgba);
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        {
            let view = g.view();
            assert!(!view.fw_segs.is_empty());
            assert!(view.fw_flash.x.max(view.fw_flash.y).max(view.fw_flash.z) <= 0.02 + 1e-6);
        }
        let mut sound = vec![0.0; 44_100];
        g.audio(&mut sound);
        assert!(sound.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        run(&mut g, 4.5, Keys::default());
        assert!(g.bursts.iter().all(|b| b.age(g.wall) < BURST_LIFE));
        assert!(g.bursts.len() <= MAX_BURSTS);
        // Al cruzar la meta.
        let length = g.track.length();
        g.cars[0].s = length - 0.2;
        let before = g.burst_count;
        g.update(1.0 / 60.0, &Keys::default());
        assert!(g.burst_count >= before + 4, "{} {}", before, g.burst_count);
    }

    /// Los barriletes: dentro de la vuelta, los del festival de los dos
    /// lados (las curvas cambian de sentido) y de tres tamanos.
    #[test]
    fn the_kites_line_the_lap() {
        let track = Track::new();
        let spots = kite_spots(&track);
        let length = track.length();
        assert!(spots.iter().all(|k| (0.0..length).contains(&k.s) && k.side.abs() == 1.0 && (k.design as usize) < escenario::KITE_DESIGNS));
        let festival: Vec<&KiteSpot> = spots.iter().filter(|k| (FESTIVAL.0..FESTIVAL.1).contains(&k.s)).collect();
        assert!(festival.len() >= 12);
        assert!(festival.iter().any(|k| k.side > 0.0) && festival.iter().any(|k| k.side < 0.0));
        let sizes: Vec<f32> = festival.iter().map(|k| k.size).collect();
        assert!(sizes.contains(&9.0) && sizes.contains(&14.0) && sizes.contains(&18.0));
        // Los ids no se repiten.
        let mut ids: Vec<u32> = spots.iter().map(|k| k.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), spots.len());
        // Todos a medio camino entre dos postes y ninguno en un tunel.
        for k in &spots {
            assert!(((k.s - 0.5 * LAMP_EVERY) / LAMP_EVERY).fract().abs() < 1e-4, "{}", k.s);
            assert!(!ARCH_ZONES.iter().any(|&(a, b)| (a - 6.0..b + 6.0).contains(&k.s)), "{}", k.s);
        }
    }

    /// Los postes: se cuentan por vuelta, en orden, ninguno a menos de
    /// `LAMP_CLEAR` m de la meta, y `lamp_after` da el primero despues.
    #[test]
    fn the_lamps_skip_the_finish_line() {
        let length = Track::new().length();
        let mut last = f32::MIN;
        for n in -100i64..300 {
            let (s, side) = lamp(n, length);
            assert!(s > last && side.abs() == 1.0);
            last = s;
            let lap_s = s.rem_euclid(length);
            assert!(lap_s.min(length - lap_s) >= LAMP_CLEAR, "{s}");
            assert_eq!(lamp_after(s - 0.01, length), n);
            assert_eq!(lamp_after(s, length), n + 1);
        }
    }

    /// Lo que se dibuja encima solo cae entre sus filas: fuera de
    /// `[top, bottom]` no toca nada (asi las filas se lo pueden saltar), en
    /// la largada (la torre y los arcos), en la carrera y en el festival.
    #[test]
    fn the_items_stay_inside_their_rows() {
        let mut g = game(Kind::TukTuk);
        g.set_autopilot(true);
        let mut checked = 0;
        for (at, start) in [(0.0f32, false), (2.0, true), (12.0, false)] {
            if start {
                g.start();
            }
            run(&mut g, at, Keys::default());
            for festival_s in [None, Some(1900.0)] {
                if let Some(s) = festival_s {
                    g.cars[0].s = s;
                }
                let view = g.view();
                for item in &view.items {
                    for y in 0..g.height {
                        let yf = y as f32 + 0.5;
                        if yf >= item.top && yf <= item.bottom {
                            continue;
                        }
                        let mut row: Vec<u8> = (0..g.width * 4).map(|i| (i * 37 % 251) as u8).collect();
                        let before = row.clone();
                        view.draw_item(item, y, yf, &mut row);
                        assert!(row == before, "{:?} pinto la fila {y} fuera de [{}, {}]", item.kind, item.top, item.bottom);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 1000, "{checked}");
    }

    /// Lo mismo a 1280 x 720 y muy de cerca: en la largada (la torre y el
    /// primer arco encima), pasando bajo un arco dentro del tunel y un
    /// barrilete y un arco puestos a mano a 300 px por metro. Ademas lo que
    /// pintan de cerca no da NaN (que saldria negro).
    #[test]
    fn the_close_items_stay_inside_their_rows() {
        let mesh = box_mesh();
        let names = ["Gallo", "Tuk-Tuk Elena", "La Mamaneta", "Colectivo Q.5"];
        let models = std::array::from_fn(|k| Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let mut g = Carrera::new(models, &[], Kind::Chela, &sky, w, h, Records::in_memory());
        let check = |view: &View, item: &Item| {
            for y in 0..h {
                let yf = y as f32 + 0.5;
                let mut row: Vec<u8> = (0..w * 4).map(|i| 60 + (i * 37 % 131) as u8).collect();
                let before = row.clone();
                view.draw_item(item, y, yf, &mut row);
                if yf < item.top || yf > item.bottom {
                    assert!(row == before, "{:?} pinto la fila {y} fuera de [{}, {}]", item.kind, item.top, item.bottom);
                } else {
                    assert!(row.chunks_exact(4).all(|p| p[0] > 0 || p[1] > 0 || p[2] > 0), "{:?} dejo un pixel negro en la fila {y}", item.kind);
                }
            }
        };
        // En la largada.
        {
            let view = g.view();
            assert!(view.items.iter().any(|i| matches!(i.kind, ItemKind::Tower { .. })));
            for item in &view.items {
                check(&view, item);
            }
        }
        // Bajo un arco del tunel de la primera loma.
        g.cars[0].s = 72.0 + 4.0;
        {
            let view = g.view();
            let close = view.items.iter().filter(|i| matches!(i.kind, ItemKind::Arch { .. }) && i.scale > 100.0).count();
            assert!(close > 0);
            for item in &view.items {
                check(&view, item);
            }
        }
        // A mano, a 300 px por metro.
        let view = g.view();
        for kind in [ItemKind::Kite { design: 2, size: 14.0, id: 3, squash: 0.8, lift: 12.0 }, ItemKind::Arch { id: 1, squash: 0.6, signal: 0 }] {
            let mut item = Item::new(3.0, kind, 640.0, 4000.0, 300.0);
            (item.top, item.bottom) = g.item_rows(&item);
            check(&view, &item);
        }
    }

    /// Fotos de la torre en verde y los cohetes, con su tiempo
    /// (`OCASO_PNG=carpeta cargo test --release fotos_cohetes -- --ignored --nocapture`).
    #[test]
    #[ignore = "guarda fotos"]
    fn fotos_cohetes() {
        let mesh = box_mesh();
        let names = ["Gallo", "Tuk-Tuk Elena", "La Mamaneta", "Colectivo Q.5"];
        let models = std::array::from_fn(|k| Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.0008, 0.001, 0.0025) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let mut g = Carrera::new(models, &[], Kind::Chela, &sky, w, h, Records::in_memory());
        g.set_autopilot(true);
        g.start();
        let mut rgba = vec![0u8; w * h * 4];
        let mut at = 0.0;
        for moment in [2.5f32, 3.1, 3.6, 4.2, 5.0, 14.0, 22.0] {
            run(&mut g, moment - at, Keys::default());
            at = moment;
            let mut times: Vec<f64> = (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    g.draw(&mut rgba);
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            times.sort_by(f64::total_cmp);
            // Lo mismo sin el humo de la camioneta (para separar lo que
            // cuesta cada cosa).
            let puffs = std::mem::take(&mut g.puffs);
            let smoke = std::mem::replace(&mut g.smoke_screen, 0.0);
            let mut clean: Vec<f64> = (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    g.draw(&mut rgba);
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            clean.sort_by(f64::total_cmp);
            (g.puffs, g.smoke_screen) = (puffs, smoke);
            g.draw(&mut rgba);
            println!("t {moment:5.1}: {:6.2} ms ({:6.2} sin humo)  cohetes {}", times[2], clean[2], g.bursts.len());
            if let Ok(dir) = std::env::var("OCASO_PNG") {
                let path = std::path::Path::new(&dir).join(format!("cohetes_{moment:04.1}.png"));
                crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
            }
        }
        // La torre de lejos al volver a la meta (la celosia sin moire): dos
        // frames seguidos a 50, 80 y 120 m.
        let length = g.track.length();
        for d in [50.0f32, 80.0, 120.0] {
            for frame in 0..2 {
                g.cars[0].s = 2.0 * length - d + frame as f32 * 1.0;
                g.puffs.clear();
                g.draw(&mut rgba);
                if let Ok(dir) = std::env::var("OCASO_PNG") {
                    let path = std::path::Path::new(&dir).join(format!("torre_{d:03.0}_{frame}.png"));
                    crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
                }
            }
        }
    }

    /// Cuanto se ahorra sin cada clase de cosa (filas en un hilo y en
    /// paralelo), por omision a los 5 s con la chela, pasando bajo la torre
    /// (`OCASO_NAVE=9 OCASO_T=7 cargo test --release bench_without --
    /// --ignored --nocapture` para el colectivo a los 7 s; 7 el tuk-tuk y 8
    /// la camioneta).
    #[test]
    #[ignore = "mide tiempos"]
    fn bench_without() {
        let mesh = box_mesh();
        let names = ["Gallo", "Tuk-Tuk Elena", "La Mamaneta", "Colectivo Q.5"];
        let models = std::array::from_fn(|k| Model { mesh: &mesh, near: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let kind = match std::env::var("OCASO_NAVE").as_deref() {
            Ok("7") => Kind::TukTuk,
            Ok("8") => Kind::Camioneta,
            Ok("9") => Kind::Colectivo,
            _ => Kind::Chela,
        };
        let at: f32 = std::env::var("OCASO_T").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
        let mut g = Carrera::new(models, &[], kind, &sky, w, h, Records::in_memory());
        g.set_autopilot(true);
        g.start();
        run(&mut g, at, Keys::default());
        let mut rgba = vec![0u8; w * h * 4];
        g.draw(&mut rgba);
        let name = |k: &ItemKind| match k { ItemKind::Car { .. } => 0, ItemKind::Lamp { .. } => 1, ItemKind::Kite { .. } => 2, ItemKind::Tower { .. } => 3, ItemKind::Arch { .. } => 4, ItemKind::Puff { .. } => 5, ItemKind::Spark { .. } => 6, ItemKind::Flame { .. } => 7 };
        for (skip, label) in [(99, "todo"), (0, "sin naves"), (1, "sin postes"), (2, "sin barriletes"), (3, "sin torre"), (4, "sin arcos"), (5, "sin humo"), (98, "sin cohetes"), (97, "nada encima")] {
            let mut view = g.view();
            view.items.retain(|i| name(&i.kind) != skip && skip != 97);
            if skip == 98 || skip == 97 {
                view.fw_segs.clear();
            }
            let one = (0..3)
                .map(|_| {
                    let t = std::time::Instant::now();
                    for (y, row) in rgba.chunks_mut(w * 4).enumerate() {
                        view.row(y, row);
                    }
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .fold(f64::MAX, f64::min);
            let par = (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    parallel::chunks_mut(&mut rgba, w * 4, 1, |y, row| view.row(y, row));
                    t.elapsed().as_secs_f64() * 1000.0
                })
                .fold(f64::MAX, f64::min);
            println!("{label:<16} un hilo {one:7.2} ms   en paralelo {par:6.2} ms   ({} cosas)", view.items.len());
        }
        let t = std::time::Instant::now();
        for _ in 0..10 {
            std::hint::black_box(g.view());
        }
        println!("view {:6.2} ms", t.elapsed().as_secs_f64() * 100.0);
    }
}
