//! Materiales originales de Blender con UV, rugosidad y metalicidad.
use super::{Fragment, Uniforms, pbr};
use crate::math::{Vec2, Vec3, smoothstep};
use crate::scene::Surface;
#[derive(Clone, Copy, Debug)]
pub struct ShipStyle {
    pub specular: f32,
    pub shininess: f32,
}
pub fn shade(f: &Fragment, u: &Uniforms, s: &ShipStyle) -> Vec3 {
    let id = (f.color.x - 100.0).round().max(0.0) as usize;
    let Some(mat) = u.textures.images.ship.get(id) else {
        return Vec3::new(0.2, 0.0, 0.2);
    };
    let uv = Vec2::new(f.uv.x, 1.0 - f.uv.y);
    let footprint = f.pos.length() * u.pixel_angle / 1.1;
    let albedo = mat
        .image
        .as_ref()
        .map_or(mat.base, |map| map.sample(uv, footprint));
    let plate = u.textures.hull.sample(f.uv * 2.0, footprint * 2.0);
    let n = f.perturb(plate.normal, 0.10);
    let wear = u.noise.noise(f.local * 29.0) * 0.5 + 0.5;
    let rough = (mat.rough + (wear - 0.5) * 0.18).clamp(0.09, 0.9);
    let dirt = smoothstep(0.55, 0.22, plate.height) * 0.55;
    let a = albedo * (1.0 - dirt);
    let lit = pbr::shade(
        f,
        u,
        a,
        rough,
        mat.metal,
        n,
        (0.45 + 0.55 * plate.height).clamp(0.25, 1.0),
    );
    let emission = if mat.emission.length_squared() > 0.01 {
        let source = if mat.image.is_some() {
            albedo
        } else {
            Vec3::ONE
        };
        mat.emission * source * 0.7
    } else {
        Vec3::ZERO
    };
    let _ = (s.specular, s.shininess);
    lit + emission
}

/// Las naves botella: el color de cada cara viene del `.mtl` (vidrio,
/// etiquetas, chapa). Todo es liso y pulido: el vidrio refleja el cielo con
/// Fresnel y la estrella deja un brillo fino. El vidrio deja pasar la luz:
/// a contraluz brilla de su color, y siempre un poco, para que la botella
/// no se pierda en la sombra.
pub fn bottle(f: &Fragment, u: &Uniforms) -> Vec3 {
    let albedo = f.color;
    let lit = pbr::shade(f, u, albedo, 0.16, 0.0, f.normal, 1.0);
    let v = f.view_dir();
    let rim = (1.0 - f.normal.dot(v).abs()).powi(4);
    let through = (-f.normal.dot(f.light_dir(u))).max(0.0);
    let glow = albedo * (u.sun_light * (0.35 * through) + Vec3::splat(0.06));
    lit + glow + (albedo * 0.5 + Vec3::splat(0.04)) * rim * 0.35
}

/// Los vehiculos chapines (el tuk-tuk, la camioneta y el colectivo). El color
/// de cada pieza viene del `.mtl` y de que es (`Surface`, en la `u`):
/// - **pintura con laca**: la base con la rugosidad que da su `Ns` y encima
///   una capa transparente pulida (Fresnel de 4 %) que deja el brillo fino
///   del sol y refleja el cielo, como un carro encerado;
/// - **cromo, plata y oro**: metal pulido que tiñe su reflejo;
/// - **vidrio**: casi negro, con el reflejo del cielo que sube en los bordes;
/// - **las LEDs**: emiten su color (`Ke`) mas fuerte que lo que ilumina el
///   sol, y el bloom las riega;
/// - **hule**: mate.
pub fn vehicle(f: &Fragment, u: &Uniforms) -> Vec3 {
    let albedo = f.color;
    let n = f.normal;
    match Surface::from_code(f.uv.x) {
        Surface::Light => pbr::shade(f, u, albedo * 0.3, 0.2, 0.0, n, 1.0) + albedo * (f.uv.y * 1.2),
        Surface::Metal => pbr::shade(f, u, albedo, (f.uv.y * 0.7).max(0.14), 1.0, n, 1.0),
        Surface::Glass => {
            let v = f.view_dir();
            let reflect = n * (2.0 * n.dot(v)) - v;
            let fresnel = 0.04 + 0.96 * (1.0 - n.dot(v).abs()).powi(5);
            pbr::shade(f, u, albedo, 0.05, 0.0, n, 1.0) + u.sky.sample(reflect) * (0.5 * fresnel)
        }
        Surface::Rubber => pbr::shade(f, u, albedo, 0.85, 0.0, n, 1.0),
        Surface::Paint => {
            let rough = f.uv.y.clamp(0.2, 0.8);
            let base = pbr::shade(f, u, albedo, rough, 0.0, n, 1.0);
            // La laca solo en lo brillante (los murales y las lonas son mates).
            let gloss = smoothstep(0.5, 0.3, rough);
            if gloss <= 0.0 {
                return base;
            }
            base + pbr::shade(f, u, Vec3::ZERO, 0.1, 0.0, n, 1.0) * (0.6 * gloss)
        }
    }
}
