//! Faro: la fisica de una estrella de neutrones que es pulsar y magnetar.
//!
//! Faro esta hecho con los numeros de 1E 1547.0-5408, el magnetar que tambien
//! emite pulsos de radio (Camilo et al. 2007; Kaspi y Beloborodov 2017,
//! tabla 1): gira cada 2.07 s y su campo dipolar es de 3.2e14 G. Todo lo
//! demas sale de las ecuaciones, en cgs:
//!
//! ```text
//!   rs   = 2GM/c^2                     radio de Schwarzschild
//!   B    = 3.2e19 sqrt(P Pdot) G       frenado de dipolo  ->  Pdot
//!   tau  = P / (2 Pdot)                edad caracteristica
//!   Edot = 4 pi^2 I Pdot / P^3         energia de rotacion que pierde
//!   R_LC = c P / (2 pi)                cilindro de luz
//!   theta_pc = asin(sqrt(R / R_LC))    casquete de lineas abiertas
//! ```
//!
//! Se dibuja con `SCALE` unidades por km (1 unidad = 500 m): Faro mide 24
//! unidades de radio y sus haces nacen a 240-430 unidades. Las proporciones
//! de la estrella, sus casquetes, su magnetosfera y sus haces son las reales.
//! El cilindro de luz (casi 100 000 km) queda fuera del sistema.
//!
//! El tiempo tambien es el real: el giro, los estallidos de milisegundos y
//! la cola de seis minutos de la gran llamarada.

use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

use super::Body;
use crate::math::{DQuat, DVec3, Vec3};
use crate::render::{GlowLine, Particle};

/// Constantes fisicas en cgs.
const C: f64 = 2.997_924_58e10;
const G: f64 = 6.674_3e-8;
const M_SUN: f64 = 1.988_47e33;
/// Momento de inercia de una estrella de neutrones (g cm^2).
const INERTIA: f64 = 1e45;
const YEAR: f64 = 3.155_76e7;

/// Masa (masas solares), radio (km), periodo (s) y campo dipolar (G).
pub const MASS: f64 = 1.4;
pub const RADIUS_KM: f64 = 12.0;
/// Unidades del juego por km al dibujar a Faro, y su radio en unidades.
pub const SCALE: f64 = 2.0;
pub const RADIUS: f64 = RADIUS_KM * SCALE;
pub const PERIOD: f64 = 2.0698;
pub const DIPOLE_FIELD: f64 = 3.2e14;
/// Inclinacion del eje magnetico respecto al de giro: casi ortogonal, asi
/// que los dos haces barren el plano del sistema (pulso e interpulso).
pub const INCLINATION: f64 = 82.0 * PI / 180.0;

/// Conductividad termica de la corteza transversal al campo sobre la
/// paralela: con B > 1e10 G es mucho menor que 1 (Shibanov y Yakovlev 1996).
pub const CONDUCTIVITY_RATIO: f64 = 0.05;
/// Temperatura del polo de la corteza: una estrella de neutrones de mil
/// anos tiene unos 1e6 K (Yakovlev y Pethick 2004); el campo la calienta.
/// La luz termica del magnetar (kT ~ 0.4 keV) sale sobre todo de los
/// casquetes del haz de corriente.
const CRUST_POLE: f64 = 1.8e6;
/// Casquete de lineas abiertas, calentado por las particulas que vuelven.
const CAP_TEMPERATURE: f64 = 3.2e6;
/// Brillo HDR de la corteza (en unidades de la temperatura del polo).
pub const SURFACE_GAIN: f64 = 1.0;
/// Chispas del carrusel y vueltas de la estrella por cada vuelta de ellas
/// (las de PSR B0943+10: 20 chispas y 37.4 P).
pub const SPARKS: u32 = 20;
pub const CAROUSEL_TURNS: f64 = 37.4;
/// Brillo de los haces en el viento y del destello directo.
pub const BEAM_GAIN: f64 = 1500.0;
/// Brillo de la bruma del viento alrededor de la estrella.
pub const HALO_GAIN: f64 = 0.4;
pub const GLARE_GAIN: f64 = 8.0;
/// Voltaje de la descarga en el haz de corriente, en 1e9 V (Beloborodov
/// 2009: 1e9 a 1e10 V).
const DISCHARGE_GV: f64 = 3.0;

/// Radio de Schwarzschild en unidades del juego.
pub fn rs() -> f64 {
    model().rs_km * SCALE
}

/// Radio de la esfera de apoyo para una camara a `distance` unidades: el
/// del rayo rasante, `b_c sqrt(1 - rs/d)` con `b_c = R / sqrt(1 - rs/R)`.
pub fn apparent_radius(distance: f64) -> f64 {
    let critical = RADIUS / model().surface_lapse;
    critical * (1.0 - rs() / distance.max(RADIUS)).max(0.0).sqrt() * 1.003
}

/// Lo que cambia con el tiempo.
#[derive(Clone, Copy, Debug)]
pub struct State {
    /// Temperaturas (K) del polo de la corteza y del casquete abierto.
    pub pole_temperature: f64,
    pub cap_temperature: f64,
    /// Torsion de la magnetosfera (rad entre los dos pies de una linea).
    pub twist: f64,
    /// Tamano del haz de corriente: `u = sin^2` del angulo de sus pies.
    pub bundle: f64,
    /// Casquete del haz de corriente: radio angular y temperatura.
    pub spot_radius: f64,
    pub spot_temperature: f64,
    /// Brillo extra de la superficie.
    pub flash: f64,
    /// Brillo de los haces de radio.
    pub radio: f64,
    /// Luz de los estallidos en la magnetosfera y azimut magnetico de las
    /// lineas donde ocurre el mas fuerte.
    pub burst: f64,
    pub burst_site: f64,
    /// Actividad de una tormenta de estallidos (0 a 1).
    pub storm: f64,
    /// La gran llamarada: el pico (0 a 1), las lineas abiertas (0 a 1,
    /// mientras se reconectan) y la bola de fuego atrapada (su luminosidad
    /// relativa).
    pub spike: f64,
    pub open: f64,
    pub fireball: f64,
    /// Segundos desde el pico de la gran llamarada, si ya paso.
    pub since_spike: Option<f64>,
}

/// El casquete caliente donde el haz de corriente (j-bundle) toca la
/// estrella (Beloborodov 2009): `u = sin^2 theta`, area `A = pi R^2 u` en
/// cada polo y luminosidad `L = 1.3e36 B14 R6 psi V9 u^2 erg/s`, con el
/// voltaje de descarga `V9` en 1e9 V. Devuelve el radio angular y la
/// temperatura de cuerpo negro que da `L = 2 A sigma T^4`.
pub fn hot_spot(u: f64, twist: f64) -> (f64, f64) {
    const SIGMA: f64 = 5.670_374e-5;
    let r6 = RADIUS_KM * 1e5 / 1e6;
    let luminosity = 1.3e36 * (DIPOLE_FIELD / 1e14) * r6 * twist * DISCHARGE_GV * u * u;
    let area = PI * (RADIUS_KM * 1e5).powi(2) * u;
    (u.sqrt().asin(), (luminosity / (2.0 * area * SIGMA)).powf(0.25))
}

/// Torsion de reposo: la corteza la sube y la resistencia de la descarga la
/// baja; anda alrededor de 1 rad.
const QUIET_TWIST: f64 = 1.0;
/// Tamano del haz de corriente en reposo (`u`, 10 grados, como XTE J1810-197).
const QUIET_BUNDLE: f64 = 0.03;
/// La gran llamarada: la corteza tuerce la magnetosfera hasta que pierde el
/// equilibrio a unos 3 rad (Parfrey, Beloborodov y Hui 2013). La torsion de
/// verdad se acumula en anos; aqui, en `WIND_UP` segundos.
pub const CRITICAL_TWIST: f64 = 3.0;
pub const WIND_UP: f64 = 8.0;
/// Un estallido precursor antes del pico (el de SGR 1806-20 fue 142 s
/// antes; aqui, 2 s).
const PRECURSOR: f64 = 6.0;
/// El pico duro dura 0.2 s; la bola de fuego se evapora en 380 s con
/// `L = L0 (1 - t / t_evap)^3` (Feroci et al. 2001).
pub const SPIKE: f64 = 0.2;
pub const EVAPORATION: f64 = 380.0;
/// El giro se frena de golpe con la llamarada (antiglitch): `dP/P = 1e-4`,
/// como en la de SGR 1900+14 (Woods et al. 1999).
pub const ANTIGLITCH: f64 = 1e-4;
/// Despues de la llamarada la torsion vuelve a crecer (de verdad, en anos).
const REGROWTH: f64 = 90.0;

/// Un numero en [0, 1) por entero y semilla.
fn uniform(n: i64, seed: u32) -> f64 {
    let low = (n as u32) ^ seed.wrapping_mul(0x9E37_79B9);
    let high = ((n >> 32) as u32).wrapping_mul(0x85EB_CA6B);
    crate::math::hash_u32(low ^ high) as f64 / 4_294_967_296.0
}

/// Actividad de las tormentas de estallidos: cada 90 s puede empezar una
/// que dura de 15 a 25 s (los magnetares disparan sus estallidos en grupos).
fn storm_at(t: f64) -> f64 {
    let window = 90.0;
    let w = (t / window).floor() as i64;
    let mut level: f64 = 0.0;
    for k in [w - 1, w] {
        if uniform(k, 0x5707) > 0.4 {
            continue;
        }
        let begin = k as f64 * window + 10.0 + 50.0 * uniform(k, 0xB361);
        let length = 15.0 + 10.0 * uniform(k, 0x1E47);
        let x = t - begin;
        if (0.0..length).contains(&x) {
            let ramp = (x / 2.0).min((length - x) / 4.0).min(1.0);
            level = level.max(ramp * ramp * (3.0 - 2.0 * ramp));
        }
    }
    level
}

/// Los estallidos cortos: un proceso de Poisson con la tasa del momento,
/// determinista (cada ranura de 50 ms decide con un hash), asi que tambien
/// se rebobina. Energias con `dN/dE ~ E^-1.7` (Gogus et al. 1999) y
/// duraciones lognormales alrededor de 100 ms. Devuelve la luz de los que
/// suenan en `t` (en energias minimas) y el azimut del mas fuerte.
fn bursts_at(t: f64, rate: impl Fn(f64) -> f64) -> (f64, f64) {
    const SLOT: f64 = 0.05;
    let now = (t / SLOT).floor() as i64;
    let (mut light, mut best, mut site) = (0.0, 0.0, 0.0);
    for k in (now - 40)..=now {
        let begin = k as f64 * SLOT;
        if uniform(k, 0xB0B0) >= rate(begin) * SLOT {
            continue;
        }
        let age = t - (begin + SLOT * uniform(k, 0x7111));
        if age < 0.0 {
            continue;
        }
        // N(>E) ~ E^-0.7: E = U^(-1/0.7), cortada a mil veces la minima.
        let energy = uniform(k, 0xE4E4).max(1e-3).powf(-1.0 / 0.7).min(1000.0);
        // Lognormal: la suma de tres uniformes ya se parece a una normal.
        let normal = (uniform(k, 1) + uniform(k, 2) + uniform(k, 3) - 1.5) * 2.0;
        let length = 0.1 * (0.6 * normal).exp();
        // Sube en 5 ms y se apaga exponencial.
        let value = energy * (age / 0.005).min(1.0) * (-age / (0.35 * length)).exp();
        light += value;
        if value > best {
            best = value;
            site = TAU * uniform(k, 0x517E);
        }
    }
    (light, site)
}

/// Faro en el instante `t`; `flare` es cuando se pidio la gran llamarada.
pub fn state(t: f64, flare: Option<f64>) -> State {
    // Torsion de reposo con un vaiven lento.
    let mut twist = QUIET_TWIST + 0.12 * (TAU * t / 140.0).sin() + 0.05 * (TAU * t / 37.0).sin();
    let mut bundle = QUIET_BUNDLE * (twist / QUIET_TWIST);
    let storm = storm_at(t);
    let (mut spike, mut open, mut fireball, mut flash, mut radio) = (0.0, 0.0, 0.0, 0.0, 1.0);
    let mut since_spike = None;
    let mut extra_rate = 0.0;
    if let Some(start) = flare.filter(|&f| t >= f) {
        let age = t - start;
        if age < WIND_UP {
            // La corteza acelera: la torsion sube cada vez mas rapido.
            let x = age / WIND_UP;
            twist += (CRITICAL_TWIST - twist) * x * x;
            bundle = QUIET_BUNDLE * (1.0 + 2.0 * x * x);
            extra_rate = 6.0 * x;
            let precursor = age - PRECURSOR;
            if precursor >= 0.0 {
                flash += 3.0 * (precursor / 0.01).min(1.0) * (-precursor / 0.4).exp();
            }
        } else {
            let after = age - WIND_UP;
            since_spike = Some(after);
            // El pico sube en 5 ms y cae en su fraccion de segundo.
            spike = (after / 0.005).min(1.0) * (-after / (SPIKE * 0.4)).exp();
            flash += 8.0 * spike;
            // Las lineas se abren casi a la velocidad de la luz y se cierran
            // al reconectarse: aqui, mientras dura el pico.
            open = if after < SPIKE { 1.0 } else { (-(after - SPIKE) / 0.6).exp() };
            if after < EVAPORATION {
                fireball = (1.0 - after / EVAPORATION).powi(3);
            }
            // Todo queda casi sin torsion y se vuelve a torcer; el casquete
            // del haz de corriente se agranda y se encoge (Beloborodov 2009:
            // `u` decrece linealmente mientras se destuerce).
            let regrow = 1.0 - (-after / REGROWTH).exp();
            twist = 0.3 + (twist - 0.3) * regrow;
            bundle = QUIET_BUNDLE + 0.07 * (1.0 - after / 240.0).max(0.0);
            flash += 0.8 * (-after / 60.0).exp();
            extra_rate = 1.5 * (-after / 120.0).exp();
            // La radio se apaga en la llamarada y vuelve despues (como en
            // PSR J1119-6127 en 2016).
            let back = ((after - 20.0) / 60.0).clamp(0.0, 1.0);
            radio = back * back * (3.0 - 2.0 * back);
        }
    }
    let rate = |at: f64| 0.04 + 3.0 * storm_at(at) + extra_rate;
    let (burst, burst_site) = bursts_at(t, rate);
    flash += 0.02 * burst;
    let (spot_radius, spot_temperature) = hot_spot(bundle, twist);
    State {
        pole_temperature: CRUST_POLE,
        cap_temperature: CAP_TEMPERATURE,
        twist,
        bundle,
        spot_radius,
        spot_temperature,
        flash,
        radio,
        burst,
        burst_site,
        storm,
        spike,
        open,
        fireball,
        since_spike,
    }
}

/// Fase del giro en vueltas, con el frenado de la llamarada: despues del
/// pico el periodo es `P (1 + dP/P)`.
pub fn turn_at(body: &Body, t: f64, flare: Option<f64>) -> f64 {
    let base = body.spin_phase / TAU;
    match flare.map(|f| f + WIND_UP).filter(|&spike| t > spike) {
        Some(spike) => base + spike / PERIOD + (t - spike) / period_at(t, flare),
        None => base + t / PERIOD,
    }
}

/// El periodo en el instante `t`: despues del pico de la llamarada, mas largo.
pub fn period_at(t: f64, flare: Option<f64>) -> f64 {
    match flare.map(|f| f + WIND_UP).filter(|&spike| t > spike) {
        Some(_) => PERIOD * (1.0 + ANTIGLITCH),
        None => PERIOD,
    }
}

/// Orientacion de la corteza: el eje inclinado del cuerpo y el giro.
pub fn crust_at(body: &Body, t: f64, flare: Option<f64>) -> DQuat {
    let spin = TAU * turn_at(body, t, flare);
    DQuat::from_axis_angle(DVec3::Z, body.axial_tilt) * DQuat::from_axis_angle(DVec3::Y, spin)
}

/// Lo que se deriva de las constantes de arriba.
#[derive(Clone, Copy, Debug)]
pub struct Model {
    /// Radio de Schwarzschild y compacidad `rs / R`.
    pub rs_km: f64,
    pub compactness: f64,
    pub pdot: f64,
    /// Edad caracteristica en anos.
    pub age_years: f64,
    pub edot: f64,
    pub light_cylinder_km: f64,
    /// Angulo del casquete polar (rad).
    pub polar_cap: f64,
    /// Haces de Rankin a 1 GHz (rad): ancho del nucleo y radios de los
    /// conos interior y exterior.
    pub core: f64,
    pub inner_cone: f64,
    pub outer_cone: f64,
    /// Altura de emision de cada cono (km): donde la tangente a la ultima
    /// linea abierta forma ese angulo, `rho = (3/2) sqrt(r / R_LC)`.
    pub inner_height_km: f64,
    /// Corrimiento gravitacional en la superficie, `sqrt(1 - rs/R)`.
    pub surface_lapse: f64,
}

pub fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| {
        let rs_cm = 2.0 * G * MASS * M_SUN / (C * C);
        let rs_km = rs_cm / 1e5;
        let compactness = rs_km / RADIUS_KM;
        let pdot = (DIPOLE_FIELD / 3.2e19).powi(2) / PERIOD;
        let light_cylinder_km = C * PERIOD / TAU / 1e5;
        // Rankin (1990, 1993): nucleo de 2.45 grados P^-1/2 y conos de 4.33
        // y 5.75 grados P^-1/2.
        let scale = PERIOD.powf(-0.5) * PI / 180.0;
        let (inner_cone, outer_cone) = (4.33 * scale, 5.75 * scale);
        let height = |rho: f64| light_cylinder_km * (rho / 1.5).powi(2);
        Model {
            rs_km,
            compactness,
            pdot,
            age_years: PERIOD / (2.0 * pdot) / YEAR,
            edot: 4.0 * PI * PI * INERTIA * pdot / PERIOD.powi(3),
            light_cylinder_km,
            polar_cap: (RADIUS_KM / light_cylinder_km).sqrt().asin(),
            core: 2.45 * scale,
            inner_cone,
            outer_cone,
            inner_height_km: height(inner_cone),
            surface_lapse: (1.0 - compactness).sqrt(),
        }
    })
}

/// Campo dipolar que dice el frenado: `B = 3.2e19 sqrt(P Pdot)`.
pub fn braking_field(period: f64, pdot: f64) -> f64 {
    3.2e19 * (period * pdot).sqrt()
}

/// El eje magnetico en el marco de la corteza: inclinado `INCLINATION` del
/// eje de giro (+Y).
pub fn magnetic_local() -> DVec3 {
    DVec3::new(INCLINATION.sin(), INCLINATION.cos(), 0.0)
}

/// Ejes del marco magnetico en la corteza: el polo norte magnetico `m` y
/// dos ejes perpendiculares (el primero en el plano del giro).
pub fn magnetic_frame() -> (DVec3, DVec3, DVec3) {
    let m = magnetic_local();
    let e1 = DVec3::new(INCLINATION.cos(), -INCLINATION.sin(), 0.0);
    (m, e1, m.cross(e1))
}

/// Una familia de lineas que se dibuja: cuantas alrededor del eje, entre que
/// colatitudes nacen (en grados), hasta que radio se siguen (las abiertas se
/// cortan), cuantos puntos y que tan brillantes.
struct LineSet {
    azimuths: usize,
    feet: usize,
    from_deg: f64,
    to_deg: f64,
    reach: f64,
    samples: usize,
    gain: f32,
}

/// Los arcos cerrados junto a la estrella, el abanico de los polos (lineas
/// que suben hacia el haz) y los arcos grandes del haz de corriente.
const LINE_SETS: [LineSet; 3] = [
    LineSet { azimuths: 32, feet: 3, from_deg: 16.0, to_deg: 42.0, reach: f64::INFINITY, samples: 56, gain: 1.0 },
    LineSet { azimuths: 28, feet: 2, from_deg: 3.0, to_deg: 9.0, reach: 9.0, samples: 28, gain: 0.9 },
    // (Sus colatitudes salen del casquete del momento.)
    LineSet { azimuths: 18, feet: 1, from_deg: 0.0, to_deg: 0.0, reach: f64::INFINITY, samples: 72, gain: 0.35 },
];
/// Brillo de las lineas y de la bola de fuego.
const PLASMA_GAIN: f32 = 0.8;
const FIREBALL_GAIN: f32 = 150.0;
/// Particulas de la bola de fuego.
const FIREBALL_COUNT: usize = 2400;

/// Color del plasma a `r` radios de la estrella: blanco azulado junto a ella
/// (donde los pares son densos y calientes), azul, violeta y morado afuera.
fn plasma_tint(r: f32) -> Vec3 {
    let stops = [
        (1.0, Vec3::new(1.0, 1.45, 3.2)),
        (2.0, Vec3::new(0.3, 0.75, 3.0)),
        (4.5, Vec3::new(0.35, 0.45, 2.6)),
        (9.0, Vec3::new(0.65, 0.25, 1.9)),
        (30.0, Vec3::new(0.5, 0.15, 1.1)),
    ];
    let mut color = stops[0].1;
    for w in stops.windows(2) {
        if r >= w[0].0 {
            let k = ((r - w[0].0) / (w[1].0 - w[0].0)).min(1.0);
            color = w[0].1.lerp(w[1].1, k);
        }
    }
    color
}

/// Las lineas de campo de la magnetosfera torcida, como hilos de luz
/// relativos a la camara.
///
/// La forma es la de Thompson, Lyutikov y Kulkarni (2002) con la torsion del
/// momento. Brillan donde hay plasma: junto a la estrella los pares e± son
/// densos y calientes (donde `B > 1e13 G`, a menos de unos 3 radios), y en
/// las lineas que llevan corriente (el haz de corriente de Beloborodov 2009)
/// suben frenandose y se aniquilan en la cima (Beloborodov 2013). Los
/// paquetes que suben se muestran diez mil veces mas lentos que la luz; la
/// descarga parpadea como un relampago continuo (Beloborodov y Thompson
/// 2007). `density` baja el brillo cuando la estrella se ve chica y las
/// lineas se enciman.
pub fn field_lines(out: &mut Vec<GlowLine>, body: &Body, t: f64, flare: Option<f64>, camera: DVec3, density: f32) {
    let s = state(t, flare);
    let crust = crust_at(body, t, flare);
    let center = body.position(t) - camera;
    let (m, e1, e2) = magnetic_frame();
    // Del marco magnetico (z sobre el eje) al mundo relativo a la camara, en km.
    let to_world = |p: DVec3| -> Vec3 {
        (center + crust.rotate((e1 * p.x + e2 * p.y + m * p.z) * RADIUS)).as_vec3()
    };
    // Mientras las lineas estan abiertas se estiran hacia afuera.
    let inflate = 1.0 + 25.0 * s.open;
    let fade = (1.0 - 0.85 * s.open) as f32;
    let activity = (s.twist / QUIET_TWIST).max(0.3).sqrt() * (1.0 + 0.6 * s.storm);
    let mut points = Vec::with_capacity(80);
    let mut colors = Vec::with_capacity(80);
    for (set_index, set) in LINE_SETS.iter().enumerate() {
        for i in 0..set.azimuths {
            for j in 0..set.feet {
                let line = ((set_index * 1000 + i * set.feet + j) as i64) << 8;
                let phi_f = TAU * (i as f64 + 0.6 * uniform(line, 0xA21)) / set.azimuths as f64;
                let spread = (j as f64 + uniform(line, 0x7E7)) / set.feet as f64;
                // Los arcos del haz de corriente nacen en la orilla de su
                // casquete, que crece y se encoge con la torsion.
                let (from, to) = if set_index == 2 {
                    let edge = s.bundle.sqrt().asin();
                    (edge * 0.9, edge)
                } else {
                    (set.from_deg.to_radians(), set.to_deg.to_radians())
                };
                let theta_f = from + (to - from) * spread;
                // La descarga parpadea; las lineas del estallido se encienden.
                let flicker = 0.75 + 0.5 * uniform(line ^ (((t * 12.0) as i64) << 20), 0xF11C);
                let gap = ((phi_f - s.burst_site + PI).rem_euclid(TAU) - PI).abs();
                let burst = 1.0 + s.burst.min(60.0) * 0.12 * (-(gap / 0.5).powi(2)).exp();
                // Unas lineas llevan mas plasma que otras.
                let load = 0.45 + 0.55 * uniform(line, 0xC0DE);
                let strength = (activity * flicker * burst * load) as f32 * fade * PLASMA_GAIN * set.gain * density;
                let phase = uniform(line, 0x9A5E);
                // Las dos mitades de una linea abierta se dibujan por separado.
                let halves: &[(f64, f64)] = if set.reach.is_finite() { &[(0.0, 0.5), (1.0, 0.5)] } else { &[(0.0, 1.0)] };
                for &(start, end) in halves {
                    points.clear();
                    colors.clear();
                    for k in 0..set.samples {
                        let x = k as f64 / (set.samples - 1) as f64;
                        // Mas puntos junto a los pies, donde la linea se curva.
                        let along = start + (end - start) * (1.0 - (1.0 - x) * (1.0 - x));
                        let mut p = super::magnetosphere::line_point(s.twist, theta_f, phi_f, along);
                        let r = p.length();
                        if r > set.reach {
                            break;
                        }
                        p = p * ((1.0 + (r - 1.0) * inflate) / r);
                        // 0 en los pies, 1 en la cima; los paquetes suben.
                        let d = (1.0 - (2.0 * along - 1.0).abs()) as f32;
                        let packets = 0.65 + 0.35 * ((TAU * (3.0 * d as f64 - 0.6 * t + phase)).sin() as f32).powi(3);
                        let near = 0.25 + 1.4 * (-(r as f32 - 1.0) / 1.6).exp();
                        // Las abiertas se apagan al alejarse.
                        let open_fade = if set.reach.is_finite() { 1.0 - (r / set.reach) as f32 } else { 1.0 };
                        points.push(to_world(p));
                        colors.push(plasma_tint(r as f32) * (near * packets * strength * open_fade));
                    }
                    let n = points.len();
                    for k in 1..n {
                        out.push(GlowLine {
                            a: points[k - 1],
                            b: points[k],
                            color_a: colors[k - 1],
                            color_b: colors[k],
                            cap_a: k == 1,
                            cap_b: k == n - 1,
                        });
                    }
                }
            }
        }
    }
}

/// La bola de fuego de la gran llamarada: pares e± y fotones a mas de 1e9 K
/// atrapados en las lineas cerradas junto a la estrella (Thompson y Duncan
/// 1995), en el sector donde se rompio la magnetosfera. Gira con la estrella
/// (por eso la cola de la llamarada pulsa con el periodo de giro) y se
/// evapora capa por capa: las lineas mas altas se vacian primero.
pub fn fireball(out: &mut Vec<Particle>, body: &Body, t: f64, flare: Option<f64>, camera: DVec3) {
    let s = state(t, flare);
    if s.fireball <= 0.0 {
        return;
    }
    let crust = crust_at(body, t, flare);
    let center = body.position(t) - camera;
    let (m, e1, e2) = magnetic_frame();
    // En los primeros 40 s brilla ademas una corona mas caliente.
    let corona = 1.0 + 2.0 * s.since_spike.map_or(0.0, |a| (-a / 40.0).exp());
    let light = (s.fireball * corona) as f32 * FIREBALL_GAIN / FIREBALL_COUNT as f32;
    // Cima de las lineas que todavia tienen plasma, en radios.
    let reach = 1.3 + 2.4 * s.fireball.powf(1.0 / 3.0);
    let slosh = t * 0.35;
    for k in 0..FIREBALL_COUNT as i64 {
        let u = |seed: u32| uniform(k, seed);
        // Linea cerrada con cima entre 1.15 radios y el alcance: sin^2 = 1/cima.
        let apex = 1.15 + (reach - 1.15) * u(1).powf(0.7);
        let theta_f = (1.0 / apex).sqrt().asin();
        let phi_f = 1.0 + 0.9 * (u(2) - 0.5);
        // El plasma va y viene a lo largo de su linea.
        let along = 0.5 + 0.47 * (TAU * (u(3) + slosh * (0.6 + 0.8 * u(4)))).sin();
        let p = super::magnetosphere::line_point(0.3, theta_f, phi_f, along);
        let local = (e1 * p.x + e2 * p.y + m * p.z) * RADIUS;
        let heat = 1.0 - ((apex - 1.15) / (reach - 1.15).max(1e-3)) as f32;
        let tint = Vec3::new(1.4, 0.35, 1.5).lerp(Vec3::new(2.4, 1.3, 2.2), heat);
        out.push(Particle {
            pos: (center + crust.rotate(local)).as_vec3(),
            radius: 1.6,
            color: tint * (light * (0.4 + 0.6 * heat)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_match_1e_1547_in_the_magnetar_catalog() {
        let m = model();
        // Kaspi y Beloborodov (2017), tabla 1: 0.69 kyr y 2.1e35 erg/s.
        assert!((m.age_years / 690.0 - 1.0).abs() < 0.05, "{}", m.age_years);
        assert!((m.edot / 2.1e35 - 1.0).abs() < 0.05, "{}", m.edot);
        assert!((braking_field(PERIOD, m.pdot) / DIPOLE_FIELD - 1.0).abs() < 1e-9);
        // 1.4 masas solares: rs = 4.13 km y compacidad 0.345.
        assert!((m.rs_km - 4.135).abs() < 0.01, "{}", m.rs_km);
        assert!(m.light_cylinder_km > 9.8e4 && m.light_cylinder_km < 9.9e4);
    }

    #[test]
    fn the_giant_flare_follows_its_sequence() {
        let flare = Some(100.0);
        let before = state(99.0, flare);
        assert_eq!(before.spike, 0.0);
        assert!(before.since_spike.is_none());
        // La torsion sube hasta el valor critico.
        let winding = state(100.0 + WIND_UP * 0.99, flare);
        assert!(winding.twist > 2.8, "{}", winding.twist);
        // El pico: las lineas abiertas y la bola de fuego entera.
        let spike = state(100.0 + WIND_UP + 0.02, flare);
        assert!(spike.spike > 0.5 && spike.open == 1.0 && spike.fireball > 0.99);
        assert_eq!(spike.radio, 0.0);
        // La cola: L0 (1 - t/380)^3 y la magnetosfera casi sin torsion.
        let tail = state(100.0 + WIND_UP + 38.0, flare);
        assert!((tail.fireball - 0.9f64.powi(3)).abs() < 1e-9);
        assert!(tail.twist < QUIET_TWIST);
        assert!(state(100.0 + WIND_UP + EVAPORATION + 1.0, flare).fireball == 0.0);
    }

    #[test]
    fn bursts_are_a_pure_function_of_time() {
        let quiet: Vec<f64> = (0..20_000).map(|k| state(k as f64 * 0.03, None).burst).collect();
        let again: Vec<f64> = (0..20_000).map(|k| state(k as f64 * 0.03, None).burst).collect();
        assert_eq!(quiet, again);
        // En diez minutos hay estallidos, pero casi siempre esta tranquila.
        let lit = quiet.iter().filter(|&&b| b > 0.05).count();
        assert!(lit > 10 && lit < quiet.len() / 4, "{lit}");
    }

    #[test]
    fn the_antiglitch_lengthens_the_period_after_the_spike() {
        let flare = Some(10.0);
        assert_eq!(period_at(10.0 + WIND_UP - 0.1, flare), PERIOD);
        let after = period_at(10.0 + WIND_UP + 1.0, flare);
        assert!((after / PERIOD - 1.0 - ANTIGLITCH).abs() < 1e-12);
    }

    #[test]
    fn the_beams_are_born_hundreds_of_km_above_the_poles() {
        let m = model();
        let outer = m.light_cylinder_km * (m.outer_cone / 1.5).powi(2);
        assert!(m.inner_height_km > 80.0 && m.inner_height_km < outer);
        assert!(outer < 400.0, "{outer}");
        // El casquete polar mide unos 130 m.
        assert!((m.polar_cap * RADIUS_KM * 1000.0 - 132.0).abs() < 5.0);
    }
}
