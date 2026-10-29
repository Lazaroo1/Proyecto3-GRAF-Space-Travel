//! Marimba procedural para la version chapina: un son en 6/8 sintetizado
//! muestra por muestra, sin archivos de audio.
//!
//! - **Las teclas** (sintesis modal): una tecla de madera vibra en pocos
//!   modos, y en la marimba se afinan a 1 : 4 : 10 (la fundamental, dos
//!   octavas arriba y dos octavas mas una tercera). Cada modo es un seno que
//!   se apaga: un fasor complejo que gira `w` y se encoge `r` cada muestra.
//!   Los sobretonos se apagan mas rapido que la fundamental y las teclas
//!   agudas mucho mas rapido que las graves.
//! - **El golpe**: la baqueta le suma un impulso a la velocidad del modo (la
//!   parte real del fasor); lo que suena es la parte imaginaria, que arranca
//!   en cero, asi el golpe nunca hace saltar la onda. Las baquetas del bajo
//!   son blandas (casi solo fundamental) y las del tiple duras (mas brillo),
//!   mas un "toc" corto de ruido contra la madera.
//! - **El redoble**: las notas largas de la melodia se tremolan con golpes
//!   cada ~60 ms. Cada golpe empuja del lado en que ya se mueve la tecla
//!   (el signo de la parte real), asi el redoble sostiene la nota en vez de
//!   cancelarla, y su fuerza se ajusta a lo que la tecla pierde entre golpe
//!   y golpe para que suene parejo en todo el teclado.
//! - **El charleo**: cada tecomate (resonador de calabaza) tiene una
//!   membrana que zumba cuando el aire la empuja fuerte. Se imita recortando
//!   la fundamental arriba de un umbral: salen pulsos al tono de la nota
//!   solo mientras suena fuerte, y se callan cuando se apaga, como la
//!   membrana. Esos pulsos, con algo de ruido, pasan por dos resonancias
//!   (formantes) que les dan el color nasal.
//! - **El conjunto**: el bajo en los tiempos fuertes (con su octava), el
//!   centro con el acorde y el tiple con la melodia en terceras paralelas
//!   (la segunda voz va una tercera abajo, dentro de la escala). Son chapin
//!   en 6/8 con sesquialtera: compases donde el centro va en 3/4 encima del
//!   bajo en 6/8 (tres contra dos) y cadencias con todos en 3/4 (hemiola).
//!   Sol mayor con I-IV-V7 y una melodia original de 32 compases (A A' B B')
//!   que se repite sin corte: las teclas siguen sonando al dar la vuelta.
//! - **La sala**: una reverberacion de Schroeder (cuatro peines que se
//!   oscurecen y dos pasa todo) para que suene en un lugar y no en el vacio.
//!
//! La partitura se arma una sola vez en `new`, con pequenas variaciones de
//! tiempo y fuerza sacadas de un hash (como musicos de verdad, pero siempre
//! igual), y `next` solo avanza el reloj, dispara los golpes que tocan y
//! suma las teclas que suenan: nada se reserva por muestra.

use crate::math::hash_u32;
use std::f32::consts::{PI, TAU};

/// Negras con puntillo por minuto: un compas de 6/8 tiene dos.
const BPM: f64 = 132.0;
/// Corcheas por segundo.
const EIGHTHS_PER_SECOND: f64 = BPM * 3.0 / 60.0;
/// Compases de la vuelta y su largo en corcheas.
const BARS: usize = 32;
const LOOP: f64 = BARS as f64 * 6.0;
/// Teclas que pueden sonar a la vez; si se acaban se roba la mas callada.
const VOICES: usize = 32;
/// Modos de una tecla de marimba afinada (1 : 4 : 10), un poco abiertos
/// como la madera de verdad.
const PARTIALS: [f32; 3] = [1.0, 3.98, 9.85];
/// Cuanto dura cada modo comparado con la fundamental.
const DECAY: [f32; 3] = [1.0, 0.3, 0.12];
/// Tiempo entre golpes del redoble, en corcheas (~16 golpes por segundo).
const TREMOLO: f64 = 0.4;
/// Cuanto antes de su nota cae un adorno, en corcheas (~45 ms).
const GRACE: f64 = 0.3;
/// Una tecla se suelta cuando la energia de su fundamental baja de esto.
const SILENCE: f32 = 1e-7;
/// Amplitud de la fundamental desde la que zumba la membrana.
const CHARLEO: f32 = 0.2;
/// Mezcla: teclas, charleo, toc y sala.
const BUZZ: f32 = 16.0;
const WET: f32 = 0.18;
const GAIN: f32 = 0.12;

/// Los tres acordes del son en Sol mayor: I, IV y V7.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Chord {
    Sol,
    Do,
    Re7,
}

impl Chord {
    /// Notas del bajo: fundamental, quinta y la que camina en la hemiola.
    fn bass(self) -> [u8; 3] {
        match self {
            Chord::Sol => [43, 50, 47], // Sol2 Re3 Si2
            Chord::Do => [48, 43, 52],  // Do3 Sol2 Mi3
            Chord::Re7 => [50, 45, 42], // Re3 La2 Fa#2 (sube a Sol2)
        }
    }

    /// El acorde del centro, cerrado entre Fa#3 y Mi4.
    fn centro(self) -> [u8; 3] {
        match self {
            Chord::Sol => [55, 59, 62], // Sol3 Si3 Re4
            Chord::Do => [55, 60, 64],  // Sol3 Do4 Mi4
            Chord::Re7 => [54, 57, 60], // Fa#3 La3 Do4
        }
    }
}

/// Como se reparte un compas de seis corcheas.
#[derive(Clone, Copy, Debug)]
enum Feel {
    /// 6/8: bajo en 1 y 4, centro en 2-3 y 5-6 (bum-pa-pa).
    Seis,
    /// Sesquialtera: el bajo sigue en 6/8 y el centro va en 3/4 encima.
    Sesqui,
    /// Hemiola de cadencia: todos en 3/4.
    Tres,
}

impl Feel {
    /// Golpes del bajo: (corchea, cual nota del acorde, fuerza).
    fn bass(self) -> &'static [(f64, usize, f32)] {
        match self {
            Feel::Seis | Feel::Sesqui => &[(0.0, 0, 1.3), (3.0, 1, 1.05)],
            Feel::Tres => &[(0.0, 0, 1.3), (2.0, 1, 1.1), (4.0, 2, 1.1)],
        }
    }

    /// Golpes del centro: (corchea, fuerza).
    fn centro(self) -> &'static [(f64, f32)] {
        match self {
            Feel::Seis => &[(1.0, 0.58), (2.0, 0.44), (4.0, 0.58), (5.0, 0.44)],
            Feel::Sesqui => &[(2.0, 0.62), (4.0, 0.58)],
            Feel::Tres => &[(1.0, 0.52), (3.0, 0.52), (5.0, 0.52)],
        }
    }
}

/// La armonia de cada compas: A y A' van y vienen de la tonica, B y B'
/// arrancan en la dominante.
const HARMONY: [Chord; BARS] = {
    use Chord::*;
    [
        Sol, Do, Re7, Sol, Sol, Do, Re7, Sol, // A
        Sol, Do, Re7, Sol, Sol, Do, Re7, Sol, // A'
        Re7, Re7, Sol, Sol, Do, Do, Re7, Sol, // B
        Re7, Re7, Sol, Sol, Do, Do, Re7, Sol, // B'
    ]
};

/// Como se siente cada compas de una frase de ocho: alterna 6/8 y
/// sesquialtera y cierra con la hemiola antes de volver a la tonica.
const FEEL: [Feel; 8] = {
    use Feel::*;
    [Seis, Sesqui, Seis, Sesqui, Seis, Sesqui, Tres, Seis]
};

/// La melodia del tiple, un compas por cadena: nota y octava (Do4 es el do
/// central), ':' y la duracion en corcheas, '~' si se redobla y '^' si lleva
/// adorno (la nota de arriba, muy corta, justo antes).
const MELODY: [&str; BARS] = [
    // A
    "Re5:2 Si4:1 Re5:2 Sol5:1",
    "^Mi5:3~ Sol5:1 Mi5:1 Do5:1",
    "Re5:2 Fa#5:1 La5:2 Fa#5:1",
    "Sol5:3~ Fa#5:1 Sol5:1 La5:1",
    "Si5:2 La5:1 Sol5:2 Re5:1",
    "Mi5:2 Sol5:1 Mi5:2 Do5:1",
    "Re5:2 Do5:2 La4:2",
    "Sol4:3~ La4:1 Si4:1 Do5:1",
    // A'
    "Re5:2 Si4:1 Re5:2 Sol5:1",
    "^Mi5:3~ Sol5:1 Mi5:1 Do5:1",
    "Re5:2 Fa#5:1 La5:2 Do6:1",
    "Si5:3~ La5:1 Sol5:1 Fa#5:1",
    "Sol5:2 Re5:1 Si4:2 Re5:1",
    "Do5:2 Mi5:1 Sol5:2 Si5:1",
    "Do6:2 La5:2 Fa#5:2",
    "Sol5:3~ Re5:1 Mi5:1 Fa#5:1",
    // B
    "^La5:3~ Sol5:1 Fa#5:1 Mi5:1",
    "Fa#5:2 Re5:1 Fa#5:2 La5:1",
    "Sol5:3~ Si4:1 Re5:1 Sol5:1",
    "Si5:3~ Do6:1 Si5:1 Sol5:1",
    "Mi5:2 Sol5:1 Do6:2 Sol5:1",
    "Mi5:3~ Do5:1 Re5:1 Mi5:1",
    "La5:4~ Do6:2",
    "Si5:3~ Sol5:1 Fa#5:1 Mi5:1",
    // B'
    "Re5:1 Fa#5:1 La5:1 ^Re6:3~",
    "Do6:2 La5:1 Fa#5:2 La5:1",
    "Sol5:3~ Re5:1 Sol5:1 Si5:1",
    "Re6:2 Si5:1 Sol5:2 Re5:1",
    "^Do6:3~ Si5:1 La5:1 Sol5:1",
    "Mi5:2 Sol5:1 Mi5:2 Do5:1",
    "Re5:2 Fa#5:2 La5:2",
    "Sol5:3~ La4:1 Si4:1 Do5:1",
];

/// Notas de Sol mayor por clase (Do = 0): Sol La Si Do Re Mi Fa#.
const IN_SCALE: [bool; 12] =
    [true, false, true, false, true, false, true, true, false, true, false, true];

/// Frecuencia de una tecla (temperamento igual, La4 = 440 Hz).
fn pitch(key: u8) -> f32 {
    440.0 * 2f32.powf((key as f32 - 69.0) / 12.0)
}

/// Segundos que tarda la fundamental en caer a 1/e: las teclas graves
/// suenan mucho mas que las agudas (por eso el tiple redobla).
fn ring(key: u8) -> f32 {
    (0.22 * (262.0 / pitch(key)).powf(0.75)).clamp(0.05, 0.5)
}

/// Sube (o baja, con `steps` negativo) grados de la escala de Sol mayor.
fn diatonic(key: u8, steps: i32) -> u8 {
    let mut key = key as i32;
    for _ in 0..steps.abs() {
        key += steps.signum();
        while !IN_SCALE[key.rem_euclid(12) as usize] {
            key += steps.signum();
        }
    }
    key as u8
}

/// Una nota de la melodia ya leida.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Note {
    key: u8,
    /// Duracion en corcheas.
    len: u8,
    tremolo: bool,
    grace: bool,
}

impl Note {
    /// Lee una nota como "^Fa#5:3~" (ver `MELODY`).
    fn parse(token: &str) -> Self {
        let grace = token.starts_with('^');
        let tremolo = token.ends_with('~');
        let token = token.trim_start_matches('^').trim_end_matches('~');
        let (name, len) = token.split_once(':').expect("nota sin duracion");
        let (name, octave) = name.split_at(name.len() - 1);
        let sharp = name.ends_with('#');
        let class = match name.trim_end_matches('#') {
            "Do" => 0,
            "Re" => 2,
            "Mi" => 4,
            "Fa" => 5,
            "Sol" => 7,
            "La" => 9,
            "Si" => 11,
            other => panic!("nota desconocida: {other}"),
        };
        let octave: u8 = octave.parse().expect("octava invalida");
        Self {
            key: 12 * (octave + 1) + class + sharp as u8,
            len: len.parse().expect("duracion invalida"),
            tremolo,
            grace,
        }
    }
}

/// Quien golpea: cada musico usa sus baquetas.
#[derive(Clone, Copy, Debug)]
enum Mallet {
    Bajo,
    Centro,
    Tiple,
}

impl Mallet {
    /// Cuanto excita la baqueta cada modo: las blandas del bajo casi solo la
    /// fundamental, las duras del tiple tambien los sobretonos.
    fn partials(self) -> [f32; 3] {
        match self {
            Mallet::Bajo => [1.0, 0.6, 0.06],
            Mallet::Centro => [1.0, 0.3, 0.07],
            Mallet::Tiple => [1.0, 0.42, 0.14],
        }
    }

    /// El toc de la baqueta contra la madera.
    fn knock(self) -> f32 {
        match self {
            Mallet::Bajo => 0.12,
            Mallet::Centro => 0.08,
            Mallet::Tiple => 0.15,
        }
    }
}

/// Un golpe de la partitura.
#[derive(Clone, Copy, Debug)]
struct Hit {
    /// Cuando, en corcheas desde el inicio de la vuelta.
    at: f64,
    key: u8,
    force: f32,
    mallet: Mallet,
}

/// Arma la partitura completa (bajo, centro, tiple y su segunda voz), ya
/// humanizada y ordenada en el tiempo.
fn score() -> Vec<Hit> {
    let mut hits = Vec::new();
    for (bar, (&chord, melody)) in HARMONY.iter().zip(MELODY).enumerate() {
        let start = bar as f64 * 6.0;
        let feel = FEEL[bar % 8];
        // Cada frase entra un poco mas fuerte que la anterior.
        let swell = 0.9 + 0.05 * (bar / 8) as f32;
        let mut hit = |at: f64, key: u8, force: f32, mallet: Mallet| {
            hits.push(Hit { at, key, force: force * swell, mallet });
        };

        let notes = chord.bass();
        for &(at, which, force) in feel.bass() {
            hit(start + at, notes[which], force, Mallet::Bajo);
        }
        // El bajista dobla la fundamental a la octava en el tiempo fuerte.
        hit(start, notes[0] + 12, 0.5, Mallet::Bajo);
        for &(at, force) in feel.centro() {
            for key in chord.centro() {
                hit(start + at, key, force, Mallet::Centro);
            }
        }

        let mut at = start;
        for token in melody.split_whitespace() {
            let note = Note::parse(token);
            let end = at + note.len as f64;
            let second = diatonic(note.key, -2);
            if note.grace {
                hit(at - GRACE, diatonic(note.key, 1), 0.45, Mallet::Tiple);
            }
            let accent = if at == start { 0.95 } else { 0.8 };
            hit(at, note.key, accent, Mallet::Tiple);
            hit(at, second, accent * 0.65, Mallet::Tiple);
            if note.tremolo {
                // Lo que la tecla pierde entre dos golpes es lo que cada golpe
                // repone: asi el redoble queda parejo en grave y en agudo.
                let gap = (TREMOLO / EIGHTHS_PER_SECOND) as f32;
                let refill = |key: u8| 1.6 * (1.0 - (-gap / ring(key)).exp());
                let mut beat = at + TREMOLO;
                let mut hand = 0;
                while beat < end - 0.25 {
                    // Una mano pega un poco mas fuerte que la otra.
                    let force = if hand == 0 { 0.85 } else { 0.75 };
                    hit(beat, note.key, force * refill(note.key), Mallet::Tiple);
                    hit(beat, second, 0.65 * force * refill(second), Mallet::Tiple);
                    beat += TREMOLO;
                    hand ^= 1;
                }
            }
            at = end;
        }
    }

    // Nadie toca como metronomo: cada golpe se corre hasta ~3 ms y cambia
    // hasta 6 % de fuerza, siempre igual (hash del numero de golpe).
    for (i, hit) in hits.iter_mut().enumerate() {
        let h = hash_u32(i as u32 ^ 0x6D61_7269);
        let early = ((h & 0xFFFF) as f64 / 65_535.0 - 0.5) * 0.04;
        let shade = ((h >> 16) as f32 / 65_535.0 - 0.5) * 0.12;
        hit.at = (hit.at + early).rem_euclid(LOOP);
        hit.force *= 1.0 + shade;
    }
    hits.sort_by(|a, b| a.at.total_cmp(&b.at));
    hits
}

/// Una tecla sonando: tres modos que giran y se apagan.
#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    active: bool,
    key: u8,
    /// Cada modo es un fasor (re, im): la parte imaginaria es lo que suena y
    /// la real lleva la velocidad, donde entra el golpe.
    z: [[f32; 2]; 3],
    /// Giro y encogimiento del fasor en una muestra: r (cos w, sin w).
    turn: [[f32; 2]; 3],
    /// 1 si el modo cabe debajo de Nyquist, 0 si se doblaria a otra nota.
    fits: [f32; 3],
    /// Toc de la baqueta: se apaga en unos milisegundos.
    knock: f32,
    /// Cuanto zumba la membrana de su tecomate (mas en las graves).
    charleo: f32,
}

impl Voice {
    fn new(key: u8, rate: f32) -> Self {
        let mut voice = Self {
            active: true,
            key,
            charleo: (330.0 / pitch(key)).powf(0.7).clamp(0.3, 1.5),
            ..Self::default()
        };
        voice.tune(rate);
        voice
    }

    /// Constantes de giro para `rate` muestras por segundo.
    fn tune(&mut self, rate: f32) {
        let base = pitch(self.key);
        let ring = ring(self.key);
        let modes = PARTIALS.iter().zip(DECAY);
        for ((turn, fits), (ratio, decay)) in self.turn.iter_mut().zip(&mut self.fits).zip(modes) {
            let freq = base * ratio;
            let r = (-1.0 / (ring * decay * rate)).exp();
            let w = TAU * freq / rate;
            *turn = [r * w.cos(), r * w.sin()];
            *fits = if freq < 0.45 * rate { 1.0 } else { 0.0 };
        }
    }

    /// Un golpe: impulso en la velocidad de cada modo, del lado en que ya se
    /// mueve para que redoblar sume energia en vez de cancelarla. Un golpe
    /// suave tambien es mas oscuro: cada modo mas agudo pierde un poco mas.
    fn strike(&mut self, force: f32, mallet: Mallet) {
        let mut bright = 1.0;
        for ((z, weight), fits) in self.z.iter_mut().zip(mallet.partials()).zip(self.fits) {
            z[0] += (force * weight * fits * bright).copysign(z[0]);
            bright *= force.min(1.0).sqrt();
        }
        self.knock += force * mallet.knock();
    }

    /// Avanza una muestra: (lo que suena la tecla, chasquido de su membrana).
    #[inline]
    fn sound(&mut self) -> (f32, f32) {
        let before = self.z[0][1];
        let mut out = 0.0;
        for (z, turn) in self.z.iter_mut().zip(&self.turn) {
            let [re, im] = *z;
            *z = [re * turn[0] - im * turn[1], re * turn[1] + im * turn[0]];
            out += z[1];
        }
        // La membrana choca una vez por ciclo, cuando la presion sube y
        // cruza el umbral, y solo si la tecla suena fuerte: un chasquido mas
        // fuerte cuanto mas pasa del umbral.
        let now = self.z[0][1];
        let slap = if before < CHARLEO && now >= CHARLEO {
            (self.energy().sqrt() - CHARLEO) * self.charleo
        } else {
            0.0
        };
        (out, slap)
    }

    fn energy(&self) -> f32 {
        let [re, im] = self.z[0];
        re * re + im * im
    }
}

/// Filtro de estado variable (forma TPT) usado como pasa banda: una
/// resonancia en `freq` con ancho `freq / q` y ganancia 1 en el centro.
#[derive(Clone, Copy, Debug, Default)]
struct Resonance {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    s1: f32,
    s2: f32,
}

impl Resonance {
    fn new(freq: f32, q: f32, rate: f32) -> Self {
        let g = (PI * freq / rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        Self { k, a1, a2: g * a1, a3: g * g * a1, s1: 0.0, s2: 0.0 }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let v3 = x - self.s2;
        let v1 = self.a1 * self.s1 + self.a2 * v3;
        let v2 = self.s2 + self.a2 * self.s1 + self.a3 * v3;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        v1 * self.k
    }
}

/// Linea de retardo circular.
#[derive(Debug)]
struct Delay {
    line: Vec<f32>,
    at: usize,
}

impl Delay {
    fn new(len: usize) -> Self {
        Self { line: vec![0.0; len], at: 0 }
    }

    /// Lo que entro hace `len` muestras.
    #[inline]
    fn front(&self) -> f32 {
        self.line[self.at]
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.line[self.at] = x;
        self.at = if self.at + 1 == self.line.len() { 0 } else { self.at + 1 };
    }
}

/// Reverberacion de Schroeder: cuatro peines en paralelo (ecos que se
/// repiten y se oscurecen con un pasa bajos) y dos pasa todo en serie que
/// los difuminan. Los largos son primos entre si para que no se alineen.
#[derive(Debug)]
struct Room {
    combs: [Delay; 4],
    dark: [f32; 4],
    diffuse: [Delay; 2],
}

impl Room {
    fn new() -> Self {
        Self {
            combs: [1116, 1188, 1277, 1356].map(Delay::new),
            dark: [0.0; 4],
            diffuse: [556, 441].map(Delay::new),
        }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let mut wet = 0.0;
        for (comb, dark) in self.combs.iter_mut().zip(&mut self.dark) {
            let echo = comb.front();
            *dark += (echo - *dark) * 0.7;
            comb.push(x + *dark * 0.8);
            wet += echo;
        }
        for diffuse in &mut self.diffuse {
            let echo = diffuse.front();
            diffuse.push(wet + echo * 0.5);
            wet = echo - wet;
        }
        wet * 0.25
    }
}

/// La marimba: el sintetizador le pide una muestra a la vez.
pub struct Marimba {
    score: Vec<Hit>,
    /// Reloj de la partitura en corcheas y el siguiente golpe por disparar.
    clock: f64,
    cursor: usize,
    voices: [Voice; VOICES],
    /// Muestras por segundo con que se afino todo (0 antes de la primera).
    rate: f32,
    /// Cuanto queda del toc de una muestra a la siguiente.
    knock_fade: f32,
    /// Formantes nasales del charleo y el color del toc.
    nasal: [Resonance; 2],
    wood: Resonance,
    room: Room,
    noise: u32,
}

impl Marimba {
    pub fn new() -> Self {
        Self {
            score: score(),
            clock: 0.0,
            cursor: 0,
            voices: [Voice::default(); VOICES],
            rate: 0.0,
            knock_fade: 0.0,
            nasal: [Resonance::default(); 2],
            wood: Resonance::default(),
            room: Room::new(),
            noise: 0x4D41_5249,
        }
    }

    /// Afina filtros y teclas para otra frecuencia de muestreo.
    fn tune(&mut self, rate: f32) {
        self.rate = rate;
        self.knock_fade = (-1.0 / (0.003 * rate)).exp();
        self.nasal = [Resonance::new(700.0, 4.0, rate), Resonance::new(2100.0, 6.0, rate)];
        self.wood = Resonance::new(1200.0, 0.8, rate);
        for voice in self.voices.iter_mut().filter(|v| v.active) {
            voice.tune(rate);
        }
    }

    #[inline]
    fn white(&mut self) -> f32 {
        // xorshift32: ruido blanco barato y determinista.
        let mut x = self.noise;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        (x >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// Dispara los golpes de la partitura que ya llegaron.
    fn fire(&mut self) {
        while let Some(hit) = self.score.get(self.cursor).copied() {
            if hit.at > self.clock {
                break;
            }
            self.strike(hit);
            self.cursor += 1;
        }
    }

    /// Golpea una tecla: la misma si ya suena (se suma al movimiento), una
    /// libre si no, o la mas callada si todas estan ocupadas.
    fn strike(&mut self, hit: Hit) {
        let ringing = self.voices.iter().position(|v| v.active && v.key == hit.key);
        let slot = ringing.unwrap_or_else(|| {
            self.voices.iter().position(|v| !v.active).unwrap_or_else(|| {
                let quietest = self
                    .voices
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.energy().total_cmp(&b.1.energy()));
                quietest.map_or(0, |(i, _)| i)
            })
        });
        if ringing.is_none() {
            self.voices[slot] = Voice::new(hit.key, self.rate);
        }
        self.voices[slot].strike(hit.force, hit.mallet);
    }

    /// La siguiente muestra (mono, entre -0.6 y 0.6 aproximadamente), a
    /// `sample_rate` muestras por segundo.
    pub fn next(&mut self, sample_rate: f32) -> f32 {
        if sample_rate != self.rate {
            self.tune(sample_rate);
        }
        self.clock += EIGHTHS_PER_SECOND / sample_rate as f64;
        self.fire();
        // Fin de la vuelta: la partitura empieza otra vez, pero las teclas
        // siguen sonando, asi no hay corte.
        if self.clock >= LOOP {
            self.clock -= LOOP;
            self.cursor = 0;
            self.fire();
        }

        let mut bars = 0.0;
        let mut slaps = 0.0;
        let mut knock = 0.0;
        for voice in self.voices.iter_mut().filter(|v| v.active) {
            let (sound, slap) = voice.sound();
            bars += sound;
            slaps += slap;
            knock += voice.knock;
            voice.knock *= self.knock_fade;
            if voice.energy() < SILENCE && voice.knock < 1e-4 {
                voice.active = false;
            }
        }
        let noise = self.white();
        // La membrana no choca siempre igual: un poco de ruido en cada
        // chasquido. Los formantes convierten el tren de chasquidos en el
        // zumbido nasal (como la voz: pulsos de la garganta por la boca).
        let slaps = slaps * (1.0 + 0.4 * noise);
        let buzz = self.nasal[0].run(slaps) + 0.6 * self.nasal[1].run(slaps);
        let knock = self.wood.run(knock * noise);
        let dry = bars + BUZZ * buzz + knock;
        let wet = self.room.run(dry);
        (dry + WET * wet) * GAIN
    }
}

impl Default for Marimba {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::SAMPLE_RATE;

    fn render(seconds: f32) -> Vec<f32> {
        let mut marimba = Marimba::new();
        let rate = SAMPLE_RATE as f32;
        (0..(seconds * rate) as usize).map(|_| marimba.next(rate)).collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Potencia de `samples` en `freq` (algoritmo de Goertzel).
    fn power_at(samples: &[f32], freq: f32) -> f32 {
        let coeff = 2.0 * (TAU * freq / SAMPLE_RATE as f32).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in samples {
            let s0 = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        s1 * s1 + s2 * s2 - coeff * s1 * s2
    }

    #[test]
    fn thirty_seconds_are_finite_bounded_and_audible() {
        let samples = render(30.0);
        assert!(samples.iter().all(|s| s.is_finite()));
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let level = rms(&samples);
        assert!(peak < 0.8, "pico {peak}");
        assert!((0.05..0.25).contains(&level), "rms {level}");
        // Suena todo el tiempo: ningun segundo queda en silencio.
        for second in samples.chunks(SAMPLE_RATE as usize) {
            assert!(rms(second) > 0.03);
        }
    }

    #[test]
    fn the_same_marimba_plays_the_same_samples() {
        assert_eq!(render(4.0), render(4.0));
    }

    #[test]
    fn every_bar_lasts_six_eighths_and_stays_in_the_key() {
        for (bar, melody) in MELODY.iter().enumerate() {
            let notes: Vec<Note> = melody.split_whitespace().map(Note::parse).collect();
            let len: u32 = notes.iter().map(|n| n.len as u32).sum();
            assert_eq!(len, 6, "compas {}", bar + 1);
            for note in notes {
                assert!(IN_SCALE[note.key as usize % 12], "compas {}", bar + 1);
                assert!((60..=88).contains(&note.key), "compas {}", bar + 1);
            }
        }
        assert_eq!(Note::parse("^Fa#5:3~"), Note { key: 78, len: 3, tremolo: true, grace: true });
        // La segunda voz va una tercera de la escala abajo.
        assert_eq!(diatonic(71, -2), 67); // Si4 -> Sol4
        assert_eq!(diatonic(74, -2), 71); // Re5 -> Si4
        assert_eq!(diatonic(72, 1), 74); // Do5 -> Re5
    }

    #[test]
    fn the_score_never_runs_out_of_voices() {
        // Si nunca se llenan, nunca se roba una tecla que todavia suena.
        let mut marimba = Marimba::new();
        let rate = SAMPLE_RATE as f32;
        let mut most = 0;
        for _ in 0..(LOOP / EIGHTHS_PER_SECOND * 1.2 * rate as f64) as usize {
            marimba.next(rate);
            most = most.max(marimba.voices.iter().filter(|v| v.active).count());
        }
        assert!(most < VOICES, "{most} teclas a la vez");
    }

    #[test]
    fn a_bar_sounds_at_its_pitch_with_overtones_that_die_first() {
        let rate = SAMPLE_RATE as f32;
        let mut voice = Voice::new(69, rate);
        voice.strike(1.0, Mallet::Tiple);
        let samples: Vec<f32> = (0..(0.3 * rate) as usize).map(|_| voice.sound().0).collect();
        let (early, late) = samples.split_at((0.05 * rate) as usize);
        let off = power_at(early, 1000.0);
        for ratio in PARTIALS {
            assert!(power_at(early, 440.0 * ratio) > 50.0 * off, "parcial {ratio}");
        }
        // El sobretono a 4x pierde terreno contra la fundamental.
        let overtone = |s: &[f32]| power_at(s, 440.0 * PARTIALS[1]) / power_at(s, 440.0);
        assert!(overtone(late) < 0.1 * overtone(early));
    }

    #[test]
    #[ignore = "escribe 30 s de marimba en un WAV para escucharla"]
    fn write_a_wav_to_listen() {
        let path = std::env::var("MARIMBA_WAV")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/target/marimba.wav").into());
        let data: Vec<u8> = render(30.0)
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32_767.0).round() as i16).to_le_bytes())
            .collect();
        // Encabezado RIFF/WAVE de 44 bytes: PCM, mono, 16 bits.
        let mut wav = Vec::with_capacity(44 + data.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        std::fs::write(&path, wav).expect("no se pudo escribir el WAV");
    }
}
