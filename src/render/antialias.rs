//! FXAA y afilado en CPU sobre el framebuffer final, en una sola pasada.
//!
//! - **FXAA** donde hay un borde (el contraste de las cuatro diagonales pasa
//!   de un umbral): muestreo bilineal a lo largo de la direccion del borde,
//!   como siempre.
//! - **Afilado adaptativo** (la idea del CAS de AMD) en lo demas: cada canal
//!   se mezcla con su cruz de vecinos con un peso negativo
//!   `w = -sqrt(min(m, 1 - M) / M) / lerp(8, 5, nitidez)` (`m` y `M` el
//!   minimo y el maximo de la luma en la cruz: un solo peso para los tres
//!   canales), `(c + w (n + s + e + o)) / (1 + 4 w)`:
//!   devuelve el detalle que suaviza el render (el filtrado de las texturas,
//!   el FXAA, la resolucion dinamica) y se afloja solo donde ya hay mucho
//!   contraste, asi no saca halos ni realza los bordes que el FXAA suavizo.
//!
//! La luma de cada fila se calcula una vez (antes cada pixel la sacaba de
//! sus cuatro vecinos: cinco veces por pixel) en un buffer de cada hilo.

use std::cell::RefCell;

use crate::math::Vec3;

thread_local! {
    static LUMA: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
}

#[inline(always)]
fn luma_of(p: &[u8]) -> f32 {
    p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114
}

/// Las filas desde `first` que caben en `out`, leyendo la imagen entera
/// `src`. `sharpen` va de 0 (sin afilar) a 1.
pub fn rows(first: usize, out: &mut [u8], src: &[u8], width: usize, height: usize, sharpen: f32) {
    let rows = out.len() / (width * 4);
    let at = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        Vec3::new(src[i] as f32, src[i + 1] as f32, src[i + 2] as f32)
    };
    let bilinear = |x: f32, y: f32| {
        let x = x.clamp(0.0, (width - 1) as f32);
        let y = y.clamp(0.0, (height - 1) as f32);
        let (ix, iy) = (x as usize, y as usize);
        at(ix, iy)
            .lerp(at((ix + 1).min(width - 1), iy), x - ix as f32)
            .lerp(
                at(ix, (iy + 1).min(height - 1)).lerp(
                    at((ix + 1).min(width - 1), (iy + 1).min(height - 1)),
                    x - ix as f32,
                ),
                y - iy as f32,
            )
    };
    let luma = |v: Vec3| v.x * 0.299 + v.y * 0.587 + v.z * 0.114;
    // El peso del afilado sin el factor adaptativo.
    let peak = if sharpen > 0.0 { -1.0 / (8.0 + (5.0 - 8.0) * sharpen.clamp(0.0, 1.0)) } else { 0.0 };
    LUMA.with(|cell| {
        let mut lumas = cell.borrow_mut();
        // La luma de las filas `first - 1` a `first + rows` (recortadas).
        lumas.clear();
        lumas.reserve((rows + 2) * width);
        for r in 0..rows + 2 {
            let y = (first + r).saturating_sub(1).min(height - 1);
            let line = &src[y * width * 4..(y + 1) * width * 4];
            lumas.extend(line.chunks_exact(4).map(luma_of));
        }
        let lumas = &lumas[..];
        for ly in 0..rows {
            let y = first + ly;
            let (above, here, below) = (&lumas[ly * width..(ly + 1) * width], &lumas[(ly + 1) * width..(ly + 2) * width], &lumas[(ly + 2) * width..(ly + 3) * width]);
            let out_row = &mut out[ly * width * 4..(ly + 1) * width * 4];
            let src_row = &src[y * width * 4..(y + 1) * width * 4];
            for x in 0..width {
                let px = &mut out_row[x * 4..x * 4 + 4];
                let m = &src_row[x * 4..x * 4 + 4];
                if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
                    px.copy_from_slice(&[m[0], m[1], m[2], 255]);
                    continue;
                }
                let (nw, ne, sw, se) = (above[x - 1], above[x + 1], below[x - 1], below[x + 1]);
                let lm = here[x];
                let low = lm.min(nw).min(ne).min(sw).min(se);
                let high = lm.max(nw).max(ne).max(sw).max(se);
                if high - low >= 10.0f32.max(high * 0.12) {
                    // Un borde: FXAA.
                    let mut dx = -((nw + ne) - (sw + se));
                    let mut dy = (nw + sw) - (ne + se);
                    let reduce = ((nw + ne + sw + se) * 0.03125).max(0.5);
                    let inv = 1.0 / (dx.abs().min(dy.abs()) + reduce);
                    dx = (dx * inv).clamp(-6.0, 6.0);
                    dy = (dy * inv).clamp(-6.0, 6.0);
                    let sample = |t: f32| bilinear(x as f32 + dx * t, y as f32 + dy * t);
                    let a = (sample(-1.0 / 6.0) + sample(1.0 / 6.0)) * 0.5;
                    let b = a * 0.5 + (sample(-0.5) + sample(0.5)) * 0.25;
                    let c = if luma(b) < low || luma(b) > high { a } else { b };
                    px.copy_from_slice(&[c.x.round() as u8, c.y.round() as u8, c.z.round() as u8, 255]);
                    continue;
                }
                if peak == 0.0 {
                    px.copy_from_slice(&[m[0], m[1], m[2], 255]);
                    continue;
                }
                // Lo demas: afilado adaptativo con la cruz, con un peso para
                // los tres canales sacado de la luma.
                let (n, s, w, e) = (above[x], below[x], here[x - 1], here[x + 1]);
                let mn = lm.min(n).min(s).min(w).min(e);
                let mx = lm.max(n).max(s).max(w).max(e);
                if !(mx > 0.5) || mx - mn < 0.5 {
                    px.copy_from_slice(&[m[0], m[1], m[2], 255]);
                    continue;
                }
                let wt = ((mn.min(255.0 - mx)) / mx).clamp(0.0, 1.0).sqrt() * peak;
                let inv = 1.0 / (1.0 + 4.0 * wt);
                let up = &src[((y - 1) * width + x) * 4..];
                let down = &src[((y + 1) * width + x) * 4..];
                let left = &src_row[(x - 1) * 4..];
                let right = &src_row[(x + 1) * 4..];
                for c in 0..3 {
                    let v = (m[c] as f32 + wt * (up[c] as f32 + down[c] as f32 + left[c] as f32 + right[c] as f32)) * inv;
                    px[c] = (v.clamp(0.0, 255.0) + 0.5) as u8;
                }
                px[3] = 255;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo parejo queda igual, un escalon recto se suaviza y un detalle chico
    /// sin bordes se afila (mas contraste con sus vecinos).
    #[test]
    fn flat_stays_edges_soften_and_detail_sharpens() {
        let (w, h) = (16, 16);
        let mut src = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = if y < 8 { 40 } else { 200 };
                src[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        // Un detalle tenue en la parte oscura, lejos del borde.
        src[(3 * w + 5) * 4..(3 * w + 5) * 4 + 3].copy_from_slice(&[46, 46, 46]);
        let mut out = vec![0u8; w * h * 4];
        rows(0, &mut out, &src, w, h, 0.5);
        // Parejo: igual.
        assert_eq!(out[(1 * w + 10) * 4], 40);
        // El detalle gana contraste.
        assert!(out[(3 * w + 5) * 4] > 46, "{}", out[(3 * w + 5) * 4]);
        // Sin afilar, el detalle queda como estaba.
        rows(0, &mut out, &src, w, h, 0.0);
        assert_eq!(out[(3 * w + 5) * 4], 46);
    }
}
