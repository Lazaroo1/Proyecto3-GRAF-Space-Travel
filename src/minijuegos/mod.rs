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
//!
//! Cada minijuego cumple `Minigame`: recibe las teclas del frame, llena su
//! propio audio (un stream aparte con bloques chicos, para que el golpe
//! suene enseguida), se dibuja sobre la imagen y dice que texto va encima.
//! Los records se guardan en `minijuegos.txt`, junto al programa.

pub mod lienzo;
pub mod marimba;

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
}

/// Los minijuegos que hay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Marimba,
}

impl Kind {
    /// Lo que dice el HUD para ofrecerlo.
    pub fn prompt(self) -> &'static str {
        match self {
            Kind::Marimba => "Enter · tocar la marimba de la Luna",
        }
    }
}

/// Que tan cerca del suelo de su planeta se ofrece un minijuego.
const NEAR: f64 = 70.0;

/// El minijuego que se puede abrir donde esta la nave.
pub fn available(app: &App) -> Option<Kind> {
    if app.system.version != Version::Chapin {
        return None;
    }
    let xela = app.system.find("Xela")?;
    (app.flight.nearest == xela && app.flight.altitude < NEAR).then_some(Kind::Marimba)
}

/// Abre un minijuego con `background` (la ultima imagen del render) de
/// fondo.
pub fn start(kind: Kind, background: &[u8], width: usize, height: usize) -> Box<dyn Minigame> {
    match kind {
        Kind::Marimba => Box::new(marimba::MarimbaTiles::new(background, width, height, Records::load(records_path()))),
    }
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
