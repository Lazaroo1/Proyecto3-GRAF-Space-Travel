//! El reproductor de musica de la version chapina.
//!
//! Toca las canciones de la carpeta `musica` (mp3, ogg, wav, qoa, xm o mod)
//! en orden aleatorio. Si hay un `primera.txt` con el nombre de una cancion
//! (o un pedazo, como "no se acaba el amor"), la primera vuelta empieza
//! siempre con esa; despues van todas revueltas y, al terminarse, se
//! vuelven a revolver (sin repetir de entrada la ultima que sono). Al final
//! de cada vuelta esta la marimba procedural (`marimba.rs`): se llega a ella
//! con U / I, pero sola no entra al shuffle porque no se acaba nunca; si la
//! carpeta esta vacia suena ella.
//!
//! **Crossfade**: cuando a una cancion le quedan `CROSSFADE` segundos
//! empieza la siguiente; una baja mientras la otra sube, con curvas de
//! potencia constante (seno y coseno) para que en medio no haya un hueco de
//! volumen. Al saltar a mano el cambio es corto (`SKIP_FADE`).
//!
//! raylib solo decodifica y reproduce el audio (no es parte del render).
//! Los archivos se leen con Rust y se le pasan a raylib desde memoria: en
//! Windows raylib abre los archivos con la pagina de codigos ANSI y una
//! ruta con tildes (como "Lázaro" o "Xelajú") no se abriria. Los bytes se
//! guardan mientras la cancion suena, porque raylib los va leyendo.

use std::f32::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};

use raylib::prelude::*;

use crate::math::hash_u32;

/// Extensiones que raylib sabe reproducir con este build.
const FORMATS: [&str; 6] = ["mp3", "ogg", "wav", "qoa", "xm", "mod"];
/// Segundos que se enciman dos canciones al pasar de una a la otra.
const CROSSFADE: f32 = 3.5;
/// Lo mismo al saltar con U / I o con los botones.
const SKIP_FADE: f32 = 0.6;

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

/// Una cancion cargada. `music` va antes de `_data` para soltarse primero:
/// raylib lee de esos bytes mientras suena (solo se guardan).
struct Song<'a> {
    music: Music<'a>,
    _data: Vec<u8>,
    /// Segundos que tarda en subir a su volumen (0: entra completa) y los
    /// que lleva sonando.
    fade_in: f32,
    age: f32,
}

/// La cancion que se va: baja en `total` segundos y le quedan `left`.
struct Leaving<'a> {
    song: Song<'a>,
    left: f32,
    total: f32,
}

pub struct Player<'a> {
    audio: Option<&'a RaylibAudio>,
    /// Las canciones por nombre y, al final, la marimba.
    tracks: Vec<Track>,
    /// El orden de esta vuelta (indices de `tracks`, la marimba al final) y
    /// donde va.
    order: Vec<usize>,
    position: usize,
    /// Cuantas vueltas van (cambia el shuffle).
    round: u32,
    seed: u32,
    song: Option<Song<'a>>,
    leaving: Option<Leaving<'a>>,
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
        let tracks = playlist(dir);
        let first = first_song(dir, &tracks);
        // Una semilla distinta cada vez que se abre el juego.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let seed = hash_u32(now.subsec_nanos() ^ hash_u32(now.as_secs() as u32));
        Self {
            order: shuffle(&songs(&tracks), tracks.len() - 1, first, None, seed),
            audio,
            tracks,
            position: 0,
            round: 0,
            seed,
            song: None,
            leaving: None,
            playing: true,
            active: false,
            started: false,
            volume: 0.8,
            marimba_time: 0.0,
        }
    }

    fn current(&self) -> usize {
        self.order[self.position]
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
            self.load(0.0);
        }
        self.apply();
    }

    /// Pausa o sigue (K).
    pub fn toggle(&mut self) {
        self.playing = !self.playing;
        self.apply();
    }

    /// La siguiente (I): tambien llega a la marimba.
    pub fn next(&mut self) {
        self.advance(true, SKIP_FADE);
    }

    pub fn previous(&mut self) {
        // Pasados unos segundos, primero vuelve al inicio de la misma.
        if self.song.as_ref().is_some_and(|s| s.music.get_time_played() > 3.0) {
            self.song = None;
            self.load(0.0);
            return;
        }
        self.fade_out(SKIP_FADE);
        self.position = (self.position + self.order.len() - 1) % self.order.len();
        self.load(SKIP_FADE);
    }

    pub fn change_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        self.set_volumes();
    }

    /// Salta a la fraccion `at` (0 a 1) de la cancion.
    pub fn seek(&mut self, at: f32) {
        if let Some(song) = &self.song {
            song.music.seek_stream(at.clamp(0.0, 0.999) * song.music.get_time_length());
        }
    }

    /// Cada frame: le da datos a los streams, sube y baja los volumenes del
    /// crossfade y, cuando a una cancion le queda poco, empieza la otra.
    pub fn update(&mut self, dt: f32) {
        if !(self.active && self.playing) {
            return;
        }
        if let Some(leaving) = &mut self.leaving {
            leaving.song.music.update_stream();
            leaving.left -= dt;
            if leaving.left <= 0.0 || !leaving.song.music.is_stream_playing() {
                self.leaving = None;
            }
        }
        let mut next = None;
        match &mut self.song {
            Some(song) => {
                song.music.update_stream();
                song.age += dt;
                let length = song.music.get_time_length();
                let left = length - song.music.get_time_played();
                if !song.music.is_stream_playing() {
                    next = Some(0.0);
                } else if self.leaving.is_none() && length > 2.0 * CROSSFADE && left <= CROSSFADE {
                    next = Some(left.max(0.5));
                }
            }
            None => self.marimba_time += dt,
        }
        if let Some(fade) = next {
            self.advance(false, fade);
        }
        self.set_volumes();
    }

    /// Si debe sonar la marimba procedural (la toca el sintetizador).
    pub fn marimba(&self) -> bool {
        self.active && self.playing && self.tracks[self.current()].path.is_none()
    }

    pub fn view(&self) -> PlayerView {
        let (time, length) = match &self.song {
            Some(song) => (song.music.get_time_played(), song.music.get_time_length()),
            None => (self.marimba_time, 0.0),
        };
        PlayerView {
            title: self.tracks[self.current()].name.clone(),
            time,
            length,
            playing: self.playing,
            volume: self.volume,
            index: self.position,
            count: self.order.len(),
        }
    }

    /// Pasa a la que sigue, con un crossfade de `fade` segundos. Sola no
    /// entra a la marimba: al llegar al final de la vuelta se revuelve otra.
    /// A mano (`by_hand`) si llega a ella.
    fn advance(&mut self, by_hand: bool, fade: f32) {
        self.fade_out(fade);
        self.position += 1;
        let at_end = self.position >= self.order.len();
        let at_marimba = !at_end && self.tracks[self.current()].path.is_none();
        if at_end || (at_marimba && !by_hand && self.order.len() > 1) {
            let last = self.order.get(self.position.wrapping_sub(1)).copied();
            self.round += 1;
            let marimba = self.tracks.len() - 1;
            self.order = shuffle(&songs(&self.tracks), marimba, None, last, hash_u32(self.seed ^ self.round));
            self.position = 0;
        }
        self.load(fade);
    }

    /// La cancion que suena se va bajando en `fade` segundos (o se corta).
    fn fade_out(&mut self, fade: f32) {
        self.leaving = match self.song.take() {
            Some(song) if fade > 0.0 => Some(Leaving { song, left: fade, total: fade }),
            _ => None,
        };
    }

    /// Carga la cancion actual, que sube en `fade` segundos, y la deja
    /// sonando o en pausa. Si no se puede leer o decodificar, pasa a la
    /// siguiente (la marimba siempre carga).
    fn load(&mut self, fade: f32) {
        self.marimba_time = 0.0;
        let Some(path) = self.tracks[self.current()].path.clone() else {
            self.song = None;
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
                music.play_stream();
                self.song = Some(Song { music, _data: data, fade_in: fade, age: 0.0 });
                self.set_volumes();
                self.apply();
            }
            None => {
                eprintln!("no se pudo reproducir {}", path.display());
                // Fuera de la lista y de la vuelta.
                let broken = self.current();
                self.tracks[broken].path = None;
                self.tracks[broken].name = String::new();
                self.order.remove(self.position);
                self.position %= self.order.len();
                self.load(fade);
            }
        }
    }

    /// El volumen de cada stream: el del reproductor por su rampa.
    fn set_volumes(&self) {
        if let Some(song) = &self.song {
            let rise = if song.fade_in > 0.0 { (song.age / song.fade_in).min(1.0) } else { 1.0 };
            song.music.set_volume(self.volume * (rise * FRAC_PI_2).sin());
        }
        if let Some(leaving) = &self.leaving {
            let fall = (leaving.left / leaving.total).clamp(0.0, 1.0);
            leaving.song.music.set_volume(self.volume * ((1.0 - fall) * FRAC_PI_2).cos());
        }
    }

    /// Deja los streams sonando o en pausa segun `active` y `playing`.
    fn apply(&self) {
        let streams = self.song.iter().chain(self.leaving.iter().map(|l| &l.song));
        for song in streams {
            if self.active && self.playing {
                song.music.resume_stream();
            } else {
                song.music.pause_stream();
            }
        }
    }
}

/// Las canciones de `dir` en orden por nombre y, al final, la marimba
/// procedural.
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
    tracks.push(Track {
        name: "Marimba procedural (hecha en código)".to_string(),
        path: None,
    });
    tracks
}

/// Las canciones que se pueden tocar (las que tienen archivo).
fn songs(tracks: &[Track]) -> Vec<usize> {
    (0..tracks.len()).filter(|&i| tracks[i].path.is_some()).collect()
}

/// La cancion de `primera.txt`: el nombre del archivo, su nombre sin la
/// extension o un pedazo de el (sin importar mayusculas ni tildes).
fn first_song(dir: &Path, tracks: &[Track]) -> Option<usize> {
    let wanted = std::fs::read_to_string(dir.join("primera.txt")).ok()?;
    let plain = |s: &str| ascii(s).to_lowercase();
    let wanted = plain(wanted.trim());
    if wanted.is_empty() {
        return None;
    }
    let songs = || tracks.iter().enumerate().filter(|(_, t)| t.path.is_some());
    let file = |t: &Track| t.path.as_ref().and_then(|p| p.file_name()).map(|f| plain(&f.to_string_lossy())).unwrap_or_default();
    songs()
        .find(|(_, t)| file(t) == wanted || plain(&t.name) == wanted)
        .or_else(|| songs().find(|(_, t)| plain(&t.name).contains(&wanted)))
        .map(|(i, _)| i)
}

/// El orden de una vuelta: las canciones `songs` revueltas (Fisher-Yates
/// con la semilla `seed`) y al final la marimba. Si hay `first`, empieza
/// con ella; si no, no empieza con `avoid` (la que acaba de sonar).
fn shuffle(songs: &[usize], marimba: usize, first: Option<usize>, avoid: Option<usize>, seed: u32) -> Vec<usize> {
    let mut songs: Vec<usize> = songs.iter().copied().filter(|&i| Some(i) != first).collect();
    let mut h = seed;
    for i in (1..songs.len()).rev() {
        h = hash_u32(h ^ i as u32);
        songs.swap(i, h as usize % (i + 1));
    }
    if songs.len() > 1 && songs.first() == avoid.as_ref() {
        let last = songs.len() - 1;
        songs.swap(0, last);
    }
    first.into_iter().chain(songs).chain([marimba]).collect()
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
    fn the_playlist_is_sorted_finds_the_chosen_song_and_ends_with_the_marimba() {
        let dir = std::env::temp_dir().join(format!("ocaso-musica-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.mp3", "A.ogg", "c.wav", "notas.txt", "d.m4a", "Grupo - No Se Acaba El Amor.mp3"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let names = |tracks: &[Track]| tracks.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        let tracks = playlist(&dir);
        assert_eq!(names(&tracks)[..4], ["A", "b", "c", "Grupo - No Se Acaba El Amor"]);
        assert!(tracks.last().unwrap().path.is_none());
        assert_eq!(first_song(&dir, &tracks), None);
        std::fs::write(dir.join("primera.txt"), "c.wav\n").unwrap();
        assert_eq!(first_song(&dir, &tracks), Some(2));
        // Basta un pedazo del nombre, sin importar mayusculas.
        std::fs::write(dir.join("primera.txt"), "no se acaba el amor").unwrap();
        assert_eq!(first_song(&dir, &tracks), Some(3));
        std::fs::remove_dir_all(&dir).unwrap();
        // Sin carpeta: solo la marimba.
        assert_eq!(playlist(&dir).len(), 1);
    }

    #[test]
    fn every_round_has_each_song_once_and_the_marimba_last() {
        // Ocho canciones y la marimba (el 8).
        let songs: Vec<usize> = (0..8).collect();
        for seed in 0..50 {
            let order = shuffle(&songs, 8, Some(5), None, seed);
            assert_eq!(order[0], 5);
            assert_eq!(*order.last().unwrap(), 8);
            let mut sorted = order.clone();
            sorted.sort();
            assert_eq!(sorted, (0..9).collect::<Vec<_>>());
            // Las vueltas siguientes no empiezan con la que acaba de sonar.
            assert_ne!(shuffle(&songs, 8, None, Some(3), seed)[0], 3);
        }
        // Revuelve de verdad: con otra semilla, otro orden.
        let orders: std::collections::HashSet<_> = (0..20).map(|seed| shuffle(&songs, 8, None, None, seed)).collect();
        assert!(orders.len() > 15);
        // Una que no se pudo tocar ya no entra.
        assert!(!shuffle(&[0, 1, 3], 4, None, None, 9).contains(&2));
        // Sin canciones queda la marimba sola.
        assert_eq!(shuffle(&[], 0, None, None, 7), [0]);
    }

    #[test]
    fn names_are_drawn_without_accents() {
        assert_eq!(ascii("Luna de Xelajú - Paco Pérez, niña"), "Luna de Xelaju - Paco Perez, nina");
    }
}
