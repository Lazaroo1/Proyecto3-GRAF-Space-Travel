//! Bloom: piramide de mips con blur gaussiano separable.
//!
//! Portado del Proyecto 2. Se extraen los pixeles mas brillantes que un
//! umbral, se arma una piramide (cada nivel a la mitad del anterior) y se
//! desenfoca cada nivel. Sumar niveles de distinta escala da un halo con un
//! nucleo concentrado y una cola ancha y suave, como la dispersion en una
//! lente real; un solo blur de radio grande costaria muchisimo y se veria plano.
//!
//! # Diferencias con el Proyecto 2
//! - La extraccion del brillo la hace cada tile al terminar de sombrearse,
//!   dentro de la misma pasada, mientras sus pixeles siguen en cache.
//! - El blur de **todos** los niveles va en una sola pasada paralela, por
//!   bandas de filas: cada banda desenfoca en horizontal sus filas mas un
//!   margen y despues en vertical. Pocas pasadas grandes rinden mas que muchas
//!   chicas (ver `parallel`).
//! - Los niveles se combinan de chico a grande dentro de la piramide, asi que
//!   el tonemap lee un solo nivel por pixel en vez de seis.
//!
//! # Por que separable
//! Un gaussiano 2D de radio `r` es el producto de dos 1D: primero horizontal y
//! despues vertical da el mismo resultado con `2(2r+1)` muestras por pixel en
//! vez de `(2r+1)^2`. Con `r = 4`: 18 contra 81.

use std::cell::RefCell;

use super::TILE;
use crate::config;
use crate::math::Vec3;
use crate::parallel::{self, DisjointMut};

/// Pesos de luminancia Rec. 709: el ojo es mas sensible al verde.
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);
/// Lado del bloque del nivel 0 que llena cada tile (el nivel 0 va a la mitad).
const BLOCK: usize = TILE / 2;
/// Filas por trabajo del blur.
const BAND: usize = 16;
/// Por debajo de esto una operacion sobre un nivel se hace en serie.
const PARALLEL_MIN: usize = 16_384;

/// Un nivel de la piramide, guardado por filas.
pub struct MipLevel {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Vec3>,
}

impl MipLevel {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Vec3::ZERO; width * height],
        }
    }

    /// Muestreo bilineal en coordenadas normalizadas `[0, 1]`.
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        let x = u * self.width as f32 - 0.5;
        let y = v * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as isize, y0 as isize);
        let top = self.texel(ix, iy).lerp(self.texel(ix + 1, iy), fx);
        let bottom = self.texel(ix, iy + 1).lerp(self.texel(ix + 1, iy + 1), fx);
        top.lerp(bottom, fy)
    }

    /// Recorte al borde, no wrap: con wrap, el brillo de un borde de la
    /// pantalla apareceria como halo en el borde opuesto.
    #[inline]
    fn texel(&self, x: isize, y: isize) -> Vec3 {
        let xi = x.clamp(0, self.width as isize - 1) as usize;
        let yi = y.clamp(0, self.height as isize - 1) as usize;
        self.pixels[yi * self.width + xi]
    }
}

/// Piramide de bloom. Es duena de sus buffers y se reutiliza entre frames.
pub struct Bloom {
    /// Brillo extraido y sus reducciones.
    levels: Vec<MipLevel>,
    /// Niveles desenfocados; al final, combinados de chico a grande.
    blurred: Vec<MipLevel>,
    kernel: Vec<f32>,
    /// Peso de cada nivel en el compuesto: caida geometrica, normalizada.
    weights: Vec<f32>,
}

impl Bloom {
    pub fn new(width: usize, height: usize) -> Self {
        let mut levels = Vec::new();
        let mut blurred = Vec::new();
        let (mut w, mut h) = ((width / 2).max(1), (height / 2).max(1));
        for _ in 0..config::BLOOM_LEVELS {
            levels.push(MipLevel::new(w, h));
            blurred.push(MipLevel::new(w, h));
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        let raw: Vec<f32> = (0..config::BLOOM_LEVELS)
            .map(|i| config::BLOOM_FALLOFF.powi(i as i32))
            .collect();
        let total: f32 = raw.iter().sum();
        Self {
            levels,
            blurred,
            kernel: gaussian_kernel(config::BLOOM_RADIUS, config::BLOOM_SIGMA),
            weights: raw.iter().map(|w| w / total).collect(),
        }
    }

    /// Nivel 0, donde cada tile deja su brillo.
    pub fn base_mut(&mut self) -> &mut MipLevel {
        &mut self.levels[0]
    }

    /// El halo combinado, a la mitad de la resolucion del frame.
    pub fn result(&self) -> &MipLevel {
        &self.blurred[0]
    }

    /// Reduce, desenfoca y combina. El nivel 0 ya debe tener el brillo.
    pub fn process(&mut self) {
        for i in 1..self.levels.len() {
            let (larger, smaller) = self.levels.split_at_mut(i);
            downsample(&larger[i - 1], &mut smaller[0]);
        }
        blur_all(&self.levels, &mut self.blurred, &self.kernel);
        let last = self.blurred.len() - 1;
        let w_last = self.weights[last];
        for p in &mut self.blurred[last].pixels {
            *p *= w_last;
        }
        for i in (0..last).rev() {
            let (larger, smaller) = self.blurred.split_at_mut(i + 1);
            upsample_add(&smaller[0], &mut larger[i], self.weights[i]);
        }
    }
}

/// Promedia bloques de 2x2 del tile ya sombreado y deja solo lo que pasa el
/// umbral, en el bloque de 32x32 que le toca del nivel 0. El umbral se aplica
/// a la luminancia y se escala el color entero: restarlo canal por canal
/// desaturaria (un naranja se volveria rojo).
pub fn extract_tile(
    tile: usize,
    hdr: &[Vec3],
    base: &DisjointMut<Vec3>,
    base_width: usize,
    base_height: usize,
    tiles_x: usize,
    visible: (usize, usize),
    threshold: f32,
) {
    let bx0 = (tile % tiles_x) * BLOCK;
    let by0 = (tile / tiles_x) * BLOCK;
    if bx0 >= base_width || by0 >= base_height {
        return;
    }
    let cols = BLOCK.min(base_width - bx0);
    let rows = BLOCK.min(base_height - by0);
    let (vis_cols, vis_rows) = visible;
    for ly in 0..rows {
        // SAFETY: cada tile escribe solo las filas de su propio bloque.
        let row = unsafe { base.range((by0 + ly) * base_width + bx0, cols) };
        let (y0, y1) = ((2 * ly).min(vis_rows - 1), (2 * ly + 1).min(vis_rows - 1));
        for (lx, out) in row.iter_mut().enumerate() {
            let (x0, x1) = ((2 * lx).min(vis_cols - 1), (2 * lx + 1).min(vis_cols - 1));
            let average = (hdr[y0 * TILE + x0] + hdr[y0 * TILE + x1] + hdr[y1 * TILE + x0] + hdr[y1 * TILE + x1])
                * 0.25;
            let luma = average.dot(LUMA);
            *out = average * ((luma - threshold).max(0.0) / luma.max(1.0e-4));
        }
    }
}

/// Reduce a la mitad con un filtro de caja de 2x2.
fn downsample(src: &MipLevel, dst: &mut MipLevel) {
    let (src_w, src_h) = (src.width, src.height);
    let src_pixels = &src.pixels;
    let dst_w = dst.width;
    parallel::chunks_mut(&mut dst.pixels, dst_w, PARALLEL_MIN, |y, row| {
        let y0 = (2 * y).min(src_h - 1) * src_w;
        let y1 = (2 * y + 1).min(src_h - 1) * src_w;
        for (x, out) in row.iter_mut().enumerate() {
            let x0 = (2 * x).min(src_w - 1);
            let x1 = (2 * x + 1).min(src_w - 1);
            *out = (src_pixels[y0 + x0] + src_pixels[y0 + x1] + src_pixels[y1 + x0] + src_pixels[y1 + x1])
                * 0.25;
        }
    });
}

thread_local! {
    /// Filas desenfocadas en horizontal de una banda, reutilizadas entre trabajos.
    static SCRATCH: RefCell<Vec<Vec3>> = const { RefCell::new(Vec::new()) };
}

/// Blur gaussiano separable de todos los niveles en una sola pasada.
fn blur_all(src: &[MipLevel], dst: &mut [MipLevel], kernel: &[f32]) {
    // Trabajos: (nivel, primera fila) de cada banda de cada nivel.
    let jobs: Vec<(usize, usize)> = src
        .iter()
        .enumerate()
        .flat_map(|(level, mip)| (0..mip.height).step_by(BAND).map(move |y| (level, y)))
        .collect();
    let outputs: Vec<DisjointMut<Vec3>> =
        dst.iter_mut().map(|mip| DisjointMut::new(&mut mip.pixels)).collect();
    let radius = kernel.len() / 2;
    parallel::for_each(jobs.len(), |job| {
        let (level, y0) = jobs[job];
        let mip = &src[level];
        let (w, h) = (mip.width, mip.height);
        let y1 = (y0 + BAND).min(h);
        // Filas que la banda necesita en horizontal: las suyas mas el margen.
        let first = y0.saturating_sub(radius);
        let last = (y1 + radius).min(h);
        SCRATCH.with_borrow_mut(|scratch| {
            scratch.resize((last - first) * w, Vec3::ZERO);
            for y in first..last {
                let line = &mip.pixels[y * w..(y + 1) * w];
                let out = &mut scratch[(y - first) * w..(y - first + 1) * w];
                blur_row(line, out, kernel);
            }
            // SAFETY: las bandas de un nivel no se solapan.
            let block = unsafe { outputs[level].range(y0 * w, (y1 - y0) * w) };
            block.fill(Vec3::ZERO);
            for y in y0..y1 {
                let out = &mut block[(y - y0) * w..(y - y0 + 1) * w];
                for (k, &weight) in kernel.iter().enumerate() {
                    // Fila de origen recortada al nivel, y luego a la banda.
                    let sy = (y + k).saturating_sub(radius).min(h - 1).clamp(first, last - 1);
                    let line = &scratch[(sy - first) * w..(sy - first + 1) * w];
                    for (o, value) in out.iter_mut().zip(line) {
                        *o += *value * weight;
                    }
                }
            }
        });
    });
}

/// Blur horizontal de una fila. En el interior la ventana del kernel cabe
/// entera y se recorre sin recortes; solo los bordes pagan el recorte.
#[inline]
fn blur_row(line: &[Vec3], out: &mut [Vec3], kernel: &[f32]) {
    let width = line.len();
    let radius = kernel.len() / 2;
    for (x, o) in out.iter_mut().enumerate() {
        let mut acc = Vec3::ZERO;
        if x >= radius && x + radius < width {
            for (value, &weight) in line[x - radius..=x + radius].iter().zip(kernel) {
                acc += *value * weight;
            }
        } else {
            for (k, &weight) in kernel.iter().enumerate() {
                acc += line[(x + k).saturating_sub(radius).min(width - 1)] * weight;
            }
        }
        *o = acc;
    }
}

/// `dst = dst * weight + src ampliado`, con muestreo bilineal.
fn upsample_add(src: &MipLevel, dst: &mut MipLevel, weight: f32) {
    let (w, h) = (dst.width, dst.height);
    parallel::chunks_mut(&mut dst.pixels, w, PARALLEL_MIN, |y, row| {
        let v = (y as f32 + 0.5) / h as f32;
        for (x, out) in row.iter_mut().enumerate() {
            let u = (x as f32 + 0.5) / w as f32;
            *out = *out * weight + src.sample(u, v);
        }
    });
}

/// Pesos de un gaussiano 1D discreto que suman exactamente 1: si no, el blur
/// cambiaria el brillo medio ademas de desenfocar.
fn gaussian_kernel(radius: usize, sigma: f32) -> Vec<f32> {
    let denom = 2.0 * sigma * sigma;
    let weights: Vec<f32> = (0..=2 * radius)
        .map(|i| {
            let x = i as f32 - radius as f32;
            (-x * x / denom).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    weights.iter().map(|w| w / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// El blur por bandas debe dar lo mismo que un blur directo, tambien en
    /// los bordes de cada banda y del nivel.
    #[test]
    fn banded_blur_matches_direct_blur() {
        let (w, h) = (37, 29);
        let src = MipLevel {
            width: w,
            height: h,
            pixels: (0..w * h).map(|i| Vec3::splat(((i * 7919) % 101) as f32)).collect(),
        };
        let kernel = gaussian_kernel(4, 2.0);
        let mut banded = vec![MipLevel::new(w, h)];
        blur_all(std::slice::from_ref(&src), &mut banded, &kernel);

        let r = kernel.len() / 2;
        let mut horizontal = vec![Vec3::ZERO; w * h];
        for y in 0..h {
            blur_row(&src.pixels[y * w..(y + 1) * w], &mut horizontal[y * w..(y + 1) * w], &kernel);
        }
        for y in 0..h {
            for x in 0..w {
                let mut acc = Vec3::ZERO;
                for (k, &weight) in kernel.iter().enumerate() {
                    let sy = (y + k).saturating_sub(r).min(h - 1);
                    acc += horizontal[sy * w + x] * weight;
                }
                assert!((acc - banded[0].pixels[y * w + x]).length() < 1e-3, "({x}, {y})");
            }
        }
    }

    /// Tiempo del bloom a 1280x720. Correr con
    /// `cargo test --release -- --ignored --nocapture medir_`.
    #[test]
    #[ignore]
    fn medir_bloom() {
        let mut bloom = Bloom::new(1280, 720);
        for (i, p) in bloom.base_mut().pixels.iter_mut().enumerate() {
            *p = Vec3::splat(((i * 7919) % 1000) as f32 / 300.0);
        }
        for _ in 0..20 {
            bloom.process();
        }
        let rounds = 200;
        let start = Instant::now();
        for _ in 0..rounds {
            bloom.process();
        }
        println!("bloom: {:.3} ms", start.elapsed().as_secs_f64() * 1e3 / rounds as f64);
    }
}
