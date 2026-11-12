//! El HUD de piloto: sin placas ni cajas, lineas finas dibujadas sobre la
//! vista como en la cabina de un avion de combate. Cada version con su
//! color (OCASO en ambar de brasa, la chapina en jade) y todo con una sombra
//! oscura debajo, para que se lea sobre el cielo de dia o sobre la estrella.
//!
//! - **Rumbo**, arriba al centro: una cinta de brujula sobre el plano de las
//!   orbitas (`App::compass`) con cada cuerpo marcado donde queda.
//! - **Velocidad** a la izquierda y **altitud** a la derecha: cintas que
//!   corren, con el valor en una caja que apunta a la cinta. El paso de las
//!   marcas sale de la escala (1, 2 o 5 por una potencia de 10).
//! - **Reticula** al centro: hacia donde apunta la camara.
//! - **Modos** abajo al centro: cada tecla es una luz, encendida si esta
//!   puesto.
//! - **Un solo aviso a la vez**, arriba bajo el rumbo (lo urgente le gana a
//!   un lugar, y un lugar a una vista fija), y el minijuego que se ofrece
//!   abajo. Las marcas de los cuerpos y los lugares no se dibujan donde se
//!   enciman con el HUD o con otra marca mas cercana.
//! - **La musica**, abajo a la derecha: el barrilete que gira, el titulo, el
//!   artista y una linea de avance con sus botones.

use raylib::prelude::*;

use super::{Align, Cap, Fonts, Key, Style, Ui, clipped, fade, pretty, song_parts, triangle};
use crate::app::{App, Compass};
use crate::player::PlayerView;

/// Los colores de una version.
#[derive(Clone, Copy)]
pub(super) struct Theme {
    /// Las lineas y lo que manda.
    pub ink: Color,
    /// El texto neutro.
    pub text: Color,
    pub warn: Color,
    /// Lo de adentro de una tecla encendida.
    pub dark: Color,
    /// Cuanto se ve todo (entra desvaneciendose al acabar la bienvenida).
    pub alpha: f32,
}

impl Theme {
    pub fn of(chapin: bool) -> Self {
        if chapin {
            Self {
                ink: Color::new(118, 240, 178, 255),
                text: Color::new(234, 255, 244, 255),
                warn: Color::new(255, 104, 124, 255),
                dark: Color::new(6, 30, 22, 255),
                alpha: 1.0,
            }
        } else {
            Self {
                ink: Color::new(255, 172, 96, 255),
                text: Color::new(255, 238, 220, 255),
                warn: Color::new(255, 86, 64, 255),
                dark: Color::new(34, 14, 6, 255),
                alpha: 1.0,
            }
        }
    }

    /// El mismo, a `alpha` de lo que se ve.
    pub fn faded(self, alpha: f32) -> Self {
        Self { ink: fade(self.ink, alpha), text: fade(self.text, alpha), warn: fade(self.warn, alpha), dark: fade(self.dark, alpha), alpha: self.alpha * alpha }
    }

    /// Un velo negro a `a` (de 255), con lo que se ve.
    pub fn veil(&self, a: u8) -> Color {
        Color::new(0, 0, 0, (a as f32 * self.alpha) as u8)
    }

    pub fn soft(&self) -> Color {
        fade(self.ink, 0.6)
    }

    pub fn faint(&self) -> Color {
        fade(self.ink, 0.32)
    }
}

/// La sombra de todo: negro a medias.
fn shadow(c: Color) -> Color {
    Color::new(0, 0, 0, (c.a as f32 * 0.62) as u8)
}

/// Una linea con su sombra.
pub(super) fn line(d: &mut RaylibDrawHandle, a: Vector2, b: Vector2, width: f32, color: Color) {
    d.draw_line_ex(a, b, width + 2.0, shadow(color));
    d.draw_line_ex(a, b, width, color);
}

/// Un texto con su sombra, alineado en `x`. Devuelve su ancho.
#[allow(clippy::too_many_arguments)]
pub(super) fn text(d: &mut RaylibDrawHandle, fonts: &Fonts, s: &str, x: f32, y: f32, style: Style, color: Color, align: Align) -> f32 {
    let w = fonts.width(s, style);
    let x = match align {
        Align::Left => x,
        Align::Center => x - w * 0.5,
        Align::Right => x - w,
    };
    let k = fonts.scale().max(1.0);
    fonts.draw(d, s, x + k, y + k, style, shadow(color));
    fonts.draw(d, s, x, y, style, color);
    w
}

/// Un texto en mayusculas con aire entre las letras (las etiquetas).
#[allow(clippy::too_many_arguments)]
fn caps(d: &mut RaylibDrawHandle, fonts: &Fonts, s: &str, x: f32, y: f32, style: Style, color: Color, align: Align) -> f32 {
    let s = s.to_uppercase();
    let size = fonts.font(style).1;
    let spacing = 1.6 * fonts.scale();
    let w = fonts.width_spaced(&s, style, size, spacing);
    let x = match align {
        Align::Left => x,
        Align::Center => x - w * 0.5,
        Align::Right => x - w,
    };
    let k = fonts.scale().max(1.0);
    fonts.draw_spaced(d, &s, x + k, y + k, style, size, spacing, shadow(color));
    fonts.draw_spaced(d, &s, x, y, style, size, spacing, color);
    w
}

/// Un rectangulo con orilla de una linea y su sombra.
fn outline(d: &mut RaylibDrawHandle, r: Rectangle, width: f32, color: Color) {
    d.draw_rectangle_lines_ex(Rectangle::new(r.x - 1.0, r.y - 1.0, r.width + 2.0, r.height + 2.0), width + 2.0, shadow(color));
    d.draw_rectangle_lines_ex(r, width, color);
}

/// Un velo oscuro en una esquina de la pantalla, que se pierde hacia el
/// centro (para que se lea sobre el cielo de dia sin ponerle caja): la
/// esquina `corner` es 0 arriba a la izquierda, 1 arriba a la derecha, 2
/// abajo a la derecha y 3 abajo a la izquierda.
pub(super) fn wash(d: &mut RaylibDrawHandle, th: &Theme, r: Rectangle, corner: usize) {
    let dark = th.veil(110);
    let clear = Color::new(0, 0, 0, 0);
    // raylib: arriba-izquierda, abajo-izquierda, abajo-derecha, arriba-derecha.
    let mut c = [clear; 4];
    c[[0, 3, 2, 1][corner % 4]] = dark;
    d.draw_rectangle_gradient_ex(r, c[0], c[1], c[2], c[3]);
}

// Las teclas ---------------------------------------------------------------

/// El ancho de la tecla `key`.
pub(super) fn chip_width(fonts: &Fonts, key: &str) -> f32 {
    let k = fonts.scale();
    (fonts.width(key, Style::Small) + 10.0 * k).max(20.0 * k)
}

/// Una tecla de 20 pixeles de alto: una caja de una linea con su letra
/// (encendida: llena). Devuelve su ancho.
#[allow(clippy::too_many_arguments)]
pub(super) fn chip(d: &mut RaylibDrawHandle, fonts: &Fonts, th: &Theme, key: &str, x: f32, y: f32, look: Cap, alpha: f32) -> f32 {
    let k = fonts.scale();
    let w = chip_width(fonts, key);
    let r = Rectangle::new(x.round(), y.round(), w.round(), (20.0 * k).round());
    let (fill, edge, ink) = match look {
        Cap::On => (Some(th.ink), th.ink, th.dark),
        Cap::Warn => (Some(th.warn), th.warn, th.dark),
        Cap::Off => (None, th.faint(), th.faint()),
        Cap::Plain => (None, th.ink, th.ink),
    };
    match fill {
        Some(fill) => {
            d.draw_rectangle_rec(Rectangle::new(r.x - 1.0, r.y - 1.0, r.width + 2.0, r.height + 2.0), fade(shadow(fill), alpha));
            d.draw_rectangle_rec(r, fade(fill, alpha));
        }
        None => outline(d, r, 1.0, fade(edge, alpha)),
    }
    let tw = fonts.width(key, Style::Small);
    fonts.draw(d, key, r.x + (r.width - tw) * 0.5, r.y + 1.0 * k, Style::Small, fade(ink, alpha));
    w
}

// El rumbo -------------------------------------------------------------------

/// Cuantos grados se ven a cada lado del rumbo.
const COMPASS_HALF: f32 = 40.0;

/// La cinta de rumbo: la linea base en `y`, marcas cada 5 grados (mas
/// largas cada 10), los puntos cardinales y los numeros cada 30 (en
/// decenas: "03" es 30), la caja con el rumbo abajo al centro y los cuerpos
/// como rombos sobre la linea (con su nombre si cabe).
pub(super) fn compass(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, compass: &Compass, cx: f32, y: f32, width: f32) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let half = width * 0.5;
    let per_degree = half / COMPASS_HALF;
    let heading = compass.heading;
    let rel = |b: f32| (b - heading + 540.0).rem_euclid(360.0) - 180.0;
    // Se desvanece hacia las puntas.
    let edge = |x: f32| (1.0 - ((x - cx).abs() / half).powi(4)).clamp(0.0, 1.0);
    line(d, Vector2::new(cx - half, y), Vector2::new(cx + half, y), 1.0, th.faint());
    let first = ((heading - COMPASS_HALF) / 5.0).ceil() as i32;
    let last = ((heading + COMPASS_HALF) / 5.0).floor() as i32;
    for step in first..=last {
        let deg = step * 5;
        let x = cx + rel(deg as f32) * per_degree;
        let a = edge(x);
        let major = deg.rem_euclid(10) == 0;
        let tall = if major { 9.0 } else { 5.0 } * k;
        line(d, Vector2::new(x, y), Vector2::new(x, y - tall), 1.0, fade(th.ink, 0.85 * a));
        let deg = deg.rem_euclid(360);
        if deg % 30 == 0 {
            let label = match deg {
                0 => "N".to_string(),
                90 => "E".to_string(),
                180 => "S".to_string(),
                270 => "O".to_string(),
                d => format!("{:02}", d / 10),
            };
            let style = if deg % 90 == 0 { Style::Bold } else { Style::Small };
            text(d, fonts, &label, x, y - tall - 20.0 * k, style, fade(th.ink, a), Align::Center);
        }
    }
    // Los cuerpos: un rombo sobre la linea y su nombre abajo, si no choca
    // con la caja del rumbo ni con otro nombre.
    let box_w = 54.0 * k;
    let mut taken: Vec<(f32, f32)> = vec![(cx - box_w * 0.5 - 6.0 * k, cx + box_w * 0.5 + 6.0 * k)];
    let mut bodies: Vec<(&str, f32)> = compass.bodies.iter().map(|&(n, b)| (n, rel(b))).filter(|(_, r)| r.abs() <= COMPASS_HALF).collect();
    bodies.sort_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    for (name, r) in bodies {
        let x = cx + r * per_degree;
        let a = edge(x);
        d.draw_poly(Vector2::new(x, y), 4, 4.5 * k + 1.5, 0.0, shadow(th.text));
        d.draw_poly(Vector2::new(x, y), 4, 4.5 * k, 0.0, fade(th.text, a));
        let name = pretty(name);
        let w = fonts.width(name, Style::Small);
        let span = (x - w * 0.5 - 4.0 * k, x + w * 0.5 + 4.0 * k);
        if taken.iter().all(|&(lo, hi)| span.1 < lo || span.0 > hi) {
            text(d, fonts, name, x, y + 8.0 * k, Style::Small, fade(th.text, 0.85 * a), Align::Center);
            taken.push(span);
        }
    }
    // La caja del rumbo, con su pico hacia la linea.
    let value = format!("{:03.0}", heading.round().rem_euclid(360.0));
    let r = Rectangle::new((cx - box_w * 0.5).round(), (y + 8.0 * k).round(), box_w.round(), (24.0 * k).round());
    d.draw_rectangle_rec(r, th.veil(120));
    outline(d, r, 1.0, th.ink);
    triangle(d, Vector2::new(cx - 5.0 * k, r.y), Vector2::new(cx + 5.0 * k, r.y), Vector2::new(cx, y + 2.0 * k), th.ink);
    text(d, fonts, &value, cx, r.y + 1.0 * k, Style::Bold, th.text, Align::Center);
    Rectangle::new(cx - half, y - 48.0 * k, width, 48.0 * k + 34.0 * k)
}

// Las cintas -----------------------------------------------------------------

/// Hacia que lado de la pantalla va una cinta (sus marcas apuntan al
/// centro).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Side {
    Left,
    Right,
}

/// Lo que muestra una cinta.
pub(super) struct Tape<'a> {
    pub value: f64,
    /// La escala: cuanto abarca la cinta de punta a punta.
    pub span: f64,
    pub label: &'a str,
    pub unit: &'a str,
    /// Un tope marcado en la cinta (la velocidad maxima).
    pub limit: Option<f64>,
    /// Encendida (pasada del tope).
    pub hot: bool,
    /// Lineas abajo de la cinta.
    pub notes: Vec<String>,
}

/// El paso de las marcas para que entren unas cinco: 1, 2 o 5 por una
/// potencia de 10.
fn nice_step(range: f64) -> f64 {
    let raw = (range / 5.0).max(1e-9);
    let p = 10f64.powf(raw.log10().floor());
    let m = raw / p;
    p * if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    }
}

/// Una cinta vertical con su linea en `x`, de `top` a `top + height`: las
/// marcas corren con el valor (cada paso con su numero, cinco marcas chicas
/// entre dos), se desvanecen hacia las puntas y el valor va en una caja con
/// pico, del lado del centro. Devuelve lo que ocupa.
#[allow(clippy::too_many_arguments)]
pub(super) fn tape(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, side: Side, x: f32, top: f32, height: f32, t: &Tape) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let mid = top + height * 0.5;
    let step = nice_step(t.span);
    let per_unit = height as f64 / t.span;
    let inward = if side == Side::Left { 1.0 } else { -1.0 };
    let color = if t.hot { th.warn } else { th.ink };
    line(d, Vector2::new(x, top), Vector2::new(x, top + height), 1.0, th.faint());
    let minor = step / 5.0;
    let lo = ((t.value - t.span * 0.5) / minor).ceil() as i64;
    let hi = ((t.value + t.span * 0.5) / minor).floor() as i64;
    for n in lo..=hi {
        let v = n as f64 * minor;
        if v < -1e-9 {
            continue;
        }
        let y = mid - ((v - t.value) * per_unit) as f32;
        let a = (1.0 - ((y - mid).abs() / (height * 0.5)).powi(3)).clamp(0.0, 1.0);
        let major = n.rem_euclid(5) == 0;
        let long = if major { 12.0 } else { 6.0 } * k;
        line(d, Vector2::new(x, y), Vector2::new(x + inward * long, y), 1.0, fade(color, 0.85 * a));
        if major {
            let label = super::grouped(v);
            let (lx, align) = if side == Side::Left { (x - 8.0 * k, Align::Right) } else { (x + 8.0 * k, Align::Left) };
            text(d, fonts, &label, lx, y - 9.0 * k, Style::Small, fade(th.text, 0.8 * a), align);
        }
    }
    // El tope: un corchete en la cinta.
    if let Some(limit) = t.limit {
        let y = mid - ((limit - t.value) * per_unit) as f32;
        if (y - mid).abs() < height * 0.5 {
            let x2 = x + inward * 18.0 * k;
            line(d, Vector2::new(x, y), Vector2::new(x2, y), 2.0, th.warn);
            line(d, Vector2::new(x2, y), Vector2::new(x2, y + 6.0 * k), 2.0, th.warn);
        }
    }
    // La caja del valor, del lado del centro, con su pico hacia la linea.
    let value = super::grouped(t.value);
    let w = (fonts.width(&value, Style::Heading) + 18.0 * k).max(64.0 * k);
    let h = 30.0 * k;
    let bx = if side == Side::Left { x + 14.0 * k } else { x - 14.0 * k - w };
    let r = Rectangle::new(bx.round(), (mid - h * 0.5).round(), w.round(), h.round());
    d.draw_rectangle_rec(r, th.veil(130));
    outline(d, r, 1.5, color);
    let tip = Vector2::new(x + inward * 3.0 * k, mid);
    let base = if side == Side::Left { r.x } else { r.x + r.width };
    triangle(d, tip, Vector2::new(base, mid - 6.0 * k), Vector2::new(base, mid + 6.0 * k), color);
    text(d, fonts, &value, r.x + r.width * 0.5, r.y + 1.0 * k, Style::Heading, if t.hot { th.warn } else { th.text }, Align::Center);
    // La etiqueta arriba y las notas abajo.
    let (align, lx) = if side == Side::Left { (Align::Left, x - 30.0 * k) } else { (Align::Right, x + 30.0 * k) };
    let lw = caps(d, fonts, t.label, lx, top - 34.0 * k, Style::Bold, th.ink, align);
    let ux = if side == Side::Left { lx + lw + 8.0 * k } else { lx - lw - 8.0 * k };
    text(d, fonts, t.unit, ux, top - 32.0 * k, Style::Small, th.soft(), align);
    for (i, note) in t.notes.iter().enumerate() {
        text(d, fonts, note, lx, top + height + 10.0 * k + i as f32 * 19.0 * k, Style::Small, if i == 0 { th.text } else { th.soft() }, align);
    }
    let (x0, x1) = if side == Side::Left { (x - 80.0 * k, r.x + r.width) } else { (r.x, x + 80.0 * k) };
    Rectangle::new(x0, top - 40.0 * k, x1 - x0, height + 50.0 * k + t.notes.len() as f32 * 19.0 * k)
}

// La reticula y los modos ------------------------------------------------------

/// Hacia donde apunta la camara: un circulito abierto con sus alas.
pub(super) fn reticle(d: &mut RaylibDrawHandle, th: &Theme, cx: f32, cy: f32, k: f32) {
    let c = fade(th.ink, 0.7);
    let center = Vector2::new(cx, cy);
    d.draw_ring(center, 6.0 * k, 6.0 * k + 3.0, 0.0, 360.0, 32, shadow(c));
    d.draw_ring(center, 6.0 * k + 0.5, 6.0 * k + 2.0, 0.0, 360.0, 32, c);
    for (a, b) in [((-22.0, 0.0), (-10.0, 0.0)), ((10.0, 0.0), (22.0, 0.0)), ((0.0, -10.0), (0.0, -16.0))] {
        line(d, Vector2::new(cx + a.0 * k, cy + a.1 * k), Vector2::new(cx + b.0 * k, cy + b.1 * k), 1.5, c);
    }
}

/// Las luces de los modos en un renglon centrado en `cx`: la tecla
/// (encendida si el modo esta puesto) y su nombre. Devuelve lo que ocupa.
pub(super) fn modes(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, items: &[(&str, &str, Cap)], cx: f32, y: f32) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let gap = 20.0 * k;
    let widths: Vec<f32> = items.iter().map(|(key, name, _)| chip_width(fonts, key) + 6.0 * k + fonts.width_spaced(&name.to_uppercase(), Style::Small, fonts.font(Style::Small).1, 1.6 * k)).collect();
    let total = widths.iter().sum::<f32>() + gap * (items.len().saturating_sub(1)) as f32;
    let mut x = cx - total * 0.5;
    for ((key, name, look), w) in items.iter().zip(&widths) {
        let cw = chip(d, fonts, th, key, x, y, *look, 1.0);
        let color = match look {
            Cap::Warn => th.warn,
            Cap::Off => th.faint(),
            Cap::Plain => th.soft(),
            Cap::On => th.text,
        };
        caps(d, fonts, name, x + cw + 6.0 * k, y + 1.0 * k, Style::Small, color, Align::Left);
        x += w + gap;
    }
    Rectangle::new(cx - total * 0.5 - 8.0 * k, y - 8.0 * k, total + 16.0 * k, 36.0 * k)
}

/// Un aviso al centro: el texto con una raya corta a cada lado. Devuelve
/// lo que ocupa.
pub(super) fn notice(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, s: &str, cx: f32, y: f32, color: Color, alpha: f32) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let w = super::keys_width(fonts, s);
    let reach = 34.0 * k;
    let mid = y + 11.0 * k;
    let c = fade(color, alpha);
    line(d, Vector2::new(cx - w * 0.5 - 12.0 * k - reach, mid), Vector2::new(cx - w * 0.5 - 12.0 * k, mid), 1.0, c);
    line(d, Vector2::new(cx + w * 0.5 + 12.0 * k, mid), Vector2::new(cx + w * 0.5 + 12.0 * k + reach, mid), 1.0, c);
    super::keys(d, fonts, th, s, cx, y, Align::Center, color, alpha);
    Rectangle::new(cx - w * 0.5 - 12.0 * k - reach, y - 6.0 * k, w + 24.0 * k + 2.0 * reach, 34.0 * k)
}

/// Las marcas de los cuerpos y los lugares: un rombo hueco en el lugar, su
/// nombre y la distancia. Las mas cercanas primero; una que se encime con
/// lo que ya hay (el HUD en `avoid` u otra marca) no se dibuja.
pub(super) fn markers(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, app: &App, avoid: &[Rectangle]) {
    let fonts = ui.fonts;
    let k = ui.k();
    let mut order: Vec<&crate::app::Marker> = app.markers.iter().collect();
    order.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    let mut taken: Vec<Rectangle> = avoid.to_vec();
    for m in order {
        let name = pretty(m.name);
        let distance = if m.distance >= 1000.0 { format!("{:.1}k", m.distance / 1000.0) } else { format!("{:.0}", m.distance) };
        let (x, y) = ui.fit.point(m.x, m.y);
        let nw = fonts.width(name, Style::Bold);
        let dw = fonts.width(&distance, Style::Small);
        let rect = Rectangle::new(x - 8.0 * k, y - 12.0 * k, 16.0 * k + nw + 8.0 * k + dw + 4.0 * k, 24.0 * k);
        if taken.iter().any(|r| r.check_collision_recs(rect)) {
            continue;
        }
        taken.push(rect);
        let center = Vector2::new(x, y);
        d.draw_poly_lines_ex(center, 4, 6.0 * k, 0.0, 3.0, shadow(th.ink));
        d.draw_poly_lines_ex(center, 4, 6.0 * k, 0.0, 1.5, th.ink);
        let tx = x + 12.0 * k;
        text(d, fonts, name, tx, y - 11.0 * k, Style::Bold, th.ink, Align::Left);
        text(d, fonts, &distance, tx + nw + 8.0 * k, y - 8.0 * k, Style::Small, th.soft(), Align::Left);
    }
}

// La musica ------------------------------------------------------------------

/// Donde va cada cosa de la musica, abajo a la derecha.
pub(super) struct MusicLayout {
    pub rect: Rectangle,
    /// El centro y el lado del barrilete.
    icon: (f32, f32, f32),
    text: f32,
    /// Anterior, pausa y siguiente.
    pub buttons: [Rectangle; 3],
    pub bar: Rectangle,
}

pub(super) fn music_layout(screen: (i32, i32), k: f32) -> MusicLayout {
    let (w, h) = (344.0 * k, 70.0 * k);
    let (x, y) = ((screen.0 as f32 - w - 22.0 * k).round(), (screen.1 as f32 - h - 18.0 * k).round());
    let icon = (x + 22.0 * k, y + 24.0 * k, 40.0 * k);
    let text = x + 52.0 * k;
    let row = y + h - 12.0 * k;
    let button = |i: usize| Rectangle::new(x + w - (3 - i) as f32 * 24.0 * k, row - 10.0 * k, 20.0 * k, 20.0 * k);
    MusicLayout {
        rect: Rectangle::new(x, y, w, h),
        icon,
        text,
        buttons: [button(0), button(1), button(2)],
        bar: Rectangle::new(text, row - 1.0 * k, x + w - 82.0 * k - text, 2.0 * k),
    }
}

/// La musica: el barrilete que gira mientras suena, el titulo, el artista
/// con los tiempos, la linea de avance con su perilla, los botones y el
/// volumen en cinco rayitas. Devuelve lo que ocupa.
pub(super) fn music(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, view: &PlayerView, screen: (i32, i32)) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let layout = music_layout(screen, k);
    let r = layout.rect;
    wash(d, th, Rectangle::new(r.x - 60.0 * k, r.y - 30.0 * k, r.width + 82.0 * k, r.height + 48.0 * k), 2);
    // El barrilete: gira con la cancion.
    let (cx, cy, size) = layout.icon;
    let size = size.round();
    let at = Rectangle::new((cx - size * 0.5).round(), (cy - size * 0.5).round(), size, size);
    let spin = (view.time * 24.0).rem_euclid(360.0);
    if ui.art.put(d, ui.thread, Key::Kite(size as u32), |_, _| at, if spin == 0.0 { 0.001 } else { spin }, Color::WHITE, false).is_none() {
        d.draw_circle_v(Vector2::new(cx, cy), size * 0.5, th.ink);
    }
    let (fill, total) = if view.length > 0.0 { ((view.time / view.length).clamp(0.0, 1.0), super::clock(view.length)) } else { (0.5 + 0.5 * (view.time * 0.5).sin(), "en vivo".to_string()) };
    // El volumen: cinco rayitas arriba a la derecha.
    for i in 0..5 {
        let tall = (4.0 + 2.5 * i as f32) * k;
        let lit = view.volume > (i as f32 + 0.5) / 5.0 - 0.1;
        let x = r.x + r.width - 26.0 * k + i as f32 * 5.0 * k;
        line(d, Vector2::new(x, r.y + 16.0 * k), Vector2::new(x, r.y + 16.0 * k - tall), 2.0 * k, if lit { th.ink } else { th.faint() });
    }
    // El titulo, el artista y los tiempos.
    let x = layout.text;
    let room = r.x + r.width - 34.0 * k - x;
    let (name, artist) = song_parts(&view.title);
    text(d, fonts, &clipped(fonts, name, Style::Bold, room), x, r.y + 2.0 * k, Style::Bold, th.text, Align::Left);
    let times = format!("{} / {total}", super::clock(view.time));
    let state = if view.playing { "" } else { "  ·  en pausa" };
    let tw = fonts.width(&times, Style::Small);
    let who = artist.map_or(format!("{} de {}{state}", view.index + 1, view.count), |a| format!("{a}{state}"));
    text(d, fonts, &clipped(fonts, &who, Style::Small, r.x + r.width - x - tw - 14.0 * k), x, r.y + 25.0 * k, Style::Small, th.soft(), Align::Left);
    text(d, fonts, &times, r.x + r.width, r.y + 25.0 * k, Style::Small, th.soft(), Align::Right);
    // La linea de avance.
    let bar = layout.bar;
    let mid = bar.y + bar.height * 0.5;
    line(d, Vector2::new(bar.x, mid), Vector2::new(bar.x + bar.width, mid), bar.height, th.faint());
    line(d, Vector2::new(bar.x, mid), Vector2::new(bar.x + bar.width * fill, mid), bar.height, th.ink);
    d.draw_circle_v(Vector2::new(bar.x + bar.width * fill, mid), 4.5 * k + 1.0, shadow(th.text));
    d.draw_circle_v(Vector2::new(bar.x + bar.width * fill, mid), 4.0 * k, th.text);
    // Los botones: anterior, pausa o seguir, siguiente.
    let c = th.ink;
    let center = |b: &Rectangle| (b.x + b.width * 0.5, b.y + b.height * 0.5);
    let (px, py) = center(&layout.buttons[0]);
    line(d, Vector2::new(px - 5.0 * k, py - 5.0 * k), Vector2::new(px - 5.0 * k, py + 5.0 * k), 2.0 * k, c);
    triangle(d, Vector2::new(px + 5.0 * k, py - 5.0 * k), Vector2::new(px - 4.0 * k, py), Vector2::new(px + 5.0 * k, py + 5.0 * k), c);
    let (bx, by) = center(&layout.buttons[1]);
    if view.playing {
        line(d, Vector2::new(bx - 3.0 * k, by - 6.0 * k), Vector2::new(bx - 3.0 * k, by + 6.0 * k), 3.0 * k, c);
        line(d, Vector2::new(bx + 3.0 * k, by - 6.0 * k), Vector2::new(bx + 3.0 * k, by + 6.0 * k), 3.0 * k, c);
    } else {
        triangle(d, Vector2::new(bx - 4.0 * k, by - 6.5 * k), Vector2::new(bx - 4.0 * k, by + 6.5 * k), Vector2::new(bx + 6.0 * k, by), c);
    }
    let (nx, ny) = center(&layout.buttons[2]);
    triangle(d, Vector2::new(nx - 5.0 * k, ny - 5.0 * k), Vector2::new(nx + 4.0 * k, ny), Vector2::new(nx - 5.0 * k, ny + 5.0 * k), c);
    line(d, Vector2::new(nx + 5.0 * k, ny - 5.0 * k), Vector2::new(nx + 5.0 * k, ny + 5.0 * k), 2.0 * k, c);
    Rectangle::new(r.x - 8.0 * k, r.y - 8.0 * k, r.width + 16.0 * k, r.height + 16.0 * k)
}

// Los bloques de texto -------------------------------------------------------

/// Un bloque de lineas (estadisticas, ajustes, Faro, la ayuda): un velo
/// oscuro sin orilla con una raya del color a la izquierda.
pub(super) fn block(d: &mut RaylibDrawHandle, th: &Theme, r: Rectangle) {
    d.draw_rectangle_rec(r, th.veil(150));
    d.draw_rectangle_rec(Rectangle::new(r.x, r.y, 3.0, r.height), th.ink);
}

// El velocimetro de los minijuegos -------------------------------------------

/// Un velocimetro de linea: un arco abierto abajo con sus marcas, la parte
/// recorrida encendida y el numero adentro.
#[allow(clippy::too_many_arguments)]
pub(super) fn gauge(d: &mut RaylibDrawHandle, ui: &Ui, th: &Theme, cx: f32, cy: f32, size: f32, fraction: f32, hot: bool, number: &str, unit: &str) {
    let fonts = ui.fonts;
    let k = ui.k();
    let radius = size * 0.5;
    let center = Vector2::new(cx.round(), cy.round());
    let (from, sweep) = (135.0f32, 270.0f32);
    let fraction = fraction.clamp(0.0, 1.0);
    let color = if hot { Color::new(150, 225, 255, 255) } else { th.ink };
    d.draw_circle_v(center, radius * 0.86, Color::new(0, 0, 0, 90));
    d.draw_ring(center, radius * 0.86 - 2.0, radius * 0.86 + 1.0, from, from + sweep, 64, shadow(th.ink));
    d.draw_ring(center, radius * 0.86 - 1.0, radius * 0.86, from, from + sweep, 64, th.faint());
    d.draw_ring(center, radius * 0.78, radius * 0.86, from, from + sweep * fraction, 64, color);
    for i in 0..=10 {
        let a = (from + sweep * i as f32 / 10.0).to_radians();
        let (s, c) = a.sin_cos();
        let (r0, r1) = (radius * if i % 5 == 0 { 0.62 } else { 0.68 }, radius * 0.74);
        line(d, Vector2::new(cx + c * r0, cy + s * r0), Vector2::new(cx + c * r1, cy + s * r1), 1.0, fade(th.text, 0.6));
    }
    let font = radius * 0.5;
    let w = fonts.width_at(number, Style::Heading, font);
    fonts.draw_at(d, number, cx - w * 0.5 + k, cy - font * 0.55 + k, Style::Heading, font, Color::new(0, 0, 0, 160));
    fonts.draw_at(d, number, cx - w * 0.5, cy - font * 0.55, Style::Heading, font, if hot { color } else { th.text });
    caps(d, fonts, unit, cx, cy + radius * 0.34, Style::Small, th.soft(), Align::Center);
}

/// Un panel de los minijuegos: un velo oscuro con esquinas de corchete del
/// color (sin placas de tela).
pub(super) fn frame(d: &mut RaylibDrawHandle, th: &Theme, r: Rectangle, k: f32, strong: bool) {
    d.draw_rectangle_rec(r, Color::new(4, 8, 10, if strong { 200 } else { 150 }));
    let arm = (22.0 * k).min(r.width * 0.25).min(r.height * 0.4);
    let c = th.ink;
    for (x, y, dx, dy) in [(r.x, r.y, 1.0, 1.0), (r.x + r.width, r.y, -1.0, 1.0), (r.x, r.y + r.height, 1.0, -1.0), (r.x + r.width, r.y + r.height, -1.0, -1.0)] {
        line(d, Vector2::new(x, y), Vector2::new(x + dx * arm, y), 2.0, c);
        line(d, Vector2::new(x, y), Vector2::new(x, y + dy * arm), 2.0, c);
    }
    if strong {
        d.draw_rectangle_lines_ex(r, 1.0, th.faint());
    }
}
