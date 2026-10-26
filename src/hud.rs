//! HUD en 2D con el texto de raylib. No es parte del pipeline 3D: se dibuja
//! encima de la imagen ya renderizada. El texto de raylib no acepta UTF-8,
//! por eso no lleva tildes.

use raylib::prelude::*;

use crate::app::App;
use crate::parallel;
use crate::tweaks::Tweaks;

pub struct Hud {
    pub help: bool,
    pub stats: bool,
}

const FONT: i32 = 18;
const LINE: i32 = 21;
const TEXT: Color = Color::new(236, 226, 214, 255);
const PANEL: Color = Color::new(0, 0, 0, 150);

pub fn draw(d: &mut RaylibDrawHandle, app: &App, fps: u32, hud: &Hud, tweaks: &Tweaks) {
    let width = d.get_screen_width();
    let height = d.get_screen_height();

    if hud.stats {
        let s = &app.stats;
        let t = &s.triangles;
        let terrain = match app.terrain_stats() {
            Some((name, ts)) => format!(
                "terreno {name}: {} chunks (nivel {})  nuevos {}  pendientes {}  cache {}  horizonte -{}  frustum -{}{}",
                ts.drawn,
                ts.deepest,
                ts.generated,
                ts.pending,
                ts.cached,
                ts.culled_horizon,
                ts.culled_frustum,
                if app.lod.enabled { "" } else { "  [LOD APAGADO]" }
            ),
            None => "terreno: -".to_string(),
        };
        let p = &app.prop_stats;
        let props = format!(
            "objetos {} (detalle {}/{}/{})  chicos -{}  horizonte -{}  frustum -{}  semilla {}",
            p.drawn_total(),
            p.drawn[0],
            p.drawn[1],
            p.drawn[2],
            p.small,
            p.horizon,
            p.frustum,
            app.system.prop_seed
        );
        let lines = [
            format!("FPS {fps}   frame {:.1} ms   {} hilos", s.ms_total, parallel::workers()),
            format!(
                "vert {:.1}  tri {:.1}  raster {:.1}  shade {:.1}  post {:.1} ms",
                s.ms_vertex, s.ms_triangles, s.ms_raster, s.ms_shade, s.ms_post
            ),
            format!("objetos {}   fuera de vista {}", s.objects, s.objects_culled),
            format!(
                "triangulos {}   frustum -{}   espalda -{}   sin pixel -{}",
                t.input, t.frustum, t.backface, t.small
            ),
            format!(
                "recortados {}   rasterizados {}   pixeles {}   particulas {}",
                t.clipped, t.emitted, s.pixels_shaded, s.particles
            ),
            terrain,
            props,
        ];
        panel(d, 10, 10, &lines, false, width);
    }

    let f = &app.flight;
    let contact = if f.contact { "   CONTACTO" } else { "" };
    let local = match (app.settings.local, f.local, f.hold) {
        (false, _, _) => "OFF".to_string(),
        (true, true, Some(h)) => format!("ACTIVO sobre {}, sostiene {:.0}", app.body_name(f.frame), h),
        (true, true, None) => format!("ACTIVO sobre {}", app.body_name(f.frame)),
        (true, false, _) if !app.settings.assist => "ON (necesita asistencia)".to_string(),
        (true, false, _) => "ON (lejos de un planeta)".to_string(),
    };
    let path = match app.path_mode {
        None => "AUTO",
        Some(true) => "ON",
        Some(false) => "OFF",
    };
    let impact = if app.path_shown && app.path_impact { "   CHOQUE PREDICHO" } else { "" };
    let lines = [
        format!("velocidad {:.1} u/s   (max {:.0})", f.speed, f.max_speed),
        format!("altitud {:.1} sobre {}", f.altitude, app.body_name(f.nearest)),
        format!("marco: {}{contact}", app.body_name(f.frame)),
        format!(
            "asistencia [Z] {}   gravedad [G] {}",
            on_off(app.settings.assist),
            on_off(app.settings.gravity)
        ),
        format!("modo local [L]: {local}"),
        format!(
            "crucero [O] {}   trayectoria [V] {path}{impact}",
            on_off(app.settings.cruise)
        ),
    ];
    panel(d, 10, 10, &lines, true, width);

    if tweaks.open {
        let lines = tweaks.lines(app);
        let y = (height - (lines.len() as i32 * LINE + 10)) / 2;
        panel(d, 10, y, &lines, true, width);
    }

    if hud.help {
        let lines = [
            "Mouse: apuntar   Clic derecho: mirar alrededor   Rueda: distancia".to_string(),
            "W/S: adelante/atras   A/D: lados   Espacio/Ctrl: subir/bajar".to_string(),
            "Q/E: alabeo   Shift: turbo   C: camara   P: foto   Tab: soltar el mouse".to_string(),
            "L: modo local   O: crucero (rodea el planeta)   V: trayectoria".to_string(),
            "T: ajustes en vivo   M: sonido".to_string(),
            "F3: wireframe   F4: colores de LOD   F5: LOD on/off   F6: otra semilla".to_string(),
            "F1: ayuda   F2: estadisticas   Esc: salir".to_string(),
        ];
        let y = height - 10 - (lines.len() as i32 * LINE + 10);
        panel(d, 10, y, &lines, false, width);
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}

fn panel(d: &mut RaylibDrawHandle, margin: i32, y: i32, lines: &[String], right: bool, screen_width: i32) {
    let text_width = lines.iter().map(|l| d.measure_text(l, FONT)).max().unwrap_or(0);
    let w = text_width + 16;
    let h = lines.len() as i32 * LINE + 10;
    let x = if right { screen_width - margin - w } else { margin };
    d.draw_rectangle(x, y, w, h, PANEL);
    for (i, line) in lines.iter().enumerate() {
        d.draw_text(line, x + 8, y + 6 + i as i32 * LINE, FONT, TEXT);
    }
}
