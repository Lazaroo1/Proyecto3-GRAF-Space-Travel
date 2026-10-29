//! El reproductor de musica de la version chapina.
//!
//! Toca las canciones de la carpeta `musica` (mp3, ogg, wav, qoa, xm o mod)
//! en orden alfabetico por nombre de archivo; si hay un `primera.txt` con
//! el nombre de una cancion, empieza siempre con esa. Al final de la lista
//! esta la marimba procedural (`marimba.rs`), que tambien suena sola si la
//! carpeta esta vacia.
//!
//! raylib solo decodifica y reproduce el audio (no es parte del render).
//! Los archivos se leen con Rust y se le pasan a raylib desde memoria: en
//! Windows raylib abre los archivos con la pagina de codigos ANSI y una
//! ruta con tildes (como "Lázaro" o "Xelajú") no se abriria. Los bytes se
//! guardan mientras la cancion suena, porque raylib los va leyendo.

use std::path::{Path, PathBuf};

use raylib::prelude::*;

/// Extensiones que raylib sabe reproducir con este build.
const FORMATS: [&str; 6] = ["mp3", "ogg", "wav", "qoa", "xm", "mod"];

/// Una cancion de la lista: un archivo o la marimba procedural.
pub struct Track {
    pub name: String,
    path: Option<PathBuf>,
}

/// Lo que el HUD dibuja del reproductor.
pub struct PlayerView {
    pub title: String,
    /// Segundos sonados y largo total (0 en la marimba: no se acaba).
    pub time: f32,
    pub length: f32,
    pub playing: bool,
    pub volume: f32,
    pub index: usize,
    pub count: usize,
}

pub struct Player<'a> {
    audio: Option<&'a RaylibAudio>,
    tracks: Vec<Track>,
    current: usize,
    /// La cancion cargada. Va antes de `data` para soltarse primero.
    music: Option<Music<'a>>,
    data: Vec<u8>,
    /// Si quien juega quiere que suene (K pausa).
    playing: bool,
    /// Si se esta en la version chapina (fuera de ella todo calla).
    active: bool,
    started: bool,
    volume: f32,
    /// Segundos de marimba, para el reloj del reproductor.
    marimba_time: f32,
}

impl<'a> Player<'a> {
    pub fn new(audio: Option<&'a RaylibAudio>, dir: &Path) -> Self {
        Self {
            audio,
            tracks: playlist(dir),
            current: 0,
            music: None,
            data: Vec::new(),
            playing: true,
            active: false,
            started: false,
            volume: 0.8,
            marimba_time: 0.0,
        }
    }

    /// Entra o sale de la version chapina: la primera vez empieza la lista;
    /// despues solo se pausa y se sigue.
    pub fn set_active(&mut self, active: bool) {
        if active == self.active {
            return;
        }
        self.active = active;
        if active && !self.started {
            self.started = true;
            self.load();
        }
        self.apply();
    }

    /// Pausa o sigue (K).
    pub fn toggle(&mut self) {
        self.playing = !self.playing;
        self.apply();
    }

    pub fn next(&mut self) {
        self.current = (self.current + 1) % self.tracks.len();
        self.load();
    }

    pub fn previous(&mut self) {
        // Pasados unos segundos, primero vuelve al inicio de la misma.
        if self.music.as_ref().is_some_and(|m| m.get_time_played() > 3.0) {
            self.load();
            return;
        }
        self.current = (self.current + self.tracks.len() - 1) % self.tracks.len();
        self.load();
    }

    pub fn change_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if let Some(music) = &self.music {
            music.set_volume(self.volume);
        }
    }

    /// Salta a la fraccion `at` (0 a 1) de la cancion.
    pub fn seek(&mut self, at: f32) {
        if let Some(music) = &self.music {
            music.seek_stream(at.clamp(0.0, 0.999) * music.get_time_length());
        }
    }

    /// Cada frame: le da datos al stream y pasa a la siguiente cuando una
    /// cancion se acaba.
    pub fn update(&mut self, dt: f32) {
        if !(self.active && self.playing) {
            return;
        }
        match &self.music {
            Some(music) if music.is_stream_playing() => music.update_stream(),
            Some(_) => self.next(),
            None => self.marimba_time += dt,
        }
    }

    /// Si debe sonar la marimba procedural (la toca el sintetizador).
    pub fn marimba(&self) -> bool {
        self.active && self.playing && self.tracks[self.current].path.is_none()
    }

    pub fn view(&self) -> PlayerView {
        let (time, length) = match &self.music {
            Some(music) => (music.get_time_played(), music.get_time_length()),
            None => (self.marimba_time, 0.0),
        };
        PlayerView {
            title: self.tracks[self.current].name.clone(),
            time,
            length,
            playing: self.playing,
            volume: self.volume,
            index: self.current,
            count: self.tracks.len(),
        }
    }

    /// Carga la cancion actual y la deja sonando o en pausa. Si no se puede
    /// leer o decodificar, pasa a la siguiente (la marimba siempre carga).
    fn load(&mut self) {
        self.music = None;
        self.marimba_time = 0.0;
        let Some(path) = self.tracks[self.current].path.clone() else {
            self.data = Vec::new();
            return;
        };
        let loaded = self.audio.and_then(|audio| {
            let data = std::fs::read(&path).ok()?;
            let extension = path.extension()?.to_str()?.to_lowercase();
            let mut music = audio.new_music_from_memory(&format!(".{extension}"), &data).ok()?;
            music.set_looping(false);
            Some((music, data))
        });
        match loaded {
            Some((music, data)) => {
                self.data = data;
                music.set_volume(self.volume);
                music.play_stream();
                self.music = Some(music);
                self.apply();
            }
            None => {
                eprintln!("no se pudo reproducir {}", path.display());
                self.tracks.remove(self.current);
                self.current %= self.tracks.len();
                self.load();
            }
        }
    }

    /// Deja el stream sonando o en pausa segun `active` y `playing`.
    fn apply(&self) {
        if let Some(music) = &self.music {
            if self.active && self.playing {
                music.resume_stream();
            } else {
                music.pause_stream();
            }
        }
    }
}

/// Las canciones de `dir` en orden por nombre, la de `primera.txt` primero,
/// y al final la marimba procedural.
fn playlist(dir: &Path) -> Vec<Track> {
    let mut tracks: Vec<Track> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| FORMATS.contains(&e.to_lowercase().as_str()))
        })
        .map(|path| Track {
            name: path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
            path: Some(path),
        })
        .collect();
    tracks.sort_by_key(|t| t.name.to_lowercase());
    if let Ok(first) = std::fs::read_to_string(dir.join("primera.txt")) {
        let first = first.trim().to_lowercase();
        let matches = |t: &Track| {
            t.path.as_ref().and_then(|p| p.file_name()).is_some_and(|f| f.to_string_lossy().to_lowercase() == first)
                || t.name.to_lowercase() == first
        };
        if let Some(i) = tracks.iter().position(matches) {
            tracks.rotate_left(i);
        }
    }
    tracks.push(Track {
        name: "Marimba procedural (hecha en código)".to_string(),
        path: None,
    });
    tracks
}

/// El texto sin tildes ni enies, letra por letra: la letra de raylib solo
/// dibuja ASCII (el HUD lo usa si no encuentra Segoe UI).
pub(crate) fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'ñ' => 'n',
            'Ñ' => 'N',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_playlist_is_sorted_starts_with_the_chosen_song_and_ends_with_the_marimba() {
        let dir = std::env::temp_dir().join(format!("ocaso-musica-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.mp3", "A.ogg", "c.wav", "notas.txt", "d.m4a"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let names = |tracks: &[Track]| tracks.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        let tracks = playlist(&dir);
        assert_eq!(names(&tracks)[..3], ["A", "b", "c"]);
        assert!(tracks.last().unwrap().path.is_none());
        std::fs::write(dir.join("primera.txt"), "c.wav\n").unwrap();
        assert_eq!(names(&playlist(&dir))[..3], ["c", "A", "b"]);
        std::fs::remove_dir_all(&dir).unwrap();
        // Sin carpeta: solo la marimba.
        assert_eq!(playlist(&dir).len(), 1);
    }

    #[test]
    fn names_are_drawn_without_accents() {
        assert_eq!(ascii("Luna de Xelajú - Paco Pérez, niña"), "Luna de Xelaju - Paco Perez, nina");
    }
}
