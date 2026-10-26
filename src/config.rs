//! Parametros del proyecto en un solo lugar.
//!
//! Lo que tiene que ver con un cuerpo concreto (radios, orbitas, colores) vive
//! en `scene/system.rs`; aqui queda lo global: render, fisica y controles.

/// Resolucion interna del render y de la ventana.
pub const DEFAULT_WIDTH: usize = 1280;
pub const DEFAULT_HEIGHT: usize = 720;

pub const FOV_Y_DEGREES: f64 = 70.0;
/// Plano cercano. Con la profundidad invertida en `f32` puede ser muy pequeno
/// sin perder precision a lo lejos.
pub const NEAR_PLANE: f64 = 0.05;
pub const EXPOSURE: f32 = 1.0;

/// Semilla de todo el ruido procedural: la misma semilla, el mismo sistema.
pub const NOISE_SEED: u32 = 1337;

/// Fisica a paso fijo: la nave responde igual a 30 que a 300 fps.
pub const PHYSICS_HZ: f64 = 120.0;
pub const PHYSICS_DT: f64 = 1.0 / PHYSICS_HZ;
/// Un frame mas lento que esto cuenta como si durara esto (evita saltos).
pub const MAX_FRAME_DT: f64 = 0.1;

/// Periodo del latido de la estrella, en segundos.
pub const HEARTBEAT_PERIOD: f64 = 1.9;

// Nave -------------------------------------------------------------------

/// Largo de la nave en unidades del mundo (el modelo se escala a esto).
pub const SHIP_LENGTH: f64 = 4.0;
/// La velocidad maxima crece con la altitud: lejos se cruza el sistema en
/// segundos y junto a la superficie se vuela con precision. Como la nave
/// tambien frena con la altitud, nunca llega al suelo a toda velocidad.
pub const SPEED_PER_ALTITUDE: f64 = 1.0;
pub const MIN_SPEED: f64 = 8.0;
pub const MAX_SPEED: f64 = 4000.0;
/// Multiplicador de la velocidad con Shift.
pub const BOOST: f64 = 3.0;
/// Aceleracion maxima como fraccion de la velocidad maxima, por segundo.
pub const ACCEL_PER_SPEED: f64 = 1.2;
/// Ganancia del asistente de vuelo (1/s): que tan rapido corrige la velocidad.
pub const ASSIST_GAIN: f64 = 3.0;
/// Giro: la nave persigue la mira con velocidad y aceleracion angular limitadas.
pub const TURN_GAIN: f64 = 5.0;
pub const MAX_TURN_RATE: f64 = 2.2;
pub const MAX_TURN_ACCEL: f64 = 9.0;
pub const ROLL_RATE: f64 = 1.6;
/// Radianes por pixel de mouse.
pub const MOUSE_SENSITIVITY: f64 = 0.0022;
/// Distancia minima entre la nave y la superficie.
pub const COLLISION_CLEARANCE: f64 = 1.5;
/// Fraccion de la velocidad contra la superficie que rebota al chocar.
pub const RESTITUTION: f64 = 0.25;

// Camara -----------------------------------------------------------------

pub const CHASE_HEIGHT: f64 = 1.8;
pub const CHASE_DISTANCE: f64 = 10.0;
/// Posicion de la cabina respecto al centro de la nave.
pub const COCKPIT_OFFSET: [f64; 3] = [0.0, 0.5, -0.6];
/// Distancia minima entre la camara y la superficie de un cuerpo.
pub const CAMERA_CLEARANCE: f64 = 0.6;
