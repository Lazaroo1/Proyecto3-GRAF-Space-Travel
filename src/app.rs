//! Estado del juego: el sistema, la nave, la camara y el armado de cada frame.
//!
//! No sabe nada de raylib: recibe la entrada ya traducida (`FrameInput`) y
//! deja la imagen en el `Renderer`. Por eso las capturas y el benchmark
//! corren sin abrir ninguna ventana.

use std::f64::consts::PI;
use std::path::PathBuf;

use crate::config;
use crate::math::{smoothing, smoothstep, DMat4, DQuat, DVec3, Vec3};
use crate::render::{Draw, Renderer, Stats, View};
use crate::scene::camera::{Camera, CameraMode};
use crate::scene::props::{self, Instance, PropKind, PropStats, PropView};
use crate::scene::ship::{self, Controls, FlightInfo, FlightSettings, ShipState};
use crate::scene::{nearest_surface, System};
use crate::shaders::{self, AtmosphereFrame, DebugView, Material, RingInfo, SkyView, Uniforms};
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
    /// Un objeto de la superficie de cerca. `distance` en unidades del mundo;
    /// `sun` es el seno de la altura del sol que se busca sobre su horizonte
    /// (bajo y de lado marca el relieve; bajo cero, de noche, se luce lo que
    /// brilla).
    Landmark { body: &'static str, kind: PropKind, distance: f64, sun: f64 },
}

/// Una vista fija con nombre, para capturas y para el benchmark.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub shot: Shot,
}

pub const PRESETS: [Preset; 13] = [
    Preset { name: "01_sistema", shot: Shot::Overview },
    Preset { name: "02_pira", shot: Shot::Orbit { body: "Pira", distance: 3.2, azimuth: 0.5, elevation: 0.2 } },
    Preset { name: "03_pira_noche", shot: Shot::Orbit { body: "Pira", distance: 2.8, azimuth: 2.6, elevation: 0.15 } },
    Preset { name: "04_jardin", shot: Shot::Orbit { body: "Jardin", distance: 3.0, azimuth: 1.2, elevation: 0.25 } },
    Preset {
        name: "05_jardin_superficie",
        shot: Shot::Surface { body: "Jardin", azimuth: 1.75, elevation: 0.3, altitude: 6.0 },
    },
    Preset { name: "06_vigia", shot: Shot::Orbit { body: "Vigia", distance: 2.6, azimuth: 0.8, elevation: 0.1 } },
    Preset {
        name: "07_requiem",
        shot: Shot::Surface { body: "Requiem", azimuth: 1.25, elevation: 0.35, altitude: 6.0 },
    },
    Preset {
        name: "08_cementerio",
        shot: Shot::Orbit { body: "Cementerio", distance: 5.0, azimuth: 0.9, elevation: 0.35 },
    },
    Preset { name: "09_nave", shot: Shot::Ship { body: "Requiem" } },
    Preset {
        name: "10_monolito",
        shot: Shot::Landmark { body: "Requiem", kind: PropKind::Monolith, distance: 16.0, sun: 0.42 },
    },
    Preset {
        name: "11_hongos",
        shot: Shot::Landmark { body: "Jardin", kind: PropKind::Mushroom, distance: 22.0, sun: -0.12 },
    },
    Preset {
        name: "12_espinas",
        shot: Shot::Landmark { body: "Pira", kind: PropKind::Spike, distance: 26.0, sun: 0.3 },
    },
    Preset {
        name: "13_naufragio",
        shot: Shot::Landmark { body: "Cementerio", kind: PropKind::Wreck, distance: 40.0, sun: 0.35 },
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
            let yaw = DQuat::from_axis_angle(DVec3::Y, -dx * sens);
            let pitch = DQuat::from_axis_angle(DVec3::X, -dy * sens);
            self.aim = self.aim * yaw * pitch;
        }
        let roll = DQuat::from_axis_angle(DVec3::Z, input.roll * config::ROLL_RATE * dt);
        self.aim = (self.aim * roll).normalize();
        self.zoom = (self.zoom * (1.0 - 0.1 * input.zoom)).clamp(0.35, 8.0);
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
        self.update_terrain(t, &camera, &view);

        let show_ship = self.camera_mode == CameraMode::Chase;
        let pixel_angle = 2.0 * (camera.fov_y * 0.5).tan() / self.renderer.height() as f64;
        let mut draws = build_draws(&self.system, t, &camera, ship_pos, ship_rot, show_ship);
        self.prop_stats = prop_draws(&self.system, t, &camera, &view, pixel_angle, &mut draws);
        let atmospheres = atmosphere_frames(&self.system, t, &camera, &view);
        let sun_light = self.system.star_light(t);
        // Si la camara esta dentro de una atmosfera, su cielo sale de una tabla.
        let sky_view = atmospheres
            .iter()
            .enumerate()
            .find(|(_, frame)| frame.center.length() < frame.model.radius + frame.model.params.thickness)
            .map(|(i, frame)| SkyView::build(i, frame, sun_light));
        let star = &self.system.bodies[0];
        let uniforms = Uniforms {
            time: t as f32,
            sun_pos: (star.position(t) - camera.pos).as_vec3(),
            sun_radius: star.render_radius(t) as f32,
            sun_light,
            noise: &self.system.noise,
            sky: &self.system.sky,
            textures: &self.system.textures,
            pixel_angle: pixel_angle as f32,
            eye_dir: self.eye.as_vec3(),
            ring: ring_info(&self.system, t, camera.pos),
            atmospheres: &atmospheres,
            sky_view: sky_view.as_ref(),
            biolum: 1.0,
            debug: self.debug,
        };
        self.stats = self.renderer.render(&draws, &view, &uniforms);
        self.stats
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
        let t = 40.0;
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
            Shot::Landmark { body, kind, distance, sun } => {
                let i = index(body);
                let (pos, target, up) = self.landmark(i, kind, distance, sun, t);
                (pos, target - pos, up, 1.0, (0.0, 0.0))
            }
        };
        self.place(pos, DQuat::look_rotation(forward, view_up), t);
        self.zoom = zoom;
        (self.look_yaw, self.look_pitch) = look;
        // En la vista del sistema y frente a los objetos la nave estorbaria:
        // se mira desde la cabina.
        self.camera_mode = match preset.shot {
            Shot::Overview | Shot::Landmark { .. } => CameraMode::Cockpit,
            _ => CameraMode::Chase,
        };
    }

    /// Encuadre de un objeto de tipo `kind` del cuerpo `index`: devuelve la
    /// posicion de la camara, el punto que mira y la vertical.
    ///
    /// Entre todos los objetos de ese tipo elige el que tiene el sol mas cerca
    /// de la altura pedida y, en los monolitos, una cara con el glifo de lado
    /// al sol. La camara se para
    /// frente a esa cara, o a 60 grados del sol en los demas; si una loma
    /// tapa el objeto, prueba otros angulos alrededor.
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
        self.eye = eye_target(&self.system, pos, t);
    }
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
fn prop_draws<'a>(
    system: &'a System,
    t: f64,
    camera: &Camera,
    view: &View,
    pixel_angle: f64,
    draws: &mut Vec<Draw<'a>>,
) -> PropStats {
    let mut stats = PropStats::default();
    let mut chosen = Vec::new();
    let star = &system.bodies[0];
    let (star_pos, star_radius) = (star.position(t), star.render_radius(t));
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
        let visible = |c: DVec3, r: f64| view.sphere_visible((center + rotation.mul_vec(c)).as_vec3(), r as f32);
        let prop_view = PropView {
            camera: quat.conjugate().rotate(-center),
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
            // Cuanto del disco de la estrella asoma sobre su horizonte.
            let to_star = star_pos - (body_pos + rotation.mul_vec(position));
            let distance = to_star.length();
            let up = rotation.mul_vec(instance.position.normalize());
            let elevation = (up.dot(to_star) / distance).clamp(-1.0, 1.0).asin();
            let disc = (star_radius / distance).min(1.0).asin();
            let light = smoothstep(-disc as f32, disc as f32, elevation as f32);
            draws.push(Draw {
                mesh,
                indices: &mesh.indices,
                local_offset: Vec3::ZERO,
                model: DMat4::from_affine(&rot.scale(scale), offset).to_f32(),
                normal: rot.to_f32(),
                material: Material::Prop {
                    kind: instance.kind,
                    scale: scale as f32,
                    light,
                },
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
) -> Vec<Draw<'a>> {
    let mut draws = Vec::new();
    for (body, terrain) in system.bodies.iter().zip(&system.terrains) {
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
                let radius = body.render_radius(t);
                draws.push(Draw {
                    mesh,
                    indices: &mesh.indices,
                    local_offset: Vec3::ZERO,
                    model: DMat4::from_affine(&rotation.scale(radius), center).to_f32(),
                    normal: rotation.to_f32(),
                    material: body.material,
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
