//! Anillos: un disco analitico, no una malla. Los de Cementerio estan
//! llenos de los restos de las naves del exodo que nunca escapo; los de
//! Vigia son polvo y hielo que brillan a contraluz, con la estructura de los
//! de Saturno: un anillo interior tenue y oscuro, uno ancho y denso, la
//! division oscura (como la de Cassini) y el anillo exterior con un hueco
//! angosto (como el de Encke) donde orbita una lunita que deja olas en sus
//! orillas; por fuera, un anillo delgado de grumos que se trenza entre dos
//! lunitas pastoras. Cada zona tiene franjas de colores (lila, rosa, durazno,
//! blanco hielo, gris azulado) y sobre el anillo denso aparecen y se
//! desvanecen rayos radiales (los "spokes" de Saturno) que giran con el
//! planeta y no con el anillo.
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
//!
//! Encima, los restos: celdas de Voronoi en el plano del anillo, cada una un
//! pedazo de casco de metal con su propio brillo, separadas por huecos, que
//! orbitan en bloque. Algunas naves todavia tienen una luz
//! de navegacion que parpadea. Cuando un pedazo mide menos que un pixel se
//! dibuja su promedio: de lejos el anillo se ve granulado, sin parpadear.

use super::Uniforms;
use crate::math::{Perlin, Vec3, hash_u32, smoothstep};

/// De que esta hecho un anillo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingStyle {
    /// Cementerio: los restos de las naves del exodo.
    Wrecks,
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

const DUST: Vec3 = Vec3::new(0.62, 0.55, 0.48);
const DARK_DUST: Vec3 = Vec3::new(0.30, 0.26, 0.24);
const STEEL: Vec3 = Vec3::new(0.22, 0.24, 0.27);
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
const RUST: Vec3 = Vec3::new(0.26, 0.13, 0.07);
/// Tamano de un pedazo de casco, en unidades del mundo.
const PIECE: f32 = 14.0;

/// Voronoi 2D: distancia al centro mas cercano, al segundo, el hash de la
/// celda mas cercana y su centro. Las celdas se repiten cada `period` en y:
/// la vuelta completa del anillo cierra sin costura.
fn pieces(x: f32, y: f32, period: i32, seed: u32) -> (f32, f32, u32, (f32, f32)) {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let mut best = (f32::MAX, f32::MAX, 0u32, (0.0, 0.0));
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (cx, cy) = (ix + dx, iy + dy);
            let h = hash_u32(
                (cx as u32).wrapping_mul(0x8DA6_B343)
                    ^ (cy.rem_euclid(period) as u32).wrapping_mul(0xD816_3841)
                    ^ seed,
            );
            let px = cx as f32 + 0.15 + 0.7 * (h & 0xFFFF) as f32 / 65536.0;
            let py = cy as f32 + 0.15 + 0.7 * (h >> 16) as f32 / 65536.0;
            let d = (x - px).hypot(y - py);
            if d < best.0 {
                best = (d, best.0, h, (px, py));
            } else if d < best.1 {
                best.1 = d;
            }
        }
    }
    best
}

/// Una nave en su celda: un casco que se afina hacia la nariz, con alas
/// cruzadas atras, un poco girado. Coordenadas en unidades del mundo desde
/// el centro de la celda; devuelve cuanto tapa y un brillo en el lomo.
fn ship(ox: f32, oy: f32, id: u32, footprint: f32) -> (f32, f32) {
    let unit = |k: u32| (hash_u32(id ^ k) >> 8) as f32 / 16_777_216.0;
    let turn = (unit(12) - 0.5) * 0.9;
    let (s, c) = turn.sin_cos();
    let (x, y) = (ox * c - oy * s, ox * s + oy * c);
    let half = 9.0 + 7.0 * unit(13);
    let radius = (2.0 + 1.6 * unit(14)) * (1.0 - 0.45 * smoothstep(-half, half, y));
    let body = x.hypot((y.abs() - half).max(0.0)) - radius;
    let wing_y = y + half * 0.55;
    let wings = (x.abs() - radius * 2.6).max(wing_y.abs() - 1.3);
    let d = body.min(wings);
    let cover = smoothstep(0.5 + footprint, -0.5, d);
    let spine = smoothstep(radius * 0.5, 0.0, x.abs()) * smoothstep(0.0, -1.0, body);
    (cover, spine)
}

/// Esquirlas de chatarra: pedazos angulosos (celdas de Voronoi encogidas).
fn scrap(x: f32, y: f32, period: i32, chance: f32) -> f32 {
    let (f1, f2, id, _) = pieces(x, y, period, 0xBEEF);
    if (hash_u32(id ^ 11) >> 8) as f32 / 16_777_216.0 > chance {
        return 0.0;
    }
    smoothstep(0.35, 0.45, f2 - f1) * smoothstep(0.42, 0.3, f1)
}

/// Coordenadas de los pedazos de un punto del anillo: radio y avance en la
/// vuelta (en celdas) y cuantas celdas tiene una vuelta.
fn polar(ring: &RingInfo, hit: Vec3, time: f32) -> (f32, f32, i32) {
    // Gira en bloque a la velocidad de Kepler del radio medio: con la de
    // cada radio, en un minuto los pedazos vecinos ya se habrian estirado
    // en rayas.
    let middle = 0.5 * (ring.inner + ring.outer);
    let spin = time * 900.0 / middle.powf(1.5);
    let (a, b) = (hit.dot(ring.axis_a), hit.dot(ring.axis_b));
    let r = a.hypot(b);
    let theta = b.atan2(a) + spin;
    // Polar, estirado a lo largo de la orbita: un numero entero de celdas
    // por vuelta.
    let per_turn = (std::f32::consts::TAU * middle / (PIECE * 2.4)).round();
    (r / PIECE, theta / std::f32::consts::TAU * per_turn, per_turn as i32)
}

/// Lo que hay en un punto del anillo: cuanto tapa, su color y la luz que
/// emite. `footprint` es lo que mide un pixel en el plano.
fn debris(ring: &RingInfo, hit: Vec3, footprint: f32, time: f32, noise: &Perlin) -> (f32, Vec3, Vec3) {
    let (x, y, period) = polar(ring, hit, time);
    // Cumulos de restos con claros entre ellos, a la escala de una flota.
    let clumps = smoothstep(-0.4, 0.25, noise.noise(Vec3::new(x * 0.25, (y * 0.3).sin() * 3.0, (y * 0.3).cos() * 3.0)));
    let resolved = smoothstep(0.5, 0.15, footprint / PIECE);
    let average = (0.1 + 0.2 * clumps, STEEL.lerp(RUST, 0.4) * 0.8);
    if resolved <= 0.0 {
        return (average.0, average.1, Vec3::ZERO);
    }
    let (_, _, id, center) = pieces(x, y, period, 0x5EED);
    let unit = |k: u32| (hash_u32(id ^ k) >> 8) as f32 / 16_777_216.0;
    let (ox, oy) = ((x - center.0) * PIECE, (y - center.1) * PIECE * 2.4);
    let (hull, spine) = if unit(11) < 0.5 * clumps + 0.12 {
        ship(ox, oy, id, footprint)
    } else {
        (0.0, 0.0)
    };
    let chips = scrap(x * 3.0, y * 3.0, period * 3, 0.4 * clumps);
    let paint = STEEL.lerp(RUST, unit(3)).lerp(Vec3::splat(0.06), unit(9) * 0.6)
        * (0.5 + 0.6 * unit(4))
        * (1.0 + 0.8 * spine);
    let cover = hull.max(chips * 0.75);
    let color = if hull >= chips { paint } else { STEEL.lerp(RUST, 0.6) * 0.6 };
    // Algunas naves todavia tienen su luz de navegacion, en la popa.
    let mut light = Vec3::ZERO;
    if hull > 0.0 && id % 4 == 0 {
        let half = 9.0 + 7.0 * unit(13);
        let d = ox.hypot(oy + half);
        let size = 0.9;
        let spot = smoothstep(size + footprint, 0.0, d) * (size / (size + footprint)).powi(2);
        let blink = smoothstep(0.8, 0.97, (time * (1.4 + unit(5)) + unit(6) * 6.28).sin());
        let tint = if unit(7) < 0.6 {
            Vec3::new(1.0, 0.08, 0.04)
        } else {
            Vec3::new(0.8, 0.9, 1.0)
        };
        light = tint * (spot * blink * 18.0);
    }
    (
        average.0 + (cover - average.0) * resolved,
        average.1.lerp(color, resolved),
        light * resolved,
    )
}

/// Densidad de los anillos (0 a ~0.9) a la distancia `r` del centro.
pub fn density(ring: &RingInfo, r: f32, noise: &Perlin) -> f32 {
    let s = (r - ring.inner) / (ring.outer - ring.inner);
    if !(0.0..=1.0).contains(&s) {
        return 0.0;
    }
    match ring.style {
        RingStyle::Wrecks => {
            let edges = smoothstep(0.0, 0.04, s) * smoothstep(1.0, 0.9, s);
            let division = 1.0 - 0.9 * smoothstep(0.02, 0.0, (s - 0.58).abs());
            let bands = noise.noise(Vec3::new(s * 37.0, 0.5, 0.5)) * 0.5 + 0.5;
            let fine = noise.noise(Vec3::new(s * 160.0, 1.5, 0.5)) * 0.5 + 0.5;
            (edges * division * (0.25 + 0.55 * bands + 0.2 * fine)).min(0.92)
        }
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
    // Vigia mucho mas); el metal y la piedra devuelven destellos.
    let (albedo, alpha, glow, scatter) = match ring.style {
        RingStyle::Wrecks => {
            let dust = DARK_DUST.lerp(DUST, smoothstep(0.1, 0.9, alpha) * (0.7 + 0.3 * s));
            let (cover, metal, beacon) = debris(ring, local, footprint, u.time, u.noise);
            (
                dust.lerp(metal, 0.75),
                (alpha * (0.15 + 1.1 * cover)).min(0.97),
                beacon * u.gains.cementerio,
                1.5,
            )
        }
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
        RingStyle::Wrecks | RingStyle::Buses | RingStyle::Kites => u.sun_light,
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