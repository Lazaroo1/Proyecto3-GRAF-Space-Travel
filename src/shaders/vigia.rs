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
//! - **Ojos chicos**: ocho tormentas que viajan con su banda, cada una con
//!   su remolino. De lejos son una rendija cerrada entre las nubes; con la
//!   nave cerca se abren (una almendra en los ejes de la tormenta) y su
//!   pupila se corre hacia donde esta la nave (`Uniforms::eye_dir`
//!   proyectada en el plano de la tormenta).
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

/// Un ojo chico: latitud y longitud al empezar (radianes), lo que corre en
/// longitud por segundo, su radio angular y el color del iris.
struct Storm {
    lat: f32,
    lon: f32,
    speed: f32,
    size: f32,
    iris: Vec3,
}

const STORMS: [Storm; 8] = [
    Storm { lat: 0.42, lon: 0.6, speed: 0.012, size: 0.12, iris: Vec3::new(0.3, 0.95, 0.75) },
    Storm { lat: -0.55, lon: 2.4, speed: -0.009, size: 0.13, iris: Vec3::new(1.0, 0.55, 0.15) },
    Storm { lat: 0.08, lon: 4.1, speed: 0.016, size: 0.1, iris: Vec3::new(0.95, 0.25, 0.6) },
    Storm { lat: -0.2, lon: 5.4, speed: -0.013, size: 0.11, iris: Vec3::new(0.45, 0.6, 1.0) },
    Storm { lat: 0.78, lon: 3.3, speed: 0.008, size: 0.1, iris: Vec3::new(0.9, 0.9, 0.35) },
    Storm { lat: -0.85, lon: 0.2, speed: 0.01, size: 0.1, iris: Vec3::new(0.35, 1.0, 0.4) },
    Storm { lat: 0.25, lon: 2.0, speed: -0.011, size: 0.12, iris: Vec3::new(1.0, 0.35, 0.3) },
    Storm { lat: -0.4, lon: 3.8, speed: 0.014, size: 0.11, iris: Vec3::new(0.6, 0.4, 1.0) },
];

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Donde esta el ojo chico `s` en `t` (direccion en el espacio de Vigia) y
/// sus ejes: al este y al norte.
#[inline]
fn storm_frame(s: &Storm, t: f32) -> (Vec3, Vec3, Vec3) {
    let lon = s.lon + s.speed * t;
    let c = Vec3::new(s.lat.cos() * lon.cos(), s.lat.sin(), s.lat.cos() * lon.sin());
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
    let mut q = rotate(p, eye, swirl);
    // Las bandas se doblan alrededor del ojo, pero menos que el detalle: con
    // todo el giro el disco entero quedaba como una paleta de dulce.
    let mut qb = rotate(p, eye, swirl * 0.4);
    // Los ojos chicos tambien enroscan las nubes a su alrededor. El ojo
    // grande se los traga si pasa encima.
    let mut near_storm: Option<(usize, f32)> = None;
    let here = p.y.clamp(-1.0, 1.0).asin();
    for (i, s) in STORMS.iter().enumerate() {
        // Primero por la latitud, que no cuesta: casi todos quedan lejos.
        if (here - s.lat).abs() > 3.0 * s.size {
            continue;
        }
        let (c, _, _) = storm_frame(s, t);
        let d = p.dot(c).clamp(-1.0, 1.0).acos() / s.size;
        if d < 3.0 {
            let away = smoothstep(1.4, 2.2, c.dot(eye).clamp(-1.0, 1.0).acos() / EYE_RADIUS);
            let turn = 2.2 * (-d * d * 0.5).exp() * away * s.speed.signum();
            q = rotate(q, c, turn);
            qb = rotate(qb, c, turn);
            if near_storm.is_none_or(|(_, best)| d < best) && away > 0.0 {
                near_storm = Some((i, d));
            }
        }
    }
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

    // Los ojos chicos: una rendija de lejos; con la nave cerca se abren y
    // miran hacia ella.
    if let Some((i, d)) = near_storm.filter(|&(_, d)| d < 1.4) {
        let s = &STORMS[i];
        let (c, east, north) = storm_frame(s, t);
        let (x, y) = (p.dot(east) / s.size, p.dot(north) / s.size);
        let open = 0.12 + 0.88 * smoothstep(0.15, 0.75, u.eye_near);
        // Una almendra: mas abierta en medio.
        let lid = (1.0 - x * x).max(0.0) * open;
        let inside = smoothstep(0.03, -0.03, y.abs() - lid) * smoothstep(1.02, 0.95, x.abs());
        // El borde de los parpados, oscuro.
        let rim = smoothstep(0.12, 0.0, (y.abs() - lid).abs()) * smoothstep(1.1, 0.9, x.abs()) * (1.0 - inside);
        // La pupila se corre hacia la nave.
        let look = eye - c * eye.dot(c);
        let (lx, ly) = (look.dot(east), look.dot(north));
        let reach = (lx * lx + ly * ly).sqrt().max(1e-4);
        let shift = (reach * 4.0).min(1.0) * 0.4 * open;
        let (px, py) = (x - lx / reach * shift, y - ly / reach * shift * open);
        let pr = (px * px + py * py).sqrt();
        let fibers = u.noise.noise(Vec3::new(py.atan2(px) * 3.0, pr * 4.0, i as f32 * 7.0)) * 0.5 + 0.5;
        let iris = s.iris * (0.35 + 0.65 * fibers) * smoothstep(0.75, 0.3, pr);
        let pupil = smoothstep(0.26, 0.2, pr);
        let eyeball = Vec3::new(0.08, 0.03, 0.1).lerp(iris, smoothstep(0.75, 0.65, pr)) * (1.0 - 0.95 * pupil);
        albedo = albedo.lerp(eyeball, inside).lerp(INDIGO, rim * 0.9);
        glow += s.iris * (inside * (1.0 - pupil) * smoothstep(0.7, 0.2, pr) * (0.25 + 0.6 * u.eye_near) * 0.6);
        let _ = d;
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
