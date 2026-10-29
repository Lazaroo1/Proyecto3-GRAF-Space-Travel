//! La cola de Pira: el polvo y el vapor de roca que la estrella le arranca.
//!
//! Un planeta que se desintegra junto a su estrella (como K2-22b o
//! KIC 1255b) no echa un chorro hacia ella: la radiacion empuja el polvo
//! hacia afuera y el planeta, que sigue orbitando, lo deja atras. La cola
//! sale del lado de noche, apunta lejos de la estrella y se curva hacia
//! atras en la orbita.
//!
//! En el marco de la cola (x lejos de la estrella, y hacia atras en la
//! orbita) su eje es la parabola `y = c x^2`. Se marcha el rayo con pocas
//! muestras dentro de la esfera que la envuelve; en cada una la densidad es
//! un tubo gaussiano alrededor del eje que se ensancha y se desvanece, por
//! un ruido estirado a lo largo de la cola: estrias que fluyen hacia afuera.
//!
//! El polvo no brilla solo: lo ilumina la estrella, mucho mas a contraluz
//! (dispersion hacia adelante, Henyey-Greenstein). Junto al planeta el vapor
//! esta caliente y si brilla, con su color de cuerpo negro.

use crate::math::blackbody::planckian_rgb;
use crate::math::{Perlin, Vec3, smoothstep};

/// Muestras por rayo dentro de la cola.
const STEPS: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Tail {
    /// Centro del planeta, relativo a la camara.
    pub origin: Vec3,
    /// Ejes de la cola: lejos de la estrella y hacia atras en la orbita.
    pub away: Vec3,
    pub lag: Vec3,
    /// Largo de la cola a lo largo de `away` y cuanto se curva al final.
    pub length: f32,
    pub drift: f32,
    pub planet_radius: f32,
    /// Centro de la estrella, relativo a la camara.
    pub star: Vec3,
    /// Luz de la estrella que llega al planeta.
    pub light: Vec3,
    pub time: f32,
}

impl Tail {
    /// Ancho del tubo: nace del tamano del planeta y se abre.
    #[inline]
    fn width(&self, s: f32) -> f32 {
        self.planet_radius * (0.5 + 2.4 * s)
    }

    /// Densidad del polvo en `p` (relativo a la camara) y su avance `s`.
    #[inline]
    fn density(&self, p: Vec3, noise: &Perlin) -> (f32, f32) {
        let q = p - self.origin;
        let x = q.dot(self.away);
        let s = x / self.length;
        if !(0.0..1.0).contains(&s) {
            return (0.0, s);
        }
        let y = q.dot(self.lag);
        let z = (q - self.away * x - self.lag * y).length();
        // Distancia a la parabola, corregida por su pendiente.
        let c = self.drift / (self.length * self.length);
        let slope = 2.0 * c * x;
        let off = (y - c * x * x) / (1.0 + slope * slope).sqrt();
        let w = self.width(s);
        let r = (off * off + z * z).sqrt() / w;
        if r > 2.2 {
            return (0.0, s);
        }
        let rest = 1.0 - s;
        let fade = smoothstep(0.0, 0.05, s) * rest * rest.sqrt();
        // Estrias: ruido estirado a lo largo de la cola que fluye hacia afuera.
        let streaks = noise.noise(Vec3::new(
            x * 0.003 - self.time * 0.04,
            y / w * 1.6,
            z / w * 1.6,
        )) * 0.5
            + 0.5;
        (
            (-r * r).exp() * fade * (0.15 + 0.85 * streaks * streaks) / w,
            s,
        )
    }

    /// Luz de la cola en la direccion `dir` (unitaria) antes de `depth`.
    pub fn light(&self, dir: Vec3, depth: f32, noise: &Perlin) -> Vec3 {
        // La esfera que envuelve la cola.
        let center =
            self.origin + self.away * (self.length * 0.5) + self.lag * (self.drift * 0.3);
        let radius = self.length * 0.6 + self.width(1.0);
        let b = dir.dot(center);
        let disc = b * b - (center.length_squared() - radius * radius);
        if disc <= 0.0 {
            return Vec3::ZERO;
        }
        let root = disc.sqrt();
        let (mut t0, mut t1) = ((b - root).max(0.0), (b + root).min(depth));
        // Solo el tramo del rayo entre el planeta y la punta de la cola: x
        // (lo lejos que esta de la estrella) cambia linealmente con t.
        let (rate, start) = (dir.dot(self.away), -self.origin.dot(self.away));
        if rate.abs() > 1e-6 {
            let (a, b) = (-start / rate, (self.length - start) / rate);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        } else if !(0.0..self.length).contains(&start) {
            return Vec3::ZERO;
        }
        if t1 <= t0 {
            return Vec3::ZERO;
        }
        // Si el rayo pasa lejos del eje (recto) mas de lo que la cola se
        // abre y se curva ahi, no la toca.
        let e = -self.origin.dot(self.away);
        let d = -self.origin.dot(dir);
        let denom = 1.0 - rate * rate;
        let x = if denom > 1e-6 {
            ((e - rate * d) / denom).clamp(0.0, self.length)
        } else {
            self.length * 0.5
        };
        let on_axis = self.origin + self.away * x;
        let gap = (dir * dir.dot(on_axis).max(0.0) - on_axis).length();
        let s = x / self.length;
        if gap > 2.2 * self.width(s) + self.drift * s * s + 0.5 * self.planet_radius {
            return Vec3::ZERO;
        }
        let dt = (t1 - t0) / STEPS as f32;
        // Dispersion hacia adelante: brilla mucho mas a contraluz. La
        // direccion a la estrella casi no cambia a lo largo del rayo.
        let to_star = (self.star - dir * (0.5 * (t0 + t1))).normalize();
        let g: f32 = 0.65;
        let base = 1.0 + g * g - 2.0 * g * dir.dot(to_star);
        let phase = (1.0 - g * g) / (base * base.sqrt());
        let scattered = self.light * Vec3::new(0.9, 0.62, 0.42) * (0.05 + 0.08 * phase);
        // Vapor caliente junto al planeta, con el color de la roca fundida.
        let vapor = planckian_rgb(1900.0) * 0.5;
        let mut light = Vec3::ZERO;
        for k in 0..STEPS {
            let t = t0 + (k as f32 + 0.5) * dt;
            let (density, s) = self.density(dir * t, noise);
            if density > 0.0 {
                light += (scattered + vapor * (-s * 9.0).exp()) * (density * dt);
            }
        }
        light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_points_away_from_the_star() {
        let noise = Perlin::new(1);
        // El planeta adelante de la camara y la estrella a su izquierda.
        let origin = Vec3::new(0.0, 0.0, -3000.0);
        let tail = Tail {
            origin,
            away: Vec3::X,
            lag: Vec3::Y,
            length: 900.0,
            drift: 400.0,
            planet_radius: 100.0,
            star: origin - Vec3::X * 1000.0,
            light: Vec3::ONE,
            time: 5.0,
        };
        let look = |p: Vec3| tail.light(p.normalize(), f32::INFINITY, &noise);
        let behind = look(origin + Vec3::X * 300.0 + Vec3::Y * 40.0);
        let toward_star = look(origin - Vec3::X * 300.0);
        assert!(behind.x > 0.01, "la cola deberia verse: {behind:?}");
        assert_eq!(toward_star, Vec3::ZERO);
        // Un objeto delante tapa la cola.
        assert_eq!(
            tail.light((origin + Vec3::X * 300.0).normalize(), 100.0, &noise),
            Vec3::ZERO
        );
    }
}
