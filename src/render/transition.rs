//! La transicion entre versiones (V): la imagen de la version que se deja
//! se guarda tal como estaba y, encima de la nueva, se abre una cortina
//! desde el centro de la pantalla. El borde de la cortina es una franja con
//! dibujo: al entrar a la version chapina, un tejido de huipil (rombos en
//! franjas de colores con su contorno oscuro, como en un telar de cintura);
//! al volver a OCASO, una brasa que se apaga hacia afuera. Todo se calcula
//! por pixel sobre la imagen ya terminada (RGBA8), en paralelo por filas.

use crate::math::{Vec3, hash_u32, smoothstep};
use crate::parallel;

/// Segundos que tarda la cortina en cubrir la pantalla.
pub const WIPE: f32 = 1.4;
/// Ancho de la franja del borde, en fracciones de media diagonal.
const BAND: f32 = 0.16;

/// Colores del huipil: rojo, amarillo, azul, verde, morado y naranja.
const HUIPIL: [Vec3; 6] = [
    Vec3::new(0.78, 0.06, 0.1),
    Vec3::new(0.96, 0.74, 0.08),
    Vec3::new(0.08, 0.28, 0.72),
    Vec3::new(0.1, 0.55, 0.24),
    Vec3::new(0.5, 0.12, 0.6),
    Vec3::new(0.95, 0.42, 0.06),
];

/// A que version se va.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Chapin,
    Ocaso,
}

/// Mezcla `from` (la imagen anterior) sobre `rgba` (la nueva) en el momento
/// `progress` (0 recien empieza, 1 la nueva cubre todo).
pub fn blend(rgba: &mut [u8], from: &[u8], width: usize, height: usize, progress: f32, toward: Toward) {
    if from.len() != rgba.len() || progress >= 1.0 {
        return;
    }
    // Arranca rapido y frena al final.
    let eased = 1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3);
    let front = eased * (1.0 + BAND * 1.5);
    let (cx, cy) = (width as f32 * 0.5, height as f32 * 0.5);
    let half = cx.hypot(cy);
    parallel::chunks_mut(rgba, width * 4, 8, |y, row| {
        let dy = y as f32 + 0.5 - cy;
        let old = &from[y * width * 4..(y + 1) * width * 4];
        for x in 0..width {
            let dx = x as f32 + 0.5 - cx;
            let r = dx.hypot(dy) / half;
            // 0 en el borde de adentro de la franja (ya la version nueva),
            // 1 en el de afuera (todavia la anterior).
            let v = (r - (front - BAND)) / BAND;
            if v <= 0.0 {
                continue;
            }
            let pixel = &mut row[x * 4..x * 4 + 3];
            if v >= 1.0 {
                pixel.copy_from_slice(&old[x * 4..x * 4 + 3]);
                continue;
            }
            let angle = dy.atan2(dx);
            let band = match toward {
                Toward::Chapin => weave(angle, v),
                Toward::Ocaso => ember(angle, v, progress),
            };
            for k in 0..3 {
                pixel[k] = (band[k].clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    });
}

/// El tejido: la franja se divide en cuadros a lo largo del borde; cada
/// cuadro lleva un rombo de un color sobre otro, con su contorno oscuro.
fn weave(angle: f32, v: f32) -> [f32; 3] {
    let u = (angle / std::f32::consts::TAU + 0.5) * 48.0;
    let cell = u.floor() as i32;
    let (fu, fv) = (u.fract() - 0.5, v - 0.5);
    let diamond = fu.abs() + fv.abs();
    let h = hash_u32(cell.rem_euclid(48) as u32 ^ 0x4019);
    let back = HUIPIL[(h % 6) as usize];
    let front = HUIPIL[((h >> 8) % 6) as usize];
    let inner = smoothstep(0.36, 0.3, diamond);
    let outline = smoothstep(0.06, 0.0, (diamond - 0.4).abs()) + smoothstep(0.12, 0.05, v.min(1.0 - v));
    let color = back.lerp(front, inner) * (1.0 - 0.8 * outline.min(1.0));
    [color.x, color.y, color.z]
}

/// La brasa: amarilla adentro, roja hacia afuera, con lenguas que tiemblan.
fn ember(angle: f32, v: f32, progress: f32) -> [f32; 3] {
    let tongue = 0.5 + 0.5 * (angle * 23.0 + progress * 30.0).sin() * (angle * 7.0 - progress * 11.0).cos();
    let heat = (1.0 - v) * (0.7 + 0.3 * tongue);
    let color = Vec3::new(0.25, 0.02, 0.0).lerp(Vec3::new(1.0, 0.45, 0.08), heat).lerp(Vec3::new(1.0, 0.9, 0.6), heat * heat * heat);
    [color.x, color.y, color.z]
}
