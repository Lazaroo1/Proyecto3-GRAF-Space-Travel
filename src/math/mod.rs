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
pub use noise::{hash_u32, Perlin};
pub use quat::DQuat;
pub use vec::{DVec3, Vec2, Vec3, Vec4};

/// Interpolacion suave de Hermite: 0 antes de `edge0`, 1 despues de `edge1`.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `e^x` rapido para las atmosferas (error relativo menor que 3e-4).
///
/// Se calcula como `2^(x / ln 2)`: la parte entera de la potencia se arma
/// directo en los bits del exponente de un `f32`, y `2^f` de la parte
/// fraccionaria sale de un polinomio cubico ajustado en `[0, 1)`.
#[inline]
pub fn fast_exp(x: f32) -> f32 {
    let y = (x * std::f32::consts::LOG2_E).clamp(-126.0, 126.0);
    let i = y.floor();
    let f = y - i;
    let p = 1.0 + f * (0.695_976 + f * (0.224_940 + f * 0.079_440));
    f32::from_bits(((i as i32 + 127) as u32) << 23) * p
}

/// Factor de suavizado exponencial independiente de los fps: con `rate` en
/// 1/s, avanzar `1 - e^(-rate*dt)` por frame da el mismo movimiento a 30 o a
/// 300 fps. Un `lerp(a, b, 0.1)` fijo por frame no lo da.
#[inline]
pub fn smoothing(rate: f64, dt: f64) -> f64 {
    1.0 - (-rate * dt).exp()
}
