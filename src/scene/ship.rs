//! Modelo de vuelo de la nave.
//!
//! # Paso fijo
//! La fisica avanza en pasos de 1/120 s sin importar los fps. Con un paso
//! variable, la misma tecla presionada el mismo tiempo daria trayectorias
//! distintas en una computadora rapida y en una lenta.
//!
//! # Marco de referencia
//! La posicion y la velocidad se guardan **relativas** al cuerpo dominante:
//! el de region (soi) mas pequena que contiene a la nave. Cerca de un planeta
//! la nave viaja con el, como cualquier objeto en su orbita: si el planeta
//! recorre 13 unidades por segundo alrededor de la estrella, la nave tambien,
//! y acercarse no es perseguir un blanco que se escapa. Al cambiar de marco
//! se convierten posicion y velocidad para que nada salte en el mundo.
//!
//! # Asistente de vuelo
//! Con el asistente, las teclas piden una **velocidad** y los propulsores
//! corrigen la diferencia con aceleracion limitada: la nave acelera, frena y
//! se detiene de forma progresiva, y compensa la gravedad. Sin asistente es
//! newtoniana pura: soltar las teclas no la detiene y la gravedad la curva.

use super::{nearest_surface, Body};
use crate::config::*;
use crate::math::{DQuat, DVec3};

#[derive(Clone, Copy, Debug)]
pub struct ShipState {
    /// Cuerpo que sirve de marco de referencia.
    pub frame: usize,
    pub rel_pos: DVec3,
    pub rel_vel: DVec3,
    pub rot: DQuat,
    /// Velocidad angular en los ejes de la nave (rad/s).
    pub ang_vel: DVec3,
    /// Posicion en el mundo al final del ultimo paso.
    pub world_pos: DVec3,
}

/// Lo que el piloto pide en este paso.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// Empuje en ejes de la nave: x derecha, y arriba, z atras.
    pub thrust: DVec3,
    pub boost: bool,
    /// Orientacion a la que debe girar la nave.
    pub aim: DQuat,
}

#[derive(Clone, Copy, Debug)]
pub struct FlightSettings {
    pub assist: bool,
    pub gravity: bool,
}

impl Default for FlightSettings {
    fn default() -> Self {
        Self {
            assist: true,
            gravity: true,
        }
    }
}

/// Resumen del ultimo paso para el HUD.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlightInfo {
    pub speed: f64,
    pub max_speed: f64,
    pub altitude: f64,
    pub nearest: usize,
    pub frame: usize,
    pub contact: bool,
}

impl ShipState {
    /// Nave en reposo respecto a su cuerpo dominante.
    pub fn new(bodies: &[Body], world_pos: DVec3, rot: DQuat, t: f64) -> Self {
        let frame = dominant_body(bodies, world_pos, t);
        Self {
            frame,
            rel_pos: world_pos - bodies[frame].position(t),
            rel_vel: DVec3::ZERO,
            rot,
            ang_vel: DVec3::ZERO,
            world_pos,
        }
    }

    pub fn world_velocity(&self, bodies: &[Body], t: f64) -> DVec3 {
        bodies[self.frame].velocity(t) + self.rel_vel
    }

    /// Cambia de marco si hace falta, conservando posicion y velocidad en el mundo.
    fn update_frame(&mut self, bodies: &[Body], t: f64) {
        let world_pos = bodies[self.frame].position(t) + self.rel_pos;
        let next = dominant_body(bodies, world_pos, t);
        if next != self.frame {
            let world_vel = self.world_velocity(bodies, t);
            self.frame = next;
            self.rel_pos = world_pos - bodies[next].position(t);
            self.rel_vel = world_vel - bodies[next].velocity(t);
        }
    }
}

/// El cuerpo de region mas pequena que contiene a `p`. La estrella tiene
/// region infinita: es el marco cuando no hay otro.
fn dominant_body(bodies: &[Body], p: DVec3, t: f64) -> usize {
    let mut best = 0;
    let mut best_soi = f64::INFINITY;
    for (i, body) in bodies.iter().enumerate() {
        if body.soi < best_soi && (p - body.position(t)).length() < body.soi {
            best = i;
            best_soi = body.soi;
        }
    }
    best
}

/// Avanza la nave un paso de `dt` segundos desde el tiempo `t`.
pub fn step(
    s: &mut ShipState,
    controls: &Controls,
    settings: &FlightSettings,
    bodies: &[Body],
    t: f64,
    dt: f64,
) -> FlightInfo {
    s.update_frame(bodies, t);
    let body = &bodies[s.frame];

    let (nearest, altitude) = nearest_surface(bodies, s.world_pos, t);
    let boost = if controls.boost { BOOST } else { 1.0 };
    let max_speed = (altitude * SPEED_PER_ALTITUDE).clamp(MIN_SPEED, MAX_SPEED) * boost;
    let max_accel = max_speed * ACCEL_PER_SPEED;

    // Giro: el error entre la orientacion actual y la mira, como eje por
    // angulo en ejes de la nave, pide una velocidad angular; la aceleracion
    // angular limitada hace que el giro arranque y frene progresivamente.
    let error = (s.rot.conjugate() * controls.aim).to_scaled_axis();
    let desired = (error * TURN_GAIN).clamp_length(MAX_TURN_RATE);
    s.ang_vel += (desired - s.ang_vel).clamp_length(MAX_TURN_ACCEL * dt);
    s.rot = (s.rot * DQuat::from_scaled_axis(s.ang_vel * dt)).normalize();

    // Gravedad del cuerpo de referencia: a = -mu r / |r|^3.
    let gravity = if settings.gravity && body.mu > 0.0 {
        let r2 = s.rel_pos.length_squared();
        s.rel_pos * (-body.mu / (r2 * r2.sqrt()))
    } else {
        DVec3::ZERO
    };

    let thrust = controls.thrust.clamp_length(1.0);
    let engine = if settings.assist {
        let local_vel = s.rot.conjugate().rotate(s.rel_vel);
        let correction = s.rot.rotate((thrust * max_speed - local_vel) * ASSIST_GAIN);
        (correction - gravity).clamp_length(max_accel)
    } else {
        s.rot.rotate(thrust * max_accel)
    };

    // Euler semi-implicito: la posicion avanza con la velocidad ya actualizada,
    // que conserva mucho mejor la energia de una orbita que Euler explicito.
    s.rel_vel += (engine + gravity) * dt;
    s.rel_pos += s.rel_vel * dt;

    // Colision con la superficie (por ahora una esfera): se empuja la nave
    // afuera y se quita la velocidad hacia adentro, con un rebote pequeno.
    let floor = body.radius + COLLISION_CLEARANCE;
    let dist = s.rel_pos.length();
    let contact = dist < floor && dist > 0.0;
    if contact {
        let n = s.rel_pos / dist;
        s.rel_pos = n * floor;
        let vn = s.rel_vel.dot(n);
        if vn < 0.0 {
            s.rel_vel -= n * (vn * (1.0 + RESTITUTION));
        }
    }

    s.world_pos = body.position(t + dt) + s.rel_pos;
    FlightInfo {
        speed: s.rel_vel.length(),
        max_speed,
        altitude,
        nearest,
        frame: s.frame,
        contact,
    }
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
    fn assist_hovers_against_gravity_and_the_ship_follows_the_planet() {
        let sys = system();
        let planet = &sys.bodies[1];
        let t0 = 10.0;
        let pos = planet.position(t0) + DVec3::Y * (planet.radius + 20.0);
        let mut ship = ShipState::new(&sys.bodies, pos, DQuat::IDENTITY, t0);
        assert_eq!(ship.frame, 1);
        let controls = Controls::default();
        let settings = FlightSettings::default();
        let mut t = t0;
        for _ in 0..(3.0 * PHYSICS_HZ) as usize {
            step(&mut ship, &controls, &settings, &sys.bodies, t, PHYSICS_DT);
            t += PHYSICS_DT;
        }
        let altitude = (ship.world_pos - planet.position(t)).length() - planet.radius;
        assert!((altitude - 20.0).abs() < 0.5, "altitud {altitude}");
        assert!(ship.rel_vel.length() < 0.2);
    }

    #[test]
    fn changing_frames_keeps_the_world_velocity() {
        let sys = system();
        let planet = &sys.bodies[1];
        let t = 5.0;
        // Justo fuera de la region del planeta, en el marco de la estrella.
        let pos = planet.position(t) + DVec3::X * (planet.soi + 1.0);
        let mut ship = ShipState::new(&sys.bodies, pos, DQuat::IDENTITY, t);
        assert_eq!(ship.frame, 0);
        ship.rel_vel = DVec3::new(-30.0, 2.0, 1.0);
        let before = ship.world_velocity(&sys.bodies, t);
        ship.rel_pos = ship.rel_pos - DVec3::X * 2.0;
        ship.update_frame(&sys.bodies, t);
        assert_eq!(ship.frame, 1);
        assert!((ship.world_velocity(&sys.bodies, t) - before).length() < 1e-9);
    }
}
