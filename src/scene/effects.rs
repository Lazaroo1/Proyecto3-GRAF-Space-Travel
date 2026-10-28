//! Efectos de particulas: los arcos de plasma de la estrella, las esporas
//! que flotan sobre Jardin, las ondas de la baliza y el escape de la nave.
//!
//! Ninguna particula guarda estado: cada una es una **funcion del tiempo**.
//! Su edad es `fract(t / vida + fase)`, con una fase sacada de un hash de su
//! indice, y su posicion sale de la edad. Asi no hay que simular nada frame
//! a frame, el efecto se ve igual a cualquier fps y rebobinar el tiempo las
//! devuelve exactamente a donde estaban.

use crate::math::blackbody::planckian_rgb;
use crate::math::{DQuat, DVec3, Vec3, hash_u32, smoothstep};
use crate::parallel;
use crate::render::Particle;

use super::Body;

/// Esporas alrededor de la camara cerca de Jardin.
pub const SPORE_COUNT: usize = 450;
/// Lado del cubo de esporas (se repite al moverse la camara).
const SPORE_BOX: f32 = 90.0;
/// Cuanto sube una espora en su vida, en unidades del mundo.
const SPORE_RISE: f32 = 8.0;
/// Particulas del escape de la nave.
pub const EXHAUST_COUNT: usize = 100;
/// Protuberancias: arcos de plasma de varias hebras torcidas.
const PROMINENCE_ARCS: usize = 5;
const PROMINENCE_STRANDS: usize = 9;
const PROMINENCE_SEGMENTS: usize = 120;
/// Chispas que salen despedidas del borde de la estrella.
const STAR_EMBERS: usize = 700;
pub const PROMINENCE_COUNT: usize =
    PROMINENCE_ARCS * PROMINENCE_STRANDS * PROMINENCE_SEGMENTS + STAR_EMBERS;

/// Arcos de plasma y chispas en el borde de la estrella. Los arcos se anclan
/// al limbo tal como se ve desde la camara: siempre se recortan contra el
/// cielo, nunca quedan aplastados sobre el disco. Son pocos y grandes, casi
/// todos arriba. Cada uno es un manojo de hebras que se enroscan a distintos
/// radios (se ve como fuego, no como un tubo), encendido en los pies y rojo
/// arriba, con nudos de material que bajan; resbalan despacio por el borde.
/// Con las llamaradas del estertor (`flares`) crecen, y uno de cada tres se
/// desprende.
pub fn prominences(
    out: &mut [Particle],
    star: &Body,
    t: f64,
    camera: DVec3,
    strength: f32,
    flares: f32,
) {
    let center = (star.position(t) - camera).as_vec3();
    let distance = center.length();
    let r = star.radius as f32;
    let time = t as f32;
    let axis = -center / distance;
    let (a, b) = basis(axis);
    // El limbo visto desde la camara: un circulo un poco hacia ella.
    let cap = (r / distance).min(0.99);
    let ring = (1.0 - cap * cap).sqrt();
    let limb = |phi: f32, lean: f32| {
        (a * (phi.cos() * ring) + b * (phi.sin() * ring) + axis * (cap + lean)).normalize()
    };
    let (arcs, embers) = out.split_at_mut(PROMINENCE_ARCS * PROMINENCE_STRANDS * PROMINENCE_SEGMENTS);
    let per_arc = PROMINENCE_STRANDS * PROMINENCE_SEGMENTS;
    for (i, p) in arcs.iter_mut().enumerate() {
        let arc = i / per_arc;
        let strand = (i / PROMINENCE_SEGMENTS) % PROMINENCE_STRANDS;
        let s = (i % PROMINENCE_SEGMENTS) as f32 / (PROMINENCE_SEGMENTS - 1) as f32;
        let h = hash_u32(arc as u32 ^ 0xAC1D_72A1);
        // Tres arriba y los demas en cualquier parte del borde.
        let spread = if arc < 3 { 0.9 } else { 2.8 };
        let phi = std::f32::consts::FRAC_PI_2 + (unit(hash_u32(h)) * 2.0 - 1.0) * spread + time * 0.004;
        let n = limb(phi, 0.06 + 0.1 * unit(hash_u32(h ^ 41)));
        // Los pies van a lo largo del borde y el arco sube hacia afuera.
        let tangent = axis.cross(n).normalize();
        let side = n.cross(tangent);
        let width = 0.12 + 0.14 * unit(hash_u32(h ^ 7));
        let angle = (s - 0.5) * width * 2.0;
        let wave = (std::f32::consts::PI * s).sin().max(0.0);
        // Uno de cada tres se desprende cuando las llamaradas arrecian.
        let erupt = if arc % 3 == 0 {
            smoothstep(0.35, 1.0, flares)
        } else {
            0.0
        };
        let tall = unit(hash_u32(h ^ 13));
        let height = (0.22 + 0.28 * tall)
            * (0.85 + 0.15 * (time * 0.12 + arc as f32).sin())
            * (1.0 + 1.6 * flares + 5.0 * erupt);
        let radial = n * angle.cos() + tangent * angle.sin();
        let lift = r * (1.0 + height * wave.powf(0.9) + 0.5 * erupt * erupt);
        // Cada hebra se enrosca a su radio y a su ritmo alrededor del eje del
        // arco: el manojo se ve deshilachado.
        let hs = hash_u32(h ^ (strand as u32 + 1) * 0x9E37);
        let twist = strand as f32 / PROMINENCE_STRANDS as f32 * std::f32::consts::TAU
            + s * (3.0 + 6.0 * unit(hs))
            + time * (0.3 + 0.4 * unit(hash_u32(hs)));
        let rope = r * (0.008 + 0.024 * wave) * (0.4 + unit(hash_u32(hs ^ 5))) * (1.0 + erupt);
        let jitter = unit(hash_u32(i as u32 ^ 0x5EED)) - 0.5;
        let flicker = unit(hash_u32(i as u32 ^ 0x0F1C)) - 0.5;
        let offset = (side * twist.cos() + radial * twist.sin()) * rope
            + radial * (jitter * r * 0.02 * (0.3 + wave))
            + side * (flicker * r * 0.015 * wave);
        // Nudos de material que caen por el arco hacia los pies.
        let knots = 0.5 + 0.5 * (s * 22.0 - time * 1.6 + arc as f32 * 1.7).sin();
        let foot = Vec3::new(3.2, 1.6, 0.45);
        let crown = Vec3::new(1.6, 0.3, 0.05);
        let brightness = strength
            * (0.5 + 0.5 * knots)
            * (0.4 + 0.6 * unit(hash_u32(hs ^ 9)))
            * (0.55 + 0.45 * (1.0 - erupt));
        *p = Particle {
            pos: center + radial * lift + offset,
            radius: r * (0.01 + 0.012 * wave) * (1.0 + 0.6 * erupt),
            color: foot.lerp(crown, wave.powf(0.6)) * (0.1 * brightness),
        };
    }
    // Chispas: salen del borde, suben girando un poco y se apagan.
    for (i, p) in embers.iter_mut().enumerate() {
        let h = hash_u32(i as u32 ^ 0xE3B3_5A11);
        let life = 3.0 + 4.0 * unit(hash_u32(h ^ 3));
        let age = (time / life + unit(h)).fract();
        let phi = unit(hash_u32(h ^ 5)) * std::f32::consts::TAU + age * 0.25 * (unit(hash_u32(h ^ 9)) - 0.5);
        let start = limb(phi, (unit(hash_u32(h ^ 11)) - 0.3) * 0.5);
        let reach = 0.15 + 0.9 * unit(hash_u32(h ^ 17)).powi(2);
        let fade = smoothstep(0.0, 0.08, age) * (1.0 - age) * (1.0 - age);
        *p = Particle {
            pos: center + start * (r * (1.0 + reach * age)),
            radius: r * (0.0025 + 0.002 * unit(hash_u32(h ^ 19))),
            color: Vec3::new(3.0, 1.1, 0.25).lerp(Vec3::new(1.4, 0.2, 0.04), age) * (fade * strength),
        };
    }
}

/// Lo que flota alrededor de la camara cerca del suelo de cada planeta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motes {
    /// Brasas que suben del suelo de Pira.
    Embers,
    /// Gotas de liquido rojo que flotan despacio sobre la carne de Argos.
    Drops,
    /// Polvo y ceniza de metal en Cementerio.
    Dust,
    /// Jirones de nube que pasan rozando a Vigia.
    Mist,
    /// Polvo de cristal que flota sobre Parpado y destella.
    Glints,
}

pub const MOTE_COUNT: usize = 520;

/// Particulas en un cubo que acompana a la camara, guardadas en el espacio
/// del planeta (como las esporas de Jardin). `strength` las apaga lejos del
/// suelo.
pub fn motes(out: &mut [Particle], body: &Body, t: f64, camera: DVec3, kind: Motes, strength: f32) {
    let center = body.position(t);
    let rotation = body.rotation(t);
    let cam_local = rotation.conjugate().rotate(camera - center);
    let up = cam_local.normalize().as_vec3();
    let (east, north) = basis(up);
    let to_world = |local: DVec3| (center + rotation.rotate(local) - camera).as_vec3();
    // Lado del cubo, recorrido vertical por vida (negativo: cae), viento
    // lateral (unidades por segundo) y tamano.
    let (size, travel, wind, radius) = match kind {
        Motes::Embers => (80.0f32, 26.0f32, 1.5f32, 0.035f32),
        Motes::Drops => (70.0, -3.0, 1.2, 0.07),
        Motes::Dust => (90.0, 2.0, 2.5, 0.12),
        Motes::Mist => (240.0, 0.0, 45.0, 9.0),
        Motes::Glints => (60.0, 3.0, 0.8, 0.03),
    };
    let half = size * 0.5;
    let time = t as f32;
    let cam = cam_local.as_vec3();
    parallel::chunks_mut(out, 256, 256, |job, chunk| {
        for (k, particle) in chunk.iter_mut().enumerate() {
            let i = (job * 256 + k) as u32;
            let h = hash_u32(i ^ 0x3077_E500);
            let (h1, h2) = (hash_u32(h), hash_u32(h ^ 0x9E37));
            let base = Vec3::new(unit(h), unit(h1), unit(h2)) * size;
            let phase = unit(h >> 4) * std::f32::consts::TAU;
            let speed = 0.6 + 0.8 * unit(h1 >> 4);
            let age = (time * speed * 0.2 + unit(h2 >> 6)).fract();
            // Recorrido acotado por la edad, para que no salten al moverse.
            let rise = up * (age * travel);
            let sway = (east * (time * 0.6 + phase).sin() + north * (time * 0.45 + phase * 1.3).cos()) * 1.4;
            let drift = east * ((time * wind) % size);
            let p = base + rise + sway + drift - cam;
            let wrapped = Vec3::new(
                p.x.rem_euclid(size) - half,
                p.y.rem_euclid(size) - half,
                p.z.rem_euclid(size) - half,
            );
            let fade = smoothstep(half, half * 0.6, wrapped.length())
                * smoothstep(0.0, 0.12, age)
                * smoothstep(1.0, 0.85, age);
            let flicker = 0.55 + 0.45 * (time * (3.0 + 4.0 * unit(h2 >> 3)) + phase).sin();
            let color = match kind {
                // Solo una de cada tres brasas esta encendida a la vez.
                Motes::Embers if h % 3 == 0 => planckian_rgb(1300.0 + 900.0 * unit(h >> 9)) * (7.0 * flicker),
                Motes::Embers => Vec3::ZERO,
                Motes::Drops => Vec3::new(0.7, 0.05, 0.1) * (0.25 + 0.35 * flicker),
                Motes::Dust => Vec3::new(0.55, 0.42, 0.32) * 0.35,
                Motes::Mist => Vec3::new(0.40, 0.28, 0.70) * 0.012,
                // Destellos cortos: casi siempre apagados.
                Motes::Glints => Vec3::new(0.8, 0.45, 1.0) * (6.0 * smoothstep(0.85, 1.0, flicker)),
            };
            *particle = Particle {
                pos: to_world((cam + wrapped).as_dvec3()),
                radius: radius * (0.6 + 0.8 * unit(h1 >> 8)),
                color: color * (fade * strength),
            };
        }
    });
}

pub const FOUNTAIN_COUNT: usize = 110;

/// Una fuente de lava: gotas que salen de la boca de un geiser, suben y
/// caen con la gravedad, enfriandose. `activity` (0 a 1) decide cuantas.
pub fn fountain(out: &mut [Particle], mouth: Vec3, up: Vec3, t: f64, activity: f32, seed: u32) {
    const LIFE: f32 = 2.6;
    const GRAVITY: f32 = 16.0;
    let (a, b) = basis(up);
    let time = t as f32;
    let colors = Blackbody::new();
    for (i, p) in out.iter_mut().enumerate() {
        let h = hash_u32(i as u32 ^ seed);
        // Con poca actividad solo salen unas pocas.
        let alive = unit(h >> 3) < activity;
        let age = (time / LIFE + unit(h)).fract() * LIFE;
        let speed = 12.0 + 14.0 * unit(hash_u32(h));
        let angle = unit(hash_u32(h ^ 1)) * std::f32::consts::TAU;
        let spread = 0.15 + 0.25 * unit(hash_u32(h ^ 2));
        let velocity = (up + (a * angle.cos() + b * angle.sin()) * spread) * speed;
        let pos = mouth + velocity * age - up * (0.5 * GRAVITY * age * age);
        let cooled = age / LIFE;
        *p = Particle {
            pos,
            radius: 0.1 + 0.18 * unit(h >> 5),
            color: if alive {
                colors.get(2300.0 - 1000.0 * cooled) * (4.0 * (1.0 - cooled).powi(2))
            } else {
                Vec3::ZERO
            },
        };
    }
}

pub const SPARK_COUNT: usize = 60;

/// Chispas de un naufragio que se despierta: rafagas cortas de chispas que
/// saltan y caen.
pub fn sparks(out: &mut [Particle], at: Vec3, up: Vec3, t: f64, strength: f32, seed: u32) {
    const LIFE: f32 = 0.7;
    let time = t as f32;
    // Una rafaga cada tanto, no siempre.
    let slot = (time / 1.9 + unit(seed)).floor();
    let burst = unit(hash_u32(seed ^ slot as u32)) < 0.55;
    let start = (time / 1.9 + unit(seed)).fract() * 1.9;
    let (a, b) = basis(up);
    for (i, p) in out.iter_mut().enumerate() {
        let h = hash_u32(i as u32 ^ seed ^ slot as u32);
        let age = start - unit(h) * 0.2;
        let alive = burst && (0.0..LIFE).contains(&age);
        let angle = unit(hash_u32(h)) * std::f32::consts::TAU;
        let velocity = (up * (0.6 + unit(h >> 4)) + (a * angle.cos() + b * angle.sin()) * 1.2) * 9.0;
        let pos = at + velocity * age - up * (6.0 * age * age);
        *p = Particle {
            pos,
            radius: 0.06,
            color: if alive {
                Vec3::new(1.0, 0.75, 0.45) * (20.0 * strength * (1.0 - age / LIFE))
            } else {
                Vec3::ZERO
            },
        };
    }
}

/// Ondas que la baliza de Cementerio emite con cada destello.
const BEACON_WAVES: usize = 3;
const BEACON_WAVE_POINTS: usize = 260;
pub const BEACON_COUNT: usize = BEACON_WAVES * BEACON_WAVE_POINTS;

/// La transmision de la baliza: con cada destello sale un anillo de luz roja
/// que se abre sobre el horizonte y sube, y se apaga. `top` es la lampara y
/// `up` la vertical del planeta ahi, ambos relativos a la camara.
pub fn beacon_waves(out: &mut [Particle], top: Vec3, up: Vec3, t: f64, period: f32, near: f32) {
    let (a, b) = basis(up);
    let life = period * BEACON_WAVES as f32;
    let time = t as f32;
    for (i, p) in out.iter_mut().enumerate() {
        let wave = i / BEACON_WAVE_POINTS;
        let k = (i % BEACON_WAVE_POINTS) as f32 / BEACON_WAVE_POINTS as f32;
        // Cada onda nace un periodo despues de la anterior.
        let age = (time - wave as f32 * period).rem_euclid(life);
        let radius = 3.0 + age * 32.0;
        let angle = k * std::f32::consts::TAU;
        let ring = a * angle.cos() + b * angle.sin();
        let fade = (1.0 - age / life).powi(2) * smoothstep(0.0, 0.15, age);
        *p = Particle {
            pos: top + ring * radius + up * (age * 6.0),
            radius: 0.8 + age * 0.35,
            color: Vec3::new(1.0, 0.1, 0.05) * ((0.55 + 1.2 * near) * fade),
        };
    }
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Dos ejes perpendiculares a `axis` (unitario).
fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = helper.cross(axis).normalize();
    (u, axis.cross(u))
}

/// Color de cuerpo negro precalculado: `planckian_rgb` tiene logaritmos y
/// potencias, y se pide miles de veces por frame.
struct Blackbody {
    table: [Vec3; 64],
}

impl Blackbody {
    const MIN: f32 = 1200.0;
    const MAX: f32 = 4400.0;

    fn new() -> Self {
        let mut table = [Vec3::ZERO; 64];
        for (i, c) in table.iter_mut().enumerate() {
            *c = planckian_rgb(Self::MIN + (Self::MAX - Self::MIN) * i as f32 / 63.0);
        }
        Self { table }
    }

    fn get(&self, temperature: f32) -> Vec3 {
        let x = ((temperature - Self::MIN) / (Self::MAX - Self::MIN)).clamp(0.0, 1.0) * 63.0;
        let i = (x as usize).min(62);
        self.table[i].lerp(self.table[i + 1], x - i as f32)
    }
}

/// Esporas que flotan sobre Jardin, en un cubo que acompana a la camara. Se
/// guardan en el espacio del planeta (giran con el suelo, no se deslizan) y
/// cada una sube despacio meciendose. Brillan mas de noche.
pub fn spores(out: &mut [Particle], jardin: &Body, t: f64, camera: DVec3, night: f32, biolum: f32) {
    let center = jardin.position(t);
    let rotation = jardin.rotation(t);
    let inverse = rotation.conjugate();
    // Camara en el espacio del planeta y la vertical ahi.
    let cam_local = inverse.rotate(camera - center);
    let up = cam_local.normalize();
    let to_world = |local: DVec3| (center + rotation.rotate(local) - camera).as_vec3();
    let time = t as f32;
    let brightness = (0.4 + 2.6 * night) * biolum;
    let half = SPORE_BOX * 0.5;
    let cam = cam_local.as_vec3();
    let up32 = up.as_vec3();
    parallel::chunks_mut(out, 256, 256, |job, chunk| {
        for (k, particle) in chunk.iter_mut().enumerate() {
            let i = (job * 256 + k) as u32;
            let h = hash_u32(i ^ 0x5B0E_5000);
            let (h1, h2) = (hash_u32(h), hash_u32(h ^ 0x9E37));
            let base = Vec3::new(unit(h), unit(h1), unit(h2)) * SPORE_BOX;
            let phase = unit(h >> 4) * std::f32::consts::TAU;
            let sway = Vec3::new(
                (time * 0.31 + phase).sin(),
                0.0,
                (time * 0.27 + phase * 1.7).cos(),
            ) * 1.6;
            // Cada espora sube unas unidades durante su vida y vuelve a
            // nacer abajo: la subida queda acotada (si creciera con el tiempo
            // total, al moverse la camara las esporas saltarian).
            let speed = 0.35 + 0.4 * unit(h1 >> 4);
            let age = (time * speed / SPORE_RISE + unit(h2 >> 6)).fract();
            let rise = up32 * (age * SPORE_RISE);
            let life = smoothstep(0.0, 0.15, age) * smoothstep(1.0, 0.8, age);
            // Posicion repetida en un cubo centrado en la camara.
            let p = base + sway + rise - cam;
            let wrapped = Vec3::new(
                p.x.rem_euclid(SPORE_BOX) - half,
                p.y.rem_euclid(SPORE_BOX) - half,
                p.z.rem_euclid(SPORE_BOX) - half,
            );
            let local = cam + wrapped;
            let distance = wrapped.length();
            // Se apagan antes del borde del cubo: no se ve donde se repiten.
            let fade = smoothstep(half, half * 0.6, distance);
            let twinkle = 0.6 + 0.4 * (time * (1.5 + unit(h2 >> 4)) + phase).sin();
            let color = if h & 1 == 0 {
                Vec3::new(0.2, 1.0, 0.85)
            } else {
                Vec3::new(0.08, 0.55, 0.9)
            };
            *particle = Particle {
                pos: to_world(local.as_dvec3()),
                radius: 0.05 + 0.07 * unit(h1 >> 8),
                color: color * (brightness * fade * life * twinkle * 14.0),
            };
        }
    });
}

/// Fuego de los motores: un cono corto de plasma azul detras de la nave que
/// crece con el empuje.
pub fn exhaust(
    out: &mut [Particle],
    ship_pos: DVec3,
    ship_rot: DQuat,
    throttle: f32,
    length: f64,
    t: f64,
    camera: DVec3,
) {
    let back = ship_rot.rotate(DVec3::Z).as_vec3();
    let nozzle = (ship_pos + ship_rot.rotate(DVec3::Z * (length * 0.5)) - camera).as_vec3();
    let (a, b) = basis(back);
    let time = t as f32;
    for (i, particle) in out.iter_mut().enumerate() {
        let h = hash_u32(i as u32 ^ 0xE4A0_5700);
        let age = (time * 3.0 + unit(h)).fract();
        let angle = unit(hash_u32(h)) * std::f32::consts::TAU;
        let spread = 0.12 * unit(hash_u32(h ^ 7)) * age;
        let dir = back + (a * angle.cos() + b * angle.sin()) * spread;
        let reach = (1.0 + 3.5 * throttle) * age;
        let heat = (1.0 - age) * (1.0 - age);
        *particle = Particle {
            pos: nozzle + dir * reach,
            radius: 0.05 + 0.12 * age,
            color: Vec3::new(0.06, 0.5, 1.0).lerp(Vec3::new(0.04, 0.16, 0.6), age)
                * (throttle * heat * 0.65),
        };
    }
}

/// Que tan de noche es en un punto sobre un cuerpo: 0 con el sol alto, 1
/// cuando ya se oculto.
pub fn night_at(body: &Body, star: &Body, point: DVec3, t: f64) -> f32 {
    let center = body.position(t);
    let up = (point - center).normalize();
    let to_star = (star.position(t) - point).normalize();
    smoothstep(0.15, -0.1, up.dot(to_star) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::System;
    use std::path::Path;

    fn system() -> System {
        System::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap()
    }

    #[test]
    fn spores_stay_around_the_camera_and_are_brighter_at_night() {
        let sys = system();
        let jardin = &sys.bodies[2];
        let t = 50.0;
        let camera = jardin.position(t) + DVec3::new(0.0, 1.0, 0.0) * (jardin.radius + 10.0);
        let blank = Particle {
            pos: Vec3::ZERO,
            radius: 0.0,
            color: Vec3::ZERO,
        };
        let (mut day, mut night) = (vec![blank; SPORE_COUNT], vec![blank; SPORE_COUNT]);
        spores(&mut day, jardin, t, camera, 0.0, 1.0);
        spores(&mut night, jardin, t, camera, 1.0, 1.0);
        assert!(day.iter().all(|q| q.pos.length() < SPORE_BOX));
        let total = |v: &[Particle]| {
            v.iter()
                .map(|q| q.color.x + q.color.y + q.color.z)
                .sum::<f32>()
        };
        assert!(total(&night) > 3.0 * total(&day));
    }
}
