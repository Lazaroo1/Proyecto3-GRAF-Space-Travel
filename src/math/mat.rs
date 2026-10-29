//! Matrices con vectores columna (`v' = M * v`), guardadas por filas:
//! `m[fila][columna]`.
//!
//! Las de `f64` construyen las transformaciones del mundo con toda la
//! precision; al final se convierten a `f32` para transformar vertices.

use super::{DVec3, Vec3, Vec4};

/// Rotacion o escala 3x3 en `f64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DMat3 {
    pub m: [[f64; 3]; 3],
}

impl DMat3 {
    pub const IDENTITY: Self = Self {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// Matriz cuyas columnas son las imagenes de los ejes X, Y y Z.
    pub fn from_cols(x: DVec3, y: DVec3, z: DVec3) -> Self {
        Self {
            m: [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]],
        }
    }

    #[inline]
    pub fn mul_vec(&self, v: DVec3) -> DVec3 {
        let m = &self.m;
        DVec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }

    pub fn mul(&self, o: &Self) -> Self {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = (0..3).map(|k| self.m[i][k] * o.m[k][j]).sum();
            }
        }
        Self { m: r }
    }

    pub fn transpose(&self) -> Self {
        let m = &self.m;
        Self {
            m: [
                [m[0][0], m[1][0], m[2][0]],
                [m[0][1], m[1][1], m[2][1]],
                [m[0][2], m[1][2], m[2][2]],
            ],
        }
    }

    pub fn scale(&self, s: f64) -> Self {
        Self {
            m: self.m.map(|row| row.map(|v| v * s)),
        }
    }

    pub fn to_f32(&self) -> Mat3 {
        Mat3 {
            m: self.m.map(|row| row.map(|v| v as f32)),
        }
    }
}

/// Rotacion 3x3 en `f32`, para las normales en la etapa de vertices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub m: [[f32; 3]; 3],
}

impl Mat3 {
    #[inline]
    pub fn mul_vec(&self, v: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }
}

/// Transformacion 4x4 en `f64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DMat4 {
    pub m: [[f64; 4]; 4],
}

impl DMat4 {
    /// Transformacion afin: primero `r` (rotacion y escala), luego trasladar `t`.
    pub fn from_affine(r: &DMat3, t: DVec3) -> Self {
        let m = &r.m;
        Self {
            m: [
                [m[0][0], m[0][1], m[0][2], t.x],
                [m[1][0], m[1][1], m[1][2], t.y],
                [m[2][0], m[2][1], m[2][2], t.z],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Proyeccion perspectiva con **Z invertida y plano lejano en el infinito**.
    ///
    /// La camara mira hacia -Z. Queda `w_clip = -z` (la distancia hacia
    /// adelante) y `z_clip = near`, asi que la profundidad que llega al raster
    /// es `z_ndc = near / w`: 1 en el plano cercano y 0 en el infinito.
    ///
    /// # Por que asi
    /// Con la proyeccion clasica, la profundidad se amontona cerca de 1 y un
    /// `f32` deja de distinguir superficies lejanas: el sol, a miles de
    /// unidades, pelearia con lo que tiene detras (z-fighting). `near / w`
    /// es proporcional a `1/w`, y como los `f32` tienen precision
    /// **relativa**, el error queda proporcional a la distancia en todo el
    /// rango: igual de bueno a 1 unidad que a 10 000. Ademas no hay plano
    /// lejano que recorte el sistema solar.
    pub fn perspective_reversed_infinite(fov_y: f64, aspect: f64, near: f64) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        Self {
            m: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, 0.0, near],
                [0.0, 0.0, -1.0, 0.0],
            ],
        }
    }

    pub fn mul(&self, o: &Self) -> Self {
        let mut r = [[0.0; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = (0..4).map(|k| self.m[i][k] * o.m[k][j]).sum();
            }
        }
        Self { m: r }
    }

    pub fn to_f32(&self) -> Mat4 {
        Mat4 {
            m: self.m.map(|row| row.map(|v| v as f32)),
        }
    }
}

/// Transformacion 4x4 en `f32`, la que se aplica a cada vertice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    /// Transforma un punto (`w = 1`) a coordenadas homogeneas.
    #[inline]
    pub fn transform_point(&self, p: Vec3) -> Vec4 {
        let m = &self.m;
        Vec4::new(
            m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
            m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
            m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
            m[3][0] * p.x + m[3][1] * p.y + m[3][2] * p.z + m[3][3],
        )
    }

    /// Transforma un punto con una matriz afin (ignora la fila de proyeccion).
    #[inline]
    pub fn transform_affine(&self, p: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
            m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
            m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversed_depth_is_one_at_near_and_falls_with_distance() {
        let near = 0.05;
        let p = DMat4::perspective_reversed_infinite(1.2, 16.0 / 9.0, near).to_f32();
        let at = |d: f32| {
            let c = p.transform_point(Vec3::new(0.0, 0.0, -d));
            c.z / c.w
        };
        assert!((at(near as f32) - 1.0).abs() < 1e-6);
        assert!(at(1.0) > at(10.0) && at(10.0) > at(10_000.0) && at(10_000.0) > 0.0);
        // Precision relativa: dos superficies a 10 000 y 10 001 unidades se distinguen.
        assert!(at(10_000.0) > at(10_001.0));
    }
}
