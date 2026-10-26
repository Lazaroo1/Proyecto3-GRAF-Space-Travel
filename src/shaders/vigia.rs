//! Vigia: gigante gaseoso con una tormenta que es un ojo. Shader animado.
//!
//! - **Bandas**: ruido fractal muestreado en un espacio estirado en latitud
//!   (las bandas son largas y delgadas). Cada latitud gira a su propia
//!   velocidad, como en Jupiter: rotacion diferencial.
//! - **Remolino**: cerca del ojo, el punto se gira alrededor del eje del ojo
//!   antes de muestrear las bandas (rotacion de Rodrigues). El giro crece
//!   hacia el centro, asi las bandas se enroscan hacia la tormenta.
//! - **Ojo**: en coordenadas polares alrededor de su eje, fibras radiales de
//!   iris, un anillo limbico oscuro y una pupila que se dilata despacio. El
//!   iris brilla con luz propia: de noche se sigue viendo, mirandote.
//!   `Uniforms::eye_dir` lo apunta hacia la nave (ver `App`).

use super::{limb_haze, Fragment, Uniforms};
use crate::math::{smoothstep, Vec3};

/// Radio angular del iris (radianes).
pub const EYE_RADIUS: f32 = 0.34;

const PLUM: Vec3 = Vec3::new(0.20, 0.06, 0.24);
const MAGENTA: Vec3 = Vec3::new(0.50, 0.14, 0.38);
const CREAM: Vec3 = Vec3::new(0.82, 0.66, 0.58);
const BLOOD: Vec3 = Vec3::new(0.30, 0.03, 0.06);
const IRIS_DEEP: Vec3 = Vec3::new(0.55, 0.04, 0.07);
const IRIS_AMBER: Vec3 = Vec3::new(0.95, 0.52, 0.12);
const HAZE: Vec3 = Vec3::new(0.40, 0.15, 0.50);

/// Gira `p` un angulo `angle` alrededor del eje unitario `axis` (Rodrigues).
#[inline]
fn rotate_about(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.dot(p) * (1.0 - c))
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let p = f.local.normalize();
    let t = u.time;
    let eye = u.eye_dir;
    let r = p.dot(eye).clamp(-1.0, 1.0).acos();
    let x = r / EYE_RADIUS;

    // Las bandas lejanas no necesitan tantas octavas.
    let footprint = f.pos.length() * u.pixel_angle;
    let octaves = if footprint > 6.0 { 3 } else { 5 };

    // El remolino y las corrientes oscilan en vez de acumularse: un giro
    // diferencial proporcional al tiempo enrollaria las bandas sin fin (a los
    // quince minutos del ciclo cada latitud estaria girada decenas de vueltas
    // respecto a la vecina y solo quedaria ruido fino).
    let swirl = 2.4 * (-x * x * 0.6).exp() + 1.4 * (t * 0.07).sin() * (-x * x).exp();
    let q = rotate_about(p, eye, swirl);
    let shift = (q.y * 6.0).sin() * 0.9 * (t * 0.045).sin();
    let (s, c) = shift.sin_cos();
    let qr = Vec3::new(q.x * c - q.z * s, q.y, q.x * s + q.z * c);
    let turbulence = u.noise.fbm(Vec3::new(qr.x * 2.2, qr.y * 9.0, qr.z * 2.2 + t * 0.01), octaves);
    let band = (q.y * 13.0 + turbulence * 2.2).sin() * 0.5 + 0.5;
    let fine = if footprint > 6.0 {
        0.5
    } else {
        u.noise.fbm(Vec3::new(qr.x * 6.0, qr.y * 30.0, qr.z * 6.0), 3) * 0.5 + 0.5
    };
    let mut albedo = PLUM.lerp(MAGENTA, band);
    albedo = albedo.lerp(CREAM, smoothstep(0.62, 0.95, band * (0.6 + 0.8 * fine)));
    albedo = albedo.lerp(BLOOD, smoothstep(0.3, 0.0, band) * 0.6);

    let mut glow = Vec3::ZERO;
    if x < 1.25 {
        let helper = if eye.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let t1 = helper.cross(eye).normalize();
        let t2 = eye.cross(t1);
        let theta = p.dot(t2).atan2(p.dot(t1));
        let fibers = u.noise.fbm(
            Vec3::new(theta.cos() * 6.0, theta.sin() * 6.0, x * 1.5 - t * 0.03),
            4,
        ) * 0.5
            + 0.5;
        let iris = IRIS_DEEP.lerp(IRIS_AMBER, (smoothstep(0.2, 1.0, x) * 0.6 + fibers * 0.5).min(1.0));
        let pupil_radius = 0.3 + 0.04 * (t * 0.4).sin();
        let pupil = smoothstep(pupil_radius + 0.04, pupil_radius - 0.02, x);
        let limbal = smoothstep(0.82, 0.97, x) * smoothstep(1.1, 0.97, x);
        let inside = smoothstep(1.05, 0.95, x);
        albedo = albedo.lerp(iris * (1.0 - 0.75 * limbal), inside);
        albedo = albedo.lerp(Vec3::splat(0.008), pupil);
        // Brillo propio del iris y un filo rojo alrededor de la pupila.
        let edge = smoothstep(pupil_radius + 0.08, pupil_radius + 0.02, x) * (1.0 - pupil);
        glow = iris * (inside * (1.0 - pupil) * 0.12 * (0.6 + 0.4 * fibers))
            + Vec3::new(0.9, 0.08, 0.04) * (edge * 0.6);
    }

    let n = f.normal;
    let v = f.view_dir();
    let ndl = n.dot(f.light_dir(u));
    // Un gas no tiene un terminador duro: la luz envuelve un poco.
    let wrap = ((ndl + 0.2) / 1.2).max(0.0);
    let ndv = n.dot(v).max(0.0);
    let limb = 0.6 + 0.4 * ndv;
    let lit = albedo * u.sun_light * (wrap * limb) + albedo * 0.01;
    lit + glow * u.gains.vigia + limb_haze(f, HAZE, 0.8, ndl)
}
