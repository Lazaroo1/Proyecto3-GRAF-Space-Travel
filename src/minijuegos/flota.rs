//! Las naves chapinas como trafico de Talega Surfers: La Mamaneta (las
//! camionetas, paradas de espaldas o viniendo de frente) y el Tuk-Tuk Elena,
//! los mismos modelos de Blender con que se vuela.
//!
//! Se hornean una vez (`hornear`), derechas en su carril (de espaldas, y
//! las de frente con media vuelta mas), desde cuatro alturas de vista: de
//! casi de lado a casi desde arriba. En la calle cada una usa la de la altura
//! con que la ve la camara, `atan(alto / distancia)`: de cerca y desde los
//! techos se ve su techo. Los tres colores de las camionetas salen al pegarlas,
//! permutando los canales del color (rojo, verde, azul -> verde, azul, rojo
//! -> azul, rojo, verde): un solo horneado da tres pinturas.
//!
//! Para la avenida de los shucos van de lado (`side`): la nariz a la
//! derecha, y al pegarlas al reves van para la izquierda.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use super::hornear::{self, Finish, Sprite};
use crate::math::Vec3;
use crate::scene::{Mesh, load_vehicle};

/// Las alturas de vista horneadas (rad sobre el horizonte).
pub const TILTS: [f32; 4] = [0.14, 0.32, 0.55, 0.85];
/// Lo alto de una camioneta hasta su techo (sin la parrilla) y del tuk-tuk.
pub const BUS_HEIGHT: f32 = 3.3;
pub const TUK_HEIGHT: f32 = 2.0;

/// Las naves horneadas y sus medidas (ancho, alto, largo, m).
pub struct Fleet {
    pub bus_back: Vec<Sprite>,
    pub bus_front: Vec<Sprite>,
    pub tuk_back: Vec<Sprite>,
    pub bus: Vec3,
    pub tuk: Vec3,
}

impl Fleet {
    /// La altura de vista horneada mas cercana a `angle` (rad).
    pub fn tilt_index(angle: f32) -> usize {
        (0..TILTS.len()).min_by(|&a, &b| (TILTS[a] - angle).abs().total_cmp(&(TILTS[b] - angle).abs())).unwrap_or(0)
    }
}

fn assets() -> PathBuf {
    let local = PathBuf::from("assets");
    if local.exists() { local } else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets") }
}

/// Una nave en metros: el frente a -z y arriba +y (como vuelan), con el
/// alto que se pide, centrada.
fn load(file: &str, height: f32) -> Result<(Mesh, Vec3), String> {
    load_detail(file, "medium", height)
}

fn load_detail(file: &str, detail: &str, height: f32) -> Result<(Mesh, Vec3), String> {
    let mut mesh = load_vehicle(&assets().join("naves").join(format!("{file}-{detail}.obj")))?;
    mesh.deform(|p| Vec3::new(-p.y, p.z, -p.x));
    mesh.normalize_size(1.0);
    let (lo, hi) = mesh.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let k = height / (hi.y - lo.y).max(1e-6);
    mesh.deform(|p| p * k);
    Ok((mesh, (hi - lo) * k))
}

/// La flota horneada a `ppm` pixeles por metro (una vez por tamano), o nada
/// si no estan los modelos.
pub fn fleet(ppm: f32) -> Option<Arc<Fleet>> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Option<Arc<Fleet>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    map.entry(ppm.to_bits())
        .or_insert_with(|| match bake(ppm) {
            Ok(f) => Some(Arc::new(f)),
            Err(e) => {
                eprintln!("sin la flota: {e}");
                None
            }
        })
        .clone()
}

/// La Mamaneta y el Tuk-Tuk Elena de lado, con la nariz a la derecha.
pub struct Side {
    pub bus: Sprite,
    pub tuk: Sprite,
}

/// Las mallas (las de poco detalle) para dibujarlas en perspectiva de
/// cerca (`malla`): en metros, el frente a -z, centradas en x y z y con las
/// llantas en `y = 0`.
pub struct Meshes {
    pub bus: Mesh,
    pub tuk: Mesh,
}

pub fn meshes() -> Option<Arc<Meshes>> {
    static CACHE: OnceLock<Option<Arc<Meshes>>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let grounded = |file: &str, height: f32| -> Result<Mesh, String> {
                let (mut mesh, _) = load_detail(file, "low", height)?;
                let (lo, hi) = mesh.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
                let shift = Vec3::new(-(lo.x + hi.x) * 0.5, -lo.y, -(lo.z + hi.z) * 0.5);
                mesh.deform(|p| p + shift);
                Ok(mesh)
            };
            match (grounded("02-la-mamaneta", BUS_HEIGHT), grounded("01-tuk-tuk-elena", TUK_HEIGHT)) {
                (Ok(bus), Ok(tuk)) => Some(Arc::new(Meshes { bus, tuk })),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("sin las mallas de las naves: {e}");
                    None
                }
            }
        })
        .clone()
}

/// Desde que altura se ven de lado (rad): casi a la altura de los ojos.
pub const SIDE_ELEVATION: f32 = 0.05;

/// Las dos naves de lado a `ppm` (una vez por tamano), o nada si no estan.
pub fn side(ppm: f32) -> Option<Arc<Side>> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Option<Arc<Side>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;
    map.entry(ppm.to_bits())
        .or_insert_with(|| {
            let baked = (|| -> Result<Side, String> {
                let (bus, _) = load("02-la-mamaneta", BUS_HEIGHT)?;
                let (tuk, _) = load("01-tuk-tuk-elena", TUK_HEIGHT)?;
                let jobs: Vec<hornear::Job> = [&bus, &tuk].iter().map(|mesh| hornear::Job { mesh, finish: Finish::Vehicle, yaw: 0.5 * PI, elevation: SIDE_ELEVATION, meters: 1.0, pixels_per_meter: ppm, eye: 0.0, rise: 0.0 }).collect();
                let mut baked = hornear::bake_all(&jobs);
                let tuk_sprite = baked.pop().ok_or("sin el tuk-tuk")?;
                let bus_sprite = baked.pop().ok_or("sin la camioneta")?;
                Ok(Side { bus: bus_sprite, tuk: tuk_sprite })
            })();
            match baked {
                Ok(s) => Some(Arc::new(s)),
                Err(e) => {
                    eprintln!("sin las naves de lado: {e}");
                    None
                }
            }
        })
        .clone()
}

fn bake(ppm: f32) -> Result<Fleet, String> {
    let (bus, bus_size) = load("02-la-mamaneta", BUS_HEIGHT)?;
    let (tuk, tuk_size) = load("01-tuk-tuk-elena", TUK_HEIGHT)?;
    let mut jobs = Vec::new();
    for (mesh, turn) in [(&bus, 0.0), (&bus, PI), (&tuk, 0.0)] {
        for tilt in TILTS {
            jobs.push(hornear::Job { mesh, finish: Finish::Vehicle, yaw: turn, elevation: tilt, meters: 1.0, pixels_per_meter: ppm, eye: 0.0, rise: 0.0 });
        }
    }
    let n = TILTS.len();
    let mut baked = hornear::bake_all(&jobs);
    let tuk_back = baked.split_off(2 * n);
    let bus_front = baked.split_off(n);
    Ok(Fleet { bus_back: baked, bus_front, tuk_back, bus: bus_size, tuk: tuk_size })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fleet_bakes_with_its_sizes() {
        let f = fleet(20.0).expect("la flota");
        assert_eq!(f.bus_back.len(), TILTS.len());
        assert_eq!(f.bus_front.len(), TILTS.len());
        assert_eq!(f.tuk_back.len(), TILTS.len());
        assert!((f.bus.y - BUS_HEIGHT).abs() < 1e-3);
        assert!(f.bus.z > 7.0 && f.bus.z < 14.0, "largo {}", f.bus.z);
        assert!(f.bus.x > 1.8 && f.bus.x < 3.6, "ancho {}", f.bus.x);
        assert!(f.tuk.z > 2.0 && f.tuk.z < 4.5, "tuk {:?}", f.tuk);
        assert_eq!(Fleet::tilt_index(0.0), 0);
        assert_eq!(Fleet::tilt_index(0.5), 2);
        assert_eq!(Fleet::tilt_index(1.4), 3);
        // Desde arriba se ve mas techo: la de arriba es mas alta en la imagen.
        assert!(f.bus_back[3].height > f.bus_back[0].height);
    }

    /// De lado una camioneta es mucho mas larga que alta.
    #[test]
    fn the_side_views_are_long() {
        let s = side(40.0).expect("de lado");
        let (w, h) = (s.bus.width as f32 / s.bus.scale, s.bus.height as f32 / s.bus.scale);
        assert!(w > 1.8 * h, "{w} x {h}");
        assert!(s.tuk.width < s.bus.width);
    }
}
