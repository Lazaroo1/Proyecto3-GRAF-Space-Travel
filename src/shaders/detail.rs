//! Detalle triplanar en CPU: normales y cavidades procedurales con mipmaps.
//! Las pendientes se llevan del espacio del modelo al mundo, asi el relieve
//! gira junto con los planetas. No usa texturas ni shaders de GPU.
use super::{
    Fragment,
    textures::{NormalMap, Texel},
};
use crate::math::{Vec2, Vec3};

pub struct Rock {
    pub normal: Vec3,
    pub color: Vec3,
    pub rough: f32,
    pub ao: f32,
}
pub fn rock(f: &Fragment, u: &super::Uniforms, scale: f32, footprint: f32, strength: f32) -> Rock {
    let m = &f.object_to_world.m;
    let n = f.normal;
    let local = Vec3::new(
        m[0][0] * n.x + m[1][0] * n.y + m[2][0] * n.z,
        m[0][1] * n.x + m[1][1] * n.y + m[2][1] * n.z,
        m[0][2] * n.x + m[1][2] * n.y + m[2][2] * n.z,
    );
    let w = local.abs();
    let w = w * w * w * w;
    let w = w / (w.x + w.y + w.z).max(0.001);
    let p = f.local * scale;
    let maps = &u.textures.images;
    let mut color = Vec3::ZERO;
    let mut grad = Vec3::ZERO;
    let mut rough = 0.0;
    let mut ao = 0.0;
    for (axis, weight, uv) in [
        (0, w.x, Vec2::new(p.y, p.z)),
        (1, w.y, Vec2::new(p.z, p.x)),
        (2, w.z, Vec2::new(p.x, p.y)),
    ] {
        if weight < 0.015 {
            continue;
        }
        color += maps.rock.sample(uv, footprint * scale) * weight;
        let normal = maps.rock_normal.sample(uv, footprint * scale) * 2.0 - Vec3::ONE;
        let slope = Vec2::new(-normal.x, -normal.y) * (1.0 / normal.z.max(0.25));
        grad += match axis {
            0 => Vec3::new(0.0, slope.x, slope.y),
            1 => Vec3::new(slope.y, 0.0, slope.x),
            _ => Vec3::new(slope.x, slope.y, 0.0),
        } * weight;
        rough += maps.rock_rough.sample(uv, footprint * scale).x * weight;
        ao += maps.rock_ao.sample(uv, footprint * scale).x * weight;
    }
    let grad = f.object_to_world.mul_vec(grad);
    Rock {
        normal: (n - (grad - n * grad.dot(n)) * strength).normalize(),
        color,
        rough,
        ao,
    }
}

pub fn sample(f: &Fragment, map: &NormalMap, scale: f32, footprint: f32, strength: f32) -> Texel {
    let p = f.local * scale;
    let m = &f.object_to_world.m;
    let n = f.normal;
    let local_n = Vec3::new(
        m[0][0] * n.x + m[1][0] * n.y + m[2][0] * n.z,
        m[0][1] * n.x + m[1][1] * n.y + m[2][1] * n.z,
        m[0][2] * n.x + m[1][2] * n.y + m[2][2] * n.z,
    );
    let w = local_n.abs();
    let w = w * w * w * w;
    let w = w / (w.x + w.y + w.z).max(0.0001);
    let a = map.sample(Vec2::new(p.y, p.z), footprint * scale);
    let b = map.sample(Vec2::new(p.z, p.x), footprint * scale);
    let c = map.sample(Vec2::new(p.x, p.y), footprint * scale);
    let slope = |t: Texel| Vec2::new(-t.normal.x, -t.normal.y) * (1.0 / t.normal.z.max(0.15));
    let (ga, gb, gc) = (slope(a), slope(b), slope(c));
    let grad = Vec3::new(0.0, ga.x, ga.y) * w.x
        + Vec3::new(gb.y, 0.0, gb.x) * w.y
        + Vec3::new(gc.x, gc.y, 0.0) * w.z;
    let grad = f.object_to_world.mul_vec(grad);
    Texel {
        normal: (n - (grad - n * grad.dot(n)) * strength).normalize(),
        height: a.height * w.x + b.height * w.y + c.height * w.z,
        glow: a.glow * w.x + b.glow * w.y + c.glow * w.z,
    }
}
