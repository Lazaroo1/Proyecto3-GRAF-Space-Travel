//! Supergigante roja moribunda con un ojo: una tormenta en un lugar fijo de
//! su superficie, un remolino de fuego alrededor de una pupila negra, el
//! borde encendido en llamas y un corazon que late.
//!
//! # El ojo
//! El ojo vive en el espacio de la estrella (`EYE_AXIS`) y gira con ella: se
//! ve de frente desde un lado, de canto en el borde y desde atras no se ve.
//! Al empezar la agonia mira hacia Pira, donde arranca la nave. Alrededor
//! del ojo el punto se gira (formula de Rodrigues, como el vortice de Vigia)
//! mas cuanto mas cerca, y todo el patron da vueltas despacio en bloque, asi
//! el remolino gira sin enroscarse sin fin. La pupila se dilata con cada
//! latido; alrededor hay un anillo de humo oscuro y despues uno de fuego,
//! como un iris.
//!
//! # El fuego
//! Dos capas de domain warping que cambian con el tiempo (el fuego hierve):
//! humo de detalle fino sobre regiones grandes que arden o se apagan y,
//! donde arde, motas brillantes y lenguas finas (donde un ruido cruza por
//! cero). El calor se pinta con una rampa de brasa: negro, granate, carmesi,
//! naranja y amarillo blanco. En lo frio hay velos malva y junto al ojo
//! hebras lilas claras.
//!
//! # Borde y llamas
//! `limb_fire` decide cuanto llega cada lengua de fuego segun el angulo
//! alrededor del disco visto desde la camara (`limb_frame`); el disco la
//! evalua en el limbo y la corona del cielo (`sky::corona`) por fuera, con el
//! mismo angulo, asi que las llamas siguen de un lado al otro del borde.
//!
//! # Latido y llamaradas
//! Cada latido sale del ojo como un anillo de luz. En el estertor aparecen
//! manchas que estallan de luz. Si la estrella se calienta al colapsar, la
//! rampa se lleva hacia el color de cuerpo negro.

use super::vigia::rotate;
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
/// Donde esta el ojo, en el espacio de la estrella: a los 40 s de la agonia
/// mira de frente a la vista de detras de Pira (22_agonia).
const EYE_AXIS: Vec3 = Vec3::new(-0.9684, 0.1954, 0.1551);
/// Hebras claras alrededor del ojo y humo frio.
const LILAC: Vec3 = Vec3::new(0.62, 0.3, 0.42);
const VIOLET: Vec3 = Vec3::new(0.36, 0.06, 0.26);
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

/// Ejes del disco visto desde la camara, para la estrella en `sun_pos`
/// (relativo a la camara): el eje que mira a la camara y dos ejes en el
/// plano del disco. Con ellos se mide el angulo alrededor del borde en el
/// disco y en la corona.
#[inline]
pub fn limb_frame(sun_pos: Vec3) -> (Vec3, Vec3, Vec3) {
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
    let reach = 0.03 + 0.42 * big * big * (0.3 + 0.7 * lick) + 0.06 * fine;
    // Hay tramos del borde que arden y otros apenas rojos.
    smoothstep(reach, 0.0, h) * (0.3 + 0.5 * lick + 0.2 * fine) * (0.2 + 0.8 * smoothstep(0.25, 0.75, big))
}

/// Rampa de brasa: calor (0 a ~1.3) a luz HDR.
fn ember(heat: f32) -> Vec3 {
    const STOPS: [(f32, Vec3); 7] = [
        (0.0, Vec3::new(0.0, 0.0, 0.0)),
        (0.18, Vec3::new(0.035, 0.005, 0.004)),
        (0.38, Vec3::new(0.28, 0.03, 0.018)),
        (0.58, Vec3::new(0.66, 0.09, 0.035)),
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
    // La superficie gira con la estrella.
    let p = f.local.normalize();
    // Radio sobre el disco tal como se ve: 0 en el centro, 1 en el limbo.
    let mu = f.normal.dot(f.view_dir()).clamp(0.0, 1.0);
    let r = (1.0 - mu * mu).sqrt();

    // El remolino alrededor del ojo, que gira en bloque.
    let angle = p.dot(EYE_AXIS).clamp(-1.0, 1.0).acos();
    let around = (-(angle / 0.75).powi(2)).exp();
    let q = rotate(p, EYE_AXIS, 0.7 * around + t * 0.035);
    // El fuego hierve: dos capas de domain warping.
    let boil = Vec3::new(0.0, 0.0, t * 0.05);
    let w = q * 2.6 + boil;
    let warp = Vec3::new(
        n.noise(w),
        n.noise(w + Vec3::new(13.1, 0.0, 0.0)),
        n.noise(w + Vec3::new(0.0, 27.3, 0.0)),
    );
    let qq = q * 5.5 + warp * 0.75;
    // Humo de detalle fino sobre regiones grandes que arden o se apagan.
    let smoke = n.fbm(qq + boil * 0.8, 4) * 0.5 + 0.5;
    let mass = n.fbm(q * 2.0 + warp * 0.5 + Vec3::new(4.0, 9.0, t * 0.02), 2) * 0.5 + 0.5;
    let burning = smoothstep(0.38, 0.72, mass);
    // Motas brillantes y lenguas finas (crestas donde el ruido cruza por
    // cero), sobre todo donde arde.
    let fleck = n.fbm(qq * 3.0 + Vec3::new(5.0, 1.0, -t * 0.1), 3) * 0.5 + 0.5;
    let flecks = smoothstep(0.5, 0.76, fleck) * (0.25 + 0.75 * burning);
    let tongues = (1.0 - n.noise(qq * 1.6 + Vec3::new(3.0, 0.0, -t * 0.12)).abs()).powi(6) * burning;

    // La pupila late con el corazon; alrededor, un anillo de humo oscuro y
    // despues uno de fuego, como un iris.
    let pupil_radius = 0.17 + 0.04 * u.beat_at(0.0);
    let pupil = smoothstep(pupil_radius + 0.08, pupil_radius - 0.02, angle);
    let smoky = smoothstep(0.12, 0.45, angle);
    let iris = (-((angle - 0.62) / 0.16).powi(2)).exp();
    // Hacia el borde arde mas; las llamas solo cuentan junto al limbo.
    let rim = smoothstep(0.55, 1.0, r);
    let limb = smoothstep(0.82, 1.0, r);
    let fire = if limb > 0.0 {
        let (_, a, b) = limb_frame(u.sun_pos);
        limb_fire(f.normal.dot(b).atan2(f.normal.dot(a)), 0.0, t, n)
    } else {
        0.0
    };
    let dense = smoothstep(0.32, 0.72, smoke);
    let mut heat = (0.03 + 0.3 * dense * dense + 0.12 * mass + 0.72 * flecks + 0.3 * tongues) * (0.22 + 0.78 * smoky)
        + iris * (0.04 + 0.18 * smoke)
        + rim * (0.02 + 0.12 * mass * mass)
        + limb * (0.2 + 0.55 * fire);

    // El latido sale del ojo como un anillo de luz.
    let front = u.beat_phase.fract() * 1.8;
    let ring = (-((angle - front) * 6.0).powi(2)).exp() * (1.0 - u.beat_phase.fract());
    heat += 0.1 * ring * u.heart;
    // Llamaradas del estertor: parches que estallan de luz.
    if u.flares > 0.0 {
        let burst = n.noise(q * 3.0 + Vec3::new(0.0, t * 0.35, 0.0));
        heat += u.flares * smoothstep(0.25, 0.6, burst) * 0.4;
    }
    let heat = heat * (1.0 - 0.97 * pupil);

    let mut color = ember(heat);
    // Velos violetas entre el fuego: hebras donde un ruido cruza por cero.
    let veil = (1.0 - n.noise(qq * 0.45 + Vec3::new(7.0, 3.0, t * 0.03)).abs()).powi(10);
    color = color.lerp(
        VIOLET * (0.25 + 0.9 * heat),
        veil * smoothstep(0.55, 0.26, heat) * smoothstep(0.3, 0.6, angle) * 0.4,
    );
    // Hebras lilas claras en el humo junto al ojo.
    let wisp = smoothstep(0.62, 0.85, smoke) * smoothstep(0.15, 0.3, angle) * smoothstep(0.55, 0.32, angle);
    color = color.lerp(LILAC * (0.2 + heat), wisp * 0.3);
    color += PUPIL_GLINT * (pupil * smoke);
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
