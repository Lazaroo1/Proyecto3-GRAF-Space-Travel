//! Particulas: puntos de luz que se suman a la imagen.
//!
//! El visibility buffer guarda un solo triangulo por pixel, asi que no hay
//! transparencia. Las particulas (brasas, esporas, el escape de la nave) son
//! luz, no superficie: basta **sumarlas** al color ya sombreado, sin tapar
//! nada. Cada una se proyecta, se reparte en los tiles que toca y, dentro
//! del mismo pase por tile, se suma como una mancha gaussiana solo en los
//! pixeles donde queda delante de la superficie (su profundidad contra la
//! del visibility buffer). Como se suma antes de extraer el brillo, las
//! particulas intensas tambien florecen con el bloom.
//!
//! # Energia
//! Una particula de radio `r` (en el mundo) cubre `r_px` pixeles de radio en
//! pantalla y aporta en total `color * pi * r_px^2`, repartido con una
//! gaussiana normalizada. Lejos, cuando es mas chica que un pixel, su luz
//! total baja como su area aparente en vez de quedarse en un pixel
//! encendido: no parpadea ni parece mas grande de lo que es.

use super::{View, VisPixel, TILE};
use crate::math::Vec3;

/// Un punto de luz, relativo a la camara.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub pos: Vec3,
    /// Radio en el mundo.
    pub radius: f32,
    /// Luz que emite (HDR lineal).
    pub color: Vec3,
}

/// Una particula ya proyectada.
#[derive(Clone, Copy, Debug)]
pub struct Splat {
    x: f32,
    y: f32,
    /// `near / w`, como el visibility buffer: gana el mayor.
    depth: f32,
    /// Desviacion de la gaussiana en pixeles.
    sigma: f32,
    /// Luz en el centro (ya dividida entre el area de la gaussiana).
    peak: Vec3,
    /// Radio de la mancha en pixeles.
    reach: f32,
}

/// La desviacion minima: una particula mas chica se dibuja de este tamano
/// con menos luz.
const MIN_SIGMA: f32 = 0.5;
/// Las particulas enormes (muy cerca de la camara) se limitan a esto.
const MAX_SIGMA: f32 = 6.0;

/// Proyecta las particulas y las reparte en los tiles que tocan.
pub struct ParticleBins {
    pub splats: Vec<Splat>,
    /// Indices de `splats` por tile.
    pub bins: Vec<Vec<u32>>,
}

impl ParticleBins {
    pub fn new() -> Self {
        Self {
            splats: Vec::new(),
            bins: Vec::new(),
        }
    }

    pub fn build(&mut self, particles: &[Particle], view: &View, width: usize, height: usize, tiles_x: usize, tiles: usize) {
        self.splats.clear();
        if self.bins.len() != tiles {
            self.bins.resize_with(tiles, Vec::new);
        }
        for bin in &mut self.bins {
            bin.clear();
        }
        let half_w = width as f32 * 0.5;
        let half_h = height as f32 * 0.5;
        // Pixeles por unidad de mundo a distancia 1.
        let scale = half_h / view.tan_half_y;
        let tiles_y = tiles / tiles_x;
        for p in particles {
            let clip = view.view_proj.transform_point(p.pos);
            if clip.w <= view.near {
                continue;
            }
            let inv_w = 1.0 / clip.w;
            let x = (clip.x * inv_w + 1.0) * half_w;
            let y = (1.0 - clip.y * inv_w) * half_h;
            let r_px = p.radius * scale * inv_w;
            let sigma = (r_px * 0.5).clamp(MIN_SIGMA, MAX_SIGMA);
            let reach = 3.0 * sigma;
            if x + reach < 0.0 || y + reach < 0.0 || x - reach > width as f32 || y - reach > height as f32 {
                continue;
            }
            // Energia total = color * area aparente, repartida en la gaussiana.
            let area = std::f32::consts::PI * r_px * r_px;
            let peak = p.color * (area / (2.0 * std::f32::consts::PI * sigma * sigma));
            let index = self.splats.len() as u32;
            self.splats.push(Splat {
                x,
                y,
                depth: view.near * inv_w,
                sigma,
                peak,
                reach,
            });
            let tx0 = ((x - reach) as i32 / TILE as i32).max(0) as usize;
            let ty0 = ((y - reach) as i32 / TILE as i32).max(0) as usize;
            let tx1 = (((x + reach) as i32 / TILE as i32).max(0) as usize).min(tiles_x - 1);
            let ty1 = (((y + reach) as i32 / TILE as i32).max(0) as usize).min(tiles_y - 1);
            for ty in ty0..=ty1 {
                for tx in tx0..=tx1 {
                    self.bins[ty * tiles_x + tx].push(index);
                }
            }
        }
    }
}

/// Suma las particulas del tile `tile` a su color HDR `out`.
pub fn splat_tile(tile: usize, vis: &[VisPixel], out: &mut [Vec3], bins: &ParticleBins, tiles_x: usize, width: usize, height: usize) {
    let Some(bin) = bins.bins.get(tile) else { return };
    if bin.is_empty() {
        return;
    }
    let x0 = ((tile % tiles_x) * TILE) as i32;
    let y0 = ((tile / tiles_x) * TILE) as i32;
    let x_end = (x0 + TILE as i32).min(width as i32);
    let y_end = (y0 + TILE as i32).min(height as i32);
    for &index in bin {
        let s = &bins.splats[index as usize];
        let px0 = ((s.x - s.reach).floor() as i32).max(x0);
        let py0 = ((s.y - s.reach).floor() as i32).max(y0);
        let px1 = ((s.x + s.reach).ceil() as i32).min(x_end);
        let py1 = ((s.y + s.reach).ceil() as i32).min(y_end);
        let k = -0.5 / (s.sigma * s.sigma);
        for py in py0..py1 {
            let dy = py as f32 + 0.5 - s.y;
            let row = ((py - y0) as usize) * TILE;
            for px in px0..px1 {
                let i = row + (px - x0) as usize;
                // Detras de la superficie no se ve.
                if s.depth <= vis[i].depth {
                    continue;
                }
                let dx = px as f32 + 0.5 - s.x;
                let w = ((dx * dx + dy * dy) * k).exp();
                out[i] += s.peak * w;
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

    #[test]
    fn a_particle_adds_light_only_in_front_of_the_surface() {
        let (w, h) = (128, 64);
        let view = view(w, h);
        let mut bins = ParticleBins::new();
        let tiles_x = w.div_ceil(TILE);
        let tiles = tiles_x * h.div_ceil(TILE);
        // Una particula a 10 unidades, en el centro de la pantalla.
        let p = Particle {
            pos: Vec3::new(0.0, 0.0, -10.0),
            radius: 0.2,
            color: Vec3::ONE,
        };
        bins.build(&[p], &view, w, h, tiles_x, tiles);
        assert_eq!(bins.splats.len(), 1);
        let center_tile = (h / 2 / TILE) * tiles_x + (w / 2 / TILE);
        let mut out = vec![Vec3::ZERO; TILE * TILE];
        // Cielo detras: se ve.
        let sky = vec![VisPixel::EMPTY; TILE * TILE];
        splat_tile(center_tile, &sky, &mut out, &bins, tiles_x, w, h);
        let lit: f32 = out.iter().map(|c| c.x).sum();
        assert!(lit > 0.0);
        // Una pared a 5 unidades (mas cerca): no se ve.
        let wall = vec![VisPixel { depth: 0.05 / 5.0, id: 0 }; TILE * TILE];
        let mut hidden = vec![Vec3::ZERO; TILE * TILE];
        splat_tile(center_tile, &wall, &mut hidden, &bins, tiles_x, w, h);
        assert!(hidden.iter().all(|c| c.x == 0.0));
    }

    #[test]
    fn the_total_light_follows_the_apparent_area() {
        let (w, h) = (256, 256);
        let view = view(w, h);
        let tiles_x = w.div_ceil(TILE);
        let tiles = tiles_x * h.div_ceil(TILE);
        let total = |distance: f32| {
            let mut bins = ParticleBins::new();
            let p = Particle {
                pos: Vec3::new(0.0, 0.0, -distance),
                radius: 1.0,
                color: Vec3::ONE,
            };
            bins.build(&[p], &view, w, h, tiles_x, tiles);
            let sky = vec![VisPixel::EMPTY; TILE * TILE];
            let mut sum = 0.0;
            for tile in 0..tiles {
                let mut out = vec![Vec3::ZERO; TILE * TILE];
                splat_tile(tile, &sky, &mut out, &bins, tiles_x, w, h);
                sum += out.iter().map(|c| c.x).sum::<f32>();
            }
            sum
        };
        // Al doble de distancia, un cuarto de la luz.
        let ratio = total(40.0) / total(80.0);
        assert!((ratio - 4.0).abs() < 0.2, "{ratio}");
    }
}
