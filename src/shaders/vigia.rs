//! Vigia: gigante gaseoso morado con una tormenta que es un ojo y te sigue.
//!
//! - **Nubes**: el mapa del Hubble de Jupiter (NASA) solo da el detalle de
//!   las nubes, por su luminancia; el color sale de una paleta propia, del
//!   indigo casi negro al morado, la orquidea y el rosa de las crestas. Cada
//!   latitud gira a su ritmo (rotacion diferencial).
//! - **Vortice**: cerca del ojo, el punto se gira alrededor del eje del ojo
//!   antes de muestrear (rotacion de Rodrigues). El giro crece hacia el
//!   centro, asi las bandas se enroscan en espiral, y avanza con el tiempo:
//!   la tormenta gira. Encima, brazos en espiral logaritmica que caen al ojo.
//! - **Ojo**: en coordenadas polares alrededor de su eje, un iris de gas
//!   turbulento que brilla rosa y magenta (ruido que se retuerce y corre
//!   hacia adentro), mas caliente junto a la pupila; un anillo oscuro en el
//!   borde y una pupila negra y redonda que se dilata con cada latido de la
//!   estrella y se cierra con la nave cerca. `Uniforms::eye_dir` lo apunta a
//!   la nave.
//! - **La tormenta cambia**: un ciclo lento le sube y le baja la fuerza y
//!   corre su color del rosa al violeta.
//! - **Relampagos**: destellos chicos dentro de las nubes del vortice, en
//!   lugares que cambian cada fraccion de segundo; de noche se ven de lejos.
//! - **De noche no se apaga**: las capas hondas del gas brillan violeta a
//!   traves de las nubes, el iris alumbra el remolino a su alrededor y, a
//!   contraluz, la bruma alta dispersa la estrella en un borde fino. Asi el
//!   disco se lee entero desde cualquier lado y el ojo no queda flotando.

use super::{Fragment, Uniforms, limb_haze};
use crate::math::{Vec3, smoothstep};

/// Radio angular del iris (radianes).
pub const EYE_RADIUS: f32 = 0.3;

const INDIGO: Vec3 = Vec3::new(0.012, 0.005, 0.04);
const VIOLET: Vec3 = Vec3::new(0.09, 0.024, 0.2);
const ORCHID: Vec3 = Vec3::new(0.3, 0.08, 0.4);
const ROSE: Vec3 = Vec3::new(0.66, 0.4, 0.78);
const IRIS_DEEP: Vec3 = Vec3::new(0.22, 0.02, 0.3);
const IRIS_HOT: Vec3 = Vec3::new(1.0, 0.32, 0.72);
const IRIS_COOL: Vec3 = Vec3::new(0.62, 0.22, 1.0);
/// Tinte del brillo propio del gas en el lado de noche.
const DEEP_GLOW: Vec3 = Vec3::new(0.75, 0.42, 1.25);

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
    } else if lum < 0.7 {
        VIOLET.lerp(ORCHID, (lum - 0.35) / 0.35)
    } else {
        ORCHID.lerp(ROSE, ((lum - 0.7) / 0.3).min(1.0))
    }
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let p = f.local.normalize();
    let t = u.time;
    let eye = u.eye_dir;
    let angle = p.dot(eye).clamp(-1.0, 1.0).acos();
    let r = angle / EYE_RADIUS;
    // El ciclo de la tormenta: fuerza (0 a 1) y su color.
    let cycle = 0.5 + 0.5 * (t * 0.11).sin();
    let hue = 0.5 + 0.5 * (t * 0.047 + 1.3).sin();

    // Vortice que gira y enrosca las bandas hacia el ojo.
    let falloff = (-r * r * 0.45).exp();
    let swirl = (2.6 + 0.8 * cycle) * falloff + t * 0.12 * (-r * r * 1.4).exp();
    let q = rotate(p, eye, swirl);
    // Rotacion diferencial: cada banda corre a su ritmo.
    let drift = t * 0.003 * (q.y * 9.0).sin();
    let (s, c) = drift.sin_cos();
    let q = Vec3::new(q.x * c - q.z * s, q.y, q.x * s + q.z * c);
    let footprint = f.pos.length() * u.pixel_angle / f.local.length().max(1.0) * 0.3;
    let clouds = u.textures.images.jupiter.sphere(q, footprint);
    let lum = smoothstep(0.03, 0.8, clouds.luminance()).powf(1.5);
    // Los cinturones rojizos de la foto quedan magenta.
    let warm = smoothstep(0.0, 0.08, clouds.x - clouds.z);
    let mut albedo = palette(lum).lerp(ORCHID * (0.5 + lum), warm * 0.35);

    // Ejes alrededor del ojo, para las coordenadas polares.
    let (a, b) = {
        let helper = if eye.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let a = helper.cross(eye).normalize();
        (a, eye.cross(a))
    };
    let theta = p.dot(b).atan2(p.dot(a));
    let iris_color = IRIS_COOL.lerp(IRIS_HOT, hue);

    let mut glow = Vec3::ZERO;
    // Brazos en espiral logaritmica que caen al ojo y giran con el: claros
    // en los brazos, oscuro entre ellos.
    if r < 3.2 {
        let arms = (theta * 3.0 + 2.2 * (r + 0.05).ln() - t * 0.35).sin() * 0.5 + 0.5;
        let band = smoothstep(3.2, 1.6, r) * smoothstep(0.9, 1.15, r);
        albedo = albedo * (1.0 + band * (arms * arms * 0.9 - 0.4));
    }
    if r < 1.6 {
        let inside = smoothstep(1.05, 0.92, r);
        let limbal = smoothstep(0.84, 0.98, r) * smoothstep(1.12, 0.98, r);
        // Iris de gas: ruido en polares que se retuerce y corre hacia el
        // centro; filamentos donde el ruido cruza por cero.
        let flow = r * 2.6 + t * (0.08 + 0.06 * cycle);
        let twist = theta + 0.6 * r + t * 0.05;
        let warp = u.noise.noise(Vec3::new(twist.cos() * 2.0, twist.sin() * 2.0, flow * 0.6));
        let gas = u.noise.fbm(
            Vec3::new(twist.cos() * 4.5, twist.sin() * 4.5, flow + warp * 0.9),
            4,
        );
        let filaments = (1.0 - gas.abs() * 2.2).max(0.0).powi(3);
        let heat = smoothstep(1.0, 0.25, r) * (0.55 + 0.45 * cycle);
        let iris = IRIS_DEEP.lerp(iris_color, (heat * (0.5 + 0.5 * gas) + filaments * 0.6).clamp(0.0, 1.0));
        albedo = albedo.lerp(iris * 0.5, inside * 0.85) * (1.0 - 0.85 * limbal);
        // La pupila: redonda, se dilata con el latido y se cierra con la
        // nave cerca.
        let pupil_radius = (0.27 + 0.06 * u.beat_at(0.0)) * (1.0 - 0.5 * u.eye_near);
        let pupil = smoothstep(pupil_radius + 0.035, pupil_radius - 0.01, r);
        albedo = albedo * (1.0 - 0.97 * pupil);
        // Brilla el gas del iris, mas junto a la pupila, y un borde fino
        // encendido alrededor de ella.
        let rim = smoothstep(pupil_radius + 0.1, pupil_radius + 0.01, r) * (1.0 - pupil);
        glow = iris
            * (inside
                * (1.0 - pupil)
                * (0.12 + 0.5 * heat + 0.6 * filaments)
                * (0.6 + 0.4 * cycle + 0.8 * u.eye_near))
            + iris_color * (rim * (0.6 + 1.2 * u.eye_near));
    }

    // Relampagos chicos en las nubes del vortice, cada uno dura una
    // fraccion de segundo. Con la nave cerca la tormenta se agita: mas
    // relampagos, y mas seguidos.
    let storm = smoothstep(2.8 + 1.5 * u.eye_near, 1.3, r) * smoothstep(0.95, 1.2, r);
    if storm > 0.0 {
        let slot = (t * (2.5 + 4.0 * u.eye_near)).floor();
        let spot = u.noise.noise(p * 26.0 + Vec3::splat(slot * 13.7));
        let gate = u.noise.noise(Vec3::new(slot * 0.71, 3.3, 1.1)) * 0.5 + 0.5;
        let flash = smoothstep(0.5, 0.75, spot) * smoothstep(0.55 - 0.3 * u.eye_near, 0.85, gate) * storm;
        // Detras de las nubes: se ven difusos y tenidos de violeta.
        glow += Vec3::new(0.55, 0.45, 1.0) * (flash * (0.25 + 0.45 * u.eye_near) * (0.4 + 0.6 * lum));
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

    // El lado de noche: el gas hondo brilla a traves de las nubes (las
    // bandas se siguen leyendo) y el iris alumbra el remolino que lo rodea.
    let night = smoothstep(0.2, -0.3, nl);
    let deep = albedo * DEEP_GLOW * (0.085 * night);
    let around = smoothstep(0.85, 1.05, r) * (-(r - 1.0).powi(2) * 0.75).exp();
    let eye_light = albedo * iris_color * (around * (0.2 + 0.8 * night) * (0.5 + 0.5 * cycle) * 1.1);
    glow += deep + eye_light;
    // A contraluz la bruma alta dispersa la estrella hacia adelante: un borde
    // fino que cierra la silueta del lado oscuro.
    let back = (-l).dot(v).max(0.0);
    let backlit = (1.0 - nv).powi(5) * back * back * smoothstep(-0.6, 0.0, nl);
    let rim = Vec3::new(0.7, 0.4, 0.95) * light * (backlit * 0.9);

    lit + rim + glow * u.gains.vigia + limb_haze(f, Vec3::new(0.36, 0.1, 0.5), 0.35, nl)
}
