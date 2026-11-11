//! Letras en CPU: de una palabra, que tan lejos esta cada pixel del borde de
//! sus letras.
//!
//! raylib solo hace lo que ya hace para el texto del HUD: leer el archivo de
//! la letra y dar la cobertura de cada glifo (`LoadFontData`, sin ventana ni
//! GPU). Lo demas es propio: se arma la palabra con el avance de cada glifo,
//! se le saca la **distancia con signo** al borde con la transformada exacta
//! de Felzenszwalb y Huttenlocher (dos pasadas de parabolas, lineal en
//! pixeles) y con esa distancia `rotulos.rs` forja las letras: el filo, el
//! bisel, las grietas y el resplandor salen todos de `d`.
//!
//! La palabra se rasteriza al doble del tamano pedido y la distancia se baja
//! promediando 2 x 2: asi el borde queda con precision de medio pixel.

use std::path::Path;

use raylib::ffi;

/// Donde estan las letras de Windows.
pub const FONT_DIR: &str = "C:/Windows/Fonts";

/// Los bytes de la primera letra de `files` que este instalada.
pub fn font_bytes(files: &[&str]) -> Option<Vec<u8>> {
    files.iter().find_map(|file| std::fs::read(Path::new(FONT_DIR).join(file)).ok())
}

/// Cuantas veces mas grande se rasteriza la palabra antes de bajarla.
const SUPER: usize = 2;

/// La cobertura de un glifo y donde va.
struct Glyph {
    w: usize,
    h: usize,
    cover: Vec<u8>,
    /// Desde la pluma (x) y desde arriba de la linea (y).
    left: i32,
    top: i32,
    advance: i32,
}

/// Los glifos de `chars` a `size` pixeles de alto, en el mismo orden.
/// `None` si la letra no trae alguno.
fn glyphs(font: &[u8], chars: &[char], size: usize) -> Option<Vec<Glyph>> {
    let codepoints: Vec<i32> = chars.iter().map(|&c| c as i32).collect();
    let mut count = 0;
    // SAFETY: `font` y `codepoints` viven durante la llamada; raylib devuelve
    // `count` glifos (los que encontro), cada uno con su imagen de un canal,
    // y se sueltan con `UnloadFontData`.
    unsafe {
        let data = ffi::LoadFontData(font.as_ptr(), font.len() as i32, size as i32, codepoints.as_ptr(), codepoints.len() as i32, 0, &mut count);
        if data.is_null() {
            return None;
        }
        let loaded = std::slice::from_raw_parts(data, count as usize);
        let glyphs: Option<Vec<Glyph>> = codepoints
            .iter()
            .map(|&cp| {
                let g = loaded.iter().find(|g| g.value == cp)?;
                let (w, h) = (g.image.width.max(0) as usize, g.image.height.max(0) as usize);
                let cover = if g.image.data.is_null() || cp == 32 { vec![0; w * h] } else { std::slice::from_raw_parts(g.image.data as *const u8, w * h).to_vec() };
                Some(Glyph { w, h, cover, left: g.offsetX, top: g.offsetY, advance: g.advanceX })
            })
            .collect();
        ffi::UnloadFontData(data, count);
        glyphs
    }
}

/// Los glifos de `text`. Si hay una letra para las cifras (`digits`), los
/// numeros salen de ella: se carga al tamano con que su H mide lo mismo que
/// la H de `font` y se asienta en el mismo renglon (la gorda de los rotulos
/// trae cifras de estilo antiguo, con el 0 del alto de una minuscula, y un
/// marcador se ve disparejo).
fn mixed(font: &[u8], digits: Option<&[u8]>, text: &str, size: usize) -> Option<Vec<Glyph>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = glyphs(font, &chars, size)?;
    let Some(digits) = digits.filter(|_| chars.iter().any(char::is_ascii_digit)) else { return Some(out) };
    let (Some(ours), Some(theirs)) = (glyphs(font, &['H'], size), glyphs(digits, &['H'], size)) else { return Some(out) };
    if theirs[0].h == 0 {
        return Some(out);
    }
    let fitted = (size as f32 * ours[0].h as f32 / theirs[0].h as f32).round() as usize;
    let wanted: Vec<char> = chars.iter().copied().filter(char::is_ascii_digit).chain(['H']).collect();
    let Some(mut figures) = glyphs(digits, &wanted, fitted) else { return Some(out) };
    let reference = figures.pop()?;
    // Cuanto hay que bajarlas para que se asienten donde la H de `font`.
    let drop = ours[0].top + ours[0].h as i32 - (reference.top + reference.h as i32);
    let mut figures = figures.into_iter();
    for (slot, _) in out.iter_mut().zip(&chars).filter(|(_, c)| c.is_ascii_digit()) {
        let mut figure = figures.next()?;
        figure.top += drop;
        *slot = figure;
    }
    Some(out)
}

/// Una palabra como campo de distancias.
pub struct Word {
    pub w: usize,
    pub h: usize,
    /// Distancia con signo al borde de las letras, en pixeles: negativa
    /// adentro.
    pub dist: Vec<f32>,
    /// De donde a donde va cada letra en x.
    pub spans: Vec<(f32, f32)>,
    /// Las filas de arriba y de abajo de las letras (sin el margen).
    pub top: f32,
    pub bottom: f32,
}

impl Word {
    /// `text` con letras de `size` pixeles de alto (el tamano de la letra,
    /// no el de las mayusculas), `tracking` pixeles de mas entre letras y
    /// `margin` pixeles libres alrededor (para el resplandor y la sombra).
    pub fn new(font: &[u8], text: &str, size: f32, tracking: f32, margin: f32) -> Option<Self> {
        Self::with_digits(font, None, text, size, tracking, margin)
    }

    /// Como `new`, con las cifras de otra letra (`digits`), a la medida y
    /// en el renglon de `font`.
    pub fn with_digits(font: &[u8], digits: Option<&[u8]>, text: &str, size: f32, tracking: f32, margin: f32) -> Option<Self> {
        let big = (size * SUPER as f32).round() as usize;
        let glyphs = mixed(font, digits, text, big)?;
        let track = (tracking * SUPER as f32).round() as i32;
        let pad = (margin * SUPER as f32).ceil() as i32;
        // La caja de la tinta.
        let (mut pen, mut x0, mut x1, mut y0, mut y1) = (0, i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        let mut places = Vec::new();
        for g in &glyphs {
            if g.w > 0 && g.h > 0 && g.cover.iter().any(|&c| c > 0) {
                x0 = x0.min(pen + g.left);
                x1 = x1.max(pen + g.left + g.w as i32);
                y0 = y0.min(g.top);
                y1 = y1.max(g.top + g.h as i32);
            }
            places.push(pen);
            pen += g.advance + track;
        }
        if x0 > x1 {
            return None;
        }
        // Multiplo de SUPER para bajarla sin sobras.
        let round = |n: i32| (n as usize).div_ceil(SUPER) * SUPER;
        let (bw, bh) = (round(x1 - x0 + 2 * pad), round(y1 - y0 + 2 * pad));
        let mut cover = vec![0u8; bw * bh];
        let mut spans = Vec::new();
        for (g, &at) in glyphs.iter().zip(&places) {
            let (gx, gy) = (at + g.left - x0 + pad, g.top - y0 + pad);
            for y in 0..g.h {
                for x in 0..g.w {
                    let (px, py) = (gx + x as i32, gy + y as i32);
                    if px >= 0 && py >= 0 && (px as usize) < bw && (py as usize) < bh {
                        let c = &mut cover[py as usize * bw + px as usize];
                        *c = (*c).max(g.cover[y * g.w + x]);
                    }
                }
            }
            spans.push((gx as f32 / SUPER as f32, (gx + g.w as i32) as f32 / SUPER as f32));
        }
        let big_dist = signed_distance(&cover, bw, bh);
        // A la mitad: el promedio de cada 2 x 2, en pixeles finales.
        let (w, h) = (bw / SUPER, bh / SUPER);
        let mut dist = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0;
                for sy in 0..SUPER {
                    for sx in 0..SUPER {
                        sum += big_dist[(y * SUPER + sy) * bw + x * SUPER + sx];
                    }
                }
                dist[y * w + x] = sum / (SUPER * SUPER * SUPER) as f32;
            }
        }
        Some(Self { w, h, dist, spans, top: pad as f32 / SUPER as f32, bottom: (pad + y1 - y0) as f32 / SUPER as f32 })
    }

    /// La distancia en un punto cualquiera (bilineal; afuera de la imagen,
    /// la del borde mas lo que falta).
    #[inline]
    pub fn at(&self, x: f32, y: f32) -> f32 {
        let (cx, cy) = (x.clamp(0.0, self.w as f32 - 1.001), y.clamp(0.0, self.h as f32 - 1.001));
        let (x0, y0) = (cx as usize, cy as usize);
        let (fx, fy) = (cx - x0 as f32, cy - y0 as f32);
        let i = y0 * self.w + x0;
        let top = self.dist[i] + (self.dist[i + 1] - self.dist[i]) * fx;
        let bottom = self.dist[i + self.w] + (self.dist[i + self.w + 1] - self.dist[i + self.w]) * fx;
        top + (bottom - top) * fy + (x - cx).abs() + (y - cy).abs()
    }

    /// Hacia donde crece la distancia (hacia afuera de la letra), de largo 1.
    #[inline]
    pub fn normal(&self, x: f32, y: f32) -> (f32, f32) {
        let gx = self.at(x + 1.0, y) - self.at(x - 1.0, y);
        let gy = self.at(x, y + 1.0) - self.at(x, y - 1.0);
        let len = gx.hypot(gy).max(1e-4);
        (gx / len, gy / len)
    }
}

/// La distancia con signo (en pixeles de `cover`, negativa adentro) al borde
/// de lo que esta cubierto mas de la mitad. Cerca del borde se corrige con
/// la cobertura, que dice por donde pasa el borde dentro del pixel.
fn signed_distance(cover: &[u8], w: usize, h: usize) -> Vec<f32> {
    let inside: Vec<bool> = cover.iter().map(|&c| c >= 128).collect();
    let to_ink = distance_to(&inside, w, h, true);
    let to_paper = distance_to(&inside, w, h, false);
    (0..w * h)
        .map(|i| {
            let d = if inside[i] { 0.5 - to_paper[i] } else { to_ink[i] - 0.5 };
            if d.abs() < 1.0 { 0.5 - cover[i] as f32 / 255.0 + d * 0.3 } else { d }
        })
        .collect()
}

/// La distancia de cada pixel al mas cercano que vale `target`.
fn distance_to(mask: &[bool], w: usize, h: usize, target: bool) -> Vec<f32> {
    const FAR: f32 = 1e12;
    let n = w.max(h);
    let mut grid: Vec<f32> = mask.iter().map(|&m| if m == target { 0.0 } else { FAR }).collect();
    let (mut f, mut d) = (vec![0.0f32; n], vec![0.0f32; n]);
    let (mut v, mut z) = (vec![0usize; n], vec![0.0f32; n + 1]);
    // Por columnas y despues por filas: la distancia al cuadrado se separa.
    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        parabolas(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..(y + 1) * w]);
        parabolas(&f[..w], &mut d[..w], &mut v, &mut z);
        grid[y * w..(y + 1) * w].copy_from_slice(&d[..w]);
    }
    grid.iter().map(|&sq| sq.sqrt()).collect()
}

/// En una dimension: `d[q] = min_p ((q - p)^2 + f[p])`, la envolvente de
/// abajo de las parabolas con vertice en cada `p` (Felzenszwalb y
/// Huttenlocher, 2012). `v` guarda los vertices que quedan en la envolvente
/// y `z` desde donde manda cada uno.
fn parabolas(f: &[f32], d: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    let mut k = 0;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        loop {
            let p = v[k];
            // Donde se cruzan la parabola de `q` y la de `p`.
            let s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q as f32 - p as f32));
            if s <= z[k] && k > 0 {
                k -= 1;
            } else {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = f32::INFINITY;
                break;
            }
        }
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f32 - p as f32).powi(2) + f[p];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La transformada da la distancia exacta: contra la busqueda a lo bruto
    /// en una mascara cualquiera.
    #[test]
    fn the_distance_transform_matches_brute_force() {
        let (w, h) = (37, 23);
        let mask: Vec<bool> = (0..w * h).map(|i| crate::math::hash_u32(i as u32 * 7 + 3) % 11 == 0).collect();
        let fast = distance_to(&mask, w, h, true);
        for y in 0..h {
            for x in 0..w {
                let mut best = f32::MAX;
                for (i, _) in mask.iter().enumerate().filter(|&(_, &m)| m) {
                    let (px, py) = ((i % w) as f32, (i / w) as f32);
                    best = best.min((px - x as f32).hypot(py - y as f32));
                }
                assert!((fast[y * w + x] - best).abs() < 1e-3, "en ({x}, {y}): {} y no {best}", fast[y * w + x]);
            }
        }
    }

    /// Un disco: la distancia con signo es el radio menos lo que falta.
    #[test]
    fn a_disc_has_its_radius_as_signed_distance() {
        let (w, h, r) = (64usize, 64usize, 20.0f32);
        let cover: Vec<u8> = (0..w * h)
            .map(|i| {
                let d = ((i % w) as f32 + 0.5 - 32.0).hypot((i / w) as f32 + 0.5 - 32.0);
                ((r - d + 0.5).clamp(0.0, 1.0) * 255.0) as u8
            })
            .collect();
        let dist = signed_distance(&cover, w, h);
        for (x, y) in [(32usize, 32usize), (40, 32), (55, 32), (32, 8), (50, 50)] {
            let truth = (x as f32 + 0.5 - 32.0).hypot(y as f32 + 0.5 - 32.0) - r;
            assert!((dist[y * w + x] - truth).abs() < 0.75, "en ({x}, {y}): {} y no {truth}", dist[y * w + x]);
        }
    }
}
