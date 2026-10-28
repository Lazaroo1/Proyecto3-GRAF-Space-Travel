//! Estado del juego: el sistema, la nave, la camara y el armado de cada frame.
//!
//! No sabe nada de raylib: recibe la entrada ya traducida (`FrameInput`) y
//! deja la imagen en el `Renderer`. Por eso las capturas y el benchmark
//! corren sin abrir ninguna ventana.

use std::f64::consts::PI;
use std::path::PathBuf;

use crate::audio::AudioParams;
use crate::config;
use crate::math::blackbody::planckian_rgb;
use crate::math::geodesic::LensTable;
use crate::math::{DMat3, DMat4, DQuat, DVec3, Vec3, hash_u32, smoothing, smoothstep};
use crate::render::shadow::ShadowMap;
use crate::render::{Draw, Particle, Renderer, Ripple, Stats, View};
use crate::scene::camera::{Camera, CameraMode};
use crate::scene::cycle::{self, CycleState, Timeline};
use crate::scene::effects::{self, EXHAUST_COUNT, SPORE_COUNT};
use crate::scene::props::{self, Instance, PropKind, PropStats, PropView};
use crate::scene::ship::{self, Controls, FlightInfo, FlightSettings, ShipState};
use crate::scene::trajectory::{self, Prediction};
use crate::scene::{Mesh, System, nearest_surface, ribbon};
use crate::shaders::{
    self, AtmosphereFrame, BlackHole, DebugView, Gains, Material, PropStyle, RingInfo, SkyView,
    SunStyle, Uniforms, heartbeat_phase,
};
use crate::terrain::{LodSettings, TerrainStats, Viewer};

/// Entrada de un frame, ya traducida de teclas y mouse.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameInput {
    /// Movimiento del mouse en pixeles.
    pub look: (f64, f64),
    /// Empuje en ejes de la nave: x derecha, y arriba, z atras.
    pub thrust: DVec3,
    /// Alabeo: positivo hacia la izquierda.
    pub roll: f64,
    pub boost: bool,
    pub brake: bool,
    /// Mirar alrededor sin girar la nave.
    pub free_look: bool,
    /// Rueda del mouse: acerca o aleja la camara.
    pub zoom: f64,
    /// Rebobinar el tiempo (mantener R).
    pub rewind: bool,
}

/// Piloto reproducible para grabar el recorrido exigido por el curso. Usa
/// la misma fisica y los mismos controles que el vuelo manual.
#[derive(Default)]
pub struct DemoTour {
    phase: u8,
    phase_time: f64,
    orbit: f64,
    previous: Option<DVec3>,
    pub elapsed: f64,
}

/// Encuadre de una vista fija. Los angulos se miden alrededor del planeta:
/// azimut 0 es la cara que mira a la estrella, y la elevacion sube desde el
/// plano de la orbita.
#[derive(Clone, Copy, Debug)]
pub enum Shot {
    Garden,
    /// Sobre el lugar de un planeta, a `altitude` del suelo y `back` antes
    /// del centro, mirando hacia el (y `lift` mas arriba).
    Site {
        body: &'static str,
        altitude: f64,
        back: f64,
        lift: f64,
    },
    BlackHole,
    Star,
    /// Todo el sistema desde arriba.
    Overview,
    /// La nave en orbita a `distance` radios, mirando al planeta.
    Orbit {
        body: &'static str,
        distance: f64,
        azimuth: f64,
        elevation: f64,
    },
    /// La nave sobre la superficie, mirando al horizonte.
    Surface {
        body: &'static str,
        azimuth: f64,
        elevation: f64,
        altitude: f64,
    },
    /// La nave de cerca, con el planeta detras.
    Ship {
        body: &'static str,
    },
    /// La nave sin asistente, de costado a `distance` radios del planeta y
    /// con `speed` veces la velocidad de una orbita circular: se ve la
    /// trayectoria que predice la gravedad.
    Coast {
        body: &'static str,
        distance: f64,
        speed: f64,
    },
    /// Pira y la cola de polvo que pierde, de lejos.
    Stream,
    /// Detras del planeta `body`, mirando hacia la estrella (o hacia
    /// Gargantua): el planeta de perfil adelante y el centro del sistema al
    /// fondo.
    Gaze {
        body: &'static str,
    },
    /// Un objeto de la superficie de cerca. `distance` en unidades del mundo;
    /// `sun` es el seno de la altura del sol que se busca sobre su horizonte
    /// (bajo y de lado marca el relieve; bajo cero, de noche, se luce lo que
    /// brilla).
    Landmark {
        body: &'static str,
        kind: PropKind,
        distance: f64,
        sun: f64,
    },
}

/// Una vista fija con nombre, para capturas y para el benchmark, en un
/// instante del ciclo.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub shot: Shot,
    pub time: f64,
    /// Segundos desde la J del final (`None`: la estrella sigue en agonia).
    pub act: Option<f64>,
}

pub const PRESETS: [Preset; 32] = [
    Preset {
        name: "01_sistema",
        shot: Shot::Overview,
        time: 40.0,
        act: None,
    },
    Preset {
        name: "02_pira",
        shot: Shot::Orbit {
            body: "Pira",
            distance: 3.2,
            azimuth: 0.5,
            elevation: 0.2,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "03_pira_noche",
        shot: Shot::Orbit {
            body: "Pira",
            distance: 2.8,
            azimuth: 2.6,
            elevation: 0.15,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "04_jardin",
        shot: Shot::Orbit {
            body: "Jardin",
            distance: 3.0,
            azimuth: 1.2,
            elevation: 0.25,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "05_jardin_superficie",
        shot: Shot::Surface {
            body: "Jardin",
            azimuth: 1.75,
            elevation: 0.3,
            altitude: 6.0,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "06_vigia",
        shot: Shot::Orbit {
            body: "Vigia",
            distance: 2.6,
            azimuth: 0.8,
            elevation: 0.1,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "07_requiem",
        shot: Shot::Surface {
            body: "Requiem",
            azimuth: 1.25,
            elevation: 0.35,
            altitude: 6.0,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "08_cementerio",
        shot: Shot::Orbit {
            body: "Cementerio",
            distance: 4.2,
            azimuth: 0.9,
            elevation: 0.7,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "09_nave",
        shot: Shot::Ship { body: "Requiem" },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "10_glifo",
        shot: Shot::Landmark {
            body: "Requiem",
            kind: PropKind::Monolith,
            distance: 16.0,
            sun: 0.42,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "11_hongos",
        shot: Shot::Landmark {
            body: "Jardin",
            kind: PropKind::Mushroom,
            distance: 22.0,
            sun: -0.38,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "12_espinas",
        shot: Shot::Landmark {
            body: "Pira",
            kind: PropKind::Spike,
            distance: 26.0,
            sun: 0.3,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "13_naufragio",
        shot: Shot::Landmark {
            body: "Cementerio",
            kind: PropKind::Wreck,
            distance: 40.0,
            sun: 0.35,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "14_cola",
        shot: Shot::Stream,
        time: 40.0,
        act: None,
    },
    Preset {
        name: "15_trayectoria",
        shot: Shot::Coast {
            body: "Requiem",
            distance: 2.2,
            speed: 0.85,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "16_arritmia",
        shot: Shot::Orbit {
            body: "Pira",
            distance: 3.4,
            azimuth: 0.3,
            elevation: 0.25,
        },
        time: 700.0,
        act: Some(16.0),
    },
    Preset {
        name: "17_colapso",
        shot: Shot::Gaze { body: "Jardin" },
        time: 725.2,
        act: Some(25.2),
    },
    Preset {
        name: "17_onda_gravitacional",
        shot: Shot::Gaze { body: "Jardin" },
        time: 726.6,
        act: Some(30.6),
    },
    Preset {
        name: "18_gargantua",
        shot: Shot::Gaze { body: "Pira" },
        time: 830.0,
        act: Some(134.0),
    },
    Preset {
        name: "19_gargantua_jardin",
        shot: Shot::Gaze { body: "Jardin" },
        time: 830.0,
        act: Some(134.0),
    },
    Preset {
        name: "20_jardin_noche_eterna",
        shot: Shot::Landmark {
            body: "Jardin",
            kind: PropKind::Coral,
            distance: 24.0,
            sun: 0.3,
        },
        time: 850.0,
        act: Some(154.0),
    },
    Preset {
        name: "22_agonia",
        shot: Shot::Star,
        time: 40.0,
        act: None,
    },
    Preset {
        name: "23_jardin_biopunk",
        shot: Shot::Garden,
        time: 40.0,
        act: None,
    },
    Preset {
        name: "25_estertor",
        shot: Shot::Star,
        time: 40.0,
        act: Some(17.0),
    },
    Preset {
        name: "26_baliza",
        shot: Shot::Site {
            body: "Cementerio",
            altitude: 18.0,
            back: 70.0,
            lift: 0.0,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "27_geiseres",
        shot: Shot::Site {
            body: "Pira",
            altitude: 16.0,
            back: 30.0,
            lift: 0.0,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "28_monolitos",
        shot: Shot::Site {
            body: "Requiem",
            altitude: 14.0,
            back: 40.0,
            lift: 0.0,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "32_espejo",
        shot: Shot::Site {
            body: "Parpado",
            altitude: 3.0,
            back: 6.0,
            lift: 6.0,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "29_ojo_cerca",
        shot: Shot::Orbit {
            body: "Vigia",
            distance: 1.3,
            azimuth: 0.8,
            elevation: 0.1,
        },
        time: 40.0,
        act: None,
    },
    Preset {
        name: "30_comida",
        shot: Shot::Gaze { body: "Pira" },
        time: 830.0,
        act: Some(44.0),
    },
    Preset {
        name: "31_digestion",
        shot: Shot::Gaze { body: "Pira" },
        time: 830.0,
        act: Some(62.0),
    },
    Preset {
        name: "24_el_ojo",
        shot: Shot::BlackHole,
        time: 835.0,
        act: Some(139.0),
    },
];

pub struct App {
    pub system: System,
    pub renderer: Renderer,
    pub settings: FlightSettings,
    pub flight: FlightInfo,
    pub stats: Stats,
    pub camera_mode: CameraMode,
    pub lod: LodSettings,
    pub debug: DebugView,
    /// Objetos de la superficie del ultimo frame.
    pub prop_stats: PropStats,
    /// Intensidad de cada shader (panel de ajustes).
    pub gains: Gains,
    /// Exposicion elegida (el ciclo la baja durante los destellos).
    pub exposure: f32,
    /// El ciclo en el ultimo frame dibujado.
    pub cycle: CycleState,
    /// Cuando se desataron el final y el renacer (tecla J).
    pub timeline: Timeline,
    /// El ultimo lugar visitado con N.
    next_site: usize,
    /// Un aviso para el HUD (el lugar al que se llego, la muerte...).
    pub banner: Option<&'static str>,
    /// Donde estuvo la nave cada medio segundo, para rebobinar si muere.
    history: std::collections::VecDeque<(f64, ShipState, DQuat)>,
    /// Cuando cruzo el horizonte de Gargantua.
    dying: Option<f64>,
    /// Tabla de geodesicas de Gargantua: se rehace al cambiar la distancia.
    lens: Option<LensTable>,
    /// Camara congelada (F7): el culling y el LOD se calculan desde ella
    /// mientras se vuela por fuera para ver que se descarta.
    pub freeze: Option<Camera>,
    /// Nombres de los cuerpos en pantalla: posicion en pixeles y distancia.
    pub markers: Vec<Marker>,
    /// Cinta con las aristas del frustum congelado.
    frustum_mesh: Mesh,
    disk_gas: crate::gargantua_p2::disk::GasTexture,
    disk_noise: crate::gargantua_p2::math::noise::NoiseTable,
    /// Nubes de la corona de la estrella, tabuladas cada frame.
    clouds: shaders::CoronaClouds,
    /// Mapa de sombras; sus buffers se reutilizan de un frame al otro.
    shadow: ShadowMap,
    cinematic_fov: Option<f64>,
    /// Sonido apagado (M).
    pub muted: bool,
    /// Trayectoria predicha: siempre (`Some(true)`), nunca (`Some(false)`) o
    /// solo sin asistente (`None`).
    pub path_mode: Option<bool>,
    /// Si el ultimo frame mostro la trayectoria y si termina en un choque.
    pub path_shown: bool,
    pub path_impact: bool,
    /// Cinta de la trayectoria, rehecha cada frame.
    path_mesh: Mesh,
    /// Empuje que se ve en los motores, suavizado.
    throttle: f32,
    /// Vertical del modo local en el frame anterior: al moverse sobre la
    /// curvatura, la mira se transporta con ella.
    local_up: Option<DVec3>,
    ship: ShipState,
    /// Estado del paso anterior: el render interpola entre los dos.
    ship_prev: ShipState,
    /// Orientacion que pide el mouse; la nave la persigue.
    aim: DQuat,
    look_yaw: f64,
    look_pitch: f64,
    zoom: f64,
    sim_time: f64,
    accumulator: f64,
    /// Hacia donde mira el ojo de Vigia, en el espacio de Vigia.
    eye: DVec3,
}

/// Carpeta de modelos: la del directorio actual o, si se corre desde otro
/// lugar, la del proyecto.
fn asset_dir() -> PathBuf {
    let local = PathBuf::from("assets");
    if local.exists() {
        local
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    }
}

impl App {
    pub fn begin_tour(&mut self) -> DemoTour {
        self.apply_preset(&Preset {
            name: "recorrido",
            shot: Shot::Orbit {
                body: "Jardin",
                distance: 3.8,
                azimuth: 2.0,
                elevation: 0.2,
            },
            time: 40.0,
            act: None,
        });
        self.settings.local = false;
        DemoTour::default()
    }

    pub fn tour_step(&mut self, tour: &mut DemoTour) -> bool {
        let dt = config::PHYSICS_DT;
        let i = self.system.find("Jardin").unwrap();
        let body = &self.system.bodies[i];
        let rel = self.ship.world_pos - body.position(self.sim_time);
        let normal = rel.normalize();
        let altitude = rel.length() - body.radius - body.surface_height(rel, self.sim_time);
        let local = body.rotation(self.sim_time).conjugate().rotate(normal);
        let mut input = FrameInput::default();
        match tour.phase {
            0 => {
                self.aim = DQuat::look_rotation(-normal, DVec3::Y);
                input.thrust.z = -1.0;
                if altitude < 27.0 {
                    tour.phase = 1;
                    tour.phase_time = 0.0;
                }
            }
            1 => {
                input.brake = true;
                if tour.phase_time > 1.4 {
                    let tangent = normal.cross(DVec3::X).normalize();
                    self.aim = DQuat::look_rotation(tangent, normal);
                    self.settings.local = true;
                    tour.phase = 2;
                    tour.phase_time = 0.0;
                }
            }
            2 => {
                input.thrust.z = -1.0;
                input.boost = true;
                if let Some(previous) = tour.previous {
                    tour.orbit += previous.dot(local).clamp(-1.0, 1.0).acos();
                }
                if tour.orbit > std::f64::consts::TAU {
                    tour.phase = 3;
                    tour.phase_time = 0.0;
                    self.settings.local = false;
                }
            }
            3 => {
                self.aim = DQuat::look_rotation(-normal, self.aim.up());
                input.thrust.z = 1.0;
                if altitude > 850.0 {
                    tour.phase = 4;
                    tour.phase_time = 0.0;
                }
            }
            _ => {
                input.brake = true;
                self.aim = DQuat::look_rotation(-normal, DVec3::Y);
                if tour.phase_time > 2.5 {
                    return true;
                }
            }
        }
        tour.previous = Some(local);
        self.update(dt, &input);
        tour.elapsed += dt;
        tour.phase_time += dt;
        false
    }

    pub fn new(width: usize, height: usize) -> Result<Self, String> {
        let system = System::load(&asset_dir())?;
        let t = 0.0;
        let system_star_radius = system.bodies[0].radius;
        let planet = &system.bodies[1];
        let center = planet.position(t);
        let to_star = (system.bodies[0].position(t) - center).normalize();
        let side = to_star.cross(DVec3::Y).normalize();
        let start = center + (to_star * 0.8 + side * 0.6).normalize() * (planet.radius * 4.5);
        let aim = DQuat::look_rotation(center - start, DVec3::Y);
        let ship = ShipState::new(&system.bodies, start, aim, t);
        let eye = eye_target(&system, start, t);
        Ok(Self {
            eye,
            system,
            renderer: Renderer::new(width, height),
            settings: FlightSettings::default(),
            flight: FlightInfo::default(),
            stats: Stats::default(),
            camera_mode: CameraMode::Chase,
            lod: LodSettings {
                factor: config::LOD_FACTOR,
                budget: config::LOD_BUDGET,
                enabled: true,
                fixed_level: config::LOD_FIXED_LEVEL,
            },
            debug: DebugView::default(),
            prop_stats: PropStats::default(),
            gains: Gains::default(),
            exposure: config::EXPOSURE,
            cycle: cycle::at(t, &Timeline::default(), system_star_radius),
            timeline: Timeline::default(),
            next_site: usize::MAX,
            banner: None,
            history: std::collections::VecDeque::new(),
            dying: None,
            lens: None,
            freeze: None,
            markers: Vec::new(),
            frustum_mesh: Mesh::default(),
            disk_gas: crate::gargantua_p2::disk::GasTexture::new(),
            clouds: shaders::CoronaClouds::default(),
            disk_noise: crate::gargantua_p2::math::noise::NoiseTable::new(
                crate::gargantua_p2::config::NOISE_SEED,
            ),
            cinematic_fov: None,
            shadow: ShadowMap::new(),
            muted: false,
            path_mode: None,
            path_shown: false,
            path_impact: false,
            path_mesh: Mesh::default(),
            throttle: 0.0,
            local_up: None,
            ship,
            ship_prev: ship,
            aim,
            look_yaw: 0.0,
            look_pitch: 0.0,
            zoom: 1.0,
            sim_time: t,
            accumulator: 0.0,
        })
    }

    /// Tiempo de simulacion.
    pub fn time(&self) -> f64 {
        self.sim_time
    }

    /// Salta `seconds` en el tiempo (negativo rebobina). Los cuerpos, el
    /// ciclo y las particulas son funciones del tiempo; la nave conserva su
    /// posicion y su velocidad respecto a su cuerpo, asi que viaja con el.
    pub fn skip(&mut self, seconds: f64) {
        let t = (self.sim_time + seconds).max(0.0);
        self.system.bodies[0].radius =
            cycle::at(t, &self.timeline, self.system.star_base).collision_radius;
        let body = &self.system.bodies[self.ship.frame];
        // Junto a un planeta la nave gira con el: sigue sobre el mismo suelo.
        if self.ship.frame != 0 {
            let turn = body.rotation(t) * body.rotation(self.sim_time).conjugate();
            self.ship.rel_pos = turn.rotate(self.ship.rel_pos);
            self.ship.rel_vel = turn.rotate(self.ship.rel_vel);
            self.ship.rot = (turn * self.ship.rot).normalize();
            self.aim = (turn * self.aim).normalize();
        }
        // Y nunca queda dentro del suelo (ni de la estrella, que cambia).
        let ground =
            body.radius + body.surface_height(self.ship.rel_pos, t) + config::COLLISION_CLEARANCE;
        let r = self.ship.rel_pos.length();
        if r < ground && r > 0.0 {
            self.ship.rel_pos = self.ship.rel_pos * (ground / r);
        }
        self.ship.world_pos = body.position(t) + self.ship.rel_pos;
        self.ship_prev = self.ship;
        self.sim_time = t;
        self.accumulator = 0.0;
        self.local_up = None;
    }

    /// N: viaja al siguiente lugar para visitar (instantaneo), conservando
    /// el tiempo y el ciclo.
    pub fn visit_next_site(&mut self) {
        if self.system.sites.is_empty() {
            return;
        }
        self.next_site = (self.next_site + 1) % self.system.sites.len();
        let site = self.system.sites[self.next_site];
        let time = self.time();
        let (altitude, back, lift) = site.view;
        self.apply_preset(&Preset {
            name: "lugar",
            shot: Shot::Site {
                body: self.system.bodies[site.body].name,
                altitude,
                back,
                lift,
            },
            time,
            act: self.timeline.since(time),
        });
        self.banner = Some(site.name);
    }

    /// Si la nave cruza el horizonte de Gargantua muere: la pantalla se va a
    /// negro y el tiempo se rebobina quince segundos, con la nave donde
    /// estaba entonces.
    fn watch_horizon(&mut self) {
        if self.history.back().is_none_or(|h| self.sim_time - h.0 >= 0.5) {
            self.history.push_back((self.sim_time, self.ship, self.aim));
            if self.history.len() > 80 {
                self.history.pop_front();
            }
        }
        if let Some(start) = self.dying {
            if self.sim_time - start > 1.4 {
                let target = start - 15.0;
                let back = self
                    .history
                    .iter()
                    .rev()
                    .find(|h| h.0 <= target)
                    .or(self.history.front())
                    .copied();
                if let Some((time, ship, aim)) = back {
                    self.history.retain(|h| h.0 <= time);
                    self.ship = ship;
                    self.ship_prev = ship;
                    self.aim = aim;
                    self.sim_time = time;
                    self.accumulator = 0.0;
                    self.local_up = None;
                }
                self.dying = None;
                self.banner = Some("Rebobinando");
            }
            return;
        }
        let state = cycle::at(self.sim_time, &self.timeline, self.system.star_base);
        let hole = self.system.bodies[0].position(self.sim_time);
        if state.rs > 0.5 && (self.ship.world_pos - hole).length() < cycle::HORIZON * state.rs {
            self.dying = Some(self.sim_time);
            self.banner = Some("Cruzaste el horizonte");
        }
    }

    /// Que tan oscura esta la pantalla mientras la nave muere (0 a 1).
    fn death_fade(&self) -> f32 {
        self.dying
            .map_or(0.0, |start| smoothstep(0.0, 1.0, (self.sim_time - start) as f32))
    }

    /// J: desata el final de la estrella o, si ya empezo, su renacer.
    pub fn press_ending(&mut self) {
        self.timeline.press(self.sim_time);
    }

    /// Lo que el sintetizador necesita saber: el latido se oye mas cerca de
    /// la estrella, el motor con el empuje, el viento dentro de una
    /// atmosfera y el fondo cambia de tono con el planeta cercano.
    pub fn audio_params(&self) -> AudioParams {
        let t = self.sim_time;
        let star = &self.system.bodies[0];
        let distance = (self.ship.world_pos - star.position(t)).length();
        let beat = 0.2 + 0.8 / (1.0 + (distance / 1500.0).powi(2));
        let f = &self.flight;
        let wind = match self
            .system
            .atmospheres
            .get(f.nearest)
            .and_then(|a| a.as_ref())
        {
            Some(model) if f.altitude < model.params.thickness as f64 => {
                let density = 1.0 - f.altitude / model.params.thickness as f64;
                (f.speed / 40.0).min(1.0) * density
            }
            _ => 0.0,
        };
        // Un tono por planeta: notas graves de una escala menor.
        let drone_hz = match self.body_name(f.nearest) {
            "Pira" => 55.0,
            "Jardin" => 73.4,
            "Vigia" => 41.2,
            "Requiem" => 49.0,
            "Cementerio" => 61.7,
            "Parpado" => 46.2,
            "Epitafio" => 43.7,
            _ => 36.7,
        };
        let state = cycle::at(t, &self.timeline, self.system.star_base);
        let heart = if state.star_radius > 0.0 { 1.0 } else { 0.0 };
        AudioParams {
            beat_phase: state.beat_phase,
            beat_rate: state.beat_rate,
            beat: (beat * heart) as f32,
            throttle: self.throttle,
            wind: wind as f32,
            drone_hz,
            drone: 0.6,
            master: if self.muted { 0.0 } else { 0.8 },
        }
    }

    pub fn body_name(&self, index: usize) -> &'static str {
        self.system.bodies.get(index).map_or("?", |b| b.name)
    }

    /// Estadisticas del terreno del planeta mas cercano a la nave.
    pub fn terrain_stats(&self) -> Option<(&'static str, TerrainStats)> {
        let nearest = self.flight.nearest;
        let terrain = self.system.terrains.get(nearest)?.as_ref()?;
        Some((self.system.bodies[nearest].name, terrain.stats))
    }

    /// Vuelve a colocar los objetos de la superficie con otra semilla.
    pub fn reseed_props(&mut self) {
        let seed = self.system.prop_seed.wrapping_add(1);
        self.system.scatter_props(seed);
    }

    pub fn toggle_camera(&mut self) {
        self.camera_mode = match self.camera_mode {
            CameraMode::Chase => CameraMode::Cockpit,
            CameraMode::Cockpit => CameraMode::Chase,
        };
    }

    /// Recuperacion del piloto: vuelve junto a Pira, conserva el tiempo y el
    /// estado del sistema. No se usa para el recorrido continuo evaluado.
    pub fn return_to_system(&mut self) {
        let time = self.time();
        self.apply_preset(&Preset {
            name: "regreso",
            shot: Shot::Orbit {
                body: "Pira",
                distance: 3.2,
                azimuth: 0.5,
                elevation: 0.2,
            },
            time,
            act: self.timeline.since(time),
        });
        self.settings.assist = true;
        self.settings.cruise = false;
    }

    /// Avanza el juego `frame_dt` segundos reales.
    pub fn update(&mut self, frame_dt: f64, input: &FrameInput) {
        let frame_dt = frame_dt.clamp(0.0, config::MAX_FRAME_DT);
        self.steer(input, frame_dt);
        let controls = Controls {
            thrust: input.thrust,
            boost: input.boost,
            brake: input.brake,
            aim: self.aim,
        };
        if input.brake {
            self.settings.cruise = false;
        }
        if input.rewind {
            // Rebobinar: todo el sistema es funcion del tiempo y vuelve atras;
            // la nave se queda quieta respecto a su cuerpo.
            self.skip(-config::REWIND_SPEED * frame_dt);
            self.history.retain(|h| h.0 <= self.sim_time);
            self.dying = None;
            return;
        }
        // Acumulador: se dan tantos pasos fijos como quepan en el tiempo real;
        // el sobrante se usa para interpolar el dibujo.
        self.accumulator += frame_dt;
        while self.accumulator >= config::PHYSICS_DT {
            // La estrella del ciclo cambia de tamano: la nave choca con ella
            // (o con la zona de exclusion del agujero) a su radio de ahora.
            let now = cycle::at(self.sim_time, &self.timeline, self.system.star_base);
            self.system.bodies[0].radius = now.collision_radius;
            if let Some(i) = self.system.find("Pira") {
                self.system.bodies[i].hidden = now.swallow >= 1.0;
            }
            self.ship_prev = self.ship;
            self.flight = ship::step(
                &mut self.ship,
                &controls,
                &self.settings,
                &self.system.bodies,
                self.sim_time,
                config::PHYSICS_DT,
            );
            self.sim_time += config::PHYSICS_DT;
            self.accumulator -= config::PHYSICS_DT;
        }
        self.timeline.advance(self.sim_time);
        self.watch_horizon();
        // El fuego de los motores sigue al empuje (y al crucero).
        let cruise = if self.flight.local && self.settings.cruise {
            config::CRUISE_THRUST
        } else {
            0.0
        };
        let push = (input.thrust.length().min(1.0).max(cruise)
            * if input.boost { 1.0 } else { 0.6 }) as f32;
        self.throttle += (push - self.throttle) * smoothing(8.0, frame_dt) as f32;
        // El ojo de Vigia gira despacio hacia la nave.
        let target = eye_target(&self.system, self.ship.world_pos, self.sim_time);
        self.eye = self.eye.lerp(target, smoothing(0.35, frame_dt)).normalize();
    }

    /// El mouse mueve la mira al ritmo de la pantalla (sin esperar a la
    /// fisica), para que apuntar se sienta inmediato.
    fn steer(&mut self, input: &FrameInput, dt: f64) {
        let (dx, dy) = input.look;
        let sens = config::MOUSE_SENSITIVITY;
        if input.free_look {
            self.look_yaw = (self.look_yaw - dx * sens).clamp(-PI, PI);
            self.look_pitch = (self.look_pitch - dy * sens).clamp(-1.4, 1.4);
        } else {
            // Al soltar el clic derecho, la mirada regresa suave al frente.
            let k = smoothing(6.0, dt);
            self.look_yaw -= self.look_yaw * k;
            self.look_pitch -= self.look_pitch * k;
        }
        let (mx, my) = if input.free_look {
            (0.0, 0.0)
        } else {
            (dx * sens, dy * sens)
        };
        if self.flight.local {
            self.steer_local(mx, my, dt);
        } else {
            self.local_up = None;
            let yaw = DQuat::from_axis_angle(DVec3::Y, -mx);
            let pitch = DQuat::from_axis_angle(DVec3::X, -my);
            let roll = DQuat::from_axis_angle(DVec3::Z, input.roll * config::ROLL_RATE * dt);
            self.aim = (self.aim * yaw * pitch * roll).normalize();
        }
        self.zoom = (self.zoom * (1.0 - 0.1 * input.zoom)).clamp(0.35, 8.0);
    }

    /// Mira del modo local, como en Mario Galaxy: el "arriba" es siempre la
    /// vertical del planeta bajo la nave. El mouse gira el rumbo alrededor de
    /// esa vertical y cabecea con un tope; la mira se transporta con la
    /// curvatura (el giro que lleva la vertical anterior a la nueva), asi que
    /// avanzar derecho rodea el planeta por un circulo maximo. El alabeo se
    /// quita suave: al entrar al modo la nave se nivela sin saltar.
    fn steer_local(&mut self, dx: f64, dy: f64, dt: f64) {
        let body = &self.system.bodies[self.ship.frame];
        let up = (self.ship.world_pos - body.position(self.sim_time)).normalize();
        // El suelo gira: la mira gira con el (asi el rumbo no se desliza
        // respecto al terreno) y despues sigue la curvatura.
        let spin = DQuat::from_scaled_axis(ship::angular_velocity(body) * dt);
        if let Some(previous) = self.local_up {
            self.aim = DQuat::from_rotation_arc(spin.rotate(previous), up) * spin * self.aim;
        }
        self.local_up = Some(up);
        let yaw = DQuat::from_axis_angle(up, -dx);
        let mut aim = yaw * self.aim * DQuat::from_axis_angle(DVec3::X, -dy);
        // Cabeceo de 75 grados como maximo. Si se paso (se entro al modo
        // mirando al suelo), se lleva suave hacia el borde sin ignorar el
        // mouse.
        let limit = 75f64.to_radians();
        let pitch = aim.forward().dot(up).clamp(-1.0, 1.0).asin();
        if pitch.abs() > limit {
            let target = pitch.clamp(-limit, limit);
            let eased = pitch + (target - pitch) * smoothing(4.0, dt).max(0.02);
            let (heading, _) = ship::heading(aim, up);
            aim = DQuat::look_rotation(heading * eased.cos() + up * eased.sin(), up);
        }
        let level = DQuat::look_rotation(aim.forward(), up);
        self.aim = aim.slerp(level, smoothing(5.0, dt)).normalize();
    }

    /// Renderiza el estado interpolado entre los dos ultimos pasos de fisica.
    pub fn render(&mut self) -> Stats {
        let frame_start = std::time::Instant::now();
        let dt = config::PHYSICS_DT;
        let alpha = (self.accumulator / dt).clamp(0.0, 1.0);
        let t = (self.sim_time - dt * (1.0 - alpha)).max(0.0);
        let ship_pos = self.ship_prev.world_pos.lerp(self.ship.world_pos, alpha);
        let ship_rot = self.ship_prev.rot.slerp(self.ship.rot, alpha);

        let camera = self.camera(ship_pos, ship_rot, t);
        let view = camera.view(self.renderer.width(), self.renderer.height());
        // El culling y el LOD se deciden desde la camara congelada si la hay.
        let cull_camera = self.freeze.unwrap_or(camera);
        let cull_view = cull_camera.view(self.renderer.width(), self.renderer.height());
        self.update_terrain(t, &cull_camera, &cull_view);
        self.renderer.cull = self.freeze.map(|frozen| {
            (
                frozen.view(self.renderer.width(), self.renderer.height()),
                (camera.pos - frozen.pos).as_vec3(),
            )
        });
        self.build_frustum(&camera, pixel_angle_of(&camera, self.renderer.height()));
        self.markers = markers(
            &self.system,
            &self.timeline,
            t,
            &camera,
            &view,
            self.renderer.width(),
            self.renderer.height(),
        );

        let show_ship = self.camera_mode == CameraMode::Chase;
        let pixel_angle = 2.0 * (camera.fov_y * 0.5).tan() / self.renderer.height() as f64;
        let state = cycle::at(t, &self.timeline, self.system.star_base);
        self.system.bodies[0].radius = state.collision_radius;
        if let Some(i) = self.system.find("Pira") {
            self.system.bodies[i].hidden = state.swallow >= 1.0;
        }
        let star = &self.system.bodies[0];
        self.cycle = state;
        let beat = if state.star_radius > 0.0 {
            heartbeat_phase(state.beat_phase) as f32
        } else {
            0.0
        };
        let heart = if state.star_radius > 0.0 { 1.0 } else { 0.0 };
        // La estrella del ciclo: su radio late, se hincha y colapsa.
        // El panel de ajustes cambia el latido base (0.012 de diseno).
        let pulse = state.pulse * star.pulse / 0.012;
        let star_radius = state.star_radius * (1.0 + pulse * beat as f64);
        let star_material = Material::Sun(SunStyle {
            temperature: state.star_temperature,
            intensity: 3.0 * state.star_intensity,
        });
        // De donde sale la luz: la estrella o, despues, el disco de Gargantua.
        let disk_light = state.disk_density * (1.0 - state.star_glow);
        let sun_light = planckian_rgb(state.star_temperature)
            * (1.5 * (1.0 + 0.06 * beat) * state.star_light)
            + planckian_rgb(state.disk_peak * 1.3) * (0.35 * disk_light);
        let light_radius = star_radius
            .max(state.disk_outer as f64 * state.rs * 0.5)
            .max(1.0);
        self.build_path(t, &camera, pixel_angle);
        self.emit_particles(t, &camera, ship_pos, ship_rot, show_ship);
        // Gargantua se come a Pira: hierve y se encoge dentro de su vapor.
        let melting = self.system.find("Pira").filter(|_| state.stream_age > 0.0);
        let shrink = melting.map(|body| Shrink {
            body,
            scale: shaders::Vapor::core(state.stream_age) as f64,
        });
        let mut draws = build_draws(
            &self.system,
            t,
            &camera,
            ship_pos,
            ship_rot,
            show_ship,
            star_radius,
            star_material,
            shrink,
        );
        self.prop_stats = prop_draws(
            &self.system,
            t,
            &camera,
            &cull_camera,
            &cull_view,
            pixel_angle,
            light_radius,
            melting,
            &mut draws,
        );
        for mesh in [&self.path_mesh, &self.frustum_mesh] {
            if mesh.indices.is_empty() {
                continue;
            }
            draws.push(Draw {
                mesh,
                indices: &mesh.indices,
                local_offset: Vec3::ZERO,
                model: DMat4::from_affine(&DMat3::IDENTITY, DVec3::ZERO).to_f32(),
                normal: DMat3::IDENTITY.to_f32(),
                material: Material::Line(Vec3::ONE),
                bounds_center: mesh.bounds_center,
                bounds_radius: mesh.bounds_radius,
                tag: 0,
            });
        }
        let (near, altitude) = nearest_surface(&self.system.bodies, camera.pos, t);
        // Sombras: junto a una superficie, el suelo hacia donde se mira; si
        // no, solo la nave.
        let shadow_area = if near != 0 && altitude < 180.0 {
            let body = &self.system.bodies[near];
            let up = (camera.pos - body.position(t)).normalize().as_vec3();
            let horizon = view.forward - up * view.forward.dot(up);
            let center = horizon.normalize() * 35.0 - up * (altitude as f32);
            Some((center, if altitude < 45.0 { 180.0 } else { 400.0 }))
        } else if show_ship {
            Some(((ship_pos - camera.pos).as_vec3(), 22.0))
        } else {
            None
        };
        if let Some((center, span)) = shadow_area {
            self.shadow.update(
                &draws,
                center,
                (self.system.bodies[0].position(t) - camera.pos).as_vec3(),
                span,
                t as f32,
                &self.system.noise,
            );
        }
        let shadow = shadow_area.map(|_| &self.shadow);
        let mut points = Vec::new();
        if let Some(i) = self.system.find("Jardin") {
            let body = &self.system.bodies[i];
            let rotation = body.rotation(t);
            let mut candidates: Vec<_> = self.system.props[i]
                .instances
                .iter()
                .filter(|p| matches!(p.kind, PropKind::Mushroom | PropKind::Coral))
                .map(|p| {
                    let height = if p.kind == PropKind::Mushroom {
                        7.0
                    } else {
                        6.0
                    };
                    let local =
                        p.position + p.rotation.rotate(DVec3::Y * (height * p.scale as f64));
                    let pos = (body.position(t) + rotation.rotate(local) - camera.pos).as_vec3();
                    (pos.length_squared(), pos, p.scale)
                })
                .filter(|p| p.0 < 10000.0)
                .collect();
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, pos, scale) in candidates.into_iter().take(12) {
                points.push(crate::shaders::PointLight {
                    pos,
                    color: Vec3::new(0.06, 2.2, 1.6) * (140.0 * scale * scale * state.biolum),
                    radius: 35.0 * scale,
                });
            }
        }
        // Los lugares de los demas planetas tambien alumbran lo de alrededor.
        let ship_rel = (ship_pos - camera.pos).as_vec3();
        for (mouth, _, activity, _) in vents_near(&self.system, t, camera.pos, ship_pos) {
            points.push(crate::shaders::PointLight {
                pos: mouth,
                color: Vec3::new(2.6, 0.9, 0.22) * (90.0 * activity),
                radius: 60.0,
            });
        }
        for (glyph, up, _, scale) in props_near(&self.system, "Requiem", PropKind::Monolith, props::GLYPH_HEIGHT, t, camera.pos, 220.0) {
            let awake = smoothstep(220.0, 30.0, (glyph - ship_rel).length());
            if awake > 0.02 {
                points.push(crate::shaders::PointLight {
                    pos: glyph + up * 1.0,
                    color: Vec3::new(0.6, 0.75, 1.2) * (22.0 * awake * scale * scale),
                    radius: 40.0 * scale,
                });
            }
        }
        for (base, up, _, scale) in props_near(&self.system, "Parpado", PropKind::Crystal, 6.0, t, camera.pos, 200.0) {
            let awake = smoothstep(150.0, 20.0, (base - ship_rel).length());
            if awake > 0.02 {
                points.push(crate::shaders::PointLight {
                    pos: base + up * 2.0,
                    color: Vec3::new(0.8, 0.3, 1.4) * (40.0 * awake * scale * scale),
                    radius: 40.0 * scale,
                });
            }
        }
        if let Some((top, _)) = beacon_lamp(&self.system, t) {
            let near = smoothstep(300.0, 60.0, (top - ship_pos).length() as f32);
            let flash = shaders::beacon_flash(t as f32, near);
            points.push(crate::shaders::PointLight {
                pos: (top - camera.pos).as_vec3(),
                color: Vec3::new(2.4, 0.12, 0.05) * (400.0 * flash),
                radius: 160.0,
            });
        }
        // Las mas cercanas primero: el shading recorre todas por pixel.
        points.sort_by(|a, b| a.pos.length_squared().total_cmp(&b.pos.length_squared()));
        points.truncate(12);
        let atmospheres = atmosphere_frames(&self.system, t, &camera, &view, melting);
        // Si la camara esta dentro de una atmosfera, su cielo sale de una tabla.
        let sky_view = atmospheres
            .iter()
            .enumerate()
            .find(|(_, frame)| {
                frame.center.length() < frame.model.radius + frame.model.params.thickness
            })
            .map(|(i, frame)| SkyView::build(i, frame, sun_light));
        let star_pos = self.system.bodies[0].position(t);
        // Gargantua: la tabla de geodesicas solo se rehace si la distancia
        // al agujero cambio lo suficiente.
        if state.rs > 0.5 {
            let observer = ((star_pos - camera.pos).length() / state.rs) as f32;
            match &mut self.lens {
                Some(table) if table.fits(observer) => {}
                Some(table) => table.rebuild(observer),
                None => self.lens = Some(LensTable::build(observer)),
            }
        }
        if state.rs > 0.5 {
            self.disk_gas.update_preview(&self.disk_noise, t as f32);
        }
        let hole = match (&self.lens, state.rs > 0.5) {
            (Some(table), true) => {
                let normal = DQuat::from_axis_angle(DVec3::Z, self.system.bodies[0].axial_tilt)
                    .rotate(DVec3::Y);
                let axis_a = normal.cross(DVec3::Z).normalize();
                let axis_b = normal.cross(axis_a);
                // Pira en el marco del disco (el mismo de `BlackHole::local`).
                let stream = self
                    .system
                    .find("Pira")
                    .filter(|_| (0.0..shaders::gargantua::STREAM_LIFE).contains(&state.stream_age))
                    .map(|i| {
                        let v = (self.system.bodies[i].position(t) - star_pos) / state.rs;
                        shaders::gargantua::Stream::new(
                            crate::gargantua_p2::math::Vec3::new(
                                v.dot(axis_a) as f32,
                                v.dot(normal) as f32,
                                -v.dot(axis_b) as f32,
                            ),
                            state.stream_age,
                        )
                    });
                let extent = stream
                    .as_ref()
                    .map_or(state.disk_outer, |s| state.disk_outer.max(s.reach()));
                Some(BlackHole {
                    center: (star_pos - camera.pos).as_vec3(),
                    rs: state.rs as f32,
                    normal: normal.as_vec3(),
                    axis_a: axis_a.as_vec3(),
                    axis_b: axis_b.as_vec3(),
                    table,
                    gas: &self.disk_gas,
                    density: state.disk_density,
                    outer: state.disk_outer,
                    peak: state.disk_peak,
                    cone_cos: table
                        .disk_cone(extent.max(3.5))
                        .max(table.shadow() * 1.2)
                        .cos(),
                    stream,
                })
            }
            _ => None,
        };
        self.renderer.p2_curve = state.rs > 0.5;
        self.renderer.exposure =
            self.exposure * state.exposure * if self.renderer.p2_curve { 5.0 } else { 1.0 };
        self.renderer.whiteout = state.whiteout;
        self.renderer.blackout = self.death_fade();
        // Lo que esta detras del agujero y cae en su sombra no se ve: las
        // brasas de Pira del otro lado no pueden brillar sobre ella.
        if let Some(hole) = &hole {
            let distance = hole.center.length();
            let to_hole = hole.center / distance;
            let cos_shadow = (hole.table.shadow() * 1.05).cos();
            self.renderer.particles.retain(|p| {
                let along = p.pos.dot(to_hole);
                along < distance || along / p.pos.length() < cos_shadow
            });
        }
        // La onda gravitacional sale del agujero cuando se abre el ojo y
        // cruza la pantalla en unos tres segundos.
        let wave = state.since_eye;
        self.renderer.ripple = None;
        if (0.0..3.0).contains(&wave) {
            let clip = view
                .view_proj
                .transform_point((star_pos - camera.pos).as_vec3());
            if clip.w > view.near {
                let (w, h) = (self.renderer.width() as f32, self.renderer.height() as f32);
                let fade = (1.0 - wave / 3.0) as f32;
                self.renderer.ripple = Some(Ripple {
                    x: (clip.x / clip.w + 1.0) * 0.5 * w,
                    y: (1.0 - clip.y / clip.w) * 0.5 * h,
                    radius: wave as f32 * w * 0.45,
                    width: h * 0.06,
                    strength: h * 0.025 * fade * fade,
                });
            }
        }
        let rings = ring_info(&self.system, t, camera.pos);
        // Las nubes de la corona se tabulan solo si pueden caer en pantalla
        // (si no, un reflejo que las vea las calcula pixel por pixel).
        let corona = {
            let to_star = star_pos - camera.pos;
            let distance = to_star.length();
            let reach = (star_radius * 3.0 / distance).min(1.0).asin();
            let half_diagonal = (view.tan_half_x as f64).hypot(view.tan_half_y as f64).atan();
            let cone = (half_diagonal + reach).min(std::f64::consts::PI);
            state.star_glow > 0.001 && to_star.dot(camera.rot.forward()) > distance * cone.cos()
        };
        if corona {
            self.clouds.update(t as f32, &self.system.noise);
        }
        let uniforms = Uniforms {
            shadow,
            points: &points,
            time: t as f32,
            beat,
            beat_phase: state.beat_phase as f32,
            beat_rate: state.beat_rate as f32,
            heart,
            star_glow: state.star_glow,
            flares: state.flares,
            tail: pira_tail(&self.system, t, camera.pos, sun_light).filter(|_| state.star_glow > 0.01),
            vapor: melting.and_then(|i| {
                let pira = &self.system.bodies[i];
                let center = pira.position(t);
                let toward = (star_pos - center).normalize();
                let velocity = pira.velocity(t);
                let ahead = (velocity - toward * velocity.dot(toward)).normalize();
                shaders::Vapor::at(
                    (center - camera.pos).as_vec3(),
                    pira.radius as f32,
                    toward.as_vec3(),
                    ahead.as_vec3(),
                    state.stream_age,
                    sun_light,
                )
            }),
            nebula: shaders::Nebula::at(
                state.shell_age,
                (star_pos - camera.pos).as_vec3(),
                self.system.star_base as f32,
                state.shell_light,
            ),
            black_hole: hole.as_ref(),
            sun_pos: (star_pos - camera.pos).as_vec3(),
            sun_radius: light_radius as f32,
            sun_light,
            noise: &self.system.noise,
            sky: &self.system.sky,
            textures: &self.system.textures,
            pixel_angle: pixel_angle as f32,
            eye_dir: self.eye.as_vec3(),
            eye_near: {
                let vigia = &self.system.bodies[self.system.eye_body];
                let distance = (self.ship.world_pos - vigia.position(t)).length() / vigia.radius;
                smoothstep(4.0, 1.25, distance as f32)
            },
            giant: {
                let vigia = &self.system.bodies[self.system.eye_body];
                let rotation = vigia.rotation(t).to_mat3();
                Some(shaders::Giant {
                    center: (vigia.position(t) - camera.pos).as_vec3(),
                    radius: vigia.radius as f32,
                    to_world: rotation.to_f32(),
                    to_local: rotation.transpose().to_f32(),
                })
            },
            ship: (ship_pos - camera.pos).as_vec3(),
            melt: state.swallow,
            rings: &rings,
            atmospheres: &atmospheres,
            sky_view: sky_view.as_ref(),
            biolum: self.gains.jardin * state.biolum,
            gains: self.gains,
            clouds: corona.then_some(&self.clouds),
            debug: self.debug,
        };
        self.stats = self.renderer.render(&draws, &view, &uniforms);
        self.stats.ms_total = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.stats
    }

    /// Aristas del frustum congelado: de la camara congelada hacia sus cuatro
    /// esquinas y el rectangulo del fondo, en amarillo.
    fn build_frustum(&mut self, camera: &Camera, pixel_angle: f64) {
        let mesh = &mut self.frustum_mesh;
        *mesh = Mesh::default();
        let Some(frozen) = self.freeze else { return };
        let aspect = self.renderer.width() as f64 / self.renderer.height() as f64;
        let tan_y = (frozen.fov_y * 0.5).tan();
        let tan_x = tan_y * aspect;
        let reach = 600.0;
        let origin = frozen.pos - camera.pos;
        let corner = |sx: f64, sy: f64| {
            let local = DVec3::new(sx * tan_x, sy * tan_y, -1.0) * reach;
            origin + frozen.rot.rotate(local)
        };
        let corners = [
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ];
        let color = Vec3::new(2.0, 1.6, 0.2);
        let pixel = pixel_angle as f32;
        for c in corners {
            // Varios puntos por arista: la cinta toma el ancho de cada tramo.
            let points: Vec<Vec3> = (0..=16)
                .map(|k| origin.lerp(c, k as f64 / 16.0).as_vec3())
                .collect();
            ribbon(mesh, &points, 2.0, pixel, color);
        }
        let rim: Vec<Vec3> = (0..=64)
            .map(|k| {
                let side = k / 16;
                let (a, b) = (corners[side % 4], corners[(side + 1) % 4]);
                a.lerp(b, (k % 16) as f64 / 16.0).as_vec3()
            })
            .collect();
        ribbon(mesh, &rim, 2.0, pixel, color);
        mesh.update_bounds();
    }

    /// Mueve la nave (y la camara) `offset` en el mundo.
    pub fn nudge(&mut self, offset: DVec3) {
        self.ship.world_pos += offset;
        self.ship.rel_pos += offset;
        self.ship_prev = self.ship;
    }

    /// La orientacion de la mira (hacia donde mira la camara).
    pub fn aim(&self) -> DQuat {
        self.aim
    }

    /// Congela o suelta la camara del culling (F7).
    pub fn toggle_freeze(&mut self) {
        self.freeze = match self.freeze {
            Some(_) => None,
            None => {
                let (ship_pos, ship_rot) = (self.ship.world_pos, self.ship.rot);
                Some(self.camera(ship_pos, ship_rot, self.sim_time))
            }
        };
    }

    /// Cinta con la caida libre predicha de la nave: cian mientras sigue en
    /// el marco de su cuerpo, naranja despues de cambiar de region, y un aro
    /// rojo donde chocaria.
    fn build_path(&mut self, t: f64, camera: &Camera, pixel_angle: f64) {
        let mesh = &mut self.path_mesh;
        *mesh = Mesh::default();
        let show = self.path_mode.unwrap_or(!self.settings.assist);
        self.path_shown = show;
        self.path_impact = false;
        if !show {
            return;
        }
        let bodies = &self.system.bodies;
        let Prediction { legs, impact } =
            trajectory::predict(&self.ship, bodies, t, self.settings.gravity);
        let pixel = pixel_angle as f32;
        for (k, leg) in legs.iter().enumerate() {
            let base = bodies[leg.frame].position(t) - camera.pos;
            let points: Vec<Vec3> = leg.points.iter().map(|p| (base + *p).as_vec3()).collect();
            let color = if k == 0 {
                Vec3::new(0.2, 0.85, 1.0)
            } else {
                Vec3::new(1.0, 0.55, 0.15)
            };
            ribbon(mesh, &points, 2.5, pixel, color * 1.4);
        }
        if let (Some(hit), Some(leg)) = (impact, legs.last()) {
            self.path_impact = true;
            let base = bodies[leg.frame].position(t) - camera.pos;
            let up = hit.normalize();
            let (a, b) = {
                let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let a = helper.cross(up).normalize();
                (a, up.cross(a))
            };
            let ring: Vec<Vec3> = (0..=32)
                .map(|i| {
                    let angle = i as f64 / 32.0 * std::f64::consts::TAU;
                    (base + hit + up * 0.5 + (a * angle.cos() + b * angle.sin()) * 4.0).as_vec3()
                })
                .collect();
            ribbon(mesh, &ring, 3.0, pixel, Vec3::new(2.5, 0.3, 0.2));
        }
        mesh.update_bounds();
    }

    /// Particulas del frame: el chorro de brasas de Pira, las esporas de
    /// Jardin si la camara esta cerca de su suelo y el fuego de los motores.
    #[allow(clippy::too_many_arguments)]
    fn emit_particles(
        &mut self,
        t: f64,
        camera: &Camera,
        ship_pos: DVec3,
        ship_rot: DQuat,
        show_ship: bool,
    ) {
        let blank = Particle {
            pos: Vec3::ZERO,
            radius: 0.0,
            color: Vec3::ZERO,
        };
        let mut out = std::mem::take(&mut self.renderer.particles);
        out.clear();
        let system = &self.system;
        let star = &system.bodies[0];
        if self.cycle.star_glow > 0.001 && star.radius > 1.0 {
            let start = out.len();
            out.resize(start + effects::PROMINENCE_COUNT, blank);
            effects::prominences(
                &mut out[start..],
                star,
                t,
                camera.pos,
                self.cycle.star_glow,
                self.cycle.flares,
            );
        }

        if let Some((top, up)) = beacon_lamp(system, t) {
            let start = out.len();
            out.resize(start + effects::BEACON_COUNT, blank);
            let near = smoothstep(300.0, 60.0, (top - ship_pos).length() as f32);
            effects::beacon_waves(
                &mut out[start..],
                (top - camera.pos).as_vec3(),
                up.as_vec3(),
                t,
                shaders::BEACON_PERIOD,
                near,
            );
        }
        // Lo que flota alrededor de la camara cerca del suelo de cada planeta.
        for (name, kind, top) in [
            ("Pira", effects::Motes::Embers, 120.0),
            ("Requiem", effects::Motes::Snow, 120.0),
            ("Cementerio", effects::Motes::Dust, 100.0),
            ("Vigia", effects::Motes::Mist, 260.0),
            ("Parpado", effects::Motes::Glints, 90.0),
        ] {
            let Some(i) = system.find(name) else { continue };
            let body = &system.bodies[i];
            let altitude = (camera.pos - body.position(t)).length() - body.radius;
            if altitude < top && !body.hidden {
                let start = out.len();
                out.resize(start + effects::MOTE_COUNT, blank);
                let strength = smoothstep(top as f32, top as f32 * 0.4, altitude as f32);
                effects::motes(&mut out[start..], body, t, camera.pos, kind, strength);
            }
        }
        // Los geiseres cerca de la camara echan lava; los naufragios
        // despiertos, chispas.
        let ship_rel = (ship_pos - camera.pos).as_vec3();
        for (mouth, up, activity, seed) in vents_near(system, t, camera.pos, ship_pos) {
            if activity > 0.05 {
                let start = out.len();
                out.resize(start + effects::FOUNTAIN_COUNT, blank);
                effects::fountain(&mut out[start..], mouth, up, t, activity, seed);
            }
        }
        for (at, up, seed) in wrecks_near(system, t, camera.pos) {
            let near = smoothstep(160.0, 30.0, (at - ship_rel).length());
            if near > 0.05 {
                let start = out.len();
                out.resize(start + effects::SPARK_COUNT, blank);
                effects::sparks(&mut out[start..], at, up, t, near, seed);
            }
        }
        if let Some(i) = system.find("Jardin") {
            let jardin = &system.bodies[i];
            let altitude = (camera.pos - jardin.position(t)).length() - jardin.radius;
            if altitude < 120.0 {
                let start = out.len();
                out.resize(start + SPORE_COUNT, blank);
                let night = effects::night_at(jardin, star, camera.pos, t);
                effects::spores(&mut out[start..], jardin, t, camera.pos, night, 1.0);
            }
        }
        if show_ship && self.throttle > 0.01 {
            let start = out.len();
            out.resize(start + EXHAUST_COUNT, blank);
            effects::exhaust(
                &mut out[start..],
                ship_pos,
                ship_rot,
                self.throttle,
                config::SHIP_LENGTH,
                t,
                camera.pos,
            );
        }
        self.renderer.particles = out;
    }

    /// Cada planeta con terreno elige sus chunks segun la camara. El recorrido
    /// trabaja en el espacio del planeta: la camara se lleva a ese espacio y
    /// las esferas de los nodos se llevan de vuelta para probarlas contra el
    /// frustum.
    fn update_terrain(&mut self, t: f64, camera: &Camera, view: &View) {
        let bodies = &self.system.bodies;
        for (body, terrain) in bodies.iter().zip(self.system.terrains.iter_mut()) {
            let Some(terrain) = terrain else { continue };
            let rotation = body.rotation(t);
            let offset = body.position(t) - camera.pos;
            let visible = |center: DVec3, radius: f64| {
                view.sphere_visible((offset + rotation.rotate(center)).as_vec3(), radius as f32)
            };
            let viewer = Viewer {
                position: rotation.conjugate().rotate(-offset),
                visible: &visible,
            };
            terrain.update(&viewer, &self.lod);
        }
    }

    fn camera(&self, ship_pos: DVec3, ship_rot: DQuat, t: f64) -> Camera {
        let look = DQuat::from_axis_angle(DVec3::Y, self.look_yaw)
            * DQuat::from_axis_angle(DVec3::X, self.look_pitch);
        let rot = (self.aim * look).normalize();
        let mut pos = match self.camera_mode {
            CameraMode::Chase => {
                let offset = DVec3::new(2.2, config::CHASE_HEIGHT, config::CHASE_DISTANCE);
                ship_pos + rot.rotate(offset * self.zoom)
            }
            CameraMode::Cockpit => {
                let [x, y, z] = config::COCKPIT_OFFSET;
                ship_pos + ship_rot.rotate(DVec3::new(x, y, z))
            }
        };
        // La camara nunca se mete bajo el terreno, aunque la nave vuele pegada.
        let bodies = &self.system.bodies;
        let (index, altitude) = nearest_surface(bodies, pos, t);
        if altitude < config::CAMERA_CLEARANCE {
            let center = bodies[index].position(t);
            let offset = pos - center;
            pos = center
                + offset.normalize() * (offset.length() + config::CAMERA_CLEARANCE - altitude);
        }
        Camera {
            pos,
            rot,
            fov_y: self
                .cinematic_fov
                .unwrap_or(config::FOV_Y_DEGREES)
                .to_radians(),
            near: config::NEAR_PLANE,
        }
    }

    /// Coloca la nave y la camara en una vista fija.
    pub fn apply_preset(&mut self, preset: &Preset) {
        let t = preset.time;
        self.timeline = Timeline::at(t, preset.act);
        self.cinematic_fov = if matches!(preset.shot, Shot::BlackHole) {
            Some(34.0)
        } else {
            None
        };
        let bodies = &self.system.bodies;
        // Ejes alrededor de un planeta: hacia la estrella, de lado y arriba.
        let frame = |index: usize| {
            let center = bodies[index].position(t);
            let to_star = (bodies[0].position(t) - center).normalize();
            let side = to_star.cross(DVec3::Y).normalize();
            let up = side.cross(to_star).normalize();
            (center, to_star, side, up)
        };
        let around = |azimuth: f64, elevation: f64, to_star: DVec3, side: DVec3, up: DVec3| {
            (to_star * azimuth.cos() + side * azimuth.sin()) * elevation.cos()
                + up * elevation.sin()
        };
        let index = |name: &str| self.system.find(name).unwrap_or(1);
        let (pos, forward, view_up, zoom, look) = match preset.shot {
            Shot::Star => {
                let i = index("Pira");
                let (center, to_star, side, up) = frame(i);
                let pos = center - to_star * 180.0 + side * 350.0 + up * 100.0;
                let target = bodies[0].position(t) + side * 180.0;
                (pos, target - pos, up, 0.8, (0.0, 0.0))
            }
            Shot::BlackHole => {
                let tilt = DQuat::from_axis_angle(DVec3::Z, bodies[0].axial_tilt);
                let pos = tilt.rotate(DVec3::new(
                    3.7_f64.sin() * 0.035_f64.cos(),
                    0.035_f64.sin(),
                    3.7_f64.cos() * 0.035_f64.cos(),
                )) * (21.5 * cycle::GARGANTUA_RS);
                (pos, -pos, tilt.rotate(DVec3::Y), 0.85, (0.0, 0.0))
            }
            Shot::Garden => {
                let i = index("Jardin");
                let body = &bodies[i];
                let (n, f, _) = self.system.garden_grove_frame();
                let rotation = body.rotation(t);
                let normal = rotation.rotate(n);
                let forward = rotation.rotate(f);
                let ground = body.surface_height(normal, t);
                let pos = body.position(t) + normal * (body.radius + ground + 6.5);
                (pos, forward + normal * 0.05, normal, 1.1, (0.0, 0.0))
            }
            Shot::Site {
                body,
                altitude,
                back,
                lift,
            } => {
                let i = index(body);
                let b = &bodies[i];
                let site = self.system.sites.iter().find(|s| s.body == i);
                let (n, f, _) = site.map_or((DVec3::Y, DVec3::Z, DVec3::X), |s| s.frame());
                let rotation = b.rotation(t);
                let normal = rotation.rotate(n);
                let forward = rotation.rotate(f);
                let center = b.position(t) + normal * (b.radius + b.surface_height(normal, t));
                let pos = center + normal * altitude - forward * back;
                let target = center + forward * 45.0 + normal * (altitude * 0.25 + lift);
                (pos, target - pos, normal, 1.0, (0.0, 0.0))
            }
            Shot::Overview => {
                // Desde arriba y de lado, mirando un poco mas alla de la estrella.
                let pos = DVec3::new(-2_500.0, 4_200.0, 6_800.0);
                let target = DVec3::new(600.0, 0.0, -1_500.0);
                (pos, target - pos, DVec3::Y, 1.0, (0.0, 0.0))
            }
            Shot::Orbit {
                body,
                distance,
                azimuth,
                elevation,
            } => {
                let i = index(body);
                let (center, to_star, side, up) = frame(i);
                let dir = around(azimuth, elevation, to_star, side, up);
                let pos = center + dir * (bodies[i].radius * distance);
                (pos, center - pos, up, 1.0, (0.0, 0.0))
            }
            Shot::Surface {
                body,
                azimuth,
                elevation,
                altitude,
            } => {
                let i = index(body);
                let (center, to_star, side, up) = frame(i);
                let normal = around(azimuth, elevation, to_star, side, up);
                let ground = bodies[i].surface_height(normal, t);
                let pos = center + normal * (bodies[i].radius + ground + altitude);
                // Mirar al horizonte, hacia la estrella si se puede.
                let mut horizon = to_star - normal * to_star.dot(normal);
                if horizon.length_squared() < 1e-6 {
                    horizon = side;
                }
                (
                    pos,
                    horizon.normalize() * 0.95 - normal * 0.12,
                    normal,
                    0.6,
                    (0.0, 0.0),
                )
            }
            Shot::Ship { body } => {
                let i = index(body);
                let (center, to_star, side, up) = frame(i);
                let dir = (side * 0.6 + to_star * 0.7 + up * 0.25).normalize();
                let pos = center + dir * (bodies[i].radius * 2.2);
                (pos, center - pos, up, 0.55, (0.7, -0.2))
            }
            Shot::Coast { body, distance, .. } => {
                // De costado al planeta, en el plano de su orbita, mirando
                // hacia donde va: la orbita queda acostada frente a la camara.
                let i = index(body);
                let (center, to_star, side, up) = frame(i);
                let pos = center + side * (bodies[i].radius * distance);
                (pos, to_star, up, 3.5, (0.35, -0.45))
            }
            Shot::Gaze { body } => {
                let i = index(body);
                let (center, to_star, side, up) = frame(i);
                let r = bodies[i].radius;
                // Atras y de lado: el planeta queda en un tercio de la imagen
                // y el centro del sistema en el medio.
                let pos = center - to_star * (r * 3.0) + up * (r * 0.5) + side * (r * 1.9);
                let target = bodies[0].position(t);
                (pos, target - pos, up, 1.0, (0.0, 0.0))
            }
            Shot::Stream => {
                // Por encima del plano de la orbita: Pira, su cola y la estrella.
                let i = index("Pira");
                let (center, to_star, side, up) = frame(i);
                let star = center + to_star * (center.length());
                let middle = (center + star) * 0.5;
                let pos = middle + up * 900.0 + side * 700.0 - to_star * 250.0;
                (pos, middle - pos, up, 1.0, (0.0, 0.0))
            }
            Shot::Landmark {
                body,
                kind,
                distance,
                sun,
            } => {
                let i = index(body);
                let (pos, target, up) = self.landmark(i, kind, distance, sun, t);
                (pos, target - pos, up, 1.0, (0.0, 0.0))
            }
        };
        // Para la vista sin asistente: hacia donde va la orbita y su gravedad.
        let coast = match preset.shot {
            Shot::Coast { body, speed, .. } => {
                let i = index(body);
                let (_, to_star, side, _) = frame(i);
                let _ = side;
                Some((to_star, bodies[i].mu, speed))
            }
            _ => None,
        };
        self.place(pos, DQuat::look_rotation(forward, view_up), t);
        self.zoom = zoom;
        (self.look_yaw, self.look_pitch) = look;
        // En la vista del sistema y frente a los objetos la nave estorbaria:
        // se mira desde la cabina.
        self.camera_mode = match preset.shot {
            Shot::Overview | Shot::Landmark { .. } | Shot::Stream | Shot::Gaze { .. } => {
                CameraMode::Cockpit
            }
            _ => CameraMode::Chase,
        };
        // Sin asistente y con velocidad de orbita: la gravedad manda.
        self.settings.assist = true;
        self.path_mode = None;
        if let Some((along, mu, speed)) = coast {
            let rel = self.ship.rel_pos;
            let r = rel.length();
            let tangent = (along - rel * (along.dot(rel) / (r * r))).normalize();
            self.ship.rel_vel = tangent * ((mu / r).sqrt() * speed);
            self.ship_prev = self.ship;
            self.settings.assist = false;
        }
    }

    /// Encuadre de un objeto de tipo `kind` del cuerpo `index`: devuelve la
    /// posicion de la camara, el punto que mira y la vertical.
    ///
    /// Entre todos los objetos de ese tipo elige el que tiene el sol mas cerca
    /// de la altura pedida y, en los monolitos, una cara con el glifo de lado
    /// al sol. La camara se para frente a esa cara, o a 60 grados del sol en
    /// los demas; si una loma tapa el objeto, prueba otros angulos alrededor.
    fn landmark(
        &self,
        index: usize,
        kind: PropKind,
        distance: f64,
        sun_height: f64,
        t: f64,
    ) -> (DVec3, DVec3, DVec3) {
        let body = &self.system.bodies[index];
        let center = body.position(t);
        let rotation = body.rotation(t);
        let to_star = (self.system.bodies[0].position(t) - center).normalize();
        // Vertical y direccion horizontal del sol.
        let axes = |p: &Instance| {
            let up = rotation.rotate(p.position.normalize());
            let sun = (to_star - up * to_star.dot(up)).normalize();
            (up, sun)
        };
        let score = |p: &Instance| (axes(p).0.dot(to_star) - sun_height).abs();
        let Some(best) = self.system.props[index]
            .instances
            .iter()
            .filter(|p| p.kind == kind)
            .min_by(|a, b| score(a).total_cmp(&score(b)))
        else {
            // Sin objetos de ese tipo: el planeta desde la orbita.
            let pos = center + to_star * (body.radius * 3.0);
            return (pos, center, DVec3::Y);
        };
        let (up, sun) = axes(best);
        let base = center + rotation.rotate(best.position);
        let model = &self.system.prop_models[kind.index()].lods[0];
        let scale = best.scale as f64;
        let side = up.cross(sun);
        let (target, directions) = {
            let around = [
                60.0f64, -60.0, 100.0, -100.0, 30.0, -30.0, 140.0, -140.0, 0.0, 180.0,
            ];
            let dirs: Vec<DVec3> = around
                .iter()
                .map(|a| sun * a.to_radians().cos() + side * a.to_radians().sin())
                .collect();
            (base + up * (model.bounds_center.y as f64 * scale), dirs)
        };
        // Altura sobre el terreno en un punto del mundo.
        let altitude = |p: DVec3| {
            let offset = p - center;
            offset.length() - body.radius - body.surface_height(offset, t)
        };
        let camera_at = |dir: DVec3| {
            let mut pos = target + dir * distance + up * (distance * 0.12);
            let lift = 2.5 - altitude(pos);
            if lift > 0.0 {
                pos += up * lift;
            }
            pos
        };
        // Linea de vision: ningun punto del camino queda bajo el suelo.
        let clear = |pos: DVec3| (1..12).all(|k| altitude(pos.lerp(target, k as f64 / 12.0)) > 0.3);
        let clearance = |pos: DVec3| {
            let segment = target - pos;
            self.system.props[index]
                .instances
                .iter()
                .filter(|p| !std::ptr::eq(*p, best))
                .map(|p| {
                    let model = &self.system.prop_models[p.kind.index()].lods[0];
                    let c = center
                        + rotation.rotate(
                            p.position
                                + p.rotation
                                    .rotate((model.bounds_center * p.scale).as_dvec3()),
                        );
                    let t = ((c - pos).dot(segment) / segment.length_squared()).clamp(0.0, 0.9);
                    (c - pos - segment * t).length()
                        - model.bounds_radius as f64 * p.scale as f64 * 0.6
                })
                .fold(f64::INFINITY, f64::min)
        };
        let pos = directions
            .iter()
            .map(|&dir| camera_at(dir))
            .filter(|&pos| clear(pos))
            .max_by(|&a, &b| clearance(a).total_cmp(&clearance(b)))
            .unwrap_or_else(|| camera_at(directions[0]) + up * distance);
        (pos, target, up)
    }

    fn place(&mut self, pos: DVec3, rot: DQuat, t: f64) {
        self.ship = ShipState::new(&self.system.bodies, pos, rot, t);
        self.ship_prev = self.ship;
        self.aim = rot;
        // El render dibuja `sim_time - dt` cuando el acumulador esta vacio.
        self.sim_time = t + config::PHYSICS_DT;
        self.accumulator = 0.0;
        self.flight.nearest = nearest_surface(&self.system.bodies, pos, t).0;
        self.local_up = None;
        self.eye = eye_target(&self.system, pos, t);
    }
}

/// Un cuerpo en pantalla.
#[derive(Clone, Debug)]
pub struct Marker {
    pub name: &'static str,
    pub x: f32,
    pub y: f32,
    pub distance: f64,
}

/// Angulo que cubre un pixel con esta camara.
fn pixel_angle_of(camera: &Camera, height: usize) -> f64 {
    2.0 * (camera.fov_y * 0.5).tan() / height as f64
}

/// Nombres de los cuerpos que estan adelante y no llenan la pantalla (de
/// cerca el nombre estorbaria). Despues del colapso la estrella se llama
/// Gargantua.
fn markers(
    system: &System,
    timeline: &Timeline,
    t: f64,
    camera: &Camera,
    view: &View,
    width: usize,
    height: usize,
) -> Vec<Marker> {
    let pixel = pixel_angle_of(camera, height);
    let state = cycle::at(t, timeline, system.star_base);
    system
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, body)| !body.hidden)
        .filter_map(|(i, body)| {
            let rel = body.position(t) - camera.pos;
            let distance = rel.length();
            let clip = view.view_proj.transform_point(rel.as_vec3());
            if clip.w <= view.near {
                return None;
            }
            let x = (clip.x / clip.w + 1.0) * 0.5 * width as f32;
            let y = (1.0 - clip.y / clip.w) * 0.5 * height as f32;
            let on_screen = x > 0.0 && y > 0.0 && x < width as f32 && y < height as f32;
            let size = body.radius / (distance * pixel);
            if !on_screen || size > height as f64 * 0.2 {
                return None;
            }
            let name = if i == 0 && state.rs > 0.0 {
                "Gargantua"
            } else {
                body.name
            };
            Some(Marker {
                name,
                x,
                y: y - (size as f32).min(60.0),
                distance: (distance - body.radius).max(0.0),
            })
        })
        .chain(system.sites.iter().filter_map(|site| {
            // Los lugares se marcan cuando se esta cerca de su planeta y del
            // lado donde estan.
            let body = &system.bodies[site.body];
            let center = body.position(t);
            let normal = body.rotation(t).rotate(site.normal);
            let at = center + normal * (body.radius + body.surface_height(normal, t) + 10.0);
            let rel = at - camera.pos;
            let distance = rel.length();
            if distance > body.radius * 6.0 || normal.dot(-rel) < 0.0 {
                return None;
            }
            let clip = view.view_proj.transform_point(rel.as_vec3());
            if clip.w <= view.near {
                return None;
            }
            let x = (clip.x / clip.w + 1.0) * 0.5 * width as f32;
            let y = (1.0 - clip.y / clip.w) * 0.5 * height as f32;
            let on_screen = x > 0.0 && y > 0.0 && x < width as f32 && y < height as f32;
            on_screen.then_some(Marker {
                name: site.name,
                x,
                y,
                distance,
            })
        }))
        .collect()
}

/// La cola de polvo de Pira: sale del lado de noche, lejos de la estrella,
/// y se curva hacia atras en la orbita.
fn pira_tail(system: &System, t: f64, camera: DVec3, light: Vec3) -> Option<shaders::Tail> {
    let i = system.find("Pira")?;
    let pira = &system.bodies[i];
    if pira.hidden {
        return None;
    }
    let center = pira.position(t);
    let star = system.bodies[0].position(t);
    let away = (center - star).normalize();
    let lag = -pira.velocity(t).normalize();
    // La parte de atras en la orbita que es perpendicular a `away`.
    let lag = (lag - away * lag.dot(away)).normalize();
    Some(shaders::Tail {
        origin: (center - camera).as_vec3(),
        away: away.as_vec3(),
        lag: lag.as_vec3(),
        length: (pira.radius * 9.0) as f32,
        drift: (pira.radius * 4.0) as f32,
        planet_radius: pira.radius as f32,
        star: (star - camera).as_vec3(),
        light,
        time: t as f32,
    })
}

/// Los objetos de `kind` de un cuerpo a menos de `reach` de la camara: el
/// punto a `lift` unidades del modelo sobre su base (relativo a la camara),
/// la vertical ahi y un numero propio de cada uno.
fn props_near(
    system: &System,
    body: &str,
    kind: PropKind,
    lift: f32,
    t: f64,
    camera: DVec3,
    reach: f64,
) -> Vec<(Vec3, Vec3, u32, f32)> {
    let Some(i) = system.find(body) else {
        return Vec::new();
    };
    let b = &system.bodies[i];
    if b.hidden {
        return Vec::new();
    }
    let center = b.position(t);
    if (center - camera).length() > b.radius + reach * 2.0 {
        return Vec::new();
    }
    let rotation = b.rotation(t);
    system.props[i]
        .instances
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind == kind)
        .filter_map(|(index, p)| {
            let top = p.position + p.rotation.rotate(DVec3::Y * (lift * p.scale) as f64);
            let world = center + rotation.rotate(top);
            ((world - camera).length() < reach).then(|| {
                (
                    (world - camera).as_vec3(),
                    rotation.rotate(p.position.normalize()).as_vec3(),
                    hash_u32(index as u32 ^ 0x5EED),
                    p.scale,
                )
            })
        })
        .collect()
}

/// Bocas de los geiseres de Pira cerca de la camara, su vertical, su
/// actividad y su numero. La nave las despierta.
fn vents_near(system: &System, t: f64, camera: DVec3, ship: DVec3) -> Vec<(Vec3, Vec3, f32, u32)> {
    let ship = (ship - camera).as_vec3();
    props_near(system, "Pira", PropKind::Vent, props::VENT_TOP, t, camera, 320.0)
        .into_iter()
        .map(|(mouth, up, seed, _)| {
            let unit = (seed >> 8) as f32 / 16_777_216.0;
            let activity = shaders::vent_activity(t as f32, unit, (mouth - ship).length());
            (mouth, up, activity, seed)
        })
        .collect()
}

/// Naufragios de Cementerio cerca de la camara: un punto sobre el casco, la
/// vertical y su numero.
fn wrecks_near(system: &System, t: f64, camera: DVec3) -> Vec<(Vec3, Vec3, u32)> {
    props_near(system, "Cementerio", PropKind::Wreck, 4.5, t, camera, 220.0)
        .into_iter()
        .map(|(at, up, seed, _)| (at, up, seed))
        .collect()
}

/// La lampara de la baliza de Cementerio en el mundo y la vertical ahi.
fn beacon_lamp(system: &System, t: f64) -> Option<(DVec3, DVec3)> {
    let i = system.find("Cementerio")?;
    let body = &system.bodies[i];
    let beacon = system.props[i]
        .instances
        .iter()
        .find(|p| p.kind == PropKind::Beacon)?;
    let rotation = body.rotation(t);
    let up = rotation.rotate(beacon.position.normalize());
    let lamp = beacon.position
        + beacon
            .rotation
            .rotate(DVec3::Y * (props::BEACON_TOP * beacon.scale) as f64);
    Some((body.position(t) + rotation.rotate(lamp), up))
}

/// Direccion de la nave vista desde el centro de Vigia, en su propio espacio:
/// hacia ahi quiere mirar el ojo.
fn eye_target(system: &System, ship_pos: DVec3, t: f64) -> DVec3 {
    let body = &system.bodies[system.eye_body];
    let toward = (ship_pos - body.position(t)).normalize();
    body.rotation(t).conjugate().rotate(toward)
}

/// Atmosferas que pueden verse en este frame, de la mas lejana a la mas
/// cercana: se componen de atras hacia adelante. Las que caen fuera del
/// frustum no le cuestan nada a ningun pixel.
fn atmosphere_frames<'a>(
    system: &'a System,
    t: f64,
    camera: &Camera,
    view: &View,
    melting: Option<usize>,
) -> Vec<AtmosphereFrame<'a>> {
    let star = system.bodies[0].position(t);
    let mut frames: Vec<(f64, AtmosphereFrame<'a>)> = system
        .bodies
        .iter()
        .zip(&system.atmospheres)
        .enumerate()
        .filter_map(|(index, (body, model))| {
            let model = model.as_ref()?;
            // Mientras Gargantua la deshace, Pira pierde su atmosfera.
            if body.hidden || melting == Some(index) {
                return None;
            }
            let position = body.position(t);
            let center = position - camera.pos;
            let top = model.radius + model.params.thickness;
            if !view.sphere_visible(center.as_vec3(), top) {
                return None;
            }
            let frame = AtmosphereFrame {
                model,
                center: center.as_vec3(),
                sun_dir: (star - position).normalize().as_vec3(),
            };
            Some((center.length(), frame))
        })
        .collect();
    frames.sort_by(|a, b| b.0.total_cmp(&a.0));
    frames.into_iter().map(|(_, frame)| frame).collect()
}

/// Anillos de los cuerpos que los tienen, relativos a la camara.
fn ring_info(system: &System, t: f64, camera: DVec3) -> Vec<RingInfo> {
    system
        .bodies
        .iter()
        .filter(|body| !body.hidden)
        .filter_map(|body| {
            let rings = body.rings?;
            let normal = body.rotation(t).rotate(DVec3::Y);
            // Ejes del plano que no giran con el planeta: los restos orbitan
            // solos.
            let axis_a = normal.cross(DVec3::Z).normalize();
            Some(RingInfo {
                style: rings.style,
                center: (body.position(t) - camera).as_vec3(),
                normal: normal.as_vec3(),
                axis_a: axis_a.as_vec3(),
                axis_b: normal.cross(axis_a).as_vec3(),
                inner: (rings.inner * body.radius) as f32,
                outer: (rings.outer * body.radius) as f32,
                planet_radius: body.radius as f32,
            })
        })
        .collect()
}

/// Un dibujo por objeto de la superficie visible. Cada objeto vive en el
/// espacio de su planeta: su matriz es la del planeta por la suya, y la
/// traslacion se resta en `f64` como en todo lo demas. Si la superficie
/// respira (Jardin), el objeto sube y baja con ella.
#[allow(clippy::too_many_arguments)]
fn prop_draws<'a>(
    system: &'a System,
    t: f64,
    camera: &Camera,
    cull_camera: &Camera,
    cull_view: &View,
    pixel_angle: f64,
    light_radius: f64,
    melting: Option<usize>,
    draws: &mut Vec<Draw<'a>>,
) -> PropStats {
    let mut stats = PropStats::default();
    let mut chosen = Vec::new();
    let star = &system.bodies[0];
    let (star_pos, star_radius) = (star.position(t), light_radius);
    for (index, (body, set)) in system.bodies.iter().zip(&system.props).enumerate() {
        let Some(field) = &body.surface else { continue };
        // Lo que hay sobre Pira cae primero cuando Gargantua la deshace.
        if set.instances.is_empty() || body.hidden || melting == Some(index) {
            continue;
        }
        let quat = body.rotation(t);
        let rotation = quat.to_mat3();
        let body_pos = body.position(t);
        let center = body_pos - camera.pos;
        let (low, high) = field.height_range();
        // La seleccion se hace desde la camara del culling (la congelada si
        // la hay); los dibujos, relativos a la camara de verdad.
        let cull_center = body_pos - cull_camera.pos;
        let visible = |c: DVec3, r: f64| {
            cull_view.sphere_visible((cull_center + rotation.mul_vec(c)).as_vec3(), r as f32)
        };
        let prop_view = PropView {
            camera: quat.conjugate().rotate(-cull_center),
            occluder: body.radius + low as f64,
            surface_top: body.radius + high as f64,
            pixel_angle,
            visible: &visible,
        };
        chosen.clear();
        props::select(
            set,
            &system.prop_models,
            &prop_view,
            &mut stats,
            &mut chosen,
        );
        for pick in &chosen {
            let instance = &set.instances[pick.index];
            let mesh = &system.prop_models[instance.kind.index()].lods[pick.lod];
            // Los modelos nacen en su base: al encogerse se hunden en el suelo.
            let scale = (instance.scale * pick.grow) as f64;
            let lift = shaders::displace(
                &body.material,
                instance.position.as_vec3(),
                t as f32,
                &system.noise,
            );
            let position = instance.position + instance.position.normalize() * lift as f64;
            let rot = rotation.mul(&instance.rotation.to_mat3());
            let linear = rot.scale(scale);
            let offset = center + rotation.mul_vec(position);
            let disc = (star_radius / (star_pos - body_pos).length())
                .min(1.0)
                .asin() as f32;
            draws.push(Draw {
                mesh,
                indices: &mesh.indices,
                local_offset: Vec3::ZERO,
                model: DMat4::from_affine(&linear, offset).to_f32(),
                normal: rot.to_f32(),
                material: Material::Prop(PropStyle {
                    kind: instance.kind,
                    scale: scale as f32,
                    planet: center.as_vec3(),
                    // La esfera que hace sombra pasa por el suelo de este objeto:
                    // el terreno de alrededor tapa la estrella a esa altura.
                    occluder: instance.position.length() as f32,
                    disc,
                    seed: (hash_u32(pick.index as u32 ^ 0x5EED) >> 8) as f32 / 16_777_216.0,
                }),
                bounds_center: (offset + linear.mul_vec(mesh.bounds_center.as_dvec3()))
                    .as_vec3(),
                bounds_radius: mesh.bounds_radius * scale as f32,
                tag: pick.lod as u32 + 1,
            });
        }
    }
    stats
}

/// Lo que queda de Pira mientras hierve: se encoge entera (sin deformarse)
/// dentro del vapor que la tapa.
#[derive(Clone, Copy)]
struct Shrink {
    body: usize,
    scale: f64,
}

/// Un dibujo por chunk de terreno visible, uno por cuerpo sin terreno y otro
/// para la nave. Las matrices de modelo llevan la posicion **relativa a la
/// camara**, restada en `f64`.
fn build_draws<'a>(
    system: &'a System,
    t: f64,
    camera: &Camera,
    ship_pos: DVec3,
    ship_rot: DQuat,
    show_ship: bool,
    star_radius: f64,
    star_material: Material,
    shrink: Option<Shrink>,
) -> Vec<Draw<'a>> {
    let mut draws = Vec::new();
    for (index, (body, terrain)) in system.bodies.iter().zip(&system.terrains).enumerate() {
        if body.hidden {
            continue;
        }
        let rotation = body.rotation(t).to_mat3();
        let center = body.position(t) - camera.pos;
        match terrain {
            Some(terrain) => {
                let normal = rotation.to_f32();
                let scale = shrink.filter(|s| s.body == index).map_or(1.0, |s| s.scale);
                let linear = rotation.scale(scale);
                for chunk in terrain.selected() {
                    let offset = center + linear.mul_vec(chunk.center);
                    draws.push(Draw {
                        mesh: &chunk.mesh,
                        indices: terrain.indices(),
                        local_offset: chunk.center.as_vec3(),
                        model: DMat4::from_affine(&linear, offset).to_f32(),
                        normal,
                        material: body.material,
                        bounds_center: offset.as_vec3(),
                        bounds_radius: chunk.radius * scale as f32,
                        tag: chunk.key.level as u32 + 1,
                    });
                }
            }
            None => {
                let mesh = &system.meshes[body.mesh];
                // La estrella toma su radio y su material del ciclo; despues
                // del colapso ya no hay nada que dibujar.
                let (radius, material) = if index == 0 {
                    (star_radius, star_material)
                } else {
                    (body.render_radius(t), body.material)
                };
                if radius < 0.5 {
                    continue;
                }
                draws.push(Draw {
                    mesh,
                    indices: &mesh.indices,
                    local_offset: Vec3::ZERO,
                    model: DMat4::from_affine(&rotation.scale(radius), center).to_f32(),
                    normal: rotation.to_f32(),
                    material,
                    bounds_center: center.as_vec3(),
                    bounds_radius: (radius * mesh.bounds_radius as f64) as f32,
                    tag: 0,
                });
            }
        }
    }
    if show_ship {
        let distance = (ship_pos - camera.pos).length();
        let lod = if distance < 5.5 {
            0
        } else if distance < 35.0 {
            1
        } else {
            2
        };
        let mesh = &system.meshes[system.ship_mesh[lod]];
        let rotation = (ship_rot * system.ship_fix).to_mat3();
        let offset = ship_pos - camera.pos;
        draws.push(Draw {
            mesh,
            indices: &mesh.indices,
            local_offset: Vec3::ZERO,
            model: DMat4::from_affine(&rotation, offset).to_f32(),
            normal: rotation.to_f32(),
            material: system.ship_material,
            bounds_center: (offset + rotation.mul_vec(mesh.bounds_center.as_dvec3())).as_vec3(),
            bounds_radius: mesh.bounds_radius,
            tag: 0,
        });
    }
    draws
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_frames_keep_the_cycle_clock_in_real_time() {
        let mut app = App::new(64, 36).unwrap();
        let start = app.time();
        for dt in [0.4, 0.7, 0.9] {
            app.update(dt, &FrameInput::default());
        }
        assert!(
            (app.time() - start - 2.0).abs() < config::PHYSICS_DT * 1.1,
            "dos segundos de frames lentos deben avanzar dos segundos del ciclo"
        );
    }

    #[test]
    fn crossing_the_horizon_kills_the_ship_and_rewinds_time() {
        let mut app = App::new(64, 36).unwrap();
        // Gargantua ya formada: la J fue hace dos minutos.
        let now = app.time();
        app.timeline = Timeline::at(now, Some(120.0));
        // Unos segundos lejos del agujero, para tener historia.
        let still = FrameInput::default();
        for _ in 0..240 {
            app.update(1.0 / 60.0, &still);
        }
        let before = app.time();
        // Y de pronto, junto al horizonte.
        let hole = app.system.bodies[0].position(before);
        app.ship.world_pos = hole + DVec3::X * cycle::GARGANTUA_RS;
        app.ship.rel_pos = app.ship.world_pos - app.system.bodies[app.ship.frame].position(before);
        app.update(1.0 / 60.0, &still);
        assert!(app.dying.is_some(), "deberia morir al cruzar el horizonte");
        for _ in 0..120 {
            app.update(1.0 / 60.0, &still);
        }
        assert!(app.dying.is_none());
        assert!(app.time() < before, "el tiempo se rebobina: {} -> {}", before, app.time());
        let distance = (app.ship.world_pos - app.system.bodies[0].position(app.time())).length();
        assert!(distance > 3.0 * cycle::GARGANTUA_RS, "la nave vuelve lejos: {distance}");
    }

    #[test]
    fn recovery_preserves_cycle_time_and_clears_velocity() {
        let mut app = App::new(64, 36).unwrap();
        app.skip(835.0);
        app.ship.rel_vel = DVec3::X * 2000.0;
        let time = app.time();
        app.return_to_system();
        assert!((app.time() - time).abs() < 0.02);
        assert!(app.ship.rel_vel.length() < 0.001);
        assert!(app.settings.assist && !app.settings.cruise);
        assert!((app.ship.world_pos - app.system.bodies[1].position(time)).length() < 500.0);
    }

    /// La app completa, sin ventana: en el modo local, mantener W rodea el
    /// planeta con la mira nivelada y la altura sostenida.
    #[test]
    fn holding_forward_in_local_mode_circles_the_planet_level() {
        let mut app = App::new(64, 36).unwrap();
        let preset = Preset {
            name: "prueba",
            shot: Shot::Surface {
                body: "Jardin",
                azimuth: 0.4,
                elevation: 0.2,
                altitude: 20.0,
            },
            time: 40.0,
            act: None,
        };
        app.apply_preset(&preset);
        let jardin = app.system.find("Jardin").unwrap();
        let start = app.ship.world_pos - app.system.bodies[jardin].position(app.sim_time);
        let input = FrameInput {
            thrust: DVec3::new(0.0, 0.0, -1.0),
            ..FrameInput::default()
        };
        for _ in 0..(30 * 60) {
            app.update(1.0 / 60.0, &input);
        }
        let body = &app.system.bodies[jardin];
        let rel = app.ship.world_pos - body.position(app.sim_time);
        let up = rel.normalize();
        assert!(app.flight.local, "el modo local deberia estar activo");
        // La mira no tiene alabeo: su derecha es horizontal.
        assert!(
            app.aim.right().dot(up).abs() < 0.02,
            "alabeo {}",
            app.aim.right().dot(up)
        );
        // Avanzo un buen trecho sobre la superficie y sigue a su altura.
        let traveled = rel
            .normalize()
            .dot(start.normalize())
            .clamp(-1.0, 1.0)
            .acos()
            * body.radius;
        assert!(traveled > 100.0, "recorrio {traveled}");
        let altitude = rel.length() - body.radius - body.surface_height(rel, app.sim_time);
        assert!(altitude > 8.0 && altitude < 40.0, "altitud {altitude}");
    }

    /// Entrar al modo local mirando de frente al suelo (lo normal al volar
    /// hacia un planeta) no deja la mira trabada: el mouse la levanta.
    #[test]
    fn looking_straight_down_in_local_mode_can_still_look_up() {
        let mut app = App::new(64, 36).unwrap();
        let preset = Preset {
            name: "prueba",
            shot: Shot::Surface {
                body: "Jardin",
                azimuth: 0.4,
                elevation: 0.2,
                altitude: 30.0,
            },
            time: 40.0,
            act: None,
        };
        app.apply_preset(&preset);
        let jardin = app.system.find("Jardin").unwrap();
        let up =
            (app.ship.world_pos - app.system.bodies[jardin].position(app.sim_time)).normalize();
        let side = up.cross(DVec3::X).normalize();
        app.aim = DQuat::look_rotation(-up, side);
        let still = FrameInput::default();
        for _ in 0..30 {
            app.update(1.0 / 60.0, &still);
        }
        assert!(app.flight.local);
        let look_up = FrameInput {
            look: (0.0, -40.0),
            ..FrameInput::default()
        };
        for _ in 0..60 {
            app.update(1.0 / 60.0, &look_up);
        }
        let body = &app.system.bodies[jardin];
        let up = (app.ship.world_pos - body.position(app.sim_time)).normalize();
        let pitch = app.aim.forward().dot(up).asin().to_degrees();
        assert!(
            pitch > -10.0,
            "la mira deberia haber subido: {pitch} grados"
        );
    }
}
