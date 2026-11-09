//! Hilos de luz: segmentos que se suman a la imagen con un nucleo fino y un
//! halo suave, como las particulas pero alargados.
//!
//! Sirven para lo que es luz y no superficie: las lineas de campo de Faro.
//! Una cinta opaca tapa lo que tiene detras y no puede tener halo; un hilo
//! solo agrega luz delante de la superficie ya resuelta.
//!
//! # Como se dibuja
//! Cada segmento se proyecta (recortado contra el plano cercano) y se anota en
//! los tiles que toca su caja, ensanchada por el alcance del halo. En el pase
//! por tile, cada pixel busca su punto mas cercano en el segmento (`t` entre
//! 0 y 1), interpola ahi la profundidad y el color, y suma
//!
//! ```text
//!   color * (exp(-d^2 / 2 s_nucleo^2) + halo * exp(-d^2 / 2 s_halo^2))
//! ```
//!
//! con `d` la distancia en pixeles. Para que las uniones de una polilinea no
//! brillen el doble, solo el primer y el ultimo segmento tienen tapas: los
//! de en medio cuentan solo los pixeles que caen frente a ellos.

use super::{View, VisPixel, TILE};
use crate::math::Vec3;

/// Un segmento de luz relativo a la camara, con el color de cada punta.
#[derive(Clone, Copy, Debug)]
pub struct GlowLine {
    pub a: Vec3,
    pub b: Vec3,
    pub color_a: Vec3,
    pub color_b: Vec3,
    /// Si tiene tapa en cada punta (los extremos de una polilinea).
    pub cap_a: bool,
    pub cap_b: bool,
}

/// Ancho del nucleo y del halo en pixeles, y brillo del halo.
const CORE: f32 = 0.7;
const HALO: f32 = 3.4;
const HALO_GAIN: f32 = 0.12;
/// Hasta donde llega el halo: tres desviaciones.
const REACH: f32 = 3.0 * HALO;

/// Un segmento ya proyectado.
#[derive(Clone, Copy, Debug)]
struct Splat {
    x0: f32,
    y0: f32,
    dx: f32,
    dy: f32,
    inv_len2: f32,
    depth0: f32,
    depth1: f32,
    color0: Vec3,
    color1: Vec3,
    cap_a: bool,
    cap_b: bool,
}

pub struct LineBins {
    splats: Vec<Splat>,
    bins: Vec<Vec<u32>>,
}

impl LineBins {
    pub fn new() -> Self {
        Self {
            splats: Vec::new(),
            bins: Vec::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.splats.len()
    }

    pub fn build(&mut self, lines: &[GlowLine], view: &View, width: usize, height: usize, tiles_x: usize, tiles: usize) {
        self.splats.clear();
        if self.bins.len() != tiles {
            self.bins.resize_with(tiles, Vec::new);
        }
        for bin in &mut self.bins {
            bin.clear();
        }
        let (half_w, half_h) = (width as f32 * 0.5, height as f32 * 0.5);
        let tiles_y = tiles / tiles_x;
        for line in lines {
            let mut ca = view.view_proj.transform_point(line.a);
            let mut cb = view.view_proj.transform_point(line.b);
            let (mut col_a, mut col_b) = (line.color_a, line.color_b);
            let (mut cap_a, mut cap_b) = (line.cap_a, line.cap_b);
            // Recorte contra el plano cercano.
            let near = view.near;
            if ca.w < near && cb.w < near {
                continue;
            }
            if ca.w < near || cb.w < near {
                let k = (near - ca.w) / (cb.w - ca.w);
                let cut = ca.lerp(cb, k);
                let col = col_a.lerp(col_b, k);
                if ca.w < near {
                    ca = cut;
                    col_a = col;
                    cap_a = false;
                } else {
                    cb = cut;
                    col_b = col;
                    cap_b = false;
                }
            }
            let project = |c: crate::math::Vec4| ((c.x / c.w + 1.0) * half_w, (1.0 - c.y / c.w) * half_h, near / c.w);
            let (x0, y0, d0) = project(ca);
            let (x1, y1, d1) = project(cb);
            let (min_x, max_x) = (x0.min(x1) - REACH, x0.max(x1) + REACH);
            let (min_y, max_y) = (y0.min(y1) - REACH, y0.max(y1) + REACH);
            if max_x < 0.0 || max_y < 0.0 || min_x > width as f32 || min_y > height as f32 {
                continue;
            }
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len2 = dx * dx + dy * dy;
            let index = self.splats.len() as u32;
            self.splats.push(Splat {
                x0,
                y0,
                dx,
                dy,
                inv_len2: if len2 > 1e-8 { 1.0 / len2 } else { 0.0 },
                depth0: d0,
                depth1: d1,
                color0: col_a,
                color1: col_b,
                // Un segmento de menos de un pixel es todo tapa.
                cap_a: cap_a || len2 < 1.0,
                cap_b: cap_b || len2 < 1.0,
            });
            let tx0 = (min_x.max(0.0) as usize / TILE).min(tiles_x - 1);
            let ty0 = (min_y.max(0.0) as usize / TILE).min(tiles_y - 1);
            let tx1 = (max_x.max(0.0) as usize / TILE).min(tiles_x - 1);
            let ty1 = (max_y.max(0.0) as usize / TILE).min(tiles_y - 1);
            for ty in ty0..=ty1 {
                for tx in tx0..=tx1 {
                    // Solo los tiles que el segmento ensanchado de verdad
                    // cruza: distancia del centro del tile a la recta.
                    let cx = (tx * TILE) as f32 + TILE as f32 * 0.5;
                    let cy = (ty * TILE) as f32 + TILE as f32 * 0.5;
                    let reach = REACH + TILE as f32 * 0.71;
                    let t = if len2 > 1e-8 { (((cx - x0) * dx + (cy - y0) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    let (px, py) = (x0 + dx * t - cx, y0 + dy * t - cy);
                    if px * px + py * py <= reach * reach {
                        self.bins[ty * tiles_x + tx].push(index);
                    }
                }
            }
        }
    }
}

/// Suma los hilos del tile `tile` a su color HDR `out`.
pub fn splat_tile(tile: usize, vis: &[VisPixel], out: &mut [Vec3], bins: &LineBins, tiles_x: usize, width: usize, height: usize) {
    let Some(bin) = bins.bins.get(tile) else { return };
    if bin.is_empty() {
        return;
    }
    let x0 = ((tile % tiles_x) * TILE) as i32;
    let y0 = ((tile / tiles_x) * TILE) as i32;
    let x_end = (x0 + TILE as i32).min(width as i32);
    let y_end = (y0 + TILE as i32).min(height as i32);
    let k_core = -0.5 / (CORE * CORE);
    let k_halo = -0.5 / (HALO * HALO);
    for &index in bin {
        let s = &bins.splats[index as usize];
        let (sx1, sy1) = (s.x0 + s.dx, s.y0 + s.dy);
        let px0 = ((s.x0.min(sx1) - REACH).floor() as i32).max(x0);
        let py0 = ((s.y0.min(sy1) - REACH).floor() as i32).max(y0);
        let px1 = ((s.x0.max(sx1) + REACH).ceil() as i32).min(x_end);
        let py1 = ((s.y0.max(sy1) + REACH).ceil() as i32).min(y_end);
        for py in py0..py1 {
            let fy = py as f32 + 0.5 - s.y0;
            let row = ((py - y0) as usize) * TILE;
            for px in px0..px1 {
                let fx = px as f32 + 0.5 - s.x0;
                let mut t = (fx * s.dx + fy * s.dy) * s.inv_len2;
                if t < 0.0 {
                    if !s.cap_a {
                        continue;
                    }
                    t = 0.0;
                } else if t >= 1.0 {
                    if !s.cap_b {
                        continue;
                    }
                    t = 1.0;
                }
                let (ex, ey) = (fx - s.dx * t, fy - s.dy * t);
                let d2 = ex * ex + ey * ey;
                if d2 > REACH * REACH {
                    continue;
                }
                let i = row + (px - x0) as usize;
                // Detras de la superficie no se ve.
                let depth = s.depth0 + (s.depth1 - s.depth0) * t;
                if depth <= vis[i].depth {
                    continue;
                }
                let w = (d2 * k_core).exp() + HALO_GAIN * (d2 * k_halo).exp();
                out[i] += s.color0.lerp(s.color1, t) * w;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::camera::Camera;

    fn view(width: usize, height: usize) -> View {
        let camera = Camera {
            pos: crate::math::DVec3::ZERO,
            rot: crate::math::DQuat::IDENTITY,
            fov_y: 1.2,
            near: 0.05,
        };
        camera.view(width, height)
    }

    /// Una polilinea recta de varios segmentos brilla parejo: las uniones no
    /// suman el doble.
    #[test]
    fn the_joints_of_a_polyline_do_not_glow_twice() {
        let (w, h) = (128, 64);
        let view = view(w, h);
        let tiles_x = w.div_ceil(TILE);
        let tiles = tiles_x * h.div_ceil(TILE);
        let points: Vec<Vec3> = (0..6).map(|k| Vec3::new(-3.0 + k as f32 * 1.2, 0.0, -10.0)).collect();
        let lines: Vec<GlowLine> = points
            .windows(2)
            .enumerate()
            .map(|(k, p)| GlowLine {
                a: p[0],
                b: p[1],
                color_a: Vec3::ONE,
                color_b: Vec3::ONE,
                cap_a: k == 0,
                cap_b: k == 4,
            })
            .collect();
        let mut bins = LineBins::new();
        bins.build(&lines, &view, w, h, tiles_x, tiles);
        let sky = vec![VisPixel::EMPTY; TILE * TILE];
        let mut image = vec![Vec3::ZERO; w * h];
        for tile in 0..tiles {
            let mut out = vec![Vec3::ZERO; TILE * TILE];
            splat_tile(tile, &sky, &mut out, &bins, tiles_x, w, h);
            let (tx, ty) = ((tile % tiles_x) * TILE, (tile / tiles_x) * TILE);
            for y in 0..TILE.min(h - ty) {
                for x in 0..TILE.min(w - tx) {
                    image[(ty + y) * w + tx + x] = out[y * TILE + x];
                }
            }
        }
        // La linea va de x = 50 a x = 78. A lo largo de la fila central, lejos
        // de las puntas, el
        // brillo no cambia mas de un 2%.
        let row = h / 2;
        let values: Vec<f32> = (54..74).map(|x| image[row * w + x].x.max(image[(row - 1) * w + x].x)).collect();
        let (lo, hi) = values.iter().fold((f32::MAX, 0.0f32), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(lo > 0.3 && hi / lo < 1.02, "{lo} {hi}");
    }
}
