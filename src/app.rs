//! Estado del juego: el sistema, la nave, la camara y el armado de cada frame.
//!
//! No sabe nada de raylib: recibe la entrada ya traducida (`FrameInput`) y
//! deja la imagen en el `Renderer`. Por eso las capturas y el benchmark
//! corren sin abrir ninguna ventana.

use std::f64::consts::PI;
use std::path::PathBuf;

use crate::config;
use crate::math::{smoothing, DMat4, DQuat, DVec3, Vec3};
use crate::render::{Draw, Renderer, Stats, View};
use crate::scene::camera::{Camera, CameraMode};
use crate::scene::ship::{self, Controls, FlightInfo, FlightSettings, ShipState};
use crate::scene::{nearest_surface, System};
use crate::shaders::{DebugView, Uniforms};
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

/// Vistas fijas para capturas y pruebas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    System,
    Approach,
    Surface,
    Ship,
}

impl Preset {
    pub const ALL: [Self; 4] = [Self::System, Self::Approach, Self::Surface, Self::Ship];

    pub fn name(self) -> &'static str {
        match self {
            Self::System => "01_sistema",
            Self::Approach => "02_acercamiento",
            Self::Surface => "03_superficie",
            Self::Ship => "04_nave",
        }
    }
}

pub struct App {
    pub system: System,
    pub renderer: Renderer,
    pub settings: FlightSettings,
    pub flight: FlightInfo,
    pub stats: Stats,
    pub camera_mode: CameraMode,
    pub lod: LodSettings,
    pub debug: DebugView,
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
        Ok(Self {
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
        let draws = build_draws(&self.system, t, &camera, ship_pos, ship_rot, show_ship);
        let star = &self.system.bodies[0];
        let uniforms = Uniforms {
            time: t as f32,
            sun_pos: (star.position(t) - camera.pos).as_vec3(),
            sun_radius: star.render_radius(t) as f32,
            sun_light: self.system.star_light(t),
            noise: &self.system.noise,
            sky: &self.system.sky,
            pixel_angle: (2.0 * (camera.fov_y * 0.5).tan() / self.renderer.height() as f64) as f32,
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

    /// Coloca la nave y la camara en una vista fija. Despues de colocarla se
    /// renderizan unos frames para que el terreno termine de cargar el detalle.
    pub fn apply_preset(&mut self, preset: Preset) {
        let t = 40.0;
        let bodies = &self.system.bodies;
        let planet = &bodies[1];
        let center = planet.position(t);
        let to_star = (bodies[0].position(t) - center).normalize();
        let side = to_star.cross(DVec3::Y).normalize();
        let up = side.cross(to_star).normalize();
        let r = planet.radius;
        let (pos, forward, view_up, zoom, look) = match preset {
            Preset::System => {
                let pos = center + (side * 0.9 + to_star * 0.15 + up * 0.25).normalize() * 2600.0;
                let target = center + to_star * 1300.0;
                (pos, target - pos, up, 1.0, (0.0, 0.0))
            }
            Preset::Approach => {
                let pos = center + (side * 0.75 + to_star * 0.45 + up * 0.2).normalize() * (r * 3.2);
                (pos, center - pos, up, 1.0, (0.0, 0.0))
            }
            Preset::Surface => {
                // Lado de dia, cerca del terminador: la estrella baja en el horizonte.
                let normal = (side * 0.94 + to_star * 0.34).normalize();
                let ground = planet.surface_height(normal, t);
                let pos = center + normal * (r + ground + 6.0);
                let horizon = (to_star - normal * to_star.dot(normal)).normalize();
                (pos, horizon * 0.95 - normal * 0.12, normal, 0.6, (0.0, 0.0))
            }
            Preset::Ship => {
                let pos = center + (side * 0.6 + to_star * 0.7 + up * 0.25).normalize() * (r * 2.2);
                (pos, center - pos, up, 0.55, (0.7, -0.2))
            }
        };
        self.place(pos, DQuat::look_rotation(forward, view_up), t);
        self.zoom = zoom;
        (self.look_yaw, self.look_pitch) = look;
    }

    fn place(&mut self, pos: DVec3, rot: DQuat, t: f64) {
        self.ship = ShipState::new(&self.system.bodies, pos, rot, t);
        self.ship_prev = self.ship;
        self.aim = rot;
        // El render dibuja `sim_time - dt` cuando el acumulador esta vacio.
        self.sim_time = t + config::PHYSICS_DT;
        self.accumulator = 0.0;
        self.flight.nearest = nearest_surface(&self.system.bodies, pos, t).0;
    }
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
