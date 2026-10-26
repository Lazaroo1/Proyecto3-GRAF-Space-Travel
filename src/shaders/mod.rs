//! Fragment shaders propios.
//!
//! Cada material es una funcion que recibe los atributos ya interpolados del
//! pixel visible (posicion, normal, coordenadas del objeto, uv, color) y
//! devuelve su color HDR lineal. Corren en el pase de shading diferido: una
//! sola vez por pixel, despues de resolver la visibilidad.

mod planet;
mod ship;
mod sky;
mod sun;

use crate::math::{Perlin, Vec2, Vec3};

pub use planet::PlanetStyle;
pub use ship::ShipStyle;
pub use sky::shade as sky;
pub use sun::{heartbeat, SunStyle};

/// Valores comunes a todos los pixeles del frame.
pub struct Uniforms<'a> {
    /// Tiempo de simulacion en segundos.
    pub time: f32,
    /// Centro de la estrella, relativo a la camara.
    pub sun_pos: Vec3,
    pub sun_radius: f32,
    /// Luz de la estrella que llega a los planetas (color * intensidad).
    pub sun_light: Vec3,
    pub noise: &'a Perlin,
    /// Angulo que cubre un pixel, en radianes.
    pub pixel_angle: f32,
}

/// Atributos interpolados del pixel visible.
pub struct Fragment {
    /// Posicion en el mundo relativa a la camara: la camara es el origen.
    pub pos: Vec3,
    /// Normal en el mundo, unitaria.
    pub normal: Vec3,
    /// Posicion en el espacio del objeto: gira con el.
    pub local: Vec3,
    /// Todavia sin uso: lo usaran las texturas y el mapeo de normales.
    #[allow(dead_code)]
    pub uv: Vec2,
    pub color: Vec3,
}

impl Fragment {
    /// Direccion unitaria hacia la camara.
    #[inline]
    pub fn view_dir(&self) -> Vec3 {
        (-self.pos).normalize()
    }

    /// Direccion unitaria hacia la estrella.
    #[inline]
    pub fn light_dir(&self, u: &Uniforms) -> Vec3 {
        (u.sun_pos - self.pos).normalize()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Material {
    Sun(SunStyle),
    Planet(PlanetStyle),
    Ship(ShipStyle),
}

#[inline]
pub fn shade(material: &Material, f: &Fragment, u: &Uniforms) -> Vec3 {
    match material {
        Material::Sun(style) => sun::shade(f, u, style),
        Material::Planet(style) => planet::shade(f, u, style),
        Material::Ship(style) => ship::shade(f, u, style),
    }
}
