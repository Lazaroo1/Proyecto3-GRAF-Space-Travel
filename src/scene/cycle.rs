//! El ciclo de OCASO, a pedido del piloto: la estrella agoniza (late) todo
//! el tiempo que se quiera; con J empieza el final y en medio minuto nace
//! Gargantua. Otra J hace renacer a la estrella.
//!
//! | Desde J | Fase | Que pasa |
//! | --- | --- | --- |
//! | antes | Agonia | La supergigante roja late cada 1.9 s; Pira se le deshace encima. |
//! | 0 - 20 s | Estertor | El latido se acelera hasta 0.7 s; la estrella se hincha, suelta una capa de gas que arde y lanza llamaradas. |
//! | 20 - 26 s | Silencio | Cae sobre si misma; su luz y la de la nebulosa se drenan. |
//! | 26 - 30 s | Silencio | Oscuridad: los ojos se acostumbran y solo queda el cielo y la bioluminiscencia. |
//! | 30 s | El ojo se abre | Una onda gravitacional barre el sistema y nace Gargantua, con un disco blanco azulado. |
//! | 36 - 52 s | La comida | La marea de Gargantua hace hervir a Pira: se funde y se encoge mientras su gas sale por el lado del agujero y entra al disco en espiral. |
//! | 52 s - | Despues | El brazo de Pira se diluye en el disco, que se enfria a naranja fuego. Cruzar el horizonte mata y rebobina 15 s. |
//! | J otra vez | Renacer | Fundido a blanco de 3 s y vuelve la agonia. |
//!
//! Todo es una **funcion pura** del tiempo y de los instantes en que se
//! presiono J ([`Timeline`]): no hay nada que se acumule frame a frame. Por
//! eso rebobinar es solo evaluarlo con otro `t`. El latido acelera, asi que
//! su fase no puede ser `t / periodo` (saltaria al cambiar el periodo): es
//! la integral cerrada de la frecuencia.

use crate::math::smoothstep;

/// Estertor: del primer latido acelerado al inicio del colapso.
const ESTERTOR: f64 = 20.0;
const COLLAPSE_LENGTH: f64 = 6.0;
/// Oscuridad entre el colapso y la apertura del ojo.
const SILENCE: f64 = 4.0;
/// Segundos desde J hasta que se abre el ojo.
const EYE: f64 = ESTERTOR + COLLAPSE_LENGTH + SILENCE;
/// Lo que tarda el disco en formarse.
const IGNITION_LENGTH: f64 = 25.0;
/// Fundido a blanco del renacer y de vuelta.
pub const REBIRTH: f64 = 3.0;
const DAWN: f64 = 2.5;
/// La estrella suelta su capa de gas a esta altura del estertor.
const SHELL_RELEASE: f64 = 8.0;
/// Periodo del latido en la agonia y al final del estertor.
const BEAT_SLOW: f64 = 1.9;
const BEAT_FAST: f64 = 0.7;
/// Radio de Schwarzschild de Gargantua en unidades del mundo: la sombra mide
/// 2.6 veces esto y el disco llega a 19 veces, todo dentro de la orbita de
/// Pira. La masa no cambia (las orbitas tampoco): `rs` es solo cuanto se
/// curva la luz, como si la luz fuera mas lenta en este sistema.
pub const GARGANTUA_RS: f64 = 40.0;
/// Radio exterior del disco formado, en `rs`.
pub const DISK_OUTER: f32 = 19.0;
/// Pasado el horizonte (con un margen: la esfera de fotones) la nave muere
/// y el tiempo se rebobina.
pub const HORIZON: f64 = 1.6;
/// Segundos despues de abrirse el ojo en que Gargantua empieza a comerse a
/// Pira, y lo que tarda en deshacerla.
const SWALLOW_START: f64 = 6.0;
const SWALLOW_LENGTH: f64 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Agony,
    Estertor,
    Silence,
    EyeOpens,
    After,
    Rebirth,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Agony => "Agonía",
            Phase::Estertor => "Estertor",
            Phase::Silence => "Silencio",
            Phase::EyeOpens => "El ojo se abre",
            Phase::After => "Después",
            Phase::Rebirth => "Renacer",
        }
    }
}

/// Instantes (en tiempo de simulacion) en que el piloto desato el final y
/// el renacer. Con el tiempo, definen todo el ciclo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timeline {
    /// Cuando se presiono J para empezar el final.
    pub ending: Option<f64>,
    /// Cuando se presiono J otra vez para que la estrella renazca.
    pub rebirth: Option<f64>,
    /// Cuando volvio la estrella: desde ahi se desvanece el blanco.
    pub dawn: Option<f64>,
}

impl Timeline {
    /// La agonia, o el final empezado hace `seconds` en el instante `t`.
    pub fn at(t: f64, seconds: Option<f64>) -> Self {
        Self {
            ending: seconds.map(|s| t - s),
            ..Self::default()
        }
    }

    /// J: empieza el final; si ya empezo, el renacer.
    pub fn press(&mut self, t: f64) {
        match self.since(t) {
            Some(_) => {
                self.rebirth.get_or_insert(t);
            }
            None => {
                *self = Self {
                    ending: Some(t),
                    ..Self::default()
                };
            }
        }
    }

    /// Termina el renacer cuando el fundido llega a blanco. Rebobinar antes
    /// de la J del renacer lo deshace.
    pub fn advance(&mut self, t: f64) {
        match self.rebirth {
            Some(r) if t < r => self.rebirth = None,
            Some(r) if t >= r + REBIRTH => {
                *self = Self {
                    dawn: Some(t),
                    ..Self::default()
                }
            }
            _ => {}
        }
    }

    /// Segundos desde la J del final, si ya empezo.
    pub fn since(&self, t: f64) -> Option<f64> {
        self.ending.filter(|&start| t >= start).map(|start| t - start)
    }
}

/// Todo lo que el ciclo decide en un instante.
#[derive(Clone, Copy, Debug)]
pub struct CycleState {
    /// Segundos desde la J del final (0 en la agonia).
    pub tau: f64,
    pub phase: Phase,
    /// Fase del latido (ciclos completos, continua).
    pub beat_phase: f64,
    /// Latidos por segundo.
    pub beat_rate: f64,
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
    /// Llamaradas: 0 en la agonia, 1 al final del estertor.
    pub flares: f32,
    /// Segundos desde que la estrella solto su capa de gas (negativo antes).
    pub shell_age: f32,
    /// Cuanta luz le da la estrella a esa capa: se apaga con ella.
    pub shell_light: f32,
    /// Cuanto se ha comido Gargantua a Pira (0 a 1; a 1 ya no queda).
    pub swallow: f32,
    /// Segundos desde que Pira empezo a deshacerse (negativo antes): su gas
    /// entra al disco y se va mezclando.
    pub stream_age: f32,
    /// Radio de Schwarzschild en unidades del mundo (0 antes del colapso).
    pub rs: f64,
    /// Segundos desde que se abrio el ojo (negativo antes).
    pub since_eye: f64,
    /// Disco: densidad (0 a 1), radio exterior en `rs` y temperatura maxima.
    pub disk_density: f32,
    pub disk_outer: f32,
    pub disk_peak: f32,
    /// Bioluminiscencia de Jardin (1 normal).
    pub biolum: f32,
    /// Exposicion guionada: baja durante el destello del ojo.
    pub exposure: f32,
    /// Fundido a blanco del renacer (0 a 1).
    pub whiteout: f32,
    /// Radio con el que la nave choca: la estrella o, despues del colapso,
    /// la zona de exclusion alrededor del agujero.
    pub collision_radius: f64,
}

/// Fase del latido: integral de la frecuencia, que sube suave (smoothstep)
/// de `1/BEAT_SLOW` a `1/BEAT_FAST` durante el estertor. La integral del
/// smoothstep `3u^2 - 2u^3` es `u^3 - u^4/2`. Devuelve la fase y su derivada.
fn beat(t: f64, since: Option<f64>) -> (f64, f64) {
    let (f0, f1) = (1.0 / BEAT_SLOW, 1.0 / BEAT_FAST);
    let Some(s) = since else {
        return (f0 * t, f0);
    };
    let before = f0 * (t - s);
    if s <= ESTERTOR {
        let u = s / ESTERTOR;
        let phase = before + f0 * s + (f1 - f0) * ESTERTOR * (u * u * u - 0.5 * u * u * u * u);
        (phase, f0 + (f1 - f0) * u * u * (3.0 - 2.0 * u))
    } else {
        let phase = before + f0 * ESTERTOR + (f1 - f0) * ESTERTOR * 0.5 + f1 * (s - ESTERTOR);
        (phase, f1)
    }
}

/// El estado del ciclo en el instante `t` para una estrella de radio `radius`.
pub fn at(t: f64, timeline: &Timeline, radius: f64) -> CycleState {
    let since = timeline.since(t);
    // En la agonia, un tiempo "antes de J" que deja todo en cero.
    let s_ = since.unwrap_or(-1.0);
    let s = |a: f64, b: f64, x: f64| smoothstep(a as f32, b as f32, x as f32) as f64;
    let estertor = s(0.0, ESTERTOR, s_);
    let collapse_end = ESTERTOR + COLLAPSE_LENGTH;
    let u = ((s_ - ESTERTOR) / COLLAPSE_LENGTH).clamp(0.0, 1.0);

    // La estrella se hincha y se enfria en el estertor; en el colapso cae
    // como en caida libre, R ~ (1 - u^2)^(2/3), y se calienta.
    let swollen = radius * (1.0 + 0.08 * estertor);
    let star_radius = if s_ < ESTERTOR {
        swollen
    } else {
        swollen * (1.0 - u * u).max(0.0).powf(2.0 / 3.0)
    };
    let cool = 3400.0 - 400.0 * estertor as f32;
    let star_temperature = if s_ < ESTERTOR {
        cool
    } else {
        (cool * (swollen / star_radius.max(1.0)).sqrt() as f32).min(25_000.0)
    };

    let alive = if s_ < collapse_end { 1.0 } else { 0.0 };
    // La luz se drena mientras cae; despues, oscuridad hasta el ojo.
    let drain = 1.0 - s(ESTERTOR, collapse_end, s_);
    // Destello cuando el ojo se abre.
    let since_eye = if since.is_some() { s_ - EYE } else { f64::NEG_INFINITY };
    // Sube de golpe y se apaga en un par de segundos.
    let width = if since_eye < 0.0 { 0.25 } else { 1.2 };
    let burst = (-(since_eye / width).powi(2)).exp();
    let open = s(-0.3, 1.2, since_eye);
    let born = s(0.0, 15.0, since_eye) as f32;
    let grown = s(0.0, IGNITION_LENGTH, since_eye) as f32;

    let rebirth = timeline.rebirth.filter(|&r| t >= r);
    let fade_out = rebirth.map_or(0.0, |r| s(r, r + REBIRTH, t));
    let fade_in = timeline.dawn.map_or(0.0, |d| s(d + DAWN, d, t));
    let phase = if rebirth.is_some() {
        Phase::Rebirth
    } else if since.is_none() {
        Phase::Agony
    } else if s_ < ESTERTOR {
        Phase::Estertor
    } else if since_eye < 0.0 {
        Phase::Silence
    } else if since_eye < IGNITION_LENGTH {
        Phase::EyeOpens
    } else {
        Phase::After
    };
    let (beat_phase, beat_rate) = beat(t, since);
    let rs = GARGANTUA_RS * open;
    CycleState {
        tau: since.unwrap_or(0.0),
        phase,
        beat_phase,
        beat_rate,
        pulse: if s_ < ESTERTOR {
            0.012 + 0.038 * estertor
        } else {
            0.0
        },
        star_radius: star_radius * alive,
        star_temperature,
        star_intensity: (1.0 - 0.3 * estertor as f32) * (1.0 + 6.0 * (u * u) as f32),
        star_light: (alive * (1.0 - 0.25 * estertor) * drain + 6.0 * burst) as f32,
        star_glow: alive as f32 * (1.0 - u as f32),
        flares: (estertor * alive * (1.0 - u)) as f32,
        shell_age: (s_ - SHELL_RELEASE) as f32,
        shell_light: (alive * drain) as f32,
        swallow: s(SWALLOW_START, SWALLOW_START + SWALLOW_LENGTH, since_eye) as f32,
        stream_age: (since_eye - SWALLOW_START) as f32,
        rs,
        since_eye,
        disk_density: born,
        disk_outer: 3.0 + (DISK_OUTER - 3.0) * grown,
        // Nace blanco azulado de caliente y se enfria a naranja fuego.
        disk_peak: 4300.0 + 11_500.0 * (-(since_eye.max(0.0)) / 20.0).exp() as f32,
        biolum: 1.0 + 2.0 * s(ESTERTOR, collapse_end + 2.0, s_) as f32,
        // En el silencio los ojos se acostumbran a la oscuridad: aparece el
        // cielo y la bioluminiscencia.
        exposure: (1.0 + 2.5 * (s(ESTERTOR + 1.0, collapse_end + 1.0, s_) * (1.0 - open)) as f32)
            / (1.0 + 2.0 * burst as f32),
        whiteout: fade_out.max(fade_in) as f32,
        collision_radius: star_radius * alive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cycle_is_a_pure_function_of_time() {
        let line = Timeline::at(100.0, Some(12.0));
        for t in [0.0, 50.0, 88.0, 95.0, 110.0, 200.0, 1234.5] {
            let (a, b) = (at(t, &line, 320.0), at(t, &line, 320.0));
            assert_eq!(a.beat_phase, b.beat_phase);
            assert_eq!(a.star_radius, b.star_radius);
            assert_eq!(a.rs, b.rs);
        }
    }

    #[test]
    fn the_star_agonizes_until_j_is_pressed() {
        let line = Timeline::default();
        for t in [0.0, 600.0, 900.0, 5000.0] {
            let state = at(t, &line, 320.0);
            assert_eq!(state.phase, Phase::Agony);
            assert_eq!(state.star_radius, 320.0);
            assert_eq!(state.rs, 0.0);
        }
    }

    #[test]
    fn the_beat_phase_is_continuous_and_only_moves_forward() {
        let line = Timeline::at(50.0, Some(0.0));
        let mut last = at(0.0, &line, 1.0).beat_phase;
        let mut t = 0.0;
        while t < 150.0 {
            t += 0.05;
            let state = at(t, &line, 1.0);
            assert!(state.beat_phase > last, "t {t}");
            // Nunca avanza mas rapido que el latido rapido.
            assert!(state.beat_phase - last <= 0.05 / BEAT_FAST + 1e-9, "t {t}");
            // La derivada que se reporta es la de la fase.
            let later = at(t + 1e-4, &line, 1.0).beat_phase;
            assert!(((later - state.beat_phase) / 1e-4 - state.beat_rate).abs() < 1e-3);
            last = state.beat_phase;
        }
    }

    #[test]
    fn j_turns_the_star_into_a_black_hole_and_j_again_brings_it_back() {
        let mut line = Timeline::default();
        line.press(100.0);
        let before = at(100.0 + ESTERTOR - 1.0, &line, 320.0);
        let during = at(100.0 + ESTERTOR + 3.0, &line, 320.0);
        let silence = at(100.0 + EYE - 1.0, &line, 320.0);
        let after = at(100.0 + EYE + 60.0, &line, 320.0);
        assert!(before.star_radius > 320.0 && before.rs == 0.0);
        assert!(
            during.star_radius < before.star_radius
                && during.star_temperature > before.star_temperature
        );
        assert_eq!(silence.phase, Phase::Silence);
        assert!(silence.star_light < 0.01 && silence.biolum > 2.9);
        assert_eq!(after.phase, Phase::After);
        assert_eq!(after.star_radius, 0.0);
        assert_eq!(after.rs, GARGANTUA_RS);
        assert!(after.disk_density > 0.99);
        // Despues se queda asi todo el tiempo que se quiera.
        assert_eq!(at(100.0 + 5000.0, &line, 320.0).phase, Phase::After);
        // Rebobinar antes de J vuelve a la agonia.
        assert_eq!(at(99.0, &line, 320.0).phase, Phase::Agony);

        line.press(200.0);
        assert_eq!(at(201.0, &line, 320.0).phase, Phase::Rebirth);
        line.advance(200.0 + REBIRTH + 0.1);
        let reborn = at(200.0 + REBIRTH + 0.1, &line, 320.0);
        assert_eq!(reborn.phase, Phase::Agony);
        assert_eq!(reborn.rs, 0.0);
        assert!(reborn.whiteout > 0.9);
        assert_eq!(at(210.0, &line, 320.0).whiteout, 0.0);
    }

    #[test]
    fn the_radius_is_continuous_until_the_collapse_ends() {
        // La caida libre termina con derivada infinita: se revisa hasta el
        // 98% del colapso.
        let line = Timeline::at(10.0, Some(0.0));
        let mut last = at(0.0, &line, 320.0).star_radius;
        let mut t = 0.0;
        while t < 10.0 + ESTERTOR + COLLAPSE_LENGTH * 0.98 {
            t += 0.01;
            let now = at(t, &line, 320.0).star_radius;
            assert!((now - last).abs() < 5.0, "t {t}: {last} -> {now}");
            last = now;
        }
    }
}
