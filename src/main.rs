//! OCASO: un sistema solar explorable con un renderer por software propio.
//!
//! raylib solo abre la ventana, lee el teclado y el mouse, y muestra la imagen
//! final como una textura 2D. Todo el 3D (transformaciones, recorte, raster,
//! profundidad y shaders) se calcula en CPU en `render` y `shaders`.

mod app;
mod audio;
mod chapin;
mod config;
mod gargantua_p2;
mod hud;
mod marimba;
#[allow(dead_code)]
mod math;
mod minijuegos;
mod muestra;
mod parallel;
mod player;
mod render;
mod scene;
mod shaders;
mod terrain;
mod tweaks;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use raylib::prelude::*;

use app::{App, FrameInput, PRESETS};
use math::DVec3;

/// Muestras por bloque de audio (unos 46 ms a 44.1 kHz).
const AUDIO_BLOCK: usize = 2048;
/// Los minijuegos tienen su propio stream con bloques chicos (unos 12 ms):
/// el golpe de una tecla tiene que sonar enseguida. Mientras se juega los
/// frames son rapidos (el mundo esta congelado) y alcanzan a llenarlos.
const GAME_BLOCK: usize = 512;

enum Mode {
    Window,
    Capture(PathBuf),
    Bench(usize),
    Tour(PathBuf),
    /// Fotos del minijuego de la marimba, sin ventana.
    GameCapture(PathBuf),
    /// Fotos de la carrera del Periferico, sin ventana.
    RaceCapture(PathBuf),
    /// Fotos de los shucos del Reformador, sin ventana.
    ShucosCapture(PathBuf),
    /// Fotos de Talega Surfers, sin ventana.
    SurfersCapture(PathBuf),
}

struct Options {
    width: usize,
    height: usize,
    mode: Mode,
    wireframe: bool,
    lod_colors: bool,
    fog_only: bool,
    preset: Option<usize>,
    /// Abrir la marimba de la Luna (o la carrera) al arrancar.
    marimba: bool,
    race: bool,
    shucos: bool,
    surfers: bool,
    /// La nave chapina con que se empieza (1 a 9).
    ship: Option<usize>,
    /// Empezar (o grabar el recorrido) en la version chapina.
    chapina: bool,
    /// Grabar el video de muestra (la ventana manejada por un guion).
    reel: Option<PathBuf>,
    /// Abrir en pantalla completa (como apachar F11).
    fullscreen: bool,
}

const USAGE: &str = "\
uso: ocaso [opciones]
  --res ANCHOxALTO      resolucion del render (por defecto 1280x720)
  --pantalla-completa   abre en pantalla completa (F11 cambia); el render
                        sigue a su resolucion y se estira al mostrarlo
  --captura CARPETA     guarda vistas fijas como PNG sin abrir ventana
  --vista NOMBRE       inicia o captura solo la vista elegida
  --recorrido MP4      graba vuelo, acercamiento, vuelta y alejamiento (ffmpeg)
  --chapina             empieza (o graba el recorrido) en la version chapina
  --muestra MP4         graba la ventana con un guion: cada lugar con N en las
                        dos versiones y los minijuegos jugandose solos (ffmpeg)
  --marimba             arranca en la Luna de Xelaju con la marimba abierta
  --carrera             arranca en la capital con la carrera del Periferico abierta
  --shucos              arranca en El Reformador con la carretilla de shucos abierta
  --surfers             arranca en Antigua con Talega Surfers abierta
  --nave N              la nave de la version chapina (1-6 botellas, 7 tuk-tuk,
                        8 camioneta, 9 colectivo)
  --captura-marimba DIR guarda fotos del minijuego de la marimba sin ventana
  --captura-carrera DIR guarda fotos de la carrera del Periferico sin ventana
  --captura-shucos DIR  guarda fotos de los shucos del Reformador sin ventana
  --captura-surfers DIR guarda fotos de Talega Surfers sin ventana
  --bench FRAMES        mide cada vista fija FRAMES veces sin abrir ventana
  --wire                empieza con el wireframe encendido (F3)
  --lod                 empieza con los colores de LOD encendidos (F4)
  --niebla              muestra solo la luz de la atmosfera en las superficies";

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        width: config::DEFAULT_WIDTH,
        height: config::DEFAULT_HEIGHT,
        mode: Mode::Window,
        wireframe: false,
        lod_colors: false,
        fog_only: false,
        preset: None,
        marimba: false,
        race: false,
        shucos: false,
        surfers: false,
        ship: None,
        chapina: false,
        reel: None,
        fullscreen: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--res" => {
                let value = args.next().ok_or("--res espera ANCHOxALTO")?;
                let (w, h) = value.split_once('x').ok_or("--res espera ANCHOxALTO")?;
                options.width = w.parse().map_err(|_| "ancho invalido")?;
                options.height = h.parse().map_err(|_| "alto invalido")?;
                if options.width < 64 || options.height < 64 {
                    return Err("la resolucion minima es 64x64".into());
                }
            }
            "--captura" => {
                let dir = args.next().ok_or("--captura espera una carpeta")?;
                options.mode = Mode::Capture(PathBuf::from(dir));
            }
            "--chapina" => options.chapina = true,
            "--pantalla-completa" => options.fullscreen = true,
            "--muestra" => {
                options.reel = Some(PathBuf::from(args.next().ok_or("--muestra espera un archivo .mp4")?));
            }
            "--recorrido" => {
                options.mode = Mode::Tour(PathBuf::from(
                    args.next().ok_or("--recorrido espera un archivo .mp4")?,
                ));
            }
            "--bench" => {
                let frames = args.next().ok_or("--bench espera un numero de frames")?;
                let frames = frames.parse().map_err(|_| "numero de frames invalido")?;
                options.mode = Mode::Bench(frames);
            }
            "--wire" => options.wireframe = true,
            "--marimba" => options.marimba = true,
            "--carrera" => options.race = true,
            "--shucos" => options.shucos = true,
            "--surfers" => options.surfers = true,
            "--nave" => {
                let n: usize = args.next().ok_or("--nave espera un numero")?.parse().map_err(|_| "nave invalida")?;
                if !(1..=9).contains(&n) {
                    return Err("--nave va de 1 a 9".into());
                }
                options.ship = Some(n - 1);
            }
            "--captura-carrera" => {
                let dir = args.next().ok_or("--captura-carrera espera una carpeta")?;
                options.mode = Mode::RaceCapture(PathBuf::from(dir));
            }
            "--captura-surfers" => {
                let dir = args.next().ok_or("--captura-surfers espera una carpeta")?;
                options.mode = Mode::SurfersCapture(PathBuf::from(dir));
            }
            "--captura-shucos" => {
                let dir = args.next().ok_or("--captura-shucos espera una carpeta")?;
                options.mode = Mode::ShucosCapture(PathBuf::from(dir));
            }
            "--captura-marimba" => {
                let dir = args.next().ok_or("--captura-marimba espera una carpeta")?;
                options.mode = Mode::GameCapture(PathBuf::from(dir));
            }
            "--vista" => {
                let name = args.next().ok_or("--vista espera el nombre de una vista")?;
                options.preset = Some(
                    PRESETS
                        .iter()
                        .position(|p| p.name == name)
                        .ok_or_else(|| format!("vista desconocida: {name}"))?,
                );
            }
            "--lod" => options.lod_colors = true,
            "--niebla" => options.fog_only = true,
            "-h" | "--help" | "--ayuda" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    Ok(options)
}

fn main() {
    // El hilo principal tambien trabaja en las pasadas paralelas.
    parallel::flush_denormals();
    let options = match parse_args() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let result = match &options.mode {
        Mode::Window => run_window(&options),
        Mode::Capture(dir) => run_capture(&options, dir),
        Mode::Bench(frames) => run_bench(&options, *frames),
        Mode::Tour(path) => run_tour(&options, path),
        Mode::GameCapture(dir) => run_game_capture(&options, dir),
        Mode::RaceCapture(dir) => run_race_capture(&options, dir),
        Mode::ShucosCapture(dir) => run_shucos_capture(&options, dir),
        Mode::SurfersCapture(dir) => run_surfers_capture(&options, dir),
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

/// Crea el juego con las vistas de depuracion pedidas en la linea de comandos.
fn new_app(options: &Options) -> Result<App, String> {
    let mut app = App::new(options.width, options.height)?;
    app.debug.wireframe = options.wireframe;
    app.debug.lod_colors = options.lod_colors;
    app.debug.fog_only = options.fog_only;
    if let Some(ship) = options.ship {
        // Se pone al entrar a la version chapina.
        app.chapin_ship = ship;
    }
    if options.chapina {
        app.use_chapin();
    }
    if let Some(i) = options.preset {
        app.apply_preset(&PRESETS[i]);
    }
    Ok(app)
}

fn run_window(options: &Options) -> Result<(), String> {
    let mut app = new_app(options)?;
    // El video de muestra: la ventana la maneja un guion, a tiempo fijo.
    let mut reel = match &options.reel {
        Some(path) => Some(muestra::Reel::new(path, options.width, options.height)?),
        None => None,
    };
    // En la ventana, la resolucion dinamica (F8 la apaga); en el video,
    // siempre completa.
    app.dynamic.enabled = reel.is_none();
    if options.marimba {
        app.apply_preset(xela_preset());
    } else if options.race {
        app.apply_preset(capital_preset());
    } else if options.shucos {
        app.apply_preset(reformador_preset());
    } else if options.surfers {
        app.apply_preset(PRESETS.iter().find(|p| p.name == "42_antigua").expect("la vista de Antigua"));
    } else if options.chapina && options.preset.is_none() {
        app.apply_preset(PRESETS.iter().find(|p| p.name == "40_chapin").expect("la vista chapina"));
    } else if options.preset.is_none() {
        app.apply_preset(&PRESETS[21]);
    }
    let (mut rl, thread) = raylib::init()
        .size(options.width as i32, options.height as i32)
        .title("OCASO - software renderer")
        .build();
    rl.set_target_fps(144);
    rl.disable_cursor();
    // Esc cierra el juego solo fuera de un minijuego (adentro, sale de el).
    rl.set_exit_key(None);
    let mut captured = true;
    // Al capturar el mouse, el primer movimiento trae el salto desde donde
    // estaba el cursor: se descarta para que la mira no pegue un brinco.
    let mut skip_mouse = true;

    // La textura solo transporta la imagen ya calculada en CPU a la pantalla.
    let image = Image::gen_image_color(options.width as i32, options.height as i32, Color::BLACK);
    let mut texture = rl
        .load_texture_from_image(&thread, &image)
        .map_err(|e| format!("no se pudo crear la textura: {e:?}"))?;
    // Las letras del HUD, a la escala con que se ve la imagen (se vuelven a
    // cargar si cambia: F11), y lo que el HUD hornea en CPU.
    let first_fit = hud::Fit::new((options.width, options.height), (rl.get_screen_width(), rl.get_screen_height()));
    let mut fonts = hud::Fonts::load(&mut rl, &thread, hud::ui_scale(options.height, first_fit));
    let art = hud::Art::new();
    // Pantalla completa (F11): la ventana sin bordes del tamano del monitor,
    // sin cambiarle la resolucion. El render no cambia de tamano (el CPU
    // hace lo mismo): la textura se estira con filtro bilineal al mostrarla
    // y el HUD crece con ella. El video de muestra graba la ventana, asi
    // que ahi no se usa.
    let mut smooth = false;
    if options.fullscreen && reel.is_none() {
        rl.toggle_borderless_windowed();
    }
    let mut hud = hud::Hud {
        help: false,
        stats: false,
        age: 0.0,
        hidden: false,
        preset: None,
        banner: None,
        banner_where: None,
        intro_over: false,
        prompt: None,
    };
    let mut preset_index: Option<usize> = None;
    let mut tweaks = tweaks::Tweaks::default();

    // Sonido: raylib solo reproduce lo que el sintetizador genera. Si no hay
    // dispositivo de audio, el juego sigue sin sonido.
    // El video genera su sonido aparte: sin dispositivo, no suena dos veces.
    let audio_device = if reel.is_none() { RaylibAudio::init_audio_device().ok() } else { None };

    let mut stream = audio_device.as_ref().map(|device| {
        device.set_audio_stream_buffer_size_default(AUDIO_BLOCK as i32);
        let stream = device.new_audio_stream(audio::SAMPLE_RATE, 32, 1);
        stream.play();
        stream
    });
    let mut synth = audio::Synth::new();
    let mut game_stream = audio_device.as_ref().map(|device| {
        device.set_audio_stream_buffer_size_default(GAME_BLOCK as i32);
        let stream = device.new_audio_stream(audio::SAMPLE_RATE, 32, 1);
        // Las canciones del reproductor se crean despues con el tamano por
        // defecto: con bloques chicos se cortarian en los frames lentos.
        device.set_audio_stream_buffer_size_default(AUDIO_BLOCK as i32);
        stream.play();
        stream
    });
    let mut game_block = vec![0.0f32; GAME_BLOCK];
    let mut minigame: Option<Box<dyn minijuegos::Minigame>> = None;
    let mut opening: Option<minijuegos::Kind> = None;
    let mut game_frame = vec![0u8; options.width * options.height * 4];
    if options.marimba || options.race || options.shucos || options.surfers {
        app.render();
        let kind = if options.race {
            minijuegos::Kind::Carrera
        } else if options.shucos {
            minijuegos::Kind::Shucos
        } else if options.surfers {
            minijuegos::Kind::Surfers
        } else {
            minijuegos::Kind::Marimba
        };
        minigame = Some(minijuegos::start(kind, &app));
        hud.intro_over = true;
    }
    // Las canciones de la version chapina.
    let mut player = player::Player::new(audio_device.as_ref(), &music_dir());
    let mut block = vec![0.0f32; AUDIO_BLOCK];

    while !rl.window_should_close() {
        let dt = if reel.is_some() { muestra::DT } else { rl.get_frame_time() };
        if reel.is_none() && rl.is_key_pressed(KeyboardKey::KEY_F11) {
            rl.toggle_borderless_windowed();
            if captured {
                rl.disable_cursor();
            }
            // Al cambiar la ventana, el mouse trae un salto.
            skip_mouse = true;
        }
        let fit = hud::Fit::new((options.width, options.height), (rl.get_screen_width(), rl.get_screen_height()));
        if (fit.scale != 1.0) != smooth {
            smooth = fit.scale != 1.0;
            let filter = if smooth { TextureFilter::TEXTURE_FILTER_BILINEAR } else { TextureFilter::TEXTURE_FILTER_POINT };
            texture.set_texture_filter(&thread, filter);
        }
        let scale = hud::ui_scale(options.height, fit);
        if scale != fonts.scale() {
            fonts = hud::Fonts::load(&mut rl, &thread, scale);
        }
        art.trim();
        let ui = hud::Ui { fonts: &fonts, art: &art, thread: &thread, fit };
        if let Some(r) = reel.as_mut() {
            let Some(action) = r.advance() else { break };
            use muestra::Action;
            match action {
                Action::Wait => {}
                Action::Next(site) => {
                    if !app.visit_site(site) {
                        println!("muestra: no hay un lugar \"{site}\"");
                    }
                    app.settings.cruise = true;
                }
                Action::Flare => app.press_flare(),
                Action::View(name) => match PRESETS.iter().find(|p| p.name == name) {
                    Some(preset) => {
                        app.apply_preset(preset);
                        app.settings.cruise = false;
                    }
                    None => println!("muestra: no hay una vista \"{name}\""),
                },
                Action::Version => {
                    app.toggle_version();
                    hud.intro_over = true;
                }
                Action::Open(kind) => {
                    if minijuegos::available(&app) != Some(kind) {
                        println!("muestra: aqui no se ofrece {kind:?}");
                    }
                    opening = Some(kind);
                    hud.prompt = None;
                    if kind == minijuegos::Kind::Carrera {
                        hud.banner = Some(("Horneando las naves…", hud.age - 0.3));
                        hud.banner_where = None;
                    }
                }
                Action::Play => {
                    if let Some(game) = minigame.as_mut() {
                        game.demo_start();
                    }
                }
                Action::Close => {
                    minigame = None;
                    skip_mouse = true;
                }
            }
            if let Some(text) = app.banner.take() {
                hud.banner = Some((text, hud.age));
                hud.banner_where = app.banner_where.take();
            }
        }
        // Un minijuego abierto: el mundo se queda quieto y el juego tiene la
        // pantalla, el teclado y su propio audio.
        if let Some(game) = minigame.as_mut() {
            hud.age += dt;
            let keys = if reel.is_some() { game.demo_input(dt as f64) } else { read_game_keys(&rl) };
            game.update(dt as f64, &keys);
            // En el video, el sonido del juego de este frame (se manda abajo,
            // con la marimba si le toca).
            let mut reel_samples = None;
            match game_stream.as_mut() {
                _ if reel.is_some() => {
                    let mut samples = vec![0.0f32; reel.as_ref().unwrap().samples()];
                    game.audio(&mut samples);
                    reel_samples = Some(samples);
                }
                Some(stream) => {
                    while stream.is_processed() {
                        game.audio(&mut game_block);
                        let _ = stream.update(&game_block);
                    }
                }
                // Sin audio, su reloj igual tiene que avanzar.
                None => game.audio(&mut vec![0.0; (dt * audio::SAMPLE_RATE as f32) as usize]),
            }
            // La carrera se corre con la musica del vuelo: el reproductor
            // sigue, con sus teclas, y si le toca a la marimba procedural el
            // sintetizador solo toca esa. Los demas tienen su propia musica.
            let music = game.keeps_music() && app.system.version == scene::Version::Chapin;
            player.set_active(music);
            if music && reel.is_none() {
                if rl.is_key_pressed(KeyboardKey::KEY_K) {
                    player.toggle();
                }
                if rl.is_key_pressed(KeyboardKey::KEY_I) {
                    player.next();
                }
                if rl.is_key_pressed(KeyboardKey::KEY_U) {
                    player.previous();
                }
                if rl.is_key_pressed(KeyboardKey::KEY_MINUS) || rl.is_key_pressed(KeyboardKey::KEY_KP_SUBTRACT) {
                    player.change_volume(-0.1);
                }
                if rl.is_key_pressed(KeyboardKey::KEY_EQUAL) || rl.is_key_pressed(KeyboardKey::KEY_KP_ADD) {
                    player.change_volume(0.1);
                }
            }
            player.update(dt);
            let marimba = music && player.marimba();
            let master = if marimba && !app.muted { 0.8 } else { 0.0 };
            let level = if marimba { 0.55 } else { 0.0 };
            if let Some(stream) = stream.as_mut() {
                synth.set(audio::AudioParams { master, marimba: level, ..audio::AudioParams::default() });
                while stream.is_processed() {
                    synth.fill(&mut block);
                    let _ = stream.update(&block);
                }
            }
            // En el video no hay canciones (no hay dispositivo de audio): en
            // la carrera suena la marimba, como cuando le toca en el juego.
            if let (Some(r), Some(mut samples)) = (reel.as_mut(), reel_samples) {
                if marimba {
                    synth.set(audio::AudioParams { master, marimba: level, ..audio::AudioParams::default() });
                    let mut music = vec![0.0f32; samples.len()];
                    synth.fill(&mut music);
                    for (s, m) in samples.iter_mut().zip(&music) {
                        *s += m;
                    }
                }
                r.push_audio(&samples);
            }
            game.draw(&mut game_frame);
            texture
                .update_texture(&game_frame)
                .map_err(|e| format!("no se pudo actualizar la textura: {e:?}"))?;
            let overlay = game.overlay();
            let done = game.done();
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::BLACK);
            hud::frame(&mut d, &texture, fit);
            hud::minigame(&mut d, &ui, &overlay);
            if let Some(r) = reel.as_mut() {
                r.grab()?;
            }
            drop(d);
            if done {
                minigame = None;
                skip_mouse = true;
            }
            continue;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            break;
        }
        // Un minijuego pedido el frame pasado: ya se vio el aviso, se abre
        // (la carrera tarda un par de segundos en hornear las naves).
        if let Some(kind) = opening.take() {
            // Mientras se hornean las naves, la musica sigue sonando.
            let mut last = std::time::Instant::now();
            minigame = Some(minijuegos::start_while(kind, &app, || {
                let now = std::time::Instant::now();
                player.update((now - last).as_secs_f32());
                last = now;
            }));
            continue;
        }
        hud.prompt = minijuegos::available(&app).map(minijuegos::Kind::prompt);
        if rl.is_key_pressed(KeyboardKey::KEY_ENTER) || rl.is_key_pressed(KeyboardKey::KEY_KP_ENTER) {
            if let Some(kind) = minijuegos::available(&app) {
                opening = Some(kind);
                hud.prompt = None;
                if kind == minijuegos::Kind::Carrera {
                    // Ya visible en este frame (los avisos entran en 0.15 s).
                    hud.banner = Some(("Horneando las naves…", hud.age - 0.3));
                    hud.banner_where = None;
                }
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_TAB) {
            captured = !captured;
            if captured {
                rl.disable_cursor();
                skip_mouse = true;
            } else {
                rl.enable_cursor();
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            hud.help = !hud.help;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F2) {
            hud.stats = !hud.stats;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F9) {
            hud.hidden = !hud.hidden;
        }
        // Vistas fijas en orden, para mostrar algo rapido en la demostracion.
        // En las laptops sin RePag/AvPag, la coma y el punto.
        let step = if rl.is_key_pressed(KeyboardKey::KEY_PAGE_DOWN) || rl.is_key_pressed(KeyboardKey::KEY_PERIOD) {
            1
        } else if rl.is_key_pressed(KeyboardKey::KEY_PAGE_UP) || rl.is_key_pressed(KeyboardKey::KEY_COMMA) {
            -1
        } else {
            0
        };
        if step != 0 {
            let count = PRESETS.len() as i32;
            let next = match preset_index {
                Some(i) => (i as i32 + step).rem_euclid(count),
                None if step > 0 => 0,
                None => count - 1,
            } as usize;
            preset_index = Some(next);
            app.apply_preset(&PRESETS[next]);
            hud.preset = Some((PRESETS[next].name, hud.age));
        }
        if rl.is_key_pressed(KeyboardKey::KEY_Z) {
            app.settings.assist = !app.settings.assist;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_G) {
            app.settings.gravity = !app.settings.gravity;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_C) {
            app.toggle_camera();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_HOME) || rl.is_key_pressed(KeyboardKey::KEY_H) {
            app.return_to_system();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_N) {
            app.visit_next_site();
        }
        if let Some(text) = app.banner.take() {
            hud.banner = Some((text, hud.age));
            hud.banner_where = app.banner_where.take();
        }
        // El reproductor solo existe en la version chapina.
        let chapin = app.system.version == scene::Version::Chapin;
        player.set_active(chapin);
        if chapin {
            if rl.is_key_pressed(KeyboardKey::KEY_K) {
                player.toggle();
            }
            if rl.is_key_pressed(KeyboardKey::KEY_I) {
                player.next();
            }
            if rl.is_key_pressed(KeyboardKey::KEY_U) {
                player.previous();
            }
            if rl.is_key_pressed(KeyboardKey::KEY_MINUS) || rl.is_key_pressed(KeyboardKey::KEY_KP_SUBTRACT) {
                player.change_volume(-0.1);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_EQUAL) || rl.is_key_pressed(KeyboardKey::KEY_KP_ADD) {
                player.change_volume(0.1);
            }
            // Con el mouse suelto (Tab) tambien se le puede dar clic.
            if !captured && rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                let mouse = rl.get_mouse_position();
                let screen = (rl.get_screen_width(), rl.get_screen_height());
                match hud::player_hit(screen, fonts.scale(), (mouse.x, mouse.y)) {
                    Some(hud::PlayerHit::Previous) => player.previous(),
                    Some(hud::PlayerHit::Toggle) => player.toggle(),
                    Some(hud::PlayerHit::Next) => player.next(),
                    Some(hud::PlayerHit::Seek(at)) => player.seek(at),
                    None => {}
                }
            }
        }
        player.update(dt);
        app.marimba_on = player.marimba();
        if rl.is_key_pressed(KeyboardKey::KEY_F3) {
            app.debug.wireframe = !app.debug.wireframe;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F4) {
            app.debug.lod_colors = !app.debug.lod_colors;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F5) {
            app.lod.enabled = !app.lod.enabled;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F6) {
            app.reseed_props();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F7) {
            app.toggle_freeze();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F8) {
            app.dynamic.enabled = !app.dynamic.enabled;
            app.banner = Some(if app.dynamic.enabled { "Resolución dinámica: sí" } else { "Resolución dinámica: no" });
        }
        if rl.is_key_pressed(KeyboardKey::KEY_L) {
            app.settings.local = !app.settings.local;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_O) {
            app.settings.cruise = !app.settings.cruise;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_RIGHT_BRACKET) {
            app.skip(30.0);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_LEFT_BRACKET) {
            app.skip(-30.0);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_J) {
            app.press_ending();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F) {
            app.press_flare();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_M) {
            app.muted = !app.muted;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_T) {
            tweaks.open = !tweaks.open;
        }
        if tweaks.open {
            let big = if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) {
                10
            } else {
                1
            };
            if rl.is_key_pressed(KeyboardKey::KEY_UP) {
                tweaks.select(-1);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_DOWN) {
                tweaks.select(1);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_RIGHT)
                || rl.is_key_pressed_repeat(KeyboardKey::KEY_RIGHT)
            {
                tweaks.adjust(&mut app, big);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_LEFT)
                || rl.is_key_pressed_repeat(KeyboardKey::KEY_LEFT)
            {
                tweaks.adjust(&mut app, -big);
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_V) {
            app.toggle_version();
            hud.intro_over = true;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_B) {
            app.next_ship();
        }
        let digits = [
            KeyboardKey::KEY_ONE,
            KeyboardKey::KEY_TWO,
            KeyboardKey::KEY_THREE,
            KeyboardKey::KEY_FOUR,
            KeyboardKey::KEY_FIVE,
            KeyboardKey::KEY_SIX,
            KeyboardKey::KEY_SEVEN,
            KeyboardKey::KEY_EIGHT,
            KeyboardKey::KEY_NINE,
        ];
        for (index, key) in digits.into_iter().enumerate() {
            if rl.is_key_pressed(key) {
                app.select_ship(index);
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_Y) {
            // AUTO -> ON -> OFF -> AUTO
            app.path_mode = match app.path_mode {
                None => Some(true),
                Some(true) => Some(false),
                Some(false) => None,
            };
        }
        let photo = rl.is_key_pressed(KeyboardKey::KEY_P);
        // La foto, siempre a resolucion completa.
        app.dynamic.force_full |= photo;

        let input = if reel.is_some() { FrameInput::default() } else { read_input(&rl, captured && !skip_mouse) };
        skip_mouse = false;
        hud.age += dt;
        app.update(dt as f64, &input);
        app.render();
        if let Some(r) = reel.as_mut() {
            synth.set(app.audio_params());
            let mut samples = vec![0.0f32; r.samples()];
            synth.fill(&mut samples);
            r.push_audio(&samples);
        } else if let Some(stream) = stream.as_mut() {
            synth.set(app.audio_params());
            while stream.is_processed() {
                synth.fill(&mut block);
                let _ = stream.update(&block);
            }
        }
        if photo {
            save_photo(&app);
        }
        texture
            .update_texture(app.frame())
            .map_err(|e| format!("no se pudo actualizar la textura: {e:?}"))?;

        let fps = rl.get_fps();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        hud::frame(&mut d, &texture, fit);
        let view = chapin.then(|| player.view());
        hud::draw(&mut d, &ui, &app, fps, &hud, &tweaks, view.as_ref());
        if let Some(r) = reel.as_mut() {
            r.grab()?;
        }
    }
    if let Some(r) = reel {
        r.finish()?;
    }
    Ok(())
}

/// Las teclas de un minijuego: D F J K, las flechas (y W A S D), Enter, Esc,
/// Shift y H.
fn read_game_keys(rl: &RaylibHandle) -> minijuegos::Keys {
    use KeyboardKey::*;
    let lanes = [KEY_D, KEY_F, KEY_J, KEY_K];
    let pressed = |keys: &[KeyboardKey]| keys.iter().any(|&k| rl.is_key_pressed(k));
    let down = |keys: &[KeyboardKey]| keys.iter().any(|&k| rl.is_key_down(k));
    minijuegos::Keys {
        lane_pressed: lanes.map(|k| rl.is_key_pressed(k)),
        lane_down: lanes.map(|k| rl.is_key_down(k)),
        up: pressed(&[KEY_UP, KEY_W]),
        down: pressed(&[KEY_DOWN, KEY_S]),
        left: pressed(&[KEY_LEFT]),
        right: pressed(&[KEY_RIGHT]),
        enter: pressed(&[KEY_ENTER, KEY_KP_ENTER]),
        escape: pressed(&[KEY_ESCAPE, KEY_BACKSPACE]),
        steer: f32::from(i8::from(down(&[KEY_RIGHT, KEY_D])) - i8::from(down(&[KEY_LEFT, KEY_A]))),
        throttle: down(&[KEY_UP, KEY_W]),
        brake: down(&[KEY_DOWN, KEY_S]),
        boost: pressed(&[KEY_LEFT_SHIFT, KEY_RIGHT_SHIFT, KEY_SPACE]),
        horn: pressed(&[KEY_H]),
        typed: typed_keys(rl),
    }
}

/// Las letras y los numeros apretados en este frame, como los marca
/// `minijuegos::Keys::press` (los numeros tambien del teclado numerico).
fn typed_keys(rl: &RaylibHandle) -> u64 {
    use KeyboardKey::*;
    let letters = [
        KEY_A, KEY_B, KEY_C, KEY_D, KEY_E, KEY_F, KEY_G, KEY_H, KEY_I, KEY_J, KEY_K, KEY_L, KEY_M, KEY_N, KEY_O, KEY_P, KEY_Q, KEY_R, KEY_S, KEY_T, KEY_U, KEY_V, KEY_W, KEY_X, KEY_Y, KEY_Z,
    ];
    let digits = [KEY_ZERO, KEY_ONE, KEY_TWO, KEY_THREE, KEY_FOUR, KEY_FIVE, KEY_SIX, KEY_SEVEN, KEY_EIGHT, KEY_NINE];
    let pad = [KEY_KP_0, KEY_KP_1, KEY_KP_2, KEY_KP_3, KEY_KP_4, KEY_KP_5, KEY_KP_6, KEY_KP_7, KEY_KP_8, KEY_KP_9];
    let mut keys = minijuegos::Keys::default();
    for (k, key) in letters.into_iter().enumerate() {
        if rl.is_key_pressed(key) {
            keys.press((b'a' + k as u8) as char);
        }
    }
    for k in 0..10 {
        if rl.is_key_pressed(digits[k]) || rl.is_key_pressed(pad[k]) {
            keys.press((b'0' + k as u8) as char);
        }
    }
    keys.typed
}

/// La capital de lejos, con su anillo (version chapina): ahi se ofrece la
/// carrera.
fn capital_preset() -> &'static app::Preset {
    PRESETS.iter().find(|p| p.name == "51_capital").expect("la vista de la capital")
}

/// El Reformador, en el suelo de la capital (version chapina): ahi esta la
/// carretilla de shucos.
fn reformador_preset() -> &'static app::Preset {
    PRESETS.iter().find(|p| p.name == "52_reformador").expect("la vista del Reformador")
}

/// Fotos de los shucos del Reformador: el menu, la noche con el piloto
/// automatico (al abrir, en la salida de la U, en la del estadio y con los
/// bolos) y los resultados. Solo lo que dibuja la CPU.
fn run_shucos_capture(options: &Options, dir: &Path) -> Result<(), String> {
    use minijuegos::Minigame;
    use minijuegos::shucos::Shucos;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (w, h) = (options.width, options.height);
    let start = std::time::Instant::now();
    let mut game = Shucos::new(w, h, minijuegos::Records::in_memory());
    println!("fondo horneado en {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
    let mut frame = vec![0u8; w * h * 4];
    let mut save = |game: &mut Shucos, name: &str| -> Result<(), String> {
        let mut times: Vec<f64> = (0..9)
            .map(|_| {
                let start = std::time::Instant::now();
                game.draw(&mut frame);
                start.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(f64::total_cmp);
        let path = dir.join(format!("{name}.png"));
        render::png::write_rgba(&path, w as u32, h as u32, &frame).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{name:<24} {:6.2} ms   -> {}", times[4], path.display());
        Ok(())
    };
    save(&mut game, "shucos_1_menu")?;
    game.open();
    game.set_autopilot(true);
    let dt = 1.0 / 60.0;
    let mut elapsed = 0.0;
    for (at, name) in [(7.0, "shucos_2_abriendo"), (0.0, "shucos_3_don_meyer"), (46.0, "shucos_4_salida_de_la_u"), (76.0, "shucos_5_salida_del_estadio"), (128.0, "shucos_6_bolos")] {
        // Don Meyer: hasta que este esperando (llega a eso de las 8 y cuarto).
        let wait_meyer = at == 0.0;
        while (wait_meyer && !game.meyer_waiting() && elapsed < 90.0) || (!wait_meyer && elapsed < at) {
            game.update(dt, &minijuegos::Keys::default());
            elapsed += dt;
        }
        if wait_meyer {
            for _ in 0..30 {
                game.update(dt, &minijuegos::Keys::default());
                elapsed += dt;
            }
        }
        save(&mut game, name)?;
    }
    while !game.in_results() && elapsed < 400.0 {
        game.update(dt, &minijuegos::Keys::default());
        elapsed += dt;
    }
    save(&mut game, "shucos_7_resultados")?;
    let (sold, returned, angry, money) = game.tally();
    println!("vendidos {sold}, devueltos {returned}, enojados {angry}, ganancia Q{money}");
    Ok(())
}

/// Fotos de Talega Surfers: el menu, la corrida con el piloto
/// automatico (al salir, con las camionetas, con la ceniza y las bombas) y
/// el final. Solo lo que dibuja la CPU.
fn run_surfers_capture(options: &Options, dir: &Path) -> Result<(), String> {
    use minijuegos::Minigame;
    use minijuegos::surfers::TalegaSurfers;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (w, h) = (options.width, options.height);
    let mut game = TalegaSurfers::new(w, h, minijuegos::Records::in_memory());
    let mut frame = vec![0u8; w * h * 4];
    let mut save = |game: &mut TalegaSurfers, name: &str| -> Result<(), String> {
        let mut times: Vec<f64> = (0..9)
            .map(|_| {
                let start = std::time::Instant::now();
                game.draw(&mut frame);
                start.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(f64::total_cmp);
        let path = dir.join(format!("{name}.png"));
        render::png::write_rgba(&path, w as u32, h as u32, &frame).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{name:<24} {:6.2} ms   -> {}", times[4], path.display());
        Ok(())
    };
    save(&mut game, "surfers_1_menu")?;
    game.start();
    game.set_autopilot(true);
    let dt = 1.0 / 60.0;
    let mut elapsed = 0.0;
    for (at, name) in [(3.0, "surfers_2_salida"), (14.0, "surfers_3_calle"), (30.0, "surfers_4_ceniza"), (55.0, "surfers_5_bombas")] {
        while elapsed < at && game.running() {
            game.update(dt, &minijuegos::Keys::default());
            elapsed += dt;
        }
        save(&mut game, name)?;
    }
    while game.running() && elapsed < 240.0 {
        game.update(dt, &minijuegos::Keys::default());
        elapsed += dt;
    }
    save(&mut game, "surfers_6_final")?;
    let (meters, lenes, score) = game.tally();
    println!("corrio {meters:.0} m, {lenes} lenes, {score} puntos: {}", game.reason());
    Ok(())
}

/// El lugar de la marimba en la Luna de Xelaju (version chapina): al
/// salir del minijuego se queda junto a ella.
fn xela_preset() -> &'static app::Preset {
    PRESETS.iter().find(|p| p.name == "64_la_marimba").expect("la vista de la marimba")
}

/// Fotos del minijuego de la marimba: el menu, la entrada, el juego en cada
/// parte (con algunas fallas a proposito) y los resultados. Solo lo que
/// dibuja la CPU: el texto del HUD lo pone raylib en la ventana.
fn run_game_capture(options: &Options, dir: &Path) -> Result<(), String> {
    use minijuegos::Minigame;
    use minijuegos::marimba::{Level, MarimbaTiles};
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut app = new_app(options)?;
    app.apply_preset(xela_preset());
    settle(&mut app);
    app.render();
    let (w, h) = (options.width, options.height);
    let background = app.renderer.rgba().to_vec();
    let mut frame = vec![0u8; w * h * 4];
    let save = |game: &mut MarimbaTiles, frame: &mut Vec<u8>, name: &str| -> Result<(), String> {
        let start = std::time::Instant::now();
        game.draw(frame);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let path = dir.join(format!("{name}.png"));
        render::png::write_rgba(&path, w as u32, h as u32, frame).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{name:<26} {ms:6.2} ms   -> {}", path.display());
        Ok(())
    };
    // A 120 fps, con el audio llevando el reloj como en la ventana.
    let dt = 1.0 / 120.0;
    let mut block = vec![0.0f32; (dt * audio::SAMPLE_RATE as f64).round() as usize];
    let step = |game: &mut MarimbaTiles, block: &mut Vec<f32>, keys: minijuegos::Keys| {
        game.update(dt, &keys);
        game.audio(block);
    };

    let mut game = MarimbaTiles::new(&background, w, h, minijuegos::Records::in_memory());
    let mut keys = minijuegos::Keys::default();
    keys.lane_pressed[1] = true;
    step(&mut game, &mut block, keys);
    for _ in 0..6 {
        step(&mut game, &mut block, minijuegos::Keys::default());
    }
    save(&mut game, &mut frame, "marimba_1_menu")?;

    let shots: [(Level, &[(f64, &str)]); 3] = [
        (Level::Bajo, &[(4.4, "marimba_2_bajo")]),
        (Level::Tiple, &[(1.6, "marimba_3_entrada"), (7.3, "marimba_4_tiple"), (33.0, "marimba_5_segunda_vuelta")]),
        (Level::Redoble, &[(9.65, "marimba_6_redoble")]),
    ];
    for (level, moments) in shots {
        game.play(level);
        let mut elapsed = 0.0;
        for &(at, name) in moments {
            while elapsed < at {
                let keys = game.demo_keys(dt, 9);
                step(&mut game, &mut block, keys);
                elapsed += dt;
            }
            save(&mut game, &mut frame, name)?;
        }
    }
    // Hasta el final de la cancion, para los resultados.
    for _ in 0..(120.0 / dt) as usize {
        let keys = game.demo_keys(dt, 9);
        step(&mut game, &mut block, keys);
    }
    save(&mut game, &mut frame, "marimba_7_resultados")?;
    Ok(())
}

/// Fotos de la carrera del Periferico con el piloto automatico: la largada,
/// el semaforo, la carrera en varios momentos y la meta.
fn run_race_capture(options: &Options, dir: &Path) -> Result<(), String> {
    use minijuegos::Minigame;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut app = new_app(options)?;
    app.apply_preset(capital_preset());
    let start = std::time::Instant::now();
    let mut race = minijuegos::race(&app, minijuegos::Records::in_memory());
    println!("naves horneadas en {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
    let (w, h) = (options.width, options.height);
    let mut frame = vec![0u8; w * h * 4];
    let mut save = |race: &mut minijuegos::carrera::Carrera, name: &str| -> Result<(), String> {
        // La mediana de nueve: un solo frame en frio no dice mucho.
        let mut times: Vec<f64> = (0..9)
            .map(|_| {
                let start = std::time::Instant::now();
                race.draw(&mut frame);
                start.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(f64::total_cmp);
        let ms = times[4];
        let path = dir.join(format!("{name}.png"));
        render::png::write_rgba(&path, w as u32, h as u32, &frame).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("{name:<26} {ms:6.2} ms   -> {}", path.display());
        Ok(())
    };
    save(&mut race, "carrera_1_largada")?;
    race.set_autopilot(true);
    race.start();
    let dt = 1.0 / 60.0;
    let mut elapsed = 0.0;
    for (at, name) in [(1.6, "carrera_2_semaforo"), (7.0, "carrera_3_arranque"), (16.0, "carrera_4_curva"), (27.0, "carrera_5_loma"), (30.0, "carrera_6_tumulos"), (70.0, "carrera_7_segunda_vuelta"), (260.0, "carrera_8_meta")] {
        while elapsed < at {
            race.update(dt, &minijuegos::Keys::default());
            elapsed += dt;
        }
        // La de los tumulos: cuando la nave va a unos 30 m del proximo
        // (cada nave llega a otra hora), hasta 20 s despues.
        if name == "carrera_6_tumulos" {
            while elapsed < at + 20.0 && !race.bump_ahead().is_some_and(|d| (25.0..=35.0).contains(&d)) {
                race.update(dt, &minijuegos::Keys::default());
                elapsed += dt;
            }
        }
        // La de la loma, adentro de un tunel de arcos, y la de la meta, en
        // el festival de barriletes (hasta 20 y 60 s despues).
        if name == "carrera_5_loma" {
            while elapsed < at + 20.0 && !race.in_tunnel() {
                race.update(dt, &minijuegos::Keys::default());
                elapsed += dt;
            }
        }
        if name == "carrera_8_meta" {
            while elapsed < at + 60.0 && !race.in_festival() {
                race.update(dt, &minijuegos::Keys::default());
                elapsed += dt;
            }
        }
        save(&mut race, name)?;
    }
    Ok(())
}

fn read_input(rl: &RaylibHandle, captured: bool) -> FrameInput {
    let down = |key| rl.is_key_down(key);
    let axis = |pos, neg| f64::from(i8::from(down(pos)) - i8::from(down(neg)));
    let look = if captured {
        let delta = rl.get_mouse_delta();
        (delta.x as f64, delta.y as f64)
    } else {
        (0.0, 0.0)
    };
    FrameInput {
        look,
        thrust: DVec3::new(
            axis(KeyboardKey::KEY_D, KeyboardKey::KEY_A),
            axis(KeyboardKey::KEY_SPACE, KeyboardKey::KEY_LEFT_CONTROL),
            axis(KeyboardKey::KEY_S, KeyboardKey::KEY_W),
        ),
        roll: axis(KeyboardKey::KEY_Q, KeyboardKey::KEY_E),
        boost: down(KeyboardKey::KEY_LEFT_SHIFT),
        brake: down(KeyboardKey::KEY_X),
        free_look: rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT),
        rewind: down(KeyboardKey::KEY_R),
        zoom: rl.get_mouse_wheel_move() as f64,
    }
}

fn save_photo(app: &App) {
    let dir = Path::new("capturas");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("foto_{stamp}.png"));
    let result = std::fs::create_dir_all(dir).and_then(|_| {
        let (width, height) = app.frame_size();
        render::png::write_rgba(&path, width as u32, height as u32, app.frame())
    });
    match result {
        Ok(()) => println!("foto guardada en {}", path.display()),
        Err(error) => eprintln!("no se pudo guardar la foto: {error}"),
    }
}

/// Renderiza hasta que el terreno termine de generar el detalle de la vista
/// (o un maximo de frames), para no capturar ni medir la carga.
fn settle(app: &mut App) {
    for _ in 0..200 {
        app.render();
        if app
            .terrain_stats()
            .is_none_or(|(_, s)| s.pending == 0 && s.generated == 0)
        {
            break;
        }
    }
}

fn run_capture(options: &Options, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut app = new_app(options)?;
    let counts: Vec<String> = app
        .system
        .bodies
        .iter()
        .zip(&app.system.props)
        .filter(|(_, set)| !set.instances.is_empty())
        .map(|(body, set)| format!("{} {}", body.name, set.instances.len()))
        .collect();
    println!("objetos de la superficie: {}", counts.join(", "));
    let mut shots: Vec<(String, Option<&app::Preset>)> = PRESETS
        .iter()
        .map(|p| (p.name.to_string(), Some(p)))
        .collect();
    // Demostracion del culling: se congela el frustum en el nido de Argos y
    // la camara se aleja hacia atras y arriba.
    shots.push(("21_frustum_congelado".to_string(), None));
    for (name, preset) in shots {
        if options.preset.is_some_and(|i| name != PRESETS[i].name) {
            continue;
        }
        match preset {
            Some(preset) => app.apply_preset(preset),
            None => {
                let nest = PRESETS.iter().find(|p| p.name == "28_nido").expect("la vista del nido");
                app.apply_preset(nest);
                settle(&mut app);
                app.toggle_freeze();
                let back = app.aim().rotate(DVec3::new(0.0, 0.6, 1.0)).normalize();
                app.nudge(back * 520.0);
            }
        }
        settle(&mut app);
        let stats = app.render();
        let path = dir.join(format!("{name}.png"));
        render::png::write_rgba(
            &path,
            options.width as u32,
            options.height as u32,
            app.renderer.rgba(),
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "{:<16} {:6.1} ms   {:>7} triangulos rasterizados   {:>8} pixeles   -> {}",
            name,
            stats.ms_total,
            stats.triangles.emitted,
            stats.pixels_shaded,
            path.display()
        );
    }
    Ok(())
}

/// Mide cada vista fija `frames` veces y reporta medianas por etapa. Con la
/// vista fija, la diferencia entre frames es solo ruido de la maquina, y la
/// mediana lo filtra mucho mejor que la media de un recorrido.
fn run_tour(options: &Options, path: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let encoder = std::env::var_os("OCASO_FFMPEG").unwrap_or_else(|| "ffmpeg".into());
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut child = Command::new(encoder)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
        ])
        .arg(format!("{}x{}", options.width, options.height))
        .args([
            "-framerate",
            "24",
            "-i",
            "-",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}. Instala ffmpeg o define OCASO_FFMPEG."))?;
    let result = (|| -> Result<(), String> {
        let mut pipe = child.stdin.take().ok_or("ffmpeg no abrio la entrada")?;
        let mut app = new_app(options)?;
        let mut tour = app.begin_tour();
        settle(&mut app);
        let mut done = false;
        let mut frames = 0;
        while !done && tour.elapsed < 240.0 {
            // 15 pasos de 1/120 por frame a 24 Hz: simulacion x3.
            for _ in 0..15 {
                if app.tour_step(&mut tour) {
                    done = true;
                    break;
                }
            }
            app.render();
            pipe.write_all(app.renderer.rgba())
                .map_err(|e| e.to_string())?;
            frames += 1;
            if frames % 120 == 0 {
                println!("recorrido: {:.1} s de simulacion", tour.elapsed);
            }
        }
        if !done {
            return Err("el piloto no completo el recorrido en 240 s".into());
        }
        println!(
            "recorrido continuo completo: {} frames, {:.1} s de video (simulacion x3)",
            frames,
            frames as f64 / 24.0
        );
        Ok(())
    })();
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    result?;
    if !status.success() {
        return Err(format!("ffmpeg termino con {status}"));
    }
    Ok(())
}

fn run_bench(options: &Options, frames: usize) -> Result<(), String> {
    let frames = frames.max(1);
    let mut app = new_app(options)?;
    println!(
        "{}x{}  {} frames por vista  {} hilos",
        options.width,
        options.height,
        frames,
        parallel::workers()
    );
    println!(
        "{:<16} {:>7} {:>7} | {:>6} {:>6} {:>6} {:>6} {:>6} {:>7}  (medianas, ms)",
        "vista", "min", "frame", "vert", "tri", "raster", "shade", "bloom", "tonemap"
    );
    for preset in &PRESETS {
        if options
            .preset
            .is_some_and(|i| preset.name != PRESETS[i].name)
        {
            continue;
        }
        app.apply_preset(preset);
        settle(&mut app);
        let samples: Vec<render::Stats> = (0..frames).map(|_| app.render()).collect();
        let median = |f: fn(&render::Stats) -> f32| {
            let mut values: Vec<f32> = samples.iter().map(f).collect();
            values.sort_by(f32::total_cmp);
            values[values.len() / 2]
        };
        let min = samples.iter().map(|s| s.ms_total).fold(f32::MAX, f32::min);
        println!(
            "{:<16} {:>7.2} {:>7.2} | {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>7.2}",
            preset.name,
            min,
            median(|s| s.ms_total),
            median(|s| s.ms_vertex),
            median(|s| s.ms_triangles),
            median(|s| s.ms_raster),
            median(|s| s.ms_shade),
            median(|s| s.ms_bloom),
            median(|s| s.ms_post),
        );
    }
    Ok(())
}

/// La carpeta de las canciones de la version chapina: la del directorio
/// actual o, si se corre desde otro lado, la del proyecto.
fn music_dir() -> PathBuf {
    let local = PathBuf::from("musica");
    if local.exists() {
        local
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("musica")
    }
}

