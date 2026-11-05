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
//! - **El cielo** es el de la version chapina: el cubemap se pasa una vez a
//!   un panorama (rumbo y altura), y cada pixel lo lee con el rumbo de la
//!   camara, asi gira de verdad en las curvas. Abajo, la capital de noche:
//!   una esfera con barrancos oscuros entre las luces de sodio.
//! - **La fisica** es de arcade a 120 pasos por segundo: aceleracion que se
//!   apaga cerca de la velocidad maxima, la curva empuja hacia afuera con
//!   `v^2 kappa`, el volante con el manejo de cada nave, choques entre
//!   rectangulos con masa, y fuera del asfalto se frena.
//! - **La IA** busca el lado de adentro de las curvas, frena lo justo para
//!   no pasar de `sqrt(agarre / kappa)`, esquiva el trafico y los tumulos, y
//!   gasta el turbo en las rectas. Un poco de liga: se apura si va lejos
//!   detras de quien juega y afloja si va muy adelante.

use std::f32::consts::{PI, TAU};

use super::hornear::{self, Finish, Sprite};
use super::lienzo::{self, cover, halo, over};
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::math::{Perlin, Vec3, hash_u32, smoothstep};
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
        Self { segments, headings, map }
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
    minimap: Vec<[f32; 4]>,
    map_size: usize,
    noise: Perlin,
    puffs: Vec<Puff>,
    width: usize,
    height: usize,
    /// Sacudida de la camara y humo en la pantalla.
    shake: f32,
    smoke_screen: f32,
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
            minimap,
            map_size,
            noise: Perlin::new(0xCA77_E7A),
            puffs: Vec::new(),
            width,
            height,
            shake: 0.0,
            smoke_screen: 0.0,
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
        self.shake = (self.shake - dt * 2.5).max(0.0);
        self.smoke_screen = (self.smoke_screen - dt * 1.1).max(0.0);
        self.horn_cooldown = (self.horn_cooldown - dt).max(0.0);
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

    fn draw(&mut self, rgba: &mut [u8]) {
        let view = self.view();
        let (w, _) = (self.width, self.height);
        parallel::chunks_mut(rgba, w * 4, 1, |y, row| view.row(y, row));
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
        let boost = if player.boost > 0.0 { 0.9 } else { 1.0 };
        Camera {
            s,
            x: player.x * 0.75,
            y: self.track.height(s).max(self.track.height(player.s) - 1.0) + up + shake,
            focal: FOCAL * self.height as f32 * boost,
            horizon: self.height as f32 * HORIZON + shake * 4.0,
            heading: self.track.heading(s),
        }
    }

    /// Todo lo que se dibuja en este frame: las filas de la pista y lo que
    /// va encima, ordenado de lejos a cerca.
    fn view(&self) -> View<'_> {
        let cam = self.camera();
        let (w, h) = (self.width, self.height);
        let cx = w as f32 * 0.5;
        let mut rows: Vec<Option<RoadRow>> = vec![None; h];
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
            if sy1 >= sy0 {
                continue;
            }
            let from = sy1.max(0.0).ceil() as i32;
            let to = sy0.min(top).ceil() as i32;
            for y in from..to.min(h as i32) {
                let yf = y as f32 + 0.5;
                let t = ((sy0 - yf) / (sy0 - sy1)).clamp(0.0, 1.0);
                // En perspectiva se interpola 1 / z.
                let inv = (1.0 / d0) * (1.0 - t) + (1.0 / d1) * t;
                let d = 1.0 / inv;
                let u = (d - d0) / (d1 - d0);
                rows[y as usize] = Some(RoadRow {
                    center: sx0 + (sx1 - sx0) * t,
                    per_meter: cam.focal / d,
                    depth: d,
                    s: s0 + u * SEGMENT,
                    segment: self.track.index(s0 + u * SEGMENT),
                });
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
            items.push(Item { depth: d, kind: ItemKind::Car { set, sprite, width: spec.width, hover: spec.hover, boost: glow, player: i == 0 }, x: sx, y: sy, scale: cam.focal / d });
        }
        items.sort_by(|a, b| b.depth.total_cmp(&a.depth));

        // Los puntos del mapa.
        let dots = self
            .cars
            .iter()
            .filter(|c| c.role != Role::Traffic)
            .map(|c| {
                let i = self.track.index(c.s);
                let (mx, my) = self.track.map[i];
                (mx, my, c.kind.color(), c.role == Role::Player)
            })
            .collect();
        View {
            game: self,
            cam,
            rows,
            items,
            dots,
            column_yaw: (0..w).map(|x| ((x as f32 + 0.5 - cx) / cam.focal).atan()).collect(),
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
    /// Una nave: su juego de sprites (`usize::MAX` el propio) y cual.
    Car { set: usize, sprite: usize, width: f32, hover: f32, boost: f32, player: bool },
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
    items: Vec<Item>,
    dots: Vec<(f32, f32, Vec3, bool)>,
    column_yaw: Vec<f32>,
}

const LAMP_COLOR: Vec3 = Vec3::new(1.0, 0.62, 0.25);
const FOG: Vec3 = Vec3::new(0.035, 0.03, 0.06);
const ASPHALT: Vec3 = Vec3::new(0.05, 0.05, 0.06);
const NEON: [Vec3; 2] = [Vec3::new(1.0, 0.2, 0.6), Vec3::new(0.15, 0.85, 1.0)];

impl View<'_> {
    fn row(&self, y: usize, row: &mut [u8]) {
        let game = self.game;
        let yf = y as f32 + 0.5;
        let pitch = ((self.cam.horizon - yf) / self.cam.focal).atan();
        let road = self.rows[y];
        for x in 0..game.width {
            let mut c = match road {
                Some(r) => self.road(r, x as f32 + 0.5, pitch, x),
                None => self.background(x, pitch),
            };
            c = self.minimap(x, y, c);
            lienzo::write(&mut row[x * 4..x * 4 + 4], c);
        }
        for item in &self.items {
            self.draw_item(item, y, yf, row);
        }
        // El humo en la pantalla y la vineta del turbo.
        let smoke = game.smoke_screen;
        let boost = if game.player().boost > 0.0 { 1.0 } else { 0.0 };
        if smoke > 0.0 || boost > 0.0 {
            let (cx, cy) = (game.width as f32 * 0.5, game.height as f32 * 0.5);
            for x in 0..game.width {
                let p = &mut row[x * 4..x * 4 + 4];
                let mut c = lienzo::read(p);
                let r = ((x as f32 - cx) / cx).hypot((yf - cy) / cy);
                if smoke > 0.0 {
                    let n = game.noise.noise(Vec3::new(x as f32 * 0.01, yf * 0.01, game.wall * 0.7)) * 0.5 + 0.5;
                    c = over(c, Vec3::new(0.05, 0.045, 0.04), smoke * (0.55 + 0.35 * n));
                }
                if boost > 0.0 {
                    c = c + Vec3::new(0.1, 0.35, 0.6) * (smoothstep(0.7, 1.3, r) * 0.6);
                }
                lienzo::write(p, c);
            }
        }
    }

    /// El cielo del panorama y, abajo, la capital de noche.
    fn background(&self, x: usize, pitch: f32) -> Vec3 {
        let yaw = self.cam.heading + self.column_yaw[x];
        let planet = self.planet(yaw, pitch);
        planet.unwrap_or_else(|| self.game.panorama.sample(yaw, pitch))
    }

    /// La capital: una esfera grande abajo a la derecha (la pista le da la
    /// vuelta), de noche: barrancos oscuros entre las luces de sodio, unas
    /// nubes y el borde de la atmosfera.
    fn planet(&self, yaw: f32, pitch: f32) -> Option<Vec3> {
        let game = self.game;
        let progress = (game.player().s / game.track.length()).rem_euclid(1.0);
        let center_yaw = progress * TAU + 1.25;
        let center_pitch = -0.62f32;
        let radius = 0.68f32;
        let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
        let c = Vec3::new(center_yaw.sin() * center_pitch.cos(), center_pitch.sin(), -center_yaw.cos() * center_pitch.cos());
        let cos = dir.dot(c);
        let edge = radius.cos();
        if cos < edge - 0.02 {
            return None;
        }
        // El rayo contra la esfera de radio sin(radio) a distancia 1.
        let r = radius.sin();
        let b = cos;
        let disc = b * b - (1.0 - r * r);
        let rim = smoothstep(edge - 0.02, edge + 0.004, cos);
        let space = game.panorama.sample(yaw, pitch);
        if disc <= 0.0 {
            // Solo el halo de la atmosfera afuera del borde.
            return Some(space + Vec3::new(0.35, 0.22, 0.12) * (rim * 0.5));
        }
        let t = b - disc.sqrt();
        let n = (dir * t - c) * (1.0 / r);
        // Gira despacio bajo la pista.
        let spin = game.wall * 0.004 + progress * 0.6;
        let (ss, cs) = spin.sin_cos();
        let p = Vec3::new(n.x * cs - n.z * ss, n.y, n.x * ss + n.z * cs);
        let noise = &game.noise;
        let land = noise.fbm(p * 3.0, 3) * 0.5 + 0.5;
        let ravine = (noise.ridged(p * 5.0 + Vec3::new(3.1, 0.0, 1.7), 3)).clamp(0.0, 1.0);
        let city = smoothstep(0.45, 0.6, land) * (1.0 - smoothstep(0.75, 0.92, ravine));
        let sparkle = noise.noise(p * 90.0) * 0.5 + 0.5;
        let lights = Vec3::new(1.0, 0.5, 0.15) * (city * sparkle.powi(4) * 0.8);
        let ground = Vec3::new(0.004, 0.004, 0.007) * (0.5 + land);
        let clouds = smoothstep(0.6, 0.85, noise.fbm(p * 2.2 + Vec3::new(9.0, 2.0, 5.0), 4) * 0.5 + 0.5);
        let limb = smoothstep(edge + 0.05, edge, cos);
        let mut color = ground + lights;
        // Nubes oscuras que tapan las luces y que ellas alumbran desde abajo.
        color = color.lerp(Vec3::new(0.02, 0.015, 0.02) + lights * 0.15, clouds * 0.8);
        color = color + Vec3::new(0.25, 0.14, 0.06) * (limb * 0.3) + Vec3::new(0.05, 0.1, 0.25) * (limb * limb * 0.3);
        Some(space.lerp(tone(color), rim))
    }

    /// Un pixel de la fila de la pista (o lo que se ve a los lados).
    fn road(&self, r: RoadRow, x: f32, pitch: f32, column: usize) -> Vec3 {
        let game = self.game;
        let u = (x - r.center) / r.per_meter;
        let a = u.abs();
        let px = 1.0 / r.per_meter;
        let fog = smoothstep(DRAW as f32 * SEGMENT * 0.45, DRAW as f32 * SEGMENT * 0.95, r.depth);
        let segment = &game.track.segments[r.segment];
        // Afuera de las barandas: el cielo, con el brillo del neon.
        if a > RAIL + 0.35 {
            let back = self.background(column, pitch);
            let side = (u > 0.0) as usize;
            let glow = NEON[side] * (0.5 * halo((a - RAIL - 0.35) * r.per_meter, 6.0 + 0.5 * r.per_meter));
            return back + glow * (1.0 - fog);
        }
        let mut c;
        if a > RAIL {
            // La baranda de luz.
            let side = (u > 0.0) as usize;
            let pulse = 0.75 + 0.25 * (r.s * 0.15 - game.wall * 6.0).sin();
            c = tone(NEON[side] * (2.2 * pulse));
        } else if a > HALF_ROAD + 0.7 {
            // El hombro de concreto.
            c = Vec3::new(0.09, 0.09, 0.1);
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
            let dash = (r.s / 12.0).rem_euclid(1.0) < 0.5;
            for lane in [-LANE, 0.0, LANE] {
                let d = ((u - lane).abs() - 0.12) * r.per_meter;
                if dash {
                    c = over(c, Vec3::new(0.75, 0.75, 0.72), cover(d));
                }
            }
            let edge = ((a - (HALF_ROAD - 0.35)).abs() - 0.1) * r.per_meter;
            c = over(c, Vec3::new(0.8, 0.6, 0.08), cover(edge));
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
        c.lerp(FOG, fog)
    }

    /// El mapa de la pista abajo a la izquierda, con un punto por nave.
    fn minimap(&self, x: usize, y: usize, c: Vec3) -> Vec3 {
        let game = self.game;
        let size = game.map_size;
        let margin = (16.0 * game.height as f32 / 720.0) as usize;
        let (x0, y0) = (margin, game.height.saturating_sub(size + margin));
        if x < x0 || x >= x0 + size || y < y0 || y >= y0 + size {
            return c;
        }
        let (lx, ly) = (x - x0, y - y0);
        let m = game.minimap[ly * size + lx];
        let mut out = c * (1.0 - m[3] * 0.55) + Vec3::new(m[0], m[1], m[2]);
        for &(mx, my, color, player) in &self.dots {
            let (dx, dy) = (lx as f32 + 0.5 - mx * size as f32, ly as f32 + 0.5 - my * size as f32);
            let radius = if player { 5.0 } else { 4.0 } * game.height as f32 / 720.0;
            let d = dx.hypot(dy) - radius;
            out = over(out, if player { Vec3::ONE } else { color }, cover(d));
            if player {
                out = over(out, color, cover(d + 2.0));
            }
        }
        out
    }

    /// Lo que cae en la fila `y` de un objeto encima de la pista. No se ve
    /// donde la pista que muestra la fila esta mas cerca (una loma lo tapa).
    fn draw_item(&self, item: &Item, y: usize, yf: f32, row: &mut [u8]) {
        let game = self.game;
        let road = self.rows[y];
        let hidden = |x: f32| road.is_some_and(|r| r.depth < item.depth - 1.0 && ((x - r.center) / r.per_meter).abs() < RAIL + 0.4);
        let fog = smoothstep(DRAW as f32 * SEGMENT * 0.45, DRAW as f32 * SEGMENT * 0.95, item.depth);
        let k = item.scale;
        // Coordenadas en metros alrededor del punto en el suelo (y hacia arriba).
        let my = (item.y - yf) / k;
        match item.kind {
            ItemKind::Car { set, sprite, width, hover, boost, player } => {
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
                            lienzo::write(p, lienzo::read(p) * (1.0 - 0.45 * near * (1.0 - e * e).powi(2)));
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
                            lienzo::write(p, lienzo::read(p) + Vec3::new(0.9, 0.7, 0.3) * (0.35 * (1.0 - e)));
                        }
                    }
                }
                if yf < top || yf >= bottom {
                    return;
                }
                let sy = (yf - item.y) / scale + sprite.anchor.1;
                let x0 = (item.x - sprite.anchor.0 * scale).max(0.0) as usize;
                let x1 = ((item.x + (sprite.width as f32 - sprite.anchor.0) * scale) as usize + 1).min(game.width);
                for x in x0..x1 {
                    let xf = x as f32 + 0.5;
                    if hidden(xf) {
                        continue;
                    }
                    let sx = (xf - item.x) / scale + sprite.anchor.0;
                    let [r, g, b, a] = sprite.sample(sx, sy).map(|v| v * near);
                    if a <= 0.0 && r + g + b <= 0.0 {
                        continue;
                    }
                    let p = &mut row[x * 4..x * 4 + 4];
                    let back = lienzo::read(p);
                    let mut src = Vec3::new(r, g, b);
                    // De lejos se pierde en la neblina.
                    src = src.lerp(FOG * a, fog);
                    lienzo::write(p, src + back * (1.0 - a));
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
                                lienzo::write(p, lienzo::read(p) + Vec3::new(0.3, 0.6, 1.0) * ((1.0 - d) * flicker));
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
                    let mut c = lienzo::read(p);
                    c = over(c, Vec3::new(0.06, 0.06, 0.07).lerp(FOG, fog), cover(pole.min(arm)));
                    c = over(c, Vec3::new(1.0, 0.85, 0.6), cover(head));
                    // El resplandor: gaussiano en metros (y nunca menos de
                    // unos pixeles); a 2.6 anchos ya no se ve.
                    let dm = (mx - 2.7).hypot((my - 8.5) * 1.5) / spread;
                    c = c + LAMP_COLOR * (0.55 * (-dm * dm).exp() * (1.0 - fog));
                    lienzo::write(p, c);
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
                    let mut c = lienzo::read(p);
                    let legs = ((mx.abs() - 3.0).abs() - 0.15).max(my - 3.2) / px;
                    c = over(c, Vec3::new(0.08, 0.08, 0.09).lerp(FOG, fog), cover(legs));
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
                        c = over(c, (face * lit).lerp(FOG, fog), cover(panel));
                    }
                    lienzo::write(p, c);
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
                    let mut c = lienzo::read(p);
                    let posts = ((mx.abs() - span).abs() - 0.3).max(my - 9.0) / px;
                    c = over(c, Vec3::new(0.12, 0.12, 0.14).lerp(FOG, fog), cover(posts));
                    let beam = (mx.abs() - span).max((my - 8.4).abs() - 0.6) / px;
                    if beam < 1.0 {
                        let check = ((mx.floor() as i64) + ((my - 7.8) / 0.6).floor() as i64).rem_euclid(2) == 0;
                        let face = if check { Vec3::new(0.85, 0.85, 0.85) } else { Vec3::new(0.03, 0.03, 0.03) };
                        c = over(c, face.lerp(FOG, fog), cover(beam));
                    }
                    if lights > 0 {
                        let housing = (mx.abs() - 0.75).max((my - 6.3).abs() - 1.5) / px;
                        c = over(c, Vec3::new(0.02, 0.02, 0.025), cover(housing));
                        for (k_light, (ly, on_color)) in [(7.2, Vec3::new(1.0, 0.1, 0.08)), (6.3, Vec3::new(1.0, 0.1, 0.08)), (5.4, Vec3::new(1.0, 0.1, 0.08))].into_iter().enumerate() {
                            let green = lights >= 4;
                            let on = green || lights as usize > k_light;
                            let color = if green { Vec3::new(0.2, 1.0, 0.35) } else { on_color };
                            let d = (mx.hypot(my - ly) - 0.36) / px;
                            let bulb = if on { color * 1.2 } else { color * 0.12 };
                            c = over(c, bulb, cover(d));
                            if on {
                                c = c + color * (0.5 * halo(d, 8.0 + 0.6 * k));
                            }
                        }
                    }
                    lienzo::write(p, c);
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
                    let n = game.noise.noise(Vec3::new(xf / radius * 1.5, yf / radius * 1.5, age)) * 0.5 + 0.5;
                    let a = opacity * (1.0 - d * d) * (0.6 + 0.4 * n);
                    let p = &mut row[x * 4..x * 4 + 4];
                    lienzo::write(p, over(lienzo::read(p), Vec3::new(0.05, 0.045, 0.04), a));
                }
            }
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

/// Mapeo de tonos y gamma (de luz lineal a lo que se ve).
fn tone(c: Vec3) -> Vec3 {
    let t = |v: f32| (1.0 - (-v.max(0.0) * 1.3).exp()).powf(1.0 / 2.2);
    Vec3::new(t(c.x), t(c.y), t(c.z))
}

/// El cielo pasado a un panorama: rumbo de 0 a 2 pi y altura de -1 a 1.3
/// radianes.
struct Panorama {
    width: usize,
    height: usize,
    texels: Vec<Vec3>,
}

const PITCH_LOW: f32 = -1.0;
const PITCH_HIGH: f32 = 1.3;

impl Panorama {
    fn new(sky: &(dyn Fn(Vec3) -> Vec3 + Sync)) -> Self {
        let (width, height) = (2048, 720);
        let mut texels = vec![Vec3::ZERO; width * height];
        parallel::chunks_mut(&mut texels, width, 1, |y, row| {
            let pitch = PITCH_HIGH - (y as f32 + 0.5) / height as f32 * (PITCH_HIGH - PITCH_LOW);
            for (x, t) in row.iter_mut().enumerate() {
                let yaw = (x as f32 + 0.5) / width as f32 * TAU;
                let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
                *t = tone(sky(dir) * 0.9);
            }
        });
        Self { width, height, texels }
    }

    fn sample(&self, yaw: f32, pitch: f32) -> Vec3 {
        let u = (yaw / TAU).rem_euclid(1.0) * self.width as f32 - 0.5;
        let v = ((PITCH_HIGH - pitch) / (PITCH_HIGH - PITCH_LOW)).clamp(0.0, 1.0) * self.height as f32 - 0.5;
        let (x0, y0) = (u.floor(), v.floor().max(0.0));
        let (fx, fy) = (u - x0, (v - y0).clamp(0.0, 1.0));
        let x0 = (x0 as i64).rem_euclid(self.width as i64) as usize;
        let x1 = (x0 + 1) % self.width;
        let y0 = (y0 as usize).min(self.height - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let t = |x: usize, y: usize| self.texels[y * self.width + x];
        let top = t(x0, y0).lerp(t(x1, y0), fx);
        let bottom = t(x0, y1).lerp(t(x1, y1), fx);
        top.lerp(bottom, fy)
    }
}

/// El dibujo del mapa una sola vez: la linea de la pista con su borde,
/// premultiplicada (color y alfa).
fn draw_map(track: &Track, size: usize) -> Vec<[f32; 4]> {
    let mut image = vec![[0.0f32; 4]; size * size];
    let pad = 0.08;
    let points: Vec<(f32, f32)> = track.map.iter().map(|&(x, y)| ((pad + x * (1.0 - 2.0 * pad)) * size as f32, (pad + y * (1.0 - 2.0 * pad)) * size as f32)).collect();
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
