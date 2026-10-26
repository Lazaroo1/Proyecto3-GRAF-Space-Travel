//! Matematica propia: vectores, matrices, cuaterniones, ruido y color.
//!
//! Hay dos precisiones a proposito. El mundo vive en `f64` (`DVec3`, `DQuat`,
//! `DMat3`, `DMat4`) porque el sistema mide miles de unidades y la nave
//! necesita precision de centimetros junto a la superficie. El pipeline de
//! render trabaja en `f32` (`Vec3`, `Mat4`) y solo recibe posiciones
//! **relativas a la camara**: numeros pequenos justo donde importa la
//! precision. La resta grande (objeto - camara) se hace en `f64`.

pub mod blackbody;
mod mat;
mod noise;
mod quat;
mod vec;

pub use mat::{DMat3, DMat4, Mat3, Mat4};
pub use noise::Perlin;
pub use quat::DQuat;
pub use vec::{DVec3, Vec2, Vec3, Vec4};

/// Interpolacion suave de Hermite: 0 antes de `edge0`, 1 despues de `edge1`.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Factor de suavizado exponencial independiente de los fps: con `rate` en
/// 1/s, avanzar `1 - e^(-rate*dt)` por frame da el mismo movimiento a 30 o a
/// 300 fps. Un `lerp(a, b, 0.1)` fijo por frame no lo da.
#[inline]
pub fn smoothing(rate: f64, dt: f64) -> f64 {
    1.0 - (-rate * dt).exp()
}
