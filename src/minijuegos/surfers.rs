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
use super::lienzo;
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
/// Lo alto del techo de una camioneta (y su parrilla).
const ROOF: f32 = 3.0;
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
            Thing::Parked | Thing::Bus => (1.25, 0.0, ROOF),
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
                        push(&mut self.obstacles, thing, 1 << lane, at, 11.0, v, k);
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
                        push(&mut self.obstacles, Thing::TukTuk, 1 << lane, z0 + 6.0 + 9.0 * k as f32, 2.6, 0.0, 5 + k as u32);
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
        r.phase += dt * r.v * 0.55;
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

    /// Una fachada colonial: el color de la casa, el zocalo, la puerta de
    /// madera, las ventanas con reja, el farol y el alero de teja.
    fn facade(side: i32, z: f32, y: f32, top: f32) -> (Vec3, f32) {
        let k = (z / HOUSE).floor() as i32;
        let u = z - k as f32 * HOUSE;
        let h = hash_u32((k as u32).wrapping_mul(0x9E37_79B9) ^ (side as u32).wrapping_mul(0x85EB_CA6B));
        let palette = [
            Vec3::new(0.9, 0.68, 0.26),
            Vec3::new(0.72, 0.24, 0.17),
            Vec3::new(0.28, 0.46, 0.66),
            Vec3::new(0.88, 0.85, 0.76),
            Vec3::new(0.46, 0.62, 0.36),
            Vec3::new(0.92, 0.56, 0.36),
        ];
        let base = palette[(h % palette.len() as u32) as usize];
        let plaster = 0.9 + 0.1 * value_noise(z * 1.7, y * 1.7);
        let mut c = base * plaster;
        let mut glow = 0.0;
        if y > top - 0.42 {
            // El alero de teja con sus canales.
            let tile = (z * 5.0).fract();
            c = Vec3::new(0.5, 0.2, 0.11) * (0.75 + 0.25 * (TAU * tile).sin().abs());
            if y < top - 0.36 {
                c = Vec3::new(0.25, 0.1, 0.06);
            }
            return (c, glow);
        }
        if y < 0.55 {
            c = base * 0.62;
        }
        // La puerta, con sus tableros.
        if (3.6..5.4).contains(&u) && y < 2.5 {
            let (pu, pv) = ((u - 3.6) / 1.8, y / 2.5);
            c = Vec3::new(0.32, 0.17, 0.08);
            let panel = ((pu * 2.0).fract() - 0.5).abs() < 0.38 && ((pv * 3.0).fract() - 0.5).abs() < 0.38;
            if panel {
                c = c * 1.25;
            }
            if (pu - 0.5).abs() < 0.015 {
                c = c * 0.5;
            }
        }
        // Las ventanas con reja.
        for w0 in [1.0f32, 6.6] {
            if u > w0 - 0.08 && u < w0 + 1.48 && y > 0.92 && y < 2.38 {
                c = Vec3::new(0.35, 0.18, 0.08);
                if u > w0 && u < w0 + 1.4 && y > 1.0 && y < 2.3 {
                    c = Vec3::new(0.05, 0.04, 0.05);
                    if h % 3 == 0 {
                        c = Vec3::new(0.85, 0.55, 0.25) * 0.6;
                        glow = 0.3;
                    }
                    let bar = ((u - w0) / 0.14).fract();
                    if bar < 0.18 || ((y - 1.0) / 0.43).fract() < 0.08 {
                        c = Vec3::new(0.06, 0.06, 0.07);
                        glow = 0.0;
                    }
                }
            }
        }
        // El farol de hierro con su luz calida en la pared.
        let (lu, ly) = (u - 4.5, y - 3.0);
        let d2 = lu * lu + ly * ly;
        c = c + Vec3::new(1.0, 0.65, 0.3) * (0.35 * (-d2 / 0.8).exp());
        if lu.abs() < 0.14 && ly.abs() < 0.22 {
            c = Vec3::new(1.0, 0.85, 0.55);
            glow = 1.0;
        }
        (c, glow)
    }

    /// El empedrado: piedras redondeadas en hileras corridas, con su mezcla
    /// oscura; de lejos (un pixel ya mide varias piedras) se promedia.
    fn cobble(x: f32, z: f32, foot: f32) -> Vec3 {
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
        let stone = Vec3::new(0.42, 0.37, 0.32) * (0.72 + 0.4 * ((h >> 12) & 0xFF) as f32 / 255.0);
        let mortar = Vec3::new(0.13, 0.11, 0.1);
        let edge = smoothstep(0.01 + foot, -0.01, d);
        let top = 1.0 + 0.25 * smoothstep(0.0, -0.06, d) * (-fv * 2.0 + 0.5).clamp(0.0, 1.0);
        let near = mortar.lerp(stone * top, edge);
        let average = Vec3::new(0.3, 0.27, 0.23);
        near.lerp(average, smoothstep(0.04, 0.16, foot))
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
        let arch_k = ((cam.pos.z + 2.0) / ARCH_EVERY).ceil();
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
                    let mut c = if gx.abs() < STREET {
                        Self::cobble(gx, gz, foot)
                    } else if gx.abs() < STREET + 0.12 {
                        Vec3::new(0.62, 0.58, 0.52)
                    } else {
                        let slab = ((gz / 0.7).fract() < 0.04) as u8 as f32;
                        Vec3::new(0.5, 0.46, 0.42) * (1.0 - 0.3 * slab)
                    };
                    for o in self.obstacles.iter().filter(|o| o.thing == Thing::Carpet) {
                        let (min, max) = o.bounds();
                        if gx > min.x && gx < max.x && gz > o.z && gz < o.z + o.len {
                            c = Self::carpet(o, gx, gz, min.x, max.x);
                        }
                    }
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
            // El Arco de Santa Catalina: un muro amarillo de lado a lado con
            // su arco, la hilera de ventanitas, el reloj y la cupula.
            for k in [arch_k, arch_k + 1.0] {
                let az = k * ARCH_EVERY;
                let t = az - cam.pos.z;
                if t <= 0.0 || t >= best {
                    continue;
                }
                let (ax, ay) = (cam.pos.x + rx * t, cam.pos.y - ry * t);
                let opening = ax.abs() < STREET && (ay < 4.2 || ax.hypot(ay - 4.2) < STREET);
                let wall = ax.abs() < WALL + 0.6 && ay >= 0.0 && ay < 8.2 && !opening;
                let cupola = ax.abs() < 1.1 && (8.2..10.4).contains(&ay) || ax.abs() < (0.9 - (ay - 10.4) * 0.6) && (10.4..11.8).contains(&ay);
                if wall || cupola {
                    best = t;
                    let yellow = Vec3::new(0.93, 0.72, 0.26);
                    let mut c = yellow;
                    let rim = ax.hypot(ay - 4.2) - STREET;
                    if ax.abs() < STREET + 0.35 && ay > 4.2 && rim < 0.35 {
                        c = Vec3::new(0.95, 0.92, 0.85);
                    }
                    if (7.6..7.85).contains(&ay) || (5.15..5.3).contains(&ay) {
                        c = Vec3::new(0.95, 0.92, 0.85);
                    }
                    if (5.6..6.6).contains(&ay) && ((ax / 1.1).fract().abs() - 0.5).abs() < 0.18 && ax.abs() > 0.8 {
                        c = Vec3::new(0.12, 0.08, 0.06);
                    }
                    if ax.hypot(ay - 6.5) < 0.62 {
                        c = Vec3::new(0.95, 0.94, 0.9);
                        let hand = (self.wall * 0.2).sin_cos();
                        let (hx, hy) = (ax, ay - 6.5);
                        if (hx * hand.1 - hy * hand.0).abs() < 0.03 && (hx * hand.0 + hy * hand.1) > -0.05 || (hx.abs() < 0.03 && hy > 0.0 && hy < 0.38) {
                            c = Vec3::new(0.08, 0.07, 0.07);
                        }
                    }
                    if cupola {
                        c = if ay > 10.4 { Vec3::new(0.85, 0.82, 0.75) } else { yellow * 0.95 };
                    }
                    color = Some((c * 0.95, 0.0));
                }
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
                    let lit = c * light + lava_light + c * glow * 0.6;
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

    /// Las figuras de este frame: lenes, bombas que caen, chispas y humo, el
    /// patojo y, si lo persiguen, los cucuruchos.
    fn figures(&self, cam: &Camera) -> Canvas {
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
        let Some((x, ground, k, d)) = cam.project(Vec3::new(r.x, r.y, r.z)) else { return c };
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
            c.rect(x, cy - m(0.36), m(0.15), m(0.06), m(0.05), Vec3::new(0.85, 0.12, 0.12));
            return self.chasers(c, cam);
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
        self.chasers(c, cam)
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
            if !o.solid() {
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
        let figures = self.figures(&cam);
        let ash = 0.55 * self.eruption();
        let dim = if self.screen == Screen::Running { 1.0 } else { 0.55 };
        let this = &*self;
        parallel::chunks_mut(rgba, width * 4, 1, |y, row| {
            let mut depth = vec![f32::MAX; width];
            this.row(y, row, &mut depth, &cam, &boxes, ash);
            paint(&figures.shapes, y as f32 + 0.5, row, width, Some(&depth));
            if dim < 1.0 {
                for p in row.chunks_exact_mut(4) {
                    for k in 0..3 {
                        p[k] = (p[k] as f32 * dim) as u8;
                    }
                }
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
