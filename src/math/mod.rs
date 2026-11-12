//! Matematica propia: vectores, matrices, cuaterniones, ruido y color.
//!
//! Hay dos precisiones a proposito. El mundo vive en `f64` (`DVec3`, `DQuat`,
//! `DMat3`, `DMat4`) porque el sistema mide miles de unidades y la nave
//! necesita precision de centimetros junto a la superficie. El pipeline de
//! render trabaja en `f32` (`Vec3`, `Mat4`) y solo recibe posiciones
//! **relativas a la camara**: numeros pequenos justo donde importa la
//! precision. La resta grande (objeto - camara) se hace en `f64`.

pub mod blackbody;
pub mod geodesic;
pub mod neutron;
pub mod relativity;
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
/// `atan2` con un polinomio de grado 9 (Abramowitz y Stegun 4.4.49, error
/// menor que 1.2e-5 rad): la lente de Gargantua lo pide una vez por pixel y
/// el de la biblioteca es lento.
#[inline]
pub fn fast_atan2(y: f32, x: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (ax, ay) = (x.abs(), y.abs());
    let a = ax.min(ay) / ax.max(ay).max(1e-30);
    let s = a * a;
    let mut r = a * (0.999_866 + s * (-0.330_299_5 + s * (0.180_141 + s * (-0.085_133 + s * 0.020_835_1))));
    if ay > ax {
        r = FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = PI - r;
    }
    if y < 0.0 {
        r = -r;
    }
    r
}

#[inline]
pub fn fast_exp(x: f32) -> f32 {
    let y = (x * std::f32::consts::LOG2_E).clamp(-126.0, 126.0);
    let i = y.floor();
    let f = y - i;
    let p = 1.0 + f * (0.695_976 + f * (0.224_940 + f * 0.079_440));
    f32::from_bits(((i as i32 + 127) as u32) << 23) * p
}

/// `log2(x)` rapido para `x > 0` (error menor que 1e-6): el exponente sale
/// de los bits del `f32` y la mantisa `m`, llevada a `[1/sqrt 2, sqrt 2)`,
/// con `ln m = 2 atanh(s) = 2 (s + s^3/3 + s^5/5 + s^7/7)`,
/// `s = (m - 1) / (m + 1)` (con `|s| < 0.172` el termino que falta es de
/// 3e-8). Con `x = 0` da -127.
#[inline]
pub fn fast_log2(x: f32) -> f32 {
    let bits = x.to_bits();
    let mut e = ((bits >> 23) & 0xff) as i32 - 127;
    let mut m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    if m > std::f32::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let ln = 2.0 * s * (1.0 + s2 * (1.0 / 3.0 + s2 * (0.2 + s2 * (1.0 / 7.0))));
    e as f32 + ln * std::f32::consts::LOG2_E
}

/// `x^p` rapido para `x >= 0` (error relativo de unos 3e-4, el de
/// `fast_exp`): `2^(p log2 x)`.
#[inline]
pub fn fast_pow(x: f32, p: f32) -> f32 {
    if x <= 0.0 {
        return if p == 0.0 { 1.0 } else { 0.0 };
    }
    fast_exp(p * fast_log2(x) * std::f32::consts::LN_2)
}

/// Factor de suavizado exponencial independiente de los fps: con `rate` en
/// 1/s, avanzar `1 - e^(-rate*dt)` por frame da el mismo movimiento a 30 o a
/// 300 fps. Un `lerp(a, b, 0.1)` fijo por frame no lo da.
#[inline]
pub fn smoothing(rate: f64, dt: f64) -> f64 {
    1.0 - (-rate * dt).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_pow_matches_the_library() {
        for i in 1..4000 {
            let x = i as f32 / 4000.0 * 3.0;
            assert!((fast_log2(x) - x.log2()).abs() < 2e-6, "{x}");
            for p in [0.45f32, 1.0 / 2.2, 0.6, 5.0, 40.0, 600.0] {
                // (Los exponentes altos son de brillos especulares: la base
                // es un coseno, hasta 1. `fast_exp` se topa en 2^126.)
                if p > 50.0 && x > 1.0 {
                    continue;
                }
                let (a, b) = (fast_pow(x, p), x.powf(p));
                assert!((a - b).abs() <= 1e-3 * b + 1e-30, "{x}^{p}: {a} {b}");
            }
        }
        assert_eq!(fast_pow(0.0, 2.0), 0.0);
    }

    #[test]
    fn fast_atan2_matches_the_library() {
        for i in 0..2000 {
            let angle = -3.14 + i as f32 * 0.00314;
            for radius in [0.001f32, 1.0, 50.0] {
                let (y, x) = (angle.sin() * radius, angle.cos() * radius);
                assert!((fast_atan2(y, x) - y.atan2(x)).abs() < 2e-5, "{angle}");
            }
        }
    }
}
