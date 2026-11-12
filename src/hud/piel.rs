//! Lo horneado del HUD que no es letra: el barrilete de Sumpango que gira
//! en la musica y la chispa de las brasas del titulo de OCASO. raylib solo
//! los pega como texturas.

use std::f32::consts::TAU;

use crate::math::{Vec3, smoothstep};
use crate::render::transition;

const RED: Vec3 = transition::HUIPIL_RED;

#[inline]
fn put(out: &mut [u8], color: Vec3, alpha: f32) {
    out[0] = (color.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    out[1] = (color.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    out[2] = (color.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    out[3] = (alpha.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
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
        save("barrilete.png", 256, 256, &kite(256));
        save("chispa.png", 32, 32, &spark(32));
    }
}
