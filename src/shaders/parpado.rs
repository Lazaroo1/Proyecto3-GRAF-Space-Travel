//! Parpado, la luna de Vigia: roca violeta oscura, cordilleras de cristal y
//! lagos quietos.
//!
//! - **Lagos espejo**: el agua refleja el cielo y, sobre todo, a Vigia. El
//!   rayo reflejado se intersecta con la esfera de Vigia y, si le pega, ese
//!   punto se sombrea con el mismo shader de Vigia: el ojo que te sigue
//!   tambien se ve en el lago. Fresnel de Schlick: de lado el agua es casi
//!   un espejo, desde arriba se ve oscura. La nave cerca del agua levanta
//!   ondas: anillos que se alejan del punto bajo ella.
//! - **Cristal**: en las crestas altas la roca se vuelve cristal violeta,
//!   liso y brillante, con una luz adentro que late despacio y se aviva con
//!   la nave cerca.
use super::{Fragment, Uniforms, detail, pbr, vigia};
use crate::math::{Vec2, Vec3, Vec4, smoothstep};

const ROCK: Vec3 = Vec3::new(0.055, 0.04, 0.075);
const DUST: Vec3 = Vec3::new(0.2, 0.15, 0.26);
const CRYSTAL: Vec3 = Vec3::new(0.32, 0.16, 0.55);
const CRYSTAL_LIGHT: Vec3 = Vec3::new(0.75, 0.25, 1.0);
const WATER: Vec3 = Vec3::new(0.006, 0.003, 0.016);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (lake, crystal, shore) = (f.color.x, f.color.y, f.color.z);
    if lake > 0.5 {
        return water(f, u);
    }
    let footprint = f.pos.length() * u.pixel_angle;
    let rock = detail::rock(f, u, 0.18, footprint, 0.7);
    let up = f.local.normalize();
    let flat = smoothstep(0.75, 0.95, f.normal.dot(up));
    // Polvo claro en lo plano y junto al agua, roca en las paredes.
    let ground = ROCK.lerp(DUST, flat * 0.55 + shore * 0.25) * (0.6 + rock.color.luminance());
    let albedo = ground.lerp(CRYSTAL, crystal);
    let lit = pbr::shade(f, u, albedo, 0.85 - 0.7 * crystal, 0.0, rock.normal, rock.ao);
    // Vetas de luz en el cristal: laten despacio, cada una a su ritmo, y
    // se avivan con la nave cerca.
    let veins = u.noise.ridged(f.local * 0.09, 2);
    let pulse = 0.5 + 0.5 * (u.time * 0.7 + veins * 9.0).sin();
    let near = super::props::near_ship(u, f.pos, 160.0, 25.0);
    let glow = crystal * smoothstep(0.55, 0.9, veins) * (0.05 + 0.12 * pulse + 0.9 * near);
    lit + CRYSTAL_LIGHT * glow
}

/// Agua quieta: refleja el cielo y a Vigia, con ondas si la nave esta cerca.
fn water(f: &Fragment, u: &Uniforms) -> Vec3 {
    let up = f.normal;
    // Ondas que se alejan del punto bajo la nave.
    let to_ship = u.ship - f.pos;
    let height = to_ship.dot(up);
    let flat = to_ship - up * height;
    let d = flat.length().max(1e-3);
    let strength = smoothstep(45.0, 6.0, height) * (-d / 22.0).exp();
    let wave = (d * 0.8 - u.time * 5.0).cos() * strength * 0.3;
    // Y un rizo muy fino del viento.
    let breeze = u.noise.noise(f.local * 0.4 + Vec3::new(u.time * 0.3, 0.0, 0.0));
    let n = (up - flat / d * (wave * 0.12) + f.tangent.xyz().normalize() * (breeze * 0.015)).normalize();
    let v = f.view_dir();
    let ndv = n.dot(v).max(0.0);
    let r = n * (2.0 * n.dot(v)) - v;
    let fresnel = 0.02 + 0.98 * (1.0 - ndv).powi(5);
    let reflected = environment(f, r, u);
    WATER * u.sun_light.luminance() + reflected * (0.08 + 0.92 * fresnel)
}

/// Lo que se ve desde `f` en la direccion `dir`: Vigia si el rayo le pega
/// (con su propio shader), si no el cielo. Lo usan el agua y los cristales.
pub fn environment(f: &Fragment, dir: Vec3, u: &Uniforms) -> Vec3 {
    let sky = {
        let stars = u.sky.sample(dir);
        match u.sky_view {
            Some(view) => view.sample(dir).apply(stars),
            None => stars,
        }
    };
    let Some(giant) = &u.giant else { return sky };
    let oc = f.pos - giant.center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - giant.radius * giant.radius;
    let disc = b * b - c;
    if disc <= 0.0 {
        return sky;
    }
    let t = -b - disc.sqrt();
    if t <= 0.0 {
        return sky;
    }
    let hit = f.pos + dir * t;
    let normal = (hit - giant.center) / giant.radius;
    let surface = Fragment {
        ao: 1.0,
        object_to_world: giant.to_world,
        pos: hit,
        normal,
        local: giant.to_local.mul_vec(hit - giant.center),
        uv: Vec2::ZERO,
        color: Vec3::ZERO,
        tangent: Vec4::new(0.0, 0.0, 0.0, 1.0),
    };
    vigia::shade(&surface, u)
}
