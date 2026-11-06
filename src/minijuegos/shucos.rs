//! Shucos del Reformador (la capital, version chapina): una carretilla al
//! pie de la Torre del Reformador, de noche. Llegan clientes con su pedido
//! en un globito y hay que armarselos.
//!
//! - **El shuco**: el pan (1), la carne en la plancha (2 salchicha, 3
//!   longaniza, 4 chorizo, 5 salami: la primera vez se pone a cocinar, la
//!   segunda se pasa al pan) y lo de encima (G guacamol, R repollo, C
//!   cebolla, M mostaza, Y mayonesa, T salsa de tomate; cada vez es una
//!   porcion mas). X tira lo que hay en la tabla y lo quemado de la plancha.
//!   Con las flechas se elige a quien y Enter se lo entrega.
//! - **La plancha**: cada carne tiene su tiempo para quedar lista y para
//!   quemarse. Cruda o quemada te la devuelven.
//! - **Los pedidos** dicen la carne, si va "con todo" o con que, que no le
//!   pongas y que le eches bastante de algo. Mal armado, te lo devuelven (y
//!   se queda en la tabla para arreglarlo); bien, te pagan y, si fue rapido,
//!   dejan propina. Si se cansan de esperar se van enojados.
//! - **La noche** va de las 7 de la noche a las 3 de la manana en 150 s,
//!   con horas pico: la salida de la U (estudiantes), la salida del estadio
//!   (aficionados) y despues de la una, los bolos.
//! - **Los ahorros**: lo que se gana en la noche se guarda y alcanza para
//!   mejoras de la carretilla (plancha de hierro, toldo, ayudante).
//!
//! Todo se pinta en CPU: el fondo (el cielo, los edificios, la torre de
//! celosia, la calle y el mostrador) se hornea una vez y encima, por filas
//! en paralelo, van las figuras de cada frame. Cada figura es una distancia
//! con signo (caja redondeada, disco, segmento o arco, `lienzo`) y la
//! cobertura de su borde la suaviza.

use std::f32::consts::{PI, TAU};

use super::lienzo::{self, cover, halo, over, round_box};
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::marimba::{Instrument, Mallet};
use crate::math::{Vec3, hash_u32, smoothstep};
use crate::parallel;

/// Segundos que dura la noche (de las 19:00 a las 03:00).
const NIGHT: f32 = 150.0;
/// Cuanto se espera despues de cerrar a los que ya estaban.
const LAST_CALL: f32 = 30.0;
/// Lugares en el mostrador.
const SLOTS: usize = 4;
/// Lo que tarda un cliente en llegar o irse.
const WALK: f32 = 1.2;
/// Cuanto aguanta un cliente (s), antes de su caracter.
const PATIENCE: f32 = 32.0;
/// Lo que cuesta cada pan y cada carne.
const BREAD_COST: i64 = 1;
const MEAT_COST: i64 = 3;
/// Cada cuanto hace algo el piloto automatico (s).
const AUTO_STEP: f32 = 0.16;

const SAVINGS: &str = "shucos_ahorros";
const UPGRADES: &str = "shucos_mejoras";
const RECORD: &str = "shucos_record";

/// Las mejoras: nombre, precio, que hacen y su bit.
const SHOP: [(&str, i64, &str); 3] = [
    ("Plancha de hierro", 120, "la carne se cocina 35 % más rápido"),
    ("Toldo y bancos", 100, "la gente aguanta 25 % más"),
    ("Ayudante", 180, "te pone el pan solo después de cada entrega"),
];
const IRON: u8 = 1;
const AWNING: u8 = 2;
const HELPER: u8 = 4;

// Lo que lleva un shuco --------------------------------------------------------

/// Las carnes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meat {
    Salchicha,
    Longaniza,
    Chorizo,
    Salami,
}

impl Meat {
    pub const ALL: [Meat; 4] = [Meat::Salchicha, Meat::Longaniza, Meat::Chorizo, Meat::Salami];

    fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        ["salchicha", "longaniza", "chorizo", "salami"][self.index()]
    }

    fn key(self) -> char {
        ['2', '3', '4', '5'][self.index()]
    }

    fn price(self) -> i64 {
        [10, 12, 12, 10][self.index()]
    }

    /// La salchicha y la longaniza; el chorizo y el salami.
    fn feminine(self) -> bool {
        matches!(self, Meat::Salchicha | Meat::Longaniza)
    }

    /// Segundos en la plancha para quedar lista y para quemarse.
    pub fn times(self) -> (f32, f32) {
        [(3.5, 9.5), (5.0, 11.5), (5.5, 12.0), (3.0, 8.5)][self.index()]
    }

    /// El color crudo.
    fn raw(self) -> Vec3 {
        [Vec3::new(0.88, 0.56, 0.5), Vec3::new(0.7, 0.36, 0.3), Vec3::new(0.78, 0.26, 0.16), Vec3::new(0.8, 0.36, 0.4)][self.index()]
    }
}

/// Como va una carne: cruda, lista o quemada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    Raw,
    Ready,
    Burnt,
}

pub fn doneness(meat: Meat, t: f32) -> Done {
    let (ready, burn) = meat.times();
    if t < ready {
        Done::Raw
    } else if t < burn {
        Done::Ready
    } else {
        Done::Burnt
    }
}

/// El color de una carne a los `t` segundos de plancha: de cruda a dorada
/// hasta que esta lista, y de ahi a negra cuando se quema.
fn meat_color(meat: Meat, t: f32) -> Vec3 {
    let (ready, burn) = meat.times();
    let raw = meat.raw();
    let cooked = raw * Vec3::new(0.66, 0.5, 0.4) + Vec3::new(0.06, 0.03, 0.0);
    if t < ready {
        raw.lerp(cooked, smoothstep(0.0, ready, t))
    } else if t < burn {
        cooked * (1.0 - 0.12 * (t - ready) / (burn - ready))
    } else {
        (cooked * 0.88).lerp(Vec3::new(0.07, 0.055, 0.05), smoothstep(burn, burn + 2.5, t))
    }
}

/// Lo de encima.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Top {
    Guacamol,
    Repollo,
    Cebolla,
    Mostaza,
    Mayonesa,
    Tomate,
}

impl Top {
    pub const ALL: [Top; 6] = [Top::Guacamol, Top::Repollo, Top::Cebolla, Top::Mostaza, Top::Mayonesa, Top::Tomate];

    fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        ["guacamol", "repollo", "cebolla", "mostaza", "mayonesa", "salsa de tomate"][self.index()]
    }

    fn label(self) -> &'static str {
        ["Guacamol", "Repollo", "Cebolla", "Mostaza", "Mayonesa", "Tomate"][self.index()]
    }

    fn key(self) -> char {
        ['g', 'r', 'c', 'm', 'y', 't'][self.index()]
    }

    fn color(self) -> Vec3 {
        [
            Vec3::new(0.42, 0.62, 0.16),
            Vec3::new(0.74, 0.86, 0.58),
            Vec3::new(0.95, 0.93, 0.88),
            Vec3::new(0.96, 0.76, 0.08),
            Vec3::new(0.97, 0.94, 0.8),
            Vec3::new(0.82, 0.07, 0.05),
        ][self.index()]
    }
}

/// Lo que hay en la tabla: las carnes (con sus segundos de plancha) y
/// cuantas porciones de cada cosa de encima.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shuco {
    pub meats: Vec<(Meat, f32)>,
    pub tops: [u8; 6],
}

/// Un pedido: cuantas de cada carne, cuanto de cada cosa (0 si no la
/// pidio), que no lleva, si va "con todo" (y entonces lo demas no importa) y
/// como lo dice.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub meats: [u8; 4],
    pub want: [u8; 6],
    pub forbid: [bool; 6],
    pub everything: bool,
    pub lines: [String; 2],
}

impl Order {
    /// `meats` (una o dos), "con todo" o no, lo que se pide de mas (cuantas
    /// porciones) y lo que no lleva.
    pub fn new(meats: &[Meat], everything: bool, extra: &[(Top, u8)], without: &[Top], line2: &str) -> Self {
        let mut counts = [0u8; 4];
        for m in meats {
            counts[m.index()] += 1;
        }
        let mut forbid = [false; 6];
        for t in without {
            forbid[t.index()] = true;
        }
        let mut want = [0u8; 6];
        if everything {
            for t in Top::ALL {
                if !forbid[t.index()] {
                    want[t.index()] = 1;
                }
            }
        }
        for &(t, n) in extra {
            want[t.index()] = want[t.index()].max(n);
        }
        let line1 = match meats {
            [m] => format!("Uno de {}", m.name()),
            [a, b] => format!("Mixto: {} y {}", a.name(), b.name()),
            _ => "Un shuco".to_string(),
        };
        Self { meats: counts, want, forbid, everything, lines: [line1, line2.to_string()] }
    }

    /// Lo que paga (sin propina): cada carne, y el mixto con descuento.
    pub fn price(&self) -> i64 {
        let n: u8 = self.meats.iter().sum();
        let sum: i64 = Meat::ALL.iter().map(|m| m.price() * self.meats[m.index()] as i64).sum();
        if n >= 2 { sum - 4 } else { sum }
    }

    /// Si `shuco` es lo que pidio, o lo que dice al devolverlo.
    pub fn check(&self, shuco: &Shuco) -> Result<(), String> {
        let mut counts = [0u8; 4];
        for (m, _) in &shuco.meats {
            counts[m.index()] += 1;
        }
        if counts != self.meats {
            let (have, need) = (counts.iter().sum::<u8>(), self.meats.iter().sum::<u8>());
            return Err(if have < need {
                "¡Le falta carne!".to_string()
            } else if have > need {
                "¡Le pusiste carne de más!".to_string()
            } else {
                "¡Esa no es la carne que pedí!".to_string()
            });
        }
        for &(m, t) in &shuco.meats {
            let (article, end) = if m.feminine() { ("La", "a") } else { ("El", "o") };
            match doneness(m, t) {
                Done::Raw => return Err(format!("¡{article} {} está crud{end}!", m.name())),
                Done::Burnt => return Err(format!("¡{article} {} está quemad{end}!", m.name())),
                Done::Ready => {}
            }
        }
        for t in Top::ALL {
            let (have, want) = (shuco.tops[t.index()], self.want[t.index()]);
            if self.forbid[t.index()] && have > 0 {
                return Err(format!("¡Te dije sin {}!", t.name()));
            }
            if have < want {
                return Err(if want >= 2 { format!("Bastante {} te dije", t.name()) } else { format!("Le falta {}", t.name()) });
            }
            if !self.everything && want == 0 && have > 0 {
                return Err(format!("No pedí {}", t.name()));
            }
        }
        Ok(())
    }
}

// Los clientes -----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Who {
    Estudiante,
    Agente,
    Senora,
    Bolo,
    Aficionado,
}

impl Who {
    fn name(self) -> &'static str {
        match self {
            Who::Estudiante => "El de la U",
            Who::Agente => "El agente de la PMT",
            Who::Senora => "La señora",
            Who::Bolo => "El bolo de las 2 a. m.",
            Who::Aficionado => "El del estadio",
        }
    }

    /// Cuanto aguanta, contra el resto.
    fn patience(self) -> f32 {
        match self {
            Who::Estudiante => 1.0,
            Who::Agente => 0.72,
            Who::Senora => 1.0,
            Who::Bolo => 1.45,
            Who::Aficionado => 0.9,
        }
    }

    /// Lo que dice cuando le sale bien.
    fn thanks(self) -> &'static str {
        match self {
            Who::Estudiante => "¡Gracias, seño!",
            Who::Agente => "Buen turno, jefe",
            Who::Senora => "Así me gusta, mijo",
            Who::Bolo => "¡Te quiero un montón, vos!",
            Who::Aficionado => "¡Vamos rojos!",
        }
    }
}

/// Como va un cliente.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mood {
    Arriving(f32),
    Waiting,
    Leaving { happy: bool, t: f32 },
}

#[derive(Clone, Copy, Debug)]
struct Look {
    skin: Vec3,
    shirt: Vec3,
    hair: Vec3,
    seed: u32,
}

#[derive(Clone, Debug)]
struct Customer {
    who: Who,
    slot: usize,
    order: Order,
    patience: f32,
    waited: f32,
    mood: Mood,
    look: Look,
    /// Llega (y se va) por la izquierda.
    left: bool,
    /// Cuando le devolvieron el shuco por ultima vez (se le ve molesto un rato).
    upset: f32,
}

impl Customer {
    fn waiting(&self) -> bool {
        self.mood == Mood::Waiting
    }

    /// Lo que le falta de paciencia (0 a 1).
    fn left_patience(&self) -> f32 {
        (1.0 - self.waited / self.patience).clamp(0.0, 1.0)
    }
}

/// Un texto que sube y se apaga sobre la escena.
#[derive(Clone, Debug)]
struct Floater {
    text: String,
    x: f32,
    y: f32,
    color: [u8; 4],
    age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Puff {
    Steam,
    Smoke,
    Coin,
    Anger,
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    kind: Puff,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
}

/// La noche que se esta jugando.
#[derive(Clone, Debug, Default)]
struct Night {
    clock: f32,
    customers: Vec<Customer>,
    selected: usize,
    /// La plancha: un lugar por carne, con sus segundos de coccion.
    plancha: [Option<f32>; 4],
    bun: Option<Shuco>,
    money: i64,
    tips: i64,
    sold: u32,
    returned: u32,
    angry: u32,
    spawn_in: f32,
    floaters: Vec<Floater>,
    particles: Vec<Particle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Playing,
    Results,
}

/// Los sonidos cortos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sfx {
    Coin,
    Place,
    Squirt,
    Buzz,
    Bell,
    Trash,
    Horn,
}

#[derive(Clone, Copy, Debug)]
struct Voice {
    sfx: Sfx,
    n: u32,
}

// Donde va cada cosa -----------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Layout {
    width: usize,
    height: usize,
    w: f32,
    h: f32,
    s: f32,
    /// La fila del borde del mostrador y el fin del toldo.
    counter: f32,
    awning: f32,
}

impl Layout {
    fn new(width: usize, height: usize) -> Self {
        let (w, h) = (width as f32, height as f32);
        Self { width, height, w, h, s: h / 720.0, counter: (0.6 * h).round(), awning: 0.045 * h }
    }

    fn slot_x(&self, slot: usize) -> f32 {
        self.w * (0.28 + 0.195 * slot as f32)
    }

    /// El centro del globito de cada lugar: alternan arriba y abajo para
    /// que quepan.
    fn bubble_y(&self, slot: usize) -> f32 {
        if slot % 2 == 1 { 0.125 * self.h } else { 0.255 * self.h }
    }

    fn head_y(&self) -> f32 {
        self.counter - 175.0 * self.s
    }

    fn basket(&self) -> (f32, f32) {
        (self.w * 0.075, self.h * 0.8)
    }

    /// Centro y medios lados de la plancha.
    fn plancha(&self) -> (f32, f32, f32, f32) {
        (self.w * 0.285, self.h * 0.79, (self.w * 0.135).min(190.0 * self.s), 92.0 * self.s)
    }

    fn meat_at(&self, i: usize) -> (f32, f32) {
        let (cx, cy, hx, hy) = self.plancha();
        let col = if i % 2 == 0 { -0.5 } else { 0.5 };
        let row = if i < 2 { -0.48 } else { 0.48 };
        (cx + col * hx, cy + row * hy)
    }

    fn board(&self) -> (f32, f32, f32, f32) {
        (self.w * 0.565, self.h * 0.8, 150.0 * self.s, 70.0 * self.s)
    }

    fn top_at(&self, k: usize) -> (f32, f32) {
        (self.w * (0.775 + 0.075 * (k % 3) as f32), self.h * (0.715 + 0.16 * (k / 3) as f32))
    }
}

// Las figuras ------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Form {
    /// Caja redondeada con medios lados y radio de esquina.
    Rect { hx: f32, hy: f32, r: f32 },
    Disc { r: f32 },
    /// Segmento de `(x, y)` a `(bx, by)` con radio `r` (una capsula).
    Line { bx: f32, by: f32, r: f32 },
    /// Arco de radio `r` y medio grueso `t` entre los angulos `from` y `to`
    /// (con y hacia abajo: de 0 a pi es la mitad de abajo).
    Arc { r: f32, t: f32, from: f32, to: f32 },
}

/// Una figura: su forma, su color, cuanto cubre, que tan suave es su borde
/// (pixeles) y su resplandor (`glow` pixeles de alcance, `glow_k` fuerza).
#[derive(Clone, Copy, Debug)]
struct Shape {
    x: f32,
    y: f32,
    form: Form,
    color: Vec3,
    alpha: f32,
    soft: f32,
    glow: f32,
    glow_k: f32,
    /// La caja que ocupa (sin el borde suave ni el resplandor) y cuanto
    /// agregarle.
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    pad: f32,
}

impl Shape {
    fn alpha(&mut self, a: f32) -> &mut Self {
        self.alpha = a;
        self
    }

    fn soft(&mut self, s: f32) -> &mut Self {
        self.soft = s.max(0.5);
        self.repad();
        self
    }

    fn glow(&mut self, spread: f32, k: f32) -> &mut Self {
        self.glow = spread;
        self.glow_k = k;
        self.repad();
        self
    }

    fn repad(&mut self) {
        self.pad = 1.0 + 1.5 * self.soft + 5.0 * self.glow;
    }

    /// Distancia con signo (pixeles) del punto al borde.
    fn sdf(&self, px: f32, py: f32) -> f32 {
        let (dx, dy) = (px - self.x, py - self.y);
        match self.form {
            Form::Rect { hx, hy, r } => round_box(dx, dy, hx, hy, r),
            Form::Disc { r } => dx.hypot(dy) - r,
            Form::Line { bx, by, r } => segment(px, py, (self.x, self.y), (bx, by)) - r,
            Form::Arc { r, t, from, to } => {
                let angle = dy.atan2(dx);
                if angle >= from && angle <= to {
                    (dx.hypot(dy) - r).abs() - t
                } else {
                    let end = |a: f32| {
                        let (s, c) = a.sin_cos();
                        (dx - c * r).hypot(dy - s * r)
                    };
                    end(from).min(end(to)) - t
                }
            }
        }
    }
}

/// Distancia de `(x, y)` al segmento `a b`.
fn segment(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (x - a.0, y - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 { ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (apx - abx * t).hypot(apy - aby * t)
}

#[derive(Default)]
struct Canvas {
    shapes: Vec<Shape>,
}

impl Canvas {
    fn push(&mut self, x: f32, y: f32, form: Form, color: Vec3, bounds: (f32, f32, f32, f32)) -> &mut Shape {
        let mut shape = Shape { x, y, form, color, alpha: 1.0, soft: 1.0, glow: 0.0, glow_k: 0.0, x0: bounds.0, y0: bounds.1, x1: bounds.2, y1: bounds.3, pad: 0.0 };
        shape.repad();
        self.shapes.push(shape);
        self.shapes.last_mut().expect("recien puesta")
    }

    fn rect(&mut self, x: f32, y: f32, hx: f32, hy: f32, r: f32, color: Vec3) -> &mut Shape {
        self.push(x, y, Form::Rect { hx, hy, r }, color, (x - hx, y - hy, x + hx, y + hy))
    }

    fn disc(&mut self, x: f32, y: f32, r: f32, color: Vec3) -> &mut Shape {
        self.push(x, y, Form::Disc { r }, color, (x - r, y - r, x + r, y + r))
    }

    fn line(&mut self, ax: f32, ay: f32, bx: f32, by: f32, r: f32, color: Vec3) -> &mut Shape {
        self.push(ax, ay, Form::Line { bx, by, r }, color, (ax.min(bx) - r, ay.min(by) - r, ax.max(bx) + r, ay.max(by) + r))
    }

    fn arc(&mut self, x: f32, y: f32, r: f32, t: f32, from: f32, to: f32, color: Vec3) -> &mut Shape {
        let e = r + t;
        self.push(x, y, Form::Arc { r, t, from, to }, color, (x - e, y - e, x + e, y + e))
    }
}

/// Pinta las figuras que tocan la fila `yf`, en orden.
fn paint(shapes: &[Shape], yf: f32, row: &mut [u8], width: usize) {
    for sh in shapes {
        if yf < sh.y0 - sh.pad || yf > sh.y1 + sh.pad {
            continue;
        }
        let x0 = ((sh.x0 - sh.pad).floor().max(0.0) as usize).min(width);
        let x1 = ((sh.x1 + sh.pad).ceil().max(0.0) as usize).min(width);
        for x in x0..x1 {
            let xf = x as f32 + 0.5;
            let d = sh.sdf(xf, yf);
            let a = cover(d / sh.soft) * sh.alpha;
            let g = if sh.glow > 0.0 && d > 0.0 { halo(d, sh.glow) * sh.glow_k } else { 0.0 };
            if a <= 0.002 && g <= 0.002 {
                continue;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let mut c = lienzo::read(p);
            if g > 0.0 {
                c = c + sh.color * g;
            }
            if a > 0.0 {
                c = over(c, sh.color, a);
            }
            lienzo::write(p, c);
        }
    }
}

/// Un numero de 0 a 1 de un hash.
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

// El fondo -----------------------------------------------------------------------

/// Los colores de la faja del toldo.
const FAJA: [Vec3; 4] = [Vec3::new(0.72, 0.06, 0.1), Vec3::new(0.98, 0.78, 0.12), Vec3::new(0.08, 0.5, 0.28), Vec3::new(0.12, 0.3, 0.75)];
const SODIUM: Vec3 = Vec3::new(1.0, 0.62, 0.25);
const IRON_COLOR: Vec3 = Vec3::new(0.11, 0.075, 0.08);
const STEEL: Vec3 = Vec3::new(0.44, 0.46, 0.49);

/// El color del fondo en un pixel: el toldo arriba, el cielo de noche, los
/// edificios con ventanas, la 7a Avenida con su paso de cebra y la banqueta,
/// y abajo el mostrador de acero.
fn backdrop(lay: &Layout, x: f32, y: f32) -> Vec3 {
    let (w, h, s) = (lay.w, lay.h, lay.s);
    let street = 0.5 * h;
    let walk = 0.565 * h;
    if y >= lay.counter {
        // El mostrador: acero cepillado (una veta por fila), el filo claro
        // arriba y mas oscuro hacia uno, con el reflejo calido de los postes.
        let lip = lay.counter + 10.0 * s;
        if y < lip {
            let t = (y - lay.counter) / (lip - lay.counter);
            return Vec3::new(0.86, 0.87, 0.9).lerp(Vec3::new(0.55, 0.57, 0.6), t);
        }
        let grain = unit(hash_u32(y as u32 * 2_654_435_761)) * 0.05 - 0.025;
        let depth = (y - lip) / (h - lip);
        let warm = SODIUM * (0.06 * (-((x - 0.5 * w) / (0.35 * w)).powi(2)).exp());
        return STEEL * (1.0 - 0.35 * depth) + Vec3::splat(grain) + warm;
    }
    // El toldo de faja con su orilla de ondas.
    let stripe = 42.0 * s;
    let k = (x / stripe).floor();
    let u = x / stripe - k;
    let scallop = lay.awning + 9.0 * s * (1.0 - (2.0 * u - 1.0).powi(2));
    if y < scallop {
        let base = FAJA[(k as i32).rem_euclid(4) as usize];
        let shade = 0.75 + 0.25 * (1.0 - (2.0 * u - 1.0).abs());
        return base * shade * (0.8 + 0.2 * (y / scallop));
    }
    if y < street {
        // El cielo: indigo arriba, violeta y sodio hacia la calle.
        let t = y / street;
        let mut c = Vec3::new(0.02, 0.022, 0.06).lerp(Vec3::new(0.17, 0.08, 0.17), t.powf(2.2));
        c = c + SODIUM * (0.22 * smoothstep(0.6, 1.0, t));
        let star = hash_u32((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663));
        if star % 700 == 0 && t < 0.55 {
            c = c + Vec3::splat(0.35 + 0.5 * unit(star));
        }
        // Los edificios del centro, con ventanas encendidas.
        let block = 46.0 * s;
        let b = (x / block).floor() as i32;
        let hb = hash_u32(b as u32 ^ 0xB11D);
        let top = street - (0.05 + 0.17 * unit(hb)) * h;
        if y >= top && (x - 0.5 * w).abs() > 0.03 * w {
            let mut c = Vec3::new(0.045, 0.04, 0.065) * (0.8 + 0.4 * unit(hb >> 3));
            let (cx, cy) = ((x / (9.0 * s)).floor() as i32, ((y - top) / (12.0 * s)).floor() as i32);
            let (fx, fy) = ((x / (9.0 * s)).fract(), ((y - top) / (12.0 * s)).fract());
            let lit = hash_u32((cx as u32).wrapping_mul(0x9E37) ^ (cy as u32).wrapping_mul(0x85EB) ^ hb);
            if cy >= 1 && fx > 0.25 && fx < 0.75 && fy > 0.3 && fy < 0.8 && lit % 3 == 0 {
                let warm = if lit % 7 == 0 { Vec3::new(0.7, 0.85, 1.0) } else { Vec3::new(1.0, 0.72, 0.38) };
                c = warm * (0.45 + 0.4 * unit(lit));
            }
            return c;
        }
        return c;
    }
    // La calle: asfalto con los charcos de luz de los postes, el paso de
    // cebra y la banqueta.
    let pool = |lx: f32| (-((x - lx) / (0.1 * w)).powi(2)).exp();
    let light = SODIUM * (0.2 * (pool(0.04 * w) + pool(0.96 * w)) + 0.05);
    if y >= walk {
        let curb = (y - walk) < 3.0 * s;
        return if curb { Vec3::new(0.42, 0.4, 0.38) } else { Vec3::new(0.2, 0.19, 0.2) } + light * 0.6;
    }
    let mut c = Vec3::new(0.055, 0.055, 0.065) + light * 0.5;
    let zebra = ((x - 0.5 * w) / (24.0 * s)).floor() as i32;
    if (x - 0.5 * w).abs() < 0.28 * w && zebra.rem_euclid(2) == 0 && y > street + 0.012 * h && y < walk - 0.008 * h {
        c = Vec3::new(0.62, 0.62, 0.6) * 0.8 + light * 0.4;
    }
    c
}

/// Lo que no se mueve: la Torre del Reformador, los postes y lo del
/// mostrador (la canasta del pan, la plancha, la tabla, los tazones y los
/// botes).
fn static_shapes(lay: &Layout, c: &mut Canvas) {
    let (w, h, s) = (lay.w, lay.h, lay.s);
    // La torre: dos patas de celosia que suben juntandose (curva
    // `x = 0.11 w (1 - t)^1.6`), cruces entre ellas, dos plataformas, el
    // arco de abajo y la caja de la campana con su faro rojo.
    let (cx, base, top) = (0.5 * w, 0.5 * h, 0.085 * h);
    let leg = |t: f32| 0.11 * w * (1.0 - t).powf(1.6) + 0.011 * w;
    let at = |t: f32| base + (top - base) * t;
    let steps = 14;
    for side in [-1.0f32, 1.0] {
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            c.line(cx + side * leg(t0), at(t0), cx + side * leg(t1), at(t1), 3.0 * s, IRON_COLOR);
            c.line(cx + side * (leg(t0) - 0.012 * w * (1.0 - t0)), at(t0), cx + side * (leg(t1) - 0.012 * w * (1.0 - t1)), at(t1), 1.6 * s, IRON_COLOR);
        }
    }
    for i in 1..steps {
        let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
        if t0 < 0.22 {
            continue;
        }
        c.line(cx - leg(t0), at(t0), cx + leg(t1), at(t1), 1.2 * s, IRON_COLOR);
        c.line(cx + leg(t0), at(t0), cx - leg(t1), at(t1), 1.2 * s, IRON_COLOR);
    }
    for t in [0.36f32, 0.64] {
        c.rect(cx, at(t), leg(t) + 6.0 * s, 3.5 * s, 1.0 * s, IRON_COLOR);
    }
    // El arco entre las patas, abajo.
    let arch_top = at(0.2);
    let n = 16;
    for i in 0..n {
        let (a0, a1) = (PI * i as f32 / n as f32, PI * (i + 1) as f32 / n as f32);
        let half = leg(0.0) * 0.86;
        let p = |a: f32| (cx - half * a.cos(), base - (base - arch_top) * a.sin());
        let (p0, p1) = (p(a0), p(a1));
        c.line(p0.0, p0.1, p1.0, p1.1, 2.4 * s, IRON_COLOR);
    }
    c.rect(cx, top - 10.0 * s, 0.014 * w, 12.0 * s, 2.0 * s, IRON_COLOR);
    c.line(cx, top - 22.0 * s, cx, top - 40.0 * s, 1.4 * s, IRON_COLOR);
    c.disc(cx, top - 42.0 * s, 3.5 * s, Vec3::new(1.0, 0.15, 0.1)).glow(10.0 * s, 0.8);
    // Los focos de la torre, a lo largo de las patas.
    for i in 0..10 {
        let t = 0.08 + 0.09 * i as f32;
        for side in [-1.0f32, 1.0] {
            c.disc(cx + side * leg(t), at(t), 1.8 * s, Vec3::new(1.0, 0.86, 0.6)).glow(5.0 * s, 0.5);
        }
    }
    // Los postes de sodio a los lados.
    for (lx, dir) in [(0.04 * w, 1.0f32), (0.96 * w, -1.0)] {
        let (foot, head) = (0.57 * h, 0.2 * h);
        c.line(lx, foot, lx, head, 3.2 * s, Vec3::new(0.12, 0.12, 0.13));
        c.line(lx, head, lx + dir * 34.0 * s, head - 6.0 * s, 2.4 * s, Vec3::new(0.12, 0.12, 0.13));
        c.rect(lx + dir * 40.0 * s, head - 4.0 * s, 14.0 * s, 4.0 * s, 2.0 * s, Vec3::new(1.0, 0.8, 0.5)).glow(30.0 * s, 0.6);
    }

    // El mostrador. La canasta del pan, tejida.
    let (bx, by) = lay.basket();
    c.rect(bx, by, 62.0 * s, 58.0 * s, 16.0 * s, Vec3::new(0.45, 0.27, 0.12));
    for i in 0..6 {
        let y = by - 45.0 * s + i as f32 * 18.0 * s;
        c.line(bx - 55.0 * s, y, bx + 55.0 * s, y, 2.0 * s, Vec3::new(0.32, 0.18, 0.08)).alpha(0.8);
    }
    for i in 0..3 {
        let y = by - 26.0 * s + i as f32 * 26.0 * s;
        c.line(bx - 34.0 * s, y, bx + 34.0 * s, y - 3.0 * s, 13.0 * s, Vec3::new(0.86, 0.6, 0.3));
        c.line(bx - 30.0 * s, y - 6.0 * s, bx + 30.0 * s, y - 9.0 * s, 3.0 * s, Vec3::new(0.97, 0.78, 0.5)).alpha(0.7);
    }
    // La plancha: hierro negro con su borde y las rayas de la parrilla.
    let (px, py, phx, phy) = lay.plancha();
    c.rect(px, py, phx + 8.0 * s, phy + 8.0 * s, 12.0 * s, Vec3::new(0.22, 0.22, 0.23));
    c.rect(px, py, phx, phy, 8.0 * s, Vec3::new(0.09, 0.09, 0.1));
    let lines = 9;
    for i in 1..lines {
        let x = px - phx + 2.0 * phx * i as f32 / lines as f32;
        c.line(x, py - phy + 6.0 * s, x, py + phy - 6.0 * s, 1.2 * s, Vec3::new(0.16, 0.15, 0.15)).alpha(0.8);
    }
    // La tabla de picar.
    let (tx, ty, thx, thy) = lay.board();
    c.rect(tx, ty, thx, thy, 14.0 * s, Vec3::new(0.52, 0.33, 0.17));
    for i in 0..5 {
        let y = ty - thy + (i as f32 + 0.6) * 2.0 * thy / 5.5;
        c.line(tx - thx + 12.0 * s, y, tx + thx - 12.0 * s, y + 4.0 * s, 1.2 * s, Vec3::new(0.62, 0.42, 0.24)).alpha(0.6);
    }
    // Los tazones (guacamol, repollo, cebolla) y los botes (mostaza,
    // mayonesa, salsa de tomate).
    for t in Top::ALL {
        let (x, y) = lay.top_at(t.index());
        let col = t.color();
        if t.index() < 3 {
            c.disc(x, y, 40.0 * s, Vec3::new(0.86, 0.86, 0.84));
            c.disc(x, y, 33.0 * s, col * 0.9);
            let seed = t.index() as u32 * 977;
            for k in 0..14u32 {
                let hk = hash_u32(seed + k);
                let (a, r) = (TAU * unit(hk), 26.0 * s * unit(hk >> 5).sqrt());
                let (dx, dy) = (a.cos() * r, a.sin() * r);
                match t {
                    Top::Repollo => {
                        c.line(x + dx - 6.0 * s, y + dy, x + dx + 6.0 * s, y + dy - 3.0 * s, 1.6 * s, Vec3::new(0.9, 0.95, 0.8));
                    }
                    Top::Cebolla => {
                        c.rect(x + dx, y + dy, 3.0 * s, 3.0 * s, 1.0 * s, Vec3::new(0.85, 0.82, 0.88));
                    }
                    _ => {
                        c.disc(x + dx, y + dy, 3.0 * s, col * 1.25).alpha(0.6);
                    }
                }
            }
        } else {
            c.rect(x, y, 19.0 * s, 42.0 * s, 9.0 * s, col);
            c.rect(x - 7.0 * s, y - 4.0 * s, 4.0 * s, 30.0 * s, 4.0 * s, Vec3::splat(1.0)).alpha(0.25);
            c.rect(x, y - 47.0 * s, 10.0 * s, 7.0 * s, 3.0 * s, Vec3::new(0.95, 0.95, 0.95) * if t == Top::Tomate { 0.25 } else { 0.9 });
            c.line(x, y - 54.0 * s, x + 3.0 * s, y - 64.0 * s, 2.5 * s, Vec3::new(0.9, 0.9, 0.9) * if t == Top::Tomate { 0.25 } else { 0.95 });
        }
    }
}

/// Hornea el fondo de una vez.
fn bake_scene(lay: &Layout) -> Vec<u8> {
    let (w, h) = (lay.width, lay.height);
    let mut img = vec![0u8; w * h * 4];
    parallel::chunks_mut(&mut img, w * 4, 1, |y, row| {
        let yf = y as f32 + 0.5;
        for x in 0..w {
            lienzo::write(&mut row[x * 4..x * 4 + 4], backdrop(lay, x as f32 + 0.5, yf));
        }
    });
    let mut c = Canvas::default();
    static_shapes(lay, &mut c);
    parallel::chunks_mut(&mut img, w * 4, 1, |y, row| paint(&c.shapes, y as f32 + 0.5, row, w));
    img
}

// Dibujar lo que se mueve --------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Face {
    Calm,
    Happy,
    Angry,
}

/// Una persona detras del mostrador, centrada en `x` (lo de abajo lo tapa
/// el mostrador).
fn person(c: &mut Canvas, lay: &Layout, who: Who, look: &Look, x: f32, bob: f32, face: Face) {
    let s = lay.s;
    let base = lay.counter;
    let hy = lay.head_y() + bob;
    let shirt = look.shirt;
    let skin = if face == Face::Angry { look.skin.lerp(Vec3::new(0.85, 0.2, 0.15), 0.3) } else { look.skin };
    // Atras de la cabeza: el pelo largo y el chongo de la senora.
    if who == Who::Senora {
        c.disc(x + 22.0 * s, hy - 26.0 * s, 15.0 * s, look.hair);
    }
    // El cuerpo y el cuello.
    c.rect(x, base - 50.0 * s + 0.5 * bob, 52.0 * s, 90.0 * s, 30.0 * s, shirt);
    c.rect(x, hy + 34.0 * s, 12.0 * s, 14.0 * s, 5.0 * s, skin * 0.85);
    match who {
        Who::Estudiante => {
            // Los tirantes de la mochila.
            for side in [-1.0f32, 1.0] {
                c.line(x + side * 28.0 * s, base - 128.0 * s + 0.5 * bob, x + side * 32.0 * s, base - 10.0 * s, 5.5 * s, Vec3::new(0.12, 0.12, 0.16));
            }
        }
        Who::Agente => {
            // El chaleco con sus franjas que brillan y la placa.
            for dy in [-82.0f32, -42.0] {
                c.rect(x, base + dy * s + 0.5 * bob, 52.0 * s, 5.0 * s, 2.0 * s, Vec3::new(0.85, 0.95, 0.2)).glow(3.0 * s, 0.25);
            }
            c.disc(x - 24.0 * s, base - 108.0 * s + 0.5 * bob, 6.0 * s, Vec3::new(0.95, 0.78, 0.25));
        }
        Who::Senora => {
            // El delantal con florecitas.
            c.rect(x, base - 34.0 * s + 0.5 * bob, 40.0 * s, 52.0 * s, 12.0 * s, Vec3::new(0.93, 0.91, 0.86));
            for k in 0..6u32 {
                let hk = hash_u32(look.seed ^ (k * 977));
                c.disc(x + (unit(hk) - 0.5) * 60.0 * s, base - 70.0 * s + unit(hk >> 6) * 60.0 * s, 4.0 * s, Vec3::new(0.9, 0.35, 0.55));
            }
        }
        Who::Bolo => {
            // La camisa de cuadros (rayas mas claras) por fuera.
            for k in -2..=2 {
                c.line(x + k as f32 * 18.0 * s, base - 130.0 * s, x + k as f32 * 18.0 * s, base, 2.0 * s, shirt * 1.35).alpha(0.6);
            }
        }
        Who::Aficionado => {
            // La camisola roja con su franja blanca y la bufanda.
            c.rect(x, base - 92.0 * s + 0.5 * bob, 52.0 * s, 7.0 * s, 2.0 * s, Vec3::new(0.95, 0.95, 0.95));
            c.line(x - 26.0 * s, base - 130.0 * s, x + 18.0 * s, base - 40.0 * s, 8.0 * s, Vec3::new(0.95, 0.95, 0.95));
            c.line(x - 18.0 * s, base - 116.0 * s, x + 4.0 * s, base - 72.0 * s, 8.5 * s, Vec3::new(0.8, 0.08, 0.1));
        }
    }
    // Los brazos con las manos en el mostrador (el bolo, con su botella).
    for side in [-1.0f32, 1.0] {
        let hand = (x + side * 64.0 * s, base - 6.0 * s);
        c.line(x + side * 42.0 * s, base - 112.0 * s + 0.5 * bob, hand.0, hand.1, 12.0 * s, shirt * 0.88);
        c.disc(hand.0, hand.1, 11.0 * s, skin);
    }
    if who == Who::Bolo {
        let (hx, hy2) = (x + 64.0 * s, base - 6.0 * s);
        c.line(hx + 4.0 * s, hy2 - 8.0 * s, hx + 10.0 * s, hy2 - 58.0 * s, 8.0 * s, Vec3::new(0.5, 0.3, 0.08)).alpha(0.9);
        c.line(hx + 10.0 * s, hy2 - 58.0 * s, hx + 12.0 * s, hy2 - 74.0 * s, 3.5 * s, Vec3::new(0.5, 0.3, 0.08));
    }
    // El pelo (un disco detras de la cabeza: se ve la orilla de arriba) y la
    // cabeza.
    match who {
        Who::Bolo => {
            for k in 0..7u32 {
                let a = PI + PI * (k as f32 + 0.5) / 7.0;
                c.disc(x + a.cos() * 26.0 * s, hy + a.sin() * 26.0 * s - 4.0 * s, 10.0 * s, look.hair);
            }
        }
        _ => {
            c.disc(x, hy - 7.0 * s, 32.5 * s, look.hair);
        }
    }
    c.disc(x, hy + 2.0 * s, 30.0 * s, skin);
    // La cara: ojos, boca y lo de cada quien.
    let eye_y = hy - 2.0 * s;
    if who == Who::Bolo {
        for side in [-1.0f32, 1.0] {
            c.line(x + side * 10.0 * s - 4.0 * s, eye_y, x + side * 10.0 * s + 4.0 * s, eye_y + 1.0 * s, 1.6 * s, Vec3::new(0.08, 0.06, 0.06));
        }
        c.disc(x, hy + 6.0 * s, 6.5 * s, Vec3::new(0.82, 0.28, 0.26));
    } else {
        for side in [-1.0f32, 1.0] {
            c.disc(x + side * 10.0 * s, eye_y, 3.6 * s, Vec3::new(0.06, 0.05, 0.06));
        }
    }
    if face == Face::Angry {
        for side in [-1.0f32, 1.0] {
            c.line(x + side * 15.0 * s, eye_y - 10.0 * s, x + side * 5.0 * s, eye_y - 6.0 * s, 1.8 * s, Vec3::new(0.08, 0.05, 0.05));
        }
    }
    let mouth = Vec3::new(0.35, 0.1, 0.1);
    match face {
        Face::Happy => {
            c.arc(x, hy + 6.0 * s, 11.0 * s, 1.8 * s, 0.2 * PI, 0.8 * PI, mouth);
        }
        Face::Angry => {
            c.arc(x, hy + 24.0 * s, 10.0 * s, 1.8 * s, -0.8 * PI, -0.2 * PI, mouth);
        }
        Face::Calm => {
            c.line(x - 7.0 * s, hy + 15.0 * s, x + 7.0 * s, hy + 15.0 * s, 1.6 * s, mouth);
        }
    }
    match who {
        Who::Estudiante => {
            for side in [-1.0f32, 1.0] {
                c.arc(x + side * 10.0 * s, eye_y, 8.0 * s, 1.3 * s, -PI, PI, Vec3::new(0.1, 0.1, 0.12));
            }
            c.line(x - 2.0 * s, eye_y, x + 2.0 * s, eye_y, 1.2 * s, Vec3::new(0.1, 0.1, 0.12));
        }
        Who::Agente => {
            // La gorra con su visera y su escudo.
            c.rect(x, hy - 25.0 * s, 32.0 * s, 12.0 * s, 9.0 * s, Vec3::new(0.08, 0.12, 0.32));
            c.rect(x, hy - 14.0 * s, 37.0 * s, 4.0 * s, 3.0 * s, Vec3::new(0.03, 0.03, 0.05));
            c.disc(x, hy - 26.0 * s, 5.0 * s, Vec3::new(0.95, 0.78, 0.25));
        }
        Who::Senora => {
            for side in [-1.0f32, 1.0] {
                c.disc(x + side * 30.0 * s, hy + 12.0 * s, 3.5 * s, Vec3::new(0.95, 0.78, 0.25));
            }
        }
        Who::Aficionado => {
            // La gorra al reves.
            c.rect(x, hy - 24.0 * s, 31.0 * s, 11.0 * s, 9.0 * s, Vec3::new(0.8, 0.08, 0.1));
            c.rect(x + 2.0 * s, hy - 33.0 * s, 16.0 * s, 4.0 * s, 3.0 * s, Vec3::new(0.6, 0.05, 0.08));
        }
        Who::Bolo => {}
    }
}

/// Una carne en `(x, y)` a lo largo (`len` es medio largo) a los `t`
/// segundos de plancha. En la plancha chirria (puntitos que brillan).
fn draw_meat(c: &mut Canvas, meat: Meat, x: f32, y: f32, len: f32, t: f32, s: f32, sizzle: Option<f32>) {
    let col = meat_color(meat, t);
    let (ready, _) = meat.times();
    match meat {
        Meat::Salchicha => {
            c.line(x - len, y, x + len, y, 13.0 * s, col);
        }
        Meat::Longaniza => {
            c.line(x - len, y, x + len, y, 14.5 * s, col);
            for k in -2..=2 {
                let xk = x + k as f32 * len * 0.4;
                c.line(xk - 2.0 * s, y - 12.0 * s, xk + 2.0 * s, y + 12.0 * s, 1.6 * s, col * 0.7).alpha(0.8);
            }
        }
        Meat::Chorizo => {
            c.line(x - 0.8 * len, y, x + 0.8 * len, y, 16.0 * s, col);
            for side in [-1.0f32, 1.0] {
                c.disc(x + side * (0.8 * len + 14.0 * s), y, 4.0 * s, col * 0.6);
            }
        }
        Meat::Salami => {
            for k in [-1.0f32, 0.0, 1.0] {
                c.disc(x + k * len * 0.55, y + k.abs() * 2.0 * s, 19.0 * s, col);
                for j in 0..4u32 {
                    let hj = hash_u32(j * 31 + (k + 2.0) as u32 * 7);
                    c.disc(x + k * len * 0.55 + (unit(hj) - 0.5) * 22.0 * s, y + (unit(hj >> 7) - 0.5) * 22.0 * s, 2.4 * s, Vec3::new(0.95, 0.85, 0.8) * (0.4 + 0.6 * col.max_element())).alpha(0.8);
                }
            }
        }
    }
    // El brillo de arriba y las rayas de la parrilla.
    if meat != Meat::Salami {
        c.line(x - 0.7 * len, y - 6.0 * s, x + 0.7 * len, y - 6.0 * s, 3.0 * s, Vec3::splat(1.0)).alpha(0.22);
    }
    let marks = smoothstep(0.25 * ready, ready, t);
    if marks > 0.0 {
        for k in -2..=2 {
            let xk = x + k as f32 * len * 0.3;
            c.line(xk - 5.0 * s, y + 9.0 * s, xk + 5.0 * s, y - 9.0 * s, 2.0 * s, Vec3::new(0.12, 0.06, 0.03)).alpha(0.55 * marks);
        }
    }
    if let Some(wall) = sizzle {
        let burnt = doneness(meat, t) == Done::Burnt;
        for k in 0..5u32 {
            let hk = hash_u32(k * 7919 + (wall * 20.0) as u32 * 31 + meat.index() as u32 * 131);
            if hk % 3 == 0 && !burnt {
                let px = x + (unit(hk) - 0.5) * 2.2 * len;
                let py = y + (unit(hk >> 9) - 0.5) * 30.0 * s;
                c.disc(px, py, 1.6 * s, Vec3::new(1.0, 0.92, 0.7)).glow(3.0 * s, 0.6);
            }
        }
    }
}

/// El shuco en la tabla, armado por capas como uno de verdad: el pan, las
/// carnes, el guacamol, el repollo, la cebolla y las salsas en zigzag.
fn draw_bun(c: &mut Canvas, lay: &Layout, shuco: &Shuco) {
    let s = lay.s;
    let (bx, by, _, _) = lay.board();
    let half = 112.0 * s;
    // El pan abierto: la tapa atras, la base adelante.
    c.line(bx - half, by - 18.0 * s, bx + half, by - 18.0 * s, 24.0 * s, Vec3::new(0.82, 0.55, 0.26));
    c.line(bx - half, by + 10.0 * s, bx + half, by + 10.0 * s, 28.0 * s, Vec3::new(0.88, 0.62, 0.3));
    c.line(bx - half * 0.9, by - 2.0 * s, bx + half * 0.9, by - 2.0 * s, 10.0 * s, Vec3::new(0.96, 0.86, 0.66));
    c.line(bx - half * 0.85, by + 26.0 * s, bx + half * 0.85, by + 26.0 * s, 4.0 * s, Vec3::new(1.0, 0.85, 0.55)).alpha(0.5);
    let n = shuco.meats.len().max(1) as f32;
    for (i, &(meat, t)) in shuco.meats.iter().enumerate() {
        let off = if shuco.meats.len() > 1 { (i as f32 / (n - 1.0) - 0.5) * 10.0 * s } else { 0.0 };
        draw_meat(c, meat, bx, by - 6.0 * s + off, half * 0.85, t, s, None);
    }
    let layer = |k: usize| shuco.tops[k];
    // Guacamol: una tira gorda, mas gorda si va bastante.
    let g = layer(Top::Guacamol.index());
    if g > 0 {
        c.line(bx - half * 0.8, by - 24.0 * s, bx + half * 0.8, by - 24.0 * s, (6.0 + 3.5 * g as f32) * s, Top::Guacamol.color());
        c.line(bx - half * 0.6, by - 28.0 * s, bx + half * 0.5, by - 28.0 * s, 2.0 * s, Vec3::new(0.65, 0.82, 0.35)).alpha(0.6);
    }
    // Repollo: tiritas.
    for j in 0..(layer(Top::Repollo.index()) as u32 * 24) {
        let hj = hash_u32(j * 2_654_435_761 ^ 0x5E);
        let x = bx + (unit(hj) - 0.5) * 1.8 * half;
        let y = by - 33.0 * s + (unit(hj >> 8) - 0.5) * 16.0 * s;
        let a = (unit(hj >> 16) - 0.5) * 1.6;
        c.line(x - a.cos() * 9.0 * s, y - a.sin() * 9.0 * s, x + a.cos() * 9.0 * s, y + a.sin() * 9.0 * s, 2.6 * s, Vec3::new(0.86, 0.94, 0.74));
        c.line(x - a.cos() * 7.0 * s, y - a.sin() * 7.0 * s - 0.8 * s, x + a.cos() * 7.0 * s, y + a.sin() * 7.0 * s - 0.8 * s, 0.9 * s, Vec3::new(0.95, 1.0, 0.9)).alpha(0.7);
    }
    // Cebolla: cuadritos.
    for j in 0..(layer(Top::Cebolla.index()) as u32 * 14) {
        let hj = hash_u32(j * 40_503 ^ 0xCEB0);
        let (x, y) = (bx + (unit(hj) - 0.5) * 1.7 * half, by - 34.0 * s + (unit(hj >> 8) - 0.5) * 10.0 * s);
        c.rect(x, y, 4.2 * s, 4.2 * s, 1.2 * s, Vec3::new(0.55, 0.5, 0.55));
        c.rect(x, y - 0.6 * s, 3.4 * s, 3.4 * s, 1.0 * s, Vec3::new(0.97, 0.95, 1.0));
    }
    // Las salsas: zigzags, uno por porcion.
    let mut lane = 0;
    for t in [Top::Mostaza, Top::Mayonesa, Top::Tomate] {
        for _ in 0..layer(t.index()) {
            let y = by - 30.0 * s - lane as f32 * 5.0 * s;
            let zig = 10;
            for k in 0..zig {
                let (x0, x1) = (bx - half * 0.85 + 1.7 * half * k as f32 / zig as f32, bx - half * 0.85 + 1.7 * half * (k + 1) as f32 / zig as f32);
                let (y0, y1) = if k % 2 == 0 { (y - 5.0 * s, y + 5.0 * s) } else { (y + 5.0 * s, y - 5.0 * s) };
                c.line(x0, y0, x1, y1, 2.4 * s, t.color());
            }
            lane += 1;
        }
    }
}

// El juego ------------------------------------------------------------------------

pub struct Shucos {
    lay: Layout,
    scene: Vec<u8>,
    records: Records,
    screen: Screen,
    choice: usize,
    notice: Option<(String, f32)>,
    savings: i64,
    upgrades: u8,
    night: Night,
    best: Option<i64>,
    autopilot: bool,
    auto_wait: f32,
    wall: f32,
    rng: u32,
    done: bool,
    // El sonido.
    instrument: Instrument,
    notes: Vec<(u32, u8, f32)>,
    pending: Vec<Sfx>,
    voices: Vec<Voice>,
    sizzle: f32,
    sizzle_now: f32,
    crackle: f32,
    noise: u32,
    rumble: f32,
    horn_in: f32,
}

impl Shucos {
    pub fn new(width: usize, height: usize, records: Records) -> Self {
        let lay = Layout::new(width, height);
        let savings = records.get(SAVINGS).unwrap_or(0);
        let upgrades = records.get(UPGRADES).unwrap_or(0) as u8;
        let best = records.get(RECORD);
        Self {
            lay,
            scene: bake_scene(&lay),
            records,
            screen: Screen::Menu,
            choice: 0,
            notice: None,
            savings,
            upgrades,
            night: Night::default(),
            best,
            autopilot: false,
            auto_wait: 0.0,
            wall: 0.0,
            rng: 0x5EC0_0C05,
            done: false,
            instrument: Instrument::new(),
            notes: Vec::new(),
            pending: Vec::new(),
            voices: Vec::new(),
            sizzle: 0.0,
            sizzle_now: 0.0,
            crackle: 0.0,
            noise: 0x1234_5678,
            rumble: 0.0,
            horn_in: 9.0,
        }
    }

    /// Juega solo (las fotos y las pruebas).
    pub fn set_autopilot(&mut self, on: bool) {
        self.autopilot = on;
    }

    /// Abre la carretilla: empieza la noche.
    pub fn open(&mut self) {
        self.night = Night { spawn_in: 1.2, ..Night::default() };
        if self.upgrades & HELPER != 0 {
            self.night.bun = Some(Shuco::default());
        }
        self.screen = Screen::Playing;
        self.pending.push(Sfx::Bell);
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        unit(self.rng)
    }

    /// La hora (19 a 27) a los `clock` segundos.
    fn hour(clock: f32) -> f32 {
        19.0 + 8.0 * (clock / NIGHT).clamp(0.0, 1.0)
    }

    /// La hora pico que hay, si hay.
    fn rush(&self) -> Option<&'static str> {
        let h = Self::hour(self.night.clock);
        if self.night.clock >= NIGHT {
            None
        } else if (21.0..22.0).contains(&h) {
            Some("¡Salida de la U!")
        } else if (22.5..23.5).contains(&h) {
            Some("¡Salida del estadio!")
        } else if h >= 25.0 {
            Some("Ya salieron los bolos")
        } else {
            None
        }
    }

    fn heat(&self) -> f32 {
        if self.upgrades & IRON != 0 { 1.35 } else { 1.0 }
    }

    /// Llega alguien a un lugar libre, con su pedido.
    fn spawn(&mut self) {
        let taken: Vec<usize> = self.night.customers.iter().map(|c| c.slot).collect();
        let free: Vec<usize> = (0..SLOTS).filter(|i| !taken.contains(i)).collect();
        if free.is_empty() {
            return;
        }
        let slot = free[(self.random() * free.len() as f32) as usize % free.len()];
        let h = Self::hour(self.night.clock);
        let mut weights = [3.0f32, 2.0, 2.0, 0.0, 1.0];
        if (21.0..22.0).contains(&h) {
            weights[0] = 9.0;
        }
        if (22.5..23.5).contains(&h) {
            weights[4] = 9.0;
        }
        if h >= 25.0 {
            weights[3] = 6.0;
        }
        let total: f32 = weights.iter().sum();
        let mut pick = self.random() * total;
        let mut who = Who::Estudiante;
        for (i, &wgt) in weights.iter().enumerate() {
            if pick < wgt {
                who = [Who::Estudiante, Who::Agente, Who::Senora, Who::Bolo, Who::Aficionado][i];
                break;
            }
            pick -= wgt;
        }
        let order = self.order_for(who);
        let skins = [Vec3::new(0.8, 0.58, 0.44), Vec3::new(0.66, 0.45, 0.32), Vec3::new(0.55, 0.37, 0.26), Vec3::new(0.86, 0.66, 0.52)];
        let shirts = [Vec3::new(0.2, 0.45, 0.7), Vec3::new(0.15, 0.55, 0.4), Vec3::new(0.85, 0.75, 0.3), Vec3::new(0.5, 0.25, 0.6), Vec3::new(0.9, 0.9, 0.88)];
        let choose = |r: f32, n: usize| (r * n as f32) as usize % n;
        let (r0, r1, r2) = (self.random(), self.random(), self.random());
        let shirt = match who {
            Who::Agente => Vec3::new(0.12, 0.2, 0.45),
            Who::Senora => [Vec3::new(0.8, 0.45, 0.65), Vec3::new(0.55, 0.4, 0.75), Vec3::new(0.3, 0.55, 0.6)][choose(r1, 3)],
            Who::Bolo => Vec3::new(0.42, 0.25, 0.16),
            Who::Aficionado => Vec3::new(0.82, 0.1, 0.12),
            Who::Estudiante => shirts[choose(r1, shirts.len())],
        };
        let hair = if who == Who::Senora && r2 > 0.6 { Vec3::new(0.7, 0.7, 0.72) } else { Vec3::new(0.08, 0.06, 0.05) * (1.0 + r2) };
        let look = Look { skin: skins[choose(r0, skins.len())], shirt, hair, seed: hash_u32(self.rng) };
        let toldo = if self.upgrades & AWNING != 0 { 1.25 } else { 1.0 };
        let left = slot < 2;
        self.night.customers.push(Customer { who, slot, order, patience: PATIENCE * who.patience() * toldo, waited: 0.0, mood: Mood::Arriving(0.0), look, left, upset: -10.0 });
        self.pending.push(Sfx::Bell);
    }

    /// Lo que pide cada quien.
    fn order_for(&mut self, who: Who) -> Order {
        let r = self.random();
        match who {
            Who::Estudiante => {
                let meat = [Meat::Salchicha, Meat::Longaniza, Meat::Salami][(r * 3.0) as usize % 3];
                if self.random() < 0.55 {
                    Order::new(&[meat], true, &[], &[], "con todo, porfa")
                } else {
                    let a = Top::ALL[(self.random() * 3.0) as usize % 3];
                    let b = Top::ALL[3 + (self.random() * 3.0) as usize % 3];
                    Order::new(&[meat], false, &[(a, 1), (b, 1)], &[], &format!("con {} y {}", a.name(), b.name()))
                }
            }
            Who::Agente => {
                let meat = if r < 0.6 { Meat::Chorizo } else { Meat::Longaniza };
                Order::new(&[meat], true, &[], &[], "con todo, rapidito")
            }
            Who::Senora => Order::new(&[Meat::Salchicha, Meat::Longaniza], true, &[(Top::Guacamol, 2)], &[Top::Cebolla], "sin cebolla y bastante guacamol"),
            Who::Bolo => {
                if r < 0.5 {
                    Order::new(&[Meat::Chorizo], true, &[(Top::Mostaza, 2)], &[], "con todo y bastante mostaza")
                } else {
                    Order::new(&[Meat::Salchicha, Meat::Chorizo], true, &[], &[Top::Repollo], "con todo pero sin repollo, mi rey")
                }
            }
            Who::Aficionado => {
                let meat = if r < 0.5 { Meat::Salami } else { Meat::Longaniza };
                Order::new(&[meat], true, &[(Top::Tomate, 2)], &[], "con todo y doble salsa de tomate")
            }
        }
    }

    fn float(&mut self, text: impl Into<String>, x: f32, y: f32, color: [u8; 4]) {
        self.night.floaters.push(Floater { text: text.into(), x, y, color, age: 0.0 });
    }

    /// Un mensaje en el centro del mostrador.
    fn hint(&mut self, text: &str) {
        let (x, y, _, hy) = self.lay.board();
        self.float(text, x, y - hy - 40.0 * self.lay.s, [255, 232, 200, 255]);
    }

    /// El pan en la tabla.
    fn bread(&mut self) {
        if self.night.bun.is_some() {
            self.hint("Ya hay pan en la tabla");
            return;
        }
        self.night.bun = Some(Shuco::default());
        self.night.money -= BREAD_COST;
        self.pending.push(Sfx::Place);
    }

    /// La tecla de una carne: a la plancha si su lugar esta libre; si no,
    /// al pan.
    fn meat(&mut self, meat: Meat) {
        let i = meat.index();
        match self.night.plancha[i] {
            None => {
                self.night.plancha[i] = Some(0.0);
                self.night.money -= MEAT_COST;
                self.pending.push(Sfx::Place);
            }
            Some(t) => match self.night.bun.as_mut() {
                Some(bun) => {
                    bun.meats.push((meat, t));
                    self.night.plancha[i] = None;
                    self.pending.push(Sfx::Place);
                }
                None => self.hint("Primero el pan (1)"),
            },
        }
    }

    fn topping(&mut self, top: Top) {
        match self.night.bun.as_mut() {
            Some(bun) => {
                let n = &mut bun.tops[top.index()];
                *n = (*n + 1).min(3);
                self.pending.push(Sfx::Squirt);
            }
            None => self.hint("Primero el pan (1)"),
        }
    }

    /// X: a la basura lo de la tabla y lo quemado de la plancha.
    fn trash(&mut self) {
        let mut any = self.night.bun.take().is_some();
        for (i, slot) in self.night.plancha.iter_mut().enumerate() {
            if slot.is_some_and(|t| doneness(Meat::ALL[i], t) == Done::Burnt) {
                *slot = None;
                any = true;
            }
        }
        if any {
            self.hint("A la basura");
            self.pending.push(Sfx::Trash);
        }
    }

    /// Enter: el shuco a quien esta elegido.
    fn deliver(&mut self) {
        let lay = self.lay;
        let Some(ci) = self.night.customers.iter().position(|c| c.slot == self.night.selected && c.waiting()) else {
            self.hint("No hay a quién entregarle");
            return;
        };
        let Some(bun) = self.night.bun.clone() else {
            self.hint("Primero armá el shuco");
            return;
        };
        let x = lay.slot_x(self.night.customers[ci].slot);
        let y = lay.head_y() - 60.0 * lay.s;
        match self.night.customers[ci].order.check(&bun) {
            Ok(()) => {
                let c = &mut self.night.customers[ci];
                let price = c.order.price();
                let tip = (5.0 * c.left_patience()).round() as i64;
                c.mood = Mood::Leaving { happy: true, t: 0.0 };
                let thanks = c.who.thanks();
                self.night.money += price + tip;
                self.night.tips += tip;
                self.night.sold += 1;
                self.night.bun = if self.upgrades & HELPER != 0 { Some(Shuco::default()) } else { None };
                self.float(thanks, x, y - 30.0 * lay.s, [255, 240, 210, 255]);
                let pay = if tip > 0 { format!("+Q{price}  (+Q{tip} de propina)") } else { format!("+Q{price}") };
                self.float(pay, x, y, [255, 214, 110, 255]);
                for k in 0..8 {
                    let a = -PI * (0.2 + 0.6 * k as f32 / 7.0);
                    self.night.particles.push(Particle { kind: Puff::Coin, x, y: y + 40.0 * lay.s, vx: a.cos() * 160.0 * lay.s, vy: a.sin() * 260.0 * lay.s, age: 0.0, life: 0.9 });
                }
                self.pending.push(Sfx::Coin);
                // Un arpegio en la marimba.
                for (k, key) in [72u8, 76, 79, 84].into_iter().enumerate() {
                    self.notes.push(((k as f32 * 0.09 * SAMPLE_RATE as f32) as u32, key, 0.6));
                }
            }
            Err(complaint) => {
                let c = &mut self.night.customers[ci];
                c.waited += 3.0;
                c.upset = self.wall;
                self.night.returned += 1;
                self.float(complaint, x, y, [255, 120, 100, 255]);
                self.pending.push(Sfx::Buzz);
            }
        }
    }

    /// Con las flechas: el siguiente lugar con alguien esperando.
    fn select(&mut self, dir: i32) {
        let waiting: Vec<usize> = (0..SLOTS).filter(|&i| self.night.customers.iter().any(|c| c.slot == i && c.waiting())).collect();
        if waiting.is_empty() {
            return;
        }
        let at = waiting.iter().position(|&i| i == self.night.selected);
        let next = match at {
            Some(k) => (k as i32 + dir).rem_euclid(waiting.len() as i32) as usize,
            None => 0,
        };
        self.night.selected = waiting[next];
    }

    /// Lo que haria quien sabe: atender primero al que menos aguanta, poner
    /// a la plancha lo que piden todos, pasar al pan lo que ya esta listo,
    /// echarle lo de encima y entregar.
    fn auto_keys(&mut self) -> Keys {
        let mut k = Keys::default();
        let night = &self.night;
        let target = night.customers.iter().filter(|c| c.waiting()).min_by(|a, b| (a.patience - a.waited).total_cmp(&(b.patience - b.waited)));
        let Some((slot, order)) = target.map(|c| (c.slot, c.order.clone())) else {
            return k;
        };
        // Lo quemado de la plancha, a la basura.
        if night.plancha.iter().enumerate().any(|(i, t)| t.is_some_and(|t| doneness(Meat::ALL[i], t) == Done::Burnt)) {
            k.press('x');
            return k;
        }
        let Some(bun) = &night.bun else {
            k.press('1');
            return k;
        };
        // Lo que ya no sirve en la tabla, tambien.
        let mut have = [0u8; 4];
        for (m, t) in &bun.meats {
            have[m.index()] += 1;
            if doneness(*m, *t) != Done::Ready {
                k.press('x');
                return k;
            }
        }
        if (0..4).any(|i| have[i] > order.meats[i]) {
            k.press('x');
            return k;
        }
        // A la plancha lo que falta para este y lo que piden los demas.
        for m in Meat::ALL {
            let i = m.index();
            let for_target = order.meats[i] > have[i];
            let for_others = night.customers.iter().any(|c| c.waiting() && c.slot != slot && c.order.meats[i] > 0);
            if (for_target || for_others) && night.plancha[i].is_none() {
                k.press(m.key());
                return k;
            }
        }
        // Al pan lo que ya esta listo.
        for m in Meat::ALL {
            if have[m.index()] < order.meats[m.index()] {
                if night.plancha[m.index()].is_some_and(|t| doneness(m, t) == Done::Ready) {
                    k.press(m.key());
                }
                return k;
            }
        }
        for t in Top::ALL {
            if bun.tops[t.index()] < order.want[t.index()] {
                k.press(t.key());
                return k;
            }
        }
        self.night.selected = slot;
        k.enter = true;
        k
    }

    fn play(&mut self, dt: f32, keys: &Keys) {
        let lay = self.lay;
        let s = lay.s;
        let night_was_open = self.night.clock < NIGHT;
        self.night.clock += dt;
        if keys.escape {
            self.finish();
            return;
        }
        // Las teclas.
        if keys.typed('1') {
            self.bread();
        }
        for m in Meat::ALL {
            if keys.typed(m.key()) {
                self.meat(m);
            }
        }
        for t in Top::ALL {
            if keys.typed(t.key()) {
                self.topping(t);
            }
        }
        if keys.typed('x') {
            self.trash();
        }
        if keys.left {
            self.select(-1);
        }
        if keys.right {
            self.select(1);
        }
        if keys.enter {
            self.deliver();
        }
        // La plancha cocina; lo que se quema echa humo.
        let heat = self.heat();
        let mut cooking = 0.0;
        let mut burning = 0.0;
        for i in 0..4 {
            if let Some(t) = self.night.plancha[i] {
                let t = t + dt * heat;
                self.night.plancha[i] = Some(t);
                cooking += 1.0;
                let (mx, my) = lay.meat_at(i);
                let burnt = doneness(Meat::ALL[i], t) == Done::Burnt;
                if burnt {
                    burning += 1.0;
                }
                let rate = if burnt { 12.0 } else { 4.0 };
                let r = self.random();
                if r < rate * dt {
                    let r2 = self.random();
                    self.night.particles.push(Particle {
                        kind: if burnt { Puff::Smoke } else { Puff::Steam },
                        x: mx + (r2 - 0.5) * 80.0 * s,
                        y: my - 10.0 * s,
                        vx: (r2 - 0.5) * 20.0 * s,
                        vy: -60.0 * s,
                        age: 0.0,
                        life: if burnt { 2.2 } else { 1.4 },
                    });
                }
            }
        }
        self.sizzle = 0.07 * cooking + 0.08 * burning;
        // Los clientes llegan, esperan y se van.
        if self.night.clock < NIGHT {
            self.night.spawn_in -= dt;
            if self.night.spawn_in <= 0.0 {
                self.spawn();
                let base = if self.rush().is_some_and(|r| r != "Ya salieron los bolos") { 3.4 } else { 7.0 };
                self.night.spawn_in = base * (0.75 + 0.5 * self.random());
            }
        }
        let mut angry = Vec::new();
        for c in &mut self.night.customers {
            match c.mood {
                Mood::Arriving(t) => {
                    c.mood = if t + dt >= WALK { Mood::Waiting } else { Mood::Arriving(t + dt) };
                }
                Mood::Waiting => {
                    c.waited += dt;
                    if c.waited >= c.patience {
                        c.mood = Mood::Leaving { happy: false, t: 0.0 };
                        angry.push(c.slot);
                    }
                    // Cerrado y sin servirle a tiempo: se va (sin enojarse).
                    if self.night.clock > NIGHT + LAST_CALL {
                        c.mood = Mood::Leaving { happy: true, t: 0.0 };
                    }
                }
                Mood::Leaving { happy, t } => {
                    c.mood = Mood::Leaving { happy, t: t + dt };
                }
            }
        }
        for slot in angry {
            self.night.angry += 1;
            let (x, y) = (lay.slot_x(slot), lay.head_y() - 60.0 * s);
            self.float("¡Ya me voy, pues!", x, y, [255, 110, 90, 255]);
            for k in 0..6 {
                self.night.particles.push(Particle { kind: Puff::Anger, x: x + (k as f32 - 2.5) * 10.0 * s, y: y + 20.0 * s, vx: 0.0, vy: -50.0 * s, age: 0.0, life: 1.0 });
            }
            self.pending.push(Sfx::Buzz);
        }
        self.night.customers.retain(|c| !matches!(c.mood, Mood::Leaving { t, .. } if t >= WALK));
        // El elegido siempre es alguien que espera.
        let selected_ok = self.night.customers.iter().any(|c| c.slot == self.night.selected && c.waiting());
        if !selected_ok {
            if let Some(c) = self.night.customers.iter().filter(|c| c.waiting()).min_by_key(|c| c.slot) {
                self.night.selected = c.slot;
            }
        }
        // Fin de la noche.
        if night_was_open && self.night.clock >= NIGHT {
            self.hint("¡Ya cerramos! Atendé a los que quedan");
        }
        if self.night.clock >= NIGHT && self.night.customers.is_empty() {
            self.finish();
        }
    }

    /// Se acaba la noche: lo ganado a los ahorros y el record.
    fn finish(&mut self) {
        let profit = self.night.money;
        self.savings += profit.max(0);
        self.records.set(SAVINGS, self.savings);
        if self.best.is_none_or(|b| profit > b) {
            self.best = Some(profit);
            self.records.set(RECORD, profit);
        }
        self.screen = Screen::Results;
        self.sizzle = 0.0;
    }

    /// Comprar la mejora `k` del menu.
    fn buy(&mut self, k: usize) {
        let (name, price, _) = SHOP[k];
        let bit = 1u8 << k;
        if self.upgrades & bit != 0 {
            self.notice = Some((format!("Ya tenés {}", name.to_lowercase()), self.wall));
        } else if self.savings < price {
            self.notice = Some((format!("No te alcanza: te faltan Q{}", price - self.savings), self.wall));
            self.pending.push(Sfx::Buzz);
        } else {
            self.savings -= price;
            self.upgrades |= bit;
            self.records.set(SAVINGS, self.savings);
            self.records.set(UPGRADES, self.upgrades as i64);
            self.notice = Some((format!("¡Compraste {}!", name.to_lowercase()), self.wall));
            self.pending.push(Sfx::Coin);
        }
    }

    /// Los resultados de la noche (para las pruebas).
    pub fn tally(&self) -> (u32, u32, u32, i64) {
        (self.night.sold, self.night.returned, self.night.angry, self.night.money)
    }

    pub fn in_results(&self) -> bool {
        self.screen == Screen::Results
    }

    /// Lo que se mueve detras del mostrador: los clientes y sus globitos.
    fn behind(&self) -> Canvas {
        let lay = &self.lay;
        let s = lay.s;
        let mut c = Canvas::default();
        let ease = |t: f32| smoothstep(0.0, 1.0, t);
        for cu in &self.night.customers {
            let dir = if cu.left { -1.0 } else { 1.0 };
            let (shift, face) = match cu.mood {
                Mood::Arriving(t) => ((1.0 - ease(t / WALK)) * 0.55 * lay.w, Face::Calm),
                Mood::Waiting => {
                    let upset = self.wall - cu.upset < 2.0 || cu.left_patience() < 0.25;
                    (0.0, if upset { Face::Angry } else { Face::Calm })
                }
                Mood::Leaving { happy, t } => (ease(t / WALK) * 0.55 * lay.w, if happy { Face::Happy } else { Face::Angry }),
            };
            let walking = !matches!(cu.mood, Mood::Waiting);
            let phase = unit(cu.look.seed) * TAU;
            let bob = if walking { (self.wall * 9.0 + phase).sin().abs() * -8.0 * s } else { (self.wall * 1.6 + phase).sin() * 2.0 * s };
            let sway = if cu.who == Who::Bolo { (self.wall * 1.3 + phase).sin() * 9.0 * s } else { 0.0 };
            let x = lay.slot_x(cu.slot) + dir * shift + sway;
            person(&mut c, lay, cu.who, &cu.look, x, bob, face);
        }
        // Los globitos, con su colita hacia la cabeza; el elegido con su
        // orilla ambar.
        for cu in self.night.customers.iter().filter(|c| c.waiting()) {
            let (x, y) = (lay.slot_x(cu.slot), lay.bubble_y(cu.slot));
            let (hx, hy) = (134.0 * s, 47.0 * s);
            let paper = Vec3::new(0.97, 0.95, 0.9);
            if cu.slot == self.night.selected {
                c.rect(x, y, hx + 4.0 * s, hy + 4.0 * s, 18.0 * s, Vec3::new(1.0, 0.72, 0.25)).glow(10.0 * s, 0.5);
                c.line(x - 18.0 * s, y + hy, x - 10.0 * s, lay.head_y() - 52.0 * s, 9.0 * s, Vec3::new(1.0, 0.72, 0.25));
            }
            c.line(x - 18.0 * s, y + hy - 4.0 * s, x - 10.0 * s, lay.head_y() - 52.0 * s, 6.0 * s, paper);
            c.rect(x, y, hx, hy, 15.0 * s, paper);
        }
        c
    }

    /// Lo que se mueve sobre el mostrador: la plancha y la tabla.
    fn on_counter(&self) -> Canvas {
        let lay = &self.lay;
        let s = lay.s;
        let mut c = Canvas::default();
        let (px, py, phx, phy) = lay.plancha();
        let cooking = self.night.plancha.iter().filter(|t| t.is_some()).count();
        if cooking > 0 {
            c.rect(px, py, phx - 4.0 * s, phy - 4.0 * s, 8.0 * s, Vec3::new(1.0, 0.3, 0.05)).alpha(0.05 + 0.02 * cooking as f32).glow(14.0 * s, 0.06);
        }
        for (i, slot) in self.night.plancha.iter().enumerate() {
            let (mx, my) = lay.meat_at(i);
            let Some(t) = *slot else {
                // El lugar vacio: una marquita de su carne.
                c.line(mx - 50.0 * s, my, mx + 50.0 * s, my, 12.0 * s, Meat::ALL[i].raw()).alpha(0.08);
                continue;
            };
            let meat = Meat::ALL[i];
            draw_meat(&mut c, meat, mx, my - 4.0 * s, 52.0 * s, t, s, Some(self.wall));
            // Cuanto le falta: la barrita bajo la carne (verde = lista).
            let (ready, burn) = meat.times();
            let bar_y = my + 28.0 * s;
            let len = 50.0 * s;
            c.rect(mx, bar_y, len, 3.0 * s, 2.0 * s, Vec3::splat(0.05)).alpha(0.8);
            let f = (t / burn).min(1.0);
            let col = match doneness(meat, t) {
                Done::Raw => Vec3::new(0.95, 0.8, 0.25),
                Done::Ready => Vec3::new(0.35, 0.9, 0.35).lerp(Vec3::new(1.0, 0.55, 0.1), smoothstep(0.6, 1.0, (t - ready) / (burn - ready))),
                Done::Burnt => Vec3::new(0.95, 0.15, 0.1),
            };
            let fill = len * f;
            c.rect(mx - len + fill, bar_y, fill.max(1.0), 3.0 * s, 2.0 * s, col).glow(3.0 * s, 0.3);
            c.rect(mx - len + 2.0 * len * ready / burn, bar_y, 1.0 * s, 5.0 * s, 0.5 * s, Vec3::splat(0.95)).alpha(0.7);
        }
        if let Some(bun) = &self.night.bun {
            draw_bun(&mut c, lay, bun);
        }
        c
    }

    /// El vapor, el humo, el enojo y las monedas.
    fn air(&self) -> Canvas {
        let s = self.lay.s;
        let mut c = Canvas::default();
        for p in &self.night.particles {
            let f = p.age / p.life;
            match p.kind {
                Puff::Steam => {
                    c.disc(p.x, p.y, (8.0 + 22.0 * f) * s, Vec3::new(0.85, 0.85, 0.88)).alpha(0.22 * (1.0 - f) * smoothstep(0.0, 0.15, f)).soft(10.0 * s);
                }
                Puff::Smoke => {
                    c.disc(p.x, p.y, (10.0 + 30.0 * f) * s, Vec3::new(0.12, 0.11, 0.11)).alpha(0.55 * (1.0 - f) * smoothstep(0.0, 0.1, f)).soft(12.0 * s);
                }
                Puff::Anger => {
                    c.disc(p.x, p.y, (5.0 + 10.0 * f) * s, Vec3::new(0.95, 0.95, 0.95)).alpha(0.5 * (1.0 - f)).soft(5.0 * s);
                }
                Puff::Coin => {
                    c.disc(p.x, p.y, 7.0 * s, Vec3::new(0.98, 0.78, 0.25)).alpha(1.0 - f * f).glow(5.0 * s, 0.4);
                    c.disc(p.x - 2.0 * s, p.y - 2.0 * s, 2.5 * s, Vec3::new(1.0, 0.97, 0.8)).alpha(1.0 - f * f);
                }
            }
        }
        c
    }
}

impl Minigame for Shucos {
    fn update(&mut self, dt: f64, keys: &Keys) {
        let dt = (dt as f32).min(0.1);
        self.wall += dt;
        let keys = if self.autopilot && self.screen == Screen::Playing {
            self.auto_wait -= dt;
            if self.auto_wait <= 0.0 {
                self.auto_wait = AUTO_STEP;
                let mut k = self.auto_keys();
                k.escape = keys.escape;
                k
            } else {
                Keys { escape: keys.escape, ..Keys::default() }
            }
        } else {
            *keys
        };
        match self.screen {
            Screen::Menu => {
                let options = 1 + SHOP.len();
                if keys.escape {
                    self.done = true;
                }
                if keys.up {
                    self.choice = (self.choice + options - 1) % options;
                }
                if keys.down {
                    self.choice = (self.choice + 1) % options;
                }
                if keys.enter {
                    if self.choice == 0 {
                        self.open();
                    } else {
                        self.buy(self.choice - 1);
                    }
                }
            }
            Screen::Playing => self.play(dt, &keys),
            Screen::Results => {
                if keys.enter || keys.escape {
                    self.screen = Screen::Menu;
                    self.night = Night::default();
                }
            }
        }
        // Lo que vuela.
        let s = self.lay.s;
        for p in &mut self.night.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            if p.kind == Puff::Coin {
                p.vy += 700.0 * s * dt;
            } else {
                p.vx *= 1.0 - 0.8 * dt;
            }
            p.age += dt;
        }
        self.night.particles.retain(|p| p.age < p.life);
        for f in &mut self.night.floaters {
            f.age += dt;
        }
        self.night.floaters.retain(|f| f.age < 1.8);
    }

    fn audio(&mut self, out: &mut [f32]) {
        let rate = SAMPLE_RATE as f32;
        self.instrument.tune_to(rate);
        for sfx in self.pending.drain(..) {
            self.voices.push(Voice { sfx, n: 0 });
        }
        let level = if self.screen == Screen::Playing { self.sizzle } else { 0.0 };
        for sample in out.iter_mut() {
            // Las notas de la marimba que tocan ahora.
            let mut k = 0;
            while k < self.notes.len() {
                if self.notes[k].0 == 0 {
                    let (_, key, force) = self.notes.remove(k);
                    self.instrument.strike(key, force, Mallet::Tiple);
                } else {
                    self.notes[k].0 -= 1;
                    k += 1;
                }
            }
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            let n = unit(self.noise) * 2.0 - 1.0;
            // La plancha chirria: ruido con chasquidos sueltos.
            self.sizzle_now += (level - self.sizzle_now) * 0.0005;
            self.crackle *= 0.993;
            if unit(self.noise.rotate_left(7)) < self.sizzle_now * 0.004 {
                self.crackle = 1.0;
            }
            let mut x = n * (0.18 * self.sizzle_now + 0.35 * self.crackle * self.sizzle_now);
            // El trafico de la 7a avenida, lejos.
            self.rumble = self.rumble * 0.997 + n * 0.003;
            x += self.rumble * 0.5;
            // Los sonidos cortos.
            for v in &mut self.voices {
                let t = v.n as f32 / rate;
                v.n += 1;
                x += match v.sfx {
                    Sfx::Coin => {
                        let f = if t < 0.07 { 1568.0 } else { 2093.0 };
                        0.16 * (-t * 9.0).exp() * (TAU * f * t).sin()
                    }
                    Sfx::Place => n * 0.3 * (-t * 22.0).exp(),
                    Sfx::Squirt => n * 0.2 * (PI * (t / 0.16).min(1.0)).sin() * (TAU * 300.0 * t).sin().abs(),
                    Sfx::Buzz => 0.09 * (-t * 5.0).exp() * if (130.0 * t).fract() < 0.5 { 1.0 } else { -1.0 },
                    Sfx::Bell => 0.1 * (-t * 6.0).exp() * ((TAU * 1046.0 * t).sin() + 0.6 * (TAU * 1318.0 * t).sin()),
                    Sfx::Trash => n * 0.25 * (-t * 9.0).exp(),
                    Sfx::Horn => {
                        let f = if (t / 0.3) as u32 % 2 == 0 { 392.0 } else { 494.0 };
                        0.025 * (1.0 - t / 0.9).max(0.0) * if (f * t).fract() < 0.5 { 1.0 } else { -1.0 }
                    }
                };
            }
            self.voices.retain(|v| (v.n as f32 / rate) < 0.9);
            x += self.instrument.next(rate) * 0.7;
            *sample = x / (1.0 + 0.4 * x.abs());
        }
        // De vez en cuando pita una camioneta en la avenida.
        if self.screen == Screen::Playing {
            self.horn_in -= out.len() as f32 / rate;
            if self.horn_in <= 0.0 {
                self.horn_in = 12.0 + 10.0 * self.random();
                self.voices.push(Voice { sfx: Sfx::Horn, n: 0 });
            }
        }
    }

    fn draw(&mut self, rgba: &mut [u8]) {
        let lay = self.lay;
        let width = lay.width;
        let behind = self.behind();
        let front = self.on_counter();
        let air = self.air();
        let counter = lay.counter;
        let scene = &self.scene;
        let dim = if self.screen == Screen::Playing { 1.0 } else { 0.5 };
        parallel::chunks_mut(rgba, width * 4, 1, |y, row| {
            let src = &scene[y * width * 4..(y + 1) * width * 4];
            if src.len() == row.len() {
                row.copy_from_slice(src);
            }
            let yf = y as f32 + 0.5;
            if yf < counter {
                paint(&behind.shapes, yf, row, width);
            } else {
                paint(&front.shapes, yf, row, width);
            }
            paint(&air.shapes, yf, row, width);
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
        let lay = &self.lay;
        let s = lay.s;
        let w = lay.w;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 220];
        let accent = [255, 204, 120, 255];
        let ink = [40, 30, 26, 255];
        match self.screen {
            Screen::Menu => {
                let (pw, ph) = (640.0 * s, 430.0 * s);
                let (px, py) = ((w - pw) * 0.5, 70.0 * s);
                o.panel(px, py, pw, ph);
                o.text("Shucos del Reformador", w * 0.5, py + 14.0 * s, Font::Big(50.0 * s), accent, Align::Center);
                o.text("Una carretilla al pie de la torre, de 7 de la noche a 3 de la mañana", w * 0.5, py + 78.0 * s, Font::Body, dim, Align::Center);
                for i in 0..=SHOP.len() {
                    let y = py + 122.0 * s + i as f32 * 58.0 * s;
                    let chosen = i == self.choice;
                    if chosen {
                        o.bar(px + 18.0 * s, y - 6.0 * s, pw - 36.0 * s, 52.0 * s, 1.0, [255, 204, 120, 34], false);
                    }
                    let color = if chosen { accent } else { cream };
                    o.text(if chosen { "»" } else { " " }, px + 30.0 * s, y, Font::Heading, accent, Align::Left);
                    if i == 0 {
                        o.text("Abrir la carretilla", px + 54.0 * s, y, Font::Heading, color, Align::Left);
                        o.text("empieza la noche", px + 54.0 * s, y + 28.0 * s, Font::Small, dim, Align::Left);
                    } else {
                        let (name, price, blurb) = SHOP[i - 1];
                        let owned = self.upgrades & (1 << (i - 1)) != 0;
                        o.text(name, px + 54.0 * s, y, Font::Heading, color, Align::Left);
                        o.text(blurb, px + 54.0 * s, y + 28.0 * s, Font::Small, dim, Align::Left);
                        let tag = if owned { "ya la tenés".to_string() } else { format!("Q{price}") };
                        o.text(tag, px + pw - 30.0 * s, y + 4.0 * s, Font::Bold, if owned { dim } else { accent }, Align::Right);
                    }
                }
                let best = match self.best {
                    Some(b) => format!("Mejor noche: Q{b}"),
                    None => "Todavía no hay mejor noche".to_string(),
                };
                o.text(format!("Ahorros: Q{}    ·    {best}", self.savings), w * 0.5, py + ph - 70.0 * s, Font::Bold, cream, Align::Center);
                if let Some((text, at)) = &self.notice
                    && self.wall - at < 2.5
                {
                    o.text(text.clone(), w * 0.5, py + ph - 96.0 * s, Font::Body, accent, Align::Center);
                }
                o.text("↑ ↓: elegir   ·   Enter: abrir o comprar   ·   Esc: salir", w * 0.5, py + ph - 34.0 * s, Font::Small, cream, Align::Center);
            }
            Screen::Playing | Screen::Results => {
                // La hora, la plata y la cuenta.
                let (hx, hy) = (16.0 * s, lay.awning + 22.0 * s);
                o.panel(hx, hy, 200.0 * s, 104.0 * s);
                let hour = Self::hour(self.night.clock);
                let (hh, mm) = ((hour.floor() as i32).rem_euclid(24), ((hour.fract() * 60.0) as i32 / 5) * 5);
                o.text(format!("{hh:02}:{mm:02}"), hx + 16.0 * s, hy + 10.0 * s, Font::Heading, cream, Align::Left);
                o.text(format!("Q{}", self.night.money), hx + 184.0 * s, hy + 10.0 * s, Font::Heading, accent, Align::Right);
                o.text(format!("Vendidos {}  ·  Devueltos {}", self.night.sold, self.night.returned), hx + 16.0 * s, hy + 46.0 * s, Font::Small, dim, Align::Left);
                o.text(format!("Se fueron enojados {}", self.night.angry), hx + 16.0 * s, hy + 68.0 * s, Font::Small, dim, Align::Left);
                // La hora pico, abajo del panel (arriba a la derecha van los
                // globitos).
                if let Some(rush) = self.rush() {
                    o.text(rush, hx + 4.0 * s, hy + 116.0 * s, Font::Bold, accent, Align::Left);
                }
                if self.night.clock >= NIGHT {
                    o.text("Ya cerramos:", hx + 4.0 * s, hy + 116.0 * s, Font::Bold, accent, Align::Left);
                    o.text("atendé a los que quedan", hx + 4.0 * s, hy + 140.0 * s, Font::Small, accent, Align::Left);
                }
                // Los pedidos en los globitos, con su paciencia.
                for cu in self.night.customers.iter().filter(|c| c.waiting()) {
                    let (x, y) = (lay.slot_x(cu.slot), lay.bubble_y(cu.slot));
                    o.text(cu.who.name(), x, y - 42.0 * s, Font::Small, [110, 90, 80, 255], Align::Center);
                    o.text(cu.order.lines[0].clone(), x, y - 23.0 * s, Font::Bold, ink, Align::Center);
                    o.text(cu.order.lines[1].clone(), x, y + 0.0 * s, Font::Small, ink, Align::Center);
                    let left = cu.left_patience();
                    let color = if left > 0.5 {
                        [90, 190, 90, 255]
                    } else if left > 0.25 {
                        [235, 180, 50, 255]
                    } else {
                        [225, 70, 60, 255]
                    };
                    o.bar(x - 110.0 * s, y + 27.0 * s, 220.0 * s, 8.0 * s, left, color, false);
                }
                // Las teclas de cada cosa del mostrador.
                let (bx, by) = lay.basket();
                o.text("1 · Pan", bx, by + 64.0 * s, Font::Bold, cream, Align::Center);
                for m in Meat::ALL {
                    let (mx, my) = lay.meat_at(m.index());
                    let label = format!("{} · {}", m.key(), m.name());
                    o.text(label, mx, my + 34.0 * s, Font::Small, cream, Align::Center);
                }
                for t in Top::ALL {
                    let (x, y) = lay.top_at(t.index());
                    // Los tazones, con su letra arriba; los botes, abajo.
                    let ly = if t.index() < 3 { y - 64.0 * s } else { y + 46.0 * s };
                    o.text(format!("{} · {}", t.key().to_ascii_uppercase(), t.label()), x, ly, Font::Small, cream, Align::Center);
                }
                let (tx, ty, _, thy) = lay.board();
                o.text("X · tirar     ← → · cliente     Enter · entregar", tx, ty + thy + 14.0 * s, Font::Small, cream, Align::Center);
                for f in &self.night.floaters {
                    let a = (1.0 - (f.age / 1.8).powi(3)).clamp(0.0, 1.0);
                    let mut color = f.color;
                    color[3] = (color[3] as f32 * a) as u8;
                    o.text(f.text.clone(), f.x, f.y - 40.0 * s * f.age, Font::Bold, color, Align::Center);
                }
            }
        }
        if self.screen == Screen::Results {
            let (pw, ph) = (560.0 * s, 380.0 * s);
            let (px, py) = ((w - pw) * 0.5, 120.0 * s);
            o.panel(px, py, pw, ph);
            o.text("Se acabó la noche", w * 0.5, py + 16.0 * s, Font::Big(46.0 * s), accent, Align::Center);
            let n = &self.night;
            let lines = [
                format!("Vendidos: {}", n.sold),
                format!("Devueltos: {}", n.returned),
                format!("Se fueron enojados: {}", n.angry),
                format!("Propinas: Q{}", n.tips),
                format!("Ganancia de la noche: Q{}", n.money),
                format!("Ahorros: Q{}", self.savings),
            ];
            for (i, line) in lines.iter().enumerate() {
                o.text(line.clone(), w * 0.5, py + 86.0 * s + i as f32 * 32.0 * s, if i == 4 { Font::Heading } else { Font::Body }, if i == 4 { accent } else { cream }, Align::Center);
            }
            let verdict = match (n.angry, n.returned, n.sold) {
                (0, 0, s) if s >= 12 => "¡El mejor shuco de la sexta avenida!",
                (0, _, _) => "Nadie se fue enojado. Bien ahí.",
                (a, _, _) if a <= 3 => "Hubo sus enojados, pero la carretilla aguanta.",
                _ => "La gente se fue con hambre. Mañana mejor.",
            };
            o.text(verdict, w * 0.5, py + ph - 70.0 * s, Font::Bold, dim, Align::Center);
            o.text("Enter: volver a la carretilla", w * 0.5, py + ph - 36.0 * s, Font::Small, cream, Align::Center);
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

    fn shuco(meats: &[(Meat, f32)], tops: &[(Top, u8)]) -> Shuco {
        let mut s = Shuco { meats: meats.to_vec(), tops: [0; 6] };
        for &(t, n) in tops {
            s.tops[t.index()] = n;
        }
        s
    }

    #[test]
    fn the_orders_check_what_was_asked() {
        let all: Vec<(Top, u8)> = Top::ALL.iter().map(|&t| (t, 1)).collect();
        let senora = Order::new(&[Meat::Salchicha, Meat::Longaniza], true, &[(Top::Guacamol, 2)], &[Top::Cebolla], "sin cebolla y bastante guacamol");
        let ready = |m: Meat| (m, m.times().0 + 0.5);
        let mut good = shuco(&[ready(Meat::Salchicha), ready(Meat::Longaniza)], &all);
        good.tops[Top::Cebolla.index()] = 0;
        good.tops[Top::Guacamol.index()] = 2;
        assert_eq!(senora.check(&good), Ok(()));
        // Con cebolla, poco guacamol, cruda, quemada o sin una carne: no.
        let mut onion = good.clone();
        onion.tops[Top::Cebolla.index()] = 1;
        assert_eq!(senora.check(&onion), Err("¡Te dije sin cebolla!".to_string()));
        let mut little = good.clone();
        little.tops[Top::Guacamol.index()] = 1;
        assert!(senora.check(&little).unwrap_err().contains("Bastante guacamol"));
        let mut raw = good.clone();
        raw.meats[0].1 = 1.0;
        assert_eq!(senora.check(&raw), Err("¡La salchicha está cruda!".to_string()));
        let mut burnt = good.clone();
        burnt.meats[1].1 = 30.0;
        assert_eq!(senora.check(&burnt), Err("¡La longaniza está quemada!".to_string()));
        let mut short = good.clone();
        short.meats.pop();
        assert_eq!(senora.check(&short), Err("¡Le falta carne!".to_string()));
        // Lo que no va "con todo" no lleva de mas.
        let plain = Order::new(&[Meat::Chorizo], false, &[(Top::Guacamol, 1), (Top::Mostaza, 1)], &[], "con guacamol y mostaza");
        let ok = shuco(&[ready(Meat::Chorizo)], &[(Top::Guacamol, 1), (Top::Mostaza, 1)]);
        assert_eq!(plain.check(&ok), Ok(()));
        let extra = shuco(&[ready(Meat::Chorizo)], &[(Top::Guacamol, 1), (Top::Mostaza, 1), (Top::Mayonesa, 1)]);
        assert_eq!(plain.check(&extra), Err("No pedí mayonesa".to_string()));
        // El mixto tiene descuento.
        assert_eq!(senora.price(), 10 + 12 - 4);
        assert_eq!(plain.price(), 12);
    }

    #[test]
    fn the_meat_cooks_then_burns() {
        for m in Meat::ALL {
            let (ready, burn) = m.times();
            assert_eq!(doneness(m, 0.0), Done::Raw);
            assert_eq!(doneness(m, ready + 0.01), Done::Ready);
            assert_eq!(doneness(m, burn + 0.01), Done::Burnt);
            assert!(meat_color(m, burn + 5.0).max_element() < 0.12);
            assert!(meat_color(m, 0.0).max_element() > meat_color(m, ready).max_element());
        }
    }

    /// Con el piloto automatico una noche entera: vende, no le devuelven
    /// nada y gana plata; despues la plata va a los ahorros.
    #[test]
    fn a_whole_night_on_autopilot_sells_and_earns() {
        let mut g = Shucos::new(320, 180, Records::in_memory());
        g.update(0.016, &Keys { enter: true, ..Keys::default() });
        assert_eq!(g.screen, Screen::Playing);
        g.set_autopilot(true);
        let mut t = 0.0;
        while !g.in_results() && t < NIGHT + LAST_CALL + 10.0 {
            g.update(1.0 / 30.0, &Keys::default());
            t += 1.0 / 30.0;
        }
        assert!(g.in_results(), "la noche se acaba");
        let (sold, returned, angry, money) = g.tally();
        assert!(sold >= 14, "vendio {sold} (enojados {angry})");
        assert_eq!(returned, 0);
        assert!(angry <= sold / 3, "enojados {angry} de {sold}");
        assert!(money > 100, "{money}");
        assert_eq!(g.savings, money);
    }

    #[test]
    fn upgrades_cost_savings_and_stay() {
        let mut g = Shucos::new(160, 90, Records::in_memory());
        g.savings = 150;
        g.buy(0);
        assert_eq!(g.savings, 30);
        assert!(g.upgrades & IRON != 0);
        g.buy(2);
        assert!(g.upgrades & HELPER == 0, "no alcanza");
        assert_eq!(g.savings, 30);
        assert!((g.heat() - 1.35).abs() < 1e-6);
    }

    /// Se dibuja en todas las pantallas, sin agujeros, y el texto usa letras
    /// que la fuente carga.
    #[test]
    fn every_screen_draws_and_its_text_has_glyphs() {
        let (w, h) = (320, 180);
        let mut g = Shucos::new(w, h, Records::in_memory());
        let mut rgba = vec![0u8; w * h * 4];
        let mut check = |g: &mut Shucos| {
            rgba.fill(0);
            g.draw(&mut rgba);
            assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
            super::super::assert_loaded_glyphs(&g.overlay());
        };
        check(&mut g);
        g.open();
        g.set_autopilot(true);
        for _ in 0..600 {
            g.update(1.0 / 30.0, &Keys::default());
        }
        assert!(!g.night.customers.is_empty() || g.night.sold > 0);
        check(&mut g);
        // Todos los pedidos y quejas posibles.
        for who in [Who::Estudiante, Who::Agente, Who::Senora, Who::Bolo, Who::Aficionado] {
            for _ in 0..6 {
                let order = g.order_for(who);
                let mut o = Overlay::default();
                o.text(order.lines[0].clone(), 0.0, 0.0, Font::Body, [0; 4], Align::Left);
                o.text(order.lines[1].clone(), 0.0, 0.0, Font::Body, [0; 4], Align::Left);
                o.text(who.name(), 0.0, 0.0, Font::Body, [0; 4], Align::Left);
                o.text(who.thanks(), 0.0, 0.0, Font::Body, [0; 4], Align::Left);
                super::super::assert_loaded_glyphs(&o);
            }
        }
        g.finish();
        check(&mut g);
    }
}
