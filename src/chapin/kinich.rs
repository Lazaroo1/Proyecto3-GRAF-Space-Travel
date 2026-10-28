//! Kinich Ajaw, el sol maya (la version chapina de la estrella).
//!
//! Provisional: por ahora un disco liso.

use crate::math::Vec3;
use crate::shaders::{Fragment, SunStyle, Uniforms};

pub fn shade(f: &Fragment, u: &Uniforms, s: &SunStyle) -> Vec3 {
    let mu = f.normal.dot(f.view_dir()).max(0.0);
    Vec3::new(1.6, 0.75, 0.2) * (s.intensity * u.gains.sun * (0.4 + 0.6 * mu))
}
