//! HUD en 2D. No es parte del pipeline 3D: se dibuja encima de la imagen ya
//! renderizada.
//!
//! Lo que tiene dibujo (los titulos, los fondos de los paneles, la caratula
//! del velocimetro, el barrilete del reproductor) se **hornea en CPU**
//! (`rotulos.rs`, `piel.rs`) y raylib solo lo pega como textura; encima
//! raylib pone el texto, las teclas, los arcos y las agujas. Cada version
//! tiene su piel:
//!
//! - **OCASO**: el titulo forjado en hierro con grietas de lava que laten
//!   con la estrella, laminas oscuras con filo de bronce, letra romana de
//!   lapida para los rotulos y un velocimetro que es un ojo.
//! - **Chapina**: GUATEMALA cortada de un tejido bajo un lazo de papel
//!   picado, paneles de tela de corte con su faja, letra gorda de rotulo de
//!   camioneta, velocimetro con aguja y un reproductor cuyo disco es un
//!   barrilete que gira.
//!
//! Todo va a la escala `k` (alto de la imagen entre 720): en pantalla
//! completa el HUD crece con la imagen y las letras se cargan a ese tamano,
//! para que se vean nitidas. La letra del texto es Segoe UI (la de Windows)
//! con sus tildes; si falta alguna letra se usa la de raylib sin tildes.

pub mod letras;
pub mod piel;
pub mod rotulos;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use raylib::prelude::*;

use crate::app::{App, FaroReadout, FlarePhase};
use crate::minijuegos::{Align, Font, Item, Overlay, Paint};
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
    /// El planeta del lugar del aviso, si es un lugar: "San Andres Xecul"
    /// no dice que esta en los Cuchumatanes.
    pub banner_where: Option<&'static str>,
    /// Si el titulo del principio ya se quito (al cambiar de version se
    /// quita para que no se encime con el de la transicion).
    pub intro_over: bool,
    /// El minijuego que se puede abrir aqui (Enter), si hay.
    pub prompt: Option<&'static str>,
}

/// Como cabe la imagen del render en la ventana: a que escala y desde donde
/// (centrada y sin deformarla). En la ventana normal mide lo mismo que el
/// render (escala 1, en la esquina); en pantalla completa (F11) se estira
/// hasta llenar el monitor y el render sigue a su resolucion, asi que el
/// CPU hace el mismo trabajo: solo cambia el tamano con que se muestra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
}

impl Fit {
    pub fn new(render: (usize, usize), screen: (i32, i32)) -> Self {
        let (rw, rh) = (render.0 as f32, render.1 as f32);
        let (sw, sh) = (screen.0 as f32, screen.1 as f32);
        let scale = (sw / rw).min(sh / rh);
        Self {
            scale,
            x: ((sw - rw * scale) * 0.5).round(),
            y: ((sh - rh * scale) * 0.5).round(),
        }
    }

    /// Un punto del render en la pantalla.
    pub fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.y + y * self.scale)
    }
}

/// La escala del HUD: el alto con que se ve la imagen entre 720, en pasos
/// de 0.05 (cada escala carga sus letras).
pub fn ui_scale(render_height: usize, fit: Fit) -> f32 {
    ((render_height as f32 * fit.scale / 720.0) * 20.0).round().clamp(12.0, 80.0) / 20.0
}

/// La imagen del render en la pantalla: tal cual en la ventana o estirada
/// (con el filtro bilineal que le pone `main`) en pantalla completa.
pub fn frame(d: &mut RaylibDrawHandle, texture: &Texture2D, fit: Fit) {
    let (w, h) = (texture.width() as f32, texture.height() as f32);
    if fit.scale == 1.0 {
        d.draw_texture(texture, fit.x as i32, fit.y as i32, Color::WHITE);
    } else {
        let to = Rectangle::new(fit.x, fit.y, w * fit.scale, h * fit.scale);
        d.draw_texture_pro(texture, Rectangle::new(0.0, 0.0, w, h), to, Vector2::new(0.0, 0.0), 0.0, Color::WHITE);
    }
}

/// Segundos que dura el titulo al empezar.
const INTRO: f32 = 8.5;
/// Segundos que dura un aviso.
const TOAST: f32 = 3.0;

/// Lo que se carga de cada letra: ASCII y lo del espanol.
pub(crate) const GLYPHS: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~áéíóúÁÉÍÓÚñÑüÜäöÄÖ¿¡°·–—−…’×√πψτ⁰¹²³⁴⁵⁶⁷⁸⁹⁻»←↑→↓º";

/// Los estilos de letra.
#[derive(Clone, Copy)]
enum Style {
    Body,
    Bold,
    Small,
    Heading,
    /// La gorda de los rotulos, a cualquier tamano.
    Title,
    /// La romana de lapida: chica, mediana y a cualquier tamano.
    Caps,
    CapsBig,
    Roman,
    /// La gorda: chica y mediana.
    FatSmall,
    Fat,
    /// Las cifras de la gorda grande (ver `Fonts::figures`).
    Digits,
}

const TEXT_FONT: &[&str] = &["segoeui.ttf"];
const BOLD_FONT: &[&str] = &["segoeuib.ttf"];
/// La romana de OCASO (las dos primeras vienen con Office; las otras, con
/// Windows).
pub(crate) const ROMAN_FONT: &[&str] = &["PERTIBD.TTF", "FELIXTI.TTF", "palab.ttf", "timesbd.ttf"];
/// La gorda de la version chapina.
pub(crate) const FAT_FONT: &[&str] = &["COOPBL.TTF", "seguibl.ttf", "ariblk.ttf", "segoeuib.ttf"];
/// Las cifras de la gorda: la Cooper trae cifras de estilo antiguo (el 0 del
/// alto de una minuscula) y en un marcador se ven disparejas.
pub(crate) const DIGIT_FONT: &[&str] = &["ROCKEB.TTF", "seguibl.ttf", "ariblk.ttf", "segoeuib.ttf"];

/// Archivo, tamano en pixeles y si se carga a la escala del HUD (las que se
/// dibujan a cualquier tamano van en un atlas grande fijo).
const STYLES: [(&[&str], f32, bool); 11] = [
    (TEXT_FONT, 18.0, true),
    (BOLD_FONT, 18.0, true),
    (TEXT_FONT, 15.0, true),
    (BOLD_FONT, 24.0, true),
    (FAT_FONT, 120.0, false),
    (ROMAN_FONT, 15.0, true),
    (ROMAN_FONT, 24.0, true),
    (ROMAN_FONT, 96.0, false),
    (FAT_FONT, 15.0, true),
    (FAT_FONT, 22.0, true),
    (DIGIT_FONT, 120.0, false),
];

const LINE: f32 = 23.0;

const TEXT: Color = Color::new(242, 236, 226, 255);
const DIM: Color = Color::new(200, 190, 176, 210);
const FAINT: Color = Color::new(200, 190, 176, 160);
const PANEL: Color = Color::new(10, 10, 14, 190);
const EDGE: Color = Color::new(255, 255, 255, 30);
const WARN: Color = Color::new(255, 118, 92, 255);

/// Los colores y la letra de cada version.
#[derive(Clone, Copy)]
struct Skin {
    chapin: bool,
    accent: Color,
    /// El fondo liso (los paneles de texto) y su borde.
    flat: Color,
    edge: Color,
    plate: piel::Panel,
    /// La letra de las etiquetas y la de los encabezados, con su espacio
    /// entre letras.
    label: Style,
    heading: Style,
    tracking: f32,
}

impl Skin {
    fn of(chapin: bool) -> Self {
        if chapin {
            Self {
                chapin,
                accent: Color::new(250, 200, 70, 255),
                flat: Color::new(12, 15, 44, 225),
                edge: Color::new(250, 200, 70, 150),
                plate: piel::Panel::Placa,
                label: Style::FatSmall,
                heading: Style::Fat,
                tracking: 0.4,
            }
        } else {
            Self {
                chapin,
                accent: Color::new(255, 176, 104, 255),
                flat: Color::new(12, 9, 10, 215),
                edge: Color::new(168, 120, 74, 170),
                plate: piel::Panel::Lamina,
                label: Style::Caps,
                heading: Style::CapsBig,
                tracking: 1.6,
            }
        }
    }
}

fn fade(c: Color, alpha: f32) -> Color {
    Color::new(c.r, c.g, c.b, (c.a as f32 * alpha.clamp(0.0, 1.0)) as u8)
}

/// Las letras del HUD, una por estilo.
pub struct Fonts {
    fonts: Vec<WeakFont>,
    /// El tamano de cada atlas.
    sizes: Vec<f32>,
    scale: f32,
    /// Si la letra tiene tildes (la de raylib no).
    unicode: bool,
    /// Lo que le falta a cada letra de `GLYPHS` (la gorda no trae el punto
    /// medio ni las flechas): eso se dibuja con la negrita del texto.
    missing: Vec<String>,
    /// Para poner las cifras de `Style::Digits` en un texto de la gorda
    /// grande: cuanto mas grandes van (para que midan lo que sus
    /// mayusculas) y cuanto hay que bajarlas (para que se asienten en su
    /// renglon), las dos por tamano de letra.
    figures: Option<(f32, f32)>,
}

/// El alto de la H de `font` y donde se asienta, entre el tamano del atlas.
fn cap_metrics(font: &WeakFont) -> Option<(f32, f32)> {
    let raw: &raylib::ffi::Font = font.as_ref();
    if raw.glyphs.is_null() || raw.recs.is_null() {
        return None;
    }
    // SAFETY: raylib guarda `glyphCount` glifos y sus rectangulos mientras
    // la letra este cargada.
    let (glyphs, recs) = unsafe { (std::slice::from_raw_parts(raw.glyphs, raw.glyphCount.max(0) as usize), std::slice::from_raw_parts(raw.recs, raw.glyphCount.max(0) as usize)) };
    let i = glyphs.iter().position(|g| g.value == 'H' as i32)?;
    let size = raw.baseSize as f32;
    Some((recs[i].height / size, (glyphs[i].offsetY as f32 + recs[i].height) / size))
}

/// Las letras de `GLYPHS` que `font` no trae.
fn missing_glyphs(font: &WeakFont) -> String {
    let raw: &raylib::ffi::Font = font.as_ref();
    if raw.glyphs.is_null() {
        return String::new();
    }
    // SAFETY: raylib guarda `glyphCount` glifos en `glyphs` mientras la
    // letra este cargada.
    let glyphs = unsafe { std::slice::from_raw_parts(raw.glyphs, raw.glyphCount.max(0) as usize) };
    GLYPHS.chars().filter(|&c| !glyphs.iter().any(|g| g.value == c as i32)).collect()
}

impl Fonts {
    /// Las letras a `scale` veces su tamano (la escala del HUD).
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, scale: f32) -> Self {
        let sizes: Vec<f32> = STYLES.iter().map(|&(_, size, scaled)| if scaled { (size * scale).round() } else { size }).collect();
        let mut fonts = Vec::new();
        for (&(files, _, scaled), &size) in STYLES.iter().zip(&sizes) {
            match load_font(rl, thread, files, size, scaled) {
                Some(font) => fonts.push(font),
                None => {
                    return Self {
                        fonts: STYLES.iter().map(|_| rl.get_font_default()).collect(),
                        sizes,
                        scale,
                        unicode: false,
                        missing: vec![String::new(); STYLES.len()],
                        figures: None,
                    };
                }
            }
        }
        let missing = fonts.iter().map(missing_glyphs).collect();
        let figures = match (cap_metrics(&fonts[Style::Title as usize]), cap_metrics(&fonts[Style::Digits as usize])) {
            (Some((cap, base)), Some((their_cap, their_base))) if their_cap > 0.0 => {
                let grow = cap / their_cap;
                Some((grow, base - their_base * grow))
            }
            _ => None,
        };
        Self {
            fonts,
            sizes,
            scale,
            unicode: true,
            missing,
            figures,
        }
    }

    /// `text` en pedazos, cada uno con la letra que lo dibuja: la de
    /// `style`, la negrita del texto para lo que a esa le falta y, en la
    /// gorda grande, las cifras de `Style::Digits`.
    fn runs<'t>(&self, text: &'t str, style: Style) -> Vec<(Style, &'t str)> {
        let missing = &self.missing[style as usize];
        let figures = matches!(style, Style::Title) && self.figures.is_some();
        let pick = |c: char| {
            if figures && c.is_ascii_digit() {
                Style::Digits
            } else if missing.contains(c) {
                Style::Bold
            } else {
                style
            }
        };
        let mut runs = Vec::new();
        let mut start = 0;
        let mut current = style;
        for (i, c) in text.char_indices() {
            let with = pick(c);
            if with as usize != current as usize {
                if i > start {
                    runs.push((current, &text[start..i]));
                }
                start = i;
                current = with;
            }
        }
        runs.push((current, &text[start..]));
        runs
    }

    /// El tamano y lo que baja un pedazo dibujado con `with` en un texto de
    /// tamano `size`.
    fn run_place(&self, with: Style, size: f32) -> (f32, f32) {
        match (with, self.figures) {
            (Style::Digits, Some((grow, drop))) => (size * grow, size * drop),
            _ => (size, 0.0),
        }
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    fn font(&self, style: Style) -> (&WeakFont, f32) {
        (&self.fonts[style as usize], self.sizes[style as usize])
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

    fn width_spaced(&self, text: &str, style: Style, size: f32, spacing: f32) -> f32 {
        let text = self.clean(text);
        self.runs(&text, style)
            .into_iter()
            .map(|(with, run)| {
                let (font, _) = self.font(with);
                font.measure_text(run, self.run_place(with, size).0, spacing).x + spacing
            })
            .sum::<f32>()
            - spacing
    }

    fn width_at(&self, text: &str, style: Style, size: f32) -> f32 {
        self.width_spaced(text, style, size, 0.0)
    }

    fn width(&self, text: &str, style: Style) -> f32 {
        self.width_at(text, style, self.font(style).1)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_spaced(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, style: Style, size: f32, spacing: f32, color: Color) {
        let text = self.clean(text);
        // En pixeles enteros, para que la letra no se emborrone.
        let mut at = Vector2::new(x.round(), y.round());
        for (with, run) in self.runs(&text, style) {
            let (font, _) = self.font(with);
            let (size, drop) = self.run_place(with, size);
            d.draw_text_ex(font, run, Vector2::new(at.x, (at.y + drop).round()), size, spacing, color);
            at.x += (font.measure_text(run, size, spacing).x + spacing).round();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_at(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, style: Style, size: f32, color: Color) {
        self.draw_spaced(d, text, x, y, style, size, 0.0, color);
    }

    fn draw(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, style: Style, color: Color) {
        self.draw_at(d, text, x, y, style, self.font(style).1, color);
    }

    /// Una etiqueta o un encabezado de la piel: en OCASO, en mayusculas y
    /// con aire entre las letras.
    fn label_width(&self, text: &str, style: Style, skin: &Skin) -> f32 {
        let text = if skin.chapin { Cow::from(text) } else { Cow::from(text.to_uppercase()) };
        self.width_spaced(&text, style, self.font(style).1, skin.tracking * self.scale)
    }

    fn label(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, style: Style, skin: &Skin, color: Color) {
        let text = if skin.chapin { Cow::from(text) } else { Cow::from(text.to_uppercase()) };
        self.draw_spaced(d, &text, x, y, style, self.font(style).1, skin.tracking * self.scale, color);
    }
}

fn load_font(rl: &mut RaylibHandle, thread: &RaylibThread, files: &[&str], size: f32, scaled: bool) -> Option<WeakFont> {
    let data = files.iter().find_map(|file| std::fs::read(Path::new(letras::FONT_DIR).join(file)).ok())?;
    let font = rl
        .load_font_from_memory(thread, ".ttf", &data, size as i32, Some(GLYPHS))
        .ok()?;
    if scaled {
        font.texture()
            .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
    } else {
        // Las que se dibujan a cualquier tamano van con sus mipmaps: asi
        // achicadas no se ven dentadas.
        let mut texture: raylib::ffi::Texture2D = *font.texture().as_ref();
        // SAFETY: la textura del atlas esta cargada; los mipmaps se generan
        // en la misma textura (su identificador no cambia).
        unsafe {
            raylib::ffi::GenTextureMipmaps(&mut texture);
            raylib::ffi::SetTextureFilter(texture, TextureFilter::TEXTURE_FILTER_TRILINEAR as i32);
        }
    }
    Some(font.make_weak())
}

// Lo horneado ----------------------------------------------------------------

/// Algo horneado en CPU que raylib pega como textura.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    /// Un fondo de panel: clase, ancho, alto y escala en centesimas.
    Panel(piel::Panel, u32, u32, u32),
    Dial(bool, u32),
    Kite(u32),
    Spark,
    /// El titulo de una version a este ancho: su cuerpo o su luz.
    Title { chapin: bool, width: u32, light: bool },
    /// Un rotulo pintado, con la letra a este tamano.
    Sign { text: String, paint: Paint, size: u32 },
}

/// Lo horneado, guardado por lo que es: se hornea la primera vez que se
/// pide (unos milisegundos) y despues solo se pega.
pub struct Art {
    cache: RefCell<HashMap<Key, Texture2D>>,
    roman: Option<Vec<u8>>,
    fat: Option<Vec<u8>>,
    /// La letra de las cifras de los rotulos.
    digits: Option<Vec<u8>>,
}

/// Cuanto margen llevan los titulos y los rotulos, en tamanos de letra.
const TITLE_MARGIN: f32 = 0.3;
const SIGN_MARGIN: f32 = 0.12;

impl Art {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            roman: letras::font_bytes(ROMAN_FONT),
            fat: letras::font_bytes(FAT_FONT),
            digits: letras::font_bytes(DIGIT_FONT),
        }
    }

    /// Antes de dibujar un frame: si se junto demasiado (rotulos de avisos
    /// que ya no salen, tamanos de otra escala), se suelta todo.
    pub fn trim(&self) {
        let mut cache = self.cache.borrow_mut();
        if cache.len() > 200 {
            cache.clear();
        }
    }

    /// Hornea `key`: cada textura que sale, con su tamano.
    fn bake(&self, key: &Key) -> Vec<(Key, usize, usize, Vec<u8>)> {
        match key {
            Key::Panel(kind, w, h, scale) => vec![(key.clone(), *w as usize, *h as usize, piel::panel(*kind, *w as usize, *h as usize, *scale as f32 / 100.0))],
            Key::Dial(chapin, size) => vec![(key.clone(), *size as usize, *size as usize, piel::dial(*chapin, *size as usize))],
            Key::Kite(size) => vec![(key.clone(), *size as usize, *size as usize, piel::kite(*size as usize))],
            Key::Spark => vec![(key.clone(), 32, 32, piel::spark(32))],
            Key::Title { chapin, width, .. } => {
                let (font, text, tracking, margin) = if *chapin { (&self.fat, "GUATEMALA", 0.02, 0.12) } else { (&self.roman, "OCASO", 0.09, TITLE_MARGIN) };
                let Some(font) = font else { return Vec::new() };
                // Una de prueba para saber que tamano de letra da ese ancho.
                let Some(probe) = letras::Word::new(font, text, 60.0, 60.0 * tracking, 60.0 * margin) else { return Vec::new() };
                let size = 60.0 * *width as f32 / probe.w as f32;
                let Some(word) = letras::Word::new(font, text, size, size * tracking, size * margin) else { return Vec::new() };
                let layers = if *chapin { rotulos::garlanded(&word) } else { rotulos::forged(&word, 2.5, 7) };
                let mut out = vec![(Key::Title { chapin: *chapin, width: *width, light: false }, layers.w, layers.h, layers.body)];
                if !layers.light.is_empty() {
                    out.push((Key::Title { chapin: *chapin, width: *width, light: true }, layers.w, layers.h, layers.light));
                }
                out
            }
            Key::Sign { text, paint, size } => {
                let Some(font) = &self.fat else { return Vec::new() };
                let size = *size as f32;
                let Some(word) = letras::Word::with_digits(font, self.digits.as_deref(), text, size, 0.0, size * SIGN_MARGIN) else { return Vec::new() };
                let paint = match paint {
                    Paint::Sun => rotulos::Paint::SUN,
                    Paint::Cream => rotulos::Paint::CREAM,
                    Paint::Jade => rotulos::Paint::JADE,
                    Paint::Rose => rotulos::Paint::ROSE,
                };
                let layers = rotulos::painted(&word, paint);
                vec![(key.clone(), layers.w, layers.h, layers.body)]
            }
        }
    }

    /// Hornea `key` y lo sube como textura, si no esta ya.
    fn ensure(&self, rl: &mut RaylibHandle, thread: &RaylibThread, key: &Key) {
        if self.cache.borrow().contains_key(key) {
            return;
        }
        for (name, w, h, rgba) in self.bake(key) {
            let image = Image::gen_image_color(w as i32, h as i32, Color::BLANK);
            let Ok(mut texture) = rl.load_texture_from_image(thread, &image) else { continue };
            if texture.update_texture(&rgba).is_err() {
                continue;
            }
            texture.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            self.cache.borrow_mut().insert(name, texture);
        }
    }

    /// Pega `key` donde diga `place` (que recibe el tamano de la textura),
    /// girada `rotation` grados alrededor de su centro; `additive` la suma
    /// en vez de taparla. Devuelve donde quedo, o `None` si no se pudo
    /// hornear (falta la letra).
    #[allow(clippy::too_many_arguments)]
    fn put(&self, d: &mut RaylibDrawHandle, thread: &RaylibThread, key: Key, place: impl FnOnce(f32, f32) -> Rectangle, rotation: f32, tint: Color, additive: bool) -> Option<Rectangle> {
        self.ensure(d, thread, &key);
        let cache = self.cache.borrow();
        let texture = cache.get(&key)?;
        let (w, h) = (texture.width() as f32, texture.height() as f32);
        let to = place(w, h);
        let source = Rectangle::new(0.0, 0.0, w, h);
        let (dest, origin) = if rotation == 0.0 {
            (to, Vector2::new(0.0, 0.0))
        } else {
            (Rectangle::new(to.x + to.width * 0.5, to.y + to.height * 0.5, to.width, to.height), Vector2::new(to.width * 0.5, to.height * 0.5))
        };
        if additive {
            let mut lit = d.begin_blend_mode(BlendMode::BLEND_ADDITIVE);
            lit.draw_texture_pro(texture, source, dest, origin, rotation, tint);
        } else {
            d.draw_texture_pro(texture, source, dest, origin, rotation, tint);
        }
        Some(to)
    }
}

impl Default for Art {
    fn default() -> Self {
        Self::new()
    }
}

/// Con que se dibuja el HUD en este frame.
pub struct Ui<'a> {
    pub fonts: &'a Fonts,
    pub art: &'a Art,
    pub thread: &'a RaylibThread,
    pub fit: Fit,
}

impl Ui<'_> {
    /// La escala del HUD (la de sus letras).
    fn k(&self) -> f32 {
        self.fonts.scale()
    }

    /// El fondo de un panel con dibujo.
    fn panel(&self, d: &mut RaylibDrawHandle, kind: piel::Panel, rect: Rectangle) {
        let (w, h) = (rect.width.round().max(8.0), rect.height.round().max(8.0));
        let key = Key::Panel(kind, w as u32, h as u32, (self.k() * 100.0).round() as u32);
        let at = Rectangle::new(rect.x.round(), rect.y.round(), w, h);
        if self.art.put(d, self.thread, key, |_, _| at, 0.0, Color::WHITE, false).is_none() {
            flat_panel(d, at, PANEL, EDGE, 12.0);
        }
    }

    /// Un rotulo pintado con la letra a `size` pixeles, con su renglon
    /// empezando en `y`. Se hornea a un tamano redondeado hacia arriba y se
    /// pega al que toca. Devuelve su ancho.
    #[allow(clippy::too_many_arguments)]
    fn sign(&self, d: &mut RaylibDrawHandle, text: &str, x: f32, y: f32, size: f32, paint: Paint, align: Align, alpha: f32) -> f32 {
        let baked = ((size / 16.0).ceil() * 16.0).clamp(16.0, 320.0);
        let scale = size / baked;
        let key = Key::Sign { text: text.to_string(), paint, size: baked as u32 };
        let margin = baked * SIGN_MARGIN * scale;
        let place = |w: f32, h: f32| {
            let (w, h) = (w * scale, h * scale);
            let left = match align {
                Align::Left => x - margin,
                Align::Center => x - w * 0.5,
                Align::Right => x - w + margin,
            };
            Rectangle::new(left.round(), (y + 0.2 * size - margin).round(), w, h)
        };
        match self.art.put(d, self.thread, key, place, 0.0, fade(Color::WHITE, alpha), false) {
            Some(rect) => rect.width - 2.0 * margin,
            None => {
                // Sin la letra gorda: el texto del HUD.
                let w = self.fonts.width_at(text, Style::Title, size);
                let left = match align {
                    Align::Left => x,
                    Align::Center => x - w * 0.5,
                    Align::Right => x - w,
                };
                self.fonts.draw_at(d, text, left, y, Style::Title, size, fade(Color::new(255, 204, 120, 255), alpha));
                w
            }
        }
    }
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
        "El Periferico" => "El Periférico",
        other => other,
    }
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

/// Un panel liso con las esquinas de `radius` pixeles y su borde.
fn flat_panel(d: &mut RaylibDrawHandle, rect: Rectangle, fill: Color, edge: Color, radius: f32) {
    let roundness = (radius / rect.height.min(rect.width) * 2.0).min(1.0);
    d.draw_rectangle_rounded(rect, roundness, 10, fill);
    d.draw_rectangle_rounded_lines_ex(rect, roundness, 10, 1.0, edge);
}

/// Una pildora (los avisos, los nombres de los planetas).
fn pill(d: &mut RaylibDrawHandle, rect: Rectangle, fill: Color, edge: Option<Color>) {
    d.draw_rectangle_rounded(rect, 1.0, 12, fill);
    if let Some(edge) = edge {
        d.draw_rectangle_rounded_lines_ex(rect, 1.0, 12, 1.0, edge);
    }
}

// Las teclas -----------------------------------------------------------------

/// Como se ve una tecla.
#[derive(Clone, Copy, PartialEq)]
enum Cap {
    /// Una tecla suelta (las ayudas).
    Plain,
    /// Un interruptor encendido, apagado o avisando.
    On,
    Off,
    Warn,
}

/// El ancho de la tecla `text`.
fn cap_width(fonts: &Fonts, text: &str) -> f32 {
    let k = fonts.scale();
    (fonts.width(text, Style::Small) + 12.0 * k).max(22.0 * k)
}

/// Una tecla de 22 pixeles de alto con su letra. Devuelve su ancho.
fn cap(d: &mut RaylibDrawHandle, fonts: &Fonts, skin: &Skin, text: &str, x: f32, y: f32, look: Cap, alpha: f32) -> f32 {
    let k = fonts.scale();
    let w = cap_width(fonts, text);
    let rect = Rectangle::new(x.round(), y.round(), w.round(), (22.0 * k).round());
    let round = 0.34;
    let (fill, edge, ink) = match (look, skin.chapin) {
        (Cap::On, _) => (skin.accent, None, Color::new(34, 18, 14, 255)),
        (Cap::Warn, _) => (WARN, None, Color::new(34, 10, 8, 255)),
        (Cap::Off, _) => (Color::new(255, 255, 255, 16), Some(Color::new(255, 255, 255, 46)), FAINT),
        (Cap::Plain, true) => (Color::new(238, 230, 212, 255), None, Color::new(44, 30, 50, 255)),
        (Cap::Plain, false) => (Color::new(255, 255, 255, 20), Some(Color::new(190, 138, 90, 210)), TEXT),
    };
    if look == Cap::Plain && skin.chapin {
        // El canto de la tecla, abajo.
        let lip = Rectangle::new(rect.x, rect.y + 2.0 * k, rect.width, rect.height);
        d.draw_rectangle_rounded(lip, round, 6, fade(Color::new(150, 126, 110, 255), alpha));
    }
    d.draw_rectangle_rounded(rect, round, 6, fade(fill, alpha));
    if let Some(edge) = edge {
        d.draw_rectangle_rounded_lines_ex(rect, round, 6, 1.0, fade(edge, alpha));
    }
    let tw = fonts.width(text, Style::Small);
    fonts.draw(d, text, rect.x + (rect.width - tw) * 0.5, rect.y + 2.0 * k, Style::Small, fade(ink, alpha));
    w
}

/// Parte un renglon de teclas: lo que va entre corchetes es una tecla.
fn key_parts(text: &str) -> Vec<(bool, &str)> {
    let mut parts = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match (rest.find('['), rest.find(']')) {
            (Some(0), Some(end)) => {
                parts.push((true, &rest[1..end]));
                rest = &rest[end + 1..];
            }
            (Some(open), Some(end)) if open < end => {
                parts.push((false, &rest[..open]));
                rest = &rest[open..];
            }
            _ => {
                parts.push((false, rest));
                rest = "";
            }
        }
    }
    parts
}

fn keys_width(fonts: &Fonts, text: &str) -> f32 {
    let k = fonts.scale();
    key_parts(text).iter().map(|&(key, part)| if key { cap_width(fonts, part) + 4.0 * k } else { fonts.width(part, Style::Small) }).sum()
}

/// Un renglon de teclas y texto ("[N] siguiente lugar   [H] volver").
#[allow(clippy::too_many_arguments)]
fn keys(d: &mut RaylibDrawHandle, fonts: &Fonts, skin: &Skin, text: &str, x: f32, y: f32, align: Align, color: Color, alpha: f32) -> f32 {
    let k = fonts.scale();
    let total = keys_width(fonts, text);
    let mut at = match align {
        Align::Left => x,
        Align::Center => x - total * 0.5,
        Align::Right => x - total,
    };
    for (key, part) in key_parts(text) {
        if key {
            at += cap(d, fonts, skin, part, at, y, Cap::Plain, alpha) + 4.0 * k;
        } else {
            fonts.draw(d, part, at, y + 2.0 * k, Style::Small, fade(color, alpha));
            at += fonts.width(part, Style::Small);
        }
    }
    total
}

/// Un bloque de lineas sueltas (ayuda, estadisticas, ajustes) en un panel
/// liso. Devuelve el rectangulo que ocupa.
fn lines_panel(d: &mut RaylibDrawHandle, fonts: &Fonts, skin: &Skin, x: f32, y: f32, lines: &[String]) -> Rectangle {
    let k = fonts.scale();
    let pad = 14.0 * k;
    let (w, h) = lines_size(fonts, lines);
    let rect = Rectangle::new(x.round(), y.round(), w, h);
    flat_panel(d, rect, skin.flat, skin.edge, 12.0 * k);
    for (i, line) in lines.iter().enumerate() {
        fonts.draw(d, line, rect.x + pad, rect.y + pad - 3.0 * k + i as f32 * LINE * k, Style::Body, TEXT);
    }
    rect
}

fn lines_size(fonts: &Fonts, lines: &[String]) -> (f32, f32) {
    let k = fonts.scale();
    let pad = 14.0 * k;
    let w = lines.iter().map(|l| fonts.width(l, Style::Body)).fold(0.0, f32::max) + pad * 2.0;
    (w.round(), (lines.len() as f32 * LINE * k + pad * 2.0 - 6.0 * k).round())
}

/// El ancho de un aviso.
fn toast_width(fonts: &Fonts, text: &str) -> f32 {
    keys_width(fonts, text) + 36.0 * fonts.scale()
}

/// Un aviso en una pildora, que aparece y se desvanece. `center` y `bottom`
/// dicen donde y `slot` lo sube si hay otro debajo.
#[allow(clippy::too_many_arguments)]
fn toast(d: &mut RaylibDrawHandle, fonts: &Fonts, skin: &Skin, text: &str, alpha: f32, center: f32, bottom: f32, slot: f32) {
    if alpha <= 0.0 {
        return;
    }
    let k = fonts.scale();
    let w = toast_width(fonts, text);
    let h = 36.0 * k;
    let rect = Rectangle::new((center - w * 0.5).round(), (bottom - h - slot * (h + 8.0 * k)).round(), w.round(), h.round());
    pill(d, rect, fade(skin.flat, alpha), Some(fade(skin.accent, alpha * 0.6)));
    keys(d, fonts, skin, text, rect.x + 18.0 * k, rect.y + 7.0 * k, Align::Left, skin.accent, alpha);
}

// El velocimetro -------------------------------------------------------------

/// El velocimetro de `size` pixeles centrado en `(cx, cy)`: la caratula
/// horneada y encima el arco (OCASO) o la aguja (chapina), el numero y la
/// unidad. `fraction` es cuanto de la escala va (0 a 1) y `hot` lo
/// enciende (turbo, o mas rapido que el maximo).
#[allow(clippy::too_many_arguments)]
fn gauge(d: &mut RaylibDrawHandle, ui: &Ui, skin: &Skin, cx: f32, cy: f32, size: f32, fraction: f32, hot: bool, number: &str, unit: &str) {
    let fonts = ui.fonts;
    let size = size.round();
    let radius = size * 0.5;
    let center = Vector2::new(cx.round(), cy.round());
    let at = Rectangle::new(center.x - radius, center.y - radius, size, size);
    if ui.art.put(d, ui.thread, Key::Dial(skin.chapin, size as u32), |_, _| at, 0.0, Color::WHITE, false).is_none() {
        d.draw_circle_v(center, radius, PANEL);
    }
    let fraction = fraction.clamp(0.0, 1.0);
    let angle = piel::DIAL_FROM + piel::DIAL_SWEEP * fraction;
    let (sin, cos) = angle.to_radians().sin_cos();
    if skin.chapin {
        // La aguja, con su cola corta y el eje dorado.
        let needle = if hot { Color::new(255, 110, 150, 255) } else { Color::new(236, 52, 70, 255) };
        let tip = Vector2::new(center.x + cos * radius * 0.74, center.y + sin * radius * 0.74);
        let tail = Vector2::new(center.x - cos * radius * 0.16, center.y - sin * radius * 0.16);
        d.draw_line_ex(tail, tip, (radius * 0.075).max(2.0), Color::new(20, 8, 20, 200));
        d.draw_line_ex(tail, tip, (radius * 0.045).max(1.5), needle);
        d.draw_circle_v(center, radius * 0.1, Color::new(250, 200, 70, 255));
        d.draw_circle_v(center, radius * 0.045, Color::new(40, 20, 40, 255));
        let font = radius * 0.52;
        let w = fonts.width_at(number, Style::Title, font);
        let y = center.y + radius * 0.2;
        fonts.draw_at(d, number, center.x - w * 0.5 + 1.0, y + font * 0.06, Style::Title, font, Color::new(8, 6, 24, 220));
        fonts.draw_at(d, number, center.x - w * 0.5, y, Style::Title, font, if hot { Color::new(255, 150, 180, 255) } else { TEXT });
        let uw = fonts.width(unit, Style::FatSmall);
        fonts.draw(d, unit, center.x - uw * 0.5, center.y + radius * 0.64, Style::FatSmall, skin.accent);
    } else {
        // El arco de brasa, mas claro hacia la punta, y una chispa al final.
        let (inner, outer) = (radius * 0.6, radius * 0.66);
        d.draw_ring(center, inner, outer, piel::DIAL_FROM, piel::DIAL_FROM + piel::DIAL_SWEEP, 48, Color::new(255, 255, 255, 14));
        let pieces = 24;
        for i in 0..pieces {
            let (a, b) = (i as f32 / pieces as f32, (i + 1) as f32 / pieces as f32);
            if a >= fraction {
                break;
            }
            let b = b.min(fraction);
            let t = a / fraction.max(1e-3);
            let color = if hot { Color::new(255, (190.0 + 60.0 * t) as u8, (130.0 + 110.0 * t) as u8, 255) } else { Color::new(255, (96.0 + 110.0 * t) as u8, (30.0 + 90.0 * t) as u8, 255) };
            d.draw_ring(center, inner, outer, piel::DIAL_FROM + piel::DIAL_SWEEP * a, piel::DIAL_FROM + piel::DIAL_SWEEP * b, 4, color);
        }
        let glow = radius * 0.34;
        let tip = (center.x + cos * (inner + outer) * 0.5, center.y + sin * (inner + outer) * 0.5);
        let spot = Rectangle::new(tip.0 - glow * 0.5, tip.1 - glow * 0.5, glow, glow);
        ui.art.put(d, ui.thread, Key::Spark, |_, _| spot, 0.0, Color::new(255, 190, 120, 230), true);
        let font = radius * 0.4;
        let w = fonts.width_at(number, Style::Roman, font);
        fonts.draw_at(d, number, center.x - w * 0.5, center.y - font * 0.62, Style::Roman, font, if hot { Color::new(255, 232, 190, 255) } else { TEXT });
        let uw = fonts.label_width(unit, Style::Caps, skin);
        fonts.label(d, unit, center.x - uw * 0.5, center.y + radius * 0.14, Style::Caps, skin, skin.accent);
    }
}

/// Un numero con sus miles separados: 12 480.
fn grouped(value: f64) -> String {
    let digits = format!("{:.0}", value.abs());
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    if value < -0.5 { format!("−{out}") } else { out }
}

// El vuelo -------------------------------------------------------------------

/// El tablero de vuelo, abajo a la izquierda: el velocimetro y, a su lado,
/// la altitud, donde se esta y los interruptores (cada uno con su tecla,
/// encendida si esta puesto). Devuelve el rectangulo que ocupa.
fn dashboard(d: &mut RaylibDrawHandle, ui: &Ui, skin: &Skin, app: &App, height: f32) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let f = &app.flight;
    let body = |i| pretty(app.body_name(i));
    let dial = 168.0 * k;
    let margin = 12.0 * k;
    let (pw, ph) = (384.0 * k, 134.0 * k);
    let plate = Rectangle::new((margin + dial * 0.5).round(), (height - margin - ph).round(), pw, ph);
    ui.panel(d, skin.plate, plate);
    let center = (margin + dial * 0.5, height - margin - dial * 0.5);
    let fraction = if f.max_speed > 0.0 { f.speed / f.max_speed } else { 0.0 };
    gauge(d, ui, skin, center.0, center.1, dial, fraction as f32, fraction > 1.02, &format!("{:.0}", f.speed), "u/s");

    let x = margin + dial + 14.0 * k;
    let top = plate.y + if skin.chapin { 18.0 } else { 12.0 } * k;
    // La altitud y sobre que.
    fonts.label(d, "Altitud", x, top, skin.label, skin, fade(skin.accent, 0.85));
    let limit = format!("máx {:.0} u/s", f.max_speed);
    let lw = fonts.width(&limit, Style::Small);
    fonts.draw(d, &limit, plate.x + plate.width - 14.0 * k - lw, top, Style::Small, FAINT);
    let altitude = grouped(f.altitude);
    fonts.draw(d, &altitude, x, top + 15.0 * k, Style::Heading, TEXT);
    let aw = fonts.width(&altitude, Style::Heading);
    fonts.draw(d, &format!("sobre {}", body(f.nearest)), x + aw + 8.0 * k, top + 20.0 * k, Style::Body, DIM);
    // Donde se esta: el modo local o el marco, y el contacto.
    let place = match (app.settings.local, f.local, f.hold) {
        (true, true, Some(h)) => format!("Local: {}, a {:.0}", body(f.frame), h),
        (true, true, None) => format!("Local: {}", body(f.frame)),
        _ => format!("Marco: {}", body(f.frame)),
    };
    let place = if f.contact { format!("{place}  ·  contacto") } else { place };
    fonts.draw(d, &place, x, top + 44.0 * k, Style::Small, DIM);

    // Los interruptores.
    let path = match (app.path_mode, app.path_shown && app.path_impact) {
        (_, true) => ("choque", Cap::Warn),
        (None, _) => ("ruta auto", Cap::Off),
        (Some(true), _) => ("ruta", Cap::On),
        (Some(false), _) => ("sin ruta", Cap::Off),
    };
    let switch = |on: bool| if on { Cap::On } else { Cap::Off };
    let local = match (app.settings.local, f.local) {
        (false, _) => Cap::Off,
        (true, true) => Cap::On,
        (true, false) => Cap::Plain,
    };
    let switches = [("Z", "asist.", switch(app.settings.assist)), ("G", "graved.", switch(app.settings.gravity)), ("L", "local", local), ("O", "crucero", switch(app.settings.cruise)), ("Y", path.0, path.1)];
    let room = plate.x + plate.width - 12.0 * k - x;
    let step = room / switches.len() as f32;
    let row = top + 68.0 * k;
    for (i, (key, name, look)) in switches.into_iter().enumerate() {
        let cx = x + step * (i as f32 + 0.5);
        let w = cap_width(fonts, key).max(26.0 * k);
        // La tecla un poco mas ancha, centrada en su columna.
        let rect = Rectangle::new((cx - w * 0.5).round(), row.round(), w.round(), (22.0 * k).round());
        let (fill, edge, ink) = match look {
            Cap::On => (skin.accent, None, Color::new(34, 18, 14, 255)),
            Cap::Warn => (WARN, None, Color::new(34, 10, 8, 255)),
            Cap::Plain => (Color::new(255, 255, 255, 26), Some(fade(skin.accent, 0.8)), TEXT),
            Cap::Off => (Color::new(255, 255, 255, 14), Some(Color::new(255, 255, 255, 44)), FAINT),
        };
        d.draw_rectangle_rounded(rect, 0.34, 6, fill);
        if let Some(edge) = edge {
            d.draw_rectangle_rounded_lines_ex(rect, 0.34, 6, 1.0, edge);
        }
        let tw = fonts.width(key, Style::Bold);
        fonts.draw(d, key, rect.x + (rect.width - tw) * 0.5, rect.y, Style::Bold, ink);
        let nw = fonts.width(name, Style::Small);
        let color = match look {
            Cap::Warn => WARN,
            Cap::Off => FAINT,
            _ => TEXT,
        };
        fonts.draw(d, name, cx - nw * 0.5, row + 23.0 * k, Style::Small, color);
    }
    Rectangle::new(margin, center.1 - dial * 0.5, plate.x + plate.width - margin, dial)
}

/// La placa de arriba a la derecha: la fase del final (o la version) con su
/// tecla y, abajo, las teclas de moverse entre lugares. Devuelve el
/// rectangulo que ocupa.
fn status(d: &mut RaylibDrawHandle, ui: &Ui, skin: &Skin, app: &App, width: f32) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let c = &app.cycle;
    let clock = |seconds: f64| format!("{:02}:{:02}", (seconds / 60.0) as u32, (seconds % 60.0) as u32);
    let (title, hint) = match (app.ship_name(), c.phase) {
        (Some(_), _) => ("Guatemala".to_string(), "[V] OCASO"),
        (None, Phase::Agony) => (Phase::Agony.name().to_string(), "[J] desatar el final"),
        (None, phase) => (format!("{}  {}", phase.name(), clock(c.tau)), "[J] renacer"),
    };
    let second = match app.ship_name() {
        Some(name) => format!("[B] {}   [N] lugar   [H] volver   [F1] ayuda", pretty(name)),
        None => "[V] chapina   [N] lugar   [H] volver   [F1] ayuda".to_string(),
    };
    let pad = 16.0 * k;
    let w = (fonts.label_width(&title, skin.heading, skin) + 22.0 * k + keys_width(fonts, hint)).max(keys_width(fonts, &second)).max(330.0 * k) + pad * 2.0;
    // En pasos: el reloj cambia de ancho y cada ancho hornea su fondo.
    let w = (w / (28.0 * k)).ceil() * 28.0 * k;
    let h = if skin.chapin { 92.0 } else { 84.0 } * k;
    let rect = Rectangle::new((width - 12.0 * k - w).round(), (12.0 * k).round(), w.round(), h.round());
    ui.panel(d, skin.plate, rect);
    let top = rect.y + if skin.chapin { 20.0 } else { 13.0 } * k;
    fonts.label(d, &title, rect.x + pad, top, skin.heading, skin, skin.accent);
    keys(d, fonts, skin, hint, rect.x + rect.width - pad, top + 2.0 * k, Align::Right, DIM, 1.0);
    keys(d, fonts, skin, &second, rect.x + pad, top + 36.0 * k, Align::Left, DIM, 1.0);
    rect
}

// Los titulos ----------------------------------------------------------------

/// El ancho con que se hornea el titulo de una version en una pantalla.
fn title_key(chapin: bool, width: f32, height: f32, light: bool) -> Key {
    let wide = (width * if chapin { 0.74 } else { 0.86 }).min(height * 2.2);
    Key::Title { chapin, width: ((wide / 16.0).round() * 16.0) as u32, light }
}

/// El titulo de una version centrado en `(cx, cy)`, a `shrink` veces su
/// tamano de la bienvenida: el cuerpo con opacidad `body` y, en OCASO, la
/// luz de las grietas (`light`, que late con `beat`) y las chispas que
/// suben. Devuelve donde acaban las letras, o `None` si falta la letra.
#[allow(clippy::too_many_arguments)]
fn title(d: &mut RaylibDrawHandle, ui: &Ui, chapin: bool, cx: f32, cy: f32, shrink: f32, body: f32, light: f32, beat: f32, age: f32) -> Option<f32> {
    let (width, height) = (d.get_screen_width() as f32, d.get_screen_height() as f32);
    let key = |light| title_key(chapin, width, height, light);
    let place = |w: f32, h: f32| Rectangle::new((cx - w * shrink * 0.5).round(), (cy - h * shrink * 0.5).round(), w * shrink, h * shrink);
    let rect = ui.art.put(d, ui.thread, key(false), place, 0.0, fade(Color::WHITE, body), false)?;
    if chapin {
        return Some(rect.y + rect.height);
    }
    // La luz: late con la estrella (un golpe y su eco).
    let thump = (-beat * 7.0).exp() + 0.5 * (-(beat - 0.22).abs() * 9.0).exp();
    let glow = (0.62 + 0.38 * thump).min(1.0) * light;
    ui.art.put(d, ui.thread, key(true), place, 0.0, fade(Color::WHITE, glow), true);
    // Las chispas: suben desde las letras, se mecen y se apagan.
    let margin = rect.height * TITLE_MARGIN / (1.0 + 2.0 * TITLE_MARGIN);
    for i in 0..54u32 {
        let h = crate::math::hash_u32(i * 2_654_435_761 + 17);
        let r = |n: u32| (crate::math::hash_u32(h ^ n) >> 8) as f32 / 16_777_216.0;
        let life = (age * (0.16 + 0.2 * r(1)) + r(2)).fract();
        let x = rect.x + rect.width * (0.06 + 0.88 * r(3)) + (age * (0.8 + r(4)) + r(5) * 6.3).sin() * rect.height * 0.03;
        let y = rect.y + rect.height - margin * (0.6 + 0.8 * r(6)) - life * rect.height * (0.55 + 0.4 * r(7));
        let size = rect.height * (0.012 + 0.03 * r(8)) * (1.0 - 0.5 * life);
        let shine = (life * std::f32::consts::PI).sin().powf(0.7) * light * (0.5 + 0.5 * r(9));
        let color = Color::new(255, (150.0 + 70.0 * r(10) - 60.0 * life) as u8, (60.0 * (1.0 - life)) as u8, (255.0 * shine) as u8);
        ui.art.put(d, ui.thread, Key::Spark, |_, _| Rectangle::new(x - size, y - size, size * 2.0, size * 2.0), 0.0, color, true);
    }
    Some(rect.y + rect.height - margin)
}

/// La bienvenida: el titulo de la version en grande, con un velo oscuro
/// detras para que se lea sobre lo que sea, una frase y las teclas.
fn intro(d: &mut RaylibDrawHandle, ui: &Ui, skin: &Skin, app: &App, hud: &Hud, width: f32, height: f32) {
    let fonts = ui.fonts;
    let k = ui.k();
    let age = hud.age;
    let out = ((INTRO - age) / 1.6).clamp(0.0, 1.0);
    let ease = |from: f32, to: f32| crate::math::smoothstep(from, to, age) * out;
    let cy = height * 0.36;
    // El velo: oscuro detras del titulo, se pierde arriba y abajo.
    let veil = (ease(0.0, 1.0) * if skin.chapin { 120.0 } else { 150.0 }) as u8;
    let (dark, clear) = (Color::new(4, 2, 6, veil), Color::new(4, 2, 6, 0));
    let (mid, half) = (cy as i32, (height * 0.34) as i32);
    d.draw_rectangle_gradient_v(0, mid - half, width as i32, half, clear, dark);
    d.draw_rectangle_gradient_v(0, mid, width as i32, half, dark, clear);

    // Mientras entra el titulo se hornea el de la otra version: asi la
    // primera V no da un tiron.
    ui.art.ensure(d, ui.thread, &title_key(!skin.chapin, width, height, false));
    let beat = app.cycle.beat_phase.fract() as f32;
    let below = match title(d, ui, skin.chapin, width * 0.5, cy, 1.0, ease(0.5, 1.7), ease(0.1, 1.1), beat, age) {
        Some(below) => below,
        None => {
            // Sin la letra del titulo: el texto del HUD.
            let text = if skin.chapin { "GUATEMALA" } else { "OCASO" };
            let size = (height / 6.0).min(120.0);
            let w = fonts.width_at(text, Style::Title, size);
            fonts.draw_at(d, text, (width - w) * 0.5, cy - size * 0.6, Style::Title, size, fade(skin.accent, ease(0.5, 1.7)));
            cy + size * 0.6
        }
    };
    let words = ease(1.6, 2.8);
    let (line, hints) = if skin.chapin {
        ("Un sistema solar bien chapín, con su marimba y sus camionetas", "[F1] controles   [N] siguiente lugar   [V] OCASO")
    } else {
        ("Una estrella moribunda late como un corazón", "[J] desatar el final   [F1] controles   [V] versión chapina")
    };
    let y = below + 14.0 * k;
    let lw = fonts.label_width(line, skin.heading, skin);
    fonts.label(d, line, (width - lw) * 0.5 + 1.0, y + 2.0, skin.heading, skin, fade(Color::new(0, 0, 0, 190), words));
    fonts.label(d, line, (width - lw) * 0.5, y, skin.heading, skin, fade(TEXT, words));
    // Una raya con su rombo entre la frase y las teclas.
    let rule = y + 40.0 * k;
    let reach = (lw * 0.5).min(width * 0.3);
    let stroke = fade(skin.accent, words * 0.75);
    d.draw_line_ex(Vector2::new(width * 0.5 - reach, rule), Vector2::new(width * 0.5 - 10.0 * k, rule), 1.0, stroke);
    d.draw_line_ex(Vector2::new(width * 0.5 + 10.0 * k, rule), Vector2::new(width * 0.5 + reach, rule), 1.0, stroke);
    d.draw_poly(Vector2::new(width * 0.5, rule), 4, 4.5 * k, 0.0, stroke);
    keys(d, fonts, skin, hints, width * 0.5, rule + 14.0 * k, Align::Center, TEXT, words);
}

pub fn draw(d: &mut RaylibDrawHandle, ui: &Ui, app: &App, fps: u32, hud: &Hud, tweaks: &Tweaks, player: Option<&PlayerView>) {
    let width = d.get_screen_width() as f32;
    let height = d.get_screen_height() as f32;
    if hud.hidden {
        return;
    }
    let fonts = ui.fonts;
    let k = ui.k();
    let chapin = app.ship_name().is_some();
    let skin = Skin::of(chapin);
    let greeting = hud.age < INTRO && !hud.intro_over;

    let plate = status(d, ui, &skin, app, width);

    // Nombres de los cuerpos sobre su posicion en pantalla; los que caen
    // bajo la placa de arriba no se dibujan (se enciman con su texto).
    // (Mientras se ve un titulo no salen: se le enciman.)
    let titled = greeting || app.transition_title().is_some_and(|(_, shown)| shown > 0.05);
    for m in app.markers.iter().filter(|_| !titled) {
        let name = pretty(m.name);
        let distance = if m.distance >= 1000.0 {
            format!("{:.1}k", m.distance / 1000.0)
        } else {
            format!("{:.0}", m.distance)
        };
        let nw = fonts.width(name, Style::Bold);
        let dw = fonts.width(&distance, Style::Small);
        let w = nw + dw + 28.0 * k;
        // La marca sale en pixeles del render: en pantalla completa, estirada.
        let (x, y) = ui.fit.point(m.x, m.y);
        let rect = Rectangle::new((x - w * 0.5).round(), (y - 40.0 * k).round(), w.round(), (26.0 * k).round());
        let below = Rectangle::new(rect.x, rect.y, rect.width, 40.0 * k);
        if below.check_collision_recs(plate) {
            continue;
        }
        // Un rombo en el lugar y la pildora con el nombre encima.
        d.draw_poly(Vector2::new(x, y - 6.0 * k), 4, 5.0 * k, 0.0, skin.accent);
        pill(d, rect, fade(skin.flat, 0.8), Some(fade(skin.accent, 0.45)));
        fonts.draw(d, name, rect.x + 10.0 * k, rect.y + 2.0 * k, Style::Bold, skin.accent);
        fonts.draw(d, &distance, rect.x + 18.0 * k + nw, rect.y + 5.0 * k, Style::Small, DIM);
    }

    // Arriba a la izquierda: las estadisticas y el panel de Faro.
    let mut left_top = 12.0 * k;
    if hud.stats {
        let rect = lines_panel(d, fonts, &skin, 12.0 * k, left_top, &stats_lines(app, fps));
        left_top = rect.y + rect.height + 10.0 * k;
    }
    if let Some(readout) = app.faro_readout().filter(|_| !titled) {
        lines_panel(d, fonts, &skin, 12.0 * k, left_top, &faro_lines(&readout));
    }
    if tweaks.open {
        let lines = tweaks.lines(app);
        let (_, h) = lines_size(fonts, &lines);
        lines_panel(d, fonts, &skin, 12.0 * k, (height - h) * 0.42, &lines);
    }

    let board = dashboard(d, ui, &skin, app, height);
    let mut right = width - 12.0 * k;
    if let Some(view) = player {
        let rect = draw_player(d, ui, &skin, view, (width as i32, height as i32));
        right = rect.x;
    }

    // Arriba al centro: lo urgente.
    let f = &app.flight;
    let alert = if f.braking {
        Some("[X] freno activo")
    } else if f.boundary {
        Some("Límite del sistema: girá para regresar")
    } else if app.freeze.is_some() {
        Some("[F7] frustum congelado: el culling y el LOD se calculan desde la cámara amarilla")
    } else {
        None
    };
    if let Some(text) = alert {
        let w = toast_width(fonts, text);
        let rect = Rectangle::new(((width - w) * 0.5).round(), (14.0 * k).round(), w.round(), (36.0 * k).round());
        let color = if app.freeze.is_some() && !f.braking && !f.boundary { skin.accent } else { WARN };
        pill(d, rect, skin.flat, Some(fade(color, 0.8)));
        keys(d, fonts, &skin, text, rect.x + 18.0 * k, rect.y + 7.0 * k, Align::Left, color, 1.0);
    }

    if greeting {
        intro(d, ui, &skin, app, hud, width, height);
    }
    // El titulo de la transicion entre versiones, al centro.
    if let Some((name, shown)) = app.transition_title() {
        if title(d, ui, chapin, width * 0.5, height * 0.44, if chapin { 0.94 } else { 0.78 }, shown, shown, app.cycle.beat_phase.fract() as f32, hud.age).is_none() {
            let size = (height / 7.0).min(120.0);
            let w = fonts.width_at(name, Style::Title, size);
            fonts.draw_at(d, name, (width - w) * 0.5, height * 0.44 - size * 0.6, Style::Title, size, fade(skin.accent, shown));
        }
    }

    // Los avisos, abajo: entre el tablero y el reproductor si caben; si no,
    // encima de ellos.
    let left = board.x + board.width;
    let mut texts: Vec<(String, f32)> = Vec::new();
    if let Some(text) = hud.prompt {
        // Siempre visible mientras se este cerca: respira.
        let (key, rest) = text.split_once(" · ").unwrap_or(("Enter", text));
        texts.push((format!("[{key}] {rest}"), (0.5 + 0.5 * (hud.age * 3.0).sin()) * 0.25 + 0.75));
    }
    let fading = |age: f32| ((TOAST - age) / 0.5).clamp(0.0, 1.0) * (age / 0.15).clamp(0.0, 1.0);
    if let Some((text, since)) = hud.banner
        && hud.age - since < TOAST
    {
        let text = match hud.banner_where {
            Some(place) => format!("{}  ·  {}", pretty(text), pretty(place)),
            None => pretty(text).to_string(),
        };
        texts.push((text, fading(hud.age - since)));
    }
    if let Some((name, since)) = hud.preset
        && hud.age - since < TOAST
    {
        texts.push((format!("Vista fija: {name}"), fading(hud.age - since)));
    }
    let widest = texts.iter().map(|(t, _)| toast_width(fonts, t)).fold(0.0, f32::max);
    let fits = widest + 24.0 * k < right - left;
    let (center, bottom) = if fits { ((left + right) * 0.5, height - 14.0 * k) } else { (width * 0.5, board.y - 10.0 * k) };
    for (slot, (text, alpha)) in texts.iter().enumerate() {
        toast(d, fonts, &skin, text, *alpha, center, bottom, slot as f32);
    }

    if hud.help {
        help(d, fonts, &skin, width, height);
    }
}

/// Las lineas del panel de estadisticas (F2).
fn stats_lines(app: &App, fps: u32) -> Vec<String> {
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
    vec![
        format!(
            "FPS {fps}   ·   frame {:.1} ms   ·   {} hilos   ·   {}×{}{}",
            s.ms_total,
            parallel::workers(),
            app.renderer.width(),
            app.renderer.height(),
            if app.dynamic.is_low() {
                "  (en movimiento)"
            } else if app.dynamic.enabled {
                ""
            } else {
                "  (resolución dinámica apagada)"
            }
        ),
        format!(
            "vértices {:.1}   triángulos {:.1}   raster {:.1}   shading {:.1}   post {:.1} ms",
            s.ms_vertex, s.ms_triangles, s.ms_raster, s.ms_shade, s.ms_post
        ),
        format!("objetos {}   fuera de vista {}", s.objects, s.objects_culled),
        format!("triángulos {}   frustum −{}   espalda −{}   sin pixel −{}", t.input, t.frustum, t.backface, t.small),
        format!("recortados {}   rasterizados {}   pixeles {}   partículas {}", t.clipped, t.emitted, s.pixels_shaded, s.particles),
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
    ]
}

/// La ayuda (F1): cada tecla con lo que hace, en dos columnas, al centro.
fn help(d: &mut RaylibDrawHandle, fonts: &Fonts, skin: &Skin, width: f32, height: f32) {
    const KEYS: [(&str, &str); 26] = [
        ("Mouse", "apuntar la nave"),
        ("Clic der.", "mirar alrededor"),
        ("Rueda", "distancia de la cámara"),
        ("W S A D", "adelante, atrás y a los lados"),
        ("Espacio Ctrl", "subir y bajar"),
        ("Q E", "alabeo"),
        ("Shift", "turbo"),
        ("X", "freno de emergencia"),
        ("H", "volver al principio (sin reiniciar el tiempo)"),
        ("N", "ir al siguiente lugar"),
        ("L", "modo local, cerca de un planeta"),
        ("O", "crucero: rodea el planeta"),
        ("Y", "ruta predicha: a dónde te lleva la gravedad"),
        ("V", "versión chapina / OCASO"),
        ("B  1-9", "nave: 6 botellas, tuk-tuk, camioneta y colectivo"),
        ("Enter", "el minijuego del lugar, cuando se ofrece"),
        ("K  U I  − +", "música: pausa, canción y volumen"),
        ("J", "desatar el final / renacer"),
        ("F", "gran llamarada de Faro"),
        ("R  [ ]", "rebobinar, −30 y +30 s"),
        ("C  P  M", "cámara, foto y sonido"),
        ("T", "ajustes en vivo (flechas)"),
        (", .", "vistas fijas (también RePág y AvPág)"),
        ("F2 F3 F4 F5", "estadísticas, wireframe, colores de LOD y LOD"),
        ("F6 F7 F8", "otra semilla, congelar frustum, resolución dinámica"),
        ("F9 F11 Tab", "ocultar todo, pantalla completa, soltar el mouse"),
    ];
    let k = fonts.scale();
    let rows = KEYS.len().div_ceil(2);
    let key_w = KEYS.iter().map(|(key, _)| key.split(' ').filter(|p| !p.is_empty()).map(|p| cap_width(fonts, p) + 4.0 * k).sum::<f32>()).fold(0.0, f32::max);
    let text_w = KEYS.iter().map(|(_, text)| fonts.width(text, Style::Body)).fold(0.0, f32::max);
    let column = key_w + 10.0 * k + text_w;
    let pad = 22.0 * k;
    let (w, h) = (column * 2.0 + 36.0 * k + pad * 2.0, rows as f32 * 28.0 * k + pad * 2.0 + 40.0 * k);
    let rect = Rectangle::new(((width - w) * 0.5).round(), ((height - h) * 0.5).round(), w.round(), h.round());
    flat_panel(d, rect, Color::new(skin.flat.r, skin.flat.g, skin.flat.b, 238), skin.edge, 14.0 * k);
    fonts.label(d, "Controles", rect.x + pad, rect.y + pad - 6.0 * k, skin.heading, skin, skin.accent);
    let hint = "[F1] cerrar";
    keys(d, fonts, skin, hint, rect.x + rect.width - pad, rect.y + pad - 4.0 * k, Align::Right, DIM, 1.0);
    for (i, (key, text)) in KEYS.iter().enumerate() {
        let x = rect.x + pad + (i / rows) as f32 * (column + 36.0 * k);
        let y = rect.y + pad + 40.0 * k + (i % rows) as f32 * 28.0 * k;
        let mut at = x;
        for part in key.split(' ').filter(|p| !p.is_empty()) {
            at += cap(d, fonts, skin, part, at, y, Cap::Plain, 1.0) + 4.0 * k;
        }
        fonts.draw(d, text, x + key_w + 10.0 * k, y, Style::Body, TEXT);
    }
}

/// Lo que pone encima un minijuego: texto con sombra (para que se lea sobre
/// lo que sea), carteles y placas de tela, rotulos pintados, teclas, barras
/// y el velocimetro. Viene en pixeles del render: en pantalla completa se
/// estira igual que el juego.
pub fn minigame(d: &mut RaylibDrawHandle, ui: &Ui, overlay: &Overlay) {
    let fonts = ui.fonts;
    let fit = ui.fit;
    let skin = Skin::of(true);
    let rgba = |c: [u8; 4]| Color::new(c[0], c[1], c[2], c[3]);
    let s = fit.scale;
    let k = ui.k();
    let place = |x: f32, y: f32, w: f32, h: f32| {
        let (x, y) = fit.point(x, y);
        Rectangle::new(x, y, w * s, h * s)
    };
    for item in &overlay.items {
        match item {
            Item::Panel { x, y, w, h } => {
                let rect = place(*x, *y, *w, *h);
                // Las muy bajitas (un renglon) van lisas: no les cabe la orilla.
                if rect.height < 56.0 * k {
                    flat_panel(d, rect, skin.flat, skin.edge, 12.0 * k);
                } else {
                    ui.panel(d, piel::Panel::Placa, rect);
                }
            }
            Item::Card { x, y, w, h } => ui.panel(d, piel::Panel::Cartel, place(*x, *y, *w, *h)),
            Item::Sign { text, x, y, size, paint, align, alpha } => {
                let (x, y) = fit.point(*x, *y);
                ui.sign(d, text, x, y, size * s, *paint, *align, *alpha as f32 / 255.0);
            }
            Item::Keys { text, x, y, align } => {
                let (x, y) = fit.point(*x, *y);
                keys(d, fonts, &skin, text, x, y, *align, TEXT, 1.0);
            }
            Item::Gauge { x, y, size, value, max, unit, hot } => {
                let (x, y) = fit.point(*x, *y);
                gauge(d, ui, &skin, x, y, size * s, value / max.max(1e-3), *hot, &format!("{value:.0}"), unit);
            }
            Item::Bar { x, y, w, h, fill, color, vertical } => {
                let rect = place(*x, *y, *w, *h);
                let round = (12.0 / rect.width.min(rect.height)).min(1.0);
                let fill = fill.clamp(0.0, 1.0);
                if fill < 1.0 {
                    d.draw_rectangle_rounded(rect, round, 8, Color::new(255, 255, 255, 34));
                }
                if fill > 0.0 {
                    let part = if *vertical {
                        let fh = (rect.height * fill).max(rect.width);
                        Rectangle::new(rect.x, rect.y + rect.height - fh, rect.width, fh)
                    } else {
                        Rectangle::new(rect.x, rect.y, (rect.width * fill).max(rect.height), rect.height)
                    };
                    d.draw_rectangle_rounded(part, round, 8, rgba(*color));
                }
            }
            Item::Text { text, x, y, font, color, align } => {
                let (style, size) = match font {
                    Font::Small => (Style::Small, fonts.font(Style::Small).1),
                    Font::Body => (Style::Body, fonts.font(Style::Body).1),
                    Font::Bold => (Style::Bold, fonts.font(Style::Bold).1),
                    Font::Heading => (Style::Fat, fonts.font(Style::Fat).1),
                    Font::Big(size) => (Style::Title, *size * s),
                };
                let w = fonts.width_at(text, style, size);
                let (x, y) = fit.point(*x, *y);
                let x = match align {
                    Align::Left => x,
                    Align::Center => x - w * 0.5,
                    Align::Right => x - w,
                };
                let dark = Color::new(10, 4, 20, (color[3] as f32 * 0.7) as u8);
                if matches!(font, Font::Big(_)) {
                    // La gorda lleva su filo oscuro y el canto abajo.
                    let edge = (size / 22.0).clamp(1.0, 4.0 * k.max(1.0));
                    for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, -0.7), (-0.7, 0.7), (0.7, 0.7), (0.6, 2.2), (0.0, 2.6)] {
                        fonts.draw_at(d, text, x + ox * edge, y + oy * edge, style, size, dark);
                    }
                } else {
                    let drop = (size / 18.0).clamp(1.0, (3.0 * k).max(1.0));
                    fonts.draw_at(d, text, x + drop, y + drop, style, size, Color::new(0, 0, 0, (color[3] as f32 * 0.55) as u8));
                }
                fonts.draw_at(d, text, x, y, style, size, rgba(*color));
            }
        }
    }
}

/// `x` en notacion cientifica con exponente en superindice: 4.8×10⁻¹¹.
fn scientific(x: f64, digits: usize) -> String {
    let exponent = x.abs().log10().floor() as i32;
    let mantissa = x / 10f64.powi(exponent);
    let sup: String = exponent
        .to_string()
        .chars()
        .map(|c| match c {
            '-' => '⁻',
            d => "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().nth(d.to_digit(10).unwrap() as usize).unwrap(),
        })
        .collect();
    format!("{mantissa:.digits$}×10{sup}")
}

/// El panel de Faro: las ecuaciones del modelo con sus numeros y lo que
/// esta pasando.
fn faro_lines(r: &FaroReadout) -> Vec<String> {
    let flare = match r.flare {
        FlarePhase::Ready => "lista".to_string(),
        FlarePhase::Twisting => format!("la corteza tuerce: ψ {:.2} de {:.1} rad", r.twist, r.critical_twist),
        FlarePhase::Fireball => format!("bola de fuego al {:.0} %, L = L0 (1 − t/380 s)³", r.fireball * 100.0),
    };
    vec![
        format!("Faro · púlsar y magnetar (1E 1547.0−5408) · a {:.0} km", r.distance),
        format!("P = {:.5} s   ·   dP/dt = {} s/s", r.period, scientific(r.pdot, 2)),
        format!("B = 3.2×10¹⁹ √(P dP/dt) = {} G (el límite cuántico es 4.4×10¹³)", scientific(r.field, 1)),
        format!("τ = P / 2(dP/dt) = {:.0} años   ·   dE/dt = 4π²I(dP/dt)/P³ = {} erg/s", r.age_years, scientific(r.edot, 1)),
        format!("cilindro de luz cP/2π = {:.0} km   ·   rs/R = {:.3}: se ve el {:.0} % de la estrella", r.light_cylinder_km, r.compactness, 50.0 / (1.0 - r.compactness)),
        format!("torsión ψ = {:.2} rad   ·   casquete {:.1}° a {:.1} MK   ·   corteza {:.1} MK", r.twist, r.cap_degrees, r.cap_temperature / 1e6, r.crust_temperature / 1e6),
        format!("estallidos: {}   ·   gran llamarada [F]: {flare}", if r.storm > 0.3 { "tormenta" } else { "sueltos" }),
    ]
}

// El reproductor -------------------------------------------------------------

/// Tamano del reproductor (a escala 1): abajo a la derecha.
const PLAYER_SIZE: (f32, f32) = (432.0, 134.0);

/// Donde va cada cosa del reproductor.
struct PlayerLayout {
    rect: Rectangle,
    /// El centro y el radio del disco.
    disc: (f32, f32, f32),
    /// Donde empieza el texto.
    text: f32,
    /// Anterior, pausa y siguiente.
    buttons: [Rectangle; 3],
    bar: Rectangle,
    /// Donde empiezan las barritas del volumen.
    volume: (f32, f32),
}

fn player_layout(screen: (i32, i32), k: f32) -> PlayerLayout {
    let (w, h) = (PLAYER_SIZE.0 * k, PLAYER_SIZE.1 * k);
    let (x, y) = ((screen.0 as f32 - w - 12.0 * k).round(), (screen.1 as f32 - h - 12.0 * k).round());
    let radius = 46.0 * k;
    let disc = (x + 18.0 * k + radius, y + h * 0.5 + 5.0 * k, radius);
    let text = disc.0 + radius + 18.0 * k;
    let row = y + h - 30.0 * k;
    let button = |cx: f32, size: f32| Rectangle::new(text + cx * k - size * k * 0.5, row - size * k * 0.5, size * k, size * k);
    PlayerLayout {
        rect: Rectangle::new(x, y, w, h),
        disc,
        text,
        buttons: [button(16.0, 30.0), button(58.0, 38.0), button(100.0, 30.0)],
        bar: Rectangle::new(text, y + 72.0 * k, x + w - 18.0 * k - text, 5.0 * k),
        volume: (x + w - 18.0 * k - 62.0 * k, row),
    }
}

/// Lo que se toco del reproductor con el mouse.
pub enum PlayerHit {
    Previous,
    Toggle,
    Next,
    /// En la barra: a que fraccion de la cancion saltar.
    Seek(f32),
}

/// `scale` es la del HUD (`Fonts::scale`).
pub fn player_hit(screen: (i32, i32), scale: f32, mouse: (f32, f32)) -> Option<PlayerHit> {
    let layout = player_layout(screen, scale);
    let point = Vector2::new(mouse.0, mouse.1);
    if layout.buttons[0].check_collision_point_rec(point) {
        return Some(PlayerHit::Previous);
    }
    if layout.buttons[1].check_collision_point_rec(point) {
        return Some(PlayerHit::Toggle);
    }
    if layout.buttons[2].check_collision_point_rec(point) {
        return Some(PlayerHit::Next);
    }
    let bar = layout.bar;
    let grown = Rectangle::new(bar.x, bar.y - 8.0 * scale, bar.width, bar.height + 16.0 * scale);
    grown
        .check_collision_point_rec(point)
        .then(|| PlayerHit::Seek(((mouse.0 - bar.x) / bar.width).clamp(0.0, 1.0)))
}

/// El nombre de una cancion partido en titulo y artista: los archivos se
/// llaman "Titulo - Artista".
fn song_parts(name: &str) -> (&str, Option<&str>) {
    match name.split_once(" - ") {
        Some((title, artist)) if !title.trim().is_empty() && !artist.trim().is_empty() => (title.trim(), Some(artist.trim())),
        _ => (name, None),
    }
}

/// `text` cortado con puntos suspensivos para que quepa en `room`.
fn clipped(fonts: &Fonts, text: &str, style: Style, room: f32) -> String {
    if fonts.width(text, style) <= room {
        return text.to_string();
    }
    let mut cut = text.to_string();
    while !cut.is_empty() && fonts.width(&format!("{cut}…"), style) > room {
        cut.pop();
    }
    format!("{}…", cut.trim_end())
}

/// El reproductor: una placa de tela con un barrilete que gira mientras
/// suena (con un arco alrededor que marca por donde va la cancion), el
/// titulo y el artista, la barra de avance con sus tiempos, los botones
/// dibujados y el volumen en barritas. Devuelve el rectangulo que ocupa.
fn draw_player(d: &mut RaylibDrawHandle, ui: &Ui, skin: &Skin, view: &PlayerView, screen: (i32, i32)) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let layout = player_layout(screen, k);
    let rect = layout.rect;
    ui.panel(d, piel::Panel::Placa, rect);

    // El disco: gira con la cancion (en pausa se queda donde iba).
    let (cx, cy, radius) = layout.disc;
    let center = Vector2::new(cx.round(), cy.round());
    let (fill, right) = if view.length > 0.0 {
        ((view.time / view.length).clamp(0.0, 1.0), clock(view.length))
    } else {
        // La marimba no se acaba: va y viene.
        (0.5 + 0.5 * (view.time * 0.5).sin(), "en vivo".to_string())
    };
    d.draw_circle_v(Vector2::new(center.x + 2.0 * k, center.y + 3.0 * k), radius + 3.0 * k, Color::new(0, 0, 0, 90));
    let size = (radius * 2.0).round();
    let at = Rectangle::new(center.x - size * 0.5, center.y - size * 0.5, size, size);
    let spin = (view.time * 24.0).rem_euclid(360.0);
    if ui.art.put(d, ui.thread, Key::Kite(size as u32), |_, _| at, if spin == 0.0 { 0.001 } else { spin }, Color::WHITE, false).is_none() {
        d.draw_circle_v(center, radius, Color::new(142, 26, 48, 255));
    }
    d.draw_ring(center, radius + 3.0 * k, radius + 6.5 * k, -90.0, 270.0, 48, Color::new(255, 255, 255, 30));
    d.draw_ring(center, radius + 3.0 * k, radius + 6.5 * k, -90.0, -90.0 + 360.0 * fill, 48, skin.accent);

    // El titulo, el artista y cual es de cuantas.
    let x = layout.text;
    let room = rect.x + rect.width - 18.0 * k - x;
    let (name, artist) = song_parts(&view.title);
    let top = rect.y + 20.0 * k;
    fonts.draw(d, &clipped(fonts, name, Style::Fat, room), x, top, Style::Fat, TEXT);
    let state = if view.playing { "sonando" } else { "en pausa" };
    let count = format!("{} de {}  ·  {state}", view.index + 1, view.count);
    let line = match artist {
        Some(artist) => format!("{artist}  ·  {count}"),
        None => count,
    };
    fonts.draw(d, &clipped(fonts, &line, Style::Small, room), x, top + 27.0 * k, Style::Small, DIM);

    // La barra de avance con su perilla y los tiempos a los lados.
    let bar = layout.bar;
    d.draw_rectangle_rounded(bar, 1.0, 6, Color::new(255, 255, 255, 38));
    if fill > 0.0 {
        d.draw_rectangle_rounded(Rectangle::new(bar.x, bar.y, (bar.width * fill).max(bar.height), bar.height), 1.0, 6, skin.accent);
    }
    let knob = Vector2::new(bar.x + bar.width * fill, bar.y + bar.height * 0.5);
    d.draw_circle_v(knob, 7.0 * k, Color::new(0, 0, 0, 90));
    d.draw_circle_v(knob, 5.5 * k, TEXT);

    // Los botones: anterior, pausa (un circulo dorado) y siguiente.
    let mid = |r: &Rectangle| (r.x + r.width * 0.5, r.y + r.height * 0.5);
    let (px, py) = mid(&layout.buttons[0]);
    d.draw_rectangle_rounded(Rectangle::new(px - 9.0 * k, py - 7.0 * k, 3.0 * k, 14.0 * k), 0.5, 2, TEXT);
    triangle(d, Vector2::new(px + 8.0 * k, py - 7.0 * k), Vector2::new(px - 5.0 * k, py), Vector2::new(px + 8.0 * k, py + 7.0 * k), TEXT);
    let (bx, by) = mid(&layout.buttons[1]);
    d.draw_circle_v(Vector2::new(bx, by), 18.0 * k, skin.accent);
    let dark = Color::new(34, 18, 40, 255);
    if view.playing {
        d.draw_rectangle_rounded(Rectangle::new(bx - 6.5 * k, by - 8.0 * k, 4.5 * k, 16.0 * k), 0.4, 2, dark);
        d.draw_rectangle_rounded(Rectangle::new(bx + 2.0 * k, by - 8.0 * k, 4.5 * k, 16.0 * k), 0.4, 2, dark);
    } else {
        triangle(d, Vector2::new(bx - 5.0 * k, by - 9.0 * k), Vector2::new(bx - 5.0 * k, by + 9.0 * k), Vector2::new(bx + 10.0 * k, by), dark);
    }
    let (nx, ny) = mid(&layout.buttons[2]);
    triangle(d, Vector2::new(nx - 8.0 * k, ny - 7.0 * k), Vector2::new(nx + 5.0 * k, ny), Vector2::new(nx - 8.0 * k, ny + 7.0 * k), TEXT);
    d.draw_rectangle_rounded(Rectangle::new(nx + 6.0 * k, ny - 7.0 * k, 3.0 * k, 14.0 * k), 0.5, 2, TEXT);

    // Los tiempos, entre los botones y el volumen.
    let times = format!("{} / {right}", clock(view.time));
    fonts.draw(d, &times, x + 128.0 * k, by - 9.0 * k, Style::Small, DIM);

    // El volumen: una bocina y cinco barritas que suben.
    let (vx, vy) = layout.volume;
    d.draw_rectangle((vx - 16.0 * k) as i32, (vy - 3.0 * k) as i32, (4.0 * k) as i32, (7.0 * k) as i32, DIM);
    triangle(d, Vector2::new(vx - 12.0 * k, vy - 3.0 * k), Vector2::new(vx - 5.0 * k, vy - 8.0 * k), Vector2::new(vx - 5.0 * k, vy + 8.0 * k), DIM);
    triangle(d, Vector2::new(vx - 12.0 * k, vy - 3.0 * k), Vector2::new(vx - 5.0 * k, vy + 8.0 * k), Vector2::new(vx - 12.0 * k, vy + 4.0 * k), DIM);
    for i in 0..5 {
        let tall = (5.0 + 3.5 * i as f32) * k;
        let lit = view.volume > (i as f32 + 0.5) / 5.0 - 0.1;
        let piece = Rectangle::new(vx + 4.0 * k + i as f32 * 11.0 * k, vy + 9.0 * k - tall, 7.0 * k, tall);
        d.draw_rectangle_rounded(piece, 0.5, 3, if lit { skin.accent } else { Color::new(255, 255, 255, 40) });
    }
    rect
}

fn clock(seconds: f32) -> String {
    format!("{}:{:02}", (seconds / 60.0) as u32, (seconds % 60.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El panel de Faro solo usa letras que se cargan de Segoe UI.
    #[test]
    fn the_faro_panel_only_uses_loaded_glyphs() {
        let readout = FaroReadout {
            distance: 812.0,
            period: 2.0698,
            pdot: 4.83e-11,
            field: 3.2e14,
            age_years: 679.0,
            edot: 2.14e35,
            light_cylinder_km: 98_760.0,
            compactness: 0.345,
            twist: 1.05,
            critical_twist: 3.0,
            cap_degrees: 10.0,
            cap_temperature: 5.4e6,
            crust_temperature: 1.8e6,
            storm: 0.0,
            flare: FlarePhase::Fireball,
            fireball: 0.64,
        };
        for line in faro_lines(&readout) {
            for c in line.chars() {
                assert!(GLYPHS.contains(c), "falta {c:?} en: {line}");
            }
        }
        assert_eq!(scientific(4.83e-11, 2), "4.83×10⁻¹¹");
    }

    /// En la ventana el render va 1:1; en pantalla completa se estira sin
    /// deformarse y, si el monitor no es 16:9, queda centrado con franjas.
    #[test]
    fn the_render_fits_the_screen_without_stretching() {
        assert_eq!(Fit::new((1280, 720), (1280, 720)), Fit { scale: 1.0, x: 0.0, y: 0.0 });
        let full = Fit::new((1280, 720), (1920, 1080));
        assert_eq!(full, Fit { scale: 1.5, x: 0.0, y: 0.0 });
        assert_eq!(full.point(640.0, 360.0), (960.0, 540.0));
        // Un monitor 16:10: franjas arriba y abajo.
        let tall = Fit::new((1280, 720), (1920, 1200));
        assert_eq!((tall.scale, tall.x, tall.y), (1.5, 0.0, 60.0));
        // Uno ultra ancho: franjas a los lados.
        let wide = Fit::new((1280, 720), (2560, 1080));
        assert_eq!((wide.scale, wide.x, wide.y), (1.5, 320.0, 0.0));
    }

    /// El HUD crece con la imagen: 1 en la ventana de 720, 1.5 a 1080 (en
    /// pantalla completa o con un render de ese alto).
    #[test]
    fn the_hud_scales_with_the_picture() {
        assert_eq!(ui_scale(720, Fit::new((1280, 720), (1280, 720))), 1.0);
        assert_eq!(ui_scale(720, Fit::new((1280, 720), (1920, 1080))), 1.5);
        assert_eq!(ui_scale(1080, Fit::new((1920, 1080), (1920, 1080))), 1.5);
    }

    /// Un renglon de teclas se parte en teclas y texto.
    #[test]
    fn a_key_row_splits_into_caps_and_text() {
        assert_eq!(key_parts("[N] lugar   [←][→] carril"), vec![(true, "N"), (false, " lugar   "), (true, "←"), (true, "→"), (false, " carril")]);
        assert_eq!(key_parts("sin teclas"), vec![(false, "sin teclas")]);
    }

    /// Las canciones se llaman "Titulo - Artista"; la marimba no.
    #[test]
    fn a_song_name_splits_into_title_and_artist() {
        assert_eq!(song_parts("Aquí Te Voy - Bohemia Suburbana"), ("Aquí Te Voy", Some("Bohemia Suburbana")));
        assert_eq!(song_parts("Marimba procedural"), ("Marimba procedural", None));
        assert_eq!(grouped(12_480.4), "12 480");
        assert_eq!(grouped(999.0), "999");
    }

    /// Todo lo que el HUD escribe por su cuenta esta en las letras cargadas.
    #[test]
    fn the_hud_only_writes_loaded_glyphs() {
        for text in ["máx 120 u/s", "Límite del sistema: girá para regresar", "Una estrella moribunda late como un corazón", "Un sistema solar bien chapín, con su marimba y sus camionetas", "−12 480"] {
            for c in text.chars() {
                assert!(GLYPHS.contains(c), "falta {c:?} en: {text}");
            }
        }
    }
}
