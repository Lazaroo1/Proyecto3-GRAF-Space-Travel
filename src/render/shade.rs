//! Shading diferido: reconstruye los atributos del triangulo visible en cada
//! pixel y ejecuta el fragment shader de su material.
//!
//! # Correccion de perspectiva
//! Las baricentricas que salen de la pantalla son lineales en **pantalla**,
//! pero los atributos (posicion, normal, uv) son lineales en el **mundo**.
//! Interpolarlos directo deforma las texturas al ver un triangulo de lado.
//! Lo que si es lineal en pantalla es `atributo / w`, asi que se pesa cada
//! vertice por `l_i / w_i` y se normaliza por la suma.

use std::sync::atomic::{AtomicU64, Ordering};

use super::pipeline::{ClipVertex, EXTRA_VERTEX, TriChunk, Varyings};
use super::{Draw, TILE, View, VisPixel};
use crate::math::{Vec3, smoothstep};
use crate::shaders::gargantua::GasField;
use crate::shaders::{self, Fragment, Uniforms};

pub struct ShadeContext<'a> {
    pub global_vis: &'a [VisPixel],
    pub gas_field: &'a GasField,
    pub chunks: &'a [TriChunk],
    pub verts: &'a [ClipVertex],
    pub draws: &'a [Draw<'a>],
    pub view: &'a View,
    pub uniforms: &'a Uniforms<'a>,
    pub width: usize,
    pub height: usize,
    pub tiles_x: usize,
    pub shaded: &'a AtomicU64,
}

/// Sombrea los pixeles visibles del tile a partir de su visibility buffer.
pub fn shade_tile(tile: usize, vis: &[VisPixel], out: &mut [Vec3], ctx: &ShadeContext) {
    let x0 = (tile % ctx.tiles_x) * TILE;
    let y0 = (tile / ctx.tiles_x) * TILE;
    let cols = TILE.min(ctx.width - x0);
    let rows = TILE.min(ctx.height - y0);
    let inv_width = 1.0 / ctx.width as f32;
    let inv_height = 1.0 / ctx.height as f32;
    let volumes = shaders::has_volumes(ctx.uniforms);
    let mut shaded = 0u64;

    for ly in 0..rows {
        let py = (y0 + ly) as f32 + 0.5;
        for lx in 0..cols {
            let i = ly * TILE + lx;
            let px = (x0 + lx) as f32 + 0.5;
            let pixel = vis[i];
            let dir = ctx.view.pixel_direction(px, py, inv_width, inv_height);
            let sky = pixel.id == VisPixel::EMPTY_ID;
            let (color, depth) = match (sky, ctx.uniforms.black_hole) {
                (true, None) => (shaders::sky(dir, ctx.uniforms), f32::INFINITY),
                // Con Gargantua el cielo se ve a traves de su lente.
                (true, Some(hole)) => {
                    ctx.gas_field
                        .sky(hole, dir.normalize(), ctx.uniforms, px, py)
                }
                (false, hole) => {
                    shaded += 1;
                    let (color, depth) = shade_surface(pixel.id, px, py, ctx);
                    match hole {
                        Some(hole) => ctx
                            .gas_field
                            .surface(hole, dir.normalize(), color, depth, px, py),
                        None => (color, depth),
                    }
                }
            };
            // Anillos (y en el cielo, atmosferas) contra la distancia resuelta.
            out[i] = if volumes {
                shaders::volumes(color, dir, depth, sky, ctx.uniforms)
            } else {
                color
            };
        }
    }
    ctx.shaded.fetch_add(shaded, Ordering::Relaxed);
}

/// Color del pixel y distancia de la superficie a la camara.
#[inline]
fn shade_surface(id: u32, px: f32, py: f32, ctx: &ShadeContext) -> (Vec3, f32) {
    let chunk = &ctx.chunks[(id >> 16) as usize];
    let tri = &chunk.tris[(id & 0xFFFF) as usize];

    let dx = px - tri.ox;
    let dy = py - tri.oy;
    let l1 = tri.l1[0] * dx + tri.l1[1] * dy;
    let l2 = tri.l2[0] * dx + tri.l2[1] * dy;
    let l0 = 1.0 - l1 - l2;
    let q0 = l0 * tri.inv_w[0];
    let q1 = l1 * tri.inv_w[1];
    let q2 = l2 * tri.inv_w[2];
    let norm = 1.0 / (q0 + q1 + q2);

    let var = Varyings::blend(
        fetch(chunk, ctx.verts, tri.verts[0]),
        fetch(chunk, ctx.verts, tri.verts[1]),
        fetch(chunk, ctx.verts, tri.verts[2]),
        q0 * norm,
        q1 * norm,
        q2 * norm,
    );
    let draw = &ctx.draws[tri.draw as usize];
    let normal = var.normal.normalize();
    let ao = ambient_occlusion(var.pos, normal, px, py, ctx);
    let fragment = Fragment {
        ao,
        object_to_world: draw.normal,
        pos: var.pos,
        normal: var.normal.normalize(),
        local: var.local,
        uv: var.uv,
        color: var.color,
        tangent: var.tangent,
    };
    // Superficie vista a traves de la atmosfera calculada en los vertices.
    let mut color =
        shaders::shade(&draw.material, &fragment, ctx.uniforms) * var.fog_trans + var.fog_scatter;

    let debug = ctx.uniforms.debug;
    if debug.fog_only {
        color = var.fog_scatter;
    }
    if debug.lod_colors && draw.tag > 0 {
        let tint = shaders::lod_color(draw.tag - 1);
        color = tint * (0.06 + 0.7 * color.luminance());
    }
    if debug.wireframe {
        // Distancia en pixeles a cada arista: la baricentrica de pantalla
        // dividida entre la longitud de su gradiente. Las tres se anulan
        // justo sobre la arista opuesta a su vertice.
        let g1 = (tri.l1[0], tri.l1[1]);
        let g2 = (tri.l2[0], tri.l2[1]);
        let g0 = (-g1.0 - g2.0, -g1.1 - g2.1);
        let edge = (l0 / g0.0.hypot(g0.1))
            .min(l1 / g1.0.hypot(g1.1))
            .min(l2 / g2.0.hypot(g2.1));
        let line = smoothstep(1.1, 0.25, edge);
        let ink = if draw.tag > 0 {
            shaders::lod_color(draw.tag - 1)
        } else {
            Vec3::splat(0.7)
        };
        color = color.lerp(ink * 0.5, line);
    }
    (color, fragment.pos.length())
}

#[inline]
fn fetch<'a>(chunk: &'a TriChunk, verts: &'a [ClipVertex], id: u32) -> &'a Varyings {
    if id & EXTRA_VERTEX != 0 {
        &chunk.extra[(id & !EXTRA_VERTEX) as usize].var
    } else {
        &verts[id as usize].var
    }
}

/// Oclusion de horizonte en pantalla. La profundidad ya es de solo lectura.
fn ambient_occlusion(pos: Vec3, n: Vec3, px: f32, py: f32, ctx: &ShadeContext) -> f32 {
    let footprint = (pos.length() * ctx.uniforms.pixel_angle).max(0.0001);
    let r = (2.4 / footprint).clamp(2.0, 42.0);
    let mut occ = 0.0;
    let dirs = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.707, 0.707),
        (-0.707, 0.707),
        (0.707, -0.707),
        (-0.707, -0.707),
    ];
    for (k, (dx, dy)) in dirs.iter().enumerate() {
        let radius = if k < 4 { r * 0.45 } else { r };
        let x = (px + dx * radius) as i32;
        let y = (py + dy * radius) as i32;
        if x < 0 || y < 0 || x >= ctx.width as i32 || y >= ctx.height as i32 {
            continue;
        }
        let (x, y) = (x as usize, y as usize);
        let index =
            ((y / TILE) * ctx.tiles_x + x / TILE) * TILE * TILE + (y % TILE) * TILE + x % TILE;
        let sample = ctx.global_vis[index];
        if sample.id == VisPixel::EMPTY_ID || sample.depth <= 0.0 {
            continue;
        }
        let z = ctx.view.near / sample.depth;
        let point = ctx.view.pixel_direction(
            x as f32 + 0.5,
            y as f32 + 0.5,
            1.0 / ctx.width as f32,
            1.0 / ctx.height as f32,
        ) * z;
        let d = point - pos;
        let distance = d.length();
        if distance > 0.02 && distance < 7.0 {
            occ += (n.dot(d) / distance - 0.10).max(0.0) / (1.0 + distance * distance * 0.2);
        }
    }
    (1.0 - occ * 0.5).clamp(0.15, 1.0)
}
