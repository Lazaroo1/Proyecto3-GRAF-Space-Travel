//! Atmosferas: dispersion simple de Rayleigh y Mie, calculada por pixel.
//!
//! # El modelo
//! La densidad del aire cae exponencialmente con la altura:
//! `rho(h) = e^(-h / H)`. Por cada pixel se recorre el tramo del rayo de la
//! camara que cruza la atmosfera (hasta la superficie, si la hay) en unos
//! pocos pasos. En cada paso:
//!
//! - llega luz de la estrella atenuada por el aire que atraviesa hasta ese
//!   punto (`T_sol`) y por el que hay de ahi a la camara (`T_vista`);
//! - una fraccion se desvia hacia la camara: Rayleigh (moleculas, mucho mas
//!   el azul que el rojo) y Mie (polvo y humo, casi gris, hacia adelante).
//!
//! Al final el color de atras se atenua con `T_vista` total y se le suma la
//! luz dispersada. Con eso salen solos el cielo de dia, el limbo que brilla
//! visto desde la orbita y los atardeceres: con la estrella baja, la luz
//! cruza tanta atmosfera que el azul se pierde antes de llegar.
//!
//! # Tabla de profundidad optica
//! `T_sol` necesita integrar la densidad desde cada punto hasta salir de la
//! atmosfera hacia la estrella. Eso solo depende de la altura y del angulo
//! con la vertical, asi que se precalcula en una tabla por planeta. Si el
//! rayo hacia la estrella choca con el planeta, la tabla guarda una
//! profundidad enorme: ese punto esta en la sombra del planeta.

use crate::math::{fast_exp, Vec3};
use crate::parallel::{self, DisjointMut};

#[derive(Clone, Copy, Debug)]
pub struct AtmosphereParams {
    /// Grosor sobre el radio base, en unidades del mundo.
    pub thickness: f32,
    /// Altura de escala: la densidad cae a `1/e` cada `scale_height`.
    pub scale_height: f32,
    /// Coeficientes de Rayleigh al nivel del suelo, por unidad de longitud.
    /// Su proporcion entre canales decide el color del cielo.
    pub rayleigh: Vec3,
    /// Coeficiente de Mie (polvo, humo) al nivel del suelo.
    pub mie: f32,
    /// Anisotropia de Mie: cuanto dispersa hacia adelante (0 a 1).
    pub mie_g: f32,
    /// Escala artistica de la luz dispersada.
    pub intensity: f32,
}

const H_SIZE: usize = 32;
const MU_SIZE: usize = 128;
/// Profundidad optica de un punto en la sombra del planeta.
const SHADOW: f32 = 1.0e6;

/// Atmosfera de un planeta con su tabla precalculada.
pub struct AtmosphereModel {
    pub params: AtmosphereParams,
    pub radius: f32,
    lut: Vec<f32>,
}

impl AtmosphereModel {
    pub fn new(radius: f32, params: AtmosphereParams) -> Self {
        let mut lut = vec![0.0; H_SIZE * MU_SIZE];
        for hi in 0..H_SIZE {
            let h = params.thickness * hi as f32 / (H_SIZE - 1) as f32;
            for mi in 0..MU_SIZE {
                let mu = -1.0 + 2.0 * mi as f32 / (MU_SIZE - 1) as f32;
                lut[hi * MU_SIZE + mi] = optical_depth(radius, &params, h, mu);
            }
        }
        Self { params, radius, lut }
    }

    /// Profundidad optica desde la altura `h` mirando con coseno cenital `mu`.
    #[inline]
    fn sun_depth(&self, h: f32, mu: f32) -> f32 {
        let x = (h / self.params.thickness).clamp(0.0, 1.0) * (H_SIZE - 1) as f32;
        let y = ((mu + 1.0) * 0.5).clamp(0.0, 1.0) * (MU_SIZE - 1) as f32;
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(H_SIZE - 1), (y0 + 1).min(MU_SIZE - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |h: usize, m: usize| self.lut[h * MU_SIZE + m];
        let a = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
        let b = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
        a + (b - a) * fy
    }
}

/// Integral de la densidad desde la altura `h` en la direccion de coseno
/// cenital `mu` hasta salir de la atmosfera (enorme si choca con el suelo).
fn optical_depth(radius: f32, p: &AtmosphereParams, h: f32, mu: f32) -> f32 {
    let r = radius + h;
    let b = r * mu;
    if mu < 0.0 && b * b - (r * r - radius * radius) > 0.0 {
        return SHADOW;
    }
    let top = radius + p.thickness;
    let exit = -b + (b * b - (r * r - top * top)).max(0.0).sqrt();
    let steps = 48;
    let ds = exit / steps as f32;
    let sin = (1.0 - mu * mu).max(0.0).sqrt();
    (0..steps)
        .map(|i| {
            let t = (i as f32 + 0.5) * ds;
            let (x, y) = (sin * t, r + mu * t);
            ((-((x * x + y * y).sqrt() - radius)) / p.scale_height).exp() * ds
        })
        .sum()
}

/// Una atmosfera en el frame actual.
pub struct AtmosphereFrame<'a> {
    pub model: &'a AtmosphereModel,
    /// Centro del planeta, relativo a la camara.
    pub center: Vec3,
    /// Direccion del centro del planeta hacia la estrella.
    pub sun_dir: Vec3,
}

/// Efecto de la atmosfera sobre un tramo de rayo: lo que de atras llega se
/// multiplica por `transmittance` y se le suma `scattered`. Dos tramos
/// seguidos se componen en uno solo (ver `then`).
#[derive(Clone, Copy, Debug)]
pub struct Fog {
    pub scattered: Vec3,
    pub transmittance: Vec3,
}

impl Fog {
    pub const CLEAR: Self = Self {
        scattered: Vec3::ZERO,
        transmittance: Vec3::ONE,
    };

    #[inline]
    pub fn apply(&self, color: Vec3) -> Vec3 {
        color * self.transmittance + self.scattered
    }

    /// Este tramo (lejano) seguido de `near`, mas cerca de la camara.
    #[inline]
    pub fn then(&self, near: &Fog) -> Fog {
        Fog {
            scattered: self.scattered * near.transmittance + near.scattered,
            transmittance: self.transmittance * near.transmittance,
        }
    }
}

impl AtmosphereFrame<'_> {
    /// Integra la atmosfera sobre el tramo del rayo `dir` (unitario) que va
    /// de la camara hasta `depth`, en `steps` pasos.
    pub fn integrate(&self, dir: Vec3, depth: f32, sun_light: Vec3, steps: usize) -> Fog {
        let m = self.model;
        let p = &m.params;
        let top = m.radius + p.thickness;
        let oc = -self.center;
        let b = oc.dot(dir);
        let disc = b * b - (oc.length_squared() - top * top);
        if disc <= 0.0 {
            return Fog::CLEAR;
        }
        let s = disc.sqrt();
        let t0 = (-b - s).max(0.0);
        let t1 = (-b + s).min(depth);
        if t1 <= t0 {
            return Fog::CLEAR;
        }

        let ds = (t1 - t0) / steps as f32;
        let extinction = p.rayleigh + Vec3::splat(p.mie * 1.1);
        let mut view_depth = 0.0;
        let mut sum = Vec3::ZERO;
        for i in 0..steps {
            let pos = oc + dir * (t0 + (i as f32 + 0.5) * ds);
            let r = pos.length();
            let h = (r - m.radius).max(0.0);
            let rho = fast_exp(-h / p.scale_height) * ds;
            // La mitad del paso antes de la muestra y la mitad despues.
            view_depth += rho * 0.5;
            let mu = pos.dot(self.sun_dir) / r;
            let tau = extinction * (view_depth + m.sun_depth(h, mu));
            sum += Vec3::new(fast_exp(-tau.x), fast_exp(-tau.y), fast_exp(-tau.z)) * rho;
            view_depth += rho * 0.5;
        }

        let cos = dir.dot(self.sun_dir);
        // Rayleigh: 3/(16 pi) (1 + cos^2). Mie: Henyey-Greenstein.
        let phase_r = 0.059_683 * (1.0 + cos * cos);
        let g = p.mie_g;
        let k = 1.0 + g * g - 2.0 * g * cos;
        let phase_m = 0.079_577 * (1.0 - g * g) / (k * k.sqrt());
        let tv = extinction * view_depth;
        Fog {
            scattered: sum * (p.rayleigh * phase_r + Vec3::splat(p.mie * phase_m)) * sun_light * p.intensity,
            transmittance: Vec3::new(fast_exp(-tv.x), fast_exp(-tv.y), fast_exp(-tv.z)),
        }
    }
}

/// Todas las atmosferas sobre el tramo de rayo `dir` hasta `depth`, de la
/// mas lejana a la mas cercana, compuestas en un solo efecto.
pub fn integrate_all(atmospheres: &[AtmosphereFrame], dir: Vec3, depth: f32, sun_light: Vec3, steps: usize) -> Fog {
    atmospheres.iter().fold(Fog::CLEAR, |fog, atmosphere| {
        fog.then(&atmosphere.integrate(dir, depth, sun_light, steps))
    })
}

const VIEW_AZIMUTH: usize = 64;
const VIEW_ZENITH: usize = 96;
const VIEW_STEPS: usize = 10;

/// Cielo precalculado desde la camara cuando esta dentro de una atmosfera
/// (la idea del "Sky-View LUT" de Hillaire, 2020).
///
/// Desde un punto fijo, la luz del cielo solo depende de dos angulos de la
/// direccion: el cenital (contra la vertical) y el acimut medido desde la
/// estrella (el cielo es simetrico respecto al plano vertical que la
/// contiene). Se integra una tabla de esos dos angulos una vez por frame y
/// cada pixel de cielo solo la lee: unos 6000 rayos por frame en vez de
/// medio millon.
///
/// El cenital no se reparte parejo: cerca del horizonte el cielo cambia mucho
/// mas rapido, asi que ahi van mas filas de la tabla.
pub struct SkyView {
    /// Posicion de la atmosfera en `Uniforms::atmospheres`.
    pub index: usize,
    up: Vec3,
    sun_side: Vec3,
    sun_other: Vec3,
    horizon: f32,
    data: Vec<Fog>,
}

impl SkyView {
    pub fn build(index: usize, frame: &AtmosphereFrame, sun_light: Vec3) -> Self {
        let up = (-frame.center).normalize();
        // Ejes del plano tangente: hacia la estrella y perpendicular.
        let mut sun_side = frame.sun_dir - up * frame.sun_dir.dot(up);
        if sun_side.length_squared() < 1e-8 {
            sun_side = if up.y.abs() < 0.9 { Vec3::Y.cross(up) } else { Vec3::X.cross(up) };
        }
        let sun_side = sun_side.normalize();
        let sun_other = up.cross(sun_side);
        let r = frame.center.length();
        let horizon = std::f32::consts::PI - (frame.model.radius / r).min(1.0).asin();
        let mut view = Self { index, up, sun_side, sun_other, horizon, data: vec![Fog::CLEAR; VIEW_AZIMUTH * VIEW_ZENITH] };
        let data = DisjointMut::new(&mut view.data);
        let geometry = (up, sun_side, sun_other, horizon);
        parallel::for_each(VIEW_ZENITH, |row| {
            let theta = zenith_of(row as f32 / (VIEW_ZENITH - 1) as f32, geometry.3);
            // SAFETY: cada fila escribe solo su propio rango.
            let out = unsafe { data.range(row * VIEW_AZIMUTH, VIEW_AZIMUTH) };
            for (column, fog) in out.iter_mut().enumerate() {
                let phi = column as f32 / (VIEW_AZIMUTH - 1) as f32 * std::f32::consts::PI;
                let (st, ct) = theta.sin_cos();
                let (sp, cp) = phi.sin_cos();
                let dir = geometry.0 * ct + (geometry.1 * cp + geometry.2 * sp) * st;
                *fog = frame.integrate(dir, f32::INFINITY, sun_light, VIEW_STEPS);
            }
        });
        view
    }

    /// Atmosfera del cielo en la direccion `dir` (unitaria).
    pub fn sample(&self, dir: Vec3) -> Fog {
        let theta = dir.dot(self.up).clamp(-1.0, 1.0).acos();
        let along = dir.dot(self.sun_side);
        let across = dir.dot(self.sun_other).abs();
        let phi = across.atan2(along);
        let x = phi / std::f32::consts::PI * (VIEW_AZIMUTH - 1) as f32;
        let y = coord_of(theta, self.horizon) * (VIEW_ZENITH - 1) as f32;
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(VIEW_AZIMUTH - 1), (y0 + 1).min(VIEW_ZENITH - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |c: usize, r: usize| &self.data[r * VIEW_AZIMUTH + c];
        let mix = |a: &Fog, b: &Fog, t: f32| Fog {
            scattered: a.scattered.lerp(b.scattered, t),
            transmittance: a.transmittance.lerp(b.transmittance, t),
        };
        mix(&mix(at(x0, y0), at(x1, y0), fx), &mix(at(x0, y1), at(x1, y1), fx), fy)
    }
}

/// Coordenada de tabla (0 a 1) de un angulo cenital: la mitad de arriba
/// cubre del cenit al horizonte y la de abajo del horizonte al nadir, las dos
/// con mas resolucion junto al horizonte (v = 0.5).
#[inline]
fn coord_of(theta: f32, horizon: f32) -> f32 {
    if theta < horizon {
        0.5 * (1.0 - (1.0 - theta / horizon).max(0.0).sqrt())
    } else {
        0.5 + 0.5 * ((theta - horizon) / (std::f32::consts::PI - horizon)).clamp(0.0, 1.0).sqrt()
    }
}

/// Inversa de `coord_of`.
#[inline]
fn zenith_of(v: f32, horizon: f32) -> f32 {
    if v < 0.5 {
        let s = 1.0 - 2.0 * v;
        horizon * (1.0 - s * s)
    } else {
        let s = 2.0 * v - 1.0;
        horizon + (std::f32::consts::PI - horizon) * s * s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> AtmosphereParams {
        AtmosphereParams {
            thickness: 20.0,
            scale_height: 5.0,
            rayleigh: Vec3::new(0.006, 0.016, 0.038),
            mie: 0.004,
            mie_g: 0.76,
            intensity: 18.0,
        }
    }

    #[test]
    fn sun_below_the_horizon_is_shadowed_and_above_is_not() {
        let model = AtmosphereModel::new(100.0, params());
        assert!(model.sun_depth(1.0, 1.0) < 10.0);
        assert!(model.sun_depth(1.0, -0.9) > 1000.0);
        // Rasante: mas aire que mirando hacia arriba.
        assert!(model.sun_depth(1.0, 0.05) > 3.0 * model.sun_depth(1.0, 1.0));
    }

    #[test]
    fn looking_up_from_the_ground_gives_a_sky_and_space_is_untouched() {
        let model = AtmosphereModel::new(100.0, params());
        // Camara a 1 unidad del suelo, planeta debajo, estrella arriba.
        let frame = AtmosphereFrame { model: &model, center: Vec3::new(0.0, -101.0, 0.0), sun_dir: Vec3::Y };
        let sky = frame.integrate(Vec3::Y, f32::INFINITY, Vec3::ONE, 6).apply(Vec3::ZERO);
        assert!(sky.z > sky.x, "el cielo debe tirar al azul: {sky:?}");
        // Un rayo que no toca la atmosfera no cambia nada.
        let far = AtmosphereFrame { model: &model, center: Vec3::new(0.0, -1000.0, 0.0), sun_dir: Vec3::Y };
        assert_eq!(far.integrate(Vec3::Y, f32::INFINITY, Vec3::ONE, 6).apply(Vec3::ONE), Vec3::ONE);
    }

    /// La tabla del cielo debe dar casi lo mismo que integrar cada rayo.
    #[test]
    fn sky_view_matches_direct_integration() {
        let model = AtmosphereModel::new(170.0, params());
        let frame = AtmosphereFrame {
            model: &model,
            center: Vec3::new(0.0, -176.0, 0.0),
            sun_dir: Vec3::new(0.3, 0.5, 0.8).normalize(),
        };
        let view = SkyView::build(0, &frame, Vec3::ONE);
        for i in 0..200 {
            let t = i as f32 * 0.031;
            let dir = Vec3::new(t.sin(), 0.15 + 0.8 * (t * 1.7).cos().abs(), t.cos()).normalize();
            let direct = frame.integrate(dir, f32::INFINITY, Vec3::ONE, VIEW_STEPS).scattered;
            let table = view.sample(dir).scattered;
            assert!((direct - table).length() <= direct.length() * 0.08 + 1e-4, "{dir:?}: {direct:?} vs {table:?}");
        }
        for v in [0.0, 0.2, 0.5, 0.7, 1.0] {
            assert!((coord_of(zenith_of(v, 1.9), 1.9) - v).abs() < 1e-5);
        }
    }

    /// Componer dos tramos debe dar lo mismo que aplicarlos en orden.
    #[test]
    fn composed_fog_equals_applying_in_order() {
        let far = Fog { scattered: Vec3::new(0.1, 0.2, 0.3), transmittance: Vec3::new(0.5, 0.6, 0.7) };
        let near = Fog { scattered: Vec3::new(0.05, 0.0, 0.4), transmittance: Vec3::new(0.9, 0.3, 0.8) };
        let color = Vec3::new(1.0, 2.0, 3.0);
        let a = near.apply(far.apply(color));
        let b = far.then(&near).apply(color);
        assert!((a - b).length() < 1e-6);
    }

    /// Costo de integrar un rayo dentro de la atmosfera, en un solo hilo.
    /// Correr con `cargo test --release -- --ignored --nocapture medir_`.
    #[test]
    #[ignore]
    fn medir_atmosfera() {
        let model = AtmosphereModel::new(170.0, params());
        let frame = AtmosphereFrame { model: &model, center: Vec3::new(0.0, -176.0, 0.0), sun_dir: Vec3::new(0.3, 0.5, 0.8).normalize() };
        let n = 400_000;
        let time = |label: &str, f: &dyn Fn(usize) -> f32| {
            let start = std::time::Instant::now();
            let mut acc = 0.0;
            for i in 0..n {
                acc += f(i);
            }
            std::hint::black_box(acc);
            println!("{label:<18} {:.1} ns", start.elapsed().as_secs_f64() * 1e9 / n as f64);
        };
        time("fast_exp", &|i| crate::math::fast_exp(-((i % 1000) as f32) * 0.01));
        time("exp", &|i| (-((i % 1000) as f32) * 0.01).exp());
        time("tabla", &|i| model.sun_depth((i % 20) as f32, ((i % 200) as f32) * 0.01 - 1.0));
        let dirs: Vec<Vec3> = (0..n)
            .map(|i| {
                let t = i as f32 * 0.0001;
                Vec3::new(t.sin(), 0.4 + 0.3 * (t * 3.0).cos(), t.cos()).normalize()
            })
            .collect();
        time("rayo sin pasos", &|i| frame.integrate(dirs[i], f32::INFINITY, Vec3::ONE, 1).scattered.x);
        for steps in [4, 6] {
            let start = std::time::Instant::now();
            let mut acc = Vec3::ZERO;
            for i in 0..n {
                let t = i as f32 * 0.0001;
                let dir = Vec3::new(t.sin(), 0.4 + 0.3 * (t * 3.0).cos(), t.cos()).normalize();
                acc += frame.integrate(dir, f32::INFINITY, Vec3::ONE, steps).scattered;
            }
            std::hint::black_box(acc);
            println!("atmosfera {steps} pasos: {:.1} ns por rayo", start.elapsed().as_secs_f64() * 1e9 / n as f64);
        }
    }

    #[test]
    fn fast_exp_is_close_to_exp() {
        for i in 0..2000 {
            let x = -(i as f32) * 0.01;
            let (a, b) = (crate::math::fast_exp(x), x.exp());
            assert!((a - b).abs() <= b * 3e-4 + 1e-9, "{x}: {a} vs {b}");
        }
    }
}
