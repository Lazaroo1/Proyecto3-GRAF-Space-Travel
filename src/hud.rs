//! HUD en 2D con el texto de raylib. No es parte del pipeline 3D: se dibuja
//! encima de la imagen ya renderizada. El texto de raylib no acepta UTF-8,
//! por eso no lleva tildes.

use raylib::prelude::*;

use crate::app::App;
use crate::player::PlayerView;
use crate::parallel;
use crate::tweaks::Tweaks;

pub struct Hud {
    pub help: bool,
    pub stats: bool,
    /// Segundos desde que se abrio la ventana (para la intro).
    pub age: f32,
    /// Sin nada encima de la imagen (F9), para grabar o sacar fotos.
    pub hidden: bool,
    /// Nombre de la ultima vista fija elegida y cuando se eligio.
    pub preset: Option<(&'static str, f32)>,
    /// Un aviso del juego (un lugar, una nave) y cuando aparecio.
    pub banner: Option<(&'static str, f32)>,
    /// Si el titulo del principio ya se quito (al cambiar de version se
    /// quita para que no se encime con el de la transicion).
    pub intro_over: bool,
}

/// Segundos que dura el titulo al empezar.
const INTRO: f32 = 7.0;

const FONT: i32 = 18;
const LINE: i32 = 21;
const TEXT: Color = Color::new(236, 226, 214, 255);
const PANEL: Color = Color::new(0, 0, 0, 150);
const MARKER: Color = Color::new(255, 214, 120, 220);

pub fn draw(
    d: &mut RaylibDrawHandle,
    app: &App,
    fps: u32,
    hud: &Hud,
    tweaks: &Tweaks,
    player: Option<&PlayerView>,
) {
    let width = d.get_screen_width();
    let height = d.get_screen_height();
    if hud.hidden {
        return;
    }

    // Intro: el titulo aparece y se desvanece en los primeros segundos.
    if hud.age < INTRO && !hud.intro_over {
        let fade = (hud.age / 1.2).min(1.0) * ((INTRO - hud.age) / 2.0).min(1.0);
        let alpha = |a: f32| (a * fade * 255.0) as u8;
        let title = "OCASO";
        let size = height / 6;
        let w = d.measure_text(title, size);
        d.draw_text(
            title,
            (width - w) / 2,
            height / 3 - size / 2,
            size,
            Color::new(255, 190, 140, alpha(1.0)),
        );
        let lines = [
            "Una estrella moribunda late como un corazon.",
            "En quince minutos colapsa y nace Gargantua.",
            "F1: controles",
        ];
        for (i, line) in lines.iter().enumerate() {
            let w = d.measure_text(line, 22);
            let y = height / 3 + size / 2 + 20 + i as i32 * 30;
            d.draw_text(
                line,
                (width - w) / 2,
                y,
                22,
                Color::new(236, 226, 214, alpha(0.85)),
            );
        }
    }

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
                if app.lod.enabled {
                    ""
                } else {
                    "  [LOD APAGADO]"
                }
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
            format!(
                "FPS {fps}   frame {:.1} ms   {} hilos",
                s.ms_total,
                parallel::workers()
            ),
            format!(
                "vert {:.1}  tri {:.1}  raster {:.1}  shade {:.1}  post {:.1} ms",
                s.ms_vertex, s.ms_triangles, s.ms_raster, s.ms_shade, s.ms_post
            ),
            format!(
                "objetos {}   fuera de vista {}",
                s.objects, s.objects_culled
            ),
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
        (true, true, Some(h)) => {
            format!("ACTIVO sobre {}, sostiene {:.0}", app.body_name(f.frame), h)
        }
        (true, true, None) => format!("ACTIVO sobre {}", app.body_name(f.frame)),
        (true, false, _) if !app.settings.assist => "ON (necesita asistencia)".to_string(),
        (true, false, _) => "ON (lejos de un planeta)".to_string(),
    };
    let path = match app.path_mode {
        None => "AUTO",
        Some(true) => "ON",
        Some(false) => "OFF",
    };
    let impact = if app.path_shown && app.path_impact {
        "   CHOQUE PREDICHO"
    } else {
        ""
    };
    let c = &app.cycle;
    let clock = |seconds: f64| {
        format!(
            "{:02}:{:02}",
            (seconds / 60.0) as u32,
            (seconds % 60.0) as u32
        )
    };
    let lines = [
        match (c.phase, app.bottle_name()) {
            (_, Some(_)) => "VERSION CHAPINA   V: volver a OCASO".to_string(),
            (crate::scene::cycle::Phase::Agony, None) => "AGONIA   J: desatar el final".to_string(),
            (phase, None) => format!(
                "{}   {}   J: renacer",
                phase.name().to_uppercase(),
                clock(c.tau)
            ),
        },
        format!("velocidad {:.1} u/s   (max {:.0})", f.speed, f.max_speed),
        if f.braking {
            "FRENO ACTIVO [X]".to_string()
        } else if f.boundary {
            "LIMITE DEL SISTEMA: gira para regresar".to_string()
        } else {
            "X: frenar   N: viajar al siguiente lugar   Inicio: recuperar posicion".to_string()
        },
        format!(
            "altitud {:.1} sobre {}",
            f.altitude,
            app.body_name(f.nearest)
        ),
        format!("marco: {}{contact}", app.body_name(f.frame)),
        format!(
            "asistencia [Z] {}   gravedad [G] {}",
            on_off(app.settings.assist),
            on_off(app.settings.gravity)
        ),
        format!("modo local [L]: {local}"),
        format!(
            "crucero [O] {}   trayectoria [Y] {path}{impact}",
            on_off(app.settings.cruise)
        ),
        match app.bottle_name() {
            Some(name) => format!("nave [B o 1-6]: {name}"),
            None => "version chapina [V]".to_string(),
        },
    ];
    panel(d, 10, 10, &lines, true, width);

    // Nombres de los cuerpos sobre su posicion en pantalla.
    for m in &app.markers {
        let label = if m.distance >= 1000.0 {
            format!("{} {:.1}k", m.name, m.distance / 1000.0)
        } else {
            format!("{} {:.0}", m.name, m.distance)
        };
        let w = d.measure_text(&label, 16);
        let (x, y) = (m.x as i32, m.y as i32);
        d.draw_triangle(
            Vector2::new(x as f32, y as f32 - 2.0),
            Vector2::new(x as f32 + 5.0, y as f32 - 9.0),
            Vector2::new(x as f32 - 5.0, y as f32 - 9.0),
            MARKER,
        );
        d.draw_text(&label, x - w / 2, y - 28, 16, MARKER);
    }
    if let Some((name, since)) = hud.preset {
        if hud.age - since < 3.0 {
            let text = format!("vista fija: {name}");
            let w = d.measure_text(&text, 26);
            d.draw_text(&text, (width - w) / 2, 60, 26, MARKER);
        }
    }
    if let Some((text, since)) = hud.banner {
        if hud.age - since < 3.0 {
            let w = d.measure_text(text, 26);
            d.draw_text(text, (width - w) / 2, 96, 26, MARKER);
        }
    }
    if let Some(view) = player {
        draw_player(d, view, (width, height));
    }
    // El titulo grande de la transicion entre versiones.
    if let Some((title, fade)) = app.transition_title() {
        let size = height / 7;
        let w = d.measure_text(title, size);
        let color = if app.bottle_name().is_some() {
            Color::new(255, 214, 120, (fade * 255.0) as u8)
        } else {
            Color::new(255, 190, 140, (fade * 255.0) as u8)
        };
        d.draw_text(title, (width - w) / 2, height / 3 - size / 2, size, color);
    }
    if app.freeze.is_some() {
        let text =
            "FRUSTUM CONGELADO (F7): el culling y el LOD se calculan desde la camara amarilla";
        let w = d.measure_text(text, FONT);
        d.draw_text(text, (width - w) / 2, height - 40, FONT, MARKER);
    }

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
            "X: freno de emergencia   Inicio: volver a Pira sin reiniciar el ciclo".to_string(),
            "L: modo local   O: crucero (rodea el planeta)   Y: trayectoria".to_string(),
            "V: version chapina / OCASO   B o 1-6: nave botella (en la chapina)".to_string(),
            "T: ajustes en vivo   M: sonido".to_string(),
            "R: rebobinar   [ ]: -30/+30 s   J: final / renacer   RePag/AvPag: vistas fijas".to_string(),
            "F3: wireframe   F4: colores de LOD   F5: LOD on/off   F6: otra semilla   F7: congelar frustum".to_string(),
            "F1: ayuda   F2: estadisticas   F9: ocultar todo   Esc: salir".to_string(),
        ];
        let y = height - 10 - (lines.len() as i32 * LINE + 10);
        panel(d, 10, y, &lines, false, width);
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}

fn panel(
    d: &mut RaylibDrawHandle,
    margin: i32,
    y: i32,
    lines: &[String],
    right: bool,
    screen_width: i32,
) {
    let text_width = lines
        .iter()
        .map(|l| d.measure_text(l, FONT))
        .max()
        .unwrap_or(0);
    let w = text_width + 16;
    let h = lines.len() as i32 * LINE + 10;
    let x = if right {
        screen_width - margin - w
    } else {
        margin
    };
    d.draw_rectangle(x, y, w, h, PANEL);
    for (i, line) in lines.iter().enumerate() {
        d.draw_text(line, x + 8, y + 6 + i as i32 * LINE, FONT, TEXT);
    }
}

/// Tamano y lugar del reproductor: abajo a la derecha.
const PLAYER_SIZE: (i32, i32) = (380, 92);

fn player_rect(screen: (i32, i32)) -> (i32, i32) {
    (screen.0 - PLAYER_SIZE.0 - 10, screen.1 - PLAYER_SIZE.1 - 10)
}

/// Los tres botones (anterior, pausa, siguiente) y la barra de avance.
fn player_controls(screen: (i32, i32)) -> ([Rectangle; 3], Rectangle) {
    let (x, y) = player_rect(screen);
    let button = |k: i32| Rectangle::new((x + 10 + k * 46) as f32, (y + 52) as f32, 40.0, 30.0);
    let bar = Rectangle::new((x + 160) as f32, (y + 60) as f32, (PLAYER_SIZE.0 - 172) as f32, 12.0);
    ([button(0), button(1), button(2)], bar)
}

/// Lo que se toco del reproductor con el mouse.
pub enum PlayerHit {
    Previous,
    Toggle,
    Next,
    /// En la barra: a que fraccion de la cancion saltar.
    Seek(f32),
}

pub fn player_hit(screen: (i32, i32), mouse: (f32, f32)) -> Option<PlayerHit> {
    let (buttons, bar) = player_controls(screen);
    let point = Vector2::new(mouse.0, mouse.1);
    if buttons[0].check_collision_point_rec(point) {
        return Some(PlayerHit::Previous);
    }
    if buttons[1].check_collision_point_rec(point) {
        return Some(PlayerHit::Toggle);
    }
    if buttons[2].check_collision_point_rec(point) {
        return Some(PlayerHit::Next);
    }
    let grown = Rectangle::new(bar.x, bar.y - 6.0, bar.width, bar.height + 12.0);
    grown
        .check_collision_point_rec(point)
        .then(|| PlayerHit::Seek((mouse.0 - bar.x) / bar.width))
}

/// El mini reproductor: la cancion, los botones, el avance y el volumen.
fn draw_player(d: &mut RaylibDrawHandle, view: &PlayerView, screen: (i32, i32)) {
    let (x, y) = player_rect(screen);
    d.draw_rectangle(x, y, PLAYER_SIZE.0, PLAYER_SIZE.1, PANEL);
    let title = format!("{}/{}  {}", view.index + 1, view.count, view.title);
    // Si el nombre no cabe, se corta.
    let mut shown = title.clone();
    while d.measure_text(&shown, FONT) > PLAYER_SIZE.0 - 20 && shown.len() > 4 {
        shown.pop();
    }
    if shown.len() < title.len() {
        shown.push_str("..");
    }
    d.draw_text(&shown, x + 10, y + 8, FONT, TEXT);
    let hint = "K pausa  U/I cancion  -/+ volumen";
    d.draw_text(hint, x + 10, y + 30, 14, Color::new(236, 226, 214, 150));
    let (buttons, bar) = player_controls(screen);
    let labels = ["<<", if view.playing { "||" } else { ">" }, ">>"];
    for (rect, label) in buttons.iter().zip(labels) {
        d.draw_rectangle_rec(*rect, Color::new(255, 214, 120, 60));
        let w = d.measure_text(label, 20);
        d.draw_text(label, rect.x as i32 + (rect.width as i32 - w) / 2, rect.y as i32 + 5, 20, MARKER);
    }
    d.draw_rectangle_rec(bar, Color::new(255, 255, 255, 40));
    let clock = |s: f32| format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32);
    let (fill, time) = if view.length > 0.0 {
        (
            (view.time / view.length).clamp(0.0, 1.0),
            format!("{} / {}", clock(view.time), clock(view.length)),
        )
    } else {
        // La marimba no se acaba: la barra va y viene.
        (0.5 + 0.5 * (view.time * 0.5).sin(), clock(view.time))
    };
    d.draw_rectangle(bar.x as i32, bar.y as i32, (bar.width * fill) as i32, bar.height as i32, MARKER);
    d.draw_text(&time, bar.x as i32, bar.y as i32 + 14, 14, TEXT);
    let volume = format!("vol {:.0}%", view.volume * 100.0);
    let w = d.measure_text(&volume, 14);
    d.draw_text(&volume, (bar.x + bar.width) as i32 - w, bar.y as i32 + 14, 14, TEXT);
}
