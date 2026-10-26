//! Nubes basadas en el mapa Hubble de NASA; tormenta deformada en CPU.
use super::{Fragment, Uniforms, limb_haze};
use crate::math::{Vec3, smoothstep};
pub const EYE_RADIUS: f32 = 0.24;
fn rotate(p: Vec3, a: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + a.cross(p) * s + a * (a.dot(p) * (1.0 - c))
}
pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let p = f.local.normalize();
    let eye = u.eye_dir;
    let angle = p.dot(eye).clamp(-1.0, 1.0).acos();
    let r = angle / EYE_RADIUS;
    let swirl = 1.2 * (-r * r * 0.7).exp() * (0.85 + 0.15 * (u.time * 0.06).sin());
    let q = rotate(p, eye, swirl);
    let footprint = f.pos.length() * u.pixel_angle / f.local.length().max(1.0) * 0.3;
    let clouds = u.textures.images.jupiter.sphere(q, footprint);
    let lum = clouds.luminance();
    // Mantener los filamentos fotografiados al adaptar la paleta a violeta.
    let violet = Vec3::new(lum * 0.48, lum * 0.32, lum * 0.64);
    let mut a = clouds.lerp(violet, 0.78) * (0.48 + 0.52 * smoothstep(0.02, 0.6, lum));
    let turbulence = u.noise.noise(p * 48.0) * 0.08;
    let pupil = smoothstep(0.32, 0.12, r + turbulence) * 0.91;
    a = a * (1.0 - pupil);
    let n = f.normal;
    let l = f.light_dir(u);
    let v = f.view_dir();
    let nl = n.dot(l);
    let nv = n.dot(v).max(0.0);
    let wrap = ((nl + 0.05) / 1.05).max(0.0);
    let light = Vec3::splat(u.sun_light.luminance()) * 1.65 + Vec3::new(0.02, 0.035, 0.08);
    let lit = a * light * (wrap * (0.55 + 0.45 * nv)) + a * 0.008;
    lit + limb_haze(f, Vec3::new(0.12, 0.08, 0.22), 0.08, nl)
}
