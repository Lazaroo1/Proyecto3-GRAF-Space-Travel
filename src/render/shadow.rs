//! Mapa ortografico de sombras rasterizado en CPU, con PCF de nueve muestras.
//!
//! Se arma en paralelo y reutiliza sus buffers entre frames: los vertices se
//! proyectan por bloques, los triangulos se reparten en franjas horizontales
//! del mapa y cada hilo rasteriza solo su franja (filas disjuntas, sin
//! carreras en la profundidad).
use super::Draw;
use crate::math::{Perlin, Vec3};
use crate::parallel;
use crate::shaders::{self, Material};

/// Filas del mapa por franja.
const BAND_ROWS: usize = 16;
/// Triangulos que reparte cada trabajo del binning.
const TRIS_PER_JOB: usize = 8192;
/// Vertices que proyecta cada trabajo.
const POINTS_PER_JOB: usize = 4096;

/// Ejes del mapa: la luz entra por `-light` y el mapa mide `span` de lado.
#[derive(Clone, Copy)]
struct Frame {
    size: f32,
    span: f32,
    center: Vec3,
    right: Vec3,
    up: Vec3,
    light: Vec3,
}

impl Frame {
    #[inline]
    fn project(&self, p: Vec3) -> Vec3 {
        let q = p - self.center;
        Vec3::new(
            (q.dot(self.right) / self.span + 0.5) * self.size,
            (q.dot(self.up) / self.span + 0.5) * self.size,
            -q.dot(self.light),
        )
    }
}

/// Un dibujo que proyecta sombra: su indice y donde empiezan sus vertices y
/// sus triangulos en los buffers del frame.
struct Caster {
    draw: usize,
    points: usize,
    tris: usize,
}

pub struct ShadowMap {
    size: usize,
    frame: Frame,
    depth: Vec<f32>,
    casters: Vec<Caster>,
    points: Vec<Vec3>,
    /// Por trabajo de binning, los triangulos (indices globales de sus
    /// vertices) que tocan cada franja.
    bins: Vec<Vec<Vec<[u32; 3]>>>,
}

impl ShadowMap {
    pub fn new() -> Self {
        Self::with_size(1024)
    }

    fn with_size(size: usize) -> Self {
        Self {
            size,
            frame: Frame {
                size: size as f32,
                span: 1.0,
                center: Vec3::ZERO,
                right: Vec3::X,
                up: Vec3::Y,
                light: Vec3::Z,
            },
            depth: vec![f32::INFINITY; size * size],
            casters: Vec::new(),
            points: Vec::new(),
            bins: Vec::new(),
        }
    }

    /// Rehace el mapa para este frame.
    pub fn update(
        &mut self,
        draws: &[Draw],
        center: Vec3,
        light: Vec3,
        span: f32,
        time: f32,
        noise: &Perlin,
    ) {
        let light = light.normalize();
        let helper = if light.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let right = helper.cross(light).normalize();
        let up = light.cross(right);
        let frame = Frame {
            size: self.size as f32,
            span,
            center,
            right,
            up,
            light,
        };
        self.frame = frame;

        // Solo lo que cae sobre el cuadrado del mapa (visto desde la luz) y
        // no esta demasiado lejos de su centro.
        self.casters.clear();
        let (mut points, mut tris) = (0, 0);
        let half = span * 0.5;
        for (index, draw) in draws.iter().enumerate() {
            if matches!(
                draw.material,
                Material::Sun(_) | Material::Line(_) | Material::Vigia
            ) {
                continue;
            }
            let q = draw.bounds_center - center;
            let r = draw.bounds_radius;
            if q.length() > span * 1.5 + r
                || q.dot(right).abs() > half + r
                || q.dot(up).abs() > half + r
            {
                continue;
            }
            self.casters.push(Caster {
                draw: index,
                points,
                tris,
            });
            points += draw.mesh.positions.len();
            tris += draw.indices.len();
        }

        // 1. Vertices al espacio del mapa, deformados como en la vista.
        self.points.resize(points, Vec3::ZERO);
        let casters = &self.casters;
        parallel::chunks_mut(&mut self.points, POINTS_PER_JOB, POINTS_PER_JOB, |job, out| {
            let first = job * POINTS_PER_JOB;
            let end = first + out.len();
            let mut k = casters.partition_point(|c| c.points + draws[c.draw].mesh.positions.len() <= first);
            while k < casters.len() && casters[k].points < end {
                let caster = &casters[k];
                let draw = &draws[caster.draw];
                let mesh = draw.mesh;
                let deforms = draw.material.deforms();
                let lo = first.max(caster.points);
                let hi = end.min(caster.points + mesh.positions.len());
                for global in lo..hi {
                    let i = global - caster.points;
                    let mut p = mesh.positions[i];
                    if deforms {
                        p += mesh.normals[i]
                            * shaders::displace(&draw.material, p + draw.local_offset, time, noise);
                    }
                    out[global - first] = frame.project(draw.model.transform_affine(p));
                }
                k += 1;
            }
        });

        // 2. Cada triangulo a las franjas que toca.
        let bands = self.size.div_ceil(BAND_ROWS);
        let jobs = tris.div_ceil(TRIS_PER_JOB);
        if self.bins.len() < jobs {
            self.bins.resize_with(jobs, Vec::new);
        }
        let projected = &self.points;
        let size = self.size as f32;
        parallel::chunks_mut(&mut self.bins[..jobs], 1, 1, |job, slot| {
            let lists = &mut slot[0];
            lists.resize_with(bands, Vec::new);
            for list in lists.iter_mut() {
                list.clear();
            }
            let first = job * TRIS_PER_JOB;
            let end = (first + TRIS_PER_JOB).min(tris);
            let mut k = casters.partition_point(|c| c.tris + draws[c.draw].indices.len() <= first);
            while k < casters.len() && casters[k].tris < end {
                let caster = &casters[k];
                let indices = draws[caster.draw].indices;
                let lo = first.max(caster.tris);
                let hi = end.min(caster.tris + indices.len());
                let base = caster.points as u32;
                for tri in &indices[lo - caster.tris..hi - caster.tris] {
                    let ids = tri.map(|v| base + v);
                    let [a, b, c] = ids.map(|v| projected[v as usize]);
                    let (x0, x1) = (a.x.min(b.x).min(c.x), a.x.max(b.x).max(c.x));
                    let (y0, y1) = (a.y.min(b.y).min(c.y), a.y.max(b.y).max(c.y));
                    if x1 < 0.0 || y1 < 0.0 || x0 >= size || y0 >= size {
                        continue;
                    }
                    let first_band = (y0.max(0.0) as usize) / BAND_ROWS;
                    let last_band = ((y1.min(size - 1.0)) as usize) / BAND_ROWS;
                    for list in &mut lists[first_band..=last_band] {
                        list.push(ids);
                    }
                }
                k += 1;
            }
        });

        // 3. Cada franja limpia y rasteriza sus filas.
        let bins = &self.bins[..jobs];
        let width = self.size;
        parallel::chunks_mut(&mut self.depth, BAND_ROWS * width, BAND_ROWS * width, |band, rows| {
            rows.fill(f32::INFINITY);
            let y_first = band * BAND_ROWS;
            let y_last = y_first + rows.len() / width - 1;
            for lists in bins {
                for ids in &lists[band] {
                    let [a, b, c] = ids.map(|v| projected[v as usize]);
                    raster(rows, width, y_first, y_last, a, b, c);
                }
            }
        });
    }

    pub fn visibility(&self, p: Vec3, n: Vec3) -> f32 {
        let f = &self.frame;
        let q = f.project(p + n * 0.08);
        let x = q.x as i32;
        let y = q.y as i32;
        if x < 3 || y < 3 || x >= self.size as i32 - 3 || y >= self.size as i32 - 3 {
            return 1.0;
        }
        let bias = f.span / self.size as f32 * 0.8 + 0.05 * (1.0 - n.dot(f.light).max(0.0));
        let mut lit = 0.0;
        for dy in [-2, 0, 2] {
            for dx in [-2, 0, 2] {
                if q.z - bias <= self.depth[(y + dy) as usize * self.size + (x + dx) as usize] {
                    lit += 1.0;
                }
            }
        }
        lit / 9.0
    }
}

impl Default for ShadowMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Rasteriza el triangulo en las filas `y_first..=y_last` del mapa, que
/// llegan en `rows`, guardando la menor profundidad.
#[inline]
fn raster(rows: &mut [f32], width: usize, y_first: usize, y_last: usize, a: Vec3, b: Vec3, c: Vec3) {
    let edge = |p: Vec3, q: Vec3, x: f32, y: f32| (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x);
    let area = edge(a, b, c.x, c.y);
    if area.abs() < 0.0001 {
        return;
    }
    let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let y0 = (a.y.min(b.y).min(c.y).floor().max(0.0) as usize).max(y_first);
    let x1 = a.x.max(b.x).max(c.x).ceil().min(width as f32 - 1.0);
    let y1 = a.y.max(b.y).max(c.y).ceil().min(y_last as f32);
    if x1 < 0.0 || y1 < 0.0 || x0 > x1 as usize || y0 > y1 as usize {
        return;
    }
    let (x1, y1) = (x1 as usize, y1 as usize);
    let inv = 1.0 / area;
    for y in y0..=y1 {
        let py = y as f32 + 0.5;
        let row = &mut rows[(y - y_first) * width..(y - y_first + 1) * width];
        for (x, d) in row.iter_mut().enumerate().take(x1 + 1).skip(x0) {
            let px = x as f32 + 0.5;
            let wa = edge(b, c, px, py) * inv;
            let wb = edge(c, a, px, py) * inv;
            let wc = 1.0 - wa - wb;
            if wa >= 0.0 && wb >= 0.0 && wc >= 0.0 {
                *d = d.min(a.z * wa + b.z * wb + c.z * wc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn occluder_casts_shadow_but_point_toward_light_is_visible() {
        let mut map = ShadowMap::with_size(64);
        map.frame.span = 20.0;
        raster(
            &mut map.depth,
            64,
            0,
            63,
            Vec3::new(12.0, 12.0, -2.0),
            Vec3::new(52.0, 12.0, -2.0),
            Vec3::new(32.0, 52.0, -2.0),
        );
        assert!(map.visibility(Vec3::ZERO, Vec3::Z) < 0.01);
        assert_eq!(map.visibility(Vec3::Z * 4.0, Vec3::Z), 1.0);
    }

    #[test]
    fn bands_produce_the_same_map_as_a_single_pass() {
        // Un triangulo grande que cruza varias franjas: rasterizado franja
        // por franja da la misma profundidad que de una vez.
        let size = 64;
        let (a, b, c) = (
            Vec3::new(3.0, 1.0, -1.0),
            Vec3::new(60.0, 20.0, -3.0),
            Vec3::new(10.0, 62.0, -2.0),
        );
        let mut whole = vec![f32::INFINITY; size * size];
        raster(&mut whole, size, 0, size - 1, a, b, c);
        let mut banded = vec![f32::INFINITY; size * size];
        for (band, rows) in banded.chunks_mut(BAND_ROWS * size).enumerate() {
            let first = band * BAND_ROWS;
            raster(rows, size, first, first + rows.len() / size - 1, a, b, c);
        }
        assert_eq!(whole, banded);
        assert!(whole.iter().filter(|d| d.is_finite()).count() > 1000);
    }
}
