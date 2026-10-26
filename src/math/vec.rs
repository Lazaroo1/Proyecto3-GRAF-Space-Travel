//! Vectores de 2, 3 y 4 componentes.
//!
//! `Vec3` (f32) es el vector del pipeline y de los shaders; `DVec3` (f64) el
//! del mundo y la fisica. Los dos salen de la misma macro para tener
//! exactamente las mismas operaciones.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

macro_rules! vec3_type {
    ($name:ident, $t:ty) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        pub struct $name {
            pub x: $t,
            pub y: $t,
            pub z: $t,
        }

        impl $name {
            pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
            pub const ONE: Self = Self::new(1.0, 1.0, 1.0);
            pub const X: Self = Self::new(1.0, 0.0, 0.0);
            pub const Y: Self = Self::new(0.0, 1.0, 0.0);
            pub const Z: Self = Self::new(0.0, 0.0, 1.0);

            #[inline]
            pub const fn new(x: $t, y: $t, z: $t) -> Self {
                Self { x, y, z }
            }

            #[inline]
            pub const fn splat(v: $t) -> Self {
                Self { x: v, y: v, z: v }
            }

            #[inline]
            pub fn dot(self, o: Self) -> $t {
                self.x * o.x + self.y * o.y + self.z * o.z
            }

            #[inline]
            pub fn cross(self, o: Self) -> Self {
                Self::new(
                    self.y * o.z - self.z * o.y,
                    self.z * o.x - self.x * o.z,
                    self.x * o.y - self.y * o.x,
                )
            }

            #[inline]
            pub fn length_squared(self) -> $t {
                self.dot(self)
            }

            #[inline]
            pub fn length(self) -> $t {
                self.length_squared().sqrt()
            }

            /// Vector unitario. El vector cero se devuelve sin cambios para no
            /// propagar NaN por el pipeline.
            #[inline]
            pub fn normalize(self) -> Self {
                let len = self.length();
                if len > 0.0 { self * (1.0 / len) } else { self }
            }

            /// Recorta la longitud a `max` conservando la direccion.
            #[inline]
            pub fn clamp_length(self, max: $t) -> Self {
                let len = self.length();
                if len > max && len > 0.0 { self * (max / len) } else { self }
            }

            #[inline]
            pub fn lerp(self, o: Self, t: $t) -> Self {
                self + (o - self) * t
            }

            #[inline]
            pub fn min(self, o: Self) -> Self {
                Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
            }

            #[inline]
            pub fn max(self, o: Self) -> Self {
                Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
            }

            #[inline]
            pub fn abs(self) -> Self {
                Self::new(self.x.abs(), self.y.abs(), self.z.abs())
            }

            #[inline]
            pub fn floor(self) -> Self {
                Self::new(self.x.floor(), self.y.floor(), self.z.floor())
            }

            #[inline]
            pub fn max_element(self) -> $t {
                self.x.max(self.y).max(self.z)
            }

            #[inline]
            pub fn min_element(self) -> $t {
                self.x.min(self.y).min(self.z)
            }
        }

        impl Add for $name {
            type Output = Self;
            #[inline]
            fn add(self, o: Self) -> Self {
                Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
            }
        }

        impl Sub for $name {
            type Output = Self;
            #[inline]
            fn sub(self, o: Self) -> Self {
                Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
            }
        }

        // Producto componente a componente: sirve para mezclar colores.
        impl Mul for $name {
            type Output = Self;
            #[inline]
            fn mul(self, o: Self) -> Self {
                Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
            }
        }

        impl Mul<$t> for $name {
            type Output = Self;
            #[inline]
            fn mul(self, s: $t) -> Self {
                Self::new(self.x * s, self.y * s, self.z * s)
            }
        }

        impl Mul<$name> for $t {
            type Output = $name;
            #[inline]
            fn mul(self, v: $name) -> $name {
                v * self
            }
        }

        impl Div<$t> for $name {
            type Output = Self;
            #[inline]
            fn div(self, s: $t) -> Self {
                self * (1.0 / s)
            }
        }

        impl Neg for $name {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self {
                Self::new(-self.x, -self.y, -self.z)
            }
        }

        impl AddAssign for $name {
            #[inline]
            fn add_assign(&mut self, o: Self) {
                *self = *self + o;
            }
        }

        impl SubAssign for $name {
            #[inline]
            fn sub_assign(&mut self, o: Self) {
                *self = *self - o;
            }
        }

        impl MulAssign<$t> for $name {
            #[inline]
            fn mul_assign(&mut self, s: $t) {
                *self = *self * s;
            }
        }

        impl DivAssign<$t> for $name {
            #[inline]
            fn div_assign(&mut self, s: $t) {
                *self = *self / s;
            }
        }
    };
}

vec3_type!(Vec3, f32);
vec3_type!(DVec3, f64);

impl Vec3 {
    #[inline]
    pub fn as_dvec3(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }

    /// Luminancia relativa (Rec. 709) de un color lineal.
    #[inline]
    pub fn luminance(self) -> f32 {
        0.2126 * self.x + 0.7152 * self.y + 0.0722 * self.z
    }
}

impl DVec3 {
    #[inline]
    pub fn as_vec3(self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

/// Coordenadas homogeneas de clip: `(x, y, z, w)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub fn lerp(self, o: Self, t: f32) -> Self {
        Self::new(
            self.x + (o.x - self.x) * t,
            self.y + (o.y - self.y) * t,
            self.z + (o.z - self.z) * t,
            self.w + (o.w - self.w) * t,
        )
    }
}
