//! Geodesicas de la luz alrededor de Faro, la estrella de neutrones.
//!
//! # La metrica
//! Afuera de una estrella esferica el espacio-tiempo es el de Schwarzschild
//! (teorema de Birkhoff): la luz sigue la misma ecuacion de Binet que en
//! Gargantua (`geodesic.rs`), en unidades con `rs = 2GM/c^2 = 1`,
//!
//! ```text
//!   u'' + u = (3/2) u^2,     u = 1/r,
//! ```
//!
//! integrada como la aceleracion central `a = -(3/2) h^2 r / |r|^5`.
//!
//! # Lo distinto: una superficie
//! Faro tiene 1.4 masas solares y 12 km de radio: su radio de Schwarzschild
//! es `rs = 2GM/c^2 = 4.13 km` y su compacidad `rs/R = 0.345`. La superficie
//! (`R = 2.9 rs`) queda afuera de la esfera de fotones (`1.5 rs`), asi que
//! ningun rayo da vueltas: cada uno o toca la superficie o escapa. Los que
//! la tocan revelan mas de media estrella: el rayo rasante llega desde la
//! colatitud `psi_max = 123 grados` (con la aproximacion de Beloborodov 2002,
//! `1 - cos alpha = (1 - cos psi)(1 - rs/R)`; aqui se integra exacto) y el
//! disco se ve con el radio aparente `R / sqrt(1 - rs/R) = 14.9 km`.
//!
//! # La tabla
//! Igual que en Gargantua: desde una camara a distancia `D` el camino de un
//! rayo depende solo del angulo `s` entre el rayo y la direccion a la
//! estrella. Cada vez que la distancia cambia se integran unos cientos de
//! rayos en el plano (RK4) y cada pixel busca en la tabla:
//! - si toca la superficie, el angulo de posicion `psi` donde la toca;
//! - si escapa, la direccion con la que sale al cielo.
//!
//! La camara es un observador estatico: su angulo local `s` se pasa a
//! coordenadas acortando la componente radial por el lapso `sqrt(1 - 1/D)`,
//! y el parametro de impacto es `b = D sin s / sqrt(1 - 1/D)`.

use std::f32::consts::PI;

use super::geodesic::weak_deflection;
use crate::parallel::{self, DisjointMut};

/// Rayos de la tabla.
const RAYS: usize = 768;
/// Fraccion de los rayos que caen dentro del disco.
const DISC_SHARE: f32 = 0.5;

/// Lo que le pasa a un rayo: si toca la superficie y el angulo que importa
/// (donde la toca o hacia donde sale).
#[derive(Clone, Copy, Debug, Default)]
struct Ray {
    hit: bool,
    angle: f32,
}

/// Un rayo ya buscado en la tabla.
#[derive(Clone, Copy, Debug)]
pub enum Sight {
    /// Toca la superficie en el angulo de posicion `psi` (medido desde la
    /// direccion estrella -> camara) y sale de ella con angulo `alpha`
    /// respecto a la vertical local (`cos_alpha`).
    Surface { psi: f32, cos_alpha: f32 },
    /// Escapa: su direccion final forma el angulo `exit` con la direccion
    /// estrella -> camara, en el plano del rayo.
    Sky { exit: f32 },
}

pub struct NeutronLens {
    /// Distancia de la camara a la estrella y radio de la estrella, en `rs`.
    pub observer: f32,
    pub radius: f32,
    /// Lapso del observador, `sqrt(1 - 1/D)`.
    pub lapse: f32,
    /// Angulo `s` del borde del disco visto desde la camara.
    edge: f32,
    span: f32,
    angles: Vec<f32>,
    rays: Vec<Ray>,
}

impl NeutronLens {
    /// Integra la tabla para una estrella de radio `radius` (en `rs`) vista
    /// desde `observer` radios de Schwarzschild.
    pub fn build(radius: f32, observer: f32) -> Self {
        let mut lens = Self {
            observer: 0.0,
            radius,
            lapse: 0.0,
            edge: 0.0,
            span: 0.0,
            angles: vec![0.0; RAYS],
            rays: vec![Ray::default(); RAYS],
        };
        lens.rebuild(observer);
        lens
    }

    /// Parametro de impacto del rayo rasante: `b_c = R / sqrt(1 - rs/R)`.
    pub fn critical_impact(&self) -> f32 {
        self.radius / (1.0 - 1.0 / self.radius).sqrt()
    }

    /// Si la tabla sirve para una camara a `observer` (cambio menor que un
    /// cuarto por ciento: el borde del disco se mueve menos de un pixel).
    pub fn fits(&self, observer: f32) -> bool {
        (observer.max(self.radius * 1.0001) / self.observer).ln().abs() < 0.0025
    }

    pub fn rebuild(&mut self, observer: f32) {
        let observer = observer.max(self.radius * 1.0001);
        self.observer = observer;
        self.lapse = (1.0 - 1.0 / observer).sqrt();
        // El rayo rasante tiene b = b_c: sin s = b_c lapso / D (Synge).
        self.edge = (self.critical_impact() * self.lapse / observer).min(1.0).asin();
        self.span = if observer < 80.0 { PI } else { PI * 0.5 }.max(self.edge * 1.5);
        for i in 0..RAYS {
            self.angles[i] = self.angle_of(i as f32 / (RAYS - 1) as f32);
        }
        let (angles, radius) = (&self.angles, self.radius);
        let rays = DisjointMut::new(&mut self.rays);
        parallel::for_each(RAYS, |i| {
            // SAFETY: cada rayo escribe solo su casilla.
            let slot = unsafe { &mut rays.range(i, 1)[0] };
            *slot = trace(observer, radius, angles[i]);
        });
    }

    /// Angulo del borde del disco visto desde la camara.
    pub fn edge(&self) -> f32 {
        self.edge
    }

    /// Angulo `s` del rayo en la posicion `x` (0 a 1) de la tabla: la mitad
    /// dentro del disco, mas densa junto al borde (ahi `psi` cambia rapido),
    /// y la otra mitad afuera, tambien mas densa junto al borde.
    fn angle_of(&self, x: f32) -> f32 {
        let edge = self.edge * 0.9995;
        if x < DISC_SHARE {
            let y = 1.0 - x / DISC_SHARE;
            edge * (1.0 - y * y)
        } else {
            let y = (x - DISC_SHARE) / (1.0 - DISC_SHARE);
            let outside = self.edge * 1.0005;
            outside + (self.span - outside) * y * y * y
        }
    }

    fn position_of(&self, s: f32) -> f32 {
        let edge = self.edge * 0.9995;
        let outside = self.edge * 1.0005;
        if s <= edge {
            let y = (1.0 - s / edge).max(0.0).sqrt();
            DISC_SHARE * (1.0 - y)
        } else if s < outside {
            DISC_SHARE
        } else {
            let y = ((s - outside) / (self.span - outside)).clamp(0.0, 1.0);
            DISC_SHARE + (1.0 - DISC_SHARE) * y.cbrt()
        }
    }

    /// Que ve el rayo que sale de la camara a un angulo `s` de la estrella.
    pub fn sight(&self, s: f32) -> Sight {
        if s > self.span {
            let exit = s.sin().atan2(-s.cos() * self.lapse) + weak_deflection(self.observer, s);
            return Sight::Sky { exit };
        }
        let x = self.position_of(s.max(0.0)) * (RAYS - 1) as f32;
        let lo = (x as usize).min(RAYS - 2);
        let frac = (x - lo as f32).clamp(0.0, 1.0);
        let (a, b) = (self.rays[lo], self.rays[lo + 1]);
        // En el borde, el vecino del otro lado no sirve: se usa el del mismo.
        let (ray, frac) = match (a.hit, b.hit) {
            (true, true) | (false, false) => (a, frac),
            (true, false) => (a, 0.0),
            (false, true) => (b, 0.0),
        };
        let angle = if a.hit == b.hit {
            a.angle + (b.angle - a.angle) * frac
        } else {
            ray.angle
        };
        if ray.hit {
            // sin alpha = b sqrt(1 - rs/R) / R.
            let b = self.observer * s.sin() / self.lapse;
            let sin_alpha = (b * (1.0 - 1.0 / self.radius).sqrt() / self.radius).min(1.0);
            Sight::Surface {
                psi: angle,
                cos_alpha: (1.0 - sin_alpha * sin_alpha).sqrt(),
            }
        } else {
            Sight::Sky { exit: angle }
        }
    }
}

/// Integra un rayo en el plano: sale de `(D, 0)` hacia la estrella girado `s`
/// hacia `+y`, hasta que toca la superficie `r = radius` o se aleja.
fn trace(observer: f32, radius: f32, s: f32) -> Ray {
    let radius = radius as f64;
    let (mut x, mut y) = (observer as f64, 0.0f64);
    let lapse = (1.0 - 1.0 / observer as f64).sqrt();
    let (mut vx, mut vy) = (-(s as f64).cos() * lapse, (s as f64).sin());
    let h2 = (x * vy - y * vx).powi(2);
    let accel = |x: f64, y: f64| {
        let r2 = (x * x + y * y).max(1e-12);
        let k = -1.5 * h2 / (r2 * r2 * r2.sqrt());
        (x * k, y * k)
    };
    let mut phi = 0.0f64;
    let mut heading = vy.atan2(vx);
    let step_of = |r: f64| (0.01 * r).clamp(0.0005, 50.0);
    let far = (observer as f64 * 4.0).max(300.0);
    for _ in 0..400_000 {
        let r = (x * x + y * y).sqrt();
        if r > far && x * vx + y * vy > 0.0 {
            break;
        }
        // Junto a la superficie el paso no la salta: se acorta para llegar.
        let mut h = step_of(r);
        let radial = (x * vx + y * vy) / r;
        if radial < 0.0 {
            h = h.min(((r - radius) / -radial).max(1e-6) * 1.01);
        }
        let (ax1, ay1) = accel(x, y);
        let (x2, y2, vx2, vy2) = (x + vx * h * 0.5, y + vy * h * 0.5, vx + ax1 * h * 0.5, vy + ay1 * h * 0.5);
        let (ax2, ay2) = accel(x2, y2);
        let (x3, y3, vx3, vy3) = (x + vx2 * h * 0.5, y + vy2 * h * 0.5, vx + ax2 * h * 0.5, vy + ay2 * h * 0.5);
        let (ax3, ay3) = accel(x3, y3);
        let (x4, y4, vx4, vy4) = (x + vx3 * h, y + vy3 * h, vx + ax3 * h, vy + ay3 * h);
        let (ax4, ay4) = accel(x4, y4);
        let (old_x, old_y, old_r) = (x, y, r);
        let (old_vx, old_vy) = (vx, vy);
        x += h / 6.0 * (vx + 2.0 * vx2 + 2.0 * vx3 + vx4);
        y += h / 6.0 * (vy + 2.0 * vy2 + 2.0 * vy3 + vy4);
        vx += h / 6.0 * (ax1 + 2.0 * ax2 + 2.0 * ax3 + ax4);
        vy += h / 6.0 * (ay1 + 2.0 * ay2 + 2.0 * ay3 + ay4);
        let turn = (old_x * y - old_y * x).atan2(old_x * x + old_y * y);
        heading += (old_vx * vy - old_vy * vx).atan2(old_vx * vx + old_vy * vy);
        let r_new = (x * x + y * y).sqrt();
        if r_new <= radius {
            // Cruzo la superficie en este paso: el angulo donde r = R, entre
            // el principio y el final del paso.
            let k = ((old_r - radius) / (old_r - r_new).max(1e-12)).clamp(0.0, 1.0);
            return Ray {
                hit: true,
                angle: (phi + turn * k) as f32,
            };
        }
        phi += turn;
    }
    // Lo que falta curvarse desde `far` hasta el infinito.
    let r = (x * x + y * y).sqrt();
    let rest = 0.375 * h2 * h2.sqrt() / (r * r * r * r);
    Ray {
        hit: false,
        angle: (heading + rest) as f32,
    }
}

/// `psi_max` de la aproximacion de Beloborodov (2002): la colatitud del
/// punto mas lejano que se ve, `cos psi = 1 - 1 / (1 - rs/R)`.
#[cfg(test)]
fn beloborodov_limit(radius: f32) -> f32 {
    (1.0 - 1.0 / (1.0 - 1.0 / radius)).acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1.4 masas solares y 12 km: R en radios de Schwarzschild.
    const R: f32 = 12.0 / 4.135;

    #[test]
    fn from_far_away_the_disc_has_the_apparent_radius_r_over_sqrt_of_one_minus_compactness() {
        let d = 5000.0;
        let lens = NeutronLens::build(R, d);
        let apparent = lens.edge().sin() * d / lens.lapse;
        let expected = R / (1.0 - 1.0 / R).sqrt();
        assert!((apparent / expected - 1.0).abs() < 1e-3, "{apparent} vs {expected}");
    }

    #[test]
    fn the_grazing_ray_shows_more_than_half_the_star_as_beloborodov_says() {
        let lens = NeutronLens::build(R, 4000.0);
        let Sight::Surface { psi, .. } = lens.sight(lens.edge() * 0.9999) else {
            panic!("el rayo rasante deberia tocar la superficie");
        };
        // Beloborodov (2002) acierta a pocos por ciento para R > 2 rs.
        let approx = beloborodov_limit(R);
        assert!(psi > PI * 0.5, "{psi}");
        assert!((psi / approx - 1.0).abs() < 0.05, "{psi} vs {approx}");
    }

    #[test]
    fn the_center_of_the_disc_is_the_point_facing_the_camera() {
        let lens = NeutronLens::build(R, 300.0);
        match lens.sight(0.0) {
            Sight::Surface { psi, cos_alpha } => {
                assert!(psi.abs() < 1e-4, "{psi}");
                assert!((cos_alpha - 1.0).abs() < 1e-4);
            }
            Sight::Sky { .. } => panic!("el centro deberia ser la superficie"),
        }
    }

    #[test]
    fn rays_that_miss_bend_like_the_weak_field_far_from_the_star() {
        let d = 200.0;
        let lens = NeutronLens::build(R, d);
        for s in [0.3f32, 0.6, 1.0] {
            let Sight::Sky { exit } = lens.sight(s) else { panic!() };
            let start = s.sin().atan2(-s.cos() * lens.lapse);
            let bend = exit - start;
            let expected = weak_deflection(d, s);
            assert!(bend > expected && bend < 1.06 * expected, "s {s}: {bend} vs {expected}");
        }
    }
}
