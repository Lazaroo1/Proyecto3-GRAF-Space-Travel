//! El sistema de OCASO: la estrella moribunda, sus cinco planetas y sus
//! lunas.
//!
//! Todo movimiento es una funcion del tiempo de simulacion `t`: la posicion
//! en la orbita, el giro sobre el eje y el latido de la estrella. Nada se
//! acumula frame a frame, asi que el movimiento no depende de los fps, y
//! rebobinar el tiempo sera evaluar las mismas funciones con un `t` menor.

use std::f64::consts::TAU;
use std::path::Path;
use std::sync::Arc;

use super::effects::Exhaust;
use crate::chapin::{self, Chapin};
use super::props::{self, PropModel, PropSet};
use super::{Mesh, icosphere, load_obj};
use crate::config;
use crate::math::{DQuat, DVec3, Perlin, Vec3};
use crate::shaders::{
    ArgosLook, AtmosphereModel, AtmosphereParams, Material, RingStyle, ShipStyle, SkyCube,
    SkyStyle, SunStyle, Textures, heartbeat,
};
use crate::terrain::shapes::{
    ArgosField, CementerioField, JardinField, MIRROR_SHORE, ParpadoField, PiraField,
};
use crate::terrain::{HeightField, Smooth, Terrain};

pub struct Body {
    /// Nombre interno: el codigo busca los cuerpos por el (no cambia con la
    /// version).
    pub name: &'static str,
    /// Nombre en el HUD, el de la version que se esta viendo.
    pub label: &'static str,
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
    /// Anillos, si tiene.
    pub rings: Option<Rings>,
    /// Ya no existe (Gargantua se la comio, o no es de esta version): no se
    /// dibuja ni se choca con ella.
    pub hidden: bool,
    /// No existe en la version que se esta viendo (la Luna de Xelaju solo
    /// esta en la chapina).
    pub absent: bool,
    /// Si es una luna, el planeta alrededor del que gira: su orbita (la de
    /// arriba) es alrededor de el.
    pub host: Option<Host>,
}

/// Anillos de un cuerpo: radios interior y exterior (en radios del cuerpo)
/// y de que estan hechos.
#[derive(Clone, Copy, Debug)]
pub struct Rings {
    pub inner: f64,
    pub outer: f64,
    pub style: RingStyle,
}

/// El planeta de una luna: su indice y una copia de su orbita, para que la
/// posicion de la luna siga siendo funcion solo del tiempo. Si la orbita del
/// planeta cambia, `System::set_orbit_period` actualiza la copia.
#[derive(Clone, Copy, Debug)]
pub struct Host {
    pub index: usize,
    pub radius: f64,
    pub period: f64,
    pub phase: f64,
    pub inclination: f64,
}

impl Host {
    pub fn of(index: usize, body: &Body) -> Self {
        Self {
            index,
            radius: body.orbit_radius,
            period: body.orbit_period,
            phase: body.orbit_phase,
            inclination: body.orbit_inclination,
        }
    }
}

/// Punto de una orbita circular inclinada en el instante `t` (cero si el
/// radio es cero: el cuerpo esta fijo).
fn orbit_point(radius: f64, period: f64, phase: f64, inclination: f64, t: f64) -> DVec3 {
    if radius == 0.0 {
        return DVec3::ZERO;
    }
    let (s, c) = (phase + TAU * t / period).sin_cos();
    incline(DVec3::new(c, 0.0, -s) * radius, inclination)
}

/// Derivada exacta de `orbit_point`.
fn orbit_velocity(radius: f64, period: f64, phase: f64, inclination: f64, t: f64) -> DVec3 {
    if radius == 0.0 {
        return DVec3::ZERO;
    }
    let rate = TAU / period;
    let (s, c) = (phase + rate * t).sin_cos();
    incline(DVec3::new(-s, 0.0, -c) * (radius * rate), inclination)
}

fn incline(v: DVec3, inclination: f64) -> DVec3 {
    let (s, c) = inclination.sin_cos();
    DVec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c)
}

impl Body {
    pub fn position(&self, t: f64) -> DVec3 {
        let around = self.host.map_or(DVec3::ZERO, |h| {
            orbit_point(h.radius, h.period, h.phase, h.inclination, t)
        });
        around
            + orbit_point(
                self.orbit_radius,
                self.orbit_period,
                self.orbit_phase,
                self.orbit_inclination,
                t,
            )
    }

    /// Derivada exacta de `position`.
    pub fn velocity(&self, t: f64) -> DVec3 {
        let around = self.host.map_or(DVec3::ZERO, |h| {
            orbit_velocity(h.radius, h.period, h.phase, h.inclination, t)
        });
        around
            + orbit_velocity(
                self.orbit_radius,
                self.orbit_period,
                self.orbit_phase,
                self.orbit_inclination,
                t,
            )
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
        let atmosphere = self
            .atmosphere
            .map(|p| AtmosphereModel::new(self.radius as f32, p));
        let body = Body {
            name: self.name,
            label: self.name,
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
            hidden: false,
            absent: false,
            host: None,
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
    pub ship_mesh: [usize; 3],
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
    /// Radio de diseno de la estrella: el ciclo lo hincha y lo colapsa, y
    /// deja en `bodies[0].radius` el radio de cada momento.
    pub star_base: f64,
    /// Cuerpo con la tormenta-ojo (Vigia).
    pub eye_body: usize,
    /// Un lugar para visitar en cada planeta con superficie.
    pub sites: Vec<Site>,
    /// La version que se esta viendo; `other` guarda lo de la otra (V las
    /// intercambia).
    pub version: Version,
    other: Vec<Look>,
    other_sites: Vec<Site>,
    /// El cielo de la otra version.
    other_sky: SkyCube,
    /// Semilla con la que se colocaron los objetos de la otra version.
    other_seed: u32,
    /// Las naves botella de la version chapina.
    pub bottles: Vec<Bottle>,
}

/// La version del sistema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Ocaso,
    /// Guatemala: los mismos cuerpos y orbitas con otra piel (`chapin/`).
    Chapin,
}

/// Una nave botella: su nombre, su malla y lo que echa por detras.
#[derive(Clone, Copy, Debug)]
pub struct Bottle {
    pub name: &'static str,
    pub mesh: usize,
    pub exhaust: Exhaust,
}

/// Las naves botella: nombre, archivo en `assets/botellas` y su escape.
const BOTTLES: [(&str, &str, Exhaust); 6] = [
    ("Gallo", "06-gallo", Exhaust::Foam),
    ("Cabro Reserva", "01-cabro-reserva", Exhaust::Foam),
    (
        "Jagermeister",
        "02-jagermeister",
        Exhaust::Flame(Vec3::new(0.35, 1.2, 0.25), Vec3::new(0.05, 0.4, 0.08)),
    ),
    (
        "Blue Label",
        "03-johnnie-walker-blue",
        Exhaust::Flame(Vec3::new(0.2, 0.55, 1.4), Vec3::new(0.05, 0.12, 0.6)),
    ),
    (
        "Cubata Uva",
        "04-cubata-uva",
        Exhaust::Flame(Vec3::new(0.9, 0.3, 1.4), Vec3::new(0.3, 0.05, 0.55)),
    ),
    ("Four Loko", "05-four-loko-white", Exhaust::Sparks),
];

/// Un numero por nombre, para mezclar la semilla de cada cuerpo.
fn hash_name(name: &str) -> u32 {
    name.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// Lo que cambia de un cuerpo entre versiones. La version que no se esta
/// viendo vive aqui; `System::toggle_version` lo intercambia con lo del
/// cuerpo.
struct Look {
    label: &'static str,
    material: Material,
    surface: Option<Arc<dyn HeightField>>,
    rings: Option<Rings>,
    absent: bool,
    terrain: Option<Terrain>,
    atmosphere: Option<AtmosphereModel>,
    props: PropSet,
}

/// Un lugar para visitar en la superficie de un planeta, en el espacio del
/// planeta: su vertical y hacia donde mirar al llegar.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub body: usize,
    pub name: &'static str,
    pub normal: DVec3,
    pub forward: DVec3,
    /// Como llegar a verlo: altura sobre el suelo, distancia antes del
    /// lugar y cuanto mas arriba mirar.
    pub view: (f64, f64, f64),
}

impl Site {
    /// Vertical, frente y derecha del lugar (en el espacio del planeta).
    pub fn frame(&self) -> (DVec3, DVec3, DVec3) {
        (self.normal, self.forward, self.forward.cross(self.normal))
    }
}

impl System {
    pub fn load(assets: &Path) -> Result<Self, String> {
        let mut ship = load_obj(&assets.join("OCASO-1.obj"))?;
        ship.normalize_size(config::SHIP_LENGTH as f32);
        let mut medium = load_obj(&assets.join("OCASO-1-medium.obj"))?;
        let mut low = load_obj(&assets.join("OCASO-1-low.obj"))?;
        medium.normalize_size(config::SHIP_LENGTH as f32);
        low.normalize_size(config::SHIP_LENGTH as f32);
        let mut meshes = vec![icosphere(5), ship, medium, low];
        // Las naves botella: en los OBJ la botella esta parada sobre +Z con la
        // etiqueta hacia +Y. Vuelan con el pico adelante (-Z) y la etiqueta
        // arriba (+Y): media vuelta alrededor de Y.
        let mut bottles = Vec::new();
        for &(name, file, exhaust) in &BOTTLES {
            let mut mesh = load_obj(&assets.join("botellas").join(format!("{file}-low.obj")))?;
            mesh.deform(|p| Vec3::new(-p.x, p.y, -p.z));
            mesh.normalize_size(config::SHIP_LENGTH as f32 * 1.15);
            bottles.push(Bottle {
                name,
                mesh: meshes.len(),
                exhaust,
            });
            meshes.push(mesh);
        }
        let seed = config::NOISE_SEED;

        let star = Body {
            name: "Ocaso",
            label: "Ocaso",
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
                glyph: false,
            }),
            surface: None,
            rings: None,
            hidden: false,
            absent: false,
            host: None,
        };

        let (mut pira, pira_terrain, pira_air) = PlanetSpec {
            name: "Pira",
            radius: 110.0,
            orbit: 800.0,
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
                intensity: 2.2,
            }),
        }
        .build();
        // Acoplamiento de marea: gira con su orbita y le da siempre la misma
        // cara a la estrella.
        pira.spin_phase = pira.orbit_phase;

        let (jardin, jardin_terrain, jardin_air) = PlanetSpec {
            name: "Jardin",
            radius: 400.0,
            orbit: 1500.0,
            period: 700.0,
            phase: 0.6,
            inclination: 0.03,
            spin: 150.0,
            tilt: 0.3,
            gravity: 4.0,
            material: Material::Jardin,
            surface: Arc::new(JardinField::new(seed ^ 0x7A3D, seed)),
            // Bruma fina: conserva el violeta de Vigía visto desde el suelo.
            atmosphere: Some(AtmosphereParams {
                thickness: 20.0,
                scale_height: 5.0,
                rayleigh: Vec3::new(0.001, 0.004, 0.007),
                mie: 0.0015,
                mie_g: 0.76,
                intensity: 2.2,
            }),
        }
        .build();

        let (mut vigia, vigia_terrain, vigia_air) = PlanetSpec {
            name: "Vigia",
            radius: 540.0,
            orbit: 3100.0,
            period: 1300.0,
            phase: 0.765,
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
                intensity: 0.25,
            }),
        }
        .build();
        vigia.soi = vigia.radius * 4.0;

        // Argos: carne con un ojo que sigue a la nave por todo el sistema. Su
        // material lleva a donde mira el ojo; la app lo cambia cada frame.
        let (argos, argos_terrain, argos_air) = PlanetSpec {
            name: "Argos",
            radius: 200.0,
            orbit: 4000.0,
            period: 2000.0,
            phase: 1.3,
            inclination: 0.05,
            spin: 240.0,
            tilt: 0.4,
            gravity: 4.0,
            material: Material::Argos(ArgosLook {
                radius: 200.0,
                ..ArgosLook::default()
            }),
            surface: Arc::new(ArgosField::new(seed ^ 0x0E9)),
            // Una bruma humeda y rojiza, apenas: mas, y deslava la carne.
            atmosphere: Some(AtmosphereParams {
                thickness: 6.0,
                scale_height: 2.0,
                rayleigh: Vec3::new(0.012, 0.0025, 0.008),
                mie: 0.003,
                mie_g: 0.7,
                intensity: 0.5,
            }),
        }
        .build();

        let (mut cementerio, cementerio_terrain, _) = PlanetSpec {
            name: "Cementerio",
            radius: 140.0,
            orbit: 5200.0,
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
        cementerio.rings = Some(Rings {
            inner: 1.7,
            outer: 3.1,
            style: RingStyle::Wrecks,
        });
        // Anillos delgados de polvo, por dentro de la orbita de su luna.
        vigia.rings = Some(Rings {
            inner: 1.45,
            outer: 2.15,
            style: RingStyle::Dust,
        });

        // Parpado, la luna de Vigia: cristales y lagos. Le da siempre la
        // misma cara (-X) a Vigia, asi que desde sus lagos el ojo esta
        // siempre en el mismo lugar del cielo.
        let (mut parpado, parpado_terrain, parpado_air) = PlanetSpec {
            name: "Parpado",
            radius: 200.0,
            orbit: 1700.0,
            period: 520.0,
            phase: 2.2,
            inclination: 0.0,
            spin: 520.0,
            tilt: 0.0,
            gravity: 2.5,
            material: Material::Parpado,
            surface: Arc::new(ParpadoField::new(seed ^ 0x9A4D)),
            // Bruma fina y violeta.
            atmosphere: Some(AtmosphereParams {
                thickness: 6.0,
                scale_height: 2.0,
                rayleigh: Vec3::new(0.006, 0.0025, 0.014),
                mie: 0.002,
                mie_g: 0.7,
                intensity: 1.6,
            }),
        }
        .build();
        parpado.spin_phase = parpado.orbit_phase;
        parpado.host = Some(Host::of(3, &vigia));
        // Su region no llega hasta Vigia.
        parpado.soi = parpado.radius * 2.5;

        // La Luna de Xelaju: solo existe en la version chapina, alrededor de
        // los Cuchumatanes (donde esta Argos).
        let (mut xela, xela_terrain, _) = PlanetSpec {
            name: "Xela",
            radius: 48.0,
            orbit: 560.0,
            period: 260.0,
            phase: 2.0,
            inclination: 0.25,
            spin: 260.0,
            tilt: 0.0,
            gravity: 2.0,
            material: Material::Chapin(Chapin::Xela),
            surface: chapin::field("Xela", seed ^ 0x3E1A).unwrap(),
            atmosphere: None,
        }
        .build();
        xela.host = Some(Host::of(4, &argos));
        xela.soi = xela.radius * 2.5;
        xela.label = "Luna de Xelaju";
        xela.absent = true;
        xela.hidden = true;

        let mut system = Self {
            bodies: vec![star, pira, jardin, vigia, argos, cementerio, parpado, xela],
            terrains: vec![
                None,
                Some(pira_terrain),
                Some(jardin_terrain),
                Some(vigia_terrain),
                Some(argos_terrain),
                Some(cementerio_terrain),
                Some(parpado_terrain),
                Some(xela_terrain),
            ],
            atmospheres: vec![
                None,
                pira_air,
                jardin_air,
                vigia_air,
                argos_air,
                None,
                parpado_air,
                None,
            ],
            meshes,
            ship_mesh: [1, 2, 3],
            // OCASO-1 se exporta con frente -Z y arriba +Y.
            ship_fix: DQuat::IDENTITY,
            ship_material: Material::Ship(ShipStyle {
                specular: 0.5,
                shininess: 40.0,
            }),
            noise: Perlin::new(seed),
            sky: SkyCube::bake(config::SKY_SIZE, seed, SkyStyle::Ocaso),
            textures: Textures::generate(),
            prop_models: props::build_models(),
            props: Vec::new(),
            prop_seed: config::PROP_SEED,
            star_base: 320.0,
            eye_body: 3,
            sites: Vec::new(),
            version: Version::Ocaso,
            other: Vec::new(),
            other_sites: Vec::new(),
            other_sky: SkyCube::bake(config::SKY_SIZE, seed ^ 0xC4A9, SkyStyle::Chapin),
            other_seed: config::PROP_SEED,
            bottles,
        };
        system.sites = system.build_sites();
        system.scatter_props(config::PROP_SEED);
        system.build_chapin(seed);
        Ok(system)
    }

    /// Arma la version chapina de cada cuerpo (queda guardada en `other`).
    fn build_chapin(&mut self, seed: u32) {
        let looks: Vec<Look> = (0..self.bodies.len())
            .map(|i| {
                let body = &self.bodies[i];
                let Some(spec) = chapin::spec(body.name) else {
                    // La estrella: su material lo arma la app cada frame.
                    return Look {
                        label: if i == 0 { chapin::STAR_LABEL } else { body.label },
                        material: body.material,
                        surface: body.surface.clone(),
                        rings: body.rings,
                        absent: false,
                        terrain: None,
                        atmosphere: None,
                        props: PropSet::default(),
                    };
                };
                let surface = chapin::field(body.name, seed ^ hash_name(body.name));
                let terrain = surface
                    .clone()
                    .map(|field| Terrain::new(body.radius, field, config::TERRAIN_FINEST));
                Look {
                    label: spec.label,
                    material: Material::Chapin(spec.material),
                    surface,
                    rings: spec.rings,
                    absent: false,
                    terrain,
                    atmosphere: spec.atmosphere.map(|p| AtmosphereModel::new(body.radius as f32, p)),
                    props: PropSet::default(),
                }
            })
            .collect();
        self.other = looks;
        self.other_sites = self.chapin_sites();
        // Los objetos de la version chapina, con su relieve y sus lugares.
        let props: Vec<PropSet> = (0..self.bodies.len())
            .map(|i| match &self.other[i].surface {
                Some(field) => self.scatter_with(i, self.prop_seed, &**field, Version::Chapin, &self.other_sites),
                None => PropSet::default(),
            })
            .collect();
        for (look, set) in self.other.iter_mut().zip(props) {
            look.props = set;
        }
        self.other_seed = self.prop_seed;
    }

    /// Los lugares de la version chapina: donde estan los de OCASO (o, si el
    /// cuerpo no tiene, cerca del terminador), con su nombre y sus objetos.
    fn chapin_sites(&self) -> Vec<Site> {
        let t = 40.0;
        let mut sites = Vec::new();
        for (i, body) in self.bodies.iter().enumerate() {
            let default = match self.sites.iter().find(|s| s.body == i) {
                Some(site) => (site.normal, site.forward),
                None => {
                    let center = body.position(t);
                    let to_star = (self.bodies[0].position(t) - center).normalize();
                    let side = to_star.cross(DVec3::Y).normalize();
                    let up = side.cross(to_star).normalize();
                    let normal = (to_star * 0.35 - side * 0.85 + up * 0.4).normalize();
                    let forward = (to_star - normal * to_star.dot(normal)).normalize();
                    let inv = body.rotation(t).conjugate();
                    (inv.rotate(normal), inv.rotate(forward))
                }
            };
            if let Some((name, normal, forward, _)) = chapin::site(body.name, default) {
                sites.push(Site {
                    body: i,
                    name,
                    normal,
                    forward,
                    view: Self::SITE_VIEW,
                });
            }
        }
        sites
    }

    /// Cambia a la otra version: intercambia el nombre, el material, el
    /// relieve, los anillos, el terreno, la atmosfera, los objetos y los
    /// lugares de cada cuerpo con los guardados. Las orbitas, los radios y
    /// la gravedad no cambian, asi que la nave sigue donde estaba.
    pub fn toggle_version(&mut self) {
        for (i, look) in self.other.iter_mut().enumerate() {
            let body = &mut self.bodies[i];
            std::mem::swap(&mut body.label, &mut look.label);
            std::mem::swap(&mut body.material, &mut look.material);
            std::mem::swap(&mut body.surface, &mut look.surface);
            std::mem::swap(&mut body.rings, &mut look.rings);
            std::mem::swap(&mut body.absent, &mut look.absent);
            body.hidden = body.absent;
            std::mem::swap(&mut self.terrains[i], &mut look.terrain);
            std::mem::swap(&mut self.atmospheres[i], &mut look.atmosphere);
            std::mem::swap(&mut self.props[i], &mut look.props);
        }
        std::mem::swap(&mut self.sites, &mut self.other_sites);
        std::mem::swap(&mut self.sky, &mut self.other_sky);
        std::mem::swap(&mut self.prop_seed, &mut self.other_seed);
        self.version = match self.version {
            Version::Ocaso => Version::Chapin,
            Version::Chapin => Version::Ocaso,
        };
        // Si en la otra version se cambio un radio o la semilla, lo que se
        // guardo quedo viejo: se rehace.
        for i in 0..self.bodies.len() {
            let stale = self.terrains[i]
                .as_ref()
                .is_some_and(|terrain| terrain.radius != self.bodies[i].radius);
            if stale {
                self.set_radius(i, self.bodies[i].radius);
            }
        }
        if self.prop_seed != self.other_seed {
            self.scatter_props(self.other_seed);
        }
    }

    /// Coloca (o vuelve a colocar) los objetos de todos los planetas con
    /// `seed`. Cada planeta mezcla la semilla con su indice para no repetir
    /// el mismo patron.
    pub fn scatter_props(&mut self, seed: u32) {
        self.prop_seed = seed;
        self.props = (0..self.bodies.len())
            .map(|index| self.scatter_body(index, seed))
            .collect();
    }

    /// Indice de un cuerpo por nombre.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.bodies.iter().position(|b| b.name == name)
    }

    /// Los lugares para visitar. En Pira (que le da siempre la misma cara a
    /// la estrella) y en Argos quedan cerca del terminador: la estrella baja
    /// en el horizonte marca el relieve. En Cementerio, al pie de la baliza.
    fn build_sites(&self) -> Vec<Site> {
        let t = 40.0;
        let local = |index: usize, normal: DVec3, toward: DVec3| {
            let inv = self.bodies[index].rotation(t).conjugate();
            let forward = (toward - normal * toward.dot(normal)).normalize();
            (inv.rotate(normal), inv.rotate(forward))
        };
        let axes = |index: usize| {
            let center = self.bodies[index].position(t);
            let to_star = (self.bodies[0].position(t) - center).normalize();
            let side = to_star.cross(DVec3::Y).normalize();
            (to_star, side, side.cross(to_star).normalize())
        };
        let mut sites = Vec::new();
        if let Some(i) = self.find("Jardin") {
            let (normal, forward, _) = self.garden_grove_frame();
            sites.push(Site { body: i, name: "El claro", normal, forward, view: Self::SITE_VIEW });
        }
        if let Some(i) = self.find("Pira") {
            let (to_star, side, up) = axes(i);
            let (normal, forward) = local(i, (to_star * 0.42 + side * 0.9 + up * 0.1).normalize(), to_star);
            sites.push(Site { body: i, name: "Los geiseres", normal, forward, view: Self::SITE_VIEW });
        }
        if let Some(i) = self.find("Argos") {
            let (to_star, side, up) = axes(i);
            let (normal, forward) = local(i, (to_star * 0.35 - side * 0.85 + up * 0.4).normalize(), to_star);
            sites.push(Site { body: i, name: "El nido", normal, forward, view: Self::SITE_VIEW });
        }
        if let Some(i) = self.find("Cementerio") {
            let normal = DVec3::new(0.32, 0.22, 0.92).normalize();
            let forward = normal.cross(DVec3::Y).normalize();
            sites.push(Site { body: i, name: "La baliza", normal, forward, view: Self::SITE_VIEW });
        }
        // En la luna de Vigia, a la orilla del lago: Vigia del otro lado del
        // agua, con su ojo mirando y reflejado.
        if let Some(i) = self.find("Parpado") {
            let normal = MIRROR_SHORE.as_dvec3().normalize();
            let toward = DVec3::new(-1.0, 0.0, 0.0);
            let forward = (toward - normal * toward.dot(normal)).normalize();
            sites.push(Site {
                body: i,
                name: "El espejo",
                normal,
                forward,
                view: (3.0, 6.0, 6.0),
            });
        }
        sites
    }

    /// Como se llega a los lugares de los planetas: desde arriba y atras.
const SITE_VIEW: (f64, f64, f64) = (22.0, 90.0, 0.0);

/// Bioma de llegada, fijo en el espacio local de Jardín. El claro y sus
    /// grandes colonias forman parte del planeta y giran con él.
    pub fn garden_grove_frame(&self) -> (DVec3, DVec3, DVec3) {
        let i = self.find("Jardin").unwrap();
        let body = &self.bodies[i];
        let t = 40.0;
        let center = body.position(t);
        let to_star = (self.bodies[0].position(t) - center).normalize();
        let side = to_star.cross(DVec3::Y).normalize();
        let up = side.cross(to_star).normalize();
        let giant = (self.bodies[self.eye_body].position(t) - center).normalize();
        let normal = (giant * 0.37 + side * 0.92 + up * 0.12).normalize();
        let toward = (giant - normal * giant.dot(normal)).normalize();
        let right = toward.cross(normal);
        let forward = (toward + right * 0.65).normalize();
        let inv = body.rotation(t).conjugate();
        let n = inv.rotate(normal);
        let f = inv.rotate(forward);
        (n, f, f.cross(n))
    }
}

/// El cuerpo cuya superficie esta mas cerca de `p`, y la altitud sobre ella
/// (contando el relieve del terreno).
pub fn nearest_surface(bodies: &[Body], p: DVec3, t: f64) -> (usize, f64) {
    bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.hidden)
        .map(|(i, b)| {
            let offset = p - b.position(t);
            (i, offset.length() - b.radius - b.surface_height(offset, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, f64::INFINITY))
}
