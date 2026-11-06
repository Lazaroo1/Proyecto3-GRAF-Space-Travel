//! Barriletes en San Andres Xecul (los Cuchumatanes, version chapina): se
//! vuela un barrilete desde la plaza de la iglesia amarilla jalando y
//! soltando el hilo segun las rafagas, y con la cola se le corta el hilo a
//! los barriletes de los otros patojos.
//!
//! **La fisica** (en metros, a 240 pasos por segundo, de lado: x a favor
//! del viento, y arriba):
//!
//! - **El viento** sopla a ~8 m/s, mas fuerte arriba
//!   (`W (1 + 0.006 y)`), con su vaiven (suma de senos) y rafagas que suben
//!   de golpe y se van.
//! - **El aire sobre el barrilete**: con el viento aparente `w = W - v`
//!   (el aire visto desde el barrilete) y `q = rho |w|^2 A / 2`, la
//!   sustentacion es `q C_L` perpendicular a `w` (hacia arriba) y el
//!   arrastre `q C_D` a lo largo de `w`. Para una placa plana con angulo de
//!   ataque `a` (lo que se inclina el barrilete con las riendas):
//!   `C_L = 1.2 sen 2a` (lo mas a 45 grados) y `C_D = 0.08 + 1.2 sen^2 a`.
//!   Mas el peso.
//! - **El hilo es una restriccion**: si el barrilete se aleja de la mano
//!   mas que el largo del hilo, se le regresa al circulo y se le quita la
//!   velocidad hacia afuera, `v_r = v . d`. Ese impulso, `m v_r`, dividido
//!   entre el paso, es la tension. Si pasa de 50 N por un cuarto de segundo,
//!   el hilo revienta. Si el hilo queda flojo, el barrilete ya no tiene de
//!   donde agarrarse y se cae.
//! - **La cola** es una cadena de puntos con Verlet: cada uno sigue su
//!   inercia, lo empuja el viento y lo jala la gravedad, y despues se
//!   corrigen las distancias entre vecinos (tres pasadas). Lo que arrastra
//!   frena al barrilete.
//!
//! Con la cola se corta: si un tramo de la cola cruza el hilo de otro
//! barrilete (prueba de cruce de segmentos), ese hilo se revienta. Y al
//! reves: la cola de otro tambien corta el nuestro.

use std::f32::consts::{PI, TAU};

use super::carrera::value_noise;
use super::figuras::{Canvas, paint};
use super::lienzo;
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::marimba::Marimba;
use crate::math::{Vec3, hash_u32, smoothstep};
use crate::parallel;

const STEP: f32 = 1.0 / 240.0;
const MASS: f32 = 0.35;
const AREA: f32 = 0.6;
const RHO: f32 = 1.2;
const GRAVITY: f32 = 9.8;
/// Lo que aguanta el hilo (N) y por cuanto tiempo.
const BREAK: f32 = 50.0;
const BREAK_TIME: f32 = 0.25;
/// Donde esta la mano de quien vuela.
const HAND: Vec3 = Vec3::new(0.0, 1.3, 0.0);
/// El hilo: lo menos y lo mas, y que tan rapido se jala o se suelta (m/s).
const MIN_LINE: f32 = 8.0;
const MAX_LINE: f32 = 120.0;
const PULL: f32 = 3.0;
const RELEASE: f32 = 6.0;
/// La cola: cuantos puntos y cada cuanto.
const TAIL: usize = 14;
const TAIL_SEG: f32 = 0.6;
/// Lo que dura la tarde (s).
const AFTERNOON: f32 = 150.0;
const RECORD: &str = "barriletes_record";

/// Un barrilete con su hilo y su cola.
#[derive(Clone, Debug)]
struct Kite {
    p: Vec3,
    v: Vec3,
    line: f32,
    /// El angulo de ataque (rad).
    angle: f32,
    /// La tension (N, suavizada) y cuanto tiempo lleva arriba de lo que
    /// aguanta.
    tension: f32,
    over: f32,
    /// Sin hilo (se lo cortaron o revento).
    free: bool,
    anchor: Vec3,
    tail: Vec<Vec3>,
    tail_prev: Vec<Vec3>,
    colors: [Vec3; 3],
    /// Para los de los otros patojos: la altura que buscan.
    target: f32,
    seed: u32,
}

impl Kite {
    fn new(anchor: Vec3, line: f32, seed: u32) -> Self {
        let palette = [
            Vec3::new(0.92, 0.12, 0.2),
            Vec3::new(0.98, 0.78, 0.1),
            Vec3::new(0.1, 0.55, 0.3),
            Vec3::new(0.15, 0.35, 0.85),
            Vec3::new(0.65, 0.2, 0.7),
            Vec3::new(0.98, 0.45, 0.1),
        ];
        let h = hash_u32(seed);
        let colors = [palette[(h % 6) as usize], palette[((h >> 4) % 6) as usize], palette[((h >> 8) % 6) as usize]];
        let p = anchor + Vec3::new(line * 0.5, line * 0.85, 0.0);
        let tail: Vec<Vec3> = (0..TAIL).map(|i| p - Vec3::new(0.0, 0.7 + TAIL_SEG * i as f32, 0.0)).collect();
        Self { p, v: Vec3::ZERO, line, angle: 0.5, tension: 0.0, over: 0.0, free: false, anchor, tail_prev: tail.clone(), tail, colors, target: 25.0, seed }
    }

    /// Lo que empuja el aire: sustentacion y arrastre con el viento
    /// aparente, mas el arrastre de la cola.
    fn aero(&self, wind: Vec3) -> Vec3 {
        let w = wind - self.v;
        let speed = w.length();
        if speed < 1e-3 {
            return Vec3::ZERO;
        }
        let u = w * (1.0 / speed);
        let mut n = Vec3::new(-u.y, u.x, 0.0);
        if n.y < 0.0 {
            n = -n;
        }
        let a = self.angle;
        let cl = 1.2 * (2.0 * a).sin();
        let cd = 0.08 + 1.2 * a.sin() * a.sin();
        let q = 0.5 * RHO * speed * speed * AREA;
        n * (q * cl) + u * (q * cd + 0.02 * speed * speed)
    }

    /// Un paso de fisica con el viento de su altura. Devuelve si reviento
    /// el hilo en este paso.
    fn step(&mut self, wind: Vec3, dt: f32) -> bool {
        let force = self.aero(wind) + Vec3::new(0.0, -MASS * GRAVITY, 0.0);
        self.v = self.v + force * (dt / MASS);
        self.p = self.p + self.v * dt;
        let mut snapped = false;
        if !self.free {
            let d = self.p - self.anchor;
            let dist = d.length();
            let mut inst = 0.0;
            if dist > self.line {
                let dir = d * (1.0 / dist);
                self.p = self.anchor + dir * self.line;
                let vr = self.v.dot(dir);
                if vr > 0.0 {
                    self.v = self.v - dir * vr;
                    inst = MASS * vr / dt;
                }
            }
            self.tension += (inst - self.tension) * (1.0 - (-dt / 0.08).exp());
            if self.tension > BREAK {
                self.over += dt;
                if self.over > BREAK_TIME {
                    self.free = true;
                    snapped = true;
                }
            } else {
                self.over = 0.0;
            }
        } else {
            self.tension = 0.0;
        }
        // La cola: Verlet, viento, gravedad y sus distancias.
        let root = self.p - Vec3::new(0.0, 0.7, 0.0);
        let damping = 0.985;
        for i in 0..TAIL {
            let q = self.tail[i];
            let vel = (q - self.tail_prev[i]) * damping;
            self.tail_prev[i] = q;
            let air = (wind - vel * (1.0 / dt)) * 1.4;
            self.tail[i] = q + vel + (air + Vec3::new(0.0, -GRAVITY, 0.0)) * (dt * dt);
        }
        for _ in 0..3 {
            let mut prev = root;
            for i in 0..TAIL {
                let d = self.tail[i] - prev;
                let len = d.length().max(1e-6);
                self.tail[i] = prev + d * (TAIL_SEG / len);
                prev = self.tail[i];
            }
        }
        snapped
    }

    /// El hilo como segmentos (de la mano al barrilete, con su comba si
    /// esta flojo): `n` puntos.
    fn line_points(&self, n: usize) -> Vec<Vec3> {
        let (a, b) = (self.anchor, self.p);
        let dist = (b - a).length();
        let slack = (self.line - dist).max(0.0);
        let sag = (0.6 * slack + 0.025 * dist) * (1.0 - smoothstep(10.0, 30.0, self.tension) * 0.9);
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                a.lerp(b, t) - Vec3::new(0.0, 4.0 * sag * t * (1.0 - t), 0.0)
            })
            .collect()
    }

    /// La cola como segmentos, desde la punta de abajo del barrilete.
    fn tail_points(&self) -> Vec<Vec3> {
        let mut pts = vec![self.p - Vec3::new(0.0, 0.7, 0.0)];
        pts.extend(self.tail.iter().copied());
        pts
    }
}

/// Los primeros segundos nadie corta.
const GRACE: f32 = 10.0;

/// Si el barrilete `seed` de otro patojo esta atacando a los `t` segundos:
/// unos 8 s de cada 30, cada uno a su hora.
fn attacking(t: f32, seed: u32) -> bool {
    let phase = (t + (hash_u32(seed) % 30) as f32) % 30.0;
    phase < 8.0
}

/// Si los segmentos `a b` y `c d` se cruzan (en el plano xy).
fn crosses(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let cross = |o: Vec3, p: Vec3, q: Vec3| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Si alguna parte de la polilinea `p` cruza la polilinea `q`.
fn polylines_cross(p: &[Vec3], q: &[Vec3]) -> bool {
    p.windows(2).any(|a| q.windows(2).any(|b| crosses(a[0], a[1], b[0], b[1])))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Flying,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sfx {
    Snap,
    Cut,
    Cheer,
    Gust,
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    sfx: Sfx,
    n: u32,
}

/// Un otro patojo con su barrilete (y cuando vuelve a elevar otro).
#[derive(Clone, Debug)]
struct Rival {
    kite: Kite,
    respawn: f32,
    name: &'static str,
}

pub struct Barriletes {
    width: usize,
    height: usize,
    s: f32,
    screen: Screen,
    kite: Kite,
    rivals: Vec<Rival>,
    time: f32,
    score: f32,
    cuts: u32,
    peak: f32,
    reason: &'static str,
    gust: f32,
    gust_in: f32,
    /// Cuando empezo la ultima rafaga.
    gust_at: f32,
    /// La camara: el centro (m) y los pixeles por metro.
    cam: (f32, f32, f32),
    records: Records,
    best: Option<i64>,
    rng: u32,
    wall: f32,
    done: bool,
    autopilot: bool,
    message: Option<(String, f32)>,
    // El sonido.
    marimba: Marimba,
    pending: Vec<Sfx>,
    voices: Vec<Voice>,
    noise: u32,
    wind_lp: f32,
    flutter: f32,
}

const NAMES: [&str; 3] = ["el patojo de la tienda", "la seño Juana", "el Chepe"];

impl Barriletes {
    pub fn new(width: usize, height: usize, records: Records) -> Self {
        let best = records.get(RECORD);
        let mut game = Self {
            width,
            height,
            s: height as f32 / 720.0,
            screen: Screen::Menu,
            kite: Kite::new(HAND, 30.0, 7),
            rivals: Vec::new(),
            time: 0.0,
            score: 0.0,
            cuts: 0,
            peak: 0.0,
            reason: "",
            gust: 0.0,
            gust_in: 6.0,
            gust_at: -100.0,
            cam: (20.0, 15.0, 10.0),
            records,
            best,
            rng: 0xBA77_1E7E,
            wall: 0.0,
            done: false,
            autopilot: false,
            message: None,
            marimba: Marimba::new(),
            pending: Vec::new(),
            voices: Vec::new(),
            noise: 0x1357_9BDF,
            wind_lp: 0.0,
            flutter: 0.0,
        };
        game.reset();
        game
    }

    pub fn set_autopilot(&mut self, on: bool) {
        self.autopilot = on;
    }

    pub fn start(&mut self) {
        self.reset();
        self.screen = Screen::Flying;
    }

    pub fn flying(&self) -> bool {
        self.screen == Screen::Flying
    }

    /// Puntos, cortes y la altura mas alta.
    pub fn tally(&self) -> (i64, u32, f32) {
        (self.score as i64, self.cuts, self.peak)
    }

    pub fn reason(&self) -> &'static str {
        self.reason
    }

    fn reset(&mut self) {
        self.kite = Kite::new(HAND, 30.0, 7);
        self.rivals = [(-34.0f32, 40.0f32), (-20.0, 30.0), (22.0, 24.0)]
            .iter()
            .enumerate()
            .map(|(i, &(x, line))| {
                let mut kite = Kite::new(Vec3::new(x, 1.3, 0.0), line, 100 + i as u32 * 31);
                kite.target = 18.0 + 10.0 * i as f32;
                Rival { kite, respawn: 0.0, name: NAMES[i] }
            })
            .collect();
        self.time = 0.0;
        self.score = 0.0;
        self.cuts = 0;
        self.peak = 0.0;
        self.reason = "";
        self.gust = 0.0;
        self.gust_in = 6.0;
        self.gust_at = -100.0;
        self.rng = 0xBA77_1E7E;
        // Que todos arranquen ya volando (con viento suave, sin que el
        // tiron del principio les reviente el hilo).
        for _ in 0..480 {
            let w = Vec3::new(7.0, 0.0, 0.0);
            self.kite.step(w, STEP);
            self.kite.over = 0.0;
            for r in &mut self.rivals {
                r.kite.step(w, STEP);
                r.kite.over = 0.0;
            }
        }
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// El viento a la altura `y`: su vaiven, la rafaga y mas fuerte arriba.
    fn wind_at(&self, y: f32) -> Vec3 {
        let t = self.time;
        let sway = 1.0 + 0.12 * (0.31 * t).sin() + 0.07 * (0.83 * t + 1.0).sin() + 0.04 * (2.1 * t).sin();
        Vec3::new(8.0 * sway * (1.0 + 0.35 * self.gust) * (1.0 + 0.006 * y.max(0.0)), 0.25 * (0.5 * t).sin(), 0.0)
    }

    fn crash(&mut self, reason: &'static str) {
        if self.screen != Screen::Flying {
            return;
        }
        self.reason = reason;
        self.finish();
    }

    fn finish(&mut self) {
        self.screen = Screen::Over;
        let score = self.score as i64;
        if self.best.is_none_or(|b| score > b) {
            self.best = Some(score);
            self.records.set(RECORD, score);
        }
    }

    /// Lo que haria quien sabe: subir jalando cuando esta bajo o flojo,
    /// soltar cuando la tension se acerca a lo que aguanta.
    fn auto_keys(&self) -> Keys {
        let mut k = Keys::default();
        let kite = &self.kite;
        if kite.tension > 30.0 || self.gust > 0.05 {
            k.up = true;
        } else if kite.tension < 6.0 || kite.p.y < 30.0 {
            k.down = true;
        } else if kite.p.y > 40.0 {
            k.up = true;
        }
        let want = if self.gust > 0.05 { 0.35 } else { 0.5 };
        if kite.angle > want + 0.05 {
            k.steer = -1.0;
        } else if kite.angle < want - 0.05 {
            k.steer = 1.0;
        }
        k
    }

    fn fly(&mut self, dt: f32, keys: &Keys) {
        self.time += dt;
        // Las rafagas: llegan cada tanto, suben en 1.2 s (el aviso da
        // tiempo de soltar), se quedan y se van:
        // `smoothstep(0, 1.2, t) (1 - smoothstep(2.7, 5.2, t))`.
        self.gust_in -= dt;
        if self.gust_in <= 0.0 {
            self.gust_in = 9.0 + 8.0 * self.random();
            self.gust_at = self.time;
            self.pending.push(Sfx::Gust);
            self.message = Some(("¡Ráfaga!".to_string(), self.wall));
        }
        let t = self.time - self.gust_at;
        self.gust = smoothstep(0.0, 1.2, t) * (1.0 - smoothstep(2.7, 5.2, t));
        // El hilo y las riendas.
        let pull = keys.down || keys.typed('s');
        let release = keys.up || keys.typed('w');
        let k = &mut self.kite;
        if pull {
            k.line = (k.line - PULL * dt).max(MIN_LINE);
        }
        if release {
            k.line = (k.line + RELEASE * dt).min(MAX_LINE);
        }
        let steer = if keys.steer != 0.0 { keys.steer } else if keys.left { -1.0 } else if keys.right { 1.0 } else { 0.0 };
        k.angle = (k.angle + steer * 0.8 * dt).clamp(0.22, 0.85);
        // La fisica, en pasos fijos.
        let n = (dt / STEP).round().max(1.0) as usize;
        for _ in 0..n {
            let w = self.wind_at(self.kite.p.y);
            if self.kite.step(w, STEP) {
                self.pending.push(Sfx::Snap);
                self.message = Some(("¡Reventó el hilo!".to_string(), self.wall));
            }
            for i in 0..self.rivals.len() {
                let w = self.wind_at(self.rivals[i].kite.p.y);
                self.rivals[i].kite.step(w, STEP);
            }
        }
        // Los otros patojos: buscan su altura y no se pasan de tension.
        for r in &mut self.rivals {
            let k = &mut r.kite;
            if k.free {
                continue;
            }
            // A ratos ataca: busca la altura del nuestro para cruzarle la
            // cola por el hilo; si no, se queda en la suya.
            k.target = if self.time > GRACE && attacking(self.time, k.seed) { self.kite.p.y + 4.0 } else { 16.0 + 14.0 * (0.5 + 0.5 * (self.time * 0.07 + k.seed as f32).sin()) };
            if k.tension > 32.0 || k.p.y > k.target + 3.0 {
                k.line = (k.line + RELEASE * dt).min(70.0);
            } else if k.tension < 5.0 || k.p.y < k.target - 3.0 {
                k.line = (k.line - PULL * 0.6 * dt).max(MIN_LINE);
            }
        }
        // Cortes con la cola.
        if !self.kite.free {
            let my_tail = self.kite.tail_points();
            let my_line = self.kite.line_points(8);
            let mut cut_me: Option<&'static str> = None;
            let mut cut_them = Vec::new();
            for (i, r) in self.rivals.iter().enumerate() {
                if r.kite.free || r.respawn > 0.0 {
                    continue;
                }
                if polylines_cross(&my_tail, &r.kite.line_points(8)) {
                    cut_them.push(i);
                } else if self.time > GRACE && attacking(self.time, r.kite.seed) && polylines_cross(&r.kite.tail_points()[..6], &my_line) {
                    cut_me = Some(r.name);
                }
            }
            for i in cut_them {
                self.rivals[i].kite.free = true;
                self.rivals[i].respawn = 9.0;
                self.cuts += 1;
                self.score += 500.0;
                self.pending.push(Sfx::Cut);
                self.pending.push(Sfx::Cheer);
                self.message = Some((format!("¡Le cortaste el hilo a {}!", self.rivals[i].name), self.wall));
            }
            if let Some(name) = cut_me {
                self.kite.free = true;
                self.pending.push(Sfx::Snap);
                self.message = Some((format!("¡{}{} te cortó el hilo!", name[..1].to_uppercase(), &name[1..]), self.wall));
            }
        }
        // Los que se fueron: cuando caen, otro patojo eleva otro.
        for i in 0..self.rivals.len() {
            if self.rivals[i].respawn > 0.0 {
                self.rivals[i].respawn -= dt;
                if self.rivals[i].respawn <= 0.0 {
                    let anchor = self.rivals[i].kite.anchor;
                    let seed = self.rivals[i].kite.seed.wrapping_add(17);
                    self.rivals[i].kite = Kite::new(anchor, 20.0, seed);
                }
            }
        }
        // Puntos por altura.
        let k = &self.kite;
        if !k.free {
            self.score += k.p.y.max(0.0) * dt * 2.0;
            self.peak = self.peak.max(k.p.y);
        }
        if k.p.y < 0.6 {
            self.crash(if k.free { "Se fue el barrilete" } else { "Se cayó el barrilete" });
        } else if k.free && (k.p.x - self.cam.0).abs() * self.cam.2 > self.width as f32 * 0.7 {
            self.crash("Se fue el barrilete");
        } else if self.time >= AFTERNOON {
            self.reason = "Se acabó la tarde";
            self.finish();
        }
        // La camara sigue a la mano y al barrilete.
        let (kx, ky) = (self.kite.p.x, self.kite.p.y);
        let (x0, x1) = ((kx - 18.0).min(-12.0), (kx + 18.0).max(30.0));
        let (y0, y1) = (-3.0, (ky + 10.0).max(24.0));
        let scale = (self.width as f32 / (x1 - x0)).min(self.height as f32 / (y1 - y0));
        let blend = 1.0 - (-dt * 3.0).exp();
        let c = &mut self.cam;
        c.0 += ((x0 + x1) * 0.5 - c.0) * blend;
        c.1 += ((y0 + y1) * 0.5 - c.1) * blend;
        c.2 += (scale - c.2) * blend;
    }

    /// Del mundo (m) a la pantalla.
    fn to_screen(&self, p: Vec3) -> (f32, f32) {
        let (cx, cy, k) = self.cam;
        (self.width as f32 * 0.5 + (p.x - cx) * k, self.height as f32 * 0.5 - (p.y - cy) * k)
    }

    /// El fondo: el cielo de la tarde con sus nubes que corren con el
    /// viento y los Cuchumatanes en dos capas.
    fn sky(&self, x: f32, y: f32, ridges: &[[f32; 2]]) -> Vec3 {
        let (w, h, s) = (self.width as f32, self.height as f32, self.s);
        let t = y / h;
        let mut c = Vec3::new(0.18, 0.42, 0.82).lerp(Vec3::new(0.78, 0.86, 0.95), t.powf(1.3));
        // El sol de la tarde, a la izquierda.
        let sun = ((x - 0.12 * w).powi(2) + (y - 0.18 * h).powi(2)).sqrt() / h;
        c = c + Vec3::new(1.0, 0.9, 0.7) * (0.35 * (-sun * 9.0).exp()) + Vec3::splat(0.6) * smoothstep(0.045, 0.035, sun);
        // Las nubes: ruido en dos escalas, corriendo con el viento.
        let drift = self.wall * 6.0 + self.cam.0 * 2.0;
        let (u, v) = ((x + drift) / (220.0 * s), y / (90.0 * s));
        let n = 0.65 * value_noise(u, v) + 0.35 * value_noise(u * 2.3 + 7.0, v * 2.3);
        let cloud = smoothstep(0.15, 0.55, n) * smoothstep(0.75, 0.2, t);
        c = c.lerp(Vec3::new(0.97, 0.97, 0.99) - Vec3::splat(0.12 * smoothstep(0.3, 0.8, n)), cloud * 0.85);
        // Las montanas: la de atras azulada, la de adelante verde oscuro;
        // se corren poco con la camara (estan lejos).
        let column = ridges.get(x as usize).copied().unwrap_or([h, h]);
        for (layer, col) in [Vec3::new(0.42, 0.52, 0.7), Vec3::new(0.22, 0.38, 0.32)].iter().enumerate() {
            if y > column[layer] {
                let shade = 0.9 + 0.1 * value_noise(x / (32.0 * s), y / (30.0 * s) + layer as f32 * 7.0);
                c = *col * shade + c * 0.08;
            }
        }
        c
    }

    /// Donde empieza cada capa de montanas en cada columna (la de atras y
    /// la de adelante): un perfil de ruido en tres escalas que se corre poco
    /// con la camara (estan lejos).
    fn ridges(&self) -> Vec<[f32; 2]> {
        let (h, s) = (self.height as f32, self.s);
        (0..self.width)
            .map(|x| {
                let x = x as f32 + 0.5;
                std::array::from_fn(|layer| {
                    let (base, amp) = [(0.62f32, 0.12f32), (0.7, 0.1)][layer];
                    let shift = self.cam.0 * (0.4 + 0.5 * layer as f32) * 3.0;
                    let xr = (x + shift) / (160.0 * s);
                    base * h - amp * h * (0.6 * value_noise(xr, 3.0 + layer as f32 * 5.0) + 0.3 * value_noise(xr * 2.7, 9.0) + 0.1 * value_noise(xr * 7.0, 1.0) + 0.5)
                })
            })
            .collect()
    }

    /// Lo que va en el mundo: el pueblo, la iglesia de San Andres Xecul,
    /// los patojos con sus barriletes y el viento.
    fn figures(&self) -> Canvas {
        let mut c = Canvas::default();
        let s = self.s;
        let k = self.cam.2;
        let at = |x: f32, y: f32| self.to_screen(Vec3::new(x, y, 0.0));
        // El suelo de la plaza.
        let (gx0, gy) = at(-200.0, 0.0);
        let (gx1, _) = at(300.0, 0.0);
        c.rect((gx0 + gx1) * 0.5, gy + 400.0 * s, (gx1 - gx0) * 0.5, 400.0 * s, 0.0, Vec3::new(0.55, 0.5, 0.45));
        // Las casas del pueblo, en hilera.
        for i in -14..24 {
            let h = hash_u32(i as u32 ^ 0xCA5A);
            let x = i as f32 * 7.0;
            if (-34.0..-8.0).contains(&x) {
                continue;
            }
            let height = 4.0 + 3.0 * (h % 100) as f32 / 100.0;
            let colors = [Vec3::new(0.85, 0.45, 0.35), Vec3::new(0.3, 0.55, 0.75), Vec3::new(0.9, 0.8, 0.45), Vec3::new(0.55, 0.75, 0.5), Vec3::new(0.92, 0.9, 0.85)];
            let col = colors[(h >> 8) as usize % colors.len()];
            let (sx, sy) = at(x + 3.5, height * 0.5);
            c.rect(sx, sy, 3.4 * k, height * 0.5 * k, 0.0, col * 0.85);
            let (rx, ry) = at(x + 3.5, height);
            c.rect(rx, ry, 3.7 * k, 0.35 * k, 0.1 * k, Vec3::new(0.5, 0.2, 0.12));
            let (dx, dy) = at(x + 3.5, 1.1);
            c.rect(dx, dy, 0.6 * k, 1.1 * k, 0.1 * k, Vec3::new(0.3, 0.17, 0.08));
        }
        // La iglesia de San Andres Xecul: la fachada amarilla con sus
        // columnas de colores, los angelitos y las enredaderas, las dos
        // torres y la cupula.
        let yellow = Vec3::new(0.97, 0.8, 0.22);
        let (fx, fy) = at(-21.0, 9.0);
        c.rect(fx, fy, 9.0 * k, 9.0 * k, 0.0, yellow);
        for (i, col) in [Vec3::new(0.85, 0.1, 0.15), Vec3::new(0.1, 0.55, 0.3), Vec3::new(0.95, 0.95, 0.92)].iter().cycle().take(8).enumerate() {
            let x = -28.5 + 2.15 * i as f32;
            let (cx, cy) = at(x, 8.0);
            c.rect(cx, cy, 0.18 * k, 7.0 * k, 0.1 * k, *col);
        }
        for row in 0..3 {
            for i in 0..4 {
                let (ax, ay) = at(-26.5 + 3.8 * i as f32, 4.5 + 4.2 * row as f32);
                c.disc(ax, ay - 0.5 * k, 0.45 * k, Vec3::new(0.95, 0.85, 0.75));
                c.rect(ax, ay + 0.4 * k, 0.4 * k, 0.6 * k, 0.2 * k, [Vec3::new(0.3, 0.6, 0.85), Vec3::new(0.9, 0.3, 0.4)][(row + i) % 2]);
            }
        }
        let (px, py) = at(-21.0, 2.0);
        c.rect(px, py, 1.4 * k, 2.0 * k, 1.2 * k, Vec3::new(0.35, 0.18, 0.1));
        let (gx, gy2) = at(-21.0, 18.0);
        c.arc(gx, gy2, 4.5 * k, 0.4 * k, -PI, 0.0, Vec3::new(0.9, 0.2, 0.25));
        for side in [-1.0f32, 1.0] {
            let tx = -21.0 + side * 10.5;
            let (tx_s, ty_s) = at(tx, 11.0);
            c.rect(tx_s, ty_s, 1.8 * k, 11.0 * k, 0.0, yellow * 0.95);
            let (bx, by) = at(tx, 19.0);
            c.arc(bx, by + 0.6 * k, 0.9 * k, 0.25 * k, -PI, 0.0, Vec3::new(0.15, 0.1, 0.08));
            let (cx, cy) = at(tx, 22.6);
            c.disc(cx, cy, 1.7 * k, Vec3::new(0.85, 0.25, 0.25));
            let (sx2, sy2) = at(tx, 25.0);
            c.line(sx2, sy2, sx2, sy2 - 1.6 * k, 0.15 * k, Vec3::new(0.95, 0.85, 0.4));
            c.line(sx2 - 0.6 * k, sy2 - 1.1 * k, sx2 + 0.6 * k, sy2 - 1.1 * k, 0.15 * k, Vec3::new(0.95, 0.85, 0.4));
        }
        let (dx, dy) = at(-21.0, 20.5);
        c.disc(dx, dy, 3.0 * k, Vec3::new(0.95, 0.88, 0.6));
        // El viento: rayitas que pasan, mas con la rafaga.
        let wind = self.wind_at(10.0).x;
        let count = 10 + (self.gust * 18.0) as u32;
        for i in 0..count {
            let h = hash_u32(i * 977 + 13);
            let speed = wind * 18.0 * s;
            let x = ((h % 1000) as f32 / 1000.0 * self.width as f32 * 1.4 + self.wall * speed) % (self.width as f32 * 1.4) - 0.2 * self.width as f32;
            let y = ((h >> 10) % 1000) as f32 / 1000.0 * self.height as f32 * 0.6;
            let len = (30.0 + 40.0 * self.gust) * s;
            c.line(x, y, x + len, y + 2.0 * s, 1.0 * s, Vec3::splat(1.0)).alpha(0.25 + 0.3 * self.gust);
        }
        // Los barriletes de los otros y el nuestro, con sus hilos y colas.
        for r in &self.rivals {
            if r.respawn > 0.0 && r.kite.p.y < 0.5 {
                continue;
            }
            self.draw_kite(&mut c, &r.kite, 0.8);
            self.draw_kid(&mut c, r.kite.anchor, r.kite.colors[1], !r.kite.free);
        }
        self.draw_kid(&mut c, HAND, Vec3::new(0.25, 0.6, 0.9), !self.kite.free);
        self.draw_kite(&mut c, &self.kite, 1.0);
        c
    }

    /// Un patojo de lado con el brazo hacia su hilo y su lata.
    fn draw_kid(&self, c: &mut Canvas, hand: Vec3, shirt: Vec3, holding: bool) {
        let k = self.cam.2.max(14.0 * self.s);
        let (hx, hy) = self.to_screen(hand);
        let (fx, fy) = self.to_screen(Vec3::new(hand.x - 0.3, 0.0, 0.0));
        let m = |v: f32| v * k;
        c.line(fx, fy, fx, fy - m(0.85), m(0.1), Vec3::new(0.15, 0.2, 0.35));
        c.rect(fx, fy - m(1.2), m(0.2), m(0.35), m(0.08), shirt);
        c.disc(fx, fy - m(1.72), m(0.17), Vec3::new(0.65, 0.45, 0.32));
        c.disc(fx, fy - m(1.82), m(0.16), Vec3::new(0.08, 0.06, 0.05)).alpha(0.9);
        if holding {
            c.line(fx, fy - m(1.4), hx, hy, m(0.07), shirt * 0.9);
            c.disc(hx, hy, m(0.13), Vec3::new(0.7, 0.72, 0.75));
        }
    }

    /// El barrilete: un hexagono de papel de china con sus gajos, su
    /// centro, el fleco de la orilla, las canas y la cola con sus monos.
    fn draw_kite(&self, c: &mut Canvas, kite: &Kite, size: f32) {
        let s = self.s;
        // El hilo.
        if !kite.free {
            let pts = kite.line_points(10);
            for w in pts.windows(2) {
                let (a, b) = (self.to_screen(w[0]), self.to_screen(w[1]));
                c.line(a.0, a.1, b.0, b.1, 0.7 * s, Vec3::new(0.97, 0.97, 0.94)).alpha(0.85);
            }
        }
        // La cola.
        let tail = kite.tail_points();
        for (i, w) in tail.windows(2).enumerate() {
            let (a, b) = (self.to_screen(w[0]), self.to_screen(w[1]));
            c.line(a.0, a.1, b.0, b.1, 1.2 * s, Vec3::new(0.9, 0.85, 0.7));
            if i % 2 == 1 {
                let col = kite.colors[i / 2 % 3];
                let r = 6.0 * s * size;
                c.poly(&[(b.0, b.1), (b.0 - r, b.1 - r * 0.6), (b.0 - r, b.1 + r * 0.6)], col);
                c.poly(&[(b.0, b.1), (b.0 + r, b.1 - r * 0.6), (b.0 + r, b.1 + r * 0.6)], col);
            }
        }
        // El hexagono, ladeado segun por donde lo jala el viento.
        let (x, y) = self.to_screen(kite.p);
        let radius = (1.1 * self.cam.2).max(30.0 * s) * size;
        let lean = (kite.v.x * 0.02).clamp(-0.3, 0.3) + (kite.angle - 0.5) * 0.4;
        let corner = |i: usize, r: f32| {
            let a = lean + PI / 6.0 + i as f32 * TAU / 6.0;
            (x + r * a.cos(), y + r * a.sin())
        };
        // El fleco: triangulitos de colores alrededor.
        for j in 0..18 {
            let a0 = lean + j as f32 * TAU / 18.0;
            let (ca, sa) = a0.cos_sin_pair();
            let r1 = radius * 1.18;
            let tip = (x + r1 * ca, y + r1 * sa);
            let (cb, sb) = (a0 + 0.12).cos_sin_pair();
            let (cc, sc) = (a0 - 0.12).cos_sin_pair();
            c.poly(&[tip, (x + radius * 0.95 * cb, y + radius * 0.95 * sb), (x + radius * 0.95 * cc, y + radius * 0.95 * sc)], kite.colors[j % 3] * 0.9);
        }
        for i in 0..6 {
            let col = if i % 2 == 0 { kite.colors[0] } else { kite.colors[1] };
            c.poly(&[(x, y), corner(i, radius), corner(i + 1, radius)], col);
        }
        let inner: Vec<(f32, f32)> = (0..6).map(|i| corner(i, radius * 0.55)).collect();
        c.poly(&inner, kite.colors[2]);
        c.disc(x, y, radius * 0.22, Vec3::new(0.98, 0.95, 0.85));
        c.disc(x, y, radius * 0.1, kite.colors[0]);
        for i in 0..3 {
            let (a, b) = (corner(i, radius), corner(i + 3, radius));
            c.line(a.0, a.1, b.0, b.1, 1.1 * s, Vec3::new(0.35, 0.22, 0.1)).alpha(0.8);
        }
    }
}

/// `(cos, sin)` de un angulo.
trait CosSin {
    fn cos_sin_pair(self) -> (f32, f32);
}

impl CosSin for f32 {
    fn cos_sin_pair(self) -> (f32, f32) {
        let (s, c) = self.sin_cos();
        (c, s)
    }
}

impl Minigame for Barriletes {
    fn update(&mut self, dt: f64, keys: &Keys) {
        let dt = (dt as f32).min(0.05);
        self.wall += dt;
        let keys = if self.autopilot && self.screen == Screen::Flying { Keys { escape: keys.escape, ..self.auto_keys() } } else { *keys };
        match self.screen {
            Screen::Menu => {
                if keys.escape {
                    self.done = true;
                } else if keys.enter {
                    self.start();
                }
                // De fondo, el barrilete vuela solo.
                let w = self.wind_at(self.kite.p.y);
                for _ in 0..(dt / STEP).round().max(1.0) as usize {
                    self.kite.step(w, STEP);
                }
            }
            Screen::Flying => {
                if keys.escape {
                    self.reason = "Bajaste el barrilete";
                    self.finish();
                } else {
                    self.fly(dt, &keys);
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
    }

    fn audio(&mut self, out: &mut [f32]) {
        let rate = SAMPLE_RATE as f32;
        for sfx in self.pending.drain(..) {
            self.voices.push(Voice { sfx, n: 0 });
        }
        let flying = self.screen == Screen::Flying;
        let wind = self.wind_at(self.kite.p.y).x / 8.0;
        let apparent = (self.wind_at(self.kite.p.y) - self.kite.v).length() / 10.0;
        let tension = if flying { (self.kite.tension / BREAK).clamp(0.0, 1.2) } else { 0.0 };
        for sample in out.iter_mut() {
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            let n = (self.noise >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0;
            // El viento y el papel que tiembla.
            self.wind_lp += (n - self.wind_lp) * (0.01 + 0.02 * wind);
            self.flutter += 1.0 / rate;
            let flap = (TAU * (14.0 + 10.0 * apparent) * self.flutter).sin().max(0.0);
            let mut x = self.wind_lp * 0.25 * wind + n * 0.02 * apparent * flap;
            // El hilo que zumba con la tension.
            if tension > 0.5 {
                let f = 180.0 + 220.0 * tension;
                x += 0.02 * (tension - 0.5) * (TAU * f * self.flutter).sin();
            }
            // La marimba de la fiesta, de fondo.
            x += self.marimba.next(rate) * 0.22;
            for v in &mut self.voices {
                let t = v.n as f32 / rate;
                v.n += 1;
                x += match v.sfx {
                    Sfx::Snap => n * 0.5 * (-t * 30.0).exp() + 0.15 * (TAU * (900.0 - 600.0 * t) * t).sin() * (-t * 6.0).exp(),
                    Sfx::Cut => n * 0.3 * (-t * 18.0).exp() + 0.1 * (TAU * 1200.0 * t).sin() * (-t * 10.0).exp(),
                    Sfx::Cheer => n * 0.18 * (PI * (t / 1.2).min(1.0)).sin() * (0.6 + 0.4 * (TAU * 6.0 * t).sin()),
                    Sfx::Gust => self.wind_lp * 1.2 * (PI * (t / 1.5).min(1.0)).sin(),
                };
            }
            self.voices.retain(|v| (v.n as f32 / rate) < 1.5);
            *sample = x / (1.0 + 0.5 * x.abs());
        }
    }

    fn draw(&mut self, rgba: &mut [u8]) {
        let width = self.width;
        let figures = self.figures();
        let ridges = self.ridges();
        let dim = if self.screen == Screen::Flying { 1.0 } else { 0.6 };
        let this = &*self;
        parallel::chunks_mut(rgba, width * 4, 1, |y, row| {
            let yf = y as f32 + 0.5;
            for x in 0..width {
                lienzo::write(&mut row[x * 4..x * 4 + 4], this.sky(x as f32 + 0.5, yf, &ridges));
            }
            paint(&figures.shapes, yf, row, width, None);
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
                let (pw, ph) = (660.0 * s, 380.0 * s);
                let (px, py) = ((w - pw) * 0.5, 80.0 * s);
                o.panel(px, py, pw, ph);
                o.text("Barriletes de Xecul", w * 0.5, py + 14.0 * s, Font::Big(50.0 * s), accent, Align::Center);
                o.text("En la plaza de San Andrés Xecul, el Día de Todos los Santos", w * 0.5, py + 78.0 * s, Font::Body, dim, Align::Center);
                let lines = [
                    "↓ o S: jalar el hilo   ·   ↑ o W: soltar hilo   ·   ← →: inclinarlo",
                    "Jalando sube y aguanta más viento; si soltás mucho, se cae",
                    "Si jalás en una ráfaga, la tensión revienta el hilo",
                    "Con la cola le cortás el hilo a los barriletes de los otros",
                    "(y ellos con la suya te lo pueden cortar a vos)",
                ];
                for (i, line) in lines.iter().enumerate() {
                    o.text(*line, w * 0.5, py + 120.0 * s + i as f32 * 30.0 * s, Font::Body, cream, Align::Center);
                }
                let best = match self.best {
                    Some(b) => format!("Récord: {b} puntos"),
                    None => "Todavía no hay récord".to_string(),
                };
                o.text(best, w * 0.5, py + ph - 66.0 * s, Font::Bold, accent, Align::Center);
                o.text("Enter: elevar   ·   Esc: salir", w * 0.5, py + ph - 34.0 * s, Font::Small, cream, Align::Center);
            }
            Screen::Flying | Screen::Over => {
                let k = &self.kite;
                o.panel(16.0 * s, 16.0 * s, 250.0 * s, 150.0 * s);
                o.text(format!("{} puntos", self.score as i64), 32.0 * s, 24.0 * s, Font::Heading, accent, Align::Left);
                o.text(format!("Altura {:.0} m   ·   hilo {:.0} m", k.p.y.max(0.0), k.line), 32.0 * s, 60.0 * s, Font::Small, cream, Align::Left);
                o.text(format!("Cortes {}   ·   quedan {:.0} s", self.cuts, (AFTERNOON - self.time).max(0.0)), 32.0 * s, 82.0 * s, Font::Small, cream, Align::Left);
                o.text("Tensión del hilo", 32.0 * s, 108.0 * s, Font::Small, dim, Align::Left);
                let f = (k.tension / BREAK).clamp(0.0, 1.0);
                let color = if f < 0.6 {
                    [90, 200, 110, 255]
                } else if f < 0.85 {
                    [240, 190, 60, 255]
                } else {
                    [235, 70, 60, 255]
                };
                o.bar(32.0 * s, 132.0 * s, 218.0 * s, 10.0 * s, f, color, false);
                o.text(format!("Viento {:.0} km/h", self.wind_at(k.p.y).x * 3.6), w - 20.0 * s, 24.0 * s, Font::Bold, cream, Align::Right);
                o.bar(w - 220.0 * s, 52.0 * s, 200.0 * s, 8.0 * s, (self.wind_at(k.p.y).x / 16.0).clamp(0.0, 1.0), [140, 200, 255, 255], false);
                if let Some((text, at)) = &self.message
                    && self.wall - at < 2.2
                {
                    o.text(text.clone(), w * 0.5, 90.0 * s, Font::Heading, [255, 240, 210, 255], Align::Center);
                }
            }
        }
        if self.screen == Screen::Over {
            let (pw, ph) = (560.0 * s, 280.0 * s);
            let (px, py) = ((w - pw) * 0.5, 170.0 * s);
            o.panel(px, py, pw, ph);
            o.text(self.reason, w * 0.5, py + 18.0 * s, Font::Big(38.0 * s), accent, Align::Center);
            let lines = [format!("Puntos: {}", self.score as i64), format!("Hilos cortados: {}", self.cuts), format!("Lo más alto: {:.0} m", self.peak)];
            for (i, line) in lines.iter().enumerate() {
                o.text(line.clone(), w * 0.5, py + 84.0 * s + i as f32 * 32.0 * s, if i == 0 { Font::Heading } else { Font::Body }, if i == 0 { accent } else { cream }, Align::Center);
            }
            o.text("Enter: elevar otro   ·   Esc: al menú", w * 0.5, py + ph - 34.0 * s, Font::Small, cream, Align::Center);
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

    /// Con viento parejo el barrilete se queda arriba, con el hilo tenso y
    /// del lado a favor del viento; sin viento se cae.
    #[test]
    fn the_kite_flies_in_the_wind_and_falls_without_it() {
        let mut kite = Kite::new(HAND, 30.0, 1);
        for _ in 0..240 * 20 {
            kite.step(Vec3::new(8.0, 0.0, 0.0), STEP);
        }
        assert!(!kite.free);
        assert!(kite.p.y > 18.0, "altura {}", kite.p.y);
        assert!(kite.p.x > 0.0);
        assert!(kite.tension > 10.0 && kite.tension < BREAK, "tension {}", kite.tension);
        assert!(((kite.p - HAND).length() - 30.0).abs() < 0.2, "el hilo tenso");
        // Sin viento baja planeando (el papel frena, pero baja).
        let mut calm = Kite::new(HAND, 30.0, 1);
        calm.p = kite.p;
        for _ in 0..240 * 10 {
            calm.step(Vec3::ZERO, STEP);
        }
        assert!(calm.p.y < kite.p.y - 10.0, "sin viento baja: {} contra {}", calm.p.y, kite.p.y);
    }

    /// Con un ventarron y el hilo corto, la tension lo revienta.
    #[test]
    fn too_much_pull_snaps_the_line() {
        let mut kite = Kite::new(HAND, 30.0, 1);
        let mut snapped = false;
        for _ in 0..240 * 10 {
            snapped |= kite.step(Vec3::new(20.0, 0.0, 0.0), STEP);
        }
        assert!(snapped && kite.free);
    }

    #[test]
    fn segments_that_cross_are_found() {
        let v = |x: f32, y: f32| Vec3::new(x, y, 0.0);
        assert!(crosses(v(0.0, 0.0), v(2.0, 2.0), v(0.0, 2.0), v(2.0, 0.0)));
        assert!(!crosses(v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0), v(1.0, 1.0)));
        assert!(polylines_cross(&[v(-1.0, 0.5), v(0.5, 0.5), v(3.0, 0.5)], &[v(1.0, -1.0), v(1.0, 2.0)]));
    }

    /// Con el piloto automatico aguanta toda la tarde.
    #[test]
    fn the_autopilot_keeps_it_up() {
        let mut g = Barriletes::new(320, 180, Records::in_memory());
        g.start();
        g.set_autopilot(true);
        let mut t = 0.0;
        while g.flying() && t < 90.0 {
            g.update(1.0 / 60.0, &Keys::default());
            t += 1.0 / 60.0;
        }
        assert!(g.flying() || g.reason() == "Se acabó la tarde", "se acabo: {} a los {t:.1} s ({:?})", g.reason(), g.message);
        assert!(g.tally().2 > 15.0);
    }

    #[test]
    fn every_screen_draws_and_its_text_has_glyphs() {
        let (w, h) = (320, 180);
        let mut g = Barriletes::new(w, h, Records::in_memory());
        let mut rgba = vec![0u8; w * h * 4];
        let mut check = |g: &mut Barriletes| {
            rgba.fill(0);
            g.draw(&mut rgba);
            assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
            super::super::assert_loaded_glyphs(&g.overlay());
        };
        check(&mut g);
        g.start();
        for _ in 0..120 {
            g.update(1.0 / 60.0, &Keys::default());
        }
        g.message = Some(("¡Le cortaste el hilo a la seño Juana!".to_string(), g.wall));
        check(&mut g);
        for reason in ["Se cayó el barrilete", "Se fue el barrilete", "Se acabó la tarde", "Bajaste el barrilete"] {
            g.reason = reason;
            g.finish();
            check(&mut g);
            g.start();
        }
    }
}
