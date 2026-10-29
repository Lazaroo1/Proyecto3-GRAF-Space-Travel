//! Roca craterizada fotografica y metal enterrado en el suelo.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::Vec3;
pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let photo = u
        .textures
        .images
        .moon
        .sphere(f.local, footprint / f.local.length().max(1.0) * 0.4);
    let rock = detail::rock(f, u, 0.1, footprint, 0.8);
    let trench = f.color.y;
    let albedo = photo * rock.color * Vec3::new(0.55, 0.43, 0.34) * (1.0 - 0.55 * trench);
    pbr::shade(
        f,
        u,
        albedo,
        0.85 - 0.35 * trench,
        0.1 + 0.3 * trench,
        rock.normal,
        rock.ao,
    ) * (0.75 + 0.25 * u.gains.cementerio)
}
