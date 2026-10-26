//! Efectos de particulas: el chorro de brasas que Pira pierde hacia la
//! estrella, las esporas que flotan sobre Jardin y el escape de la nave.
//!
//! Ninguna particula guarda estado: cada una es una **funcion del tiempo**.
//! Su edad es `fract(t / vida + fase)`, con una fase sacada de un hash de su
//! indice, y su posicion sale de la edad. Asi no hay que simular nada frame
//! a frame, el efecto se ve igual a cualquier fps y rebobinar el tiempo las
//! devuelve exactamente a donde estaban.

use crate::math::blackbody::planckian_rgb;
use crate::math::{hash_u32, smoothstep, DQuat, DVec3, Vec3};
use crate::parallel;
use crate::render::Particle;

use super::Body;

/// Particulas del chorro de Pira.
pub const STREAM_COUNT: usize = 6000;
/// Segundos que tarda una brasa en caer de Pira a la estrella.
const STREAM_LIFE: f32 = 36.0;
/// Esporas alrededor de la camara cerca de Jardin.
pub const SPORE_COUNT: usize = 1400;
/// Lado del cubo de esporas (se repite al moverse la camara).
const SPORE_BOX: f32 = 90.0;
/// Cuanto sube una espora en su vida, en unidades del mundo.
const SPORE_RISE: f32 = 8.0;
/// Particulas del escape de la nave.
pub const EXHAUST_COUNT: usize = 260;

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

/// El chorro de brasas: sale de la cara de Pira que mira a la estrella y
/// cae en espiral hacia ella, quedandose atras de la orbita del planeta (la
/// materia conserva la velocidad con la que salio). Cada brasa recorre una
/// curva de Bezier cuadratica y se calienta al acercarse: de rojo oscuro a
/// amarillo.
/// `sink` es el radio donde termina: la superficie de la estrella o, despues
/// del colapso, el borde del disco de Gargantua.
pub fn pira_stream(out: &mut [Particle], pira: &Body, star: &Body, sink: f64, t: f64, camera: DVec3) {
    let p = pira.position(t);
    let star_pos = star.position(t);
    let to_star = (star_pos - p).normalize();
    let along_orbit = pira.velocity(t).normalize();
    let start = p + to_star * (pira.radius * 0.97);
    let end = star_pos - to_star * (sink * 0.98);
    let span = (end - start).length();
    // La curva se queda atras del planeta y un poco por fuera.
    let control = (start + end) * 0.5 - along_orbit * (span * 0.32);
    // Relativo a la camara en f64 y despues a f32.
    let (start, control, end) = ((start - camera).as_vec3(), (control - camera).as_vec3(), (end - camera).as_vec3());
    let (side_a, side_b) = basis(to_star.as_vec3());
    let width = pira.radius as f32 * 0.45;
    let colors = Blackbody::new();
    let time = t as f32;
    parallel::chunks_mut(out, 512, 512, |job, chunk| {
        for (k, particle) in chunk.iter_mut().enumerate() {
            let i = (job * 512 + k) as u32;
            let h = hash_u32(i ^ 0x51A3_7F00);
            let age = (time / STREAM_LIFE + unit(h)).fract();
            // Cae cada vez mas rapido.
            let s = age.powf(1.6);
            let a = 1.0 - s;
            let center = start * (a * a) + control * (2.0 * a * s) + end * (s * s);
            // Helice alrededor del eje del chorro, que se angosta al caer.
            let h2 = hash_u32(h);
            let angle = unit(h2) * std::f32::consts::TAU + s * 9.0;
            let radius = width * unit(hash_u32(h2)).sqrt() * (1.0 - 0.75 * s);
            let pos = center + (side_a * angle.cos() + side_b * angle.sin()) * radius;
            let fade = smoothstep(0.0, 0.06, s) * smoothstep(1.0, 0.9, s);
            let temperature = 1500.0 + 2600.0 * s + 300.0 * unit(h2 >> 3);
            *particle = Particle {
                pos,
                radius: 1.0 + 1.6 * unit(h >> 5),
                color: colors.get(temperature) * (6.0 * fade),
            };
        }
    });
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
            let sway = Vec3::new((time * 0.31 + phase).sin(), 0.0, (time * 0.27 + phase * 1.7).cos()) * 1.6;
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
                Vec3::new(1.0, 0.3, 0.8)
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
pub fn exhaust(out: &mut [Particle], ship_pos: DVec3, ship_rot: DQuat, throttle: f32, length: f64, t: f64, camera: DVec3) {
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
            radius: 0.12 + 0.25 * age,
            color: Vec3::new(0.45, 0.65, 1.0).lerp(Vec3::new(1.0, 0.55, 0.3), age) * (throttle * heat * 9.0),
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
    fn the_stream_goes_from_pira_to_the_star() {
        let sys = system();
        let (star, pira) = (&sys.bodies[0], &sys.bodies[1]);
        let t = 12.0;
        let mut out = vec![
            Particle {
                pos: Vec3::ZERO,
                radius: 0.0,
                color: Vec3::ZERO,
            };
            STREAM_COUNT
        ];
        pira_stream(&mut out, pira, star, star.radius, t, DVec3::ZERO);
        let (p, s) = (pira.position(t).as_vec3(), star.position(t).as_vec3());
        let span = (p - s).length();
        let near_pira = out.iter().filter(|q| (q.pos - p).length() < pira.radius as f32 * 1.5).count();
        let near_star = out.iter().filter(|q| (q.pos - s).length() < star.radius as f32 * 1.3).count();
        assert!(near_pira > 0 && near_star > 0);
        // Ninguna se va lejos del tramo entre los dos.
        assert!(out.iter().all(|q| (q.pos - p).length() < span * 1.2 && (q.pos - s).length() < span * 1.2));
        // Las que nacen junto a Pira son mas frias (mas rojas) que las que caen.
        let born = out.iter().filter(|q| (q.pos - p).length() < pira.radius as f32 * 1.5 && q.color.x > 0.0);
        assert!(born.clone().all(|q| q.color.z / q.color.x < 0.6));
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
        let total = |v: &[Particle]| v.iter().map(|q| q.color.x + q.color.y + q.color.z).sum::<f32>();
        assert!(total(&night) > 3.0 * total(&day));
    }
}
