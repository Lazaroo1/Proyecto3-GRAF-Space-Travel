//! Meyer, el personaje de Blender que corre en Talega Surfers.
//!
//! El modelo viene en una sola pose (saludando con la mano derecha), sin
//! esqueleto. Para que corra se le arman las poses aqui, girando sus piezas:
//!
//! - **Las piernas** (los jeans, los zapatos y lo que va pegado a ellos)
//!   vienen como piezas aparte, una por lado: cada una gira por la rodilla
//!   (lo de abajo de la rodilla, hacia atras) y despues por la cadera
//!   (hacia adelante o hacia atras).
//! - **El brazo de abajo**: la mano es pieza aparte, pero la manga es parte
//!   de la guerrera (cuerpo y mangas en una sola malla). Cada vertice de la
//!   guerrera de ese lado gira por el hombro con un peso que sube con lo
//!   afuera que esta del torso (`smoothstep`), y la posicion es la mezcla de
//!   la de antes y la girada: un esqueleto de un hueso, de los mas simples.
//! - **El brazo del saludo** se queda saludando: Meyer corre saludando.
//!
//! Un giro alrededor del eje x con pivote `p` (y arriba, z al frente):
//! `y' = p.y + dy cos a + dz sen a` y `z' = p.z + dz cos a - dy sen a`; con `a`
//! positivo lo de abajo del pivote va hacia adelante. Las normales giran
//! igual, sin trasladarse.
//!
//! **La cara** viene con textura (`map_Kd`): una imagen de la referencia
//! proyectada sobre su malla, que es fina (unas 70 mil caras). Se guarda
//! en PNG a 1024 x 1024 (`meyer-rostro-color.png`, RGB de 8 bits) y cada
//! esquina toma el color de su coordenada de textura (bilineal), pasado de
//! sRGB a lineal como los colores del `.mtl`: con una malla asi de fina, el
//! color por vertice conserva el detalle a los tamanos en que se ve Meyer.
//!
//! Con cada pose se hornea un sprite (`hornear`) desde atras y desde la
//! altura de la camara del juego: ocho de la carrera, uno del salto y uno
//! agachado (en cuclillas: las rodillas dobladas, todo el cuerpo inclinado
//! hacia adelante por la cadera y la mano de abajo hacia adelante; la otra
//! sigue saludando). Se
//! hornean una vez por tamano de pantalla y se guardan.
//!
//! En los shucos llega de cliente (`customer`): un cuadro de frente,
//! parado saludando, y ocho caminando de lado (con pasos mas cortos que los
//! de la carrera), vistos a la altura de los ojos.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use super::hornear::{self, Finish, Sprite};
use crate::math::{Vec2, Vec3, smoothstep};
use crate::scene::{Mesh, Surface};

/// Cuanto mide Meyer en el juego (m).
pub const HEIGHT: f32 = 1.75;
/// Los cuadros de la carrera.
pub const RUN_FRAMES: usize = 8;

/// A que parte se mueve cada vertice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Body,
    /// Pierna izquierda (x < 0) o derecha.
    Leg(i8),
    /// La mano del brazo de abajo.
    Hand,
    /// La guerrera (el brazo de abajo sale de aqui con su peso).
    Jacket,
}

/// El modelo leido: la malla y la parte de cada vertice, mas las medidas
/// para las poses.
pub struct Character {
    mesh: Mesh,
    parts: Vec<Part>,
    /// Lo alto en unidades del modelo, la cadera, la rodilla, el centro de
    /// cada pierna (x, z), el hombro del brazo de abajo, de que lado esta
    /// ese brazo y lo ancho del torso.
    height: f32,
    hip: f32,
    knee: f32,
    legs: [(f32, f32); 2],
    shoulder: Vec3,
    arm_side: f32,
    torso: f32,
}

/// Un material del `.mtl`: el color difuso, el exponente especular y su
/// imagen, si tiene.
#[derive(Clone, Debug)]
struct Material {
    kd: Vec3,
    ns: f32,
    map: Option<String>,
}

/// Lee un `.mtl`: el color difuso, el exponente especular y la imagen
/// (`map_Kd`) de cada material.
fn read_mtl(text: &str) -> HashMap<String, Material> {
    let mut out = HashMap::new();
    let mut name = String::new();
    let fresh = || Material { kd: Vec3::splat(0.7), ns: 50.0, map: None };
    let mut current = fresh();
    for line in text.lines().map(str::trim) {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("newmtl") => {
                if !name.is_empty() {
                    out.insert(name.clone(), current.clone());
                }
                name = it.collect::<Vec<_>>().join(" ");
                current = fresh();
            }
            Some("Kd") => {
                let v: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                if v.len() >= 3 {
                    current.kd = Vec3::new(v[0], v[1], v[2]);
                }
            }
            Some("Ns") => current.ns = it.next().and_then(|t| t.parse().ok()).unwrap_or(50.0),
            Some("map_Kd") => current.map = it.last().map(str::to_string),
            _ => {}
        }
    }
    if !name.is_empty() {
        out.insert(name, current);
    }
    out
}

/// Una imagen cruda cuadrada (RGB de 8 bits por canal), en lineal.
struct Texture {
    size: usize,
    texels: Vec<Vec3>,
}

impl Texture {
    /// La version en PNG de `image` (la misma carpeta y nombre, `.png`).
    fn load(dir: &Path, image: &str) -> Option<Self> {
        let (w, h, raw) = crate::render::png::read_rgb(&dir.join(Path::new(image).with_extension("png"))).ok()?;
        let size = w as usize;
        if size == 0 || h != w {
            return None;
        }
        let linear: [f32; 256] = std::array::from_fn(|i| (i as f32 / 255.0).powf(2.2));
        let texels = raw.chunks_exact(3).map(|p| Vec3::new(linear[p[0] as usize], linear[p[1] as usize], linear[p[2] as usize])).collect();
        Some(Self { size, texels })
    }

    /// El color en `(u, v)` (v hacia arriba, como en el `.obj`), bilineal y
    /// repetida.
    fn sample(&self, u: f32, v: f32) -> Vec3 {
        let n = self.size;
        let x = (u - u.floor()) * n as f32 - 0.5;
        let y = (1.0 - (v - v.floor())) * n as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let wrap = |i: f32| (i as i64).rem_euclid(n as i64) as usize;
        let (xa, xb, ya, yb) = (wrap(x0), wrap(x0 + 1.0), wrap(y0), wrap(y0 + 1.0));
        let at = |x: usize, y: usize| self.texels[y * n + x];
        let top = at(xa, ya).lerp(at(xb, ya), fx);
        let bottom = at(xa, yb).lerp(at(xb, yb), fx);
        top.lerp(bottom, fy)
    }
}

/// La parte de una pieza por su nombre (las de dos lados, por el lado de
/// cada vertice).
fn part_of(object: &str, x: f32) -> Part {
    let side = if x < 0.0 { -1 } else { 1 };
    let leg = ["Jeans_pierna", "Costura_lateral_jeans", "Dobladillo_jeans", "Suela_zapato", "Zapato_cuerpo", "Empeine", "Cerco_zapato", "Pliegue_denim", "Bolsillo_posterior_jeans", "Costura_bolsillo_trasero", "Cordon_zapato", "Costura_puntera"];
    let hand = ["Banda_oscura_puno", "Puno_vuelto", "Mano_palma", "Dedo_", "Yema_dedo", "Pulgar", "Yema_pulgar"];
    if leg.iter().any(|p| object.starts_with(p)) {
        Part::Leg(side)
    } else if hand.iter().any(|p| object.starts_with(p)) {
        // Se decide despues cual es la mano de abajo.
        Part::Leg(side * 2)
    } else if object.starts_with("Guerrera") {
        Part::Jacket
    } else {
        Part::Body
    }
}

impl Character {
    /// Lee el `.obj` (con su `.mtl` al lado): un vertice por esquina de
    /// cada cara (las caras de mas de tres lados en abanico), con el color
    /// de su material y la clase de pieza (pintura) en la `uv`.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mtl_path = path.with_extension("mtl");
        let materials = read_mtl(&std::fs::read_to_string(&mtl_path).map_err(|e| format!("{}: {e}", mtl_path.display()))?);
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut textures: HashMap<String, Option<Texture>> = HashMap::new();
        for m in materials.values() {
            if let Some(map) = &m.map {
                textures.entry(map.clone()).or_insert_with(|| Texture::load(dir, map));
            }
        }
        let mut texture: Option<&Texture> = None;
        let mut coords: Vec<Vec2> = Vec::new();
        // Que vertices llevan imagen (la cara): dicen hacia donde mira.
        let mut faced: Vec<bool> = Vec::new();
        let (mut positions, mut normals) = (Vec::new(), Vec::new());
        let mut mesh = Mesh::default();
        let mut tags: Vec<(usize, Part)> = Vec::new();
        let mut objects: Vec<String> = Vec::new();
        let mut color = Vec3::splat(0.7);
        let mut rough = 0.6;
        for line in text.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("v") | Some("vn") => {
                    let kind = line.starts_with("vn");
                    let v: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if v.len() >= 3 {
                        let p = Vec3::new(v[0], v[1], v[2]);
                        if kind { normals.push(p) } else { positions.push(p) }
                    }
                }
                Some("vt") => {
                    let v: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if v.len() >= 2 {
                        coords.push(Vec2::new(v[0], v[1]));
                    }
                }
                Some("o") => objects.push(it.collect::<Vec<_>>().join(" ")),
                Some("usemtl") => {
                    let name = it.collect::<Vec<_>>().join(" ");
                    let m = materials.get(&name);
                    color = m.map_or(Vec3::splat(0.7), |m| m.kd);
                    rough = (2.0 / (m.map_or(50.0, |m| m.ns).max(0.0) + 2.0)).powf(0.25);
                    texture = m.and_then(|m| m.map.as_ref()).and_then(|map| textures.get(map)).and_then(Option::as_ref);
                }
                Some("f") => {
                    let corners: Vec<(usize, Option<usize>, Option<usize>)> = it
                        .filter_map(|c| {
                            let mut f = c.split('/');
                            let index = |t: Option<&str>, n: usize| -> Option<usize> {
                                let i: i64 = t?.parse().ok()?;
                                let i = if i < 0 { n as i64 + i } else { i - 1 };
                                (i >= 0 && (i as usize) < n).then_some(i as usize)
                            };
                            let v = index(f.next(), positions.len())?;
                            let t = index(f.next(), coords.len());
                            Some((v, t, index(f.next(), normals.len())))
                        })
                        .collect();
                    if corners.len() < 3 {
                        continue;
                    }
                    let object = objects.last().map_or("", String::as_str);
                    let base = mesh.positions.len() as u32;
                    for &(v, t, n) in &corners {
                        let p = positions[v];
                        mesh.positions.push(p);
                        mesh.normals.push(n.map_or(Vec3::Y, |n| normals[n]));
                        // Con imagen, el color de su coordenada de textura.
                        mesh.colors.push(match (texture, t) {
                            (Some(image), Some(t)) => image.sample(coords[t].x, coords[t].y),
                            _ => color,
                        });
                        faced.push(texture.is_some());
                        mesh.uvs.push(Vec2::new(Surface::Paint as u8 as f32, rough));
                        tags.push((objects.len(), part_of(object, p.x)));
                    }
                    for k in 1..corners.len() as u32 - 1 {
                        mesh.indices.push([base, base + k, base + k + 1]);
                    }
                }
                _ => {}
            }
        }
        if mesh.positions.is_empty() {
            return Err(format!("{}: sin caras", path.display()));
        }
        // El frente va hacia -z (como el primer modelo): si la cara queda del
        // lado +z, media vuelta alrededor de y.
        let mean_z = |pick: &dyn Fn(usize) -> bool| {
            let (n, sum) = (0..mesh.positions.len()).filter(|&i| pick(i)).fold((0usize, 0.0f32), |(n, s), i| (n + 1, s + mesh.positions[i].z));
            (n > 0).then(|| sum / n as f32)
        };
        if let (Some(face), Some(all)) = (mean_z(&|i| faced[i]), mean_z(&|_| true))
            && face > all
        {
            for p in mesh.positions.iter_mut().chain(mesh.normals.iter_mut()) {
                *p = Vec3::new(-p.x, p.y, -p.z);
            }
            // Lo de cada lado se marco con la x de antes: cambia de lado.
            for tag in &mut tags {
                if let Part::Leg(side) = tag.1 {
                    tag.1 = Part::Leg(-side);
                }
            }
        }
        mesh.update_bounds();
        let mut parts: Vec<Part> = tags.iter().map(|t| t.1).collect();
        // La mano de abajo: la de los dos lados que queda mas abajo (la
        // otra saluda).
        let hand_y = |side: i8| {
            let ys: Vec<f32> = parts.iter().zip(&mesh.positions).filter(|(p, _)| **p == Part::Leg(side * 2)).map(|(_, v)| v.y).collect();
            ys.iter().sum::<f32>() / ys.len().max(1) as f32
        };
        let down: i8 = if hand_y(-1) < hand_y(1) { -1 } else { 1 };
        for p in &mut parts {
            *p = match *p {
                Part::Leg(s) if s.abs() == 2 => {
                    if s.signum() == down {
                        Part::Hand
                    } else {
                        Part::Body
                    }
                }
                other => other,
            };
        }
        // Las medidas.
        let (lo, hi) = mesh.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
        let height = hi.y - lo.y;
        let legs_of = |side: i8| parts.iter().zip(&mesh.positions).filter(move |(p, _)| **p == Part::Leg(side)).map(|(_, v)| *v);
        let hip = legs_of(-1).chain(legs_of(1)).map(|v| v.y).fold(lo.y, f32::max);
        let knee = lo.y + 0.48 * (hip - lo.y);
        let center = |side: i8| {
            let (n, sum) = legs_of(side).fold((0.0f32, Vec3::ZERO), |(n, s), v| (n + 1.0, s + v));
            let c = sum * (1.0 / n.max(1.0));
            (c.x, c.z)
        };
        let legs = [center(-1), center(1)];
        let jacket: Vec<Vec3> = parts.iter().zip(&mesh.positions).filter(|(p, _)| **p == Part::Jacket).map(|(_, v)| *v).collect();
        let waist = jacket.iter().filter(|v| v.y > hip && v.y < hip + 0.12 * height).map(|v| v.x.abs()).fold(0.0f32, f32::max);
        let torso = if waist > 0.0 { waist } else { 0.15 * height };
        let side = down as f32;
        let sleeve_top = jacket.iter().filter(|v| v.x * side > torso * 1.05).map(|v| v.y).fold(hip, f32::max);
        let mid_z = jacket.iter().map(|v| v.z).sum::<f32>() / jacket.len().max(1) as f32;
        let shoulder = Vec3::new(side * torso * 0.85, sleeve_top - 0.03 * height, mid_z);
        Ok(Self { mesh, parts, height, hip, knee, legs, shoulder, arm_side: side, torso })
    }

    /// Una pose: cada pierna con su angulo de cadera y de rodilla
    /// (izquierda, derecha) y el angulo del hombro del brazo de abajo; de
    /// espaldas a la camara (girado media vuelta: el frente a -z, como las
    /// naves).
    fn pose(&self, hips: [f32; 2], knees: [f32; 2], arm: f32) -> Mesh {
        self.pose_leaning(hips, knees, arm, 0.0)
    }

    /// Agachado: en cuclillas (los muslos adelante, las rodillas dobladas) y
    /// todo el cuerpo inclinado hacia adelante por la cadera, la mano de
    /// abajo hacia adelante; la otra sigue saludando.
    fn crouch(&self) -> Mesh {
        // (Con el modelo volteado al cargarlo, adelante es el giro negativo
        // de las piernas y el positivo del cuerpo.)
        self.pose_leaning([-1.7, -1.7], [2.3, 2.3], -1.0, 0.45)
    }

    /// `pose` con todo el cuerpo inclinado `lean` rad por la cadera (negativo
    /// hacia adelante).
    fn pose_leaning(&self, hips: [f32; 2], knees: [f32; 2], arm: f32, lean: f32) -> Mesh {
        let mut out = self.mesh.clone();
        let middle = (self.legs[0].1 + self.legs[1].1) * 0.5;
        let waist = Vec3::new(0.0, self.hip, middle);
        let turn = |p: Vec3, pivot: Vec3, a: f32| {
            let (dy, dz) = (p.y - pivot.y, p.z - pivot.z);
            let (s, c) = a.sin_cos();
            Vec3::new(p.x, pivot.y + dy * c + dz * s, pivot.z + dz * c - dy * s)
        };
        let spin = |n: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(n.x, n.y * c + n.z * s, n.z * c - n.y * s)
        };
        for (i, part) in self.parts.iter().enumerate() {
            let (mut p, mut n) = (self.mesh.positions[i], self.mesh.normals[i]);
            match *part {
                Part::Leg(side) => {
                    let k = if side < 0 { 0 } else { 1 };
                    let (lx, lz) = self.legs[k];
                    let _ = lx;
                    if p.y < self.knee {
                        // La rodilla dobla hacia atras: lo de abajo sube y
                        // se va para atras.
                        let knee = Vec3::new(p.x, self.knee, lz);
                        p = turn(p, knee, -knees[k]);
                        n = spin(n, -knees[k]);
                    }
                    let hip = Vec3::new(p.x, self.hip, lz);
                    p = turn(p, hip, hips[k]);
                    n = spin(n, hips[k]);
                }
                Part::Hand | Part::Jacket => {
                    let w = if *part == Part::Hand {
                        1.0
                    } else {
                        smoothstep(self.torso * 0.95, self.torso * 1.15, p.x * self.arm_side) * smoothstep(self.shoulder.y + 0.02 * self.height, self.shoulder.y - 0.06 * self.height, p.y)
                    };
                    if w > 0.0 {
                        let q = turn(p, self.shoulder, arm);
                        p = p.lerp(q, w);
                        let m = spin(n, arm);
                        n = n.lerp(m, w).normalize();
                    }
                }
                Part::Body => {}
            }
            if lean != 0.0 {
                p = turn(p, waist, lean);
                n = spin(n, lean);
            }
            // Media vuelta: la espalda hacia la camara.
            out.positions[i] = Vec3::new(-p.x, p.y, -p.z);
            out.normals[i] = Vec3::new(-n.x, n.y, -n.z);
        }
        out.update_bounds();
        out
    }

    /// Las poses de la carrera (`RUN_FRAMES`, un ciclo de pasos) y la del
    /// salto, horneadas a `ppm` pixeles por metro vistas desde `elevation`.
    pub fn bake(&self, elevation: f32, ppm: f32) -> Vec<Sprite> {
        let mut meshes = Vec::new();
        for f in 0..RUN_FRAMES {
            let phi = TAU * f as f32 / RUN_FRAMES as f32;
            // La pierna izquierda en `phi`, la derecha media vuelta despues;
            // la rodilla dobla mas cuando la pierna viene de atras.
            let leg = |psi: f32| (0.6 * psi.sin(), 1.1 * (0.5 - 0.5 * (psi + 0.6).cos()).powf(1.5) * if psi.cos() < 0.3 { 1.0 } else { 0.5 });
            let (hl, kl) = leg(phi);
            let (hr, kr) = leg(phi + PI);
            let arm_phase = if self.arm_side < 0.0 { phi + PI } else { phi };
            meshes.push(self.pose([hl, hr], [kl, kr], -0.7 * arm_phase.sin()));
        }
        // El salto: las rodillas arriba y el brazo adelante.
        meshes.push(self.pose([0.9, 0.5], [1.5, 1.2], 1.0));
        // Agachado (abajo).
        meshes.push(self.crouch());
        let meters = HEIGHT / self.height.max(1e-6);
        let jobs: Vec<hornear::Job> = meshes.iter().map(|mesh| hornear::Job { mesh, finish: Finish::Vehicle, yaw: 0.0, elevation, meters, pixels_per_meter: ppm, eye: 0.0, rise: 0.0 }).collect();
        hornear::bake_all(&jobs)
    }
}

/// Los cuadros del cliente: los que siguen al de frente, caminando.
pub const WALK_FRAMES: usize = 8;

impl Character {
    /// Meyer de cliente: el cuadro 0 de frente, parado (la mano de abajo
    /// colgando y la otra saludando), y `WALK_FRAMES` caminando con la nariz
    /// a la derecha, a `ppm` pixeles por metro desde `elevation`.
    pub fn bake_customer(&self, elevation: f32, ppm: f32) -> Vec<Sprite> {
        let mut meshes = vec![self.pose([0.0, 0.0], [0.0, 0.0], 0.05)];
        for f in 0..WALK_FRAMES {
            let phi = TAU * f as f32 / WALK_FRAMES as f32;
            let leg = |psi: f32| (0.36 * psi.sin(), 0.6 * (0.5 - 0.5 * (psi + 0.6).cos()).powf(1.5) * if psi.cos() < 0.3 { 1.0 } else { 0.4 });
            let (hl, kl) = leg(phi);
            let (hr, kr) = leg(phi + PI);
            let arm_phase = if self.arm_side < 0.0 { phi + PI } else { phi };
            meshes.push(self.pose([hl, hr], [kl, kr], -0.35 * arm_phase.sin()));
        }
        let meters = HEIGHT / self.height.max(1e-6);
        let jobs: Vec<hornear::Job> = meshes
            .iter()
            .enumerate()
            .map(|(i, mesh)| hornear::Job { mesh, finish: Finish::Vehicle, yaw: if i == 0 { PI } else { 0.5 * PI }, elevation, meters, pixels_per_meter: ppm, eye: 0.0, rise: 0.0 })
            .collect();
        hornear::bake_all(&jobs)
    }
}

/// Meyer de frente y saludando, solo ese cuadro, a `ppm` pixeles por metro
/// (la bienvenida de Talega Surfers), una vez por tamano.
pub fn greeter(ppm: f32) -> Option<Arc<Sprite>> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Option<Arc<Sprite>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    map.entry(ppm.to_bits())
        .or_insert_with(|| match Character::load(&assets().join("personajes").join("meyer.obj")) {
            Ok(c) => {
                let mesh = c.pose([0.0, 0.0], [0.0, 0.0], 0.05);
                let meters = HEIGHT / c.height.max(1e-6);
                let job = hornear::Job { mesh: &mesh, finish: Finish::Vehicle, yaw: PI, elevation: 0.02, meters, pixels_per_meter: ppm, eye: 0.0, rise: 0.0 };
                hornear::bake_all(&[job]).pop().map(Arc::new)
            }
            Err(e) => {
                eprintln!("sin el personaje: {e}");
                None
            }
        })
        .clone()
}

/// Meyer de cliente para los shucos (ver `Character::bake_customer`), una
/// vez por tamano, o nada si no esta el modelo.
pub fn customer(elevation: f32, ppm: f32) -> Option<Arc<Vec<Sprite>>> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u32), Option<Arc<Vec<Sprite>>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    map.entry((elevation.to_bits(), ppm.to_bits()))
        .or_insert_with(|| match Character::load(&assets().join("personajes").join("meyer.obj")) {
            Ok(c) => Some(Arc::new(c.bake_customer(elevation, ppm))),
            Err(e) => {
                eprintln!("sin el personaje: {e}");
                None
            }
        })
        .clone()
}

/// La carpeta de los modelos: la del directorio actual o la del proyecto.
fn assets() -> PathBuf {
    let local = PathBuf::from("assets");
    if local.exists() { local } else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets") }
}

/// Los sprites de Meyer para una pantalla de `height` pixeles de alto (se
/// hornean la primera vez y se guardan), o nada si no esta el modelo.
pub fn meyer(elevation: f32, ppm: f32) -> Option<Arc<Vec<Sprite>>> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u32), Option<Arc<Vec<Sprite>>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (elevation.to_bits(), ppm.to_bits());
    let mut map = cache.lock().ok()?;
    map.entry(key)
        .or_insert_with(|| match Character::load(&assets().join("personajes").join("meyer.obj")) {
            Ok(c) => Some(Arc::new(c.bake(elevation, ppm))),
            Err(e) => {
                eprintln!("sin el personaje: {e}");
                None
            }
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Meyer se lee con sus piernas, su mano de abajo y su guerrera, y al
    /// correr una pierna va adelante y la otra atras.
    #[test]
    fn meyer_loads_and_his_legs_swing() {
        let c = Character::load(&assets().join("personajes").join("meyer.obj")).expect("el modelo");
        assert!(c.mesh.indices.len() > 20_000);
        for part in [Part::Leg(-1), Part::Leg(1), Part::Hand, Part::Jacket] {
            assert!(c.parts.iter().any(|p| *p == part), "{part:?}");
        }
        assert!(c.hip > c.knee && c.knee > 0.0 && c.hip < c.height * 0.6, "cadera {} rodilla {} alto {}", c.hip, c.knee, c.height);
        // La mano de abajo esta mas abajo que el hombro.
        let posed = c.pose([0.6, -0.6], [0.0, 0.0], 0.0);
        let foot = |side: i8| {
            let zs: Vec<f32> = c.parts.iter().zip(&posed.positions).filter(|(p, v)| **p == Part::Leg(side) && v.y < c.knee * 0.5).map(|(_, v)| v.z).collect();
            zs.iter().sum::<f32>() / zs.len().max(1) as f32
        };
        // Con la media vuelta, adelante es -z.
        assert!(foot(-1) < foot(1) - 0.2, "{} {}", foot(-1), foot(1));
    }

    /// Los cuadros en una tira, en PNG (`cargo test --release dump_meyer --
    /// --ignored --nocapture`). Tambien de lado, para ver las piernas.
    #[test]
    #[ignore = "guarda una foto"]
    fn dump_meyer() {
        let c = Character::load(&assets().join("personajes").join("meyer.obj")).expect("el modelo");
        for (name, yaw) in [("atras", 0.0f32), ("lado", 1.4)] {
            let mut sprites = c.bake(0.37, 160.0);
            if yaw != 0.0 {
                let meters = HEIGHT / c.height;
                let meshes: Vec<Mesh> = (0..RUN_FRAMES + 2).map(|f| {
                    let phi = TAU * f as f32 / RUN_FRAMES as f32;
                    if f == RUN_FRAMES + 1 {
                        return c.crouch();
                    }
                    let leg = |psi: f32| (0.6 * psi.sin(), 1.1 * (0.5 - 0.5 * (psi + 0.6).cos()).powf(1.5) * if psi.cos() < 0.3 { 1.0 } else { 0.5 });
                    let ((hl, kl), (hr, kr)) = (leg(phi), leg(phi + PI));
                    c.pose([hl, hr], [kl, kr], 0.0)
                }).collect();
                let jobs: Vec<hornear::Job> = meshes.iter().map(|mesh| hornear::Job { mesh, finish: Finish::Vehicle, yaw, elevation: 0.1, meters, pixels_per_meter: 160.0, eye: 0.0, rise: 0.0 }).collect();
                sprites = hornear::bake_all(&jobs);
            }
            let (w, h) = (sprites.iter().map(|s| s.width).sum::<usize>(), sprites.iter().map(|s| s.height).max().unwrap_or(1));
            let mut rgba = vec![0u8; w * h * 4];
            let mut x0 = 0;
            for s in &sprites {
                for y in 0..s.height {
                    for x in 0..s.width {
                        let p = s.pixels[y * s.width + x];
                        let a = p[3] as f32 / 255.0;
                        let o = (y * w + x0 + x) * 4;
                        for k in 0..3 {
                            rgba[o + k] = (p[k] as f32 + 40.0 * (1.0 - a)).min(255.0) as u8;
                        }
                        rgba[o + 3] = 255;
                    }
                }
                x0 += s.width;
            }
            let path = std::env::temp_dir().join(format!("meyer_{name}.png"));
            crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
            println!("{}", path.display());
        }
    }

    /// Los cuadros del cliente en una tira, en PNG (`cargo test --release
    /// dump_customer -- --ignored --nocapture`).
    #[test]
    #[ignore = "guarda una foto"]
    fn dump_customer() {
        let sprites = customer(0.03, 240.0).expect("los sprites");
        let (w, h) = (sprites.iter().map(|s| s.width).sum::<usize>(), sprites.iter().map(|s| s.height).max().unwrap_or(1));
        let mut rgba = vec![0u8; w * h * 4];
        let mut x0 = 0;
        for s in sprites.iter() {
            for y in 0..s.height {
                for x in 0..s.width {
                    let p = s.pixels[y * s.width + x];
                    let a = p[3] as f32 / 255.0;
                    let o = (y * w + x0 + x) * 4;
                    for k in 0..3 {
                        rgba[o + k] = (p[k] as f32 + 40.0 * (1.0 - a)).min(255.0) as u8;
                    }
                    rgba[o + 3] = 255;
                }
            }
            x0 += s.width;
        }
        let path = std::env::temp_dir().join("meyer_cliente.png");
        crate::render::png::write_rgba(&path, w as u32, h as u32, &rgba).expect("png");
        println!("{}", path.display());
    }

    /// De cliente: de frente es mas ancho que de lado (los hombros) y los
    /// pasos cambian de cuadro a cuadro.
    #[test]
    fn meyer_bakes_as_a_customer() {
        let sprites = customer(0.03, 50.0).expect("los sprites");
        assert_eq!(sprites.len(), 1 + WALK_FRAMES);
        let tall = sprites[0].height as f32 / sprites[0].scale;
        assert!(tall > 1.5 && tall < 2.6, "{tall} m");
        assert_ne!(sprites[1].pixels, sprites[3].pixels);
    }

    #[test]
    fn meyer_bakes_his_frames() {
        let sprites = meyer(0.37, 60.0).expect("los sprites");
        assert_eq!(sprites.len(), RUN_FRAMES + 2);
        // Agachado es mas bajito que corriendo.
        let tall = |s: &Sprite| s.anchor.1 / s.scale;
        assert!(tall(&sprites[RUN_FRAMES + 1]) < 0.75 * tall(&sprites[0]), "{} {}", tall(&sprites[RUN_FRAMES + 1]), tall(&sprites[0]));
        for s in sprites.iter() {
            let tall = s.height as f32 / s.scale;
            assert!(tall > 1.2 && tall < 2.6, "{tall} m");
            assert!(s.pixels.iter().any(|p| p[3] > 200));
        }
    }
}
