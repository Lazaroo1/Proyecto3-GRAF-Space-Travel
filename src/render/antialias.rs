//! FXAA en CPU sobre el framebuffer final: muestreo bilineal a lo largo de
//! las diagonales de contraste. Conserva el centro en regiones uniformes.
use crate::math::Vec3;

pub fn rows(first: usize, out: &mut [u8], src: &[u8], width: usize, height: usize) {
    let at = |x: usize, y: usize| {
        let i = (y * width + x) * 4;
        Vec3::new(src[i] as f32, src[i + 1] as f32, src[i + 2] as f32)
    };
    let bilinear = |x: f32, y: f32| {
        let x = x.clamp(0.0, (width - 1) as f32);
        let y = y.clamp(0.0, (height - 1) as f32);
        let (ix, iy) = (x as usize, y as usize);
        at(ix, iy)
            .lerp(at((ix + 1).min(width - 1), iy), x - ix as f32)
            .lerp(
                at(ix, (iy + 1).min(height - 1)).lerp(
                    at((ix + 1).min(width - 1), (iy + 1).min(height - 1)),
                    x - ix as f32,
                ),
                y - iy as f32,
            )
    };
    let luma = |v: Vec3| v.x * 0.299 + v.y * 0.587 + v.z * 0.114;
    for (k, pixel) in out.chunks_exact_mut(4).enumerate() {
        let x = k % width;
        let y = first + k / width;
        let m = at(x, y);
        let c = if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
            m
        } else {
            let (nw, ne, sw, se) = (
                luma(at(x - 1, y - 1)),
                luma(at(x + 1, y - 1)),
                luma(at(x - 1, y + 1)),
                luma(at(x + 1, y + 1)),
            );
            let lm = luma(m);
            let low = lm.min(nw).min(ne).min(sw).min(se);
            let high = lm.max(nw).max(ne).max(sw).max(se);
            if high - low < 10.0f32.max(high * 0.12) {
                m
            } else {
                let mut dx = -((nw + ne) - (sw + se));
                let mut dy = (nw + sw) - (ne + se);
                let reduce = ((nw + ne + sw + se) * 0.03125).max(0.5);
                let inv = 1.0 / (dx.abs().min(dy.abs()) + reduce);
                dx = (dx * inv).clamp(-6.0, 6.0);
                dy = (dy * inv).clamp(-6.0, 6.0);
                let sample = |t: f32| bilinear(x as f32 + dx * t, y as f32 + dy * t);
                let a = (sample(-1.0 / 6.0) + sample(1.0 / 6.0)) * 0.5;
                let b = a * 0.5 + (sample(-0.5) + sample(0.5)) * 0.25;
                if luma(b) < low || luma(b) > high {
                    a
                } else {
                    b
                }
            }
        };
        pixel.copy_from_slice(&[c.x.round() as u8, c.y.round() as u8, c.z.round() as u8, 255]);
    }
}
