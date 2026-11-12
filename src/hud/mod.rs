//! HUD en 2D. No es parte del pipeline 3D: se dibuja encima de la imagen ya
//! renderizada.
//!
//! El vuelo lleva un **HUD de piloto** (`piloto.rs`): lineas finas sobre la
//! vista, sin placas, con la cinta de rumbo arriba, la velocidad y la
//! altitud en cintas a los lados, los modos como luces abajo y un solo
//! aviso a la vez; en ambar de brasa en OCASO y en blanco tibio en la
//! chapina (con los colores de la faja en detalles chicos). Lo
//! que tiene dibujo (los titulos de la bienvenida, el barrilete de la
//! musica, los rotulos de los minijuegos) se **hornea en CPU**
//! (`rotulos.rs`, `piel.rs`) y raylib solo lo pega como textura:
//!
//! - **OCASO**: el titulo forjado en hierro con grietas de lava que laten
//!   con la estrella.
//! - **Chapina**: GUATEMALA cortada de un tejido bajo un lazo de papel
//!   picado, y un barrilete que gira mientras suena la musica.
//!
//! Todo va a la escala `k` (alto de la imagen entre 720): en pantalla
//! completa el HUD crece con la imagen y las letras se cargan a ese tamano,
//! para que se vean nitidas. La letra del texto es Segoe UI (la de Windows)
//! con sus tildes; si falta alguna letra se usa la de raylib sin tildes.

pub mod letras;
pub mod piel;
pub mod piloto;
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
    /// La romana de lapida (las frases de OCASO).
    CapsBig,
    /// La gorda mediana (las frases de la chapina).
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
const STYLES: [(&[&str], f32, bool); 8] = [
    (TEXT_FONT, 18.0, true),
    (BOLD_FONT, 18.0, true),
    (TEXT_FONT, 15.0, true),
    (BOLD_FONT, 24.0, true),
    (FAT_FONT, 120.0, false),
    (ROMAN_FONT, 24.0, true),
    (FAT_FONT, 22.0, true),
    (DIGIT_FONT, 120.0, false),
];

const LINE: f32 = 23.0;

const TEXT: Color = Color::new(242, 236, 226, 255);

/// El color y la letra de los titulos y las frases de cada version.
#[derive(Clone, Copy)]
struct Skin {
    chapin: bool,
    accent: Color,
    /// La letra de los encabezados, con su espacio entre letras.
    heading: Style,
    tracking: f32,
}

impl Skin {
    fn of(chapin: bool) -> Self {
        let accent = piloto::Theme::of(chapin).ink;
        if chapin {
            Self { chapin, accent, heading: Style::Fat, tracking: 0.4 }
        } else {
            Self { chapin, accent, heading: Style::CapsBig, tracking: 1.6 }
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
    key_parts(text).iter().map(|&(key, part)| if key { piloto::chip_width(fonts, part) + 4.0 * k } else { fonts.width(part, Style::Small) }).sum()
}

/// Un renglon de teclas y texto ("[N] siguiente lugar   [H] volver"): las
/// teclas son cajas de una linea del color de la version.
#[allow(clippy::too_many_arguments)]
fn keys(d: &mut RaylibDrawHandle, fonts: &Fonts, th: &piloto::Theme, text: &str, x: f32, y: f32, align: Align, color: Color, alpha: f32) -> f32 {
    let k = fonts.scale();
    let total = keys_width(fonts, text);
    let mut at = match align {
        Align::Left => x,
        Align::Center => x - total * 0.5,
        Align::Right => x - total,
    };
    for (key, part) in key_parts(text) {
        if key {
            at += piloto::chip(d, fonts, th, part, at, y, Cap::Plain, alpha) + 4.0 * k;
        } else {
            at += piloto::text(d, fonts, part, at, y + 1.0 * k, Style::Small, fade(color, alpha), Align::Left);
        }
    }
    total
}

/// Un bloque de lineas sueltas (estadisticas, ajustes, Faro). Devuelve el
/// rectangulo que ocupa.
fn lines_panel(d: &mut RaylibDrawHandle, fonts: &Fonts, th: &piloto::Theme, x: f32, y: f32, lines: &[String]) -> Rectangle {
    let k = fonts.scale();
    let pad = 14.0 * k;
    let (w, h) = lines_size(fonts, lines);
    let rect = Rectangle::new(x.round(), y.round(), w, h);
    piloto::block(d, th, rect);
    for (i, line) in lines.iter().enumerate() {
        fonts.draw(d, line, rect.x + pad, rect.y + pad - 3.0 * k + i as f32 * LINE * k, Style::Body, TEXT);
    }
    rect
}

/// Lo mismo en letra chica (el panel de Faro).
fn small_panel(d: &mut RaylibDrawHandle, fonts: &Fonts, th: &piloto::Theme, x: f32, y: f32, lines: &[String]) -> Rectangle {
    let k = fonts.scale();
    let pad = 10.0 * k;
    let (w, h) = small_size(fonts, lines);
    let rect = Rectangle::new(x.round(), y.round(), w, h);
    piloto::block(d, th, rect);
    for (i, line) in lines.iter().enumerate() {
        fonts.draw(d, line, rect.x + pad + 2.0 * k, rect.y + pad - 2.0 * k + i as f32 * 19.0 * k, Style::Small, fade(TEXT, th.alpha));
    }
    rect
}

fn small_size(fonts: &Fonts, lines: &[String]) -> (f32, f32) {
    let k = fonts.scale();
    let pad = 10.0 * k;
    let w = lines.iter().map(|l| fonts.width(l, Style::Small)).fold(0.0, f32::max) + pad * 2.0 + 2.0 * k;
    (w.round(), (lines.len() as f32 * 19.0 * k + pad * 2.0 - 4.0 * k).round())
}

fn lines_size(fonts: &Fonts, lines: &[String]) -> (f32, f32) {
    let k = fonts.scale();
    let pad = 14.0 * k;
    let w = lines.iter().map(|l| fonts.width(l, Style::Body)).fold(0.0, f32::max) + pad * 2.0;
    (w.round(), (lines.len() as f32 * LINE * k + pad * 2.0 - 6.0 * k).round())
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

/// Arriba a la izquierda: la version (o la fase del final, con su reloj) en
/// mayusculas sobre una raya, y abajo las teclas de moverse entre lugares.
/// Devuelve lo que ocupa.
fn corner(d: &mut RaylibDrawHandle, ui: &Ui, th: &piloto::Theme, app: &App) -> Rectangle {
    let fonts = ui.fonts;
    let k = ui.k();
    let c = &app.cycle;
    let clock = |seconds: f64| format!("{:02}:{:02}", (seconds / 60.0) as u32, (seconds % 60.0) as u32);
    let title = match (app.ship_name(), c.phase) {
        (Some(_), _) => "Guatemala".to_string(),
        (None, Phase::Agony) => Phase::Agony.name().to_string(),
        (None, phase) => format!("{}  {}", phase.name(), clock(c.tau)),
    };
    let hints = match (app.ship_name(), c.phase) {
        (Some(name), _) => format!("[V] OCASO   [B] {}   [N] lugar   [H] volver   [F1] ayuda", pretty(name)),
        (None, Phase::Agony) => "[J] desatar el final   [V] chapina   [N] lugar   [F1] ayuda".to_string(),
        (None, _) => "[J] renacer   [V] chapina   [N] lugar   [F1] ayuda".to_string(),
    };
    let (x, y) = (24.0 * k, 18.0 * k);
    let hw = keys_width(fonts, &hints);
    piloto::wash(d, th, Rectangle::new(0.0, 0.0, x + hw + 140.0 * k, y + 110.0 * k), 0);
    let tw = piloto::text(d, fonts, &title.to_uppercase(), x, y, Style::Heading, th.ink, Align::Left);
    let rule = y + 34.0 * k;
    piloto::rule(d, th, x, x + tw.max(120.0 * k) + 40.0 * k, rule, 2.0 * k, 9.0 * k, th.soft());
    keys(d, fonts, th, &hints, x, rule + 8.0 * k, Align::Left, th.text, 0.9);
    Rectangle::new(0.0, 0.0, x + tw.max(hw) + 16.0 * k, rule + 36.0 * k)
}

/// Los interruptores del vuelo con su tecla: encendidos si estan puestos.
fn switches(app: &App) -> [(&'static str, &'static str, Cap); 5] {
    let f = &app.flight;
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
    [("Z", "asist.", switch(app.settings.assist)), ("G", "graved.", switch(app.settings.gravity)), ("L", "local", local), ("O", "crucero", switch(app.settings.cruise)), ("Y", path.0, path.1)]
}

/// Las dos cintas: la velocidad a la izquierda (con la maxima marcada) y la
/// altitud a la derecha (sobre que, y en que marco o a que altura la
/// sostiene el modo local). Devuelve lo que ocupan.
fn tapes(d: &mut RaylibDrawHandle, ui: &Ui, th: &piloto::Theme, app: &App, width: f32, height: f32) -> [Rectangle; 2] {
    let k = ui.k();
    let f = &app.flight;
    let body = |i| pretty(app.body_name(i));
    let tall = 250.0 * k;
    let top = (height - tall) * 0.5;
    let speed = piloto::Tape {
        value: f.speed,
        span: f.max_speed.max(f.speed).max(20.0),
        label: "vel",
        unit: "u/s",
        limit: (f.max_speed > 0.0).then_some(f.max_speed),
        hot: f.max_speed > 0.0 && f.speed > f.max_speed * 1.02,
        notes: vec![format!("máx {:.0}", f.max_speed)],
    };
    let place = match (app.settings.local, f.local, f.hold) {
        (true, true, Some(h)) => format!("local · sostiene {h:.0}"),
        (true, true, None) => "local".to_string(),
        _ => format!("marco: {}", body(f.frame)),
    };
    let place = if f.contact { format!("{place} · contacto") } else { place };
    let altitude = piloto::Tape {
        value: f.altitude.max(0.0),
        span: (f.altitude * 1.2).max(60.0),
        label: "alt",
        unit: "u",
        limit: None,
        hot: f.contact,
        notes: vec![format!("sobre {}", body(f.nearest)), place],
    };
    [
        piloto::tape(d, ui, th, piloto::Side::Left, 118.0 * k, top, tall, &speed),
        piloto::tape(d, ui, th, piloto::Side::Right, width - 118.0 * k, top, tall, &altitude),
    ]
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
    keys(d, fonts, &piloto::Theme::of(skin.chapin), hints, width * 0.5, rule + 14.0 * k, Align::Center, TEXT, words);
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
    // Durante la bienvenida el vuelo no se ve: entra al irse el titulo.
    let shown = if hud.intro_over { 1.0 } else { crate::math::smoothstep(INTRO - 1.6, INTRO - 0.2, hud.age) };
    let th = piloto::Theme::of(chapin).faded(shown);
    // Mientras se ve un titulo no salen las marcas: se le enciman.
    let titled = greeting || app.transition_title().is_some_and(|(_, shown)| shown > 0.05);
    // Lo que ocupa el HUD: las marcas no se dibujan encima.
    let mut zones: Vec<Rectangle> = Vec::new();

    zones.push(corner(d, ui, &th, app));
    let compass = piloto::compass(d, ui, &th, &app.compass, width * 0.5, 46.0 * k, (440.0 * k).min(width * 0.4));
    zones.push(compass);
    zones.extend(tapes(d, ui, &th, app, width, height));
    piloto::reticle(d, &th, width * 0.5, height * 0.5, k);
    zones.push(piloto::modes(d, ui, &th, &switches(app), width * 0.5, height - 34.0 * k));
    if let Some(view) = player {
        zones.push(piloto::music(d, ui, &th, view, (width as i32, height as i32)));
    }

    // A la izquierda, abajo del titulo, las estadisticas; abajo a la
    // izquierda, el panel de Faro; al centro, los ajustes.
    if hud.stats {
        let top = zones[0].y + zones[0].height + 8.0 * k;
        zones.push(lines_panel(d, fonts, &th, 16.0 * k, top, &stats_lines(app, fps)));
    }
    if let Some(readout) = app.faro_readout().filter(|_| !titled) {
        // Abajo a la izquierda, en letra chica: arriba tapaba la cinta.
        let lines = faro_lines(&readout);
        let (_, h) = small_size(fonts, &lines);
        zones.push(small_panel(d, fonts, &th, 16.0 * k, height - h - 16.0 * k, &lines));
    }
    if tweaks.open {
        let lines = tweaks.lines(app);
        let (w, h) = lines_size(fonts, &lines);
        zones.push(lines_panel(d, fonts, &th, (width - w) * 0.5, (height - h) * 0.5, &lines));
    }

    // Un solo aviso, bajo el rumbo: lo urgente le gana a un lugar, y un
    // lugar a una vista fija.
    let f = &app.flight;
    let fading = |age: f32| ((TOAST - age) / 0.5).clamp(0.0, 1.0) * (age / 0.15).clamp(0.0, 1.0);
    let alert = if f.braking {
        Some(("[X] freno activo".to_string(), th.warn))
    } else if f.boundary {
        Some(("Límite del sistema: girá para regresar".to_string(), th.warn))
    } else if app.freeze.is_some() {
        Some(("[F7] frustum congelado: el culling y el LOD se calculan desde la cámara amarilla".to_string(), th.ink))
    } else {
        None
    };
    let banner = hud.banner.filter(|&(_, since)| hud.age - since < TOAST).map(|(text, since)| {
        let text = match hud.banner_where {
            Some(place) => format!("{}  ·  {}", pretty(text), pretty(place)),
            None => pretty(text).to_string(),
        };
        (text, fading(hud.age - since))
    });
    let preset = hud.preset.filter(|&(_, since)| hud.age - since < TOAST).map(|(name, since)| (format!("Vista fija: {name}"), fading(hud.age - since)));
    let notice_y = compass.y + compass.height + 6.0 * k;
    match (alert, banner.or(preset)) {
        (Some((text, color)), _) => zones.push(piloto::notice(d, ui, &th, &text, width * 0.5, notice_y, color, 1.0)),
        (None, Some((text, alpha))) if alpha > 0.0 => zones.push(piloto::notice(d, ui, &th, &text, width * 0.5, notice_y, th.text, alpha)),
        _ => {}
    }
    // El minijuego que se ofrece, abajo sobre los modos: respira.
    if let Some(text) = hud.prompt {
        let (key, rest) = text.split_once(" · ").unwrap_or(("Enter", text));
        let breathe = (0.5 + 0.5 * (hud.age * 3.0).sin()) * 0.25 + 0.75;
        zones.push(piloto::notice(d, ui, &th, &format!("[{key}] {rest}"), width * 0.5, height - 78.0 * k, th.ink, breathe));
    }

    if !titled {
        piloto::markers(d, ui, &th, app, &zones);
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
    if hud.help {
        help(d, fonts, &th, width, height);
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
fn help(d: &mut RaylibDrawHandle, fonts: &Fonts, th: &piloto::Theme, width: f32, height: f32) {
    const KEYS: [(&str, &str); 26] = [
        ("Mouse", "apuntar la nave"),
        ("Clic_der.", "mirar alrededor"),
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
    // (Las teclas van separadas por espacios; `_` es un espacio adentro de
    // una tecla.)
    let parts = |key: &'static str| key.split(' ').filter(|p| !p.is_empty()).map(|p| p.replace('_', " "));
    let key_w = KEYS.iter().map(|(key, _)| parts(key).map(|p| piloto::chip_width(fonts, &p) + 4.0 * k).sum::<f32>()).fold(0.0, f32::max);
    let text_w = KEYS.iter().map(|(_, text)| fonts.width(text, Style::Body)).fold(0.0, f32::max);
    let column = key_w + 10.0 * k + text_w;
    let pad = 22.0 * k;
    let (w, h) = (column * 2.0 + 36.0 * k + pad * 2.0, rows as f32 * 28.0 * k + pad * 2.0 + 40.0 * k);
    let rect = Rectangle::new(((width - w) * 0.5).round(), ((height - h) * 0.5).round(), w.round(), h.round());
    d.draw_rectangle(0, 0, width as i32, height as i32, Color::new(0, 0, 0, 90));
    piloto::block(d, th, rect);
    d.draw_rectangle_rec(rect, Color::new(0, 0, 0, 70));
    piloto::text(d, fonts, "CONTROLES", rect.x + pad, rect.y + pad - 6.0 * k, Style::Heading, th.ink, Align::Left);
    let hint = "[F1] cerrar";
    keys(d, fonts, th, hint, rect.x + rect.width - pad, rect.y + pad - 4.0 * k, Align::Right, th.text, 0.85);
    for (i, (key, text)) in KEYS.iter().enumerate() {
        let x = rect.x + pad + (i / rows) as f32 * (column + 36.0 * k);
        let y = rect.y + pad + 40.0 * k + (i % rows) as f32 * 28.0 * k;
        let mut at = x;
        for part in parts(key) {
            at += piloto::chip(d, fonts, th, &part, at, y, Cap::Plain, 1.0) + 4.0 * k;
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
    let th = piloto::Theme::of(true);
    let rgba = |c: [u8; 4]| Color::new(c[0], c[1], c[2], c[3]);
    let s = fit.scale;
    let k = ui.k();
    let place = |x: f32, y: f32, w: f32, h: f32| {
        let (x, y) = fit.point(x, y);
        Rectangle::new(x, y, w * s, h * s)
    };
    for item in &overlay.items {
        match item {
            Item::Panel { x, y, w, h } => piloto::frame(d, &th, place(*x, *y, *w, *h), k, false),
            Item::Card { x, y, w, h } => piloto::frame(d, &th, place(*x, *y, *w, *h), k, true),
            Item::Sign { text, x, y, size, paint, align, alpha } => {
                let (x, y) = fit.point(*x, *y);
                ui.sign(d, text, x, y, size * s, *paint, *align, *alpha as f32 / 255.0);
            }
            Item::Keys { text, x, y, align } => {
                let (x, y) = fit.point(*x, *y);
                keys(d, fonts, &th, text, x, y, *align, TEXT, 1.0);
            }
            Item::Gauge { x, y, size, value, max, unit, hot } => {
                let (x, y) = fit.point(*x, *y);
                piloto::gauge(d, ui, &th, x, y, size * s, value / max.max(1e-3), *hot, &format!("{value:.0}"), unit);
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
    let layout = piloto::music_layout(screen, scale);
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
