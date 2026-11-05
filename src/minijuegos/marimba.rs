//! La marimba de la Luna: el minijuego de la Luna de Xelaju (version
//! chapina). Fichas que bajan por cuatro carriles hasta las teclas de una
//! marimba (D, F, J, K), como Piano Tiles, pero con la marimba de verdad:
//!
//! - **Vos tocas una parte del conjunto** y la marimba toca lo demas. En
//!   *Bajo* (facil) quien juega lleva el bajo y suenan solos el centro y el
//!   tiple; en *Tiple* y *Tiple con redobles* lleva la melodia (con su
//!   segunda voz) y suenan solos el bajo y el centro. La partitura es la del
//!   son de `crate::marimba` (`written`), y cada golpe se sintetiza con el
//!   mismo instrumento (`Instrument`): lo que no se toca no suena, y se oye.
//! - **El redoble**: en la marimba una nota larga no se sostiene apretando,
//!   se redobla (golpes rapidos). En *Tiple con redobles* las fichas largas
//!   piden machacar la tecla; cada golpe repone lo que la tecla perdio desde
//!   el anterior (`refill_after`), asi que si se deja de redoblar la nota se
//!   apaga. En *Tiple* el redoble suena solo al acertar la nota.
//! - **Las tres vueltas**, como en Piano Tiles, cada una mas rapida: 119,
//!   139 y 158 negras con puntillo por minuto. Los carriles siguen el
//!   contorno de la melodia: si sube, la ficha va mas a la derecha.
//! - **El reloj sale del audio**: el sintetizador cuenta las muestras que
//!   genera, y lo que se ve es ese tiempo menos lo que tarda en oirse (los
//!   bloques en cola y el ajuste de quien juega, para audifonos lentos),
//!   suavizado porque el audio avanza a saltos de un bloque. Asi la ficha
//!   llega a la tecla cuando suena su nota, aunque un frame tarde.
//! - **El juicio**: Perfecto a 45 ms, Bien a 90 y Ok a 135; mas tarde es
//!   falla. La racha multiplica (hasta x4) y el animo del publico sube con
//!   los aciertos y baja con las fallas: si se acaba, el publico se va.
//! - **Lo que se ve**: la pista en perspectiva (una camara mirando un plano:
//!   `y = y_fuga + k / z`), con las fichas puestas por tiempo; cada fila de
//!   la pantalla es un instante, asi que las fichas, las lineas del compas y
//!   la faja de los bordes se acortan con la distancia solas. Abajo, las
//!   teclas de hormigo con sus cuerdas y el faldon de tela tipica. Todo por
//!   pixel en CPU y en paralelo por filas.

use std::f32::consts::{PI, TAU};

use super::lienzo::{self, cover, diamond, halo, over, round_box};
use super::{Align, Font, Keys, Minigame, Overlay, Records};
use crate::audio::SAMPLE_RATE;
use crate::marimba::{self as son, Instrument, Mallet, Role};
use crate::math::{Perlin, Vec3, hash_u32, smoothstep};
use crate::parallel;

/// Carriles (y teclas: D, F, J, K).
pub const LANES: usize = 4;
/// Ventanas del juicio, en segundos a cada lado de la nota.
const PERFECT: f64 = 0.045;
const GOOD: f64 = 0.09;
const OK: f64 = 0.135;
/// El tempo de cada vuelta, en veces el del son.
const LAPS: [f64; 3] = [0.9, 1.05, 1.2];
/// Corcheas de entrada (dos compases de bajo y centro) antes de la melodia.
const INTRO: f64 = 12.0;
/// Silencio antes de la entrada.
const PREROLL: f64 = 0.8;
/// Lo que tarda en oirse lo sintetizado: un bloque y medio de 512 muestras
/// en la cola de raylib y el periodo del dispositivo.
const LATENCY: f64 = 0.03;
/// Medio largo de una ficha de un golpe, en segundos.
const TILE: f32 = 0.034;
/// Muestras por segundo.
const RATE: f64 = SAMPLE_RATE as f64;

/// Los colores de los carriles: hilos de un huipil (magenta, oro,
/// turquesa y morado), calidos y frios alternados para distinguirlos.
const LANE_COLORS: [Vec3; LANES] = [
    Vec3::new(1.0, 0.2, 0.52),
    Vec3::new(1.0, 0.68, 0.16),
    Vec3::new(0.18, 0.86, 0.78),
    Vec3::new(0.6, 0.32, 1.0),
];
const GOLD: Vec3 = Vec3::new(1.0, 0.78, 0.4);
const HUIPIL_RED: Vec3 = Vec3::new(0.55, 0.035, 0.07);
/// Madera de hormigo (la de las teclas) y del marco.
const HORMIGO: Vec3 = Vec3::new(0.34, 0.12, 0.05);
const FRAME: Vec3 = Vec3::new(0.11, 0.045, 0.022);
/// Cuerda de algodon, clara sobre el marco oscuro.
const CORD: Vec3 = Vec3::new(0.62, 0.48, 0.32);
const BRASS: Vec3 = Vec3::new(0.85, 0.62, 0.3);
/// Largo de cada tecla (en profundidad del plano): las graves son mas
/// largas, como en la marimba.
const BAR_LENGTH: [f32; LANES] = [0.2, 0.185, 0.17, 0.155];
/// Medio ancho de una tecla, en fraccion de su carril: entre tecla y tecla
/// se ve el marco con las cuerdas.
const BAR_HALF: f32 = 0.36;
/// Donde pasan las cuerdas por cada tecla (sus nodos), de la punta de abajo
/// a la de arriba.
const NODES: [f32; 2] = [0.224, 0.776];
/// Teclas que suenan si se golpea sin ficha (un acorde de Sol).
const FREE_TIPLE: [u8; LANES] = [67, 71, 74, 79];
const FREE_BAJO: [u8; LANES] = [43, 47, 50, 55];

/// Que parte toca quien juega.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Bajo,
    Tiple,
    Redoble,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Bajo, Level::Tiple, Level::Redoble];

    fn name(self) -> &'static str {
        match self {
            Level::Bajo => "Bajo · fácil",
            Level::Tiple => "Tiple · normal",
            Level::Redoble => "Tiple con redobles · difícil",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Level::Bajo => "Llevás el bajo: dos o tres golpes por compás; la melodía suena sola",
            Level::Tiple => "Llevás la melodía; el bajo y el centro te acompañan",
            Level::Redoble => "La melodía, y las notas largas se redoblan: machacá la tecla",
        }
    }

    fn record(self) -> &'static str {
        match self {
            Level::Bajo => "marimba_bajo",
            Level::Tiple => "marimba_tiple",
            Level::Redoble => "marimba_redoble",
        }
    }

    /// Segundos que tarda una ficha en bajar toda la pista.
    fn approach(self) -> f32 {
        match self {
            Level::Bajo => 1.7,
            Level::Tiple => 1.45,
            Level::Redoble => 1.35,
        }
    }

    fn mallet(self) -> Mallet {
        match self {
            Level::Bajo => Mallet::Bajo,
            _ => Mallet::Tiple,
        }
    }
}

/// Un golpe con su tiempo en segundos de la cancion.
#[derive(Clone, Copy, Debug)]
struct Strike {
    time: f64,
    key: u8,
    force: f32,
    mallet: Mallet,
}

/// Que tan bien se le pego a una ficha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    Perfect,
    Good,
    Ok,
}

impl Grade {
    fn of(error: f64) -> Self {
        if error.abs() <= PERFECT {
            Grade::Perfect
        } else if error.abs() <= GOOD {
            Grade::Good
        } else {
            Grade::Ok
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Waiting,
    Hit(Grade),
    Missed,
}

/// Una ficha: una nota que toca quien juega.
#[derive(Clone, Debug)]
struct Tile {
    time: f64,
    /// Donde termina el redoble (igual a `time` si es de un golpe).
    end: f64,
    lane: usize,
    /// Lo que suena al acertarla: la tecla y la que va con ella (la segunda
    /// voz o la octava del bajo).
    sound: [(u8, f32); 2],
    /// Los golpes escritos del redoble: en *Tiple* suenan solos al acertar.
    roll: Vec<Strike>,
    state: State,
    /// Golpes de redoble que lleva y cuando fue el ultimo.
    mashes: u32,
    last_mash: f64,
}

impl Tile {
    fn long(&self) -> bool {
        self.end > self.time
    }
}

/// La cancion armada para una parte.
struct Song {
    tiles: Vec<Tile>,
    /// Lo que toca la marimba sola, en orden.
    band: Vec<Strike>,
    /// Los pulsos (negras con puntillo) y si empiezan compas.
    beats: Vec<(f64, bool)>,
    /// Cuando empieza cada vuelta y cuando termina la ultima.
    laps: [f64; 4],
    /// Cuando ya no queda nada que oir.
    end: f64,
}

/// El tempo de la cancion: la entrada y cada vuelta a su velocidad.
struct Tempo {
    /// (desde que corchea, en que segundo, corcheas por segundo).
    segments: Vec<(f64, f64, f64)>,
}

impl Tempo {
    fn new() -> Self {
        let first = son::EIGHTHS_PER_SECOND * LAPS[0];
        let mut segments = vec![(0.0, 0.0, first)];
        let (mut eighth, mut second) = (INTRO, INTRO / first);
        for factor in LAPS {
            let rate = son::EIGHTHS_PER_SECOND * factor;
            segments.push((eighth, second, rate));
            eighth += son::LOOP;
            second += son::LOOP / rate;
        }
        Self { segments }
    }

    /// El segundo de la corchea `eighth` (contando desde la entrada).
    fn seconds(&self, eighth: f64) -> f64 {
        let segment = self.segments.iter().rev().find(|s| eighth >= s.0).unwrap_or(&self.segments[0]);
        segment.1 + (eighth - segment.0) / segment.2
    }

    /// Corcheas por segundo en `eighth`.
    fn rate(&self, eighth: f64) -> f64 {
        self.segments.iter().rev().find(|s| eighth >= s.0).unwrap_or(&self.segments[0]).2
    }
}

/// Los carriles de una melodia: siguen su contorno en una ventana de unas
/// pocas notas (la nota mas grave de la ventana a la izquierda, la mas aguda
/// a la derecha). Una nota repetida va en el mismo carril y una distinta en
/// otro, hacia donde va la melodia.
fn contour(keys: &[u8]) -> Vec<usize> {
    let top = LANES - 1;
    let mut lanes: Vec<usize> = Vec::with_capacity(keys.len());
    for (i, &key) in keys.iter().enumerate() {
        let window = &keys[i.saturating_sub(5)..(i + 6).min(keys.len())];
        let lo = *window.iter().min().unwrap() as f32;
        let hi = *window.iter().max().unwrap() as f32;
        let mut lane = if hi > lo { ((key as f32 - lo) / (hi - lo) * top as f32).round() as usize } else { 1 };
        if let (Some(&prev_key), Some(&prev_lane)) = (i.checked_sub(1).map(|j| &keys[j]), lanes.last()) {
            let up = key > prev_key;
            if key == prev_key {
                lane = prev_lane;
            } else if lane == prev_lane || (lane > prev_lane) != up {
                lane = if up { (prev_lane + 1).min(top) } else { prev_lane.saturating_sub(1) };
            }
        }
        lanes.push(lane);
    }
    lanes
}

/// Los carriles del bajo: por altura, del mas grave al mas agudo.
fn bass_lane(keys: &[u8], key: u8) -> usize {
    let mut distinct: Vec<u8> = keys.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    let rank = distinct.iter().position(|&k| k == key).unwrap_or(0);
    rank * LANES / distinct.len().max(1)
}

/// Arma la cancion para `level`: la entrada, tres vueltas del son y el
/// acorde final.
fn build(level: Level) -> Song {
    let tempo = Tempo::new();
    let written = son::written();
    let mut tiles = Vec::new();
    let mut band = Vec::new();
    let strike = |time: f64, h: &son::Hit| Strike { time, key: h.key, force: h.force, mallet: h.mallet };
    let accompaniment = |role: Role| matches!(role, Role::Bajo | Role::Octava | Role::Centro);

    // La entrada: el bajo y el centro del compas 1 (Sol) y del 3 (Re7).
    for h in written.iter().filter(|h| accompaniment(h.role)) {
        if (0.0..6.0).contains(&h.at) {
            band.push(strike(tempo.seconds(h.at), h));
        } else if (12.0..18.0).contains(&h.at) {
            band.push(strike(tempo.seconds(h.at - 6.0), h));
        }
    }

    let melody: Vec<u8> = written.iter().filter(|h| h.role == Role::Melodia).map(|h| h.key).collect();
    let melody_lanes = contour(&melody);
    let bass: Vec<u8> = written.iter().filter(|h| h.role == Role::Bajo).map(|h| h.key).collect();

    for lap in 0..LAPS.len() {
        let start = INTRO + lap as f64 * son::LOOP;
        let at = |eighth: f64| tempo.seconds(start + eighth);
        let mut nth_note = 0;
        let mut i = 0;
        while i < written.len() {
            let h = &written[i];
            match (level, h.role) {
                (Level::Bajo, Role::Bajo) => {
                    let octave = written
                        .iter()
                        .find(|o| o.role == Role::Octava && o.at == h.at)
                        .map_or((h.key, 0.0), |o| (o.key, o.force));
                    let time = at(h.at);
                    tiles.push(Tile {
                        time,
                        end: time,
                        lane: bass_lane(&bass, h.key),
                        sound: [(h.key, h.force), octave],
                        roll: Vec::new(),
                        state: State::Waiting,
                        mashes: 0,
                        last_mash: 0.0,
                    });
                }
                (Level::Bajo, Role::Octava) => {}
                (Level::Bajo, _) => band.push(strike(at(h.at), h)),
                (_, Role::Melodia) => {
                    // Detras de la nota vienen su segunda voz y su redoble.
                    let second = written.get(i + 1).filter(|s| s.role == Role::Segunda);
                    let mut roll = Vec::new();
                    let mut j = i + 1 + second.is_some() as usize;
                    while let Some(r) = written.get(j).filter(|r| r.role == Role::Redoble) {
                        roll.push(strike(at(r.at), r));
                        j += 1;
                    }
                    let time = at(h.at);
                    let end = if h.tremolo { at(h.at + h.len - 0.35) } else { time };
                    tiles.push(Tile {
                        time,
                        end,
                        lane: melody_lanes[nth_note],
                        sound: [(h.key, h.force), second.map_or((h.key, 0.0), |s| (s.key, s.force))],
                        roll,
                        state: State::Waiting,
                        mashes: 0,
                        last_mash: 0.0,
                    });
                    nth_note += 1;
                    i = j;
                    continue;
                }
                (_, role) if accompaniment(role) || role == Role::Adorno => band.push(strike(at(h.at), h)),
                _ => {}
            }
            i += 1;
        }
    }

    // El final: todos en Sol y el tiple redobla un compas y medio.
    let close = INTRO + LAPS.len() as f64 * son::LOOP;
    let final_time = tempo.seconds(close);
    let rate = tempo.rate(close);
    let chord: [(u8, f32, Mallet); 5] = [
        (43, 1.3, Mallet::Bajo),
        (55, 0.6, Mallet::Bajo),
        (55, 0.7, Mallet::Centro),
        (59, 0.7, Mallet::Centro),
        (62, 0.7, Mallet::Centro),
    ];
    let (lead, second) = (74u8, son::second_voice(74));
    let hold = 9.0;
    let roll: Vec<Strike> = (1..)
        .map(|k| k as f64 * 0.4)
        .take_while(|&e| e < hold - 0.25)
        .flat_map(|e| {
            let time = final_time + e / rate;
            [
                Strike { time, key: lead, force: 0.8 * son::refill(lead), mallet: Mallet::Tiple },
                Strike { time, key: second, force: 0.52 * son::refill(second), mallet: Mallet::Tiple },
            ]
        })
        .collect();
    let end = final_time + (hold - 0.35) / rate;
    for (k, &(key, force, mallet)) in chord.iter().enumerate() {
        if level == Level::Bajo && k == 0 {
            tiles.push(Tile {
                time: final_time,
                end: final_time,
                lane: bass_lane(&bass, key),
                sound: [(key, force), (55, 0.6)],
                roll: Vec::new(),
                state: State::Waiting,
                mashes: 0,
                last_mash: 0.0,
            });
        } else if !(level == Level::Bajo && k == 1) {
            band.push(Strike { time: final_time, key, force, mallet });
        }
    }
    if level == Level::Bajo {
        band.push(Strike { time: final_time, key: lead, force: 1.0, mallet: Mallet::Tiple });
        band.push(Strike { time: final_time, key: second, force: 0.65, mallet: Mallet::Tiple });
        band.extend(&roll);
    } else {
        tiles.push(Tile {
            time: final_time,
            end,
            lane: 2,
            sound: [(lead, 1.0), (second, 0.65)],
            roll,
            state: State::Waiting,
            mashes: 0,
            last_mash: 0.0,
        });
    }

    // La marimba sola tampoco toca como metronomo: hasta 2 ms de mas o de
    // menos en cada golpe, siempre igual.
    for (i, s) in band.iter_mut().enumerate() {
        let h = hash_u32(i as u32 ^ 0x4C55_4E41);
        s.time += ((h & 0xFFFF) as f64 / 65_535.0 - 0.5) * 0.004;
    }
    band.sort_by(|a, b| a.time.total_cmp(&b.time));
    tiles.sort_by(|a, b| a.time.total_cmp(&b.time));

    let mut beats = Vec::new();
    let mut eighth = 0.0;
    while eighth <= close + 6.0 {
        beats.push((tempo.seconds(eighth), eighth % 6.0 == 0.0));
        eighth += 3.0;
    }
    let laps = [0, 1, 2, 3].map(|k| tempo.seconds(INTRO + k as f64 * son::LOOP));
    Song { tiles, band, beats, laps, end: end + 1.6 }
}

/// La geometria de la pantalla: la pista es un plano visto en perspectiva.
/// A la profundidad `z` (1 en la linea de golpe, mas lejos hacia arriba) la
/// fila es `y = fuga + k / z` y todo mide `1 / z`.
#[derive(Clone, Copy, Debug)]
struct Stage {
    width: usize,
    height: usize,
    /// Escala respecto a 720 de alto.
    s: f32,
    cx: f32,
    hit: f32,
    vanish: f32,
    k: f32,
    /// Medio ancho de la pista en la linea de golpe.
    half: f32,
    far: f32,
}

impl Stage {
    fn new(width: usize, height: usize) -> Self {
        let (w, h) = (width as f32, height as f32);
        let hit = 0.66 * h;
        let vanish = -0.42 * h;
        let k = hit - vanish;
        Self {
            width,
            height,
            s: h / 720.0,
            cx: w * 0.5,
            hit,
            vanish,
            k,
            half: (0.27 * w).min(0.62 * h),
            far: k / (0.03 * h - vanish),
        }
    }

    fn z(&self, y: f32) -> f32 {
        self.k / (y - self.vanish)
    }

    fn y(&self, z: f32) -> f32 {
        self.vanish + self.k / z
    }

    /// Cuanto cambia `z` por pixel hacia abajo en la fila `y`.
    fn dz(&self, y: f32) -> f32 {
        let d = y - self.vanish;
        self.k / (d * d)
    }

    /// Segundos por delante de la linea de golpe a la profundidad `z`.
    fn ahead(&self, z: f32, approach: f32) -> f32 {
        (z - 1.0) / (self.far - 1.0) * approach
    }

    /// El centro del carril `lane` a la profundidad `z`.
    fn lane_x(&self, lane: usize, z: f32) -> f32 {
        self.cx + ((lane as f32 + 0.5) / LANES as f32 * 2.0 - 1.0) * self.half / z
    }

    /// Fila del borde de abajo de las teclas.
    fn keys_bottom(&self) -> f32 {
        self.y(1.0 - BAR_LENGTH[0])
    }
}

/// Una chispa de un acierto.
#[derive(Clone, Copy, Debug)]
struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    color: Vec3,
    size: f32,
}

/// Un anillo que se abre desde la tecla al acertar.
#[derive(Clone, Copy, Debug)]
struct Ring {
    x: f32,
    y: f32,
    age: f32,
    color: Vec3,
}

/// Un texto que sube y se desvanece sobre un carril.
#[derive(Clone, Copy, Debug)]
struct Popup {
    text: &'static str,
    lane: usize,
    age: f32,
    color: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Menu,
    Playing,
    Results,
}

/// Lo que se lleva de la partida.
#[derive(Clone, Copy, Debug, Default)]
struct Stats {
    score: i64,
    combo: u32,
    best_combo: u32,
    /// Perfectos, bien, ok y fallas.
    counts: [u32; 4],
    rolls: u32,
    /// Animo del publico, de 0 a 1.
    cheer: f64,
    failed: bool,
}

impl Stats {
    fn multiplier(&self) -> i64 {
        1 + (self.combo / 10).min(3) as i64
    }

    /// Precision de 0 a 1: perfecto vale 1, bien 0.7 y ok 0.4.
    fn accuracy(&self) -> f64 {
        let [p, g, o, m] = self.counts.map(f64::from);
        let total = p + g + o + m;
        if total == 0.0 { 0.0 } else { (p + 0.7 * g + 0.4 * o) / total }
    }
}

/// Lo que se dice al terminar.
fn verdict(stats: &Stats) -> &'static str {
    let accuracy = stats.accuracy();
    if stats.failed {
        "Se fue el público"
    } else if accuracy >= 0.95 {
        "¡De a huevo!"
    } else if accuracy >= 0.85 {
        "¡Chilero!"
    } else if accuracy >= 0.7 {
        "Va pues"
    } else if accuracy >= 0.5 {
        "Púchica…"
    } else {
        "Ni modo, otra vez"
    }
}

pub struct MarimbaTiles {
    screen: Screen,
    /// La parte elegida en el menu.
    choice: usize,
    level: Level,
    song: Option<Song>,
    /// Indices de las fichas de cada carril en orden, y la primera que
    /// falta juzgar.
    lanes: [Vec<usize>; LANES],
    cursor: [usize; LANES],
    stats: Stats,
    /// Tiempo de la cancion que se ve (sigue al del audio).
    clock: f64,
    /// Muestras sintetizadas desde que empezo la cancion.
    rendered: u64,
    band_cursor: usize,
    /// Si la marimba sola sigue tocando (calla si se va el publico).
    band_on: bool,
    /// Golpes que suenan solos al acertar (el redoble en *Tiple*), en orden.
    queued: Vec<Strike>,
    /// Golpes de quien juega: suenan al principio del siguiente bloque.
    pending: Vec<(u8, f32, Mallet)>,
    instrument: Instrument,
    stage: Stage,
    background: Vec<u8>,
    noise: Perlin,
    /// Segundos desde que se abrio (para lo que se mueve en los menus).
    wall: f64,
    /// Cuando se apreto cada carril, si esta abajo y cuando se acerto (con
    /// el color del juicio).
    pressed: [f64; LANES],
    held: [bool; LANES],
    lit: [(f64, Vec3); LANES],
    sparks: Vec<Spark>,
    rings: Vec<Ring>,
    popups: Vec<Popup>,
    /// Cuando se termino la cancion (o se fue el publico).
    finished: Option<f64>,
    records: Records,
    /// Ajuste de quien juega para audifonos con retraso, en ms.
    offset_ms: i64,
    new_record: bool,
    done: bool,
}

const OFFSET_KEY: &str = "marimba_ajuste_ms";

impl MarimbaTiles {
    pub fn new(background: &[u8], width: usize, height: usize, records: Records) -> Self {
        let stage = Stage::new(width, height);
        let mut backdrop = lienzo::backdrop(background, width, height, (3.0 * stage.s) as usize, 0.45);
        moonlight(&mut backdrop, &stage);
        Self {
            screen: Screen::Menu,
            choice: 1,
            level: Level::Tiple,
            song: None,
            lanes: Default::default(),
            cursor: [0; LANES],
            stats: Stats::default(),
            clock: 0.0,
            rendered: 0,
            band_cursor: 0,
            band_on: false,
            queued: Vec::new(),
            pending: Vec::new(),
            instrument: Instrument::new(),
            stage,
            background: backdrop,
            noise: Perlin::new(0x4D41_524D),
            wall: 0.0,
            pressed: [-10.0; LANES],
            held: [false; LANES],
            lit: [(-10.0, GOLD); LANES],
            sparks: Vec::new(),
            rings: Vec::new(),
            popups: Vec::new(),
            finished: None,
            offset_ms: records.get(OFFSET_KEY).unwrap_or(0),
            records,
            new_record: false,
            done: false,
        }
    }

    /// Empieza la cancion de `level` desde la entrada.
    pub fn play(&mut self, level: Level) {
        let song = build(level);
        let mut lanes: [Vec<usize>; LANES] = Default::default();
        for (i, tile) in song.tiles.iter().enumerate() {
            lanes[tile.lane].push(i);
        }
        self.level = level;
        self.song = Some(song);
        self.lanes = lanes;
        self.cursor = [0; LANES];
        self.stats = Stats { cheer: 0.6, ..Stats::default() };
        self.rendered = 0;
        self.clock = -PREROLL - self.lag();
        self.band_cursor = 0;
        self.band_on = true;
        self.queued.clear();
        self.finished = None;
        self.new_record = false;
        self.popups.clear();
        self.screen = Screen::Playing;
    }

    /// Lo que tarda en oirse lo sintetizado, con el ajuste de quien juega.
    fn lag(&self) -> f64 {
        LATENCY + self.offset_ms as f64 / 1000.0
    }

    fn approach(&self) -> f32 {
        self.level.approach()
    }

    /// Teclas para jugar solo (pruebas y capturas): golpea cada ficha a su
    /// tiempo, redobla las largas y falla una de cada `miss_every` (0 nunca).
    pub fn demo_keys(&self, dt: f64, miss_every: usize) -> Keys {
        let mut keys = Keys::default();
        let Some(song) = &self.song else { return keys };
        let next = self.clock + dt;
        for (i, tile) in song.tiles.iter().enumerate() {
            if tile.time > next + 1.0 {
                break;
            }
            let skip = miss_every > 0 && i % miss_every == miss_every - 1;
            if tile.state == State::Waiting && tile.time > self.clock && tile.time <= next && !skip {
                keys.lane_pressed[tile.lane] = true;
            }
            // El redoble: un golpe cada ~90 ms mientras dura.
            let rolling = matches!(tile.state, State::Hit(_)) && tile.long() && next > tile.time + 0.05 && next < tile.end - 0.1;
            if rolling && self.level == Level::Redoble && ((next - tile.time) / 0.09).floor() != ((self.clock - tile.time) / 0.09).floor() {
                keys.lane_pressed[tile.lane] = true;
            }
        }
        keys
    }

    /// Un golpe de quien juega en `lane` en el tiempo `t` de la cancion.
    fn press(&mut self, lane: usize, t: f64) {
        let Some(song) = &self.song else { return };
        let list = &self.lanes[lane];
        let mut c = self.cursor[lane];
        while c < list.len() && song.tiles[list[c]].state != State::Waiting {
            c += 1;
        }
        self.cursor[lane] = c;
        let next = list.get(c).copied().map(|i| (i, t - song.tiles[i].time));
        // La ficha anterior del carril, si se esta redoblando.
        let rolling = c
            .checked_sub(1)
            .map(|p| list[p])
            .filter(|&i| {
                let tile = &song.tiles[i];
                matches!(tile.state, State::Hit(_)) && tile.long() && t > tile.time && t < tile.end
            })
            .filter(|_| self.level == Level::Redoble);
        // Mientras se redobla, la nota que sigue en el carril solo se toma
        // con un golpe casi exacto: machacar no se la roba.
        match (next, rolling) {
            (Some((i, error)), _) if error.abs() <= PERFECT => self.judge(i, error),
            (_, Some(i)) => self.mash(i, t),
            (Some((i, error)), None) if error.abs() <= OK => self.judge(i, error),
            _ => self.free(lane, 0.55),
        }
    }

    /// Una ficha acertada con `error` segundos de diferencia.
    fn judge(&mut self, index: usize, error: f64) {
        let grade = Grade::of(error);
        let level = self.level;
        let now = self.clock;
        let Some(song) = &mut self.song else { return };
        let tile = &mut song.tiles[index];
        tile.state = State::Hit(grade);
        tile.last_mash = tile.time;
        let lane = tile.lane;
        for (key, force) in tile.sound {
            if force > 0.0 {
                self.pending.push((key, force, level.mallet()));
            }
        }
        if level == Level::Tiple {
            // El redoble suena solo, del golpe que sigue en adelante.
            self.queued.extend(tile.roll.iter().filter(|s| s.time > now));
            self.queued.sort_by(|a, b| a.time.total_cmp(&b.time));
        }
        let stats = &mut self.stats;
        stats.counts[grade.index()] += 1;
        stats.combo += 1;
        stats.best_combo = stats.best_combo.max(stats.combo);
        let base = match grade {
            Grade::Perfect => 100,
            Grade::Good => 70,
            Grade::Ok => 40,
        };
        stats.score += base * stats.multiplier();
        stats.cheer = (stats.cheer
            + match grade {
                Grade::Perfect => 0.03,
                Grade::Good => 0.02,
                Grade::Ok => 0.01,
            })
        .min(1.0);
        let (text, color, glow) = match grade {
            Grade::Perfect => ("¡Perfecto!", [255, 214, 120, 255], GOLD),
            Grade::Good => ("¡Bien!", [120, 236, 220, 255], LANE_COLORS[2]),
            Grade::Ok => ("Ok", [236, 230, 222, 255], Vec3::splat(0.9)),
        };
        self.popup(text, lane, color);
        self.lit[lane] = (self.wall, LANE_COLORS[lane].lerp(glow, 0.35));
        let sparks = match grade {
            Grade::Perfect => 22,
            Grade::Good => 14,
            Grade::Ok => 8,
        };
        self.burst(lane, sparks, glow);
        if self.stats.combo > 0 && self.stats.combo % 50 == 0 {
            self.popup("¡Qué racha!", lane, [255, 180, 220, 255]);
        }
    }

    /// Un golpe del redoble de la ficha `index`.
    fn mash(&mut self, index: usize, t: f64) {
        let Some(song) = &mut self.song else { return };
        let tile = &mut song.tiles[index];
        let gap = (t - tile.last_mash).clamp(0.04, 0.4) as f32;
        tile.last_mash = t;
        tile.mashes += 1;
        let first = tile.mashes == 1;
        let lane = tile.lane;
        let [(key, _), (second, weight)] = tile.sound;
        self.pending.push((key, 0.8 * son::refill_after(key, gap), Mallet::Tiple));
        if weight > 0.0 {
            self.pending.push((second, 0.52 * son::refill_after(second, gap), Mallet::Tiple));
        }
        self.stats.rolls += 1;
        self.stats.score += 10 * self.stats.multiplier();
        self.stats.cheer = (self.stats.cheer + 0.003).min(1.0);
        self.lit[lane] = (self.wall, LANE_COLORS[lane]);
        self.burst(lane, 4, LANE_COLORS[lane]);
        if first {
            self.popup("¡Redoble!", lane, [255, 190, 230, 255]);
        }
    }

    /// Un golpe sin ficha: suena la tecla del carril, mas suave.
    fn free(&mut self, lane: usize, force: f32) {
        let key = match self.level {
            Level::Bajo => FREE_BAJO[lane],
            _ => FREE_TIPLE[lane],
        };
        self.pending.push((key, force, self.level.mallet()));
        self.lit[lane] = (self.wall, LANE_COLORS[lane] * 0.6);
    }

    /// Las fichas que ya pasaron sin golpe.
    fn sweep(&mut self) {
        let Some(song) = &mut self.song else { return };
        let mut missed = Vec::new();
        for lane in 0..LANES {
            let list = &self.lanes[lane];
            let mut c = self.cursor[lane];
            while let Some(&i) = list.get(c) {
                let tile = &mut song.tiles[i];
                if tile.state == State::Waiting {
                    if self.clock - tile.time <= OK {
                        break;
                    }
                    tile.state = State::Missed;
                    missed.push(lane);
                }
                c += 1;
            }
            self.cursor[lane] = c;
        }
        for lane in missed {
            let stats = &mut self.stats;
            stats.counts[3] += 1;
            stats.combo = 0;
            stats.cheer -= 0.075;
            self.popup("Fallaste", lane, [255, 120, 100, 255]);
        }
    }

    fn popup(&mut self, text: &'static str, lane: usize, color: [u8; 4]) {
        // Uno por carril: el nuevo reemplaza al anterior.
        self.popups.retain(|p| p.lane != lane);
        self.popups.push(Popup { text, lane, age: 0.0, color });
    }

    /// Chispas y un anillo desde la tecla de `lane`.
    fn burst(&mut self, lane: usize, count: u32, color: Vec3) {
        let st = self.stage;
        let (x, y) = (st.lane_x(lane, 1.0), st.hit);
        let seed = hash_u32(self.wall.to_bits() as u32 ^ lane as u32 * 0x9E37);
        for k in 0..count {
            let h = hash_u32(seed ^ k.wrapping_mul(0x85EB_CA6B));
            let unit = |shift: u32| ((h >> shift) & 0xFF) as f32 / 255.0;
            let angle = -PI * (0.15 + 0.7 * unit(0));
            let speed = (160.0 + 300.0 * unit(8)) * st.s;
            self.sparks.push(Spark {
                x: x + (unit(16) - 0.5) * 40.0 * st.s,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                age: 0.0,
                life: 0.3 + 0.4 * unit(24),
                color: LANE_COLORS[lane].lerp(color, unit(4)),
                size: (1.6 + 1.6 * unit(12)) * st.s,
            });
        }
        if count > 6 {
            self.rings.push(Ring { x, y, age: 0.0, color });
        }
    }

    /// Termina la cancion: guarda el record y pasa a los resultados.
    fn finish(&mut self) {
        self.screen = Screen::Results;
        self.finished = Some(self.wall);
        let key = self.level.record();
        if !self.stats.failed && self.stats.score > self.records.get(key).unwrap_or(0) {
            self.records.set(key, self.stats.score);
            self.new_record = true;
        }
    }

    /// Avanza el reloj que se ve hacia el del audio.
    fn follow_audio(&mut self, dt: f64) {
        let target = self.rendered as f64 / RATE - PREROLL - self.lag();
        self.clock += dt;
        let error = target - self.clock;
        if error.abs() > 0.1 {
            self.clock = target;
        } else {
            self.clock += error * (1.0 - (-8.0 * dt).exp());
        }
    }
}

impl Minigame for MarimbaTiles {
    fn update(&mut self, dt: f64, keys: &Keys) {
        self.wall += dt;
        self.held = keys.lane_down;
        for (lane, &pressed) in keys.lane_pressed.iter().enumerate() {
            if pressed {
                self.pressed[lane] = self.wall;
            }
        }
        match self.screen {
            Screen::Menu => {
                if keys.escape {
                    self.done = true;
                }
                if keys.up {
                    self.choice = (self.choice + Level::ALL.len() - 1) % Level::ALL.len();
                }
                if keys.down {
                    self.choice = (self.choice + 1) % Level::ALL.len();
                }
                if keys.left || keys.right {
                    self.offset_ms = (self.offset_ms + if keys.right { 10 } else { -10 }).clamp(-300, 300);
                    self.records.set(OFFSET_KEY, self.offset_ms);
                }
                self.level = Level::ALL[self.choice];
                for lane in 0..LANES {
                    if keys.lane_pressed[lane] {
                        self.free(lane, 0.8);
                    }
                }
                if keys.enter {
                    self.play(Level::ALL[self.choice]);
                }
            }
            Screen::Playing => {
                self.follow_audio(dt);
                if keys.escape {
                    self.band_on = false;
                    self.queued.clear();
                    self.screen = Screen::Menu;
                    return;
                }
                if !self.stats.failed {
                    for lane in 0..LANES {
                        if keys.lane_pressed[lane] {
                            self.press(lane, self.clock);
                        }
                    }
                    self.sweep();
                    if self.stats.cheer <= 0.0 {
                        self.stats.cheer = 0.0;
                        self.stats.failed = true;
                        self.band_on = false;
                        self.queued.clear();
                        self.finished = Some(self.wall);
                    }
                }
                let over = self.song.as_ref().is_some_and(|s| self.clock > s.end);
                let gone = self.stats.failed && self.finished.is_some_and(|at| self.wall - at > 1.4);
                if over || gone {
                    self.finish();
                }
            }
            Screen::Results => {
                if keys.enter {
                    self.play(self.level);
                } else if keys.escape {
                    self.screen = Screen::Menu;
                }
                for lane in 0..LANES {
                    if keys.lane_pressed[lane] {
                        self.free(lane, 0.8);
                    }
                }
            }
        }

        let dt = dt as f32;
        let gravity = 900.0 * self.stage.s;
        for spark in &mut self.sparks {
            spark.x += spark.vx * dt;
            spark.y += spark.vy * dt;
            spark.vy += gravity * dt;
            spark.age += dt;
        }
        self.sparks.retain(|s| s.age < s.life);
        for ring in &mut self.rings {
            ring.age += dt;
        }
        self.rings.retain(|r| r.age < 0.35);
        for popup in &mut self.popups {
            popup.age += dt;
        }
        self.popups.retain(|p| p.age < 0.7);
    }

    fn audio(&mut self, out: &mut [f32]) {
        let rate = SAMPLE_RATE as f32;
        self.instrument.tune_to(rate);
        for (key, force, mallet) in std::mem::take(&mut self.pending) {
            self.instrument.strike(key, force, mallet);
        }
        let playing = self.screen == Screen::Playing;
        for sample in out.iter_mut() {
            if playing {
                let t = self.rendered as f64 / RATE - PREROLL;
                if let Some(song) = &self.song {
                    while self.band_on && self.band_cursor < song.band.len() && song.band[self.band_cursor].time <= t {
                        let s = song.band[self.band_cursor];
                        self.instrument.strike(s.key, s.force, s.mallet);
                        self.band_cursor += 1;
                    }
                }
                while self.queued.first().is_some_and(|s| s.time <= t) {
                    let s = self.queued.remove(0);
                    self.instrument.strike(s.key, s.force, s.mallet);
                }
            }
            self.rendered += 1;
            let x = self.instrument.next(rate) * 1.3;
            *sample = x / (1.0 + 0.3 * x.abs());
        }
    }

    fn draw(&mut self, rgba: &mut [u8]) {
        let st = self.stage;
        let frame = self.frame();
        let background = &self.background;
        parallel::chunks_mut(rgba, st.width * 4, 1, |y, row| {
            let back = &background[y * st.width * 4..(y + 1) * st.width * 4];
            if back.len() == row.len() {
                row.copy_from_slice(back);
            }
            frame.row(y, row);
        });
    }

    fn overlay(&self) -> Overlay {
        let mut o = Overlay::default();
        let st = &self.stage;
        let s = st.s;
        let w = st.width as f32;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 210];
        let accent = [255, 204, 120, 255];

        // Las letras de las teclas, sobre cada tecla.
        for (lane, letter) in ["D", "F", "J", "K"].into_iter().enumerate() {
            let z = 1.0 - BAR_LENGTH[lane] * 0.5;
            let alpha = if self.held[lane] { 255 } else { 170 };
            o.text(letter, st.lane_x(lane, z), st.y(z) - 13.0 * s, Font::Heading, [255, 236, 200, alpha], Align::Center);
        }

        match self.screen {
            Screen::Menu => {
                let pw = 620.0 * s;
                let ph = 404.0 * s;
                let (px, py) = ((w - pw) * 0.5, 40.0 * s);
                o.panel(px, py, pw, ph);
                o.text("La marimba de la Luna", w * 0.5, py + 14.0 * s, Font::Big(52.0 * s), accent, Align::Center);
                o.text("Luna de Xelajú  ·  el son de la marimba procedural, en tres vueltas", w * 0.5, py + 80.0 * s, Font::Body, dim, Align::Center);
                for (i, level) in Level::ALL.into_iter().enumerate() {
                    let y = py + 120.0 * s + i as f32 * 76.0 * s;
                    let chosen = i == self.choice;
                    if chosen {
                        o.bar(px + 18.0 * s, y - 6.0 * s, pw - 36.0 * s, 68.0 * s, 1.0, [255, 204, 120, 34], false);
                    }
                    let color = if chosen { accent } else { cream };
                    o.text(if chosen { "»" } else { " " }, px + 30.0 * s, y, Font::Heading, accent, Align::Left);
                    o.text(level.name(), px + 54.0 * s, y, Font::Heading, color, Align::Left);
                    let record = match self.records.get(level.record()) {
                        Some(r) => format!("récord {}", thousands(r)),
                        None => "sin récord".to_string(),
                    };
                    o.text(record, px + pw - 30.0 * s, y + 4.0 * s, Font::Small, dim, Align::Right);
                    o.text(level.blurb(), px + 54.0 * s, y + 31.0 * s, Font::Body, dim, Align::Left);
                }
                let offset = format!("Ajuste de tiempo (← →): {:+} ms   ·   subilo si las notas te suenan tarde", self.offset_ms);
                o.text(offset, w * 0.5, py + ph - 52.0 * s, Font::Small, dim, Align::Center);
                o.text("D F J K: las teclas (probalas)   ·   ↑ ↓: elegir   ·   Enter: tocar   ·   Esc: salir", w * 0.5, py + ph - 28.0 * s, Font::Small, cream, Align::Center);
            }
            Screen::Playing | Screen::Results => self.playing_overlay(&mut o),
        }
        if self.screen == Screen::Results {
            self.results_overlay(&mut o);
        }
        o
    }

    fn done(&self) -> bool {
        self.done
    }
}

impl MarimbaTiles {
    fn playing_overlay(&self, o: &mut Overlay) {
        let st = &self.stage;
        let s = st.s;
        let w = st.width as f32;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 210];
        let accent = [255, 204, 120, 255];
        let Some(song) = &self.song else { return };

        // El puntaje y la racha, arriba a la izquierda.
        o.panel(16.0 * s, 16.0 * s, 250.0 * s, 112.0 * s);
        o.text("Puntaje", 32.0 * s, 26.0 * s, Font::Small, dim, Align::Left);
        o.text(thousands(self.stats.score), 32.0 * s, 42.0 * s, Font::Big(46.0 * s), cream, Align::Left);
        let combo = if self.stats.combo > 0 {
            format!("Racha {}   ×{}", self.stats.combo, self.stats.multiplier())
        } else {
            "Racha 0".to_string()
        };
        o.text(combo, 32.0 * s, 96.0 * s, Font::Bold, accent, Align::Left);

        // La cancion, la vuelta y cuanto falta, arriba a la derecha.
        let lap = song.laps.iter().take(3).filter(|&&t| self.clock >= t).count().max(1);
        let (pw, px) = (300.0 * s, w - 316.0 * s);
        o.panel(px, 16.0 * s, pw, 92.0 * s);
        o.text("Son de la Luna", px + 16.0 * s, 24.0 * s, Font::Heading, accent, Align::Left);
        let bpm = son::BPM * LAPS[lap - 1];
        o.text(format!("{}   ·   vuelta {lap} de 3   ·   {bpm:.0} por minuto", self.level.name().split(" · ").next().unwrap_or("")), px + 16.0 * s, 56.0 * s, Font::Small, dim, Align::Left);
        let progress = ((self.clock - song.laps[0]) / (song.laps[3] - song.laps[0])).clamp(0.0, 1.0) as f32;
        o.bar(px + 16.0 * s, 84.0 * s, pw - 32.0 * s, 6.0 * s, progress, accent, false);

        // El animo del publico, a la izquierda de la pista.
        let cheer = self.stats.cheer.clamp(0.0, 1.0) as f32;
        let color = if cheer > 0.5 {
            [120, 230, 150, 255]
        } else if cheer > 0.25 {
            [255, 210, 110, 255]
        } else {
            [255, 110, 90, 255]
        };
        let (mx, my, mh) = (st.cx - st.half * 1.075 - 50.0 * s, st.height as f32 * 0.3, st.height as f32 * 0.32);
        o.text("Ánimo", mx + 7.0 * s, my - 26.0 * s, Font::Small, dim, Align::Center);
        o.bar(mx, my, 14.0 * s, mh, cheer, color, true);

        // Los juicios sobre cada carril.
        for p in &self.popups {
            let rise = p.age * 60.0 * s;
            let alpha = (1.0 - (p.age / 0.7).powi(2)).clamp(0.0, 1.0);
            let mut color = p.color;
            color[3] = (color[3] as f32 * alpha) as u8;
            o.text(p.text, st.lane_x(p.lane, 1.12), st.hit - 70.0 * s - rise, Font::Heading, color, Align::Center);
        }

        // La entrada, el cambio de vuelta y el publico que se va.
        let first = song.tiles.first().map_or(0.0, |t| t.time);
        let banner = if self.stats.failed {
            Some(("Se fue el público", [255, 130, 100, 255]))
        } else if self.clock < first - 0.25 && self.screen == Screen::Playing {
            Some((
                match self.level {
                    Level::Bajo => "Vos llevás el bajo",
                    _ => "Vos llevás la melodía",
                },
                cream,
            ))
        } else {
            [(1, "Segunda vuelta · más alegre"), (2, "Tercera vuelta · ¡a todo!")]
                .into_iter()
                .find(|&(k, _)| self.clock >= song.laps[k] && self.clock < song.laps[k] + 1.6)
                .map(|(_, text)| (text, accent))
        };
        if let Some((text, color)) = banner {
            o.text(text, st.cx, st.height as f32 * 0.3, Font::Big(40.0 * s), color, Align::Center);
        }
        if self.screen == Screen::Playing {
            o.text("Esc: dejar de tocar", w - 16.0 * s, st.height as f32 - 30.0 * s, Font::Small, [200, 190, 176, 150], Align::Right);
        }
    }

    fn results_overlay(&self, o: &mut Overlay) {
        let st = &self.stage;
        let s = st.s;
        let w = st.width as f32;
        let cream = [242, 236, 226, 255];
        let dim = [200, 190, 176, 210];
        let accent = [255, 204, 120, 255];
        let stats = &self.stats;
        let (pw, ph) = (520.0 * s, 330.0 * s);
        let (px, py) = ((w - pw) * 0.5, 120.0 * s);
        o.panel(px, py, pw, ph);
        let headline = if stats.failed { [255, 130, 100, 255] } else { accent };
        o.text(verdict(stats), w * 0.5, py + 16.0 * s, Font::Big(50.0 * s), headline, Align::Center);
        o.text(format!("{}   ·   {}", self.level.name(), thousands(stats.score)), w * 0.5, py + 86.0 * s, Font::Heading, cream, Align::Center);
        let record = if self.new_record {
            "¡Récord nuevo!".to_string()
        } else if stats.failed {
            "Sin terminar no cuenta para el récord".to_string()
        } else {
            match self.records.get(self.level.record()) {
                Some(r) => format!("Récord: {}", thousands(r)),
                None => String::new(),
            }
        };
        o.text(record, w * 0.5, py + 122.0 * s, Font::Bold, if self.new_record { accent } else { dim }, Align::Center);
        let [p, g, k, m] = stats.counts;
        let lines = [
            format!("Precisión {:.0} %   ·   racha máxima {}", stats.accuracy() * 100.0, stats.best_combo),
            format!("Perfectos {p}   ·   bien {g}   ·   ok {k}   ·   fallas {m}"),
            if self.level == Level::Redoble { format!("Golpes de redoble {}", stats.rolls) } else { String::new() },
        ];
        for (i, line) in lines.iter().enumerate() {
            o.text(line.as_str(), w * 0.5, py + 166.0 * s + i as f32 * 30.0 * s, Font::Body, cream, Align::Center);
        }
        o.text("Enter: otra vez   ·   Esc: elegir otra parte", w * 0.5, py + ph - 36.0 * s, Font::Small, dim, Align::Center);
    }

    /// Lo que se necesita para dibujar este frame (solo lectura, para las
    /// filas en paralelo).
    fn frame(&self) -> Frame<'_> {
        let approach = self.approach();
        let now = if self.screen == Screen::Menu { 0.0 } else { self.clock };
        let mut lanes: [Vec<Look>; LANES] = Default::default();
        let mut beats = Vec::new();
        if let (Some(song), true) = (&self.song, self.screen != Screen::Menu) {
            let from = song.tiles.partition_point(|t| t.time < now - 4.0);
            for tile in &song.tiles[from..] {
                let head = (tile.time - now) as f32;
                if head > approach + 0.2 {
                    break;
                }
                let tail = (tile.end - now) as f32;
                if tail < -0.3 {
                    continue;
                }
                let state = match tile.state {
                    State::Waiting => Shown::Waiting,
                    State::Missed => Shown::Missed,
                    State::Hit(_) if tile.long() => {
                        let rolling = match self.level {
                            Level::Redoble => (now - tile.last_mash) < 0.2,
                            _ => true,
                        };
                        Shown::Rolling(if rolling { 1.0 } else { 0.25 })
                    }
                    State::Hit(_) => continue,
                };
                lanes[tile.lane].push(Look { head, tail, state });
            }
            let from = song.beats.partition_point(|b| b.0 < now - 0.2);
            for &(time, bar) in &song.beats[from..] {
                let ahead = (time - now) as f32;
                if ahead > approach + 0.2 {
                    break;
                }
                beats.push((ahead, bar));
            }
        }
        let wall = self.wall;
        let press = std::array::from_fn(|l| {
            let age = (wall - self.pressed[l]) as f32;
            (if self.held[l] { 0.08 } else { 0.0 }) + 0.28 * (1.0 - age / 0.15).max(0.0)
        });
        let lit = std::array::from_fn(|l| {
            let (at, color) = self.lit[l];
            let age = (wall - at) as f32;
            color * (1.0 - age / 0.35).max(0.0).powi(2)
        });
        let sink = std::array::from_fn(|l| ((wall - self.pressed[l]) as f32 / 0.09).clamp(0.0, 1.0));
        Frame {
            st: self.stage,
            approach,
            now: now as f32,
            scroll: if self.screen == Screen::Menu { wall as f32 * 0.25 } else { now as f32 },
            lanes,
            beats,
            press,
            lit,
            sink,
            sparks: &self.sparks,
            rings: &self.rings,
            noise: &self.noise,
            dim: if self.screen == Screen::Playing { 1.0 } else { 0.55 },
        }
    }
}

/// Distancia de (x, y) al segmento de `a` a `b`.
fn segment(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (px, py) = (x - a.0, y - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let t = ((px * bx + py * by) / (bx * bx + by * by).max(1e-6)).clamp(0.0, 1.0);
    (px - bx * t).hypot(py - by * t)
}

/// Un numero con espacios entre los miles: 48 230.
fn thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Un brillo de luna detras de la pista, en el fondo ya preparado.
fn moonlight(rgba: &mut [u8], st: &Stage) {
    let (cx, cy) = (st.cx, st.height as f32 * 0.12);
    let radius = st.height as f32 * 0.75;
    parallel::chunks_mut(rgba, st.width * 4, 1, |y, row| {
        for x in 0..st.width {
            let d = (x as f32 + 0.5 - cx).hypot((y as f32 + 0.5 - cy) * 1.4) / radius;
            let glow = (-d * d * 2.5).exp();
            let p = &mut row[x * 4..x * 4 + 4];
            let c = lienzo::read(p) + Vec3::new(0.09, 0.07, 0.15) * glow;
            lienzo::write(p, c);
        }
    });
}

/// Como se ve una ficha.
#[derive(Clone, Copy, Debug)]
enum Shown {
    Waiting,
    Missed,
    /// Acertada y larga: el redoble con su brillo (1 si se esta redoblando).
    Rolling(f32),
}

/// Una ficha en pantalla: segundos hasta la cabeza y hasta el final.
#[derive(Clone, Copy, Debug)]
struct Look {
    head: f32,
    tail: f32,
    state: Shown,
}

/// Lo que se dibuja de un frame.
struct Frame<'a> {
    st: Stage,
    approach: f32,
    now: f32,
    /// Lo que corre la faja de los bordes (el tiempo de la cancion).
    scroll: f32,
    lanes: [Vec<Look>; LANES],
    /// Segundos hasta cada pulso y si empieza compas.
    beats: Vec<(f32, bool)>,
    /// Luz de cada carril por la tecla apretada.
    press: [f32; LANES],
    /// Brillo de cada tecla por el ultimo acierto.
    lit: [Vec3; LANES],
    /// Que tanto subio la tecla despues del golpe (0 recien golpeada).
    sink: [f32; LANES],
    sparks: &'a [Spark],
    rings: &'a [Ring],
    noise: &'a Perlin,
    /// Que tan encendida esta la pista (menos en el menu).
    dim: f32,
}

impl Frame<'_> {
    fn row(&self, y: usize, row: &mut [u8]) {
        let st = &self.st;
        let yf = y as f32 + 0.5;
        let top = st.y(st.far) - 4.0 * st.s;
        let keys_bottom = st.keys_bottom();
        let skirt_top = keys_bottom + 3.0 * st.s;
        if yf >= top && yf < st.hit {
            self.track(yf, row);
        }
        if yf >= st.hit - 2.0 * st.s && yf < skirt_top + 6.0 * st.s {
            self.keyboard(yf, row);
        }
        if yf >= skirt_top {
            self.skirt(yf, skirt_top, row);
        }
        self.targets(yf, row);
        if yf >= top && yf < st.hit + 60.0 * st.s {
            self.tiles(yf, row);
        }
        self.hit_line(yf, row);
        self.particles(yf, row);
    }

    /// El ancho de la pista en pixeles a la profundidad de la fila.
    fn span(&self, z: f32, extra: f32) -> (usize, usize) {
        let st = &self.st;
        let half = st.half / z * (1.0 + extra);
        let x0 = (st.cx - half - 2.0).floor().max(0.0) as usize;
        let x1 = ((st.cx + half + 2.0).ceil() as usize).min(st.width);
        (x0, x1)
    }

    /// La pista: vidrio oscuro, los carriles, las lineas de los pulsos, la
    /// luz de las teclas apretadas y la faja de los bordes.
    fn track(&self, yf: f32, row: &mut [u8]) {
        let st = &self.st;
        let z = st.z(yf);
        let half = st.half / z;
        let ahead = st.ahead(z, self.approach);
        // Segundos por pixel en esta fila.
        let per_px = self.approach / (st.far - 1.0) * st.dz(yf);
        let fog = smoothstep(st.far, st.far - 0.45, z);
        let near = smoothstep(st.far, 1.0, z);
        let band = 0.075;
        let (x0, x1) = self.span(z, band);
        let to_hit = st.hit - yf;
        for x in x0..x1 {
            let q = (x as f32 + 0.5 - st.cx) / half;
            let outside = q.abs() - 1.0;
            if outside > band + 2.0 / half {
                continue;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let back = lienzo::read(p);
            let mut c;
            if outside > 0.0 {
                // La faja: picos de colores que bajan con la cancion.
                let v = outside / band;
                let along = (self.scroll + ahead) / 0.16;
                let cell = along.floor();
                let peak = 1.0 - ((along - cell) - 0.5).abs() * 2.0;
                let soft = 1.5 / (band * half);
                let thread = [GOLD, LANE_COLORS[2], LANE_COLORS[0], LANE_COLORS[3]][(cell as i32).rem_euclid(4) as usize];
                let tooth = smoothstep(peak * 0.8 + soft, peak * 0.8 - soft, v);
                let face = HUIPIL_RED.lerp(thread, tooth);
                let edge = smoothstep(1.0, 1.0 - soft, v) * smoothstep(0.0, soft, v);
                c = over(back, face * (0.45 + 0.55 * near), edge * fog * 0.92 * self.dim.max(0.7));
            } else {
                // Vidrio oscuro, un poco mas claro cerca de las teclas.
                let glass = back * 0.32 + Vec3::new(0.03, 0.012, 0.045) + Vec3::new(0.05, 0.025, 0.03) * near * near;
                c = over(back, glass, 0.82 * fog);
                let lane_f = (q + 1.0) * 0.5 * LANES as f32;
                let lane = (lane_f as usize).min(LANES - 1);
                let u = lane_f - lane as f32;
                let lane_px = half * 2.0 / LANES as f32;
                // Hilos finos a lo largo de cada carril, como tejido.
                let weave = ((u - 0.5).abs() - 0.3).abs() * lane_px;
                c = c + LANE_COLORS[lane] * (0.05 * cover(weave - 0.6) * fog * self.dim);
                // La luz de la tecla apretada, que sube por su carril.
                let light = self.press[lane] * (-to_hit / (0.16 * st.height as f32)).exp();
                c = c + LANE_COLORS[lane] * (light * fog);
                // Los pulsos: una linea por negra con puntillo, mas fuerte
                // en el compas.
                for &(time, bar) in &self.beats {
                    let d = (ahead - time).abs() / per_px;
                    let (width, strength) = if bar { (1.0, 0.3) } else { (0.45, 0.13) };
                    c = c + GOLD * (cover(d - width) * strength * fog * self.dim);
                }
                // Las divisiones entre carriles.
                let seam = (q * 2.0 - (q * 2.0).round()).abs() * half * 0.5;
                if (q * 2.0).round().abs() < 1.5 {
                    c = over(c, GOLD * 0.8, cover(seam - 0.6) * 0.5 * fog);
                }
            }
            // Los bordes de la pista: una linea de oro con su brillo.
            let rim = outside.abs() * half;
            c = over(c, GOLD, cover(rim - 1.0) * 0.85 * fog);
            c = c + GOLD * (0.18 * halo(rim - 1.0, 4.0 * st.s) * fog * self.dim);
            lienzo::write(p, c);
        }
    }

    /// Las teclas de hormigo sobre su marco. Como en la marimba de verdad,
    /// las cuerdas pasan por los nodos de cada tecla (por dentro de la
    /// madera, asi que arriba no se ven) y de una tecla a la otra, entre
    /// postes de bronce: de arriba solo se ven en los huecos.
    fn keyboard(&self, yf: f32, row: &mut [u8]) {
        let st = &self.st;
        let z = st.z(yf);
        let half = st.half / z;
        let dz = st.dz(yf);
        let shortest = 1.0 - BAR_LENGTH[0];
        let (x0, x1) = self.span(z, 0.06);
        for x in x0..x1 {
            let xf = x as f32 + 0.5;
            let q = (xf - st.cx) / half;
            if q.abs() > 1.06 + 2.0 / half {
                continue;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let mut c = lienzo::read(p);
            // El marco: madera oscura detras de las teclas.
            let frame_d = (q.abs() - 1.05) * half;
            let frame_v = ((z - (shortest - 0.012)) / dz).min((1.0 + 0.004 - z) / dz);
            let frame = cover(frame_d).min(cover(-frame_v));
            if frame > 0.0 {
                let grain = self.noise.noise(Vec3::new(xf * 0.02, yf * 0.2, 3.0));
                c = over(c, FRAME * (0.8 + 0.2 * grain), frame);
            }
            let lane_f = (q + 1.0) * 0.5 * LANES as f32;
            let lane = (lane_f.floor().max(0.0) as usize).min(LANES - 1);
            let lane_px = half * 2.0 / LANES as f32;
            // Las cuerdas en el hueco mas cercano, con su poste.
            let gap = lane_f.round() as usize;
            if (1..LANES).contains(&gap) && frame > 0.0 {
                let left = gap - 1;
                for node in NODES {
                    let a = self.node(left, node, 1.0);
                    let b = self.node(gap, node, -1.0);
                    let d = segment(xf, yf, a, b) - 0.9 * st.s / z;
                    c = over(c, CORD * 0.8, cover(d) * 0.85);
                    let mid = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
                    let post = (xf - mid.0).hypot((yf - mid.1) * 1.6) - 3.4 * st.s / z;
                    c = over(c, BRASS * 0.8, cover(post));
                    c = c + BRASS * (0.5 * cover(post + 1.6 * st.s / z));
                }
            }
            let u = lane_f - lane as f32;
            let length = BAR_LENGTH[lane];
            // La tecla golpeada baja un poco y vuelve.
            let sink = (1.0 - self.sink[lane]) * 2.5 * st.s * dz;
            let mid = 1.0 - length * 0.5 - sink;
            let along_px = (z - mid) / dz;
            let d = round_box((u - 0.5) * lane_px, along_px, BAR_HALF * lane_px, length * 0.5 / dz, 11.0 * st.s / z);
            let a = cover(d);
            if a > 0.0 {
                // 0 en la punta de abajo, 1 arriba, en la linea de golpe.
                let along = ((z - (1.0 - length - sink)) / length).clamp(0.0, 1.0);
                let n = self.noise.noise(Vec3::new(u * 3.0 + lane as f32 * 7.3, along * 1.5, 0.0));
                let grain = 0.5 + 0.5 * ((u * 18.0 + 3.0 * n + lane as f32) * PI).sin();
                let mut wood = HORMIGO * (0.85 + 0.25 * grain + 0.3 * n);
                // La luz viene de arriba a la izquierda: la tecla es un poco
                // abombada, mas clara de un lado.
                let side = ((u - 0.5) / (2.0 * BAR_HALF)).clamp(-0.5, 0.5);
                wood = wood * (1.0 - 0.35 * side);
                // Laca: un brillo largo cerca de la punta de arriba.
                let gloss = (-((along - 0.8) / 0.12).powi(2)).exp();
                wood = wood + Vec3::new(0.3, 0.2, 0.15) * (0.45 * gloss * (0.6 - side));
                // Bisel angosto: el canto oscuro y un filo de luz por dentro.
                let bevel = smoothstep(-3.5 * st.s, -0.5, d);
                wood = wood * (1.0 - 0.45 * bevel);
                let lip = cover((d + 3.0 * st.s).abs() - 0.7 * st.s);
                wood = wood + Vec3::new(0.45, 0.32, 0.22) * (0.3 * lip * (0.5 - side));
                // El acierto enciende la tecla.
                // El acierto enciende la tecla sin tapar la veta.
                let lit = self.lit[lane];
                wood = wood * (1.0 + 1.6 * lit.luminance()) + lit * 0.35 + LANE_COLORS[lane] * (0.2 * self.press[lane]);
                c = over(c, wood, a);
            }
            // El brillo del acierto se riega alrededor de la tecla.
            c = c + self.lit[lane] * (0.35 * halo(d, 10.0 * st.s) * (1.0 - a));
            lienzo::write(p, c);
        }
    }

    /// Donde pasa una cuerda por el canto de la tecla `lane` (el derecho con
    /// `side` 1, el izquierdo con -1), en el nodo `node`.
    fn node(&self, lane: usize, node: f32, side: f32) -> (f32, f32) {
        let st = &self.st;
        let length = BAR_LENGTH[lane];
        let z = 1.0 - length * (1.0 - node);
        let lane_px = st.half / z * 2.0 / LANES as f32;
        (st.lane_x(lane, z) + side * BAR_HALF * lane_px, st.y(z))
    }

    /// El faldon: tela tipica que cuelga al frente de la marimba, con
    /// pliegues, una greca de dientes y rombos.
    fn skirt(&self, yf: f32, top: f32, row: &mut [u8]) {
        let st = &self.st;
        let half = st.half / (1.0 - BAR_LENGTH[0]) * 1.02;
        let x0 = (st.cx - half - 1.0).max(0.0) as usize;
        let x1 = ((st.cx + half + 1.0) as usize).min(st.width);
        let stitch = 6.0 * st.s;
        let j = ((yf - top) / stitch).floor() as i32;
        let fy = (yf - top) / stitch - j as f32 - 0.5;
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - st.cx;
            let edge = cover(dx.abs() - half);
            if edge <= 0.0 {
                continue;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let back = lienzo::read(p);
            let i = (dx / stitch).floor() as i32;
            let fx = dx / stitch - i as f32 - 0.5;
            let thread = match j {
                // El borde de arriba: el canto del marco.
                0 => Some(FRAME * 2.2),
                2 | 12 => Some(GOLD),
                4..=6 => {
                    // Greca de dientes.
                    let tooth = 4 + (i.rem_euclid(6) - 3).abs().min(2);
                    (j == tooth || j == 10 - tooth).then_some(LANE_COLORS[2])
                }
                8 | 9 if i.rem_euclid(2) == 0 => Some(LANE_COLORS[0]),
                14..=22 => {
                    // Rombos con su punto.
                    let c = i.rem_euclid(10) - 5;
                    let d = c.abs() + (j - 18).abs();
                    if d == 3 {
                        Some(LANE_COLORS[3])
                    } else if d == 0 {
                        Some(GOLD)
                    } else if d == 1 {
                        Some(LANE_COLORS[0])
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let mut cloth = HUIPIL_RED;
            if let Some(color) = thread {
                // Cada puntada, un cuadrito con hueco.
                let square = round_box(fx * stitch, fy * stitch, stitch * 0.38, stitch * 0.38, stitch * 0.15);
                cloth = over(cloth, color, cover(square));
            }
            // Pliegues: la tela se ondula.
            let fold = 0.78 + 0.22 * (dx / (46.0 * st.s) * TAU).cos();
            let fade = 1.0 - 0.35 * smoothstep(top, st.height as f32, yf);
            lienzo::write(p, over(back, cloth * (fold * fade), edge));
        }
    }

    /// Las fichas de cada carril.
    fn tiles(&self, yf: f32, row: &mut [u8]) {
        let st = &self.st;
        let z = st.z(yf);
        let half = st.half / z;
        let ahead = st.ahead(z, self.approach);
        let per_px = self.approach / (st.far - 1.0) * st.dz(yf);
        let fog = smoothstep(st.far, st.far - 0.4, z);
        let lane_px = half * 2.0 / LANES as f32;
        let radius = 7.0 * st.s / z;
        for (lane, looks) in self.lanes.iter().enumerate() {
            if looks.is_empty() {
                continue;
            }
            let center = st.lane_x(lane, z);
            let color = LANE_COLORS[lane];
            let x0 = (center - lane_px * 0.5).max(0.0) as usize;
            let x1 = ((center + lane_px * 0.5) as usize + 1).min(st.width);
            for look in looks {
                let head_px = (ahead - look.head) / per_px;
                let tail_px = (ahead - look.tail) / per_px;
                let tile_px = TILE / per_px;
                // Rapido: la fila no toca esta ficha ni su brillo.
                if head_px < -tile_px - 14.0 || tail_px > tile_px + 14.0 {
                    continue;
                }
                // Lo que ya paso de la linea desaparece (si no se fallo).
                let gone = match look.state {
                    Shown::Missed => smoothstep(-0.02, -0.14, look.head),
                    Shown::Rolling(_) => 0.0,
                    Shown::Waiting => smoothstep(-0.08, -0.16, look.head),
                };
                if gone >= 1.0 {
                    continue;
                }
                // Abajo de la linea la ficha se hunde en la tecla.
                let sunk = 1.0 - smoothstep(0.0, 16.0 * st.s, yf - st.hit);
                if sunk <= 0.0 {
                    continue;
                }
                for x in x0..x1 {
                    let dx = x as f32 + 0.5 - center;
                    let p = &mut row[x * 4..x * 4 + 4];
                    let mut c = lienzo::read(p);
                    // El cuerpo de una nota larga (el redoble).
                    if look.tail > look.head {
                        let from = match look.state {
                            Shown::Rolling(_) => look.head.max(0.0),
                            _ => look.head,
                        };
                        let (a_px, b_px) = ((ahead - from) / per_px, (ahead - look.tail) / per_px);
                        if look.tail > from {
                            let mid = (a_px + b_px) * 0.5;
                            let hl = (a_px - b_px) * 0.5;
                            let d = round_box(dx, mid, 0.2 * lane_px, hl.abs(), 4.0 * st.s / z);
                            let a = cover(d);
                            if a > 0.0 || d < 12.0 {
                                let (body, glow) = match look.state {
                                    Shown::Missed => (Vec3::new(0.25, 0.17, 0.17), 0.0),
                                    Shown::Rolling(on) => (color * (0.5 + 0.6 * on), on),
                                    Shown::Waiting => (color * 0.42, 0.15),
                                };
                                // Greca en zigzag que corre con la cancion.
                                let phase = ((self.now + ahead) / 0.09).fract();
                                let zig = ((phase - 0.5).abs() * 2.0 - 0.5) * 0.28 * lane_px;
                                let line = cover((dx - zig).abs() - 1.4 * st.s / z);
                                let fill = over(body, GOLD.lerp(Vec3::ONE, glow * 0.5), line * 0.8);
                                c = over(c, fill, a * fog);
                                c = c + color * (glow * 0.5 * halo(d, 8.0 * st.s) * (1.0 - a) * fog);
                            }
                        }
                    }
                    // La cabeza (o la ficha entera si es de un golpe).
                    if !matches!(look.state, Shown::Rolling(_)) {
                        let dy = head_px;
                        let d = round_box(dx, dy, 0.4 * lane_px, tile_px, radius);
                        if d < 16.0 * st.s {
                            let a = cover(d) * (1.0 - gone) * fog * sunk;
                            let (face, glow) = match look.state {
                                Shown::Missed => (Vec3::new(0.32, 0.12, 0.12), 0.0),
                                _ => (color, 1.0),
                            };
                            // Mas clara arriba, un canto blanco y un rombo con su
                            // punto al centro, como un brocado.
                            let v = (-dy / tile_px).clamp(-1.0, 1.0);
                            let mut fill = face * (0.82 + 0.25 * v);
                            fill = over(fill, Vec3::ONE, smoothstep(-4.5 * st.s / z, -0.8, d) * 0.55 * glow);
                            let rombo = diamond(dx, dy, 0.17 * lane_px, tile_px * 0.62);
                            fill = over(fill, face * 0.35, cover(rombo) * 0.85);
                            fill = over(fill, Vec3::new(1.0, 0.95, 0.85), cover(rombo + 0.55 * tile_px.min(0.17 * lane_px)) * glow);
                            c = over(c, fill, a);
                            c = c + color * (0.45 * glow * halo(d, 6.0 * st.s) * (1.0 - cover(d)) * (1.0 - gone) * fog * sunk);
                        }
                    }
                    lienzo::write(p, c);
                }
            }
        }
    }

    /// Las marcas de la linea de golpe: el contorno de una ficha en cada
    /// carril, donde tiene que caer. Se encienden con la tecla.
    fn targets(&self, yf: f32, row: &mut [u8]) {
        let st = &self.st;
        let per_px = self.approach / (st.far - 1.0) * st.dz(yf);
        let tile_px = TILE / per_px;
        let dy = yf - st.hit;
        if dy.abs() > tile_px + 3.0 {
            return;
        }
        let z = st.z(yf);
        let lane_px = st.half / z * 2.0 / LANES as f32;
        for lane in 0..LANES {
            let center = st.lane_x(lane, z);
            let x0 = (center - lane_px * 0.5).max(0.0) as usize;
            let x1 = ((center + lane_px * 0.5) as usize + 1).min(st.width);
            let strength = (0.35 + 1.5 * self.press[lane]).min(1.0) * self.dim;
            for x in x0..x1 {
                let d = round_box(x as f32 + 0.5 - center, dy, 0.4 * lane_px, tile_px, 7.0 * st.s);
                let ring = cover(d.abs() - 0.9 * st.s);
                if ring <= 0.0 {
                    continue;
                }
                let p = &mut row[x * 4..x * 4 + 4];
                lienzo::write(p, over(lienzo::read(p), LANE_COLORS[lane], ring * strength));
            }
        }
    }

    /// La linea de golpe: oro con su brillo, de borde a borde de la pista.
    fn hit_line(&self, yf: f32, row: &mut [u8]) {
        let st = &self.st;
        let d = (yf - st.hit).abs();
        if d > 14.0 * st.s {
            return;
        }
        let x0 = (st.cx - st.half - 2.0).max(0.0) as usize;
        let x1 = ((st.cx + st.half + 2.0) as usize).min(st.width);
        for x in x0..x1 {
            let q = ((x as f32 + 0.5 - st.cx) / st.half).abs();
            let inside = cover((q - 1.0) * st.half);
            let p = &mut row[x * 4..x * 4 + 4];
            let c = lienzo::read(p) + GOLD * ((0.75 * cover(d - 1.2 * st.s) + 0.25 * halo(d - 1.2 * st.s, 5.0 * st.s)) * inside * self.dim);
            lienzo::write(p, c);
        }
    }

    /// Las chispas y los anillos de los aciertos, que se suman a la imagen.
    fn particles(&self, yf: f32, row: &mut [u8]) {
        let st = &self.st;
        for spark in self.sparks {
            let reach = spark.size * 3.0;
            if (yf - spark.y).abs() > reach {
                continue;
            }
            let fade = (1.0 - spark.age / spark.life).max(0.0);
            let x0 = (spark.x - reach).max(0.0) as usize;
            let x1 = ((spark.x + reach) as usize + 1).min(st.width);
            for x in x0..x1 {
                let d2 = ((x as f32 + 0.5 - spark.x).powi(2) + (yf - spark.y).powi(2)) / (spark.size * spark.size);
                let glow = (-d2).exp() * fade;
                if glow < 0.003 {
                    continue;
                }
                let p = &mut row[x * 4..x * 4 + 4];
                lienzo::write(p, lienzo::read(p) + spark.color * glow);
            }
        }
        for ring in self.rings {
            let t = ring.age / 0.35;
            let (rx, ry) = ((20.0 + 150.0 * t) * st.s, (8.0 + 50.0 * t) * st.s);
            if (yf - ring.y).abs() > ry + 6.0 {
                continue;
            }
            let x0 = (ring.x - rx - 6.0).max(0.0) as usize;
            let x1 = ((ring.x + rx + 6.0) as usize).min(st.width);
            for x in x0..x1 {
                let e = ((x as f32 + 0.5 - ring.x) / rx).hypot((yf - ring.y) / ry);
                let d = (e - 1.0).abs() * rx;
                let glow = cover(d - 1.5 * st.s) * (1.0 - t) * 0.8;
                if glow <= 0.0 {
                    continue;
                }
                let p = &mut row[x * 4..x * 4 + 4];
                lienzo::write(p, lienzo::read(p) + ring.color * glow);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> MarimbaTiles {
        let (w, h) = (320, 180);
        MarimbaTiles::new(&vec![40u8; w * h * 4], w, h, Records::in_memory())
    }

    /// Juega `seconds` con `keys` cada frame (a 120 fps), con el audio
    /// llevando el reloj como en el juego.
    fn run(game: &mut MarimbaTiles, seconds: f64, mut keys: impl FnMut(&MarimbaTiles, f64) -> Keys) -> Vec<f32> {
        let dt = 1.0 / 120.0;
        let mut sound = Vec::new();
        let steps = (seconds / dt) as usize;
        for step in 0..steps {
            let k = keys(game, dt);
            game.update(dt, &k);
            let samples = ((step + 1) as f64 * dt * RATE) as usize - (step as f64 * dt * RATE) as usize;
            let mut block = vec![0.0; samples];
            game.audio(&mut block);
            sound.extend(block);
        }
        sound
    }

    #[test]
    fn every_part_has_its_notes_in_order_and_in_the_lanes() {
        let melody = son::written().iter().filter(|h| h.role == Role::Melodia).count();
        let bass = son::written().iter().filter(|h| h.role == Role::Bajo).count();
        for level in Level::ALL {
            let song = build(level);
            let notes = if level == Level::Bajo { bass } else { melody };
            assert_eq!(song.tiles.len(), 3 * notes + 1, "{level:?}");
            assert!(song.tiles.windows(2).all(|w| w[0].time <= w[1].time));
            assert!(song.band.windows(2).all(|w| w[0].time <= w[1].time));
            assert!(song.tiles.iter().all(|t| t.lane < LANES && t.end >= t.time));
            // Cada carril se usa.
            for lane in 0..LANES {
                assert!(song.tiles.iter().any(|t| t.lane == lane), "{level:?} carril {lane}");
            }
            // Ninguna ficha empieza antes de que suene la entrada, y la
            // ultima vuelta es la mas rapida.
            assert!(song.tiles[0].time > 1.5);
            let lap = |k: usize| song.laps[k + 1] - song.laps[k];
            assert!(lap(0) > lap(1) && lap(1) > lap(2));
            assert!(song.end > song.tiles.last().unwrap().end);
            // Las largas solo en la melodia.
            let long = song.tiles.iter().filter(|t| t.long()).count();
            if level == Level::Bajo {
                assert_eq!(long, 0);
            } else {
                assert!(long > 20, "{long}");
            }
        }
    }

    #[test]
    fn the_lanes_follow_the_melody() {
        let keys: Vec<u8> = son::written().iter().filter(|h| h.role == Role::Melodia).map(|h| h.key).collect();
        let lanes = contour(&keys);
        for i in 1..keys.len() {
            let (dk, dl) = (keys[i] as i32 - keys[i - 1] as i32, lanes[i] as i32 - lanes[i - 1] as i32);
            if dk == 0 {
                assert_eq!(dl, 0, "nota {i}");
            } else if dl != 0 {
                assert_eq!(dk.signum(), dl.signum(), "nota {i}");
            } else {
                // Solo en la orilla una nota distinta repite carril.
                assert!(lanes[i] == 0 || lanes[i] == LANES - 1, "nota {i}");
            }
        }
    }

    #[test]
    fn playing_every_note_on_time_is_perfect() {
        for level in Level::ALL {
            let mut game = game();
            game.play(level);
            let total = game.song.as_ref().unwrap().tiles.len() as u32;
            let length = game.song.as_ref().unwrap().end + PREROLL + 1.0;
            let sound = run(&mut game, length, |g, dt| g.demo_keys(dt, 0));
            assert_eq!(game.screen, Screen::Results, "{level:?}");
            let stats = game.stats;
            assert!(!stats.failed);
            assert_eq!(stats.counts[0], total, "{level:?}: {:?}", stats.counts);
            assert_eq!(stats.best_combo, total);
            assert!((stats.accuracy() - 1.0).abs() < 1e-9);
            assert_eq!(verdict(&stats), "¡De a huevo!");
            if level == Level::Redoble {
                assert!(stats.rolls > 100, "{}", stats.rolls);
            }
            assert!(game.new_record);
            assert!(sound.iter().all(|s| s.is_finite() && s.abs() < 1.0));
            let rms = (sound.iter().map(|s| s * s).sum::<f32>() / sound.len() as f32).sqrt();
            assert!(rms > 0.02, "rms {rms}");
        }
    }

    #[test]
    fn missing_everything_sends_the_crowd_home() {
        let mut game = game();
        game.play(Level::Tiple);
        run(&mut game, 30.0, |_, _| Keys::default());
        assert!(game.stats.failed);
        assert_eq!(game.screen, Screen::Results);
        assert_eq!(game.stats.counts[0], 0);
        assert!(!game.new_record, "perder no cuenta como record");
    }

    #[test]
    fn an_early_hit_is_good_and_a_stray_hit_is_free() {
        let mut game = game();
        game.play(Level::Bajo);
        let first = game.song.as_ref().unwrap().tiles[0].clone();
        // 70 ms antes de la primera nota, y despues un golpe en otro carril.
        let dt = 1.0 / 120.0;
        while game.clock < first.time - 0.07 - dt {
            game.update(dt, &Keys::default());
            game.audio(&mut vec![0.0; (dt * RATE) as usize]);
        }
        let mut keys = Keys::default();
        keys.lane_pressed[first.lane] = true;
        game.update(0.0, &keys);
        assert_eq!(game.stats.counts, [0, 1, 0, 0]);
        let other = (first.lane + 2) % LANES;
        let mut keys = Keys::default();
        keys.lane_pressed[other] = true;
        game.update(0.0, &keys);
        assert_eq!(game.stats.counts, [0, 1, 0, 0]);
        assert_eq!(game.stats.combo, 1);
    }

    #[test]
    fn the_frame_draws_without_holes_on_every_screen() {
        let mut game = game();
        let (w, h) = (game.stage.width, game.stage.height);
        let mut rgba = vec![0u8; w * h * 4];
        game.draw(&mut rgba);
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        game.play(Level::Redoble);
        run(&mut game, 12.0, |g, dt| g.demo_keys(dt, 7));
        game.draw(&mut rgba);
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        assert!(!game.overlay().items.is_empty());
        assert_eq!(thousands(1_234_567), "1 234 567");
        assert_eq!(thousands(980), "980");
    }
}
