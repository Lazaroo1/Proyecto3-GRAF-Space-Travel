//! OCASO: un sistema solar explorable con un renderer por software propio.
//!
//! raylib solo abre la ventana, lee el teclado y el mouse, y muestra la imagen
//! final como una textura 2D. Todo el 3D (transformaciones, recorte, raster,
//! profundidad y shaders) se calcula en CPU en `render` y `shaders`.

mod app;
mod audio;
mod config;
mod hud;
#[allow(dead_code)]
mod math;
mod parallel;
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

enum Mode {
    Window,
    Capture(PathBuf),
    Bench(usize),
}

struct Options {
    width: usize,
    height: usize,
    mode: Mode,
    wireframe: bool,
    lod_colors: bool,
    fog_only: bool,
}

const USAGE: &str = "\
uso: ocaso [opciones]
  --res ANCHOxALTO      resolucion del render (por defecto 1280x720)
  --captura CARPETA     guarda vistas fijas como PNG sin abrir ventana
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
            "--bench" => {
                let frames = args.next().ok_or("--bench espera un numero de frames")?;
                let frames = frames.parse().map_err(|_| "numero de frames invalido")?;
                options.mode = Mode::Bench(frames);
            }
            "--wire" => options.wireframe = true,
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
    Ok(app)
}

fn run_window(options: &Options) -> Result<(), String> {
    let mut app = new_app(options)?;
    let (mut rl, thread) = raylib::init()
        .size(options.width as i32, options.height as i32)
        .title("OCASO - software renderer")
        .build();
    rl.set_target_fps(144);
    rl.disable_cursor();
    let mut captured = true;
    // Al capturar el mouse, el primer movimiento trae el salto desde donde
    // estaba el cursor: se descarta para que la mira no pegue un brinco.
    let mut skip_mouse = true;

    // La textura solo transporta la imagen ya calculada en CPU a la pantalla.
    let image = Image::gen_image_color(options.width as i32, options.height as i32, Color::BLACK);
    let mut texture = rl
        .load_texture_from_image(&thread, &image)
        .map_err(|e| format!("no se pudo crear la textura: {e:?}"))?;
    let mut hud = hud::Hud {
        help: true,
        stats: true,
        age: 0.0,
        hidden: false,
        preset: None,
    };
    let mut preset_index: Option<usize> = None;
    let mut tweaks = tweaks::Tweaks::default();

    // Sonido: raylib solo reproduce lo que el sintetizador genera. Si no hay
    // dispositivo de audio, el juego sigue sin sonido.
    let audio_device = RaylibAudio::init_audio_device().ok();
    let mut stream = audio_device.as_ref().map(|device| {
        device.set_audio_stream_buffer_size_default(AUDIO_BLOCK as i32);
        let stream = device.new_audio_stream(audio::SAMPLE_RATE, 32, 1);
        stream.play();
        stream
    });
    let mut synth = audio::Synth::new();
    let mut block = vec![0.0f32; AUDIO_BLOCK];

    while !rl.window_should_close() {
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
        let step = if rl.is_key_pressed(KeyboardKey::KEY_PAGE_DOWN) {
            1
        } else if rl.is_key_pressed(KeyboardKey::KEY_PAGE_UP) {
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
            // Veinte segundos antes del colapso.
            app.jump_to_cycle(scene::cycle::phase_start(scene::cycle::Phase::Collapse) - 20.0);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_M) {
            app.muted = !app.muted;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_T) {
            tweaks.open = !tweaks.open;
        }
        if tweaks.open {
            let big = if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) { 10 } else { 1 };
            if rl.is_key_pressed(KeyboardKey::KEY_UP) {
                tweaks.select(-1);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_DOWN) {
                tweaks.select(1);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_RIGHT) || rl.is_key_pressed_repeat(KeyboardKey::KEY_RIGHT) {
                tweaks.adjust(&mut app, big);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_LEFT) || rl.is_key_pressed_repeat(KeyboardKey::KEY_LEFT) {
                tweaks.adjust(&mut app, -big);
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_V) {
            // AUTO -> ON -> OFF -> AUTO
            app.path_mode = match app.path_mode {
                None => Some(true),
                Some(true) => Some(false),
                Some(false) => None,
            };
        }
        let photo = rl.is_key_pressed(KeyboardKey::KEY_P);

        let input = read_input(&rl, captured && !skip_mouse);
        skip_mouse = false;
        hud.age += rl.get_frame_time();
        app.update(rl.get_frame_time() as f64, &input);
        app.render();
        if let Some(stream) = stream.as_mut() {
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
            .update_texture(app.renderer.rgba())
            .map_err(|e| format!("no se pudo actualizar la textura: {e:?}"))?;

        let fps = rl.get_fps();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_texture(&texture, 0, 0, Color::WHITE);
        hud::draw(&mut d, &app, fps, &hud, &tweaks);
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
        render::png::write_rgba(
            &path,
            app.renderer.width() as u32,
            app.renderer.height() as u32,
            app.renderer.rgba(),
        )
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
        if app.terrain_stats().is_none_or(|(_, s)| s.pending == 0 && s.generated == 0) {
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
    let mut shots: Vec<(String, Option<&app::Preset>)> = PRESETS.iter().map(|p| (p.name.to_string(), Some(p))).collect();
    // Demostracion del culling: se congela el frustum sobre Requiem y la
    // camara se aleja hacia atras y arriba.
    shots.push(("21_frustum_congelado".to_string(), None));
    for (name, preset) in shots {
        match preset {
            Some(preset) => app.apply_preset(preset),
            None => {
                app.apply_preset(&PRESETS[6]);
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
fn run_bench(options: &Options, frames: usize) -> Result<(), String> {
    let frames = frames.max(1);
    let mut app = new_app(options)?;
    println!("{}x{}  {} frames por vista  {} hilos", options.width, options.height, frames, parallel::workers());
    println!(
        "{:<16} {:>7} {:>7} | {:>6} {:>6} {:>6} {:>6} {:>6} {:>7}  (medianas, ms)",
        "vista", "min", "frame", "vert", "tri", "raster", "shade", "bloom", "tonemap"
    );
    for preset in &PRESETS {
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
