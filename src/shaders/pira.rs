//! Pira: la marea de la estrella la esta despedazando. Sus continentes se
//! separan y entre ellos se ve el magma del interior (el relieve esta en
//! `terrain::shapes::PiraField`; sus atributos dicen que tan abajo esta cada
//! punto, de la placa al fondo).
//!
//! - **Las placas**: corteza negra de basalto con grietas que brillan (la
//!   frontera entre celdas de Voronoi, `f2 - f1` cerca de cero). Cuando una
//!   grieta es mas delgada que un pixel se ensancha y baja su brillo en la
//!   misma proporcion: desde la orbita sigue siendo una red fina y no una
//!   mancha. Del lado de la estrella (Pira le da siempre la misma cara) las
//!   grietas son mas anchas.
//! - **Las paredes de los abismos**: la misma roca, iluminada desde abajo por
//!   el magma y al rojo cerca del fondo. Desde la orilla bajan **cascadas de
//!   lava**: rayas finas sobre la pared (un ruido de la direccion, que no
//!   cambia al bajar) con pulsos que corren hacia abajo, donde la placa
//!   derrama.
//! - **El magma**: celdas de conveccion (Worley sobre un ruido que fluye)
//!   con las costuras mas calientes, del color de un cuerpo negro a su
//!   temperatura.
//! - **Latido**: todo lo que brilla late con la estrella, con un retraso que
//!   cambia sobre la superficie: se ve el pulso recorrerla.
//! - **Mientras Gargantua se la come**: la corteza se pone al rojo, las
//!   grietas se abren y el magma se calienta.
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

/// Radio de Pira: las cascadas se miden sobre la esfera.
const RADIUS: f32 = 110.0;

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let t = u.time;
    let footprint = f.pos.length() * u.pixel_angle;
    let (molten, heat, fall) = (f.color.x, f.color.y, f.color.z);
    let day = smoothstep(-0.05, 0.6, f.normal.dot(f.light_dir(u)));
    let up = f.local.normalize();
    // El pulso del latido recorre el planeta con retraso.
    let delay = (u.noise.noise(f.local * 0.01) * 0.5 + 0.5) * 2.0;
    let pulse = (0.75 + 0.5 * u.beat_at(delay)) * u.gains.pira;
    let melt = u.melt;

    // El fondo: magma en celdas de conveccion que fluyen.
    let magma = if molten > 0.01 {
        let churn = u.noise.fbm(f.local * 0.06 + Vec3::new(t * 0.025, -t * 0.015, t * 0.02), 3) * 0.5 + 0.5;
        let cells = u.noise.cells(f.local * 0.09 + Vec3::splat(churn * 1.4));
        let seam = smoothstep(0.14, 0.0, cells.1 - cells.0);
        // La piel de los centros de las celdas se enfria un poco.
        let skin = smoothstep(0.25, 0.6, cells.0) * (1.0 - seam) * 0.55;
        let temperature = 1500.0 + 800.0 * churn + 600.0 * seam + 300.0 * heat + 900.0 * melt;
        planckian_rgb(temperature) * ((2.6 + 3.0 * churn + 3.2 * seam) * (1.0 - skin) * pulse * (1.0 + melt))
    } else {
        Vec3::ZERO
    };
    if molten > 0.99 {
        return magma;
    }

    let rock = detail::rock(f, u, 0.13, footprint, 0.9);
    let basalt = rock.color * Vec3::new(0.06, 0.052, 0.05);

    // Las grietas de la placa: grandes con bordes torcidos y finas de cerca.
    let warp = Vec3::new(
        u.noise.noise(f.local * 0.035),
        u.noise.noise(f.local * 0.035 + Vec3::splat(17.0)),
        u.noise.noise(f.local * 0.05),
    );
    let plates = u.noise.cells(f.local * 0.05 + warp * 0.6);
    let big = line(plates.1 - plates.0, 0.008 + 0.012 * day + 0.01 * heat + 0.03 * melt, (footprint * 0.05).max(0.0005));
    let fine_scale = 0.16;
    let fine = if footprint * fine_scale < 0.08 {
        let cracks = u.noise.cells(f.local * fine_scale + warp * 0.3);
        line(cracks.1 - cracks.0, 0.012, footprint * fine_scale) * 0.5
    } else {
        0.0
    };
    // Cerca de la orilla la corteza esta mas rota y mas caliente.
    let edge = smoothstep(0.0, 0.08, fall);
    let cracks = (big + fine) * (0.12 + 0.15 * day + 0.3 * heat + 1.0 * edge);

    // La pared: cuanto se ha bajado hacia el magma.
    let wall = smoothstep(0.04, 0.2, fall) * (1.0 - molten);
    let deep = fall * fall;
    // Iluminada desde abajo: mas cerca del magma, mas luz; las caras que
    // miran hacia el abismo (no hacia arriba) la reciben.
    let facing = (1.0 - f.normal.dot(up).max(0.0)).max(0.25);
    let underlight = planckian_rgb(1200.0 + 700.0 * fall) * (deep * fall * facing * 0.9 * (1.0 + melt));
    // Al rojo cerca del fondo.
    let hot = smoothstep(0.7, 1.0, fall);
    let incandescent = planckian_rgb(900.0 + 800.0 * fall) * (hot * hot * 2.4);
    // Cascadas: rayas a lo largo de la pared (no cambian al bajar) donde la
    // placa derrama, con pulsos que bajan.
    let falls = {
        let stripe = 1.0 - u.noise.noise(up * (RADIUS * 0.55) + Vec3::splat(2.0)).abs();
        let source = smoothstep(0.05, 0.45, u.noise.noise(up * 5.0 + Vec3::splat(11.0))) * (0.35 + 0.65 * heat);
        let band = smoothstep(0.02, 0.12, fall) * smoothstep(1.0, 0.8, fall);
        let pour = 0.55 + 0.45 * (f.local.length() * 1.4 + t * 5.0 + stripe * 9.0).sin();
        smoothstep(0.86, 0.97, stripe) * source * band * pour
    };

    let crust = 1.0 - molten;
    let lit = pbr::shade(f, u, basalt * crust, 0.72, 0.05, rock.normal, rock.ao);
    let temperature = 1050.0 + 500.0 * big + 250.0 * heat + 900.0 * melt;
    let glow = planckian_rgb(temperature) * (cracks * 1.6 * (1.0 - wall) * crust)
        + (underlight + incandescent) * wall
        + planckian_rgb(1450.0 + 400.0 * heat) * (falls * 5.0 * wall);
    // Y la corteza misma se pone al rojo mientras Gargantua se la come.
    let ember = planckian_rgb(950.0 + 350.0 * melt) * (melt * 0.06);
    lit + glow * pulse + ember + magma * molten
}
