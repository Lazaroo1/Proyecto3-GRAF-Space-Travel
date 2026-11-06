//! Talega Surfers (Antigua Guatemala, version chapina): el patojo se baja
//! de la nave y corre por la calle del Arco, sobre el empedrado, en tres
//! carriles.
//!
//! - **Lo que hay en la calle**: camionetas en fila, derechas en su carril
//!   como un convoy: paradas (por la rampa de tablones que tienen adelante se
//!   sube corriendo y se corre por los techos de toda la fila, de uno al
//!   otro; sin rampa, se esquivan) o que vienen de frente (esas no perdonan,
//!   salvo que se vaya por los techos), tuk-tuks parados, carretas de
//!   helados (se saltan), lazos de papel picado (se pasan rodando) y
//!   alfombras de aserrin: si se pisa una, salen los cucuruchos a
//!   perseguirlo, y si se pisa otra mientras tanto, lo agarran. Los lenes son
//!   las monedas.
//! - **El Fuego** va explotando atras: la ceniza oscurece todo poco a poco y
//!   caen bombas de lava que marcan su sombra antes de caer.
//! - **Las teclas**: flechas o A D para cambiar de carril, arriba o W para
//!   saltar, abajo o S para rodar (en el aire, baja de golpe).
//!
//! **El dibujo**, en CPU por filas en paralelo: cada pixel lanza su rayo
//! desde la camara (que mira de frente, asi `t` es la profundidad) contra el
//! suelo, las dos fachadas (planos `x = +-4.4 m` con la altura de cada casa),
//! el Arco de Santa Catalina y las cajas de lo que hay en la calle (prueba de
//! las placas: entra al ultimo, sale al primero). El cielo del atardecer con
//! el Volcan de Agua detras del arco y el Fuego con su columna de ceniza se
//! pinta por pixel. Encima van, con su profundidad, las monedas, las bombas,
//! el humo, el patojo y los cucuruchos (figuras de `figuras.rs`).

use std::f32::consts::{PI, TAU};

use super::carrera::value_noise;
use super::figuras::{Canvas, paint};
use super::flota::{self, Fleet};
use super::hornear::{Lut, Pick, Sprite};
use super::lienzo;
use super::lienzo::{emission, write_emit};
use super::malla::{self, Raster};
use super::personaje::{self, RUN_FRAMES};
use super::sonido;
use super::posproceso::{self, Posproceso};
use std::sync::Arc;
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::math::{Vec3, fast_exp, hash_u32, smoothstep};
use crate::parallel;


/// Lo ancho de un carril, del empedrado (medio) y donde estan las fachadas.
const LANE: f32 = 2.2;
const STREET: f32 = 3.4;
const WALL: f32 = 4.4;
/// La camara: cuanto atras y cuanto arriba del patojo.
const CAM_BACK: f32 = 6.0;
const CAM_UP: f32 = 4.2;
/// Gravedad, velocidad del salto y lo que dura una rodada.
const GRAVITY: f32 = 26.0;
const JUMP: f32 = 8.6;
const ROLL: f32 = 0.62;
/// Lo alto del techo de una camioneta (sobre la parrilla se corre) y lo
/// largo de una camioneta y de un tuk-tuk.
const ROOF: f32 = flota::BUS_HEIGHT - 0.1;
const BUS_LEN: f32 = 8.0;
/// Lo que queda entre dos camionetas de una fila (se pasa corriendo por los
/// techos) y lo largo de la rampa de tablones que sube al primer techo.
const BUS_GAP: f32 = 0.5;
const RAMP_LEN: f32 = 5.0;
/// Hasta que distancia las naves van en 3D (su malla en perspectiva); mas
/// lejos, el sprite horneado (de lejos no se nota la diferencia).
const RASTER_DIST: f32 = 42.0;
const TUK_LEN: f32 = 2.6;
/// El sol de la tarde: hacia donde esta (bajo, de la derecha y un poco de
/// atras, normalizado), su color y el de la luz del cielo (la sombra).
const SUN_DIR: Vec3 = Vec3::new(0.7, 0.66, -0.27);
const SUN: Vec3 = Vec3::new(1.2, 0.84, 0.52);
const SKY_LIGHT: Vec3 = Vec3::new(0.36, 0.4, 0.56);
/// El alero: cuanto sale de la pared y su grueso. La reja volada, el
/// tejadillo de la ventana, lo hondo de las ventanas y del zaguan, y el
/// alto de la banqueta.
const EAVE: f32 = 0.6;
const EAVE_T: f32 = 0.28;
const GRILLE: f32 = 0.24;
const AWNING_OUT: f32 = 0.34;
const RECESS: f32 = 0.24;
const DOOR_RECESS: f32 = 0.36;
const CURB: f32 = 0.15;
/// El fondo detras de los techos: los arboles y las iglesias, y los cerros.
const BG_NEAR: f32 = 42.0;
const BG_FAR: f32 = 110.0;
/// Las ventanas de cada casa (donde empiezan, a lo largo) y su alto.
const WINDOWS: [f32; 2] = [1.0, 6.4];
const WIN_LO: f32 = 1.05;
const WIN_HI: f32 = 2.5;
/// Lo hondo del Arco y donde arranca su boveda.
const ARCH_DEPTH: f32 = 3.0;
const ARCH_SPRING: f32 = 4.2;
/// Lo largo de cada casa de la calle.
const HOUSE: f32 = 9.0;
/// Cada cuanto pasa el Arco.
const ARCH_EVERY: f32 = 420.0;
/// Cada tramo de calle que se arma de una vez, y hasta donde adelante.
const CHUNK: f32 = 28.0;
const AHEAD: f32 = 170.0;
/// Velocidad al empezar y la de a todo.
const START_SPEED: f32 = 12.0;
const TOP_SPEED: f32 = 24.0;
/// Lo que dura la persecucion de los cucuruchos.
const CHASE: f32 = 7.0;
/// Cuanto antes de caer se ve la sombra de una bomba.
const WARNING: f32 = 1.6;
const RECORD: &str = "surfers_record";

/// Lo que hay en la calle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Thing {
    /// Camioneta parada, de espaldas: la escalera de atras sube al techo.
    Parked,
    /// Camioneta que viene de frente.
    Bus,
    Cart,
    TukTuk,
    /// Lazo de papel picado de lado a lado: se pasa rodando.
    Banner,
    /// Alfombra de aserrin en el suelo (`lanes` dice en que carriles).
    Carpet,
    /// Rampa de tablones que sube del suelo al techo de una fila de
    /// camionetas paradas: se sube corriendo.
    Ramp,
}

#[derive(Clone, Copy, Debug)]
struct Obstacle {
    thing: Thing,
    /// Los carriles que ocupa (un bit por carril).
    lanes: u8,
    /// Donde empieza (lo de mas atras) y lo largo (m), y a que velocidad
    /// viene hacia el patojo.
    z: f32,
    len: f32,
    v: f32,
    seed: u32,
    stepped: bool,
}

impl Obstacle {
    /// La caja (minimo y maximo).
    fn bounds(&self) -> (Vec3, Vec3) {
        let first = self.lanes.trailing_zeros() as usize;
        let last = 7 - self.lanes.leading_zeros() as usize;
        let (x0, x1) = (lane_x(first.min(2)), lane_x(last.min(2)));
        let (half, y0, y1) = match self.thing {
            Thing::Parked | Thing::Bus => (1.3, 0.0, ROOF),
            Thing::Cart => (0.8, 0.0, 1.1),
            Thing::TukTuk => (0.78, 0.0, 1.95),
            Thing::Banner => return (Vec3::new(-WALL, 1.05, self.z), Vec3::new(WALL, 1.65, self.z + 0.12)),
            Thing::Carpet => (1.1, 0.0, 0.0),
            Thing::Ramp => (1.05, 0.0, ROOF),
        };
        (Vec3::new(x0 - half, y0, self.z), Vec3::new(x1 + half, y1, self.z + self.len))
    }

    fn solid(&self) -> bool {
        !matches!(self.thing, Thing::Carpet | Thing::Ramp)
    }

    /// Lo alto de la rampa a la distancia `z` (0 al pie, el techo arriba).
    fn ramp_height(&self, z: f32) -> f32 {
        ROOF * ((z - self.z) / self.len).clamp(0.0, 1.0)
    }
}

fn lane_x(lane: usize) -> f32 {
    (lane as f32 - 1.0) * LANE
}

/// Alguien en la banqueta: donde va, a que velocidad y quien es (0 una
/// senora con su canasta, 1 un turista, 2 el de los globos).
#[derive(Clone, Copy, Debug)]
struct Walker {
    x: f32,
    z: f32,
    v: f32,
    kind: u8,
    seed: u32,
}

/// Una paloma: en el suelo picoteando hasta que pasa el patojo, y entonces
/// sale volando.
#[derive(Clone, Copy, Debug)]
struct Pigeon {
    p: Vec3,
    v: Vec3,
    flying: bool,
    seed: u32,
}

#[derive(Clone, Copy, Debug)]
struct Coin {
    x: f32,
    y: f32,
    z: f32,
    taken: bool,
}

#[derive(Clone, Copy, Debug)]
struct Bomb {
    x: f32,
    z: f32,
    /// Cuando cae (segundos de la corrida).
    land: f32,
    done: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Puff {
    Dust,
    Lava,
    Spark,
    Ash,
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    kind: Puff,
    p: Vec3,
    v: Vec3,
    age: f32,
    life: f32,
}

/// El patojo.
#[derive(Clone, Copy, Debug)]
struct Runner {
    lane: usize,
    x: f32,
    y: f32,
    vy: f32,
    z: f32,
    v: f32,
    roll: f32,
    /// Subiendo por la escalera de una camioneta (lo que falta, s).
    climb: f32,
    /// La fase de la carrera (los pasos).
    phase: f32,
    /// Lo que tiene abajo: el suelo o un techo.
    support: f32,
}

impl Runner {
    fn new() -> Self {
        Self { lane: 1, x: 0.0, y: 0.0, vy: 0.0, z: 0.0, v: START_SPEED, roll: 0.0, climb: 0.0, phase: 0.0, support: 0.0 }
    }

    fn grounded(&self) -> bool {
        self.y <= self.support + 1e-3 && self.vy <= 0.0
    }

    fn height(&self) -> f32 {
        if self.roll > 0.0 { 0.8 } else { 1.7 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Running,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sfx {
    Step,
    Jump,
    Land,
    Coin,
    Crash,
    Horn,
    Whistle,
    Boom,
    Climb,
    Flap,
    /// El claxon de don Meyer (H).
    Meyer,
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    sfx: Sfx,
    n: u32,
}

/// La camara de este frame.
#[derive(Clone, Copy, Debug)]
struct Camera {
    pos: Vec3,
    focal: f32,
    cx: f32,
    horizon: f32,
}

impl Camera {
    /// Un punto del mundo en la pantalla: `(x, y, pixeles por metro,
    /// profundidad)`.
    fn project(&self, p: Vec3) -> Option<(f32, f32, f32, f32)> {
        let d = p.z - self.pos.z;
        if d < 0.3 {
            return None;
        }
        let k = self.focal / d;
        Some((self.cx + (p.x - self.pos.x) * k, self.horizon - (p.y - self.pos.y) * k, k, d))
    }
}

/// Lo suave del cielo de un frame a un cuarto de resolucion (cambia despacio):
/// se calcula una vez y cada pixel lo lee con interpolacion bilineal.
struct SkyBuf {
    w: usize,
    h: usize,
    px: Vec<Vec3>,
}

impl SkyBuf {
    fn sample(&self, x: f32, y: f32) -> Vec3 {
        let (u, v) = ((x * 0.25).max(0.0), (y * 0.25).max(0.0));
        let (i, j) = ((u as usize).min(self.w - 2), (v as usize).min(self.h - 2));
        let (fu, fv) = ((u - i as f32).min(1.0), (v - j as f32).min(1.0));
        let at = |a: usize, b: usize| self.px[b * self.w + a];
        let top = at(i, j).lerp(at(i + 1, j), fu);
        let bottom = at(i, j + 1).lerp(at(i + 1, j + 1), fu);
        top.lerp(bottom, fv)
    }
}

/// Lo del suelo de una fila (ver `TalegaSurfers::ground_row`).
struct GroundRow<'a> {
    carpets: Vec<&'a Obstacle>,
    shadows: Vec<(f32, f32)>,
    casters: Vec<(Vec3, Vec3)>,
    lamp: f32,
}

/// Si el rayo desde `p` hacia `d` cruza la caja (prueba de las placas).
fn hits_box(p: Vec3, d: Vec3, min: Vec3, max: Vec3) -> bool {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for (o, dir, lo, hi) in [(p.x, d.x, min.x, max.x), (p.y, d.y, min.y, max.y), (p.z, d.z, min.z, max.z)] {
        if dir.abs() < 1e-6 {
            if o < lo || o > hi {
                return false;
            }
        } else {
            let (a, b) = ((lo - o) / dir, (hi - o) / dir);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    t0 <= t1
}

/// Una nave de la flota en la pantalla: su sprite y su mip, donde pisa, su
/// escala, su profundidad (la de su cara de enfrente), con que permutacion
/// de colores se pinta y su neblina.
struct Ride<'a> {
    sprite: &'a Sprite,
    pick: Pick,
    at: (f32, f32),
    scale: f32,
    depth: f32,
    paint: u8,
    fog: f32,
    /// La luz de donde esta: sol o sombra.
    tint: Vec3,
}

/// Una caja para los rayos, con su caja en la pantalla.
#[derive(Clone, Copy, Debug)]
struct BoxHit {
    index: usize,
    min: Vec3,
    max: Vec3,
    y0: f32,
    y1: f32,
    x0: f32,
    x1: f32,
}

pub struct TalegaSurfers {
    width: usize,
    height: usize,
    /// Meyer de frente saludando, para la bienvenida.
    greeter: Option<Arc<Sprite>>,
    /// El claxon de don Meyer (las muestras, a la frecuencia del juego).
    claxon: Arc<Vec<f32>>,
    s: f32,
    focal: f32,
    horizon: f32,
    screen: Screen,
    run: Runner,
    obstacles: Vec<Obstacle>,
    coins: Vec<Coin>,
    walkers: Vec<Walker>,
    pigeons: Vec<Pigeon>,
    bombs: Vec<Bomb>,
    particles: Vec<Particle>,
    gen_z: f32,
    /// Hasta donde ya hay una fila de camionetas en cada carril: lo que se
    /// genere despues ahi empieza mas adelante.
    lane_busy: [f32; 3],
    time: f32,
    lenes: u32,
    chase: f32,
    reason: &'static str,
    bomb_in: f32,
    records: Records,
    best: Option<i64>,
    rng: u32,
    wall: f32,
    done: bool,
    autopilot: bool,
    /// Meyer horneado (los cuadros de la carrera y el del salto), si esta
    /// el modelo; si no, se dibuja un patojo con figuras.
    meyer: Option<Arc<Vec<Sprite>>>,
    /// Las mallas de las naves para dibujarlas de cerca, y el rasterizador.
    meshes: Option<Arc<flota::Meshes>>,
    /// Uno por nave de cerca: se arman en paralelo.
    rasters: Vec<Raster>,
    /// Las naves chapinas horneadas (camionetas y tuk-tuks), si estan los
    /// modelos; si no, cajas pintadas.
    fleet: Option<Arc<Fleet>>,
    /// El brillo de lo que emite, la vineta, la gradacion y las lineas de
    /// velocidad (el mismo posproceso de la carrera).
    post: Posproceso,
    auto_wait: f32,
    message: Option<(&'static str, f32)>,
    // El sonido.
    pending: Vec<Sfx>,
    voices: Vec<Voice>,
    noise: u32,
    wind: f32,
    rumble: f32,
    march: f32,
    step_at: f32,
}

impl TalegaSurfers {
    pub fn new(width: usize, height: usize, records: Records) -> Self {
        let best = records.get(RECORD);
        let mut game = Self {
            width,
            height,
            greeter: None,
            claxon: Arc::new(Vec::new()),
            meshes: None,
            rasters: Vec::new(),
            s: height as f32 / 720.0,
            focal: 0.95 * height as f32,
            horizon: 0.36 * height as f32,
            screen: Screen::Menu,
            run: Runner::new(),
            obstacles: Vec::new(),
            coins: Vec::new(),
            walkers: Vec::new(),
            pigeons: Vec::new(),
            bombs: Vec::new(),
            particles: Vec::new(),
            gen_z: 26.0,
            lane_busy: [0.0; 3],
            time: 0.0,
            lenes: 0,
            chase: 0.0,
            reason: "",
            bomb_in: 18.0,
            records,
            best,
            rng: 0xA471_6A00,
            wall: 0.0,
            done: false,
            autopilot: false,
            meyer: None,
            fleet: None,
            post: Posproceso::new(width, height),
            auto_wait: 0.0,
            message: None,
            pending: Vec::new(),
            voices: Vec::new(),
            noise: 0x2468_ACE1,
            wind: 0.0,
            rumble: 0.0,
            march: 0.0,
            step_at: 0.0,
        };
        // Meyer, horneado desde la camara del juego: de espaldas, desde
        // `atan((alto de la camara - 0.9) / atras)` arriba, a los pixeles por
        // metro de donde corre.
        let elevation = ((CAM_UP - 0.9) / CAM_BACK).atan();
        game.meyer = personaje::meyer(elevation, game.focal / CAM_BACK);
        // La bienvenida: Meyer de frente, de casi todo el alto de la pantalla.
        game.greeter = personaje::greeter((0.8 * height as f32 / personaje::HEIGHT).round());
        game.claxon = Arc::new(claxon());
        // Las naves, a un decimo de la escala de la camara (de cerca se
        // agrandan poco; de lejos van los mips).
        game.fleet = flota::fleet((game.focal / 10.0).round());
        game.meshes = flota::meshes();
        game.reset();
        game
    }

    /// Corre solo (las fotos y las pruebas).
    pub fn set_autopilot(&mut self, on: bool) {
        self.autopilot = on;
    }

    /// Empieza a correr desde el principio de la calle.
    pub fn start(&mut self) {
        self.reset();
        self.screen = Screen::Running;
    }

    fn reset(&mut self) {
        self.run = Runner::new();
        self.obstacles.clear();
        self.coins.clear();
        self.walkers.clear();
        self.pigeons.clear();
        self.bombs.clear();
        self.particles.clear();
        self.gen_z = 26.0;
        self.lane_busy = [0.0; 3];
        self.time = 0.0;
        self.lenes = 0;
        self.chase = 0.0;
        self.reason = "";
        self.bomb_in = 18.0;
        self.rng = 0xA471_6A00;
        self.generate();
    }

    pub fn running(&self) -> bool {
        self.screen == Screen::Running
    }

    /// Por que se acabo la corrida.
    pub fn reason(&self) -> &'static str {
        self.reason
    }

    /// Metros, lenes y puntos.
    pub fn tally(&self) -> (f32, u32, i64) {
        (self.run.z, self.lenes, self.score())
    }

    fn score(&self) -> i64 {
        self.run.z as i64 + 10 * self.lenes as i64
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Cuanto ha explotado el Fuego (0 a 1).
    fn eruption(&self) -> f32 {
        (self.time / 100.0).clamp(0.0, 1.0)
    }

    /// Arma la calle hasta `AHEAD` metros adelante.
    fn generate(&mut self) {
        while self.gen_z < self.run.z + AHEAD {
            let z = self.gen_z;
            self.chunk(z);
            self.gen_z += CHUNK;
        }
    }

    /// Un tramo de calle: siempre queda un carril para pasar.
    fn chunk(&mut self, z0: f32) {
        // La gente en las banquetas.
        for side in [-1.0f32, 1.0] {
            for _ in 0..2 {
                if self.random() < 0.55 {
                    let z = z0 + self.random() * CHUNK;
                    let v = (self.random() - 0.5) * 2.6;
                    let kind = (self.random() * 3.0) as u8 % 3;
                    let seed = hash_u32(z.to_bits() ^ 0xC0FE);
                    self.walkers.push(Walker { x: side * (STREET + 0.5), z, v, kind, seed });
                }
            }
        }
        // A veces, una parvada de palomas en el empedrado.
        if self.random() < 0.45 {
            let (cx, cz) = ((self.random() - 0.5) * 5.0, z0 + self.random() * CHUNK);
            let n = 4 + (self.random() * 5.0) as usize;
            for k in 0..n {
                let (dx, dz) = (self.random() - 0.5, self.random() - 0.5);
                self.pigeons.push(Pigeon { p: Vec3::new(cx + 1.6 * dx, 0.0, cz + 1.6 * dz), v: Vec3::ZERO, flying: false, seed: hash_u32(cz.to_bits() ^ k as u32) });
            }
        }
        let difficulty = (z0 / 1800.0).clamp(0.0, 1.0);
        let mut free = (self.random() * 3.0) as usize % 3;
        // El carril libre, de los que no tiene encima una fila de antes.
        let busy = self.lane_busy;
        if busy[free] > z0 {
            free = (0..3).min_by(|&a, &b| busy[a].total_cmp(&busy[b])).unwrap_or(free);
        }
        let (first_obstacle, first_coin) = (self.obstacles.len(), self.coins.len());
        let pattern = (self.random() * 5.0) as usize % 5;
        let seed = hash_u32(z0 as u32 ^ 0x5EED);
        let push = |obs: &mut Vec<Obstacle>, thing: Thing, lanes: u8, z: f32, len: f32, v: f32, k: u32| {
            obs.push(Obstacle { thing, lanes, z, len, v, seed: hash_u32(seed ^ k.wrapping_mul(0x9E37)), stepped: false });
        };
        match pattern {
            0 => {
                // Camionetas en fila, derechas en su carril: paradas (casi
                // siempre con su rampa adelante, para subirse corriendo y
                // seguir por los techos) o de frente (de dos en dos).
                let mut k = 0;
                for lane in 0..3 {
                    if lane == free || busy[lane] > z0 {
                        continue;
                    }
                    if self.random() < 0.55 + 0.35 * difficulty {
                        let parked = self.random() < 0.65;
                        let count = if parked { 2 + (self.random() * 2.99) as usize } else { 2 };
                        let mut at = z0 + 2.0 + self.random() * 4.0;
                        let ramp = parked && self.random() < 0.75;
                        if ramp {
                            push(&mut self.obstacles, Thing::Ramp, 1 << lane, at, RAMP_LEN, 0.0, k);
                            k += 1;
                            // Lenes subiendo por la rampa.
                            for i in 0..4 {
                                let u = (i as f32 + 0.5) / 4.0;
                                self.coins.push(Coin { x: lane_x(lane), y: 0.9 + ROOF * u, z: at + RAMP_LEN * u, taken: false });
                            }
                            at += RAMP_LEN;
                        }
                        let (thing, v) = if parked { (Thing::Parked, 0.0) } else { (Thing::Bus, 7.0) };
                        for b in 0..count {
                            push(&mut self.obstacles, thing, 1 << lane, at + b as f32 * (BUS_LEN + BUS_GAP), BUS_LEN, v, k);
                            k += 1;
                        }
                        let end = at + count as f32 * (BUS_LEN + BUS_GAP);
                        if parked && (ramp || self.random() < 0.4) {
                            self.coin_line(lane_x(lane), ROOF + 0.9, at + 1.0, count * 4, 2.1);
                        }
                        self.lane_busy[lane] = self.lane_busy[lane].max(end + 2.0);
                    }
                }
                self.coin_line(lane_x(free), 0.9, z0 + 4.0, 7, 2.0);
            }
            1 => {
                // Alfombras de aserrin: a lo ancho (con suerte queda un
                // carril limpio) y lenes en arco para saltarlas.
                let mut lanes = 0b111u8;
                if self.random() > difficulty {
                    lanes &= !(1 << free);
                }
                let at = z0 + 8.0;
                // Una alfombra por cada tramo de carriles seguidos (la caja
                // va del primero al ultimo: `101` serian dos).
                let mut run = 0u8;
                for lane in 0..=3 {
                    if lane < 3 && lanes & (1 << lane) != 0 {
                        run |= 1 << lane;
                    } else if run != 0 {
                        push(&mut self.obstacles, Thing::Carpet, run, at, 5.0, 0.0, 1 + lane as u32);
                        run = 0;
                    }
                }
                let lane = (free + 1) % 3;
                for i in 0..7 {
                    let u = i as f32 / 6.0;
                    let x = lane_x(lane);
                    self.coins.push(Coin { x, y: 0.9 + 1.3 * (PI * u).sin(), z: at - 2.0 + 9.0 * u, taken: false });
                }
            }
            2 => {
                // Un lazo de papel picado (rodar) y una carreta de helados
                // (saltar).
                push(&mut self.obstacles, Thing::Banner, 0b111, z0 + 6.0, 0.12, 0.0, 2);
                let lane = (free + 1 + (self.random() * 2.0) as usize % 2) % 3;
                push(&mut self.obstacles, Thing::Cart, 1 << lane, z0 + 18.0, 1.0, 0.0, 3);
                if self.random() < 0.5 + 0.4 * difficulty {
                    let other = 3 - free - lane;
                    if other < 3 && other != free {
                        push(&mut self.obstacles, Thing::Cart, 1 << other, z0 + 19.5, 1.0, 0.0, 4);
                    }
                }
                self.coin_line(lane_x(free), 0.9, z0 + 10.0, 6, 2.0);
            }
            3 => {
                // Tuk-tuks parados.
                for (k, lane) in (0..3).filter(|&l| l != free).enumerate() {
                    if self.random() < 0.7 + 0.3 * difficulty {
                        push(&mut self.obstacles, Thing::TukTuk, 1 << lane, z0 + 6.0 + 9.0 * k as f32, TUK_LEN, 0.0, 5 + k as u32);
                    }
                }
                self.coin_line(lane_x(free), 0.9, z0 + 3.0, 8, 2.2);
            }
            _ => {
                // Respiro: lenes en zigzag.
                for i in 0..10 {
                    let lane = [0usize, 1, 2, 1][i % 4];
                    self.coins.push(Coin { x: lane_x(lane), y: 0.9, z: z0 + 3.0 + 2.2 * i as f32, taken: false });
                }
            }
        }
        self.clear_under(busy, first_obstacle, first_coin);
    }

    /// Lo de este tramo que cayo encima de una fila de camionetas de antes
    /// (en su carril y antes de que termine) se quita: lo que va en el suelo
    /// y los lenes de abajo. Los lazos de papel picado se quedan (pasan
    /// arriba de los que van por el suelo y abajo de los que van por los
    /// techos).
    fn clear_under(&mut self, busy: [f32; 3], first_obstacle: usize, first_coin: usize) {
        let blocked = |lanes: u8, z: f32| (0..3).any(|l| lanes & (1 << l) != 0 && z < busy[l]);
        let mut i = first_obstacle;
        while i < self.obstacles.len() {
            let o = self.obstacles[i];
            if o.thing != Thing::Banner && blocked(o.lanes, o.z) {
                self.obstacles.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = first_coin;
        while i < self.coins.len() {
            let c = self.coins[i];
            let lane = (0..3).min_by(|&a, &b| (lane_x(a) - c.x).abs().total_cmp(&(lane_x(b) - c.x).abs())).unwrap_or(1);
            if c.y < ROOF && c.z < busy[lane] {
                self.coins.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn coin_line(&mut self, x: f32, y: f32, z: f32, n: usize, gap: f32) {
        for i in 0..n {
            self.coins.push(Coin { x, y, z: z + gap * i as f32, taken: false });
        }
    }

    /// Donde esta la camara.
    fn camera(&self) -> Camera {
        let (w, h) = (self.width as f32, self.height as f32);
        let _ = h;
        // Con cada paso la camara sube y baja un poco (corriendo, no en el
        // aire ni rodando).
        let r = &self.run;
        let bob = if r.grounded() && r.roll <= 0.0 && self.screen == Screen::Running { 0.045 * (2.0 * r.phase).sin().abs() } else { 0.0 };
        Camera { pos: Vec3::new(r.x * 0.6, CAM_UP + bob + r.support * 0.6 + (r.y - r.support) * 0.3, r.z - CAM_BACK), focal: self.focal, cx: w * 0.5, horizon: self.horizon }
    }

    /// Lo que haria quien sabe: ir por el carril mas libre, saltar las
    /// carretas y las alfombras, rodar bajo los lazos y no estar donde cae
    /// una bomba.
    fn auto_keys(&self) -> Keys {
        let mut k = Keys::default();
        let r = &self.run;
        let reach = 14.0 + r.v * 0.6;
        let high = r.y >= ROOF - 0.45;
        // Si la fila de la camioneta `bus` tiene su rampa y todavia se puede
        // tomar desde el carril `lane` (esta adelante, o ya se va en ella).
        let ramp_for = |lane: usize, bus: &Obstacle| -> bool {
            let mut start = bus.z;
            while let Some(p) = self.obstacles.iter().find(|p| p.thing == Thing::Parked && p.lanes == bus.lanes && (p.z + p.len + BUS_GAP - start).abs() < 0.05) {
                start = p.z;
            }
            self.obstacles.iter().any(|p| {
                p.thing == Thing::Ramp && p.lanes == bus.lanes && (p.z + p.len - start).abs() < 0.05 && (p.z > r.z + 0.5 || (lane == r.lane && r.z >= p.z - 0.5))
            })
        };
        // Que tan lejos esta lo primero que mata en cada carril.
        let danger = |lane: usize| -> f32 {
            let mut near = f32::MAX;
            for o in &self.obstacles {
                if o.lanes & (1 << lane) == 0 || (!o.solid() && o.thing != Thing::Ramp) || o.thing == Thing::Banner {
                    continue;
                }
                let ahead = o.z - r.z;
                if ahead + o.len < -0.5 || ahead > reach {
                    continue;
                }
                let deadly = match o.thing {
                    // Por los techos se sigue; a una fila parada se sube por
                    // su rampa.
                    Thing::Parked => !high && !ramp_for(lane, o),
                    Thing::Bus => !high,
                    // Meterse de lado a media rampa.
                    Thing::Ramp => lane != r.lane && !high && o.z < r.z + 1.0,
                    Thing::Cart => ahead < 0.18 * r.v + 0.6,
                    _ => true,
                };
                if deadly {
                    near = near.min(ahead.max(0.0));
                }
            }
            for b in &self.bombs {
                if !b.done && (b.x - lane_x(lane)).abs() < 1.0 {
                    let arrive = r.z + r.v * (b.land - self.time);
                    if (arrive - b.z).abs() < 6.0 {
                        near = near.min((b.z - r.z).max(0.0));
                    }
                }
            }
            for o in &self.obstacles {
                if o.thing == Thing::Carpet && o.lanes & (1 << lane) != 0 && o.z - r.z < reach && o.z + o.len > r.z {
                    near = near.min(f32::MAX - 1.0).min(reach * 0.9 + (o.z - r.z) * 0.01);
                }
            }
            near
        };
        let here = danger(r.lane);
        let mut best = r.lane;
        for lane in [r.lane.saturating_sub(1), (r.lane + 1).min(2)] {
            if danger(lane) > danger(best) + 2.0 {
                best = lane;
            }
        }
        if best < r.lane {
            k.left = true;
            k.steer = -1.0;
        } else if best > r.lane {
            k.right = true;
            k.steer = 1.0;
        }
        let _ = here;
        // Saltar o rodar lo que viene en el carril.
        for o in &self.obstacles {
            if o.lanes & (1 << r.lane) == 0 {
                continue;
            }
            let ahead = o.z - r.z;
            // La carreta (1.1 m de alto) se salta cuando esta entre `0.18 v`
            // y `0.5 v - 1.5` adelante: con `y = 8.6 t - 13 t^2` se pasa de
            // 1.1 m a los 0.16 s y se baja de ahi a los 0.5 s. La alfombra
            // (5 m), justo antes, para caer despues de ella: en el aire se
            // avanzan `2 v0 / g v` metros.
            let (low, high) = (0.18 * r.v + 0.4, 0.5 * r.v - 1.5);
            let carpet = 2.0 * JUMP / GRAVITY * r.v - o.len - 1.0;
            match o.thing {
                Thing::Cart if ahead > low && ahead < high && r.grounded() && r.y < 0.1 => k.up = true,
                Thing::Carpet if ahead > 0.45 && ahead < carpet.max(0.8) && r.grounded() && r.y < 0.1 => k.up = true,
                Thing::Banner if ahead > 0.2 && ahead < 0.45 * r.v && r.y < 1.0 => k.down = true,
                _ => {}
            }
        }
        k
    }

    fn crash(&mut self, reason: &'static str) {
        if self.screen != Screen::Running {
            return;
        }
        self.reason = reason;
        self.screen = Screen::Over;
        self.pending.push(Sfx::Crash);
        let score = self.score();
        if self.best.is_none_or(|b| score > b) {
            self.best = Some(score);
            self.records.set(RECORD, score);
        }
    }

    fn step(&mut self, dt: f32, keys: &Keys) {
        self.time += dt;
        let r = &mut self.run;
        r.v = (START_SPEED + 0.11 * self.time).min(TOP_SPEED);
        // Los carriles.
        if keys.left || keys.typed('a') {
            r.lane = r.lane.saturating_sub(1);
        }
        if keys.right || keys.typed('d') {
            r.lane = (r.lane + 1).min(2);
        }
        let target = lane_x(r.lane);
        let dx = target - r.x;
        r.x += dx.signum() * (dx.abs()).min(14.0 * dt);
        // Saltar y rodar.
        let jump = keys.up || keys.typed('w');
        let duck = keys.down || keys.typed('s');
        if r.climb <= 0.0 {
            if jump && r.grounded() {
                r.vy = JUMP;
                r.roll = 0.0;
                self.pending.push(Sfx::Jump);
            }
            if duck {
                if r.grounded() {
                    r.roll = ROLL;
                } else {
                    r.vy = r.vy.min(-14.0);
                    r.roll = ROLL;
                }
            }
        }
        r.roll = (r.roll - dt).max(0.0);
        r.z += r.v * dt;
        r.phase += dt * r.v * 0.75;
        // Lo que viene de frente avanza hacia atras.
        for o in &mut self.obstacles {
            o.z -= o.v * dt;
        }
        // Lo que tiene abajo: el techo de una camioneta si va encima.
        let r = &mut self.run;
        let mut support = 0.0f32;
        for o in &self.obstacles {
            let (min, max) = o.bounds();
            if r.x <= min.x - 0.2 || r.x >= max.x + 0.2 {
                continue;
            }
            match o.thing {
                // El techo, y el hueco con la que sigue en la fila.
                Thing::Parked | Thing::Bus => {
                    if r.z > min.z - BUS_GAP && r.z < max.z + BUS_GAP && r.y >= ROOF - 0.45 {
                        support = ROOF;
                    }
                }
                // La rampa sube con la distancia.
                Thing::Ramp => {
                    if r.z >= min.z && r.z <= max.z {
                        let h = o.ramp_height(r.z);
                        if r.y >= h - 0.8 {
                            support = support.max(h);
                        }
                    }
                }
                _ => {}
            }
        }
        r.support = support;
        if r.climb > 0.0 {
            r.climb -= dt;
            r.y = ROOF * (1.0 - (r.climb / 0.25).max(0.0));
            r.vy = 0.0;
            if r.climb <= 0.0 {
                r.y = ROOF;
            }
        } else {
            let was_air = r.y > r.support + 1e-3;
            r.vy -= GRAVITY * dt;
            r.y += r.vy * dt;
            if r.y <= r.support {
                r.y = r.support;
                if was_air && r.vy < -3.0 {
                    self.pending.push(Sfx::Land);
                    for k in 0..6 {
                        let a = TAU * k as f32 / 6.0;
                        self.particles.push(Particle { kind: Puff::Dust, p: Vec3::new(r.x + 0.3 * a.cos(), r.y + 0.1, r.z + 0.3 * a.sin()), v: Vec3::new(a.cos(), 0.6, a.sin() + r.v * 0.3), age: 0.0, life: 0.5 });
                    }
                }
                r.vy = 0.0;
            }
        }
        // Los pasos.
        if self.run.grounded() && self.run.roll <= 0.0 && self.run.phase > self.step_at {
            self.step_at = self.run.phase + PI;
            self.pending.push(Sfx::Step);
        }
        // Choques.
        let r = self.run;
        let (top, bottom) = (r.y + r.height(), r.y);
        let mut hit: Option<&'static str> = None;
        let mut boards = false;
        let mut chased = false;
        for o in &mut self.obstacles {
            let (min, max) = o.bounds();
            let overlap_x = r.x + 0.32 > min.x && r.x - 0.32 < max.x;
            let overlap_z = r.z + 0.3 > min.z && r.z - 0.3 < max.z;
            if !overlap_x || !overlap_z {
                continue;
            }
            match o.thing {
                Thing::Carpet => {
                    if bottom < 0.02 && r.vy <= 0.0 && !o.stepped {
                        o.stepped = true;
                        chased = true;
                    }
                }
                Thing::Banner => {
                    if bottom < max.y && top > min.y {
                        hit = Some("¡Te enredaste en el papel picado!");
                    }
                }
                Thing::Parked | Thing::Bus => {
                    if bottom >= ROOF - 0.45 || r.climb > 0.0 {
                        continue;
                    }
                    hit = Some(if o.thing == Thing::Bus { "¡Te llevó la camioneta!" } else { "¡Te estrellaste con la camioneta!" });
                }
                Thing::Ramp => {
                    // Se sube por el pie; de lado, a media rampa, se choca.
                    if bottom < o.ramp_height(r.z) - 0.8 {
                        hit = Some("¡Te diste contra la rampa!");
                    } else if !o.stepped {
                        o.stepped = true;
                        boards = true;
                    }
                }
                Thing::Cart => {
                    if bottom < max.y {
                        hit = Some("¡Te comiste la carreta de helados!");
                    }
                }
                Thing::TukTuk => {
                    if bottom < max.y {
                        hit = Some("¡Te estrellaste con el tuk-tuk!");
                    }
                }
            }
        }
        // Los tablones suenan al pisar la rampa.
        if boards {
            self.pending.push(Sfx::Climb);
        }
        if chased {
            if self.chase > 0.0 {
                hit = Some("¡Te agarraron los cucuruchos!");
            } else {
                self.chase = CHASE;
                self.message = Some(("¡Pisaste la alfombra! Te persiguen los cucuruchos", self.wall));
            }
        }
        self.chase = (self.chase - dt).max(0.0);
        if let Some(reason) = hit {
            self.crash(reason);
            return;
        }
        // Los lenes.
        let r = self.run;
        let mut got = 0;
        for c in &mut self.coins {
            if !c.taken && (c.z - r.z).abs() < 0.8 && (c.x - r.x).abs() < 0.9 && (c.y - (r.y + 0.8)).abs() < 1.1 {
                c.taken = true;
                got += 1;
            }
        }
        if got > 0 {
            self.lenes += got;
            self.pending.push(Sfx::Coin);
            for k in 0..5 {
                let a = TAU * k as f32 / 5.0;
                self.particles.push(Particle { kind: Puff::Spark, p: Vec3::new(r.x, r.y + 1.0, r.z + 0.6), v: Vec3::new(1.5 * a.cos(), 2.0 + 1.5 * a.sin(), r.v), age: 0.0, life: 0.35 });
            }
        }
        // Las bombas del Fuego: cada vez mas seguido, caen adelante en un
        // carril, con su sombra antes.
        if self.time > 12.0 {
            self.bomb_in -= dt;
            if self.bomb_in <= 0.0 {
                let e = self.eruption();
                self.bomb_in = (5.0 - 3.0 * e) * (0.7 + 0.6 * self.random());
                let lane = (self.random() * 3.0) as usize % 3;
                let z = r.z + r.v * (WARNING + 0.2);
                self.bombs.push(Bomb { x: lane_x(lane), z, land: self.time + WARNING, done: false });
                self.pending.push(Sfx::Whistle);
            }
        }
        let mut boom = Vec::new();
        for b in &mut self.bombs {
            if !b.done && self.time >= b.land {
                b.done = true;
                boom.push((b.x, b.z));
            }
        }
        for (x, z) in boom {
            self.pending.push(Sfx::Boom);
            for k in 0..18 {
                let a = TAU * k as f32 / 18.0;
                let up = 3.0 + 4.0 * self.random();
                self.particles.push(Particle { kind: Puff::Lava, p: Vec3::new(x, 0.3, z), v: Vec3::new(a.cos() * 3.0, up, a.sin() * 3.0), age: 0.0, life: 0.9 });
            }
            for _ in 0..6 {
                let (ox, oz) = (self.random() - 0.5, self.random() - 0.5);
                self.particles.push(Particle { kind: Puff::Ash, p: Vec3::new(x + ox, 0.5, z + oz), v: Vec3::new(ox, 1.5, oz), age: 0.0, life: 1.6 });
            }
            let r = self.run;
            if (x - r.x).abs() < 1.0 && (z - r.z).abs() < 1.3 && r.y < 1.5 {
                self.crash("¡Te cayó una bomba del Fuego!");
                return;
            }
        }
        // Lo que ya quedo atras se va; adelante se sigue armando la calle.
        let behind = self.run.z - CAM_BACK - 4.0;
        self.obstacles.retain(|o| o.z + o.len > behind);
        self.coins.retain(|c| c.z > behind && !c.taken);
        for w in &mut self.walkers {
            w.z += w.v * dt;
        }
        self.walkers.retain(|w| w.z > behind);
        // Las palomas: cuando el patojo llega cerca salen volando, hacia
        // arriba y para un lado, aleteando.
        let (rz, rx) = (self.run.z, self.run.x);
        let mut flap = false;
        for i in 0..self.pigeons.len() {
            let pg = self.pigeons[i];
            if !pg.flying && pg.p.z - rz < 7.0 + 0.2 * self.run.v {
                let away = if pg.p.x > rx { 1.0 } else { -1.0 };
                let (a, b) = (self.random(), self.random());
                self.pigeons[i].flying = true;
                self.pigeons[i].v = Vec3::new(away * (2.0 + 3.0 * a), 3.5 + 2.0 * b, 3.0 + 4.0 * a);
                flap = true;
            }
        }
        for pg in &mut self.pigeons {
            if pg.flying {
                pg.v.y += 1.5 * dt;
                pg.p = pg.p + pg.v * dt;
            }
        }
        if flap {
            self.pending.push(Sfx::Flap);
        }
        self.pigeons.retain(|pg| pg.p.z > behind && pg.p.y < 25.0);
        self.bombs.retain(|b| b.z > behind && (!b.done || self.time - b.land < 1.2));
        self.generate();
        // La camioneta que viene pita.
        for o in &self.obstacles {
            if o.thing == Thing::Bus && o.z - self.run.z < 18.0 && o.z - self.run.z > 17.0 - o.v * dt - self.run.v * dt {
                self.pending.push(Sfx::Horn);
            }
        }
    }

    /// El color del cielo (y lo que se ve en el): el atardecer, el Volcan de
    /// Agua al fondo, el Fuego a la derecha con su penacho y la ceniza que
    /// oscurece todo.
    fn sky(&self, x: f32, y: f32, ash: f32) -> Vec3 {
        self.volcanoes(x, y, ash, self.sky_base(x, y, ash))
    }

    /// Lo suave del cielo (se calcula a un cuarto de resolucion): el
    /// degradado de la tarde, el resplandor del sol (a la derecha, fuera del
    /// cuadro), las nubes que agarran la luz por abajo, el penacho del Fuego
    /// que el viento ladea a la izquierda y la ceniza.
    fn sky_base(&self, x: f32, y: f32, ash: f32) -> Vec3 {
        let (w, hor, s) = (self.width as f32, self.horizon, self.s);
        let t = (y / hor).clamp(0.0, 1.2);
        let mut c = Vec3::new(0.24, 0.36, 0.66).lerp(Vec3::new(0.86, 0.58, 0.58), smoothstep(0.0, 0.75, t)).lerp(Vec3::new(1.0, 0.76, 0.48), smoothstep(0.55, 1.0, t));
        c = c + Vec3::new(1.0, 0.78, 0.45) * (0.45 * (-((x - 1.05 * w) / (0.35 * w)).powi(2)).exp() * (0.4 + 0.6 * t));
        let band = smoothstep(0.1, 0.35, t) * smoothstep(0.95, 0.6, t);
        if band > 0.0 {
            let n = 0.6 * value_noise(x / (180.0 * s) + self.wall * 0.01, y / (40.0 * s)) + 0.4 * value_noise(x / (70.0 * s), y / (18.0 * s) + 3.0);
            let cloud = smoothstep(0.25, 0.65, n) * band;
            c = c.lerp(Vec3::new(1.0, 0.7, 0.55) * (0.85 + 0.25 * t), cloud * 0.6);
        }
        let fx = 0.86 * w;
        let e = self.eruption();
        let crater = hor - 0.24 * hor;
        if y < crater + 4.0 * s {
            let up = (crater - y) / hor;
            let center = fx - up * 0.45 * w;
            let width = (0.02 + 0.22 * up) * w;
            let plume = smoothstep(1.0, 0.4, (x - center).abs() / width) * smoothstep(-0.02, 0.05, up) * smoothstep(0.25 + 0.6 * e, 0.0, up - 0.2 * e);
            if plume > 0.0 {
                let pulse = 0.6 + 0.4 * (self.wall * 2.3).sin().abs();
                let n = value_noise(x / (60.0 * s) + self.wall * 0.15, y / (60.0 * s) - self.wall * 0.3) * 0.5 + 0.5;
                let density = (plume * (0.55 + 0.45 * n)).clamp(0.0, 1.0) * (0.45 + 0.55 * e);
                let lit = Vec3::new(0.9, 0.35, 0.1) * (pulse * smoothstep(0.15, 0.0, up) * 0.6);
                c = c.lerp(Vec3::new(0.12, 0.1, 0.11) + lit, density);
            }
        }
        c.lerp(Vec3::new(0.14, 0.1, 0.1), ash * (1.0 - 0.4 * t))
    }

    /// Lo fino del cielo, exacto en cada pixel: las siluetas del Volcan de
    /// Agua (un cono ancho, centrado) y del Fuego con su crater encendido, y
    /// las estrellas que se ven cuando la ceniza apaga la tarde.
    fn volcanoes(&self, x: f32, y: f32, ash: f32, base: Vec3) -> Vec3 {
        let (agua, fuego) = self.cones(x);
        self.volcanoes_at(x, y, ash, base, agua, fuego)
    }

    /// Donde empiezan, en la columna `x`, el Volcan de Agua y el Fuego (la
    /// fila de su silueta): solo depende de la columna.
    fn cones(&self, x: f32) -> (f32, f32) {
        let (w, hor) = (self.width as f32, self.horizon);
        let agua = hor - 0.3 * hor * (1.0 - ((x - 0.5 * w) / (0.42 * w)).abs()).max(0.0).powf(1.15);
        let fuego = hor - 0.24 * hor * (1.0 - ((x - 0.86 * w) / (0.2 * w)).abs()).max(0.0).powf(1.3);
        (agua, fuego)
    }

    /// `volcanoes` con las siluetas de la columna ya sacadas.
    fn volcanoes_at(&self, x: f32, y: f32, ash: f32, base: Vec3, agua: f32, fuego: f32) -> Vec3 {
        let (w, hor, s) = (self.width as f32, self.horizon, self.s);
        let t = (y / hor).clamp(0.0, 1.2);
        if y <= agua.min(fuego) && (y >= hor || t >= 0.5 || ash <= 0.0) {
            return base;
        }
        let mut c = base;
        let dusk = |c: Vec3| c.lerp(Vec3::new(0.14, 0.1, 0.1), ash * (1.0 - 0.4 * t));
        if y > agua {
            let shade = 0.55 + 0.45 * (1.0 - (x - 0.5 * w).abs() / (0.42 * w));
            c = dusk(Vec3::new(0.22, 0.2, 0.36) * shade + Vec3::new(0.25, 0.12, 0.08) * ((y - agua) / (hor - agua + 1.0)).min(1.0) * 0.4);
        }
        let fx = 0.86 * w;
        if y > fuego && y > agua.min(fuego) {
            let e = self.eruption();
            let pulse = 0.6 + 0.4 * (self.wall * 2.3).sin().abs();
            let lava = (1.0 - ((x - fx) / (0.012 * w + (y - fuego) * 0.25)).abs()).max(0.0) * smoothstep(fuego + 30.0 * s, fuego, y);
            c = dusk(Vec3::new(0.17, 0.13, 0.22)) + Vec3::new(1.0, 0.35, 0.05) * lava * (0.5 + 0.5 * e) * pulse;
        }
        if y < hor && hash_u32((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663)) % 900 == 0 && t < 0.5 {
            c = c + Vec3::splat(0.4 * ash);
        }
        c
    }

    /// La altura de la casa que esta en `z` en un lado.
    fn house_top(side: i32, z: f32) -> f32 {
        let k = (z / HOUSE).floor() as i32;
        let h = hash_u32((k as u32).wrapping_mul(0x9E37_79B9) ^ (side as u32).wrapping_mul(0x85EB_CA6B));
        3.6 + 1.3 * ((h >> 8) as f32 / 16_777_216.0)
    }

    /// Una fachada colonial de Antigua: el repello de su color con manchas
    /// de humedad, el zocalo de otro color, las pilastras en las esquinas, la
    /// cornisa y el alero de teja (con su sombra), el zaguan de madera con
    /// arco y marco de piedra, las ventanas con marco, reja y macetas en el
    /// alfeizar, y el farol de hierro con su luz en la pared. Devuelve el
    /// color y cuanto emite.
    fn facade(side: i32, z: f32, y: f32, top: f32) -> (Vec3, f32) {
        let k = (z / HOUSE).floor() as i32;
        let u = z - k as f32 * HOUSE;
        let h = hash_u32((k as u32).wrapping_mul(0x9E37_79B9) ^ (side as u32).wrapping_mul(0x85EB_CA6B));
        let palette = [
            Vec3::new(0.88, 0.62, 0.22),
            Vec3::new(0.7, 0.26, 0.17),
            Vec3::new(0.24, 0.38, 0.64),
            Vec3::new(0.9, 0.87, 0.78),
            Vec3::new(0.44, 0.58, 0.36),
            Vec3::new(0.86, 0.5, 0.44),
            Vec3::new(0.8, 0.56, 0.16),
            Vec3::new(0.46, 0.62, 0.78),
        ];
        let base = palette[(h % palette.len() as u32) as usize];
        let trim = [Vec3::new(0.94, 0.9, 0.8), Vec3::new(0.6, 0.18, 0.12), Vec3::new(0.9, 0.78, 0.4)][((h >> 5) % 3) as usize];
        let plinth = [Vec3::new(0.55, 0.16, 0.12), Vec3::new(0.32, 0.3, 0.3), Vec3::new(0.62, 0.42, 0.16), Vec3::new(0.2, 0.3, 0.5)][((h >> 9) % 4) as usize];
        // El repello: grano y manchas de humedad que bajan del alero.
        let grain = 0.92 + 0.08 * value_noise(z * 2.3, y * 2.3);
        let near_top = smoothstep(top - 2.5, top - 0.6, y);
        let damp = if near_top > 0.0 { smoothstep(0.35, 0.8, value_noise(z * 1.2 + 7.0, y * 0.35)) * near_top * 0.22 } else { 0.0 };
        let mut c = base * (grain - damp);
        let mut glow = 0.0;
        // El alero de teja: canales curvos en hileras; abajo, su sombra.
        if y > top - 0.42 {
            let along = z * 4.0;
            let tile = along - along.floor();
            let crest = (PI * tile).sin();
            let row = ((top - y) / 0.14).floor() as i32;
            let tint = 0.82 + 0.18 * ((hash_u32((along.floor() as i32 as u32) ^ (row as u32).wrapping_mul(977)) >> 24) as f32 / 255.0);
            c = Vec3::new(0.56, 0.24, 0.13) * (0.55 + 0.45 * crest) * tint;
            if y < top - 0.34 {
                c = Vec3::new(0.18, 0.08, 0.05);
            }
            return (c, 0.0);
        }
        // La sombra del alero y la cornisa.
        let under = smoothstep(top - 1.0, top - 0.42, y);
        if (top - 0.66..top - 0.48).contains(&y) {
            c = trim * (0.9 + 0.1 * ((y - (top - 0.66)) / 0.18));
        } else if (top - 0.72..top - 0.66).contains(&y) {
            c = trim * 0.55;
        }
        // Las pilastras de las esquinas.
        let edge = u.min(HOUSE - u);
        if edge < 0.32 && y < top - 0.66 {
            c = trim * (0.06 + grain * 0.9);
            if edge > 0.27 {
                c = c * 0.7;
            }
        }
        // El zocalo, con su filo.
        if y < 0.7 && edge >= 0.32 {
            c = plinth * grain;
            if y > 0.64 {
                c = trim * 0.85;
            }
        }
        // El zaguan: puerta de tablones con clavos, arco de medio punto y
        // marco de piedra.
        let door = (u - 4.5).abs();
        let arch_top = 2.25 + (1.0 - (door / 1.05).powi(2)).max(0.0).sqrt() * 0.62;
        if door < 1.25 && y < arch_top + 0.18 && y > 0.0 {
            if door < 1.05 && y < arch_top {
                let plank = ((u - 3.45) / 0.26).fract();
                c = Vec3::new(0.34, 0.18, 0.08) * (0.82 + 0.18 * (plank * PI).sin()) * grain;
                if ((y - 0.5) / 0.55).fract() < 0.06 && door > 0.1 {
                    c = Vec3::new(0.2, 0.1, 0.04);
                }
                let stud = (((u - 3.45) / 0.26 + 0.5).fract() - 0.5).abs() < 0.12 && (((y - 0.25) / 0.55).fract() - 0.5).abs() < 0.07;
                if stud {
                    c = Vec3::new(0.7, 0.6, 0.38);
                }
                if door < 0.02 {
                    c = Vec3::new(0.08, 0.04, 0.02);
                }
            } else {
                c = Vec3::new(0.85, 0.82, 0.74) * grain;
            }
        }
        // Las ventanas: marco de piedra, adentro oscuro (o con luz), la reja
        // con su remate y las macetas en el alfeizar.
        for (n, w0) in [1.0f32, 6.4].into_iter().enumerate() {
            let (wu, wv) = (u - w0, y - 1.05);
            if (-0.14..1.74).contains(&wu) && (-0.22..1.62).contains(&wv) {
                let inside = (0.0..1.6).contains(&wu) && (0.0..1.45).contains(&wv);
                if !inside {
                    c = Vec3::new(0.86, 0.83, 0.75) * grain;
                    if wv < 0.0 {
                        c = c * 0.8;
                    }
                } else {
                    // Encendida (la luz de adentro) o con las contraventanas
                    // de madera cerradas, de tablillas, verdes o cafes.
                    let lit = hash_u32(h ^ (n as u32 + 1).wrapping_mul(0x51ED)) % 3 == 0;
                    if lit {
                        c = Vec3::new(1.0, 0.68, 0.32) * (0.55 + 0.25 * wv / 1.45);
                        glow = 0.5;
                    } else {
                        let wood = if (h >> 13) % 2 == 0 { Vec3::new(0.2, 0.36, 0.26) } else { Vec3::new(0.36, 0.2, 0.1) };
                        let slat = 0.8 + 0.2 * ((wv / 0.09).fract() * PI).sin();
                        c = wood * slat * if (wu - 0.8).abs() < 0.02 { 0.4 } else { 1.0 };
                    }
                    // (La reja va adelante, volada: ver `grille`.)
                }
            }
            // Las macetas con flores sobre el alfeizar.
            if (0.1..1.5).contains(&wu) && (-0.18..0.16).contains(&wv) && hash_u32(h ^ n as u32) % 2 == 0 {
                let pot = ((wu - 0.1) / 0.35).floor();
                let pu = (wu - 0.1) / 0.35 - pot - 0.5;
                if wv < 0.0 && pu.abs() < 0.32 {
                    c = Vec3::new(0.62, 0.28, 0.14);
                } else if wv >= 0.0 && (pu * 1.2).hypot(wv - 0.06) < 0.22 {
                    let flower = [Vec3::new(0.95, 0.2, 0.4), Vec3::new(0.95, 0.75, 0.15), Vec3::new(0.85, 0.3, 0.9)][(hash_u32(h ^ pot as u32 * 31) % 3) as usize];
                    let leaf = value_noise(wu * 30.0, y * 30.0) > 0.0;
                    c = if leaf { Vec3::new(0.18, 0.45, 0.16) } else { flower };
                }
            }
        }
        // El farol de hierro (un cuerpo oscuro con su vidrio calido) y su luz
        // en la pared.
        let (lu, ly) = (u - 4.5, y - 3.15);
        let d2 = lu * lu + ly * ly * 1.4;
        c = c + Vec3::new(1.0, 0.62, 0.28) * (0.42 * fast_exp(-d2 / 1.1));
        if lu.abs() < 0.2 && (-0.3..0.3).contains(&ly) {
            let glass = lu.abs() < 0.13 && (-0.2..0.18).contains(&ly);
            c = if glass { Vec3::new(1.0, 0.82, 0.5) } else { Vec3::new(0.06, 0.06, 0.07) };
            glow = if glass { 1.0 } else { 0.0 };
        } else if lu.abs() < 0.04 && (0.3..0.55).contains(&ly) {
            c = Vec3::new(0.06, 0.06, 0.07);
        }
        // Sombra al pie y bajo el alero.
        let ao = (0.72 + 0.28 * smoothstep(0.0, 0.5, y)) * (1.0 - 0.35 * under);
        (c * ao, glow)
    }

    /// El empedrado: piedras redondeadas en hileras corridas, cada una con
    /// su tono, su relieve (la orilla se inclina: brilla del lado que da a
    /// la luz y se oscurece del otro) y algunas brillosas; la mezcla oscura
    /// entre ellas; al centro, la canaleta de losas. De lejos (un pixel ya
    /// mide varias piedras) se promedia.
    fn cobble(x: f32, z: f32, foot: f32) -> Vec3 {
        if x.abs() < 0.28 {
            // La canaleta: losas largas, un poco hundidas y humedas.
            let seam = ((z / 0.9).fract() < 0.03) as u8 as f32;
            let side = smoothstep(0.2, 0.28, x.abs());
            return Vec3::new(0.52, 0.49, 0.45) * (0.85 + 0.15 * value_noise(z * 2.0, x * 8.0)) * (1.0 - 0.3 * seam) * (1.0 - 0.2 * side);
        }
        let (cw, ch) = (0.34, 0.26);
        let row = (z / ch).floor();
        let shift = if (row as i32).rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let col = (x / cw + shift).floor();
        let h = hash_u32((col as i32 as u32).wrapping_mul(0x9E37_79B9) ^ (row as i32 as u32).wrapping_mul(0x85EB_CA6B));
        // (`v - floor(v)` y no `fract`, que con negativos da negativo.)
        let (qu, qv) = (x / cw + shift, z / ch);
        let (fu, fv) = (qu - qu.floor() - 0.5, qv - qv.floor() - 0.5);
        let jitter = ((h >> 4) & 0xFF) as f32 / 255.0 * 0.1;
        let d = lienzo::round_box(fu * cw, fv * ch, cw * (0.4 - jitter * 0.3), ch * (0.38 - jitter * 0.2), 0.07);
        let tone = 0.68 + 0.42 * ((h >> 12) & 0xFF) as f32 / 255.0;
        let hue = [Vec3::new(0.42, 0.37, 0.32), Vec3::new(0.36, 0.35, 0.34), Vec3::new(0.45, 0.38, 0.3)][(h % 3) as usize];
        // El relieve: cerca de la orilla, la normal se inclina hacia afuera;
        // la luz viene de adelante y un poco de la izquierda.
        let slope = smoothstep(-0.08, 0.0, d);
        let (nx, nz) = (fu * cw, fv * ch);
        let len = nx.hypot(nz).max(1e-4);
        let facing = (-0.35 * nx + 0.94 * nz) / len;
        let relief = 1.0 + 0.5 * slope * facing - 0.15 * slope;
        let shiny = if (h >> 20) % 5 == 0 { 0.12 * smoothstep(-0.02, -0.07, d) } else { 0.0 };
        let stone = hue * tone * relief + Vec3::new(0.9, 0.6, 0.4) * shiny;
        let mortar = Vec3::new(0.11, 0.095, 0.085);
        let fill = smoothstep(0.01 + foot, -0.01, d);
        let near = mortar.lerp(stone, fill);
        near.lerp(Vec3::new(0.29, 0.26, 0.23), smoothstep(0.04, 0.16, foot))
    }

    /// La banqueta: losas grandes con sus juntas y el bordillo de piedra
    /// (claro arriba, oscuro de lado).
    fn sidewalk(x: f32, z: f32) -> Vec3 {
        let a = x.abs();
        if a < STREET + 0.16 {
            let top = smoothstep(STREET + 0.02, STREET + 0.08, a);
            return Vec3::new(0.38, 0.35, 0.32).lerp(Vec3::new(0.68, 0.64, 0.58), top);
        }
        let k = (z / 0.9).floor();
        let h = hash_u32(k as i32 as u32 ^ if x < 0.0 { 0x51DE } else { 0x0 });
        let joint = ((z / 0.9).fract() < 0.035) as u8 as f32;
        Vec3::new(0.52, 0.48, 0.43) * (0.9 + 0.12 * ((h >> 24) as f32 / 255.0)) * (1.0 - 0.35 * joint) * (0.95 + 0.05 * value_noise(x * 6.0, z * 6.0))
    }

    /// El Arco de Santa Catalina, con su grueso: el muro amarillo de
    /// enfrente con la arcada, las molduras blancas, la hilera de ventanitas
    /// del pasillo de arriba y la torre del reloj; por dentro del arco, sus
    /// costados y la boveda (un cilindro `x^2 + (y - 4.2)^2 = R^2`), en
    /// sombra. Devuelve la profundidad y el color del primero que pega.
    fn arch(&self, cam: &Camera, rx: f32, ry: f32, best: f32) -> Option<(f32, Vec3, f32)> {
        let first = ((cam.pos.z - ARCH_DEPTH) / ARCH_EVERY).ceil();
        let mut hit: Option<(f32, Vec3, f32)> = None;
        let yellow = Vec3::new(0.95, 0.72, 0.22);
        let white = Vec3::new(0.95, 0.92, 0.84);
        let radius = STREET;
        for k in [first, first + 1.0] {
            let az = k * ARCH_EVERY;
            let limit = hit.map_or(best, |h| h.0);
            let t0 = az - cam.pos.z;
            if t0 > limit {
                continue;
            }
            let at = |t: f32| (cam.pos.x + rx * t, cam.pos.y - ry * t);
            // Enfrente: el muro, menos la arcada.
            if t0 > 0.05 {
                let (ax, ay) = at(t0);
                let opening = ax.abs() < radius && (ay < ARCH_SPRING || ax.hypot(ay - ARCH_SPRING) < radius);
                if ax.abs() < WALL + 0.7 && (0.0..8.4).contains(&ay) && !opening {
                    let rim = ax.hypot(ay - ARCH_SPRING) - radius;
                    let mut c = yellow * (0.92 + 0.08 * value_noise(ax * 2.0, ay * 2.0));
                    if (ay > ARCH_SPRING && rim < 0.38) || (ay <= ARCH_SPRING && ax.abs() - radius < 0.38) {
                        c = white * if rim < 0.06 && ay > ARCH_SPRING { 0.7 } else { 1.0 };
                    }
                    // Pilastras blancas, cornisas y el zocalo.
                    let pil = (ax.abs() - (radius + 0.75)).abs() < 0.22 || (ax.abs() - (WALL + 0.4)).abs() < 0.22;
                    if pil && ay < 7.7 {
                        c = white * 0.95;
                    }
                    if (4.95..5.2).contains(&ay) || (7.65..7.95).contains(&ay) {
                        c = white * if (ay - 4.95) < 0.05 || (ay - 7.65) < 0.05 { 0.7 } else { 1.0 };
                    }
                    if ay < 0.55 {
                        c = Vec3::new(0.62, 0.45, 0.2);
                    }
                    if ay > 7.95 {
                        c = white * 0.9;
                    }
                    // Las ventanitas en arco del pasillo de arriba.
                    let cell = (ax + 0.55) / 1.1;
                    let cu = cell - cell.floor() - 0.5;
                    let win = cu.abs() < 0.2 && (5.55..6.6).contains(&ay) || ((cu * 1.1).hypot(ay - 6.6) < 0.22 && ay >= 6.6);
                    if win && ax.abs() < WALL && ax.hypot(ay - ARCH_SPRING) > radius + 0.4 {
                        c = Vec3::new(0.95, 0.7, 0.35) * 0.55;
                        return Some((t0, c, 0.5));
                    }
                    hit = Some((t0, c, 0.0));
                    continue;
                }
                // La torre del reloj, un poco atras, arriba del muro.
                let tz = t0 + 0.8;
                let (tx, ty) = at(tz);
                let tower = tx.abs() < 1.8 && (8.4..11.0).contains(&ty);
                let roof = (11.0..12.6).contains(&ty) && tx.abs() < 1.9 * (12.6 - ty) / 1.6;
                let spire = tx.abs() < 0.06 && (12.6..13.4).contains(&ty) || (ty - 13.2).abs() < 0.05 && tx.abs() < 0.22;
                if tz < limit && (tower || roof || spire) {
                    let mut c = yellow * 0.97;
                    if roof {
                        c = Vec3::new(0.86, 0.84, 0.78) * (0.8 + 0.2 * ((12.6 - ty) / 1.6));
                    }
                    if spire {
                        c = Vec3::new(0.15, 0.13, 0.12);
                    }
                    if tower && (tx.abs() > 1.55 || (10.75..11.0).contains(&ty)) {
                        c = white;
                    }
                    let dial = tx.hypot(ty - 9.75);
                    if tower && dial < 0.72 {
                        c = if dial > 0.64 { Vec3::new(0.15, 0.12, 0.1) } else { white };
                        let hand = (self.wall * 0.2).sin_cos();
                        let (hx, hy) = (tx, ty - 9.75);
                        if (hx * hand.1 - hy * hand.0).abs() < 0.035 && (hx * hand.0 + hy * hand.1) > -0.05 || (hx.abs() < 0.035 && hy > 0.0 && hy < 0.42) {
                            c = Vec3::new(0.08, 0.07, 0.07);
                        }
                    }
                    hit = Some((tz, c, 0.0));
                    continue;
                }
            }
            // Por dentro: los costados (hasta donde arranca la boveda) y la
            // boveda, entre la cara de enfrente y la de atras.
            let inside = |t: f32| {
                let z = cam.pos.z + t;
                z >= az && z <= az + ARCH_DEPTH && t > 0.05 && t < limit
            };
            if rx.abs() > 1e-5 {
                let side = rx.signum();
                let t = (side * radius - cam.pos.x) / rx;
                let (_, wy) = at(t);
                if inside(t) && (0.0..ARCH_SPRING).contains(&wy) {
                    let band = ((cam.pos.z + t - az) / 0.75).fract() < 0.05;
                    let c = yellow * if band { 0.38 } else { 0.5 };
                    hit = Some((t, c, 0.0));
                    continue;
                }
            }
            let (ox, oy) = (cam.pos.x, cam.pos.y - ARCH_SPRING);
            let (a, b, cc) = (rx * rx + ry * ry, 2.0 * (ox * rx - oy * ry), ox * ox + oy * oy - radius * radius);
            let disc = b * b - 4.0 * a * cc;
            if disc > 0.0 {
                let t = (-b + disc.sqrt()) / (2.0 * a);
                let (vx, vy) = at(t);
                if inside(t) && vy >= ARCH_SPRING {
                    // La luz baja por la boveda: mas clara a los lados.
                    let shade = 0.32 + 0.22 * (vx.abs() / radius);
                    let rib = ((cam.pos.z + t - az) / 0.75).fract() < 0.05;
                    hit = Some((t, yellow * if rib { shade * 0.7 } else { shade }, 0.0));
                }
            }
        }
        hit
    }

    /// La alfombra de aserrin: orilla de colores, rombos y flores; pisada,
    /// se vuelve un manchon.
    fn carpet(o: &Obstacle, x: f32, z: f32, min_x: f32, max_x: f32) -> Vec3 {
        let (u, v) = ((x - min_x) / (max_x - min_x), (z - o.z) / o.len);
        let palette = [Vec3::new(0.45, 0.1, 0.55), Vec3::new(0.95, 0.75, 0.1), Vec3::new(0.85, 0.15, 0.12), Vec3::new(0.1, 0.55, 0.3), Vec3::new(0.95, 0.92, 0.85), Vec3::new(0.15, 0.35, 0.8)];
        let k = (o.seed % 6) as usize;
        let edge = u.min(1.0 - u).min(v.min(1.0 - v) * o.len / (max_x - min_x));
        let mut c = if edge < 0.04 {
            palette[k]
        } else if edge < 0.07 {
            palette[(k + 1) % 6]
        } else {
            let (gu, gv) = (u * 6.0, v * 4.0);
            let (cu, cv) = (gu.fract() - 0.5, gv.fract() - 0.5);
            let cell = (gu.floor() + gv.floor()) as i32;
            let diamond = cu.abs() + cv.abs();
            if diamond < 0.18 {
                palette[(k + 4) % 6]
            } else if diamond < 0.42 {
                palette[(k + 2 + cell.rem_euclid(2) as usize) % 6]
            } else {
                palette[(k + 5) % 6] * 0.9
            }
        };
        // El grano del aserrin.
        let grain = hash_u32(((x * 60.0) as i32 as u32).wrapping_mul(0x9E37) ^ ((z * 60.0) as i32 as u32).wrapping_mul(0x85EB));
        c = c * (0.85 + 0.25 * (grain >> 24) as f32 / 255.0);
        if o.stepped {
            let n = value_noise(x * 3.0, z * 3.0) * 0.5 + 0.5;
            c = c.lerp(Vec3::new(0.35, 0.27, 0.2), 0.45 + 0.4 * n);
        }
        c
    }

    /// El color de una cara de una caja: la camioneta (de espaldas con su
    /// escalera o de frente con sus focos), el tuk-tuk, la carreta y el lazo
    /// de papel picado. `face` es 0 la de atras (mira a la camara), 1 el
    /// techo, 2 los lados. `(a, b)` son las coordenadas en la cara (m).
    fn paint_box(o: &Obstacle, face: u8, a: f32, b: f32, width: f32) -> (Vec3, f32) {
        let palette = [Vec3::new(0.85, 0.12, 0.12), Vec3::new(0.1, 0.35, 0.8), Vec3::new(0.95, 0.75, 0.1), Vec3::new(0.1, 0.55, 0.3), Vec3::new(0.95, 0.95, 0.95)];
        let c1 = palette[(o.seed % 5) as usize];
        let c2 = palette[((o.seed >> 3) % 5) as usize];
        let c2 = if c2 == c1 { palette[((o.seed >> 3) as usize + 1) % 5] } else { c2 };
        let mut glow = 0.0;
        let c = match o.thing {
            Thing::Parked | Thing::Bus => match face {
                1 => {
                    // El techo: la parrilla y los bultos.
                    let rail = (a / width * 8.0).fract() < 0.08 || (b / 0.6).fract() < 0.06;
                    let bundle = hash_u32((b / 1.6) as u32 ^ o.seed) % 3 == 0 && (a / width - 0.5).abs() < 0.3;
                    if bundle {
                        palette[(hash_u32((b / 1.6) as u32 * 7 ^ o.seed) % 5) as usize] * 0.7
                    } else if rail {
                        Vec3::new(0.55, 0.55, 0.58)
                    } else {
                        Vec3::new(0.12, 0.12, 0.14)
                    }
                }
                2 => {
                    // Los lados: ventanas en hilera, franjas y las llantas.
                    let mut c = if b > 2.4 { c2 } else if b > 1.35 { Vec3::new(0.85, 0.85, 0.82) } else if b > 0.9 { c2 } else { c1 };
                    if (1.45..2.3).contains(&b) && (a / 0.95).fract() < 0.82 {
                        c = Vec3::new(0.08, 0.1, 0.14);
                    }
                    for wz in [1.6f32, 8.8] {
                        if b < 0.62 && ((a - wz).powi(2) + (b - 0.45).powi(2)).sqrt() < 0.5 {
                            c = Vec3::new(0.05, 0.05, 0.06);
                        }
                    }
                    c
                }
                _ => {
                    let u = a / width;
                    if o.thing == Thing::Parked {
                        // La de atras: ventana, calaveras, defensa y la
                        // escalera a la parrilla.
                        let mut c = if b > 2.45 { c2 } else if b > 0.9 { c1 } else { c2 * 0.9 };
                        if (1.5..2.3).contains(&b) && (0.12..0.58).contains(&u) {
                            c = Vec3::new(0.07, 0.09, 0.13) + Vec3::splat(0.12 * (1.0 - (b - 1.5) / 0.8));
                        }
                        if (0.55..0.75).contains(&b) && ((0.06..0.16).contains(&u) || (0.84..0.94).contains(&u)) {
                            c = Vec3::new(1.0, 0.15, 0.1);
                            glow = 0.8;
                        }
                        if (0.22..0.38).contains(&b) {
                            c = Vec3::new(0.7, 0.72, 0.75);
                        }
                        let ladder = (0.64..0.68).contains(&u) || (0.8..0.84).contains(&u) || ((0.64..0.84).contains(&u) && (b / 0.32).fract() < 0.12 && b > 0.4);
                        if ladder {
                            c = Vec3::new(0.62, 0.64, 0.68);
                        }
                        c
                    } else {
                        // La de frente: el parabrisas, el letrero, los
                        // focos y la parrilla del radiador.
                        let mut c = if b > 2.55 { c2 } else { c1 };
                        if (1.45..2.45).contains(&b) && (0.06..0.94).contains(&u) && (u - 0.5).abs() > 0.015 {
                            c = Vec3::new(0.08, 0.12, 0.18) + Vec3::splat(0.18 * ((u * 3.0 + b).fract() < 0.2) as u8 as f32);
                        }
                        if (2.55..2.85).contains(&b) && (0.2..0.8).contains(&u) {
                            c = Vec3::new(0.95, 0.85, 0.3);
                            if ((u * 40.0).fract() < 0.5) && (2.62..2.78).contains(&b) {
                                c = Vec3::new(0.1, 0.08, 0.05);
                            }
                        }
                        if (0.55..0.85).contains(&b) && ((0.08..0.2).contains(&u) || (0.8..0.92).contains(&u)) {
                            c = Vec3::new(1.0, 0.95, 0.8);
                            glow = 1.0;
                        }
                        if (0.3..1.0).contains(&b) && (0.3..0.7).contains(&u) {
                            c = if (b * 12.0).fract() < 0.5 { Vec3::new(0.75, 0.77, 0.8) } else { Vec3::new(0.2, 0.2, 0.22) };
                        }
                        c
                    }
                }
            },
            Thing::TukTuk => {
                // La lona negra con su ventanita, la carroceria con su
                // franja, las calaveras y la placa; de lado, las llantas.
                let u = a / width;
                if face == 1 {
                    Vec3::new(0.08, 0.08, 0.09)
                } else if face == 2 {
                    if b > 1.25 {
                        Vec3::new(0.08, 0.08, 0.09)
                    } else if b < 0.5 && ((a - 0.5).powi(2) + (b - 0.3).powi(2) < 0.09 || (a - 2.2).powi(2) + (b - 0.3).powi(2) < 0.09) {
                        Vec3::new(0.04, 0.04, 0.05)
                    } else {
                        c1
                    }
                } else if b > 1.25 {
                    if (1.42..1.78).contains(&b) && (0.22..0.78).contains(&u) {
                        Vec3::new(0.3, 0.33, 0.36)
                    } else {
                        Vec3::new(0.08, 0.08, 0.09)
                    }
                } else if (0.35..0.6).contains(&b) && (u < 0.16 || u > 0.84) {
                    glow = 0.6;
                    Vec3::new(1.0, 0.2, 0.12)
                } else if (0.38..0.56).contains(&b) && (0.35..0.65).contains(&u) {
                    Vec3::new(0.92, 0.92, 0.86)
                } else if (0.86..0.95).contains(&b) {
                    c2
                } else if b < 0.18 {
                    Vec3::new(0.15, 0.15, 0.16)
                } else {
                    c1
                }
            }
            Thing::Cart => {
                // La carreta de helados: blanca con franjas y su helado de
                // cucurucho pintado.
                let cone = face == 0 && (0.35..0.75).contains(&b) && (a - width * 0.5).abs() < (b - 0.35) * 0.3;
                let scoop = face == 0 && ((a - width * 0.5).powi(2) + (b - 0.86).powi(2)).sqrt() < 0.14;
                if face == 1 {
                    Vec3::new(0.85, 0.85, 0.88)
                } else if scoop {
                    Vec3::new(0.95, 0.55, 0.7)
                } else if cone {
                    Vec3::new(0.85, 0.6, 0.3)
                } else if (0.45..0.62).contains(&b) {
                    Vec3::new(0.95, 0.45, 0.6)
                } else if (0.68..0.8).contains(&b) {
                    Vec3::new(0.3, 0.6, 0.95)
                } else {
                    Vec3::new(0.95, 0.94, 0.9)
                }
            }
            Thing::Banner => {
                // Papel picado: un cuadro de color cada 0.5 m, con sus
                // recortes, colgando del lazo.
                let k = (a / 0.5).floor() as i32;
                let cut = ((a / 0.08).fract() - 0.5).abs() < 0.2 && ((b / 0.1).fract() - 0.5).abs() < 0.2;
                if b > 0.54 {
                    Vec3::new(0.85, 0.8, 0.7)
                } else if cut || (a / 0.5).fract() > 0.9 {
                    return (Vec3::new(-1.0, 0.0, 0.0), 0.0);
                } else {
                    palette[(k.rem_euclid(5)) as usize]
                }
            }
            Thing::Carpet | Thing::Ramp => Vec3::ZERO,
        };
        (c, glow)
    }
}

/// De la luz lineal al color de la pantalla, como al hornear los sprites:
/// `1 - e^(-x)` y la gamma 2.2, en una tabla (por pixel, `powf` costaba).
fn display(x: f32) -> f32 {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| (0..=1024).map(|i| (1.0 - (-(i as f32) / 128.0).exp()).powf(1.0 / 2.2)).collect());
    let k = (x.max(0.0) * 128.0).min(1023.999);
    let i = k as usize;
    table[i] + (table[i + 1] - table[i]) * (k - i as f32)
}

/// La fila `yf` de la bienvenida: la luz calida detras de Meyer y Meyer de
/// frente (premultiplicado) con los pies en `at`, sobre la imagen ya
/// terminada.
fn greet_row(sprite: &Sprite, at: (f32, f32), height: f32, yf: f32, row: &mut [u8], width: usize) {
    let center = (at.0, at.1 - 0.45 * sprite.height as f32);
    let reach = 0.42 * height;
    // Solo donde la luz se nota: `0.22 e^(-1.6 r^2) > 0.004` es `r < 1.6`.
    let (gx0, gx1) = (((center.0 - 1.6 * reach).max(0.0) as usize).min(width), ((center.0 + 1.6 * reach).max(0.0) as usize).min(width));
    for x in gx0..gx1 {
        let (dx, dy) = ((x as f32 + 0.5 - center.0) / reach, (yf - center.1) / (1.2 * reach));
        let glow = (-(dx * dx + dy * dy) * 1.6).exp() * 0.22;
        if glow > 0.004 {
            let p = &mut row[x * 4..x * 4 + 4];
            let c = lienzo::read(p) + Vec3::new(1.0, 0.72, 0.42) * glow;
            lienzo::write(p, c);
        }
    }
    let v = sprite.anchor.1 + (yf - at.1);
    if v < 0.0 || v >= sprite.height as f32 {
        return;
    }
    let x0 = ((at.0 - sprite.anchor.0).floor().max(0.0) as usize).min(width);
    let x1 = ((at.0 + sprite.width as f32 - sprite.anchor.0).ceil().max(0.0) as usize).min(width);
    for x in x0..x1 {
        let u = sprite.anchor.0 + (x as f32 + 0.5 - at.0);
        let s = sprite.sample(u, v);
        if s[3] <= 0.002 {
            continue;
        }
        let p = &mut row[x * 4..x * 4 + 4];
        let c = lienzo::read(p) * (1.0 - s[3]) + Vec3::new(s[0], s[1], s[2]) * Vec3::new(1.25, 1.12, 1.0);
        lienzo::write(p, c);
    }
}

/// El claxon de don Meyer: `assets/sonidos/claxon_meyer.wav` si esta; si
/// no, uno de corneta militar sintetizado (dos notas de cuadrada suavizada,
/// la segunda mas larga).
fn claxon() -> Vec<f32> {
    let dir = {
        let local = std::path::PathBuf::from("assets");
        if local.exists() { local } else { std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets") }
    };
    if let Some(samples) = sonido::load_wav(&dir.join("sonidos").join("claxon_meyer.wav"), SAMPLE_RATE) {
        return samples;
    }
    let rate = SAMPLE_RATE as f32;
    (0..(1.4 * rate) as usize)
        .map(|i| {
            let t = i as f32 / rate;
            let (f, start, end) = if t < 0.38 { (392.0, 0.0, 0.38) } else { (523.25, 0.42, 1.4) };
            if t < start {
                return 0.0;
            }
            let env = ((t - start) / 0.02).min(1.0) * ((end - t) / 0.08).clamp(0.0, 1.0);
            let wave = (TAU * f * t).sin() + 0.4 * (TAU * 2.0 * f * t).sin() + 0.2 * (TAU * 3.0 * f * t).sin();
            0.22 * env * wave
        })
        .collect()
}

/// Pega en la fila `yf` un sprite horneado (premultiplicado) con su ancla
/// en `at` (donde pisa, en la pantalla) a `scale` pixeles de pantalla por
/// pixel del sprite, detras de lo que este mas cerca que `depth`.
#[allow(clippy::too_many_arguments)]
fn blit(sprite: &Sprite, at: (f32, f32), scale: f32, depth: f32, light: f32, yf: f32, row: &mut [u8], depths: &[f32], width: usize) {
    if scale <= 0.0 {
        return;
    }
    let inv = 1.0 / scale;
    let v = sprite.anchor.1 + (yf - at.1) * inv;
    if v < 0.0 || v >= sprite.height as f32 {
        return;
    }
    let x0 = ((at.0 - sprite.anchor.0 * scale).floor().max(0.0) as usize).min(width);
    let x1 = ((at.0 + (sprite.width as f32 - sprite.anchor.0) * scale).ceil().max(0.0) as usize).min(width);
    for x in x0..x1 {
        if depths.get(x).is_some_and(|&z| z < depth) {
            continue;
        }
        let u = sprite.anchor.0 + (x as f32 + 0.5 - at.0) * inv;
        let s = sprite.sample(u, v);
        if s[3] <= 0.002 {
            continue;
        }
        let p = &mut row[x * 4..x * 4 + 4];
        let c = lienzo::read(p) * (1.0 - s[3]) + Vec3::new(s[0], s[1], s[2]) * light;
        write_emit(p, c, emission(p) * (1.0 - s[3]));
    }
}

/// Pega en la fila `yf` una nave de la flota: su color (con sus colores
/// permutados), la luz de sus LEDs encima, su neblina y detras de lo que
/// este mas cerca.
#[allow(clippy::too_many_arguments)]
fn ride(v: &Ride, lut: &Lut, light: f32, haze: Vec3, yf: f32, row: &mut [u8], depths: &[f32], width: usize) {
    let s = v.sprite;
    if v.scale <= 0.0 {
        return;
    }
    let inv = 1.0 / v.scale;
    let sy = s.anchor.1 + (yf - v.at.1) * inv;
    if sy < 0.0 || sy >= s.height as f32 {
        return;
    }
    let x0 = ((v.at.0 - s.anchor.0 * v.scale).floor().max(0.0) as usize).min(width);
    let x1 = ((v.at.0 + (s.width as f32 - s.anchor.0) * v.scale).ceil().max(0.0) as usize).min(width);
    let swap = |c: [f32; 3]| match v.paint {
        1 => [c[1], c[2], c[0]],
        2 => [c[2], c[0], c[1]],
        _ => c,
    };
    for x in x0..x1 {
        if depths.get(x).is_some_and(|&z| z < v.depth) {
            continue;
        }
        let sx = s.anchor.0 + (x as f32 + 0.5 - v.at.0) * inv;
        let t = s.sample_pick(&v.pick, sx, sy, lut);
        let a = t.body[3];
        if a <= 0.002 && t.light[0] + t.light[1] + t.light[2] <= 0.003 {
            continue;
        }
        let body = swap([t.body[0], t.body[1], t.body[2]]);
        let glow = swap(t.light);
        let led = Vec3::new(glow[0], glow[1], glow[2]);
        let src = Vec3::new(body[0], body[1], body[2]) * v.tint * light + led * 0.6;
        let src = src * (1.0 - v.fog) + haze * (a * v.fog);
        let p = &mut row[x * 4..x * 4 + 4];
        let under = emission(p) * (1.0 - a);
        let c = lienzo::read(p) * (1.0 - a) + src;
        write_emit(p, c, (under + led.max_element() * 0.45 * (1.0 - v.fog)).min(1.0));
    }
}

/// Prueba un rayo (origen `o`, direccion `(rx, -ry, 1)`) contra una caja:
/// la profundidad donde entra y por que cara (0 de frente, 1 arriba, 2 de
/// lado), o nada.
fn ray_box(o: Vec3, rx: f32, ry: f32, min: Vec3, max: Vec3) -> Option<(f32, u8)> {
    let (mut t0, mut t1, mut face) = (min.z - o.z, max.z - o.z, 0u8);
    if t1 < 0.0 {
        return None;
    }
    let dy = -ry;
    if dy.abs() < 1e-6 {
        if o.y < min.y || o.y > max.y {
            return None;
        }
    } else {
        let (a, b) = ((min.y - o.y) / dy, (max.y - o.y) / dy);
        let (lo, hi) = (a.min(b), a.max(b));
        if lo > t0 {
            t0 = lo;
            face = 1;
        }
        t1 = t1.min(hi);
    }
    if rx.abs() < 1e-6 {
        if o.x < min.x || o.x > max.x {
            return None;
        }
    } else {
        let (a, b) = ((min.x - o.x) / rx, (max.x - o.x) / rx);
        let (lo, hi) = (a.min(b), a.max(b));
        if lo > t0 {
            t0 = lo;
            face = 2;
        }
        t1 = t1.min(hi);
    }
    if t0 <= t1 && t0 > 0.05 { Some((t0, face)) } else { None }
}

impl TalegaSurfers {
    /// Si el punto `p` le da el sol: el rayo hacia el sol no choca con las
    /// casas de la derecha (pasa arriba del frente de su alero). Con un poco
    /// de penumbra.
    fn sunlit(p: Vec3) -> f32 {
        let t = (WALL - EAVE - p.x) / SUN_DIR.x;
        if t <= 0.0 {
            return 1.0;
        }
        let h = p.y + SUN_DIR.y * t;
        let top = Self::house_top(1, p.z + SUN_DIR.z * t);
        smoothstep(-0.12, 0.12, h - top)
    }

    /// La luz sobre un color: la del cielo (`sky`, la sombra) y la del sol
    /// segun lo que mira la cara (`ndl`) y si le da (`sun`).
    #[inline]
    fn lit(albedo: Vec3, ndl: f32, sun: f32, sky: f32) -> Vec3 {
        albedo * (SKY_LIGHT * sky + SUN * (ndl.max(0.0) * sun))
    }

    /// La reja volada de una ventana en `(u, v)` (u a lo largo de la casa
    /// desde donde empieza la ventana, v desde su piso): barrotes, dos
    /// travesanos y puntas de lanza arriba.
    fn grille(u: f32, v: f32) -> bool {
        let bar = (u / 0.16).fract() < 0.16;
        let cross = ((v + 0.08) / 0.5).fract() < 0.07 && v < 1.45;
        let tip = v > 1.45 && ((u / 0.16).fract() - 0.08).abs() < 0.1 * (1.62 - v) / 0.17;
        (bar && v < 1.45) || cross || tip || u < 0.03 || u > 1.57
    }

    /// El fondo detras de los techos, en el plano `x = side 24 m`: los
    /// arboles (algunos jacarandas moradas) y, cada tanto, una iglesia
    /// amarilla con su torre y su cupula. El color sin luz, o nada.
    fn backdrop(side: i32, z: f32, y: f32, top: f32) -> Option<(Vec3, f32)> {
        let k = (z / 150.0).floor();
        let u = z - k * 150.0;
        let h = hash_u32(k as i32 as u32 ^ (side as u32).wrapping_mul(0x9E37));
        if h % 2 == 0 {
            // La iglesia: la torre con su campanario, el remate y la cupula.
            let tower = (60.0..66.0).contains(&u) && y < 18.0;
            let cap = (60.4..65.6).contains(&u) && (18.0..21.0).contains(&y) && (u - 63.0).abs() < 2.6 * (21.0 - y) / 3.0 + 0.3;
            let dome = ((u - 71.0) / 5.0).hypot((y - 11.0) / 5.0) < 1.0 && y > 11.0;
            let drum = (66.5..75.5).contains(&u) && (7.0..11.0).contains(&y);
            let lantern = (70.2..71.8).contains(&u) && (15.8..17.6).contains(&y);
            if tower || cap || dome || drum || lantern {
                let yellow = Vec3::new(0.93, 0.7, 0.24);
                let white = Vec3::new(0.95, 0.92, 0.84);
                let mut c = if dome || cap { white * 0.92 } else { yellow };
                let bell = (61.6..64.4).contains(&u) && (13.5..16.5).contains(&y) && ((u - 63.0).abs() < 1.2 || y < 15.5);
                if tower && bell {
                    c = Vec3::new(0.08, 0.06, 0.05);
                }
                if tower && ((y - 13.0).abs() < 0.25 || (y - 17.6).abs() < 0.25) {
                    c = white;
                }
                return Some((c, if dome { 0.85 } else { 1.0 }));
            }
        }
        // Los arboles: la copa como un perfil de ruido en tres escalas.
        if y < top {
            let cell = (z / 7.0).floor();
            let jacaranda = hash_u32(cell as i32 as u32 ^ 0x1ACA) % 5 == 0;
            let leaf = 0.75 + 0.35 * value_noise(z * 2.2, y * 2.2);
            let c = if jacaranda { Vec3::new(0.55, 0.38, 0.78) } else { Vec3::new(0.2, 0.36, 0.15) } * leaf;
            // La copa de arriba agarra el sol.
            return Some((c, 0.7 + 0.5 * smoothstep(top - 2.0, top, y)));
        }
        None
    }

    /// Lo suave del cielo del frame a un cuarto de resolucion, en paralelo.
    fn sky_buffer(&self, ash: f32) -> SkyBuf {
        let (w, h) = (self.width / 4 + 2, self.height / 4 + 2);
        let mut px = vec![Vec3::ZERO; w * h];
        parallel::chunks_mut(&mut px, w, 1, |j, line| {
            for (i, p) in line.iter_mut().enumerate() {
                *p = self.sky_base(i as f32 * 4.0, j as f32 * 4.0, ash);
            }
        });
        SkyBuf { w, h, px }
    }

    /// El fondo de cada columna: el rayo de la columna `x` pega en el plano
    /// de los arboles (y en el de los cerros) siempre a la misma distancia,
    /// asi que su perfil se saca una vez por columna y no por pixel: la
    /// distancia, el `z` y lo alto de las copas (y de los cerros).
    fn backdrop_columns(&self, cam: &Camera) -> Vec<[f32; 8]> {
        (0..self.width)
            .map(|x| {
                let rx = (x as f32 + 0.5 - cam.cx) / cam.focal;
                let (agua, fuego) = self.cones(x as f32 + 0.5);
                if rx.abs() <= 1e-4 {
                    return [f32::MAX, 0.0, f32::MAX, f32::MAX, 0.0, f32::MAX, agua, fuego];
                }
                let s = rx.signum();
                let t1 = (s * BG_NEAR - cam.pos.x) / rx;
                let z1 = cam.pos.z + t1;
                let top = 8.0 + 2.6 * value_noise(z1 * 0.11, s * 7.0) + 1.4 * value_noise(z1 * 0.45, 3.0) + 0.6 * value_noise(z1 * 1.7, 9.0);
                let t2 = (s * BG_FAR - cam.pos.x) / rx;
                let z2 = cam.pos.z + t2;
                let hill = 24.0 + 16.0 * value_noise(z2 * 0.006, s * 3.0) + 5.0 * value_noise(z2 * 0.03, 1.0);
                [t1, z1, top, t2, z2, hill, agua, fuego]
            })
            .collect()
    }

    /// Una fila de la imagen: cada pixel con su rayo, su color con la luz
    /// (lo que emite va en el alfa, para el brillo) y su profundidad.
    #[allow(clippy::too_many_arguments)]
    fn row(&self, y: usize, row: &mut [u8], depth: &mut [f32], cam: &Camera, boxes: &[BoxHit], ash: f32, bg: &[[f32; 8]], sky: &SkyBuf) {
        let width = self.width;
        let yf = y as f32 + 0.5;
        let ry = (yf - cam.horizon) / cam.focal;
        let haze = self.sky(0.5 * width as f32, cam.horizon - 2.0, ash);
        let row_boxes: Vec<&BoxHit> = boxes.iter().filter(|b| yf >= b.y0 && yf <= b.y1).collect();
        // Lo del suelo que cae en esta fila (toda la fila de la calle esta a
        // la misma distancia): las alfombras, las sombras y el farol.
        let ground = self.ground_row(cam, ry);
        let lava_light = Vec3::new(1.0, 0.4, 0.15) * (0.08 * self.eruption());
        let next_arch = ((cam.pos.z - ARCH_DEPTH) / ARCH_EVERY).ceil() * ARCH_EVERY - cam.pos.z;
        let arch_near = next_arch < 200.0;
        // La ceniza tapa el sol y apaga el cielo.
        let sun_k = 1.0 - 0.8 * ash;
        let sky_k = 1.0 - 0.4 * ash;
        for x in 0..width {
            let xf = x as f32 + 0.5;
            let rx = (xf - cam.cx) / cam.focal;
            let mut best = f32::MAX;
            // Color ya con su luz, cuanto emite y cuanta neblina lleva.
            let mut color: Option<(Vec3, f32)> = None;
            let at = |t: f32| Vec3::new(cam.pos.x + rx * t, cam.pos.y - ry * t, cam.pos.z + t);
            // El suelo: la calle, la banqueta (15 cm mas alta) y el bordillo.
            if ry > 1e-4 {
                let ts = (cam.pos.y - CURB) / ry;
                let ps = at(ts);
                let tg = cam.pos.y / ry;
                let pg = at(tg);
                let (t, p, surface) = if ps.x.abs() >= STREET && ps.x.abs() <= WALL {
                    (ts, ps, 1)
                } else if pg.x.abs() < STREET {
                    (tg, pg, 0)
                } else if ps.x.abs() < STREET && rx.abs() > 1e-5 {
                    // Pasa la altura de la banqueta adentro de la calle y
                    // baja afuera: pega en el bordillo.
                    let tc = (pg.x.signum() * STREET - cam.pos.x) / rx;
                    (tc, at(tc), 2)
                } else {
                    (f32::MAX, pg, 3)
                };
                if surface < 3 && t > 0.0 {
                    best = t;
                    let foot = t / cam.focal;
                    let sun = Self::sunlit(p) * self.vehicle_light(p, &ground);
                    let (albedo, ndl) = match surface {
                        0 => {
                            let mut c = Self::cobble(p.x, p.z, foot);
                            for o in &ground.carpets {
                                let (min, max) = o.bounds();
                                if p.x > min.x && p.x < max.x {
                                    c = Self::carpet(o, p.x, p.z, min.x, max.x);
                                }
                            }
                            // La sombra de contacto de lo que hay en la calle.
                            for &(cx, hw) in &ground.shadows {
                                let e = (p.x - cx).abs() - hw;
                                c = c * (1.0 - 0.6 * smoothstep(0.35, -0.15, e));
                            }
                            // Junto al bordillo, un poco de sombra.
                            c = c * (1.0 - 0.3 * smoothstep(STREET - 0.3, STREET, p.x.abs()));
                            (c, SUN_DIR.y)
                        }
                        1 => (Self::sidewalk(p.x, p.z), SUN_DIR.y),
                        _ => (Vec3::new(0.58, 0.55, 0.5), if p.x < 0.0 { SUN_DIR.x } else { 0.0 }),
                    };
                    let mut c = Self::lit(albedo, ndl, sun * sun_k, sky_k * 0.95);
                    // El charco de luz del farol mas cercano, de cada lado.
                    for side in [-1.0f32, 1.0] {
                        let dx = p.x - side * WALL;
                        c = c + albedo * Vec3::new(1.0, 0.6, 0.28) * (0.9 * ground.lamp * fast_exp(-dx * dx / 4.0));
                    }
                    let mut glow: f32 = 0.0;
                    // Las sombras de las bombas que van a caer.
                    for b in self.bombs.iter().filter(|b| !b.done) {
                        let near = ((p.x - b.x).powi(2) + (p.z - b.z).powi(2)).sqrt();
                        let k = 1.0 - ((b.land - self.time) / WARNING).clamp(0.0, 1.0);
                        if near < 1.3 {
                            c = c * (1.0 - 0.75 * k * smoothstep(1.3, 0.6, near));
                            let ring = (near - 1.0 - 0.1 * (self.wall * 12.0).sin()).abs();
                            let r = 0.8 * k * smoothstep(0.08, 0.0, ring);
                            c = c + Vec3::new(1.0, 0.25, 0.05) * r;
                            glow = glow.max(r);
                        }
                    }
                    // Lo que ya cayo: una mancha de lava que se enfria.
                    for b in self.bombs.iter().filter(|b| b.done) {
                        let near = ((p.x - b.x).powi(2) + (p.z - b.z).powi(2)).sqrt();
                        let age = (self.time - b.land).max(0.0);
                        if near < 1.1 {
                            let hot = (1.0 - age / 1.2).max(0.0) * smoothstep(1.1, 0.3, near);
                            c = c.lerp(Vec3::new(1.4, 0.6, 0.12), hot);
                            glow = glow.max(hot);
                        }
                    }
                    color = Some((c, glow));
                }
            }
            // Las fachadas, con lo que sale de ellas: el alero (su frente de
            // teja y sus vigas por abajo), el tejadillo y la reja volada de
            // cada ventana, la pared, y lo hundido de las ventanas y del
            // zaguan (con sus jambas).
            if rx.abs() > 1e-5 {
                let s = if rx < 0.0 { -1.0f32 } else { 1.0 };
                let side = s as i32;
                let plane = |x: f32| (s * x - cam.pos.x) / rx;
                // Lo que mira cada cara: la pared y lo que da a la calle de la
                // izquierda mira hacia el sol; la de la derecha, no.
                let face_ndl = if side < 0 { SUN_DIR.x } else { 0.0 };
                // El rebote calido de la pared de enfrente, en la sombra.
                let bounce = if side > 0 { Vec3::new(0.16, 0.1, 0.05) } else { Vec3::ZERO };
                let wall_light = |albedo: Vec3, p: Vec3, sky: f32| {
                    let mut sun = Self::sunlit(p);
                    if side < 0 {
                        // La sombra de su propio alero.
                        let top = Self::house_top(side, p.z);
                        let reach = p.y + SUN_DIR.y * EAVE / SUN_DIR.x;
                        sun *= 1.0 - smoothstep(top - EAVE_T - 0.06, top - EAVE_T + 0.06, reach);
                    }
                    Self::lit(albedo, face_ndl, sun * sun_k, sky * sky_k) + albedo * bounce
                };
                let tf = plane(WALL - EAVE);
                if tf > 0.0 && tf < best {
                    let pf = at(tf);
                    let top = Self::house_top(side, pf.z);
                    if pf.y >= top - EAVE_T && pf.y < top {
                        // El frente del alero: tejas en canal.
                        let along = pf.z * 4.0;
                        let tile = along - along.floor();
                        let c = Vec3::new(0.6, 0.27, 0.14) * (0.6 + 0.4 * (PI * tile).sin());
                        let c = if pf.y < top - EAVE_T + 0.06 { c * 0.55 } else { c };
                        best = tf;
                        color = Some((wall_light(c, pf, 1.0), 0.0));
                    } else if pf.y < top - EAVE_T {
                        let tw = plane(WALL);
                        let pw = at(tw);
                        let top_w = Self::house_top(side, pw.z);
                        let eave_y = top_w - EAVE_T;
                        if pw.y >= eave_y && ry < 0.0 {
                            // Por abajo del alero: las vigas y las tablas, en
                            // sombra.
                            let tu = (cam.pos.y - eave_y) / ry;
                            if tu > 0.0 && tu < best {
                                let pu = at(tu);
                                let beam = ((pu.z / 0.55).fract() < 0.22) as u8 as f32;
                                let c = Vec3::new(0.32, 0.18, 0.09) * (0.8 - 0.35 * beam);
                                best = tu;
                                color = Some((Self::lit(c, 0.0, 0.0, 0.75 * sky_k) + c * bounce, 0.0));
                            }
                        } else if pw.y >= 0.0 && tw > 0.0 && tw < best {
                            let k = (pw.z / HOUSE).floor();
                            let u = pw.z - k * HOUSE;
                            let mut done = false;
                            // El tejadillo y la reja volada de cada ventana.
                            for &w0 in &WINDOWS {
                                let ta = plane(WALL - AWNING_OUT);
                                let pa = at(ta);
                                let ua = pa.z - k * HOUSE - w0;
                                if ta > 0.0 && ta < best && (-0.2..1.8).contains(&ua) && (WIN_HI + 0.12..WIN_HI + 0.34).contains(&pa.y) {
                                    let tile = (pa.z * 5.0).fract();
                                    let c = Vec3::new(0.58, 0.25, 0.13) * (0.6 + 0.4 * (PI * tile).sin());
                                    best = ta;
                                    color = Some((wall_light(c, pa, 1.0), 0.0));
                                    done = true;
                                    break;
                                }
                                let tg = plane(WALL - GRILLE);
                                let pg = at(tg);
                                let (ug, vg) = (pg.z - k * HOUSE - w0, pg.y - WIN_LO + 0.08);
                                if tg > 0.0 && tg < best && (0.0..1.6).contains(&ug) && (0.0..1.62).contains(&vg) && Self::grille(ug, vg) {
                                    let c = Vec3::new(0.07, 0.07, 0.08);
                                    best = tg;
                                    color = Some((wall_light(c, pg, 1.0) + Vec3::splat(0.05 * face_ndl), 0.0));
                                    done = true;
                                    break;
                                }
                            }
                            if !done {
                                // Lo hundido: ventanas y zaguan.
                                let window = WINDOWS.iter().find(|&&w0| (w0..w0 + 1.6).contains(&u) && (WIN_LO..WIN_HI).contains(&pw.y)).copied();
                                let door_dx = (u - 4.5).abs();
                                let arch_top = 2.25 + (1.0 - (door_dx / 1.05).powi(2)).max(0.0).sqrt() * 0.62;
                                let door = door_dx < 1.05 && pw.y < arch_top;
                                if window.is_some() || door {
                                    let depth_in = if door { DOOR_RECESS } else { RECESS };
                                    let tr = plane(WALL + depth_in);
                                    let pr = at(tr);
                                    let ur = pr.z - k * HOUSE;
                                    let inside = match window {
                                        Some(w0) => (w0..w0 + 1.6).contains(&ur) && (WIN_LO..WIN_HI).contains(&pr.y),
                                        None => (ur - 4.5).abs() < 1.05 && pr.y < 2.25 + (1.0 - ((ur - 4.5).abs() / 1.05).powi(2)).max(0.0).sqrt() * 0.62,
                                    };
                                    if inside {
                                        let top = Self::house_top(side, pr.z);
                                        let (c, glow) = Self::facade(side, pr.z, pr.y, top);
                                        best = tr;
                                        let lit = Self::lit(c, face_ndl * 0.5, Self::sunlit(pr) * sun_k * 0.6, 0.7 * sky_k) + c * bounce;
                                        color = Some((lit + c * (glow * 1.2), glow));
                                    } else {
                                        // La jamba: piedra, en sombra.
                                        let c = Vec3::new(0.82, 0.78, 0.7);
                                        best = tw;
                                        color = Some((Self::lit(c, 0.2 * face_ndl, Self::sunlit(pw) * sun_k * 0.5, 0.75 * sky_k) + c * bounce, 0.0));
                                    }
                                } else {
                                    let top = Self::house_top(side, pw.z);
                                    let (c, glow) = Self::facade(side, pw.z, pw.y, top);
                                    best = tw;
                                    color = Some((wall_light(c, pw, 1.0) + c * (glow * 1.2), glow));
                                }
                            }
                        }
                    }
                }
            }
            // El Arco de Santa Catalina (si hay uno a la vista).
            if arch_near && let Some((t, c, glow)) = self.arch(cam, rx, ry, best) {
                best = t;
                let p = at(t);
                color = Some((Self::lit(c, 0.45, Self::sunlit(p) * sun_k, sky_k) + c * (glow * 1.2), glow));
            }
            // Lo que hay en la calle (sin los modelos de las naves).
            for b in &row_boxes {
                if xf < b.x0 || xf > b.x1 {
                    continue;
                }
                let Some((t, face)) = ray_box(cam.pos, rx, ry, b.min, b.max) else { continue };
                if t >= best {
                    continue;
                }
                let o = &self.obstacles[b.index];
                let p = at(t);
                let width_m = b.max.x - b.min.x;
                let (a, bb) = match face {
                    0 => (p.x - b.min.x, p.y - b.min.y),
                    1 => (p.x - b.min.x, p.z - b.min.z),
                    _ => (p.z - b.min.z, p.y - b.min.y),
                };
                let (c, glow) = Self::paint_box(o, face, a, bb, width_m);
                if c.x < 0.0 {
                    continue;
                }
                best = t;
                let ndl = match face {
                    0 => -SUN_DIR.z,
                    1 => SUN_DIR.y,
                    _ => {
                        if p.x < cam.pos.x {
                            SUN_DIR.x
                        } else {
                            0.0
                        }
                    }
                };
                color = Some((Self::lit(c, ndl, Self::sunlit(p) * sun_k, sky_k) + c * glow, glow));
            }
            // Detras de los techos: los arboles y las iglesias, y los cerros.
            if color.is_none() && rx.abs() > 1e-4 {
                let s = if rx < 0.0 { -1.0f32 } else { 1.0 };
                let [t1, z1, top, t2, z2, hill, _, _] = bg[x];
                let y1 = cam.pos.y - ry * t1;
                if let Some((c, sun)) = Self::backdrop(s as i32, z1, y1, top) {
                    best = t1;
                    let ndl = if s < 0.0 { 0.75 } else { 0.25 };
                    color = Some((Self::lit(c, ndl, sun * sun_k, sky_k), 0.0));
                } else {
                    let y2 = cam.pos.y - ry * t2;
                    if y2 < hill {
                        best = t2;
                        let c = Vec3::new(0.24, 0.36, 0.22) * (0.9 + 0.15 * ((z2 * 0.08).sin() * (y2 * 0.1).cos()));
                        color = Some((Self::lit(c, 0.55, sun_k, sky_k), 0.0));
                    }
                }
            }
            let px = &mut row[x * 4..x * 4 + 4];
            let (c, e) = match color {
                Some((c, glow)) => {
                    let fog = 1.0 - fast_exp(-best / 120.0);
                    ((c + lava_light).lerp(haze, fog), glow * (1.0 - fog))
                }
                None => {
                    best = f32::MAX;
                    let [.., agua, fuego] = bg[x];
                    (self.volcanoes_at(xf, yf, ash, sky.sample(xf, yf), agua, fuego), 0.0)
                }
            };
            depth[x] = best;
            write_emit(px, c, e);
        }
    }

    /// Cuanto sol le deja a un punto del suelo lo que hay en la calle: el
    /// rayo hacia el sol contra las cajas de las naves y carretas cercanas
    /// (`ground.casters`).
    fn vehicle_light(&self, p: Vec3, ground: &GroundRow) -> f32 {
        for &(min, max) in &ground.casters {
            if hits_box(p, SUN_DIR, min, max) {
                return 0.15;
            }
        }
        1.0
    }

    /// Lo del suelo de la fila que mira `ry` abajo del horizonte: a que
    /// profundidad cae, las alfombras que la cruzan, las sombras de contacto
    /// (centro y medio ancho de cada cosa en la calle), lo que le puede
    /// hacer sombra (con el sol de lado, las sombras se alargan hacia
    /// adelante) y cuanta luz del farol mas cercano le llega.
    fn ground_row(&self, cam: &Camera, ry: f32) -> GroundRow<'_> {
        let mut g = GroundRow { carpets: Vec::new(), shadows: Vec::new(), casters: Vec::new(), lamp: 0.0 };
        if ry <= 1e-4 {
            return g;
        }
        let gz = cam.pos.z + cam.pos.y / ry;
        let reach = -SUN_DIR.z / SUN_DIR.y;
        for o in &self.obstacles {
            if o.thing == Thing::Carpet {
                if gz > o.z && gz < o.z + o.len {
                    g.carpets.push(o);
                }
                continue;
            }
            if o.thing == Thing::Banner {
                continue;
            }
            let (min, max) = o.bounds();
            let e = (gz - (min.z + max.z) * 0.5).abs() - (max.z - min.z) * 0.5;
            if e < 0.4 {
                let k = smoothstep(0.4, -0.2, e);
                g.shadows.push(((min.x + max.x) * 0.5, (max.x - min.x) * 0.5 * (0.6 + 0.4 * k)));
            }
            if gz > min.z - 0.5 && gz < max.z + reach * max.y + 0.5 {
                g.casters.push((min, max));
            }
        }
        let lamp = (gz - 4.5) / HOUSE;
        let d = (lamp - lamp.round()) * HOUSE;
        g.lamp = (-d * d / 6.0).exp();
        g
    }

    /// Las figuras de este frame: lenes, bombas que caen, chispas y humo, el
    /// patojo y, si lo persiguen, los cucuruchos.
    fn figures(&self, cam: &Camera) -> (Canvas, Canvas) {
        let mut c = Canvas::default();
        let s = self.s;
        let mut items: Vec<(f32, u8, usize)> = Vec::new();
        for (i, coin) in self.coins.iter().enumerate() {
            if !coin.taken {
                items.push((coin.z - cam.pos.z, 0, i));
            }
        }
        for (i, b) in self.bombs.iter().enumerate() {
            if !b.done {
                items.push((b.z - cam.pos.z, 1, i));
            }
        }
        for (i, p) in self.particles.iter().enumerate() {
            items.push((p.p.z - cam.pos.z, 2, i));
        }
        for (i, w) in self.walkers.iter().enumerate() {
            let near = w.z - cam.pos.z;
            if near > 0.8 && near < 90.0 {
                items.push((near, 5, i));
            }
        }
        for (i, pg) in self.pigeons.iter().enumerate() {
            let near = pg.p.z - cam.pos.z;
            if near > 0.5 && near < 60.0 {
                items.push((near, 7, i));
            }
        }
        // El papel picado de adorno, de alero a alero, cada 36 m.
        let first = ((cam.pos.z + 1.0) / 36.0).ceil() as i32;
        for k in first..first + 4 {
            let near = k as f32 * 36.0 + 18.0 - cam.pos.z;
            if near > 0.8 {
                items.push((near, 6, k as usize));
            }
        }
        for (i, o) in self.obstacles.iter().enumerate() {
            let near = o.z - cam.pos.z;
            if near > 0.3 && near < AHEAD + 20.0 {
                match o.thing {
                    Thing::Cart => items.push((near, 3, i)),
                    Thing::Banner => items.push((near, 4, i)),
                    Thing::Ramp => items.push((near, 8, i)),
                    _ => {}
                }
            }
        }
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, kind, i) in items {
            match kind {
                0 => {
                    let coin = self.coins[i];
                    let Some((x, y, k, d)) = cam.project(Vec3::new(coin.x, coin.y, coin.z)) else { continue };
                    c.depth = d;
                    let r = 0.2 * k;
                    let spin = (self.wall * 4.0 + coin.z * 0.7).cos().abs().max(0.15);
                    c.rect(x, y, r * spin, r, r * spin, Vec3::new(0.78, 0.48, 0.22));
                    c.rect(x, y, r * spin * 0.7, r * 0.7, r * spin * 0.7, Vec3::new(0.93, 0.66, 0.34));
                    c.disc(x - 0.3 * r * spin, y - 0.3 * r, 0.18 * r, Vec3::new(1.0, 0.92, 0.75)).alpha(0.8);
                }
                1 => {
                    // La bomba en el aire: de 40 m a 0 en `WARNING` segundos.
                    let b = self.bombs[i];
                    let f = 1.0 - ((b.land - self.time) / WARNING).clamp(0.0, 1.0);
                    let height = 40.0 * (1.0 - f * f);
                    let Some((x, y, k, d)) = cam.project(Vec3::new(b.x, height + 0.4, b.z)) else { continue };
                    c.depth = d;
                    if let Some((tx, ty, _, _)) = cam.project(Vec3::new(b.x, height + 3.5, b.z + 1.0)) {
                        c.line(x, y, tx, ty, 0.25 * k, Vec3::new(0.25, 0.2, 0.2)).alpha(0.5).soft(0.2 * k);
                    }
                    c.disc(x, y, 0.42 * k, Vec3::new(1.0, 0.5, 0.1)).glow(0.6 * k, 0.9);
                    c.disc(x, y, 0.25 * k, Vec3::new(1.0, 0.9, 0.6));
                }
                3 => self.cart(&mut c, cam, &self.obstacles[i]),
                8 => self.ramp(&mut c, cam, &self.obstacles[i]),
                4 => self.banner(&mut c, cam, self.obstacles[i].z, 0.0, self.obstacles[i].seed),
                5 => self.walker(&mut c, cam, &self.walkers[i]),
                7 => self.pigeon(&mut c, cam, &self.pigeons[i]),
                6 => self.banner(&mut c, cam, i as f32 * 36.0 + 18.0, 2.7, hash_u32(i as u32)),
                _ => {
                    let p = self.particles[i];
                    let Some((x, y, k, d)) = cam.project(p.p) else { continue };
                    c.depth = d;
                    let f = p.age / p.life;
                    match p.kind {
                        Puff::Lava => {
                            c.disc(x, y, (0.12 * k).clamp(1.5, 9.0 * s), Vec3::new(1.0, 0.55, 0.12)).alpha(1.0 - f).glow((0.2 * k).min(10.0 * s), 0.6);
                        }
                        Puff::Spark => {
                            c.disc(x, y, (0.06 * k).max(1.0), Vec3::new(1.0, 0.9, 0.6)).alpha(1.0 - f);
                        }
                        Puff::Dust => {
                            c.disc(x, y, (0.3 + 0.5 * f) * k, Vec3::new(0.6, 0.55, 0.5)).alpha(0.35 * (1.0 - f)).soft(0.25 * k);
                        }
                        Puff::Ash => {
                            c.disc(x, y, ((0.4 + 1.2 * f) * k).min(120.0 * s), Vec3::new(0.15, 0.13, 0.13)).alpha(0.6 * (1.0 - f)).soft((0.4 * k).min(40.0 * s));
                        }
                    }
                }
            }
        }
        // El patojo, de espaldas.
        let r = &self.run;
        let Some((x, ground, k, d)) = cam.project(Vec3::new(r.x, r.y, r.z)) else { return (c, Canvas::default()) };
        c.depth = d - 0.6;
        if let Some((sx, sy, sk, _)) = cam.project(Vec3::new(r.x, r.support, r.z)) {
            let lift = ((r.y - r.support) / 2.0).clamp(0.0, 1.0);
            c.rect(sx, sy, 0.38 * sk * (1.0 - 0.4 * lift), 0.08 * sk, 0.08 * sk, Vec3::ZERO).alpha(0.4 * (1.0 - 0.6 * lift)).soft(0.06 * sk);
        }
        c.depth = d;
        let skin = Vec3::new(0.66, 0.45, 0.32);
        let jersey = Vec3::new(0.25, 0.6, 0.9);
        let white = Vec3::new(0.95, 0.95, 0.95);
        let jeans = Vec3::new(0.15, 0.22, 0.4);
        let m = |v: f32| v * k;
        // Con Meyer, rueda con su guerrera beige y su gorra.
        let (jersey, cap) = if self.meyer.is_some() { (Vec3::new(0.66, 0.6, 0.45), Vec3::new(0.85, 0.82, 0.72)) } else { (jersey, Vec3::new(0.85, 0.12, 0.12)) };
        if r.roll > 0.0 && r.climb <= 0.0 && self.meyer.is_none() {
            // Sin Meyer, el patojo rueda hecho bolita.
            let cy = ground - m(0.42);
            c.disc(x, cy, m(0.42), jersey);
            let a = r.phase * 2.0;
            for j in 0..3 {
                let aj = a + j as f32 * TAU / 3.0;
                c.line(x + aj.cos() * m(0.1), cy + aj.sin() * m(0.1), x + aj.cos() * m(0.38), cy + aj.sin() * m(0.38), m(0.06), white);
            }
            c.disc(x, cy - m(0.28), m(0.14), skin);
            c.rect(x, cy - m(0.36), m(0.15), m(0.06), m(0.05), cap);
            return (c, self.chasers(Canvas::default(), cam));
        }
        if self.meyer.is_some() {
            // Meyer va en su sprite (`blit`), entre lo de atras y los
            // cucuruchos.
            return (c, self.chasers(Canvas::default(), cam));
        }
        let air = r.y > r.support + 0.05;
        let climbing = r.climb > 0.0;
        let swing = if air || climbing { 0.0 } else { r.phase.sin() };
        let hip = ground - m(0.92);
        let shoulder = ground - m(1.45);
        // Las piernas y los tenis.
        for side in [-1.0f32, 1.0] {
            let lift = if air { 0.35 } else { (0.28 * (r.phase + if side < 0.0 { 0.0 } else { PI }).sin()).max(0.0) };
            let foot = (x + side * m(0.13), ground - m(0.08 + lift));
            let knee = (x + side * m(0.15), hip + m(0.45) - m(lift * 0.8));
            c.line(x + side * m(0.1), hip, knee.0, knee.1, m(0.09), jeans);
            c.line(knee.0, knee.1, foot.0, foot.1, m(0.08), jeans);
            c.rect(foot.0, foot.1, m(0.09), m(0.06), m(0.04), white);
            c.rect(foot.0, foot.1 + m(0.03), m(0.09), m(0.025), m(0.02), Vec3::new(0.85, 0.15, 0.15));
        }
        // El cuerpo con la camisola celeste y blanco y el numero de atras.
        c.rect(x, ground - m(1.2), m(0.24), m(0.3), m(0.1), jersey);
        for dx in [-0.08f32, 0.08] {
            c.rect(x + m(dx), ground - m(1.2), m(0.035), m(0.29), m(0.02), white);
        }
        c.rect(x, ground - m(0.93), m(0.24), m(0.04), m(0.02), white);
        // El morral tipico de lado, con su cinta.
        c.line(x - m(0.18), shoulder + m(0.02), x + m(0.24), hip + m(0.05), m(0.03), Vec3::new(0.8, 0.15, 0.2));
        let bag = (x + m(0.3), hip + m(0.02));
        c.rect(bag.0, bag.1, m(0.11), m(0.13), m(0.03), Vec3::new(0.75, 0.12, 0.2));
        for (j, col) in [Vec3::new(0.95, 0.8, 0.1), Vec3::new(0.1, 0.55, 0.3), Vec3::new(0.15, 0.3, 0.8)].into_iter().enumerate() {
            c.rect(bag.0, bag.1 - m(0.07) + m(0.06 * j as f32), m(0.11), m(0.012), m(0.006), col);
        }
        // Los brazos.
        for side in [-1.0f32, 1.0] {
            let hand = if climbing {
                (x + side * m(0.22), shoulder - m(0.35))
            } else if air {
                (x + side * m(0.42), shoulder - m(0.1))
            } else {
                (x + side * m(0.3), shoulder + m(0.42) - m(0.18 * side * swing))
            };
            c.line(x + side * m(0.22), shoulder + m(0.04), hand.0, hand.1, m(0.065), jersey * 0.92);
            c.disc(hand.0, hand.1, m(0.06), skin);
        }
        // La cabeza con la gorra al reves.
        let head = ground - m(1.68);
        c.rect(x, head + m(0.13), m(0.06), m(0.06), m(0.03), skin * 0.85);
        c.disc(x, head, m(0.14), Vec3::new(0.08, 0.06, 0.05));
        c.disc(x, head + m(0.03), m(0.12), skin);
        c.disc(x, head - m(0.04), m(0.135), Vec3::new(0.85, 0.12, 0.12));
        c.rect(x, head + m(0.05), m(0.09), m(0.03), m(0.02), Vec3::new(0.7, 0.08, 0.1));
        let _ = s;
        (c, self.chasers(Canvas::default(), cam))
    }

    /// La carreta de helados: el cajon blanco con sus franjas y su helado
    /// pintado, las llantas, la tapa de lamina, la sombrilla de gajos de
    /// colores y las campanitas.
    /// El color de un pixel de una nave en 3D: la clase de cada pieza (la
    /// pintura con su brillo, el metal, el vidrio que refleja el cielo, el
    /// hule y las LEDs que emiten y corren), con la luz del cielo y la del
    /// sol de la tarde si le da (`sunlit`), la ceniza y la neblina. La
    /// normal se voltea hacia la camara (las mallas tienen caras abiertas) y
    /// la pintura de las camionetas sale permutando sus canales (tres
    /// pinturas, como los sprites).
    fn vehicle_shade(&self, f: &malla::Fragment, ash: f32, cam: &Camera, haze: Vec3) -> (Vec3, f32) {
        use crate::scene::Surface;
        let view = (f.world - cam.pos).normalize();
        let mut n = f.normal.normalize();
        if n.dot(view) > 0.0 {
            n = -n;
        }
        let paint = match self.obstacles.get(f.tag as usize) {
            Some(o) if o.thing != Thing::TukTuk => (o.seed % 3) as u8,
            _ => 0,
        };
        let albedo = match paint {
            1 => Vec3::new(f.color.y, f.color.z, f.color.x),
            2 => Vec3::new(f.color.z, f.color.x, f.color.y),
            _ => f.color,
        };
        let sun = Self::sunlit(f.world) * (1.0 - 0.8 * ash);
        let ndl = n.dot(SUN_DIR).max(0.0);
        let sky = SKY_LIGHT * (0.75 + 0.35 * n.y.max(0.0)) * (1.0 - 0.5 * ash);
        let r = view - n * (2.0 * view.dot(n));
        let reflection = Vec3::new(1.0, 0.62, 0.42).lerp(Vec3::new(0.3, 0.38, 0.62), r.y.clamp(0.0, 1.0)) * 0.8;
        let fresnel = 0.04 + 0.96 * (1.0 + view.dot(n)).clamp(0.0, 1.0).powi(5);
        let h = (SUN_DIR - view).normalize();
        let spec = |power: i32| n.dot(h).max(0.0).powi(power) * sun;
        let (lin, glow) = match Surface::from_code(f.class) {
            Surface::Light => {
                let pulse = 0.6 + 0.4 * (self.wall * 5.0 + f.world.z * 1.7 + f.world.y * 3.0).sin();
                (albedo * (0.4 + 2.6 * pulse), 0.75 * pulse)
            }
            Surface::Metal => (albedo * (sky * 1.3 + SUN * (ndl * sun) + reflection * 0.6) + SUN * (spec(40) * 1.4), 0.0),
            Surface::Glass => (albedo * 0.25 + reflection * (0.25 + 0.75 * fresnel) + SUN * (spec(90) * 1.2), 0.0),
            Surface::Rubber => (albedo * (sky + SUN * (ndl * sun * 0.5)), 0.0),
            Surface::Paint => {
                let gloss = (0.5 - f.value).clamp(0.0, 0.5);
                (albedo * (sky * 1.35 + SUN * (ndl * sun * 1.15)) + SUN * (spec(24) * (0.15 + gloss)) + reflection * (fresnel * 0.35), 0.0)
            }
        };
        let lin = lin * 1.25;
        let c = Vec3::new(display(lin.x), display(lin.y), display(lin.z)) * (1.0 - 0.3 * ash);
        let fog = 1.0 - fast_exp(-f.depth / 120.0);
        (c.lerp(haze, fog), glow * (1.0 - fog))
    }

    /// La rampa de tablones, de abajo hacia arriba en el orden en que se
    /// tapan: su sombra en el empedrado (proyectada con la direccion del
    /// sol), los caballetes que la sostienen (postes y una cruz, del lado que
    /// da a la camara), los dos largueros pintados de turquesa con su grueso,
    /// los tablones con sus rendijas, la veta y dos clavos en cada punta (cada
    /// uno de un tono de madera), la placa de metal del pie con franjas
    /// amarillas y negras en diagonal y el canto de metal de arriba. Todo con
    /// la luz del cielo y la del sol segun hacia donde mira cada cara y si le
    /// da.
    fn ramp(&self, c: &mut Canvas, cam: &Camera, o: &Obstacle) {
        let (min, max) = o.bounds();
        let x = (min.x + max.x) * 0.5;
        let hw = 1.0;
        // Lo que queda detras de la camara no se dibuja.
        let t0 = ((cam.pos.z + 0.4 - o.z) / o.len).clamp(0.0, 1.0);
        if t0 >= 1.0 {
            return;
        }
        let p = |side: f32, t: f32, dy: f32| cam.project(Vec3::new(x + side * hw, ROOF * t + dy, o.z + o.len * t));
        let ground = |gx: f32, gz: f32| cam.project(Vec3::new(gx, 0.0, gz));
        let Some((_, _, k0, d0)) = p(0.0, t0, 0.0) else { return };
        c.depth = d0;
        let quad = |c: &mut Canvas, q: [Option<(f32, f32, f32, f32)>; 4], col: Vec3| -> bool {
            let [Some(a), Some(b), Some(cc), Some(d)] = q else { return false };
            c.poly(&[(a.0, a.1), (b.0, b.1), (cc.0, cc.1), (d.0, d.1)], col);
            true
        };
        // La luz: el cielo y el sol segun la normal de cada cara.
        let sun = Self::sunlit(Vec3::new(x, 1.6, o.z + 0.5 * o.len));
        let lit = |n: Vec3| SKY_LIGHT * 1.15 + SUN * (0.8 * n.dot(SUN_DIR).max(0.0) * sun);
        let slope = (ROOF / o.len).atan();
        let top_light = lit(Vec3::new(0.0, slope.cos(), -slope.sin()));
        let side = if x > cam.pos.x { -1.0f32 } else { 1.0 };
        let side_light = lit(Vec3::new(side, 0.0, 0.0));
        // La sombra: cada punto alto cae `h / L.y` corrido contra el sol.
        let (sx, sz) = (-SUN_DIR.x / SUN_DIR.y, -SUN_DIR.z / SUN_DIR.y);
        let z1 = o.z + o.len;
        let foot_z = o.z.max(cam.pos.z + 0.4);
        let shadow = [ground(x - hw, foot_z), ground(x + hw, foot_z), ground(x + hw + sx * ROOF, z1 + sz * ROOF), ground(x - hw + sx * ROOF, z1 + sz * ROOF)];
        if let [Some(a), Some(b), Some(cc), Some(d)] = shadow {
            c.poly(&[(a.0, a.1), (b.0, b.1), (cc.0, cc.1), (d.0, d.1)], Vec3::ZERO).alpha(0.38 * sun).soft(0.15 * k0);
        }
        // Los caballetes: dos postes a cada lado y una cruz entre ellos.
        let post = Vec3::new(0.3, 0.2, 0.12) * side_light;
        for t in [0.48f32, 0.97] {
            if t <= t0 {
                continue;
            }
            for s in [-1.0f32, 1.0] {
                if let (Some(a), Some(b)) = (ground(x + s * (hw - 0.12), o.z + o.len * t), p(s * 0.88, t, -0.12)) {
                    c.line(a.0, a.1, b.0, b.1, (0.06 * a.2).max(0.7), post * if s == side { 1.0 } else { 0.6 });
                }
            }
        }
        if 0.48 > t0 {
            let near = |t: f32, dy: f32| cam.project(Vec3::new(x + side * (hw - 0.1), ROOF * t + dy, o.z + o.len * t));
            if let (Some(a), Some(b), Some(cc), Some(d)) = (near(0.48, -ROOF * 0.48 + 0.05), near(0.97, -0.15), near(0.48, -0.15), near(0.97, -ROOF * 0.97 + 0.05)) {
                c.line(a.0, a.1, b.0, b.1, (0.035 * a.2).max(0.6), post * 0.9);
                c.line(cc.0, cc.1, d.0, d.1, (0.035 * cc.2).max(0.6), post * 0.9);
            }
        }
        // Los largueros: la cara de lado (con su grueso) y la de arriba.
        let paint = Vec3::new(0.07, 0.42, 0.45);
        quad(c, [p(side, t0, 0.04), p(side, 1.0, 0.04), p(side, 1.0, -0.26), p(side, t0, -0.26)], paint * side_light * 0.9);
        // Debajo de los tablones, la sombra que se ve por las rendijas.
        quad(c, [p(-0.95, t0, -0.03), p(0.95, t0, -0.03), p(0.95, 1.0, -0.03), p(-0.95, 1.0, -0.03)], Vec3::new(0.06, 0.04, 0.03));
        // Los tablones, con rendijas entre ellos.
        let planks = 12;
        let tones = [Vec3::new(0.62, 0.42, 0.24), Vec3::new(0.55, 0.36, 0.2), Vec3::new(0.68, 0.48, 0.28), Vec3::new(0.5, 0.33, 0.19)];
        for i in 0..planks {
            let (ta, tb) = (i as f32 / planks as f32, (i as f32 + 0.86) / planks as f32);
            if tb <= t0 {
                continue;
            }
            let ta = ta.max(t0);
            let h = hash_u32(o.seed ^ (i as u32).wrapping_mul(0x9E37));
            let wood = tones[(h % 4) as usize] * (0.92 + 0.16 * ((h >> 4) % 64) as f32 / 64.0) * top_light;
            if !quad(c, [p(-0.92, ta, 0.0), p(0.92, ta, 0.0), p(0.92, tb, 0.0), p(-0.92, tb, 0.0)], wood) {
                continue;
            }
            // La veta y los clavos.
            let tm = 0.5 * (ta + tb);
            for (u, dv) in [(-0.6f32, 0.25f32), (0.15, 0.6), (0.55, 0.35)] {
                let tv = ta + (tb - ta) * dv;
                if let (Some(a), Some(b)) = (p(u - 0.3, tv, 0.0), p(u + 0.3, tv, 0.0)) {
                    c.line(a.0, a.1, b.0, b.1, (0.008 * a.2).max(0.4), wood * 0.72).alpha(0.6);
                }
            }
            for s in [-0.82f32, 0.82] {
                if let Some(nail) = p(s, tm, 0.005) {
                    c.disc(nail.0, nail.1, (0.025 * nail.2).max(0.6), Vec3::new(0.2, 0.2, 0.22) * top_light).gloss(0.6);
                }
            }
        }
        // Los largueros por arriba (encima de las puntas de los tablones).
        for s in [-1.0f32, 1.0] {
            quad(c, [p(s * 0.9, t0, 0.04), p(s * 1.02, t0, 0.04), p(s * 1.02, 1.0, 0.04), p(s * 0.9, 1.0, 0.04)], paint * top_light * if s == side { 1.0 } else { 0.85 });
        }
        // La placa del pie: franjas amarillas y negras en diagonal.
        if t0 < 0.04 {
            let n = 7;
            for s in 0..n {
                let (u0, u1) = (-0.98 + 1.96 * s as f32 / n as f32, -0.98 + 1.96 * (s + 1) as f32 / n as f32);
                let lean = 0.12;
                let col = if s % 2 == 0 { Vec3::new(0.98, 0.76, 0.08) } else { Vec3::new(0.05, 0.05, 0.05) };
                quad(c, [p(u0, 0.0, 0.0), p(u1, 0.0, 0.0), p(u1 + lean, 0.07, 0.0), p(u0 + lean, 0.07, 0.0)], col * top_light);
            }
            if let (Some(a), Some(b)) = (p(-0.98, 0.0, 0.0), p(0.98, 0.0, 0.0)) {
                c.line(a.0, a.1, b.0, b.1, (0.02 * a.2).max(0.6), Vec3::new(0.75, 0.76, 0.78) * top_light).gloss(0.8);
            }
        }
        // El canto de metal de arriba, donde empieza el techo.
        if let (Some(a), Some(b)) = (p(-0.98, 1.0, 0.0), p(0.98, 1.0, 0.0)) {
            c.line(a.0, a.1, b.0, b.1, (0.03 * a.2).max(0.6), Vec3::new(0.7, 0.72, 0.75) * top_light).gloss(0.8);
        }
    }

    fn cart(&self, c: &mut Canvas, cam: &Camera, o: &Obstacle) {
        let (min, max) = o.bounds();
        let x = (min.x + max.x) * 0.5;
        let z = o.z;
        let Some((sx, ground, k, d)) = cam.project(Vec3::new(x, 0.0, z)) else { return };
        c.depth = d;
        let m = |v: f32| v * k;
        let at = |dx: f32, dy: f32| (sx + m(dx), ground - m(dy));
        for side in [-1.0f32, 1.0] {
            let (wx, wy) = at(side * 0.58, 0.28);
            c.disc(wx, wy, m(0.28), Vec3::new(0.08, 0.08, 0.09));
            c.disc(wx, wy, m(0.1), Vec3::new(0.7, 0.7, 0.72));
        }
        let (bx, by) = at(0.0, 0.72);
        c.rect(bx, by, m(0.76), m(0.37), m(0.08), Vec3::new(0.95, 0.93, 0.88));
        let (sx1, sy1) = at(0.0, 0.62);
        c.rect(sx1, sy1, m(0.76), m(0.06), m(0.02), Vec3::new(0.95, 0.5, 0.65));
        let (sx2, sy2) = at(0.0, 0.5);
        c.rect(sx2, sy2, m(0.76), m(0.04), m(0.02), Vec3::new(0.35, 0.65, 0.95));
        let (cx, cy) = at(0.0, 0.84);
        c.poly(&[(cx - m(0.1), cy - m(0.06)), (cx + m(0.1), cy - m(0.06)), (cx, cy + m(0.2))], Vec3::new(0.85, 0.6, 0.3));
        c.disc(cx - m(0.05), cy - m(0.1), m(0.09), Vec3::new(0.95, 0.55, 0.7));
        c.disc(cx + m(0.05), cy - m(0.12), m(0.08), Vec3::new(0.6, 0.9, 0.7));
        let (lx, ly) = at(0.0, 1.12);
        c.rect(lx, ly, m(0.8), m(0.05), m(0.03), Vec3::new(0.75, 0.77, 0.8));
        // La sombrilla.
        let (px0, py0) = at(0.48, 1.12);
        let (px1, py1) = at(0.48, 2.15);
        c.line(px0, py0, px1, py1, m(0.025), Vec3::new(0.6, 0.6, 0.62));
        let colors = [Vec3::new(0.92, 0.2, 0.25), Vec3::new(0.97, 0.95, 0.9), Vec3::new(0.15, 0.55, 0.85), Vec3::new(0.97, 0.8, 0.2)];
        let apex = at(0.48, 2.42);
        let n = 8;
        for i in 0..n {
            let (a0, a1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let rim = |a: f32| at(0.48 + (a * 2.0 - 1.0) * 0.95, 2.05 + 0.12 * (1.0 - (a * 2.0 - 1.0).powi(2)));
            c.poly(&[apex, rim(a0), rim(a1)], colors[i % colors.len()] * (0.85 + 0.15 * (1.0 - (a0 * 2.0 - 1.0).abs())));
        }
        for j in 0..5 {
            let (bx, by) = at(0.48 - 0.76 + 0.38 * j as f32, 2.02);
            c.disc(bx, by, m(0.035), Vec3::new(0.95, 0.78, 0.3));
        }
    }

    /// El lazo de papel picado de pared a pared: el hilo combado y los
    /// papelitos de colores colgando, con sus recortes, meciendose.
    fn banner(&self, c: &mut Canvas, cam: &Camera, z: f32, lift: f32, seed: u32) {
        let Some((_, _, _, d)) = cam.project(Vec3::new(0.0, 1.8 + lift, z)) else { return };
        c.depth = d;
        let colors = [Vec3::new(0.92, 0.15, 0.4), Vec3::new(0.98, 0.78, 0.1), Vec3::new(0.15, 0.7, 0.4), Vec3::new(0.2, 0.5, 0.95), Vec3::new(0.7, 0.3, 0.9), Vec3::new(1.0, 0.5, 0.15)];
        let sag = |x: f32| 1.9 + lift - 0.18 * (1.0 - (x / WALL).powi(2));
        let n = 16;
        let mut prev = cam.project(Vec3::new(-WALL, sag(-WALL), z));
        for i in 1..=n {
            let x = -WALL + 2.0 * WALL * i as f32 / n as f32;
            let next = cam.project(Vec3::new(x, sag(x), z));
            if let (Some(a), Some(b)) = (prev, next) {
                c.line(a.0, a.1, b.0, b.1, (0.012 * a.2).max(0.6), Vec3::new(0.85, 0.82, 0.75));
            }
            prev = next;
        }
        let mut x = -WALL + 0.3;
        let mut i = 0;
        while x < WALL - 0.2 {
            let sway = 0.04 * (self.wall * 3.0 + i as f32 * 1.3).sin();
            let top = sag(x) - 0.02;
            if let Some((fx, fy, k, _)) = cam.project(Vec3::new(x + sway, top, z)) {
                let (hw, hh) = (0.17 * k, 0.21 * k);
                let col = colors[(i + (seed as usize)) % colors.len()];
                c.rect(fx, fy + hh, hw, hh, 0.01 * k, col);
                // Los recortes: tres agujeritos y la orilla en piquitos.
                for j in 0..3 {
                    let (hx, hy) = (fx + (j as f32 - 1.0) * 0.09 * k, fy + hh * (0.8 + 0.25 * (j % 2) as f32));
                    c.disc(hx, hy, 0.025 * k, col * 0.35).alpha(0.8);
                }
                c.rect(fx, fy + 2.0 * hh, hw * 0.9, 0.012 * k, 0.006 * k, col * 0.6);
            }
            x += 0.44;
            i += 1;
        }
    }

    /// Alguien en la banqueta: la senora con su huipil, su corte y la
    /// canasta en la cabeza; el turista con su sombrero y su mochila; o el
    /// de los globos con su racimo de colores. Con su sombra larga si le da
    /// el sol.
    fn walker(&self, c: &mut Canvas, cam: &Camera, w: &Walker) {
        let Some((x, ground, k, d)) = cam.project(Vec3::new(w.x, CURB, w.z)) else { return };
        c.depth = d;
        let m = |v: f32| v * k;
        let step = (self.wall * 5.0 + (w.seed % 100) as f32).sin();
        let bob = step.abs() * m(0.03);
        let sun = Self::sunlit(Vec3::new(w.x, 0.8, w.z));
        let light = SKY_LIGHT * 1.15 + SUN * (0.55 * sun);
        let tint = |v: Vec3| v * light;
        if sun > 0.1 {
            // La sombra: hacia donde no esta el sol, alargada.
            if let Some((sx, sy, sk, _)) = cam.project(Vec3::new(w.x - 0.75, CURB, w.z + 0.3)) {
                c.rect(sx, sy, 0.6 * sk, 0.12 * sk, 0.12 * sk, Vec3::ZERO).alpha(0.3 * sun).soft(0.08 * sk);
            }
        }
        let skin = tint(Vec3::new(0.6, 0.4, 0.28));
        let h = hash_u32(w.seed);
        match w.kind {
            0 => {
                // El corte azul con rayas y el huipil de colores.
                let corte = tint(Vec3::new(0.12, 0.16, 0.38));
                c.rect(x, ground - m(0.42) - bob, m(0.2), m(0.42), m(0.05), corte);
                for j in 0..3 {
                    c.rect(x, ground - m(0.2 + 0.2 * j as f32) - bob, m(0.2), m(0.012), m(0.006), tint(Vec3::new(0.5, 0.55, 0.85)));
                }
                let huipil = tint([Vec3::new(0.85, 0.15, 0.25), Vec3::new(0.55, 0.2, 0.6), Vec3::new(0.9, 0.5, 0.1)][(h % 3) as usize]);
                c.rect(x, ground - m(1.05) - bob, m(0.23), m(0.24), m(0.08), huipil);
                for j in 0..3 {
                    let col = tint([Vec3::new(0.95, 0.85, 0.2), Vec3::new(0.1, 0.6, 0.35), Vec3::new(0.2, 0.4, 0.9)][j]);
                    c.rect(x, ground - m(1.12 - 0.07 * j as f32) - bob, m(0.2), m(0.018), m(0.008), col);
                }
                c.disc(x, ground - m(1.42) - bob, m(0.12), skin);
                c.rect(x, ground - m(1.5) - bob, m(0.13), m(0.06), m(0.05), tint(Vec3::new(0.05, 0.04, 0.04)));
                // La canasta con frutas.
                c.rect(x, ground - m(1.62) - bob, m(0.24), m(0.07), m(0.05), tint(Vec3::new(0.55, 0.38, 0.18)));
                for (j, col) in [Vec3::new(0.95, 0.3, 0.1), Vec3::new(0.95, 0.85, 0.2), Vec3::new(0.3, 0.7, 0.2)].into_iter().enumerate() {
                    c.disc(x + m(-0.12 + 0.12 * j as f32), ground - m(1.71) - bob, m(0.06), tint(col));
                }
            }
            1 => {
                // El turista: short, camisa clara, mochila y sombrero.
                c.line(x - m(0.08), ground - m(0.05) - bob, x - m(0.08), ground - m(0.75) - bob, m(0.06), skin);
                c.line(x + m(0.08), ground - m(0.05) - bob, x + m(0.08), ground - m(0.75) - bob, m(0.06), skin);
                c.rect(x, ground - m(0.68) - bob, m(0.17), m(0.12), m(0.04), tint(Vec3::new(0.62, 0.55, 0.4)));
                let shirt = tint([Vec3::new(0.92, 0.9, 0.85), Vec3::new(0.4, 0.65, 0.85), Vec3::new(0.85, 0.4, 0.35)][(h % 3) as usize]);
                c.rect(x, ground - m(1.06) - bob, m(0.2), m(0.27), m(0.07), shirt);
                c.rect(x + m(0.2), ground - m(1.05) - bob, m(0.08), m(0.17), m(0.04), tint(Vec3::new(0.25, 0.4, 0.3)));
                c.disc(x, ground - m(1.45) - bob, m(0.12), skin);
                c.rect(x, ground - m(1.54) - bob, m(0.24), m(0.025), m(0.02), tint(Vec3::new(0.85, 0.78, 0.55)));
                c.rect(x, ground - m(1.6) - bob, m(0.12), m(0.06), m(0.05), tint(Vec3::new(0.85, 0.78, 0.55)));
            }
            _ => {
                // El de los globos: el racimo arriba, con sus hilos.
                c.rect(x, ground - m(0.4) - bob, m(0.16), m(0.4), m(0.04), tint(Vec3::new(0.2, 0.2, 0.25)));
                c.rect(x, ground - m(1.05) - bob, m(0.21), m(0.27), m(0.07), tint(Vec3::new(0.75, 0.3, 0.2)));
                c.disc(x, ground - m(1.45) - bob, m(0.12), skin);
                c.rect(x, ground - m(1.56) - bob, m(0.2), m(0.03), m(0.02), tint(Vec3::new(0.85, 0.75, 0.45)));
                let pole = (x + m(0.25), ground - m(1.0) - bob);
                c.line(pole.0, pole.1, pole.0, pole.1 - m(1.1), m(0.025), tint(Vec3::new(0.5, 0.35, 0.2)));
                let colors = [Vec3::new(0.95, 0.2, 0.3), Vec3::new(0.2, 0.6, 0.95), Vec3::new(0.95, 0.85, 0.2), Vec3::new(0.3, 0.85, 0.4), Vec3::new(0.85, 0.35, 0.9), Vec3::new(1.0, 0.55, 0.15)];
                for j in 0..9u32 {
                    let a = j as f32 * 2.399;
                    let r = 0.32 * ((j as f32 + 0.5) / 9.0).sqrt();
                    let sway = 0.03 * (self.wall * 1.5 + j as f32).sin();
                    let (bx, by) = (pole.0 + m(r * a.cos() + sway), pole.1 - m(1.45 + r * a.sin()));
                    c.line(pole.0, pole.1 - m(1.1), bx, by + m(0.12), m(0.006), tint(Vec3::splat(0.8)));
                    c.disc(bx, by, m(0.13), tint(colors[j as usize % colors.len()]));
                    c.disc(bx - m(0.04), by - m(0.04), m(0.035), Vec3::splat(1.0)).alpha(0.6);
                }
            }
        }
    }

    /// Una paloma: el cuerpo gris con su cuello tornasol y la cabeza; en el
    /// suelo picotea, volando bate las alas.
    fn pigeon(&self, c: &mut Canvas, cam: &Camera, pg: &Pigeon) {
        let Some((x, y, k, d)) = cam.project(pg.p + Vec3::new(0.0, 0.12, 0.0)) else { return };
        c.depth = d;
        let m = |v: f32| v * k;
        let phase = self.wall * 3.0 + (pg.seed % 97) as f32;
        let body = Vec3::new(0.52, 0.53, 0.58);
        if pg.flying {
            let flap = (self.wall * 22.0 + (pg.seed % 13) as f32).sin();
            c.rect(x, y, m(0.14), m(0.06), m(0.05), body);
            for side in [-1.0f32, 1.0] {
                c.line(x, y, x + side * m(0.26), y - m(0.16) * flap, m(0.035), body * 1.1);
            }
            c.disc(x, y - m(0.06), m(0.045), Vec3::new(0.35, 0.36, 0.4));
        } else {
            // Picoteando: la cabeza baja y sube.
            let peck = (phase.sin() * 0.5 + 0.5).powi(4);
            c.rect(x, y, m(0.11), m(0.075), m(0.06), body);
            c.disc(x, y - m(0.05), m(0.05), Vec3::new(0.3, 0.45, 0.42));
            c.disc(x + m(0.06), y - m(0.09) + m(0.08) * peck, m(0.04), Vec3::new(0.38, 0.39, 0.43));
            if let Some((sx, sy, sk, _)) = cam.project(pg.p) {
                c.rect(sx, sy, 0.1 * sk, 0.025 * sk, 0.025 * sk, Vec3::ZERO).alpha(0.3).soft(0.02 * sk);
            }
        }
    }

    /// Los cucuruchos que persiguen: tunica morada, cordon blanco y el
    /// capirote, atras del patojo (mas cerca de la camara).
    fn chasers(&self, mut c: Canvas, cam: &Camera) -> Canvas {
        if self.chase <= 0.0 || self.screen != Screen::Running {
            return c;
        }
        let r = &self.run;
        let come = smoothstep(CHASE, CHASE - 0.6, self.chase) * smoothstep(0.0, 0.6, self.chase);
        for (j, side) in [(0, -1.0f32), (1, 1.0)] {
            let back = 3.0 - 0.6 * j as f32;
            let p = Vec3::new(r.x + side * 1.1, r.support.min(r.y), r.z - back);
            let Some((x, ground, k, d)) = cam.project(p) else { continue };
            c.depth = d;
            let bob = (self.wall * 9.0 + j as f32).sin().abs() * 0.06 * k;
            let base = ground + (1.0 - come) * 2.5 * k;
            let purple = Vec3::new(0.38, 0.12, 0.48);
            let m = |v: f32| v * k;
            c.rect(x, base - m(0.6) - bob, m(0.32), m(0.62), m(0.14), purple);
            c.rect(x, base - m(0.85) - bob, m(0.3), m(0.03), m(0.015), Vec3::new(0.95, 0.95, 0.92));
            c.disc(x, base - m(1.35) - bob, m(0.17), purple * 0.9);
            for (h, rad) in [(1.55f32, 0.13f32), (1.75, 0.09), (1.92, 0.05)] {
                c.disc(x, base - m(h) - bob, m(rad), purple * 0.85);
            }
            c.rect(x, base - m(1.12) - bob, m(0.28), m(0.06), m(0.03), Vec3::new(0.95, 0.95, 0.92));
            for arm in [-1.0f32, 1.0] {
                c.line(x + arm * m(0.28), base - m(1.0) - bob, x + arm * m(0.45), base - m(0.7) + bob, m(0.07), purple);
            }
        }
        c
    }
}

impl Minigame for TalegaSurfers {
    fn update(&mut self, dt: f64, keys: &Keys) {
        let dt = (dt as f32).min(0.05);
        self.wall += dt;
        let keys = if self.autopilot && self.screen == Screen::Running {
            self.auto_wait -= dt;
            if self.auto_wait <= 0.0 {
                self.auto_wait = 0.12;
                Keys { escape: keys.escape, ..self.auto_keys() }
            } else {
                Keys { escape: keys.escape, ..Keys::default() }
            }
        } else {
            *keys
        };
        // H: el claxon de don Meyer (vuelve a empezar si ya sonaba).
        if keys.horn {
            self.voices.retain(|v| v.sfx != Sfx::Meyer);
            self.pending.retain(|s| *s != Sfx::Meyer);
            self.pending.push(Sfx::Meyer);
        }
        match self.screen {
            Screen::Menu => {
                if keys.escape {
                    self.done = true;
                } else if keys.enter {
                    self.start();
                }
            }
            Screen::Running => {
                if keys.escape {
                    self.crash("Te saliste de la carrera");
                } else {
                    // En pasos chicos: a toda velocidad no se atraviesa nada.
                    let n = (dt / 0.01).ceil().max(1.0) as usize;
                    for i in 0..n {
                        let k = if i == 0 { keys } else { Keys::default() };
                        self.step(dt / n as f32, &k);
                        if self.screen != Screen::Running {
                            break;
                        }
                    }
                }
            }
            Screen::Over => {
                if keys.enter {
                    self.start();
                } else if keys.escape {
                    self.reset();
                    self.screen = Screen::Menu;
                }
            }
        }
        for p in &mut self.particles {
            p.p = p.p + p.v * dt;
            let fall = if p.kind == Puff::Ash { -0.5 } else { 9.8 };
            p.v.y -= fall * dt;
            p.age += dt;
        }
        self.particles.retain(|p| p.age < p.life);
    }

    fn audio(&mut self, out: &mut [f32]) {
        let rate = SAMPLE_RATE as f32;
        let claxon = Arc::clone(&self.claxon);
        for sfx in self.pending.drain(..) {
            self.voices.push(Voice { sfx, n: 0 });
        }
        let running = self.screen == Screen::Running;
        let speed = if running { self.run.v / TOP_SPEED } else { 0.0 };
        let e = self.eruption();
        let chased = running && self.chase > 0.0;
        for sample in out.iter_mut() {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            let n = (self.noise >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0;
            // El viento sube con la velocidad; el volcan retumba.
            self.wind += (n - self.wind) * 0.02;
            self.rumble = self.rumble * 0.998 + n * 0.002;
            let mut x = self.wind * 0.12 * speed + self.rumble * (0.4 + 2.5 * e) * running as u8 as f32;
            // La marcha de procesion cuando persiguen: bombo y metales graves.
            if chased {
                self.march += 1.0 / rate;
                let beat = self.march % 0.75;
                x += 0.35 * (-beat * 9.0).exp() * (TAU * 55.0 * beat).sin();
                let chord = [110.0f32, 130.8, 164.8];
                let horn: f32 = chord.iter().map(|f| if (f * self.march).fract() < 0.5 { 1.0 } else { -1.0 }).sum();
                x += 0.012 * horn;
            }
            for v in &mut self.voices {
                let t = v.n as f32 / rate;
                v.n += 1;
                x += match v.sfx {
                    Sfx::Step => n * 0.12 * (-t * 60.0).exp(),
                    Sfx::Jump => n * 0.15 * (-t * 8.0).exp() * (TAU * (300.0 + 900.0 * t) * t).sin().abs(),
                    Sfx::Land => n * 0.3 * (-t * 25.0).exp(),
                    Sfx::Coin => 0.12 * (-t * 10.0).exp() * (TAU * if t < 0.06 { 1318.0 } else { 1760.0 } * t).sin(),
                    Sfx::Crash => (n * 0.6 + 0.4 * (TAU * 60.0 * t).sin()) * (-t * 4.0).exp(),
                    Sfx::Horn => {
                        let f = if (t / 0.25) as u32 % 2 == 0 { 349.0 } else { 440.0 };
                        0.08 * (1.0 - t / 0.9).max(0.0) * if (f * t).fract() < 0.5 { 1.0 } else { -1.0 }
                    }
                    Sfx::Whistle => 0.05 * (TAU * (1400.0 - 700.0 * t / WARNING) * t).sin() * (t / WARNING).min(1.0),
                    Sfx::Boom => (n * 0.5 + 0.6 * (TAU * (50.0 - 20.0 * t) * t).sin()) * (-t * 3.0).exp(),
                    Sfx::Climb => n * 0.1 * ((t * 30.0).fract() < 0.3) as u8 as f32 * (t < 0.25) as u8 as f32,
                    Sfx::Flap => n * 0.16 * (0.5 + 0.5 * (TAU * 14.0 * t).sin()).powi(3) * (1.0 - t / 0.9).max(0.0),
                    Sfx::Meyer => claxon.get(v.n as usize - 1).copied().unwrap_or(0.0) * 0.9,
                };
            }
            self.voices.retain(|v| match v.sfx {
                Sfx::Meyer => (v.n as usize) < claxon.len(),
                Sfx::Whistle => (v.n as f32 / rate) < WARNING,
                _ => (v.n as f32 / rate) < 1.2,
            });
            *sample = x / (1.0 + 0.5 * x.abs());
        }
    }

    fn draw(&mut self, rgba: &mut [u8]) {
        let cam = self.camera();
        let width = self.width;
        // Las cajas de lo que hay en la calle y donde caen en la pantalla.
        let mut boxes = Vec::new();
        for (index, o) in self.obstacles.iter().enumerate() {
            // Con la flota, las camionetas y los tuk-tuks son sprites; las
            // carretas y el papel picado van siempre como figuras.
            let boxed = matches!(o.thing, Thing::Parked | Thing::Bus | Thing::TukTuk) && self.fleet.is_none();
            if !boxed {
                continue;
            }
            let (min, max) = o.bounds();
            if max.z < cam.pos.z + 0.3 || min.z - cam.pos.z > AHEAD + 20.0 {
                continue;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for k in 0..8 {
                let p = Vec3::new(if k & 1 == 0 { min.x } else { max.x }, if k & 2 == 0 { min.y } else { max.y }, (if k & 4 == 0 { min.z } else { max.z }).max(cam.pos.z + 0.3));
                if let Some((sx, sy, _, _)) = cam.project(p) {
                    x0 = x0.min(sx);
                    x1 = x1.max(sx);
                    y0 = y0.min(sy);
                    y1 = y1.max(sy);
                }
            }
            boxes.push(BoxHit { index, min, max, x0: x0 - 1.0, x1: x1 + 1.0, y0: y0 - 1.0, y1: y1 + 1.0 });
        }
        let (figures, front) = self.figures(&cam);
        let ash = 0.55 * self.eruption();
        // Meyer: el cuadro de su carrera (o el del salto), donde pisa en la
        // pantalla, su escala y su profundidad.
        let r = self.run;
        let runner = match (&self.meyer, cam.project(Vec3::new(r.x, r.y, r.z))) {
            (Some(sprites), Some((x, ground, k, d))) => {
                let air = r.y > r.support + 0.05 || r.climb > 0.0;
                // Agachado (abajo), en el aire (el salto) o corriendo.
                let frame = if r.roll > 0.0 && r.climb <= 0.0 {
                    RUN_FRAMES + 1
                } else if air {
                    RUN_FRAMES
                } else {
                    ((r.phase / TAU).rem_euclid(1.0) * RUN_FRAMES as f32) as usize % RUN_FRAMES
                };
                sprites.get(frame).map(|s| (s, (x, ground), k / s.scale, d))
                    .map(|(s, at, scale, d)| (s, at, scale, d, Self::sunlit(Vec3::new(r.x, r.y + 1.0, r.z))))
            }
            _ => None,
        };
        let light = 1.0 - 0.45 * ash;
        // La flota: cada camioneta o tuk-tuk con el sprite del angulo con que
        // la ve la camara, su mip y sus colores; las que ya quedaron atras
        // del patojo se pintan despues de el.
        let mut rides: Vec<(Ride, bool)> = Vec::new();
        // Las de cerca van en 3D: su malla en perspectiva, con la camara del
        // juego (de cerca un sprite es una tarjeta: sin costado y corto
        // desde arriba).
        let has_meshes = self.meshes.is_some();
        let near3d = |o: &Obstacle| has_meshes && matches!(o.thing, Thing::Parked | Thing::Bus | Thing::TukTuk) && o.z - cam.pos.z < RASTER_DIST && o.z + o.len > cam.pos.z + 0.05;
        let mut rasters = std::mem::take(&mut self.rasters);
        let near: Vec<usize> = (0..self.obstacles.len()).filter(|&i| near3d(&self.obstacles[i])).collect();
        rasters.resize_with(near.len(), Raster::default);
        if let Some(meshes) = self.meshes.clone() {
            let view = malla::View { pos: cam.pos, focal: cam.focal, cx: cam.cx, horizon: cam.horizon };
            let (obstacles, height) = (&self.obstacles, self.height);
            parallel::chunks_mut(&mut rasters, 1, 1, |k, slot| {
                let raster = &mut slot[0];
                raster.clear(height);
                let index = near[k];
                let o = &obstacles[index];
                // Las paradas y los tuk-tuks de espaldas (media vuelta); las
                // que vienen, de frente.
                let (mesh, turn) = match o.thing {
                    Thing::Parked => (&meshes.bus, true),
                    Thing::Bus => (&meshes.bus, false),
                    _ => (&meshes.tuk, true),
                };
                let (min, max) = o.bounds();
                let center = Vec3::new((min.x + max.x) * 0.5, 0.0, o.z + o.len * 0.5);
                let place = |p: Vec3| if turn { Vec3::new(-p.x, p.y, -p.z) } else { p };
                for tri in &mesh.indices {
                    let v = tri.map(|i| {
                        let i = i as usize;
                        malla::Vertex { world: place(mesh.positions[i]) + center, normal: place(mesh.normals[i]), color: mesh.colors[i] }
                    });
                    let uv = mesh.uvs.get(tri[0] as usize).copied().unwrap_or_default();
                    raster.add(&view, v, uv.x, uv.y, index as u32);
                }
            });
        }
        self.rasters = rasters;
        if let Some(fleet) = &self.fleet {
            for o in &self.obstacles {
                if near3d(o) {
                    continue;
                }
                let (set, len) = match o.thing {
                    Thing::Parked => (&fleet.bus_back, fleet.bus.z),
                    Thing::Bus => (&fleet.bus_front, fleet.bus.z),
                    Thing::TukTuk => (&fleet.tuk_back, fleet.tuk.z),
                    _ => continue,
                };
                let (min, max) = o.bounds();
                let x = (min.x + max.x) * 0.5;
                let center = o.z + o.len * 0.5;
                let Some((sx, sy, k, d)) = cam.project(Vec3::new(x, 0.0, center)) else { continue };
                if d > AHEAD + 30.0 || o.z + o.len < cam.pos.z + 0.5 {
                    continue;
                }
                // Derechas en su carril, vistas desde la altura con que las
                // ve la camara (de cerca y desde los techos, el techo).
                let tilt = ((cam.pos.y - 1.6).max(0.0) / d.max(0.5)).atan();
                let Some(sprite) = set.get(Fleet::tilt_index(tilt)) else { continue };
                let scale = k / sprite.scale;
                let lod = ((1.0 / scale).log2() - 0.5).max(0.0);
                let near = (d - len * 0.5).max(0.3);
                let paint = if o.thing == Thing::TukTuk { 0 } else { (o.seed % 3) as u8 };
                let sun = Self::sunlit(Vec3::new(x, 2.2, center)) * (1.0 - 0.8 * ash);
                let tint = (SKY_LIGHT * 1.5 + SUN * (0.7 * sun)) * 0.85;
                let ride = Ride { sprite, pick: sprite.pick(lod), at: (sx, sy), scale, depth: near, paint, fog: 1.0 - (-d / 120.0).exp(), tint };
                let behind = o.z + o.len > r.z;
                rides.push((ride, behind));
            }
        }
        rides.sort_by(|a, b| b.0.depth.total_cmp(&a.0.depth));
        let (back_rides, front_rides): (Vec<Ride>, Vec<Ride>) = {
            let (b, f): (Vec<_>, Vec<_>) = rides.into_iter().partition(|r| r.1);
            (b.into_iter().map(|r| r.0).collect(), f.into_iter().map(|r| r.0).collect())
        };
        // Las LEDs de las camionetas: la persecucion de feria y la
        // respiracion (como en la carrera).
        let wall = self.wall;
        let lut: Lut = std::array::from_fn(|b| {
            let f = (b & 63) as f32;
            match b >> 6 {
                0 => 1.0,
                1 => 0.5 + 0.5 * smoothstep(0.55, 1.0, 0.5 + 0.5 * (TAU * (f / 64.0 * 4.0 - wall * 1.6)).sin()),
                2 => 0.7 + 0.3 * (wall * 2.1 + f * 0.1).sin(),
                _ => 0.6,
            }
        });
        let haze = self.sky(0.5 * width as f32, cam.horizon - 2.0, ash);
        let sun_k = 1.0 - 0.8 * ash;
        let bg = self.backdrop_columns(&cam);
        let sky = self.sky_buffer(ash);
        let mut sums = self.post.take_sums();
        let this = &*self;
        sums.paint(rgba, |y, row| {
            let yf = y as f32 + 0.5;
            let mut depth = vec![f32::MAX; width];
            this.row(y, row, &mut depth, &cam, &boxes, ash, &bg, &sky);
            // Las naves de cerca, en 3D, con su profundidad.
            for raster in &this.rasters {
                if !raster.is_empty() {
                    raster.row(y, width, &mut depth, |f| this.vehicle_shade(f, ash, &cam, haze), |x, c, e| write_emit(&mut row[x * 4..x * 4 + 4], c, e));
                }
            }
            // Lo de atras (lenes, bombas, carretas, papel picado) y las naves,
            // de lejos a cerca.
            let mut vi = 0;
            for (si, shape) in figures.shapes.iter().enumerate() {
                while vi < back_rides.len() && back_rides[vi].depth >= shape.depth {
                    ride(&back_rides[vi], &lut, light, haze, yf, row, &depth, width);
                    vi += 1;
                }
                paint(&figures.shapes[si..si + 1], yf, row, width, Some(&depth));
            }
            for v in &back_rides[vi..] {
                ride(v, &lut, light, haze, yf, row, &depth, width);
            }
            if let Some((sprite, at, scale, d, sun)) = runner {
                let k = (SKY_LIGHT.luminance() * 1.6 + SUN.luminance() * 0.55 * sun * sun_k) * light;
                blit(sprite, at, scale, d, k, yf, row, &depth, width);
            }
            for v in &front_rides {
                ride(v, &lut, light, haze, yf, row, &depth, width);
            }
            paint(&front.shapes, yf, row, width, Some(&depth));
        });
        // El brillo de lo que emite (faroles, ventanas, LEDs, lava), la
        // vineta, la gradacion, el grano y, con la velocidad, las lineas.
        let running = self.screen == Screen::Running;
        let params = posproceso::Params {
            speed: if running { ((self.run.v - START_SPEED) / (TOP_SPEED - START_SPEED)).clamp(0.0, 1.0) } else { 0.0 },
            wall: self.wall,
            fade: if running { 0.0 } else { 0.45 },
            ..Default::default()
        };
        self.post.prepare(sums, &params);
        // La bienvenida: Meyer de frente saludando, encima de la calle
        // oscurecida, con una luz calida detras.
        let greet = match (&self.greeter, self.screen) {
            (Some(sprite), Screen::Menu) => Some((sprite.as_ref(), 0.27 * width as f32, 0.99 * self.height as f32 + 3.0 * self.s * (self.wall * 1.7).sin())),
            _ => None,
        };
        let height = self.height as f32;
        self.post.compose(rgba, &params, |y, row| {
            if let Some((sprite, x, feet)) = greet {
                greet_row(sprite, (x, feet), height, y as f32 + 0.5, row, width);
            }
        });
    }

    fn overlay(&self) -> Overlay {
        let mut o = Overlay::default();
        let s = self.s;
        let w = self.width as f32;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 220];
        let accent = [255, 204, 120, 255];
        match self.screen {
            Screen::Menu => {
                // La bienvenida: Meyer a la izquierda (se dibuja con el
                // frame) y el panel a la derecha.
                let (pw, ph) = (600.0 * s, 470.0 * s);
                let (px, py) = (w - pw - 40.0 * s, 60.0 * s);
                let cx = px + pw * 0.5;
                o.panel(px, py, pw, ph);
                o.text("¡Bienvenido a", cx, py + 16.0 * s, Font::Heading, dim, Align::Center);
                o.text("Talega Surfers!", cx, py + 46.0 * s, Font::Big(52.0 * s), accent, Align::Center);
                o.text("Don Meyer te saluda. Antigua Guatemala, la calle del Arco,", cx, py + 112.0 * s, Font::Body, dim, Align::Center);
                o.text("y el Fuego explotando atrás.", cx, py + 136.0 * s, Font::Body, dim, Align::Center);
                let lines = [
                    "← →: carril   ·   ↑: saltar   ·   ↓: rodar",
                    "H: el claxon de don Meyer",
                    "Subite por las rampas y corré por los techos de las camionetas",
                    "Las que vienen de frente no perdonan; las carretas se saltan",
                    "Los lazos de papel picado se pasan rodando",
                    "No pisés las alfombras de aserrín: te persiguen los cucuruchos",
                    "Mirá la sombra de las bombas del Fuego antes de que caigan",
                ];
                for (i, line) in lines.iter().enumerate() {
                    o.text(*line, cx, py + 178.0 * s + i as f32 * 30.0 * s, Font::Body, cream, Align::Center);
                }
                let best = match self.best {
                    Some(b) => format!("Récord: {b} puntos"),
                    None => "Todavía no hay récord".to_string(),
                };
                o.text(best, cx, py + ph - 66.0 * s, Font::Bold, accent, Align::Center);
                o.text("Enter: correr   ·   Esc: salir", cx, py + ph - 34.0 * s, Font::Small, cream, Align::Center);
            }
            Screen::Running | Screen::Over => {
                o.panel(16.0 * s, 16.0 * s, 210.0 * s, 92.0 * s);
                o.text(format!("{} m", self.run.z as i64), 32.0 * s, 24.0 * s, Font::Heading, cream, Align::Left);
                o.text(format!("{} lenes", self.lenes), 210.0 * s, 26.0 * s, Font::Bold, accent, Align::Right);
                o.text(format!("{} puntos", self.score()), 32.0 * s, 62.0 * s, Font::Body, dim, Align::Left);
                if self.chase > 0.0 && self.screen == Screen::Running {
                    o.text("¡Te persiguen los cucuruchos!", w - 20.0 * s, 24.0 * s, Font::Heading, [220, 160, 255, 255], Align::Right);
                    o.bar(w - 260.0 * s, 58.0 * s, 240.0 * s, 8.0 * s, self.chase / CHASE, [170, 90, 220, 255], false);
                }
                if let Some((text, at)) = self.message
                    && self.wall - at < 2.0
                    && self.screen == Screen::Running
                {
                    o.text(text, w * 0.5, 0.56 * self.height as f32, Font::Bold, [255, 230, 200, 255], Align::Center);
                }
            }
        }
        if self.screen == Screen::Over {
            let (pw, ph) = (560.0 * s, 300.0 * s);
            let (px, py) = ((w - pw) * 0.5, 150.0 * s);
            o.panel(px, py, pw, ph);
            o.text(self.reason, w * 0.5, py + 18.0 * s, Font::Big(36.0 * s), accent, Align::Center);
            let lines = [format!("Corriste {} m", self.run.z as i64), format!("Lenes: {}", self.lenes), format!("Puntos: {}", self.score())];
            for (i, line) in lines.iter().enumerate() {
                o.text(line.clone(), w * 0.5, py + 86.0 * s + i as f32 * 34.0 * s, if i == 2 { Font::Heading } else { Font::Body }, if i == 2 { accent } else { cream }, Align::Center);
            }
            let best = self.best.map_or(String::new(), |b| format!("Récord: {b}"));
            o.text(best, w * 0.5, py + ph - 66.0 * s, Font::Bold, dim, Align::Center);
            o.text("Enter: otra vez   ·   Esc: al menú", w * 0.5, py + ph - 34.0 * s, Font::Small, cream, Align::Center);
        }
        o
    }

    fn done(&self) -> bool {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_for(g: &mut TalegaSurfers, seconds: f32) {
        let mut t = 0.0;
        while t < seconds && g.running() {
            g.update(1.0 / 60.0, &Keys::default());
            t += 1.0 / 60.0;
        }
    }

    /// Una camioneta parada, una que viene, un tuk-tuk, una carreta y un lazo,
    /// en PNG (`cargo test --release dump_street -- --ignored --nocapture`).
    #[test]
    #[ignore = "guarda una foto"]
    fn dump_street() {
        let (w, h) = (1280, 720);
        let mut g = TalegaSurfers::new(w, h, Records::in_memory());
        g.start();
        g.obstacles.clear();
        g.coins.clear();
        g.gen_z = 1e9;
        let mut put = |thing: Thing, lane: usize, z: f32, len: f32, seed: u32| g.obstacles.push(Obstacle { thing, lanes: 1 << lane, z, len, v: 0.0, seed, stepped: false });
        // Una fila parada con su rampa en medio, una que viene de frente a la
        // izquierda y un tuk-tuk y una carreta a la derecha.
        put(Thing::Ramp, 1, 7.0, RAMP_LEN, 7);
        for b in 0..3 {
            put(Thing::Parked, 1, 7.0 + RAMP_LEN + b as f32 * (BUS_LEN + BUS_GAP), BUS_LEN, 7 + b);
        }
        for b in 0..2 {
            put(Thing::Bus, 0, 26.0 + b as f32 * (BUS_LEN + BUS_GAP), BUS_LEN, 12 + b);
        }
        put(Thing::TukTuk, 2, 12.0, 2.6, 3);
        put(Thing::Cart, 2, 30.0, 1.0, 5);
        g.obstacles.push(Obstacle { thing: Thing::Banner, lanes: 0b111, z: 40.0, len: 0.12, v: 0.0, seed: 9, stepped: false });
        let mut rgba = vec![0u8; w * h * 4];
        g.draw(&mut rgba);
        // Tambien subiendo la rampa y corriendo por los techos.
        for (name, until) in [("surfers_rampa.png", 7.5f32), ("surfers_techos.png", 16.0)] {
            while g.run.z < until && g.running() {
                g.update(1.0 / 60.0, &Keys::default());
            }
            let mut frame = vec![0u8; w * h * 4];
            g.draw(&mut frame);
            let path = std::env::temp_dir().join(name);
            crate::render::png::write_rgba(&path, w as u32, h as u32, &frame).expect("png");
            println!("{} y {:.2}", path.display(), g.run.y);
        }
        // A la par de una fila parada (en el carril de la derecha).
        let mut side = TalegaSurfers::new(w, h, Records::in_memory());
        side.start();
        side.obstacles.clear();
        side.coins.clear();
        side.gen_z = 1e9;
        side.run.lane = 2;
        side.run.x = lane_x(2);
        for b in 0..3 {
            side.obstacles.push(Obstacle { thing: Thing::Parked, lanes: 0b010, z: 4.0 + b as f32 * (BUS_LEN + BUS_GAP), len: BUS_LEN, v: 0.0, seed: 21 + b, stepped: false });
        }
        while side.run.z < 9.0 && side.running() {
            side.update(1.0 / 60.0, &Keys::default());
        }
        let mut frame = vec![0u8; w * h * 4];
        side.draw(&mut frame);
        crate::render::png::write_rgba(&std::env::temp_dir().join("surfers_a_la_par.png"), w as u32, h as u32, &frame).expect("png");
        let path = std::env::temp_dir().join("surfers_calle.png");
        crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
        println!("{}", path.display());
    }

    /// Cuanto cuesta un frame y sus partes (`cargo test --release
    /// bench_surfers -- --ignored --nocapture`).
    #[test]
    #[ignore = "mide tiempos"]
    fn bench_surfers() {
        let (w, h) = (1280, 720);
        let mut g = TalegaSurfers::new(w, h, Records::in_memory());
        g.start();
        g.set_autopilot(true);
        run_for(&mut g, 14.0);
        let mut rgba = vec![0u8; w * h * 4];
        for _ in 0..3 {
            g.draw(&mut rgba);
        }
        let t = std::time::Instant::now();
        for _ in 0..20 {
            g.draw(&mut rgba);
        }
        println!("frame        {:6.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
        let cam = g.camera();
        let bg = g.backdrop_columns(&cam);
        let ash = 0.55 * g.eruption();
        let sky = g.sky_buffer(ash);
        let mut depth = vec![0.0f32; w];
        let t = std::time::Instant::now();
        for y in 0..h {
            let row = &mut rgba[y * w * 4..(y + 1) * w * 4];
            g.row(y, row, &mut depth, &cam, &[], ash, &bg, &sky);
        }
        let rows = t.elapsed().as_secs_f64() * 1000.0;
        println!("filas 1 hilo {rows:6.2} ms");
        for (name, y0, y1) in [("cielo", 0, h / 3), ("medio", h / 3, 2 * h / 3), ("suelo", 2 * h / 3, h)] {
            let t = std::time::Instant::now();
            for y in y0..y1 {
                let row = &mut rgba[y * w * 4..(y + 1) * w * 4];
                g.row(y, row, &mut depth, &cam, &[], ash, &bg, &sky);
            }
            println!("  {name:<6} {:6.2} ms", t.elapsed().as_secs_f64() * 1000.0);
        }
        let t = std::time::Instant::now();
        for _ in 0..10 {
            let _ = g.sky_buffer(ash);
        }
        println!("cielo buffer {:6.2} ms", t.elapsed().as_secs_f64() * 100.0);
        let params = posproceso::Params::default();
        let t = std::time::Instant::now();
        for _ in 0..10 {
            let mut sums = g.post.take_sums();
            sums.paint(&mut rgba, |_, _| {});
            g.post.prepare(sums, &params);
            g.post.compose(&mut rgba, &params, |_, _| {});
        }
        println!("posproceso   {:6.2} ms", t.elapsed().as_secs_f64() * 100.0);
        let t = std::time::Instant::now();
        let cols = g.backdrop_columns(&cam);
        println!("columnas     {:6.2} ms ({})", t.elapsed().as_secs_f64() * 1000.0, cols.len());
        let t = std::time::Instant::now();
        let (f, front) = g.figures(&cam);
        println!("figuras      {:6.2} ms ({} y {})", t.elapsed().as_secs_f64() * 1000.0, f.shapes.len(), front.shapes.len());
    }

    #[test]
    fn a_ray_enters_a_box_by_the_right_face() {
        let o = Vec3::new(0.0, 3.0, 0.0);
        let (min, max) = (Vec3::new(-1.0, 0.0, 10.0), Vec3::new(1.0, 2.0, 20.0));
        // De frente, a la altura de la caja.
        let (t, face) = ray_box(o, 0.0, 0.2, min, max).expect("pega");
        assert_eq!(face, 0);
        assert!((t - 10.0).abs() < 1e-4);
        // Mas abajo pega en el techo (el rayo baja 0.1 por metro).
        let (t, face) = ray_box(o, 0.0, 0.08, min, max).expect("pega");
        assert_eq!(face, 1);
        assert!(t > 10.0 && t < 20.0);
        // Al lado no pega.
        assert!(ray_box(o, 0.5, 0.2, min, max).is_none());
    }

    #[test]
    fn every_chunk_leaves_a_lane_free() {
        let mut g = TalegaSurfers::new(160, 90, Records::in_memory());
        for _ in 0..40 {
            g.gen_z += CHUNK;
            let z0 = g.gen_z;
            let before = g.obstacles.len();
            g.chunk(z0);
            let fresh = &g.obstacles[before..];
            let blocked = (0..3).filter(|&lane| fresh.iter().any(|o| o.lanes & (1 << lane) != 0 && matches!(o.thing, Thing::Bus | Thing::TukTuk))).count();
            assert!(blocked < 3);
        }
    }

    /// Saltar pasa la carreta; quedarse en el suelo choca. Subirse por atras
    /// a una camioneta parada deja al patojo en el techo.
    #[test]
    fn jumping_clears_the_cart_and_parked_buses_are_climbed() {
        let mut g = TalegaSurfers::new(160, 90, Records::in_memory());
        g.start();
        g.obstacles.clear();
        g.gen_z = 1e9;
        g.obstacles.push(Obstacle { thing: Thing::Cart, lanes: 0b010, z: 8.0, len: 1.0, v: 0.0, seed: 1, stepped: false });
        run_for(&mut g, 2.0);
        assert!(!g.running(), "sin saltar choca");
        assert!(g.reason.contains("carreta"));

        g.start();
        g.obstacles.clear();
        g.gen_z = 1e9;
        g.obstacles.push(Obstacle { thing: Thing::Cart, lanes: 0b010, z: 8.0, len: 1.0, v: 0.0, seed: 1, stepped: false });
        let mut jumped = false;
        let mut t = 0.0;
        while t < 2.0 && g.running() {
            let near = 8.0 - g.run.z;
            let keys = if !jumped && near < 3.5 { Keys { up: true, ..Keys::default() } } else { Keys::default() };
            jumped |= keys.up;
            g.update(1.0 / 60.0, &keys);
            t += 1.0 / 60.0;
        }
        assert!(g.running(), "saltando pasa: {}", g.reason);

        // Sin rampa, una parada choca; por la rampa se sube y se corre por
        // los techos de toda la fila.
        g.start();
        g.obstacles.clear();
        g.gen_z = 1e9;
        g.obstacles.push(Obstacle { thing: Thing::Parked, lanes: 0b010, z: 6.0, len: BUS_LEN, v: 0.0, seed: 3, stepped: false });
        run_for(&mut g, 1.0);
        assert!(!g.running(), "sin rampa choca");

        g.start();
        g.obstacles.clear();
        g.gen_z = 1e9;
        g.obstacles.push(Obstacle { thing: Thing::Ramp, lanes: 0b010, z: 6.0, len: RAMP_LEN, v: 0.0, seed: 3, stepped: false });
        for b in 0..3 {
            g.obstacles.push(Obstacle { thing: Thing::Parked, lanes: 0b010, z: 6.0 + RAMP_LEN + b as f32 * (BUS_LEN + BUS_GAP), len: BUS_LEN, v: 0.0, seed: 3, stepped: false });
        }
        run_for(&mut g, 1.0);
        assert!(g.running(), "{}", g.reason);
        assert!((g.run.y - ROOF).abs() < 0.05, "en el techo: {}", g.run.y);
        run_for(&mut g, 1.2);
        assert!(g.running(), "{}", g.reason);
        assert!((g.run.y - ROOF).abs() < 0.05, "sigue por los techos: {} en z {}", g.run.y, g.run.z);
    }

    #[test]
    fn a_second_carpet_while_chased_gets_you_caught() {
        let mut g = TalegaSurfers::new(160, 90, Records::in_memory());
        g.start();
        g.obstacles.clear();
        g.gen_z = 1e9;
        g.obstacles.push(Obstacle { thing: Thing::Carpet, lanes: 0b111, z: 5.0, len: 5.0, v: 0.0, seed: 1, stepped: false });
        g.obstacles.push(Obstacle { thing: Thing::Carpet, lanes: 0b111, z: 30.0, len: 5.0, v: 0.0, seed: 2, stepped: false });
        run_for(&mut g, 1.2);
        assert!(g.chase > 0.0 && g.running());
        run_for(&mut g, 2.5);
        assert!(!g.running());
        assert!(g.reason.contains("cucuruchos"));
    }

    /// Con el piloto automatico corre un buen rato y junta lenes.
    #[test]
    fn the_autopilot_runs_far() {
        let mut g = TalegaSurfers::new(160, 90, Records::in_memory());
        g.start();
        g.set_autopilot(true);
        run_for(&mut g, 60.0);
        let (meters, lenes, _) = g.tally();
        assert!(meters > 500.0, "corrio {meters} m ({})", g.reason);
        assert!(lenes > 10, "{lenes}");
    }

    #[test]
    fn every_screen_draws_and_its_text_has_glyphs() {
        let (w, h) = (320, 180);
        let mut g = TalegaSurfers::new(w, h, Records::in_memory());
        let mut rgba = vec![0u8; w * h * 4];
        let mut check = |g: &mut TalegaSurfers| {
            rgba.fill(0);
            g.draw(&mut rgba);
            assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
            super::super::assert_loaded_glyphs(&g.overlay());
        };
        check(&mut g);
        g.start();
        g.set_autopilot(true);
        run_for(&mut g, 20.0);
        g.chase = 3.0;
        check(&mut g);
        for reason in ["¡Te llevó la camioneta!", "¡Te agarraron los cucuruchos!", "¡Te cayó una bomba del Fuego!", "¡Te enredaste en el papel picado!", "¡Te comiste la carreta de helados!", "¡Te estrellaste con el tuk-tuk!"] {
            g.crash(reason);
            check(&mut g);
            g.start();
        }
    }
}
