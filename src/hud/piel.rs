//! La piel del HUD: los fondos de los paneles, la caratula del velocimetro
//! y el barrilete del reproductor, horneados en CPU. raylib solo los pega
//! como texturas y pone encima el texto, las agujas y los arcos.
//!
//! Cada version tiene su piel:
//! - **OCASO**: laminas de hierro casi negro con las esquinas cortadas, un
//!   filo de bronce y una brasa en la orilla de arriba; el velocimetro es un
//!   ojo (iris de brasas, pupila oscura donde va el numero).
//! - **Chapina**: tela de corte (indigo con rayitas de jaspe), con una faja
//!   tejida arriba (la misma de la transicion) y la orilla bordada con hilo
//!   dorado; el velocimetro lleva un aro de picos de faja y el disco del
//!   reproductor es un barrilete de Sumpango.
//!
//! Las formas son distancias con signo (caja redondeada, caja con chaflan,
//! circulo) y cada borde se suaviza un pixel.

use std::f32::consts::TAU;

use crate::math::{Vec3, hash_u32, smoothstep};
use crate::render::transition;

/// Un fondo de panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    /// OCASO: lamina de hierro con filo de bronce.
    Lamina,
    /// Chapina, grande: tela con la faja de motivos arriba.
    Cartel,
    /// Chapina, chico: tela con una orilla de picos.
    Placa,
}

const INDIGO: Vec3 = Vec3::new(0.055, 0.07, 0.2);
const INDIGO_DEEP: Vec3 = Vec3::new(0.03, 0.035, 0.115);
const GOLD: Vec3 = Vec3::new(0.98, 0.78, 0.22);
const BRONZE: Vec3 = Vec3::new(0.66, 0.43, 0.25);
const RED: Vec3 = transition::HUIPIL_RED;

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

#[inline]
fn put(out: &mut [u8], color: Vec3, alpha: f32) {
    out[0] = (color.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    out[1] = (color.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    out[2] = (color.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    out[3] = (alpha.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
}

/// Distancia a una caja de medios lados `(hx, hy)` con esquinas de radio `r`.
#[inline]
fn round_box(x: f32, y: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let (qx, qy) = (x.abs() - hx + r, y.abs() - hy + r);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

/// Distancia a una caja con las esquinas cortadas `c` pixeles.
#[inline]
fn cut_box(x: f32, y: f32, hx: f32, hy: f32, c: f32) -> f32 {
    let (qx, qy) = (x.abs() - hx, y.abs() - hy);
    qx.max(qy).max((qx + qy + c) * std::f32::consts::FRAC_1_SQRT_2)
}

/// El hilo torcido: claro y oscuro en diagonal.
#[inline]
fn rope(x: f32, y: f32, s: f32) -> f32 {
    0.8 + 0.2 * ((x + y) / (1.3 * s)).sin().signum()
}

/// El fondo de un panel de `w` x `h` pixeles a escala `s` (RGBA).
pub fn panel(kind: Panel, w: usize, h: usize, s: f32) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let (hx, hy) = (w as f32 * 0.5, h as f32 * 0.5);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let px = &mut out[(y * w + x) * 4..(y * w + x) * 4 + 4];
            match kind {
                Panel::Lamina => {
                    let d = cut_box(fx - hx, fy - hy, hx - 0.5, hy - 0.5, 9.0 * s);
                    if d > 0.7 {
                        continue;
                    }
                    let grain = unit(hash_u32(x as u32 ^ hash_u32(y as u32 ^ 0x51)));
                    let v = fy / h as f32;
                    let mut color = Vec3::new(0.05, 0.038, 0.04).lerp(Vec3::new(0.022, 0.018, 0.022), v) * (0.9 + 0.2 * grain);
                    let mut alpha = 0.86;
                    // La brasa: la orilla de arriba encendida al centro.
                    let across = 1.0 - ((fx - hx) / hx).abs();
                    let ember = (-fy / (9.0 * s)).exp() * across.powf(1.5);
                    color += Vec3::new(0.55, 0.16, 0.03) * ember * 0.55;
                    // El filo de bronce, mas claro arriba a la izquierda, y
                    // una linea fina adentro.
                    let lit = 0.55 + 0.45 * (1.0 - (fx / w as f32 + v) * 0.5);
                    let edge = smoothstep(-1.6 * s, -0.4 * s, d);
                    color = color.lerp(BRONZE * lit, edge);
                    alpha += (1.0 - alpha) * edge;
                    let inner = smoothstep(1.0, 0.0, (d + 5.0 * s).abs() / (0.6 * s).max(0.6));
                    color = color.lerp(BRONZE * 0.7, inner * 0.35);
                    put(px, color, alpha * smoothstep(0.7, -0.7, d));
                }
                Panel::Cartel | Panel::Placa => {
                    let big = kind == Panel::Cartel;
                    let radius = if big { 18.0 } else { 12.0 } * s;
                    let d = round_box(fx - hx, fy - hy, hx - 0.5, hy - 0.5, radius);
                    if d > 0.7 {
                        continue;
                    }
                    // El corte: indigo con rayitas de jaspe (hilos tenidos a
                    // pedazos antes de tejer).
                    let v = fy / h as f32;
                    let mut color = INDIGO.lerp(INDIGO_DEEP, v);
                    let column = (fx / (13.0 * s)).floor();
                    let thread = smoothstep(0.09, 0.03, ((fx / (13.0 * s)).fract() - 0.5).abs());
                    let piece = fy / (9.0 * s) + unit(hash_u32(column as u32 ^ 0xA5)) * 8.0;
                    let dyed = unit(hash_u32(column as u32 ^ hash_u32(piece.floor() as u32)));
                    let ends = smoothstep(0.0, 0.25, piece.fract()) * smoothstep(1.0, 0.75, piece.fract());
                    color = color.lerp(Vec3::new(0.3, 0.38, 0.62), thread * ends * f32::from(dyed > 0.5) * 0.3);
                    let mut alpha = 0.93;
                    // Arriba, la faja: completa en el cartel, solo los picos
                    // en la placa. Abajo, otra orilla de picos.
                    let rim = 3.5 * s;
                    let band = if big { 26.0 } else { 9.0 } * s;
                    let top = fy - rim;
                    if top < band {
                        let cell = band / if big { 1.0 } else { transition::EDGE };
                        let across = if big { top / band } else { transition::EDGE * (1.0 - top / band) };
                        color = transition::sash(fx / (cell * 1.5 * transition::CELLS), across.clamp(0.0, 1.0), 1.5 / cell);
                        alpha = 1.0;
                    } else if top < band + 1.6 * s {
                        color = GOLD * 0.85;
                        alpha = 1.0;
                    }
                    let foot = 7.0 * s;
                    let bottom = h as f32 - rim - fy;
                    if big && bottom < foot {
                        color = transition::sash(fx / (foot / transition::EDGE * 1.5 * transition::CELLS), (transition::EDGE * bottom / foot).clamp(0.0, 1.0), 0.05);
                        alpha = 1.0;
                    }
                    // La orilla bordada.
                    let stitch = smoothstep(rim, rim - 1.0, -d);
                    let round = 1.0 - ((-d / rim) * 2.0 - 1.0).powi(2);
                    color = color.lerp(GOLD * rope(fx, fy, s) * (0.6 + 0.45 * round.max(0.0)), stitch);
                    alpha += (1.0 - alpha) * stitch;
                    put(px, color, alpha * smoothstep(0.7, -0.7, d));
                }
            }
        }
    }
    out
}

/// De donde a donde va la escala del velocimetro, en grados de pantalla
/// (0 a la derecha, creciendo en el sentido del reloj): de abajo a la
/// izquierda, por arriba, hasta abajo a la derecha.
pub const DIAL_FROM: f32 = 135.0;
pub const DIAL_SWEEP: f32 = 270.0;

/// La caratula del velocimetro (`size` x `size`, RGBA): el aro, la cara y
/// las rayitas de la escala. El arco, la aguja y el numero van encima.
pub fn dial(chapin: bool, size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let c = size as f32 * 0.5;
    let radius = c - 1.0;
    let key = Vec3::new(-0.42, -0.62, 0.66).normalize();
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = dx.hypot(dy) / radius;
            if r > 1.0 + 1.0 / radius {
                continue;
            }
            let (ux, uy) = (dx / (r * radius).max(1e-3), dy / (r * radius).max(1e-3));
            let angle = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            // Donde cae en la escala (0 a 1), si cae.
            let along = (angle - DIAL_FROM).rem_euclid(360.0) / DIAL_SWEEP;
            let px_r = 1.0 / radius;
            // Las rayitas: 20 divisiones, las pares mas largas.
            let tick = {
                let k = (along * 20.0).round();
                let off = (along * 20.0 - k).abs() / 20.0 * DIAL_SWEEP.to_radians() * r * radius;
                let major = k as i32 % 2 == 0;
                let (from, to) = if chapin { (if major { 0.6 } else { 0.68 }, 0.76) } else { (if major { 0.7 } else { 0.78 }, 0.84) };
                let width = if major { 1.1 } else { 0.7 } * (size as f32 / 160.0).max(0.8);
                if along <= 1.0 + 0.01 { smoothstep(width + 0.6, width - 0.4, off) * smoothstep(from - px_r, from + px_r, r) * smoothstep(to + px_r, to - px_r, r) } else { 0.0 }
            };
            let (color, alpha);
            if chapin {
                let bezel = 0.82;
                if r > bezel {
                    // El aro: picos de faja amarillos y verdes sobre rojo.
                    let t = (r - bezel) / (1.0 - bezel);
                    let turns = angle / 360.0 * 40.0;
                    let peak = (turns.fract() - 0.5).abs() * 2.0;
                    let tooth = if turns as i32 % 2 == 0 { transition::YELLOW } else { transition::GREEN };
                    let mut ring = RED.lerp(tooth, smoothstep(peak + 0.08, peak - 0.08, 1.0 - t * 1.25));
                    // Dos hilos dorados: adentro y afuera.
                    let thread = smoothstep(0.16, 0.08, t).max(smoothstep(0.86, 0.94, t));
                    ring = ring.lerp(GOLD * rope(dx, dy, size as f32 / 160.0) * 0.95, thread);
                    color = ring;
                    alpha = 1.0;
                } else {
                    let face = INDIGO.lerp(INDIGO_DEEP, smoothstep(0.1, 0.82, r)) * 1.15;
                    let warm = along > 0.8;
                    let mark = if warm { Vec3::new(1.0, 0.36, 0.42) } else { Vec3::new(0.97, 0.92, 0.82) };
                    color = face.lerp(mark, tick);
                    alpha = 0.95;
                }
            } else {
                let bezel = 0.9;
                if r > bezel {
                    // El aro de bronce, con su bisel.
                    let t = (r - bezel) / (1.0 - bezel);
                    let tilt = (t - 0.5) * 1.7;
                    let n = Vec3::new(ux * tilt, uy * tilt, 1.0).normalize();
                    let grain = unit(hash_u32(x as u32 * 73 ^ hash_u32(y as u32)));
                    let spec = n.dot((key + Vec3::Z).normalize()).max(0.0).powf(24.0);
                    color = BRONZE * (0.75 + 0.3 * grain) * (0.22 + 1.0 * n.dot(key).max(0.0)) + Vec3::new(1.0, 0.82, 0.58) * spec * 0.7;
                    alpha = 1.0;
                } else {
                    // El ojo: iris de brasas (rayas que salen del centro) y
                    // la pupila oscura.
                    let (ax, ay) = (ux * 7.0, uy * 7.0);
                    let streak = 0.5 + 0.5 * ((ax * 3.1 + r * 2.0).sin() * (ay * 2.7 - r * 1.3).cos() + 0.6 * ((ax + ay) * 5.3).sin()) / 1.6;
                    let iris = smoothstep(0.44, 0.62, r) * smoothstep(0.92, 0.7, r);
                    let mut face = Vec3::new(0.035, 0.028, 0.032);
                    face += Vec3::new(0.5, 0.14, 0.03) * iris * streak * streak * 0.55;
                    let ring = smoothstep(1.5 * px_r, 0.0, (r - 0.46).abs());
                    face = face.lerp(BRONZE * 0.8, ring * 0.5);
                    color = face.lerp(Vec3::new(0.85, 0.6, 0.38), tick);
                    alpha = 0.92;
                }
            }
            let edge = smoothstep(1.0 + px_r, 1.0 - px_r, r);
            put(&mut out[(y * size + x) * 4..(y * size + x) * 4 + 4], color, alpha * edge);
        }
    }
    out
}

/// Un barrilete de Sumpango visto de frente (`size` x `size`, RGBA): el
/// centro, una estrella de petalos, gajos de papel de china, un aro de
/// picos y los flecos. Es el disco que gira en el reproductor.
pub fn kite(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let c = size as f32 * 0.5;
    let radius = c - 1.0;
    let soft = 1.2 / radius;
    let paper = [
        Vec3::new(0.12, 0.32, 0.8),
        Vec3::new(0.98, 0.76, 0.1),
        Vec3::new(0.5, 0.15, 0.62),
        Vec3::new(0.05, 0.6, 0.3),
        Vec3::new(1.0, 0.5, 0.08),
        Vec3::new(0.92, 0.2, 0.5),
    ];
    let white = Vec3::new(0.97, 0.94, 0.88);
    let ink = Vec3::new(0.08, 0.02, 0.05);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = dx.hypot(dy) / radius;
            let turn = dy.atan2(dx).rem_euclid(TAU) / TAU;
            // Los flecos: la orilla ondulada.
            let fringe = 0.955 + 0.045 * (turn * 48.0 * TAU).sin();
            if r > fringe + soft {
                continue;
            }
            let paint = |base: Vec3, color: Vec3, d: f32| base.lerp(color, smoothstep(soft, -soft, d));
            let gore = (turn * 16.0).floor() as usize;
            let mut color = if (turn * 48.0) as usize % 2 == 0 { white } else { paper[(turn * 24.0) as usize % 6] };
            // El aro de picos.
            let peak = ((turn * 32.0).fract() - 0.5).abs() * 2.0;
            let ring = RED.lerp(if (turn * 32.0) as usize % 2 == 0 { transition::YELLOW } else { white }, smoothstep(peak + 0.1, peak - 0.1, (0.9 - r) / 0.1 * 1.2));
            color = paint(color, ring, r - 0.9);
            color = paint(color, ink, (r - 0.9).abs() - 0.008);
            // Los gajos, con su costilla oscura entre uno y otro.
            let rib = ((turn * 16.0).fract() - 0.5).abs();
            color = paint(color, paper[gore % 6], r - 0.8);
            color = paint(color, paper[(gore + 3) % 6], (r - 0.64).abs() - 0.035 - 0.03 * (1.0 - rib * 2.0));
            color = color.lerp(ink, smoothstep(0.47, 0.5, rib) * f32::from(r < 0.8) * 0.8);
            // Un aro blanco con rombos.
            color = paint(color, white, r - 0.46);
            let diamond = ((turn * 24.0).fract() - 0.5).abs() * 0.5 + (r - 0.415).abs() * 6.0;
            color = paint(color, paper[5], (diamond - 0.2).max(r - 0.46));
            // La estrella de ocho petalos.
            let petal = 0.2 + 0.16 * (0.5 + 0.5 * (turn * 8.0 * TAU).cos()).powf(0.7);
            color = paint(color, paper[2], r - 0.37);
            color = paint(color, if (turn * 8.0 + 0.5) as usize % 2 == 0 { paper[1] } else { paper[4] }, r - petal);
            color = paint(color, white, r - 0.12);
            color = paint(color, RED, r - 0.075);
            color = paint(color, ink, r - 0.025);
            put(&mut out[(y * size + x) * 4..(y * size + x) * 4 + 4], color, smoothstep(fringe + soft, fringe - soft, r));
        }
    }
    out
}

/// Una chispa: un punto de luz suave (`size` x `size`, RGBA blanco).
pub fn spark(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let c = size as f32 * 0.5;
    for y in 0..size {
        for x in 0..size {
            let r = (x as f32 + 0.5 - c).hypot(y as f32 + 0.5 - c) / c;
            let glow = (-r * r * 5.0).exp() * smoothstep(1.0, 0.7, r);
            put(&mut out[(y * size + x) * 4..(y * size + x) * 4 + 4], Vec3::ONE, glow);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La piel en PNG sobre un fondo (`OCASO_PNG=carpeta cargo test --release
    /// piel_en_png -- --ignored`).
    #[test]
    #[ignore = "guarda fotos"]
    fn piel_en_png() {
        let Ok(dir) = std::env::var("OCASO_PNG") else { return };
        let save = |name: &str, w: usize, h: usize, rgba: &[u8]| {
            let mut flat = vec![255u8; w * h * 4];
            for i in 0..w * h {
                let a = rgba[i * 4 + 3] as f32 / 255.0;
                let back = if (i % w / 16 + i / w / 16) % 2 == 0 { [90.0, 70.0, 60.0] } else { [140.0, 110.0, 90.0] };
                for c in 0..3 {
                    flat[i * 4 + c] = (back[c] * (1.0 - a) + rgba[i * 4 + c] as f32 * a) as u8;
                }
            }
            crate::render::png::write_rgba(&std::path::Path::new(&dir).join(name), w as u32, h as u32, &flat).unwrap();
        };
        let start = std::time::Instant::now();
        save("lamina.png", 380, 150, &panel(Panel::Lamina, 380, 150, 1.0));
        save("cartel.png", 600, 470, &panel(Panel::Cartel, 600, 470, 1.0));
        save("placa.png", 230, 124, &panel(Panel::Placa, 230, 124, 1.0));
        println!("paneles en {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
        save("dial_ocaso.png", 320, 320, &dial(false, 320));
        save("dial_chapin.png", 320, 320, &dial(true, 320));
        save("barrilete.png", 256, 256, &kite(256));
        save("chispa.png", 32, 32, &spark(32));
    }
}
