//! Núcleo volumétrico de la Variante predeterminada del Proyecto 2.
//! Se conserva su ruido, transferencia, corrimiento y RK4; no usa minifb.
//! Procedencia: Proyecto2-graf, src/{scene,math}, autor del mismo proyecto.
#![allow(dead_code, unused_imports)]
pub mod config;
pub mod disk;
pub mod math;
pub mod photon;
pub mod relativity;
use math::Vec2;
use photon::Photon;
pub fn volume_step(photon: &Photon, medium: disk::Medium) -> f32 {
    let enhanced = medium.enhanced;
    let base = photon.step_length();
    let radius = Vec2::new(photon.pos.x, photon.pos.z).length();
    let outer_radius = if enhanced {
        config::OUTER_GAS_END
    } else {
        config::DISK_OUTER_RADIUS
    };
    if !(config::DISK_INNER_RADIUS - 1.0..=outer_radius + 2.0).contains(&radius) {
        return base;
    }
    // Fuera del disco termico resolver solo el espesor de la envoltura gris;
    // lejos de ambos medios conservar el paso de vacio de la geodesica.
    if enhanced {
        let thermal_distance = (radius - config::DISK_OUTER_RADIUS)
            .max(photon.pos.y.abs() - 3.5 * radius * medium.atmosphere_ratio);
        if thermal_distance > 0.0 {
            let speed = photon.vel.length().max(1e-6);
            let height = radius * medium.outer_ratio;
            let distance = photon.pos.y.abs() - height * 3.5;
            if distance > height {
                let approach = photon.vel.y.abs() + 3.5 * medium.outer_ratio * speed;
                return base.min((distance / approach.max(1e-6) * 0.8).max(config::MIN_STEP));
            }
            return base
                .min(0.4 / speed)
                .min(height * 0.65 / photon.vel.y.abs().max(1e-6))
                .max(config::MIN_STEP);
        }
    }
    let height = radius * config::DISK_HEIGHT_RATIO;
    let speed = photon.vel.length().max(1e-6);
    let atmosphere_height = radius * medium.atmosphere_ratio;
    let base = if photon.pos.y.abs() < atmosphere_height * 4.0 {
        base.min(config::DISK_VOLUME_STEP / speed)
            .min(atmosphere_height * 0.65 / photon.vel.y.abs().max(1e-6))
    } else {
        base
    };
    let distance = photon.pos.y.abs() - height * 3.5;
    if distance > height {
        // Distancia conservadora a la envolvente conica; no saltarse el gas.
        let approach = photon.vel.y.abs() + 3.5 * config::DISK_HEIGHT_RATIO * speed;
        return base.min((distance / approach.max(1e-6) * 0.8).max(config::MIN_STEP));
    }
    base.min(config::DISK_VOLUME_STEP / speed)
        .min(height * 0.65 / photon.vel.y.abs().max(1e-6))
        .max(config::MIN_STEP)
}
