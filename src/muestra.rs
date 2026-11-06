//! El video de muestra (`--muestra VIDEO.mp4`): la ventana de siempre, con
//! su HUD, manejada por un guion en vez del teclado. Pasa con N por cada
//! lugar de OCASO (con el crucero rodeando el planeta), echa la llamarada de
//! Faro, desata el final de la estrella, cambia a la version chapina y pasa
//! por cada lugar, abriendo y jugando (con su piloto automatico) los
//! minijuegos donde los hay.
//!
//! El tiempo avanza fijo (1/30 s por frame) aunque cada frame tarde lo que
//! tarde: el video sale en tiempo real. Cada frame ya dibujado (el render,
//! el HUD, los nombres y los minijuegos) se lee de la pantalla y va a
//! ffmpeg; el sonido (el sintetizador y el de los minijuegos) se genera con
//! el mismo reloj, se guarda y al final se junta con la imagen.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use crate::audio;
use crate::minijuegos::Kind;

/// Segundos de juego por frame del video.
pub const DT: f32 = 1.0 / 30.0;

/// Lo que el guion hace al empezar cada paso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Nada: solo dejar correr.
    Wait,
    /// N hasta el lugar con ese nombre, con el crucero encendido.
    Next(&'static str),
    /// F: la gran llamarada de Faro.
    Flare,
    /// Una vista fija (AvPag) con su momento del final: desde ahi sigue
    /// corriendo.
    View(&'static str),
    /// V: a la otra version.
    Version,
    /// Enter: abrir el minijuego del lugar (se ve su menu).
    Open(Kind),
    /// Que el minijuego empiece a jugarse solo.
    Play,
    /// Esc: cerrar el minijuego.
    Close,
}

/// El guion: cada paso, su accion y cuantos segundos dura.
fn script() -> Vec<(Action, f64)> {
    use Action::*;
    let mut steps = vec![(Wait, 6.0)];
    // OCASO: el claro, los geiseres, el nido, el espejo y el faro.
    for site in ["El claro", "Los geiseres", "El nido", "El espejo"] {
        steps.push((Next(site), 7.0));
    }
    steps.push((Next("El faro"), 3.0));
    steps.push((Flare, 9.0));
    // El final (J), con las vistas fijas que lo encuadran, cada una en su
    // momento y corriendo desde ahi: el estertor, el colapso visto desde
    // Jardin, la onda gravitacional, Gargantua comiendose a Pira y
    // digiriendola, el agujero ya solo y El Ojo.
    for (view, hold) in [
        ("25_estertor", 7.0),
        ("17_colapso", 5.0),
        ("17_onda_gravitacional", 5.0),
        ("30_comida", 7.0),
        ("31_digestion", 5.0),
        ("18_gargantua", 5.0),
        ("24_el_ojo", 6.0),
    ] {
        steps.push((View(view), hold));
    }
    // La version chapina.
    steps.push((Version, 6.0));
    // Cada lugar chapin; donde hay minijuego, se abre (su menu) y se juega.
    let sites: [(&'static str, Option<(Kind, f64)>); 8] = [
        ("Antigua", Some((Kind::Surfers, 22.0))),
        ("Tikal", None),
        ("Panajachel", None),
        ("San Andres Xecul", None),
        ("El Reformador", Some((Kind::Shucos, 24.0))),
        ("Las pozas", None),
        ("La marimba", Some((Kind::Marimba, 26.0))),
        ("El Periferico", Some((Kind::Carrera, 26.0))),
    ];
    for (site, game) in sites {
        match game {
            Some((kind, play)) => {
                steps.push((Next(site), 4.0));
                steps.push((Open(kind), 3.5));
                steps.push((Play, play));
                steps.push((Close, 1.5));
            }
            None => steps.push((Next(site), 6.0)),
        }
    }
    steps.push((Wait, 2.0));
    steps
}

pub struct Reel {
    path: PathBuf,
    video: PathBuf,
    encoder: Child,
    pipe: Option<ChildStdin>,
    width: usize,
    height: usize,
    steps: Vec<(Action, f64)>,
    step: usize,
    left: f64,
    started: bool,
    /// Segundos de video y muestras de sonido escritas.
    time: f64,
    audio: Vec<f32>,
    frames: usize,
}

impl Reel {
    pub fn new(path: &Path, width: usize, height: usize) -> Result<Self, String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Primero sin sonido; al final se le junta.
        let video = path.with_extension("imagen.mp4");
        let mut encoder = Command::new(ffmpeg())
            .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pixel_format", "rgba", "-video_size"])
            .arg(format!("{width}x{height}"))
            .args(["-framerate", "30", "-i", "-", "-an", "-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p"])
            .arg(&video)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg: {e}. Instala ffmpeg o define OCASO_FFMPEG."))?;
        let pipe = encoder.stdin.take();
        Ok(Self {
            path: path.to_path_buf(),
            video,
            encoder,
            pipe,
            width,
            height,
            steps: script(),
            step: 0,
            left: 0.0,
            started: false,
            time: 0.0,
            audio: Vec::new(),
            frames: 0,
        })
    }

    /// Avanza el guion un frame: la accion del paso que empieza (si empieza
    /// uno). `None` cuando ya se acabo.
    pub fn advance(&mut self) -> Option<Action> {
        if !self.started {
            self.started = true;
            self.left = self.steps[0].1;
            return Some(self.steps[0].0);
        }
        self.left -= DT as f64;
        if self.left > 0.0 {
            return Some(Action::Wait);
        }
        self.step += 1;
        let (action, length) = *self.steps.get(self.step)?;
        self.left = length;
        println!("muestra: {:6.1} s  {:?}", self.time, action);
        Some(action)
    }

    /// Cuantas muestras de sonido le tocan a este frame (sin acumular error).
    pub fn samples(&self) -> usize {
        let end = ((self.time + DT as f64) * audio::SAMPLE_RATE as f64).round() as usize;
        end.saturating_sub(self.audio.len())
    }

    pub fn push_audio(&mut self, samples: &[f32]) {
        self.audio.extend_from_slice(samples);
    }

    /// Lee la pantalla ya dibujada y la manda al video. Se llama antes de
    /// cerrar el dibujo del frame.
    pub fn grab(&mut self) -> Result<(), String> {
        let bytes = self.width * self.height * 4;
        let mut frame = vec![0u8; bytes];
        unsafe {
            raylib::ffi::rlDrawRenderBatchActive();
            let image = raylib::ffi::LoadImageFromScreen();
            let (w, h) = (image.width as usize, image.height as usize);
            if !image.data.is_null() && w * h * 4 >= bytes {
                // Con escala de pantalla la imagen puede venir mas grande:
                // se toma la esquina del tamano del video.
                let data = std::slice::from_raw_parts(image.data as *const u8, w * h * 4);
                for y in 0..self.height.min(h) {
                    let row = &data[y * w * 4..y * w * 4 + self.width * 4];
                    frame[y * self.width * 4..(y + 1) * self.width * 4].copy_from_slice(row);
                }
            }
            raylib::ffi::UnloadImage(image);
        }
        if let Some(pipe) = self.pipe.as_mut() {
            pipe.write_all(&frame).map_err(|e| format!("ffmpeg: {e}"))?;
        }
        self.time += DT as f64;
        self.frames += 1;
        if self.frames % 300 == 0 {
            println!("muestra: {:.0} s de video", self.time);
        }
        Ok(())
    }

    /// Cierra el video y le junta el sonido.
    pub fn finish(mut self) -> Result<(), String> {
        drop(self.pipe.take());
        let status = self.encoder.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("ffmpeg termino con {status}"));
        }
        let wav = self.path.with_extension("sonido.wav");
        write_wav(&wav, &self.audio)?;
        let status = Command::new(ffmpeg())
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&self.video)
            .arg("-i")
            .arg(&wav)
            .args(["-c:v", "copy", "-c:a", "aac", "-b:a", "160k", "-shortest", "-movflags", "+faststart"])
            .arg(&self.path)
            .status()
            .map_err(|e| format!("ffmpeg: {e}"))?;
        if !status.success() {
            return Err(format!("ffmpeg (sonido) termino con {status}"));
        }
        let _ = std::fs::remove_file(&self.video);
        let _ = std::fs::remove_file(&wav);
        println!("muestra: {} frames, {:.1} s -> {}", self.frames, self.time, self.path.display());
        Ok(())
    }
}

fn ffmpeg() -> std::ffi::OsString {
    std::env::var_os("OCASO_FFMPEG").unwrap_or_else(|| "ffmpeg".into())
}

/// WAV de 16 bits, mono.
fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let rate = audio::SAMPLE_RATE;
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}
