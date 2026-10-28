//! La Luna de Xelaju (luna de los Cuchumatanes, solo en la version chapina).
//!
//! Provisional: todavia usa las piezas de `stub`.

use std::sync::Arc;

use super::{Chapin, Spec, stub};
use crate::math::Vec3;
use crate::shaders::{Fragment, Uniforms};
use crate::terrain::HeightField;

pub fn spec() -> Spec {
    Spec {
        label: "Luna de Xelaju",
        material: Chapin::Xela,
        atmosphere: None,
        rings: None,
        motes: None,
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    stub::field(seed, 2.0)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    stub::shade(f, u, Vec3::new(0.5, 0.5, 0.48))
}
