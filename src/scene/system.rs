//! El sistema de OCASO: la estrella moribunda y sus planetas.
//!
//! Todo movimiento es una funcion del tiempo de simulacion `t`: la posicion
//! en la orbita, el giro sobre el eje y el latido de la estrella. Nada se
//! acumula frame a frame, asi que el movimiento no depende de los fps, y
//! rebobinar el tiempo sera evaluar las mismas funciones con un `t` menor.

use std::f64::consts::TAU;
use std::path::Path;
use std::sync::Arc;

use super::{icosphere, load_obj, Mesh};
use crate::config;
use crate::math::blackbody::planckian_rgb;
use crate::math::{DQuat, DVec3, Perlin, Vec3};
use crate::shaders::{heartbeat, Material, ShipStyle, SkyCube, SunStyle};
use crate::terrain::shapes::RequiemField;
use crate::terrain::{HeightField, Terrain};

pub struct Body {
    pub name: &'static str,
    pub radius: f64,
    /// Amplitud del latido del radio (fraccion del radio); 0 no late.
    pub pulse: f64,
    /// Radio de la orbita circular; 0 deja el cuerpo fijo en el origen.
    pub orbit_radius: f64,
    /// Segundos por vuelta alrededor de la estrella.
    pub orbit_period: f64,
    /// Angulo en la orbita cuando `t = 0`.
    pub orbit_phase: f64,
    /// Inclinacion del plano de la orbita.
    pub orbit_inclination: f64,
    /// Segundos por vuelta sobre su propio eje.
    pub spin_period: f64,
    pub axial_tilt: f64,
    /// Parametro gravitacional `G * M`.
    pub mu: f64,
    /// Radio de la region donde este cuerpo es el marco de referencia de la nave.
    pub soi: f64,
    /// Malla para los cuerpos sin terreno (la estrella).
    pub mesh: usize,
    pub material: Material,
    /// Forma del terreno; `None` es una esfera lisa del radio base.
    pub surface: Option<Arc<dyn HeightField>>,
}

impl Body {
    pub fn position(&self, t: f64) -> DVec3 {
        if self.orbit_radius == 0.0 {
            return DVec3::ZERO;
        }
        let angle = self.orbit_phase + TAU * t / self.orbit_period;
        let (s, c) = angle.sin_cos();
        self.incline(DVec3::new(c, 0.0, -s) * self.orbit_radius)
    }

    /// Derivada exacta de `position`.
    pub fn velocity(&self, t: f64) -> DVec3 {
        if self.orbit_radius == 0.0 {
            return DVec3::ZERO;
        }
        let rate = TAU / self.orbit_period;
        let angle = self.orbit_phase + rate * t;
        let (s, c) = angle.sin_cos();
        self.incline(DVec3::new(-s, 0.0, -c) * (self.orbit_radius * rate))
    }

    fn incline(&self, v: DVec3) -> DVec3 {
        let (s, c) = self.orbit_inclination.sin_cos();
        DVec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c)
    }

    /// Inclinacion del eje y giro sobre el.
    pub fn rotation(&self, t: f64) -> DQuat {
        let spin = if self.spin_period == 0.0 {
            0.0
        } else {
            TAU * t / self.spin_period
        };
        DQuat::from_axis_angle(DVec3::Z, self.axial_tilt) * DQuat::from_axis_angle(DVec3::Y, spin)
    }

    /// Radio que se dibuja: la estrella late.
    pub fn render_radius(&self, t: f64) -> f64 {
        self.radius * (1.0 + self.pulse * heartbeat(t, config::HEARTBEAT_PERIOD))
    }

    /// Altura del terreno en la direccion `offset` (relativa al centro, en
    /// ejes del mundo) en el instante `t`. El terreno gira con el planeta,
    /// asi que la direccion se lleva primero al espacio del planeta.
    pub fn surface_height(&self, offset: DVec3, t: f64) -> f64 {
        match &self.surface {
            None => 0.0,
            Some(field) => {
                let local = self.rotation(t).conjugate().rotate(offset);
                field.sample(local.normalize().as_vec3()).height as f64
            }
        }
    }
}

pub struct System {
    pub bodies: Vec<Body>,
    /// Terreno de cada cuerpo, en el mismo orden que `bodies`.
    pub terrains: Vec<Option<Terrain>>,
    pub meshes: Vec<Mesh>,
    pub ship_mesh: usize,
    /// Giro que alinea el modelo de la nave con -Z (adelante) y +Y (arriba).
    pub ship_fix: DQuat,
    pub ship_material: Material,
    pub noise: Perlin,
    pub sky: SkyCube,
}

impl System {
    pub fn load(assets: &Path) -> Result<Self, String> {
        let mut ship = load_obj(&assets.join("naveGraficas.obj"))?;
        ship.normalize_size(config::SHIP_LENGTH as f32);
        let meshes = vec![icosphere(5), ship];

        let star = Body {
            name: "Ocaso",
            radius: 320.0,
            pulse: 0.012,
            orbit_radius: 0.0,
            orbit_period: 1.0,
            orbit_phase: 0.0,
            orbit_inclination: 0.0,
            spin_period: 600.0,
            axial_tilt: 0.12,
            mu: 1.5e6,
            soi: f64::INFINITY,
            mesh: 0,
            material: Material::Sun(SunStyle {
                temperature: 3400.0,
                intensity: 3.0,
            }),
            surface: None,
        };
        let requiem_radius = 160.0;
        let requiem_field: Arc<dyn HeightField> = Arc::new(RequiemField::new(config::NOISE_SEED ^ 0x0E9));
        let requiem = Body {
            name: "Requiem",
            radius: requiem_radius,
            pulse: 0.0,
            orbit_radius: 3200.0,
            orbit_period: 1500.0,
            orbit_phase: 0.9,
            orbit_inclination: 0.04,
            spin_period: 240.0,
            axial_tilt: 0.4,
            // Gravedad de 4 u/s^2 en la superficie: g = mu / r^2.
            mu: 4.0 * requiem_radius * requiem_radius,
            soi: requiem_radius * 6.0,
            mesh: 0,
            material: Material::Requiem,
            surface: Some(requiem_field.clone()),
        };
        let terrains = vec![
            None,
            Some(Terrain::new(requiem_radius, requiem_field, config::TERRAIN_FINEST)),
        ];

        Ok(Self {
            bodies: vec![star, requiem],
            terrains,
            meshes,
            ship_mesh: 1,
            // El modelo de Blender tiene la punta hacia +Z; adelante es -Z.
            ship_fix: DQuat::from_axis_angle(DVec3::Y, std::f64::consts::PI),
            ship_material: Material::Ship(ShipStyle {
                specular: 0.5,
                shininess: 40.0,
            }),
            noise: Perlin::new(config::NOISE_SEED),
            sky: SkyCube::bake(config::SKY_SIZE, config::NOISE_SEED),
        })
    }

    /// Luz de la estrella que llega a los planetas: su color de cuerpo negro
    /// por una intensidad que sube un poco con cada latido.
    pub fn star_light(&self, t: f64) -> Vec3 {
        let Material::Sun(style) = self.bodies[0].material else {
            return Vec3::ONE;
        };
        let beat = heartbeat(t, config::HEARTBEAT_PERIOD) as f32;
        planckian_rgb(style.temperature) * (1.5 * (1.0 + 0.06 * beat))
    }
}

/// El cuerpo cuya superficie esta mas cerca de `p`, y la altitud sobre ella
/// (contando el relieve del terreno).
pub fn nearest_surface(bodies: &[Body], p: DVec3, t: f64) -> (usize, f64) {
    bodies
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let offset = p - b.position(t);
            (i, offset.length() - b.radius - b.surface_height(offset, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, f64::INFINITY))
}
