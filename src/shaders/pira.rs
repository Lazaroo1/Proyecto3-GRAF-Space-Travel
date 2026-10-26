//! Placas de basalto rugoso, grietas fracturadas y lava bajo la costra.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::blackbody::planckian_rgb;
use crate::math::{Vec3, smoothstep};
pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let rock = detail::rock(f, u, 0.13, footprint, 0.9);
    let warp = u.noise.fbm(f.local * 0.035, 3);
    let shear = Vec3::new(
        warp,
        u.noise.fbm(f.local * 0.08 + Vec3::splat(17.0), 3),
        u.noise.noise(f.local * 0.13),
    );
    let cells = u.noise.cells(f.local * 0.043 + shear * 0.7);
    let edge = cells.1 - cells.0;
    let aa = (footprint * 0.043).max(0.001);
    let width = 0.012 + 0.022 * (u.noise.fbm(f.local * 0.23, 3) * 0.5 + 0.5);
    let crack = smoothstep(width + aa, 0.004 - aa, edge);
    let heat = f.color.y;
    let fire = u.noise.fbm(
        f.local * 0.17 + Vec3::new(u.time * 0.03, 0.0, u.time * 0.015),
        3,
    ) * 0.5
        + 0.5;
    let rock_color = rock.color * Vec3::new(0.11, 0.10, 0.095) * (0.5 + 0.5 * cells.0.min(1.0));
    let lit = pbr::shade(f, u, rock_color, 0.72, 0.05, rock.normal, rock.ao);
    let molten = f.color.x * smoothstep(0.38, 0.58, fire) * 0.28;
    let glow = (crack.powi(2) * (0.3 + 1.0 * heat) + molten) * (0.2 + 0.8 * fire) * u.gains.pira;
    lit + planckian_rgb(1200.0 + 850.0 * fire) * glow * 3.5
}
