//! Fragment shaders propios.
//!
//! Cada material es una funcion que recibe los atributos ya interpolados del
//! pixel visible (posicion, normal, coordenadas del objeto, uv, color) y
//! devuelve su color HDR lineal. Corren en el pase de shading diferido: una
//! sola vez por pixel, despues de resolver la visibilidad.

mod requiem;
mod ship;
mod sky;
mod sun;

use crate::math::{Perlin, Vec2, Vec3};

pub use ship::ShipStyle;
pub use sky::{shade as sky, SkyCube};
pub use sun::{heartbeat, SunStyle};

/// Vistas de depuracion para demostrar el trabajo del renderer.
#[derive(Clone, Copy, Debug, Default)]
pub struct DebugView {
    /// Tine cada chunk de terreno con el color de su nivel de detalle.
    pub lod_colors: bool,
    /// Dibuja las aristas de los triangulos.
    pub wireframe: bool,
}

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
    /// Estrellas y nebulosa horneadas.
    pub sky: &'a SkyCube,
    /// Angulo que cubre un pixel, en radianes.
    pub pixel_angle: f32,
    pub debug: DebugView,
}

/// Atributos interpolados del pixel visible.
pub struct Fragment {
    /// Posicion en el mundo relativa a la camara: la camara es el origen.
    pub pos: Vec3,
    /// Normal en el mundo, unitaria.
    pub normal: Vec3,
    /// Posicion en el espacio del objeto: gira con el.
    pub local: Vec3,
    /// En el terreno: altura normalizada y pendiente.
    pub uv: Vec2,
    /// Color del material, o mascaras del terreno.
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
    Requiem,
    Ship(ShipStyle),
}

#[inline]
pub fn shade(material: &Material, f: &Fragment, u: &Uniforms) -> Vec3 {
    match material {
        Material::Sun(style) => sun::shade(f, u, style),
        Material::Requiem => requiem::shade(f, u),
        Material::Ship(style) => ship::shade(f, u, style),
    }
}

/// Color de depuracion de cada nivel de detalle.
pub fn lod_color(level: u32) -> Vec3 {
    const PALETTE: [Vec3; 8] = [
        Vec3::new(0.9, 0.2, 0.2),
        Vec3::new(0.95, 0.55, 0.15),
        Vec3::new(0.95, 0.9, 0.2),
        Vec3::new(0.3, 0.85, 0.3),
        Vec3::new(0.2, 0.8, 0.85),
        Vec3::new(0.25, 0.4, 0.95),
        Vec3::new(0.6, 0.3, 0.9),
        Vec3::new(0.95, 0.4, 0.8),
    ];
    PALETTE[level as usize % PALETTE.len()]
}
