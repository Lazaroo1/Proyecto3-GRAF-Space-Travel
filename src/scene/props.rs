//! Objetos de la superficie: modelos procedurales, distribucion y detalle.
//!
//! # Modelos
//! Seis tipos, todos generados con codigo (`MeshBuilder`), cada uno en tres
//! niveles de detalle: monolitos y obeliscos (Requiem), hongos y corales
//! (Jardin), espinas de obsidiana (Pira) y naves naufragas (Cementerio).
//!
//! # Distribucion
//! Cada cara del cubo-esfera se divide en una grilla; cada celda propone un
//! candidato en un punto al azar dentro de ella (un hash de la semilla, la
//! cara y la celda). El candidato se acepta con una probabilidad que sale de
//! **ruido fractal** (asi los objetos forman grupos y claros, no una lluvia
//! pareja) multiplicado por lo que dice el terreno en ese punto: altura,
//! pendiente y mascaras (no hay hongos en la lava ni monolitos en el hielo).
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
//! esfera envolvente en pixeles), no por la distancia: un monolito grande
//! conserva su detalle mas lejos que un hongo, y todo se adapta solo a la
//! resolucion. Se descartan los que se verian de menos de un par de pixeles,
//! los que quedan detras del horizonte del planeta y los que caen fuera del
//! frustum. Cerca del limite de tamano el objeto se encoge hacia el suelo en
//! vez de desaparecer de golpe. Los contadores van al HUD.

use std::f32::consts::TAU;

use super::builder::MeshBuilder;
use super::Mesh;
use crate::config;
use crate::math::{hash_u32, smoothstep, DMat3, DQuat, DVec3, Perlin, Vec3};
use crate::parallel::{self, DisjointMut};
use crate::terrain::{cube_to_sphere, HeightField, TerrainSample};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Monolith,
    Obelisk,
    Mushroom,
    Coral,
    Spike,
    Wreck,
}

impl PropKind {
    pub const ALL: [Self; 6] = [
        Self::Monolith,
        Self::Obelisk,
        Self::Mushroom,
        Self::Coral,
        Self::Spike,
        Self::Wreck,
    ];

    /// Posicion en `ALL` (y en la lista de modelos).
    pub fn index(self) -> usize {
        self as usize
    }

    /// Cuanto cuenta su tamano en pantalla para decidir si se dibuja. Los
    /// naufragios (1) se ven desde lejos; los monumentos, delgados, un poco
    /// menos (en el borde del planeta parecerian pelos); la vegetacion y las
    /// espinas (0.5) necesitan el doble de tamano: desde la orbita solo
    /// ensuciarian.
    pub fn visibility(self) -> f32 {
        match self {
            Self::Wreck => 1.0,
            Self::Monolith | Self::Obelisk => 0.75,
            Self::Mushroom | Self::Coral | Self::Spike => 0.5,
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

// Colores de vertice: algunos shaders los usan como marcas.
const STONE: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const GLYPH_FACE: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const STEM: Vec3 = Vec3::new(0.72, 0.66, 0.6);
const CAP: Vec3 = Vec3::new(0.36, 0.05, 0.3);
/// Bulbos del coral (el verde en 0 marca la punta que brilla).
const TIP: Vec3 = Vec3::new(1.0, 0.0, 1.0);
/// Paredes de adentro del casco: el shader las oscurece con el rojo.
const INSIDE: Vec3 = Vec3::new(0.15, 1.0, 1.0);

/// Ejes de una caja girada `angle` alrededor de z.
fn tilted(angle: f32) -> [Vec3; 3] {
    let (s, c) = angle.sin_cos();
    [Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0), Vec3::Z]
}

fn monolith(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    // Losa principal: el frente y la espalda (las caras anchas, sus primeros
    // ocho vertices) llevan el glifo del ojo.
    let slab = b.cuboid(Vec3::new(0.0, 7.5, 0.0), [Vec3::X, Vec3::Y, Vec3::Z], Vec3::new(1.2, 7.5, 0.45));
    b.paint(slab..slab + 8, GLYPH_FACE);
    if detail >= 1 {
        // Remate roto e inclinado.
        b.cuboid(Vec3::new(0.15, 15.65, 0.0), tilted(0.22), Vec3::new(1.0, 0.8, 0.38));
    }
    if detail >= 2 {
        // Escombros al pie.
        // El suelo queda en y = 0.8 (el monolito se hunde eso).
        b.cuboid(Vec3::new(1.9, 1.0, 0.9), tilted(0.5), Vec3::new(0.5, 0.35, 0.35));
        b.cuboid(Vec3::new(-1.7, 0.95, -0.7), tilted(-0.4), Vec3::new(0.35, 0.3, 0.5));
    }
    b.build()
}

fn obelisk(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(STONE);
    let base = if detail >= 1 {
        b.cuboid(Vec3::new(0.0, 0.6, 0.0), [Vec3::X, Vec3::Y, Vec3::Z], Vec3::new(1.6, 0.6, 1.6));
        1.2
    } else {
        0.0
    };
    // Fuste de seccion cuadrada (cuatro lados) y piramidion.
    b.frustum(Vec3::new(0.0, base, 0.0), Vec3::Y, 16.0, 1.25, 0.85, 4, false, (false, false));
    b.frustum(Vec3::new(0.0, base + 16.0, 0.0), Vec3::Y, 2.2, 0.85, 0.0, 4, false, (false, false));
    b.build()
}

fn mushroom(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    if detail == 0 {
        // De lejos basta la silueta: tallo de cuatro caras y sombrero conico.
        b.color(STEM);
        b.frustum(Vec3::ZERO, Vec3::new(0.12, 1.0, 0.0).normalize(), 6.2, 0.7, 0.45, 4, false, (false, false));
        b.color(CAP);
        b.frustum(Vec3::new(0.69, 6.1, 0.0), Vec3::Y, 1.9, 3.6, 0.0, 6, false, (true, false));
        return b.build();
    }
    let segments = [5, 9, 14][detail];
    let rings = [3, 4, 6][detail];
    b.color(STEM);
    // Tallo curvo que se adelgaza.
    let path: Vec<Vec3> = (0..=4)
        .map(|k| {
            let t = k as f32 / 4.0;
            Vec3::new((t * 1.4).sin() * 0.7, t * 6.5, 0.0)
        })
        .collect();
    let radii: Vec<f32> = (0..=4).map(|k| 0.75 - 0.3 * k as f32 / 4.0).collect();
    b.tube(&path, &radii, segments);
    b.color(CAP);
    let top = path[4] + Vec3::new(0.0, -0.4, 0.0);
    b.ellipsoid(top, Vec3::new(3.6, 1.9, 3.6), rings, segments * 2, true);
    b.build()
}

/// Rama de coral: un tubo curvo que se abre en dos o tres ramas mas chicas.
fn coral_branch(b: &mut MeshBuilder, origin: Vec3, dir: Vec3, length: f32, radius: f32, depth: usize, segments: usize, seed: u32) {
    let path: Vec<Vec3> = (0..=3)
        .map(|k| {
            let t = k as f32 / 3.0;
            // Se curva hacia arriba al crecer, como buscando luz.
            let bend = Vec3::Y * (t * t * length * 0.25);
            origin + dir * (t * length) + bend
        })
        .collect();
    let radii: Vec<f32> = (0..=3).map(|k| radius * (1.0 - 0.35 * k as f32 / 3.0)).collect();
    b.tube(&path, &radii, segments);
    let end = path[3];
    if depth == 0 {
        // Bulbo en la punta: el color lo marca para que brille.
        b.color(TIP);
        // De cerca el bulbo se ve redondo; de lejos bastan pocas caras.
        let (rings, around) = if segments >= 6 { (5, segments * 2) } else { (3, segments) };
        b.ellipsoid(end, Vec3::splat(radius * 1.35), rings, around, false);
        b.color(Vec3::ONE);
        return;
    }
    // Nudo que cierra la union con las ramas hijas (sin el, el tubo abierto
    // dejaria ver a traves).
    b.ellipsoid(end, Vec3::splat(radius * 0.7), 2, segments, false);
    let branches = 3 + (hash_u32(seed) % 2) as usize;
    for k in 0..branches {
        let h = hash_u32(seed.wrapping_add(k as u32 * 7919));
        // Ramas repartidas alrededor, con un poco de azar.
        let angle = (k as f32 + 0.6 * unit(h)) / branches as f32 * TAU;
        let side = Vec3::new(angle.cos(), 0.0, angle.sin());
        let child = (dir + side * 0.8 + Vec3::Y * 0.35).normalize();
        coral_branch(b, end, child, length * 0.72, radius * 0.68, depth - 1, segments, h);
    }
}

fn coral(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    b.color(Vec3::ONE);
    let (depth, segments) = [(0, 4), (1, 5), (2, 6)][detail];
    coral_branch(&mut b, Vec3::ZERO, Vec3::Y, 4.5, 1.0, depth, segments, 0xC0A1);
    b.build()
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
        b.frustum(base, (Vec3::Y + lean).normalize(), height, radius, 0.0, 5, false, (false, false));
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
    b.frustum(Vec3::new(-11.2, 0.0, 0.0), Vec3::X, 16.0, HULL, 2.7, segments, smooth, (true, false));
    // El extremo partido esta abierto: por dentro, una pared oscura que mira
    // hacia adentro, para que no se vea a traves del casco.
    let inside = b.mark();
    b.color(INSIDE);
    b.frustum(Vec3::new(-2.0, 0.0, 0.0), Vec3::X, 6.8, HULL * 0.92, 2.7 * 0.92, segments, smooth, (true, false));
    b.flip(inside);
    b.color(Vec3::ONE);
    if detail >= 1 {
        // La nariz se partio y quedo torcida a un lado.
        let nose_axis = Vec3::new(1.0, 0.18, 0.35).normalize();
        b.frustum(Vec3::new(6.7, 0.3, 1.45), nose_axis, 7.2, 2.55, 0.55, segments, smooth, (true, false));
        // Aletas.
        b.cuboid(Vec3::new(-8.8, 3.85, 0.0), tilted(0.3), Vec3::new(2.55, 1.45, 0.2));
        let fin = [Vec3::X, Vec3::new(0.0, 0.5, 0.87), Vec3::new(0.0, -0.87, 0.5)];
        b.cuboid(Vec3::new(-8.8, -1.9, 3.05), fin, Vec3::new(2.25, 1.3, 0.2));
    }
    if detail >= 2 {
        // Escombros (apoyados donde quedaria el suelo) y una antena doblada.
        b.cuboid(Vec3::new(2.4, -2.25, -4.15), tilted(0.7), Vec3::new(1.1, 0.65, 0.8));
        b.cuboid(Vec3::new(-4.0, -2.4, 4.5), tilted(-0.5), Vec3::new(0.8, 0.8, 0.95));
        let antenna = Vec3::new(0.2, 1.0, 0.1).normalize();
        b.frustum(Vec3::new(-1.6, 2.55, 0.0), antenna, 3.85, 0.24, 0.13, 4, false, (false, false));
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

/// Los seis modelos, en el orden de `PropKind::ALL`.
pub fn build_models() -> Vec<PropModel> {
    PropKind::ALL
        .iter()
        .map(|&kind| {
            let make = |detail: usize| match kind {
                PropKind::Monolith => monolith(detail),
                PropKind::Obelisk => obelisk(detail),
                PropKind::Mushroom => mushroom(detail),
                PropKind::Coral => coral(detail),
                PropKind::Spike => spikes(detail),
                PropKind::Wreck => wreck(detail),
            };
            let mut lods = [make(2), make(1), make(0)];
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
pub fn scatter(radius: f64, field: &dyn HeightField, rules: &[Scatter], seed: u32) -> Vec<Instance> {
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
                let h = hash_u32(seed ^ hash_u32((rule_index as u32) << 24 ^ (face as u32) << 20 ^ (j as u32) << 10 ^ i as u32));
                let h2 = hash_u32(h);
                let u = -1.0 + 2.0 * (i as f64 + unit(h) as f64) / n as f64;
                let v = -1.0 + 2.0 * (j as f64 + unit(h2) as f64) / n as f64;
                let dir = cube_to_sphere(face, u, v);
                let d = dir.as_vec3();
                // Prueba barata primero: el ruido de densidad hace grupos y claros.
                let density = noise.fbm(d * 3.0 + Vec3::new(rule_index as f32 * 7.7, 1.3, -2.1), 3) * 0.5 + 0.5;
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
                    let lean = (a * (unit(h6) - 0.5) + b * (unit(hash_u32(h6)) - 0.5)).as_dvec3() * 0.6;
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
        Self { instances, max_radius }
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
pub fn select(set: &PropSet, models: &[PropModel], view: &PropView, stats: &mut PropStats, out: &mut Vec<Chosen>) {
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
    let camera_horizon = if d > view.occluder { (view.occluder / d).acos() } else { f64::INFINITY };
    for (index, instance) in set.instances.iter().enumerate() {
        let mesh = &models[instance.kind.index()].lods[0];
        let scale = instance.scale as f64;
        let center = instance.position + instance.rotation.rotate(mesh.bounds_center.as_dvec3() * scale);
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
            grow: smoothstep(config::PROP_MIN_PIXELS, config::PROP_FULL_PIXELS, presence),
        });
    }
}

/// Reglas de cada planeta, por nombre.
pub fn rules_for(planet: &str) -> Vec<Scatter> {
    match planet {
        // Las ruinas: en tierra firme y plana, nunca en las llanuras de hielo.
        "Requiem" => vec![
            Scatter {
                kind: PropKind::Monolith,
                cells: 22,
                scale: (0.8, 1.3),
                sink: 0.8,
                upright: true,
                accept: |density, s, slope| (1.0 - s.attrs.x) * smoothstep(0.6, 0.2, slope) * smoothstep(0.55, 0.7, density) * 0.7,
            },
            Scatter {
                kind: PropKind::Obelisk,
                cells: 14,
                scale: (0.8, 1.2),
                sink: 0.5,
                upright: true,
                accept: |density, s, slope| (1.0 - s.attrs.x) * smoothstep(0.4, 0.1, slope) * smoothstep(0.6, 0.72, density) * 0.6,
            },
        ],
        // Hongos en los valles, corales en las crestas.
        "Jardin" => vec![
            Scatter {
                kind: PropKind::Mushroom,
                cells: 30,
                scale: (0.5, 1.25),
                sink: 0.3,
                upright: true,
                accept: |density, s, slope| smoothstep(0.7, 0.3, s.attrs.z) * smoothstep(0.8, 0.3, slope) * smoothstep(0.45, 0.65, density) * 0.6,
            },
            Scatter {
                kind: PropKind::Coral,
                cells: 24,
                scale: (0.7, 1.3),
                sink: 0.3,
                upright: true,
                accept: |density, s, slope| smoothstep(0.45, 0.8, s.attrs.z) * smoothstep(0.9, 0.4, slope) * smoothstep(0.5, 0.7, density) * 0.5,
            },
        ],
        // En la roca enfriada, lejos del mar de lava.
        "Pira" => vec![Scatter {
            kind: PropKind::Spike,
            cells: 30,
            scale: (0.6, 1.4),
            sink: 0.5,
            upright: true,
            accept: |density, s, _| (1.0 - s.attrs.x) * (1.0 - 0.6 * s.attrs.y) * smoothstep(0.45, 0.68, density) * 0.6,
        }],
        // Naves estrelladas en las zanjas y junto a los crateres.
        "Cementerio" => vec![Scatter {
            kind: PropKind::Wreck,
            cells: 13,
            scale: (0.8, 1.25),
            sink: 1.2,
            upright: false,
            accept: |density, s, _| (s.attrs.y.max(s.attrs.x * 0.6) + 0.15) * smoothstep(0.4, 0.65, density) * 0.8,
        }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::shapes::RequiemField;

    #[test]
    fn same_seed_same_places_and_another_seed_moves_them() {
        let field = RequiemField::new(3);
        let rules = rules_for("Requiem");
        let a = scatter(160.0, &field, &rules, 42);
        let b = scatter(160.0, &field, &rules, 42);
        let c = scatter(160.0, &field, &rules, 43);
        assert!(!a.is_empty());
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(x, y)| x.position == y.position && x.scale == y.scale));
        assert!(a.len() != c.len() || a.iter().zip(&c).any(|(x, y)| x.position != y.position));
    }

    #[test]
    fn instances_stand_on_the_surface_pointing_up() {
        let field = RequiemField::new(3);
        let instances = scatter(160.0, &field, &rules_for("Requiem"), 7);
        for i in instances.iter().take(200) {
            let up = i.rotation.rotate(DVec3::Y);
            let radial = i.position.normalize();
            assert!(up.dot(radial) > 0.999, "el objeto debe pararse sobre la normal radial");
            let ground = 160.0 + field.sample(radial.as_vec3()).height as f64;
            assert!((i.position.length() - ground).abs() < 2.0);
        }
    }

    #[test]
    fn selection_picks_detail_by_screen_size_and_hides_the_far_side() {
        let models = build_models();
        let field = RequiemField::new(3);
        let set = PropSet::new(scatter(160.0, &field, &rules_for("Requiem"), 5), &models);
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
        assert!(out.iter().any(|c| c.index == 0 && c.lod == 0 && c.grow == 1.0), "{stats:?}");
        assert!(stats.horizon > set.instances.len() / 3, "{stats:?}");
        // Muy lejos: todo diminuto, nada se dibuja.
        let (stats, out) = run(DVec3::new(0.0, 0.0, 1e6));
        assert!(out.is_empty() && stats.small == set.instances.len());
    }

    #[test]
    fn every_model_has_three_levels_with_less_detail_each_time() {
        for (kind, model) in PropKind::ALL.iter().zip(build_models()) {
            let tris: Vec<usize> = model.lods.iter().map(|m| m.indices.len()).collect();
            assert!(tris[0] >= tris[1] && tris[1] >= tris[2], "{kind:?}: {tris:?}");
            assert!(model.lods.iter().all(|m| m.tangents.len() == m.positions.len()));
        }
    }
}
