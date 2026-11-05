//! La magnetosfera torcida de Faro.
//!
//! # El campo
//! Un magnetar no tiene un dipolo limpio: los movimientos de su corteza
//! tuercen las lineas de campo que salen de ella, y la torsion lleva
//! corrientes que brillan. El modelo es la familia autosimilar de Thompson,
//! Lyutikov y Kulkarni (2002), tomada de Wolfson (1995): con `mu = cos theta`
//! y `r` en radios de la estrella, la funcion de flujo es
//! `P = (B_pole R^2 / 2) F(mu) / r^p` y
//!
//! ```text
//!   (1 - mu^2) F'' + p (p + 1) F + C F^(1 + 2/p) = 0,
//!   F(1) = 0,   F'(1) = -2,   F'(0) = 0,
//!
//!   B_r   = -(B_pole / 2) r^-(2+p) F'
//!   B_th  =  (B_pole / 2) r^-(2+p) p F / sin theta
//!   B_phi =  B_th sqrt(C / (p (1 + p))) F^(1/p)
//! ```
//!
//! Para cada `p` la constante `C` es el autovalor que cumple las tres
//! condiciones: se busca disparando desde el polo. `p = 1, C = 0` es el
//! dipolo (`F = 1 - mu^2`); al bajar `p` las lineas se inflan y se tuercen
//! mas, y la torsion entre los dos pies de una linea es
//!
//! ```text
//!   dphi = 2 sqrt(C / (p (1 + p))) int_0^1 F^(1/p) / (1 - mu^2) dmu,
//! ```
//!
//! que llega a pi cuando `p -> 0` (el monopolo partido).
//!
//! # Una linea
//! Las lineas son las curvas de `P` constante. La que nace en `mu_f` sube
//! hasta `r(mu) = (F(mu) / F(mu_f))^(1/p)` y su azimut avanza con
//! `dphi/dmu = -k F^(1/p) / (1 - mu^2)`, `k = sqrt(C / (p (1 + p)))`. Todas las
//! lineas de una familia tienen la misma forma, escalada.

use std::sync::OnceLock;

use crate::math::DVec3;

/// Muestras de `F` entre el ecuador (`mu = 0`) y el polo (`mu = 1`).
const SAMPLES: usize = 512;

/// La solucion para un `p`.
#[derive(Clone, Debug)]
pub struct Family {
    pub p: f64,
    /// `sqrt(C / (p (1 + p)))`.
    pub k: f64,
    /// Torsion entre los pies de una linea que sale del polo.
    pub twist: f64,
    /// `F(mu)` y `G(mu) = int_mu^1 F^(1/p) / (1 - mu^2)` en `mu = i / (SAMPLES-1)`.
    f: Vec<f64>,
    g: Vec<f64>,
}

/// Integra desde el polo hasta el ecuador: devuelve `F'(0)` y llena `f`
/// (de `mu = 0` a `mu = 1`) si se pide.
fn shoot(p: f64, c: f64, mut f: Option<&mut [f64]>) -> f64 {
    // Junto al polo la ecuacion es singular: se arranca con la serie
    // F = 2e - p(p+1) e^2 / 2, F' = -2 + p(p+1) e, con e = 1 - mu.
    let steps = 4000;
    let start = 1e-4;
    let mut mu = 1.0 - start;
    let mut y = 2.0 * start - 0.5 * p * (p + 1.0) * start * start;
    let mut dy = -2.0 + p * (p + 1.0) * start;
    let h = -(1.0 - start) / steps as f64;
    let rhs = |mu: f64, y: f64| -(p * (p + 1.0) * y + c * y.max(0.0).powf(1.0 + 2.0 / p)) / (1.0 - mu * mu);
    if let Some(out) = f.as_deref_mut() {
        out[SAMPLES - 1] = 0.0;
    }
    // Siguiente muestra de la grilla por llenar (de arriba hacia abajo).
    let mut next = SAMPLES as isize - 2;
    for i in 0..steps {
        let (old_mu, old_y) = (mu, y);
        // RK4 sobre (F, F').
        let k1 = (dy, rhs(mu, y));
        let k2 = (dy + 0.5 * h * k1.1, rhs(mu + 0.5 * h, y + 0.5 * h * k1.0));
        let k3 = (dy + 0.5 * h * k2.1, rhs(mu + 0.5 * h, y + 0.5 * h * k2.0));
        let k4 = (dy + h * k3.1, rhs(mu + h, y + h * k3.0));
        y += h / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0);
        dy += h / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1);
        mu = 1.0 - start - (1.0 - start) * (i + 1) as f64 / steps as f64;
        if let Some(out) = f.as_deref_mut() {
            // Las muestras de la grilla que cayeron en este paso, interpoladas
            // entre su principio y su final.
            while next >= 0 {
                let grid = next as f64 / (SAMPLES - 1) as f64;
                if grid < mu - 1e-12 {
                    break;
                }
                let t = ((old_mu - grid) / (old_mu - mu)).clamp(0.0, 1.0);
                out[next as usize] = old_y + (y - old_y) * t;
                next -= 1;
            }
        }
    }
    dy
}

impl Family {
    /// Resuelve la familia de indice `p`: `C` por biseccion sobre `F'(0)`.
    pub fn solve(p: f64) -> Self {
        let c = if (p - 1.0).abs() < 1e-9 {
            0.0
        } else {
            // F'(0) cambia de signo entre C = 0 y el maximo de la familia.
            let (mut lo, mut hi) = (0.0, 0.8735);
            let sign_lo = shoot(p, lo, None).signum();
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if shoot(p, mid, None).signum() == sign_lo {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        };
        let mut f = vec![0.0; SAMPLES];
        shoot(p, c, Some(&mut f));
        // G(mu) = int_mu^1 F^(1/p) / (1 - mu^2), por trapecios desde el polo.
        // Cerca del polo el integrando es (2 e)^(1/p) / (2 e): integrable.
        let mut g = vec![0.0; SAMPLES];
        let dmu = 1.0 / (SAMPLES - 1) as f64;
        let integrand = |j: usize| {
            let mu = j as f64 * dmu;
            if j == SAMPLES - 1 {
                // Limite en el polo: 1 si p = 1, 0 si p < 1.
                if (p - 1.0).abs() < 1e-9 { 1.0 } else { 0.0 }
            } else {
                f[j].max(0.0).powf(1.0 / p) / (1.0 - mu * mu)
            }
        };
        for j in (0..SAMPLES - 1).rev() {
            g[j] = g[j + 1] + 0.5 * (integrand(j) + integrand(j + 1)) * dmu;
        }
        let k = if c > 0.0 { (c / (p * (1.0 + p))).sqrt() } else { 0.0 };
        Self {
            p,
            k,
            twist: 2.0 * k * g[0],
            f,
            g,
        }
    }

    #[inline]
    fn sample(table: &[f64], mu: f64) -> f64 {
        let x = mu.clamp(0.0, 1.0) * (SAMPLES - 1) as f64;
        let i = (x as usize).min(SAMPLES - 2);
        let t = x - i as f64;
        table[i] + (table[i + 1] - table[i]) * t
    }

    pub fn f(&self, mu: f64) -> f64 {
        Self::sample(&self.f, mu.abs())
    }

    /// `int_mu^1 F^(1/p) / (1 - mu^2)` para cualquier `mu` (por simetria).
    fn g(&self, mu: f64) -> f64 {
        if mu >= 0.0 {
            Self::sample(&self.g, mu)
        } else {
            2.0 * self.g[0] - Self::sample(&self.g, -mu)
        }
    }

    /// Punto de la linea que nace en la colatitud `theta_f` (hemisferio
    /// norte) cuando su `mu` va de `mu_f` (`s = 0`) a `-mu_f` (`s = 1`): radio
    /// en radios de la estrella, colatitud y azimut desde el pie norte.
    pub fn line(&self, theta_f: f64, s: f64) -> (f64, f64, f64) {
        let mu_f = theta_f.cos();
        let mu = mu_f * (1.0 - 2.0 * s);
        let r = (self.f(mu) / self.f(mu_f)).powf(1.0 / self.p);
        let phi = self.k * (self.g(mu) - self.g(mu_f));
        (r, mu.clamp(-1.0, 1.0).acos(), phi)
    }
}

/// Las familias, de la menos torcida (el dipolo) a la mas torcida.
pub fn families() -> &'static [Family] {
    static TABLE: OnceLock<Vec<Family>> = OnceLock::new();
    TABLE.get_or_init(|| {
        // De p = 1 a p = 0.2 (torsion de 0 a unos 2.7 rad), con el autovalor
        // en la rama de C creciente hasta su maximo (p = 0.63) y despues
        // decreciente.
        (0..=40).map(|i| Family::solve(1.0 - 0.8 * i as f64 / 40.0)).collect()
    })
}

/// Las dos familias que rodean la torsion `twist` y el peso de la segunda.
pub fn bracket(twist: f64) -> (&'static Family, &'static Family, f64) {
    let all = families();
    let i = all.iter().position(|f| f.twist >= twist).unwrap_or(all.len() - 1).max(1);
    let (a, b) = (&all[i - 1], &all[i]);
    let w = ((twist - a.twist) / (b.twist - a.twist).max(1e-9)).clamp(0.0, 1.0);
    (a, b, w)
}

/// Punto de una linea con la torsion `twist`, en coordenadas magneticas
/// cartesianas (z sobre el eje magnetico), en radios de la estrella. La
/// linea nace en la colatitud `theta_f` y el azimut `phi_f` del norte.
pub fn line_point(twist: f64, theta_f: f64, phi_f: f64, s: f64) -> DVec3 {
    let (a, b, w) = bracket(twist);
    let (ra, ta, pa) = a.line(theta_f, s);
    let (rb, tb, pb) = b.line(theta_f, s);
    let (r, theta, phi) = (ra + (rb - ra) * w, ta + (tb - ta) * w, phi_f + pa + (pb - pa) * w);
    DVec3::new(theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos()) * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_one_is_the_dipole() {
        let f = Family::solve(1.0);
        assert!(f.k.abs() < 1e-12 && f.twist.abs() < 1e-12);
        for mu in [0.0, 0.3, 0.7] {
            assert!((f.f(mu) - (1.0 - mu * mu)).abs() < 1e-3, "{}", f.f(mu));
        }
        // La linea dipolar r = sin^2 theta / sin^2 theta_f.
        let theta_f = 0.3f64;
        let (apex, _, _) = f.line(theta_f, 0.5);
        assert!((apex - 1.0 / theta_f.sin().powi(2)).abs() < 1e-2, "{apex}");
    }

    /// La tabla de autovalores y torsiones de Thompson, Lyutikov y Kulkarni
    /// (2002), recalculada por disparo.
    #[test]
    fn the_eigenvalues_and_twists_match_tlk() {
        for (p, c, twist, f0) in [
            (0.9, 0.3898, 0.931, 1.056),
            (0.8, 0.6743, 1.311, 1.117),
            (0.5, 0.7543, 2.062, 1.345),
            (0.3, 0.2314, 2.460, 1.549),
        ] {
            let fam = Family::solve(p);
            let fam_c = fam.k * fam.k * p * (1.0 + p);
            assert!((fam_c - c).abs() < 3e-3, "p {p}: C {fam_c} vs {c}");
            assert!((fam.twist - twist).abs() < 0.02, "p {p}: twist {} vs {twist}", fam.twist);
            assert!((fam.f(0.0) - f0).abs() < 3e-3, "p {p}: F(0) {} vs {f0}", fam.f(0.0));
        }
    }

    #[test]
    fn the_twist_grows_as_p_falls() {
        let all = families();
        assert!(all.windows(2).all(|w| w[1].twist > w[0].twist));
        assert!(all.last().unwrap().twist > 2.5);
    }

    #[test]
    fn a_line_goes_from_its_north_foot_to_its_south_foot_twisted() {
        let fam = Family::solve(0.8);
        let theta_f = 0.25;
        let (r0, t0, phi0) = fam.line(theta_f, 0.0);
        let (r1, t1, phi1) = fam.line(theta_f, 1.0);
        assert!((r0 - 1.0).abs() < 1e-6 && (r1 - 1.0).abs() < 1e-3);
        assert!((t0 - theta_f).abs() < 1e-6 && (t1 - (std::f64::consts::PI - theta_f)).abs() < 1e-6);
        assert!(phi0.abs() < 1e-9 && phi1 > 0.5, "{phi1}");
    }
}
