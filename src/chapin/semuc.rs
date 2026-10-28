//! Semuc Champey (la version chapina de Parpado): pozas turquesa en terrazas de piedra caliza, entre la selva.
//!
//! Provisional: todavia usa las piezas de `stub`.

use std::sync::Arc;

use super::{Chapin, Placer, Spec, stub};
use crate::math::{DVec3, Perlin, Vec3};
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, Scatter};
use crate::shaders::{Fragment, Uniforms};
use crate::terrain::HeightField;

pub fn spec() -> Spec {
    Spec {
        label: "Semuc Champey",
        material: Chapin::Semuc,
        atmosphere: None,
        rings: None,
        motes: Some((Motes::Spray, 90.0)),
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    stub::field(seed, 3.0)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    stub::shade(f, u, Vec3::new(0.1, 0.4, 0.38))
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

pub fn rules() -> Vec<Scatter> {
    Vec::new()
}

pub fn site(default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    Some(("Las pozas", default.0, default.1, place))
}

fn place(_instances: &mut Vec<Instance>, _radius: f64, _field: &dyn HeightField, _seed: u32, _frame: (DVec3, DVec3, DVec3)) {}
