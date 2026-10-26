//! Etapas geometricas: vertices y triangulos.
//!
//! La etapa de triangulos hace, por cada triangulo:
//! 1. **Descarte por frustum**: si sus tres vertices quedan del mismo lado de
//!    un plano del frustum, no se ve.
//! 2. **Recorte**: si cruza el plano cercano (o se sale de la banda de guarda)
//!    se recorta con Sutherland-Hodgman en espacio de clip y se triangula en
//!    abanico.
//! 3. **Division de perspectiva y viewport**: a pixeles en punto fijo 24.8.
//! 4. **Descarte de espaldas**: por el signo del area en pantalla.
//! 5. **Setup**: caja envolvente y planos de interpolacion.
//! 6. **Reparto**: el triangulo se anota en la lista de cada tile que toca.
//!
//! # Banda de guarda
//! Recortar contra los cuatro lados de la pantalla es caro y casi nunca hace
//! falta: el raster ya limita su recorrido a la pantalla. Solo se recorta
//! contra los lados cuando un vertice se sale `GUARD_BAND` veces del frustum,
//! que es cuando las coordenadas en punto fijo podrian desbordarse.

use super::stats::TriCounters;
use super::{Draw, View, TILE};
use crate::math::{Vec2, Vec3, Vec4};
use crate::shaders::atmosphere::{integrate_all, Fog};
use crate::shaders::{self, Uniforms};

/// Bits de subpixel del punto fijo: 1/256 de pixel.
pub const SUBPIXEL_BITS: u32 = 8;
pub const SUBPIXEL: i64 = 1 << SUBPIXEL_BITS;
/// Triangulos de entrada por bloque de trabajo de la etapa de triangulos.
pub const TRIS_PER_CHUNK: usize = 4096;
/// Bit que marca un vertice creado por el recorte (vive en el bloque).
pub const EXTRA_VERTEX: u32 = 1 << 31;

const GUARD_BAND: f32 = 8.0;
const CLIP_NEAR: u32 = 1;
const CLIP_RIGHT: u32 = 2;
const CLIP_LEFT: u32 = 4;
const CLIP_TOP: u32 = 8;
const CLIP_BOTTOM: u32 = 16;
const CLIP_GUARD: u32 = CLIP_RIGHT | CLIP_LEFT | CLIP_TOP | CLIP_BOTTOM;

/// Atributos que viajan del vertice al pixel.
#[derive(Clone, Copy, Debug, Default)]
pub struct Varyings {
    /// Posicion en el mundo relativa a la camara.
    pub pos: Vec3,
    /// Normal en el mundo (sin normalizar despues de interpolar).
    pub normal: Vec3,
    /// Posicion en el espacio del objeto.
    pub local: Vec3,
    pub uv: Vec2,
    pub color: Vec3,
    /// Atmosfera entre la camara y el vertice: luz dispersada y transmitancia.
    pub fog_scatter: Vec3,
    pub fog_trans: Vec3,
}

impl Varyings {
    #[inline]
    fn lerp(&self, o: &Self, t: f32) -> Self {
        Self {
            pos: self.pos.lerp(o.pos, t),
            normal: self.normal.lerp(o.normal, t),
            local: self.local.lerp(o.local, t),
            uv: self.uv + (o.uv - self.uv) * t,
            color: self.color.lerp(o.color, t),
            fog_scatter: self.fog_scatter.lerp(o.fog_scatter, t),
            fog_trans: self.fog_trans.lerp(o.fog_trans, t),
        }
    }

    /// Combinacion con pesos baricentricos (ya corregidos por perspectiva).
    #[inline]
    pub fn blend(a: &Self, b: &Self, c: &Self, wa: f32, wb: f32, wc: f32) -> Self {
        Self {
            pos: a.pos * wa + b.pos * wb + c.pos * wc,
            normal: a.normal * wa + b.normal * wb + c.normal * wc,
            local: a.local * wa + b.local * wb + c.local * wc,
            uv: a.uv * wa + b.uv * wb + c.uv * wc,
            color: a.color * wa + b.color * wb + c.color * wc,
            fog_scatter: a.fog_scatter * wa + b.fog_scatter * wb + c.fog_scatter * wc,
            fog_trans: a.fog_trans * wa + b.fog_trans * wb + c.fog_trans * wc,
        }
    }
}

/// Pasos de la atmosfera por vertice. Basta con pocos: la atmosfera cambia
/// suave y el resultado se interpola entre vertices.
const FOG_STEPS: usize = 5;

/// Vertice ya transformado: coordenadas de clip y atributos.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClipVertex {
    pub clip: Vec4,
    pub var: Varyings,
}

impl ClipVertex {
    /// En espacio de clip (antes de dividir por w) los atributos varian
    /// linealmente a lo largo de una arista, asi que recortar es interpolar.
    #[inline]
    fn lerp(&self, o: &Self, t: f32) -> Self {
        Self {
            clip: self.clip.lerp(o.clip, t),
            var: self.var.lerp(&o.var, t),
        }
    }
}

/// Triangulo listo para rasterizar y sombrear.
#[derive(Clone, Copy, Debug)]
pub struct TriRecord {
    /// Vertices en pixeles, en punto fijo con `SUBPIXEL_BITS` de fraccion.
    pub fx: [i32; 3],
    pub fy: [i32; 3],
    /// Caja envolvente en pixeles, inclusiva y recortada a la pantalla.
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
    /// Origen de los planos de interpolacion (vertice 0), en pixeles.
    pub ox: f32,
    pub oy: f32,
    /// Baricentricas de pantalla respecto al vertice 0:
    /// `l1 = l1[0]*dx + l1[1]*dy`, `l2 = l2[0]*dx + l2[1]*dy`, `l0 = 1-l1-l2`.
    pub l1: [f32; 2],
    pub l2: [f32; 2],
    /// Profundidad en el vertice 0 y su gradiente en pantalla. `near / w` es
    /// lineal en pantalla, asi que se interpola sin correccion.
    pub z0: f32,
    pub dz: [f32; 2],
    /// `1/w` por vertice, para corregir la perspectiva de los atributos.
    pub inv_w: [f32; 3],
    /// Indice del dibujo (para saber su material).
    pub draw: u32,
    /// Indices de los vertices: globales, o del bloque si llevan `EXTRA_VERTEX`.
    pub verts: [u32; 3],
}

/// Salida de un bloque de la etapa de triangulos. Se reutiliza entre frames
/// para no pedir memoria en cada uno.
#[derive(Default)]
pub struct TriChunk {
    pub tris: Vec<TriRecord>,
    /// Vertices creados por el recorte.
    pub extra: Vec<ClipVertex>,
    /// Por tile: indices locales de los triangulos que lo tocan, en orden.
    pub bins: Vec<Vec<u16>>,
    pub counters: TriCounters,
}

/// Un dibujo que paso el culling y su lugar en los buffers del frame.
pub struct VisibleDraw {
    pub draw: usize,
    pub vert_offset: usize,
    pub vert_count: usize,
    pub tri_offset: usize,
    pub tri_count: usize,
}

pub struct ScreenSetup {
    pub width: i32,
    pub height: i32,
    pub tiles_x: usize,
    pub tiles: usize,
    pub near: f32,
}

/// Transforma el rango global `[first, first + out.len())` de vertices.
/// Un rango puede abarcar varios dibujos.
pub fn transform_vertices(
    first: usize,
    out: &mut [ClipVertex],
    visible: &[VisibleDraw],
    draws: &[Draw],
    view: &View,
    uniforms: &Uniforms,
) {
    let end = first + out.len();
    let mut k = visible.partition_point(|v| v.vert_offset + v.vert_count <= first);
    while k < visible.len() && visible[k].vert_offset < end {
        let v = &visible[k];
        let draw = &draws[v.draw];
        let mesh = draw.mesh;
        let deforms = draw.material.deforms();
        let lo = first.max(v.vert_offset);
        let hi = end.min(v.vert_offset + v.vert_count);
        for global in lo..hi {
            let i = global - v.vert_offset;
            let mut position = mesh.positions[i];
            // Deformacion del material (vertex shader): sobre la normal.
            if deforms {
                let lift = shaders::displace(&draw.material, position + draw.local_offset, uniforms);
                position += mesh.normals[i] * lift;
            }
            // Modelo: al mundo relativo a la camara. Vista y proyeccion: a clip.
            let world = draw.model.transform_affine(position);
            // Atmosfera entre la camara y el vertice (aerial perspective).
            let fog = if uniforms.atmospheres.is_empty() {
                Fog::CLEAR
            } else {
                let distance = world.length();
                integrate_all(uniforms.atmospheres, world / distance, distance, uniforms.sun_light, FOG_STEPS)
            };
            out[global - first] = ClipVertex {
                clip: view.view_proj.transform_point(world),
                var: Varyings {
                    pos: world,
                    normal: draw.normal.mul_vec(mesh.normals[i]),
                    local: position + draw.local_offset,
                    uv: mesh.uvs[i],
                    color: mesh.colors[i],
                    fog_scatter: fog.scattered,
                    fog_trans: fog.transmittance,
                },
            };
        }
        k += 1;
    }
}

/// Procesa el bloque `index` de triangulos (de todos los dibujos visibles).
pub fn process_chunk(
    index: usize,
    out: &mut TriChunk,
    draws: &[Draw],
    visible: &[VisibleDraw],
    verts: &[ClipVertex],
    screen: &ScreenSetup,
) {
    out.tris.clear();
    out.extra.clear();
    out.counters = TriCounters::default();
    if out.bins.len() != screen.tiles {
        out.bins.resize_with(screen.tiles, Vec::new);
    }
    for bin in &mut out.bins {
        bin.clear();
    }

    let start = index * TRIS_PER_CHUNK;
    let end = start + TRIS_PER_CHUNK;
    let mut k = visible.partition_point(|v| v.tri_offset + v.tri_count <= start);
    while k < visible.len() && visible[k].tri_offset < end {
        let v = &visible[k];
        let indices = draws[v.draw].indices;
        let lo = start.max(v.tri_offset) - v.tri_offset;
        let hi = end.min(v.tri_offset + v.tri_count) - v.tri_offset;
        let base = v.vert_offset as u32;
        for tri in &indices[lo..hi] {
            let ids = [base + tri[0], base + tri[1], base + tri[2]];
            process_triangle(out, verts, ids, v.draw as u32, screen);
        }
        k += 1;
    }
}

fn process_triangle(
    out: &mut TriChunk,
    verts: &[ClipVertex],
    ids: [u32; 3],
    draw: u32,
    screen: &ScreenSetup,
) {
    out.counters.input += 1;
    let v = [
        &verts[ids[0] as usize],
        &verts[ids[1] as usize],
        &verts[ids[2] as usize],
    ];
    let near = screen.near;

    // Descarte trivial con el frustum real.
    let outcode = |c: &Vec4| {
        u32::from(c.x > c.w)
            | u32::from(c.x < -c.w) << 1
            | u32::from(c.y > c.w) << 2
            | u32::from(c.y < -c.w) << 3
            | u32::from(c.w < near) << 4
    };
    if outcode(&v[0].clip) & outcode(&v[1].clip) & outcode(&v[2].clip) != 0 {
        out.counters.frustum += 1;
        return;
    }

    // Planos contra los que de verdad hay que recortar.
    let clipcode = |c: &Vec4| {
        let g = GUARD_BAND * c.w;
        let mut code = 0;
        if c.w < near {
            code |= CLIP_NEAR;
        }
        if c.x > g {
            code |= CLIP_RIGHT;
        }
        if c.x < -g {
            code |= CLIP_LEFT;
        }
        if c.y > g {
            code |= CLIP_TOP;
        }
        if c.y < -g {
            code |= CLIP_BOTTOM;
        }
        code
    };
    let mut mask = clipcode(&v[0].clip) | clipcode(&v[1].clip) | clipcode(&v[2].clip);
    if mask == 0 {
        setup_triangle(out, v, ids, draw, screen);
        return;
    }

    out.counters.clipped += 1;
    // Un vertice detras de la camara tiene x/y sin sentido: tras recortarlo
    // contra el plano cercano hay que revisar tambien la banda de guarda.
    if mask & CLIP_NEAR != 0 {
        mask |= CLIP_GUARD;
    }
    let mut poly = Polygon::new();
    for vertex in v {
        poly.push(*vertex);
    }
    clip_polygon(&mut poly, mask, near);
    if poly.len < 3 {
        return;
    }
    let base = out.extra.len() as u32;
    out.extra.extend_from_slice(&poly.verts[..poly.len]);
    for k in 1..poly.len - 1 {
        let ids = [
            EXTRA_VERTEX | base,
            EXTRA_VERTEX | (base + k as u32),
            EXTRA_VERTEX | (base + k as u32 + 1),
        ];
        setup_triangle(
            out,
            [&poly.verts[0], &poly.verts[k], &poly.verts[k + 1]],
            ids,
            draw,
            screen,
        );
    }
}

/// Un triangulo recortado por 5 planos tiene a lo sumo 8 vertices.
const MAX_POLY: usize = 9;

struct Polygon {
    verts: [ClipVertex; MAX_POLY],
    len: usize,
}

impl Polygon {
    fn new() -> Self {
        Self {
            verts: [ClipVertex::default(); MAX_POLY],
            len: 0,
        }
    }

    fn push(&mut self, v: ClipVertex) {
        debug_assert!(self.len < MAX_POLY);
        if self.len < MAX_POLY {
            self.verts[self.len] = v;
            self.len += 1;
        }
    }
}

/// Sutherland-Hodgman en espacio de clip: el poligono se recorta plano por
/// plano, conservando lo que queda del lado positivo de cada uno.
fn clip_polygon(poly: &mut Polygon, mask: u32, near: f32) {
    for plane in 0..5 {
        if mask & (1 << plane) == 0 {
            continue;
        }
        let distance = |c: &Vec4| match plane {
            0 => c.w - near,
            1 => GUARD_BAND * c.w - c.x,
            2 => GUARD_BAND * c.w + c.x,
            3 => GUARD_BAND * c.w - c.y,
            _ => GUARD_BAND * c.w + c.y,
        };
        let mut clipped = Polygon::new();
        for i in 0..poly.len {
            let a = &poly.verts[i];
            let b = &poly.verts[(i + 1) % poly.len];
            let da = distance(&a.clip);
            let db = distance(&b.clip);
            if da >= 0.0 {
                clipped.push(*a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                clipped.push(a.lerp(b, da / (da - db)));
            }
        }
        *poly = clipped;
        if poly.len < 3 {
            return;
        }
    }
}

/// Division de perspectiva, viewport, descarte de espaldas, setup y reparto.
fn setup_triangle(
    out: &mut TriChunk,
    v: [&ClipVertex; 3],
    ids: [u32; 3],
    draw: u32,
    s: &ScreenSetup,
) {
    let (w, h) = (s.width as f32, s.height as f32);
    let mut fx = [0i64; 3];
    let mut fy = [0i64; 3];
    let mut z = [0f32; 3];
    let mut inv_w = [0f32; 3];
    for k in 0..3 {
        let c = v[k].clip;
        let iw = 1.0 / c.w;
        inv_w[k] = iw;
        z[k] = c.z * iw;
        // NDC -> pixeles; en pantalla la y crece hacia abajo.
        let sx = (c.x * iw * 0.5 + 0.5) * w;
        let sy = (0.5 - c.y * iw * 0.5) * h;
        fx[k] = (sx * SUBPIXEL as f32).round() as i64;
        fy[k] = (sy * SUBPIXEL as f32).round() as i64;
    }

    // Area con signo (el doble). Las caras de frente son antihorarias en NDC;
    // al voltear la y para la pantalla quedan horarias y el area es negativa.
    let area = (fx[1] - fx[0]) * (fy[2] - fy[0]) - (fy[1] - fy[0]) * (fx[2] - fx[0]);
    if area >= 0 {
        out.counters.backface += 1;
        return;
    }
    // Intercambiar 1 y 2 deja el area positiva: el raster asume ese orden.
    fx.swap(1, 2);
    fy.swap(1, 2);
    z.swap(1, 2);
    inv_w.swap(1, 2);
    let ids = [ids[0], ids[2], ids[1]];

    // Pixeles cuyo centro (x + 0.5) puede caer dentro del triangulo.
    let half = SUBPIXEL / 2;
    let min_x = ((fx.iter().min().unwrap() - half + SUBPIXEL - 1) >> SUBPIXEL_BITS).max(0);
    let max_x = ((fx.iter().max().unwrap() - half) >> SUBPIXEL_BITS).min(s.width as i64 - 1);
    let min_y = ((fy.iter().min().unwrap() - half + SUBPIXEL - 1) >> SUBPIXEL_BITS).max(0);
    let max_y = ((fy.iter().max().unwrap() - half) >> SUBPIXEL_BITS).min(s.height as i64 - 1);
    if min_x > max_x || min_y > max_y {
        out.counters.small += 1;
        return;
    }

    // Baricentricas de pantalla: resolver p - v0 = l1 (v1 - v0) + l2 (v2 - v0).
    // En f64 para no perder precision con triangulos enormes.
    let inv_sub = 1.0 / SUBPIXEL as f64;
    let x0 = fx[0] as f64 * inv_sub;
    let y0 = fy[0] as f64 * inv_sub;
    let (d1x, d1y) = ((fx[1] - fx[0]) as f64 * inv_sub, (fy[1] - fy[0]) as f64 * inv_sub);
    let (d2x, d2y) = ((fx[2] - fx[0]) as f64 * inv_sub, (fy[2] - fy[0]) as f64 * inv_sub);
    let inv_det = 1.0 / (d1x * d2y - d2x * d1y);
    let l1 = [d2y * inv_det, -d2x * inv_det];
    let l2 = [-d1y * inv_det, d1x * inv_det];
    let (dz1, dz2) = ((z[1] - z[0]) as f64, (z[2] - z[0]) as f64);
    let dz = [dz1 * l1[0] + dz2 * l2[0], dz1 * l1[1] + dz2 * l2[1]];

    let local = out.tris.len();
    out.tris.push(TriRecord {
        fx: fx.map(|v| v as i32),
        fy: fy.map(|v| v as i32),
        min_x: min_x as i32,
        min_y: min_y as i32,
        max_x: max_x as i32,
        max_y: max_y as i32,
        ox: x0 as f32,
        oy: y0 as f32,
        l1: [l1[0] as f32, l1[1] as f32],
        l2: [l2[0] as f32, l2[1] as f32],
        z0: z[0],
        dz: [dz[0] as f32, dz[1] as f32],
        inv_w,
        draw,
        verts: ids,
    });

    let tile = TILE as i64;
    for ty in (min_y / tile)..=(max_y / tile) {
        for tx in (min_x / tile)..=(max_x / tile) {
            out.bins[ty as usize * s.tiles_x + tx as usize].push(local as u16);
        }
    }
    out.counters.emitted += 1;
}

#[cfg(test)]
mod tests {
    use super::super::raster::rasterize;
    use super::*;

    fn screen(size: i32) -> ScreenSetup {
        ScreenSetup {
            width: size,
            height: size,
            tiles_x: (size as usize).div_ceil(TILE),
            tiles: (size as usize).div_ceil(TILE).pow(2),
            near: 0.05,
        }
    }

    fn vertex(x: f32, y: f32, w: f32) -> ClipVertex {
        ClipVertex {
            clip: Vec4::new(x * w, y * w, 0.05, w),
            var: Varyings::default(),
        }
    }

    /// Un abanico que cubre toda la pantalla debe pintar cada pixel una vez:
    /// ni huecos entre triangulos vecinos ni pixeles dobles en las aristas.
    #[test]
    fn shared_edges_cover_every_pixel_exactly_once() {
        let size = 70;
        let s = screen(size);
        let center = (0.137, -0.071);
        let ring = [
            (1.0, -1.0),
            (1.0, -0.2),
            (1.0, 1.0),
            (0.3, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.45),
            (-1.0, -1.0),
            (-0.6, -1.0),
        ];
        let mut verts = vec![vertex(center.0, center.1, 1.0)];
        verts.extend(ring.iter().map(|&(x, y)| vertex(x, y, 1.0)));
        let mut out = TriChunk::default();
        out.bins.resize_with(s.tiles, Vec::new);
        for i in 0..ring.len() {
            let next = (i + 1) % ring.len();
            process_triangle(&mut out, &verts, [0, 1 + i as u32, 1 + next as u32], 0, &s);
        }
        assert_eq!(out.counters.emitted as usize, ring.len());

        let mut coverage = vec![0u8; (size * size) as usize];
        for tri in &out.tris {
            rasterize(tri, 0, 0, size - 1, size - 1, |x, y, _| {
                coverage[(y * size + x) as usize] += 1;
            });
        }
        assert!(coverage.iter().all(|&c| c == 1), "hay huecos o pixeles dobles");
    }

    #[test]
    fn back_faces_are_discarded() {
        let s = screen(64);
        let verts = [vertex(-0.5, -0.5, 1.0), vertex(0.5, -0.5, 1.0), vertex(0.0, 0.5, 1.0)];
        let mut out = TriChunk::default();
        out.bins.resize_with(s.tiles, Vec::new);
        process_triangle(&mut out, &verts, [0, 2, 1], 0, &s);
        assert_eq!(out.counters.backface, 1);
        process_triangle(&mut out, &verts, [0, 1, 2], 0, &s);
        assert_eq!(out.counters.emitted, 1);
    }

    #[test]
    fn triangles_crossing_the_near_plane_are_clipped_in_front_of_it() {
        let s = screen(64);
        // Un vertice detras de la camara (w negativo).
        let verts = [
            ClipVertex { clip: Vec4::new(-0.5, -0.5, 0.05, 1.0), var: Varyings::default() },
            ClipVertex { clip: Vec4::new(0.5, -0.5, 0.05, 1.0), var: Varyings::default() },
            ClipVertex { clip: Vec4::new(0.0, 0.5, 0.05, -2.0), var: Varyings::default() },
        ];
        let mut out = TriChunk::default();
        out.bins.resize_with(s.tiles, Vec::new);
        process_triangle(&mut out, &verts, [0, 1, 2], 0, &s);
        assert_eq!(out.counters.clipped, 1);
        assert!(!out.extra.is_empty());
        assert!(out.extra.iter().all(|v| v.clip.w >= s.near - 1e-6));
    }
}
