//! Gargantúa: Variante predeterminada del Proyecto 2, con gas de volumen finito.
//! La tabla acelera el cielo lejano; en el disco se integra el mismo RK4 y
//! transferencia radiativa del Proyecto 2, incluyendo las nueve capas de gas.
use super::{Uniforms, sky::corona};
use crate::gargantua_p2::{
    self as p2,
    disk::{GasTexture, Medium},
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
    pub fn trace(&self, dir: Vec3, limit: f32) -> Traced {
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
        let mut traced = Traced {
            captured: false,
            sky_dir: dir,
            light: Vec3::ZERO,
            transmittance: 1.0,
            depth: f32::INFINITY,
        };
        let outside = self.density <= 0.0 || cos_s < self.cone_cos;
        if outside {
            if let Some((c, sn)) = self.table.far_turn(s) {
                traced.sky_dir = dir * c + (to_hole * sin_s - e2 * cos_s) * sn;
                return traced;
            }
        }
        if s > self.table.span() {
            let exit =
                sin_s.atan2(-cos_s * self.table.lapse) + weak_deflection(self.table.observer, s);
            traced.sky_dir = e1 * exit.cos() + e2 * exit.sin();
            return traced;
        }
        let ray = self.table.lookup(s);
        traced.captured = ray.path.captured;
        traced.sky_dir = e1 * ray.path.exit.cos() + e2 * ray.path.exit.sin();
        if outside || limit < (distance - self.outer * self.rs).max(0.0) {
            return traced;
        }
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
            if let Some(mut sample) = p2::disk::prepare(mid, step, frame, medium) {
                sample.optical_depth *= self.density;
                let (emission, alpha) = sample.shade(self.gas);
                traced.light +=
                    Vec3::new(emission.x, emission.y, emission.z) * (alpha * traced.transmittance);
                let after = traced.transmittance * (1.0 - alpha);
                if traced.transmittance > 0.5 && after <= 0.5 {
                    traced.depth = hit_distance;
                }
                traced.transmittance = after;
                if after < p2::config::MIN_TRANSMITTANCE {
                    break;
                }
            }
        }
        let peak = traced.light.max_element();
        if peak > 30.0 {
            traced.light = traced.light * (30.0 / peak);
        }
        // La sombra también da una profundidad a los volúmenes posteriores
        // (anillos y atmósferas): no pueden pintarse encima desde atrás.
        if traced.captured && cos_s > 0.0 {
            traced.depth = traced.depth.min(distance / cos_s);
        }
        traced
    }

    /// Color de un pixel de cielo visto a traves del agujero. Las estrellas
    /// del fondo se ven en la direccion curvada; la corona de la estrella que
    /// colapsa esta junto al agujero, no detras, y no se curva.
    pub fn sky(&self, dir: Vec3, u: &Uniforms) -> (Vec3, f32) {
        let traced = self.trace(dir, f32::INFINITY);
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
    pub fn surface(&self, dir: Vec3, color: Vec3, depth: f32) -> (Vec3, f32) {
        let distance = self.center.length();
        let cos_s = dir.dot(self.center) / distance;
        if cos_s < self.cone_cos {
            return (color, depth);
        }
        // Solo cuentan los cruces del disco que estan antes de la superficie;
        // si ademas la superficie esta detras del agujero y el rayo cae, la
        // sombra la oculta.
        let beyond = depth * cos_s > distance;
        let traced = self.trace(dir, depth);
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
