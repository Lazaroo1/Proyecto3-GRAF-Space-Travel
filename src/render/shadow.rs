//! Mapa ortografico de sombras rasterizado en CPU, con PCF de nueve muestras.
use super::Draw;
use crate::math::{Perlin, Vec3};
use crate::shaders::{self, Material};

pub struct ShadowMap {
    size: usize,
    span: f32,
    center: Vec3,
    right: Vec3,
    up: Vec3,
    light: Vec3,
    depth: Vec<f32>,
}
impl ShadowMap {
    pub fn build(
        draws: &[Draw],
        center: Vec3,
        light: Vec3,
        span: f32,
        time: f32,
        noise: &Perlin,
    ) -> Self {
        let light = light.normalize();
        let helper = if light.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let right = helper.cross(light).normalize();
        let up = light.cross(right);
        let size = 1024;
        let mut map = Self {
            size,
            span,
            center,
            right,
            up,
            light,
            depth: vec![f32::INFINITY; size * size],
        };
        for draw in draws {
            if matches!(
                draw.material,
                Material::Sun(_) | Material::Line(_) | Material::Vigia
            ) {
                continue;
            }
            if (draw.bounds_center - center).length() > span * 1.5 + draw.bounds_radius {
                continue;
            }
            let points: Vec<Vec3> = draw
                .mesh
                .positions
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    let p = if draw.material.deforms() {
                        p + draw.mesh.normals[i]
                            * shaders::displace(&draw.material, p + draw.local_offset, time, noise)
                    } else {
                        p
                    };
                    map.project(draw.model.transform_affine(p))
                })
                .collect();
            for &[a, b, c] in draw.indices {
                map.triangle(points[a as usize], points[b as usize], points[c as usize]);
            }
        }
        map
    }
    fn project(&self, p: Vec3) -> Vec3 {
        let q = p - self.center;
        Vec3::new(
            (q.dot(self.right) / self.span + 0.5) * self.size as f32,
            (q.dot(self.up) / self.span + 0.5) * self.size as f32,
            -q.dot(self.light),
        )
    }
    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let edge =
            |p: Vec3, q: Vec3, x: f32, y: f32| (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x);
        let area = edge(a, b, c.x, c.y);
        if area.abs() < 0.0001 {
            return;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let x1 = a.x.max(b.x).max(c.x).ceil().min(self.size as f32 - 1.0) as i32;
        let y1 = a.y.max(b.y).max(c.y).ceil().min(self.size as f32 - 1.0) as i32;
        if x1 < 0 || y1 < 0 || x0 >= self.size || y0 >= self.size {
            return;
        }
        let inv = 1.0 / area;
        for y in y0..=y1 as usize {
            for x in x0..=x1 as usize {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let wa = edge(b, c, px, py) * inv;
                let wb = edge(c, a, px, py) * inv;
                let wc = 1.0 - wa - wb;
                if wa >= 0.0 && wb >= 0.0 && wc >= 0.0 {
                    let z = a.z * wa + b.z * wb + c.z * wc;
                    let d = &mut self.depth[y * self.size + x];
                    *d = d.min(z);
                }
            }
        }
    }
    pub fn visibility(&self, p: Vec3, n: Vec3) -> f32 {
        let q = self.project(p + n * 0.08);
        let x = q.x as i32;
        let y = q.y as i32;
        if x < 3 || y < 3 || x >= self.size as i32 - 3 || y >= self.size as i32 - 3 {
            return 1.0;
        }
        let bias = self.span / self.size as f32 * 0.8 + 0.05 * (1.0 - n.dot(self.light).max(0.0));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn occluder_casts_shadow_but_point_toward_light_is_visible() {
        let mut map = ShadowMap {
            size: 64,
            span: 20.0,
            center: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
            light: Vec3::Z,
            depth: vec![f32::INFINITY; 4096],
        };
        map.triangle(
            Vec3::new(12.0, 12.0, -2.0),
            Vec3::new(52.0, 12.0, -2.0),
            Vec3::new(32.0, 52.0, -2.0),
        );
        assert!(map.visibility(Vec3::ZERO, Vec3::Z) < 0.01);
        assert_eq!(map.visibility(Vec3::Z * 4.0, Vec3::Z), 1.0);
    }
}
