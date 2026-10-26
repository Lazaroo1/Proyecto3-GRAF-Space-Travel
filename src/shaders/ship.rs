//! Nave: Blinn-Phong con el color de los materiales del `.mtl`, mas una luz
//! de relleno y un borde frio para que se lea contra el espacio negro.

use super::{Fragment, Uniforms};
use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct ShipStyle {
    pub specular: f32,
    pub shininess: f32,
}

pub fn shade(f: &Fragment, u: &Uniforms, s: &ShipStyle) -> Vec3 {
    let n = f.normal;
    let v = f.view_dir();
    let l = f.light_dir(u);
    let ndl = n.dot(l).max(0.0);
    let ndv = n.dot(v).max(0.0);
    let specular = if ndl > 0.0 {
        let half = (l + v).normalize();
        s.specular * n.dot(half).max(0.0).powf(s.shininess)
    } else {
        0.0
    };
    // En el espacio no hay luz ambiente: sin relleno, el lado de sombra de la
    // nave desapareceria contra el fondo.
    let fill = f.color * (0.05 + 0.07 * ndv);
    let rim = Vec3::new(0.45, 0.6, 1.0) * (0.35 * (1.0 - ndv).powi(3));
    f.color * u.sun_light * ndl + u.sun_light * specular + fill + rim
}
