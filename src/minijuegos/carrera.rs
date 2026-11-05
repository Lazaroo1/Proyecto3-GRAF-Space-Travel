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
//! - **Las naves son sprites horneados** al abrir el juego (`hornear`) con
//!   los modelos de Blender: 15 angulos para verlas desde cualquier lado y 9
//!   de cerca para la propia, que se ladea al girar. Se eligen por el angulo
//!   entre el rumbo de la nave y el rayo de la camara.
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
use super::hornear::{self, Finish, Sprite};
use super::lienzo::{self, cover, emission, halo, over, write_emit};
use super::posproceso::{self, Posproceso};
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::math::{Vec3, fast_atan2, fast_exp, hash_u32, smoothstep};
use crate::parallel;
use crate::render::transition;
use crate::scene::Mesh;

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
/// Un poste de luz cada tanto, de un lado y del otro.
const LAMP_EVERY: f32 = 48.0;
const BILLBOARD_EVERY: f32 = 230.0;
/// Rumbo de los angulos horneados: de -70 a 70 grados, cada 10.
const YAWS: usize = 15;
const YAW_STEP: f32 = 10.0 * PI / 180.0;
/// Los de la nave propia: de -20 a 20, cada 5.
const PLAYER_YAWS: usize = 9;
const PLAYER_YAW_STEP: f32 = 5.0 * PI / 180.0;
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
        }
    }

    fn lap(&self, length: f32) -> u32 {
        (self.s.max(0.0) / length) as u32
    }
}

/// Un soplo de humo negro de la camioneta, quieto en la pista.
#[derive(Clone, Copy, Debug)]
struct Puff {
    s: f32,
    x: f32,
    y: f32,
    age: f32,
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
    panorama: Panorama,
    /// La capital horneada, el gradiente del cielo sobre el limbo (lineal y
    /// ya con tono) y lo que pasa en el: el Fuego y las estrellas fugaces.
    ciudad: Ciudad,
    sky_lin_lut: Vec<Vec3>,
    sky_lut: Vec<Vec3>,
    eruption: Eruption,
    meteor: Option<Meteor>,
    next_meteor: f32,
    minimap: Vec<[f32; 4]>,
    map_size: usize,
    puffs: Vec<Puff>,
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
    /// La entrada del frame (la fisica la usa en cada paso).
    steer_in: f32,
    throttle_in: bool,
    brake_in: bool,
}

impl Carrera {
    /// Abre la carrera: `models` son las cuatro clases en el orden de
    /// `Kind::ALL`; `sky` da el color del cielo en cada direccion.
    pub fn new(models: [Model; 4], player: Kind, sky: &(dyn Fn(Vec3) -> Vec3 + Sync), width: usize, height: usize, records: Records) -> Self {
        let s = height as f32 / 720.0;
        let focal = FOCAL * height as f32;
        // Los angulos de las otras: vistas de lejos, desde poco arriba.
        let yaws: Vec<f32> = (0..YAWS).map(|k| (k as f32 - (YAWS / 2) as f32) * YAW_STEP).collect();
        let mut jobs = Vec::new();
        for m in &models {
            for &yaw in &yaws {
                jobs.push(hornear::Job { mesh: m.mesh, finish: m.finish, yaw, elevation: 0.16, meters: m.meters, pixels_per_meter: 46.0 * s });
            }
        }
        // La propia, de cerca: tal como la ve la camara, a su tamano en
        // pantalla.
        let spec = player.spec();
        let (back, up) = camera_offset(spec);
        let model = &models[player.index()];
        let elevation = ((up - spec.height * 0.45) / back).atan();
        for k in 0..PLAYER_YAWS {
            let yaw = (k as f32 - (PLAYER_YAWS / 2) as f32) * PLAYER_YAW_STEP;
            jobs.push(hornear::Job { mesh: model.mesh, finish: model.finish, yaw, elevation, meters: model.meters, pixels_per_meter: focal / back });
        }
        let mut baked = hornear::bake_all(&jobs);
        let player_set = baked.split_off(models.len() * YAWS);
        let mut sets: Vec<Vec<Sprite>> = Vec::new();
        while !baked.is_empty() {
            let rest = baked.split_off(YAWS);
            sets.push(std::mem::replace(&mut baked, rest));
        }
        let track = Track::new();
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
            panorama: Panorama::new(sky),
            ciudad: Ciudad::new(),
            sky_lin_lut: (0..SKY_LUT).map(|i| sky_lin(sky_eps(i))).collect(),
            sky_lut: (0..SKY_LUT).map(|i| tone(sky_lin(sky_eps(i)))).collect(),
            eruption: Eruption::new(),
            meteor: None,
            next_meteor: 3.0,
            minimap,
            map_size,
            puffs: Vec::new(),
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
        for k in 0..16u32 {
            let h = hash_u32(0x7EAF ^ k.wrapping_mul(0x9E37));
            let kind = Kind::ALL[(h % 4) as usize];
            let lane = ((h >> 4) % 4) as f32;
            let s = 120.0 + k as f32 * (length - 160.0) / 16.0;
            let mut car = Car::new(kind, Role::Traffic, "", s, -HALF_ROAD + LANE * (lane + 0.5));
            car.v = 16.0 + 10.0 * ((h >> 8) & 0xFF) as f32 / 255.0;
            car.skill = car.v;
            car.lane_timer = 4.0 + ((h >> 16) & 0xFF) as f32 / 20.0;
            cars.push(car);
        }
        self.cars = cars;
        self.clock = 0.0;
        self.puffs.clear();
        self.messages.clear();
        self.new_record = false;
        self.shake = 0.0;
        self.smoke_screen = 0.0;
        self.fov = 0.0;
        self.fov_v = 0.0;
        self.boost_s = 0.0;
    }

    /// Que la IA maneje la nave propia (pruebas y capturas).
    pub fn set_autopilot(&mut self, on: bool) {
        self.autopilot = on;
    }

    /// Empieza el semaforo.
    pub fn start(&mut self) {
        self.reset();
        self.phase = Phase::Countdown(0.0);
    }

    fn player(&self) -> &Car {
        &self.cars[0]
    }

    fn message(&mut self, text: &'static str, color: [u8; 4]) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.text == text) {
            m.age = 0.0;
            return;
        }
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
            // Las barandas.
            let limit = RAIL - spec.width * 0.5;
            if car.x.abs() > limit {
                car.x = car.x.clamp(-limit, limit);
                car.v *= 1.0 - 0.6 * STEP;
            }
            let before = car.s;
            car.s += car.v * STEP;
            car.bumped += STEP;
            // Los tumulos: si se pasa uno en su carril con velocidad.
            let crossed = self.track.index(before) != self.track.index(car.s);
            if crossed && spec.hover == 0.0 {
                let segment = self.track.segment(car.s);
                let lane = (((car.x + HALF_ROAD) / LANE).floor() as i32).clamp(0, 3);
                if segment.bumps & (1 << lane) != 0 && car.v > 18.0 {
                    car.v *= 0.62;
                    if car.role == Role::Player {
                        self.shake = 1.0;
                        self.message("¡Túmulo!", [255, 214, 90, 255]);
                        self.sound(Sound::Bump);
                    }
                }
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
                if player && self.cars[0].bumped > 0.4 {
                    self.cars[0].bumped = 0.0;
                    self.shake = 0.8;
                    self.message("¡Pum!", [255, 130, 100, 255]);
                    self.sound(Sound::Thud);
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

    /// El humo negro de las camionetas que aceleran.
    fn exhaust(&mut self) {
        for i in 0..self.cars.len() {
            let car = &mut self.cars[i];
            if car.kind != Kind::Camioneta || car.v < 8.0 {
                continue;
            }
            car.smoke -= STEP;
            if car.smoke <= 0.0 {
                car.smoke = 0.11;
                let spec = car.kind.spec();
                let s = car.s - spec.length * 0.55;
                let puff = Puff { s, x: car.x + 0.8, y: self.track.height(s) + 2.4, age: 0.0 };
                self.puffs.push(puff);
            }
        }
        for puff in &mut self.puffs {
            puff.age += STEP;
            puff.y += 0.6 * STEP;
        }
        self.puffs.retain(|p| p.age < 2.6);
        // Si quien juega pasa por el humo, se le nubla la vista.
        let (s, x) = (self.cars[0].s, self.cars[0].x);
        let dense = self.puffs.iter().any(|p| (p.s - s).abs() < 2.5 && (p.x - x).abs() < 1.5 + p.age && p.age < 1.6);
        if dense && self.cars[0].kind != Kind::Camioneta {
            if self.smoke_screen < 0.3 {
                self.message("¡Humo negro!", [200, 200, 200, 255]);
            }
            self.smoke_screen = 1.0;
        }
    }
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
    fn update(&mut self, dt: f64, keys: &Keys) {
        let dt = (dt as f32).min(0.1);
        self.wall += dt;
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
                    // Con el verde, el Fuego da la salida.
                    self.eruption.next = self.wall;
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
        self.accumulator += dt;
        while self.accumulator >= STEP {
            self.physics();
            self.accumulator -= STEP;
        }
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
                };
                *age += dt;
            }
            *sample = mix / (1.0 + mix.abs());
        }
        synth.events.retain(|(_, age)| *age < 1.2);
    }

    /// Las filas en paralelo (con la emision de cada pixel en su alfa) y
    /// despues el posproceso: el brillo, el acabado, la velocidad y el
    /// humo, con el mapa encima.
    fn draw(&mut self, rgba: &mut [u8]) {
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
                let (pw, ph) = (640.0 * s, 330.0 * s);
                let (px, py) = ((w - pw) * 0.5, 60.0 * s);
                o.panel(px, py, pw, ph);
                o.text("La carrera del Periférico", w * 0.5, py + 12.0 * s, Font::Big(48.0 * s), accent, Align::Center);
                o.text("Tres vueltas en órbita sobre la capital, de noche", w * 0.5, py + 72.0 * s, Font::Body, dim, Align::Center);
                let mut y = py + 108.0 * s;
                for car in self.cars.iter().filter(|c| c.role != Role::Traffic) {
                    let who = if car.role == Role::Player { format!("Vos · {}", self.player_name()) } else { car.name.to_string() };
                    o.text(who, px + 34.0 * s, y, Font::Bold, if car.role == Role::Player { accent } else { cream }, Align::Left);
                    o.text(car.kind.blurb(), px + 34.0 * s, y + 21.0 * s, Font::Small, dim, Align::Left);
                    y += 44.0 * s;
                }
                let record = match self.records.get(self.player_kind.record()) {
                    Some(ms) => format!("Tu récord con esta nave: {}", clock(ms as f32 / 1000.0)),
                    None => "Sin récord con esta nave".to_string(),
                };
                o.text(record, w * 0.5, py + ph - 52.0 * s, Font::Small, dim, Align::Center);
                o.text("Flechas o W A S D: manejar   ·   Shift: turbo   ·   H: pito   ·   Enter: arrancar   ·   Esc: salir", w * 0.5, py + ph - 28.0 * s, Font::Small, cream, Align::Center);
            }
            Phase::Countdown(t) => {
                let text = match t as u32 {
                    0 => "3",
                    1 => "2",
                    _ => "1",
                };
                let pulse = 1.0 - t.fract();
                o.text(text, w * 0.5, h * 0.2, Font::Big((90.0 + 30.0 * pulse) * s), [255, 120, 100, 255], Align::Center);
            }
            _ => {}
        }
        if matches!(self.phase, Phase::Racing | Phase::Finished(_)) || matches!(self.phase, Phase::Countdown(_)) {
            // La posicion, la vuelta y el tiempo, arriba a la izquierda.
            o.panel(16.0 * s, 16.0 * s, 230.0 * s, 124.0 * s);
            let place = self.place() + 1;
            o.text(ordinal(place), 32.0 * s, 18.0 * s, Font::Big(58.0 * s), if place == 1 { accent } else { cream }, Align::Left);
            o.text("de 4", 112.0 * s, 52.0 * s, Font::Bold, dim, Align::Left);
            let lap = (player.lap(length) + 1).min(LAPS);
            o.text(format!("Vuelta {lap} de {LAPS}"), 32.0 * s, 88.0 * s, Font::Bold, cream, Align::Left);
            let best = player.best_lap.map_or(String::new(), |b| format!("   ·   mejor {}", clock(b)));
            o.text(format!("{}{best}", clock(self.clock)), 32.0 * s, 110.0 * s, Font::Small, dim, Align::Left);
            // La velocidad y el turbo, arriba a la derecha.
            let (pw, px) = (230.0 * s, w - 246.0 * s);
            o.panel(px, 16.0 * s, pw, 104.0 * s);
            o.text(format!("{:.0}", player.v * 3.6), px + pw - 74.0 * s, 16.0 * s, Font::Big(52.0 * s), cream, Align::Right);
            o.text("km/h", px + pw - 18.0 * s, 46.0 * s, Font::Small, dim, Align::Right);
            let turbo_color = if player.boost > 0.0 { [140, 220, 255, 255] } else if player.turbo > 0.3 { accent } else { [200, 190, 176, 255] };
            o.text("Turbo (Shift)", px + 16.0 * s, 78.0 * s, Font::Small, dim, Align::Left);
            o.bar(px + 110.0 * s, 84.0 * s, pw - 126.0 * s, 8.0 * s, if player.boost > 0.0 { 1.0 } else { player.turbo }, turbo_color, false);
            // Los nombres de las rivales sobre sus naves.
            for (place, &i) in self.standings().iter().enumerate() {
                let car = &self.cars[i];
                if car.role != Role::Rival {
                    continue;
                }
                if let Some((x, y)) = self.label_at(car) {
                    o.text(format!("{} {}", ordinal(place + 1), car.name), x, y, Font::Small, [255, 240, 220, 230], Align::Center);
                }
            }
        }
        // Los avisos, al centro (no encima de los resultados).
        let results = matches!(self.phase, Phase::Finished(t) if t > 1.0);
        for (k, m) in self.messages.iter().enumerate().filter(|_| !results) {
            let alpha = (1.0 - (m.age / 1.4).powi(3)).clamp(0.0, 1.0);
            let mut color = m.color;
            color[3] = (color[3] as f32 * alpha) as u8;
            o.text(m.text, w * 0.5, h * 0.27 + k as f32 * 46.0 * s - m.age * 12.0 * s, Font::Big(36.0 * s), color, Align::Center);
        }
        if let Phase::Finished(t) = self.phase {
            if t > 1.0 {
                self.results(&mut o);
            }
        }
        if matches!(self.phase, Phase::Racing) {
            o.text("Esc: salir de la carrera", w - 16.0 * s, h - 28.0 * s, Font::Small, [200, 190, 176, 150], Align::Right);
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

    fn results(&self, o: &mut Overlay) {
        let s = self.height as f32 / 720.0;
        let w = self.width as f32;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 210];
        let accent = [255, 204, 120, 255];
        let (pw, ph) = (520.0 * s, 330.0 * s);
        let (px, py) = ((w - pw) * 0.5, 150.0 * s);
        o.panel(px, py, pw, ph);
        let place = self.place();
        let headline = match place {
            0 => "¡Ganaste!",
            1 => "Segundo lugar",
            2 => "Tercer lugar",
            _ => "Último… ¡a la otra!",
        };
        o.text(headline, w * 0.5, py + 14.0 * s, Font::Big(46.0 * s), if place == 0 { accent } else { cream }, Align::Center);
        let mut y = py + 82.0 * s;
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
            let name = if car.role == Role::Player { "Vos" } else { car.name };
            let color = if car.role == Role::Player { accent } else { cream };
            o.text(format!("{}  {name}", ordinal(k + 1)), px + 40.0 * s, y, Font::Bold, color, Align::Left);
            o.text(time, px + pw - 40.0 * s, y, Font::Bold, color, Align::Right);
            y += 30.0 * s;
        }
        let record = if self.new_record {
            "¡Récord nuevo con esta nave!".to_string()
        } else {
            match self.records.get(self.player_kind.record()) {
                Some(ms) => format!("Récord con esta nave: {}", clock(ms as f32 / 1000.0)),
                None => String::new(),
            }
        };
        o.text(record, w * 0.5, py + ph - 64.0 * s, Font::Bold, if self.new_record { accent } else { dim }, Align::Center);
        o.text("Enter: otra carrera   ·   Esc: salir", w * 0.5, py + ph - 32.0 * s, Font::Small, dim, Align::Center);
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
                RoadRow { center: sx0 + (sx1 - sx0) * t, per_meter: cam.focal / d, depth: d, s: s0 + u * SEGMENT, segment: self.track.index(s0 + u * SEGMENT) }
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
        // Los postes, las vallas y la meta.
        let first = ((cam.s / LAMP_EVERY).floor() + 1.0) as i64;
        for k in first..first + (DRAW as f32 * SEGMENT / LAMP_EVERY) as i64 {
            let s = k as f32 * LAMP_EVERY;
            let d = s - cam.s;
            if let Some((x, y, _)) = at(d) {
                let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                items.push(Item { depth: d, kind: ItemKind::Lamp { side }, x: cx + (x + side * (RAIL + 0.7) - cam.x) * cam.focal / d, y: cam.horizon - (y - cam.y) * cam.focal / d, scale: cam.focal / d });
            }
        }
        let first = ((cam.s / BILLBOARD_EVERY).floor() + 1.0) as i64;
        for k in first..first + 5 {
            let s = k as f32 * BILLBOARD_EVERY + 60.0;
            let d = s - cam.s;
            if let Some((x, y, _)) = at(d) {
                let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                items.push(Item { depth: d, kind: ItemKind::Billboard { side, motif: k.rem_euclid(5) as usize }, x: cx + (x + side * (RAIL + 7.0) - cam.x) * cam.focal / d, y: cam.horizon - (y - cam.y) * cam.focal / d, scale: cam.focal / d });
            }
        }
        let length = self.track.length();
        let next_line = ((cam.s / length).floor() + 1.0) * length;
        for s in [next_line - length, next_line] {
            let d = s - cam.s;
            if let Some((x, y, _)) = at(d) {
                let lights = match self.phase {
                    Phase::Ready => 0,
                    Phase::Countdown(t) => 1 + t as u8,
                    _ => 4,
                };
                items.push(Item { depth: d, kind: ItemKind::Gantry { lights }, x: cx + (x - cam.x) * cam.focal / d, y: cam.horizon - (y - cam.y) * cam.focal / d, scale: cam.focal / d });
            }
        }
        // El humo.
        for puff in &self.puffs {
            let d = puff.s - cam.s;
            if let Some((x, _, _)) = at(d) {
                items.push(Item { depth: d, kind: ItemKind::Puff { age: puff.age }, x: cx + (x + puff.x - cam.x) * cam.focal / d, y: cam.horizon - (puff.y - cam.y) * cam.focal / d, scale: cam.focal / d });
            }
        }
        // Las naves.
        for (i, car) in self.cars.iter().enumerate() {
            let spec = car.kind.spec();
            let d = car.s - cam.s;
            let Some((x, y, theta)) = at(d) else { continue };
            let wx = x + car.x - cam.x;
            let sx = cx + wx * cam.focal / d;
            let sy = cam.horizon - (y + spec.hover - cam.y) * cam.focal / d;
            let yaw = theta + car.steer * 0.12 - (wx / d).atan();
            let (set, sprite) = if i == 0 {
                let k = ((car.steer * 0.17 / PLAYER_YAW_STEP).round() as i32 + (PLAYER_YAWS / 2) as i32).clamp(0, PLAYER_YAWS as i32 - 1);
                (usize::MAX, k as usize)
            } else {
                let k = ((yaw / YAW_STEP).round() as i32 + (YAWS / 2) as i32).clamp(0, YAWS as i32 - 1);
                (car.kind.index(), k as usize)
            };
            let glow = if car.boost > 0.0 { 1.0 } else { 0.0 };
            // La apertura de la camara (`focal / (1 + fov)`) encoge todo; a
            // la nave propia se le devuelve la mitad, `1 + fov / 2`, para
            // que no pierda presencia con el turbo.
            let keep = if i == 0 { 1.0 + 0.5 * self.fov.max(0.0) } else { 1.0 };
            items.push(Item { depth: d, kind: ItemKind::Car { set, sprite, width: spec.width, length: spec.length, hover: spec.hover, boost: glow, player: i == 0 }, x: sx, y: sy, scale: cam.focal / d * keep });
        }
        items.sort_by(|a, b| b.depth.total_cmp(&a.depth));

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
        }
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
    /// La pista de atras en esta fila y de que columna a cual va (entre
    /// las orillas de sus barandas).
    far: Option<RoadRow>,
    far_span: (f32, f32),
    /// Cuanto queda del destello del Fuego a esta altura,
    /// `e^(-max(p - cima, 0) / 0.1)` con la elevacion de la columna del
    /// centro (en las orillas cambia poco y es un resplandor suave).
    flash_up: f32,
}

/// Que punto de la pista muestra una fila.
#[derive(Clone, Copy, Debug)]
struct RoadRow {
    /// Donde cae el centro de la pista y cuantos pixeles mide un metro.
    center: f32,
    per_meter: f32,
    depth: f32,
    s: f32,
    segment: usize,
}

#[derive(Clone, Copy, Debug)]
enum ItemKind {
    /// Una nave: su juego de sprites (`usize::MAX` el propio) y cual, y su
    /// largo (m).
    Car { set: usize, sprite: usize, width: f32, length: f32, hover: f32, boost: f32, player: bool },
    Lamp { side: f32 },
    Billboard { side: f32, motif: usize },
    Gantry { lights: u8 },
    Puff { age: f32 },
}

/// Algo encima de la pista, con su punto en el suelo en la pantalla.
#[derive(Clone, Copy, Debug)]
struct Item {
    depth: f32,
    kind: ItemKind,
    x: f32,
    y: f32,
    /// Pixeles por metro a su distancia.
    scale: f32,
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
}

const LAMP_COLOR: Vec3 = Vec3::new(1.0, 0.62, 0.25);
const ASPHALT: Vec3 = Vec3::new(0.05, 0.05, 0.06);
const NEON: [Vec3; 2] = [Vec3::new(1.0, 0.2, 0.6), Vec3::new(0.15, 0.85, 1.0)];

impl View<'_> {
    /// Lo de la fila `y` que no cambia de pixel a pixel.
    fn row_ctx<'s>(&self, y: usize, sparks: &'s [Spark], span: (f32, f32)) -> RowCtx<'s> {
        let yoff = self.cam.horizon - (y as f32 + 0.5);
        let fog = |r: Option<RoadRow>| r.map_or(0.0, |r| fog_at(r.depth).max(horizon_fog(-yoff, r.depth)));
        let summit = FUEGO.height - self.limb;
        let pitch = (yoff / self.cam.focal).atan();
        let far = self.far[y];
        let far_span = far.map_or((f32::MAX, f32::MIN), |f| (f.center - (RAIL + 0.35) * f.per_meter, f.center + (RAIL + 0.35) * f.per_meter));
        RowCtx { y, yoff, sparks, span, fog: fog(self.rows[y]), far_fog: fog(far), far, far_span, flash_up: (-(pitch - summit).max(0.0) / 0.1).exp() }
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
        let road = self.rows[y];
        for x in 0..game.width {
            let (c, e) = match road {
                Some(r) => self.road(r, x as f32 + 0.5, &ctx, x),
                None => self.background(x, &ctx),
            };
            write_emit(&mut row[x * 4..x * 4 + 4], c, e);
        }
        for item in &self.items {
            self.draw_item(item, y, yf, row);
        }
    }

    /// Lo que el HUD de CPU necesita despues del posproceso (dueno de sus
    /// datos: la vista se consume aqui, ya no existe cuando se pinta).
    fn into_hud(self) -> HudFrame {
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
    /// Va siempre en linea dentro del ciclo de la fila: asi lo que solo
    /// depende de la fila de pista (`1 / px`, el tramo, las rayas, el pulso
    /// de la baranda) se saca del ciclo. La pista de atras va aparte, en
    /// una funcion que no se mete en linea (son pocos pixeles). Da el color
    /// y cuanto emite.
    #[inline(always)]
    fn road(&self, r: RoadRow, x: f32, ctx: &RowCtx, column: usize) -> (Vec3, f32) {
        let u = (x - r.center) / r.per_meter;
        let a = u.abs();
        // Afuera de las barandas: la pista de atras si asoma ahi, o el
        // fondo, con un halo corto del neon (el ancho lo pone el brillo
        // del posproceso; este no emite).
        if a > RAIL + 0.35 {
            let (back, e) = if x > ctx.far_span.0 && x < ctx.far_span.1 { self.far_road(x, ctx) } else { self.background(column, ctx) };
            let side = (u > 0.0) as usize;
            let glow = NEON[side] * (0.45 * halo((a - RAIL - 0.35) * r.per_meter, 6.0 + 0.5 * r.per_meter));
            return (back + glow * (1.0 - ctx.fog), e);
        }
        self.surface(r, u, ctx.fog, ctx)
    }

    /// La pista de atras que asoma al lado de la de adelante.
    #[inline(never)]
    fn far_road(&self, x: f32, ctx: &RowCtx) -> (Vec3, f32) {
        match ctx.far {
            Some(f) => self.surface(f, (x - f.center) / f.per_meter, ctx.far_fog, ctx),
            None => (Vec3::ZERO, 0.0),
        }
    }

    /// La pista misma entre las barandas, a `u` metros del centro, ya con
    /// su neblina `fog`, y cuanto emite (solo la baranda de neon,
    /// `0.9 pulso`, que se apaga en la neblina).
    #[inline(always)]
    fn surface(&self, r: RoadRow, u: f32, fog: f32, ctx: &RowCtx) -> (Vec3, f32) {
        let game = self.game;
        let a = u.abs();
        let px = 1.0 / r.per_meter;
        let segment = &game.track.segments[r.segment];
        let mut c;
        let mut e = 0.0;
        if a > RAIL {
            // La baranda de luz: un tubo de neon con su color de pantalla
            // (no el tono de `2.2 NEON`, que se recortaba casi a blanco),
            // mas oscuro en las orillas del tubo, `0.55 + 0.45 (1 - t^2)`
            // con `t` de -1 a 1 a lo ancho, y una linea fina casi blanca al
            // centro (la parte mas caliente del gas). De lejos, cuando el
            // tubo mide pocos pixeles, la linea se apaga y queda el color.
            let side = (u > 0.0) as usize;
            let pulse = 0.75 + 0.25 * (r.s * 0.15 - game.wall * 6.0).sin();
            let t = (a - RAIL) * (2.0 / 0.35) - 1.0;
            c = NEON[side] * ((0.55 + 0.45 * (1.0 - t * t)) * (0.8 + 0.2 * pulse));
            let tube_px = 0.35 * r.per_meter;
            if tube_px > 1.5 {
                let core = cover(t.abs() * 0.5 * tube_px - (0.06 * tube_px).max(0.4)) * smoothstep(1.5, 6.0, tube_px) * (0.5 + 0.4 * pulse);
                c = c.lerp(NEON[side] * 0.25 + Vec3::splat(0.75), core);
            }
            e = 0.9 * pulse;
        } else if a > HALF_ROAD + 0.7 {
            // El hombro de concreto, tenido por el neon de su lado: un
            // halo corto pintado aqui, `0.35 e^(-d / ancho)` a `d` pixeles
            // de la baranda (el brillo del posproceso pone la cola ancha).
            let side = (u > 0.0) as usize;
            let d = (RAIL - a) * r.per_meter;
            c = Vec3::new(0.09, 0.09, 0.1) + NEON[side] * (0.35 * halo(d, 2.0 + 0.25 * r.per_meter));
        } else if a > HALF_ROAD {
            // El bordillo rojo y blanco.
            let stripe = ((r.s / (SEGMENT * 2.0)).floor() as i64).rem_euclid(2) == 0;
            c = if stripe { Vec3::new(0.55, 0.05, 0.06) } else { Vec3::new(0.7, 0.7, 0.72) };
        } else {
            // El asfalto: manchas suaves y un grano fino que se apagan de
            // lejos (ahi se veria solo ruido).
            let resolved = smoothstep(0.6, 0.15, px);
            let patches = value_noise(u * 0.35, r.s * 0.12);
            let grain = if px < 0.08 { value_noise(u * 6.0 + 17.0, r.s * 6.0) } else { 0.0 };
            c = ASPHALT * (1.0 + (0.25 * patches + 0.18 * grain * smoothstep(0.08, 0.02, px)) * resolved);
            // Las rayas: discontinuas entre carriles y amarillas en la orilla.
            // Una raya mas angosta que un pixel no cubre mas que su ancho en
            // pixeles: `min(cover, ancho * px por metro)`. Asi de lejos se
            // apagan en vez de salir como puntitos blancos.
            let dash = (r.s / 12.0).rem_euclid(1.0) < 0.5;
            for lane in [-LANE, 0.0, LANE] {
                let d = ((u - lane).abs() - 0.12) * r.per_meter;
                if dash {
                    c = over(c, Vec3::new(0.75, 0.75, 0.72), cover(d).min(0.24 * r.per_meter));
                }
            }
            let edge = ((a - (HALF_ROAD - 0.35)).abs() - 0.1) * r.per_meter;
            c = over(c, Vec3::new(0.8, 0.6, 0.08), cover(edge).min(0.2 * r.per_meter));
            // Los tumulos: franjas amarillas y negras en sus carriles.
            if segment.bumps != 0 {
                let lane = (((u + HALF_ROAD) / LANE).floor() as i32).clamp(0, 3);
                let within = (r.s / SEGMENT).rem_euclid(1.0);
                if segment.bumps & (1 << lane) != 0 && (0.3..0.7).contains(&within) {
                    let stripe = ((u + r.s) * 1.5).rem_euclid(1.0) < 0.5;
                    c = if stripe { Vec3::new(0.85, 0.65, 0.05) } else { Vec3::new(0.03, 0.03, 0.03) };
                }
            }
            // La meta: cuadros blancos y negros.
            let lap_s = r.s.rem_euclid(game.track.length());
            if lap_s < 2.5 {
                let check = ((u.floor() as i64) + ((lap_s * 1.0).floor() as i64)).rem_euclid(2) == 0;
                c = if check { Vec3::new(0.8, 0.8, 0.8) } else { Vec3::new(0.02, 0.02, 0.02) };
            }
        }
        // La luz de los postes en charcos naranja.
        let lamp = (r.s / LAMP_EVERY).round();
        let side = if (lamp as i64).rem_euclid(2) == 0 { 1.0 } else { -1.0 };
        let ds = r.s - lamp * LAMP_EVERY;
        let dx = u - side * (HALF_ROAD - 1.0);
        let pool = (-(ds * ds) / 90.0 - (dx * dx) / 45.0).exp();
        c = c + LAMP_COLOR * (0.18 * pool) + c * (pool * 1.5);
        // La neblina toma el color del cielo de atras: la pista lejana se
        // funde en el horizonte.
        (c.lerp(self.haze_row[ctx.y], fog), e * (1.0 - fog))
    }

    /// Lo que cae en la fila `y` de un objeto encima de la pista. No se ve
    /// donde la pista que muestra la fila esta mas cerca (una loma lo tapa).
    fn draw_item(&self, item: &Item, y: usize, yf: f32, row: &mut [u8]) {
        let game = self.game;
        let road = self.rows[y];
        let far = self.far[y];
        // Lo tapa la pista que esta mas cerca que su punta de adelante (una
        // loma). Para una nave es la cola, medio largo mas cerca que su
        // centro: el sprite visto desde arriba baja en la pantalla hasta
        // la cola, y si se midiera desde el centro la pista de enfrente le
        // cortaria las llantas de atras en recto (la Mamaneta mide 9.6 m).
        let front = item.depth - 1.0 - if let ItemKind::Car { length, .. } = item.kind { 0.5 * length } else { 0.0 };
        // Lo que tapa cada pista en esta fila es un tramo de `x` (entre
        // sus barandas, `|x - centro| < (RAIL + 0.4) px por metro`), que
        // se saca una vez por fila y no por pixel.
        let reach = |r: Option<RoadRow>| match r {
            Some(r) if r.depth < front => (r.center - (RAIL + 0.4) * r.per_meter, r.center + (RAIL + 0.4) * r.per_meter),
            _ => (f32::MAX, f32::MIN),
        };
        let (near_road, far_road) = (reach(road), reach(far));
        let hidden = |x: f32| (x > near_road.0 && x < near_road.1) || (x > far_road.0 && x < far_road.1);
        let fog = fog_at(item.depth).max(horizon_fog(item.y - self.cam.horizon, item.depth));
        let haze = self.haze_row[y];
        let k = item.scale;
        // Coordenadas en metros alrededor del punto en el suelo (y hacia arriba).
        let my = (item.y - yf) / k;
        match item.kind {
            ItemKind::Car { set, sprite, width, hover, boost, player, .. } => {
                let sprite = if set == usize::MAX { &game.player_set[sprite] } else { &game.sets[set][sprite] };
                let scale = k / sprite.scale;
                // Las que pasan pegadas a la camara se desvanecen: de tan
                // cerca el sprite se estiraria y taparia la pantalla.
                let near = if player { 1.0 } else { smoothstep(2.5, 5.5, item.depth) };
                if near <= 0.0 {
                    return;
                }
                let top = item.y - sprite.anchor.1 * scale;
                let bottom = item.y + (sprite.height as f32 - sprite.anchor.1) * scale;
                // La sombra (o el brillo de la chela que flota) en el suelo.
                let ground = item.y + hover * k;
                let shadow_r = (width * 0.65 * k, width * 0.18 * k + 2.0);
                if (yf - ground).abs() < shadow_r.1 && hover == 0.0 {
                    let x0 = (item.x - shadow_r.0).max(0.0) as usize;
                    let x1 = ((item.x + shadow_r.0) as usize).min(game.width);
                    for x in x0..x1 {
                        let e = ((x as f32 + 0.5 - item.x) / shadow_r.0).hypot((yf - ground) / shadow_r.1);
                        if e < 1.0 && !hidden(x as f32) {
                            let p = &mut row[x * 4..x * 4 + 4];
                            let dim = 1.0 - 0.45 * near * (1.0 - e * e).powi(2);
                            write_emit(p, lienzo::read(p) * dim, emission(p) * dim);
                        }
                    }
                }
                if hover > 0.0 && (yf - ground).abs() < shadow_r.1 * 2.0 {
                    let x0 = (item.x - shadow_r.0 * 1.3).max(0.0) as usize;
                    let x1 = ((item.x + shadow_r.0 * 1.3) as usize).min(game.width);
                    for x in x0..x1 {
                        let e = ((x as f32 + 0.5 - item.x) / (shadow_r.0 * 1.3)).hypot((yf - ground) / (shadow_r.1 * 2.0));
                        if e < 1.0 && !hidden(x as f32) {
                            let p = &mut row[x * 4..x * 4 + 4];
                            let light = 0.35 * (1.0 - e);
                            write_emit(p, lienzo::read(p) + Vec3::new(0.9, 0.7, 0.3) * light, emission(p).max(light));
                        }
                    }
                }
                if yf < top || yf >= bottom {
                    return;
                }
                let sy = (yf - item.y) / scale + sprite.anchor.1;
                // Solo las columnas donde la fila del sprite tiene algo (si
                // no tiene nada, ninguna; el fuego del turbo va aparte).
                let (lo, hi) = sprite.span(sy);
                let x0 = (item.x + (lo - sprite.anchor.0) * scale).floor().max(0.0) as usize;
                let x1 = if hi > lo { ((item.x + (hi - sprite.anchor.0) * scale).ceil().max(0.0) as usize + 1).min(game.width) } else { 0 };
                for x in x0..x1 {
                    let xf = x as f32 + 0.5;
                    if hidden(xf) {
                        continue;
                    }
                    let sx = (xf - item.x) / scale + sprite.anchor.0;
                    let [r, g, b, a] = sprite.sample(sx, sy);
                    if a <= 0.0 && r + g + b <= 0.0 {
                        continue;
                    }
                    // Emiten solo las LEDs, con la emision que guardo el
                    // horneado de las piezas con clase LED (no la laca,
                    // el vidrio crema ni las letras claras, aunque esten
                    // casi blancas). A 0.8: la camioneta lleva decenas de
                    // focos juntos y a toda fuerza velaria su propia laca.
                    let e_src = 0.8 * near * sprite.emit_at(sx, sy) * (1.0 - fog);
                    let (r, g, b, a) = (r * near, g * near, b * near, a * near);
                    let p = &mut row[x * 4..x * 4 + 4];
                    let back = lienzo::read(p);
                    let mut src = Vec3::new(r, g, b);
                    // De lejos se pierde en la neblina.
                    src = src.lerp(haze * a, fog);
                    write_emit(p, src + back * (1.0 - a), e_src + emission(p) * (1.0 - a));
                }
                // El fuego del turbo atras de la propia.
                if player && boost > 0.0 {
                    let flame = (item.x, item.y - 0.5 * k);
                    let reach = 1.6 * k;
                    if (yf - flame.1).abs() < reach {
                        let x0 = (flame.0 - reach).max(0.0) as usize;
                        let x1 = ((flame.0 + reach) as usize).min(game.width);
                        for x in x0..x1 {
                            let d = ((x as f32 + 0.5 - flame.0) / reach).hypot((yf - flame.1) / (reach * 0.5));
                            if d < 1.0 {
                                let flicker = 0.8 + 0.2 * (game.wall * 40.0 + x as f32 * 0.3).sin();
                                let p = &mut row[x * 4..x * 4 + 4];
                                write_emit(p, lienzo::read(p) + Vec3::new(0.3, 0.6, 1.0) * ((1.0 - d) * flicker), emission(p).max(0.2 * (1.0 - d) * (1.0 - d)));
                            }
                        }
                    }
                }
            }
            ItemKind::Lamp { side } => {
                // Poste de 9 m con su brazo hacia la pista y la lampara. En
                // cada fila solo se recorre lo que tiene algo: el poste y,
                // cerca de la lampara, el brazo y su resplandor.
                if !(-0.3..11.5).contains(&my) {
                    return;
                }
                let spread = 0.8f32.max(3.0 / k);
                let near_head = ((my - 8.5) * 1.5).abs() < 2.6 * spread || (8.4..9.1).contains(&my);
                // Lo que se recorre, en metros hacia la pista.
                let (from, to) = if near_head { (-0.3, 2.7 + 2.6 * spread) } else if my <= 9.0 { (-0.3, 0.3) } else { return };
                // De metros hacia la pista a pixeles de la pantalla.
                let (a, b) = (item.x - side * from * k, item.x - side * to * k);
                let x0 = (a.min(b) - 1.0).max(0.0) as usize;
                let x1 = ((a.max(b) + 1.0).max(0.0) as usize).min(game.width);
                let px = 1.0 / k;
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
                    let p = &mut row[x * 4..x * 4 + 4];
                    let (mut c, mut e) = (lienzo::read(p), emission(p));
                    let steel = cover(pole.min(arm));
                    c = over(c, Vec3::new(0.06, 0.06, 0.07).lerp(haze, fog), steel);
                    // La lampara de sodio: su tono, `(1, 0.72, 0.38)`, sin
                    // recortarse (antes se le sumaba el resplandor encima
                    // y salia crema), con el centro apenas mas claro.
                    let bulb = cover(head);
                    let hot = smoothstep(0.5, 0.0, (mx - 2.7).abs() / 0.5);
                    c = over(c, Vec3::new(1.0, 0.72, 0.38).lerp(Vec3::new(1.0, 0.82, 0.55), hot), bulb);
                    // La lampara emite toda; lo que tapa el poste no.
                    e = (e * (1.0 - steel) * (1.0 - bulb)).max(bulb * (1.0 - fog));
                    // El resplandor: gaussiano en metros (y nunca menos de
                    // unos pixeles); a 2.6 anchos ya no se ve. Solo afuera
                    // de la lampara.
                    let dm = (mx - 2.7).hypot((my - 8.5) * 1.5) / spread;
                    let glow = (-dm * dm).exp() * (1.0 - fog) * (1.0 - bulb);
                    c = c + LAMP_COLOR * (0.55 * glow);
                    write_emit(p, c, e.max(0.5 * glow));
                }
            }
            ItemKind::Billboard { side, motif } => {
                // Valla de 10 x 5 m sobre dos patas de 3 m, con motivos de la
                // faja tejida.
                if !(-0.2..8.5).contains(&my) {
                    return;
                }
                let x0 = (item.x - 6.0 * k).max(0.0) as usize;
                let x1 = ((item.x + 6.0 * k) as usize + 1).min(game.width);
                let px = 1.0 / k;
                for x in x0..x1 {
                    let xf = x as f32 + 0.5;
                    if hidden(xf) {
                        continue;
                    }
                    let mx = (xf - item.x) / k * side;
                    let p = &mut row[x * 4..x * 4 + 4];
                    let (mut c, mut e) = (lienzo::read(p), emission(p));
                    let legs = ((mx.abs() - 3.0).abs() - 0.15).max(my - 3.2) / px;
                    c = over(c, Vec3::new(0.08, 0.08, 0.09).lerp(haze, fog), cover(legs));
                    e *= 1.0 - cover(legs);
                    let panel = (mx.abs() - 5.0).max((my - 5.5).abs() - 2.5) / px;
                    if panel < 1.0 {
                        let border = (mx.abs() - 4.7).max((my - 5.5).abs() - 2.2) / px;
                        let mut face = Vec3::new(0.95, 0.75, 0.15);
                        if border < 0.0 {
                            // Dos cuadros con su motivo.
                            face = Vec3::new(0.6, 0.04, 0.07);
                            let cell = if mx < 0.0 { motif } else { motif + 2 };
                            let (cx, cy) = (((mx + 4.7) / 4.7).fract() - 0.5, (my - 5.5) / 4.4);
                            transition::motif(cell, &mut face, cx, cy, (px / 4.4).max(0.004));
                        }
                        // Los reflectores de abajo la alumbran.
                        let lit = 0.75 + 0.5 * smoothstep(8.0, 3.0, my);
                        c = over(c, (face * lit).lerp(haze, fog), cover(panel));
                        e *= 1.0 - cover(panel);
                    }
                    write_emit(p, c, e);
                }
            }
            ItemKind::Gantry { lights } => {
                // La meta: dos postes, una viga a cuadros a 8 m y, colgado al
                // centro, el semaforo.
                if !(-0.2..11.0).contains(&my) {
                    return;
                }
                let span = RAIL + 0.6;
                let x0 = (item.x - (span + 0.6) * k).max(0.0) as usize;
                let x1 = ((item.x + (span + 0.6) * k) as usize + 1).min(game.width);
                let px = 1.0 / k;
                for x in x0..x1 {
                    let xf = x as f32 + 0.5;
                    if hidden(xf) {
                        continue;
                    }
                    let mx = (xf - item.x) / k;
                    let p = &mut row[x * 4..x * 4 + 4];
                    let (mut c, mut e) = (lienzo::read(p), emission(p));
                    let posts = ((mx.abs() - span).abs() - 0.3).max(my - 9.0) / px;
                    c = over(c, Vec3::new(0.12, 0.12, 0.14).lerp(haze, fog), cover(posts));
                    e *= 1.0 - cover(posts);
                    let beam = (mx.abs() - span).max((my - 8.4).abs() - 0.6) / px;
                    if beam < 1.0 {
                        let check = ((mx.floor() as i64) + ((my - 7.8) / 0.6).floor() as i64).rem_euclid(2) == 0;
                        let face = if check { Vec3::new(0.85, 0.85, 0.85) } else { Vec3::new(0.03, 0.03, 0.03) };
                        c = over(c, face.lerp(haze, fog), cover(beam));
                        e *= 1.0 - cover(beam);
                    }
                    if lights > 0 {
                        let housing = (mx.abs() - 0.75).max((my - 6.3).abs() - 1.5) / px;
                        c = over(c, Vec3::new(0.02, 0.02, 0.025), cover(housing));
                        e *= 1.0 - cover(housing);
                        for (k_light, (ly, on_color)) in [(7.2, Vec3::new(1.0, 0.1, 0.08)), (6.3, Vec3::new(1.0, 0.1, 0.08)), (5.4, Vec3::new(1.0, 0.1, 0.08))].into_iter().enumerate() {
                            let green = lights >= 4;
                            let on = green || lights as usize > k_light;
                            let color = if green { Vec3::new(0.2, 1.0, 0.35) } else { on_color };
                            let d = (mx.hypot(my - ly) - 0.36) / px;
                            let bulb = if on { color * 1.2 } else { color * 0.12 };
                            c = over(c, bulb, cover(d));
                            e = if on { (e * (1.0 - cover(d))).max(cover(d)) } else { e * (1.0 - cover(d)) };
                            if on {
                                c = c + color * (0.5 * halo(d, 8.0 + 0.6 * k));
                            }
                        }
                    }
                    write_emit(p, c, e);
                }
            }
            ItemKind::Puff { age } => {
                let radius = (1.0 + 1.6 * age) * k;
                if (yf - item.y).abs() > radius {
                    return;
                }
                let opacity = 0.6 * smoothstep(2.6, 0.4, age) * smoothstep(0.0, 0.15, age);
                let x0 = (item.x - radius).max(0.0) as usize;
                let x1 = ((item.x + radius) as usize + 1).min(game.width);
                for x in x0..x1 {
                    let xf = x as f32 + 0.5;
                    if hidden(xf) {
                        continue;
                    }
                    let d = ((xf - item.x) / radius).hypot((yf - item.y) / radius);
                    if d >= 1.0 {
                        continue;
                    }
                    // El ruido horneado del posproceso (no un Perlin por
                    // pixel): un cuadro de la textura cada dos radios, que
                    // sube con la edad. El borde sale de un umbral sobre el
                    // ruido, `v = n + 0.5 (1 - d^2) - 0.45` y
                    // `alfa = smoothstep(0, 0.3, v)`: al centro casi todo
                    // pasa y hacia la orilla solo lo que tiene mas ruido,
                    // asi la bocanada se rompe en volutas y no es una bola.
                    // Adentro, el mismo ruido le da cuerpo, `0.5 + 0.5 n`.
                    let n = game.post.tex(xf / radius * 0.5, yf / radius * 0.5 + age * 0.3);
                    let v = n + 0.5 * (1.0 - d * d) - 0.45;
                    let a = opacity * smoothstep(0.0, 0.3, v) * (0.5 + 0.5 * n);
                    let p = &mut row[x * 4..x * 4 + 4];
                    write_emit(p, over(lienzo::read(p), Vec3::new(0.05, 0.045, 0.04), a), emission(p) * (1.0 - a));
                }
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

/// Ruido de valor en 2D, de -1 a 1: un numero al azar en cada esquina de la
/// cuadricula, interpolado suave. Mucho mas barato que el Perlin, y para el
/// asfalto alcanza.
#[inline]
fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i32 as u32, yi as i32 as u32);
    let corner = |dx: u32, dy: u32| (hash_u32(xi.wrapping_add(dx).wrapping_mul(0x9E37_79B9) ^ yi.wrapping_add(dy).wrapping_mul(0x85EB_CA6B)) >> 8) as f32 / 8_388_608.0 - 1.0;
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
fn tone(c: Vec3) -> Vec3 {
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
}

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
        Self { width, height, texels }
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
        let models = std::array::from_fn(|k| Model { mesh: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        Carrera::new(models, player, &sky, 320, 180, Records::in_memory())
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
        let models = std::array::from_fn(|k| Model { mesh: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let mut g = Carrera::new(models, Kind::Camioneta, &sky, w, h, Records::in_memory());
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
        let models = std::array::from_fn(|k| Model { mesh: &mesh, finish: Finish::Vehicle, meters: Kind::ALL[k].spec().length / 4.0, name: names[k] });
        let sky = |d: Vec3| Vec3::new(0.02, 0.03, 0.06) * (1.0 + d.y.max(0.0));
        let (w, h) = (1280, 720);
        let mut g = Carrera::new(models, Kind::Colectivo, &sky, w, h, Records::in_memory());
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
            g.puffs.push(Puff { s: at, x: x + 0.8, y: g.track.height(at) + 2.4, age: 0.1 * k as f32 });
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

    /// Lo opaco que se pinta encima (una valla) borra la emision de lo que
    /// tapa: la baranda o el cielo que florecian atras ya no florecen.
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
        // La valla a 10 px por metro con el pie 50 px abajo: la fila cae
        // a 5 m de alto, en medio del panel (de 3 a 8 m, 10 m de ancho).
        let item = Item { depth: 30.0, kind: ItemKind::Billboard { side: 1.0, motif: 0 }, x: 160.0, y: y as f32 + 0.5 + 50.0, scale: 10.0 };
        view.draw_item(&item, y, y as f32 + 0.5, &mut row);
        for x in 120..200 {
            assert_eq!(emission(&row[x * 4..x * 4 + 4]), 0.0, "x = {x}");
        }
        // Afuera de la valla sigue emitiendo.
        assert_eq!(emission(&row[10 * 4..10 * 4 + 4]), 1.0);
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
}
