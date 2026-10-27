//! Pira: la estrella la esta devorando. Corteza negra de basalto partida en
//! placas, con lava que asoma por las grietas.
//!
//! - **Grietas**: la frontera entre celdas de Voronoi (`f2 - f1` cerca de
//!   cero), en dos escalas: placas grandes y fracturas finas. Cuando una
//!   grieta es mas delgada que un pixel se ensancha y baja su brillo en la
//!   misma proporcion: desde la orbita sigue siendo una red fina y no una
//!   mancha, y no parpadea.
//! - **Mares de lava**: donde el terreno marca lava, placas de costra oscura
//!   flotan sobre lava que fluye (ruido que se desplaza con el tiempo).
//! - **Lado dia**: Pira le da siempre la misma cara a la estrella; ahi la
//!   lava se ve mas y las grietas son mas anchas. De noche solo quedan las
//!   grietas incandescentes.
//! - **Latido**: las grietas brillan con cada latido de la estrella, con un
//!   retraso que cambia sobre la superficie: se ve el pulso recorrerla.
//!
//! El color de la lava es el de un cuerpo negro a su temperatura.
use super::{Fragment, Uniforms, detail, pbr};
use crate::math::blackbody::planckian_rgb;
use crate::math::{Vec3, smoothstep};

/// Mascara de una linea: `edge` es la distancia a la frontera, `width` su
/// ancho y `aa` cuanto mide un pixel en esas unidades. Si la linea es mas
/// delgada que un pixel, se ensancha y baja su pico: su brillo total se
/// conserva.
#[inline]
fn line(edge: f32, width: f32, aa: f32) -> f32 {
    smoothstep(width + aa, 0.0, edge) * (width / (width + aa)).min(1.0)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let t = u.time;
    let footprint = f.pos.length() * u.pixel_angle;
    let rock = detail::rock(f, u, 0.13, footprint, 0.9);
    let (molten, heat) = (f.color.x, f.color.y);
    let day = smoothstep(-0.05, 0.6, f.normal.dot(f.light_dir(u)));

    // Placas grandes, con bordes torcidos.
    let warp = Vec3::new(
        u.noise.fbm(f.local * 0.035, 3),
        u.noise.fbm(f.local * 0.035 + Vec3::splat(17.0), 3),
        u.noise.noise(f.local * 0.05),
    );
    let plates = u.noise.cells(f.local * 0.04 + warp * 0.6);
    let big = line(
        plates.1 - plates.0,
        0.010 + 0.012 * day + 0.01 * heat,
        (footprint * 0.04).max(0.0005),
    );
    // Fracturas finas solo de cerca.
    let fine_scale = 0.14;
    let fine = if footprint * fine_scale < 0.08 {
        let cracks = u.noise.cells(f.local * fine_scale + warp * 0.3);
        line(cracks.1 - cracks.0, 0.012, footprint * fine_scale) * 0.55
    } else {
        0.0
    };
    // Lava que fluye bajo la costra de los mares.
    let flow = u.noise.fbm(f.local * 0.06 + Vec3::new(t * 0.02, 0.0, t * 0.012), 3) * 0.5 + 0.5;
    // La lava sube entre las placas: una franja junto a cada borde, mas
    // ancha donde fluye y del lado dia.
    let reach = 0.05 + 0.12 * flow + 0.08 * day;
    let sea = molten * smoothstep(reach, reach * 0.3, plates.1 - plates.0);
    // Mientras Gargantua se la come, la lava sube por todas las grietas.
    let sea = sea.max(u.melt * smoothstep(0.12 + 0.3 * u.melt, 0.02, plates.1 - plates.0));
    let crust = 1.0 - sea;

    let basalt = rock.color * Vec3::new(0.06, 0.052, 0.05) * (0.6 + 0.6 * plates.0.min(1.0));
    let lit = pbr::shade(f, u, basalt * crust, 0.7, 0.05, rock.normal, rock.ao);

    // El pulso del latido recorre el planeta con retraso.
    let delay = (u.noise.noise(f.local * 0.01) * 0.5 + 0.5) * 2.0;
    let pulse = 0.75 + 0.5 * u.beat_at(delay);
    let cracks = (big + fine) * (0.55 + 0.45 * day + 0.6 * heat);
    let glow = (cracks * 1.4 + sea * (0.35 + 0.65 * flow)) * pulse * u.gains.pira;
    let temperature = 1150.0 + 900.0 * (flow * sea + big * 0.6 + 0.2 * day);
    // Y la corteza misma se pone al rojo.
    let ember = planckian_rgb(950.0 + 350.0 * u.melt) * (u.melt * (0.03 + 0.05 * flow));
    lit + planckian_rgb(temperature) * (glow * 2.6) + ember
}
