//! BRDF Cook-Torrance: GGX, Fresnel de Schlick y Smith. Todo en CPU.
use super::{Fragment, Uniforms};
use crate::math::Vec3;
use std::f32::consts::PI;

fn brdf(albedo: Vec3, rough: f32, metal: f32, n: Vec3, v: Vec3, l: Vec3) -> Vec3 {
    let nl = n.dot(l).max(0.0);
    let nv = n.dot(v).max(0.001);
    if nl <= 0.0 {
        return Vec3::ZERO;
    }
    let h = (v + l).normalize();
    let nh = n.dot(h).max(0.0);
    let vh = v.dot(h).max(0.0);
    let a = rough.max(0.07).powi(2);
    let a2 = a * a;
    let d = a2 / (PI * (nh * nh * (a2 - 1.0) + 1.0).powi(2)).max(0.00001);
    let k = (rough + 1.0).powi(2) / 8.0;
    let g = (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
    let f0 = Vec3::splat(0.04).lerp(albedo, metal);
    let fresnel = f0 + (Vec3::ONE - f0) * (1.0 - vh).powi(5);
    let spec = fresnel * (d * g / (4.0 * nl * nv).max(0.001));
    let diffuse = (Vec3::ONE - fresnel) * albedo * ((1.0 - metal) / PI);
    (diffuse + spec) * nl
}

pub fn shade(
    f: &Fragment,
    u: &Uniforms,
    albedo: Vec3,
    rough: f32,
    metal: f32,
    n: Vec3,
    cavity: f32,
) -> Vec3 {
    shade_light(f, u, albedo, rough, metal, n, cavity, u.sun_light)
}
pub fn shade_light(
    f: &Fragment,
    u: &Uniforms,
    albedo: Vec3,
    rough: f32,
    metal: f32,
    n: Vec3,
    cavity: f32,
    direct: Vec3,
) -> Vec3 {
    let v = f.view_dir();
    let l = f.light_dir(u);
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, n));
    let mut color = brdf(albedo, rough, metal, n, v, l) * direct * (3.6 * shadow);
    // Luz indirecta tenue; las cavidades permanecen oscuras.
    let nv = n.dot(v).max(0.0);
    let reflect = n * (2.0 * n.dot(v)) - v;
    let f0 = Vec3::splat(0.04).lerp(albedo, metal);
    let fresnel = f0 + (Vec3::ONE - f0) * (1.0 - nv).powi(5);
    let env = u.sky.sample(reflect) * 0.5 + Vec3::new(0.015, 0.022, 0.030);
    let ao = f.ao * cavity;
    color += (albedo * Vec3::new(0.055, 0.070, 0.085) * (u.fill * (1.0 - metal))
        + env * fresnel * (1.0 - rough * 0.7))
        * ao;
    for point in u.points {
        let delta = point.pos - f.pos;
        let r2 = delta.length_squared();
        if r2 < point.radius * point.radius && r2 > 0.01 {
            let fade = (1.0 - r2 / (point.radius * point.radius)).powi(2);
            color += brdf(albedo, rough, metal, n, v, delta / r2.sqrt())
                * point.color
                * (fade / (r2 + 0.8))
                * ao;
        }
    }
    color
}

#[derive(Clone, Copy)]
pub struct PointLight {
    pub pos: Vec3,
    pub color: Vec3,
    pub radius: f32,
}
