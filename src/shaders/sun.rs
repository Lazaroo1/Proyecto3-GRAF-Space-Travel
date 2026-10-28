//! Supergigante roja moribunda con un ojo: un remolino de fuego que gira
//! hacia una pupila negra, el borde encendido en llamas y un corazon que
//! late.
//!
//! # El ojo
//! Todo se dibuja en coordenadas polares alrededor del eje que va del centro
//! de la estrella a la camara (`eye_frame`): la pupila siempre queda en el
//! centro del disco y la estrella te mira desde donde estes, como Vigia.
//! El angulo se tuerce mas cerca del centro y gira con el tiempo, mas rapido
//! adentro: el gas se arremolina hacia la pupila. La pupila se dilata con
//! cada latido.
//!
//! # El gas
//! Humo con domain warping en el dominio polar (pocas vueltas a lo largo y
//! muchas a lo ancho: las hebras quedan concentricas), filamentos de fuego
//! donde un ruido cruza por cero y, entre la pupila y el borde, humo claro
//! tenido de lila. El calor sale del radio (oscuro adentro, encendido en el
//! borde) y del gas, y se pinta con una rampa de brasa: negro, granate,
//! rojo, naranja y amarillo blanco.
//!
//! # Borde y llamas
//! El borde es lo mas caliente. `limb_fire` decide cuanto llega cada lengua
//! de fuego segun el angulo; el disco la evalua en el limbo y la corona del
//! cielo (`sky::corona`) por fuera, con el mismo angulo, asi que las llamas
//! siguen de un lado al otro del borde.
//!
//! # Latido y llamaradas
//! Cada latido sale de la pupila como un anillo de luz hacia el borde. En el
//! estertor aparecen manchas que estallan de luz. Si la estrella se calienta
//! al colapsar, la rampa se lleva hacia el color de cuerpo negro.

use super::{Fragment, Uniforms};
use crate::math::blackbody::planckian_rgb;
use crate::math::{Perlin, Vec3, smoothstep};

#[derive(Clone, Copy, Debug)]
pub struct SunStyle {
    /// Temperatura de la fotosfera en kelvin: decide el color.
    pub temperature: f32,
    /// Brillo de la superficie (HDR).
    pub intensity: f32,
}

/// Temperatura de la estrella en la agonia: arriba de esto se calienta.
const CALM_TEMPERATURE: f32 = 3400.0;
/// La rampa de brasa esta en unidades de la intensidad de la superficie.
const DISC_GAIN: f32 = 0.45;
/// Humo claro entre la pupila y el borde.
const LILAC: Vec3 = Vec3::new(0.62, 0.3, 0.42);
/// Brillo morado dentro de la pupila.
const PUPIL_GLINT: Vec3 = Vec3::new(0.004, 0.0015, 0.012);

/// Latido en `[0, 1]` aproximadamente: dos golpes por periodo (lub-dub).
pub fn heartbeat(t: f64, period: f64) -> f64 {
    heartbeat_phase(t / period)
}

/// El mismo latido en la fase `phase`, medida en latidos. El ciclo de OCASO
/// acelera el corazon, asi que la fase es la integral de la frecuencia y no
/// `t / periodo`.
pub fn heartbeat_phase(phase: f64) -> f64 {
    let phase = phase.rem_euclid(1.0);
    let beat = |center: f64, width: f64, height: f64| {
        let x = (phase - center) / width;
        height * (-x * x).exp()
    };
    beat(0.1, 0.045, 1.0) + beat(0.3, 0.06, 0.55)
}

/// Ejes del ojo para la estrella en `sun_pos` (relativo a la camara): el
/// eje que mira a la camara y dos ejes en el plano del disco. Los comparten
/// el disco, la corona y los arcos de plasma.
#[inline]
pub fn eye_frame(sun_pos: Vec3) -> (Vec3, Vec3, Vec3) {
    let axis = (-sun_pos).normalize();
    let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let a = helper.cross(axis).normalize();
    (axis, a, axis.cross(a))
}

/// Fuego del borde en el angulo `theta` alrededor del disco, a la altura
/// `h` sobre el limbo (en radios; 0 en el limbo). Regiones donde el fuego
/// llega lejos que van y vienen; adentro, lenguas delgadas (donde un ruido
/// cruza por cero) que suben, y todas se tuercen al subir.
pub fn limb_fire(theta: f32, h: f32, t: f32, n: &Perlin) -> f32 {
    let theta = theta + 0.9 * h * n.noise(Vec3::new(theta.cos() * 2.5, theta.sin() * 2.5, t * 0.15));
    let (s, c) = theta.sin_cos();
    let big = n.noise(Vec3::new(c * 3.0, s * 3.0, t * 0.05)) * 0.5 + 0.5;
    let lick = (1.0 - n.noise(Vec3::new(c * 11.0, s * 11.0, h * 4.0 - t * 0.6)).abs()).powi(3);
    let fine = n.noise(Vec3::new(c * 30.0, s * 30.0, h * 10.0 - t * 1.2)) * 0.5 + 0.5;
    let reach = 0.02 + 0.3 * big * big * (0.3 + 0.7 * lick) + 0.05 * fine;
    // Hay tramos del borde que arden y otros apenas rojos.
    smoothstep(reach, 0.0, h) * (0.3 + 0.5 * lick + 0.2 * fine) * (0.2 + 0.8 * smoothstep(0.25, 0.75, big))
}

/// Rampa de brasa: calor (0 a ~1.3) a luz HDR.
fn ember(heat: f32) -> Vec3 {
    const STOPS: [(f32, Vec3); 7] = [
        (0.0, Vec3::new(0.0, 0.0, 0.0)),
        (0.18, Vec3::new(0.035, 0.005, 0.004)),
        (0.38, Vec3::new(0.28, 0.03, 0.018)),
        (0.58, Vec3::new(0.8, 0.14, 0.04)),
        (0.78, Vec3::new(1.5, 0.52, 0.12)),
        (1.0, Vec3::new(2.6, 1.5, 0.6)),
        (1.3, Vec3::new(4.2, 3.2, 1.9)),
    ];
    let heat = heat.clamp(0.0, 1.3);
    for pair in STOPS.windows(2) {
        let ((h0, c0), (h1, c1)) = (pair[0], pair[1]);
        if heat <= h1 {
            return c0.lerp(c1, (heat - h0) / (h1 - h0));
        }
    }
    STOPS[6].1
}

pub fn shade(f: &Fragment, u: &Uniforms, s: &SunStyle) -> Vec3 {
    let t = u.time;
    let n = u.noise;
    let (_, a, b) = eye_frame(u.sun_pos);
    let p = f.normal;
    // Radio sobre el disco tal como se ve: 0 en el centro, 1 en el limbo.
    let mu = p.dot(f.view_dir()).clamp(0.0, 1.0);
    let r = (1.0 - mu * mu).sqrt();
    let theta = p.dot(b).atan2(p.dot(a));

    // El remolino: el angulo se tuerce hacia el centro y gira, mas rapido
    // adentro; el humo se muestrea en el disco ya torcido, asi sus manchas
    // se estiran en espiral.
    let inner = (1.0 - r) * (1.0 - r);
    let spun = theta + 1.3 * inner + t * (0.012 + 0.1 * inner);
    let (sn, cs) = spun.sin_cos();
    let q = Vec3::new(cs * r * 3.0, sn * r * 3.0, t * 0.02);
    let warp = Vec3::new(
        n.noise(q * 0.9 + Vec3::new(5.2, 1.3, 0.0)),
        n.noise(q * 0.9 + Vec3::new(-3.7, 8.1, 0.0)),
        0.0,
    );
    // Zonas grandes oscuras y claras, y encima el humo turbulento.
    let large = n.fbm(q + warp * 0.6, 3) * 0.5 + 0.5;
    let detail = n.fbm(q * 2.7 + warp * 1.5 + Vec3::new(0.0, 0.0, t * 0.03), 3) * 0.5 + 0.5;
    let smoke = smoothstep(0.25, 0.8, 0.5 * large + 0.5 * detail);
    // Motas de fuego amarillo hacia el borde.
    let flecks = smoothstep(0.56, 0.82, detail) * smoothstep(0.3, 0.8, r) * smoothstep(0.3, 0.6, large);
    // Vetas que siguen el giro: pocas a lo largo, muchas a lo ancho.
    let polar = Vec3::new(cs * 2.0, sn * 2.0, r * 6.0 - t * 0.03);
    let streak = n.noise(polar + warp * 0.8) * 0.5 + 0.5;
    // Filamentos de fuego donde un ruido cruza por cero, solo en algunas
    // regiones.
    let thread = n.fbm(q * 2.4 + warp * 1.6 + Vec3::new(0.0, 0.0, -t * 0.05), 2);
    let patches = smoothstep(0.0, 0.5, n.noise(q * 0.7 + Vec3::new(9.0, 4.0, t * 0.01)));
    let filaments = (1.0 - thread.abs() * 4.0).max(0.0).powi(3) * patches;

    // La pupila late con el corazon.
    let pupil_radius = 0.13 + 0.035 * u.beat_at(0.0);
    let pupil = smoothstep(pupil_radius + 0.08, pupil_radius - 0.02, r);
    // Oscuro junto a la pupila, encendido hacia el borde.
    let depth = smoothstep(0.08, 0.95, r);
    let limb = smoothstep(0.88, 1.0, r);
    let rim = smoothstep(0.6, 0.97, r);
    // Las llamas solo cuentan junto al borde.
    let fire = if limb > 0.0 { limb_fire(theta, 0.0, t, n) } else { 0.0 };
    let mut heat = depth * (0.13 + 0.36 * smoke + 0.08 * streak)
        + 0.3 * filaments * smoothstep(0.2, 0.6, r)
        + 0.55 * flecks
        + 0.14 * rim
        + limb * (0.3 + 0.45 * fire);

    // El latido sale de la pupila como un anillo de luz.
    let front = u.beat_phase.fract() * 1.4;
    let ring = (-((r - front) * 7.0).powi(2)).exp() * (1.0 - u.beat_phase.fract());
    heat += 0.12 * ring * u.heart;
    // Llamaradas del estertor: parches que estallan de luz.
    if u.flares > 0.0 {
        let burst = n.noise(q * 1.4 + Vec3::new(0.0, t * 0.35, 0.0));
        heat += u.flares * smoothstep(0.25, 0.6, burst) * 0.4;
    }
    let heat = heat * (1.0 - 0.97 * pupil);

    let mut color = ember(heat);
    // Humo claro y lila en el anillo de en medio.
    let veil = smoothstep(0.6, 0.95, smoke) * smoothstep(0.2, 0.45, r) * smoothstep(0.85, 0.55, r);
    color = color.lerp(LILAC * (0.1 + 0.9 * heat), veil * 0.18);
    color += PUPIL_GLINT * (pupil * smoothstep(0.0, 0.6, smoke));
    // Si se calienta al colapsar, el fuego se vuelve de cuerpo negro.
    let hot = smoothstep(CALM_TEMPERATURE * 1.05, CALM_TEMPERATURE * 3.0, s.temperature);
    if hot > 0.0 {
        let glow = planckian_rgb(s.temperature * (0.6 + 0.4 * heat)) * color.luminance();
        color = color.lerp(glow, hot);
    }
    // El estertor la enfria: un poco mas roja.
    let cool = (s.temperature / CALM_TEMPERATURE).min(1.0);
    color = Vec3::new(color.x, color.y * cool, color.z * cool * cool);
    color * (DISC_GAIN * s.intensity * u.gains.sun * (1.0 + 0.16 * u.beat))
}
