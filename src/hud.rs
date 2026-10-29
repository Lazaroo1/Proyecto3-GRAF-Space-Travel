//! HUD en 2D con el texto de raylib. No es parte del pipeline 3D: se dibuja
//! encima de la imagen ya renderizada.
//!
//! La letra es Segoe UI (la de Windows), cargada desde memoria con las
//! tildes y las enies, cada tamano en su propio atlas para que se vea
//! nitida. Si no esta, se usa la de raylib y el texto va sin tildes (esa
//! solo tiene ASCII). Los paneles son rectangulos redondeados con un borde
//! fino; las etiquetas van en gris y los valores en blanco. Los avisos salen
//! en una pildora abajo al centro, lejos del panel de controles.

use std::borrow::Cow;
use std::path::Path;

use raylib::prelude::*;

use crate::app::App;
use crate::parallel;
use crate::player::PlayerView;
use crate::scene::cycle::Phase;
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
/// Segundos que dura un aviso.
const TOAST: f32 = 3.0;

/// Donde estan las letras de Windows.
const FONT_DIR: &str = "C:/Windows/Fonts";
/// Lo que se carga de cada letra: ASCII y lo del espanol.
const GLYPHS: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~áéíóúÁÉÍÓÚñÑüÜäöÄÖ¿¡°·–—−…’";

/// Los estilos de letra: archivo y tamano en pixeles. Cada uno se dibuja a
/// su tamano, sin escalar.
#[derive(Clone, Copy)]
enum Style {
    Body,
    Bold,
    Small,
    Large,
    Heading,
    Title,
}

const STYLES: [(Style, &str, f32); 6] = [
    (Style::Body, "segoeui.ttf", 18.0),
    (Style::Bold, "segoeuib.ttf", 18.0),
    (Style::Small, "segoeui.ttf", 15.0),
    (Style::Large, "segoeui.ttf", 24.0),
    (Style::Heading, "segoeuib.ttf", 24.0),
    (Style::Title, "segoeuib.ttf", 120.0),
];

const LINE: f32 = 23.0;

const TEXT: Color = Color::new(242, 236, 226, 255);
const DIM: Color = Color::new(200, 190, 176, 210);
const FAINT: Color = Color::new(200, 190, 176, 160);
const PANEL: Color = Color::new(10, 10, 14, 178);
const EDGE: Color = Color::new(255, 255, 255, 30);
const ACCENT: Color = Color::new(255, 204, 120, 255);
const WARN: Color = Color::new(255, 118, 92, 255);

/// Las letras del HUD, una por estilo.
pub struct Fonts {
    fonts: Vec<WeakFont>,
    /// Si la letra tiene tildes (la de raylib no).
    unicode: bool,
}

impl Fonts {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let mut fonts = Vec::new();
        for (_, file, size) in STYLES {
            match load_font(rl, thread, file, size) {
                Some(font) => fonts.push(font),
                None => {
                    return Self {
                        fonts: STYLES.iter().map(|_| rl.get_font_default()).collect(),
                        unicode: false,
                    };
                }
            }
        }
        Self {
            fonts,
            unicode: true,
        }
    }

    fn font(&self, style: Style) -> (&WeakFont, f32) {
        (&self.fonts[style as usize], STYLES[style as usize].2)
    }

    /// El texto como se puede dibujar: sin tildes con la letra de raylib y,
    /// con Segoe, sin lo que no se cargo (de los nombres de las canciones).
    fn clean<'t>(&self, text: &'t str) -> Cow<'t, str> {
        if self.unicode && text.chars().all(|c| GLYPHS.contains(c)) {
            return text.into();
        }
        let plain = crate::player::ascii(text);
        if !self.unicode {
            return plain.into();
        }
        text.chars()
            .zip(plain.chars())
            .map(|(c, p)| if GLYPHS.contains(c) { c } else { p })
            .collect::<String>()
            .into()
    }

    fn width_at(&self, text: &str, style: Style, size: f32) -> f32 {
        let (font, _) = self.font(style);
        font.measure_text(&self.clean(text), size, 0.0).x
    }

    fn width(&self, text: &str, style: Style) -> f32 {
        self.width_at(text, style, self.font(style).1)
    }

    fn draw_at(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, style: Style, size: f32, color: Color) {
        let (font, _) = self.font(style);
        // En pixeles enteros, para que la letra no se emborrone.
        let at = Vector2::new(x.round(), y.round());
        d.draw_text_ex(font, &self.clean(text), at, size, 0.0, color);
    }

    fn draw(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, style: Style, color: Color) {
        self.draw_at(d, text, x, y, style, self.font(style).1, color);
    }
}

fn load_font(rl: &mut RaylibHandle, thread: &RaylibThread, file: &str, size: f32) -> Option<WeakFont> {
    let data = std::fs::read(Path::new(FONT_DIR).join(file)).ok()?;
    let font = rl
        .load_font_from_memory(thread, ".ttf", &data, size as i32, Some(GLYPHS))
        .ok()?;
    font.texture()
        .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
    Some(font.make_weak())
}

/// Los nombres con sus tildes (los internos van sin ellas porque con ellos
/// se buscan los cuerpos y los lugares).
fn pretty(name: &str) -> &str {
    match name {
        "Jardin" => "Jardín",
        "Vigia" => "Vigía",
        "Parpado" => "Párpado",
        "Volcan de Fuego" => "Volcán de Fuego",
        "Peten" => "Petén",
        "Atitlan" => "Atitlán",
        "Luna de Xelaju" => "Luna de Xelajú",
        "Los geiseres" => "Los géiseres",
        "San Andres Xecul" => "San Andrés Xecul",
        "Jagermeister" => "Jägermeister",
        "Gargantua" => "Gargantúa",
        other => other,
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "sí" } else { "no" }
}

/// Un triangulo en cualquier orden (raylib solo dibuja los de un sentido).
fn triangle(d: &mut RaylibDrawHandle, a: Vector2, b: Vector2, c: Vector2, color: Color) {
    let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if cross < 0.0 {
        d.draw_triangle(a, b, c, color);
    } else {
        d.draw_triangle(a, c, b, color);
    }
}

/// Un panel redondeado con su borde.
fn panel(d: &mut RaylibDrawHandle, rect: Rectangle) {
    let roundness = (20.0 / rect.height.min(rect.width)).min(1.0);
    d.draw_rectangle_rounded(rect, roundness, 10, PANEL);
    d.draw_rectangle_rounded_lines_ex(rect, roundness, 10, 1.0, EDGE);
}

/// Una pildora (los avisos, los nombres de los planetas).
fn pill(d: &mut RaylibDrawHandle, rect: Rectangle, fill: Color, edge: Option<Color>) {
    d.draw_rectangle_rounded(rect, 1.0, 12, fill);
    if let Some(edge) = edge {
        d.draw_rectangle_rounded_lines_ex(rect, 1.0, 12, 1.0, edge);
    }
}

/// Una fila del panel de vuelo: la etiqueta, la tecla y el valor.
struct Row {
    label: &'static str,
    key: &'static str,
    value: String,
    color: Color,
}

fn row(label: &'static str, key: &'static str, value: impl Into<String>) -> Row {
    Row {
        label,
        key,
        value: value.into(),
        color: TEXT,
    }
}

/// Un bloque de lineas sueltas (ayuda, estadisticas, ajustes). Devuelve
/// donde empieza arriba.
fn lines_panel(d: &mut RaylibDrawHandle, fonts: &Fonts, x: f32, y: f32, lines: &[String], from_bottom: Option<f32>) -> f32 {
    let pad = 14.0;
    let w = lines.iter().map(|l| fonts.width(l, Style::Body)).fold(0.0, f32::max) + pad * 2.0;
    let h = lines.len() as f32 * LINE + pad * 2.0 - 6.0;
    let y = from_bottom.map_or(y, |bottom| bottom - y - h);
    panel(d, Rectangle::new(x, y, w, h));
    for (i, line) in lines.iter().enumerate() {
        fonts.draw(d, line, x + pad, y + pad - 3.0 + i as f32 * LINE, Style::Body, TEXT);
    }
    y
}

/// Un aviso en una pildora abajo al centro, que aparece y se desvanece.
/// `slot` lo sube si hay otro aviso; `bottom` es hasta donde puede bajar.
fn toast(d: &mut RaylibDrawHandle, fonts: &Fonts, text: &str, age: f32, width: f32, bottom: f32, slot: f32) {
    let alpha = ((TOAST - age) / 0.5).clamp(0.0, 1.0) * (age / 0.15).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let w = fonts.width(text, Style::Heading) + 44.0;
    let h = 46.0;
    let x = (width - w) * 0.5;
    let y = bottom - h - slot * (h + 10.0);
    let fade = |c: Color| Color::new(c.r, c.g, c.b, (c.a as f32 * alpha) as u8);
    pill(d, Rectangle::new(x, y, w, h), fade(PANEL), Some(fade(Color::new(255, 204, 120, 110))));
    fonts.draw(d, text, x + 22.0, y + 9.0, Style::Heading, fade(Color::new(255, 222, 168, 255)));
}

pub fn draw(
    d: &mut RaylibDrawHandle,
    app: &App,
    fps: u32,
    hud: &Hud,
    tweaks: &Tweaks,
    fonts: &Fonts,
    player: Option<&PlayerView>,
) {
    let width = d.get_screen_width() as f32;
    let height = d.get_screen_height() as f32;
    if hud.hidden {
        return;
    }
    let chapin = app.bottle_name();

    // Intro: el titulo aparece y se desvanece en los primeros segundos.
    if hud.age < INTRO && !hud.intro_over {
        let fade = (hud.age / 1.2).min(1.0) * ((INTRO - hud.age) / 2.0).min(1.0);
        let title = "OCASO";
        let size = (height / 6.0).min(120.0);
        let w = fonts.width_at(title, Style::Title, size);
        let top = height / 3.0 - size * 0.62;
        fonts.draw_at(d, title, (width - w) * 0.5, top, Style::Title, size, Color::new(255, 190, 140, (fade * 255.0) as u8));
        let lines = [
            "Una estrella moribunda late como un corazón.",
            "En quince minutos colapsa y nace Gargantúa.",
            "F1: controles   ·   V: versión chapina",
        ];
        for (i, line) in lines.iter().enumerate() {
            let w = fonts.width(line, Style::Large);
            let y = top + size * 1.2 + i as f32 * 34.0;
            fonts.draw(d, line, (width - w) * 0.5, y, Style::Large, Color::new(242, 236, 226, (fade * 215.0) as u8));
        }
    }

    if hud.stats {
        let s = &app.stats;
        let t = &s.triangles;
        let terrain = match app.terrain_stats() {
            Some((name, ts)) => format!(
                "terreno {}: {} chunks (nivel {})   nuevos {}   pendientes {}   caché {}   horizonte −{}   frustum −{}{}",
                pretty(name),
                ts.drawn,
                ts.deepest,
                ts.generated,
                ts.pending,
                ts.cached,
                ts.culled_horizon,
                ts.culled_frustum,
                if app.lod.enabled { "" } else { "   [LOD APAGADO]" }
            ),
            None => "terreno: —".to_string(),
        };
        let p = &app.prop_stats;
        let lines = [
            format!("FPS {fps}   ·   frame {:.1} ms   ·   {} hilos", s.ms_total, parallel::workers()),
            format!(
                "vértices {:.1}   triángulos {:.1}   raster {:.1}   shading {:.1}   post {:.1} ms",
                s.ms_vertex, s.ms_triangles, s.ms_raster, s.ms_shade, s.ms_post
            ),
            format!("objetos {}   fuera de vista {}", s.objects, s.objects_culled),
            format!(
                "triángulos {}   frustum −{}   espalda −{}   sin pixel −{}",
                t.input, t.frustum, t.backface, t.small
            ),
            format!(
                "recortados {}   rasterizados {}   pixeles {}   partículas {}",
                t.clipped, t.emitted, s.pixels_shaded, s.particles
            ),
            terrain,
            format!(
                "objetos {} (detalle {}/{}/{})   chicos −{}   horizonte −{}   frustum −{}   semilla {}",
                p.drawn_total(),
                p.drawn[0],
                p.drawn[1],
                p.drawn[2],
                p.small,
                p.horizon,
                p.frustum,
                app.system.prop_seed
            ),
        ];
        lines_panel(d, fonts, 12.0, 12.0, &lines, None);
    }

    let flight = flight_panel(d, fonts, app, width);
    let panel_bottom = flight.y + flight.height;

    // Nombres de los cuerpos sobre su posicion en pantalla; los que caen
    // bajo el panel de vuelo no se dibujan (se enciman con su texto).
    for m in &app.markers {
        let name = pretty(m.name);
        let distance = if m.distance >= 1000.0 {
            format!("{:.1}k", m.distance / 1000.0)
        } else {
            format!("{:.0}", m.distance)
        };
        let nw = fonts.width(name, Style::Bold);
        let dw = fonts.width(&distance, Style::Small);
        let w = nw + 8.0 + dw + 20.0;
        let (x, y) = (m.x, m.y);
        let rect = Rectangle::new((x - w * 0.5).round(), y - 40.0, w.round(), 26.0);
        let below = Rectangle::new(rect.x, rect.y, rect.width, 40.0);
        if below.check_collision_recs(flight) {
            continue;
        }
        triangle(d, Vector2::new(x, y - 3.0), Vector2::new(x + 5.0, y - 10.0), Vector2::new(x - 5.0, y - 10.0), ACCENT);
        pill(d, rect, Color::new(10, 10, 14, 140), None);
        fonts.draw(d, name, rect.x + 10.0, rect.y + 2.0, Style::Bold, ACCENT);
        fonts.draw(d, &distance, rect.x + 18.0 + nw, rect.y + 5.0, Style::Small, DIM);
    }

    if tweaks.open {
        let lines = tweaks.lines(app);
        let h = lines.len() as f32 * LINE + 22.0;
        lines_panel(d, fonts, 12.0, (height - h) * 0.5, &lines, None);
    }

    // Lo que tapa abajo: los avisos van encima de la ayuda y del
    // reproductor si estan abiertos.
    let mut bottom = height - 64.0;
    if hud.help {
        let lines = [
            "Mouse: apuntar   ·   clic derecho: mirar alrededor   ·   rueda: distancia".to_string(),
            "W / S: adelante / atrás   ·   A / D: lados   ·   Espacio / Ctrl: subir / bajar".to_string(),
            "Q / E: alabeo   ·   Shift: turbo   ·   C: cámara   ·   P: foto   ·   Tab: soltar el mouse".to_string(),
            "X: freno de emergencia   ·   H (o Inicio): volver al principio sin reiniciar el tiempo".to_string(),
            "L: modo local   ·   O: crucero (rodea el planeta)   ·   N: ir al siguiente lugar".to_string(),
            "Y: ruta predicha, la línea de a dónde te lleva la gravedad (auto / siempre / nunca)".to_string(),
            "V: versión chapina / OCASO   ·   B o 1 a 6: nave botella (en la chapina)".to_string(),
            "K: pausa   ·   U / I: canción anterior / siguiente   ·   − / +: volumen (en la chapina)".to_string(),
            "T: ajustes en vivo   ·   M: sonido   ·   J: final / renacer   ·   R: rebobinar   ·   [ ]: −30 / +30 s".to_string(),
            "RePág / AvPág: vistas fijas   ·   F3: wireframe   ·   F4: colores de LOD   ·   F5: LOD".to_string(),
            "F6: otra semilla   ·   F7: congelar frustum   ·   F1: ayuda   ·   F2: estadísticas   ·   F9: ocultar todo".to_string(),
        ];
        let top = lines_panel(d, fonts, 12.0, 12.0, &lines, Some(height));
        bottom = bottom.min(top - 14.0);
    }
    if app.freeze.is_some() {
        let text = "Frustum congelado (F7): el culling y el LOD se calculan desde la cámara amarilla";
        let w = fonts.width(text, Style::Bold);
        fonts.draw(d, text, (width - w) * 0.5, height - 40.0, Style::Bold, ACCENT);
    }
    if let Some(view) = player {
        draw_player(d, fonts, view, (width as i32, height as i32), hud.age);
    }

    // Los avisos: el del juego y el de la vista fija.
    let mut slot = 0.0;
    if let Some((text, since)) = hud.banner {
        if hud.age - since < TOAST {
            toast(d, fonts, pretty(text), hud.age - since, width, bottom, slot);
            slot += 1.0;
        }
    }
    if let Some((name, since)) = hud.preset {
        toast(d, fonts, &format!("Vista fija: {name}"), hud.age - since, width, bottom, slot);
    }

    // El titulo grande de la transicion entre versiones: debajo del panel
    // de vuelo, para no encimarse con el.
    if let Some((title, fade)) = app.transition_title() {
        let size = (height / 7.0).min(120.0);
        let w = fonts.width_at(title, Style::Title, size);
        let top = (height * 0.56 - size * 0.62).max(panel_bottom + 12.0);
        let color = if chapin.is_some() {
            Color::new(255, 214, 120, (fade * 255.0) as u8)
        } else {
            Color::new(255, 190, 140, (fade * 255.0) as u8)
        };
        fonts.draw_at(d, title, (width - w) * 0.5, top, Style::Title, size, color);
    }
}

/// El panel de vuelo, arriba a la derecha: un encabezado con la fase o la
/// version, las filas (etiqueta, tecla, valor) y una linea con las teclas
/// de emergencia. Devuelve el rectangulo que ocupa.
fn flight_panel(d: &mut RaylibDrawHandle, fonts: &Fonts, app: &App, width: f32) -> Rectangle {
    let f = &app.flight;
    let c = &app.cycle;
    let clock = |seconds: f64| format!("{:02}:{:02}", (seconds / 60.0) as u32, (seconds % 60.0) as u32);
    let (title, hint) = match (app.bottle_name(), c.phase) {
        (Some(_), _) => ("Versión chapina".to_string(), "V: volver a OCASO"),
        (None, Phase::Agony) => (Phase::Agony.name().to_string(), "J: desatar el final"),
        (None, phase) => (format!("{}   {}", phase.name(), clock(c.tau)), "J: renacer"),
    };
    let body = |i| pretty(app.body_name(i));
    let local = match (app.settings.local, f.local, f.hold) {
        (false, _, _) => "no".to_string(),
        (true, true, Some(h)) => format!("sobre {}, a {:.0}", body(f.frame), h),
        (true, true, None) => format!("sobre {}", body(f.frame)),
        (true, false, _) if !app.settings.assist => "sí (necesita asistencia)".to_string(),
        (true, false, _) => "sí (lejos de un planeta)".to_string(),
    };
    let path = match app.path_mode {
        None => "auto (sin asistencia)",
        Some(true) => "siempre",
        Some(false) => "nunca",
    };
    let mut rows = vec![
        row("Velocidad", "", format!("{:.1} u/s   (máx {:.0})", f.speed, f.max_speed)),
        row("Altitud", "", format!("{:.0} sobre {}", f.altitude, body(f.nearest))),
        row("Marco", "", body(f.frame)),
        row("Asistencia", "Z", on_off(app.settings.assist)),
        row("Gravedad", "G", on_off(app.settings.gravity)),
        row("Modo local", "L", local),
        row("Crucero", "O", on_off(app.settings.cruise)),
        row("Ruta predicha", "Y", path),
    ];
    if f.contact {
        rows[2].value.push_str("   ·   contacto");
    }
    if app.path_shown && app.path_impact {
        let last = rows.last_mut().unwrap();
        last.value.push_str("   ·   choque");
        last.color = WARN;
    }
    rows.push(match app.bottle_name() {
        Some(name) => row("Nave", "B / 1-6", pretty(name)),
        None => row("Guatemala", "V", "versión chapina"),
    });
    let (footer, footer_color) = if f.braking {
        ("FRENO ACTIVO   (X)", WARN)
    } else if f.boundary {
        ("Límite del sistema: gira para regresar", WARN)
    } else {
        ("X frenar   ·   N siguiente lugar   ·   H volver   ·   F1 ayuda", FAINT)
    };

    let pad = 16.0;
    let label_w = rows.iter().map(|r| fonts.width(r.label, Style::Body)).fold(0.0, f32::max);
    let key_w = rows.iter().map(|r| fonts.width(r.key, Style::Small)).fold(0.0, f32::max) + 14.0;
    let value_x = label_w + 10.0 + key_w + 8.0;
    let value_w = rows.iter().map(|r| fonts.width(&r.value, Style::Body)).fold(0.0, f32::max);
    // Ancho fijo minimo para que el panel no tiemble cuando cambian los numeros.
    let w = (value_x + value_w)
        .max(fonts.width(footer, Style::Small))
        .max(fonts.width(&title, Style::Heading) + 16.0 + fonts.width(hint, Style::Small))
        .max(340.0)
        + pad * 2.0;
    let h = pad + 32.0 + 10.0 + rows.len() as f32 * LINE + 10.0 + 18.0 + pad - 4.0;
    let x = (width - 12.0 - w).round();
    let y = 12.0;
    panel(d, Rectangle::new(x, y, w, h));

    fonts.draw(d, &title, x + pad, y + pad - 4.0, Style::Heading, ACCENT);
    let hw = fonts.width(hint, Style::Small);
    fonts.draw(d, hint, x + w - pad - hw, y + pad + 4.0, Style::Small, DIM);
    let mut cy = y + pad + 34.0;
    d.draw_rectangle(x as i32 + pad as i32, cy as i32, (w - pad * 2.0) as i32, 1, EDGE);
    cy += 8.0;
    for r in &rows {
        fonts.draw(d, r.label, x + pad, cy, Style::Body, DIM);
        if !r.key.is_empty() {
            // La tecla en una cajita.
            let kw = fonts.width(r.key, Style::Small) + 10.0;
            let kx = x + pad + label_w + 10.0;
            d.draw_rectangle_rounded(Rectangle::new(kx, cy + 3.0, kw, 18.0), 0.35, 6, Color::new(255, 255, 255, 26));
            fonts.draw(d, r.key, kx + 5.0, cy + 3.0, Style::Small, DIM);
        }
        fonts.draw(d, &r.value, x + pad + value_x, cy, Style::Body, r.color);
        cy += LINE;
    }
    cy += 6.0;
    d.draw_rectangle(x as i32 + pad as i32, cy as i32, (w - pad * 2.0) as i32, 1, EDGE);
    fonts.draw(d, footer, x + pad, cy + 6.0, Style::Small, footer_color);
    Rectangle::new(x, y, w, h)
}

/// Tamano y lugar del reproductor: abajo a la derecha.
const PLAYER_SIZE: (i32, i32) = (400, 138);

fn player_rect(screen: (i32, i32)) -> (i32, i32) {
    (screen.0 - PLAYER_SIZE.0 - 12, screen.1 - PLAYER_SIZE.1 - 12)
}

/// Los tres botones (anterior, pausa, siguiente) y la barra de avance.
fn player_controls(screen: (i32, i32)) -> ([Rectangle; 3], Rectangle) {
    let (x, y) = player_rect(screen);
    let (x, y) = (x as f32, y as f32);
    let button = |cx: f32, size: f32| Rectangle::new(x + cx - size * 0.5, y + 98.0 - size * 0.5, size, size);
    let bar = Rectangle::new(x + 86.0, y + 60.0, PLAYER_SIZE.0 as f32 - 102.0, 5.0);
    ([button(150.0, 32.0), button(196.0, 40.0), button(242.0, 32.0)], bar)
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
    let grown = Rectangle::new(bar.x, bar.y - 8.0, bar.width, bar.height + 16.0);
    grown
        .check_collision_point_rec(point)
        .then(|| PlayerHit::Seek(((mouse.0 - bar.x) / bar.width).clamp(0.0, 1.0)))
}

/// El mini reproductor: una portada de tejido con un ecualizador que se
/// mueve mientras suena, el nombre de la cancion, la barra de avance con su
/// perilla y los tiempos, los botones (dibujados, no letras), el volumen y
/// las teclas.
fn draw_player(d: &mut RaylibDrawHandle, fonts: &Fonts, view: &PlayerView, screen: (i32, i32), age: f32) {
    let (x, y) = player_rect(screen);
    let (x, y) = (x as f32, y as f32);
    panel(d, Rectangle::new(x, y, PLAYER_SIZE.0 as f32, PLAYER_SIZE.1 as f32));

    // La portada: un cuadro rojo de huipil con franjas y el ecualizador.
    let cover = Rectangle::new(x + 16.0, y + 16.0, 58.0, 58.0);
    d.draw_rectangle_rounded(cover, 0.22, 8, Color::new(142, 26, 48, 255));
    let stripes = [
        Color::new(240, 184, 60, 255),
        Color::new(40, 150, 140, 255),
        Color::new(214, 70, 140, 255),
    ];
    for (k, color) in stripes.iter().enumerate() {
        let sy = cover.y + 40.0 + k as f32 * 5.0;
        // Una franja en zigzag, de triangulitos.
        for i in 0..7 {
            let sx = cover.x + 5.0 + i as f32 * 7.0;
            triangle(d, Vector2::new(sx, sy + 4.0), Vector2::new(sx + 3.5, sy), Vector2::new(sx + 7.0, sy + 4.0), *color);
        }
    }
    for i in 0..5 {
        let t = age * 6.0 + i as f32 * 1.7;
        let level = if view.playing {
            0.35 + 0.65 * (t.sin() * 0.5 + 0.5) * ((t * 0.37 + i as f32).cos() * 0.3 + 0.7)
        } else {
            0.18
        };
        let bh = 4.0 + 26.0 * level;
        let bx = cover.x + 10.0 + i as f32 * 8.0;
        d.draw_rectangle_rounded(Rectangle::new(bx, cover.y + 36.0 - bh, 5.0, bh), 0.6, 4, TEXT);
    }

    // El nombre (cortado si no cabe) y cual es de cuantas.
    let room = PLAYER_SIZE.0 as f32 - 102.0;
    let mut title = view.title.clone();
    if fonts.width(&title, Style::Bold) > room {
        while !title.is_empty() && fonts.width(&format!("{title}…"), Style::Bold) > room {
            title.pop();
        }
        title = format!("{}…", title.trim_end());
    }
    fonts.draw(d, &title, x + 86.0, y + 14.0, Style::Bold, TEXT);
    let state = if view.playing { "sonando" } else { "en pausa" };
    let count = format!("{} de {}   ·   {state}", view.index + 1, view.count);
    fonts.draw(d, &count, x + 86.0, y + 36.0, Style::Small, DIM);

    // La barra de avance con su perilla y los tiempos.
    let (buttons, bar) = player_controls(screen);
    let clock = |s: f32| format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32);
    let (fill, right) = if view.length > 0.0 {
        ((view.time / view.length).clamp(0.0, 1.0), clock(view.length))
    } else {
        // La marimba no se acaba: la perilla va y viene.
        (0.5 + 0.5 * (view.time * 0.5).sin(), "en vivo".to_string())
    };
    d.draw_rectangle_rounded(bar, 1.0, 6, Color::new(255, 255, 255, 38));
    if fill > 0.0 {
        d.draw_rectangle_rounded(Rectangle::new(bar.x, bar.y, (bar.width * fill).max(bar.height), bar.height), 1.0, 6, ACCENT);
    }
    let knob = Vector2::new(bar.x + bar.width * fill, bar.y + bar.height * 0.5);
    d.draw_circle_v(knob, 7.0, Color::new(0, 0, 0, 90));
    d.draw_circle_v(knob, 6.0, TEXT);
    fonts.draw(d, &clock(view.time), bar.x, bar.y + 10.0, Style::Small, DIM);
    let rw = fonts.width(&right, Style::Small);
    fonts.draw(d, &right, bar.x + bar.width - rw, bar.y + 10.0, Style::Small, DIM);

    // Los botones: anterior, pausa (un circulo de color) y siguiente.
    let center = |r: &Rectangle| (r.x + r.width * 0.5, r.y + r.height * 0.5);
    let (px, py) = center(&buttons[0]);
    d.draw_rectangle_rounded(Rectangle::new(px - 9.0, py - 7.0, 3.0, 14.0), 0.5, 2, TEXT);
    triangle(d, Vector2::new(px + 8.0, py - 7.0), Vector2::new(px - 5.0, py), Vector2::new(px + 8.0, py + 7.0), TEXT);
    let (cx, cy) = center(&buttons[1]);
    d.draw_circle_v(Vector2::new(cx, cy), 19.0, ACCENT);
    let dark = Color::new(28, 18, 12, 255);
    if view.playing {
        d.draw_rectangle_rounded(Rectangle::new(cx - 6.5, cy - 8.0, 4.5, 16.0), 0.4, 2, dark);
        d.draw_rectangle_rounded(Rectangle::new(cx + 2.0, cy - 8.0, 4.5, 16.0), 0.4, 2, dark);
    } else {
        triangle(d, Vector2::new(cx - 5.0, cy - 9.0), Vector2::new(cx - 5.0, cy + 9.0), Vector2::new(cx + 10.0, cy), dark);
    }
    let (nx, ny) = center(&buttons[2]);
    triangle(d, Vector2::new(nx - 8.0, ny - 7.0), Vector2::new(nx + 5.0, ny), Vector2::new(nx - 8.0, ny + 7.0), TEXT);
    d.draw_rectangle_rounded(Rectangle::new(nx + 6.0, ny - 7.0, 3.0, 14.0), 0.5, 2, TEXT);

    // El volumen: una bocina y su barra.
    let vx = x + 282.0;
    let vy = y + 98.0;
    d.draw_rectangle(vx as i32, (vy - 3.0) as i32, 4, 7, DIM);
    triangle(d, Vector2::new(vx + 4.0, vy - 3.0), Vector2::new(vx + 11.0, vy - 8.0), Vector2::new(vx + 11.0, vy + 8.0), DIM);
    triangle(d, Vector2::new(vx + 4.0, vy - 3.0), Vector2::new(vx + 11.0, vy + 8.0), Vector2::new(vx + 4.0, vy + 4.0), DIM);
    let vol = Rectangle::new(vx + 20.0, vy - 2.0, PLAYER_SIZE.0 as f32 - 16.0 - (vx + 20.0 - x), 4.0);
    d.draw_rectangle_rounded(vol, 1.0, 6, Color::new(255, 255, 255, 38));
    if view.volume > 0.0 {
        d.draw_rectangle_rounded(Rectangle::new(vol.x, vol.y, (vol.width * view.volume).max(vol.height), vol.height), 1.0, 6, DIM);
    }

    // Las teclas, abajo y chiquitas.
    let keys = "K pausa   ·   U / I canción   ·   − / + volumen";
    let kw = fonts.width(keys, Style::Small);
    fonts.draw(d, keys, x + (PLAYER_SIZE.0 as f32 - kw) * 0.5, y + PLAYER_SIZE.1 as f32 - 24.0, Style::Small, FAINT);
}
