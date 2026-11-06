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
//! - **Don Meyer** (el personaje de Blender de Talega Surfers) llega
//!   cada noche a eso de las 8 y cuarto a pedir su shuco, saludando.
//!
//! Todo se pinta en CPU. La escena quieta se hornea una vez: lo de lejos (el
//! cielo con nubes, dos filas de edificios, la torre de celosia, la avenida
//! y la banqueta) se desenfoca como en una foto que enfoca a la gente, y
//! encima van nitidos el toldo, los focos y el mostrador de acero con lo que
//! tiene. Encima, por filas en paralelo, va lo de cada frame: el trafico de
//! la avenida (carros y, de lado, La Mamaneta y el Tuk-Tuk Elena), los
//! clientes, Meyer (horneado de frente y caminando), la plancha, el shuco y
//! el humo. Cada figura es una distancia con signo con volumen (`figuras`):
//! la luz de los focos, la del cielo y la contraluz naranja de los postes.
//! Lo que emite pasa por el brillo, la vineta y la gradacion de la carrera.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use super::carrera::value_noise;
use super::figuras::{Canvas, Light, paint, paint_lit};
use super::flota;
use super::hornear::{Lut, Sprite};
use super::lienzo;
use super::personaje::{self, WALK_FRAMES};
use super::posproceso::{self, Posproceso};
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
    /// Don Meyer, el de Talega Surfers.
    Meyer,
}

impl Who {
    fn name(self) -> &'static str {
        match self {
            Who::Estudiante => "El de la U",
            Who::Agente => "El agente de la PMT",
            Who::Senora => "La señora",
            Who::Bolo => "El bolo de las 2 a. m.",
            Who::Aficionado => "El del estadio",
            Who::Meyer => "Don Meyer",
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
            Who::Meyer => 1.1,
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
            Who::Meyer => "¡Saludos, mano! Así sí",
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
    /// Si ya vino don Meyer esta noche.
    meyer_came: bool,
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

/// Lo que pasa por la avenida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ride {
    Sedan,
    Pickup,
    Moto,
    /// La Mamaneta y el Tuk-Tuk Elena, de lado.
    Bus,
    Tuk,
}

/// Un carro en la avenida: donde va (px), para donde, que tan rapido
/// (m/s), en que carril, su color y la pintura de la camioneta.
#[derive(Clone, Copy, Debug)]
struct Car {
    x: f32,
    dir: f32,
    speed: f32,
    lane: usize,
    kind: Ride,
    color: Vec3,
    paint: u8,
}

/// Una nave de lado ya puesta en la pantalla.
#[derive(Clone, Copy, Debug)]
struct RideBlit {
    bus: bool,
    at: (f32, f32),
    scale: f32,
    flip: bool,
    paint: u8,
    lane: usize,
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

    /// Donde empieza la avenida y donde la banqueta.
    fn street(&self) -> f32 {
        0.5 * self.h
    }

    fn walk(&self) -> f32 {
        0.565 * self.h
    }

    /// Los tres focos que cuelgan del toldo, entre los globitos.
    fn bulbs(&self) -> [(f32, f32); 3] {
        let y = self.awning + 34.0 * self.s;
        [(0.3 * self.w, y), (0.52 * self.w, y + 5.0 * self.s), (0.74 * self.w, y)]
    }

    /// Donde toca el asfalto cada carril de la avenida (0 el de alla, 1 el de
    /// aca) y cuantos pixeles mide un metro ahi.
    fn lane(&self, lane: usize) -> (f32, f32) {
        let (st, wk) = (self.street(), self.walk());
        let ppm = 0.15 * self.h / flota::BUS_HEIGHT;
        if lane == 0 { (st + 0.38 * (wk - st), 0.78 * ppm) } else { (st + 0.86 * (wk - st), ppm) }
    }

    /// El bote de las propinas.
    fn jar(&self) -> (f32, f32) {
        (0.71 * self.w, 0.69 * self.h)
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
/// La luz de los focos de la carretilla.
const BULB: Vec3 = Vec3::new(1.0, 0.8, 0.52);
/// Los postes de la avenida, lejos (fraccion del ancho y del alto, color):
/// desenfocados se ven como discos de luz y se reflejan en el asfalto. El
/// ultimo es el semaforo, en verde.
const FAR_LAMPS: [(f32, f32, Vec3); 7] = [
    (0.13, 0.432, Vec3::new(1.0, 0.62, 0.25)),
    (0.235, 0.452, Vec3::new(1.0, 0.62, 0.25)),
    (0.355, 0.468, Vec3::new(0.82, 0.9, 1.0)),
    (0.645, 0.468, Vec3::new(0.82, 0.9, 1.0)),
    (0.765, 0.452, Vec3::new(1.0, 0.62, 0.25)),
    (0.87, 0.432, Vec3::new(1.0, 0.62, 0.25)),
    (0.452, 0.482, Vec3::new(0.25, 1.0, 0.5)),
];
/// Cuanto se desenfoca lo de atras (pixeles a 720p): se enfoca a la gente.
const BLUR: f32 = 2.0;
/// Los pixeles por metro de Meyer a 720p (es cabezon: asi queda del tamano
/// de los demas).
const MEYER_PPM: f32 = 165.0;

/// La luz sobre la gente: los focos de la carretilla les dan de arriba y de
/// frente, el cielo de noche los llena de azul y los postes de sodio de
/// atras les dejan la orilla naranja.
fn people_light() -> Light {
    Light { key: Vec3::new(0.15, -0.6, 0.79).normalize(), key_color: BULB * 0.66, ambient: Vec3::new(0.27, 0.29, 0.42), rim: Vec3::new(1.0, 0.55, 0.25) * 0.5 }
}

/// La luz sobre el mostrador: los focos desde arriba, del lado de la gente.
fn counter_light() -> Light {
    Light { key: Vec3::new(0.0, -0.62, 0.78).normalize(), key_color: BULB * 0.68, ambient: Vec3::new(0.3, 0.31, 0.42), rim: Vec3::new(1.0, 0.7, 0.4) * 0.12 }
}

/// Una fila de edificios de `width` pixeles cada uno, de `lo` a `hi` del alto
/// de la pantalla sobre la calle, con sus ventanas (unas encendidas, calidas
/// o del azul de la tele) y algun tinaco o antena en el techo. `haze` los
/// hace mas lejanos: mas azules y con la luz mas apagada. Da el color y lo
/// que emite.
#[allow(clippy::too_many_arguments)]
fn building(lay: &Layout, x: f32, y: f32, width: f32, lo: f32, hi: f32, salt: u32, haze: f32) -> Option<(Vec3, f32)> {
    let s = lay.s;
    let b = (x / width).floor();
    let hb = hash_u32((b as i32) as u32 ^ salt);
    let top = lay.street() - (lo + (hi - lo) * unit(hb)) * lay.h;
    let roof = Vec3::new(0.04, 0.035, 0.055).lerp(Vec3::new(0.08, 0.06, 0.11), haze);
    if y < top {
        let fx = x / width - b;
        if hb % 3 == 0 && fx > 0.6 && fx < 0.8 && y > top - 7.0 * s {
            return Some((roof, 0.0));
        }
        if hb % 5 == 1 && (fx - 0.3).abs() < 0.025 && y > top - 22.0 * s {
            return Some((roof, 0.0));
        }
        return None;
    }
    let mut c = Vec3::new(0.05, 0.045, 0.07) * (0.75 + 0.5 * unit(hb >> 3));
    c = c.lerp(Vec3::new(0.09, 0.07, 0.13), haze);
    let (gx, gy) = (x / (9.0 * s), (y - top) / (12.0 * s));
    let (fx, fy) = (gx - gx.floor(), gy - gy.floor());
    let mut e = 0.0;
    if gy >= 1.0 && fx > 0.22 && fx < 0.78 && fy > 0.28 && fy < 0.8 {
        let lit = hash_u32((gx.floor() as i32 as u32).wrapping_mul(0x9E37) ^ (gy.floor() as u32).wrapping_mul(0x85EB) ^ hb);
        if lit % 3 == 0 {
            let warm = match lit % 9 {
                0 => Vec3::new(0.65, 0.82, 1.0),
                3 => Vec3::new(0.5, 0.58, 1.0),
                _ => Vec3::new(1.0, 0.72, 0.38),
            };
            let k = (0.45 + 0.45 * unit(lit)) * (1.0 - 0.45 * haze);
            c = warm * k;
            e = 0.45 * k;
        } else {
            c = c * 1.35 + Vec3::new(0.02, 0.015, 0.03);
        }
    }
    // Lo de abajo lo alumbra la calle.
    c = c + SODIUM * (0.06 * smoothstep(top + 0.2 * lay.h, lay.street(), y));
    Some((c, e))
}

/// El fondo lejano en un pixel, antes de desenfocarlo: el cielo de noche con
/// nubes que alumbra la ciudad desde abajo, dos filas de edificios (la de
/// aca deja la plaza de la torre), la 7a Avenida con los reflejos de los
/// postes en el asfalto mojado, su raya y el paso de cebra, y la banqueta.
fn far(lay: &Layout, x: f32, y: f32) -> (Vec3, f32) {
    let (w, s) = (lay.w, lay.s);
    let (street, walk) = (lay.street(), lay.walk());
    if y < street {
        let mid = (x - 0.5 * w).abs();
        if mid > 0.1 * w
            && let Some(b) = building(lay, x, y, 56.0 * s, 0.03, 0.19, 0xB11D, 0.0)
        {
            return b;
        }
        // Al pie de la torre, los arboles del parque.
        if mid <= 0.1 * w {
            let crown = street - (0.028 + 0.02 * value_noise(x / (14.0 * s), 3.0) + 0.01 * value_noise(x / (5.0 * s), 9.0)) * lay.h;
            if y > crown {
                return (Vec3::new(0.025, 0.045, 0.03) + SODIUM * (0.1 * smoothstep(crown, street, y)), 0.0);
            }
        }
        if let Some(b) = building(lay, x, y, 34.0 * s, 0.1, 0.3, 0x7A11, 0.55) {
            return b;
        }
        let t = y / street;
        let mut c = Vec3::new(0.018, 0.02, 0.055).lerp(Vec3::new(0.15, 0.07, 0.15), t.powf(2.2));
        c = c + SODIUM * (0.2 * smoothstep(0.55, 1.0, t));
        let n = 0.6 * value_noise(x / (210.0 * s), y / (46.0 * s)) + 0.4 * value_noise(x / (80.0 * s) + 7.0, y / (20.0 * s));
        let cloud = smoothstep(0.5, 0.8, n) * smoothstep(0.05, 0.45, t) * (1.0 - smoothstep(0.85, 1.0, t));
        c = c.lerp(Vec3::new(0.28, 0.15, 0.2) * (0.45 + 0.7 * t), cloud * 0.5);
        let star = hash_u32((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663));
        if star % 500 == 0 && t < 0.6 && cloud < 0.2 {
            c = c + Vec3::splat(0.3 + 0.5 * unit(star));
        }
        return (c, 0.0);
    }
    let pool = |lx: f32| (-((x - lx) / (0.1 * w)).powi(2)).exp();
    if y < walk {
        let v = (y - street) / (walk - street);
        let light = SODIUM * (0.16 * (pool(0.04 * w) + pool(0.96 * w)) + 0.04);
        let mut c = Vec3::new(0.05, 0.05, 0.06) + light * (0.4 + 0.4 * v);
        let mut e = 0.0;
        for (fx, _, col) in FAR_LAMPS {
            let k = (-((x - fx * w) / ((3.5 + 3.0 * v) * s)).powi(2)).exp() * (0.25 + 0.3 * v);
            c = c + col * (0.35 * k);
            e += 0.25 * k;
        }
        let line = (y - (street + 0.45 * (walk - street))).abs() < 1.2 * s && (x / (30.0 * s)).rem_euclid(2.0) < 1.0;
        if line {
            c = Vec3::new(0.33, 0.3, 0.18) + light * 0.3;
        }
        let zebra = ((x - 0.5 * w) / (22.0 * s)).floor() as i32;
        if (x - 0.5 * w).abs() < 0.22 * w && zebra.rem_euclid(2) == 0 && v > 0.55 && v < 0.95 {
            c = Vec3::new(0.45, 0.45, 0.44) + light * 0.4;
        }
        return (c, e.min(0.5));
    }
    let v = (y - walk) / (lay.counter - walk).max(1.0);
    let curb = y - walk < 3.0 * s;
    let seam = ((x - 0.5 * w) * (1.0 - 0.3 * v) / (38.0 * s)).rem_euclid(1.0) < 0.05;
    let cart = (-((x - 0.52 * w) / (0.45 * w)).powi(2)).exp();
    let mut c = if curb { Vec3::new(0.4, 0.38, 0.36) } else { Vec3::new(0.19, 0.18, 0.19) };
    if seam && !curb {
        c = c * 0.7;
    }
    c = c + BULB * (0.12 * cart * v) + SODIUM * (0.06 * (pool(0.04 * w) + pool(0.96 * w)));
    (c, 0.0)
}

/// El toldo (nitido, encima de lo desenfocado), si el pixel cae en el: la
/// faja de colores con cada pano abombado y sus arrugas, el tejido, la luz
/// de los focos que le da desde abajo y la orilla de ondas con su ribete.
/// Da el color y cuanto cubre (el borde de las ondas, suavizado).
fn awning(lay: &Layout, x: f32, y: f32) -> Option<(Vec3, f32)> {
    let s = lay.s;
    let stripe = 42.0 * s;
    let k = (x / stripe).floor();
    let u = x / stripe - k;
    let edge = lay.awning + 11.0 * s * (1.0 - (2.0 * u - 1.0).powi(2));
    let cover = (edge - y + 0.5).clamp(0.0, 1.0);
    if cover <= 0.0 {
        return None;
    }
    let mut lamp = 0.0;
    for (bx, _) in lay.bulbs() {
        lamp += (-((x - bx) / (130.0 * s)).powi(2)).exp();
    }
    let below = smoothstep(0.0, edge, y);
    let light = 0.42 + 0.3 * below + 0.5 * lamp * below;
    if y > edge - 3.0 * s {
        // El ribete blanco de la orilla.
        return Some((Vec3::new(0.9, 0.86, 0.78) * light, cover));
    }
    let base = FAJA[(k as i32).rem_euclid(4) as usize];
    let bulge = 0.7 + 0.3 * (PI * u).sin();
    let ripple = 0.94 + 0.06 * (y / (5.0 * s) + 3.0 * (PI * u).sin()).sin();
    let weave = if ((x / (2.0 * s)).floor() as i32 + (y / (2.0 * s)).floor() as i32).rem_euclid(2) == 0 { 1.03 } else { 0.97 };
    Some((base * (bulge * ripple * light * weave), cover))
}

/// El mostrador de acero visto desde arriba (lo de alla, el borde que da a
/// la gente, arriba): el pasamanos redondo con su brillo, el acero cepillado
/// (una veta por fila, corrida a lo largo) y el reflejo de Fresnel: de lejos
/// (rasante) refleja mas, y los focos se reflejan como estelas debajo de
/// cada uno. Da el color y lo que emite.
fn counter_top(lay: &Layout, x: f32, y: f32) -> (Vec3, f32) {
    let (h, s) = (lay.h, lay.s);
    let lip = lay.counter + 10.0 * s;
    if y < lip {
        let t = (y - lay.counter) / (lip - lay.counter);
        let hl = (-((t - 0.3) / 0.18).powi(2)).exp();
        return (Vec3::new(0.42, 0.43, 0.46) * (0.55 + 0.5 * (1.0 - t)) + BULB * (0.55 * hl), 0.15 * hl);
    }
    let v = ((y - lip) / (h - lip)).clamp(0.0, 1.0);
    let grain = (unit(hash_u32(y as u32 * 2_654_435_761)) - 0.5) * 0.045 + (value_noise(x / (90.0 * s), y / (3.0 * s)) - 0.5) * 0.04;
    let base = STEEL * (0.56 + 0.26 * v) + Vec3::splat(grain);
    let fresnel = 0.25 + 0.75 * (1.0 - v).powi(3);
    let mut refl = Vec3::new(0.12, 0.08, 0.14) * fresnel;
    let mut e = 0.0;
    for (bx, _) in lay.bulbs() {
        let k = (-((x - bx) / ((10.0 + 40.0 * v) * s)).powi(2)).exp() * (0.35 + 0.65 * fresnel);
        refl = refl + BULB * (0.55 * k);
        e += 0.25 * k * fresnel;
    }
    let smudge = value_noise(x / (40.0 * s) + 3.0, y / (25.0 * s));
    ((base + refl) * (0.92 + 0.12 * smudge), e.min(0.6))
}

/// Que tan abierta esta la torre a la altura `t` (0 abajo, 1 arriba) y en que
/// fila de la pantalla queda.
fn tower_leg(lay: &Layout, t: f32) -> f32 {
    0.11 * lay.w * (1.0 - t).powf(1.6) + 0.011 * lay.w
}

fn tower_at(lay: &Layout, t: f32) -> f32 {
    let (base, top) = (lay.street(), 0.085 * lay.h);
    base + (top - base) * t
}

/// La Torre del Reformador: dos patas de celosia que suben juntandose
/// (`x = 0.11 w (1 - t)^1.6`), cada una con su cordon de afuera, el de
/// adentro y sus diagonales; las cruces entre las patas, dos plataformas con
/// baranda, el arco de abajo y el campanario con su cupula. Los reflectores
/// del pie la alumbran calida y se apaga hacia arriba.
fn tower(lay: &Layout, c: &mut Canvas) {
    let (w, s) = (lay.w, lay.s);
    let cx = 0.5 * w;
    let leg = |t: f32| tower_leg(lay, t);
    let at = |t: f32| tower_at(lay, t);
    let lit = |t: f32| IRON_COLOR.lerp(Vec3::new(0.46, 0.28, 0.17), (1.0 - t).powi(2) * 0.85);
    c.ellipse(cx, at(0.0) - 0.04 * lay.h, 0.13 * w, 0.06 * lay.h, Vec3::new(1.0, 0.6, 0.3)).alpha(0.12).soft(30.0 * s).emit(0.1);
    let steps = 18;
    let inner = |t: f32| leg(t) - 0.014 * w * (1.0 - t);
    for side in [-1.0f32, 1.0] {
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let col = lit(t0);
            c.line(cx + side * leg(t0), at(t0), cx + side * leg(t1), at(t1), 3.2 * s, col);
            c.line(cx + side * inner(t0), at(t0), cx + side * inner(t1), at(t1), 1.8 * s, col);
            c.line(cx + side * leg(t0), at(t0), cx + side * inner(t1), at(t1), 0.8 * s, col);
            c.line(cx + side * inner(t0), at(t0), cx + side * leg(t1), at(t1), 0.8 * s, col);
        }
    }
    for i in 1..steps {
        let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
        if t0 < 0.22 {
            continue;
        }
        c.line(cx - inner(t0), at(t0), cx + inner(t1), at(t1), 1.1 * s, lit(t0));
        c.line(cx + inner(t0), at(t0), cx - inner(t1), at(t1), 1.1 * s, lit(t0));
    }
    for t in [0.36f32, 0.64] {
        let (y, hw) = (at(t), leg(t) + 8.0 * s);
        c.rect(cx, y, hw, 3.5 * s, 1.0 * s, lit(t));
        c.line(cx - hw, y - 7.0 * s, cx + hw, y - 7.0 * s, 0.8 * s, lit(t));
        for k in 0..=12 {
            let xk = cx - hw + 2.0 * hw * k as f32 / 12.0;
            c.line(xk, y - 7.0 * s, xk, y - 3.0 * s, 0.6 * s, lit(t));
        }
    }
    // El arco entre las patas, abajo.
    let arch_top = at(0.2);
    let n = 16;
    for i in 0..n {
        let (a0, a1) = (PI * i as f32 / n as f32, PI * (i + 1) as f32 / n as f32);
        let half = leg(0.0) * 0.86;
        let p = |a: f32| (cx - half * a.cos(), at(0.0) - (at(0.0) - arch_top) * a.sin());
        let (p0, p1) = (p(a0), p(a1));
        c.line(p0.0, p0.1, p1.0, p1.1, 2.4 * s, lit(0.1));
    }
    // El campanario, la cupula y la aguja.
    let top = at(1.0);
    c.rect(cx, top - 10.0 * s, 0.016 * w, 12.0 * s, 2.0 * s, lit(1.0));
    c.disc(cx, top - 10.0 * s, 4.0 * s, Vec3::new(0.5, 0.38, 0.15));
    c.ellipse(cx, top - 23.0 * s, 0.012 * w, 7.0 * s, lit(1.0));
    c.line(cx, top - 27.0 * s, cx, top - 44.0 * s, 1.4 * s, lit(1.0));
}

/// Lo demas de lejos (antes de desenfocar): los postes de los lados, la cruz
/// verde de una farmacia y un rotulo de neon en los edificios.
fn far_props(lay: &Layout, c: &mut Canvas) {
    let (w, h, s) = (lay.w, lay.h, lay.s);
    for (lx, dir) in [(0.04 * w, 1.0f32), (0.96 * w, -1.0)] {
        let (foot, head) = (0.57 * h, 0.2 * h);
        c.line(lx, foot, lx, head, 3.2 * s, Vec3::new(0.12, 0.12, 0.13));
        c.line(lx, head, lx + dir * 34.0 * s, head - 6.0 * s, 2.4 * s, Vec3::new(0.12, 0.12, 0.13));
    }
    let green = Vec3::new(0.2, 1.0, 0.45);
    let (fx, fy) = (0.17 * w, lay.street() - 0.115 * h);
    c.rect(fx, fy, 9.0 * s, 3.0 * s, 1.0 * s, green).emit(0.85).glow(6.0 * s, 0.5);
    c.rect(fx, fy, 3.0 * s, 9.0 * s, 1.0 * s, green).emit(0.85).glow(6.0 * s, 0.5);
    let pink = Vec3::new(1.0, 0.3, 0.7);
    let (nx, ny, hw, hh) = (0.83 * w, lay.street() - 0.13 * h, 26.0 * s, 9.0 * s);
    for (ax, ay, bx, by) in [(-hw, -hh, hw, -hh), (hw, -hh, hw, hh), (hw, hh, -hw, hh), (-hw, hh, -hw, -hh)] {
        c.line(nx + ax, ny + ay, nx + bx, ny + by, 1.3 * s, pink).emit(0.8).glow(5.0 * s, 0.45);
    }
    for k in 0..4 {
        let x = nx - hw + (k as f32 + 1.0) * 2.0 * hw / 5.0;
        c.line(x, ny - 4.0 * s, x, ny + 4.0 * s, 1.0 * s, Vec3::new(0.4, 0.9, 1.0)).emit(0.8).glow(4.0 * s, 0.4);
    }
}

/// Las luces de lejos ya desenfocadas: los focos de la torre y su faro
/// rojo, los postes de la avenida como discos de luz con la orilla un poco
/// mas brillante (como los de una lente), el semaforo y las cabezas de los
/// postes de los lados.
fn bokeh(lay: &Layout, c: &mut Canvas) {
    let (w, h, s) = (lay.w, lay.h, lay.s);
    let cx = 0.5 * w;
    for i in 0..12 {
        let t = 0.06 + 0.075 * i as f32;
        for side in [-1.0f32, 1.0] {
            c.disc(cx + side * tower_leg(lay, t), tower_at(lay, t), 2.6 * s, Vec3::new(1.0, 0.86, 0.6)).soft(1.4 * s).alpha(0.9).emit(0.75).glow(4.0 * s, 0.35);
        }
    }
    c.disc(cx, tower_at(lay, 1.0) - 46.0 * s, 3.4 * s, Vec3::new(1.0, 0.15, 0.1)).emit(1.0).glow(10.0 * s, 0.8);
    for (fx, fy, col) in FAR_LAMPS {
        c.disc(fx * w, fy * h, 6.5 * s, col).alpha(0.5).soft(1.5 * s).emit(0.6).glow(9.0 * s, 0.22);
        c.arc(fx * w, fy * h, 5.6 * s, 0.9 * s, -PI, PI, col).alpha(0.3).soft(1.2 * s).emit(0.6);
    }
    for (lx, dir) in [(0.04 * w, 1.0f32), (0.96 * w, -1.0)] {
        let head = 0.2 * h;
        c.rect(lx + dir * 40.0 * s, head - 4.0 * s, 14.0 * s, 4.0 * s, 2.0 * s, Vec3::new(1.0, 0.8, 0.5)).emit(1.0).glow(30.0 * s, 0.6);
    }
}

/// Lo que cuelga del toldo: la serie (el alambre entre las ondas; los
/// foquitos van aparte porque titilan) y los tres focos con su cable y su
/// socket.
fn hanging(lay: &Layout, c: &mut Canvas) {
    let s = lay.s;
    let stripe = 42.0 * s;
    let n = (lay.w / stripe) as usize + 1;
    let wire = Vec3::new(0.05, 0.05, 0.05);
    for i in 0..n {
        let (x0, x1) = ((i as f32 + 0.5) * stripe, (i as f32 + 1.5) * stripe);
        let y = lay.awning + 13.0 * s;
        let mid = (0.5 * (x0 + x1), y + 4.0 * s);
        c.line(x0, y, mid.0, mid.1, 0.7 * s, wire);
        c.line(mid.0, mid.1, x1, y, 0.7 * s, wire);
    }
    for (bx, by) in lay.bulbs() {
        c.line(bx, lay.awning + 6.0 * s, bx, by - 9.0 * s, 1.1 * s, wire);
        c.rect(bx, by - 7.0 * s, 4.5 * s, 4.0 * s, 1.5 * s, Vec3::new(0.12, 0.1, 0.08)).vol(3.0 * s).gloss(0.6);
        c.ellipse(bx, by + 2.0 * s, 6.5 * s, 8.0 * s, Vec3::new(1.0, 0.9, 0.7)).emit(1.0).glow(22.0 * s, 0.55);
        c.ellipse(bx, by + 2.0 * s, 3.0 * s, 4.0 * s, Vec3::ONE).emit(1.0);
    }
}

/// Lo que esta quieto en el mostrador, visto desde arriba y con volumen: la
/// canasta del pan con su servilleta tipica, la plancha de hierro hundida en
/// el acero (con su frente, sus perillas y las rayas de la parrilla que se
/// juntan hacia alla), la tabla de picar con su canto, los tazones de barro
/// y los botes de las salsas. Cada cosa con su sombra en el acero.
fn counter_items(lay: &Layout, c: &mut Canvas) {
    let s = lay.s;
    let m = |v: f32| v * s;
    let shadow = Vec3::ZERO;
    // La canasta, la servilleta de rayas y tres panes franceses.
    let (bx, by) = lay.basket();
    c.ellipse(bx, by + m(56.0), m(72.0), m(13.0), shadow).alpha(0.45).soft(m(10.0));
    c.rect(bx, by, m(64.0), m(56.0), m(14.0), Vec3::new(0.56, 0.34, 0.15)).vol(m(12.0)).gloss(0.1);
    for i in 0..9 {
        let y = by - m(50.0) + i as f32 * m(12.5);
        c.line(bx - m(60.0), y, bx + m(60.0), y, m(1.4), Vec3::new(0.32, 0.18, 0.07)).alpha(0.55);
    }
    c.rect(bx, by + m(3.0), m(54.0), m(46.0), m(10.0), Vec3::new(0.26, 0.15, 0.06)).vol(m(6.0));
    c.rect(bx, by + m(4.0), m(52.0), m(44.0), m(8.0), FAJA[0] * 0.95).vol(m(5.0));
    for (k, col) in [FAJA[1], FAJA[2], FAJA[3], FAJA[1]].into_iter().enumerate() {
        let y = by - m(30.0) + k as f32 * m(20.0);
        c.line(bx - m(50.0), y, bx + m(50.0), y, m(2.6), col).alpha(0.9);
        c.line(bx - m(50.0), y + m(4.5), bx + m(50.0), y + m(4.5), m(0.9), Vec3::splat(0.95)).alpha(0.7);
    }
    for i in 0..3 {
        let y = by - m(24.0) + i as f32 * m(25.0);
        c.ellipse(bx + m(2.0), y + m(9.0), m(40.0), m(7.0), shadow).alpha(0.35).soft(m(5.0));
        c.line(bx - m(34.0), y, bx + m(34.0), y - m(3.0), m(13.0), Vec3::new(0.86, 0.58, 0.28)).vol(m(12.0)).gloss(0.18);
        c.line(bx - m(28.0), y - m(5.0), bx + m(28.0), y - m(8.0), m(2.2), Vec3::new(0.98, 0.84, 0.58)).alpha(0.75);
    }
    // La plancha: el marco, la plancha hundida (un trapecio: lo de alla mas
    // angosto) y su frente con las perillas.
    let (px, py, phx, phy) = lay.plancha();
    let trap = |mx: f32, my: f32| [(px - (phx + mx) * 0.94, py - phy - my), (px + (phx + mx) * 0.94, py - phy - my), (px + phx + mx, py + phy + my), (px - phx - mx, py + phy + my)];
    c.rect(px, py + phy + m(26.0), phx + m(14.0), m(10.0), m(6.0), shadow).alpha(0.45).soft(m(10.0));
    c.poly(&[(px - phx - m(10.0), py + phy + m(9.0)), (px + phx + m(10.0), py + phy + m(9.0)), (px + phx + m(9.0), py + phy + m(27.0)), (px - phx - m(9.0), py + phy + m(27.0))], Vec3::new(0.18, 0.18, 0.2)).vol(m(5.0)).gloss(0.4);
    for k in [-0.6f32, 0.0, 0.6] {
        let kx = px + k * phx;
        c.disc(kx, py + phy + m(18.0), m(6.0), Vec3::new(0.08, 0.08, 0.09)).vol(m(6.0)).gloss(0.7);
        c.line(kx, py + phy + m(13.0), kx, py + phy + m(16.0), m(1.0), Vec3::splat(0.9));
    }
    c.poly(&trap(m(10.0), m(9.0)), Vec3::new(0.26, 0.26, 0.28)).vol(m(8.0)).gloss(0.55);
    c.poly(&trap(0.0, 0.0), Vec3::new(0.085, 0.085, 0.095)).vol(m(3.0)).gloss(0.35);
    let lines = 9;
    for i in 1..lines {
        let f = i as f32 / lines as f32 * 2.0 - 1.0;
        c.line(px + f * phx * 0.94, py - phy + m(5.0), px + f * phx, py + phy - m(5.0), m(1.2), Vec3::new(0.14, 0.13, 0.13)).alpha(0.8);
    }
    for k in 0..6u32 {
        let hk = hash_u32(k * 4099 + 7);
        c.disc(px + (unit(hk) - 0.5) * 1.7 * phx, py + (unit(hk >> 9) - 0.5) * 1.6 * phy, m(9.0 + 8.0 * unit(hk >> 3)), Vec3::new(0.03, 0.025, 0.02)).alpha(0.35).soft(m(6.0));
    }
    for (bxl, _) in lay.bulbs() {
        if (bxl - px).abs() < phx {
            c.ellipse(bxl, py - phy * 0.35, m(14.0), phy * 0.5, BULB).alpha(0.1).soft(m(12.0));
        }
    }
    // La tabla de picar: el canto y la cara de arriba con su veta.
    let (tx, ty, thx, thy) = lay.board();
    c.rect(tx + m(4.0), ty + thy + m(14.0), thx + m(6.0), m(8.0), m(6.0), shadow).alpha(0.45).soft(m(9.0));
    c.rect(tx, ty + thy + m(5.0), thx, m(8.0), m(5.0), Vec3::new(0.36, 0.21, 0.1)).vol(m(5.0));
    c.rect(tx, ty, thx, thy, m(12.0), Vec3::new(0.62, 0.41, 0.22)).vol(m(9.0)).gloss(0.08);
    for i in 0..7 {
        let y = ty - thy + (i as f32 + 0.6) * 2.0 * thy / 7.5;
        c.line(tx - thx + m(12.0), y, tx + thx - m(12.0), y + m(4.0), m(1.1), Vec3::new(0.48, 0.3, 0.15)).alpha(0.5);
    }
    c.ellipse(tx + m(70.0), ty - m(26.0), m(9.0), m(4.0), Vec3::new(0.45, 0.28, 0.13)).alpha(0.6);
    for k in 0..5u32 {
        let hk = hash_u32(k * 733 + 91);
        let (x0, y0) = (tx + (unit(hk) - 0.5) * 1.6 * thx, ty + (unit(hk >> 8) - 0.5) * 1.4 * thy);
        c.line(x0, y0, x0 + m(18.0), y0 - m(5.0), m(0.6), Vec3::new(0.85, 0.7, 0.5)).alpha(0.35);
    }
    // Los tazones de barro (guacamol, repollo, cebolla) con su franja de
    // puntitos blancos, y los botes (mostaza, mayonesa, salsa de tomate).
    for t in Top::ALL {
        let (x, y) = lay.top_at(t.index());
        let col = t.color();
        if t.index() < 3 {
            c.ellipse(x + m(3.0), y + m(30.0), m(46.0), m(11.0), shadow).alpha(0.45).soft(m(8.0));
            c.ellipse(x, y + m(7.0), m(42.0), m(29.0), Vec3::new(0.6, 0.3, 0.16)).vol(m(16.0)).gloss(0.3);
            for k in 0..11 {
                let a = PI * (0.12 + 0.76 * k as f32 / 10.0);
                c.disc(x - m(36.0) * a.cos(), y + m(10.0) + m(17.0) * a.sin(), m(1.6), Vec3::new(0.95, 0.9, 0.82)).alpha(0.85);
            }
            c.ellipse(x, y - m(2.0), m(40.0), m(21.0), Vec3::new(0.72, 0.4, 0.23)).vol(m(6.0)).gloss(0.4);
            c.ellipse(x, y - m(1.0), m(33.0), m(15.0), col * 0.85).vol(m(10.0)).gloss(if t == Top::Guacamol { 0.35 } else { 0.1 });
            let seed = t.index() as u32 * 977;
            for k in 0..16u32 {
                let hk = hash_u32(seed + k);
                let (a, r) = (TAU * unit(hk), unit(hk >> 5).sqrt());
                let (dx, dy) = (a.cos() * r * m(27.0), a.sin() * r * m(11.0) - m(1.0));
                match t {
                    Top::Repollo => {
                        c.line(x + dx - m(6.0), y + dy, x + dx + m(6.0), y + dy - m(2.0), m(1.5), Vec3::new(0.9, 0.96, 0.8));
                    }
                    Top::Cebolla => {
                        c.rect(x + dx, y + dy, m(2.8), m(2.4), m(0.8), Vec3::new(0.97, 0.95, 0.99)).vol(m(1.5));
                    }
                    _ => {
                        c.disc(x + dx, y + dy, m(2.8), col * 1.25).alpha(0.6).vol(m(2.0));
                    }
                }
            }
        } else {
            let (cap, tip) = if t == Top::Tomate { (Vec3::new(0.55, 0.05, 0.04), col) } else { (Vec3::new(0.95, 0.95, 0.93), col) };
            c.ellipse(x + m(5.0), y + m(43.0), m(26.0), m(7.0), shadow).alpha(0.5).soft(m(6.0));
            c.rect(x, y, m(19.0), m(42.0), m(9.0), col).vol(m(18.0)).gloss(0.7);
            c.rect(x, y + m(6.0), m(19.3), m(12.0), m(2.0), Vec3::new(0.95, 0.93, 0.86)).vol(m(5.0)).gloss(0.2);
            c.disc(x, y + m(6.0), m(6.0), col * 0.8);
            c.rect(x, y - m(46.0), m(11.0), m(6.0), m(3.0), cap).vol(m(5.0)).gloss(0.5);
            c.line(x, y - m(52.0), x + m(3.0), y - m(64.0), m(2.6), cap).vol(m(2.5)).gloss(0.5);
            c.disc(x + m(3.2), y - m(65.0), m(2.2), tip).gloss(0.6).vol(m(2.0));
        }
    }
}

/// Desenfoca las filas `[0, rows)` de la imagen, el color y la emision:
/// tres pasadas de caja de radio `r` en cada sentido, que juntas se parecen
/// a una gaussiana (teorema del limite central).
fn blur_rgba(img: &mut [u8], w: usize, rows: usize, r: usize) {
    if r == 0 || rows == 0 || w == 0 {
        return;
    }
    let rows = rows.min(img.len() / (4 * w));
    let mut buf: Vec<[f32; 4]> = img[..rows * w * 4].chunks_exact(4).map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]).collect();
    let scale = 1.0 / (2 * r + 1) as f32;
    let ri = r as isize;
    for _ in 0..3 {
        parallel::chunks_mut(&mut buf, w, 1, |_, row| {
            let src = row.to_vec();
            let at = |i: isize| src[i.clamp(0, w as isize - 1) as usize];
            let mut sum = [0.0f32; 4];
            for i in -ri..=ri {
                let p = at(i);
                for k in 0..4 {
                    sum[k] += p[k];
                }
            }
            for x in 0..w as isize {
                for k in 0..4 {
                    row[x as usize][k] = sum[k] * scale;
                }
                let (a, b) = (at(x + ri + 1), at(x - ri));
                for k in 0..4 {
                    sum[k] += a[k] - b[k];
                }
            }
        });
        let src = buf.clone();
        let line = |y: isize| {
            let y = y.clamp(0, rows as isize - 1) as usize;
            &src[y * w..(y + 1) * w]
        };
        let mut sum = vec![[0.0f32; 4]; w];
        for y in -ri..=ri {
            for (sm, p) in sum.iter_mut().zip(line(y)) {
                for k in 0..4 {
                    sm[k] += p[k];
                }
            }
        }
        for y in 0..rows as isize {
            let out = &mut buf[y as usize * w..(y as usize + 1) * w];
            let (enter, leave) = (line(y + ri + 1), line(y - ri));
            for x in 0..w {
                for k in 0..4 {
                    out[x][k] = sum[x][k] * scale;
                    sum[x][k] += enter[x][k] - leave[x][k];
                }
            }
        }
    }
    for (p, q) in img[..rows * w * 4].chunks_exact_mut(4).zip(&buf) {
        for k in 0..4 {
            p[k] = (q[k] + 0.5).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Hornea la escena quieta de una vez: el fondo lejano con la torre, sus
/// postes y rotulos, desenfocado; encima las luces de lejos (ya como
/// manchas), el toldo, la serie y los focos, el mostrador y lo que esta en
/// el, nitidos.
fn bake_scene(lay: &Layout) -> Vec<u8> {
    let (w, h) = (lay.width, lay.height);
    let mut img = vec![0u8; w * h * 4];
    parallel::chunks_mut(&mut img, w * 4, 1, |y, row| {
        let yf = y as f32 + 0.5;
        for x in 0..w {
            let xf = x as f32 + 0.5;
            let (c, e) = if yf < lay.counter { far(lay, xf, yf) } else { counter_top(lay, xf, yf) };
            lienzo::write_emit(&mut row[x * 4..x * 4 + 4], c, e);
        }
    });
    let mut back = Canvas::default();
    tower(lay, &mut back);
    far_props(lay, &mut back);
    parallel::chunks_mut(&mut img, w * 4, 1, |y, row| {
        let yf = y as f32 + 0.5;
        if yf < lay.counter {
            paint(&back.shapes, yf, row, w, None);
        }
    });
    blur_rgba(&mut img, w, (lay.counter as usize).min(h), (BLUR * lay.s).round().max(1.0) as usize);
    let mut lights = Canvas::default();
    bokeh(lay, &mut lights);
    let mut near = Canvas::default();
    hanging(lay, &mut near);
    counter_items(lay, &mut near);
    let light = counter_light();
    let top = lay.awning + 16.0 * lay.s;
    parallel::chunks_mut(&mut img, w * 4, 1, |y, row| {
        let yf = y as f32 + 0.5;
        if yf < lay.counter {
            paint(&lights.shapes, yf, row, w, None);
        }
        if yf < top {
            for x in 0..w {
                if let Some((c, a)) = awning(lay, x as f32 + 0.5, yf) {
                    let p = &mut row[x * 4..x * 4 + 4];
                    let (old, e) = (lienzo::read(p), lienzo::emission(p));
                    lienzo::write_emit(p, old.lerp(c, a), e * (1.0 - a));
                }
            }
        }
        paint_lit(&near.shapes, yf, row, w, None, &light);
    });
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
/// el mostrador). Cada pieza tiene volumen (`figuras`): el cuerpo con los
/// hombros redondos, los brazos con las manos en el mostrador, la cabeza un
/// poco de huevo con sus orejas, los ojos con su brillo, las cejas, la
/// nariz y la boca segun como este, y lo de cada quien.
fn person(c: &mut Canvas, lay: &Layout, who: Who, look: &Look, x: f32, bob: f32, face: Face) {
    let s = lay.s;
    let m = |v: f32| v * s;
    let base = lay.counter;
    let hy = lay.head_y() + bob;
    let by = 0.5 * bob;
    let shirt = look.shirt;
    let skin = if face == Face::Angry { look.skin.lerp(Vec3::new(0.85, 0.22, 0.16), 0.25) } else { look.skin };
    let ink = Vec3::new(0.06, 0.04, 0.04);
    let white = Vec3::new(0.95, 0.94, 0.92);
    // Lo de atras: el chongo de la senora.
    if who == Who::Senora {
        c.disc(x + m(21.0), hy - m(28.0), m(15.0), look.hair).vol(m(14.0)).gloss(0.4);
    }
    // El cuerpo y el cuello, con la sombra de la quijada.
    c.rect(x, base - m(40.0) + by, m(57.0), m(98.0), m(36.0), shirt).vol(m(30.0)).gloss(0.06);
    c.rect(x, hy + m(37.0), m(12.5), m(15.0), m(6.0), skin * 0.85).vol(m(9.0));
    c.ellipse(x, hy + m(31.0), m(16.0), m(6.0), Vec3::ZERO).alpha(0.3).soft(m(4.0));
    match who {
        Who::Estudiante => {
            // La playera polo con su cuello y los tirantes de la mochila.
            for side in [-1.0f32, 1.0] {
                c.poly(&[(x + side * m(15.0), hy + m(43.0)), (x + side * m(1.0), hy + m(55.0)), (x + side * m(5.0), hy + m(62.0)), (x + side * m(22.0), hy + m(50.0))], shirt * 1.15).vol(m(3.0));
                c.line(x + side * m(31.0), hy + m(50.0) + by, x + side * m(36.0), base, m(5.5), Vec3::new(0.12, 0.12, 0.16)).vol(m(4.0)).gloss(0.2);
            }
            c.disc(x, hy + m(70.0) + by, m(2.2), shirt * 0.7);
        }
        Who::Agente => {
            // El chaleco que refleja con sus franjas plateadas, la placa y el pito.
            for side in [-1.0f32, 1.0] {
                c.rect(x + side * m(33.0), base - m(36.0) + by, m(21.0), m(84.0), m(10.0), Vec3::new(0.76, 0.92, 0.16)).vol(m(10.0)).gloss(0.15);
            }
            for dy in [-78.0f32, -40.0] {
                c.rect(x, base + m(dy) + by, m(56.0), m(4.5), m(2.0), Vec3::new(0.85, 0.87, 0.9)).vol(m(3.0)).gloss(0.9);
            }
            c.disc(x - m(30.0), hy + m(68.0) + by, m(5.5), Vec3::new(0.95, 0.78, 0.25)).vol(m(5.0)).gloss(0.8);
            c.line(x + m(5.0), hy + m(45.0), x + m(15.0), hy + m(80.0) + by, m(0.9), white);
            c.ellipse(x + m(15.0), hy + m(83.0) + by, m(5.0), m(3.5), Vec3::new(0.75, 0.77, 0.8)).vol(m(3.0)).gloss(0.9);
        }
        Who::Senora => {
            // El delantal con florecitas y el perraje de rayas en los hombros.
            c.rect(x, base - m(30.0) + by, m(42.0), m(56.0), m(12.0), Vec3::new(0.93, 0.91, 0.86)).vol(m(8.0));
            for k in 0..7u32 {
                let hk = hash_u32(look.seed ^ (k * 977));
                let (fx, fy) = (x + (unit(hk) - 0.5) * m(62.0), base - m(72.0) + unit(hk >> 6) * m(62.0) + by);
                c.disc(fx, fy, m(4.0), Vec3::new(0.9, 0.35, 0.55)).vol(m(3.0));
                c.disc(fx, fy, m(1.4), Vec3::new(1.0, 0.85, 0.3));
            }
            for side in [-1.0f32, 1.0] {
                let (a, b) = ((x + side * m(14.0), hy + m(46.0)), (x + side * m(50.0), base - m(24.0) + by));
                c.line(a.0, a.1, b.0, b.1, m(11.0), FAJA[3] * 0.9).vol(m(8.0));
                c.line(a.0, a.1, b.0, b.1, m(3.5), FAJA[1]);
                c.line(a.0 + side * m(5.0), a.1, b.0 + side * m(5.0), b.1, m(1.2), FAJA[0]);
            }
        }
        Who::Bolo => {
            // La camiseta blanca y la camisa de cuadros abierta encima.
            c.rect(x, base - m(40.0) + by, m(22.0), m(92.0), m(8.0), Vec3::new(0.84, 0.8, 0.7)).vol(m(8.0));
            for k in -3..=3 {
                if k == 0 {
                    continue;
                }
                let xk = x + k as f32 * m(14.0);
                c.line(xk, hy + m(48.0), xk, base, m(1.8), shirt * 1.45).alpha(0.5);
            }
            for k in 0..5 {
                let yk = hy + m(56.0) + k as f32 * m(22.0) + by;
                c.line(x - m(54.0), yk, x - m(24.0), yk, m(1.6), shirt * 0.6).alpha(0.5);
                c.line(x + m(24.0), yk, x + m(54.0), yk, m(1.6), shirt * 0.6).alpha(0.5);
            }
            c.disc(x + m(6.0), base - m(52.0) + by, m(7.0), Vec3::new(0.5, 0.35, 0.15)).alpha(0.3).soft(m(3.0));
        }
        Who::Aficionado => {
            // La camisola roja con el cuello blanco en V, la franja y la bufanda.
            c.poly(&[(x - m(16.0), hy + m(44.0)), (x + m(16.0), hy + m(44.0)), (x, hy + m(64.0))], white).vol(m(3.0));
            c.poly(&[(x - m(11.0), hy + m(44.0)), (x + m(11.0), hy + m(44.0)), (x, hy + m(57.0))], skin * 0.8);
            c.rect(x, base - m(86.0) + by, m(57.0), m(6.0), m(2.0), white).vol(m(4.0));
            let (a, b) = ((x - m(26.0), hy + m(48.0)), (x + m(18.0), base - m(36.0) + by));
            for k in 0..5 {
                let (t0, t1) = (k as f32 / 5.0, (k + 1) as f32 / 5.0);
                let col = if k % 2 == 0 { Vec3::new(0.82, 0.08, 0.1) } else { white };
                c.line(a.0 + (b.0 - a.0) * t0, a.1 + (b.1 - a.1) * t0, a.0 + (b.0 - a.0) * t1, a.1 + (b.1 - a.1) * t1, m(8.5), col).vol(m(6.0));
            }
        }
        Who::Meyer => {}
    }
    // Los brazos, con las manos en el mostrador (el bolo con su botella).
    for side in [-1.0f32, 1.0] {
        let shoulder = (x + side * m(45.0), hy + m(70.0) + by);
        let hand = (x + side * m(67.0), base - m(8.0));
        let sleeve = if who == Who::Agente { Vec3::new(0.12, 0.2, 0.45) } else { shirt * 0.92 };
        c.line(shoulder.0, shoulder.1, hand.0, hand.1 - m(10.0), m(13.5), sleeve).vol(m(13.0));
        c.ellipse(hand.0, hand.1, m(13.0), m(10.0), skin).vol(m(9.0)).gloss(0.12);
    }
    if who == Who::Bolo {
        let (hx, hy2) = (x + m(67.0), base - m(8.0));
        let glass = Vec3::new(0.42, 0.22, 0.05);
        c.line(hx + m(4.0), hy2 - m(10.0), hx + m(9.0), hy2 - m(58.0), m(8.5), glass).vol(m(8.0)).gloss(0.9).alpha(0.92);
        c.line(hx + m(9.0), hy2 - m(58.0), hx + m(11.0), hy2 - m(76.0), m(3.8), glass).vol(m(3.5)).gloss(0.9);
        c.rect(hx + m(6.5), hy2 - m(33.0), m(7.5), m(9.0), m(2.0), Vec3::new(0.9, 0.85, 0.6)).vol(m(6.0));
        c.ellipse(hx, hy2, m(13.0), m(10.0), skin).vol(m(9.0));
    }
    // Las orejas, el pelo de atras y la cabeza.
    for side in [-1.0f32, 1.0] {
        c.ellipse(x + side * m(29.0), hy + m(4.0), m(6.5), m(9.5), skin * 0.92).vol(m(5.0));
    }
    match who {
        Who::Bolo => {
            for k in 0..8u32 {
                let side = if k < 4 { -1.0 } else { 1.0 };
                let a = PI * (0.9 + 0.1 * (k % 4) as f32);
                c.disc(x + side * a.cos().abs() * m(27.0), hy - a.sin().abs() * m(14.0) - m(2.0), m(9.0), look.hair).vol(m(7.0));
            }
        }
        Who::Agente | Who::Aficionado => {}
        _ => {
            c.ellipse(x, hy - m(8.0), m(32.5), m(31.0), look.hair).vol(m(18.0)).gloss(0.35);
        }
    }
    c.ellipse(x, hy + m(2.0), m(29.0), m(33.0), skin).vol(m(27.0)).gloss(if who == Who::Bolo { 0.3 } else { 0.1 });
    match who {
        Who::Estudiante => {
            for k in 0..5 {
                let fx = x - m(19.0) + k as f32 * m(9.5);
                c.ellipse(fx, hy - m(25.0) + m(2.0) * ((k % 2) as f32), m(9.5), m(7.5), look.hair).vol(m(6.0)).gloss(0.4);
            }
        }
        Who::Senora => {
            for side in [-1.0f32, 1.0] {
                c.ellipse(x + side * m(14.0), hy - m(23.0), m(18.0), m(10.0), look.hair).vol(m(7.0)).gloss(0.45);
            }
        }
        _ => {}
    }
    // La cara.
    let eye_y = hy - m(1.0);
    for side in [-1.0f32, 1.0] {
        let ex = x + side * m(11.0);
        if face == Face::Happy {
            c.arc(ex, eye_y + m(3.0), m(5.5), m(1.5), -0.92 * PI, -0.08 * PI, ink);
        } else {
            let red = if who == Who::Bolo { Vec3::new(1.0, 0.82, 0.8) } else { Vec3::ONE };
            c.ellipse(ex, eye_y, m(6.0), m(4.6), white * red).vol(m(3.0));
            c.disc(ex, eye_y + m(0.5), m(3.4), Vec3::new(0.25, 0.15, 0.08));
            c.disc(ex, eye_y + m(0.5), m(1.8), ink);
            c.disc(ex - m(1.1), eye_y - m(0.9), m(0.9), Vec3::ONE);
            let lid = match (who, face) {
                (Who::Bolo, _) => 0.55,
                (_, Face::Angry) => 0.4,
                _ => 0.0,
            };
            if lid > 0.0 {
                c.rect(ex, eye_y - m(4.6) + m(4.6) * lid, m(6.6), m(4.6) * lid + m(0.6), m(1.0), skin * 0.95);
            }
            c.line(ex - m(6.0), eye_y - m(4.6) + m(9.2) * lid, ex + m(6.0), eye_y - m(4.9) + m(9.2) * lid, m(1.1), ink);
        }
        let (inner, outer) = match face {
            Face::Angry => (m(-6.5), m(-11.5)),
            Face::Happy => (m(-11.5), m(-10.5)),
            Face::Calm => (m(-9.5), m(-10.0)),
        };
        c.line(x + side * m(5.0), eye_y + inner, x + side * m(16.0), eye_y + outer, m(1.9), look.hair * 0.9 + Vec3::splat(0.02));
    }
    c.ellipse(x, hy + m(10.0), m(4.6), m(6.5), skin).vol(m(4.0)).gloss(0.2);
    c.ellipse(x, hy + m(15.5), m(4.2), m(1.5), Vec3::ZERO).alpha(0.3).soft(m(1.5));
    for side in [-1.0f32, 1.0] {
        c.disc(x + side * m(17.0), hy + m(13.0), m(6.0), Vec3::new(0.95, 0.42, 0.38)).alpha(if who == Who::Bolo { 0.4 } else { 0.16 }).soft(m(4.0));
    }
    let lips = Vec3::new(0.45, 0.14, 0.13);
    match face {
        Face::Happy => {
            c.ellipse(x, hy + m(21.0), m(9.0), m(5.5), Vec3::new(0.3, 0.06, 0.07));
            c.rect(x, hy + m(17.5), m(7.0), m(1.7), m(1.0), white);
        }
        Face::Angry => {
            c.arc(x, hy + m(29.0), m(9.0), m(1.6), -0.8 * PI, -0.2 * PI, lips);
        }
        Face::Calm => {
            c.arc(x, hy + m(12.0), m(9.0), m(1.5), 0.3 * PI, 0.7 * PI, lips);
        }
    }
    // Lo de cada quien en la cabeza.
    match who {
        Who::Estudiante => {
            let frame = Vec3::new(0.1, 0.1, 0.12);
            for side in [-1.0f32, 1.0] {
                c.arc(x + side * m(11.0), eye_y, m(8.5), m(1.3), -PI, PI, frame);
                c.line(x + side * m(7.0), eye_y - m(4.0), x + side * m(10.0), eye_y - m(6.0), m(0.8), Vec3::ONE).alpha(0.5);
            }
            c.line(x - m(2.5), eye_y, x + m(2.5), eye_y, m(1.2), frame);
        }
        Who::Agente => {
            let navy = Vec3::new(0.08, 0.12, 0.32);
            c.rect(x, hy - m(25.0), m(33.0), m(13.0), m(10.0), navy).vol(m(8.0)).gloss(0.2);
            c.ellipse(x, hy - m(13.0), m(36.0), m(5.5), Vec3::new(0.03, 0.03, 0.05)).vol(m(3.0)).gloss(0.8);
            c.disc(x, hy - m(27.0), m(5.0), Vec3::new(0.95, 0.78, 0.25)).vol(m(4.0)).gloss(0.9);
        }
        Who::Senora => {
            for side in [-1.0f32, 1.0] {
                c.disc(x + side * m(30.0), hy + m(14.0), m(3.5), Vec3::new(0.95, 0.78, 0.25)).vol(m(3.0)).gloss(0.9);
            }
        }
        Who::Bolo => {
            for k in 0..22u32 {
                let hk = hash_u32(look.seed ^ (k * 131));
                let a = PI * (0.15 + 0.7 * unit(hk));
                let r = m(19.0) + unit(hk >> 7) * m(9.0);
                c.disc(x + a.cos() * r, hy + m(6.0) + a.sin() * r * 0.8, m(0.9), look.hair).alpha(0.45);
            }
            c.disc(x, hy + m(10.0), m(5.5), Vec3::new(0.9, 0.2, 0.18)).alpha(0.35).soft(m(2.5));
            c.ellipse(x + m(24.0), hy - m(12.0), m(2.2), m(3.4), Vec3::new(0.75, 0.88, 1.0)).alpha(0.7).vol(m(2.0)).gloss(1.0);
        }
        Who::Aficionado => {
            let red = Vec3::new(0.8, 0.08, 0.1);
            c.rect(x, hy - m(24.0), m(31.0), m(11.0), m(9.0), red).vol(m(7.0)).gloss(0.2);
            c.rect(x + m(2.0), hy - m(33.0), m(16.0), m(4.0), m(3.0), red * 0.75).vol(m(3.0));
            for side in [-1.0f32, 1.0] {
                c.rect(x + side * m(17.0), hy + m(10.0), m(2.0), m(5.0), m(1.0), red);
                c.rect(x + side * m(21.0), hy + m(10.0), m(2.0), m(5.0), m(1.0), white);
            }
        }
        Who::Meyer => {}
    }
}

/// Una carne en `(x, y)` a lo largo (`len` es medio largo) a los `t`
/// segundos de plancha, con volumen y el brillo de la grasa (la quemada ya
/// no brilla). En la plancha chirria (puntitos que brillan).
#[allow(clippy::too_many_arguments)]
fn draw_meat(c: &mut Canvas, meat: Meat, x: f32, y: f32, len: f32, t: f32, s: f32, sizzle: Option<f32>) {
    let col = meat_color(meat, t);
    let (ready, burn) = meat.times();
    let gloss = if t < burn { 0.6 } else { 0.08 };
    c.line(x - len, y + 8.0 * s, x + len, y + 8.0 * s, 9.0 * s, Vec3::ZERO).alpha(0.4).soft(4.0 * s);
    match meat {
        Meat::Salchicha => {
            c.line(x - len, y, x + len, y, 13.0 * s, col).vol(12.0 * s).gloss(gloss);
        }
        Meat::Longaniza => {
            c.line(x - len, y, x + len, y, 14.5 * s, col).vol(13.0 * s).gloss(gloss);
            for k in -2..=2 {
                let xk = x + k as f32 * len * 0.4;
                c.line(xk - 2.0 * s, y - 12.0 * s, xk + 2.0 * s, y + 12.0 * s, 1.6 * s, col * 0.6).alpha(0.8);
            }
        }
        Meat::Chorizo => {
            c.line(x - 0.8 * len, y, x + 0.8 * len, y, 16.0 * s, col).vol(15.0 * s).gloss(gloss);
            for side in [-1.0f32, 1.0] {
                c.disc(x + side * (0.8 * len + 15.0 * s), y, 4.5 * s, col * 0.6).vol(4.0 * s);
            }
        }
        Meat::Salami => {
            for k in [-1.0f32, 0.0, 1.0] {
                let (sx, sy) = (x + k * len * 0.55, y + k.abs() * 2.0 * s);
                c.ellipse(sx, sy, 19.0 * s, 15.0 * s, col).vol(6.0 * s).gloss(gloss * 0.6);
                for j in 0..4u32 {
                    let hj = hash_u32(j * 31 + (k + 2.0) as u32 * 7);
                    c.disc(sx + (unit(hj) - 0.5) * 22.0 * s, sy + (unit(hj >> 7) - 0.5) * 16.0 * s, 2.4 * s, Vec3::new(0.95, 0.85, 0.8) * (0.4 + 0.6 * col.max_element())).alpha(0.8);
                }
            }
        }
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
        for k in 0..6u32 {
            let hk = hash_u32(k * 7919 + (wall * 20.0) as u32 * 31 + meat.index() as u32 * 131);
            if hk % 3 == 0 && !burnt {
                let px = x + (unit(hk) - 0.5) * 2.2 * len;
                let py = y + (unit(hk >> 9) - 0.5) * 30.0 * s;
                c.disc(px, py, 1.6 * s, Vec3::new(1.0, 0.92, 0.7)).glow(3.0 * s, 0.6).emit(0.8);
            }
        }
    }
}

/// El shuco en la tabla, armado por capas como uno de verdad: el pan
/// abierto (la tapa atras, la base adelante, la miga), las carnes, el
/// guacamol, el repollo, la cebolla y las salsas en zigzag, todo con su
/// volumen.
fn draw_bun(c: &mut Canvas, lay: &Layout, shuco: &Shuco) {
    let s = lay.s;
    let (bx, by, _, _) = lay.board();
    let half = 112.0 * s;
    c.line(bx - half, by + 30.0 * s, bx + half, by + 30.0 * s, 16.0 * s, Vec3::ZERO).alpha(0.35).soft(8.0 * s);
    c.line(bx - half, by - 18.0 * s, bx + half, by - 18.0 * s, 24.0 * s, Vec3::new(0.84, 0.56, 0.26)).vol(20.0 * s).gloss(0.2);
    c.line(bx - half, by + 10.0 * s, bx + half, by + 10.0 * s, 28.0 * s, Vec3::new(0.9, 0.63, 0.3)).vol(24.0 * s).gloss(0.2);
    c.line(bx - half * 0.9, by - 2.0 * s, bx + half * 0.9, by - 2.0 * s, 10.0 * s, Vec3::new(0.97, 0.88, 0.68)).vol(6.0 * s);
    let n = shuco.meats.len().max(1) as f32;
    for (i, &(meat, t)) in shuco.meats.iter().enumerate() {
        let off = if shuco.meats.len() > 1 { (i as f32 / (n - 1.0) - 0.5) * 10.0 * s } else { 0.0 };
        draw_meat(c, meat, bx, by - 6.0 * s + off, half * 0.85, t, s, None);
    }
    let layer = |k: usize| shuco.tops[k];
    let g = layer(Top::Guacamol.index());
    if g > 0 {
        let col = Top::Guacamol.color();
        c.line(bx - half * 0.8, by - 24.0 * s, bx + half * 0.8, by - 24.0 * s, (6.0 + 3.5 * g as f32) * s, col).vol((5.0 + 3.0 * g as f32) * s).gloss(0.35);
        for k in 0..(6 * g as u32) {
            let hk = hash_u32(k * 911 + 5);
            c.disc(bx + (unit(hk) - 0.5) * 1.5 * half, by - 26.0 * s + (unit(hk >> 8) - 0.5) * 6.0 * s, (3.0 + 2.0 * unit(hk >> 4)) * s, col * 1.1).vol(3.0 * s).gloss(0.4);
        }
    }
    for j in 0..(layer(Top::Repollo.index()) as u32 * 24) {
        let hj = hash_u32(j.wrapping_mul(2_654_435_761) ^ 0x5E);
        let x = bx + (unit(hj) - 0.5) * 1.8 * half;
        let y = by - 33.0 * s + (unit(hj >> 8) - 0.5) * 16.0 * s;
        let a = (unit(hj >> 16) - 0.5) * 1.6;
        c.line(x - a.cos() * 9.0 * s, y - a.sin() * 9.0 * s, x + a.cos() * 9.0 * s, y + a.sin() * 9.0 * s, 2.6 * s, Vec3::new(0.86, 0.94, 0.74)).vol(2.0 * s);
    }
    for j in 0..(layer(Top::Cebolla.index()) as u32 * 14) {
        let hj = hash_u32(j * 40_503 ^ 0xCEB0);
        let (x, y) = (bx + (unit(hj) - 0.5) * 1.7 * half, by - 34.0 * s + (unit(hj >> 8) - 0.5) * 10.0 * s);
        c.rect(x, y, 4.0 * s, 4.0 * s, 1.2 * s, Vec3::new(0.97, 0.95, 1.0)).vol(2.5 * s).gloss(0.3);
    }
    let mut lane = 0;
    for t in [Top::Mostaza, Top::Mayonesa, Top::Tomate] {
        for _ in 0..layer(t.index()) {
            let y = by - 30.0 * s - lane as f32 * 5.0 * s;
            let zig = 10;
            for k in 0..zig {
                let (x0, x1) = (bx - half * 0.85 + 1.7 * half * k as f32 / zig as f32, bx - half * 0.85 + 1.7 * half * (k + 1) as f32 / zig as f32);
                let (y0, y1) = if k % 2 == 0 { (y - 5.0 * s, y + 5.0 * s) } else { (y + 5.0 * s, y - 5.0 * s) };
                c.line(x0, y0, x1, y1, 2.4 * s, t.color()).vol(2.2 * s).gloss(0.7);
            }
            lane += 1;
        }
    }
}

/// Un carro de la avenida, desenfocado (con las orillas suaves): la
/// carroceria, la cabina con sus vidrios, las llantas, los faros con la luz
/// que echan en el asfalto y las calaveras rojas. Las motos con su piloto.
fn car(c: &mut Canvas, lay: &Layout, car: &Car) {
    let (ground, ppm) = lay.lane(car.lane);
    let m = |v: f32| v * ppm;
    let (x, dir) = (car.x, car.dir);
    let soft = 2.2 * lay.s;
    let body = car.color * 0.55 + SODIUM * 0.05;
    let tire = Vec3::new(0.03, 0.03, 0.035);
    let head = Vec3::new(1.0, 0.95, 0.8);
    let tail = Vec3::new(1.0, 0.08, 0.05);
    match car.kind {
        Ride::Moto => {
            for k in [-1.0f32, 1.0] {
                c.disc(x + k * m(0.6), ground - m(0.3), m(0.3), tire).soft(soft);
            }
            c.line(x - m(0.5), ground - m(0.6), x + m(0.5), ground - m(0.7), m(0.18), body).soft(soft);
            c.rect(x - dir * m(0.1), ground - m(1.15), m(0.22), m(0.35), m(0.12), Vec3::new(0.06, 0.06, 0.08)).soft(soft);
            c.disc(x, ground - m(1.62), m(0.16), car.color * 0.7).soft(soft);
            c.disc(x + dir * m(0.68), ground - m(0.8), m(0.09), head).emit(1.0).glow(3.0 * lay.s, 0.6);
            c.ellipse(x + dir * m(2.2), ground - m(0.04), m(1.3), m(0.1), head).alpha(0.25).soft(soft * 2.0).emit(0.3);
            c.disc(x - dir * m(0.75), ground - m(0.75), m(0.06), tail).emit(0.8).glow(2.5 * lay.s, 0.5);
        }
        _ => {
            let (len, cab) = (4.4f32, if car.kind == Ride::Pickup { 0.9f32 } else { -0.2 });
            let half = 0.5 * len;
            c.rect(x, ground - m(0.65), m(half), m(0.36), m(0.25), body).soft(soft);
            let (c0, c1) = (x + dir * m(cab) - m(1.1), x + dir * m(cab) + m(1.1));
            c.poly(&[(c0, ground - m(0.98)), (c1, ground - m(0.98)), (c1 - m(0.35), ground - m(1.45)), (c0 + m(0.35), ground - m(1.45))], body * 0.9).soft(soft);
            c.poly(&[(c0 + m(0.15), ground - m(1.02)), (c1 - m(0.15), ground - m(1.02)), (c1 - m(0.42), ground - m(1.38)), (c0 + m(0.42), ground - m(1.38))], Vec3::new(0.06, 0.08, 0.12)).soft(soft);
            c.line(c0 + m(0.4), ground - m(1.3), c0 + m(0.9), ground - m(1.1), m(0.05), SODIUM * 0.6).alpha(0.5).soft(soft);
            for k in [-1.0f32, 1.0] {
                c.disc(x + k * m(1.35), ground - m(0.33), m(0.33), tire).soft(soft);
                c.disc(x + k * m(1.35), ground - m(0.33), m(0.14), Vec3::splat(0.25)).soft(soft);
            }
            c.disc(x + dir * m(half - 0.12), ground - m(0.72), m(0.11), head).emit(1.0).glow(4.0 * lay.s, 0.6);
            c.ellipse(x + dir * m(half + 1.8), ground - m(0.04), m(1.9), m(0.12), head).alpha(0.3).soft(soft * 2.0).emit(0.3);
            c.rect(x - dir * m(half - 0.06), ground - m(0.75), m(0.06), m(0.11), m(0.03), tail).emit(0.8).glow(3.0 * lay.s, 0.5);
        }
    }
}

/// Pega en la fila `yf` una nave de lado (La Mamaneta o el Tuk-Tuk Elena)
/// con su ancla en `at` (el suelo bajo su centro), `scale` veces su tamano
/// horneado, al reves si va para la izquierda, con una de las tres pinturas
/// y la luz de sus LEDs encima (que emite).
#[allow(clippy::too_many_arguments)]
fn ride_row(sprite: &Sprite, at: (f32, f32), scale: f32, flip: bool, paint: u8, lut: &Lut, yf: f32, row: &mut [u8], width: usize) {
    if scale <= 0.0 {
        return;
    }
    let inv = 1.0 / scale;
    let v = sprite.anchor.1 + (yf - at.1) * inv;
    if v < 0.0 || v >= sprite.height as f32 {
        return;
    }
    let (left, right) = (sprite.anchor.0 * scale, (sprite.width as f32 - sprite.anchor.0) * scale);
    let (l, r) = if flip { (right, left) } else { (left, right) };
    let x0 = ((at.0 - l).floor().max(0.0) as usize).min(width);
    let x1 = ((at.0 + r).ceil().max(0.0) as usize).min(width);
    let pick = sprite.pick(0.0);
    for x in x0..x1 {
        let dx = (x as f32 + 0.5 - at.0) * inv;
        let u = sprite.anchor.0 + if flip { -dx } else { dx };
        let t = sprite.sample_pick(&pick, u, v, lut);
        let a = t.body[3];
        let light = Vec3::new(t.light[0], t.light[1], t.light[2]) * 0.5;
        if a <= 0.002 && light.max_element() <= 0.002 {
            continue;
        }
        let b = match paint {
            1 => Vec3::new(t.body[1], t.body[2], t.body[0]),
            2 => Vec3::new(t.body[2], t.body[0], t.body[1]),
            _ => Vec3::new(t.body[0], t.body[1], t.body[2]),
        };
        let p = &mut row[x * 4..x * 4 + 4];
        let c = lienzo::read(p) * (1.0 - a) + b * 0.85 + light;
        let e = lienzo::emission(p) * (1.0 - a) + light.max_element().min(1.0) * 0.6;
        lienzo::write_emit(p, c, e.min(1.0));
    }
}

/// Pega en la fila `yf` un cuadro de Meyer con los pies en `at`, al reves si
/// camina para la izquierda, con la luz calida de los focos (`tint`).
#[allow(clippy::too_many_arguments)]
fn meyer_row(sprite: &Sprite, at: (f32, f32), flip: bool, tint: Vec3, yf: f32, row: &mut [u8], width: usize) {
    let v = sprite.anchor.1 + (yf - at.1);
    if v < 0.0 || v >= sprite.height as f32 {
        return;
    }
    let (left, right) = (sprite.anchor.0, sprite.width as f32 - sprite.anchor.0);
    let (l, r) = if flip { (right, left) } else { (left, right) };
    let x0 = ((at.0 - l).floor().max(0.0) as usize).min(width);
    let x1 = ((at.0 + r).ceil().max(0.0) as usize).min(width);
    for x in x0..x1 {
        let dx = x as f32 + 0.5 - at.0;
        let u = sprite.anchor.0 + if flip { -dx } else { dx };
        let s = sprite.sample(u, v);
        if s[3] <= 0.002 {
            continue;
        }
        let p = &mut row[x * 4..x * 4 + 4];
        let c = lienzo::read(p) * (1.0 - s[3]) + Vec3::new(s[0], s[1], s[2]) * tint;
        let e = lienzo::emission(p) * (1.0 - s[3]);
        lienzo::write_emit(p, c, e);
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
    // Lo de ver.
    post: Posproceso,
    meyer: Option<Arc<Vec<Sprite>>>,
    side: Option<Arc<flota::Side>>,
    cars: Vec<Car>,
    car_in: f32,
    traffic_rng: u32,
}

impl Shucos {
    pub fn new(width: usize, height: usize, records: Records) -> Self {
        let lay = Layout::new(width, height);
        let savings = records.get(SAVINGS).unwrap_or(0);
        let upgrades = records.get(UPGRADES).unwrap_or(0) as u8;
        let best = records.get(RECORD);
        // Meyer y las naves de lado (horneadas a la mitad: al ampliarlas
        // quedan suaves, como lo demas de lejos).
        let meyer = personaje::customer(0.03, MEYER_PPM * lay.s);
        let side = flota::side(0.5 * lay.lane(1).1);
        let mut game = Self {
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
            horn_in: 6.0,
            post: Posproceso::new(width, height),
            meyer,
            side,
            cars: Vec::new(),
            car_in: 1.0,
            traffic_rng: 0x7A3F_1C2D,
        };
        for k in 0..3 {
            let x = (0.2 + 0.3 * k as f32) * lay.w;
            game.spawn_car(Ride::Sedan, k % 2, Some(x));
        }
        game
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
        // Don Meyer viene cada noche a eso de las 8 y cuarto, y a veces
        // vuelve (pero no hay dos).
        let can_meyer = self.meyer.is_some() && !self.night.customers.iter().any(|c| c.who == Who::Meyer);
        let who = if can_meyer && !self.night.meyer_came && h >= 20.25 {
            Who::Meyer
        } else {
            let mut weights = [3.0f32, 2.0, 2.0, 0.0, 1.0, if can_meyer { 0.35 } else { 0.0 }];
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
                    who = [Who::Estudiante, Who::Agente, Who::Senora, Who::Bolo, Who::Aficionado, Who::Meyer][i];
                    break;
                }
                pick -= wgt;
            }
            who
        };
        if who == Who::Meyer {
            self.night.meyer_came = true;
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
            Who::Meyer => Vec3::new(0.66, 0.6, 0.45),
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
            Who::Meyer => {
                if r < 0.5 {
                    Order::new(&[Meat::Longaniza, Meat::Chorizo], true, &[(Top::Guacamol, 2)], &[], "con todo y bastante guacamol")
                } else {
                    Order::new(&[Meat::Salchicha], true, &[(Top::Mostaza, 2)], &[Top::Cebolla], "sin cebolla y doble mostaza")
                }
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

    /// La guia de abajo: el siguiente paso para el cliente elegido, en
    /// palabras (lo mismo que haria el piloto automatico, pero diciendolo).
    fn guide(&self) -> Option<String> {
        let night = &self.night;
        let c = night.customers.iter().find(|c| c.slot == night.selected && c.waiting())?;
        let order = &c.order;
        let who = c.who.name();
        let article = |m: Meat| if m.feminine() { "la" } else { "el" };
        if night.plancha.iter().enumerate().any(|(i, t)| t.is_some_and(|t| doneness(Meat::ALL[i], t) == Done::Burnt)) {
            return Some("¡Se quemó algo en la plancha! X para tirarlo".to_string());
        }
        let Some(bun) = &night.bun else {
            return Some(format!("Para {who}: apachá 1 para poner el pan en la tabla"));
        };
        let mut have = [0u8; 4];
        for (m, t) in &bun.meats {
            have[m.index()] += 1;
            if doneness(*m, *t) != Done::Ready {
                return Some("Ese shuco tiene carne cruda o quemada: X para tirarlo y empezar otra vez".to_string());
            }
        }
        if (0..4).any(|i| have[i] > order.meats[i]) {
            return Some("Le pusiste carne de más: X para tirarlo y empezar otra vez".to_string());
        }
        for m in Meat::ALL {
            let i = m.index();
            if have[i] < order.meats[i] {
                let k = m.key();
                return Some(match night.plancha[i] {
                    None => format!("Apachá {k} para poner {} {} en la plancha", article(m), m.name()),
                    Some(t) if doneness(m, t) == Done::Raw => format!("Esperá a que la barrita de {} {} se ponga verde…", article(m), m.name()),
                    Some(_) => format!("¡Lista! Apachá {k} otra vez para pasar {} {} al pan", article(m), m.name()),
                });
            }
        }
        let missing: Vec<String> = Top::ALL
            .iter()
            .filter(|t| bun.tops[t.index()] < order.want[t.index()])
            .map(|t| {
                let n = order.want[t.index()] - bun.tops[t.index()];
                let times = if n > 1 { format!(" ({n} veces)") } else { String::new() };
                format!("{} {}{times}", t.key().to_ascii_uppercase(), t.name())
            })
            .collect();
        if !missing.is_empty() {
            return Some(format!("Ahora lo de encima: {}", missing.join(", ")));
        }
        Some(format!("¡Ya está! Enter para dárselo a {} (← → para elegir a otro)", who.to_lowercase()))
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

    fn traffic_random(&mut self) -> f32 {
        self.traffic_rng ^= self.traffic_rng << 13;
        self.traffic_rng ^= self.traffic_rng >> 17;
        self.traffic_rng ^= self.traffic_rng << 5;
        unit(self.traffic_rng)
    }

    /// Un carro que entra por un lado (o en `x`): el carril de alla va para
    /// la izquierda y el de aca para la derecha.
    fn spawn_car(&mut self, kind: Ride, lane: usize, x: Option<f32>) {
        let dir = if lane == 0 { -1.0 } else { 1.0 };
        let w = self.lay.w;
        let x = x.unwrap_or(if dir > 0.0 { -0.3 * w } else { 1.3 * w });
        let speed = match kind {
            Ride::Bus => 9.0,
            Ride::Tuk => 7.0,
            Ride::Moto => 14.0,
            _ => 9.0 + 6.0 * self.traffic_random(),
        };
        let palette = [Vec3::new(0.85, 0.85, 0.85), Vec3::new(0.55, 0.57, 0.6), Vec3::new(0.7, 0.08, 0.08), Vec3::new(0.08, 0.08, 0.1), Vec3::new(0.12, 0.22, 0.55), Vec3::new(0.15, 0.4, 0.25)];
        let color = palette[(self.traffic_random() * palette.len() as f32) as usize % palette.len()];
        let paint = (self.traffic_random() * 3.0) as u8 % 3;
        self.cars.push(Car { x, dir, speed, lane, kind, color, paint });
    }

    /// La avenida: los carros avanzan, salen otros y, cada tanto, pasa una
    /// camioneta pitando.
    fn traffic(&mut self, dt: f32) {
        let lay = self.lay;
        for c in &mut self.cars {
            c.x += c.dir * c.speed * lay.lane(c.lane).1 * dt;
        }
        self.cars.retain(|c| c.x > -0.35 * lay.w && c.x < 1.35 * lay.w);
        self.car_in -= dt;
        if self.car_in <= 0.0 {
            self.car_in = 1.2 + 2.8 * self.traffic_random();
            let lane = if self.traffic_random() < 0.5 { 0 } else { 1 };
            let r = self.traffic_random();
            let kind = if r < 0.12 {
                Ride::Moto
            } else if r < 0.3 {
                Ride::Pickup
            } else if r < 0.42 && lane == 1 && self.side.is_some() {
                Ride::Tuk
            } else {
                Ride::Sedan
            };
            self.spawn_car(kind, lane, None);
        }
        self.horn_in -= dt;
        if self.horn_in <= 0.0 {
            self.horn_in = 12.0 + 10.0 * self.traffic_random();
            if self.side.is_some() {
                self.spawn_car(Ride::Bus, 1, None);
            }
            self.pending.push(Sfx::Horn);
        }
    }

    /// Los resultados de la noche (para las pruebas).
    pub fn tally(&self) -> (u32, u32, u32, i64) {
        (self.night.sold, self.night.returned, self.night.angry, self.night.money)
    }

    /// Si don Meyer esta esperando su shuco (para las fotos).
    pub fn meyer_waiting(&self) -> bool {
        self.night.customers.iter().any(|c| c.who == Who::Meyer && c.waiting())
    }

    pub fn in_results(&self) -> bool {
        self.screen == Screen::Results
    }

    /// Donde va un cliente y como: x, cuanto rebota, su cara, si camina y
    /// si va para la derecha.
    fn placement(&self, cu: &Customer) -> (f32, f32, Face, bool, bool) {
        let lay = &self.lay;
        let s = lay.s;
        let ease = |t: f32| smoothstep(0.0, 1.0, t);
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
        let right = match cu.mood {
            Mood::Arriving(_) => cu.left,
            _ => !cu.left,
        };
        (lay.slot_x(cu.slot) + dir * shift + sway, bob, face, walking, right)
    }

    /// Lo que se mueve detras del mostrador: los clientes (Meyer va aparte,
    /// es un sprite) y sus globitos con su sombra.
    fn behind(&self) -> Canvas {
        let lay = &self.lay;
        let s = lay.s;
        let mut c = Canvas::default();
        for cu in &self.night.customers {
            if cu.who == Who::Meyer && self.meyer.is_some() {
                continue;
            }
            let (x, bob, face, _, _) = self.placement(cu);
            person(&mut c, lay, cu.who, &cu.look, x, bob, face);
        }
        for cu in self.night.customers.iter().filter(|c| c.waiting()) {
            let (x, y) = (lay.slot_x(cu.slot), lay.bubble_y(cu.slot));
            let (hx, hy) = (134.0 * s, 47.0 * s);
            let paper = Vec3::new(0.97, 0.95, 0.9);
            let tip = (x - 10.0 * s, lay.head_y() - 52.0 * s);
            c.rect(x + 5.0 * s, y + 7.0 * s, hx, hy, 15.0 * s, Vec3::ZERO).alpha(0.35).soft(10.0 * s);
            if cu.slot == self.night.selected {
                c.rect(x, y, hx + 4.0 * s, hy + 4.0 * s, 18.0 * s, Vec3::new(1.0, 0.72, 0.25)).glow(10.0 * s, 0.5).emit(0.35);
                c.line(x - 18.0 * s, y + hy, tip.0, tip.1, 9.0 * s, Vec3::new(1.0, 0.72, 0.25)).emit(0.35);
            }
            c.line(x - 18.0 * s, y + hy - 4.0 * s, tip.0, tip.1, 6.0 * s, paper);
            c.rect(x, y, hx, hy, 15.0 * s, paper);
        }
        c
    }

    /// Los cuadros de Meyer que van en este frame: cual, x, donde van los
    /// pies, si va al reves y su luz.
    fn meyer_blits(&self) -> Vec<(usize, f32, f32, bool, Vec3)> {
        if self.meyer.is_none() {
            return Vec::new();
        }
        let lay = &self.lay;
        let feet = lay.head_y() - 42.0 * lay.s + personaje::HEIGHT * MEYER_PPM * lay.s;
        self.night
            .customers
            .iter()
            .filter(|c| c.who == Who::Meyer)
            .map(|cu| {
                let (x, bob, face, walking, right) = self.placement(cu);
                let frame = if walking { 1 + (self.wall * 22.0) as usize % WALK_FRAMES } else { 0 };
                let tint = if face == Face::Angry { Vec3::new(1.4, 0.95, 0.85) } else { Vec3::new(1.22, 1.07, 0.92) };
                (frame, x, feet + 0.4 * bob, walking && !right, tint)
            })
            .collect()
    }

    /// La serie del toldo: un foquito de color en cada onda, que se prenden
    /// en carrerita como los de diciembre.
    fn deco(&self) -> Canvas {
        let lay = &self.lay;
        let s = lay.s;
        let mut c = Canvas::default();
        let stripe = 42.0 * s;
        let n = (lay.w / stripe) as usize + 1;
        let colors = [Vec3::new(1.0, 0.2, 0.15), Vec3::new(0.2, 1.0, 0.35), Vec3::new(0.25, 0.45, 1.0), Vec3::new(1.0, 0.75, 0.15), Vec3::new(1.0, 0.3, 0.85)];
        for i in 0..n {
            let x = (i as f32 + 0.5) * stripe;
            let y = lay.awning + 18.0 * s;
            let on = 0.35 + 0.65 * smoothstep(0.3, 0.9, (self.wall * 2.2 - i as f32 * 0.35).sin() * 0.5 + 0.5);
            let col = colors[i % colors.len()];
            c.rect(x, y - 4.0 * s, 1.6 * s, 2.0 * s, 0.5 * s, Vec3::splat(0.1));
            c.ellipse(x, y, 2.6 * s, 3.6 * s, col * (0.45 + 0.55 * on) + Vec3::splat(0.25 * on)).emit(on).glow(4.0 * s, 0.45 * on);
        }
        c
    }

    /// La avenida: los carros de cada carril (figuras) y las naves de lado.
    fn street(&self) -> ([Canvas; 2], Vec<RideBlit>) {
        let lay = &self.lay;
        let mut lanes = [Canvas::default(), Canvas::default()];
        let mut rides = Vec::new();
        for cr in &self.cars {
            match cr.kind {
                Ride::Bus | Ride::Tuk => {
                    if let Some(side) = &self.side {
                        let bus = cr.kind == Ride::Bus;
                        let sprite = if bus { &side.bus } else { &side.tuk };
                        let (ground, ppm) = lay.lane(cr.lane);
                        rides.push(RideBlit { bus, at: (cr.x, ground), scale: ppm / sprite.scale.max(1e-6), flip: cr.dir < 0.0, paint: cr.paint, lane: cr.lane });
                    }
                }
                _ => car(&mut lanes[cr.lane.min(1)], lay, cr),
            }
        }
        (lanes, rides)
    }

    /// Lo que se mueve sobre el mostrador: el calor de la plancha, las
    /// carnes con su barrita, el shuco y el bote de las propinas, que se
    /// llena con las monedas de la noche.
    fn on_counter(&self) -> Canvas {
        let lay = &self.lay;
        let s = lay.s;
        let m = |v: f32| v * s;
        let mut c = Canvas::default();
        let (px, py, phx, phy) = lay.plancha();
        let cooking = self.night.plancha.iter().filter(|t| t.is_some()).count();
        if cooking > 0 {
            c.rect(px, py, phx - m(6.0), phy - m(6.0), m(8.0), Vec3::new(1.0, 0.32, 0.06)).alpha(0.05 + 0.02 * cooking as f32).soft(m(14.0)).emit(0.1 + 0.04 * cooking as f32);
        }
        for (i, slot) in self.night.plancha.iter().enumerate() {
            let (mx, my) = lay.meat_at(i);
            let Some(t) = *slot else {
                c.line(mx - m(50.0), my, mx + m(50.0), my, m(12.0), Meat::ALL[i].raw()).alpha(0.08);
                continue;
            };
            let meat = Meat::ALL[i];
            draw_meat(&mut c, meat, mx, my - m(4.0), m(52.0), t, s, Some(self.wall));
            let (ready, burn) = meat.times();
            let bar_y = my + m(28.0);
            let len = m(50.0);
            c.rect(mx, bar_y, len, m(3.0), m(2.0), Vec3::splat(0.05)).alpha(0.8);
            let f = (t / burn).min(1.0);
            let col = match doneness(meat, t) {
                Done::Raw => Vec3::new(0.95, 0.8, 0.25),
                Done::Ready => Vec3::new(0.35, 0.9, 0.35).lerp(Vec3::new(1.0, 0.55, 0.1), smoothstep(0.6, 1.0, (t - ready) / (burn - ready))),
                Done::Burnt => Vec3::new(0.95, 0.15, 0.1),
            };
            let fill = len * f;
            c.rect(mx - len + fill, bar_y, fill.max(1.0), m(3.0), m(2.0), col).glow(m(3.0), 0.3).emit(0.4);
            c.rect(mx - len + 2.0 * len * ready / burn, bar_y, m(1.0), m(5.0), m(0.5), Vec3::splat(0.95)).alpha(0.7);
        }
        if let Some(bun) = &self.night.bun {
            draw_bun(&mut c, lay, bun);
        }
        let (jx, jy) = lay.jar();
        c.ellipse(jx + m(4.0), jy + m(32.0), m(26.0), m(7.0), Vec3::ZERO).alpha(0.45).soft(m(6.0));
        let gold = Vec3::new(0.95, 0.74, 0.25);
        let n = (self.night.tips.max(0) as usize).min(42);
        for k in 0..n {
            let hk = hash_u32(k as u32 * 977 + 13);
            let (col, row) = (k % 6, k / 6);
            let cx = jx - m(14.0) + col as f32 * m(5.6) + (unit(hk) - 0.5) * m(2.0);
            let cy = jy + m(27.0) - row as f32 * m(4.0);
            c.ellipse(cx, cy, m(5.0), m(2.4), gold * (0.8 + 0.3 * unit(hk >> 5))).vol(m(2.0)).gloss(0.8);
        }
        c.rect(jx, jy, m(22.0), m(32.0), m(8.0), Vec3::new(0.7, 0.82, 0.88)).alpha(0.2).vol(m(10.0)).gloss(0.9);
        c.ellipse(jx, jy - m(31.0), m(22.0), m(5.0), Vec3::new(0.8, 0.9, 0.95)).alpha(0.3).vol(m(3.0)).gloss(0.9);
        c.line(jx - m(14.0), jy - m(22.0), jx - m(14.0), jy + m(22.0), m(2.0), Vec3::ONE).alpha(0.35);
        c.rect(jx, jy - m(6.0), m(16.0), m(6.0), m(1.5), Vec3::new(0.95, 0.9, 0.7)).alpha(0.9);
        c
    }

    /// El vapor (que alumbran los focos), el humo, el enojo y las monedas.
    fn air(&self) -> Canvas {
        let s = self.lay.s;
        let mut c = Canvas::default();
        for p in &self.night.particles {
            let f = p.age / p.life;
            match p.kind {
                Puff::Steam => {
                    c.disc(p.x, p.y, (8.0 + 22.0 * f) * s, Vec3::new(0.95, 0.9, 0.84)).alpha(0.2 * (1.0 - f) * smoothstep(0.0, 0.15, f)).soft(10.0 * s);
                }
                Puff::Smoke => {
                    c.disc(p.x, p.y, (10.0 + 30.0 * f) * s, Vec3::new(0.12, 0.11, 0.11)).alpha(0.55 * (1.0 - f) * smoothstep(0.0, 0.1, f)).soft(12.0 * s);
                }
                Puff::Anger => {
                    c.disc(p.x, p.y, (5.0 + 10.0 * f) * s, Vec3::new(0.95, 0.95, 0.95)).alpha(0.5 * (1.0 - f)).soft(5.0 * s);
                }
                Puff::Coin => {
                    c.disc(p.x, p.y, 7.0 * s, Vec3::new(0.98, 0.78, 0.25)).alpha(1.0 - f * f).vol(6.0 * s).gloss(0.9).emit(0.3).glow(5.0 * s, 0.4);
                }
            }
        }
        c
    }
}

impl Minigame for Shucos {
    fn demo_start(&mut self) {
        self.open();
        self.set_autopilot(true);
    }

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
        self.traffic(dt);
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
    }

    fn draw(&mut self, rgba: &mut [u8]) {
        let lay = self.lay;
        let width = lay.width;
        let deco = self.deco();
        let (lanes, rides) = self.street();
        let meyers = self.meyer_blits();
        let behind = self.behind();
        let front = self.on_counter();
        let air = self.air();
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
        let (people, counter_l) = (people_light(), counter_light());
        let deco_end = lay.awning + 30.0 * lay.s;
        let street_top = lay.street() - 0.2 * lay.h;
        let counter = lay.counter;
        let mut sums = self.post.take_sums();
        let this = &*self;
        sums.paint(rgba, |y, row| {
            let src = &this.scene[y * width * 4..(y + 1) * width * 4];
            if src.len() == row.len() {
                row.copy_from_slice(src);
            }
            let yf = y as f32 + 0.5;
            if yf < counter {
                if yf < deco_end {
                    paint(&deco.shapes, yf, row, width, None);
                }
                if yf > street_top {
                    for (lane, canvas) in lanes.iter().enumerate() {
                        paint(&canvas.shapes, yf, row, width, None);
                        if let Some(side) = &this.side {
                            for r in rides.iter().filter(|r| r.lane == lane) {
                                let sprite = if r.bus { &side.bus } else { &side.tuk };
                                ride_row(sprite, r.at, r.scale, r.flip, r.paint, &lut, yf, row, width);
                            }
                        }
                    }
                }
                if let Some(sprites) = &this.meyer {
                    for &(frame, x, feet, flip, tint) in &meyers {
                        if let Some(sprite) = sprites.get(frame) {
                            meyer_row(sprite, (x, feet), flip, tint, yf, row, width);
                        }
                    }
                }
                paint_lit(&behind.shapes, yf, row, width, None, &people);
            } else {
                paint_lit(&front.shapes, yf, row, width, None, &counter_l);
            }
            paint_lit(&air.shapes, yf, row, width, None, &counter_l);
        });
        // El brillo de lo que emite (los focos, la serie, las ventanas, los
        // faros, la plancha), la vineta, la gradacion y el grano.
        let params = posproceso::Params { wall: self.wall, fade: if self.screen == Screen::Playing { 0.0 } else { 0.45 }, ..Default::default() };
        self.post.prepare(sums, &params);
        self.post.compose(rgba, &params, |_, _| {});
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
                o.text("Arriba del mostrador te va diciendo qué apachar en cada paso", w * 0.5, py + 100.0 * s, Font::Small, accent, Align::Center);
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
                o.text("X · tirar     ← → · cliente     Enter · entregar", tx, ty + thy + 24.0 * s, Font::Small, cream, Align::Center);
                // El siguiente paso, en grande, justo arriba del mostrador.
                if self.screen == Screen::Playing
                    && let Some(step) = self.guide()
                {
                    let gw = 720.0 * s;
                    let gx = 0.44 * w - 0.5 * gw;
                    o.panel(gx, lay.counter + 4.0 * s, gw, 34.0 * s);
                    o.text(step, 0.44 * w, lay.counter + 9.0 * s, Font::Bold, accent, Align::Center);
                }
                let (jx, jy) = lay.jar();
                o.text(format!("Propinas Q{}", self.night.tips), jx, jy + 42.0 * s, Font::Small, dim, Align::Center);
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

    /// Don Meyer llega cada noche (despues de las 8 y cuarto), se dibuja y
    /// se le puede servir.
    #[test]
    fn don_meyer_comes_for_his_shuco() {
        let (w, h) = (320, 180);
        let mut g = Shucos::new(w, h, Records::in_memory());
        assert!(g.meyer.is_some(), "sin el modelo de Meyer");
        g.open();
        g.set_autopilot(true);
        let mut seen = false;
        let mut rgba = vec![0u8; w * h * 4];
        let mut t = 0.0;
        while !g.in_results() && t < NIGHT + LAST_CALL + 10.0 {
            g.update(1.0 / 30.0, &Keys::default());
            t += 1.0 / 30.0;
            if !seen && g.night.customers.iter().any(|c| c.who == Who::Meyer && c.waiting()) {
                seen = true;
                g.draw(&mut rgba);
                assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
                assert!(Shucos::hour(g.night.clock) >= 20.25);
            }
        }
        assert!(seen && g.night.meyer_came, "Meyer no vino");
    }

    /// La avenida tiene trafico y de vez en cuando pasa una camioneta.
    #[test]
    fn the_avenue_has_traffic_and_buses() {
        let mut g = Shucos::new(320, 180, Records::in_memory());
        let mut bus = false;
        for _ in 0..(30 * 30) {
            g.update(1.0 / 30.0, &Keys::default());
            bus |= g.cars.iter().any(|c| c.kind == Ride::Bus);
            assert!(g.cars.len() < 40);
        }
        assert!(!g.cars.is_empty());
        assert!(bus || g.side.is_none());
    }

    /// Cuanto cuesta cada parte del frame (`cargo test --release
    /// bench_shucos -- --ignored --nocapture`).
    #[test]
    #[ignore = "mide tiempos"]
    fn bench_shucos() {
        let (w, h) = (1280, 720);
        let mut g = Shucos::new(w, h, Records::in_memory());
        g.open();
        g.set_autopilot(true);
        for _ in 0..(60 * 50) {
            g.update(1.0 / 60.0, &Keys::default());
        }
        let mut rgba = vec![0u8; w * h * 4];
        let time = |g: &mut Shucos, rgba: &mut Vec<u8>| {
            let mut t: Vec<f64> = (0..15)
                .map(|_| {
                    let start = std::time::Instant::now();
                    g.draw(rgba);
                    start.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            t.sort_by(f64::total_cmp);
            t[7]
        };
        println!("todo          {:6.2} ms ({} clientes, {} carros)", time(&mut g, &mut rgba), g.night.customers.len(), g.cars.len());
        let start = std::time::Instant::now();
        for _ in 0..15 {
            let _ = (g.deco(), g.street(), g.behind(), g.on_counter(), g.air());
        }
        println!("figuras       {:6.2} ms", start.elapsed().as_secs_f64() * 1000.0 / 15.0);
        let cars = std::mem::take(&mut g.cars);
        println!("sin carros    {:6.2} ms", time(&mut g, &mut rgba));
        let people = std::mem::take(&mut g.night.customers);
        println!("sin gente     {:6.2} ms", time(&mut g, &mut rgba));
        let (bun, plancha) = (g.night.bun.take(), g.night.plancha);
        g.night.plancha = [None; 4];
        println!("sin mostrador {:6.2} ms", time(&mut g, &mut rgba));
        g.night.customers = people;
        g.cars = cars;
        g.night.bun = bun;
        g.night.plancha = plancha;
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
        for who in [Who::Estudiante, Who::Agente, Who::Senora, Who::Bolo, Who::Aficionado, Who::Meyer] {
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
