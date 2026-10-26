//! Lo que existe en el mundo: mallas, cuerpos celestes, la nave y la camara.

pub mod camera;
mod icosphere;
mod mesh;
mod obj;
pub mod ship;
mod system;

pub use icosphere::icosphere;
pub use mesh::Mesh;
pub use obj::load_obj;
pub use system::{nearest_surface, Body, System};
