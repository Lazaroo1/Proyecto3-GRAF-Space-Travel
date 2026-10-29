//! Parametros del proyecto en un solo lugar.
//!
//! Lo que tiene que ver con un cuerpo concreto (radios, orbitas, colores) vive
//! en `scene/system.rs`; aqui queda lo global: render, fisica y controles.

/// Resolucion interna del render y de la ventana.
pub const DEFAULT_WIDTH: usize = 1280;
pub const DEFAULT_HEIGHT: usize = 720;

pub const FOV_Y_DEGREES: f64 = 58.0;
/// Plano cercano. Con la profundidad invertida en `f32` puede ser muy pequeno
/// sin perder precision a lo lejos.
pub const NEAR_PLANE: f64 = 0.05;
pub const EXPOSURE: f32 = 1.0;

/// Bloom: luminancia HDR desde la que algo empieza a brillar, intensidad del
/// halo, niveles de la piramide, radio y sigma del blur y caida entre niveles.
pub const BLOOM_THRESHOLD: f32 = 1.1;
pub const BLOOM_INTENSITY: f32 = 0.22;
pub const BLOOM_LEVELS: usize = 6;
pub const BLOOM_RADIUS: usize = 4;
pub const BLOOM_SIGMA: f32 = 2.0;
pub const BLOOM_FALLOFF: f32 = 0.75;

/// Semilla de todo el ruido procedural: la misma semilla, el mismo sistema.
pub const NOISE_SEED: u32 = 1337;
/// Lado de cada cara del cielo horneado. Con 512 el cubemap pesa 6 MB y cabe
/// en la cache L3; con 1024 pesaria 25 MB y cada muestra esperaria a la RAM.
/// Un texel mide 0.18 grados, poco mas de un pixel a 720p.
pub const SKY_SIZE: usize = 1024;

// Terreno -----------------------------------------------------------------

/// Lado del cuadro mas fino del terreno, en unidades del mundo.
pub const TERRAIN_FINEST: f64 = 0.35;
/// Se parte un chunk si la camara esta a menos de este factor por su tamano.
pub const LOD_FACTOR: f64 = 1.8;
/// Chunks nuevos por frame como maximo.
pub const LOD_BUDGET: usize = 24;
/// Nivel fijo cuando se apaga el LOD para comparar.
pub const LOD_FIXED_LEVEL: u8 = 4;

// Objetos de la superficie --------------------------------------------------

/// Semilla de la distribucion de los objetos; F6 la cambia en vivo.
pub const PROP_SEED: u32 = 2024;
/// Radio en pixeles a partir del que un objeto usa el detalle 0 y el 1.
pub const PROP_LOD_PIXELS: [f32; 2] = [70.0, 18.0];
/// Radio en pixeles debajo del que un objeto ya no se dibuja (menos que eso
/// solo parpadearia al moverse la camara) y radio desde el que se dibuja
/// completo. Entre los dos se encoge hacia el suelo: no aparece de golpe.
pub const PROP_MIN_PIXELS: f32 = 2.5;
pub const PROP_FULL_PIXELS: f32 = 6.0;

/// Fisica a paso fijo: la nave responde igual a 30 que a 300 fps.
pub const PHYSICS_HZ: f64 = 120.0;
pub const PHYSICS_DT: f64 = 1.0 / PHYSICS_HZ;
/// Un frame mas lento que esto cuenta como si durara esto (evita saltos).
// Admite los frames del perfil de detalle sin ralentizar el ciclo de 15 min.
// Las pausas largas siguen acotadas para evitar una recuperación ilimitada.
pub const MAX_FRAME_DT: f64 = 2.0;

/// Periodo del latido de la estrella, en segundos (al principio del ciclo).
pub const HEARTBEAT_PERIOD: f64 = 1.9;
/// Rebobinar (R) va tantas veces mas rapido que el tiempo normal.
pub const REWIND_SPEED: f64 = 8.0;

// Nave -------------------------------------------------------------------

/// Largo de la nave en unidades del mundo (el modelo se escala a esto).
pub const SHIP_LENGTH: f64 = 4.0;
/// La velocidad maxima crece con la altitud: lejos se cruza el sistema en
/// segundos y junto a la superficie se vuela con precision. Como la nave
/// tambien frena con la altitud, nunca llega al suelo a toda velocidad.
pub const SPEED_PER_ALTITUDE: f64 = 0.55;
pub const MIN_SPEED: f64 = 8.0;
pub const MAX_SPEED: f64 = 420.0;
/// Multiplicador de la velocidad con Shift.
pub const BOOST: f64 = 2.6;
/// El freno mantiene su fuerza cuando baja la velocidad maxima junto al suelo.
pub const BRAKE_GAIN: f64 = 8.0;
pub const BRAKE_MIN_ACCEL: f64 = 100.0;
/// Margen explorable mas alla de la orbita mas lejana.
pub const SYSTEM_MARGIN: f64 = 3200.0;
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
/// El modo local se activa por debajo de esta altitud, en radios del planeta.
pub const LOCAL_RANGE: f64 = 1.5;
/// Ganancia (1/s) con la que el modo local recupera la altura que sostiene.
pub const HOLD_GAIN: f64 = 1.5;
/// Empuje hacia adelante del crucero (fraccion de la velocidad maxima).
pub const CRUISE_THRUST: f64 = 0.6;

// Camara -----------------------------------------------------------------

pub const CHASE_HEIGHT: f64 = 2.8;
pub const CHASE_DISTANCE: f64 = 8.8;
/// Posicion de la cabina respecto al centro de la nave.
pub const COCKPIT_OFFSET: [f64; 3] = [0.0, 0.5, -0.6];
/// Distancia minima entre la camara y la superficie de un cuerpo.
pub const CAMERA_CLEARANCE: f64 = 0.6;
