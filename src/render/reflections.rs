//! Reflexion de superficies cercanas a partir del framebuffer y su z-buffer.
//! Lee una imagen terminada y escribe otra; los hilos no leen sus resultados.
use super::{
    Draw, TILE, TILE_PIXELS, View, VisPixel,
    pipeline::{ClipVertex, EXTRA_VERTEX, TriChunk, Varyings},
};
use crate::{math::Vec3, shaders::Material};
pub struct Context<'a> {
    pub src: &'a [Vec3],
    pub vis: &'a [VisPixel],
    pub chunks: &'a [TriChunk],
    pub verts: &'a [ClipVertex],
    pub draws: &'a [Draw<'a>],
    pub view: &'a View,
    pub width: usize,
    pub height: usize,
    pub tiles_x: usize,
}
fn vary<'a>(chunk: &'a TriChunk, verts: &'a [ClipVertex], id: u32) -> &'a Varyings {
    if id & EXTRA_VERTEX != 0 {
        &chunk.extra[(id & !EXTRA_VERTEX) as usize].var
    } else {
        &verts[id as usize].var
    }
}
pub fn tile(tile: usize, out: &mut [Vec3], c: &Context) {
    let x0 = tile % c.tiles_x * TILE;
    let y0 = tile / c.tiles_x * TILE;
    out.copy_from_slice(&c.src[tile * TILE_PIXELS..(tile + 1) * TILE_PIXELS]);
    for ly in 0..TILE.min(c.height - y0) {
        for lx in 0..TILE.min(c.width - x0) {
            let pixel = c.vis[tile * TILE_PIXELS + ly * TILE + lx];
            if pixel.id == VisPixel::EMPTY_ID {
                continue;
            }
            let chunk = &c.chunks[(pixel.id >> 16) as usize];
            let tri = &chunk.tris[(pixel.id & 0xffff) as usize];
            if !matches!(
                c.draws[tri.draw as usize].material,
                Material::Jardin | Material::Ship(_)
            ) {
                continue;
            }
            let px = (x0 + lx) as f32 + 0.5;
            let py = (y0 + ly) as f32 + 0.5;
            let dx = px - tri.ox;
            let dy = py - tri.oy;
            let l1 = tri.l1[0] * dx + tri.l1[1] * dy;
            let l2 = tri.l2[0] * dx + tri.l2[1] * dy;
            let l0 = 1.0 - l1 - l2;
            let q = [l0 * tri.inv_w[0], l1 * tri.inv_w[1], l2 * tri.inv_w[2]];
            let inv = 1.0 / (q[0] + q[1] + q[2]);
            let p = Varyings::blend(
                vary(chunk, c.verts, tri.verts[0]),
                vary(chunk, c.verts, tri.verts[1]),
                vary(chunk, c.verts, tri.verts[2]),
                q[0] * inv,
                q[1] * inv,
                q[2] * inv,
            );
            if p.pos.length() > 110.0 {
                continue;
            }
            let n = p.normal.normalize();
            let v = (-p.pos).normalize();
            let ndv = n.dot(v).max(0.0);
            let ray = n * (2.0 * n.dot(v)) - v;
            let start = p.pos + n * 0.12;
            let mut t = 0.4;
            for _ in 0..20 {
                let point = start + ray * t;
                let clip = c.view.view_proj.transform_point(point);
                if clip.w <= c.view.near {
                    break;
                }
                let x = ((clip.x / clip.w + 1.0) * 0.5 * c.width as f32) as i32;
                let y = ((1.0 - clip.y / clip.w) * 0.5 * c.height as f32) as i32;
                if x < 0 || y < 0 || x >= c.width as i32 || y >= c.height as i32 {
                    break;
                }
                let (x, y) = (x as usize, y as usize);
                let index = ((y / TILE) * c.tiles_x + x / TILE) * TILE_PIXELS
                    + (y % TILE) * TILE
                    + x % TILE;
                let hit = c.vis[index];
                if hit.id != VisPixel::EMPTY_ID && hit.id != pixel.id {
                    let depth = c.view.near / hit.depth;
                    let delta = clip.w - depth;
                    if delta > 0.0 && delta < (0.3 + t * 0.04) {
                        let fresnel = 0.06 + 0.55 * (1.0 - ndv).powi(5);
                        let fade = (1.0 - t / 35.0).max(0.0);
                        out[ly * TILE + lx] += c.src[index] * (fresnel * fade * 0.65);
                        break;
                    }
                }
                t += 0.25 + t * 0.18;
                if t > 35.0 {
                    break;
                }
            }
        }
    }
}
