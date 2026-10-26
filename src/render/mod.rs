//! Pipeline de render por software, todo en CPU.
//!
//! ```text
//! dibujos (malla + matriz + material)
//!  -> culling por objeto: esfera envolvente contra el frustum
//!  -> vertices: objeto -> mundo relativo a la camara -> clip         (paralelo)
//!  -> triangulos: descarte, recorte, setup y reparto en tiles        (paralelo)
//!  -> raster por tile: profundidad + id del triangulo visible        (paralelo)
//!  -> shading diferido: un fragment shader por pixel visible         (paralelo)
//!  -> tonemap HDR -> RGBA8                                            (paralelo)
//! ```
//!
//! # Por que un visibility buffer
//! El raster no ejecuta shaders: solo decide que triangulo se ve en cada pixel
//! y guarda su id junto a la profundidad. Despues cada pixel visible se
//! sombrea **una sola vez**. Con shaders caros (ruido fractal, atmosfera) eso
//! evita pagar el overdraw: un pixel tapado tres veces cuesta tres pruebas de
//! profundidad, no tres shaders.
//!
//! # Por que tiles
//! La pantalla se parte en cuadros de 64x64. Cada tile tiene un solo escritor,
//! asi que los hilos no se pisan y no hacen falta candados ni atomicos en el
//! framebuffer. Los buffers se guardan por tiles (no por filas) para que cada
//! hilo reciba un bloque contiguo de memoria con `chunks_mut`.

mod pipeline;
pub mod png;
mod post;
mod raster;
mod shade;
mod stats;

use std::sync::atomic::AtomicU64;
use std::time::Instant;

use crate::math::{Mat3, Mat4, Vec3};
use crate::parallel;
use crate::scene::Mesh;
use crate::shaders::{Material, Uniforms};

use pipeline::{ClipVertex, ScreenSetup, TriChunk, VisibleDraw, TRIS_PER_CHUNK};
pub use stats::Stats;

/// Lado de un tile en pixeles.
pub const TILE: usize = 64;
pub const TILE_PIXELS: usize = TILE * TILE;
/// Vertices por trabajo de la etapa de vertices.
const VERTS_PER_JOB: usize = 2048;

/// Un objeto a dibujar en este frame.
pub struct Draw<'a> {
    pub mesh: &'a Mesh,
    /// Objeto -> mundo relativo a la camara (rotacion, escala y traslacion).
    pub model: Mat4,
    /// Rotacion de las normales del objeto al mundo (la escala es uniforme).
    pub normal: Mat3,
    pub material: Material,
    /// Esfera envolvente en el mundo relativo a la camara.
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
}

/// Camara del frame. El mundo llega relativo a ella: la camara es el origen.
pub struct View {
    /// Mundo relativo a la camara -> clip: rotacion de la vista y proyeccion.
    pub view_proj: Mat4,
    /// Rotacion mundo -> camara, para el culling por objeto.
    pub world_to_camera: Mat3,
    pub near: f32,
    /// Tangentes de la mitad del campo de vision.
    pub tan_half_x: f32,
    pub tan_half_y: f32,
    /// Ejes de la camara en el mundo, escalados por las tangentes: el pixel
    /// con coordenadas NDC `(u, v)` mira hacia `forward + right*u + up*v`.
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl View {
    /// Prueba conservadora de una esfera contra los cinco planos utiles del
    /// frustum (el lejano esta en el infinito).
    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        let c = self.world_to_camera.mul_vec(center);
        // La camara mira hacia -Z: lo visible esta en z <= -near.
        if c.z - radius > -self.near {
            return false;
        }
        let norm_x = (1.0 + self.tan_half_x * self.tan_half_x).sqrt();
        let norm_y = (1.0 + self.tan_half_y * self.tan_half_y).sqrt();
        let side_x = c.z * self.tan_half_x;
        let side_y = c.z * self.tan_half_y;
        (c.x + side_x) / norm_x <= radius
            && (-c.x + side_x) / norm_x <= radius
            && (c.y + side_y) / norm_y <= radius
            && (-c.y + side_y) / norm_y <= radius
    }

    /// Direccion en el mundo que ve el punto de pantalla `(x, y)` en pixeles.
    #[inline]
    pub fn pixel_direction(&self, x: f32, y: f32, inv_width: f32, inv_height: f32) -> Vec3 {
        let u = x * inv_width * 2.0 - 1.0;
        let v = 1.0 - y * inv_height * 2.0;
        (self.forward + self.right * u + self.up * v).normalize()
    }
}

/// Un pixel del visibility buffer.
#[derive(Clone, Copy, Debug)]
pub struct VisPixel {
    /// `near / w`: 1 en el plano cercano, 0 en el infinito. Gana el mayor.
    pub depth: f32,
    /// Bloque de triangulos en los 16 bits altos, indice local en los bajos.
    pub id: u32,
}

impl VisPixel {
    pub const EMPTY_ID: u32 = u32::MAX;
    pub const EMPTY: Self = Self {
        depth: 0.0,
        id: Self::EMPTY_ID,
    };
}

pub struct Renderer {
    width: usize,
    height: usize,
    tiles_x: usize,
    tiles_y: usize,
    /// Visibility buffer guardado por tiles.
    vbuf: Vec<VisPixel>,
    /// Color HDR lineal guardado por tiles.
    hdr: Vec<Vec3>,
    /// Imagen final por filas, RGBA8, lista para subir a la textura.
    rgba: Vec<u8>,
    visible: Vec<VisibleDraw>,
    verts: Vec<ClipVertex>,
    chunks: Vec<TriChunk>,
    pub exposure: f32,
}

impl Renderer {
    pub fn new(width: usize, height: usize) -> Self {
        let tiles_x = width.div_ceil(TILE);
        let tiles_y = height.div_ceil(TILE);
        let tiles = tiles_x * tiles_y;
        Self {
            width,
            height,
            tiles_x,
            tiles_y,
            vbuf: vec![VisPixel::EMPTY; tiles * TILE_PIXELS],
            hdr: vec![Vec3::ZERO; tiles * TILE_PIXELS],
            rgba: vec![0; width * height * 4],
            visible: Vec::new(),
            verts: Vec::new(),
            chunks: Vec::new(),
            exposure: crate::config::EXPOSURE,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// La imagen del ultimo frame, RGBA8 por filas.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn render(&mut self, draws: &[Draw], view: &View, uniforms: &Uniforms) -> Stats {
        let start = Instant::now();
        let mut stats = Stats::default();

        // 1. Culling por objeto y reparto del espacio de vertices/triangulos.
        self.visible.clear();
        let (mut vert_total, mut tri_total) = (0, 0);
        for (index, draw) in draws.iter().enumerate() {
            stats.objects += 1;
            if !view.sphere_visible(draw.bounds_center, draw.bounds_radius) {
                stats.objects_culled += 1;
                continue;
            }
            let vert_count = draw.mesh.positions.len();
            let tri_count = draw.mesh.indices.len();
            self.visible.push(VisibleDraw {
                draw: index,
                vert_offset: vert_total,
                vert_count,
                tri_offset: tri_total,
                tri_count,
            });
            vert_total += vert_count;
            tri_total += tri_count;
        }
        stats.vertices = vert_total as u32;

        // 2. Vertices.
        self.verts.resize(vert_total, ClipVertex::default());
        let visible = &self.visible;
        parallel::chunks_mut(&mut self.verts, VERTS_PER_JOB, VERTS_PER_JOB, |job, out| {
            pipeline::transform_vertices(job * VERTS_PER_JOB, out, visible, draws, view);
        });
        let t_vertex = Instant::now();

        // 3. Triangulos: descarte, recorte, setup y reparto en tiles.
        let n_chunks = tri_total.div_ceil(TRIS_PER_CHUNK);
        if self.chunks.len() < n_chunks {
            self.chunks.resize_with(n_chunks, TriChunk::default);
        }
        let screen = ScreenSetup {
            width: self.width as i32,
            height: self.height as i32,
            tiles_x: self.tiles_x,
            tiles: self.tiles_x * self.tiles_y,
            near: view.near,
        };
        let verts = &self.verts;
        parallel::chunks_mut(&mut self.chunks[..n_chunks], 1, 1, |index, slot| {
            pipeline::process_chunk(index, &mut slot[0], draws, visible, verts, &screen);
        });
        for chunk in &self.chunks[..n_chunks] {
            stats.triangles.accumulate(&chunk.counters);
        }
        let t_triangles = Instant::now();

        // 4. Raster por tile al visibility buffer.
        let chunks = &self.chunks[..n_chunks];
        let (tiles_x, width, height) = (self.tiles_x, self.width, self.height);
        parallel::chunks_mut(&mut self.vbuf, TILE_PIXELS, TILE_PIXELS, |tile, pixels| {
            raster::raster_tile(tile, pixels, chunks, tiles_x, width as i32, height as i32);
        });
        let t_raster = Instant::now();

        // 5. Shading diferido.
        let shaded = AtomicU64::new(0);
        let context = shade::ShadeContext {
            vbuf: &self.vbuf,
            chunks,
            verts,
            draws,
            view,
            uniforms,
            width,
            height,
            tiles_x,
            shaded: &shaded,
        };
        parallel::chunks_mut(&mut self.hdr, TILE_PIXELS, TILE_PIXELS, |tile, out| {
            shade::shade_tile(tile, out, &context);
        });
        stats.pixels_shaded = shaded.into_inner();
        let t_shade = Instant::now();

        // 6. Tonemap a RGBA8, por bandas de una fila de tiles.
        let hdr = &self.hdr;
        let band = width * TILE * 4;
        let exposure = self.exposure;
        parallel::chunks_mut(&mut self.rgba, band, band, |index, out| {
            post::tonemap_band(index, out, hdr, width, tiles_x, exposure);
        });
        let t_post = Instant::now();

        let ms = |a: Instant, b: Instant| (b - a).as_secs_f32() * 1000.0;
        stats.ms_vertex = ms(start, t_vertex);
        stats.ms_triangles = ms(t_vertex, t_triangles);
        stats.ms_raster = ms(t_triangles, t_raster);
        stats.ms_shade = ms(t_raster, t_shade);
        stats.ms_post = ms(t_shade, t_post);
        stats.ms_total = ms(start, t_post);
        stats
    }
}
