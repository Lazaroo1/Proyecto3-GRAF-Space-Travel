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
    /// base TBN del mapeo de normales.
    pub tangents: Vec<Vec4>,
    pub indices: Vec<[u32; 3]>,
    /// Esfera envolvente en el espacio del objeto (para el culling).
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
}

impl Mesh {
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
