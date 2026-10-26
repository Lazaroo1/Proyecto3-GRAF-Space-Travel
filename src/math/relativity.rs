//! Relatividad de Schwarzschild para el disco de Gargantua, portada del
//! Proyecto 2 (`relativity.rs`). Unidades: `rs = 2GM/c^2 = 1`, `M = 1/2`,
//! velocidades en unidades de `c`.
//!
//! | Radio | Valor | Que es |
//! | --- | --- | --- |
//! | `rs` | 1 | Horizonte de eventos |
//! | `3M` | 1.5 | Esfera de fotones |
//! | `3 sqrt(3) M` | 2.598 | Radio aparente de la sombra visto de lejos |
//! | `6M` | 3 | ISCO: la orbita circular estable mas cercana |

/// Masa del agujero en unidades de `rs`.
pub const MASS: f32 = 0.5;
/// Borde interno del disco: la ISCO.
pub const DISK_INNER: f32 = 3.0;
/// Maximo del perfil termico sin normalizar (`x^-3/4 (1 - x^-1/2)^1/4`).
const THIN_DISK_PROFILE_PEAK: f32 = 0.487_872;
const EPSILON: f32 = 1.0e-6;

/// Velocidad angular de una orbita circular en el ecuador, en tiempo de
/// coordenadas: `Omega = sqrt(M / r^3)`. Es Kepler exacto en Schwarzschild.
#[inline]
pub fn keplerian_omega(radius: f32) -> f32 {
    (MASS / (radius * radius * radius)).sqrt()
}

/// Factor de corrimiento `g = nu_observada / nu_emitida` del gas en orbita
/// circular visto por un observador estatico:
///
/// ```text
///   g = sqrt(1 - 3M/r_emit) / [ (1 - Omega L_z/E) sqrt(1 - rs/r_obs) ]
/// ```
///
/// El numerador es la dilatacion temporal del gas (gravitacional y por su
/// velocidad), el `1 - Omega L_z/E` es el Doppler (el lado que se acerca
/// brilla mas) y el ultimo factor es el corrimiento del observador. La
/// temperatura observada es `g T` y el brillo crece como `g^4`.
#[inline]
pub fn redshift_factor(radius_emit: f32, radius_obs: f32, lz_over_e: f32) -> f32 {
    let omega = keplerian_omega(radius_emit);
    let emitter = (1.0 - 3.0 * MASS / radius_emit).max(EPSILON);
    let observer = (1.0 - 1.0 / radius_obs).max(EPSILON);
    let doppler = 1.0 - omega * lz_over_e;
    if doppler.abs() < EPSILON {
        return 0.0;
    }
    emitter.sqrt() / (doppler * observer.sqrt())
}

/// Perfil termico de disco delgado con torque nulo en la ISCO, normalizado
/// para que su maximo valga `peak`:
///
/// ```text
///   T(r) ~ (r/r_in)^(-3/4) [1 - sqrt(r_in/r)]^(1/4)
/// ```
///
/// El maximo no esta en la ISCO sino en `r = (49/36) r_in`: hay un anillo
/// oscuro entre el agujero y la parte mas brillante del disco.
#[inline]
pub fn disk_temperature(radius: f32, peak: f32) -> f32 {
    if radius <= DISK_INNER {
        return 0.0;
    }
    let x = radius / DISK_INNER;
    let profile = x.powf(-0.75) * (1.0 - x.powf(-0.5)).max(0.0).powf(0.25);
    peak * profile / THIN_DISK_PROFILE_PEAK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doppler_matches_local_special_relativity() {
        let radius: f32 = 6.0;
        let observer: f32 = 1000.0;
        let beta = (MASS / (radius - 1.0)).sqrt();
        let gamma = 1.0 / (1.0 - beta * beta).sqrt();
        let b = radius / (1.0 - 1.0 / radius).sqrt();
        let gravity = ((1.0 - 1.0 / radius) / (1.0 - 1.0 / observer)).sqrt();
        let approaching = redshift_factor(radius, observer, b);
        let receding = redshift_factor(radius, observer, -b);
        assert!((approaching - gravity / (gamma * (1.0 - beta))).abs() < 1e-5);
        assert!((receding - gravity / (gamma * (1.0 + beta))).abs() < 1e-5);
        assert!(approaching > 1.0 && receding < 1.0);
    }

    #[test]
    fn the_thermal_profile_has_a_dark_inner_edge() {
        assert_eq!(disk_temperature(DISK_INNER, 6500.0), 0.0);
        let peak = disk_temperature(DISK_INNER * 49.0 / 36.0, 6500.0);
        assert!((peak / 6500.0 - 1.0).abs() < 1e-4);
    }
}
