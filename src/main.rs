//! OCASO: un sistema solar explorable con un renderer por software propio.
//!
//! raylib solo abre la ventana, lee el teclado y el mouse, y muestra la imagen
//! final como una textura 2D. Todo el 3D (transformaciones, recorte, raster,
//! profundidad y shaders) se calcula en CPU en `render` y `shaders`.

mod app;
mod config;
mod hud;
#[allow(dead_code)]
mod math;
mod parallel;
mod render;
mod scene;
mod shaders;
mod terrain;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use raylib::prelude::*;

use app::{App, FrameInput, Preset};
use math::DVec3;

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
}

const USAGE: &str = "\
uso: ocaso [opciones]
  --res ANCHOxALTO      resolucion del render (por defecto 1280x720)
  --captura CARPETA     guarda vistas fijas como PNG sin abrir ventana
  --bench FRAMES        mide cada vista fija FRAMES veces sin abrir ventana
  --wire                empieza con el wireframe encendido (F3)
  --lod                 empieza con los colores de LOD encendidos (F4)";

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        width: config::DEFAULT_WIDTH,
        height: config::DEFAULT_HEIGHT,
        mode: Mode::Window,
        wireframe: false,
        lod_colors: false,
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
    };

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
        let photo = rl.is_key_pressed(KeyboardKey::KEY_P);

        let input = read_input(&rl, captured && !skip_mouse);
        skip_mouse = false;
        app.update(rl.get_frame_time() as f64, &input);
        app.render();
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
        hud::draw(&mut d, &app, fps, &hud);
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
    for preset in Preset::ALL {
        app.apply_preset(preset);
        settle(&mut app);
        let stats = app.render();
        let path = dir.join(format!("{}.png", preset.name()));
        render::png::write_rgba(
            &path,
            options.width as u32,
            options.height as u32,
            app.renderer.rgba(),
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        println!(
            "{:<16} {:6.1} ms   {:>7} triangulos rasterizados   {:>8} pixeles   -> {}",
            preset.name(),
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
    for preset in Preset::ALL {
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
            preset.name(),
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
