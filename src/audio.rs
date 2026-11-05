//! Sonido procedural: todo se sintetiza muestra por muestra, sin archivos.
//!
//! raylib solo reproduce el buffer, igual que con la imagen. Aqui se genera:
//! - **El latido de la estrella**: dos golpes graves (lub-dub) en el mismo
//!   instante que el latido que se ve, mas fuertes cerca de ella. Cada golpe
//!   es un seno cuya frecuencia cae de 70 a 38 Hz mientras se apaga, mas un
//!   chasquido corto de ruido al inicio.
//! - **Los motores**: un zumbido (diente de sierra filtrado) y un soplido
//!   (ruido filtrado) que crecen con el empuje.
//! - **El viento**: ruido filtrado que se abre con la velocidad dentro de una
//!   atmosfera.
//! - **El fondo**: un acorde grave que respira despacio y cambia de tono con
//!   el planeta cercano.
//! - **Faro**: un golpe seco cada vez que uno de sus haces pasa por la nave
//!   (asi suenan los pulsares en un radiotelescopio), chisporroteo con sus
//!   estallidos y, en la gran llamarada, el estruendo del pico y la cola que
//!   pulsa con el giro, con las oscilaciones de la corteza que se midieron
//!   en SGR 1806-20 (18, 26, 30, 92.5, 150 y 625 Hz; Israel et al. 2005,
//!   Watts y Strohmayer 2006).
//!
//! El reloj del audio sigue al de la simulacion: si el juego salta (una
//! vista fija, rebobinar), el audio salta con el sin disparar golpes viejos.

use std::f32::consts::TAU;

pub const SAMPLE_RATE: u32 = 44_100;

/// Lo que el juego le dice al sintetizador cada frame.
#[derive(Clone, Copy, Debug)]
pub struct AudioParams {
    /// Fase del latido (en latidos) y latidos por segundo: el sonido sigue
    /// al corazon que se ve aunque acelere.
    pub beat_phase: f64,
    pub beat_rate: f64,
    /// Volumen del latido, de 0 a 1.
    pub beat: f32,
    /// Empuje de los motores, de 0 a 1.
    pub throttle: f32,
    /// Viento, de 0 a 1.
    pub wind: f32,
    /// Tono del fondo en Hz y su volumen.
    pub drone_hz: f32,
    pub drone: f32,
    /// Volumen de la marimba de la version chapina (0 apagada).
    pub marimba: f32,
    /// Faro: fase del giro (vueltas), vueltas por segundo, en que fraccion de
    /// la vuelta pasa el haz norte por la nave y que tan fuerte se oye cada
    /// haz (0 si la nave no esta en su cono).
    pub pulsar_phase: f64,
    pub pulsar_rate: f64,
    pub pulse_at: f32,
    pub pulse_north: f32,
    pub pulse_south: f32,
    /// Estallidos (0 a 1), el pico de la gran llamarada y su cola (0 a 1).
    pub bursts: f32,
    pub spike: f32,
    pub tail: f32,
    pub master: f32,
}

impl Default for AudioParams {
    fn default() -> Self {
        Self {
            beat_phase: 0.0,
            beat_rate: 1.0 / 1.9,
            beat: 0.0,
            throttle: 0.0,
            wind: 0.0,
            drone_hz: 55.0,
            drone: 0.0,
            marimba: 0.0,
            pulsar_phase: 0.0,
            pulsar_rate: 0.5,
            pulse_at: 0.0,
            pulse_north: 0.0,
            pulse_south: 0.0,
            bursts: 0.0,
            spike: 0.0,
            tail: 0.0,
            master: 0.8,
        }
    }
}

/// Un golpe del latido sonando.
#[derive(Clone, Copy, Debug)]
struct Thump {
    age: f32,
    amplitude: f32,
    phase: f32,
}

/// Filtro pasa bajos de un polo.
#[derive(Clone, Copy, Debug, Default)]
struct LowPass {
    state: f32,
}

impl LowPass {
    #[inline]
    fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        let k = 1.0 - (-TAU * cutoff / SAMPLE_RATE as f32).exp();
        self.state += (x - self.state) * k;
        self.state
    }
}

pub struct Synth {
    target: AudioParams,
    /// Volumenes suavizados (sin chasquidos al cambiar).
    throttle: f32,
    wind: f32,
    drone: f32,
    beat: f32,
    drone_hz: f32,
    /// Fase del latido que suena, en latidos.
    phase: f64,
    /// Ultimo golpe disparado (indice de periodo y cual de los dos).
    last_beat: i64,
    thumps: Vec<Thump>,
    noise: u32,
    engine_phase: f32,
    drone_phase: [f32; 3],
    lfo: f32,
    engine_lp: LowPass,
    hiss_lp: LowPass,
    hiss_hp: LowPass,
    wind_lp: LowPass,
    wind_hp: LowPass,
    click_lp: LowPass,
    marimba: crate::marimba::Marimba,
    marimba_level: f32,
    /// Faro: fase del giro que suena, ultimo pulso disparado, los golpes que
    /// suenan, el estruendo del pico, las fases de las oscilaciones y los
    /// filtros del chisporroteo.
    pulsar: f64,
    last_pulse: i64,
    ticks: Vec<Thump>,
    boom: Option<f32>,
    qpo: [f32; 6],
    bursts: f32,
    tail: f32,
    crackle_hp: LowPass,
    crackle_lp: LowPass,
    rumble_lp: LowPass,
}

/// Oscilaciones de la corteza en la cola de la llamarada de SGR 1806-20: Hz y
/// amplitud relativa.
const QPO: [(f32, f32); 6] = [(18.0, 0.7), (26.0, 0.35), (30.0, 0.6), (92.5, 0.5), (150.0, 0.25), (625.0, 0.12)];

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    pub fn new() -> Self {
        Self {
            target: AudioParams::default(),
            throttle: 0.0,
            wind: 0.0,
            drone: 0.0,
            beat: 0.0,
            drone_hz: 55.0,
            phase: 0.0,
            last_beat: i64::MIN,
            thumps: Vec::new(),
            noise: 0x1234_5678,
            engine_phase: 0.0,
            drone_phase: [0.0; 3],
            lfo: 0.0,
            engine_lp: LowPass::default(),
            hiss_lp: LowPass::default(),
            hiss_hp: LowPass::default(),
            wind_lp: LowPass::default(),
            wind_hp: LowPass::default(),
            click_lp: LowPass::default(),
            marimba: crate::marimba::Marimba::new(),
            marimba_level: 0.0,
            pulsar: 0.0,
            last_pulse: i64::MIN,
            ticks: Vec::new(),
            boom: None,
            qpo: [0.0; 6],
            bursts: 0.0,
            tail: 0.0,
            crackle_hp: LowPass::default(),
            crackle_lp: LowPass::default(),
            rumble_lp: LowPass::default(),
        }
    }

    /// Indice del ultimo pulso de Faro en la fase `phase` (dos por vuelta:
    /// el haz norte en `at` y el sur media vuelta despues).
    fn pulse_index(phase: f64, at: f32) -> i64 {
        ((phase - at as f64) * 2.0).floor() as i64
    }

    pub fn set(&mut self, params: AudioParams) {
        self.target = params;
    }

    #[inline]
    fn white(&mut self) -> f32 {
        // xorshift32: ruido blanco barato y determinista.
        let mut x = self.noise;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        (x >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// Indice del ultimo golpe que ya debio sonar en la fase `phase`: dos
    /// por latido, en las fases 0.1 (lub) y 0.3 (dub).
    fn beat_index(phase: f64) -> i64 {
        let cycle = phase.floor();
        let within = phase - cycle;
        let count = if within >= 0.3 {
            2
        } else if within >= 0.1 {
            1
        } else {
            0
        };
        cycle as i64 * 2 + count - 1
    }

    /// Llena `out` con las siguientes muestras (mono, entre -1 y 1).
    pub fn fill(&mut self, out: &mut [f32]) {
        let target = self.target;
        let dt = 1.0 / SAMPLE_RATE as f32;
        // La fase sigue a la del juego; si salto (o recien empieza), se
        // resincroniza sin disparar golpes.
        // Hacia atras (rebobinando) tampoco suenan golpes: el corazon no late
        // al reves.
        if self.last_beat == i64::MIN
            || (target.beat_phase - self.phase).abs() > 0.3
            || target.beat_phase < self.phase - 1e-3
        {
            self.phase = target.beat_phase;
            self.last_beat = Self::beat_index(self.phase);
        } else {
            self.phase += (target.beat_phase - self.phase) * 0.1;
        }
        // La fase de Faro sigue a la del juego igual que la del latido.
        if self.last_pulse == i64::MIN
            || (target.pulsar_phase - self.pulsar).abs() > 0.3
            || target.pulsar_phase < self.pulsar - 1e-3
        {
            self.pulsar = target.pulsar_phase;
            self.last_pulse = Self::pulse_index(self.pulsar, target.pulse_at);
        } else {
            self.pulsar += (target.pulsar_phase - self.pulsar) * 0.1;
        }
        // El pico de la llamarada dispara el estruendo una sola vez.
        if target.spike > 0.5 && self.boom.is_none() {
            self.boom = Some(0.0);
        }
        if target.spike <= 0.0 && self.boom.is_some_and(|age| age > 3.0) {
            self.boom = None;
        }
        let smooth = 1.0 - (-dt / 0.05).exp();
        for sample in out.iter_mut() {
            self.throttle += (target.throttle - self.throttle) * smooth;
            self.wind += (target.wind - self.wind) * smooth;
            self.drone += (target.drone - self.drone) * smooth;
            self.beat += (target.beat - self.beat) * smooth;
            self.drone_hz += (target.drone_hz - self.drone_hz) * smooth * 0.05;
            self.phase += target.beat_rate * dt as f64;

            // Latido.
            let index = Self::beat_index(self.phase);
            if index > self.last_beat {
                let lub = index.rem_euclid(2) == 0;
                self.thumps.push(Thump {
                    age: 0.0,
                    amplitude: if lub { 1.0 } else { 0.6 },
                    phase: 0.0,
                });
                self.last_beat = index;
            }
            let mut heart = 0.0;
            for thump in &mut self.thumps {
                let freq = 38.0 + 32.0 * (-thump.age * 18.0).exp();
                thump.phase = (thump.phase + freq * dt).fract();
                let envelope = (-thump.age / 0.16).exp() * (thump.age / 0.004).min(1.0);
                heart += (thump.phase * TAU).sin() * envelope * thump.amplitude;
                thump.age += dt;
            }
            self.thumps.retain(|t| t.age < 0.9);
            let click_noise = self.white();
            let click = self.click_lp.run(click_noise, 900.0)
                * self.thumps.iter().map(|t| (-t.age / 0.012).exp() * t.amplitude).sum::<f32>();
            let heart = (heart + click * 0.6) * self.beat;

            // Motores: zumbido y soplido.
            let hz = 48.0 + 50.0 * self.throttle;
            self.engine_phase = (self.engine_phase + hz * dt).fract();
            let saw = self.engine_phase * 2.0 - 1.0;
            let hum = self.engine_lp.run(saw, 220.0 + 500.0 * self.throttle);
            let hiss_noise = self.white();
            let hiss_low = self.hiss_lp.run(hiss_noise, 2400.0);
            let hiss = hiss_low - self.hiss_hp.run(hiss_low, 300.0);
            let engine = (hum * 0.5 + hiss * 0.35) * self.throttle;

            // Viento: ruido que se abre con la velocidad.
            let wind_noise = self.white();
            let low = self.wind_lp.run(wind_noise, 200.0 + 1600.0 * self.wind);
            let wind = (low - self.wind_hp.run(low, 80.0)) * self.wind * 0.8;

            // Fondo: acorde que respira.
            self.lfo = (self.lfo + 0.07 * dt).fract();
            let mut drone = 0.0;
            for (k, ratio) in [1.0f32, 1.5, 2.01].iter().enumerate() {
                self.drone_phase[k] = (self.drone_phase[k] + self.drone_hz * ratio * dt).fract();
                let swell = 0.6 + 0.4 * ((self.lfo + k as f32 * 0.33) * TAU).sin();
                drone += (self.drone_phase[k] * TAU).sin() * swell / (k as f32 + 1.0);
            }
            let drone = drone * self.drone * 0.25;

            // Marimba de la version chapina: entra y sale suave.
            self.marimba_level += (target.marimba - self.marimba_level) * smooth * 0.1;
            let marimba = if self.marimba_level > 0.001 {
                self.marimba.next(SAMPLE_RATE as f32) * self.marimba_level
            } else {
                0.0
            };

            // Faro: un golpe por cada haz que pasa por la nave.
            self.pulsar += target.pulsar_rate * dt as f64;
            let index = Self::pulse_index(self.pulsar, target.pulse_at);
            if index > self.last_pulse {
                let amplitude = if index.rem_euclid(2) == 0 { target.pulse_north } else { target.pulse_south };
                if amplitude > 0.01 {
                    self.ticks.push(Thump { age: 0.0, amplitude, phase: 0.0 });
                }
                self.last_pulse = index;
            }
            let mut pulses = 0.0;
            let click = self.white();
            for tick in &mut self.ticks {
                // Un chasquido de banda ancha y un golpe grave corto.
                tick.phase = (tick.phase + (60.0 + 90.0 * (-tick.age * 40.0).exp()) * dt).fract();
                let thud = (tick.phase * TAU).sin() * (-tick.age / 0.05).exp();
                let snap = click * (-tick.age / 0.004).exp();
                pulses += (thud * 0.8 + snap * 0.7) * tick.amplitude;
                tick.age += dt;
            }
            self.ticks.retain(|t| t.age < 0.4);
            // Estallidos: chisporroteo de ruido que sigue a su luz.
            self.bursts += (target.bursts - self.bursts) * smooth * 4.0;
            let noise = self.white();
            let crackle_low = self.crackle_lp.run(noise, 5000.0);
            let crackle = (crackle_low - self.crackle_hp.run(crackle_low, 400.0)) * self.bursts;
            // El estruendo del pico: un barrido grave que tarda en apagarse.
            let mut boom = 0.0;
            if let Some(age) = self.boom {
                let rumble = self.white();
                let low = self.rumble_lp.run(rumble, 120.0);
                boom = (low * 3.0 + (TAU * (28.0 + 40.0 * (-age * 1.5).exp()) * age).sin()) * (-age / 1.2).exp();
                self.boom = Some(age + dt);
            }
            // La cola: las oscilaciones de la corteza, que se oyen mas en
            // ciertas fases del giro, y el pulso de la bola de fuego que gira.
            self.tail += (target.tail - self.tail) * smooth * 0.2;
            let mut tail = 0.0;
            if self.tail > 1e-4 {
                let spin = (self.pulsar.fract() as f32 * TAU).cos();
                let window = (0.5 + 0.5 * spin).powi(2);
                for (k, &(hz, gain)) in QPO.iter().enumerate() {
                    self.qpo[k] = (self.qpo[k] + hz * dt).fract();
                    tail += (self.qpo[k] * TAU).sin() * gain;
                }
                tail *= self.tail.sqrt() * (0.35 + 0.65 * window) * 0.4;
            }
            let faro = pulses * 0.8 + crackle * 0.6 + boom * 0.9 + tail;

            let mix = (heart * 0.9 + engine + wind + drone + marimba + faro) * target.master;
            // Saturacion suave: nunca sale de [-1, 1].
            *sample = mix / (1.0 + mix.abs());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn the_heartbeat_sounds_when_the_star_beats() {
        let mut synth = Synth::new();
        let period = 2.0;
        let rate = SAMPLE_RATE as usize;
        // Arranca justo despues de un periodo completo y avanza en bloques
        // como lo haria el juego.
        let mut out = Vec::new();
        let mut time = 10.0;
        for _ in 0..(2 * rate / 1024) {
            synth.set(AudioParams {
                beat_phase: time / period,
                beat_rate: 1.0 / period,
                beat: 1.0,
                ..AudioParams::default()
            });
            let mut block = vec![0.0f32; 1024];
            synth.fill(&mut block);
            out.extend_from_slice(&block);
            time += 1024.0 / SAMPLE_RATE as f64;
        }
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        // Tiempo 10 es fase 0: el lub suena en la fase 0.1 (0.2 s) y entre
        // 0.8 y 1.8 s no hay golpes.
        let at = |seconds: f32| (seconds * SAMPLE_RATE as f32) as usize;
        let beat = rms(&out[at(0.2)..at(0.35)]);
        let quiet = rms(&out[at(1.2)..at(1.8)]);
        assert!(beat > 10.0 * quiet.max(1e-4), "golpe {beat}, silencio {quiet}");
    }

    #[test]
    fn jumping_in_time_does_not_fire_old_beats() {
        let mut synth = Synth::new();
        let mut block = vec![0.0f32; 512];
        synth.set(AudioParams {
            beat_phase: 0.05,
            beat_rate: 1.0,
            beat: 1.0,
            ..AudioParams::default()
        });
        synth.fill(&mut block);
        // Salto de 100 latidos: no deben sonar cien golpes juntos.
        synth.set(AudioParams {
            beat_phase: 100.05,
            beat_rate: 1.0,
            beat: 1.0,
            ..AudioParams::default()
        });
        synth.fill(&mut block);
        assert!(synth.thumps.len() <= 1);
    }
}
