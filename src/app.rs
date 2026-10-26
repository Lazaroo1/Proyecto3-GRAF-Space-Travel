//! Estado del juego: el sistema, la nave, la camara y el armado de cada frame.
//!
//! No sabe nada de raylib: recibe la entrada ya traducida (`FrameInput`) y
//! deja la imagen en el `Renderer`. Por eso las capturas y el benchmark
//! corren sin abrir ninguna ventana.

use std::f64::consts::PI;
use std::path::PathBuf;

use crate::config;
use crate::math::{smoothing, DMat3, DMat4, DQuat, DVec3, Vec3};
use crate::render::{Draw, Particle, Renderer, Ripple, Stats, View};
use crate::scene::camera::{Camera, CameraMode};
use crate::scene::effects::{self, EXHAUST_COUNT, SPORE_COUNT, STREAM_COUNT};
use crate::scene::props::{self, Instance, PropKind, PropStats, PropView};
use crate::scene::ship::{self, Controls, FlightInfo, FlightSettings, ShipState};
use crate::scene::trajectory::{self, Prediction};
use crate::scene::{nearest_surface, ribbon, Mesh, System};
use crate::audio::AudioParams;
use crate::math::blackbody::planckian_rgb;
use crate::math::geodesic::LensTable;
use crate::scene::cycle::{self, CycleState};
use crate::shaders::{
    self, heartbeat_phase, AtmosphereFrame, BlackHole, DebugView, Gains, Material, PlanckTable, PropStyle, RingInfo,
    SkyView, SunStyle, Uniforms,
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
    /// Mirar alrededor sin girar la nave.
    pub free_look: bool,
    /// Rueda del mouse: acerca o aleja la camara.
    pub zoom: f64,
    /// Rebobinar el tiempo (mantener R).
    pub rewind: bool,
}

/// Encuadre de una vista fija. Los angulos se miden alrededor del planeta:
/// azimut 0 es la cara que mira a la estrella, y la elevacion sube desde el
/// plano de la orbita.
#[derive(Clone, Copy, Debug)]
pub enum Shot {
    /// Todo el sistema desde arriba.
    Overview,
    /// La nave en orbita a `distance` radios, mirando al planeta.
    Orbit { body: &'static str, distance: f64, azimuth: f64, elevation: f64 },
    /// La nave sobre la superficie, mirando al horizonte.
    Surface { body: &'static str, azimuth: f64, elevation: f64, altitude: f64 },
    /// La nave de cerca, con el planeta detras.
    Ship { body: &'static str },
    /// La nave sin asistente, de costado a `distance` radios del planeta y
    /// con `speed` veces la velocidad de una orbita circular: se ve la
    /// trayectoria que predice la gravedad.
    Coast { body: &'static str, distance: f64, speed: f64 },
    /// El chorro de brasas que Pira pierde hacia la estrella, de lejos.
    Stream,
    /// Detras del planeta `body`, mirando hacia la estrella (o hacia
    /// Gargantua): el planeta de perfil adelante y el centro del sistema al
    /// fondo.
    Gaze { body: &'static str },
    /// Un objeto de la superficie de cerca. `distance` en unidades del mundo;
    /// `sun` es el seno de la altura del sol que se busca sobre su horizonte
    /// (bajo y de lado marca el relieve; bajo cero, de noche, se luce lo que
    /// brilla).
    Landmark { body: &'static str, kind: PropKind, distance: f64, sun: f64 },
}

/// Una vista fija con nombre, para capturas y para el benchmark, en un
/// instante del ciclo.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub shot: Shot,
    pub time: f64,
}

pub const PRESETS: [Preset; 21] = [
    Preset { name: "01_sistema", shot: Shot::Overview, time: 40.0 },
    Preset { name: "02_pira", shot: Shot::Orbit { body: "Pira", distance: 3.2, azimuth: 0.5, elevation: 0.2 }, time: 40.0 },
    Preset { name: "03_pira_noche", shot: Shot::Orbit { body: "Pira", distance: 2.8, azimuth: 2.6, elevation: 0.15 }, time: 40.0 },
    Preset { name: "04_jardin", shot: Shot::Orbit { body: "Jardin", distance: 3.0, azimuth: 1.2, elevation: 0.25 }, time: 40.0 },
    Preset {
        name: "05_jardin_superficie",
        shot: Shot::Surface { body: "Jardin", azimuth: 1.75, elevation: 0.3, altitude: 6.0 },
        time: 40.0,
    },
    Preset { name: "06_vigia", shot: Shot::Orbit { body: "Vigia", distance: 2.6, azimuth: 0.8, elevation: 0.1 }, time: 40.0 },
    Preset {
        name: "07_requiem",
        shot: Shot::Surface { body: "Requiem", azimuth: 1.25, elevation: 0.35, altitude: 6.0 },
        time: 40.0,
    },
    Preset {
        name: "08_cementerio",
        shot: Shot::Orbit { body: "Cementerio", distance: 5.0, azimuth: 0.9, elevation: 0.35 },
        time: 40.0,
    },
    Preset { name: "09_nave", shot: Shot::Ship { body: "Requiem" }, time: 40.0 },
    Preset {
        name: "10_monolito",
        shot: Shot::Landmark { body: "Requiem", kind: PropKind::Monolith, distance: 16.0, sun: 0.42 },
        time: 40.0,
    },
    Preset {
        name: "11_hongos",
        shot: Shot::Landmark { body: "Jardin", kind: PropKind::Mushroom, distance: 22.0, sun: -0.38 },
        time: 40.0,
    },
    Preset {
        name: "12_espinas",
        shot: Shot::Landmark { body: "Pira", kind: PropKind::Spike, distance: 26.0, sun: 0.3 },
        time: 40.0,
    },
    Preset {
        name: "13_naufragio",
        shot: Shot::Landmark { body: "Cementerio", kind: PropKind::Wreck, distance: 40.0, sun: 0.35 },
        time: 40.0,
    },
    Preset { name: "14_chorro", shot: Shot::Stream, time: 40.0 },
    Preset {
        name: "15_trayectoria",
        shot: Shot::Coast { body: "Requiem", distance: 2.2, speed: 0.85 },
        time: 40.0,
    },
    Preset {
        name: "16_arritmia",
        shot: Shot::Orbit { body: "Pira", distance: 3.4, azimuth: 0.3, elevation: 0.25 },
        time: 700.0,
    },
    Preset { name: "17_colapso", shot: Shot::Gaze { body: "Jardin" }, time: 725.2 },
    Preset { name: "17_onda_gravitacional", shot: Shot::Gaze { body: "Jardin" }, time: 726.6 },
    Preset { name: "18_gargantua", shot: Shot::Gaze { body: "Pira" }, time: 830.0 },
    Preset { name: "19_gargantua_jardin", shot: Shot::Gaze { body: "Jardin" }, time: 830.0 },
    Preset {
        name: "20_jardin_noche_eterna",
        shot: Shot::Landmark { body: "Jardin", kind: PropKind::Coral, distance: 24.0, sun: 0.3 },
        time: 850.0,
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
    /// Tabla de geodesicas de Gargantua: se rehace al cambiar la distancia.
    lens: Option<LensTable>,
    /// Camara congelada (F7): el culling y el LOD se calculan desde ella
    /// mientras se vuela por fuera para ver que se descarta.
    pub freeze: Option<Camera>,
    /// Nombres de los cuerpos en pantalla: posicion en pixeles y distancia.
    pub markers: Vec<Marker>,
    /// Cinta con las aristas del frustum congelado.
    frustum_mesh: Mesh,
    planck: PlanckTable,
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
            cycle: cycle::at(t, system_star_radius),
            lens: None,
            freeze: None,
            markers: Vec::new(),
            frustum_mesh: Mesh::default(),
            planck: PlanckTable::new(),
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
        let body = &self.system.bodies[self.ship.frame];
        self.ship.world_pos = body.position(t) + self.ship.rel_pos;
        self.ship_prev = self.ship;
        self.sim_time = t;
        self.accumulator = 0.0;
        self.local_up = None;
    }

    /// Salta al instante `t` del ciclo actual.
    pub fn jump_to_cycle(&mut self, tau: f64) {
        let start = (self.sim_time / cycle::CYCLE).floor() * cycle::CYCLE;
        self.skip(start + tau - self.sim_time);
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
        let wind = match self.system.atmospheres.get(f.nearest).and_then(|a| a.as_ref()) {
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
            _ => 36.7,
        };
        let state = cycle::at(t, star.radius);
        let heart = if state.star_radius > 0.0 { 1.0 } else { 0.0 };
        AudioParams {
            beat_phase: state.beat_phase,
            beat_rate: beat_rate(&state),
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

    /// Avanza el juego `frame_dt` segundos reales.
    pub fn update(&mut self, frame_dt: f64, input: &FrameInput) {
        let frame_dt = frame_dt.clamp(0.0, config::MAX_FRAME_DT);
        self.steer(input, frame_dt);
        let controls = Controls {
            thrust: input.thrust,
            boost: input.boost,
            aim: self.aim,
        };
        if input.rewind {
            // Rebobinar: todo el sistema es funcion del tiempo y vuelve atras;
            // la nave se queda quieta respecto a su cuerpo.
            self.skip(-config::REWIND_SPEED * frame_dt);
            return;
        }
        // Acumulador: se dan tantos pasos fijos como quepan en el tiempo real;
        // el sobrante se usa para interpolar el dibujo.
        self.accumulator += frame_dt;
        while self.accumulator >= config::PHYSICS_DT {
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
        // El fuego de los motores sigue al empuje (y al crucero).
        let cruise = if self.flight.local && self.settings.cruise { config::CRUISE_THRUST } else { 0.0 };
        let push = (input.thrust.length().min(1.0).max(cruise) * if input.boost { 1.0 } else { 0.6 }) as f32;
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
        let (mx, my) = if input.free_look { (0.0, 0.0) } else { (dx * sens, dy * sens) };
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
        if let Some(previous) = self.local_up {
            self.aim = DQuat::from_rotation_arc(previous, up) * self.aim;
        }
        self.local_up = Some(up);
        let yaw = DQuat::from_axis_angle(up, -dx);
        let mut aim = yaw * self.aim * DQuat::from_axis_angle(DVec3::X, -dy);
        // Cabeceo maximo de 75 grados: mas alla se deshace el del mouse.
        if aim.forward().dot(up).abs() > 75f64.to_radians().sin() {
            aim = yaw * self.aim;
        }
        let level = DQuat::look_rotation(aim.forward(), up);
        self.aim = aim.slerp(level, smoothing(5.0, dt)).normalize();
    }

    /// Renderiza el estado interpolado entre los dos ultimos pasos de fisica.
    pub fn render(&mut self) -> Stats {
        let dt = config::PHYSICS_DT;
        let alpha = (self.accumulator / dt).clamp(0.0, 1.0);
        let t = self.sim_time - dt * (1.0 - alpha);
        let ship_pos = self.ship_prev.world_pos.lerp(self.ship.world_pos, alpha);
        let ship_rot = self.ship_prev.rot.slerp(self.ship.rot, alpha);

        let camera = self.camera(ship_pos, ship_rot, t);
        let view = camera.view(self.renderer.width(), self.renderer.height());
        // El culling y el LOD se deciden desde la camara congelada si la hay.
        let cull_camera = self.freeze.unwrap_or(camera);
        let cull_view = cull_camera.view(self.renderer.width(), self.renderer.height());
        self.update_terrain(t, &cull_camera, &cull_view);
        self.renderer.cull = self
            .freeze
            .map(|frozen| (frozen.view(self.renderer.width(), self.renderer.height()), (camera.pos - frozen.pos).as_vec3()));
        self.build_frustum(&camera, pixel_angle_of(&camera, self.renderer.height()));
        self.markers = markers(&self.system, t, &camera, &view, self.renderer.width(), self.renderer.height());

        let show_ship = self.camera_mode == CameraMode::Chase;
        let pixel_angle = 2.0 * (camera.fov_y * 0.5).tan() / self.renderer.height() as f64;
        let star = &self.system.bodies[0];
        let state = cycle::at(t, star.radius);
        self.cycle = state;
        let beat = if state.star_radius > 0.0 { heartbeat_phase(state.beat_phase) as f32 } else { 0.0 };
        let heart = if state.star_radius > 0.0 { 1.0 } else { 0.0 };
        // La estrella del ciclo: su radio late, se hincha y colapsa.
        let star_radius = state.star_radius * (1.0 + state.pulse * beat as f64);
        let star_material = Material::Sun(SunStyle {
            temperature: state.star_temperature,
            intensity: 3.0 * state.star_intensity,
        });
        // De donde sale la luz: la estrella o, despues, el disco de Gargantua.
        let disk_light = state.disk_density * (1.0 - state.star_glow);
        let sun_light = planckian_rgb(state.star_temperature) * (1.5 * (1.0 + 0.06 * beat) * state.star_light)
            + planckian_rgb(state.disk_peak * 1.3) * (0.35 * disk_light);
        let light_radius = star_radius.max(state.disk_outer as f64 * state.rs * 0.5).max(1.0);
        let sink = star_radius.max(state.disk_outer as f64 * state.rs * 0.8);
        self.build_path(t, &camera, pixel_angle);
        self.emit_particles(t, &camera, ship_pos, ship_rot, show_ship, sink);
        let mut draws = build_draws(&self.system, t, &camera, ship_pos, ship_rot, show_ship, star_radius, star_material);
        self.prop_stats = prop_draws(
            &self.system,
            t,
            &camera,
            &cull_camera,
            &cull_view,
            pixel_angle,
            light_radius,
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
        let atmospheres = atmosphere_frames(&self.system, t, &camera, &view);
        // Si la camara esta dentro de una atmosfera, su cielo sale de una tabla.
        let sky_view = atmospheres
            .iter()
            .enumerate()
            .find(|(_, frame)| frame.center.length() < frame.model.radius + frame.model.params.thickness)
            .map(|(i, frame)| SkyView::build(i, frame, sun_light));
        let star_pos = self.system.bodies[0].position(t);
        // Gargantua: la tabla de geodesicas solo se rehace si la distancia
        // al agujero cambio lo suficiente.
        if state.rs > 0.5 {
            let observer = ((star_pos - camera.pos).length() / state.rs) as f32;
            if !self.lens.as_ref().is_some_and(|table| table.fits(observer)) {
                self.lens = Some(LensTable::build(observer));
            }
        }
        let hole = match (&self.lens, state.rs > 0.5) {
            (Some(table), true) => {
                let normal = DQuat::from_axis_angle(DVec3::Z, self.system.bodies[0].axial_tilt).rotate(DVec3::Y);
                let axis_a = normal.cross(DVec3::Z).normalize();
                let axis_b = normal.cross(axis_a);
                Some(BlackHole {
                    center: (star_pos - camera.pos).as_vec3(),
                    rs: state.rs as f32,
                    normal: normal.as_vec3(),
                    axis_a: axis_a.as_vec3(),
                    axis_b: axis_b.as_vec3(),
                    table,
                    planck: &self.planck,
                    noise: &self.system.noise,
                    time: t as f32,
                    density: state.disk_density,
                    outer: state.disk_outer,
                    peak: state.disk_peak,
                    cone_cos: table.disk_cone(state.disk_outer.max(3.5)).max(table.shadow() * 1.2).cos(),
                })
            }
            _ => None,
        };
        self.renderer.exposure = self.exposure * state.exposure;
        // La onda gravitacional sale del agujero justo despues del destello
        // y cruza la pantalla en unos tres segundos.
        let wave = state.tau - cycle::phase_start(cycle::Phase::Gargantua);
        self.renderer.ripple = None;
        if (0.0..3.0).contains(&wave) {
            let clip = view.view_proj.transform_point((star_pos - camera.pos).as_vec3());
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
        let uniforms = Uniforms {
            time: t as f32,
            beat,
            beat_phase: state.beat_phase as f32,
            beat_rate: beat_rate(&state) as f32,
            heart,
            star_glow: state.star_glow,
            black_hole: hole.as_ref(),
            sun_pos: (star_pos - camera.pos).as_vec3(),
            sun_radius: light_radius as f32,
            sun_light,
            noise: &self.system.noise,
            sky: &self.system.sky,
            textures: &self.system.textures,
            pixel_angle: pixel_angle as f32,
            eye_dir: self.eye.as_vec3(),
            ring: ring_info(&self.system, t, camera.pos),
            atmospheres: &atmospheres,
            sky_view: sky_view.as_ref(),
            biolum: self.gains.jardin * state.biolum,
            gains: self.gains,
            debug: self.debug,
        };
        self.stats = self.renderer.render(&draws, &view, &uniforms);
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
        let corners = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
        let color = Vec3::new(2.0, 1.6, 0.2);
        let pixel = pixel_angle as f32;
        for c in corners {
            // Varios puntos por arista: la cinta toma el ancho de cada tramo.
            let points: Vec<Vec3> = (0..=16).map(|k| origin.lerp(c, k as f64 / 16.0).as_vec3()).collect();
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
        let Prediction { legs, impact } = trajectory::predict(&self.ship, bodies, t);
        let pixel = pixel_angle as f32;
        for (k, leg) in legs.iter().enumerate() {
            let base = bodies[leg.frame].position(t) - camera.pos;
            let points: Vec<Vec3> = leg.points.iter().map(|p| (base + *p).as_vec3()).collect();
            let color = if k == 0 { Vec3::new(0.2, 0.85, 1.0) } else { Vec3::new(1.0, 0.55, 0.15) };
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
    fn emit_particles(&mut self, t: f64, camera: &Camera, ship_pos: DVec3, ship_rot: DQuat, show_ship: bool, sink: f64) {
        let blank = Particle {
            pos: Vec3::ZERO,
            radius: 0.0,
            color: Vec3::ZERO,
        };
        let mut out = std::mem::take(&mut self.renderer.particles);
        out.clear();
        let system = &self.system;
        let star = &system.bodies[0];
        if let Some(i) = system.find("Pira") {
            out.resize(STREAM_COUNT, blank);
            effects::pira_stream(&mut out, &system.bodies[i], star, sink, t, camera.pos);
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
            effects::exhaust(&mut out[start..], ship_pos, ship_rot, self.throttle, config::SHIP_LENGTH, t, camera.pos);
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
                let offset = DVec3::new(0.0, config::CHASE_HEIGHT, config::CHASE_DISTANCE);
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
            pos = center + offset.normalize() * (offset.length() + config::CAMERA_CLEARANCE - altitude);
        }
        Camera {
            pos,
            rot,
            fov_y: config::FOV_Y_DEGREES.to_radians(),
            near: config::NEAR_PLANE,
        }
    }

    /// Coloca la nave y la camara en una vista fija.
    pub fn apply_preset(&mut self, preset: &Preset) {
        let t = preset.time;
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
            (to_star * azimuth.cos() + side * azimuth.sin()) * elevation.cos() + up * elevation.sin()
        };
        let index = |name: &str| self.system.find(name).unwrap_or(1);
        let (pos, forward, view_up, zoom, look) = match preset.shot {
            Shot::Overview => {
                // Desde arriba y de lado, mirando un poco mas alla de la estrella.
                let pos = DVec3::new(-2_500.0, 4_200.0, 6_800.0);
                let target = DVec3::new(600.0, 0.0, -1_500.0);
                (pos, target - pos, DVec3::Y, 1.0, (0.0, 0.0))
            }
            Shot::Orbit { body, distance, azimuth, elevation } => {
                let i = index(body);
                let (center, to_star, side, up) = frame(i);
                let dir = around(azimuth, elevation, to_star, side, up);
                let pos = center + dir * (bodies[i].radius * distance);
                (pos, center - pos, up, 1.0, (0.0, 0.0))
            }
            Shot::Surface { body, azimuth, elevation, altitude } => {
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
                (pos, horizon.normalize() * 0.95 - normal * 0.12, normal, 0.6, (0.0, 0.0))
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
                // Por encima del plano de la orbita, viendo de Pira a la estrella.
                let i = index("Pira");
                let (center, to_star, side, up) = frame(i);
                let star = center + to_star * (center.length());
                let middle = (center + star) * 0.5;
                let pos = middle + up * 900.0 + side * 700.0 - to_star * 250.0;
                (pos, middle - pos, up, 1.0, (0.0, 0.0))
            }
            Shot::Landmark { body, kind, distance, sun } => {
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
            Shot::Overview | Shot::Landmark { .. } | Shot::Stream | Shot::Gaze { .. } => CameraMode::Cockpit,
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
    fn landmark(&self, index: usize, kind: PropKind, distance: f64, sun_height: f64, t: f64) -> (DVec3, DVec3, DVec3) {
        let body = &self.system.bodies[index];
        let center = body.position(t);
        let rotation = body.rotation(t);
        let to_star = (self.system.bodies[0].position(t) - center).normalize();
        // Vertical, direccion horizontal del sol y frente del objeto.
        let axes = |p: &Instance| {
            let up = rotation.rotate(p.position.normalize());
            let sun = (to_star - up * to_star.dot(up)).normalize();
            let front = rotation.rotate(p.rotation.rotate(DVec3::Z));
            (up, sun, front)
        };
        let score = |p: &Instance| {
            let (up, sun, front) = axes(p);
            let elevation = (up.dot(to_star) - sun_height).abs();
            let facing = if kind == PropKind::Monolith { (front.dot(sun).abs() - 0.6).abs() } else { 0.0 };
            elevation + facing
        };
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
        let (up, sun, front) = axes(best);
        let base = center + rotation.rotate(best.position);
        let model = &self.system.prop_models[kind.index()].lods[0];
        let scale = best.scale as f64;
        let side = up.cross(sun);
        let (target, directions) = if kind == PropKind::Monolith {
            // Frente al glifo (a 10.2 unidades del suelo), primero la cara
            // iluminada.
            let face = if front.dot(sun) >= 0.0 { front } else { -front };
            (base + up * (10.2 * scale), vec![face, -face])
        } else {
            let around = [60.0f64, -60.0, 100.0, -100.0, 30.0, -30.0, 140.0, -140.0, 0.0, 180.0];
            let dirs = around.iter().map(|a| sun * a.to_radians().cos() + side * a.to_radians().sin()).collect();
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
        let pos = directions
            .iter()
            .map(|&dir| camera_at(dir))
            .find(|&pos| clear(pos))
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
fn markers(system: &System, t: f64, camera: &Camera, view: &View, width: usize, height: usize) -> Vec<Marker> {
    let pixel = pixel_angle_of(camera, height);
    let state = cycle::at(t, system.bodies[0].radius);
    system
        .bodies
        .iter()
        .enumerate()
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
            let name = if i == 0 && state.rs > 0.0 { "Gargantua" } else { body.name };
            Some(Marker {
                name,
                x,
                y: y - (size as f32).min(60.0),
                distance: (distance - body.radius).max(0.0),
            })
        })
        .collect()
}

/// Latidos por segundo en este momento del ciclo (la derivada de la fase).
fn beat_rate(state: &CycleState) -> f64 {
    let later = cycle::at(state.tau + 0.01, 1.0).beat_phase;
    ((later - state.beat_phase) / 0.01).max(0.0)
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
fn atmosphere_frames<'a>(system: &'a System, t: f64, camera: &Camera, view: &View) -> Vec<AtmosphereFrame<'a>> {
    let star = system.bodies[0].position(t);
    let mut frames: Vec<(f64, AtmosphereFrame<'a>)> = system
        .bodies
        .iter()
        .zip(&system.atmospheres)
        .filter_map(|(body, model)| {
            let model = model.as_ref()?;
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

/// Anillos del cuerpo que los tiene, relativos a la camara.
fn ring_info(system: &System, t: f64, camera: DVec3) -> Option<RingInfo> {
    let body = &system.bodies[system.ring_body];
    let (inner, outer) = body.rings?;
    Some(RingInfo {
        center: (body.position(t) - camera).as_vec3(),
        normal: body.rotation(t).rotate(DVec3::Y).as_vec3(),
        inner: (inner * body.radius) as f32,
        outer: (outer * body.radius) as f32,
        planet_radius: body.radius as f32,
    })
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
    draws: &mut Vec<Draw<'a>>,
) -> PropStats {
    let mut stats = PropStats::default();
    let mut chosen = Vec::new();
    let star = &system.bodies[0];
    let (star_pos, star_radius) = (star.position(t), light_radius);
    for (body, set) in system.bodies.iter().zip(&system.props) {
        let Some(field) = &body.surface else { continue };
        if set.instances.is_empty() {
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
        let visible = |c: DVec3, r: f64| cull_view.sphere_visible((cull_center + rotation.mul_vec(c)).as_vec3(), r as f32);
        let prop_view = PropView {
            camera: quat.conjugate().rotate(-cull_center),
            occluder: body.radius + low as f64,
            surface_top: body.radius + high as f64,
            pixel_angle,
            visible: &visible,
        };
        chosen.clear();
        props::select(set, &system.prop_models, &prop_view, &mut stats, &mut chosen);
        for pick in &chosen {
            let instance = &set.instances[pick.index];
            let mesh = &system.prop_models[instance.kind.index()].lods[pick.lod];
            // Los modelos nacen en su base: al encogerse se hunden en el suelo.
            let scale = (instance.scale * pick.grow) as f64;
            let lift = shaders::displace(&body.material, instance.position.as_vec3(), t as f32, &system.noise);
            let position = instance.position + instance.position.normalize() * lift as f64;
            let rot = rotation.mul(&instance.rotation.to_mat3());
            let offset = center + rotation.mul_vec(position);
            let disc = (star_radius / (star_pos - body_pos).length()).min(1.0).asin() as f32;
            draws.push(Draw {
                mesh,
                indices: &mesh.indices,
                local_offset: Vec3::ZERO,
                model: DMat4::from_affine(&rot.scale(scale), offset).to_f32(),
                normal: rot.to_f32(),
                material: Material::Prop(PropStyle {
                    kind: instance.kind,
                    scale: scale as f32,
                    planet: center.as_vec3(),
                    // La esfera que hace sombra pasa por el suelo de este objeto:
                    // el terreno de alrededor tapa la estrella a esa altura.
                    occluder: instance.position.length() as f32,
                    disc,
                }),
                bounds_center: (offset + rot.mul_vec(mesh.bounds_center.as_dvec3() * scale)).as_vec3(),
                bounds_radius: mesh.bounds_radius * scale as f32,
                tag: pick.lod as u32 + 1,
            });
        }
    }
    stats
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
) -> Vec<Draw<'a>> {
    let mut draws = Vec::new();
    for (index, (body, terrain)) in system.bodies.iter().zip(&system.terrains).enumerate() {
        let rotation = body.rotation(t).to_mat3();
        let center = body.position(t) - camera.pos;
        match terrain {
            Some(terrain) => {
                let normal = rotation.to_f32();
                for chunk in terrain.selected() {
                    let offset = center + rotation.mul_vec(chunk.center);
                    draws.push(Draw {
                        mesh: &chunk.mesh,
                        indices: terrain.indices(),
                        local_offset: chunk.center.as_vec3(),
                        model: DMat4::from_affine(&rotation, offset).to_f32(),
                        normal,
                        material: body.material,
                        bounds_center: offset.as_vec3(),
                        bounds_radius: chunk.radius,
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
        let mesh = &system.meshes[system.ship_mesh];
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

    /// La app completa, sin ventana: en el modo local, mantener W rodea el
    /// planeta con la mira nivelada y la altura sostenida.
    #[test]
    fn holding_forward_in_local_mode_circles_the_planet_level() {
        let mut app = App::new(64, 36).unwrap();
        let preset = Preset {
            name: "prueba",
            shot: Shot::Surface { body: "Jardin", azimuth: 0.4, elevation: 0.2, altitude: 20.0 },
            time: 40.0,
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
        assert!(app.aim.right().dot(up).abs() < 0.02, "alabeo {}", app.aim.right().dot(up));
        // Avanzo un buen trecho sobre la superficie y sigue a su altura.
        let traveled = rel.normalize().dot(start.normalize()).clamp(-1.0, 1.0).acos() * body.radius;
        assert!(traveled > 100.0, "recorrio {traveled}");
        let altitude = rel.length() - body.radius - body.surface_height(rel, app.sim_time);
        assert!(altitude > 8.0 && altitude < 40.0, "altitud {altitude}");
    }
}
