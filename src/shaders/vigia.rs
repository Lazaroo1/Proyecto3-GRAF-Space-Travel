//! Vigia: gigante gaseoso morado con una tormenta que es un ojo y te sigue.
//!
//! - **Nubes**: el mapa del Hubble de Jupiter (NASA) solo da el detalle de
//!   las nubes, por su luminancia; el color sale de una paleta propia del
//!   indigo al lavanda. Cada latitud gira a su ritmo (rotacion diferencial).
//! - **Vortice**: cerca del ojo, el punto se gira alrededor del eje del ojo
//!   antes de muestrear (rotacion de Rodrigues). El giro crece hacia el
//!   centro, asi las bandas se enroscan en espiral, y avanza con el tiempo:
//!   la tormenta gira.
//! - **Ojo**: en coordenadas polares alrededor de su eje, fibras de iris que
//!   brillan, un anillo oscuro en el borde y una pupila que se dilata con
//!   cada latido de la estrella. `Uniforms::eye_dir` lo apunta a la nave.
//! - **Relampagos**: destellos breves en la tormenta, en lugares que cambian
//!   cada fraccion de segundo; de noche se ven de lejos.

use super::{Fragment, Uniforms, limb_haze};
use crate::math::{Vec3, smoothstep};

/// Radio angular del iris (radianes).
pub const EYE_RADIUS: f32 = 0.3;

const INDIGO: Vec3 = Vec3::new(0.012, 0.006, 0.05);
const VIOLET: Vec3 = Vec3::new(0.10, 0.03, 0.26);
const LAVENDER: Vec3 = Vec3::new(0.27, 0.15, 0.62);
const PALE: Vec3 = Vec3::new(0.56, 0.50, 0.86);
const MAGENTA: Vec3 = Vec3::new(0.42, 0.06, 0.38);
const IRIS_DEEP: Vec3 = Vec3::new(0.08, 0.02, 0.26);
const IRIS_BRIGHT: Vec3 = Vec3::new(0.52, 0.24, 0.92);

/// Gira `p` un angulo `angle` alrededor del eje unitario `axis` (Rodrigues).
#[inline]
fn rotate(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.dot(p) * (1.0 - c))
}

/// Paleta de las nubes por su luminancia.
fn palette(lum: f32) -> Vec3 {
    if lum < 0.35 {
        INDIGO.lerp(VIOLET, lum / 0.35)
    } else if lum < 0.65 {
        VIOLET.lerp(LAVENDER, (lum - 0.35) / 0.3)
    } else {
        LAVENDER.lerp(PALE, ((lum - 0.65) / 0.35).min(1.0))
    }
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let p = f.local.normalize();
    let t = u.time;
    let eye = u.eye_dir;
    let angle = p.dot(eye).clamp(-1.0, 1.0).acos();
    let r = angle / EYE_RADIUS;

    // Vortice que gira y enrosca las bandas hacia el ojo.
    let falloff = (-r * r * 0.45).exp();
    let swirl = 2.6 * falloff + t * 0.12 * (-r * r * 1.4).exp();
    let q = rotate(p, eye, swirl);
    // Rotacion diferencial: cada banda corre a su ritmo.
    let drift = t * 0.003 * (q.y * 9.0).sin();
    let (s, c) = drift.sin_cos();
    let q = Vec3::new(q.x * c - q.z * s, q.y, q.x * s + q.z * c);
    let footprint = f.pos.length() * u.pixel_angle / f.local.length().max(1.0) * 0.3;
    let clouds = u.textures.images.jupiter.sphere(q, footprint);
    let lum = smoothstep(0.03, 0.8, clouds.luminance()).powf(1.35);
    // Los cinturones rojizos de la foto quedan magenta.
    let warm = smoothstep(0.0, 0.08, clouds.x - clouds.z);
    let mut albedo = palette(lum).lerp(MAGENTA * (0.4 + lum), warm * 0.25);

    let mut glow = Vec3::ZERO;
    if r < 1.6 {
        let (a, b) = {
            let helper = if eye.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
            let a = helper.cross(eye).normalize();
            (a, eye.cross(a))
        };
        let theta = p.dot(b).atan2(p.dot(a));
        let fibers = u.noise.fbm(
            Vec3::new(theta.cos() * 5.0, theta.sin() * 5.0, r * 2.2 - t * 0.05),
            4,
        ) * 0.5
            + 0.5;
        let inside = smoothstep(1.05, 0.9, r);
        let limbal = smoothstep(0.8, 0.97, r) * smoothstep(1.12, 0.97, r);
        let streaks = fibers * fibers * 1.3 + 0.3 * smoothstep(0.9, 0.35, r);
        let iris = IRIS_DEEP.lerp(IRIS_BRIGHT, streaks.clamp(0.0, 1.0));
        albedo = albedo.lerp(iris * (0.45 + 0.55 * lum), inside * 0.8) * (1.0 - 0.8 * limbal);
        // La pupila se dilata con el latido.
        let turbulence = u.noise.noise(p * 40.0 + Vec3::splat(t * 0.1)) * 0.05;
        let pupil_radius = 0.3 + 0.07 * u.beat_at(0.0);
        let pupil = smoothstep(pupil_radius + 0.05, pupil_radius - 0.02, r + turbulence);
        albedo = albedo * (1.0 - 0.96 * pupil);
        let edge = smoothstep(pupil_radius + 0.14, pupil_radius + 0.03, r) * (1.0 - pupil);
        glow = iris * (inside * (1.0 - pupil) * 0.08 * streaks)
            + Vec3::new(0.75, 0.3, 1.0) * (edge * 0.7);
    }

    // Relampagos en la tormenta, cada uno dura una fraccion de segundo.
    let storm = smoothstep(2.6, 1.0, r);
    if storm > 0.0 {
        let slot = (t * 2.7).floor();
        let spot = u.noise.noise(p * 7.0 + Vec3::splat(slot * 13.7));
        let gate = u.noise.noise(Vec3::new(slot * 0.71, 3.3, 1.1));
        let flash = smoothstep(0.42, 0.62, spot) * smoothstep(0.1, 0.35, gate) * storm;
        glow += Vec3::new(0.55, 0.65, 1.0) * (flash * 0.5);
    }

    let n = f.normal;
    let l = f.light_dir(u);
    let v = f.view_dir();
    let nl = n.dot(l);
    let nv = n.dot(v).max(0.0);
    // Un gas no tiene un terminador duro: la luz envuelve un poco.
    let wrap = ((nl + 0.12) / 1.12).max(0.0);
    let light = Vec3::splat(u.sun_light.luminance()) * 1.45 + Vec3::new(0.02, 0.012, 0.05);
    let lit = albedo * light * (wrap * (0.55 + 0.45 * nv)) + albedo * 0.01;
    lit + glow * u.gains.vigia + limb_haze(f, Vec3::new(0.32, 0.12, 0.55), 0.35, nl)
}
