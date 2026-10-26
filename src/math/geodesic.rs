//! Geodesicas de la luz alrededor de Gargantua (agujero de Schwarzschild).
//!
//! # De donde viene
//! La fisica es la del Proyecto 2 (`relativity.rs`): unidades con el radio
//! de Schwarzschild `rs = 1` (masa `M = 1/2`) y la geodesica nula escrita
//! como una aceleracion central,
//!
//! ```text
//!   a = -(3/2) rs h^2 r / |r|^5,     h = |r x v|  (se conserva)
//! ```
//!
//! que sale de la ecuacion de Binet `u'' + u = 3 M u^2` con `u = 1/r`.
//!
//! # Lo nuevo: una tabla por frame en vez de un rayo por pixel
//! El movimiento de un foton es plano y, desde una camara a distancia `D`
//! del agujero, su trayectoria depende de un solo numero: el angulo `s`
//! entre el rayo y la direccion hacia el agujero. Todos los pixeles con el
//! mismo `s` recorren la misma curva, solo girada alrededor de esa direccion.
//! Asi que cada frame se integran unos mil rayos en el plano (con RK4), uno
//! por cada `s`, y cada pixel solo busca en la tabla:
//! - si el rayo cae al horizonte o escapa, y en que direccion sale;
//! - el radio `r(phi)` de su camino, para saber donde cruza el disco.
//!
//! Los rayos se reparten mas densos junto al borde de la sombra, donde la
//! deflexion crece sin limite (ahi la luz da vueltas a la esfera de fotones).

use std::f32::consts::PI;

use crate::parallel::{self, DisjointMut};

/// Parametro de impacto critico `3 sqrt(3) M`: los rayos con menos caen.
pub const CRITICAL_IMPACT: f32 = 2.598_076_2;
/// Rayos de la tabla.
const RAYS: usize = 1024;
/// Muestras de `r(phi)` por rayo y angulo maximo que se sigue.
const PHI_SAMPLES: usize = 512;
const PHI_MAX: f32 = 4.0 * PI;
/// Fraccion de los rayos dentro de la sombra (alli solo importa por donde
/// cruzan el disco antes de caer).
const SHADOW_SHARE: f32 = 0.2;
/// Angulo maximo de la tabla con la direccion del agujero; mas alla se usa
/// la deflexion de campo debil.
const TABLE_SPAN: f32 = PI * 0.5;

/// Lo que le pasa a un rayo.
#[derive(Clone, Copy, Debug)]
pub struct Path {
    /// Si cae al horizonte.
    pub captured: bool,
    /// Angulo final de la direccion del foton, medido en su plano desde la
    /// direccion agujero -> camara (si escapo).
    pub exit: f32,
    /// Angulo de posicion recorrido hasta caer o hasta que ya no vuelve.
    pub end: f32,
}

pub struct LensTable {
    /// Distancia de la camara al agujero, en `rs`.
    pub observer: f32,
    /// Angulo `s` de la sombra (aproximado) y del final de la tabla.
    shadow: f32,
    span: f32,
    paths: Vec<Path>,
    /// `r(phi)` de cada rayo; `INFINITY` despues de escapar y `0` despues de
    /// caer.
    radius: Vec<f32>,
}

/// Un rayo concreto ya buscado en la tabla: interpola los dos vecinos.
#[derive(Clone, Copy, Debug)]
pub struct Lookup {
    lo: usize,
    hi: usize,
    frac: f32,
    pub path: Path,
}

impl LensTable {
    /// Integra la tabla para una camara a `observer` radios de Schwarzschild.
    pub fn build(observer: f32) -> Self {
        let observer = observer.max(3.0);
        let shadow = (CRITICAL_IMPACT / observer).min(1.0).asin();
        let span = TABLE_SPAN.max(shadow * 1.5);
        let mut table = Self {
            observer,
            shadow,
            span,
            paths: vec![
                Path {
                    captured: false,
                    exit: 0.0,
                    end: 0.0,
                };
                RAYS
            ],
            radius: vec![0.0; RAYS * PHI_SAMPLES],
        };
        let angles: Vec<f32> = (0..RAYS).map(|i| table.angle_of(i as f32 / (RAYS - 1) as f32)).collect();
        let paths = DisjointMut::new(&mut table.paths);
        let radius = DisjointMut::new(&mut table.radius);
        parallel::for_each(RAYS, |i| {
            // SAFETY: cada rayo escribe solo su casilla y su fila.
            let path = unsafe { &mut paths.range(i, 1)[0] };
            let row = unsafe { radius.range(i * PHI_SAMPLES, PHI_SAMPLES) };
            *path = trace(observer, angles[i], row);
        });
        table
    }

    /// Angulo `s` del rayo en la posicion `x` (de 0 a 1) de la tabla.
    fn angle_of(&self, x: f32) -> f32 {
        let edge = self.shadow * 0.98;
        if x < SHADOW_SHARE {
            edge * x / SHADOW_SHARE
        } else {
            let y = (x - SHADOW_SHARE) / (1.0 - SHADOW_SHARE);
            edge + (self.span - edge) * y * y * y
        }
    }

    /// Posicion (de 0 a 1) del angulo `s` en la tabla: el inverso de `angle_of`.
    fn position_of(&self, s: f32) -> f32 {
        let edge = self.shadow * 0.98;
        if s < edge {
            SHADOW_SHARE * s / edge
        } else {
            let y = ((s - edge) / (self.span - edge)).clamp(0.0, 1.0);
            SHADOW_SHARE + (1.0 - SHADOW_SHARE) * y.cbrt()
        }
    }

    /// Hasta que angulo con el agujero llega la tabla.
    pub fn span(&self) -> f32 {
        self.span
    }

    /// Busca el rayo que sale de la camara a un angulo `s` del agujero.
    pub fn lookup(&self, s: f32) -> Lookup {
        let x = self.position_of(s.max(0.0)) * (RAYS - 1) as f32;
        let lo = (x as usize).min(RAYS - 2);
        let frac = (x - lo as f32).clamp(0.0, 1.0);
        let hi = lo + 1;
        let (a, b) = (self.paths[lo], self.paths[hi]);
        let path = if a.captured == b.captured {
            Path {
                captured: a.captured,
                exit: a.exit + (b.exit - a.exit) * frac,
                end: a.end + (b.end - a.end) * frac,
            }
        } else if frac < 0.5 {
            a
        } else {
            b
        };
        Lookup { lo, hi, frac, path }
    }

    /// Radio del camino del rayo buscado cuando su angulo de posicion es
    /// `phi`. `None` si a ese angulo ya cayo o ya se fue.
    pub fn radius_at(&self, ray: &Lookup, phi: f32) -> Option<f32> {
        if phi < 0.0 || phi >= ray.path.end {
            return None;
        }
        let x = phi / PHI_MAX * (PHI_SAMPLES - 1) as f32;
        let k = x as usize;
        if k + 1 >= PHI_SAMPLES {
            return None;
        }
        let t = x - k as f32;
        let sample = |row: usize| {
            let base = row * PHI_SAMPLES;
            let (r0, r1) = (self.radius[base + k], self.radius[base + k + 1]);
            if r0.is_finite() && r1.is_finite() && r0 > 0.0 && r1 > 0.0 {
                Some(r0 + (r1 - r0) * t)
            } else {
                None
            }
        };
        match (sample(ray.lo), sample(ray.hi)) {
            (Some(a), Some(b)) => Some(a + (b - a) * ray.frac),
            (Some(a), None) if ray.frac < 0.5 => Some(a),
            (None, Some(b)) if ray.frac >= 0.5 => Some(b),
            _ => None,
        }
    }
}

/// Deflexion de campo debil para un rayo que sale a un angulo `s` del
/// agujero desde la distancia `observer`: lo que se curva desde la camara
/// hasta el infinito. Integrando la aceleracion `1.5 b^2 / r^4` a lo largo
/// de la recta, desde la camara (`l = -D cos s`) hasta el infinito, da
/// `(1 + cos s (1 + sin^2 s / 2)) / b` con `b = D sin s`; para el camino
/// completo (`s -> 0`, `D -> infinito`) es el conocido `2 / b = 4M / b`.
pub fn weak_deflection(observer: f32, s: f32) -> f32 {
    let b = observer * s.sin();
    if b <= 0.0 {
        return 0.0;
    }
    let (sin, cos) = s.sin_cos();
    (1.0 + cos * (1.0 + 0.5 * sin * sin)) / b
}

/// Integra un rayo en el plano: sale de `(D, 0)` hacia el agujero, girado
/// `s` hacia `+y`. Llena `row` con `r(phi)` y devuelve su destino.
fn trace(observer: f32, s: f32, row: &mut [f32]) -> Path {
    let (mut x, mut y) = (observer as f64, 0.0f64);
    let (mut vx, mut vy) = (-(s as f64).cos(), (s as f64).sin());
    let h2 = (x * vy - y * vx).powi(2);
    let accel = |x: f64, y: f64| {
        let r2 = (x * x + y * y).max(1e-12);
        let k = -1.5 * h2 / (r2 * r2 * r2.sqrt());
        (x * k, y * k)
    };
    // Angulo de posicion y de la velocidad, desenrollados.
    let mut phi = 0.0f64;
    let mut heading = (vy.atan2(vx)) as f64;
    let mut next = 0usize;
    let step_of = |r: f64| (0.02 * r).clamp(0.002, 50.0);
    let far = (observer as f64 * 4.0).max(300.0);
    row[0] = observer;
    next += 1;
    let mut captured = false;
    for _ in 0..200_000 {
        let r = (x * x + y * y).sqrt();
        if r < 1.0 {
            captured = true;
            break;
        }
        // Afuera y alejandose: ya no vuelve.
        if r > far && x * vx + y * vy > 0.0 {
            break;
        }
        if phi >= PHI_MAX as f64 {
            break;
        }
        // RK4 sobre (posicion, velocidad).
        let h = step_of(r);
        let (ax1, ay1) = accel(x, y);
        let (x2, y2, vx2, vy2) = (x + vx * h * 0.5, y + vy * h * 0.5, vx + ax1 * h * 0.5, vy + ay1 * h * 0.5);
        let (ax2, ay2) = accel(x2, y2);
        let (x3, y3, vx3, vy3) = (x + vx2 * h * 0.5, y + vy2 * h * 0.5, vx + ax2 * h * 0.5, vy + ay2 * h * 0.5);
        let (ax3, ay3) = accel(x3, y3);
        let (x4, y4, vx4, vy4) = (x + vx3 * h, y + vy3 * h, vx + ax3 * h, vy + ay3 * h);
        let (ax4, ay4) = accel(x4, y4);
        let (old_x, old_y) = (x, y);
        let (old_vx, old_vy) = (vx, vy);
        x += h / 6.0 * (vx + 2.0 * vx2 + 2.0 * vx3 + vx4);
        y += h / 6.0 * (vy + 2.0 * vy2 + 2.0 * vy3 + vy4);
        vx += h / 6.0 * (ax1 + 2.0 * ax2 + 2.0 * ax3 + ax4);
        vy += h / 6.0 * (ay1 + 2.0 * ay2 + 2.0 * ay3 + ay4);
        // Angulos desenrollados: se suma el giro de este paso.
        phi += (old_x * y - old_y * x).atan2(old_x * x + old_y * y);
        heading += (old_vx * vy - old_vy * vx).atan2(old_vx * vx + old_vy * vy);
        // Muestras de r en los angulos de la grilla que se cruzaron.
        let r_new = (x * x + y * y).sqrt();
        while next < row.len() && (next as f64) * (PHI_MAX as f64) / ((row.len() - 1) as f64) <= phi {
            row[next] = r_new as f32;
            next += 1;
        }
    }
    let end = phi as f32;
    let fill = if captured { 0.0 } else { f32::INFINITY };
    for r in &mut row[next..] {
        *r = fill;
    }
    // Lo que falta curvarse desde `far` hasta el infinito, saliendo casi en
    // linea recta: 1.5 b^3 / (4 r^4), despreciable pero gratis.
    let r = (x * x + y * y).sqrt();
    let rest = if captured { 0.0 } else { 0.375 * h2 * h2.sqrt() / (r * r * r * r) };
    Path {
        captured,
        exit: (heading + rest) as f32,
        end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rays_inside_the_shadow_fall_and_the_rest_escape() {
        let table = LensTable::build(40.0);
        let shadow = (CRITICAL_IMPACT / 40.0).asin();
        assert!(table.lookup(shadow * 0.9).path.captured);
        assert!(!table.lookup(shadow * 1.1).path.captured);
    }

    #[test]
    fn far_from_the_hole_the_bending_matches_the_weak_field() {
        let d = 100.0;
        let table = LensTable::build(d);
        for s in [0.4f32, 0.8, 1.2] {
            let path = table.lookup(s).path;
            // Sin curvatura saldria con angulo pi - s.
            let bend = path.exit - (PI - s);
            // El campo debil es la aproximacion de primer orden: el segundo
            // orden agrega unos puntos porcentuales.
            let expected = weak_deflection(d, s);
            assert!(bend > expected && bend < 1.06 * expected, "s {s}: {bend} vs {expected}");
        }
    }

    #[test]
    fn the_path_comes_closest_near_the_impact_parameter() {
        let d = 60.0;
        let table = LensTable::build(d);
        let s = 0.5f32;
        let ray = table.lookup(s);
        let b = d * s.sin();
        let closest = (0..400)
            .filter_map(|k| table.radius_at(&ray, k as f32 * 0.01))
            .fold(f32::MAX, f32::min);
        // Lejos del agujero la periapsis casi es b (un poco menos: la luz cae).
        assert!(closest < b && closest > 0.9 * b, "{closest} vs {b}");
    }

    #[test]
    fn grazing_rays_circle_the_photon_sphere() {
        let table = LensTable::build(30.0);
        let edge = (CRITICAL_IMPACT / 30.0).asin();
        // Justo afuera de la sombra el rayo da mas de media vuelta.
        let ray = table.lookup(edge * 1.0005);
        assert!(!ray.path.captured);
        assert!(ray.path.exit - (PI - edge) > PI * 0.5, "{}", ray.path.exit);
    }
}
