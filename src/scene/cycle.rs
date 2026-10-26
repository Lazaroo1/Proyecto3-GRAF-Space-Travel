//! El ciclo de OCASO: quince minutos desde la agonia de la estrella hasta
//! Gargantua, y vuelta a empezar.
//!
//! | Minuto | Fase | Que pasa |
//! | --- | --- | --- |
//! | 0 - 10 | Agonia | La supergigante roja late cada 1.9 s; Pira se le deshace encima. |
//! | 10 - 12 | Arritmia | El latido se acelera hasta 0.7 s, la estrella se hincha, se enfria y late mas fuerte. |
//! | 12:00 - 12:06 | Colapso | La supernova fallida: la estrella cae sobre si misma y se calienta. |
//! | 12:06 | Destello | Un estallido de luz; nace el agujero. |
//! | 12:06 - 13:06 | Ignicion | El disco de acrecion se forma de adentro hacia afuera. |
//! | 13 - 15 | Gargantua | El disco ilumina el sistema con luz fria; Jardin brilla en la oscuridad. |
//! | 14:48 - 15:00 | Renacer | Todo se funde en blanco y el ciclo empieza otra vez. |
//!
//! Todo es una **funcion pura del tiempo**: no hay nada que se acumule
//! frame a frame. Por eso saltar en el ciclo, o rebobinarlo, es solo
//! evaluarlo con otro `t`. El latido acelera, asi que su fase no puede ser
//! `t / periodo` (saltaria al cambiar el periodo): es la integral cerrada de
//! la frecuencia.

use crate::math::smoothstep;

/// Duracion del ciclo en segundos.
pub const CYCLE: f64 = 900.0;
const ARRHYTHMIA: f64 = 600.0;
const COLLAPSE: f64 = 720.0;
const COLLAPSE_LENGTH: f64 = 6.0;
const IGNITION_LENGTH: f64 = 60.0;
const REBIRTH: f64 = 12.0;
/// Periodo del latido al principio y al final de la arritmia.
const BEAT_SLOW: f64 = 1.9;
const BEAT_FAST: f64 = 0.7;
/// Radio de Schwarzschild de Gargantua en unidades del mundo: la sombra mide
/// 2.6 veces esto y el disco llega a 19 veces, todo dentro de la orbita de
/// Pira. La masa no cambia (las orbitas tampoco): `rs` es solo cuanto se
/// curva la luz, como si la luz fuera mas lenta en este sistema.
pub const GARGANTUA_RS: f64 = 40.0;
/// Radio exterior del disco formado, en `rs`.
pub const DISK_OUTER: f32 = 19.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Agony,
    Arrhythmia,
    Collapse,
    Gargantua,
    Rebirth,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Agony => "agonia",
            Phase::Arrhythmia => "arritmia",
            Phase::Collapse => "colapso",
            Phase::Gargantua => "Gargantua",
            Phase::Rebirth => "renacer",
        }
    }
}

/// Todo lo que el ciclo decide en un instante.
#[derive(Clone, Copy, Debug)]
pub struct CycleState {
    /// Segundos desde el inicio del ciclo.
    pub tau: f64,
    pub phase: Phase,
    /// Fase del latido (ciclos completos, continua).
    pub beat_phase: f64,
    /// Amplitud del latido en el radio (fraccion).
    pub pulse: f64,
    /// Radio de la estrella sin el latido; 0 cuando ya no existe.
    pub star_radius: f64,
    pub star_temperature: f32,
    /// Brillo de la superficie de la estrella.
    pub star_intensity: f32,
    /// Cuanto de su luz llega a los planetas (1 normal).
    pub star_light: f32,
    /// Corona alrededor de la estrella en el cielo.
    pub star_glow: f32,
    /// Radio de Schwarzschild en unidades del mundo (0 antes del colapso).
    pub rs: f64,
    /// Disco: densidad (0 a 1), radio exterior en `rs` y temperatura maxima.
    pub disk_density: f32,
    pub disk_outer: f32,
    pub disk_peak: f32,
    /// Bioluminiscencia de Jardin (1 normal).
    pub biolum: f32,
    /// Exposicion guionada: baja durante el destello.
    pub exposure: f32,
}

/// Fase del latido: integral de la frecuencia, que sube suave (smoothstep)
/// de `1/BEAT_SLOW` a `1/BEAT_FAST` durante la arritmia. La integral del
/// smoothstep `3u^2 - 2u^3` es `u^3 - u^4/2`.
fn beat_phase(tau: f64) -> f64 {
    let (f0, f1) = (1.0 / BEAT_SLOW, 1.0 / BEAT_FAST);
    let span = COLLAPSE - ARRHYTHMIA;
    if tau <= ARRHYTHMIA {
        f0 * tau
    } else if tau <= COLLAPSE {
        let u = (tau - ARRHYTHMIA) / span;
        f0 * tau + (f1 - f0) * span * (u * u * u - 0.5 * u * u * u * u)
    } else {
        f0 * COLLAPSE + (f1 - f0) * span * 0.5 + f1 * (tau - COLLAPSE)
    }
}

/// El estado del ciclo en el instante `t` para una estrella de radio `radius`.
pub fn at(t: f64, radius: f64) -> CycleState {
    let tau = t.rem_euclid(CYCLE);
    let s = |a: f64, b: f64, x: f64| smoothstep(a as f32, b as f32, x as f32) as f64;
    let arrhythmia = s(ARRHYTHMIA, COLLAPSE, tau);
    let collapse_end = COLLAPSE + COLLAPSE_LENGTH;
    let u = ((tau - COLLAPSE) / COLLAPSE_LENGTH).clamp(0.0, 1.0);

    // La estrella se hincha y se enfria en la arritmia; en el colapso cae
    // como en caida libre, R ~ (1 - u^2)^(2/3), y se calienta.
    let swollen = radius * (1.0 + 0.08 * arrhythmia);
    let star_radius = if tau < COLLAPSE {
        swollen
    } else {
        swollen * (1.0 - u * u).max(0.0).powf(2.0 / 3.0)
    };
    let cool = 3400.0 - 400.0 * arrhythmia as f32;
    let star_temperature = if tau < COLLAPSE {
        cool
    } else {
        (cool * (swollen / star_radius.max(1.0)).sqrt() as f32).min(25_000.0)
    };

    // Destellos: el del colapso al final de la caida y el del renacer.
    let burst = (-((tau - collapse_end) / 1.5).powi(2)).exp();
    let fade_out = s(CYCLE - REBIRTH, CYCLE, tau);
    let fade_in = if t >= CYCLE { s(4.0, 0.0, tau) } else { 0.0 };
    let flash = burst.max(fade_out).max(fade_in) as f32;

    let alive = if tau < collapse_end { 1.0 } else { 0.0 };
    let born = s(collapse_end, collapse_end + 40.0, tau) as f32;
    let grown = s(collapse_end, collapse_end + IGNITION_LENGTH, tau) as f32;
    let phase = if tau >= CYCLE - REBIRTH {
        Phase::Rebirth
    } else if tau >= collapse_end {
        Phase::Gargantua
    } else if tau >= COLLAPSE {
        Phase::Collapse
    } else if tau >= ARRHYTHMIA {
        Phase::Arrhythmia
    } else {
        Phase::Agony
    };
    CycleState {
        tau,
        phase,
        beat_phase: beat_phase(tau),
        pulse: if tau < COLLAPSE { 0.012 + 0.038 * arrhythmia } else { 0.0 },
        star_radius: star_radius * alive,
        star_temperature,
        star_intensity: (1.0 - 0.3 * arrhythmia as f32) * (1.0 + 6.0 * (u * u) as f32),
        star_light: alive as f32 * (1.0 - 0.25 * arrhythmia as f32) + 25.0 * burst as f32,
        star_glow: alive as f32 * (1.0 - u as f32),
        rs: GARGANTUA_RS * s(0.6, 1.0, u),
        disk_density: born,
        disk_outer: 3.0 + (DISK_OUTER - 3.0) * grown,
        disk_peak: 6500.0 + 2500.0 * (-((tau - collapse_end).max(0.0)) / 15.0).exp() as f32,
        biolum: 1.0 + 2.0 * s(collapse_end, collapse_end + 20.0, tau) as f32,
        exposure: 1.0 / (1.0 + 4.0 * flash),
    }
}

/// Instante (dentro del primer ciclo) en que empieza cada fase, para saltar.
pub fn phase_start(phase: Phase) -> f64 {
    match phase {
        Phase::Agony => 0.0,
        Phase::Arrhythmia => ARRHYTHMIA,
        Phase::Collapse => COLLAPSE,
        Phase::Gargantua => COLLAPSE + COLLAPSE_LENGTH,
        Phase::Rebirth => CYCLE - REBIRTH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cycle_is_a_pure_function_of_time() {
        for t in [0.0, 123.4, 650.0, 723.0, 800.0, 1234.5] {
            let (a, b) = (at(t, 320.0), at(t, 320.0));
            assert_eq!(a.beat_phase, b.beat_phase);
            assert_eq!(a.star_radius, b.star_radius);
            assert_eq!(a.rs, b.rs);
        }
    }

    #[test]
    fn the_beat_phase_is_continuous_and_only_moves_forward() {
        let mut last = beat_phase(0.0);
        let mut t = 0.0;
        while t < CYCLE {
            t += 0.05;
            let now = beat_phase(t);
            assert!(now > last, "t {t}");
            // Nunca avanza mas rapido que el latido rapido.
            assert!(now - last <= 0.05 / BEAT_FAST + 1e-9, "t {t}");
            last = now;
        }
    }

    #[test]
    fn the_star_shrinks_into_a_black_hole_and_comes_back_next_cycle() {
        let before = at(COLLAPSE - 1.0, 320.0);
        let during = at(COLLAPSE + 3.0, 320.0);
        let after = at(COLLAPSE + 100.0, 320.0);
        assert!(before.star_radius > 320.0 && before.rs == 0.0);
        assert!(during.star_radius < before.star_radius && during.star_temperature > before.star_temperature);
        assert_eq!(after.star_radius, 0.0);
        assert_eq!(after.rs, GARGANTUA_RS);
        assert!(after.disk_density > 0.99 && after.biolum > 2.9);
        let reborn = at(CYCLE + 30.0, 320.0);
        assert_eq!(reborn.phase, Phase::Agony);
        assert_eq!(reborn.rs, 0.0);
    }

    #[test]
    fn the_radius_is_continuous_until_the_collapse_ends() {
        // La caida libre termina con derivada infinita: se revisa hasta el
        // 98% del colapso.
        let mut last = at(0.0, 320.0).star_radius;
        let mut t = 0.0;
        while t < COLLAPSE + COLLAPSE_LENGTH * 0.98 {
            t += 0.01;
            let now = at(t, 320.0).star_radius;
            assert!((now - last).abs() < 5.0, "t {t}: {last} -> {now}");
            last = now;
        }
    }
}
