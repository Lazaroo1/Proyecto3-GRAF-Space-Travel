//! Objetos de la superficie: modelos procedurales, distribucion y detalle.
//!
//! # Modelos
//! Todos generados con codigo (`MeshBuilder`), cada uno en tres niveles de
//! detalle: ojos de carne (Argos), hongos y corales (Jardin), espinas de
//! obsidiana y geiseres (Pira), naves naufragas y la baliza (Cementerio) y
//! cristales (Parpado).
//!
//! # Distribucion
//! Cada cara del cubo-esfera se divide en una grilla; cada celda propone un
//! candidato en un punto al azar dentro de ella (un hash de la semilla, la
//! cara y la celda). El candidato se acepta con una probabilidad que sale de
//! **ruido fractal** (asi los objetos forman grupos y claros, no una lluvia
//! pareja) multiplicado por lo que dice el terreno en ese punto: altura,
//! pendiente y mascaras (no hay hongos en la lava ni ojos en los surcos).
//! Todo depende solo de la semilla: la misma semilla, las mismas ubicaciones.
//!
//! # Orientacion
//! Cada objeto se para sobre la normal radial del planeta, con un giro al
//! azar alrededor de ella, y se hunde un poco para no flotar en las
//! pendientes. Se guarda en el espacio del planeta: al girar el planeta, sus
//! objetos giran con el.
//!
//! # Nivel de detalle
//! Cada frame se elige el nivel por el **tamano en pantalla** (el radio de la
//! esfera envolvente en pixeles), no por la distancia: una aguja grande
//! conserva su detalle mas lejos que un hongo, y todo se adapta solo a la
//! resolucion. Se descartan los que se verian de menos de un par de pixeles,
//! los que quedan detras del horizonte del planeta y los que caen fuera del
//! frustum. Cerca del limite de tamano el objeto se encoge hacia el suelo en
//! vez de desaparecer de golpe. Los contadores van al HUD.

use std::f32::consts::TAU;

use super::Mesh;
use super::builder::MeshBuilder;
use crate::config;
use crate::math::{DMat3, DQuat, DVec3, Perlin, Vec3, hash_u32, smoothstep};
use crate::parallel::{self, DisjointMut};
use crate::terrain::{HeightField, TerrainSample, cube_to_sphere};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Eye,
    Mushroom,
    Coral,
    Spike,
    Wreck,
    Boulder,
    Beacon,
    Vent,
    Crystal,
}

impl PropKind {
    pub const ALL: [Self; 9] = [
        Self::Eye,
        Self::Mushroom,
        Self::Coral,
        Self::Spike,
        Self::Wreck,
        Self::Boulder,
        Self::Beacon,
        Self::Vent,
        Self::Crystal,
    ];

    /// Posicion en `ALL` (y en la lista de modelos).
    pub fn index(self) -> usize {
        self as usize
    }

    /// Cuanto cuenta su tamano en pantalla para decidir si se dibuja. Los
    /// naufragios (1) se ven desde lejos; los ojos de Argos, un poco menos;
    /// la vegetacion y las espinas (0.5) necesitan el doble de tamano: desde
    /// la orbita solo ensuciarian.
    pub fn visibility(self) -> f32 {
        match self {
            Self::Wreck | Self::Boulder | Self::Beacon => 1.0,
            Self::Eye => 0.75,
            Self::Mushroom | Self::Coral | Self::Spike | Self::Vent | Self::Crystal => 0.5,
        }
    }
}

/// Un tipo de objeto con sus tres niveles de detalle (0 es el mas fino).
pub struct PropModel {
    pub lods: [Mesh; 3],
}

/// Un objeto colocado en la superficie, en el espacio del planeta.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub kind: PropKind,
    pub position: DVec3,
    pub rotation: DQuat,
    pub scale: f32,
}

/// Unidades del modelo por cada repeticion de la textura del casco.
pub const WRECK_UV_SIZE: f32 = 7.0;
/// Altura del coral en todos sus niveles de detalle (el shader la usa para
/// el degradado de color).
pub const CORAL_TOP: f32 = 12.9;
/// Radio del casco de los naufragios: el modelo se arma con el casco
/// apoyado en `y = 0`.
const HULL: f32 = 3.2;

/// Centro del globo de los ojos de Argos, en unidades del modelo (radio 1).
pub const EYE_CENTER: Vec3 = Vec3::new(0.0, 0.35, 0.0);
/// Altura de la luz de la baliza de Cementerio, en unidades del modelo.
pub const BEACON_TOP: f32 = 46.0;

// Colores de vertice: algunos shaders los usan como marcas.
const STONE: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const STEM: Vec3 = Vec3::new(0.72, 0.66, 0.6);
const CAP: Vec3 = Vec3::new(0.36, 0.05, 0.3);
/// Bulbos del coral (el verde en 0 marca la punta que brilla).
const TIP: Vec3 = Vec3::new(1.0, 0.0, 1.0);
/// La lampara de la baliza (el shader la enciende).
const LAMP: Vec3 = Vec3::new(1.0, 0.0, 0.0);
/// La boca de lava de los geiseres (el shader la enciende).
const POOL: Vec3 = Vec3::new(1.0, 1.0, 0.0);
/// Altura de la boca de los geiseres, en unidades del modelo.
pub const VENT_TOP: f32 = 7.3;
/// Paredes de adentro del casco: el shader las oscurece con el rojo.
const INSIDE: Vec3 = Vec3::new(0.15, 1.0, 1.0);
/// El globo de los ojos de Argos (el shader lo distingue de la carne).
const EYEBALL: Vec3 = Vec3::new(1.0, 1.0, 1.0);

/// Ejes de una caja girada `angle` alrededor de z.
fn tilted(angle: f32) -> [Vec3; 3] {
    let (s, c) = angle.sin_cos();
    [Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0), Vec3::Z]
}

/// Un ojo de la carne de Argos: un globo medio hundido en un bulto de carne
/// y rodeado por un borde delgado y grumoso, la cuenca. Los parpados no son
/// geometria: el shader tapa el globo con piel fuera de una almendra (y asi
/// pueden parpadear). El globo lleva el color `EYEBALL`; la carne, `STONE`.
fn eye(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let (rings, segments, around, tube) = [(4, 8, 10, 4), (6, 12, 16, 6), (9, 18, 26, 8)][detail];
    b.color(EYEBALL);
    b.ellipsoid(EYE_CENTER, Vec3::splat(0.95), rings, segments, true);
    b.color(STONE);
    // El bulto de carne donde se hunde.
    b.ellipsoid(Vec3::new(0.0, -0.1, 0.0), Vec3::new(1.7, 0.6, 1.5), rings, segments, true);
    // El borde de la cuenca, pegado al globo, con grumos.
    let mut path = Vec::with_capacity(around + 1);
    let mut radii = Vec::with_capacity(around + 1);
    for k in 0..=around {
        let a = k as f32 / around as f32 * TAU;
        let h = hash_u32(0xE7E5 ^ (k % around) as u32);
        let lump = ((h >> 8) & 0xFF) as f32 / 255.0;
        path.push(Vec3::new(a.cos() * 1.0, EYE_CENTER.y + 0.04, a.sin() * 1.0));
        radii.push(0.15 + 0.1 * lump);
    }
    *radii.last_mut().unwrap() = radii[0];
    b.tube(&path, &radii, tube);
    b.build()
}

/// La baliza de Cementerio: una torre de celosia de cuatro patas que se
/// juntan arriba, con travesanos, una plataforma, una antena y la lampara.
fn beacon(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    let (base, top) = (5.0, 1.2);
    let height = 40.0;
    let leg = |k: usize, y: f32| {
        let a = (k as f32 + 0.5) * TAU / 4.0;
        let r = base + (top - base) * y / height;
        Vec3::new(a.cos() * r, y, a.sin() * r)
    };
    let sides = [3, 4, 6][detail];
    for k in 0..4 {
        let (p0, p1) = (leg(k, -1.0), leg(k, height));
        b.frustum(p0, (p1 - p0).normalize(), (p1 - p0).length(), 0.45, 0.28, sides, false, (false, true));
    }
    if detail >= 1 {
        // Travesanos en zigzag entre patas vecinas.
        let levels = [5, 8, 12][detail];
        for i in 0..levels {
            let (y0, y1) = (
                height * i as f32 / levels as f32,
                height * (i + 1) as f32 / levels as f32,
            );
            for k in 0..4 {
                let (a, c) = (leg(k, y0), leg((k + 1) % 4, y1));
                let axis = (c - a).normalize();
                let side = axis.cross(Vec3::Y).normalize();
                b.cuboid((a + c) * 0.5, [side, axis, side.cross(axis)], Vec3::new(0.12, (c - a).length() * 0.5, 0.12));
            }
        }
    }
    // Plataforma, antena y la lampara.
    b.frustum(Vec3::new(0.0, height, 0.0), Vec3::Y, 1.0, 2.6, 2.4, sides.max(4), false, (true, true));
    b.frustum(Vec3::new(0.0, height + 1.0, 0.0), Vec3::Y, 4.0, 0.25, 0.12, sides, false, (false, true));
    b.color(LAMP);
    b.ellipsoid(Vec3::new(0.0, BEACON_TOP, 0.0), Vec3::splat(1.1), [3, 4, 6][detail], [6, 8, 12][detail], false);
    if detail >= 1 {
        // La antena parabolica, apuntando al cielo de lado.
        b.color(STONE);
        b.ellipsoid(Vec3::new(2.4, height - 4.0, 0.0), Vec3::new(2.2, 0.5, 2.2), 4, [8, 10, 16][detail], true);
    }
    b.build()
}

/// Un geiser de Pira: un cono de roca rugosa con una boca de lava y rocas
/// al pie.
fn vent(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    let sides = [6, 10, 16][detail];
    b.frustum(Vec3::new(0.0, -1.5, 0.0), Vec3::Y, 8.0, 5.2, 1.9, sides, false, (false, false));
    // Labio de la boca.
    b.frustum(Vec3::new(0.0, 6.5, 0.0), Vec3::Y, 0.8, 1.95, 1.65, sides, false, (false, true));
    b.color(POOL);
    b.frustum(Vec3::new(0.0, VENT_TOP - 0.1, 0.0), Vec3::Y, 0.12, 1.5, 1.5, sides, false, (false, true));
    if detail >= 1 {
        b.color(STONE);
        for k in 0..[0, 4, 7][detail] {
            let h = hash_u32(0x7E47 + k as u32);
            let a = (h & 0xFFFF) as f32 / 65536.0 * TAU;
            let r = 4.6 + 1.6 * ((h >> 16) & 0xFF) as f32 / 255.0;
            b.cuboid(
                Vec3::new(a.cos() * r, 0.2, a.sin() * r),
                tilted(a),
                Vec3::new(0.7, 0.5, 0.6),
            );
        }
    }
    let mut mesh = b.build();
    // Roca rugosa: el cono se abolla, la boca no.
    let noise = Perlin::new(0x7E47);
    mesh.deform(|p| {
        if p.y > VENT_TOP - 0.3 {
            return p;
        }
        let bump = 1.0 + 0.16 * noise.fbm(p * 0.7, 3);
        Vec3::new(p.x * bump, p.y, p.z * bump)
    });
    mesh
}

/// Una instancia en el lugar: a `x` a la derecha y `z` al frente del centro
/// (unidades del mundo), sobre el terreno.
fn at_site(
    radius: f64,
    field: &dyn HeightField,
    frame: (DVec3, DVec3, DVec3),
    kind: PropKind,
    x: f64,
    z: f64,
    scale: f32,
    yaw: f64,
    sink: f64,
) -> Instance {
    let (n, forward, right) = frame;
    let dir = (n + (right * x + forward * z) / radius).normalize();
    let height = field.sample(dir.as_vec3()).height as f64;
    Instance {
        kind,
        position: dir * (radius + height - sink * scale as f64),
        rotation: upright_rotation(dir, yaw),
        scale,
    }
}

/// Quita del lugar los objetos repartidos a menos de `clear` unidades.
fn clear_site(instances: &mut Vec<Instance>, radius: f64, normal: DVec3, clear: f64) {
    instances.retain(|p| (p.position.normalize() - normal).length() * radius > clear);
}

/// Los geiseres de Pira: una docena de bocas de lava alrededor de un claro.
pub fn vent_field(
    instances: &mut Vec<Instance>,
    radius: f64,
    field: &dyn HeightField,
    seed: u32,
    frame: (DVec3, DVec3, DVec3),
) {
    clear_site(instances, radius, frame.0, 110.0);
    for k in 0..12u32 {
        let h = hash_u32(seed ^ k.wrapping_mul(0x9E37) ^ 0x6E15);
        let angle = unit(h) as f64 * std::f64::consts::TAU;
        let distance = 22.0 + 70.0 * unit(hash_u32(h)) as f64;
        instances.push(at_site(
            radius,
            field,
            frame,
            PropKind::Vent,
            angle.cos() * distance,
            angle.sin() * distance + 40.0,
            0.9 + 0.8 * unit(hash_u32(h ^ 3)),
            unit(hash_u32(h ^ 5)) as f64 * std::f64::consts::TAU,
            0.4,
        ));
    }
}

/// Que tan adelante del lugar queda el centro del nido de Argos, en unidades
/// del mundo.
const NEST_AHEAD: f64 = 50.0;

/// El nido de Argos: un ojo enorme en medio y un monton alrededor, de todos
/// los tamanos, apretados en la carne. Todos miran a la nave.
pub fn eye_nest(
    instances: &mut Vec<Instance>,
    radius: f64,
    field: &dyn HeightField,
    seed: u32,
    frame: (DVec3, DVec3, DVec3),
) {
    clear_site(instances, radius, frame.0, 110.0);
    let center = NEST_AHEAD;
    instances.push(at_site(radius, field, frame, PropKind::Eye, 0.0, center, 9.0, 0.0, 0.25));
    for k in 0..22u32 {
        let h = hash_u32(seed ^ k.wrapping_mul(0x51ED) ^ 0xE7E5);
        let angle = (k as f64 + 0.6 * unit(h) as f64) / 22.0 * std::f64::consts::TAU;
        let distance = 18.0 + 60.0 * unit(hash_u32(h)) as f64;
        let size = unit(hash_u32(h ^ 1));
        instances.push(at_site(
            radius,
            field,
            frame,
            PropKind::Eye,
            angle.cos() * distance,
            center + angle.sin() * distance,
            1.5 + 4.5 * size * size,
            unit(hash_u32(h ^ 9)) as f64 * std::f64::consts::TAU,
            0.25,
        ));
    }
}

/// La baliza de Cementerio con las naves que cayeron a su alrededor.
pub fn beacon_site(
    instances: &mut Vec<Instance>,
    radius: f64,
    field: &dyn HeightField,
    seed: u32,
    frame: (DVec3, DVec3, DVec3),
) {
    clear_site(instances, radius, frame.0, 90.0);
    instances.push(at_site(radius, field, frame, PropKind::Beacon, 0.0, 0.0, 1.0, 0.7, 0.8));
    for k in 0..8u32 {
        let h = hash_u32(seed ^ k.wrapping_mul(0x3C6E) ^ 0xB41A);
        let angle = (k as f64 + unit(h) as f64 * 0.6) / 8.0 * std::f64::consts::TAU;
        let distance = 32.0 + 40.0 * unit(hash_u32(h)) as f64;
        instances.push(at_site(
            radius,
            field,
            frame,
            PropKind::Wreck,
            angle.cos() * distance,
            angle.sin() * distance,
            0.9 + 0.4 * unit(hash_u32(h ^ 7)),
            unit(hash_u32(h ^ 9)) as f64 * std::f64::consts::TAU,
            1.0,
        ));
    }
}

/// Altura del cristal mas alto de un racimo, en unidades del modelo.
pub const CRYSTAL_TOP: f32 = 9.0;

/// Racimo de cristales de Parpado: prismas hexagonales con punta que salen
/// del mismo punto, de varios tamanos y algo abiertos.
fn crystal(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    let count = [3, 6, 9][detail];
    for k in 0..count {
        let h = hash_u32(0xC4A5 ^ (k as u32).wrapping_mul(0x9E37_79B9));
        let unit = |shift: u32| ((h >> shift) & 0xFF) as f32 / 255.0;
        let angle = unit(0) * TAU;
        let spread = if k == 0 { 0.0 } else { 0.35 + 0.9 * unit(8) };
        let out = Vec3::new(angle.cos(), 0.0, angle.sin());
        let base = out * spread - Vec3::Y * 0.6;
        let lean = if k == 0 { 0.06 } else { 0.25 + 0.5 * unit(16) };
        let axis = (Vec3::Y + out * lean).normalize();
        let length = if k == 0 { CRYSTAL_TOP } else { 2.8 + 5.2 * unit(24) };
        let radius = if k == 0 { 0.95 } else { 0.32 + 0.38 * unit(4) };
        let shaft = length * 0.8;
        b.frustum(base, axis, shaft, radius, radius * 0.9, 6, false, (false, false));
        b.frustum(base + axis * shaft, axis, length - shaft, radius * 0.9, 0.0, 6, false, (false, false));
    }
    b.build()
}

/// El espejo de Parpado: racimos de cristal grandes en la orilla del lago,
/// que enmarcan a Vigia del otro lado del agua. El lago esta adelante del
/// lugar (`z` positivo), a `LAKE_AHEAD` unidades.
pub fn mirror_shore(
    instances: &mut Vec<Instance>,
    radius: f64,
    field: &dyn HeightField,
    seed: u32,
    frame: (DVec3, DVec3, DVec3),
) {
    const LAKE_AHEAD: f64 = 30.0;
    const SHORE: f64 = 31.0;
    clear_site(instances, radius, frame.0, 30.0);
    for k in 0..14u32 {
        let h = hash_u32(seed ^ k.wrapping_mul(0x2C1B) ^ 0xE5BE);
        // Alrededor del lago, menos en la orilla de enfrente (la del lugar).
        let angle = (0.16 + 0.68 * (k as f64 + unit(h) as f64 * 0.7) / 14.0) * std::f64::consts::TAU;
        let distance = SHORE + 6.0 * unit(hash_u32(h)) as f64;
        let x = angle.sin() * distance;
        let z = LAKE_AHEAD - angle.cos() * distance;
        // Los de los costados del fondo, los mas grandes.
        let side = angle.sin().abs();
        let scale = 1.1 + 1.6 * side * unit(hash_u32(h ^ 3)) as f64 + 0.6 * side;
        instances.push(at_site(
            radius,
            field,
            frame,
            PropKind::Crystal,
            x,
            z,
            scale as f32,
            unit(hash_u32(h ^ 5)) as f64 * std::f64::consts::TAU,
            0.6,
        ));
    }
    // Dos al lado del lugar, cerca: enmarcan la vista.
    for (x, z, scale) in [(-15.0, -4.0, 1.3), (17.0, 2.0, 1.0)] {
        instances.push(at_site(radius, field, frame, PropKind::Crystal, x, z, scale, x * 0.1, 0.6));
    }
}

fn mushroom(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let sides = [10, 24, 48][detail];
    let top = Vec3::new(0.69, 8.6, 0.0);
    b.color(STEM);
    let path: Vec<Vec3> = (0..=6)
        .map(|k| {
            let t = k as f32 / 6.0;
            Vec3::new((t * 1.4).sin() * 0.7, t * 8.9, 0.0)
        })
        .collect();
    let radii: Vec<f32> = (0..=6).map(|k| 0.72 - 0.35 * k as f32 / 6.0).collect();
    b.tube(&path, &radii, [5, 8, 12][detail]);
    b.color(CAP);
    b.ellipsoid(
        top,
        Vec3::new(5.0, 0.85, 5.0),
        [3, 7, 12][detail],
        sides,
        true,
    );
    if detail > 0 {
        // Nervaduras arqueadas: sostienen el disco como un paraguas vivo.
        b.color(STEM);
        let count = [0, 10, 22][detail];
        for k in 0..count {
            let angle = k as f32 / count as f32 * TAU;
            let radial = Vec3::new(angle.cos(), 0.0, angle.sin());
            let rib: Vec<Vec3> = (0..=7)
                .map(|j| {
                    let t = j as f32 / 7.0;
                    let r = 0.65 + 4.3 * t.powi(3);
                    top + radial * r + Vec3::Y * (-8.6 * (1.0 - t))
                })
                .collect();
            b.tube(
                &rib,
                &(0..=7)
                    .map(|j| 0.15 - 0.075 * j as f32 / 7.0)
                    .collect::<Vec<_>>(),
                [4, 5, 6][detail],
            );
            if detail == 2 && k % 2 == 0 {
                let root = [
                    radial * 2.2 - Vec3::Y * 0.2,
                    radial * 1.0 + Vec3::Y * 0.5,
                    Vec3::Y * 1.8,
                ];
                b.tube(&root, &[0.18, 0.28, 0.22], 5);
            }
        }
        // Aro liso y fino en la orilla.
        b.color(CAP);
        let rim: Vec<Vec3> = (0..=sides)
            .map(|k| {
                let a = k as f32 / sides as f32 * TAU;
                top + Vec3::new(a.cos() * 5.0, 0.0, a.sin() * 5.0)
            })
            .collect();
        b.tube(&rim, &vec![0.08; rim.len()], 5);
    }
    b.build()
}

fn organic_link(b: &mut MeshBuilder, a: Vec3, bend: Vec3, c: Vec3, r: f32, detail: usize) {
    let control = bend * 2.0 - (a + c) * 0.5;
    let steps = [2, 3, 5][detail];
    let path: Vec<_> = (0..=steps)
        .map(|j| {
            let t = j as f32 / steps as f32;
            a * (1.0 - t).powi(2) + control * (2.0 * t * (1.0 - t)) + c * t * t
        })
        .collect();
    let radii: Vec<_> = (0..=steps)
        .map(|j| r * (0.90 + 0.10 * (j as f32 / steps as f32 * std::f32::consts::PI).sin()))
        .collect();
    b.tube(&path, &radii, [4, 6, 10][detail]);
}

fn coral(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let levels = [4, 8, 13][detail];
    let sides = [5, 8, 12][detail];
    let noise = Perlin::new(0xC0A1);
    let node = |j: usize, k: usize| {
        let t = j as f32 / levels as f32;
        let h = noise.noise(Vec3::new(k as f32 * 0.63, j as f32 * 0.47, 1.8));
        let a = k as f32 / sides as f32 * TAU + t * 1.3 + h * 0.42;
        let radius = (2.6 * (1.0 - t).max(0.0).powf(0.62) + 0.12) * (1.0 + h * 0.35);
        Vec3::new(
            a.cos() * radius + (t * 3.3).sin() * 0.6,
            t * CORAL_TOP + h * 1.2 * (std::f32::consts::PI * t).sin(),
            a.sin() * radius + (t * 4.1).sin() * 0.4,
        )
    };
    b.color(Vec3::ONE);
    for j in 0..levels {
        for k in 0..sides {
            let t = j as f32 / levels as f32;
            let r = 0.25 * (1.0 - t) + 0.11;
            let a = node(j, k);
            let c = node(j + 1, k);
            let bend = (a + c) * 0.5
                + Vec3::new(
                    (k as f32 * 1.7).sin() * 0.18,
                    0.0,
                    (k as f32 * 2.1).cos() * 0.18,
                );
            organic_link(&mut b, a, bend, c, r, detail);
            let side = node(j, (k + 1) % sides);
            organic_link(
                &mut b,
                a,
                (a + side) * 0.5 + Vec3::Y * 0.2,
                side,
                r * 0.7,
                detail,
            );
            if detail > 0 && (k + j) % 3 == 0 {
                organic_link(
                    &mut b,
                    a,
                    (a + node(j + 1, (k + 1) % sides)) * 0.5,
                    node(j + 1, (k + 1) % sides),
                    r * 0.5,
                    detail,
                );
            }
        }
    }
    b.color(TIP);
    for j in 1..levels {
        let t = j as f32 / levels as f32;
        let center = Vec3::new((t * 3.3).sin() * 0.6, t * CORAL_TOP, (t * 4.1).sin() * 0.4);
        let r = 0.95 * (1.0 - t) + 0.25;
        b.ellipsoid(
            center,
            Vec3::new(r, 0.9, r),
            [3, 5, 8][detail],
            [8, 14, 22][detail],
            false,
        );
    }
    b.build()
}

fn boulder(detail: usize) -> Mesh {
    let mut mesh = super::icosphere([1, 2, 3][detail]);
    mesh.tangents = vec![super::NO_TANGENT; mesh.positions.len()];
    let noise = Perlin::new(0xB001);
    mesh.deform(|p| {
        let h = 1.0 + noise.fbm(p * 2.2, 4) * 0.24;
        Vec3::new(p.x * 1.7, p.y * 0.9 + 0.65, p.z * 1.25) * h
    });
    mesh
}

fn spikes(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    let count = [1, 3, 5][detail];
    for k in 0..count {
        let h = hash_u32(0x5E1C + k as u32);
        let unit = |shift: u32| ((h >> shift) & 0xFF) as f32 / 255.0;
        let angle = unit(0) * TAU;
        let spread = if k == 0 { 0.0 } else { 0.55 + 0.8 * unit(8) };
        let base = Vec3::new(angle.cos() * spread, -0.3, angle.sin() * spread);
        let lean = Vec3::new(angle.cos(), 0.0, angle.sin()) * (0.15 + 0.35 * unit(16));
        let height = if k == 0 { 7.0 } else { 2.6 + 4.0 * unit(24) };
        let radius = if k == 0 { 0.95 } else { 0.4 + 0.4 * unit(4) };
        b.frustum(
            base,
            (Vec3::Y + lean).normalize(),
            height,
            radius,
            0.0,
            5,
            false,
            (false, false),
        );
    }
    b.build()
}

fn wreck(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(Vec3::ONE);
    // Las placas del casco miden lo mismo en todas las caras.
    b.uv_world(Some(WRECK_UV_SIZE));
    // Casco principal: un cilindro acostado de 16 unidades, partido. De
    // lejos basta un prisma de seis caras.
    let (segments, smooth) = if detail == 0 { (6, false) } else { (12, true) };
    b.frustum(
        Vec3::new(-11.2, 0.0, 0.0),
        Vec3::X,
        16.0,
        HULL,
        2.7,
        segments,
        smooth,
        (true, false),
    );
    // El extremo partido esta abierto: por dentro, una pared oscura que mira
    // hacia adentro, para que no se vea a traves del casco.
    let inside = b.mark();
    b.color(INSIDE);
    b.frustum(
        Vec3::new(-2.0, 0.0, 0.0),
        Vec3::X,
        6.8,
        HULL * 0.92,
        2.7 * 0.92,
        segments,
        smooth,
        (true, false),
    );
    b.flip(inside);
    b.color(Vec3::ONE);
    if detail >= 1 {
        // La nariz se partio y quedo torcida a un lado.
        let nose_axis = Vec3::new(1.0, 0.18, 0.35).normalize();
        b.frustum(
            Vec3::new(6.7, 0.3, 1.45),
            nose_axis,
            7.2,
            2.55,
            0.55,
            segments,
            smooth,
            (true, false),
        );
        // Aletas.
        b.cuboid(
            Vec3::new(-8.8, 3.85, 0.0),
            tilted(0.3),
            Vec3::new(2.55, 1.45, 0.2),
        );
        let fin = [
            Vec3::X,
            Vec3::new(0.0, 0.5, 0.87),
            Vec3::new(0.0, -0.87, 0.5),
        ];
        b.cuboid(Vec3::new(-8.8, -1.9, 3.05), fin, Vec3::new(2.25, 1.3, 0.2));
    }
    if detail >= 2 {
        // Escombros (apoyados donde quedaria el suelo) y una antena doblada.
        b.cuboid(
            Vec3::new(2.4, -2.25, -4.15),
            tilted(0.7),
            Vec3::new(1.1, 0.65, 0.8),
        );
        b.cuboid(
            Vec3::new(-4.0, -2.4, 4.5),
            tilted(-0.5),
            Vec3::new(0.8, 0.8, 0.95),
        );
        let antenna = Vec3::new(0.2, 1.0, 0.1).normalize();
        b.frustum(
            Vec3::new(-1.6, 2.55, 0.0),
            antenna,
            3.85,
            0.24,
            0.13,
            4,
            false,
            (false, false),
        );
    }
    let mut mesh = b.build();
    // Armado alrededor del eje del casco; se sube para que la base quede en
    // y = 0, como los demas modelos (asi `sink` hunde lo que dice).
    for p in &mut mesh.positions {
        p.y += HULL;
    }
    mesh.update_bounds();
    mesh
}

/// Escala `mesh` desde el origen para que su punto mas alto quede en `top`.
fn fit_height(mut mesh: Mesh, top: f32) -> Mesh {
    let current = mesh.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    if current > 0.0 {
        let k = top / current;
        for p in &mut mesh.positions {
            *p = *p * k;
        }
        mesh.update_bounds();
    }
    mesh
}

/// Los modelos, en el orden de `PropKind::ALL`.
pub fn build_models() -> Vec<PropModel> {
    PropKind::ALL
        .iter()
        .map(|&kind| {
            let make = |detail: usize| match kind {
                PropKind::Eye => eye(detail),
                PropKind::Mushroom => mushroom(detail),
                PropKind::Coral => coral(detail),
                PropKind::Spike => spikes(detail),
                PropKind::Wreck => wreck(detail),
                PropKind::Boulder => boulder(detail),
                PropKind::Beacon => beacon(detail),
                PropKind::Vent => vent(detail),
                PropKind::Crystal => crystal(detail),
            };
            let mut lods = [make(2), make(1), make(0)];
            if matches!(kind, PropKind::Mushroom | PropKind::Coral) {
                let noise = Perlin::new(0xB10C);
                for mesh in &mut lods {
                    mesh.deform(|p| {
                        let top = smoothstep(4.0, 8.6, p.y);
                        let phi = p.z.atan2(p.x - 0.69);
                        let radial = 1.0
                            + top * (0.07 * (phi * 3.0 + 0.4).sin() + 0.035 * (phi * 7.0).sin());
                        let ripple =
                            top * (0.27 * (phi * 3.0 + 0.7).sin() + 0.12 * (phi * 11.0).sin());
                        Vec3::new(p.x * radial, p.y + ripple, p.z * radial)
                            + Vec3::new(
                                noise.noise(p * 0.65),
                                noise.noise(p * 0.65 + Vec3::splat(17.0)),
                                noise.noise(p * 0.65 + Vec3::splat(39.0)),
                            ) * 0.22
                    });
                }
            }

            if kind == PropKind::Coral {
                // Con menos ramas el coral queda mas bajo: los niveles burdos
                // se estiran a la altura del fino para que no cambie de tamano
                // al pasar de uno a otro.
                lods = lods.map(|m| fit_height(m, CORAL_TOP));
            }
            PropModel { lods }
        })
        .collect()
}

/// Regla de distribucion de un tipo de objeto sobre un planeta.
pub struct Scatter {
    pub kind: PropKind,
    /// Celdas por lado de cada cara del cubo (candidatos = 6 * celdas^2).
    pub cells: usize,
    /// Escala minima y maxima.
    pub scale: (f32, f32),
    /// Cuanto se hunde en el suelo, en unidades del modelo.
    pub sink: f32,
    /// Si se para sobre la normal o queda tirado en cualquier angulo.
    pub upright: bool,
    /// Probabilidad de aceptar un candidato: recibe el ruido de densidad (0
    /// a 1), la muestra del terreno y la pendiente.
    pub accept: fn(f32, &TerrainSample, f32) -> f32,
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Coloca los objetos de un planeta. Determinista: depende solo de `seed`.
pub fn scatter(
    radius: f64,
    field: &dyn HeightField,
    rules: &[Scatter],
    seed: u32,
) -> Vec<Instance> {
    let noise = Perlin::new(seed);
    let mut instances = Vec::new();
    for (rule_index, rule) in rules.iter().enumerate() {
        let n = rule.cells;
        // Una fila de celdas por trabajo; cada trabajo deja sus objetos aparte.
        let mut rows: Vec<Vec<Instance>> = vec![Vec::new(); 6 * n];
        let out = DisjointMut::new(&mut rows);
        parallel::for_each(6 * n, |row| {
            let (face, j) = ((row / n) as u8, row % n);
            // SAFETY: cada fila escribe solo su propia casilla.
            let found = unsafe { &mut out.range(row, 1)[0] };
            for i in 0..n {
                let h = hash_u32(
                    seed ^ hash_u32(
                        (rule_index as u32) << 24
                            ^ (face as u32) << 20
                            ^ (j as u32) << 10
                            ^ i as u32,
                    ),
                );
                let h2 = hash_u32(h);
                let u = -1.0 + 2.0 * (i as f64 + unit(h) as f64) / n as f64;
                let v = -1.0 + 2.0 * (j as f64 + unit(h2) as f64) / n as f64;
                let dir = cube_to_sphere(face, u, v);
                let d = dir.as_vec3();
                // Prueba barata primero: el ruido de densidad hace grupos y claros.
                let density = noise.fbm(d * 3.0 + Vec3::new(rule_index as f32 * 7.7, 1.3, -2.1), 3)
                    * 0.5
                    + 0.5;
                if density < 0.3 {
                    continue;
                }
                let sample = field.sample(d);
                // Pendiente: cuanto cambia la altura a medio metro en dos direcciones.
                let (t1, t2) = crate::scene::builder::perpendiculars(d);
                let step = 0.5 / radius as f32;
                let hx = field.sample((d + t1 * step).normalize()).height - sample.height;
                let hz = field.sample((d + t2 * step).normalize()).height - sample.height;
                let slope = (hx * hx + hz * hz).sqrt() / 0.5;
                let h3 = hash_u32(h2);
                if unit(h3) >= (rule.accept)(density, &sample, slope) {
                    continue;
                }
                let h4 = hash_u32(h3);
                let h5 = hash_u32(h4);
                let scale = rule.scale.0 + (rule.scale.1 - rule.scale.0) * unit(h4);
                let yaw = unit(h5) as f64 * std::f64::consts::TAU;
                let up = if rule.upright {
                    dir
                } else {
                    // Tirado: inclinado hasta unos 23 grados al azar.
                    let h6 = hash_u32(h5);
                    let (a, b) = crate::scene::builder::perpendiculars(d);
                    let lean =
                        (a * (unit(h6) - 0.5) + b * (unit(hash_u32(h6)) - 0.5)).as_dvec3() * 0.6;
                    (dir + lean).normalize()
                };
                let rotation = upright_rotation(up, yaw);
                let ground = radius + sample.height as f64 - (rule.sink * scale) as f64;
                found.push(Instance {
                    kind: rule.kind,
                    position: dir * ground,
                    rotation,
                    scale,
                });
            }
        });
        instances.extend(rows.into_iter().flatten());
    }
    instances
}

/// Orientacion con +Y sobre `up` y un giro `yaw` alrededor de el.
fn upright_rotation(up: DVec3, yaw: f64) -> DQuat {
    let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let x = helper.cross(up).normalize();
    let z = x.cross(up);
    DQuat::from_mat3(&DMat3::from_cols(x, up, z)) * DQuat::from_axis_angle(DVec3::Y, yaw)
}

/// Colonias grandes a ambos lados de un claro transitable. La distribución
/// global continúa fuera del bioma; este punto de interés evita un horizonte
/// vacío al llegar a Jardín. Todas las bases usan la misma altura del terreno.
pub fn garden_grove(
    instances: &mut Vec<Instance>,
    radius: f64,
    field: &dyn HeightField,
    seed: u32,
    frame: (DVec3, DVec3, DVec3),
) {
    let (n, forward, right) = frame;
    instances.retain(|p| {
        let d = p.position - n * radius;
        let x = d.dot(right);
        let z = d.dot(forward);
        !(x.abs() < 9.0 && z > -20.0 && z < 110.0)
    });
    for row in 0..5 {
        for side in [-1.0, 1.0] {
            for col in 0..2 {
                let id = (row * 4 + col * 2 + if side < 0.0 { 0 } else { 1 }) as u32;
                let h = hash_u32(seed ^ id ^ 0xA11E_6805);
                let x = side * (20.0 + col as f64 * 22.0 + (unit(h) as f64 - 0.5) * 4.0);
                let z = 14.0 + row as f64 * 20.0 + (unit(hash_u32(h)) as f64 - 0.5) * 6.0;
                let dir = (n + (right * x + forward * z) / radius).normalize();
                let kind = if side > 0.0 && (row + col) % 2 == 0 {
                    PropKind::Coral
                } else {
                    PropKind::Mushroom
                };
                let scale = if kind == PropKind::Coral {
                    2.5 + unit(h) * 1.4
                } else {
                    2.1 + unit(h) * 1.3
                };
                let height = field.sample(dir.as_vec3()).height as f64;
                instances.push(Instance {
                    kind,
                    position: dir * (radius + height - 0.3 * scale as f64),
                    rotation: upright_rotation(
                        dir,
                        unit(hash_u32(h ^ 17)) as f64 * std::f64::consts::TAU,
                    ),
                    scale,
                });
            }
        }
    }
}

/// Los objetos de un planeta.
#[derive(Default)]
pub struct PropSet {
    pub instances: Vec<Instance>,
    /// Radio envolvente del objeto mas grande (para descartar el planeta
    /// entero cuando todo se veria diminuto).
    pub max_radius: f64,
}

impl PropSet {
    pub fn new(instances: Vec<Instance>, models: &[PropModel]) -> Self {
        let max_radius = instances
            .iter()
            .map(|i| models[i.kind.index()].lods[0].bounds_radius as f64 * i.scale as f64)
            .fold(0.0, f64::max);
        Self {
            instances,
            max_radius,
        }
    }
}

/// Lo que paso con los objetos de la superficie en el ultimo frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct PropStats {
    /// Objetos de los planetas revisados.
    pub considered: usize,
    /// Dibujados en cada nivel de detalle.
    pub drawn: [usize; 3],
    /// Descartados por verse mas chicos que `PROP_MIN_PIXELS`.
    pub small: usize,
    pub horizon: usize,
    pub frustum: usize,
}

impl PropStats {
    pub fn drawn_total(&self) -> usize {
        self.drawn.iter().sum()
    }
}

/// Lo que la seleccion necesita saber de la camara, todo en el espacio del
/// planeta.
pub struct PropView<'a> {
    pub camera: DVec3,
    /// Radio de la esfera que tapa: el radio con la altura minima del terreno.
    pub occluder: f64,
    /// Radio con la altura maxima del terreno.
    pub surface_top: f64,
    /// Angulo que cubre un pixel, en radianes.
    pub pixel_angle: f64,
    /// Si una esfera (centro, radio) cae dentro del frustum.
    pub visible: &'a dyn Fn(DVec3, f64) -> bool,
}

/// Un objeto elegido para dibujarse.
#[derive(Clone, Copy, Debug)]
pub struct Chosen {
    pub index: usize,
    pub lod: usize,
    /// De 0 a 1: cuanto de su tamano conserva (se encoge al alejarse).
    pub grow: f32,
}

/// Elige los objetos de un planeta que se dibujan y su nivel de detalle.
pub fn select(
    set: &PropSet,
    models: &[PropModel],
    view: &PropView,
    stats: &mut PropStats,
    out: &mut Vec<Chosen>,
) {
    let count = set.instances.len();
    stats.considered += count;
    // Si hasta el objeto mas grande, en el punto mas cercano de la superficie,
    // se veria diminuto, no hace falta revisar uno por uno.
    let d = view.camera.length();
    let nearest = (d - view.surface_top).max(0.0);
    if set.max_radius < nearest * view.pixel_angle * config::PROP_MIN_PIXELS as f64 {
        stats.small += count;
        return;
    }
    let camera_horizon = if d > view.occluder {
        (view.occluder / d).acos()
    } else {
        f64::INFINITY
    };
    let flora_fade = 1.0 - smoothstep(60.0, 160.0, (d - view.surface_top).max(0.0) as f32);
    for (index, instance) in set.instances.iter().enumerate() {
        let vegetation = matches!(instance.kind, PropKind::Mushroom | PropKind::Coral);
        if vegetation && flora_fade <= 0.0 {
            stats.small += 1;
            continue;
        }
        let mesh = &models[instance.kind.index()].lods[0];
        let scale = instance.scale as f64;
        let center = instance.position
            + instance
                .rotation
                .rotate(mesh.bounds_center.as_dvec3() * scale);
        let radius = mesh.bounds_radius as f64 * scale;
        let distance = (center - view.camera).length().max(1e-3);
        // Radio en pixeles de la esfera envolvente; para decidir si se
        // dibuja cuenta segun el tipo.
        let pixels = (radius / (distance * view.pixel_angle)) as f32;
        let presence = pixels * instance.kind.visibility();
        if presence < config::PROP_MIN_PIXELS {
            stats.small += 1;
            continue;
        }
        // Horizonte: la misma prueba que los chunks del terreno.
        let c = center.length();
        if camera_horizon.is_finite() && c > radius {
            let horizon = camera_horizon + (view.occluder / (c + radius).max(view.occluder)).acos();
            let angle = (view.camera.dot(center) / (d * c)).clamp(-1.0, 1.0).acos();
            if angle - (radius / c).asin() > horizon {
                stats.horizon += 1;
                continue;
            }
        }
        if !(view.visible)(center, radius) {
            stats.frustum += 1;
            continue;
        }
        let [fine, medium] = config::PROP_LOD_PIXELS;
        let lod = if pixels > fine {
            0
        } else if pixels > medium {
            1
        } else {
            2
        };
        stats.drawn[lod] += 1;
        out.push(Chosen {
            index,
            lod,
            grow: smoothstep(config::PROP_MIN_PIXELS, config::PROP_FULL_PIXELS, presence)
                * if vegetation { flora_fade } else { 1.0 },
        });
    }
}

/// Reglas de cada planeta, por nombre.
pub fn rules_for(planet: &str) -> Vec<Scatter> {
    match planet {
        // Ojos en grupos sobre la carne, en lo alto de los pliegues y nunca
        // en el fondo de los surcos.
        "Argos" => vec![Scatter {
            kind: PropKind::Eye,
            cells: 34,
            scale: (2.5, 9.0),
            sink: 0.2,
            upright: true,
            accept: |density, s, slope| {
                (1.0 - s.attrs.y)
                    * smoothstep(0.9, 0.3, slope)
                    * smoothstep(0.5, 0.7, density)
                    * 0.6
            },
        }],
        // Hongos en los valles, corales en las crestas.
        "Jardin" => vec![
            Scatter {
                kind: PropKind::Boulder,
                cells: 75,
                scale: (0.45, 3.2),
                sink: 0.35,
                upright: true,
                accept: |density, _, slope| {
                    smoothstep(0.8, 0.2, slope) * smoothstep(0.35, 0.7, density) * 0.25
                },
            },
            Scatter {
                kind: PropKind::Mushroom,
                cells: 75,
                scale: (0.75, 3.5),
                sink: 0.3,
                upright: true,
                accept: |density, s, slope| {
                    smoothstep(0.7, 0.3, s.attrs.z)
                        * smoothstep(0.8, 0.3, slope)
                        * smoothstep(0.45, 0.65, density)
                        * 0.6
                },
            },
            Scatter {
                kind: PropKind::Coral,
                cells: 60,
                scale: (1.0, 4.0),
                sink: 0.3,
                upright: true,
                accept: |density, s, slope| {
                    smoothstep(0.45, 0.8, s.attrs.z)
                        * smoothstep(0.9, 0.4, slope)
                        * smoothstep(0.5, 0.7, density)
                        * 0.5
                },
            },
        ],
        // En la roca enfriada, lejos del mar de lava.
        "Pira" => vec![Scatter {
            kind: PropKind::Spike,
            cells: 30,
            scale: (0.6, 1.4),
            sink: 0.5,
            upright: true,
            accept: |density, s, _| {
                (1.0 - s.attrs.x) * (1.0 - 0.6 * s.attrs.y) * smoothstep(0.45, 0.68, density) * 0.6
            },
        }],
        // Cristales en las crestas y junto al agua, nunca en ella.
        "Parpado" => vec![Scatter {
            kind: PropKind::Crystal,
            cells: 28,
            scale: (0.6, 1.7),
            sink: 0.6,
            upright: true,
            accept: |density, s, slope| {
                (1.0 - s.attrs.x)
                    * (0.2 + 0.8 * s.attrs.y.max(s.attrs.z * 0.7))
                    * smoothstep(0.9, 0.35, slope)
                    * smoothstep(0.5, 0.7, density)
                    * 0.6
            },
        }],
        // Naves estrelladas en las zanjas y junto a los crateres.
        "Cementerio" => vec![Scatter {
            kind: PropKind::Wreck,
            cells: 13,
            scale: (0.8, 1.25),
            sink: 1.2,
            upright: false,
            accept: |density, s, _| {
                (s.attrs.y.max(s.attrs.x * 0.6) + 0.15) * smoothstep(0.4, 0.65, density) * 0.8
            },
        }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::shapes::ArgosField;

    #[test]
    fn same_seed_same_places_and_another_seed_moves_them() {
        let field = ArgosField::new(3);
        let rules = rules_for("Argos");
        let a = scatter(160.0, &field, &rules, 42);
        let b = scatter(160.0, &field, &rules, 42);
        let c = scatter(160.0, &field, &rules, 43);
        assert!(!a.is_empty());
        assert_eq!(a.len(), b.len());
        assert!(
            a.iter()
                .zip(&b)
                .all(|(x, y)| x.position == y.position && x.scale == y.scale)
        );
        assert!(a.len() != c.len() || a.iter().zip(&c).any(|(x, y)| x.position != y.position));
    }

    #[test]
    fn instances_stand_on_the_surface_pointing_up() {
        let field = ArgosField::new(3);
        let instances = scatter(160.0, &field, &rules_for("Argos"), 7);
        for i in instances.iter().take(200) {
            let up = i.rotation.rotate(DVec3::Y);
            let radial = i.position.normalize();
            assert!(
                up.dot(radial) > 0.999,
                "el objeto debe pararse sobre la normal radial"
            );
            let ground = 160.0 + field.sample(radial.as_vec3()).height as f64;
            assert!((i.position.length() - ground).abs() < 2.0);
        }
    }

    #[test]
    fn selection_picks_detail_by_screen_size_and_hides_the_far_side() {
        let models = build_models();
        let field = ArgosField::new(3);
        let set = PropSet::new(scatter(160.0, &field, &rules_for("Argos"), 5), &models);
        let (low, high) = field.height_range();
        let everything = |_: DVec3, _: f64| true;
        let run = |camera: DVec3| {
            let view = PropView {
                camera,
                occluder: 160.0 + low as f64,
                surface_top: 160.0 + high as f64,
                pixel_angle: 0.002,
                visible: &everything,
            };
            let (mut stats, mut out) = (PropStats::default(), Vec::new());
            select(&set, &models, &view, &mut stats, &mut out);
            (stats, out)
        };
        // Pegada a un objeto: ese se dibuja con el detalle fino, y lo que
        // queda del otro lado del planeta no se dibuja.
        let first = set.instances[0];
        let (stats, out) = run(first.position * 1.02);
        assert!(
            out.iter()
                .any(|c| c.index == 0 && c.lod == 0 && c.grow == 1.0),
            "{stats:?}"
        );
        assert!(stats.horizon > set.instances.len() / 3, "{stats:?}");
        // Muy lejos: todo diminuto, nada se dibuja.
        let (stats, out) = run(DVec3::new(0.0, 0.0, 1e6));
        assert!(out.is_empty() && stats.small == set.instances.len());
    }

    #[test]
    fn every_model_has_three_levels_with_less_detail_each_time() {
        for (kind, model) in PropKind::ALL.iter().zip(build_models()) {
            let tris: Vec<usize> = model.lods.iter().map(|m| m.indices.len()).collect();
            assert!(
                tris[0] >= tris[1] && tris[1] >= tris[2],
                "{kind:?}: {tris:?}"
            );
            assert!(
                model
                    .lods
                    .iter()
                    .all(|m| m.tangents.len() == m.positions.len())
            );
        }
    }
}
