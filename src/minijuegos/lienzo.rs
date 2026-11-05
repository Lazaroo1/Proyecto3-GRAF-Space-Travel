//! Pintar en 2D sobre la imagen RGBA8, en CPU: lo que comparten los
//! minijuegos.
//!
//! - **Formas con distancia con signo**: cada figura es una funcion que da
//!   la distancia en pixeles a su borde (negativa adentro). La cobertura de
//!   un pixel sale de esa distancia (`cover`), asi los bordes quedan
//!   suavizados un pixel sin muestrear de mas.
//! - **El fondo**: al entrar a un minijuego la ultima imagen del render se
//!   congela. Se difumina con tres pasadas de caja (que juntas se parecen a
//!   una gaussiana, por el teorema del limite central), se oscurece y se le
//!   pone una vineta, para que lo del juego se lea encima.

use crate::math::{Vec3, smoothstep};
use crate::parallel;

/// El color de un pixel RGBA8, de 0 a 1.
#[inline]
pub fn read(p: &[u8]) -> Vec3 {
    Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) * (1.0 / 255.0)
}

/// Escribe un color de 0 a 1 en un pixel RGBA8 (opaco).
#[inline]
pub fn write(p: &mut [u8], c: Vec3) {
    p[0] = (c.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    p[1] = (c.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    p[2] = (c.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    p[3] = 255;
}

/// Cuanto cubre una figura a un pixel cuyo centro esta a `d` pixeles de su
/// borde (negativo adentro).
#[inline]
pub fn cover(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Distancia con signo a una caja redondeada centrada en el origen, con
/// medio ancho `hx`, medio alto `hy` y esquinas de radio `r`.
#[inline]
pub fn round_box(x: f32, y: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let r = r.min(hx).min(hy).max(0.0);
    let qx = x.abs() - hx + r;
    let qy = y.abs() - hy + r;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - r
}

/// Distancia con signo a un rombo centrado en el origen con medias
/// diagonales `hx` y `hy` (aproximada, exacta en el borde).
#[inline]
pub fn diamond(x: f32, y: f32, hx: f32, hy: f32) -> f32 {
    let (x, y) = (x.abs(), y.abs());
    (x * hy + y * hx - hx * hy) / hx.hypot(hy)
}

/// Mezcla `src` encima de `dst` con opacidad `a`.
#[inline]
pub fn over(dst: Vec3, src: Vec3, a: f32) -> Vec3 {
    dst + (src - dst) * a.clamp(0.0, 1.0)
}

/// Un brillo que se suma: `exp(-d / spread)` afuera de una figura a `d`
/// pixeles (1 adentro).
#[inline]
pub fn halo(d: f32, spread: f32) -> f32 {
    if d <= 0.0 { 1.0 } else { (-d / spread).exp() }
}

/// Prepara la imagen congelada del mundo como fondo: difuminada (radio
/// `blur` pixeles), oscurecida a `dim`, con vineta y un poco desaturada.
pub fn backdrop(src: &[u8], width: usize, height: usize, blur: usize, dim: f32) -> Vec<u8> {
    let mut color: Vec<Vec3> = src.chunks_exact(4).map(read).collect();
    if color.len() != width * height {
        return vec![0; width * height * 4];
    }
    for _ in 0..3 {
        blur_rows(&mut color, width, blur);
        blur_columns(&mut color, width, height, blur);
    }
    let mut out = vec![0u8; width * height * 4];
    let (cx, cy) = (width as f32 * 0.5, height as f32 * 0.5);
    let half = cx.hypot(cy);
    parallel::chunks_mut(&mut out, width * 4, 1, |y, row| {
        for x in 0..width {
            let c = color[y * width + x];
            let r = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) / half;
            let vignette = 1.0 - 0.55 * smoothstep(0.35, 1.05, r);
            let grey = Vec3::splat(c.luminance());
            write(&mut row[x * 4..x * 4 + 4], c.lerp(grey, 0.25) * (dim * vignette));
        }
    });
    out
}

/// Promedio corrido de `2 * radius + 1` pixeles a lo largo de cada fila
/// (el borde se repite).
fn blur_rows(color: &mut [Vec3], width: usize, radius: usize) {
    if radius == 0 || width == 0 {
        return;
    }
    let scale = 1.0 / (2 * radius + 1) as f32;
    parallel::chunks_mut(color, width, 1, |_, row| {
        let source = row.to_vec();
        let at = |i: isize| source[i.clamp(0, width as isize - 1) as usize];
        let r = radius as isize;
        let mut sum = Vec3::ZERO;
        for i in -r..=r {
            sum = sum + at(i);
        }
        for x in 0..width as isize {
            row[x as usize] = sum * scale;
            sum = sum + at(x + r + 1) - at(x - r);
        }
    });
}

/// Lo mismo a lo largo de cada columna: se recorre por filas con una suma
/// por columna, que lee la memoria en orden.
fn blur_columns(color: &mut [Vec3], width: usize, height: usize, radius: usize) {
    if radius == 0 || height == 0 {
        return;
    }
    let scale = 1.0 / (2 * radius + 1) as f32;
    let source = color.to_vec();
    let row = |y: isize| {
        let y = y.clamp(0, height as isize - 1) as usize;
        &source[y * width..(y + 1) * width]
    };
    let r = radius as isize;
    let mut sum = vec![Vec3::ZERO; width];
    for y in -r..=r {
        for (s, c) in sum.iter_mut().zip(row(y)) {
            *s = *s + *c;
        }
    }
    for y in 0..height as isize {
        let out = &mut color[y as usize * width..(y as usize + 1) * width];
        let (enter, leave) = (row(y + r + 1), row(y - r));
        for x in 0..width {
            out[x] = sum[x] * scale;
            sum[x] = sum[x] + enter[x] - leave[x];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_box_is_negative_inside_and_measures_pixels_outside() {
        assert!(round_box(0.0, 0.0, 10.0, 5.0, 2.0) < 0.0);
        assert!((round_box(13.0, 0.0, 10.0, 5.0, 2.0) - 3.0).abs() < 1e-5);
        assert!((round_box(0.0, -9.0, 10.0, 5.0, 2.0) - 4.0).abs() < 1e-5);
        // En la esquina redondeada se mide hasta el arco.
        let corner = round_box(10.0, 5.0, 10.0, 5.0, 2.0);
        assert!((corner - (2.0f32.hypot(2.0) - 2.0)).abs() < 1e-4);
        assert!((diamond(4.0, 0.0, 4.0, 2.0)).abs() < 1e-5);
        assert!(diamond(0.0, 0.0, 4.0, 2.0) < 0.0);
    }

    #[test]
    fn the_backdrop_keeps_a_flat_image_flat_and_darker() {
        let (w, h) = (40, 30);
        let src: Vec<u8> = (0..w * h).flat_map(|_| [200u8, 100, 50, 255]).collect();
        let out = backdrop(&src, w, h, 4, 0.5);
        // Al centro (sin vineta) queda la mitad, un poco hacia el gris.
        let c = read(&out[(15 * w + 20) * 4..]);
        assert!(c.x < 0.45 && c.x > 0.3, "{c:?}");
        assert!(c.z > 0.08, "{c:?}");
        // Las esquinas, mas oscuras.
        let corner = read(&out[0..4]);
        assert!(corner.x < c.x);
        assert!(out.chunks_exact(4).all(|p| p[3] == 255));
    }
}
