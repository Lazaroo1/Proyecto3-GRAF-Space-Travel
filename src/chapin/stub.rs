//! Piezas provisionales para que la version chapina compile mientras cada
//! planeta se hace. Se borra cuando todos tienen lo suyo.

use std::sync::Arc;

use crate::math::{Perlin, Vec3};
use crate::scene::Mesh;
use crate::scene::builder::MeshBuilder;
use crate::shaders::{Fragment, Uniforms, pbr};
use crate::terrain::{HeightField, TerrainSample};

/// Un relieve de lomas suaves.
struct Hills {
    noise: Perlin,
    height: f32,
}

impl HeightField for Hills {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let h = self.noise.fbm(dir * 3.0, 4);
        TerrainSample {
            height: self.height * h,
            attrs: Vec3::new(h * 0.5 + 0.5, 0.0, 0.0),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        (-self.height, self.height)
    }
}

pub fn field(seed: u32, height: f32) -> Arc<dyn HeightField> {
    Arc::new(Hills {
        noise: Perlin::new(seed),
        height,
    })
}

pub fn shade(f: &Fragment, u: &Uniforms, albedo: Vec3) -> Vec3 {
    pbr::shade(f, u, albedo * (0.6 + 0.6 * f.color.x), 0.8, 0.0, f.normal, 1.0)
}

pub fn mesh(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(Vec3::ONE);
    let sides = [4, 6, 8][detail];
    b.frustum(Vec3::ZERO, Vec3::Y, 6.0, 1.5, 0.8, sides, false, (true, true));
    b.build()
}

pub fn shade_prop(f: &Fragment, u: &Uniforms, sun: Vec3, albedo: Vec3) -> Vec3 {
    pbr::shade_light(f, u, albedo, 0.7, 0.0, f.normal, 1.0, sun)
}
