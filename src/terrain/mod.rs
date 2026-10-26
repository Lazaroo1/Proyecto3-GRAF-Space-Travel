//! Terreno de los planetas: un cubo-esfera con un quadtree por cara y nivel
//! de detalle segun la distancia.
//!
//! # La idea
//! Cada cara del cubo es un cuadrado que se proyecta sobre la esfera. La cara
//! entera es el nodo raiz; cada nodo se puede partir en cuatro hijos que
//! cubren lo mismo con el doble de resolucion. Cada nodo es un chunk de 32x32
//! cuadros cuyos vertices se desplazan con la funcion de altura del planeta.
//!
//! En cada frame se recorre el arbol desde las seis raices:
//! - **Horizonte**: un nodo que queda completo detras de la curvatura del
//!   planeta no se ve desde la camara y se descarta con todo su subarbol.
//! - **Frustum**: igual con lo que queda fuera del campo de vision.
//! - **Detalle**: un nodo se parte si la camara esta mas cerca que
//!   `factor * tamano` del nodo. Asi cada chunk ocupa mas o menos lo mismo en
//!   pantalla: los lejanos son grandes y burdos, los cercanos chicos y finos.
//!
//! Los chunks que faltan se generan en paralelo, con un limite por frame y
//! los mas cercanos primero. Mientras un hijo no existe se dibuja su padre:
//! nunca queda un hueco, solo un instante de menos detalle.
//!
//! # Grietas
//! Dos chunks vecinos de distinto nivel no comparten todos los vertices del
//! borde y entre ellos se abre una grieta. Cada chunk lleva un faldon: una
//! pared que baja desde su borde hacia el centro del planeta y tapa la
//! grieta desde cualquier angulo.

mod chunk;
pub mod shapes;

use std::collections::HashMap;
use std::sync::Arc;

use crate::math::{DVec3, Vec3};
use crate::parallel::{self, DisjointMut};
pub use chunk::{cube_to_sphere, Chunk};

/// Cuadros por lado de cada chunk.
pub const GRID: usize = 32;
/// Chunks que se conservan en memoria antes de empezar a soltar los viejos.
const CACHE_CAPACITY: usize = 2500;

/// Lo que la funcion de altura dice de un punto de la superficie.
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainSample {
    /// Altura sobre el radio base, en unidades del mundo.
    pub height: f32,
    /// Datos libres para el shader del planeta (mascaras de material, etc.).
    pub attrs: Vec3,
}

/// Forma de la superficie de un planeta. La usan la generacion de chunks y
/// la colision de la nave: lo que se dibuja y lo que se choca es lo mismo.
pub trait HeightField: Send + Sync {
    /// Muestra en la direccion unitaria `dir`, en el espacio del planeta.
    fn sample(&self, dir: Vec3) -> TerrainSample;
    /// Alturas minima y maxima posibles.
    fn height_range(&self) -> (f32, f32);
}

/// Superficie sin relieve (gigantes gaseosos).
pub struct Smooth;

impl HeightField for Smooth {
    fn sample(&self, _dir: Vec3) -> TerrainSample {
        TerrainSample::default()
    }

    fn height_range(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
}

/// Un nodo del quadtree: cara, nivel y posicion dentro de la cara.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub face: u8,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl ChunkKey {
    fn root(face: u8) -> Self {
        Self { face, level: 0, x: 0, y: 0 }
    }

    fn children(&self) -> [Self; 4] {
        let (level, x, y) = (self.level + 1, self.x * 2, self.y * 2);
        [
            Self { face: self.face, level, x, y },
            Self { face: self.face, level, x: x + 1, y },
            Self { face: self.face, level, x, y: y + 1 },
            Self { face: self.face, level, x: x + 1, y: y + 1 },
        ]
    }

    /// Rango que cubre en la cara, en `[-1, 1]`: `(u0, v0, u1, v1)`.
    fn uv_bounds(&self) -> (f64, f64, f64, f64) {
        let size = 2.0 / (1u64 << self.level) as f64;
        let u0 = -1.0 + self.x as f64 * size;
        let v0 = -1.0 + self.y as f64 * size;
        (u0, v0, u0 + size, v0 + size)
    }
}

/// Esfera envolvente de un nodo y su radio maximo desde el centro del planeta.
#[derive(Clone, Copy, Debug)]
struct Bounds {
    center: DVec3,
    radius: f64,
    max_radius: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct LodSettings {
    /// Se parte un nodo si la camara esta a menos de `factor * tamano`.
    pub factor: f64,
    /// Chunks nuevos por frame como maximo.
    pub budget: usize,
    /// Sin nivel de detalle todo se dibuja con `fixed_level`: sirve para
    /// comparar cuanto trabajo ahorra el LOD.
    pub enabled: bool,
    pub fixed_level: u8,
}

/// Lo que hizo el terreno en el ultimo frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainStats {
    pub drawn: usize,
    pub generated: usize,
    pub pending: usize,
    pub cached: usize,
    pub deepest: u8,
    pub culled_horizon: usize,
    pub culled_frustum: usize,
}

pub struct Terrain {
    pub radius: f64,
    pub field: Arc<dyn HeightField>,
    pub max_level: u8,
    min_height: f64,
    max_height: f64,
    /// Triangulos de un chunk: todos comparten la misma topologia.
    indices: Vec<[u32; 3]>,
    chunks: HashMap<ChunkKey, Chunk>,
    selected: Vec<ChunkKey>,
    frame: u64,
    pub stats: TerrainStats,
}

/// Lo que el recorrido necesita saber de la camara.
pub struct Viewer<'a> {
    /// Posicion de la camara en el espacio del planeta.
    pub position: DVec3,
    /// Si una esfera (centro en el espacio del planeta, radio) puede verse.
    pub visible: &'a dyn Fn(DVec3, f64) -> bool,
}

impl Terrain {
    /// Terreno de radio `radius` cuyo cuadro mas fino mide `finest` unidades.
    pub fn new(radius: f64, field: Arc<dyn HeightField>, finest: f64) -> Self {
        let face_edge = radius * std::f64::consts::FRAC_PI_2;
        let levels = (face_edge / GRID as f64 / finest).log2().ceil().max(0.0) as u8;
        let (min_h, max_h) = field.height_range();
        let mut terrain = Self {
            radius,
            field,
            max_level: levels.min(16),
            min_height: min_h as f64,
            max_height: max_h as f64,
            indices: chunk::indices(GRID),
            chunks: HashMap::new(),
            selected: Vec::new(),
            frame: 0,
            stats: TerrainStats::default(),
        };
        let roots: Vec<ChunkKey> = (0..6).map(ChunkKey::root).collect();
        terrain.generate(&roots);
        terrain
    }

    pub fn indices(&self) -> &[[u32; 3]] {
        &self.indices
    }

    /// Chunks elegidos en el ultimo `update`.
    pub fn selected(&self) -> impl Iterator<Item = &Chunk> {
        self.selected.iter().filter_map(|key| self.chunks.get(key))
    }

    /// Elige los chunks a dibujar y genera los que faltan.
    pub fn update(&mut self, viewer: &Viewer, settings: &LodSettings) {
        self.frame += 1;
        let mut draw = Vec::new();
        let mut missing = Vec::new();
        let mut culled = (0, 0);
        for face in 0..6 {
            self.select(ChunkKey::root(face), viewer, settings, &mut draw, &mut missing, &mut culled);
        }

        // Los mas cercanos primero, hasta el limite del frame.
        missing.sort_by(|a: &(f64, ChunkKey), b| a.0.total_cmp(&b.0));
        missing.dedup_by_key(|m| m.1);
        let pending = missing.len();
        let batch: Vec<ChunkKey> = missing.iter().take(settings.budget).map(|m| m.1).collect();
        if !batch.is_empty() {
            self.generate(&batch);
            // Otra vuelta para que los chunks nuevos se usen desde este frame.
            draw.clear();
            missing.clear();
            culled = (0, 0);
            for face in 0..6 {
                self.select(ChunkKey::root(face), viewer, settings, &mut draw, &mut missing, &mut culled);
            }
        }

        let frame = self.frame;
        let mut deepest = 0;
        for key in &draw {
            deepest = deepest.max(key.level);
            if let Some(chunk) = self.chunks.get_mut(key) {
                chunk.last_used = frame;
            }
        }
        if self.chunks.len() > CACHE_CAPACITY && frame % 30 == 0 {
            // Se sueltan los que llevan mas tiempo sin usarse; las raices nunca.
            self.chunks
                .retain(|key, chunk| key.level == 0 || chunk.last_used + 120 > frame);
        }
        self.stats = TerrainStats {
            drawn: draw.len(),
            generated: batch.len(),
            pending: pending.saturating_sub(batch.len()),
            cached: self.chunks.len(),
            deepest,
            culled_horizon: culled.0,
            culled_frustum: culled.1,
        };
        self.selected = draw;
    }

    fn select(
        &mut self,
        key: ChunkKey,
        viewer: &Viewer,
        settings: &LodSettings,
        draw: &mut Vec<ChunkKey>,
        missing: &mut Vec<(f64, ChunkKey)>,
        culled: &mut (usize, usize),
    ) {
        let bounds = self.bounds(&key);
        if self.below_horizon(&bounds, viewer.position) {
            culled.0 += 1;
            return;
        }
        if !(viewer.visible)(bounds.center, bounds.radius) {
            culled.1 += 1;
            return;
        }
        let frame = self.frame;
        if let Some(chunk) = self.chunks.get_mut(&key) {
            // Tambien se marcan los padres recorridos: sin ellos, al alejarse
            // habria que regenerarlos.
            chunk.last_used = frame;
        }
        let distance = ((viewer.position - bounds.center).length() - bounds.radius).max(0.0);
        let wants_split = key.level < self.max_level
            && if settings.enabled {
                distance < settings.factor * self.node_size(key.level)
            } else {
                key.level < settings.fixed_level
            };
        let exists = self.chunks.contains_key(&key);
        let children = key.children();
        let ready = children.iter().all(|c| self.chunks.contains_key(c));
        if (wants_split || !exists) && ready {
            for child in children {
                self.select(child, viewer, settings, draw, missing, culled);
            }
            return;
        }
        if wants_split {
            for child in children {
                if !self.chunks.contains_key(&child) {
                    missing.push((distance, child));
                }
            }
        }
        if exists {
            draw.push(key);
        } else {
            missing.push((distance, key));
        }
    }

    /// Esfera envolvente: la del chunk si ya existe, si no una estimacion
    /// a partir del rango de alturas.
    fn bounds(&self, key: &ChunkKey) -> Bounds {
        if let Some(chunk) = self.chunks.get(key) {
            return Bounds {
                center: chunk.center,
                radius: chunk.radius as f64,
                max_radius: chunk.max_radius as f64,
            };
        }
        let (u0, v0, u1, v1) = key.uv_bounds();
        let dir = chunk::cube_to_sphere(key.face, (u0 + u1) * 0.5, (v0 + v1) * 0.5);
        let mid = self.radius + (self.min_height + self.max_height) * 0.5;
        // Medio lado del nodo sobre la esfera, por la diagonal, mas el relieve.
        let half = self.node_size(key.level) * 0.75;
        Bounds {
            center: dir * mid,
            radius: half + (self.max_height - self.min_height),
            max_radius: self.radius + self.max_height,
        }
    }

    /// Lado de un nodo de nivel `level`, medido sobre la esfera.
    fn node_size(&self, level: u8) -> f64 {
        self.radius * std::f64::consts::FRAC_PI_2 / (1u64 << level) as f64
    }

    /// Si toda la esfera del nodo queda detras del horizonte del planeta.
    ///
    /// El planeta tapa como una esfera de radio `R` (su altura minima). Desde
    /// la camara a distancia `d`, el horizonte esta a un angulo
    /// `acos(R/d)` del punto bajo la camara; un punto a altura `rho` asoma
    /// sobre el horizonte hasta `acos(R/d) + acos(R/rho)`. Si el nodo, a su
    /// distancia angular menos su propio tamano angular, queda mas lejos que
    /// eso, esta tapado por el planeta.
    fn below_horizon(&self, bounds: &Bounds, camera: DVec3) -> bool {
        let occluder = self.radius + self.min_height;
        let d = camera.length();
        let c = bounds.center.length();
        if d <= occluder || c <= 0.0 {
            return false;
        }
        let horizon = (occluder / d).acos() + (occluder / bounds.max_radius.max(occluder)).acos();
        let angle = (camera.dot(bounds.center) / (d * c)).clamp(-1.0, 1.0).acos();
        let size = (bounds.radius / c).min(1.0).asin();
        angle - size > horizon
    }

    fn generate(&mut self, keys: &[ChunkKey]) {
        let mut out: Vec<Option<Chunk>> = keys.iter().map(|_| None).collect();
        let slots = DisjointMut::new(&mut out);
        let (radius, field) = (self.radius, &*self.field);
        parallel::for_each(keys.len(), |i| {
            // SAFETY: cada indice escribe solo su propia casilla.
            let slot = unsafe { slots.range(i, 1) };
            slot[0] = Some(chunk::generate(keys[i], radius, field, GRID));
        });
        for chunk in out.into_iter().flatten() {
            self.chunks.insert(chunk.key, chunk);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_tile_their_parent_exactly() {
        let parent = ChunkKey { face: 3, level: 2, x: 1, y: 2 };
        let (u0, v0, u1, v1) = parent.uv_bounds();
        let kids = parent.children();
        let (cu0, cv0, _, _) = kids[0].uv_bounds();
        let (_, _, cu1, cv1) = kids[3].uv_bounds();
        assert_eq!((u0, v0, u1, v1), (cu0, cv0, cu1, cv1));
    }

    #[test]
    fn horizon_hides_the_far_side_but_not_the_near_side() {
        let terrain = Terrain::new(100.0, Arc::new(Smooth), 1.0);
        let camera = DVec3::new(0.0, 0.0, 150.0);
        let near = Bounds { center: DVec3::new(0.0, 0.0, 100.0), radius: 5.0, max_radius: 100.0 };
        let far = Bounds { center: DVec3::new(0.0, 0.0, -100.0), radius: 5.0, max_radius: 100.0 };
        let limb = Bounds { center: DVec3::new(100.0, 0.0, 0.0), radius: 5.0, max_radius: 100.0 };
        assert!(!terrain.below_horizon(&near, camera));
        assert!(terrain.below_horizon(&far, camera));
        // Desde 150 de distancia el horizonte esta a acos(100/150) = 48
        // grados del punto bajo la camara: un chunk a 90 grados ya no se ve.
        assert!(terrain.below_horizon(&limb, camera));
        // Una montana alta en el mismo lugar si asoma sobre el horizonte.
        let peak = Bounds { center: DVec3::new(100.0, 0.0, 0.0), radius: 5.0, max_radius: 300.0 };
        assert!(!terrain.below_horizon(&peak, camera));
    }
}
