//! Regolito fotografico y hielo fracturado: NASA LROC, PBR en CPU.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::{Vec3, smoothstep};
pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let photo = u
        .textures
        .images
        .moon
        .sphere(f.local, footprint / f.local.length().max(1.0) * 0.35);
    let rock = detail::rock(f, u, 0.13, footprint, 0.55);
    let ice = f.color.x;
    let snow = f.color.y;
    let albedo = photo * Vec3::new(0.65, 0.71, 0.82) * (0.55 + rock.color.luminance())
        + Vec3::new(0.20, 0.24, 0.28) * snow * 0.5;
    let cracks = u.noise.cells(f.local * 0.035);
    let seams = smoothstep(0.012, 0.002, cracks.1 - cracks.0) * ice * 0.25;
    pbr::shade(
        f,
        u,
        albedo * (1.0 - seams * 0.75),
        0.85 - 0.65 * ice,
        0.03,
        rock.normal,
        rock.ao,
    ) + Vec3::new(0.012, 0.025, 0.05) * (seams * u.gains.requiem * 0.2)
}
