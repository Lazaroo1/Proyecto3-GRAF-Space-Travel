//! Camioneta Surfers (Antigua Guatemala, version chapina): el patojo se baja
//! de la nave y corre por la calle del Arco, sobre el empedrado, en tres
//! carriles.
//!
//! - **Lo que hay en la calle**: camionetas paradas (con la escalera de
//!   atras se sube a la parrilla del techo y se corre encima), camionetas que
//!   vienen de frente (esas no perdonan), tuk-tuks parados, carretas de
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
use super::personaje::{self, RUN_FRAMES};
use std::sync::Arc;
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::math::{Vec3, hash_u32, smoothstep};
use crate::parallel;

/// Lo ancho de un carril, del empedrado (medio) y donde estan las fachadas.
const LANE: f32 = 2.2;
const STREET: f32 = 3.4;
const WALL: f32 = 4.4;
/// La camara: cuanto atras y cuanto arriba del patojo.
const CAM_BACK: f32 = 5.6;
const CAM_UP: f32 = 3.1;
/// Gravedad, velocidad del salto y lo que dura una rodada.
const GRAVITY: f32 = 26.0;
const JUMP: f32 = 8.6;
const ROLL: f32 = 0.62;
/// Lo alto del techo de una camioneta (sobre la parrilla se corre) y lo
/// largo de una camioneta y de un tuk-tuk.
const ROOF: f32 = flota::BUS_HEIGHT - 0.1;
const BUS_LEN: f32 = 8.0;
const TUK_LEN: f32 = 2.6;
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
        };
        (Vec3::new(x0 - half, y0, self.z), Vec3::new(x1 + half, y1, self.z + self.len))
    }

    fn solid(&self) -> bool {
        self.thing != Thing::Carpet
    }
}

fn lane_x(lane: usize) -> f32 {
    (lane as f32 - 1.0) * LANE
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

/// Lo del suelo de una fila (ver `CamionetaSurfers::ground_row`).
struct GroundRow<'a> {
    carpets: Vec<&'a Obstacle>,
    shadows: Vec<(f32, f32)>,
    lamp: f32,
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

pub struct CamionetaSurfers {
    width: usize,
    height: usize,
    s: f32,
    focal: f32,
    horizon: f32,
    screen: Screen,
    run: Runner,
    obstacles: Vec<Obstacle>,
    coins: Vec<Coin>,
    bombs: Vec<Bomb>,
    particles: Vec<Particle>,
    gen_z: f32,
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
    /// Las naves chapinas horneadas (camionetas y tuk-tuks), si estan los
    /// modelos; si no, cajas pintadas.
    fleet: Option<Arc<Fleet>>,
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

impl CamionetaSurfers {
    pub fn new(width: usize, height: usize, records: Records) -> Self {
        let best = records.get(RECORD);
        let mut game = Self {
            width,
            height,
            s: height as f32 / 720.0,
            focal: 0.95 * height as f32,
            horizon: 0.36 * height as f32,
            screen: Screen::Menu,
            run: Runner::new(),
            obstacles: Vec::new(),
            coins: Vec::new(),
            bombs: Vec::new(),
            particles: Vec::new(),
            gen_z: 26.0,
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
        // Las naves, a un decimo de la escala de la camara (de cerca se
        // agrandan poco; de lejos van los mips).
        game.fleet = flota::fleet((game.focal / 10.0).round());
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
        self.bombs.clear();
        self.particles.clear();
        self.gen_z = 26.0;
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
        let difficulty = (z0 / 1800.0).clamp(0.0, 1.0);
        let free = (self.random() * 3.0) as usize % 3;
        let pattern = (self.random() * 5.0) as usize % 5;
        let seed = hash_u32(z0 as u32 ^ 0x5EED);
        let push = |obs: &mut Vec<Obstacle>, thing: Thing, lanes: u8, z: f32, len: f32, v: f32, k: u32| {
            obs.push(Obstacle { thing, lanes, z, len, v, seed: hash_u32(seed ^ k.wrapping_mul(0x9E37)), stepped: false });
        };
        match pattern {
            0 => {
                // Camionetas: paradas (se suben) o de frente.
                let mut k = 0;
                for lane in 0..3 {
                    if lane == free {
                        continue;
                    }
                    if self.random() < 0.55 + 0.35 * difficulty {
                        let parked = self.random() < 0.6;
                        let at = z0 + 2.0 + self.random() * 8.0;
                        let (thing, v) = if parked { (Thing::Parked, 0.0) } else { (Thing::Bus, 7.0) };
                        push(&mut self.obstacles, thing, 1 << lane, at, BUS_LEN, v, k);
                        if parked && self.random() < 0.7 {
                            self.coin_line(lane_x(lane), ROOF + 0.9, at + 1.5, 6, 1.4);
                        }
                        k += 1;
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
        Camera { pos: Vec3::new(self.run.x * 0.6, CAM_UP + self.run.support * 0.75 + (self.run.y - self.run.support) * 0.3, self.run.z - CAM_BACK), focal: self.focal, cx: w * 0.5, horizon: self.horizon }
    }

    /// Lo que haria quien sabe: ir por el carril mas libre, saltar las
    /// carretas y las alfombras, rodar bajo los lazos y no estar donde cae
    /// una bomba.
    fn auto_keys(&self) -> Keys {
        let mut k = Keys::default();
        let r = &self.run;
        let reach = 14.0 + r.v * 0.6;
        // Que tan lejos esta lo primero que mata en cada carril.
        let danger = |lane: usize| -> f32 {
            let mut near = f32::MAX;
            for o in &self.obstacles {
                if o.lanes & (1 << lane) == 0 || !o.solid() || o.thing == Thing::Banner {
                    continue;
                }
                let ahead = o.z - r.z;
                if ahead + o.len < -0.5 || ahead > reach {
                    continue;
                }
                let deadly = match o.thing {
                    // Una parada se sube por atras si se llega de frente a su
                    // escalera.
                    Thing::Parked => lane != r.lane || r.y < ROOF - 0.4 && ahead < 0.5,
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
        let before_z = r.z;
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
            if matches!(o.thing, Thing::Parked | Thing::Bus) {
                let (min, max) = o.bounds();
                if r.x > min.x - 0.2 && r.x < max.x + 0.2 && r.z > min.z && r.z < max.z && r.y >= ROOF - 0.45 {
                    support = ROOF;
                }
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
        let mut climb = false;
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
                    let from_back = before_z + 0.3 <= min.z + 0.6 && (r.x - (min.x + max.x) * 0.5).abs() < 0.9;
                    if o.thing == Thing::Parked && from_back {
                        climb = true;
                    } else {
                        hit = Some(if o.thing == Thing::Bus { "¡Te llevó la camioneta!" } else { "¡Te estrellaste con la camioneta!" });
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
        if climb && self.run.climb <= 0.0 && self.run.y < ROOF - 0.45 {
            self.run.climb = 0.25;
            self.run.roll = 0.0;
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
        let (w, hor, s) = (self.width as f32, self.horizon, self.s);
        let t = (y / hor).clamp(0.0, 1.2);
        let mut c = Vec3::new(0.16, 0.12, 0.3).lerp(Vec3::new(0.98, 0.55, 0.32), t.powf(2.4));
        c = c + Vec3::new(1.0, 0.75, 0.4) * (0.25 * (-((x - 0.3 * w) / (0.25 * w)).powi(2)).exp() * t.powi(4));
        // El Volcan de Agua: un cono ancho, centrado.
        let agua = hor - 0.3 * hor * (1.0 - ((x - 0.5 * w) / (0.42 * w)).abs()).max(0.0).powf(1.15);
        if y > agua {
            let shade = 0.55 + 0.45 * (1.0 - (x - 0.5 * w).abs() / (0.42 * w));
            c = Vec3::new(0.22, 0.2, 0.36) * shade + Vec3::new(0.25, 0.12, 0.08) * ((y - agua) / (hor - agua + 1.0)).min(1.0) * 0.4;
        }
        // El Fuego, a la derecha, con su crater encendido.
        let fx = 0.86 * w;
        let fuego = hor - 0.24 * hor * (1.0 - ((x - fx) / (0.2 * w)).abs()).max(0.0).powf(1.3);
        let e = self.eruption();
        let pulse = 0.6 + 0.4 * (self.wall * 2.3).sin().abs();
        if y > fuego && y > agua.min(fuego) {
            c = Vec3::new(0.17, 0.13, 0.22);
            let lava = (1.0 - ((x - fx) / (0.012 * w + (y - fuego) * 0.25)).abs()).max(0.0) * smoothstep(fuego + 30.0 * s, fuego, y);
            c = c + Vec3::new(1.0, 0.35, 0.05) * lava * (0.5 + 0.5 * e) * pulse;
        }
        // El penacho: se ladea con el viento hacia la izquierda y crece con
        // la erupcion.
        let crater = hor - 0.24 * hor;
        if y < crater + 4.0 * s {
            let up = (crater - y) / hor;
            let center = fx - up * 0.45 * w;
            let width = (0.02 + 0.22 * up) * w;
            let n = value_noise(x / (60.0 * s) + self.wall * 0.15, y / (60.0 * s) - self.wall * 0.3) * 0.5 + 0.5;
            let plume = smoothstep(1.0, 0.4, (x - center).abs() / width) * smoothstep(-0.02, 0.05, up) * smoothstep(0.25 + 0.6 * e, 0.0, up - 0.2 * e);
            let density = (plume * (0.55 + 0.45 * n)).clamp(0.0, 1.0) * (0.45 + 0.55 * e);
            let lit = Vec3::new(0.9, 0.35, 0.1) * (pulse * smoothstep(0.15, 0.0, up) * 0.6);
            c = c.lerp(Vec3::new(0.12, 0.1, 0.11) + lit, density);
        }
        // La ceniza que tapa el cielo poco a poco.
        c = c.lerp(Vec3::new(0.14, 0.1, 0.1), ash * (1.0 - 0.4 * t));
        if y < hor && hash_u32((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663)) % 900 == 0 && t < 0.5 {
            c = c + Vec3::splat(0.4 * (1.0 - ash));
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
        let damp = smoothstep(0.35, 0.8, value_noise(z * 1.2 + 7.0, y * 0.35)) * smoothstep(top - 2.5, top - 0.6, y) * 0.22;
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
            c = trim * (0.88 + 0.06 * value_noise(z * 4.0, y * 2.0));
            if edge > 0.27 {
                c = c * 0.7;
            }
        }
        // El zocalo, con su filo.
        if y < 0.7 && edge >= 0.32 {
            c = plinth * (0.9 + 0.1 * value_noise(z * 3.0, y * 5.0));
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
                c = Vec3::new(0.34, 0.18, 0.08) * (0.82 + 0.18 * (plank * PI).sin()) * (0.9 + 0.1 * value_noise(z * 9.0, y * 0.8));
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
                c = Vec3::new(0.85, 0.82, 0.74) * (0.9 + 0.1 * value_noise(z * 6.0, y * 6.0));
            }
        }
        // Las ventanas: marco de piedra, adentro oscuro (o con luz), la reja
        // con su remate y las macetas en el alfeizar.
        for (n, w0) in [1.0f32, 6.4].into_iter().enumerate() {
            let (wu, wv) = (u - w0, y - 1.05);
            if (-0.14..1.74).contains(&wu) && (-0.22..1.62).contains(&wv) {
                let inside = (0.0..1.6).contains(&wu) && (0.0..1.45).contains(&wv);
                if !inside {
                    c = Vec3::new(0.86, 0.83, 0.75) * (0.92 + 0.08 * value_noise(z * 5.0, y * 5.0));
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
                    // La reja: barrotes, dos travesanos y las puntas de
                    // lanza arriba.
                    let bar = (wu / 0.16).fract() < 0.16;
                    let cross = (wv / 0.48).fract() < 0.06 && wv < 1.3;
                    let tip = wv > 1.3 && ((wu / 0.16).fract() - 0.08).abs() < 0.12 * (1.45 - wv) / 0.15;
                    if bar && wv < 1.3 || cross || tip {
                        c = Vec3::new(0.05, 0.05, 0.06);
                        glow = 0.0;
                    }
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
        c = c + Vec3::new(1.0, 0.62, 0.28) * (0.42 * (-d2 / 1.1).exp());
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
            return Vec3::new(0.4, 0.37, 0.33) * (0.85 + 0.15 * value_noise(z * 2.0, x * 8.0)) * (1.0 - 0.35 * seam) * (1.0 - 0.3 * side);
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
            Thing::Carpet => Vec3::ZERO,
        };
        (c, glow)
    }
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
        lienzo::write(p, c);
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
        let src = Vec3::new(body[0], body[1], body[2]) * light + Vec3::new(glow[0], glow[1], glow[2]) * 0.9;
        let src = src * (1.0 - v.fog) + haze * (a * v.fog);
        let p = &mut row[x * 4..x * 4 + 4];
        let c = lienzo::read(p) * (1.0 - a) + src;
        lienzo::write(p, c);
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

impl CamionetaSurfers {
    /// Una fila de la imagen: cada pixel con su rayo, y su profundidad.
    fn row(&self, y: usize, row: &mut [u8], depth: &mut [f32], cam: &Camera, boxes: &[BoxHit], ash: f32) {
        let width = self.width;
        let yf = y as f32 + 0.5;
        let ry = (yf - cam.horizon) / cam.focal;
        let haze = self.sky(0.5 * width as f32, cam.horizon - 2.0, ash);
        let row_boxes: Vec<&BoxHit> = boxes.iter().filter(|b| yf >= b.y0 && yf <= b.y1).collect();
        // Lo del suelo que cae en esta fila (toda la fila del suelo esta a
        // la misma distancia): las alfombras, las sombras y el farol.
        let ground = self.ground_row(cam, ry);
        let lava_light = Vec3::new(1.0, 0.4, 0.15) * (0.08 * self.eruption());
        let light = 1.0 - 0.45 * ash;
        for x in 0..width {
            let xf = x as f32 + 0.5;
            let rx = (xf - cam.cx) / cam.focal;
            let mut best = f32::MAX;
            let mut color: Option<(Vec3, f32)> = None;
            // El suelo: empedrado, banquetas y alfombras.
            if ry > 1e-4 {
                let t = cam.pos.y / ry;
                let (gx, gz) = (cam.pos.x + rx * t, cam.pos.z + t);
                if gx.abs() <= WALL {
                    best = t;
                    let foot = t / cam.focal;
                    let mut c = if gx.abs() < STREET { Self::cobble(gx, gz, foot) } else { Self::sidewalk(gx, gz) };
                    for o in &ground.carpets {
                        let (min, max) = o.bounds();
                        if gx > min.x && gx < max.x {
                            c = Self::carpet(o, gx, gz, min.x, max.x);
                        }
                    }
                    // La sombra de contacto de lo que hay en la calle.
                    for &(cx, hw) in &ground.shadows {
                        let e = (gx - cx).abs() - hw;
                        c = c * (1.0 - 0.55 * smoothstep(0.45, -0.15, e));
                    }
                    // El charco de luz del farol mas cercano, de cada lado.
                    for side in [-1.0f32, 1.0] {
                        let dx = gx - side * WALL;
                        c = c + Vec3::new(1.0, 0.6, 0.28) * (0.22 * ground.lamp * (-dx * dx / 5.0).exp());
                    }
                    // La sombra de la banqueta junto al bordillo.
                    c = c * (1.0 - 0.25 * smoothstep(STREET - 0.25, STREET, gx.abs()) * (gx.abs() < STREET) as u8 as f32);
                    // Las sombras de las bombas que van a caer.
                    for b in self.bombs.iter().filter(|b| !b.done) {
                        let near = ((gx - b.x).powi(2) + (gz - b.z).powi(2)).sqrt();
                        let k = 1.0 - ((b.land - self.time) / WARNING).clamp(0.0, 1.0);
                        if near < 1.3 {
                            c = c * (1.0 - 0.75 * k * smoothstep(1.3, 0.6, near));
                            let ring = (near - 1.0 - 0.1 * (self.wall * 12.0).sin()).abs();
                            c = c + Vec3::new(1.0, 0.25, 0.05) * (0.8 * k * smoothstep(0.08, 0.0, ring));
                        }
                    }
                    // Lo que ya cayo: una mancha de lava que se enfria.
                    for b in self.bombs.iter().filter(|b| b.done) {
                        let near = ((gx - b.x).powi(2) + (gz - b.z).powi(2)).sqrt();
                        let age = (self.time - b.land).max(0.0);
                        if near < 1.1 {
                            let hot = (1.0 - age / 1.2).max(0.0) * smoothstep(1.1, 0.3, near);
                            c = c.lerp(Vec3::new(1.0, 0.45, 0.1), hot);
                        }
                    }
                    color = Some((c, 0.0));
                }
            }
            // Las fachadas.
            if rx.abs() > 1e-5 {
                let side = if rx < 0.0 { -1 } else { 1 };
                let t = (side as f32 * WALL - cam.pos.x) / rx;
                if t > 0.0 && t < best {
                    let (wy, wz) = (cam.pos.y - ry * t, cam.pos.z + t);
                    let top = Self::house_top(side, wz);
                    if wy >= 0.0 && wy < top {
                        best = t;
                        let (c, glow) = Self::facade(side, wz, wy, top);
                        let lit = if side < 0 { 1.0 } else { 0.72 };
                        color = Some((c * lit, glow));
                    }
                }
            }
            // El Arco de Santa Catalina.
            if let Some((t, c, glow)) = self.arch(cam, rx, ry, best) {
                best = t;
                color = Some((c, glow));
            }
            // Lo que hay en la calle.
            for b in &row_boxes {
                if xf < b.x0 || xf > b.x1 {
                    continue;
                }
                let Some((t, face)) = ray_box(cam.pos, rx, ry, b.min, b.max) else { continue };
                if t >= best {
                    continue;
                }
                let o = &self.obstacles[b.index];
                let p = Vec3::new(cam.pos.x + rx * t, cam.pos.y - ry * t, cam.pos.z + t);
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
                let shade = match face {
                    0 => 0.86,
                    1 => 1.0,
                    _ => {
                        if (p.x - cam.pos.x) < 0.0 {
                            0.95
                        } else {
                            0.68
                        }
                    }
                };
                color = Some((c * shade, glow));
            }
            let px = &mut row[x * 4..x * 4 + 4];
            let c = match color {
                Some((c, glow)) => {
                    let fog = 1.0 - (-best / 110.0).exp();
                    let lit = c * (light + glow * 0.8) + lava_light;
                    lit.lerp(haze, fog)
                }
                None => {
                    best = f32::MAX;
                    self.sky(xf, yf, ash)
                }
            };
            depth[x] = best;
            lienzo::write(px, c);
        }
    }

    /// Lo del suelo de la fila que mira `ry` abajo del horizonte: a que
    /// profundidad cae, las alfombras que la cruzan, las sombras de contacto
    /// (centro y medio ancho de cada cosa en la calle) y cuanta luz del
    /// farol mas cercano le llega.
    fn ground_row(&self, cam: &Camera, ry: f32) -> GroundRow<'_> {
        let mut g = GroundRow { carpets: Vec::new(), shadows: Vec::new(), lamp: 0.0 };
        if ry <= 1e-4 {
            return g;
        }
        let gz = cam.pos.z + cam.pos.y / ry;
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
        for (i, o) in self.obstacles.iter().enumerate() {
            let near = o.z - cam.pos.z;
            if near > 0.3 && near < AHEAD + 20.0 {
                match o.thing {
                    Thing::Cart => items.push((near, 3, i)),
                    Thing::Banner => items.push((near, 4, i)),
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
                4 => self.banner(&mut c, cam, &self.obstacles[i]),
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
        if r.roll > 0.0 && r.climb <= 0.0 {
            // Rodando: hecho bolita.
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
    fn banner(&self, c: &mut Canvas, cam: &Camera, o: &Obstacle) {
        let z = o.z;
        let Some((_, _, _, d)) = cam.project(Vec3::new(0.0, 1.8, z)) else { return };
        c.depth = d;
        let colors = [Vec3::new(0.92, 0.15, 0.4), Vec3::new(0.98, 0.78, 0.1), Vec3::new(0.15, 0.7, 0.4), Vec3::new(0.2, 0.5, 0.95), Vec3::new(0.7, 0.3, 0.9), Vec3::new(1.0, 0.5, 0.15)];
        let sag = |x: f32| 1.9 - 0.18 * (1.0 - (x / WALL).powi(2));
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
                let col = colors[(i + (o.seed as usize)) % colors.len()];
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

impl Minigame for CamionetaSurfers {
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
                };
            }
            self.voices.retain(|v| {
                let t = v.n as f32 / rate;
                t < if v.sfx == Sfx::Whistle { WARNING } else { 1.2 }
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
            (Some(sprites), Some((x, ground, k, d))) if r.roll <= 0.0 || r.climb > 0.0 => {
                let air = r.y > r.support + 0.05 || r.climb > 0.0;
                let frame = if air { RUN_FRAMES } else { ((r.phase / TAU).rem_euclid(1.0) * RUN_FRAMES as f32) as usize % RUN_FRAMES };
                sprites.get(frame).map(|s| (s, (x, ground), k / s.scale, d))
            }
            _ => None,
        };
        let light = 1.0 - 0.45 * ash;
        // La flota: cada camioneta o tuk-tuk con el sprite del angulo con que
        // la ve la camara, su mip y sus colores; las que ya quedaron atras
        // del patojo se pintan despues de el.
        let mut rides: Vec<(Ride, bool)> = Vec::new();
        if let Some(fleet) = &self.fleet {
            for o in &self.obstacles {
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
                let view = -((x - cam.pos.x) / d).atan();
                let Some(sprite) = set.get(Fleet::yaw_index(view)) else { continue };
                let scale = k / sprite.scale;
                let lod = ((1.0 / scale).log2() - 0.5).max(0.0);
                let near = (d - len * 0.5).max(0.3);
                let paint = if o.thing == Thing::TukTuk { 0 } else { (o.seed % 3) as u8 };
                let ride = Ride { sprite, pick: sprite.pick(lod), at: (sx, sy), scale, depth: near, paint, fog: 1.0 - (-d / 110.0).exp() };
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
        let dim = if self.screen == Screen::Running { 1.0 } else { 0.55 };
        let (w, h) = (width as f32, self.height as f32);
        let this = &*self;
        parallel::chunks_mut(rgba, width * 4, 1, |y, row| {
            let yf = y as f32 + 0.5;
            let mut depth = vec![f32::MAX; width];
            this.row(y, row, &mut depth, &cam, &boxes, ash);
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
            if let Some((sprite, at, scale, d)) = runner {
                blit(sprite, at, scale, d, light, yf, row, &depth, width);
            }
            for v in &front_rides {
                ride(v, &lut, light, haze, yf, row, &depth, width);
            }
            paint(&front.shapes, yf, row, width, Some(&depth));
            // El acabado: vineta y un poco de gradacion (sombras al indigo,
            // altas calidas).
            let vy = (yf / h - 0.5) * 1.1;
            for (x, p) in row.chunks_exact_mut(4).enumerate() {
                let vx = (x as f32 + 0.5) / w - 0.5;
                let v = 1.0 - 0.38 * smoothstep(0.35, 0.85, vx.hypot(vy));
                let c = lienzo::read(p);
                let l = c.luminance();
                let graded = c + Vec3::new(-0.01, -0.005, 0.025) * (1.0 - l) + Vec3::new(0.03, 0.012, -0.02) * l;
                lienzo::write(p, graded * (v * dim));
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
                let (pw, ph) = (640.0 * s, 390.0 * s);
                let (px, py) = ((w - pw) * 0.5, 80.0 * s);
                o.panel(px, py, pw, ph);
                o.text("Camioneta Surfers", w * 0.5, py + 14.0 * s, Font::Big(50.0 * s), accent, Align::Center);
                o.text("Antigua Guatemala  ·  la calle del Arco, con el Fuego explotando", w * 0.5, py + 78.0 * s, Font::Body, dim, Align::Center);
                let lines = [
                    "← →: carril   ·   ↑: saltar   ·   ↓: rodar (en el aire, baja de golpe)",
                    "Subite a las camionetas paradas por la escalera de atrás",
                    "Las que vienen de frente no perdonan; las carretas se saltan",
                    "Los lazos de papel picado se pasan rodando",
                    "No pisés las alfombras de aserrín: te persiguen los cucuruchos",
                    "Mirá la sombra de las bombas del Fuego antes de que caigan",
                ];
                for (i, line) in lines.iter().enumerate() {
                    o.text(*line, w * 0.5, py + 120.0 * s + i as f32 * 30.0 * s, Font::Body, cream, Align::Center);
                }
                let best = match self.best {
                    Some(b) => format!("Récord: {b} puntos"),
                    None => "Todavía no hay récord".to_string(),
                };
                o.text(best, w * 0.5, py + ph - 66.0 * s, Font::Bold, accent, Align::Center);
                o.text("Enter: correr   ·   Esc: salir", w * 0.5, py + ph - 34.0 * s, Font::Small, cream, Align::Center);
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

    fn run_for(g: &mut CamionetaSurfers, seconds: f32) {
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
        let mut g = CamionetaSurfers::new(w, h, Records::in_memory());
        g.start();
        g.obstacles.clear();
        g.coins.clear();
        g.gen_z = 1e9;
        let mut put = |thing: Thing, lane: usize, z: f32, len: f32, seed: u32| g.obstacles.push(Obstacle { thing, lanes: 1 << lane, z, len, v: 0.0, seed, stepped: false });
        put(Thing::Parked, 1, 9.0, 11.0, 7);
        put(Thing::Bus, 0, 22.0, 11.0, 12);
        put(Thing::TukTuk, 2, 12.0, 2.6, 3);
        put(Thing::Cart, 2, 30.0, 1.0, 5);
        g.obstacles.push(Obstacle { thing: Thing::Banner, lanes: 0b111, z: 40.0, len: 0.12, v: 0.0, seed: 9, stepped: false });
        let mut rgba = vec![0u8; w * h * 4];
        g.draw(&mut rgba);
        let path = std::env::temp_dir().join("surfers_calle.png");
        crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
        println!("{}", path.display());
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
        let mut g = CamionetaSurfers::new(160, 90, Records::in_memory());
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
        let mut g = CamionetaSurfers::new(160, 90, Records::in_memory());
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

        g.start();
        g.obstacles.clear();
        g.gen_z = 1e9;
        g.obstacles.push(Obstacle { thing: Thing::Parked, lanes: 0b010, z: 6.0, len: 11.0, v: 0.0, seed: 3, stepped: false });
        run_for(&mut g, 0.9);
        assert!(g.running(), "{}", g.reason);
        assert!((g.run.y - ROOF).abs() < 0.05, "en el techo: {}", g.run.y);
    }

    #[test]
    fn a_second_carpet_while_chased_gets_you_caught() {
        let mut g = CamionetaSurfers::new(160, 90, Records::in_memory());
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
        let mut g = CamionetaSurfers::new(160, 90, Records::in_memory());
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
        let mut g = CamionetaSurfers::new(w, h, Records::in_memory());
        let mut rgba = vec![0u8; w * h * 4];
        let mut check = |g: &mut CamionetaSurfers| {
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
