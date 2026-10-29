//! Camara: de donde y hacia donde se mira, y su matriz de vista y proyeccion.

use crate::math::{DMat4, DQuat, DVec3};
use crate::render::View;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraMode {
    /// Detras y arriba de la nave.
    Chase,
    /// Desde la cabina.
    Cockpit,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: DVec3,
    pub rot: DQuat,
    pub fov_y: f64,
    pub near: f64,
}

impl Camera {
    /// Matrices del frame. La vista solo rota: la traslacion ya se resto en
    /// `f64` al armar la matriz de modelo de cada objeto (render relativo a la
    /// camara), asi que aqui la camara siempre esta en el origen.
    pub fn view(&self, width: usize, height: usize) -> View {
        let aspect = width as f64 / height as f64;
        let world_to_camera = self.rot.conjugate().to_mat3();
        let projection = DMat4::perspective_reversed_infinite(self.fov_y, aspect, self.near);
        let view_proj = projection.mul(&DMat4::from_affine(&world_to_camera, DVec3::ZERO));
        let tan_y = (self.fov_y * 0.5).tan();
        let tan_x = tan_y * aspect;
        View {
            view_proj: view_proj.to_f32(),
            world_to_camera: world_to_camera.to_f32(),
            near: self.near as f32,
            tan_half_x: tan_x as f32,
            tan_half_y: tan_y as f32,
            right: (self.rot.right() * tan_x).as_vec3(),
            up: (self.rot.up() * tan_y).as_vec3(),
            forward: self.rot.forward().as_vec3(),
        }
    }
}
