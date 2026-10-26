//! Cuaterniones unitarios en `f64` para orientar la nave, la camara y los
//! planetas.
//!
//! Se usan en vez de angulos de Euler porque la nave gira libre en 3D: con
//! yaw/pitch/roll, al apuntar hacia arriba dos ejes se alinean (gimbal lock)
//! y se pierde un grado de libertad. Un cuaternion compone giros sin ese
//! problema y se interpola suave con `slerp`.

use std::ops::{Mul, Neg};

use super::{DMat3, DVec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DQuat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for DQuat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl DQuat {
    pub const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn from_axis_angle(axis: DVec3, angle: f64) -> Self {
        let a = axis.normalize();
        let (s, c) = (angle * 0.5).sin_cos();
        Self {
            w: c,
            x: a.x * s,
            y: a.y * s,
            z: a.z * s,
        }
    }

    /// Giro de angulo `|v|` alrededor de `v` (mapa exponencial). Integra una
    /// velocidad angular: `q * from_scaled_axis(omega * dt)`.
    pub fn from_scaled_axis(v: DVec3) -> Self {
        let angle = v.length();
        if angle < 1e-12 {
            return Self {
                w: 1.0,
                x: v.x * 0.5,
                y: v.y * 0.5,
                z: v.z * 0.5,
            }
            .normalize();
        }
        Self::from_axis_angle(v / angle, angle)
    }

    /// Inverso del anterior: el eje por el angulo del giro mas corto.
    pub fn to_scaled_axis(self) -> DVec3 {
        let q = if self.w < 0.0 { -self } else { self };
        let v = DVec3::new(q.x, q.y, q.z);
        let s = v.length();
        if s < 1e-12 {
            return v * 2.0;
        }
        v * (2.0 * s.atan2(q.w) / s)
    }

    /// Cuaternion de una matriz de rotacion (metodo de Shepperd).
    pub fn from_mat3(m: &DMat3) -> Self {
        let m = &m.m;
        let trace = m[0][0] + m[1][1] + m[2][2];
        let q = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            Self {
                w: 0.25 * s,
                x: (m[2][1] - m[1][2]) / s,
                y: (m[0][2] - m[2][0]) / s,
                z: (m[1][0] - m[0][1]) / s,
            }
        } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
            let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
            Self {
                w: (m[2][1] - m[1][2]) / s,
                x: 0.25 * s,
                y: (m[0][1] + m[1][0]) / s,
                z: (m[0][2] + m[2][0]) / s,
            }
        } else if m[1][1] > m[2][2] {
            let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
            Self {
                w: (m[0][2] - m[2][0]) / s,
                x: (m[0][1] + m[1][0]) / s,
                y: 0.25 * s,
                z: (m[1][2] + m[2][1]) / s,
            }
        } else {
            let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
            Self {
                w: (m[1][0] - m[0][1]) / s,
                x: (m[0][2] + m[2][0]) / s,
                y: (m[1][2] + m[2][1]) / s,
                z: 0.25 * s,
            }
        };
        q.normalize()
    }

    /// Orientacion cuyo -Z apunta a `forward` y cuyo +Y queda lo mas cerca
    /// posible de `up`.
    pub fn look_rotation(forward: DVec3, up: DVec3) -> Self {
        let z = -forward.normalize();
        let mut x = up.cross(z);
        if x.length_squared() < 1e-12 {
            // `up` paralelo a la mirada: cualquier perpendicular sirve.
            x = DVec3::X.cross(z);
            if x.length_squared() < 1e-12 {
                x = DVec3::Y.cross(z);
            }
        }
        let x = x.normalize();
        let y = z.cross(x);
        Self::from_mat3(&DMat3::from_cols(x, y, z))
    }

    #[inline]
    pub fn conjugate(self) -> Self {
        Self {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }

    #[inline]
    pub fn dot(self, o: Self) -> f64 {
        self.w * o.w + self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn normalize(self) -> Self {
        let len = self.dot(self).sqrt();
        if len > 0.0 {
            Self {
                w: self.w / len,
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        } else {
            Self::IDENTITY
        }
    }

    /// Rota un vector: `v' = q v q*`, en la forma barata con dos productos cruz.
    #[inline]
    pub fn rotate(self, v: DVec3) -> DVec3 {
        let q = DVec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }

    pub fn to_mat3(self) -> DMat3 {
        DMat3::from_cols(
            self.rotate(DVec3::X),
            self.rotate(DVec3::Y),
            self.rotate(DVec3::Z),
        )
    }

    #[inline]
    pub fn right(self) -> DVec3 {
        self.rotate(DVec3::X)
    }

    #[inline]
    pub fn up(self) -> DVec3 {
        self.rotate(DVec3::Y)
    }

    /// Hacia adelante es -Z, como en la camara.
    #[inline]
    pub fn forward(self) -> DVec3 {
        self.rotate(-DVec3::Z)
    }

    /// Interpolacion esferica: velocidad angular constante entre dos orientaciones.
    pub fn slerp(self, o: Self, t: f64) -> Self {
        let mut d = self.dot(o);
        let mut o = o;
        // q y -q son la misma rotacion: tomar el camino corto.
        if d < 0.0 {
            o = -o;
            d = -d;
        }
        if d > 0.9995 {
            return Self {
                w: self.w + (o.w - self.w) * t,
                x: self.x + (o.x - self.x) * t,
                y: self.y + (o.y - self.y) * t,
                z: self.z + (o.z - self.z) * t,
            }
            .normalize();
        }
        let theta = d.acos();
        let s = theta.sin();
        let a = ((1.0 - t) * theta).sin() / s;
        let b = (t * theta).sin() / s;
        Self {
            w: self.w * a + o.w * b,
            x: self.x * a + o.x * b,
            y: self.y * a + o.y * b,
            z: self.z * a + o.z * b,
        }
    }
}

impl Mul for DQuat {
    type Output = Self;

    /// Producto de Hamilton: `(a * b).rotate(v) == a.rotate(b.rotate(v))`.
    #[inline]
    fn mul(self, b: Self) -> Self {
        let a = self;
        Self {
            w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
            x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
            z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        }
    }
}

impl Neg for DQuat {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            w: -self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn product_composes_rotations_in_order() {
        let a = DQuat::from_axis_angle(DVec3::Y, 0.7);
        let b = DQuat::from_axis_angle(DVec3::X, -1.1);
        let v = DVec3::new(0.3, -2.0, 5.0);
        assert!(close((a * b).rotate(v), a.rotate(b.rotate(v))));
    }

    #[test]
    fn look_rotation_points_forward_and_keeps_up() {
        let forward = DVec3::new(1.0, 0.2, -3.0).normalize();
        let q = DQuat::look_rotation(forward, DVec3::Y);
        assert!(close(q.forward(), forward));
        assert!(q.up().y > 0.9);
        let m = q.to_mat3();
        assert!(close(DQuat::from_mat3(&m).rotate(DVec3::Z), q.rotate(DVec3::Z)));
    }

    #[test]
    fn scaled_axis_round_trip() {
        let v = DVec3::new(0.4, -0.2, 1.3);
        assert!(close(DQuat::from_scaled_axis(v).to_scaled_axis(), v));
    }
}
