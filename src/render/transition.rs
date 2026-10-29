//! La transicion entre versiones (V): la imagen de la version que se deja
//! se guarda tal como estaba y, encima de la nueva, se abre una cortina
//! desde el centro de la pantalla. El borde de la cortina es una franja con
//! dibujo que gira mientras se abre:
//! - Al entrar a la version chapina, una **faja tejida** como las de los
//!   trajes: dos orillas de picos amarillos y verdes y, en medio, sobre el
//!   rojo del huipil, una fila de motivos: el quetzal, el volcan humeando,
//!   la mazorca, el barrilete y la greca escalonada maya.
//! - Al volver a OCASO, una brasa que se apaga hacia afuera.
//!
//! Los motivos son formas de distancia con signo en las coordenadas de su
//! cuadro (`x` a lo largo de la franja, `y` hacia afuera), con el borde
//! suavizado un pixel. Todo se calcula por pixel sobre la imagen ya
//! terminada (RGBA8), en paralelo por filas.

use crate::math::{Vec3, smoothstep};
use crate::parallel;

/// Segundos que tarda la cortina en cubrir la pantalla.
pub const WIPE: f32 = 1.6;
/// Ancho de la franja del borde, en fracciones de media diagonal.
const BAND: f32 = 0.26;
/// Cuadros de motivos alrededor de la franja.
const CELLS: f32 = 20.0;
/// Cuanto de la franja ocupa cada orilla de picos.
const EDGE: f32 = 0.17;

const HUIPIL_RED: Vec3 = Vec3::new(0.6, 0.04, 0.07);
const YELLOW: Vec3 = Vec3::new(0.98, 0.76, 0.1);
const GREEN: Vec3 = Vec3::new(0.05, 0.55, 0.25);
const QUETZAL: Vec3 = Vec3::new(0.1, 0.75, 0.35);
const BREAST: Vec3 = Vec3::new(0.95, 0.15, 0.1);
const BLUE: Vec3 = Vec3::new(0.12, 0.32, 0.8);
const PURPLE: Vec3 = Vec3::new(0.5, 0.15, 0.62);
const ORANGE: Vec3 = Vec3::new(1.0, 0.5, 0.08);
const WHITE: Vec3 = Vec3::new(0.96, 0.93, 0.86);
const INK: Vec3 = Vec3::new(0.08, 0.02, 0.03);

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
    let front = eased * (1.0 + BAND * 1.2);
    let (cx, cy) = (width as f32 * 0.5, height as f32 * 0.5);
    let half = cx.hypot(cy);
    // Lo que mide un pixel en las coordenadas de un cuadro de motivo.
    let soft = (1.0 / (half * BAND)).max(0.004) * 1.5;
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
                Toward::Chapin => sash(angle / std::f32::consts::TAU + 0.5 + progress * 0.12, v, soft),
                Toward::Ocaso => ember(angle, v, progress),
            };
            for (out, value) in pixel.iter_mut().zip([band.x, band.y, band.z]) {
                *out = (value.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    });
}

/// La faja: `along` da la vuelta (0 a 1) y `v` cruza la franja (0 a 1).
fn sash(along: f32, v: f32, soft: f32) -> Vec3 {
    if v < EDGE || v > 1.0 - EDGE {
        // Las orillas: picos que se alternan amarillo y verde sobre rojo.
        let t = if v < EDGE { v / EDGE } else { (1.0 - v) / EDGE };
        let u = (along * CELLS * 3.0).fract();
        let peak = (u - 0.5).abs() * 2.0;
        let color = if (along * CELLS * 3.0) as i32 % 2 == 0 { YELLOW } else { GREEN };
        let line = smoothstep(0.12, 0.06, t);
        return HUIPIL_RED.lerp(color, smoothstep(peak + soft * 3.0, peak - soft * 3.0, t)).lerp(INK, line);
    }
    let w = (v - EDGE) / (1.0 - 2.0 * EDGE);
    let cell = (along * CELLS).floor() as i32;
    let x = (along * CELLS).fract() - 0.5;
    let y = w - 0.5;
    // Rayas finas amarillas arriba y abajo del registro de motivos.
    let mut c = HUIPIL_RED.lerp(YELLOW, smoothstep(0.03, 0.0, (y.abs() - 0.45).abs()));
    let s = soft * 1.2;
    match cell.rem_euclid(5) {
        0 => quetzal(&mut c, x, y, s),
        1 => volcano(&mut c, x, y, s),
        2 => corn(&mut c, x, y, s),
        3 => kite(&mut c, x, y, s),
        _ => fret(&mut c, x, y, s),
    }
    c
}

/// Pinta `color` donde la distancia `d` es negativa.
#[inline]
fn paint(base: &mut Vec3, color: Vec3, d: f32, soft: f32) {
    *base = base.lerp(color, smoothstep(soft, -soft, d));
}

#[inline]
fn circle(x: f32, y: f32, c: (f32, f32), r: f32) -> f32 {
    (x - c.0).hypot(y - c.1) - r
}

#[inline]
fn ellipse(x: f32, y: f32, c: (f32, f32), r: (f32, f32)) -> f32 {
    // Aproximada: buena cerca del borde, que es lo que se ve.
    let (px, py) = ((x - c.0) / r.0, (y - c.1) / r.1);
    (px.hypot(py) - 1.0) * r.0.min(r.1)
}

#[inline]
fn rect(x: f32, y: f32, c: (f32, f32), h: (f32, f32)) -> f32 {
    ((x - c.0).abs() - h.0).max((y - c.1).abs() - h.1)
}

/// Distancia a un segmento de `a` a `b` con grosor `r`.
#[inline]
fn stroke(x: f32, y: f32, a: (f32, f32), b: (f32, f32), r: f32) -> f32 {
    let (px, py) = (x - a.0, y - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let t = ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0);
    (px - bx * t).hypot(py - by * t) - r
}

/// El quetzal de perfil: cuerpo verde, pecho rojo, cresta y la cola larga
/// de dos plumas que cuelga.
fn quetzal(c: &mut Vec3, x: f32, y: f32, s: f32) {
    let tail = stroke(x, y, (-0.1, 0.0), (-0.38, -0.34), 0.03).min(stroke(x, y, (-0.08, -0.02), (-0.3, -0.4), 0.025));
    paint(c, QUETZAL, tail, s);
    let body = ellipse(x, y, (0.02, 0.06), (0.17, 0.11));
    paint(c, QUETZAL, body, s);
    paint(c, BREAST, ellipse(x, y, (0.08, 0.0), (0.09, 0.07)).max(body), s);
    paint(c, QUETZAL, circle(x, y, (0.15, 0.18), 0.075), s);
    paint(c, QUETZAL, stroke(x, y, (0.13, 0.24), (0.17, 0.3), 0.025), s);
    paint(c, YELLOW, stroke(x, y, (0.21, 0.18), (0.27, 0.16), 0.018), s);
    paint(c, INK, circle(x, y, (0.17, 0.2), 0.016), s);
}

/// El volcan: un cono azul oscuro con lava en la punta y humo que sube.
fn volcano(c: &mut Vec3, x: f32, y: f32, s: f32) {
    // El cono: abajo de dos lineas inclinadas y arriba de la base.
    let cone = ((x.abs() * 1.1 + y) - 0.12).max(-y - 0.38);
    paint(c, BLUE * 0.6, cone, s);
    paint(c, ORANGE, cone.max(-(y - 0.02)), s);
    for (k, (px, py, r)) in [(0.02f32, 0.2f32, 0.07f32), (0.1, 0.3, 0.06), (0.04, 0.4, 0.05)].into_iter().enumerate() {
        let gray = 0.75 - 0.1 * k as f32;
        paint(c, Vec3::splat(gray), circle(x, y, (px, py), r), s);
    }
}

/// La mazorca: granos amarillos en cuadricula y dos hojas verdes.
fn corn(c: &mut Vec3, x: f32, y: f32, s: f32) {
    paint(c, GREEN, ellipse(x, y, (-0.1, -0.12), (0.08, 0.26)), s);
    paint(c, GREEN, ellipse(x, y, (0.1, -0.12), (0.08, 0.26)), s);
    let cob = ellipse(x, y, (0.0, 0.04), (0.11, 0.3));
    paint(c, YELLOW, cob, s);
    let kernels = ((x * 28.0).rem_euclid(1.0) - 0.5).abs().max(((y * 22.0).rem_euclid(1.0) - 0.5).abs());
    paint(c, ORANGE * 0.8, (kernels - 0.42).max(cob), s * 2.0);
}

/// El barrilete: una rueda de papel de china en gajos de colores, con su
/// centro blanco y flecos.
fn kite(c: &mut Vec3, x: f32, y: f32, s: f32) {
    let r = x.hypot(y);
    let a = y.atan2(x);
    let fringe = r - (0.34 + 0.03 * (a * 24.0).sin());
    paint(c, WHITE, fringe, s);
    let gore = ((a / std::f32::consts::TAU + 0.5) * 8.0).floor() as i32;
    let colors = [BLUE, YELLOW, PURPLE, GREEN];
    paint(c, colors[gore.rem_euclid(4) as usize], r - 0.3, s);
    paint(c, BREAST, (r - 0.2).abs() - 0.02, s);
    paint(c, WHITE, r - 0.08, s);
}

/// La greca escalonada maya: un gancho en espiral cuadrada con escalones.
fn fret(c: &mut Vec3, x: f32, y: f32, s: f32) {
    let t = 0.05;
    let hook = rect(x, y, (0.0, -0.3), (0.3, t))
        .min(rect(x, y, (0.25, -0.05), (t, 0.3)))
        .min(rect(x, y, (0.05, 0.2), (0.25, t)))
        .min(rect(x, y, (-0.15, 0.05), (t, 0.2)))
        .min(rect(x, y, (-0.03, -0.1), (0.12, t)));
    // Escalones del otro lado.
    let steps = rect(x, y, (-0.3, -0.25), (0.08, 0.1))
        .min(rect(x, y, (-0.36, -0.08), (0.06, 0.08)))
        .min(rect(x, y, (-0.4, 0.06), (0.04, 0.07)));
    paint(c, WHITE, hook, s);
    paint(c, YELLOW, steps, s);
}

/// La brasa: amarilla adentro, roja hacia afuera, con lenguas que tiemblan.
fn ember(angle: f32, v: f32, progress: f32) -> Vec3 {
    let tongue = 0.5 + 0.5 * (angle * 23.0 + progress * 30.0).sin() * (angle * 7.0 - progress * 11.0).cos();
    let heat = (1.0 - v) * (0.7 + 0.3 * tongue);
    Vec3::new(0.25, 0.02, 0.0)
        .lerp(Vec3::new(1.0, 0.45, 0.08), heat)
        .lerp(Vec3::new(1.0, 0.9, 0.6), heat * heat * heat)
}
