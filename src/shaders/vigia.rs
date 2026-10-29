//! Vigia: gigante gaseoso morado con una tormenta que es un ojo y te sigue.
//!
//! - **Bandas**: franjas por latitud, cada una con su color (zonas claras
//!   lila y crema, cinturones indigo, ciruela y magenta) y corriendo a su
//!   ritmo, unas al este y otras al oeste. Sus bordes se ondulan y se
//!   enroscan: la latitud se tuerce con un fbm (turbulencia) y con ondas a
//!   lo largo de la longitud (Kelvin-Helmholtz, como entre las bandas de
//!   Saturno). El mapa del Hubble de Jupiter (NASA) solo da el detalle fino
//!   de las nubes, por su luminancia. Cerca del ojo las bandas se enroscan
//!   hacia el (se calculan en el punto ya girado por el vortice).
//! - **Tormentas electricas**: cinco cumulos de nubes claras y esponjadas en
//!   las bandas de latitudes medias; adentro, rayos que destellan rapido en
//!   puntos que cambian (celdas con su hora). Se ven de dia y mucho mas de
//!   noche.
//! - **Auroras**: un ovalo alrededor de cada polo con cortinas que se mueven
//!   (ruido estirado a lo largo de la longitud), verdes abajo y magenta
//!   arriba; laten con la estrella y se ven sobre todo de noche y en el
//!   borde, donde se miran de lado.
//! - **Relampagos por todo el planeta**: celdas sobre la esfera que destellan
//!   a ratos (cada una a su hora), mas visibles en el lado de noche.
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
use crate::math::{Vec3, hash_u32, smoothstep};

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

/// Colores de las bandas: zonas claras y cinturones oscuros.
const BANDS: [Vec3; 8] = [
    Vec3::new(0.3, 0.16, 0.42),
    Vec3::new(0.03, 0.012, 0.08),
    Vec3::new(0.42, 0.14, 0.4),
    Vec3::new(0.12, 0.03, 0.17),
    Vec3::new(0.46, 0.34, 0.54),
    Vec3::new(0.26, 0.03, 0.24),
    Vec3::new(0.07, 0.05, 0.22),
    Vec3::new(0.22, 0.07, 0.34),
];
/// Bandas por radian de latitud.
const BAND_DENSITY: f32 = 11.0;

/// Las tormentas electricas: latitud, longitud al empezar, lo que corren
/// por segundo y su radio angular.
const THUNDER: [(f32, f32, f32, f32); 5] = [
    (0.6, 1.2, -0.01, 0.07),
    (-0.32, 2.9, 0.012, 0.06),
    (0.33, 4.6, -0.008, 0.065),
    (-0.68, 5.6, 0.009, 0.055),
    (0.16, 0.1, 0.011, 0.06),
];

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Donde esta en `t` algo que va a la latitud `lat` y corre en longitud
/// desde `lon` a `speed` por segundo (direccion en el espacio de Vigia), y
/// sus ejes: al este y al norte.
#[inline]
fn drifting(lat: f32, lon: f32, speed: f32, t: f32) -> (Vec3, Vec3, Vec3) {
    let lon = lon + speed * t;
    let c = Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
    let east = Vec3::Y.cross(c).normalize();
    (c, east, c.cross(east))
}

/// Gira `p` un angulo `angle` alrededor del eje unitario `axis` (Rodrigues).
#[inline]
pub(super) fn rotate(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
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

/// El color de las bandas en `qb` (el punto ya doblado por los vortices):
/// la latitud se tuerce con turbulencia y con ondas a lo largo de la
/// longitud; cada banda tiene su color y su corriente. `lum` es el detalle
/// de las nubes del mapa. Debajo del iris del ojo grande no se calcula: no
/// se ve.
fn bands(qb: Vec3, t: f32, lum: f32, footprint: f32, u: &Uniforms) -> Vec3 {
    let lat = qb.y.clamp(-1.0, 1.0).asin();
    let lon = qb.z.atan2(qb.x);
    let curl = u.noise.fbm(qb * 3.2 + Vec3::new(0.0, t * 0.004, 0.0), 2) + 0.35 * u.noise.noise(qb * 11.0);
    let rough = lat * BAND_DENSITY + 0.55 * curl;
    let k = rough.floor();
    let id = hash_u32(k as i32 as u32 ^ 0xBA2D);
    let current = (unit(id) - 0.5) * 0.06;
    let phase = lon * (8.0 + 6.0 * unit(id >> 4)) + t * current + k * 1.7;
    // Ondas con la cresta enroscada (dos armonicos), como entre las bandas
    // de Saturno.
    let wave = phase.sin() + 0.45 * (2.0 * phase + 0.8).sin();
    let y = rough + 0.18 * wave;
    let k = y.floor();
    let pick = |k: f32| BANDS[(hash_u32(k as i32 as u32 ^ 0x51E) % BANDS.len() as u32) as usize];
    let edge = smoothstep(0.78, 0.98, y - k);
    let band = pick(k).lerp(pick(k + 1.0), edge);
    // Remolinos chicos dentro de cada banda (solo cuando se alcanzan a ver).
    let fine = smoothstep(0.02, 0.005, footprint);
    let eddies = if fine > 0.0 { u.noise.fbm(qb * 26.0 + Vec3::splat(k * 0.37), 2) * fine } else { 0.0 };
    band * (0.4 + 1.15 * lum) * (0.85 + 0.45 * eddies)
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
    // Las bandas se doblan alrededor del ojo, pero menos que el detalle: con
    // todo el giro el disco entero quedaba como una paleta de dulce.
    let qb = rotate(p, eye, swirl * 0.4);
    let here = p.y.clamp(-1.0, 1.0).asin();
    // Rotacion diferencial: cada banda corre a su ritmo.
    let drift = t * 0.003 * (q.y * 9.0).sin();
    let (s, c) = drift.sin_cos();
    let q = Vec3::new(q.x * c - q.z * s, q.y, q.x * s + q.z * c);
    let footprint = f.pos.length() * u.pixel_angle / f.local.length().max(1.0) * 0.3;
    let clouds = u.textures.images.jupiter.sphere(q, footprint);
    let lum = smoothstep(0.03, 0.8, clouds.luminance()).powf(1.5);
    // Los cinturones rojizos de la foto quedan magenta.
    let warm = smoothstep(0.0, 0.08, clouds.x - clouds.z);
    let old = palette(lum).lerp(ORCHID * (0.5 + lum), warm * 0.35);
    let mut albedo = if r < 0.88 { old } else { bands(qb, t, lum, footprint, u).lerp(old, 0.3) };
    let dark = smoothstep(0.2, -0.3, f.normal.dot(f.light_dir(u)));

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

    // Las tormentas electricas: cumulos claros con rayos adentro.
    for (k, &(lat, lon, speed, size)) in THUNDER.iter().enumerate() {
        if (here - lat).abs() > 1.6 * size {
            continue;
        }
        let (c, east, north) = drifting(lat, lon, speed, t);
        let d = p.dot(c).clamp(-1.0, 1.0).acos() / size;
        if d > 1.6 {
            continue;
        }
        // Copos esponjados: mas claros arriba de cada uno.
        let puffs = u.noise.fbm(p * 70.0 + Vec3::splat(k as f32 * 9.1), 3) * 0.5 + 0.5;
        let cloud = smoothstep(1.25, 0.55, d + 0.5 * (puffs - 0.5));
        albedo = albedo.lerp(Vec3::new(0.62, 0.56, 0.74) * (0.65 + 0.6 * puffs), cloud * 0.85);
        // Rayos: celdas dentro de la tormenta, cada una con su hora.
        let (gx, gy) = (p.dot(east) / size * 2.5, p.dot(north) / size * 2.5);
        let (fx, fy) = (gx.floor(), gy.floor());
        let id = hash_u32((fx as i32 as u32).wrapping_mul(0x8DA6_B343) ^ (fy as i32 as u32).wrapping_mul(0xD816_3841) ^ k as u32);
        let clock = t * 5.0 + unit(id) * 13.0;
        let slot = clock.floor();
        if unit(hash_u32(id ^ slot as i32 as u32)) < 0.22 {
            let (ox, oy) = (0.3 + 0.4 * unit(id >> 5), 0.3 + 0.4 * unit(id >> 11));
            let spot = (gx - fx - ox).powi(2) + (gy - fy - oy).powi(2);
            let age = clock - slot;
            let flicker = (1.0 - age).max(0.0).powi(2) * (0.5 + 0.5 * (age * 47.0).sin().abs());
            let flash = (-spot * 30.0).exp() * flicker * cloud;
            glow += Vec3::new(0.75, 0.8, 1.0) * (flash * (0.5 + 2.5 * dark));
        }
        // A ratos un rayo grande alumbra toda la nube desde adentro.
        let beat = t * 2.3 + k as f32 * 3.7;
        if unit(hash_u32(k as u32 ^ 0x7A11 ^ beat.floor() as i32 as u32)) < 0.3 {
            let age = beat - beat.floor();
            let flicker = (1.0 - age).max(0.0).powi(3) * (0.6 + 0.4 * (age * 31.0).sin().abs());
            glow += Vec3::new(0.6, 0.62, 0.95) * (cloud * puffs * flicker * (0.15 + 0.9 * dark));
        }
    }

    // Relampagos chicos en las nubes del vortice, cada uno dura una
    // fraccion de segundo. Con la nave cerca la tormenta se agita: mas
    // relampagos, y mas seguidos.
    let storm = smoothstep(2.8 + 1.5 * u.eye_near, 1.3, r) * smoothstep(0.95, 1.2, r);
    if storm > 0.0 {
        let slot = (t * (2.5 + 4.0 * u.eye_near)).floor();
        // Manchas grandes y suaves: con ruido fino y un umbral duro salian
        // esquirlas blancas que parecian pedazos de algo.
        let spot = u.noise.noise(p * 9.0 + Vec3::splat(slot * 13.7));
        let gate = u.noise.noise(Vec3::new(slot * 0.71, 3.3, 1.1)) * 0.5 + 0.5;
        let flash = smoothstep(0.25, 0.85, spot).powi(2) * smoothstep(0.55 - 0.3 * u.eye_near, 0.85, gate) * storm;
        // Detras de las nubes: se ven difusos y tenidos de violeta, mas de
        // noche.
        glow += Vec3::new(0.55, 0.45, 1.0) * (flash * (0.12 + 0.3 * u.eye_near) * (0.3 + 0.7 * dark) * (0.4 + 0.6 * lum));
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

    // Auroras: una cinta que serpentea alrededor de cada polo, con pliegues
    // finos como cortinas; verde abajo (hacia el polo) y magenta arriba.
    let colat = p.y.abs().clamp(-1.0, 1.0).acos();
    if (0.18..0.52).contains(&colat) {
        let plon = p.z.atan2(p.x);
        let pole = p.y.signum();
        let center = 0.34 + 0.045 * (plon * 3.0 + t * 0.15 + pole).sin() + 0.02 * u.noise.noise(Vec3::new(plon * 2.0, t * 0.1, pole));
        let across = (colat - center) / 0.045;
        let ribbon = (-across * across).exp();
        if ribbon > 0.01 {
            let folds = u.noise.noise(Vec3::new(plon * 38.0 + pole * 5.0, t * 0.6, 1.3)) * 0.5 + 0.5;
            let curtain = 0.35 + 0.65 * smoothstep(0.3, 0.75, folds);
            let color = Vec3::new(0.15, 1.0, 0.45).lerp(Vec3::new(0.9, 0.2, 0.85), smoothstep(-1.0, 1.2, across));
            let pulse = 0.55 + 0.9 * u.beat;
            let grazing = 1.0 + 2.5 * (1.0 - nv).powi(3);
            glow += color * (ribbon * curtain * pulse * grazing * (0.12 + 0.88 * night) * 0.6);
        }
    }
    // Relampagos por todo el planeta: celdas que destellan a ratos.
    {
        let g = p * 9.0;
        let cell = g.floor();
        let id = hash_u32(
            (cell.x as i32 as u32).wrapping_mul(0x8DA6_B343)
                ^ (cell.y as i32 as u32).wrapping_mul(0xD816_3841)
                ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F),
        );
        let clock = t * 1.7 + unit(id) * 40.0;
        let slot = clock.floor();
        if unit(hash_u32(id ^ slot as i32 as u32)) < 0.07 {
            let center = Vec3::new(unit(id >> 3), unit(id >> 9), unit(id >> 15)) * 0.6 + Vec3::splat(0.2);
            let spot = (g - cell - center).length_squared();
            let age = clock - slot;
            let flicker = (1.0 - age).max(0.0).powi(3) * (0.6 + 0.4 * (age * 60.0).sin().abs());
            let flash = (-spot * 14.0).exp() * flicker;
            glow += Vec3::new(0.6, 0.55, 1.0) * (flash * (0.06 + 0.9 * night) * (0.4 + 0.6 * lum) * 0.8);
        }
    }
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
