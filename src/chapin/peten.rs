//! Peten (la version chapina de Jardin): la selva y Tikal, con el Templo del Gran Jaguar saliendo entre los arboles, ceibas y quetzales.
//!
//! Provisional: todavia usa las piezas de `stub`.

use std::sync::Arc;

use super::{Chapin, Placer, Spec, stub};
use crate::math::{DVec3, Perlin, Vec3};
use crate::scene::Mesh;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter};
use crate::shaders::{Fragment, PropStyle, Uniforms};
use crate::terrain::HeightField;

pub fn spec() -> Spec {
    Spec {
        label: "Peten",
        material: Chapin::Peten,
        atmosphere: None,
        rings: None,
        motes: Some((Motes::Fireflies, 120.0)),
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    stub::field(seed, 3.0)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    stub::shade(f, u, Vec3::new(0.08, 0.22, 0.07))
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

pub fn rules() -> Vec<Scatter> {
    Vec::new()
}

pub fn site(default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    Some(("Tikal", default.0, default.1, place))
}

fn place(_instances: &mut Vec<Instance>, _radius: f64, _field: &dyn HeightField, _seed: u32, _frame: (DVec3, DVec3, DVec3)) {}

pub fn mesh(_kind: PropKind, detail: usize) -> Mesh {
    stub::mesh(detail)
}

pub fn shade_prop(_kind: PropKind, f: &Fragment, u: &Uniforms, _style: &PropStyle, sun: Vec3) -> Vec3 {
    stub::shade_prop(f, u, sun, Vec3::new(0.08, 0.22, 0.07))
}
