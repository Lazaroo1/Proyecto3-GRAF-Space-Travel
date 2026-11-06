//! Escalado de la imagen para la resolucion dinamica: mientras la nave se
//! mueve, el frame se renderiza mas chico y aqui se amplia al tamano de la
//! ventana.
//!
//! **Catmull-Rom** (bicubico): cada pixel de salida mezcla 4 x 4 de la
//! entrada con los pesos de la spline que pasa por las muestras,
//! `w0 = (-t^3 + 2t^2 - t) / 2`, `w1 = (3t^3 - 5t^2 + 2) / 2`,
//! `w2 = (-3t^3 + 4t^2 + t) / 2`, `w3 = (t^3 - t^2) / 2`, con `t` la parte
//! fraccionaria de la posicion en la entrada. Comparado con el bilineal
//! conserva mejor los bordes (los pesos negativos le dan un poco de
//! enfoque). Es separable: primero cada fila a lo ancho y despues cada
//! columna a lo alto, ocho lecturas por pixel en vez de dieciseis. Los
//! indices y pesos de cada columna y fila se calculan una vez por tamano.

use crate::parallel;

/// Las cuatro muestras y sus pesos para una posicion de la salida.
#[derive(Clone, Copy, Debug)]
struct Taps {
    index: [usize; 4],
    weight: [f32; 4],
}

fn taps(src: usize, dst: usize) -> Vec<Taps> {
    let ratio = src as f32 / dst as f32;
    (0..dst)
        .map(|i| {
            let x = (i as f32 + 0.5) * ratio - 0.5;
            let base = x.floor();
            let t = x - base;
            let (t2, t3) = (t * t, t * t * t);
            let weight = [(-t3 + 2.0 * t2 - t) * 0.5, (3.0 * t3 - 5.0 * t2 + 2.0) * 0.5, (-3.0 * t3 + 4.0 * t2 + t) * 0.5, (t3 - t2) * 0.5];
            let index = std::array::from_fn(|k| (base as i64 - 1 + k as i64).clamp(0, src as i64 - 1) as usize);
            Taps { index, weight }
        })
        .collect()
}

pub struct Upscaler {
    src: (usize, usize),
    dst: (usize, usize),
    columns: Vec<Taps>,
    rows: Vec<Taps>,
    /// La imagen ampliada a lo ancho, `dst.0 x src.1`, tres canales.
    wide: Vec<[f32; 3]>,
}

impl Upscaler {
    pub fn new(src: (usize, usize), dst: (usize, usize)) -> Self {
        Self { src, dst, columns: taps(src.0, dst.0), rows: taps(src.1, dst.1), wide: vec![[0.0; 3]; dst.0 * src.1] }
    }

    /// Si sirve para estos tamanos.
    pub fn fits(&self, src: (usize, usize), dst: (usize, usize)) -> bool {
        self.src == src && self.dst == dst
    }

    /// Amplia `input` (RGBA8 de `src`) a `output` (RGBA8 de `dst`, opaco).
    pub fn run(&mut self, input: &[u8], output: &mut [u8]) {
        let ((sw, sh), (dw, dh)) = (self.src, self.dst);
        if input.len() < sw * sh * 4 || output.len() < dw * dh * 4 {
            return;
        }
        let columns = &self.columns;
        parallel::chunks_mut(&mut self.wide, dw, 8 * dw, |y, row| {
            let line = &input[y * sw * 4..(y + 1) * sw * 4];
            for (out, tap) in row.iter_mut().zip(columns) {
                let mut c = [0.0f32; 3];
                for k in 0..4 {
                    let p = &line[tap.index[k] * 4..tap.index[k] * 4 + 3];
                    let w = tap.weight[k];
                    c[0] += p[0] as f32 * w;
                    c[1] += p[1] as f32 * w;
                    c[2] += p[2] as f32 * w;
                }
                *out = c;
            }
        });
        let (wide, rows) = (&self.wide, &self.rows);
        parallel::chunks_mut(&mut output[..dw * dh * 4], dw * 4, 8 * dw * 4, |y, row| {
            let tap = rows[y];
            let lines: [&[[f32; 3]]; 4] = std::array::from_fn(|k| &wide[tap.index[k] * dw..(tap.index[k] + 1) * dw]);
            for x in 0..dw {
                let mut c = [0.0f32; 3];
                for k in 0..4 {
                    let p = lines[k][x];
                    let w = tap.weight[k];
                    c[0] += p[0] * w;
                    c[1] += p[1] * w;
                    c[2] += p[2] * w;
                }
                let px = &mut row[x * 4..x * 4 + 4];
                px[0] = (c[0] + 0.5).clamp(0.0, 255.0) as u8;
                px[1] = (c[1] + 0.5).clamp(0.0, 255.0) as u8;
                px[2] = (c[2] + 0.5).clamp(0.0, 255.0) as u8;
                px[3] = 255;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Los pesos suman uno (una imagen pareja queda pareja) y al mismo
    /// tamano la imagen no cambia.
    #[test]
    fn catmull_rom_keeps_flat_images_and_identity() {
        for t in taps(30, 40) {
            assert!((t.weight.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        }
        let (w, h) = (24, 16);
        let flat = vec![[90u8, 140, 200, 255]; w * h].concat();
        let mut out = vec![0u8; 32 * 21 * 4];
        Upscaler::new((w, h), (32, 21)).run(&flat, &mut out);
        assert!(out.chunks_exact(4).all(|p| p == [90, 140, 200, 255]));
        let ramp: Vec<u8> = (0..w * h).flat_map(|i| [(i % 251) as u8, (i * 7 % 253) as u8, (i * 13 % 255) as u8, 255]).collect();
        let mut same = vec![0u8; w * h * 4];
        Upscaler::new((w, h), (w, h)).run(&ramp, &mut same);
        assert_eq!(same, ramp);
    }
}
