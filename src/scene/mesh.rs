//! Malla de triangulos indexada.

use crate::math::{Vec2, Vec3, Vec4};

/// Tangente por defecto para mallas sin textura.
pub const NO_TANGENT: Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);

/// Todos los arreglos por vertice miden lo mismo; cada triangulo guarda tres
/// indices en orden antihorario visto desde afuera.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<Vec3>,
    /// Hacia donde crece la `u` de la textura sobre la superficie; `w` (+1 o
    /// -1) dice hacia que lado queda la bitangente. Con la normal forman la
    /// base TBN del mapeo de normales. Vacio en las mallas sin mapa de
    /// normales (el terreno, la estrella, la nave): no gastan memoria en ellas.
    pub tangents: Vec<Vec4>,
    pub indices: Vec<[u32; 3]>,
    /// Esfera envolvente en el espacio del objeto (para el culling).
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
}

impl Mesh {
    /// Deformacion continua. La cofactor del Jacobiano transporta las normales
    /// sin abrir costuras entre vertices duplicados por UV o materiales.
    pub fn deform(&mut self, field: impl Fn(Vec3) -> Vec3) {
        let eps = 0.006;
        for (p, n) in self.positions.iter_mut().zip(&mut self.normals) {
            let old = *p;
            let dx = (field(old + Vec3::X * eps) - field(old - Vec3::X * eps)) / (2.0 * eps);
            let dy = (field(old + Vec3::Y * eps) - field(old - Vec3::Y * eps)) / (2.0 * eps);
            let dz = (field(old + Vec3::Z * eps) - field(old - Vec3::Z * eps)) / (2.0 * eps);
            *p = field(old);
            *n = (dy.cross(dz) * n.x + dz.cross(dx) * n.y + dx.cross(dy) * n.z).normalize();
        }
        self.update_bounds();
    }
    /// Recalcula la esfera envolvente: centro de la caja y el vertice mas lejano.
    pub fn update_bounds(&mut self) {
        let Some(&first) = self.positions.first() else {
            self.bounds_center = Vec3::ZERO;
            self.bounds_radius = 0.0;
            return;
        };
        let (min, max) = self
            .positions
            .iter()
            .fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p)));
        let center = (min + max) * 0.5;
        self.bounds_center = center;
        self.bounds_radius = self
            .positions
            .iter()
            .map(|&p| (p - center).length())
            .fold(0.0, f32::max);
    }

    /// Centra la malla en el origen y la escala para que su lado mas largo
    /// mida `length`.
    pub fn normalize_size(&mut self, length: f32) {
        self.update_bounds();
        let (min, max) = self.positions.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        );
        let center = (min + max) * 0.5;
        let scale = length / (max - min).max_element();
        for p in &mut self.positions {
            *p = (*p - center) * scale;
        }
        self.update_bounds();
    }
}
