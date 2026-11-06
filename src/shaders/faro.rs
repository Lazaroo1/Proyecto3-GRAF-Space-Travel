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
//! # La corteza que se rompe
//! El campo de un magnetar estresa su corteza hasta romperla (Thompson y
//! Duncan 1995): la superficie es una red de placas oscuras de color violeta
//! con grietas que brillan, a dos escalas (Worley con el dominio deformado).
//! El brillo sigue a la temperatura: mas en los polos magneticos que en el
//! ecuador (Greenstein y Hartke) y las grietas se vuelven rosa y naranja en
//! los casquetes calientes del haz de corriente. Es color falso: a millones
//! de kelvin el ojo veria un blanco azulado parejo. El borde brilla como una
//! atmosfera: ahi la luz sale rasante (el angulo de emision de la lente).
//!
//! # Los haces
//! El pulsar emite en radio desde las lineas de campo abiertas, a cientos de
//! km sobre los polos magneticos, en un cono hueco con nucleo (Rankin). La
//! radio no se ve: lo que se dibuja es el viento de la supergigante,
//! iluminado por los haces. Su densidad baja como `1/d^2` desde la
//! supergigante y el flujo del haz como `1/r^2` desde Faro: la luz que se
//! dispersa hacia la camara es la integral de `perfil(theta) / r^2 * n` a lo
//! largo del rayo, que se hace exacta en `1/r^2` cambiando de variable.
//! El nucleo nace en la superficie: es el plasma que sale por las lineas
//! abiertas desde el casquete polar. El cono hueco esta hecho de chispas que
//! dan vueltas alrededor del eje (el carrusel de Ruderman y Sutherland) y
//! cada pulso es distinto, como en los magnetares. Un ruido estirado a lo
//! largo del eje que corre hacia afuera dibuja las hebras del chorro.
//!
//! # La bruma
//! Alrededor de la estrella brilla la nebulosa de su viento (como la del
//! Cangrejo, mucho mas chica): una densidad `1 / (1 + (r/a)^2)^2` cuya
//! integral a lo largo del rayo es exacta, con hebras de ruido.

use super::{Fragment, Uniforms};
use crate::math::neutron::{NeutronLens, Sight};
use crate::math::{Mat3, Perlin, Vec3, hash_u32, smoothstep};

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
    /// Centro relativo a la camara y radio de la estrella (km).
    pub center: Vec3,
    pub radius: f32,
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
    /// Brillo de la superficie.
    pub gain: f32,
    pub beam: BeamShape,
    /// Fase del giro, en vueltas: decide que pulso es (cada uno distinto).
    pub turn: f32,
    pub beam_gain: f32,
    /// Brillo de la bruma del viento.
    pub halo_gain: f32,
    /// Supergigante relativa a la camara y distancia de Faro a ella: el
    /// viento que dispersa los haces.
    pub wind_source: Vec3,
    pub wind_reference: f32,
    pub time: f32,
    /// Lo que no cambia en el frame y cada pixel volvia a calcular (ver
    /// `Faro::prepared`).
    pub derived: Derived,
}

/// Lo que se saca una vez por frame de lo de arriba: la direccion a la
/// estrella, la desviacion de la luz lejos de ella en una tabla, y lo de los
/// haces (los tres ejes de la exposicion, el cono que los abarca, el pulso
/// de cada uno y los ejes alrededor de cada haz para el azimut).
#[derive(Clone, Debug, Default)]
pub struct Derived {
    to_star: Vec3,
    /// La desviacion `delta = exit - (pi - s)` de los rayos a un angulo `s`
    /// de la estrella mayor que el de `bend_cos`, en funcion de
    /// `u = sqrt(1 + cos s)` (cerca de atras crece lineal en `u`):
    /// `bend[i]` es la de `u = i / bend_scale`. Vacia si no hace falta o si
    /// no alcanza la precision.
    bend: Vec<f32>,
    bend_cos: f32,
    bend_scale: f32,
    reach_cos: f32,
    middle: Vec3,
    cos_max: f32,
    axes: [Vec3; 3],
    pulses: [f32; 2],
    /// Por eje y por haz (norte, sur): `side` y `up`.
    frames: [[(Vec3, Vec3); 2]; 3],
}

/// Error maximo que se le acepta a la tabla de la desviacion contra
/// `sight` (rad): a 720p un pixel mide unos 1.5e-3 rad, asi que es un 1 %
/// de pixel. (La misma `sight` interpola su tabla de rayos y cerca de
/// 0.6 rad ya difiere unos 5e-6 de una tabla mas fina.)
const BEND_EPSILON: f32 = 2e-5;
/// Muestras de la tabla.
const BEND_SAMPLES: usize = 2048;

/// Colores de la corteza: placas, grietas, grietas calientes y borde.
const PLATE: Vec3 = Vec3::new(0.03, 0.012, 0.12);
const PLATE_GLOW: Vec3 = Vec3::new(0.25, 0.1, 0.9);
const CRACK: Vec3 = Vec3::new(1.0, 1.5, 4.2);
const CRACK_HOT: Vec3 = Vec3::new(3.2, 1.0, 1.6);
const CRACK_HOTTEST: Vec3 = Vec3::new(3.4, 1.6, 0.55);
const RIM: Vec3 = Vec3::new(0.4, 0.35, 2.0);
/// Colores de los haces: nucleo y conos.
const BEAM_CORE: Vec3 = Vec3::new(1.5, 1.85, 2.7);
const BEAM_CONE: Vec3 = Vec3::new(0.35, 0.6, 1.9);
/// La bruma del viento: color y escala (en radios).
const HALO: Vec3 = Vec3::new(0.42, 0.17, 1.0);
const HALO_SCALE: f32 = 1.6;

impl Faro<'_> {
    /// Saca lo que no cambia en el frame (`Derived`).
    ///
    /// La desviacion de la luz: cada pixel del cielo pedia a la lente su
    /// angulo (`atan2`, senos, cosenos, la desviacion debil), unos 300 ciclos.
    /// Lejos de la estrella (mas de tres veces su radio aparente, y al menos
    /// 0.5 rad) la desviacion es suave, asi que se tabula una vez por frame
    /// con la misma `sight` y se comprueba en los puntos de en medio: si en
    /// alguno se aleja mas de `BEND_EPSILON` de `sight`, no se usa la tabla.
    pub fn prepared(mut self) -> Self {
        use std::f32::consts::PI;
        let distance = self.center.length();
        let to_star = self.center / distance;
        let apparent = (self.lens.radius * 1.5 / self.lens.observer.max(1e-3)).min(1.0).asin();
        let start = (3.0 * apparent).max(0.5);
        let delta = |s: f32| match self.lens.sight(s) {
            Sight::Sky { exit } => Some(exit - (PI - s)),
            Sight::Surface { .. } => None,
        };
        let u_max = (1.0 + start.cos()).max(0.0).sqrt();
        let scale = (BEND_SAMPLES - 1) as f32 / u_max.max(1e-6);
        let at = |u: f32| (u * u - 1.0).clamp(-1.0, 1.0).acos();
        let table: Option<Vec<f32>> = (0..BEND_SAMPLES).map(|i| delta(at(i as f32 / scale))).collect();
        let bend = table.filter(|table| {
            (0..BEND_SAMPLES - 1).all(|i| {
                let mid = delta(at((i as f32 + 0.5) / scale));
                mid.is_some_and(|m| (0.5 * (table[i] + table[i + 1]) - m).abs() < BEND_EPSILON)
            })
        });
        let bend_cos = if bend.is_some() { start.cos() } else { -2.0 };
        let widest = self.beam.outer * 1.3;
        let sweep = self.magnetic_before.dot(self.magnetic).clamp(-1.0, 1.0).acos();
        let axes: [Vec3; 3] = std::array::from_fn(|k| {
            let f = (k as f32 + 0.5) / 3.0;
            (self.magnetic_before * (1.0 - f) + self.magnetic * f).normalize()
        });
        let frames = axes.map(|axis| {
            [1.0f32, -1.0].map(|sign| {
                let beam_axis = axis * sign;
                let side = beam_axis.cross(if beam_axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize();
                (side, beam_axis.cross(side))
            })
        });
        self.derived = Derived {
            to_star,
            bend: bend.unwrap_or_default(),
            bend_cos,
            bend_scale: scale,
            reach_cos: (widest + 0.5 * sweep).cos(),
            middle: (self.magnetic_before + self.magnetic).normalize(),
            cos_max: widest.cos(),
            axes,
            pulses: [self.pulse(true), self.pulse(false)],
            frames,
        };
        self
    }

    /// Temperatura de la superficie con normal `n` (en el mundo), en
    /// unidades de la del polo, y que tan caliente es por los casquetes.
    fn temperature(&self, n: Vec3) -> (f32, f32) {
        // Greenstein y Hartke (1983): el calor sube por la corteza a lo largo
        // del campo, asi que la superficie es mas fria donde B es horizontal.
        // Para un dipolo, cos(Theta_B) = 2 cos(theta) / sqrt(1 + 3 cos^2).
        let c = n.dot(self.magnetic);
        let cos_b2 = 4.0 * c * c / (1.0 + 3.0 * c * c);
        let t4 = cos_b2 + self.conductivity_ratio * (1.0 - cos_b2);
        let mut t = t4.sqrt().sqrt();
        // Casquetes polares: las lineas abiertas, calentadas por las
        // particulas que vuelven.
        let angle = (1.0 - c.abs()).max(0.0).sqrt() * 1.414;
        let cap = smoothstep(self.polar_cap * 4.0, self.polar_cap, angle);
        t = t.max(self.cap_temperature / self.pole_temperature * cap);
        let mut hot: f32 = 0.0;
        for spot in &self.spots {
            let angle = (1.0 - n.dot(spot.center)).max(0.0).sqrt() * 1.414;
            let w = (-(angle / spot.radius).powi(2)).exp();
            let excess = spot.temperature / self.pole_temperature;
            t = t.max(excess * w + t * (1.0 - w));
            hot = hot.max(w * (excess - 1.0).max(0.0) * 0.5);
        }
        (t * (1.0 + self.flash), hot.min(1.0))
    }

    /// Color de la corteza en el punto con normal `n`, que sale con el
    /// coseno `cos_alpha` respecto a la vertical.
    fn crust(&self, n: Vec3, cos_alpha: f32, noise: &Perlin) -> Vec3 {
        let (t, hot) = self.temperature(n);
        let p = self.to_crust.mul_vec(n);
        // Dominio deformado: las placas no salen como celdas de panal.
        // Un solo ruido mueve el punto a lo largo de una direccion fija.
        let warp = noise.noise(p * 2.1);
        let q = p * 3.2 + Vec3::new(0.8, -0.5, 0.35) * (warp * 0.7);
        let (f1, f2) = noise.cells(q);
        let edge = f2 - f1;
        let (g1, g2) = noise.cells(q * 2.7 + Vec3::splat(13.0));
        let fine = g2 - g1;
        // Las grietas laten: cada placa a su ritmo.
        let beat = 0.75 + 0.25 * (self.time * 2.3 + f1 * 17.0).sin();
        let crack = smoothstep(0.09, 0.0, edge) * beat + 0.45 * smoothstep(0.05, 0.0, fine);
        // Luz que se cuela por la corteza junto a las grietas.
        let seep = (-edge * 7.0).exp() * 0.35;
        let mottle = 0.7 + 0.3 * warp;
        let plate = PLATE * mottle + PLATE_GLOW * seep;
        let crack_color = if hot > 0.5 {
            CRACK_HOT.lerp(CRACK_HOTTEST, (hot - 0.5) * 2.0)
        } else {
            CRACK.lerp(CRACK_HOT, hot * 2.0)
        };
        // Destellos sueltos: puntos donde la corteza cede.
        let sparkle = smoothstep(0.07, 0.0, g1) * (0.5 + 0.5 * (self.time * 7.0 + g2 * 31.0).sin()).powi(4) * 2.5;
        let rim = (1.0 - cos_alpha.clamp(0.0, 1.0)).powf(2.5);
        let surface = plate * t + crack_color * (crack * t * t) + CRACK * sparkle;
        (surface * (self.gain * self.redshift)) + RIM * (rim * 1.4)
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
            Sight::Surface { psi, cos_alpha } => {
                let normal = e1 * psi.cos() + e2 * psi.sin();
                self.crust(normal, cos_alpha, u.noise)
            }
            Sight::Sky { exit } => super::sky(e1 * exit.cos() + e2 * exit.sin(), u),
        }
    }

    /// La direccion del cielo que se ve en `dir`: la gravedad de Faro curva
    /// la luz de las estrellas de fondo (un anillo de Einstein de varios
    /// grados cuando se pasa cerca).
    pub fn bend(&self, dir: Vec3) -> Vec3 {
        let d = &self.derived;
        let to_star = d.to_star;
        let cos_s = dir.dot(to_star).clamp(-1.0, 1.0);
        // Lejos de la estrella, de la tabla (ver `prepared`): con
        // `exit = pi - s + delta`, la direccion de salida es
        // `cos(delta) dir + sin(delta) (to_star sin s - e2 cos s)`.
        if cos_s < d.bend_cos && !d.bend.is_empty() {
            let sin_s = (1.0 - cos_s * cos_s).max(0.0).sqrt();
            if sin_s < 1e-6 {
                return dir;
            }
            let x = (1.0 + cos_s).max(0.0).sqrt() * d.bend_scale;
            let i = (x as usize).min(d.bend.len() - 2);
            let f = x - i as f32;
            let delta = d.bend[i] + (d.bend[i + 1] - d.bend[i]) * f;
            let e2 = (dir - to_star * cos_s) / sin_s;
            let d2 = delta * delta;
            let (sin_d, cos_d) = (delta * (1.0 - d2 / 6.0), 1.0 - 0.5 * d2);
            return dir * cos_d + (to_star * sin_s - e2 * cos_s) * sin_d;
        }
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

    /// La bruma del viento alrededor de la estrella a lo largo del rayo
    /// `dir` hasta `depth`. Con `x = t - t_c` y `k^2 = a^2 + q^2` (`q` es la
    /// distancia del rayo al centro), `int a^4 dx / (k^2 + x^2)^2` es
    /// `a^4 [x / (2 k^2 (k^2 + x^2)) + atan(x/k) / (2 k^3)]`.
    pub fn halo(&self, dir: Vec3, depth: f32, noise: &Perlin) -> Vec3 {
        if self.halo_gain <= 0.0 {
            return Vec3::ZERO;
        }
        let a = self.radius * HALO_SCALE;
        let tc = self.center.dot(dir);
        let closest = dir * tc - self.center;
        let q2 = closest.length_squared();
        // A 30 escalas ya no queda nada (cae como 1/q^3).
        if q2 > (a * 30.0) * (a * 30.0) {
            return Vec3::ZERO;
        }
        let k2 = a * a + q2;
        let k = k2.sqrt();
        let primitive = |x: f32| x / (2.0 * k2 * (k2 + x * x)) + (x / k).atan() / (2.0 * k2 * k);
        let (x0, x1) = (-tc, depth.min(1e7) - tc);
        if x1 <= x0 {
            return Vec3::ZERO;
        }
        let integral = a * a * a * a * (primitive(x1) - primitive(x0));
        // Hebras: ruido en el punto mas cercano, en el marco de la corteza
        // (gira con la estrella).
        let p = self.to_crust.mul_vec(closest / a);
        let wisps = 0.45 + 0.9 * (0.5 + 0.5 * noise.fbm(p * 0.9 + Vec3::new(0.0, self.time * 0.05, 0.0), 3)).powi(2);
        HALO * (integral / a * wisps * self.halo_gain)
    }

    /// Intensidad del haz a un angulo `theta` de su eje, un azimut `phi`
    /// alrededor de el y una altura `along` sobre el centro: el nucleo, los
    /// dos conos de chispas (desde su altura de emision) y la bruma.
    fn profile(&self, theta: f32, phi: f32, along: f32, pulse: f32) -> Vec3 {
        let b = &self.beam;
        let gauss = |x: f32, width: f32| (-(x / width) * (x / width)).exp();
        // Las chispas del carrusel: el anillo es mas brillante donde hay una.
        let sparks = b.sparks as f32;
        let k = (phi / std::f32::consts::TAU + b.carousel) * sparks;
        let spark = 0.35 + 0.65 * (1.0 - (k - k.round()).abs() * 2.0).powi(2);
        let cones = smoothstep(b.height * 0.5, b.height, along);
        let core = gauss(theta, b.core * 0.6) * (0.7 + 0.6 * pulse);
        let inner = gauss(theta - b.inner * 0.75, b.inner * 0.22) * spark;
        let outer = gauss(theta - b.outer * 0.8, b.outer * 0.2) * spark * (1.4 - pulse);
        BEAM_CORE * (core * 1.6) + BEAM_CONE * ((0.8 * inner + 0.7 * outer) * cones)
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
        let d = &self.derived;
        // Descarte: el cono que abarca toda la exposicion. La mayoria de los
        // pixeles no lo tocan y se ahorran los tres de adentro.
        if !self.touches(dir, depth, d.middle, d.reach_cos) {
            return Vec3::ZERO;
        }
        let mut light = Vec3::ZERO;
        // Tres tiempos de exposicion: el haz barre su ancho en un frame.
        for k in 0..3 {
            light += self.cone_light(dir, depth, k, d.cos_max) / 3.0;
        }
        light * self.beam_gain
    }

    /// Si el rayo entra al doble cono de eje `axis` antes de `depth`.
    fn touches(&self, dir: Vec3, depth: f32, axis: Vec3, cos_max: f32) -> bool {
        let p = -self.center;
        let (pa, da, pd, pp) = (p.dot(axis), dir.dot(axis), p.dot(dir), p.dot(p));
        let c2 = cos_max * cos_max;
        let (a, b, c) = (da * da - c2, pa * da - c2 * pd, pa * pa - c2 * pp);
        let disc = b * b - a * c;
        if disc <= 0.0 {
            return a > 0.0 && c > 0.0;
        }
        if a.abs() < 1e-9 {
            return true;
        }
        let root = disc.sqrt();
        let (r1, r2) = ((-b - root) / a, (-b + root) / a);
        let (t1, t2) = (r1.min(r2), r1.max(r2));
        if a < 0.0 {
            t2 > 0.0 && t1 < depth
        } else {
            t1 > 0.0 || t2 < depth
        }
    }

    /// La luz de los dos conos con el eje `k` de la exposicion a lo largo
    /// del rayo.
    fn cone_light(&self, dir: Vec3, depth: f32, k: usize, cos_max: f32) -> Vec3 {
        let axis = self.derived.axes[k];
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
                return Vec3::ZERO;
            }
            return self.segments_light(&[(0.0, depth)], dir, k, pa, da, pd);
        }
        let root = disc.sqrt();
        let (t1, t2) = if a.abs() < 1e-9 {
            let t = if b.abs() > 1e-12 { -c / (2.0 * b) } else { 0.0 };
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
        self.segments_light(&segments[..count], dir, k, pa, da, pd)
    }

    /// La luz de los tramos `segments` del rayo que estan dentro de un cono.
    fn segments_light(&self, segments: &[(f32, f32)], dir: Vec3, k: usize, pa: f32, da: f32, pd: f32) -> Vec3 {
        let axis = self.derived.axes[k];
        let p = -self.center;
        // La recta: punto mas cercano a la estrella y su distancia.
        let tc = -pd;
        let q = (p + dir * tc).length().max(1e-3);
        let mut light = Vec3::ZERO;
        for &(s0, s1) in segments {
            if s1 <= s0 {
                continue;
            }
            let mid = p + dir * (0.5 * (s0 + s1).min(s0 + 1e5));
            let north = mid.dot(axis) > 0.0;
            let sign = if north { 1.0 } else { -1.0 };
            // Solo afuera de la estrella.
            let (mut s0, mut s1) = (s0, s1.min(1e7));
            let rate = da * sign;
            let start = pa * sign;
            if rate.abs() > 1e-6 {
                let at = (self.radius - start) / rate;
                if rate > 0.0 {
                    s0 = s0.max(at);
                } else {
                    s1 = s1.min(at);
                }
            } else if start < self.radius {
                continue;
            }
            if s1 <= s0 {
                continue;
            }
            let pulse = self.derived.pulses[usize::from(!north)];
            let beam_axis = axis * sign;
            // Ejes alrededor del haz para el azimut de las chispas.
            let (side, up) = self.derived.frames[k][usize::from(!north)];
            // Cambio de variable t = tc + q tan(u): con 1/r^2 queda du / q.
            let (u0, u1) = (((s0 - tc) / q).atan(), ((s1 - tc) / q).atan());
            const STEPS: usize = 7;
            let du = (u1 - u0) / STEPS as f32;
            let mut sum = Vec3::ZERO;
            for k in 0..STEPS {
                let uu = u0 + (k as f32 + 0.5) * du;
                let t = tc + q * uu.tan();
                let x = p + dir * t;
                let r = x.length();
                let along = x.dot(beam_axis);
                let cos_t = (along / r).clamp(-1.0, 1.0);
                let theta = (1.0 - cos_t * cos_t).max(0.0).sqrt().atan2(cos_t);
                let (sx, sy) = (x.dot(side), x.dot(up));
                let phi = sy.atan2(sx);
                let wind = self.wind_reference / (x + self.center - self.wind_source).length().max(1.0);
                // Hebras del chorro: ruido estirado a lo largo del eje que
                // corre hacia afuera.
                let fiber = strands(phi, along.max(self.radius).ln() * 6.0 - self.time * 1.4);
                sum += self.profile(theta, phi, along, pulse) * (wind * wind * fiber);
            }
            light += sum * (du.abs() / q);
        }
        light
    }

    /// El destello del haz cuando apunta a la camara: la luz directa de la
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
                    sum += self.profile(theta, 0.0, self.beam.height, self.pulse(north)).y / 8.0;
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

/// Lado de la tabla de hebras de los haces (azimut por altura).
const STRANDS: usize = 128;

/// Las hebras del chorro: cuanto brilla el haz a un azimut `phi` alrededor de
/// su eje y una coordenada `s` a lo largo (en el logaritmo de la altura, que
/// corre hacia afuera con el tiempo). Sale de una tabla de ruido periodica
/// hecha al arrancar: evaluar Perlin en cada muestra de cada pixel costaba
/// decenas de milisegundos.
fn strands(phi: f32, s: f32) -> f32 {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let noise = Perlin::new(0x57A4);
        let mut table = vec![0.0; STRANDS * STRANDS];
        for j in 0..STRANDS {
            for i in 0..STRANDS {
                // Un toro en 4D proyectado en 3D da un ruido periodico en los
                // dos ejes: el azimut da la vuelta y la altura se repite.
                let a = i as f32 / STRANDS as f32 * std::f32::consts::TAU;
                let b = j as f32 / STRANDS as f32 * std::f32::consts::TAU;
                let p = Vec3::new(a.cos() * 3.0, a.sin() * 3.0, b.cos() * 1.2 + b.sin() * 0.9);
                let v = noise.fbm(p + Vec3::new(b.sin(), b.cos(), 0.0), 3);
                table[j * STRANDS + i] = 0.4 + 1.2 * (0.5 + 0.5 * v).powi(2);
            }
        }
        table
    });
    let x = (phi / std::f32::consts::TAU).rem_euclid(1.0) * STRANDS as f32;
    let y = (s / 8.0).rem_euclid(1.0) * STRANDS as f32;
    let (i, j) = (x as usize % STRANDS, y as usize % STRANDS);
    let (fx, fy) = (x.fract(), y.fract());
    let (i1, j1) = ((i + 1) % STRANDS, (j + 1) % STRANDS);
    let at = |i: usize, j: usize| table[j * STRANDS + i];
    let top = at(i, j) + (at(i1, j) - at(i, j)) * fx;
    let bottom = at(i, j1) + (at(i1, j1) - at(i, j1)) * fx;
    top + (bottom - top) * fy
}
