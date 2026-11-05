//! Anillos: un disco analitico, no una malla. Los de Vigia son polvo y hielo
//! que brillan a contraluz, con la estructura de los de Saturno: un anillo
//! interior tenue y oscuro, uno ancho y denso, la division oscura (como la
//! de Cassini) y el anillo exterior con un hueco angosto (como el de Encke)
//! donde orbita una lunita que deja olas en sus orillas; por fuera, un
//! anillo delgado de grumos que se trenza entre dos lunitas pastoras. Cada
//! zona tiene franjas de colores (lila, rosa, durazno, blanco hielo, gris
//! azulado) y sobre el anillo denso aparecen y se desvanecen rayos radiales
//! (los "spokes" de Saturno) que giran con el planeta y no con el anillo.
//! Los de la version chapina (las camionetas de la Capital y los barriletes
//! de los Cuchumatanes) se dibujan en `chapin/` y se mezclan igual.
//!
//! Por cada pixel se intersecta el rayo de la camara con el plano de los
//! anillos y se compara con la profundidad del visibility buffer: si el
//! anillo esta mas cerca que la superficie, se mezcla encima con su densidad
//! como opacidad; si esta detras, la superficie lo tapa. Asi los anillos son
//! semitransparentes sin ordenar triangulos.
//!
//! La densidad depende de la distancia al centro: bandas de ruido 1D,
//! bordes suaves y una division oscura. Con la misma funcion se calculan las
//! dos sombras: la de los anillos sobre el planeta y la del planeta sobre los
//! anillos.

use super::Uniforms;
use crate::math::{Perlin, Vec3, smoothstep};

/// De que esta hecho un anillo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingStyle {
    /// Vigia: polvo fino en anillos delgados.
    Dust,
    /// La capital (version chapina): camionetas y tuc-tucs dando vueltas.
    Buses,
    /// Los Cuchumatanes (version chapina): los barriletes de Sumpango.
    Kites,
}

#[derive(Clone, Copy, Debug)]
pub struct RingInfo {
    pub style: RingStyle,
    /// Centro del planeta, relativo a la camara.
    pub center: Vec3,
    /// Normal del plano de los anillos.
    pub normal: Vec3,
    /// Ejes del plano (fijos, no giran con el planeta).
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    pub inner: f32,
    pub outer: f32,
    /// Radio del planeta, para su sombra sobre los anillos.
    pub planet_radius: f32,
}

/// Franjas de los anillos de Vigia: lila, rosa, durazno, blanco hielo y gris
/// azulado.
const ICE_BANDS: [Vec3; 5] = [
    Vec3::new(0.5, 0.38, 0.66),
    Vec3::new(0.6, 0.34, 0.5),
    Vec3::new(0.62, 0.45, 0.42),
    Vec3::new(0.66, 0.64, 0.76),
    Vec3::new(0.34, 0.36, 0.52),
];
/// Los anillos de Vigia, en fraccion del ancho: donde termina el interior
/// tenue, la orilla del denso, la division oscura, el hueco del anillo
/// exterior (con su lunita), el final del exterior y el anillo de grumos.
const C_END: f32 = 0.18;
const B_END: f32 = 0.55;
const DIVISION_END: f32 = 0.605;
const GAP: f32 = 0.83;
const A_END: f32 = 0.89;
const F_RING: f32 = 0.95;

/// Densidad de los anillos (0 a menos de 1) a la distancia `r` del centro.
pub fn density(ring: &RingInfo, r: f32, noise: &Perlin) -> f32 {
    let s = (r - ring.inner) / (ring.outer - ring.inner);
    if !(0.0..=1.0).contains(&s) {
        return 0.0;
    }
    match ring.style {
        // Como los de Saturno (sin lo que depende del angulo: eso solo se
        // dibuja, no da sombra).
        RingStyle::Dust => dust_density(s, 0.0, noise),
        RingStyle::Buses => crate::chapin::capital::ring_density(s, noise),
        RingStyle::Kites => crate::chapin::cuchumatanes::ring_density(s, noise),
    }
}

/// Fraccion de luz que pasa a traves de los anillos desde `point` en la
/// direccion `to_light` (ambos relativos a la camara).
pub fn transmittance(ring: &RingInfo, point: Vec3, to_light: Vec3, noise: &Perlin) -> f32 {
    let denom = to_light.dot(ring.normal);
    if denom.abs() < 1e-6 {
        return 1.0;
    }
    let t = (ring.center - point).dot(ring.normal) / denom;
    if t <= 0.0 {
        return 1.0;
    }
    let hit = point + to_light * t;
    1.0 - density(ring, (hit - ring.center).length(), noise)
}

/// Mezcla los anillos sobre `color` si el rayo `dir` (unitario) los cruza
/// antes de `depth`.
pub fn composite(color: Vec3, dir: Vec3, depth: f32, ring: &RingInfo, u: &Uniforms) -> Vec3 {
    let denom = dir.dot(ring.normal);
    if denom.abs() < 1e-6 {
        return color;
    }
    let t = ring.center.dot(ring.normal) / denom;
    if t <= 0.0 || t >= depth {
        return color;
    }
    let hit = dir * t;
    let r = (hit - ring.center).length();
    let s = (r - ring.inner) / (ring.outer - ring.inner);
    if !(0.0..=1.0).contains(&s) {
        return color;
    }
    // Los de Vigia calculan su densidad abajo (con lo que depende del angulo,
    // que tambien pone cosas donde la densidad por radio es cero).
    let alpha = if ring.style == RingStyle::Dust { 1.0 } else { density(ring, r, u.noise) };
    if alpha <= 0.0 {
        return color;
    }
    let to_sun = (u.sun_pos - hit).normalize();
    // Sombra del planeta: el rayo del punto hacia la estrella pega en la
    // esfera. La estrella es enorme, asi que el borde es una penumbra que se
    // abre con la distancia detras del planeta.
    let oc = hit - ring.center;
    let b = oc.dot(to_sun);
    let lit_part = if b < 0.0 {
        let miss = (oc.length_squared() - b * b).max(0.0).sqrt();
        let penumbra = (-b * u.sun_radius / (u.sun_pos - hit).length()).max(1.0);
        smoothstep(ring.planet_radius - penumbra, ring.planet_radius + penumbra, miss)
    } else {
        1.0
    };
    // Lo que mide un pixel en el plano, visto de lado crece.
    let footprint = t * u.pixel_angle / denom.abs().max(0.05);
    let local = hit - ring.center;
    // El polvo dispersa hacia adelante: brilla mas a contraluz (el fino de
    // Vigia mucho mas).
    let (albedo, alpha, glow, scatter) = match ring.style {
        RingStyle::Dust => {
            let (alpha, albedo) = dust(ring, local, s, footprint, u);
            (albedo, alpha, Vec3::ZERO, 3.0)
        }
        RingStyle::Buses => crate::chapin::capital::ring(ring, local, alpha, s, footprint, u),
        RingStyle::Kites => crate::chapin::cuchumatanes::ring(ring, local, alpha, s, footprint, u),
    };
    let forward = dir.dot(to_sun).max(0.0).powi(6) * scatter;
    let glint = (-dir).dot(ring.normal).abs() * 0.2;
    let light = 0.02 + (0.33 + forward + glint) * lit_part;
    // El polvo de Vigia, como sus nubes, guarda su color con la luz roja.
    let sun = match ring.style {
        RingStyle::Dust => Vec3::splat(u.sun_light.luminance() * 1.3),
        RingStyle::Buses | RingStyle::Kites => u.sun_light,
    };
    color.lerp(albedo * sun * light, alpha) + glow
}

/// Densidad de los anillos de Vigia en `s` (fraccion del ancho). `wake`
/// corre la orilla del hueco del anillo exterior (las olas de su lunita).
fn dust_density(s: f32, wake: f32, noise: &Perlin) -> f32 {
    let ringlets = noise.noise(Vec3::new(s * 80.0, 0.5, 2.5)) * 0.5 + 0.5;
    let fine = noise.noise(Vec3::new(s * 260.0, 1.5, 2.5)) * 0.5 + 0.5;
    // El interior, tenue; el denso, con anillitos; la division, casi vacia;
    // el exterior, con su hueco y otro mas fino cerca de la orilla.
    let inner = smoothstep(0.0, 0.02, s) * smoothstep(C_END + 0.02, C_END - 0.01, s) * (0.05 + 0.07 * ringlets);
    let dense = smoothstep(C_END - 0.01, C_END + 0.02, s) * smoothstep(B_END + 0.005, B_END - 0.01, s)
        * (0.2 + 0.24 * ringlets + 0.06 * fine);
    let gap = smoothstep(0.002, 0.0055, (s - GAP - wake).abs());
    let fine_gap = 1.0 - 0.9 * smoothstep(0.0018, 0.0, (s - 0.872).abs());
    let outer = smoothstep(DIVISION_END - 0.01, DIVISION_END + 0.008, s) * smoothstep(A_END + 0.005, A_END - 0.008, s)
        * (0.14 + 0.1 * ringlets + 0.04 * fine)
        * gap
        * fine_gap;
    let division = smoothstep(B_END, B_END + 0.01, s) * smoothstep(DIVISION_END, DIVISION_END - 0.01, s) * 0.025;
    (inner + dense + outer + division).min(0.8)
}

/// Lo que se ve de los anillos de Vigia en `local` (relativo al planeta,
/// en el plano): cuanto tapan y su color. Ademas de la densidad: las olas
/// que deja la lunita del hueco, la lunita misma, el anillo de grumos que
/// se trenza y los rayos radiales sobre el anillo denso.
fn dust(ring: &RingInfo, local: Vec3, s: f32, footprint: f32, u: &Uniforms) -> (f32, Vec3) {
    use std::f32::consts::{PI, TAU};
    let n = u.noise;
    let t = u.time;
    let (a, b) = (local.dot(ring.axis_a), local.dot(ring.axis_b));
    let r = a.hypot(b);
    let theta = b.atan2(a);
    let width = ring.outer - ring.inner;
    // Las cosas del anillo dan la vuelta mas rapido mientras mas adentro
    // (Kepler); las lunitas, a su radio.
    let orbit = |s: f32| {
        let r = ring.inner + s * width;
        900.0 / (r * r.sqrt())
    };
    let wrap = |x: f32| (x + PI).rem_euclid(TAU) - PI;
    // La lunita del hueco: atras de ella las orillas quedan onduladas y las
    // olas se van calmando.
    let moon = t * orbit(GAP) + 1.3;
    let behind = (moon - theta).rem_euclid(TAU);
    let wake = 0.0016 * (behind * 45.0).sin() * (-behind * 1.4).exp();
    let mut alpha = dust_density(s, wake, n);
    // El anillo de grumos: angosto, trenzado entre dos lunitas y cortado en
    // pedazos que dan la vuelta.
    let f_turn = t * orbit(F_RING);
    let braid = 0.0025 * (theta * 7.0 + f_turn * 7.0).sin();
    let strand = smoothstep(0.004, 0.0, (s - F_RING - braid).abs());
    if strand > 0.0 {
        let along = theta + f_turn;
        let clumps = n.noise(Vec3::new(along.cos() * 9.0, along.sin() * 9.0, 4.4)) * 0.5 + 0.5;
        alpha = alpha.max(strand * (0.12 + 0.5 * clumps * clumps));
    }
    // Las franjas de colores, mas oscuras en el interior y mas claras en el
    // denso.
    let shade = n.noise(Vec3::new(s * 23.0, 4.2, 0.7)) * 0.5 + 0.5;
    let k = shade * (ICE_BANDS.len() - 1) as f32;
    let i = (k as usize).min(ICE_BANDS.len() - 2);
    let mut albedo = ICE_BANDS[i].lerp(ICE_BANDS[i + 1], k - i as f32);
    albedo *= if s < C_END { 0.45 } else if s < B_END { 0.7 } else { 0.6 };
    // Los rayos radiales: cuñas oscuras sobre el anillo denso, que giran con
    // el planeta y aparecen y se desvanecen despacio.
    if (C_END..B_END).contains(&s) {
        let turn = theta + t * 0.07;
        let spokes = n.noise(Vec3::new(turn.cos() * 7.0, turn.sin() * 7.0, s * 2.5 + t * 0.02)) * 0.5 + 0.5;
        let come_and_go = 0.5 + 0.5 * (t * 0.05 + theta.sin()).sin();
        albedo *= 1.0 - 0.45 * smoothstep(0.6, 0.85, spokes) * come_and_go;
    }
    // Las lunitas: un puntito claro y redondo, solo cuando se alcanza a ver.
    // Las lunitas estan a un pelo de su anillo: casi no hay donde buscarlas.
    if (s - GAP).abs() > 0.02 && (s - F_RING).abs() > 0.03 {
        return (alpha, albedo);
    }
    for (at, phase, size) in [(GAP, moon, 2.5), (F_RING - 0.012, t * orbit(F_RING - 0.012) + 4.0, 1.6), (F_RING + 0.012, t * orbit(F_RING + 0.012) + 2.2, 1.4)] {
        let along = wrap(theta - phase) * r;
        let across = (s - at) * width;
        let d = along.hypot(across);
        let rock = smoothstep(size + footprint, size * 0.6, d) * (size / (size + footprint)).min(1.0);
        if rock > 0.0 {
            alpha = alpha.max(rock);
            albedo = albedo.lerp(Vec3::new(0.5, 0.48, 0.52), rock);
        }
    }
    (alpha, albedo)
}

/// Distancia por el rayo `dir` (unitario) al plano del anillo, si lo cruza
/// adelante: para mezclar varios anillos del mas lejano al mas cercano.
pub fn distance(ring: &RingInfo, dir: Vec3) -> f32 {
    let denom = dir.dot(ring.normal);
    if denom.abs() < 1e-6 {
        return f32::INFINITY;
    }
    let t = ring.center.dot(ring.normal) / denom;
    if t > 0.0 { t } else { f32::INFINITY }
}