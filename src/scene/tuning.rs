//! Ajustes en vivo de los cuerpos: cambiar el radio de un planeta, su
//! periodo orbital o su giro mientras el juego corre, sin que nada salte.
//!
//! En la demostracion se puede pedir cambiar un parametro y explicar el
//! resultado. Estos cambios rehacen solo lo que depende del parametro: el
//! terreno se vuelve a generar por chunks alrededor de la camara (con el
//! mismo presupuesto por frame que siempre), la atmosfera recalcula su tabla
//! y los objetos se vuelven a repartir con la misma semilla.

use std::f64::consts::TAU;

use super::System;
use super::props::{self, PropSet};
use crate::config;
use crate::math::{DVec3, hash_u32};
use crate::shaders::AtmosphereModel;
use crate::terrain::{HeightField, Terrain};

impl System {
    /// Los objetos del cuerpo `index` con la semilla `seed`. Cada cuerpo
    /// mezcla la semilla con su indice para no repetir el mismo patron.
    pub fn scatter_body(&self, index: usize, seed: u32) -> PropSet {
        let body = &self.bodies[index];
        let Some(field) = &body.surface else {
            return PropSet::default();
        };
        let rules = props::rules_for(body.name);
        let planet_seed = hash_u32(seed ^ hash_u32(index as u32 + 1));
        let mut instances = props::scatter(body.radius, &**field, &rules, planet_seed);
        if let Some(site) = self.sites.iter().find(|s| s.body == index) {
            let frame = site.frame();
            let place = match body.name {
                "Jardin" => props::garden_grove,
                "Pira" => props::vent_field,
                "Requiem" => props::monolith_field,
                "Cementerio" => props::beacon_site,
                "Parpado" => props::mirror_shore,
                _ => |_: &mut Vec<_>, _: f64, _: &dyn HeightField, _: u32, _: (DVec3, DVec3, DVec3)| {},
            };
            place(&mut instances, body.radius, &**field, planet_seed, frame);
        }
        PropSet::new(instances, &self.prop_models)
    }

    /// Cambia el radio del cuerpo `index`. Conserva la gravedad en su
    /// superficie (`mu = g r^2`) y el tamano de su region en radios, y
    /// rehace el terreno, la atmosfera y los objetos.
    pub fn set_radius(&mut self, index: usize, radius: f64) {
        let radius = radius.max(10.0);
        if index == 0 {
            // La estrella: el ciclo parte de este radio.
            self.star_base = radius;
        }
        let body = &mut self.bodies[index];
        let gravity = body.mu / (body.radius * body.radius);
        if body.soi.is_finite() {
            body.soi *= radius / body.radius;
        }
        // Los planetas conservan su gravedad en la superficie; la estrella su
        // masa (las orbitas no dependen de su tamano).
        if body.surface.is_some() {
            body.mu = gravity * radius * radius;
        }
        body.radius = radius;
        if let Some(field) = &body.surface {
            self.terrains[index] =
                Some(Terrain::new(radius, field.clone(), config::TERRAIN_FINEST));
        }
        if let Some(model) = &self.atmospheres[index] {
            self.atmospheres[index] = Some(AtmosphereModel::new(radius as f32, model.params));
        }
        self.props[index] = self.scatter_body(index, self.prop_seed);
    }

    /// Cambia el periodo orbital sin que el cuerpo salte: la fase se corrige
    /// para que en el instante `t` siga en el mismo angulo. Si su giro iba
    /// acoplado a la orbita (Pira le da siempre la misma cara a la estrella),
    /// el giro cambia con ella.
    pub fn set_orbit_period(&mut self, index: usize, period: f64, t: f64) {
        let body = &mut self.bodies[index];
        if body.orbit_radius == 0.0 {
            return;
        }
        let locked = body.spin_period == body.orbit_period && body.spin_phase == body.orbit_phase;
        let period = period.max(5.0);
        let angle = body.orbit_phase + TAU * t / body.orbit_period;
        body.orbit_phase = angle - TAU * t / period;
        body.orbit_period = period;
        if locked {
            body.spin_period = period;
            body.spin_phase = body.orbit_phase;
        }
        let host = super::system::Host::of(index, &self.bodies[index]);
        for moon in &mut self.bodies {
            if moon.host.is_some_and(|h| h.index == index) {
                moon.host = Some(host);
            }
        }
    }

    /// Cambia el periodo de giro sin que la superficie salte.
    pub fn set_spin_period(&mut self, index: usize, period: f64, t: f64) {
        let body = &mut self.bodies[index];
        if body.spin_period == 0.0 {
            return;
        }
        let period = period.max(5.0);
        let angle = body.spin_phase + TAU * t / body.spin_period;
        body.spin_phase = angle - TAU * t / period;
        body.spin_period = period;
    }
}

#[cfg(test)]
mod tests {
    use crate::math::DVec3;
    use crate::scene::System;
    use std::path::Path;

    fn system() -> System {
        System::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap()
    }

    #[test]
    fn the_moon_follows_vigia_and_always_shows_it_the_same_face() {
        let mut sys = system();
        let (moon, vigia) = (sys.find("Parpado").unwrap(), sys.find("Vigia").unwrap());
        let site = sys.sites.iter().find(|s| s.body == moon).unwrap().normal;
        let facing = |sys: &System, t: f64| {
            let (m, v) = (&sys.bodies[moon], &sys.bodies[vigia]);
            let offset = v.position(t) - m.position(t);
            assert!((offset.length() - m.orbit_radius).abs() < 1e-6);
            m.rotation(t).rotate(site).dot(offset.normalize())
        };
        let first = facing(&sys, 10.0);
        for t in [100.0, 777.0, 5_000.0] {
            assert!((facing(&sys, t) - first).abs() < 1e-9);
        }
        // Si Vigia cambia de periodo, su luna la sigue.
        sys.set_orbit_period(vigia, 900.0, 300.0);
        assert!((facing(&sys, 2_000.0) - first).abs() < 1e-9);
    }

    #[test]
    fn every_site_has_its_landmarks_and_vents_wake_up_near_the_ship() {
        use crate::scene::props::PropKind;
        let sys = system();
        assert_eq!(sys.sites.len(), 5);
        for (body, kind, count) in [
            ("Pira", PropKind::Vent, 12),
            ("Requiem", PropKind::Monolith, 8),
            ("Cementerio", PropKind::Beacon, 1),
            ("Parpado", PropKind::Crystal, 14),
        ] {
            let i = sys.find(body).unwrap();
            let site = sys.sites.iter().find(|s| s.body == i).unwrap();
            // Los objetos del lugar estan alrededor de su centro.
            let close = sys.props[i]
                .instances
                .iter()
                .filter(|p| p.kind == kind)
                .filter(|p| (p.position.normalize() - site.normal).length() * sys.bodies[i].radius < 160.0)
                .count();
            assert!(close >= count, "{body}: {close}");
        }
        // Un geiser lejos de la nave casi siempre esta tranquilo; cerca, en
        // erupcion.
        let calm = (0..200)
            .filter(|k| crate::shaders::vent_activity(*k as f32 * 0.37, 0.3, 1000.0) < 0.2)
            .count();
        assert!(calm > 100, "{calm}");
        assert_eq!(crate::shaders::vent_activity(12.0, 0.3, 20.0), 1.0);
    }

    #[test]
    fn changing_periods_does_not_move_the_body_at_that_instant() {
        let mut sys = system();
        let i = sys.find("Jardin").unwrap();
        let t = 123.4;
        let (pos, rot) = (sys.bodies[i].position(t), sys.bodies[i].rotation(t));
        sys.set_orbit_period(i, 120.0, t);
        sys.set_spin_period(i, 33.0, t);
        assert!((sys.bodies[i].position(t) - pos).length() < 1e-6);
        let probe = DVec3::new(0.3, 0.5, 0.8);
        assert!((sys.bodies[i].rotation(t).rotate(probe) - rot.rotate(probe)).length() < 1e-9);
        // Y ahora va mas rapido.
        let later = sys.bodies[i].position(t + 1.0);
        assert!((later - pos).length() > 5.0);
    }

    #[test]
    fn a_tidally_locked_planet_stays_locked() {
        let mut sys = system();
        let i = sys.find("Pira").unwrap();
        sys.set_orbit_period(i, 90.0, 40.0);
        let body = &sys.bodies[i];
        assert_eq!(body.spin_period, body.orbit_period);
        // La cara -X sigue mirando a la estrella.
        for t in [40.0, 70.0, 100.0] {
            let to_star = (sys.bodies[0].position(t) - body.position(t)).normalize();
            let face = body.rotation(t).rotate(DVec3::new(-1.0, 0.0, 0.0));
            assert!(face.dot(to_star) > 0.99, "t {t}: {}", face.dot(to_star));
        }
    }

    #[test]
    fn a_bigger_planet_keeps_its_surface_gravity_and_gets_new_objects() {
        let mut sys = system();
        let i = sys.find("Requiem").unwrap();
        let g = sys.bodies[i].mu / sys.bodies[i].radius.powi(2);
        sys.set_radius(i, 240.0);
        let body = &sys.bodies[i];
        assert!((body.mu / (240.0 * 240.0) - g).abs() < 1e-9);
        assert_eq!(sys.terrains[i].as_ref().map(|t| t.radius), Some(240.0));
        let r = sys.props[i]
            .instances
            .first()
            .map(|p| p.position.length())
            .unwrap_or(0.0);
        assert!(r > 230.0, "los objetos se paran sobre el nuevo radio: {r}");
    }
}
