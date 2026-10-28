//! La capital (la version chapina de Cementerio): barrancos y ciudad, camionetas extraurbanas y la Torre del Reformador; camionetas y tuc-tucs dan vueltas en su anillo.
//!
//! Provisional: todavia usa las piezas de `stub`.

use std::sync::Arc;

use super::{Chapin, Placer, Spec, stub};
use crate::math::{DVec3, Perlin, Vec3};
use crate::scene::Mesh;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter};
use crate::scene::Rings;
use crate::shaders::rings::{RingInfo, RingStyle};
use crate::shaders::{Fragment, PropStyle, Uniforms};
use crate::terrain::HeightField;

pub fn spec() -> Spec {
    Spec {
        label: "La Capital",
        material: Chapin::Capital,
        atmosphere: None,
        rings: Some(Rings { inner: 1.7, outer: 3.1, style: RingStyle::Buses }),
        motes: Some((Motes::Smog, 100.0)),
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    stub::field(seed, 3.0)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    stub::shade(f, u, Vec3::new(0.3, 0.27, 0.24))
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

pub fn rules() -> Vec<Scatter> {
    Vec::new()
}

pub fn site(default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    Some(("El Reformador", default.0, default.1, place))
}

fn place(_instances: &mut Vec<Instance>, _radius: f64, _field: &dyn HeightField, _seed: u32, _frame: (DVec3, DVec3, DVec3)) {}

pub fn mesh(_kind: PropKind, detail: usize) -> Mesh {
    stub::mesh(detail)
}

pub fn shade_prop(_kind: PropKind, f: &Fragment, u: &Uniforms, _style: &PropStyle, sun: Vec3) -> Vec3 {
    stub::shade_prop(f, u, sun, Vec3::new(0.3, 0.27, 0.24))
}

/// Densidad del anillo (0 a ~0.9) a la fraccion `s` de su ancho (0 adentro, 1 afuera).
pub fn ring_density(s: f32, noise: &Perlin) -> f32 {
    let bands = noise.noise(Vec3::new(s * 20.0, 0.5, 0.5)) * 0.5 + 0.5;
    (bands * 0.5).min(0.8)
}

/// Lo que se ve del anillo en `local` (el punto en su plano, relativo al planeta): color, opacidad,
/// luz propia y cuanto dispersa hacia adelante.
pub fn ring(_ring: &RingInfo, _local: Vec3, alpha: f32, _s: f32, _footprint: f32, _u: &Uniforms) -> (Vec3, f32, Vec3, f32) {
    (Vec3::new(0.3, 0.27, 0.24), alpha, Vec3::ZERO, 1.0)
}
