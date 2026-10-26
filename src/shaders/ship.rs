//! Materiales originales de Blender con UV, rugosidad y metalicidad.
use super::{Fragment, Uniforms, pbr};
use crate::math::{Vec2, Vec3, smoothstep};
#[derive(Clone, Copy, Debug)]
pub struct ShipStyle {
    pub specular: f32,
    pub shininess: f32,
}
pub fn shade(f: &Fragment, u: &Uniforms, s: &ShipStyle) -> Vec3 {
    let id = (f.color.x - 100.0).round().max(0.0) as usize;
    let Some(mat) = u.textures.images.ship.get(id) else {
        return Vec3::new(0.2, 0.0, 0.2);
    };
    let uv = Vec2::new(f.uv.x, 1.0 - f.uv.y);
    let footprint = f.pos.length() * u.pixel_angle / 1.1;
    let albedo = mat
        .image
        .as_ref()
        .map_or(mat.base, |map| map.sample(uv, footprint));
    let plate = u.textures.hull.sample(f.uv * 2.0, footprint * 2.0);
    let n = f.perturb(plate.normal, 0.10);
    let wear = u.noise.noise(f.local * 29.0) * 0.5 + 0.5;
    let rough = (mat.rough + (wear - 0.5) * 0.18).clamp(0.09, 0.9);
    let dirt = smoothstep(0.55, 0.22, plate.height) * 0.55;
    let a = albedo * (1.0 - dirt);
    let lit = pbr::shade(
        f,
        u,
        a,
        rough,
        mat.metal,
        n,
        (0.45 + 0.55 * plate.height).clamp(0.25, 1.0),
    );
    let emission = if mat.emission.length_squared() > 0.01 {
        let source = if mat.image.is_some() {
            albedo
        } else {
            Vec3::ONE
        };
        mat.emission * source * 0.7
    } else {
        Vec3::ZERO
    };
    let _ = (s.specular, s.shininess);
    lit + emission
}
