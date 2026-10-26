//! Lo que existe en el mundo: mallas, cuerpos celestes, los objetos de la
//! superficie, la nave y la camara.

mod builder;
pub mod camera;
mod icosphere;
mod mesh;
mod obj;
pub mod props;
pub mod ship;
mod system;

pub use icosphere::icosphere;
pub use mesh::Mesh;
pub use obj::load_obj;
pub use system::{nearest_surface, Body, System};
