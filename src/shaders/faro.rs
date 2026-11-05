//! Faro: una estrella de neutrones que es pulsar y magnetar a la vez.
//!
//! La fisica de cada parte esta en `scene/faro.rs`; aqui se dibuja lo que
//! ese modelo dice, cada frame, desde la camara.
//!
//! # La superficie, vista a traves de su propia gravedad
//! La estrella se dibuja sobre una esfera de apoyo del tamano del disco que
//! se ve (que es mas grande que la estrella). En cada pixel la tabla de
//! geodesicas (`math::neutron`) dice en que punto de la superficie nacio el
//! rayo: `psi` desde la direccion de la camara, en el plano del rayo. Asi se
//! ven los dos polos magneticos a la vez y el borde muestra el lado de atras.
//!
//! El color es el de un cuerpo negro a millones de kelvin. En el visible eso
//! es la cola de Rayleigh-Jeans (`h nu << kT`): el color ya no cambia con la
//! temperatura (el limite azul del lugar planckiano) y el brillo es
//! proporcional a `T`, no a `T^4`. La gravedad corre la luz al rojo al salir:
//! la temperatura que llega es `g T` con `g = sqrt(1 - rs/R) / sqrt(1 - rs/d)`
//! (`d` es la distancia de la camara, que es un observador estatico).
//!
//! # Los haces
//! El pulsar emite en radio desde las lineas de campo abiertas, a cientos de
//! km sobre los polos magneticos, en un cono hueco con nucleo (Rankin). La
//! radio no se ve: lo que se dibuja es el viento de la supergigante,
//! iluminado por los haces. Su densidad baja como `1/d^2` desde la
//! supergigante y el flujo del haz como `1/r^2` desde Faro: la luz que se
//! dispersa hacia la camara es la integral de `perfil(theta) / r^2 * n` a lo
//! largo del rayo, que se hace exacta en `1/r^2` cambiando de variable.
//! El cono hueco esta hecho de chispas que dan vueltas alrededor del eje
//! magnetico (el carrusel de Ruderman y Sutherland): las subpulsaciones que
//! derivan. Cada pulso es distinto, como en los magnetares.

use super::{Fragment, Uniforms};
use crate::math::neutron::{NeutronLens, Sight};
use crate::math::{Mat3, Perlin, Vec3, hash_u32, smoothstep};

/// Color de un cuerpo negro cuando `T -> infinito` (Rayleigh-Jeans): el
/// limite azul del lugar planckiano, `(x, y) = (0.2399, 0.2340)` en CIE 1931,
/// en sRGB lineal con luminancia 1.
pub const RAYLEIGH_JEANS: Vec3 = Vec3::new(0.664, 0.976, 2.229);

/// Una mancha caliente en la superficie: su centro (en el mundo), su radio
/// angular y su temperatura.
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub center: Vec3,
    pub radius: f32,
    pub temperature: f32,
}

/// El cono de un haz (Rankin): radios de los conos interior y exterior y
/// ancho del nucleo, en radianes, y la altura desde la que emite (km).
#[derive(Clone, Copy, Debug)]
pub struct BeamShape {
    pub core: f32,
    pub inner: f32,
    pub outer: f32,
    pub height: f32,
    /// Chispas del carrusel y su fase de giro alrededor del eje (vueltas).
    pub sparks: u32,
    pub carousel: f32,
}

/// Faro en este frame, desde la camara.
pub struct Faro<'a> {
    /// Centro relativo a la camara.
    pub center: Vec3,
    pub lens: &'a NeutronLens,
    /// Eje magnetico norte en el mundo: al final y al principio del tiempo
    /// de exposicion del frame (el haz barre varios grados en un frame).
    pub magnetic: Vec3,
    pub magnetic_before: Vec3,
    /// Del mundo al marco de la corteza (gira con la estrella).
    pub to_crust: Mat3,
    /// Corrimiento gravitacional de la luz que llega a la camara.
    pub redshift: f32,
    /// Temperatura del polo magnetico (Greenstein-Hartke) y conductividad
    /// transversal sobre la paralela.
    pub pole_temperature: f32,
    pub conductivity_ratio: f32,
    /// Casquetes polares (lineas abiertas) y manchas del haz de corriente.
    pub polar_cap: f32,
    pub cap_temperature: f32,
    pub spots: [Spot; 2],
    /// Brillo extra de toda la superficie (estallidos, la gran llamarada).
    pub flash: f32,
    /// Brillo de la superficie por kelvin (HDR).
    pub gain: f32,
    pub beam: BeamShape,
    /// Fase del giro, en vueltas: decide que pulso es (cada uno distinto).
    pub turn: f32,
    pub beam_gain: f32,
    /// Supergigante relativa a la camara y distancia de Faro a ella: el
    /// viento que dispersa los haces.
    pub wind_source: Vec3,
    pub wind_reference: f32,
}

impl Faro<'_> {
    /// Temperatura de la superficie con normal `n` (en el mundo).
    fn temperature(&self, n: Vec3, noise: &Perlin) -> f32 {
        // Greenstein y Hartke (1983): el calor sube por la corteza a lo largo
        // del campo, asi que la superficie es mas fria donde B es horizontal.
        // Para un dipolo, cos(Theta_B) = 2 cos(theta) / sqrt(1 + 3 cos^2).
        let c = n.dot(self.magnetic);
        let cos_b2 = 4.0 * c * c / (1.0 + 3.0 * c * c);
        let t4 = cos_b2 + self.conductivity_ratio * (1.0 - cos_b2);
        let mut t = self.pole_temperature * t4.sqrt().sqrt();
        // Variaciones de pequena escala del campo de la corteza.
        let crust = self.to_crust.mul_vec(n);
        t *= 1.0 + 0.14 * noise.fbm(crust * 6.0, 4);
        // Casquetes polares: las lineas abiertas, calentadas por las
        // particulas que vuelven.
        let cap = smoothstep(self.polar_cap, self.polar_cap * 0.5, (1.0 - c.abs()).max(0.0).sqrt() * 1.414);
        t = t.max(self.cap_temperature * cap);
        for spot in &self.spots {
            let angle = (1.0 - n.dot(spot.center)).max(0.0).sqrt() * 1.414;
            let w = (-(angle / spot.radius).powi(2)).exp();
            t = t.max(spot.temperature * w + t * (1.0 - w));
        }
        t * (1.0 + self.flash)
    }

    /// Luz de la superficie que la camara ve en la direccion `dir` (unitaria),
    /// o del cielo, si el rayo no toca la estrella.
    pub fn look(&self, dir: Vec3, u: &Uniforms) -> Vec3 {
        let distance = self.center.length();
        let to_star = self.center / distance;
        let cos_s = dir.dot(to_star).clamp(-1.0, 1.0);
        let sin_s = dir.cross(to_star).length();
        let e1 = -to_star;
        let e2 = if sin_s > 1e-6 {
            (dir - to_star * cos_s) / sin_s
        } else {
            to_star.cross(if to_star.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize()
        };
        match self.lens.sight(sin_s.atan2(cos_s)) {
            Sight::Surface { psi, .. } => {
                let normal = e1 * psi.cos() + e2 * psi.sin();
                RAYLEIGH_JEANS * (self.temperature(normal, u.noise) * self.redshift * self.gain)
            }
            Sight::Sky { exit } => super::sky(e1 * exit.cos() + e2 * exit.sin(), u),
        }
    }

    /// La direccion del cielo que se ve en `dir`: la gravedad de Faro curva
    /// la luz de las estrellas de fondo (un anillo de Einstein de varios
    /// grados cuando se pasa cerca).
    pub fn bend(&self, dir: Vec3) -> Vec3 {
        let distance = self.center.length();
        let to_star = self.center / distance;
        let cos_s = dir.dot(to_star).clamp(-1.0, 1.0);
        let sin_s = dir.cross(to_star).length();
        if sin_s < 1e-6 {
            return dir;
        }
        let e2 = (dir - to_star * cos_s) / sin_s;
        match self.lens.sight(sin_s.atan2(cos_s)) {
            Sight::Sky { exit } => -to_star * exit.cos() + e2 * exit.sin(),
            // Detras de la estrella: lo tapa la superficie que se dibuja.
            Sight::Surface { .. } => dir,
        }
    }

    /// Intensidad del haz a un angulo `theta` de su eje y un azimut `phi`
    /// alrededor de el: el nucleo y los dos conos de chispas.
    fn profile(&self, theta: f32, phi: f32, pulse: f32) -> f32 {
        let b = &self.beam;
        let gauss = |x: f32, width: f32| (-(x / width) * (x / width)).exp();
        // Las chispas del carrusel: el anillo es mas brillante donde hay una.
        let sparks = b.sparks as f32;
        let k = (phi / std::f32::consts::TAU + b.carousel) * sparks;
        let spark = 0.35 + 0.65 * (1.0 - (k - k.round()).abs() * 2.0).powi(2);
        let core = gauss(theta, b.core * 0.6) * (0.6 + 0.8 * pulse);
        let inner = gauss(theta - b.inner * 0.75, b.inner * 0.22) * spark;
        let outer = gauss(theta - b.outer * 0.8, b.outer * 0.2) * spark * (1.4 - pulse);
        core + 0.8 * inner + 0.7 * outer
    }

    /// Amplitud de cada pulso: cambia de una vuelta a la siguiente.
    fn pulse(&self, north: bool) -> f32 {
        let turn = self.turn;
        let k = turn.floor();
        let seed = if north { 0x51A7 } else { 0xB0E4 };
        let h = |n: f32| (hash_u32(n as i32 as u32 ^ seed) >> 8) as f32 / 16_777_216.0;
        let f = smoothstep(0.0, 1.0, turn - k);
        h(k) + (h(k + 1.0) - h(k)) * f
    }

    /// Luz de los haces que el rayo `dir` (unitario) cruza antes de `depth`.
    pub fn beams(&self, dir: Vec3, depth: f32) -> Vec3 {
        let widest = self.beam.outer * 1.25;
        let cos_max = widest.cos();
        let mut light = 0.0;
        // Tres tiempos de exposicion: el haz barre su ancho en un frame.
        for k in 0..3 {
            let f = (k as f32 + 0.5) / 3.0;
            let axis = (self.magnetic_before * (1.0 - f) + self.magnetic * f).normalize();
            light += self.cone_light(dir, depth, axis, cos_max) / 3.0;
        }
        let tint = Vec3::new(0.55, 0.75, 1.0);
        tint * (light * self.beam_gain)
    }

    /// La luz de los dos conos con eje `axis` a lo largo del rayo.
    fn cone_light(&self, dir: Vec3, depth: f32, axis: Vec3, cos_max: f32) -> f32 {
        // Puntos del rayo relativos a la estrella: x(t) = p + t dir.
        let p = -self.center;
        let (pa, da, pd, pp) = (p.dot(axis), dir.dot(axis), p.dot(dir), p.dot(p));
        let c2 = cos_max * cos_max;
        // (x.a)^2 - c^2 |x|^2 = A t^2 + 2 B t + C, positivo dentro del cono.
        let a = da * da - c2;
        let b = pa * da - c2 * pd;
        let c = pa * pa - c2 * pp;
        let disc = b * b - a * c;
        if disc <= 0.0 {
            // Sin cruces: o todo el rayo esta dentro de un cono (mira a lo
            // largo del haz desde adentro) o todo afuera.
            if a <= 0.0 || c <= 0.0 {
                return 0.0;
            }
            return self.segments_light(&[(0.0, depth)], dir, axis, pa, da, pd);
        }
        let root = disc.sqrt();
        let (t1, t2) = if a.abs() < 1e-9 {
            let t = -c / (2.0 * b);
            (t, t)
        } else {
            let (r1, r2) = ((-b - root) / a, (-b + root) / a);
            (r1.min(r2), r1.max(r2))
        };
        // Tramos del rayo dentro del cono, entre la camara y la superficie.
        let mut segments = [(0.0f32, 0.0f32); 2];
        let count = if a < 0.0 {
            segments[0] = (t1.max(0.0), t2.min(depth));
            1
        } else {
            segments[0] = (0.0, t1.min(depth));
            segments[1] = (t2.max(0.0), depth);
            2
        };
        self.segments_light(&segments[..count], dir, axis, pa, da, pd)
    }

    /// La luz de los tramos `segments` del rayo que estan dentro de un cono.
    fn segments_light(&self, segments: &[(f32, f32)], dir: Vec3, axis: Vec3, pa: f32, da: f32, pd: f32) -> f32 {
        let p = -self.center;
        // La recta: punto mas cercano a la estrella y su distancia.
        let tc = -pd;
        let q = (p + dir * tc).length().max(1e-3);
        let mut light = 0.0;
        for &(s0, s1) in segments {
            if s1 <= s0 {
                continue;
            }
            let mid = p + dir * (0.5 * (s0 + s1).min(s0 + 1e5));
            let along = mid.dot(axis);
            let north = along > 0.0;
            let sign = if north { 1.0 } else { -1.0 };
            // Solo arriba de la altura de emision.
            let (mut s0, mut s1) = (s0, s1.min(1e7));
            let rate = da * sign;
            let start = pa * sign;
            if rate.abs() > 1e-6 {
                let at = (self.beam.height - start) / rate;
                if rate > 0.0 {
                    s0 = s0.max(at);
                } else {
                    s1 = s1.min(at);
                }
            } else if start < self.beam.height {
                continue;
            }
            if s1 <= s0 {
                continue;
            }
            let pulse = self.pulse(north);
            let beam_axis = axis * sign;
            // Ejes alrededor del haz para el azimut de las chispas.
            let side = beam_axis.cross(if beam_axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize();
            let up = beam_axis.cross(side);
            // Cambio de variable t = tc + q tan(u): con 1/r^2 queda du / q.
            let (u0, u1) = (((s0 - tc) / q).atan(), ((s1 - tc) / q).atan());
            const STEPS: usize = 10;
            let du = (u1 - u0) / STEPS as f32;
            let mut sum = 0.0;
            for k in 0..STEPS {
                let uu = u0 + (k as f32 + 0.5) * du;
                let t = tc + q * uu.tan();
                let x = p + dir * t;
                let r = x.length();
                let cos_t = (x.dot(beam_axis) / r).clamp(-1.0, 1.0);
                let theta = (1.0 - cos_t * cos_t).max(0.0).sqrt().atan2(cos_t);
                let phi = x.dot(up).atan2(x.dot(side));
                let wind = self.wind_reference / (x + self.center - self.wind_source).length().max(1.0);
                sum += self.profile(theta, phi, pulse) * wind * wind;
            }
            light += sum * du.abs() / q;
        }
        light
    }

    /// El destello del haz cuando apunta a la camara: la luz directa desde la
    /// region de emision, con el perfil al angulo de la camara (promediado en
    /// la exposicion, para no perder el pulso entre dos frames).
    pub fn glare(&self) -> f32 {
        let to_camera = -self.center.normalize();
        let mut sum = 0.0;
        for k in 0..8 {
            let f = (k as f32 + 0.5) / 8.0;
            let axis = (self.magnetic_before * (1.0 - f) + self.magnetic * f).normalize();
            for (sign, north) in [(1.0, true), (-1.0, false)] {
                let cos_t = (to_camera.dot(axis) * sign).clamp(-1.0, 1.0);
                if cos_t > 0.0 {
                    let theta = cos_t.acos();
                    sum += self.profile(theta, 0.0, self.pulse(north)) / 8.0;
                }
            }
        }
        sum
    }
}

/// El shader de la esfera de apoyo de Faro.
#[inline]
pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    match u.faro {
        Some(faro) => faro.look(f.pos.normalize(), u),
        None => Vec3::ZERO,
    }
}
