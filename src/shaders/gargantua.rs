//! Gargantua: el agujero negro que deja la estrella y su disco de acrecion.
//!
//! # Lente
//! Cada pixel busca su rayo en la tabla de geodesicas (`math::geodesic`):
//! el plano del rayo lo forman la direccion al agujero y la del pixel, y en
//! ese plano la tabla dice si el rayo cae, en que direccion sale (el cielo
//! se muestrea ahi: las estrellas se curvan alrededor de la sombra) y el
//! radio de su camino en cada angulo.
//!
//! # Disco
//! El disco es delgado y vive en un plano por el agujero. El plano del rayo
//! lo corta en una recta (la linea de nodos): el foton cruza el disco cada
//! media vuelta alrededor del agujero, en los angulos `phi0 + k pi`. Cada
//! cruce con un radio dentro del disco suma su luz de adelante hacia atras:
//! el primero es el disco que se ve de frente; el segundo, el de atras
//! doblado por encima y por debajo de la sombra (la imagen de Interstellar);
//! los siguientes son los anillos de fotones, cada vez mas finos.
//!
//! La luz de cada cruce es la del Proyecto 2: temperatura de disco delgado,
//! corrimiento `g` por la gravedad y por el Doppler del gas que gira (un
//! lado mas azul y brillante que el otro, brillo `~ g^4`) y opacidad que
//! crece cuanto mas de lado lo cruza el rayo.
//!
//! # Gas
//! El gas gira mas rapido adentro que afuera (Kepler). Si la textura girara
//! sin mas, en un minuto quedaria hecha rayas finisimas. Se usan dos capas
//! de ruido que nacen y mueren desfasadas: cada una se deforma desde que
//! nace, y cuando una muere su peso ya es cero. Todo depende solo del
//! tiempo, asi que rebobinar o saltar en el ciclo da el mismo gas.

use std::f32::consts::PI;

use super::sky::corona;
use super::Uniforms;
use crate::math::blackbody::planckian_rgb;
use crate::math::geodesic::{weak_deflection, LensTable};
use crate::math::relativity::{disk_temperature, keplerian_omega, redshift_factor, DISK_INNER};
use crate::math::{fast_exp, smoothstep, Perlin, Vec3};

/// El gas gira tantas veces mas rapido que en tiempo de coordenadas (como en
/// el Proyecto 2), para que se vea fluir.
const DISK_TIME_SCALE: f32 = 8.0;
/// Vida de cada capa de gas en segundos.
const GAS_PERIOD: f32 = 8.0;
/// Brillo del disco: con `g = 1` y gas normal, la parte mas caliente brilla
/// como la superficie de la estrella.
const DISK_BRIGHTNESS: f32 = 4.0;
/// Tope de brillo por pixel (sin el, el lado que se acerca encandila todo el
/// bloom).
const HDR_CEILING: f32 = 40.0;

/// Colores de cuerpo negro precalculados en escala logaritmica.
pub struct PlanckTable {
    colors: Vec<Vec3>,
}

impl PlanckTable {
    const MIN: f32 = 800.0;
    const MAX: f32 = 40_000.0;
    const SIZE: usize = 256;

    pub fn new() -> Self {
        let colors = (0..Self::SIZE)
            .map(|i| {
                let x = i as f32 / (Self::SIZE - 1) as f32;
                planckian_rgb(Self::MIN * (Self::MAX / Self::MIN).powf(x))
            })
            .collect();
        Self { colors }
    }

    #[inline]
    pub fn get(&self, temperature: f32) -> Vec3 {
        let x = ((temperature.max(Self::MIN) / Self::MIN).ln() / (Self::MAX / Self::MIN).ln()).min(1.0);
        let f = x * (Self::SIZE - 1) as f32;
        let i = (f as usize).min(Self::SIZE - 2);
        self.colors[i].lerp(self.colors[i + 1], f - i as f32)
    }
}

impl Default for PlanckTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Gargantua en este frame.
pub struct BlackHole<'a> {
    /// Centro relativo a la camara, en unidades del mundo.
    pub center: Vec3,
    /// Radio de Schwarzschild en unidades del mundo.
    pub rs: f32,
    /// Eje del disco y dos ejes en su plano (`axis_a x axis_b = normal`).
    pub normal: Vec3,
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    pub table: &'a LensTable,
    pub planck: &'a PlanckTable,
    pub noise: &'a Perlin,
    pub time: f32,
    /// Disco: densidad (0 a 1), radio exterior en `rs` y temperatura maxima.
    pub density: f32,
    pub outer: f32,
    pub peak: f32,
    /// Coseno del mayor angulo con el agujero en que un rayo toca el disco
    /// o cae: fuera de ese cono las superficies no cambian.
    pub cone_cos: f32,
}

/// Lo que encuentra un rayo.
#[derive(Clone, Copy, Debug)]
pub struct Traced {
    pub captured: bool,
    /// Direccion en la que sale al cielo.
    pub sky_dir: Vec3,
    /// Luz del disco que cruza y cuanto deja pasar de lo de atras.
    pub light: Vec3,
    pub transmittance: f32,
    /// Distancia del cruce que ya tapa la mitad de lo de atras.
    pub depth: f32,
}

impl BlackHole<'_> {
    /// Sigue el rayo `dir` (unitario) desde la camara. Solo suma los cruces
    /// con el disco a menos de `limit` de la camara.
    pub fn trace(&self, dir: Vec3, limit: f32) -> Traced {
        let distance = self.center.length();
        let to_hole = self.center / distance;
        let cos_s = dir.dot(to_hole).clamp(-1.0, 1.0);
        let sin_s = dir.cross(to_hole).length();
        let s = sin_s.atan2(cos_s);
        let e1 = -to_hole;
        let e2 = if sin_s > 1e-6 {
            (dir - to_hole * cos_s) / sin_s
        } else {
            let helper = if to_hole.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
            helper.cross(to_hole).normalize()
        };
        let table = self.table;
        let mut traced = Traced {
            captured: false,
            sky_dir: dir,
            light: Vec3::ZERO,
            transmittance: 1.0,
            depth: f32::INFINITY,
        };
        let ray = if s > table.span() {
            // Lejos del agujero: deflexion de campo debil sobre la direccion
            // inicial en coordenadas.
            let start = sin_s.atan2(-cos_s * table.lapse);
            let exit = start + weak_deflection(table.observer, s);
            traced.sky_dir = e1 * exit.cos() + e2 * exit.sin();
            return traced;
        } else {
            table.lookup(s)
        };
        traced.captured = ray.path.captured;
        traced.sky_dir = e1 * ray.path.exit.cos() + e2 * ray.path.exit.sin();
        if self.density <= 0.0 || cos_s < self.cone_cos {
            return traced;
        }

        // Linea de nodos: donde el plano del rayo corta el del disco.
        let nodes = self.normal.cross(e1.cross(e2));
        let length = nodes.length();
        if length < 1e-5 {
            return traced;
        }
        let nodes = nodes / length;
        let phi0 = nodes.dot(e2).atan2(nodes.dot(e1)).rem_euclid(PI);
        // L_z / E del foton fisico (el rayo se traza hacia atras), con la
        // camara en unidades de rs: -(o x d) . n / lapso.
        let camera = -self.center / self.rs;
        let lz = -camera.cross(dir).dot(self.normal) / table.lapse;
        for k in 0..4 {
            let phi = phi0 + k as f32 * PI;
            if phi >= ray.path.end {
                break;
            }
            let Some(r) = table.radius_at(&ray, phi) else { continue };
            if r < DISK_INNER || r >= self.outer {
                continue;
            }
            let p = (e1 * phi.cos() + e2 * phi.sin()) * r;
            let hit_distance = (self.center + p * self.rs).length();
            if hit_distance >= limit {
                continue;
            }
            // Que tan de lado cruza: el coseno entre el camino y la normal.
            let step = 0.01;
            let slope = match (table.radius_at(&ray, phi + step), table.radius_at(&ray, phi - step)) {
                (Some(a), Some(b)) => (a - b) / (2.0 * step),
                _ => 0.0,
            };
            let tangential = e2 * phi.cos() - e1 * phi.sin();
            let mu = (r * self.normal.dot(tangential)).abs() / (r * r + slope * slope).sqrt();

            let azimuth = p.dot(self.axis_b).atan2(p.dot(self.axis_a));
            let gas = self.gas(r, azimuth);
            let g = redshift_factor(r, table.observer, lz);
            let temperature = g * disk_temperature(r, self.peak);
            if temperature <= 0.0 {
                continue;
            }
            // El disco caliente (con su borde interno oscuro) y una envoltura
            // gris mas grande que solo oscurece y brilla poco.
            let thermal = smoothstep(3.0, 3.3, r) * (1.0 - smoothstep(9.0, self.outer.max(9.5), r));
            let envelope = smoothstep(4.0, 7.0, r) * (1.0 - smoothstep(self.outer * 0.7, self.outer, r));
            let tau = self.density * (6.0 * thermal + 0.3 * envelope) / (mu.max(0.05) * g.max(0.05));
            let alpha = 1.0 - fast_exp(-tau * (0.4 + 0.6 * gas));
            let brightness = (temperature / 6500.0).powi(4) * DISK_BRIGHTNESS * self.density;
            let emission = self.planck.get(temperature) * (brightness * gas * (thermal + 0.12 * envelope));
            traced.light += emission * (alpha * traced.transmittance);
            let after = traced.transmittance * (1.0 - alpha);
            if traced.transmittance > 0.5 && after <= 0.5 {
                traced.depth = hit_distance;
            }
            traced.transmittance = after;
            if after < 0.004 {
                break;
            }
        }
        // Tope de brillo conservando el tono.
        let peak = traced.light.x.max(traced.light.y).max(traced.light.z);
        if peak > HDR_CEILING {
            traced.light = traced.light * (HDR_CEILING / peak);
        }
        traced
    }

    /// Densidad del gas en un punto del disco: dos capas de ruido que giran
    /// con la velocidad de Kepler de su radio y se relevan.
    fn gas(&self, r: f32, azimuth: f32) -> f32 {
        let omega = keplerian_omega(r) * DISK_TIME_SCALE;
        let mut sum = 0.0;
        for layer in 0..2 {
            let phase = self.time / GAS_PERIOD + layer as f32 * 0.5;
            let birth = phase.floor();
            let life = phase - birth;
            let weight = (PI * life).sin().powi(2);
            // Lo que hoy esta en `azimuth` estaba atras cuando la capa nacio.
            let angle = azimuth - omega * life * GAS_PERIOD;
            // Embebido en un circulo: sin costura al dar la vuelta.
            let p = Vec3::new(angle.cos() * 4.0, angle.sin() * 4.0, r.ln() * 5.0 + birth * 17.3);
            sum += weight * (self.noise.fbm(p, 3) * 0.5 + 0.5);
        }
        1.0 + 0.96 * (fast_exp((sum - 0.5) * 6.0) - 1.0)
    }

    /// Color de un pixel de cielo visto a traves del agujero. Las estrellas
    /// del fondo se ven en la direccion curvada; la corona de la estrella que
    /// colapsa esta junto al agujero, no detras, y no se curva.
    pub fn sky(&self, dir: Vec3, u: &Uniforms) -> (Vec3, f32) {
        let traced = self.trace(dir, f32::INFINITY);
        let background = if traced.captured { Vec3::ZERO } else { u.sky.sample(traced.sky_dir) };
        let glow = if u.star_glow > 0.0 { corona(dir, u) * u.star_glow } else { Vec3::ZERO };
        (traced.light + background * traced.transmittance + glow, traced.depth)
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
        let beyond = depth * cos_s > distance;
        let traced = self.trace(dir, if beyond { f32::INFINITY } else { depth });
        let background = if beyond && traced.captured { Vec3::ZERO } else { color };
        (traced.light + background * traced.transmittance, depth.min(traced.depth))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hole<'a>(table: &'a LensTable, planck: &'a PlanckTable, noise: &'a Perlin, center: Vec3) -> BlackHole<'a> {
        let normal = Vec3::Y;
        BlackHole {
            center,
            rs: 1.0,
            normal,
            axis_a: Vec3::Z,
            axis_b: Vec3::X,
            table,
            planck,
            noise,
            time: 3.0,
            density: 1.0,
            outer: 19.0,
            peak: 6500.0,
            cone_cos: table.disk_cone(19.0).cos(),
        }
    }

    #[test]
    fn looking_at_the_hole_is_dark_and_the_disk_shines_beside_it() {
        let (planck, noise) = (PlanckTable::new(), Perlin::new(1));
        // Camara a 40 rs, un poco por encima del plano del disco.
        let center = Vec3::new(0.0, -4.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, &planck, &noise, center);
        let at_hole = bh.trace(center.normalize(), f32::INFINITY);
        assert!(at_hole.captured);
        // Un rayo que apunta al disco a 8 rs del centro, del lado de enfrente.
        let target = Vec3::new(8.0, 0.0, 0.0) + center;
        let hit = bh.trace(target.normalize(), f32::INFINITY);
        assert!(!hit.captured);
        assert!(hit.light.x + hit.light.y + hit.light.z > 0.5, "{:?}", hit.light);
        assert!(hit.depth < center.length() + 10.0);
    }

    #[test]
    fn the_side_coming_toward_the_camera_is_bluer_and_brighter() {
        let (planck, noise) = (PlanckTable::new(), Perlin::new(1));
        let center = Vec3::new(0.0, -3.0, -60.0);
        let table = LensTable::build(center.length());
        let mut bh = hole(&table, &planck, &noise, center);
        bh.time = 0.0;
        // El gas gira a derechas alrededor de +Y: en +X va hacia -Z (hacia
        // la camara, que esta en -Z respecto al agujero... en +Z del hoyo).
        let left = bh.trace((center + Vec3::new(-7.0, 0.0, 0.0)).normalize(), f32::INFINITY);
        let right = bh.trace((center + Vec3::new(7.0, 0.0, 0.0)).normalize(), f32::INFINITY);
        let ratio = |c: Vec3| c.z / c.x.max(1e-6);
        let (bright, dim) = if left.light.x + left.light.y + left.light.z > right.light.x + right.light.y + right.light.z {
            (left, right)
        } else {
            (right, left)
        };
        assert!(ratio(bright.light) > ratio(dim.light), "el lado brillante debe ser el mas azul");
    }

    #[test]
    fn a_planet_in_front_of_the_hole_is_not_covered_by_the_disk_behind_it() {
        let (planck, noise) = (PlanckTable::new(), Perlin::new(1));
        let center = Vec3::new(0.0, -4.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, &planck, &noise, center);
        let dir = (center + Vec3::new(8.0, 0.0, 0.0)).normalize();
        let planet = Vec3::new(0.2, 0.5, 0.1);
        let (color, depth) = bh.surface(dir, planet, 10.0);
        assert_eq!(color, planet);
        assert_eq!(depth, 10.0);
        // La misma superficie detras del agujero queda tapada por el disco.
        let (behind, _) = bh.surface(dir, planet, 200.0);
        assert!(behind != planet);
    }
}
