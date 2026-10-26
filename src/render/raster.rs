//! Raster por tiles al visibility buffer.
//!
//! # Funciones de arista en punto fijo
//! Un pixel esta dentro del triangulo si queda del lado interior de sus tres
//! aristas. Para la arista de `a` a `b`:
//!
//! ```text
//! E(p) = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
//! ```
//!
//! Con los vertices en enteros (1/256 de pixel) el resultado es **exacto**:
//! dos triangulos que comparten una arista calculan exactamente el mismo
//! valor con signo contrario, asi que no puede haber huecos entre ellos.
//! Avanzar un pixel suma una constante, sin multiplicar.
//!
//! # Regla top-left
//! Si el centro de un pixel cae justo sobre una arista compartida, solo uno
//! de los dos triangulos debe pintarlo. Se lo queda el triangulo para el que
//! esa arista es "de arriba" o "de la izquierda": a las demas aristas se les
//! resta 1, que excluye el caso `E = 0`.

use super::pipeline::{TriChunk, TriRecord, SUBPIXEL};
use super::{VisPixel, TILE};

/// Limpia el tile y rasteriza en orden todos los triangulos que lo tocan.
pub fn raster_tile(
    tile: usize,
    pixels: &mut [VisPixel],
    chunks: &[TriChunk],
    tiles_x: usize,
    width: i32,
    height: i32,
) {
    pixels.fill(VisPixel::EMPTY);
    let x0 = ((tile % tiles_x) * TILE) as i32;
    let y0 = ((tile / tiles_x) * TILE) as i32;
    let x1 = (x0 + TILE as i32).min(width) - 1;
    let y1 = (y0 + TILE as i32).min(height) - 1;
    for (c, chunk) in chunks.iter().enumerate() {
        for &local in &chunk.bins[tile] {
            let tri = &chunk.tris[local as usize];
            let id = ((c as u32) << 16) | local as u32;
            rasterize(tri, x0, y0, x1, y1, |x, y, z| {
                let pixel = &mut pixels[(y - y0) as usize * TILE + (x - x0) as usize];
                // Profundidad invertida: lo mas cercano tiene el valor mayor.
                if z > pixel.depth {
                    pixel.depth = z;
                    pixel.id = id;
                }
            });
        }
    }
}

/// Recorre los pixeles de `tri` dentro del rectangulo `[x0, x1] x [y0, y1]`
/// y llama a `visit(x, y, z)` por cada centro de pixel cubierto.
#[inline(always)]
pub fn rasterize<F: FnMut(i32, i32, f32)>(
    tri: &TriRecord,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    mut visit: F,
) {
    let xs = tri.min_x.max(x0);
    let xe = tri.max_x.min(x1);
    let ys = tri.min_y.max(y0);
    let ye = tri.max_y.min(y1);
    if xs > xe || ys > ye {
        return;
    }

    // Centro del primer pixel, en punto fijo.
    let px = xs as i64 * SUBPIXEL + SUBPIXEL / 2;
    let py = ys as i64 * SUBPIXEL + SUBPIXEL / 2;
    let fx = tri.fx.map(i64::from);
    let fy = tri.fy.map(i64::from);

    // Arista k: la opuesta al vertice k, de (k+1) a (k+2).
    let mut row = [0i64; 3];
    let mut step_x = [0i64; 3];
    let mut step_y = [0i64; 3];
    for k in 0..3 {
        let (a, b) = ((k + 1) % 3, (k + 2) % 3);
        let dx = fx[b] - fx[a];
        let dy = fy[b] - fy[a];
        // Con area positiva y la y hacia abajo, las aristas de arriba van
        // hacia la derecha y las de la izquierda van hacia arriba.
        let top_left = dy < 0 || (dy == 0 && dx > 0);
        let bias = if top_left { 0 } else { -1 };
        row[k] = dx * (py - fy[a]) - dy * (px - fx[a]) + bias;
        step_x[k] = -dy * SUBPIXEL;
        step_y[k] = dx * SUBPIXEL;
    }

    let mut z_row =
        tri.z0 + tri.dz[0] * (xs as f32 + 0.5 - tri.ox) + tri.dz[1] * (ys as f32 + 0.5 - tri.oy);
    for y in ys..=ye {
        let (mut e0, mut e1, mut e2) = (row[0], row[1], row[2]);
        let mut z = z_row;
        for x in xs..=xe {
            // Las tres no negativas <=> el OR no tiene el bit de signo.
            if (e0 | e1 | e2) >= 0 {
                visit(x, y, z);
            }
            e0 += step_x[0];
            e1 += step_x[1];
            e2 += step_x[2];
            z += tri.dz[0];
        }
        row[0] += step_y[0];
        row[1] += step_y[1];
        row[2] += step_y[2];
        z_row += tri.dz[1];
    }
}
