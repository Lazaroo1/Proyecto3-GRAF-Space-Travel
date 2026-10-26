//! Prediccion de la trayectoria de la nave, como en Kerbal Space Program.
//!
//! Sin asistente la nave es newtoniana: adonde va depende de su velocidad y
//! de la gravedad, y eso es dificil de adivinar. Cada frame se integra hacia
//! adelante su caida libre con el mismo modelo que la fisica (gravedad del
//! cuerpo dominante y cambio de marco al cruzar de region) y se dibuja el
//! camino. Si el camino choca con una superficie, termina ahi.
//!
//! Los puntos se guardan **relativos al cuerpo de su marco**: al dibujarlos
//! se suman a la posicion actual de ese cuerpo. Asi una orbita alrededor de
//! un planeta se ve como una elipse cerrada que viaja con el, no como la
//! espiral que trazaria en el espacio mientras el planeta avanza.

use super::ship::{dominant_body, ShipState};
use super::Body;
use crate::math::DVec3;

/// Segundos hacia adelante que se predicen.
pub const HORIZON: f64 = 90.0;
/// Paso de la integracion. Mas largo que el de la fisica (1/120 s): la
/// prediccion es para ver, y con 1/30 s el error de una orbita en 90 s es
/// menor que el ancho de la linea.
const DT: f64 = 1.0 / 30.0;
/// Se guarda un punto cada tantos pasos.
const RECORD_EVERY: usize = 4;

/// Un tramo del camino dentro del marco de un cuerpo.
#[derive(Clone, Debug, Default)]
pub struct Leg {
    pub frame: usize,
    /// Posiciones relativas al cuerpo `frame`.
    pub points: Vec<DVec3>,
}

#[derive(Clone, Debug, Default)]
pub struct Prediction {
    pub legs: Vec<Leg>,
    /// Donde chocaria, relativo al cuerpo del ultimo tramo.
    pub impact: Option<DVec3>,
}

/// Integra la caida libre de la nave desde el instante `t`.
pub fn predict(ship: &ShipState, bodies: &[Body], t: f64) -> Prediction {
    let mut frame = ship.frame;
    let (mut pos, mut vel) = (ship.rel_pos, ship.rel_vel);
    let mut time = t;
    let mut legs = vec![Leg {
        frame,
        points: vec![pos],
    }];
    let mut impact = None;
    let steps = (HORIZON / DT) as usize;
    for k in 1..=steps {
        let body = &bodies[frame];
        // Euler semi-implicito, igual que la fisica.
        let r2 = pos.length_squared();
        if body.mu > 0.0 && r2 > 0.0 {
            vel += pos * (-body.mu / (r2 * r2.sqrt()) * DT);
        }
        pos += vel * DT;
        time += DT;

        // Choque: primero una prueba barata contra la altura maxima posible.
        if let Some(field) = &body.surface {
            let top = body.radius + field.height_range().1 as f64;
            let r = pos.length();
            if r < top && r < body.radius + body.surface_height(pos, time) {
                impact = Some(pos);
                if let Some(leg) = legs.last_mut() {
                    leg.points.push(pos);
                }
                break;
            }
        }

        // Cambio de marco, conservando posicion y velocidad en el mundo.
        let world = body.position(time) + pos;
        let next = dominant_body(bodies, world, time);
        if next != frame {
            let world_vel = body.velocity(time) + vel;
            if let Some(leg) = legs.last_mut() {
                leg.points.push(pos);
            }
            frame = next;
            pos = world - bodies[next].position(time);
            vel = world_vel - bodies[next].velocity(time);
            legs.push(Leg {
                frame,
                points: vec![pos],
            });
        } else if k % RECORD_EVERY == 0 {
            if let Some(leg) = legs.last_mut() {
                leg.points.push(pos);
            }
        }
    }
    Prediction { legs, impact }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::DQuat;
    use crate::scene::System;
    use std::path::Path;

    fn system() -> System {
        System::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap()
    }

    #[test]
    fn a_circular_orbit_is_predicted_as_a_closed_ring() {
        let sys = system();
        let i = sys.find("Requiem").unwrap();
        let planet = &sys.bodies[i];
        let t = 10.0;
        let r = planet.radius * 2.0;
        let mut ship = ShipState::new(&sys.bodies, planet.position(t) + DVec3::X * r, DQuat::IDENTITY, t);
        assert_eq!(ship.frame, i);
        // Velocidad circular: v = sqrt(mu / r).
        ship.rel_vel = DVec3::Z * (planet.mu / r).sqrt();
        let p = predict(&ship, &sys.bodies, t);
        assert!(p.impact.is_none());
        assert_eq!(p.legs.len(), 1);
        for point in &p.legs[0].points {
            assert!((point.length() - r).abs() < 0.02 * r, "radio {}", point.length());
        }
    }

    #[test]
    fn falling_straight_down_ends_in_an_impact_on_the_surface() {
        let sys = system();
        let i = sys.find("Pira").unwrap();
        let planet = &sys.bodies[i];
        let t = 3.0;
        let dir = DVec3::new(0.2, 0.9, 0.3).normalize();
        let ship = ShipState::new(&sys.bodies, planet.position(t) + dir * (planet.radius + 60.0), DQuat::IDENTITY, t);
        let p = predict(&ship, &sys.bodies, t);
        let hit = p.impact.expect("deberia chocar");
        let ground = planet.radius + planet.surface_height(hit, t);
        assert!((hit.length() - ground).abs() < 3.0);
    }
}
