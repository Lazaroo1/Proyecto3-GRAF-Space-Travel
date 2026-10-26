//! El sistema de OCASO: la estrella moribunda y sus cinco planetas.
//!
//! Todo movimiento es una funcion del tiempo de simulacion `t`: la posicion
//! en la orbita, el giro sobre el eje y el latido de la estrella. Nada se
//! acumula frame a frame, asi que el movimiento no depende de los fps, y
//! rebobinar el tiempo sera evaluar las mismas funciones con un `t` menor.

use std::f64::consts::TAU;
use std::path::Path;
use std::sync::Arc;

use super::props::{self, PropModel, PropSet};
use super::{icosphere, load_obj, Mesh};
use crate::config;
use crate::math::blackbody::planckian_rgb;
use crate::math::{DQuat, DVec3, Perlin, Vec3};
use crate::shaders::{
    heartbeat, AtmosphereModel, AtmosphereParams, Material, ShipStyle, SkyCube, SunStyle, Textures,
};
use crate::terrain::shapes::{CementerioField, JardinField, PiraField, RequiemField};
use crate::terrain::{HeightField, Smooth, Terrain};

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
    /// Angulo de giro cuando `t = 0`.
    pub spin_phase: f64,
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
    /// Anillos: radios interior y exterior, en radios del planeta.
    pub rings: Option<(f64, f64)>,
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

    /// Inclinacion del eje y giro sobre el. La orbita avanza girando sobre
    /// +Y, igual que el giro: con el mismo periodo y la misma fase, el
    /// planeta le muestra siempre la misma cara (-X) a la estrella.
    pub fn rotation(&self, t: f64) -> DQuat {
        let spin = if self.spin_period == 0.0 {
            0.0
        } else {
            self.spin_phase + TAU * t / self.spin_period
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

/// Datos de un planeta para armarlo sin repetir la lista completa de campos.
struct PlanetSpec {
    name: &'static str,
    radius: f64,
    orbit: f64,
    period: f64,
    phase: f64,
    inclination: f64,
    spin: f64,
    tilt: f64,
    /// Gravedad en la superficie: `mu = g r^2`.
    gravity: f64,
    material: Material,
    surface: Arc<dyn HeightField>,
    atmosphere: Option<AtmosphereParams>,
}

impl PlanetSpec {
    fn build(self) -> (Body, Terrain, Option<AtmosphereModel>) {
        let terrain = Terrain::new(self.radius, self.surface.clone(), config::TERRAIN_FINEST);
        let atmosphere = self.atmosphere.map(|p| AtmosphereModel::new(self.radius as f32, p));
        let body = Body {
            name: self.name,
            radius: self.radius,
            pulse: 0.0,
            orbit_radius: self.orbit,
            orbit_period: self.period,
            orbit_phase: self.phase,
            orbit_inclination: self.inclination,
            spin_period: self.spin,
            spin_phase: 0.0,
            axial_tilt: self.tilt,
            mu: self.gravity * self.radius * self.radius,
            soi: self.radius * 6.0,
            mesh: 0,
            material: self.material,
            surface: Some(self.surface),
            rings: None,
        };
        (body, terrain, atmosphere)
    }
}

pub struct System {
    pub bodies: Vec<Body>,
    /// Terreno de cada cuerpo, en el mismo orden que `bodies`.
    pub terrains: Vec<Option<Terrain>>,
    /// Atmosfera de cada cuerpo, en el mismo orden que `bodies`.
    pub atmospheres: Vec<Option<AtmosphereModel>>,
    pub meshes: Vec<Mesh>,
    pub ship_mesh: usize,
    /// Giro que alinea el modelo de la nave con -Z (adelante) y +Y (arriba).
    pub ship_fix: DQuat,
    pub ship_material: Material,
    pub noise: Perlin,
    pub sky: SkyCube,
    pub textures: Textures,
    /// Modelos de los objetos de la superficie, en el orden de `PropKind::ALL`.
    pub prop_models: Vec<PropModel>,
    /// Objetos de cada cuerpo, en el mismo orden que `bodies`.
    pub props: Vec<PropSet>,
    /// Semilla con la que se colocaron los objetos.
    pub prop_seed: u32,
    /// Cuerpo con la tormenta-ojo (Vigia).
    pub eye_body: usize,
    /// Cuerpo con anillos (Cementerio).
    pub ring_body: usize,
}

impl System {
    pub fn load(assets: &Path) -> Result<Self, String> {
        let mut ship = load_obj(&assets.join("naveGraficas.obj"))?;
        ship.normalize_size(config::SHIP_LENGTH as f32);
        let meshes = vec![icosphere(5), ship];
        let seed = config::NOISE_SEED;

        let star = Body {
            name: "Ocaso",
            radius: 320.0,
            pulse: 0.012,
            orbit_radius: 0.0,
            orbit_period: 1.0,
            orbit_phase: 0.0,
            orbit_inclination: 0.0,
            spin_period: 600.0,
            spin_phase: 0.0,
            axial_tilt: 0.12,
            mu: 1.5e6,
            soi: f64::INFINITY,
            mesh: 0,
            material: Material::Sun(SunStyle {
                temperature: 3400.0,
                intensity: 3.0,
            }),
            surface: None,
            rings: None,
        };

        let (mut pira, pira_terrain, pira_air) = PlanetSpec {
            name: "Pira",
            radius: 110.0,
            orbit: 1150.0,
            period: 260.0,
            phase: 2.4,
            inclination: 0.0,
            spin: 260.0,
            tilt: 0.0,
            gravity: 3.5,
            material: Material::Pira,
            surface: Arc::new(PiraField::new(seed ^ 0x9124)),
            // Humo y vapor de roca: mucho Mie, Rayleigh que tira al rojo.
            atmosphere: Some(AtmosphereParams {
                thickness: 9.0,
                scale_height: 2.8,
                rayleigh: Vec3::new(0.010, 0.0045, 0.002),
                mie: 0.012,
                mie_g: 0.7,
                intensity: 8.0,
            }),
        }
        .build();
        // Acoplamiento de marea: gira con su orbita y le da siempre la misma
        // cara a la estrella.
        pira.spin_phase = pira.orbit_phase;

        let (jardin, jardin_terrain, jardin_air) = PlanetSpec {
            name: "Jardin",
            radius: 170.0,
            orbit: 2300.0,
            period: 700.0,
            phase: 0.6,
            inclination: 0.03,
            spin: 150.0,
            tilt: 0.3,
            gravity: 4.0,
            material: Material::Jardin,
            surface: Arc::new(JardinField::new(seed ^ 0x7A3D, seed)),
            // Densa: dispersa mucho mas el verde y el azul. Con la luz roja de la
            // estrella el cielo de dia sale turquesa y los atardeceres rosas.
            atmosphere: Some(AtmosphereParams {
                thickness: 20.0,
                scale_height: 5.0,
                rayleigh: Vec3::new(0.002, 0.016, 0.042),
                mie: 0.0015,
                mie_g: 0.76,
                intensity: 8.0,
            }),
        }
        .build();

        let (mut vigia, vigia_terrain, vigia_air) = PlanetSpec {
            name: "Vigia",
            radius: 430.0,
            orbit: 3900.0,
            period: 1300.0,
            phase: 4.0,
            inclination: -0.04,
            spin: 90.0,
            tilt: 0.15,
            gravity: 9.0,
            material: Material::Vigia,
            surface: Arc::new(Smooth),
            // Bruma alta magenta sobre las nubes.
            atmosphere: Some(AtmosphereParams {
                thickness: 40.0,
                scale_height: 12.0,
                rayleigh: Vec3::new(0.012, 0.004, 0.016),
                mie: 0.002,
                mie_g: 0.6,
                intensity: 9.0,
            }),
        }
        .build();
        vigia.soi = vigia.radius * 4.0;

        let (requiem, requiem_terrain, requiem_air) = PlanetSpec {
            name: "Requiem",
            radius: 160.0,
            orbit: 5400.0,
            period: 2000.0,
            phase: 1.3,
            inclination: 0.05,
            spin: 240.0,
            tilt: 0.4,
            gravity: 4.0,
            material: Material::Requiem,
            surface: Arc::new(RequiemField::new(seed ^ 0x0E9)),
            // Tenue y violeta.
            atmosphere: Some(AtmosphereParams {
                thickness: 10.0,
                scale_height: 3.0,
                rayleigh: Vec3::new(0.006, 0.004, 0.012),
                mie: 0.002,
                mie_g: 0.7,
                intensity: 7.0,
            }),
        }
        .build();

        let (mut cementerio, cementerio_terrain, _) = PlanetSpec {
            name: "Cementerio",
            radius: 140.0,
            orbit: 6800.0,
            period: 2600.0,
            phase: 3.1,
            inclination: -0.06,
            spin: 170.0,
            tilt: 0.55,
            gravity: 3.5,
            material: Material::Cementerio,
            surface: Arc::new(CementerioField::new(seed ^ 0xCE3)),
            // Sin aire: la roca muerta se ve con sombras duras.
            atmosphere: None,
        }
        .build();
        cementerio.rings = Some((1.7, 3.1));

        let mut system = Self {
            bodies: vec![star, pira, jardin, vigia, requiem, cementerio],
            terrains: vec![
                None,
                Some(pira_terrain),
                Some(jardin_terrain),
                Some(vigia_terrain),
                Some(requiem_terrain),
                Some(cementerio_terrain),
            ],
            atmospheres: vec![None, pira_air, jardin_air, vigia_air, requiem_air, None],
            meshes,
            ship_mesh: 1,
            // El modelo de Blender tiene la punta hacia +Z; adelante es -Z.
            ship_fix: DQuat::from_axis_angle(DVec3::Y, std::f64::consts::PI),
            ship_material: Material::Ship(ShipStyle {
                specular: 0.5,
                shininess: 40.0,
            }),
            noise: Perlin::new(seed),
            sky: SkyCube::bake(config::SKY_SIZE, seed),
            textures: Textures::generate(),
            prop_models: props::build_models(),
            props: Vec::new(),
            prop_seed: config::PROP_SEED,
            eye_body: 3,
            ring_body: 5,
        };
        system.scatter_props(config::PROP_SEED);
        Ok(system)
    }

    /// Coloca (o vuelve a colocar) los objetos de todos los planetas con
    /// `seed`. Cada planeta mezcla la semilla con su indice para no repetir
    /// el mismo patron.
    pub fn scatter_props(&mut self, seed: u32) {
        self.prop_seed = seed;
        self.props = (0..self.bodies.len()).map(|index| self.scatter_body(index, seed)).collect();
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

    /// Indice de un cuerpo por nombre.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.bodies.iter().position(|b| b.name == name)
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
