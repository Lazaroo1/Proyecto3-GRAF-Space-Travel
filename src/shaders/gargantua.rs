//! Gargantúa: Variante predeterminada del Proyecto 2, con gas de volumen finito.
//! La tabla acelera el cielo lejano; en el disco se integra el mismo RK4 y
//! transferencia radiativa del Proyecto 2, incluyendo las nueve capas de gas.
use super::{Uniforms, sky::corona};
use crate::gargantua_p2::{
    self as p2,
    disk::{GasTexture, Medium, RadialTable},
    math::Vec3 as P2Vec,
    photon::Photon,
    relativity::RayFrame,
};
use crate::math::geodesic::{LensTable, weak_deflection};
use crate::math::{Vec3, fast_atan2};

pub struct BlackHole<'a> {
    pub center: Vec3,
    pub rs: f32,
    pub normal: Vec3,
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    pub table: &'a LensTable,
    pub gas: &'a GasTexture,
    pub density: f32,
    pub outer: f32,
    pub peak: f32,
    pub cone_cos: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Traced {
    pub captured: bool,
    pub sky_dir: Vec3,
    pub light: Vec3,
    pub transmittance: f32,
    pub depth: f32,
}
impl BlackHole<'_> {
    fn local(&self, v: Vec3) -> P2Vec {
        P2Vec::new(v.dot(self.axis_a), v.dot(self.normal), -v.dot(self.axis_b))
    }
    /// El rayo completo, sin la rejilla: la referencia de las pruebas.
    #[cfg(test)]
    pub fn trace(&self, dir: Vec3, limit: f32) -> Traced {
        let lens = self.lens(dir);
        let gas = (lens.gas && limit >= self.gas_begins()).then(|| self.gas(dir, limit));
        self.finish(&lens, gas)
    }

    /// Lo barato de un rayo: si cae, hacia donde sale al cielo (de la tabla)
    /// y si puede cruzar gas.
    fn lens(&self, dir: Vec3) -> Lens {
        let distance = self.center.length();
        let to_hole = self.center / distance;
        let cos_s = dir.dot(to_hole).clamp(-1.0, 1.0);
        let sin_s = dir.cross(to_hole).length();
        let s = fast_atan2(sin_s, cos_s);
        let e1 = -to_hole;
        let e2 = if sin_s > 1e-6 {
            (dir - to_hole * cos_s) / sin_s
        } else {
            let h = if to_hole.y.abs() < 0.9 {
                Vec3::Y
            } else {
                Vec3::X
            };
            h.cross(to_hole).normalize()
        };
        let mut lens = Lens {
            captured: false,
            sky_dir: dir,
            cos_s,
            gas: false,
        };
        let outside = self.density <= 0.0 || cos_s < self.cone_cos;
        if outside {
            if let Some((c, sn)) = self.table.far_turn(s) {
                lens.sky_dir = dir * c + (to_hole * sin_s - e2 * cos_s) * sn;
                return lens;
            }
        }
        if s > self.table.span() {
            let exit =
                sin_s.atan2(-cos_s * self.table.lapse) + weak_deflection(self.table.observer, s);
            lens.sky_dir = e1 * exit.cos() + e2 * exit.sin();
            return lens;
        }
        let ray = self.table.lookup(s);
        lens.captured = ray.path.captured;
        lens.sky_dir = e1 * ray.path.exit.cos() + e2 * ray.path.exit.sin();
        lens.gas = !outside;
        lens
    }

    /// Distancia a la camara desde la que puede haber gas.
    fn gas_begins(&self) -> f32 {
        (self.center.length() - self.outer * self.rs).max(0.0)
    }

    /// Distancia a la camara pasada la cual ya no hay gas: un limite mas
    /// lejano da lo mismo que ninguno.
    fn gas_ends(&self) -> f32 {
        self.center.length() + (self.outer * 1.01 + 1.0) * self.rs
    }

    /// Lo caro: el RK4 y la transferencia del Proyecto 2 a traves del gas,
    /// hasta la distancia `limit`.
    fn gas(&self, dir: Vec3, limit: f32) -> Gas {
        let mut gas = Gas::CLEAR;
        let origin = self.local(-self.center / self.rs);
        let mut photon = Photon::from_camera(origin, self.local(dir));
        let frame = RayFrame {
            observer_radius: origin.length(),
            lz_over_e: photon.lz_over_e,
        };
        let medium = Medium {
            peak_temperature: self.peak,
            ..Medium::VARIANT
        };
        let radial = RadialTable::shared();
        // El origen puede estar muy lejos del sistema: los pasos exteriores
        // son mayores sin saltarse la envoltura de 19 rs.
        for _ in 0..p2::config::MAX_STEPS {
            if photon.captured() || photon.escaped() {
                break;
            }
            let previous = photon.pos;
            let r = previous.length();
            // Saliendo, fuera de la esfera de fotones, el radio solo crece:
            // pasada la envoltura ya no queda gas que cruzar, y la direccion
            // final al cielo ya salio de la tabla.
            if r > self.outer + 1.0 && previous.dot(photon.vel) > 0.0 {
                break;
            }
            let step = if r > 40.0 && previous.dot(photon.vel) < 0.0 {
                photon
                    .step_length()
                    .max((r - 35.0) * 0.1 / photon.vel.length().max(0.1))
            } else {
                p2::volume_step(&photon, medium)
            };
            photon.advance(step);
            let mid = (previous + photon.pos) * 0.5;
            let hit_distance = (mid - origin).length() * self.rs;
            if hit_distance >= limit {
                break;
            }
            if mid.x.hypot(mid.z) >= self.outer {
                continue;
            }
            if let Some(mut sample) = p2::disk::prepare_variant(mid, step, frame, medium, radial) {
                sample.optical_depth *= self.density;
                let (emission, alpha) = sample.shade(self.gas);
                gas.light +=
                    Vec3::new(emission.x, emission.y, emission.z) * (alpha * gas.transmittance);
                let after = gas.transmittance * (1.0 - alpha);
                if gas.transmittance > 0.5 && after <= 0.5 {
                    gas.depth = hit_distance;
                }
                gas.transmittance = after;
                if after < p2::config::MIN_TRANSMITTANCE {
                    break;
                }
            }
        }
        let peak = gas.light.max_element();
        if peak > 30.0 {
            gas.light = gas.light * (30.0 / peak);
        }
        gas
    }

    /// Junta la lente y el gas (si se calculo) de un rayo.
    fn finish(&self, lens: &Lens, gas: Option<Gas>) -> Traced {
        let mut traced = Traced {
            captured: lens.captured,
            sky_dir: lens.sky_dir,
            light: Vec3::ZERO,
            transmittance: 1.0,
            depth: f32::INFINITY,
        };
        if let Some(gas) = gas {
            traced.light = gas.light;
            traced.transmittance = gas.transmittance;
            traced.depth = gas.depth;
            // La sombra también da una profundidad a los volúmenes posteriores
            // (anillos y atmósferas): no pueden pintarse encima desde atrás.
            if traced.captured && lens.cos_s > 0.0 {
                traced.depth = traced.depth.min(self.center.length() / lens.cos_s);
            }
        }
        traced
    }

    /// Color de un pixel de cielo visto a traves del agujero. Las estrellas
    /// del fondo se ven en la direccion curvada; la corona de la estrella que
    /// colapsa esta junto al agujero, no detras, y no se curva.
    #[cfg(test)]
    pub fn sky(&self, dir: Vec3, u: &Uniforms) -> (Vec3, f32) {
        self.sky_from(self.trace(dir, f32::INFINITY), dir, u)
    }

    fn sky_from(&self, traced: Traced, dir: Vec3, u: &Uniforms) -> (Vec3, f32) {
        let background = if traced.captured {
            Vec3::ZERO
        } else {
            u.sky.sample(traced.sky_dir)
        };
        let glow = if u.star_glow > 0.0 {
            corona(dir, u) * u.star_glow
        } else {
            Vec3::ZERO
        };
        (
            traced.light + background * traced.transmittance + glow,
            traced.depth,
        )
    }

    /// Una superficie ya sombreada (color y distancia) vista con el agujero:
    /// si esta delante, solo la tapa el disco que este mas cerca; si esta
    /// detras y el rayo cae, la sombra la oculta. Lo que esta detras no se
    /// curva (una aproximacion): solo el cielo se curva.
    #[cfg(test)]
    pub fn surface(&self, dir: Vec3, color: Vec3, depth: f32) -> (Vec3, f32) {
        self.surface_with(dir, color, depth, |limit| self.trace(dir, limit))
    }

    fn surface_with(
        &self,
        dir: Vec3,
        color: Vec3,
        depth: f32,
        trace: impl FnOnce(f32) -> Traced,
    ) -> (Vec3, f32) {
        let distance = self.center.length();
        let cos_s = dir.dot(self.center) / distance;
        if cos_s < self.cone_cos {
            return (color, depth);
        }
        // Solo cuentan los cruces del disco que estan antes de la superficie;
        // si ademas la superficie esta detras del agujero y el rayo cae, la
        // sombra la oculta.
        let beyond = depth * cos_s > distance;
        let traced = trace(depth);
        let background = if beyond && traced.captured {
            Vec3::ZERO
        } else {
            color
        };
        (
            traced.light + background * traced.transmittance,
            depth.min(traced.depth),
        )
    }
}

/// Lo que la tabla dice de un rayo, sin integrar nada.
struct Lens {
    captured: bool,
    sky_dir: Vec3,
    cos_s: f32,
    /// Si el rayo entra al cono del disco y hay que seguirlo por el gas.
    gas: bool,
}

/// Lo que el gas le hace a un rayo: luz que suma, cuanto deja pasar y a que
/// distancia ya tapa la mitad.
#[derive(Clone, Copy, Debug)]
struct Gas {
    light: Vec3,
    transmittance: f32,
    depth: f32,
}

impl Gas {
    const CLEAR: Self = Self {
        light: Vec3::ZERO,
        transmittance: 1.0,
        depth: f32::INFINITY,
    };
}

/// Pixeles entre dos muestras del gas.
const GAS_STRIDE: usize = 2;
/// Cuanto pueden diferir cuatro muestras vecinas para interpolarlas: en
/// transmitancia y en luz (relativa a la mas brillante).
const GAS_SPREAD_TRANSMITTANCE: f32 = 0.15;
const GAS_SPREAD_LIGHT: f32 = 0.25;

/// Una muestra de la rejilla: el gas de su rayo y si ese rayo cae.
#[derive(Clone, Copy)]
struct GasSample {
    gas: Gas,
    captured: bool,
}

/// El gas del disco calculado cada `GAS_STRIDE` pixeles. Su textura es mas
/// gruesa que un pixel, asi que entre cuatro muestras parecidas se
/// interpola; en los bordes (la sombra, la orilla del disco, el anillo de
/// fotones) las muestras no se parecen y el pixel se traza completo. Los
/// pixeles que caen justo en una muestra la usan tal cual.
#[derive(Default)]
pub struct GasField {
    cols: usize,
    rows: usize,
    samples: Vec<GasSample>,
}

impl GasField {
    /// Traza el gas en los puntos de la rejilla. `direction` da el rayo
    /// (sin normalizar) del centro de un pixel.
    pub fn build(
        &mut self,
        hole: &BlackHole,
        width: usize,
        height: usize,
        direction: impl Fn(f32, f32) -> Vec3 + Sync,
    ) {
        self.cols = (width - 1).div_ceil(GAS_STRIDE) + 1;
        self.rows = (height - 1).div_ceil(GAS_STRIDE) + 1;
        let empty = GasSample {
            gas: Gas::CLEAR,
            captured: false,
        };
        self.samples.resize(self.cols * self.rows, empty);
        let cols = self.cols;
        crate::parallel::chunks_mut(&mut self.samples, cols, 1, |row, out| {
            let y = (row * GAS_STRIDE) as f32 + 0.5;
            for (k, sample) in out.iter_mut().enumerate() {
                let dir = direction((k * GAS_STRIDE) as f32 + 0.5, y).normalize();
                let lens = hole.lens(dir);
                *sample = GasSample {
                    gas: if lens.gas {
                        hole.gas(dir, f32::INFINITY)
                    } else {
                        Gas::CLEAR
                    },
                    captured: lens.captured,
                };
            }
        });
    }

    /// Como `BlackHole::trace` para el pixel `(px, py)`, con el gas de la
    /// rejilla donde se puede.
    fn trace(&self, hole: &BlackHole, dir: Vec3, limit: f32, px: f32, py: f32) -> Traced {
        let lens = hole.lens(dir);
        if !lens.gas || limit < hole.gas_begins() {
            return hole.finish(&lens, None);
        }
        // Una superficie metida en el gas corta el rayo a su distancia.
        if limit < hole.gas_ends() {
            return hole.finish(&lens, Some(hole.gas(dir, limit)));
        }
        let gas = self
            .interpolate(px, py)
            .unwrap_or_else(|| hole.gas(dir, f32::INFINITY));
        hole.finish(&lens, Some(gas))
    }

    fn interpolate(&self, px: f32, py: f32) -> Option<Gas> {
        let u = (px - 0.5) / GAS_STRIDE as f32;
        let v = (py - 0.5) / GAS_STRIDE as f32;
        let (i, j) = (u as usize, v as usize);
        if i + 1 >= self.cols || j + 1 >= self.rows {
            return None;
        }
        let (fx, fy) = (u - i as f32, v - j as f32);
        let at = |i: usize, j: usize| &self.samples[j * self.cols + i];
        if fx == 0.0 && fy == 0.0 {
            return Some(at(i, j).gas);
        }
        let corners = [at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1)];
        let captured = corners[0].captured;
        let finite = corners[0].gas.depth.is_finite();
        let (mut t_min, mut t_max) = (f32::MAX, f32::MIN);
        let (mut l_min, mut l_max) = (f32::MAX, f32::MIN);
        for c in &corners {
            if c.captured != captured || c.gas.depth.is_finite() != finite {
                return None;
            }
            t_min = t_min.min(c.gas.transmittance);
            t_max = t_max.max(c.gas.transmittance);
            let l = c.gas.light.max_element();
            l_min = l_min.min(l);
            l_max = l_max.max(l);
        }
        if t_max - t_min > GAS_SPREAD_TRANSMITTANCE
            || l_max - l_min > GAS_SPREAD_LIGHT * l_max + 0.002
        {
            return None;
        }
        let w = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
        let mut gas = Gas {
            light: Vec3::ZERO,
            transmittance: 0.0,
            depth: if finite { 0.0 } else { f32::INFINITY },
        };
        for (c, w) in corners.iter().zip(w) {
            gas.light += c.gas.light * w;
            gas.transmittance += c.gas.transmittance * w;
            if finite {
                gas.depth += c.gas.depth * w;
            }
        }
        Some(gas)
    }

    /// `BlackHole::sky` con el gas de la rejilla.
    pub fn sky(&self, hole: &BlackHole, dir: Vec3, u: &Uniforms, px: f32, py: f32) -> (Vec3, f32) {
        hole.sky_from(self.trace(hole, dir, f32::INFINITY, px, py), dir, u)
    }

    /// `BlackHole::surface` con el gas de la rejilla.
    pub fn surface(
        &self,
        hole: &BlackHole,
        dir: Vec3,
        color: Vec3,
        depth: f32,
        px: f32,
        py: f32,
    ) -> (Vec3, f32) {
        hole.surface_with(dir, color, depth, |limit| {
            self.trace(hole, dir, limit, px, py)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_gas() -> &'static GasTexture {
        static GAS: std::sync::OnceLock<GasTexture> = std::sync::OnceLock::new();
        GAS.get_or_init(|| {
            let mut gas = GasTexture::new();
            gas.update_preview(
                &crate::gargantua_p2::math::noise::NoiseTable::new(
                    crate::gargantua_p2::config::NOISE_SEED,
                ),
                0.0,
            );
            gas
        })
    }

    fn hole<'a>(table: &'a LensTable, center: Vec3) -> BlackHole<'a> {
        let normal = Vec3::Y;
        BlackHole {
            center,
            rs: 1.0,
            normal,
            axis_a: Vec3::Z,
            axis_b: Vec3::X,
            table,
            gas: test_gas(),
            density: 1.0,
            outer: 19.0,
            peak: 6500.0,
            cone_cos: table.disk_cone(19.0).cos(),
        }
    }

    #[test]
    fn looking_at_the_hole_is_dark_and_the_disk_shines_beside_it() {
        // Camara a 40 rs, un poco por encima del plano del disco.
        let center = Vec3::new(0.0, -4.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let at_hole = bh.trace(center.normalize(), f32::INFINITY);
        assert!(at_hole.captured);
        // Un rayo que apunta al disco a 8 rs del centro, del lado de enfrente.
        let target = Vec3::new(8.0, 0.0, 0.0) + center;
        let hit = bh.trace(target.normalize(), f32::INFINITY);
        assert!(!hit.captured);
        assert!(
            hit.light.x + hit.light.y + hit.light.z > 0.005,
            "{:?}",
            hit.light
        );
        assert!(hit.depth < center.length() + 10.0);
    }

    #[test]
    fn the_side_coming_toward_the_camera_is_bluer_and_brighter() {
        let center = Vec3::new(0.0, -3.0, -60.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        // El gas gira a derechas alrededor de +Y: en +X va hacia -Z (hacia
        // la camara, que esta en -Z respecto al agujero... en +Z del hoyo).
        let left = bh.trace(
            (center + Vec3::new(-7.0, 0.0, 0.0)).normalize(),
            f32::INFINITY,
        );
        let right = bh.trace(
            (center + Vec3::new(7.0, 0.0, 0.0)).normalize(),
            f32::INFINITY,
        );
        let ratio = |c: Vec3| c.z / c.x.max(1e-6);
        let (bright, dim) = if left.light.x + left.light.y + left.light.z
            > right.light.x + right.light.y + right.light.z
        {
            (left, right)
        } else {
            (right, left)
        };
        assert!(
            ratio(bright.light) > ratio(dim.light),
            "el lado brillante debe ser el mas azul"
        );
    }

    #[test]
    fn a_planet_in_front_of_the_hole_is_not_covered_by_the_disk_behind_it() {
        let center = Vec3::new(0.0, -4.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let dir = (center + Vec3::new(8.0, 0.0, 0.0)).normalize();
        let planet = Vec3::new(0.2, 0.5, 0.1);
        let (color, depth) = bh.surface(dir, planet, 10.0);
        assert_eq!(color, planet);
        assert_eq!(depth, 10.0);
        // La misma superficie detras del agujero queda tapada por el disco.
        let (behind, _) = bh.surface(dir, planet, 200.0);
        assert!(behind != planet);
    }

    #[test]
    fn the_captured_sky_has_depth_to_occlude_distant_rings() {
        let center = Vec3::new(0.0, -2.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let ray = bh.trace(center.normalize(), f32::INFINITY);
        assert!(ray.captured);
        assert!(ray.depth.is_finite() && ray.depth <= center.length() + 0.1);
        assert!(ray.depth > 10.0);
    }

    #[test]
    fn the_gas_grid_matches_full_tracing() {
        // Una vista de 160x90 que encuadra el disco desde 40 rs.
        let center = Vec3::new(0.0, -2.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let (w, h) = (160usize, 90usize);
        let forward = center.normalize();
        let right = Vec3::Y.cross(forward).normalize() * 0.6;
        let up = forward.cross(right).normalize() * 0.6 * h as f32 / w as f32;
        let direction = |x: f32, y: f32| {
            forward + right * (x / w as f32 * 2.0 - 1.0) + up * (1.0 - y / h as f32 * 2.0)
        };
        let mut field = GasField::default();
        field.build(&bh, w, h, direction);
        let (mut error, mut light) = (0.0, 0.0);
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let dir = direction(px, py).normalize();
                let exact = bh.trace(dir, f32::INFINITY);
                let grid = field.trace(&bh, dir, f32::INFINITY, px, py);
                assert_eq!(exact.captured, grid.captured);
                if x % GAS_STRIDE == 0 && y % GAS_STRIDE == 0 {
                    // Sobre una muestra, la rejilla es el rayo mismo.
                    assert_eq!(exact.light, grid.light);
                    assert_eq!(exact.transmittance, grid.transmittance);
                }
                error += (exact.light - grid.light).abs().max_element();
                light += exact.light.max_element();
            }
        }
        assert!(light > 1.0, "la vista debe ver el disco");
        assert!(error / light < 0.03, "error relativo {}", error / light);
    }

    /// Costo de un pixel segun donde cae. Correr con
    /// `cargo test --release -- --ignored --nocapture medir_gargantua`.
    #[test]
    #[ignore]
    fn medir_gargantua() {
        let center = Vec3::new(0.0, -2.0, -29.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let axis = center.normalize();
        let (a, b) = {
            let helper = Vec3::Y;
            let a = helper.cross(axis).normalize();
            (a, axis.cross(a))
        };
        let n = 400_000;
        let time = |label: &str, spread: f32| {
            let dirs: Vec<Vec3> = (0..n)
                .map(|i| {
                    let angle = i as f32 * 2.399;
                    let radius = spread * ((i % 997) as f32 / 997.0 * 0.5 + 0.5);
                    (axis + (a * angle.cos() + b * angle.sin()) * radius).normalize()
                })
                .collect();
            let start = std::time::Instant::now();
            let mut acc = Vec3::ZERO;
            for d in &dirs {
                let t = bh.trace(*d, f32::INFINITY);
                acc += t.light + t.sky_dir;
            }
            std::hint::black_box(acc);
            println!(
                "{label:<22} {:6.1} ns por pixel",
                start.elapsed().as_secs_f64() * 1e9 / n as f64
            );
        };
        time("cerca (disco)", 0.4);
        time("cono sin disco", 0.75);
        time("tabla lejos", 1.6);
        time("campo debil", 6.0);
    }
}
