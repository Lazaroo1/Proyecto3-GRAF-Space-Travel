//! Las naves chapinas como trafico de la Camioneta Surfers: La Mamaneta (las
//! camionetas, paradas de espaldas o viniendo de frente) y el Tuk-Tuk Elena,
//! los mismos modelos de Blender con que se vuela.
//!
//! Se hornean una vez (`hornear`), como en la carrera, desde siete rumbos
//! (de -30 a 30 grados cada 10): en la calle cada una usa el del angulo con
//! que la ve la camara, `-atan(dx / dz)`. Las de frente se hornean con media
//! vuelta mas. Los tres colores de las camionetas salen al pegarlas,
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

/// Los rumbos horneados y cada cuanto.
pub const YAWS: usize = 7;
pub const YAW_STEP: f32 = 10.0 * PI / 180.0;
/// Desde que altura se ven (rad): la camara va a 3 m y las ve de 5 a 60 m.
pub const ELEVATION: f32 = 0.12;
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
    /// El rumbo horneado mas cercano a `view` (rad).
    pub fn yaw_index(view: f32) -> usize {
        ((view / YAW_STEP).round() as i32 + (YAWS / 2) as i32).clamp(0, YAWS as i32 - 1) as usize
    }
}

fn assets() -> PathBuf {
    let local = PathBuf::from("assets");
    if local.exists() { local } else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets") }
}

/// Una nave en metros: el frente a -z y arriba +y (como vuelan), con el
/// alto que se pide, centrada.
fn load(file: &str, height: f32) -> Result<(Mesh, Vec3), String> {
    let mut mesh = load_vehicle(&assets().join("naves").join(format!("{file}-medium.obj")))?;
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
    let yaw = |k: usize| (k as f32 - (YAWS / 2) as f32) * YAW_STEP;
    let mut jobs = Vec::new();
    for (mesh, turn) in [(&bus, 0.0), (&bus, PI), (&tuk, 0.0)] {
        for k in 0..YAWS {
            jobs.push(hornear::Job { mesh, finish: Finish::Vehicle, yaw: turn + yaw(k), elevation: ELEVATION, meters: 1.0, pixels_per_meter: ppm, eye: 0.0, rise: 0.0 });
        }
    }
    let mut baked = hornear::bake_all(&jobs);
    let tuk_back = baked.split_off(2 * YAWS);
    let bus_front = baked.split_off(YAWS);
    Ok(Fleet { bus_back: baked, bus_front, tuk_back, bus: bus_size, tuk: tuk_size })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fleet_bakes_with_its_sizes() {
        let f = fleet(20.0).expect("la flota");
        assert_eq!(f.bus_back.len(), YAWS);
        assert_eq!(f.bus_front.len(), YAWS);
        assert_eq!(f.tuk_back.len(), YAWS);
        assert!((f.bus.y - BUS_HEIGHT).abs() < 1e-3);
        assert!(f.bus.z > 7.0 && f.bus.z < 14.0, "largo {}", f.bus.z);
        assert!(f.bus.x > 1.8 && f.bus.x < 3.6, "ancho {}", f.bus.x);
        assert!(f.tuk.z > 2.0 && f.tuk.z < 4.5, "tuk {:?}", f.tuk);
        assert_eq!(Fleet::yaw_index(0.0), 3);
        assert_eq!(Fleet::yaw_index(-1.0), 0);
        assert_eq!(Fleet::yaw_index(0.12), 4);
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
