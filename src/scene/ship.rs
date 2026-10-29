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
//! newtoniana: soltar las teclas no la detiene y la gravedad la curva. Los
//! propulsores empujan con una aceleracion mas suave (siempre mas que la
//! gravedad, para poder despegar) y dejan de empujar hacia adelante al
//! llegar a la velocidad maxima; la gravedad si puede pasarla. Antes el
//! empuje sin asistente era el del asistente, que crece con el exceso de
//! velocidad para frenar a tiempo: ir rapido empujaba mas y la nave se
//! desbocaba.
//!
//! # Modo local
//! Cerca de un planeta el vuelo se mide contra su **superficie**, que gira:
//! quedarse quieto es flotar sobre el mismo punto del suelo. Las teclas
//! mueven sobre el plano tangente siguiendo la curvatura, y la nave sostiene
//! sola su altura sobre el terreno (mirando el suelo que viene adelante).
//! Rodear un planeta es mantener W presionada; el crucero lo hace solo.

use std::f64::consts::TAU;

use super::{Body, nearest_surface};
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
    /// Altura sobre el terreno que sostiene el modo local.
    pub hold: Option<f64>,
}

/// Lo que el piloto pide en este paso.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// Empuje en ejes de la nave: x derecha, y arriba, z atras.
    pub thrust: DVec3,
    pub boost: bool,
    /// Freno de emergencia: prioridad sobre empuje, turbo y crucero.
    pub brake: bool,
    /// Orientacion a la que debe girar la nave.
    pub aim: DQuat,
}

#[derive(Clone, Copy, Debug)]
pub struct FlightSettings {
    pub assist: bool,
    pub gravity: bool,
    /// Modo local cerca de los planetas (necesita el asistente).
    pub local: bool,
    /// Crucero: en el modo local, avanza solo rodeando el planeta.
    pub cruise: bool,
}

impl Default for FlightSettings {
    fn default() -> Self {
        Self {
            assist: true,
            gravity: true,
            local: true,
            cruise: false,
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
    /// Si el modo local estuvo activo en este paso.
    pub local: bool,
    /// Altura sobre el terreno que sostiene el modo local.
    pub hold: Option<f64>,
    pub braking: bool,
    pub boundary: bool,
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
            hold: None,
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

/// Velocidad angular del giro de un cuerpo sobre su eje, en ejes del mundo.
/// El giro es `Rz(inclinacion) * Ry(angulo)`: el eje es +Y inclinado.
pub fn angular_velocity(body: &Body) -> DVec3 {
    if body.spin_period == 0.0 {
        return DVec3::ZERO;
    }
    DQuat::from_axis_angle(DVec3::Z, body.axial_tilt).rotate(DVec3::Y) * (TAU / body.spin_period)
}

/// Rumbo sobre el plano tangente a `up`: el frente de la mira sin su
/// inclinacion, y su derecha. Mirando justo arriba o abajo, el rumbo sale
/// del "arriba" de la mira.
pub fn heading(aim: DQuat, up: DVec3) -> (DVec3, DVec3) {
    let look = aim.forward();
    let mut forward = look - up * look.dot(up);
    if forward.length_squared() < 1e-8 {
        let top = aim.up() * -look.dot(up).signum();
        forward = top - up * top.dot(up);
    }
    let forward = forward.normalize();
    (forward, forward.cross(up))
}

/// El cuerpo de region mas pequena que contiene a `p`. La estrella tiene
/// region infinita: es el marco cuando no hay otro.
pub fn dominant_body(bodies: &[Body], p: DVec3, t: f64) -> usize {
    // Se solapan las SOI en este sistema compacto: la capa de vuelo junto
    // a una superficie pertenece a ese planeta, aunque otra SOI sea menor.
    let surface = bodies
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, body)| {
            let normalized = (p - body.position(t)).length() / body.radius;
            (normalized < 1.0 + LOCAL_RANGE * 1.15).then_some((i, normalized))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((i, _)) = surface {
        return i;
    }
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
    // Antes el freno se debilitaba al acercarse al destino: velocidad de
    // crucero, pero aceleracion de vuelo rasante para detenerse.
    let overspeed = (s.rel_vel.length() - max_speed).max(0.0);
    let mut max_accel = (max_speed * ACCEL_PER_SPEED).max(overspeed * BRAKE_GAIN);
    if settings.assist && controls.thrust.length_squared() < 0.001 {
        max_accel = max_accel.max((s.rel_vel.length() * BRAKE_GAIN).max(BRAKE_MIN_ACCEL));
    }

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

    // Altura sobre el terreno del cuerpo de referencia.
    let frame_altitude = s.rel_pos.length() - body.radius - body.surface_height(s.rel_pos, t);
    let local = settings.assist
        && settings.local
        && s.frame != 0
        && frame_altitude < LOCAL_RANGE * body.radius;
    if !local {
        s.hold = None;
    }

    let mut thrust = controls.thrust.clamp_length(1.0);
    let engine = if controls.brake {
        let target = if local {
            angular_velocity(body).cross(s.rel_pos)
        } else {
            DVec3::ZERO
        };
        ((target - s.rel_vel) * BRAKE_GAIN - gravity)
            .clamp_length((s.rel_vel.length() * BRAKE_GAIN).max(BRAKE_MIN_ACCEL))
    } else if local {
        if settings.cruise && thrust.z == 0.0 {
            thrust.z = -CRUISE_THRUST;
        }
        local_engine(
            s,
            body,
            controls.aim,
            thrust,
            max_speed,
            max_accel,
            gravity,
            t,
        )
    } else if settings.assist {
        let local_vel = s.rot.conjugate().rotate(s.rel_vel);
        let gain = if thrust.length_squared() < 0.001 || overspeed > 1.0 {
            BRAKE_GAIN
        } else {
            ASSIST_GAIN
        };
        let correction = s.rot.rotate((thrust * max_speed - local_vel) * gain);
        (correction - gravity).clamp_length(max_accel)
    } else {
        let accel = (max_speed * MANUAL_ACCEL_PER_SPEED).max(gravity.length() * 1.5 + 2.0);
        let mut push = s.rot.rotate(thrust);
        let speed = s.rel_vel.length();
        if speed > 1e-6 {
            // Cerca de la velocidad maxima se apaga el empuje hacia adelante.
            let along = s.rel_vel / speed;
            let ahead = push.dot(along);
            if ahead > 0.0 {
                let room = ((max_speed - speed) / (0.15 * max_speed)).clamp(0.0, 1.0);
                push -= along * (ahead * (1.0 - room));
            }
        }
        push * accel
    };

    // Euler semi-implicito: la posicion avanza con la velocidad ya actualizada,
    // que conserva mucho mejor la energia de una orbita que Euler explicito.
    s.rel_vel += (engine + gravity) * dt;
    s.rel_pos += s.rel_vel * dt;

    // Colision con el terreno, con la misma funcion de altura que se dibuja:
    // se empuja la nave afuera y se quita la velocidad hacia adentro, con un
    // rebote pequeno.
    let floor = body.radius + body.surface_height(s.rel_pos, t + dt) + COLLISION_CLEARANCE;
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
    // La franja exterior limita solo el movimiento hacia afuera. El piloto
    // puede girar y volver, sin perderse a distancias ilimitadas.
    let star = bodies[0].position(t + dt);
    let offset = s.world_pos - star;
    let distance = offset.length();
    let limit = bodies
        .iter()
        .map(|b| b.orbit_radius + b.radius)
        .fold(0.0, f64::max)
        + SYSTEM_MARGIN;
    let boundary = distance > limit - 1200.0;
    if boundary && distance > 0.0 {
        let radial = offset / distance;
        let outward = s.world_velocity(bodies, t + dt).dot(radial);
        let allowed = ((limit - distance).max(0.0) * 0.45).min(max_speed);
        if outward > allowed {
            s.rel_vel -= radial * (outward - allowed);
        }
        if distance > limit {
            s.world_pos = star + radial * limit;
            s.rel_pos = s.world_pos - body.position(t + dt);
        }
    }
    FlightInfo {
        speed: s.rel_vel.length(),
        max_speed,
        altitude,
        nearest,
        frame: s.frame,
        contact,
        local,
        hold: s.hold,
        braking: controls.brake,
        boundary,
    }
}

/// Empuje del modo local. La velocidad que se pide es relativa a la
/// superficie que gira; ademas de corregir la diferencia, los propulsores
/// pagan la aceleracion de girar con el planeta y anulan la gravedad.
#[allow(clippy::too_many_arguments)]
fn local_engine(
    s: &mut ShipState,
    body: &Body,
    aim: DQuat,
    thrust: DVec3,
    max_speed: f64,
    max_accel: f64,
    gravity: DVec3,
    t: f64,
) -> DVec3 {
    let up = s.rel_pos.normalize();
    let omega = angular_velocity(body);
    let surface = omega.cross(s.rel_pos);
    let (forward, right) = heading(aim, up);
    // Empuje en ejes de la nave: x derecha, z atras.
    let horizontal = (right * thrust.x - forward * thrust.z) * max_speed;
    let altitude_at = |p: DVec3| p.length() - body.radius - body.surface_height(p, t);
    let vertical = if thrust.y != 0.0 {
        s.hold = None;
        thrust.y * max_speed
    } else {
        let now = altitude_at(s.rel_pos);
        let target = *s.hold.get_or_insert(now);
        // Lo que importa es el suelo mas alto entre el de ahora y el que se
        // tendra debajo en medio segundo y en un segundo.
        let lowest = [0.5, 1.0]
            .iter()
            .map(|&secs| altitude_at(s.rel_pos + horizontal * secs))
            .fold(now, f64::min);
        ((target - lowest) * HOLD_GAIN).clamp(-max_speed, max_speed)
    };
    let desired = surface + horizontal + up * vertical;
    let follow = omega.cross(surface);
    ((desired - s.rel_vel) * ASSIST_GAIN + follow - gravity).clamp_length(max_accel)
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
    fn emergency_brake_overrides_thrust_boost_and_unassisted_flight() {
        let sys = system();
        let pos = DVec3::new(0.0, 6000.0, 0.0);
        let mut ship = ShipState::new(&sys.bodies, pos, DQuat::IDENTITY, 0.0);
        ship.rel_vel = DVec3::X * 1000.0;
        let settings = FlightSettings {
            assist: false,
            gravity: false,
            cruise: true,
            ..FlightSettings::default()
        };
        let controls = Controls {
            thrust: DVec3::X,
            boost: true,
            brake: true,
            ..Controls::default()
        };
        for k in 0..120 {
            step(
                &mut ship,
                &controls,
                &settings,
                &sys.bodies,
                k as f64 * PHYSICS_DT,
                PHYSICS_DT,
            );
        }
        assert!(
            ship.rel_vel.length() < 0.5,
            "velocidad residual {}",
            ship.rel_vel.length()
        );
        assert!(
            (ship.world_pos - pos).length() < 130.0,
            "distancia de frenado excesiva"
        );
    }

    /// Sin asistente, mantener W lleva a la velocidad maxima y no mas alla:
    /// ir rapido no hace mas fuerte el empuje (la aceleracion extra es solo
    /// para que el asistente frene).
    #[test]
    fn unassisted_thrust_does_not_run_away() {
        let sys = system();
        let body = &sys.bodies[1];
        let t0 = 10.0;
        let pos = body.position(t0) + DVec3::Y * (body.radius + 300.0);
        let mut ship = ShipState::new(&sys.bodies, pos, DQuat::IDENTITY, t0);
        let settings = FlightSettings {
            assist: false,
            ..FlightSettings::default()
        };
        let controls = Controls {
            thrust: -DVec3::Z,
            aim: DQuat::IDENTITY,
            ..Controls::default()
        };
        let mut top: f64 = 0.0;
        for k in 0..360 {
            let info = step(&mut ship, &controls, &settings, &sys.bodies, t0 + k as f64 * PHYSICS_DT, PHYSICS_DT);
            top = top.max(info.max_speed);
        }
        // Los propulsores no pasan de la velocidad maxima; la gravedad (unos
        // 3 u/s2 aqui) puede sumar un poco en tres segundos.
        let reachable = top * 1.05 + 3.0 * 4.0;
        assert!(ship.rel_vel.length() < reachable, "{:.0} u/s, deberia ser menos de {reachable:.0}", ship.rel_vel.length());
    }

    #[test]
    fn approaching_a_planet_keeps_braking_authority_when_speed_limit_drops() {
        let sys = system();
        let body = &sys.bodies[1];
        let pos = body.position(0.0) + DVec3::Y * (body.radius + 220.0);
        let mut ship = ShipState::new(&sys.bodies, pos, DQuat::IDENTITY, 0.0);
        ship.rel_vel = -DVec3::Y * 1000.0;
        let settings = FlightSettings {
            local: false,
            ..FlightSettings::default()
        };
        for k in 0..120 {
            let info = step(
                &mut ship,
                &Controls::default(),
                &settings,
                &sys.bodies,
                k as f64 * PHYSICS_DT,
                PHYSICS_DT,
            );
            assert!(!info.contact, "no deberia estrellarse mientras frena");
        }
        assert!(
            ship.rel_vel.length() < 8.0,
            "debe frenar antes de rebasar el planeta"
        );
        assert!(ship.rel_pos.length() > body.radius + 60.0);
    }

    #[test]
    fn navigation_boundary_prevents_endless_escape_and_allows_return() {
        let sys = system();
        let limit = sys
            .bodies
            .iter()
            .map(|b| b.orbit_radius + b.radius)
            .fold(0.0, f64::max)
            + SYSTEM_MARGIN;
        let mut ship = ShipState::new(&sys.bodies, DVec3::Y * (limit - 50.0), DQuat::IDENTITY, 0.0);
        ship.rel_vel = DVec3::Y * 900.0;
        let settings = FlightSettings {
            assist: false,
            gravity: false,
            ..FlightSettings::default()
        };
        for k in 0..240 {
            step(
                &mut ship,
                &Controls::default(),
                &settings,
                &sys.bodies,
                k as f64 * PHYSICS_DT,
                PHYSICS_DT,
            );
        }
        assert!(ship.world_pos.length() <= limit + 0.01);
        ship.rel_vel = -DVec3::Y * 100.0;
        let start = ship.world_pos.length();
        for k in 240..360 {
            step(
                &mut ship,
                &Controls::default(),
                &settings,
                &sys.bodies,
                k as f64 * PHYSICS_DT,
                PHYSICS_DT,
            );
        }
        assert!(
            ship.world_pos.length() < start - 90.0,
            "el borde debe dejar regresar"
        );
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
    fn local_mode_keeps_the_altitude_and_goes_around_the_planet() {
        let sys = system();
        let planet = &sys.bodies[2];
        let t0 = 20.0;
        let dir = DVec3::new(0.3, 0.8, 0.5).normalize();
        let ground = planet.surface_height(dir, t0);
        let pos = planet.position(t0) + dir * (planet.radius + ground + 25.0);
        let aim = DQuat::look_rotation(dir.cross(DVec3::X), dir);
        let mut ship = ShipState::new(&sys.bodies, pos, aim, t0);
        let settings = FlightSettings {
            cruise: true,
            ..FlightSettings::default()
        };
        let controls = Controls {
            aim,
            ..Controls::default()
        };
        let mut t = t0;
        let mut info = FlightInfo::default();
        let (mut low, mut high) = (f64::MAX, f64::MIN);
        for k in 0..(20.0 * PHYSICS_HZ) as usize {
            info = step(&mut ship, &controls, &settings, &sys.bodies, t, PHYSICS_DT);
            t += PHYSICS_DT;
            if k > (2.0 * PHYSICS_HZ) as usize {
                let rel = ship.world_pos - planet.position(t);
                let altitude = rel.length() - planet.radius - planet.surface_height(rel, t);
                low = low.min(altitude);
                high = high.max(altitude);
            }
        }
        assert!(info.local);
        assert!(low > 12.0 && high < 40.0, "altitud entre {low} y {high}");
        // Avanzo sobre la superficie: el angulo recorrido es grande.
        let rel = (ship.world_pos - planet.position(t)).normalize();
        assert!(rel.dot(dir) < 0.95, "avance {}", rel.dot(dir).acos());
    }

    #[test]
    fn local_mode_hovers_over_the_same_ground_while_the_planet_spins() {
        let sys = system();
        let planet = &sys.bodies[2];
        let t0 = 30.0;
        let dir = DVec3::new(0.9, 0.1, 0.2).normalize();
        let pos =
            planet.position(t0) + dir * (planet.radius + planet.surface_height(dir, t0) + 15.0);
        let aim = DQuat::look_rotation(dir.cross(DVec3::Y), dir);
        let mut ship = ShipState::new(&sys.bodies, pos, aim, t0);
        // El punto del suelo bajo la nave, en el espacio del planeta.
        let below = |s: &ShipState, t: f64| {
            planet
                .rotation(t)
                .conjugate()
                .rotate(s.world_pos - planet.position(t))
                .normalize()
        };
        let controls = Controls {
            aim,
            ..Controls::default()
        };
        let mut t = t0;
        let mut run = |ship: &mut ShipState, seconds: f64| {
            for _ in 0..(seconds * PHYSICS_HZ) as usize {
                step(
                    ship,
                    &controls,
                    &FlightSettings::default(),
                    &sys.bodies,
                    t,
                    PHYSICS_DT,
                );
                t += PHYSICS_DT;
            }
            t
        };
        // Al empezar la nave esta quieta y el suelo gira: el asistente tarda
        // un momento en igualarlo. Despues ya no se desliza.
        let settled = run(&mut ship, 3.0);
        let start = below(&ship, settled);
        let end = run(&mut ship, 10.0);
        let drift = below(&ship, end).dot(start).clamp(-1.0, 1.0).acos() * planet.radius;
        assert!(drift < 1.0, "se deslizo {drift} sobre el suelo");
    }

    #[test]
    fn changing_frames_keeps_the_world_velocity() {
        let sys = system();
        let planet = &sys.bodies[1];
        let t = 5.0;
        // Justo fuera de la region del planeta, en otro marco. Las regiones
        // de influencia pueden solaparse en un sistema compacto.
        let pos = planet.position(t) + DVec3::X * (planet.soi + 1.0);
        let mut ship = ShipState::new(&sys.bodies, pos, DQuat::IDENTITY, t);
        assert_ne!(ship.frame, 1);
        ship.rel_vel = DVec3::new(-30.0, 2.0, 1.0);
        let before = ship.world_velocity(&sys.bodies, t);
        ship.rel_pos = ship.rel_pos - DVec3::X * 2.0;
        ship.update_frame(&sys.bodies, t);
        assert_eq!(ship.frame, 1);
        assert!((ship.world_velocity(&sys.bodies, t) - before).length() < 1e-9);
    }
}
