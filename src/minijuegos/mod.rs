//! Minijuegos: cada planeta tiene (o va a tener) el suyo.
//!
//! Cerca de su planeta el HUD lo ofrece y **Enter** lo abre. Mientras se
//! juega, el mundo se queda quieto: la ultima imagen del render se congela
//! de fondo (difuminada y oscura, `lienzo::backdrop`) y el juego se dibuja
//! encima en CPU, igual que todo lo demas, pero sin el costo del 3D: corre a
//! los fps de la pantalla aunque la vista de fondo cueste 100 ms. raylib
//! solo pone el texto y los paneles del HUD (`Overlay`), como en el resto
//! del juego.
//!
//! | Planeta | Version | Minijuego |
//! | --- | --- | --- |
//! | Luna de Xelaju | chapina | La marimba de la Luna ([marimba.rs]) |
//! | La capital | chapina | La carrera del Periferico ([carrera.rs]) |
//! | La capital (en el suelo) | chapina | Shucos del Reformador ([shucos.rs]) |
//! | El volcan de Fuego (Antigua) | chapina | Talega Surfers ([surfers.rs]) |
//!
//! Cada minijuego cumple `Minigame`: recibe las teclas del frame, llena su
//! propio audio (un stream aparte con bloques chicos, para que el golpe
//! suene enseguida), se dibuja sobre la imagen y dice que texto va encima.
//! Los records se guardan en `minijuegos.txt`, junto al programa.

pub mod carrera;
pub mod ciudad;
pub mod escenario;
pub mod figuras;
pub mod flota;
pub mod hornear;
pub mod lienzo;
pub mod malla;
pub mod marimba;
pub mod personaje;
pub mod posproceso;
pub mod shucos;
pub mod sonido;
pub mod surfers;

use std::path::PathBuf;

use crate::app::App;
use crate::scene::Version;

/// Las teclas de este frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Keys {
    /// D, F, J y K: las que se acaban de apretar y las que estan abajo.
    pub lane_pressed: [bool; 4],
    pub lane_down: [bool; 4],
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub enter: bool,
    pub escape: bool,
    /// Para manejar: el volante (-1 a 1, flechas o A y D), el acelerador y
    /// el freno (arriba y abajo, o W y S) mientras estan apretados, el turbo
    /// (Shift o espacio) y el pito (H) al apretarlos.
    pub steer: f32,
    pub throttle: bool,
    pub brake: bool,
    pub boost: bool,
    pub horn: bool,
    /// Las letras y los numeros apretados en este frame (un bit por tecla,
    /// ver `typed`).
    pub typed: u64,
}

impl Keys {
    /// Si se apreto la letra o el numero `c` en este frame.
    pub fn typed(&self, c: char) -> bool {
        key_bit(c).is_some_and(|b| self.typed & (1 << b) != 0)
    }

    /// Marca `c` como apretada (el piloto automatico y las pruebas).
    pub fn press(&mut self, c: char) {
        if let Some(b) = key_bit(c) {
            self.typed |= 1 << b;
        }
    }
}

/// El bit de una letra (0 a 25) o un numero (26 a 35).
fn key_bit(c: char) -> Option<u32> {
    match c.to_ascii_lowercase() {
        l @ 'a'..='z' => Some(l as u32 - 'a' as u32),
        d @ '0'..='9' => Some(26 + d as u32 - '0' as u32),
        _ => None,
    }
}

/// Un minijuego abierto.
pub trait Minigame {
    /// Avanza `dt` segundos con las teclas de este frame.
    fn update(&mut self, dt: f64, keys: &Keys);
    /// Las siguientes muestras de su audio (mono, a `audio::SAMPLE_RATE`).
    /// Su reloj sale de aqui: lo que se ve va con lo que se oye.
    fn audio(&mut self, out: &mut [f32]);
    /// Dibuja el frame sobre `rgba` (RGBA8 por filas, del tamano del render).
    fn draw(&mut self, rgba: &mut [u8]);
    /// El texto y los paneles que van encima.
    fn overlay(&self) -> Overlay;
    /// Si quien juega ya salio (Esc en el menu).
    fn done(&self) -> bool;
    /// Para el video de muestra: empieza a jugarse solo, con su piloto.
    fn demo_start(&mut self) {}
    /// Las teclas del piloto en este frame (los que tienen piloto propio no
    /// las necesitan).
    fn demo_input(&mut self, _dt: f64) -> Keys {
        Keys::default()
    }
}

/// Los minijuegos que hay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Marimba,
    Carrera,
    Shucos,
    Surfers,
}

impl Kind {
    /// Lo que dice el HUD para ofrecerlo.
    pub fn prompt(self) -> &'static str {
        match self {
            Kind::Marimba => "Enter · tocar la marimba de la Luna",
            Kind::Carrera => "Enter · correr en el Periférico",
            Kind::Shucos => "Enter · vender shucos al pie del Reformador",
            Kind::Surfers => "Enter · Talega Surfers: correr por la calle del Arco",
        }
    }
}

/// Que tan cerca del suelo de su planeta se ofrece un minijuego.
const NEAR: f64 = 70.0;

/// Hasta que altura sobre la capital se ofrece la carrera: pasando su
/// anillo de camionetas (que llega a 300).
const RING: f64 = 600.0;

/// El minijuego que se puede abrir donde esta la nave.
pub fn available(app: &App) -> Option<Kind> {
    if app.system.version != Version::Chapin {
        return None;
    }
    let near = |name: &str, height: f64| app.system.find(name).is_some_and(|i| app.flight.nearest == i && app.flight.altitude < height);
    if near("Xela", NEAR) {
        Some(Kind::Marimba)
    } else if near("Cementerio", NEAR) {
        Some(Kind::Shucos)
    } else if near("Pira", NEAR) {
        Some(Kind::Surfers)
    } else if near("Cementerio", RING) {
        Some(Kind::Carrera)
    } else {
        None
    }
}

/// Abre un minijuego. La marimba usa de fondo la ultima imagen del render;
/// la carrera hornea los modelos de las naves y lee el cielo.
pub fn start(kind: Kind, app: &App) -> Box<dyn Minigame> {
    let (width, height) = app.frame_size();
    let records = Records::load(records_path());
    match kind {
        Kind::Marimba => Box::new(marimba::MarimbaTiles::new(app.frame(), width, height, records)),
        Kind::Carrera => Box::new(race(app, records)),
        Kind::Shucos => Box::new(shucos::Shucos::new(width, height, records)),
        Kind::Surfers => Box::new(surfers::TalegaSurfers::new(width, height, records)),
    }
}

/// La carrera con las naves chapinas: la chela es la botella que se trae (o
/// la Gallo) y las demas, los tres vehiculos.
pub fn race(app: &App, records: Records) -> carrera::Carrera {
    use carrera::{Kind as Class, Model};
    use hornear::Finish;
    let ships = &app.system.chapin_ships;
    let chosen = app.chapin_ship.min(ships.len().saturating_sub(1));
    let player = Class::of_ship(chosen);
    let bottle = if player == Class::Chela { chosen } else { 0 };
    // La botella va con la boca un poco arriba (0.2 rad): de atras se le ve
    // la etiqueta y no solo el fondo. (Con 0.35 parecia una botella que se
    // cae.)
    let mut chela = app.system.meshes[ships[bottle].lods[0]].clone();
    let (sin, cos) = 0.2f32.sin_cos();
    chela.deform(|p| crate::math::Vec3::new(p.x, p.y * cos - p.z * sin, p.y * sin + p.z * cos));
    // Cuanto mide en metros una unidad de cada modelo.
    let meters = |class: Class, index: usize| class.length() / ships[index].length as f32;
    let vehicle = |class: Class, index: usize| {
        let ship = &ships[index];
        Model { mesh: &app.system.meshes[ship.lods[0]], finish: Finish::Vehicle, meters: meters(class, index), name: ship.name }
    };
    let models = [
        Model { mesh: &chela, finish: Finish::Bottle, meters: meters(Class::Chela, bottle), name: ships[bottle].name },
        vehicle(Class::TukTuk, 6),
        vehicle(Class::Camioneta, 7),
        vehicle(Class::Colectivo, 8),
    ];
    // Las latas que hacen trafico: la Four Loko y la Jager de las naves
    // botella, puestas como la chela.
    let cans: Vec<(crate::scene::Mesh, usize)> = ["Four Loko", "Jagermeister"]
        .iter()
        .filter_map(|name| ships.iter().position(|s| s.name == *name))
        .map(|i| {
            let mut mesh = app.system.meshes[ships[i].lods[0]].clone();
            mesh.deform(|p| crate::math::Vec3::new(p.x, p.y * cos - p.z * sin, p.y * sin + p.z * cos));
            (mesh, i)
        })
        .collect();
    let can_models: Vec<Model> = cans.iter().map(|(mesh, i)| Model { mesh, finish: Finish::Bottle, meters: meters(Class::Chela, *i), name: ships[*i].name }).collect();
    let sky = |dir: crate::math::Vec3| app.system.sky.sample(dir);
    let (width, height) = app.frame_size();
    carrera::Carrera::new(models, &can_models, player, &sky, width, height, records)
}

/// Donde se guardan los records: junto al programa si se corre desde la
/// carpeta del proyecto.
fn records_path() -> PathBuf {
    PathBuf::from("minijuegos.txt")
}

// El texto encima ------------------------------------------------------------

/// Tamano y peso de un texto (los estilos del HUD).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Font {
    Small,
    Body,
    Bold,
    Heading,
    /// La letra grande del titulo, a este tamano en pixeles.
    Big(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Algo que el HUD dibuja encima del minijuego. Posiciones en pixeles de la
/// pantalla.
#[derive(Clone, Debug)]
pub enum Item {
    Text {
        text: String,
        x: f32,
        y: f32,
        font: Font,
        color: [u8; 4],
        align: Align,
    },
    /// Un panel redondeado como los del HUD.
    Panel { x: f32, y: f32, w: f32, h: f32 },
    /// Una barra que se llena de abajo hacia arriba (vertical) o de
    /// izquierda a derecha.
    Bar {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: f32,
        color: [u8; 4],
        vertical: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Overlay {
    pub items: Vec<Item>,
}

impl Overlay {
    pub fn text(&mut self, text: impl Into<String>, x: f32, y: f32, font: Font, color: [u8; 4], align: Align) {
        self.items.push(Item::Text { text: text.into(), x, y, font, color, align });
    }

    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.items.push(Item::Panel { x, y, w, h });
    }

    pub fn bar(&mut self, x: f32, y: f32, w: f32, h: f32, fill: f32, color: [u8; 4], vertical: bool) {
        self.items.push(Item::Bar { x, y, w, h, fill, color, vertical });
    }
}

// Records --------------------------------------------------------------------

/// Records y ajustes de los minijuegos: lineas `nombre valor`.
#[derive(Clone, Debug, Default)]
pub struct Records {
    /// Sin archivo (en las pruebas) no se guarda nada.
    path: Option<PathBuf>,
    entries: Vec<(String, i64)>,
}

impl Records {
    pub fn load(path: PathBuf) -> Self {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let entries = text
            .lines()
            .filter_map(|line| {
                let (name, value) = line.trim().rsplit_once(' ')?;
                Some((name.trim().to_string(), value.parse().ok()?))
            })
            .collect();
        Self { path: Some(path), entries }
    }

    /// Sin archivo: para las pruebas y las capturas.
    pub fn in_memory() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<i64> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }

    /// Guarda `value` con su nombre (y escribe el archivo).
    pub fn set(&mut self, name: &str, value: i64) {
        match self.entries.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((name.to_string(), value)),
        }
        if let Some(path) = &self.path {
            let text: String = self.entries.iter().map(|(n, v)| format!("{n} {v}\n")).collect();
            if let Err(error) = std::fs::write(path, text) {
                eprintln!("no se pudieron guardar los records: {error}");
            }
        }
    }
}

/// Que todo el texto de `overlay` use letras que la fuente del HUD carga
/// (si no, salen como "?").
#[cfg(test)]
pub(crate) fn assert_loaded_glyphs(overlay: &Overlay) {
    for item in &overlay.items {
        if let Item::Text { text, .. } = item {
            for c in text.chars() {
                assert!(crate::hud::GLYPHS.contains(c), "falta {c:?} en: {text}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{FrameInput, PRESETS};

    /// Con N se llega a la marimba y ahi se ofrece; en OCASO no hay.
    #[test]
    fn the_marimba_is_offered_at_its_site_and_not_in_ocaso() {
        let mut app = App::new(64, 36).unwrap();
        let chapin = PRESETS.iter().find(|p| p.name == "40_chapin").unwrap();
        app.apply_preset(chapin);
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), None, "lejos de la luna");
        let marimba = app.system.sites.iter().position(|s| s.name == "La marimba").expect("el lugar de la luna");
        for _ in 0..=marimba {
            app.visit_next_site();
        }
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), Some(Kind::Marimba));
        app.toggle_version();
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), None, "en OCASO la luna no existe");
    }

    /// En El Reformador (en el suelo de la capital) se ofrecen los shucos; de
    /// lejos, la carrera.
    #[test]
    fn the_shucos_are_offered_at_the_reformador_and_the_race_from_afar() {
        let mut app = App::new(64, 36).unwrap();
        let chapin = PRESETS.iter().find(|p| p.name == "40_chapin").unwrap();
        app.apply_preset(chapin);
        let at = app.system.sites.iter().position(|s| s.name == "El Reformador").expect("el lugar del Reformador");
        for _ in 0..=at {
            app.visit_next_site();
        }
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), Some(Kind::Shucos));
        let capital = PRESETS.iter().find(|p| p.name == "51_capital").unwrap();
        app.apply_preset(capital);
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), Some(Kind::Carrera));
    }

    /// Con N tambien se llega a la carrera: El Periferico, en el aire sobre
    /// la capital.
    #[test]
    fn n_takes_you_to_the_race() {
        let mut app = App::new(64, 36).unwrap();
        let chapin = PRESETS.iter().find(|p| p.name == "40_chapin").unwrap();
        app.apply_preset(chapin);
        let at = app.system.sites.iter().position(|s| s.name == "El Periferico").expect("el lugar del Periferico");
        for _ in 0..=at {
            app.visit_next_site();
        }
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), Some(Kind::Carrera), "altura {}", app.flight.altitude);
    }

    /// En Antigua (con N) se ofrece Talega Surfers.
    #[test]
    fn the_surfers_are_offered_in_antigua() {
        let mut app = App::new(64, 36).unwrap();
        let chapin = PRESETS.iter().find(|p| p.name == "40_chapin").unwrap();
        app.apply_preset(chapin);
        let at = app.system.sites.iter().position(|s| s.name == "Antigua").expect("el lugar de Antigua");
        for _ in 0..=at {
            app.visit_next_site();
        }
        app.update(1.0 / 60.0, &FrameInput::default());
        assert_eq!(available(&app), Some(Kind::Surfers));
    }

    /// La carrera con las naves de verdad: cuanto cuesta cada cosa a 1280 x
    /// 720 y fotos con freno y con turbo (`OCASO_NAVE=8 OCASO_T=16
    /// OCASO_PNG=carpeta cargo test --release perfil_carrera -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore = "mide tiempos y guarda fotos"]
    fn perfil_carrera() {
        let (w, h) = (1280, 720);
        let mut app = App::new(w, h).unwrap();
        let ship: usize = std::env::var("OCASO_NAVE").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        app.chapin_ship = ship.clamp(1, 9) - 1;
        let capital = PRESETS.iter().find(|p| p.name == "51_capital").unwrap();
        app.apply_preset(capital);
        let at: f32 = std::env::var("OCASO_T").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0);
        let t = std::time::Instant::now();
        let mut game = race(&app, Records::in_memory());
        println!("horneado {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        game.set_autopilot(true);
        game.start();
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        while elapsed < at {
            game.update(dt, &Keys::default());
            elapsed += dt as f32;
        }
        // Como la foto de la loma: hasta entrar a un tunel de arcos.
        if std::env::var("OCASO_TUNEL").is_ok() {
            while elapsed < at + 20.0 && !game.in_tunnel() {
                game.update(dt, &Keys::default());
                elapsed += dt as f32;
            }
        }
        let mut rgba = vec![0u8; w * h * 4];
        game.draw(&mut rgba);
        game.bench_items(&mut rgba);
        let Ok(dir) = std::env::var("OCASO_PNG") else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let mut save = |game: &mut carrera::Carrera, name: &str| {
            game.draw(&mut rgba);
            let path = std::path::Path::new(&dir).join(format!("{name}.png"));
            crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).unwrap();
        };
        // Cuatro seguidas (las LEDs que corren), con freno y con turbo.
        for k in 0..4 {
            save(&mut game, &format!("seguidas_{k}"));
            game.update(0.1, &Keys::default());
        }
        game.force(1.0, false);
        save(&mut game, "freno");
        game.force(0.0, true);
        game.boost_rivals();
        for k in 0..3 {
            game.update(dt, &Keys::default());
            game.force(0.0, true);
            save(&mut game, &format!("turbo_{k}"));
        }
    }

    #[test]
    fn records_round_trip_through_their_file() {
        let path = std::env::temp_dir().join(format!("ocaso_records_{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut records = Records::load(path.clone());
        assert_eq!(records.get("marimba_tiple"), None);
        records.set("marimba_tiple", 41_250);
        records.set("marimba_ajuste_ms", -20);
        records.set("marimba_tiple", 52_000);
        let again = Records::load(path.clone());
        assert_eq!(again.get("marimba_tiple"), Some(52_000));
        assert_eq!(again.get("marimba_ajuste_ms"), Some(-20));
        let _ = std::fs::remove_file(&path);
    }
}
