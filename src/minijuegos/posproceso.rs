//! El posproceso de la carrera: lo que se hace con la imagen entera despues
//! de pintar las filas. Todo en CPU, por filas en paralelo.
//!
//! - **Emision**: durante la pasada de filas cada pixel guarda cuanto emite
//!   en el byte del alfa (`A = 255 - e * 255`, ver `lienzo::write_emit`).
//!   Lo opaco que se pinta encima la borra, asi una nave tapa la baranda.
//! - **Extraccion**: cada texel del nivel 0 del bloom (a un cuarto de la
//!   pantalla, 320 x 180 a 720p) promedia su bloque de 4 x 4 pixeles de
//!   `s^2 e 2`, con `s = l + 1.6 (c - l)` el color con la croma subida
//!   (`l` su luma). Las sumas de cada fila las deja la misma pasada que la
//!   pinta (`RowSums`), con la fila todavia en cache. Elevar al cuadrado pasa el color de pantalla (gamma ~2)
//!   a luz mas o menos lineal, que es la que se dispersa. Lo que emite
//!   suele llegar recortado cerca del blanco (un neon magenta a fondo sale
//!   rosa palido en 8 bits): subirle la croma antes devuelve al brillo el
//!   tono de la luz y no un velo blanco. Despues del blur se vuelve con una
//!   raiz con rodilla, `r / sqrt(r + 0.05)`, y una compresion suave,
//!   `1.25 g / (1 + 0.8 g)`: decenas de focos juntos (la camioneta) no
//!   suman blanco, cada uno deja su color.
//! - **Piramide**: la del juego principal (`render::bloom`): cada nivel a la
//!   mitad, cada uno con su gaussiano y combinados de chico a grande. Los
//!   niveles finos dan el nucleo pegado a la luz y los gruesos la cola ancha
//!   que se derrama sobre el techo y el cielo: un brillo de dos escalas.
//! - **Compuesto**: primero la vineta sobre la escena (atenuada donde algo
//!   emite: las luces de la orilla siguen calientes), despues el brillo en
//!   modo pantalla suave, `c + g (1 - c)`, y se gradua: sombras al indigo,
//!   altas al sodio, una curva S por canal `c + 0.12 c (1 - c)(2c - 1)`, un
//!   poco mas de saturacion y grano (que tambien rompe las bandas de 8 bits
//!   del cielo oscuro). Con velocidad salen lineas que corren hacia la
//!   orilla; con el turbo son mas y la aberracion cromatica separa el rojo
//!   del azul en la orilla. Y el humo, cuando se pasa por el de la
//!   camioneta. Al final de cada fila va el gancho del HUD, sobre la imagen
//!   ya graduada y sin vineta.
//! - **Por canal**: cada fila se pasa a tres arreglos (`r`, `g`, `b`) y se
//!   trabaja por pasadas sin ramas, que el compilador vectoriza con AVX2.

use std::cell::RefCell;
use std::f32::consts::TAU;

use super::lienzo;
use crate::math::{Vec3, hash_u32, smoothstep};
use crate::parallel;
use crate::render::bloom::Bloom;

/// Lado de la textura de ruido (potencia de dos: el wrap es un `&`).
const NOISE: usize = 256;
/// Lado de la tabla del grano (potencia de dos).
const GRAIN: usize = 256;
/// Celdas por lado de la primera octava del ruido.
const NOISE_CELLS: u32 = 4;
/// Cuanto se suma el brillo (con 1.3 la camioneta, que lleva decenas de
/// focos juntos, se lavaba entera).
const GAIN: f32 = 1.0;
/// La caida del peso de cada nivel de la piramide (en el juego principal es
/// 0.75): con 0.65 el nucleo pesa mas que la cola ancha, pero la cola se
/// alcanza a derramar sobre el cielo (con la compresion suave ya no vela
/// la pantalla cuando hay muchas luces).
const FALLOFF: f32 = 0.65;
/// Cuanto se sube la croma de lo que emite antes de extraerlo.
const CHROMA: f32 = 1.6;
/// La compresion suave del brillo: `COMP_GAIN g / (1 + COMP_KNEE g)`. Casi
/// lineal en las colas tenues (pendiente 1.25 en 0) y con techo en
/// `1.25 / 0.8 = 1.56` donde se juntan muchas luces.
const COMP_GAIN: f32 = 1.25;
const COMP_KNEE: f32 = 0.8;
/// Una fila del brillo cuyo maximo no llega a esto no suma nada visible
/// (medio paso de 8 bits): el compuesto se la salta.
const GLOW_FLOOR: f32 = 1.0 / 512.0;
/// La aberracion del turbo: el corrimiento maximo en la orilla (px) y
/// cuanto del canal corrido se mezcla (un velo, no una franja pura).
const ABERRATION_PX: f32 = 2.5;
const ABERRATION_MIX: f32 = 0.6;
/// La rodilla de la vuelta a pantalla: `g = r / sqrt(r + KNEE)`. Arriba de
/// la rodilla es casi la raiz (`r` estaba al cuadrado); abajo es lineal, y
/// la cola tenue de muchas luces no levanta un velo gris sobre el asfalto
/// (la raiz pura la subiria: `sqrt(0.003) = 0.055`, contra 0.013).
const KNEE: f32 = 0.05;
/// Pesos de luminancia Rec. 709.
const LUMA: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);
/// Las lineas de velocidad: cuantas en la vuelta y su color.
const SPOKES: u32 = 300;
const SPOKE_COLOR: Vec3 = Vec3::new(0.6, 0.85, 1.0);
/// Cuanto vive cada linea de velocidad (s): tres doceavos de segundo.
const LIFE: f32 = 0.25;

/// Lo del frame que el compuesto necesita.
#[derive(Clone, Copy, Debug, Default)]
pub struct Params {
    /// El turbo suavizado (0 a 1) y la velocidad sobre la maxima.
    pub boost_s: f32,
    pub speed: f32,
    /// El humo en la pantalla (0 a 1) y el reloj de pared (s).
    pub smoke: f32,
    pub wall: f32,
    /// Brillo extra de un momento (el semaforo, la meta).
    pub bloom_pulse: f32,
    /// El cierre: cuanto se oscurece y se desatura la imagen (0 a 1).
    pub fade: f32,
    pub desat: f32,
}

pub struct Posproceso {
    bloom: Bloom,
    /// Las sumas de la extraccion por fila de la pantalla (ver `RowSums`),
    /// `height x bw`, y el bloque de pixeles de cada texel del nivel 0.
    sums: Vec<Vec3>,
    bw: usize,
    sx: usize,
    sy: usize,
    /// El brillo ya en espacio de pantalla (la raiz del resultado).
    glow: Vec<Vec3>,
    /// El maximo de cada fila del brillo (para saltarse las que no suman).
    glow_max: Vec<f32>,
    /// La opacidad del humo en la misma rejilla del brillo (es una senal
    /// de baja frecuencia: 16 veces menos muestras del ruido que por
    /// pixel) y si hay humo en este frame.
    smoke: Vec<f32>,
    smoky: bool,
    gw: usize,
    gh: usize,
    /// Ruido de valor periodico de cuatro octavas, `NOISE x NOISE`.
    noise: Vec<u8>,
    /// Ruido blanco para el grano, `GRAIN x GRAIN`: cada fila de la
    /// pantalla toma una fila al azar, corrida al azar (un hash por fila y
    /// no uno por pixel).
    grain: Vec<u8>,
    frame: u32,
    width: usize,
    height: usize,
    /// Por columna: de que dos texeles del brillo lee y con que peso, y su
    /// coordenada normalizada `nx` en `[-1, 1]` (y su cuadrado).
    g0: Vec<u32>,
    g1: Vec<u32>,
    gt: Vec<f32>,
    nx: Vec<f32>,
    nx2: Vec<f32>,
}

/// Las sumas de la extraccion de cada fila de la pantalla: por cada bloque
/// de `sx` pixeles (los de un texel del nivel 0), `sum(s^2 e)` de los que
/// emiten. Las llena la misma pasada que pinta las filas, al terminar cada
/// una, mientras la fila sigue en cache: asi la extraccion no vuelve a leer
/// el frame entero (3.7 MB a 720p), solo suma `sy` filas de sumas por texel.
pub struct RowSums {
    cells: Vec<Vec3>,
    bw: usize,
    sx: usize,
    width: usize,
    height: usize,
}

impl RowSums {
    /// Pinta las filas de `rgba` en paralelo con `paint(y, fila)` y despues
    /// suma la emision de cada una.
    pub fn paint(&mut self, rgba: &mut [u8], paint: impl Fn(usize, &mut [u8]) + Sync) {
        let (w, h, bw, sx) = (self.width, self.height, self.bw, self.sx);
        let cells = parallel::DisjointMut::new(&mut self.cells);
        parallel::chunks_mut(rgba, w * 4, 1, |y, row| {
            paint(y, row);
            if y < h && row.len() == w * 4 {
                // SAFETY: cada fila escribe solo su propio tramo de sumas,
                // `[y bw, (y + 1) bw)`, y `chunks_mut` da cada `y` una vez.
                let out = unsafe { cells.range(y * bw, bw) };
                sum_row(row, out, sx);
            }
        });
    }
}

/// Las sumas de una fila: `sum(s^2 e)` por bloque de `sx` pixeles, con
/// `s = max(l + CHROMA (c - l), 0)` (la croma subida, ver arriba). Lo que
/// no emite (`A = 255`, casi todo) solo cuesta la comparacion.
fn sum_row(row: &[u8], out: &mut [Vec3], sx: usize) {
    out.fill(Vec3::ZERO);
    for (o, block) in out.iter_mut().zip(row.chunks(sx * 4)) {
        let mut sum = Vec3::ZERO;
        for p in block.chunks_exact(4) {
            if p[3] != 255 {
                let c = lienzo::read(p);
                let l = c.dot(LUMA);
                let s = (Vec3::splat(l) + (c - Vec3::splat(l)) * CHROMA).max(Vec3::ZERO);
                sum += s * s * lienzo::emission(p);
            }
        }
        *o = sum;
    }
}

/// Lo de una fila del compuesto: la fila del brillo (interpolada en
/// vertical) y los de la imagen, por canal.
#[derive(Default)]
struct Scratch {
    lr: Vec<f32>,
    lg: Vec<f32>,
    lb: Vec<f32>,
    /// La fila del brillo ampliada al ancho de la pantalla.
    ur: Vec<f32>,
    ug: Vec<f32>,
    ub: Vec<f32>,
    r: Vec<f32>,
    g: Vec<f32>,
    b: Vec<f32>,
    /// El corrimiento de la aberracion por columna (px, fraccionario).
    shift: Vec<f32>,
    /// Copias de los canales rojo y azul con `TAPS` pixeles repetidos a
    /// cada lado (las lee la aberracion).
    pr: Vec<f32>,
    pb: Vec<f32>,
    /// La fila del humo: interpolada en vertical y ampliada.
    ls: Vec<f32>,
    us: Vec<f32>,
    /// El grano de la fila.
    grain: Vec<u8>,
    /// La fila ya armada, un `u32` por pixel.
    packed: Vec<u32>,
}

thread_local! {
    /// Reutilizado entre filas y frames.
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

impl Posproceso {
    pub fn new(width: usize, height: usize) -> Self {
        let (width, height) = (width.max(4), height.max(4));
        // `Bloom::new` arma su nivel 0 a la mitad de lo que recibe: con la
        // mitad de la pantalla queda a un cuarto.
        // Los niveles chicos de la piramide tambien en paralelo (desde 2048
        // texeles por hilo): en serie costaban un tercio de la piramide.
        let mut bloom = Bloom::with_falloff(width / 2, height / 2, FALLOFF).with_parallel_min(2048);
        let (gw, gh) = (bloom.result().width, bloom.result().height);
        let (bw, bh) = (bloom.base_mut().width, bloom.base_mut().height);
        // Cuantos pixeles de la pantalla caen en cada texel del nivel 0.
        let (sx, sy) = ((width / bw).max(1), (height / bh).max(1));
        let scale = gw as f32 / width as f32;
        let cx = width as f32 * 0.5;
        let fx: Vec<f32> = (0..width).map(|x| ((x as f32 + 0.5) * scale - 0.5).max(0.0)).collect();
        let g0: Vec<u32> = fx.iter().map(|&f| (f as usize).min(gw - 1) as u32).collect();
        let g1 = g0.iter().map(|&i| (i + 1).min(gw as u32 - 1)).collect();
        let gt = fx.iter().zip(&g0).map(|(&f, &i)| (f - i as f32).clamp(0.0, 1.0)).collect();
        let nx: Vec<f32> = (0..width).map(|x| (x as f32 + 0.5 - cx) / cx).collect();
        let nx2 = nx.iter().map(|v| v * v).collect();
        let grain = (0..GRAIN * GRAIN).map(|i| (hash_u32(i as u32 ^ 0x6A41_7E5D) >> 24) as u8).collect();
        Self {
            bloom,
            sums: vec![Vec3::ZERO; height * bw],
            bw,
            sx,
            sy,
            glow: vec![Vec3::ZERO; gw * gh],
            glow_max: vec![0.0; gh],
            smoke: vec![0.0; gw * gh],
            smoky: false,
            gw,
            gh,
            noise: bake_noise(),
            grain,
            frame: 0,
            width,
            height,
            g0,
            g1,
            gt,
            nx,
            nx2,
        }
    }

    /// El ruido en `(u, v)` medido en cuadros de la textura (se repite cada
    /// 1), bilineal y de 0 a 1.
    #[inline]
    pub fn tex(&self, u: f32, v: f32) -> f32 {
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        let (x, y) = (finite(u * NOISE as f32), finite(v * NOISE as f32));
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let m = NOISE as i32 - 1;
        let (x0, y0) = (x0 as i32, y0 as i32);
        let at = |i: i32, j: i32| self.noise[((j & m) as usize) * NOISE + (i & m) as usize] as f32;
        let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * tx;
        let bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * tx;
        (top + (bottom - top) * ty) * (1.0 / 255.0)
    }

    /// Presta las sumas por fila para la pasada de pintado (se devuelven
    /// en `prepare`).
    pub fn take_sums(&mut self) -> RowSums {
        let mut cells = std::mem::take(&mut self.sums);
        cells.resize(self.height * self.bw, Vec3::ZERO);
        RowSums { cells, bw: self.bw, sx: self.sx, width: self.width, height: self.height }
    }

    /// `prepare` sobre un frame ya pintado (las sumas se sacan aparte).
    #[cfg(test)]
    fn prepare_frame(&mut self, rgba: &mut [u8], params: &Params) {
        let mut sums = self.take_sums();
        sums.paint(rgba, |_, _| {});
        self.prepare(sums, params);
    }

    /// La primera mitad, con las sumas que dejo la pasada de pintado:
    /// extraccion y piramide. El brillo queda en espacio de pantalla, la
    /// raiz (con rodilla) del resultado y la compresion suave: 58 mil
    /// raices por frame en vez de una por pixel. Si hay humo, su opacidad
    /// en la misma rejilla.
    pub fn prepare(&mut self, sums: RowSums, params: &Params) {
        self.frame = self.frame.wrapping_add(1);
        self.extract(&sums);
        self.sums = sums.cells;
        self.bloom.process();
        let result = self.bloom.result();
        let (gw, gh) = (self.gw, self.gh);
        parallel::chunks_mut(&mut self.glow, gw, gw * 8, |y, row| {
            let curve = |v: f32| {
                let g = v.max(0.0) / (v.max(0.0) + KNEE).sqrt();
                COMP_GAIN * g / (1.0 + COMP_KNEE * g)
            };
            for (g, r) in row.iter_mut().zip(&result.pixels[y * gw..(y + 1) * gw]) {
                *g = Vec3::new(curve(r.x), curve(r.y), curve(r.z));
            }
        });
        // El maximo de cada fila (en serie: son 58 mil comparaciones).
        for (top, row) in self.glow_max.iter_mut().zip(self.glow.chunks_exact(gw)) {
            *top = row.iter().fold(0.0f32, |m, g| m.max(g.x).max(g.y).max(g.z));
        }
        // El humo negro delante de la camara: dos capas del ruido que corren
        // en sentidos distintos. La primera a `(0.075, -0.125)` cuadros por
        // segundo, que con su cuadro de 400 px son 30 y 50 px/s; la segunda
        // a `(-0.05, 0.1)` con cuadros de ~167 px, unos 8 y 17 px/s. Las
        // volutas quedan de unos 100 px.
        let smoke = params.smoke.clamp(0.0, 1.0);
        self.smoky = smoke > 0.0;
        if self.smoky {
            let (sx, sy) = (self.width as f32 / gw as f32, self.height as f32 / gh as f32);
            let t = params.wall;
            let mut alpha = std::mem::take(&mut self.smoke);
            let this = &*self;
            parallel::chunks_mut(&mut alpha, gw, gw * 8, |y, row| {
                let yf = (y as f32 + 0.5) * sy;
                for (x, a) in row.iter_mut().enumerate() {
                    let xf = (x as f32 + 0.5) * sx;
                    let n = 0.6 * this.tex(xf * 0.0025 + 0.075 * t, yf * 0.0025 - 0.125 * t) + 0.4 * this.tex(xf * 0.006 - 0.05 * t, yf * 0.006 + 0.1 * t);
                    *a = smoke * (0.55 + 0.35 * n);
                }
            });
            self.smoke = alpha;
        }
    }

    /// La segunda mitad: el compuesto por filas en paralelo. Deja todo
    /// opaco (`A = 255`). `hud(y, fila)` pinta lo que va encima de cada
    /// fila al final, sobre la imagen ya graduada.
    pub fn compose(&self, rgba: &mut [u8], params: &Params, hud: impl Fn(usize, &mut [u8]) + Sync) {
        if rgba.len() != self.width * self.height * 4 {
            return;
        }
        let spokes = Spokes::new(params, self.width, self.height);
        parallel::chunks_mut(rgba, self.width * 4, 1, |y, row| {
            self.compose_row(params, &spokes, y, row);
            hud(y, row);
        });
    }

    /// Cada texel del nivel 0 promedia su bloque de `sx x sy` pixeles de
    /// `s^2 e 2`: suma las `sy` filas de sumas que dejo la pasada de
    /// pintado (cada una ya trae sus `sx` pixeles).
    fn extract(&mut self, sums: &RowSums) {
        let (h, bw, sy) = (self.height, self.bw, self.sy);
        let norm = 2.0 / (self.sx * sy) as f32;
        let cells = &sums.cells;
        if cells.len() < h * bw {
            return;
        }
        let base = self.bloom.base_mut();
        let width = base.width.min(bw);
        parallel::chunks_mut(&mut base.pixels, base.width, base.width * 16, |by, out| {
            out.fill(Vec3::ZERO);
            for dy in 0..sy {
                let y = (by * sy + dy).min(h - 1);
                for (o, c) in out[..width].iter_mut().zip(&cells[y * bw..y * bw + width]) {
                    *o += *c;
                }
            }
            for o in out.iter_mut() {
                *o *= norm;
            }
        });
    }

    /// Amplia una fila del brillo (`gw` texeles) al ancho de la pantalla,
    /// bilineal. Si la pantalla mide justo cuatro texeles por uno (720p,
    /// 1080p), cada texel `j` da cuatro pixeles con pesos fijos: el centro
    /// del pixel `4j + k` cae en `j - 0.375 + 0.25 k`. Si no, con las
    /// columnas precalculadas.
    fn widen(&self, line: &[f32], out: &mut [f32]) {
        let gw = line.len();
        if out.len() == 4 * gw && gw > 0 {
            for (j, quad) in out.chunks_exact_mut(4).enumerate() {
                let (a, b, c) = (line[j.saturating_sub(1)], line[j], line[(j + 1).min(gw - 1)]);
                quad[0] = a + (b - a) * 0.625;
                quad[1] = a + (b - a) * 0.875;
                quad[2] = b + (c - b) * 0.125;
                quad[3] = b + (c - b) * 0.375;
            }
        } else {
            for (x, o) in out.iter_mut().enumerate() {
                let (a, b) = (line[(self.g0[x] as usize).min(gw - 1)], line[(self.g1[x] as usize).min(gw - 1)]);
                *o = a + (b - a) * self.gt[x];
            }
        }
    }

    /// Una fila del compuesto. Va por pasadas sobre arreglos por canal
    /// (`r`, `g`, `b`): la del acabado, que es casi todo el costo, no tiene
    /// ramas ni lecturas cruzadas y el compilador la vectoriza con AVX2 (8
    /// pixeles por instruccion). Lo que solo pasa a veces (aberracion, humo
    /// y lineas) va en pasadas aparte que no cuestan nada cuando no estan.
    fn compose_row(&self, p: &Params, spokes: &Spokes, y: usize, row: &mut [u8]) {
        let (w, h) = (self.width, self.height);
        let (gw, gh) = (self.gw, self.gh);
        let row = &mut row[..w * 4];
        // La fila del brillo, interpolada en vertical una vez por fila.
        let fy = ((y as f32 + 0.5) / h as f32 * gh as f32 - 0.5).max(0.0);
        let y0 = (fy as usize).min(gh - 1);
        let y1 = (y0 + 1).min(gh - 1);
        let ty = fy - y0 as f32;
        let lit = self.glow_max[y0].max(self.glow_max[y1]) >= GLOW_FLOOR;
        let ny = (y as f32 + 0.5 - h as f32 * 0.5) / (h as f32 * 0.5);
        let ny2 = ny * ny;
        let vy2 = ny2 * 1.15 * 1.15;
        let gain = GAIN * (1.0 + p.bloom_pulse);
        let sat = 1.12 - p.desat.clamp(0.0, 1.12);
        let fade = 1.0 - p.fade.clamp(0.0, 1.0);
        let boost = p.boost_s.clamp(0.0, 1.0);
        let (nx, nx2) = (&self.nx[..w], &self.nx2[..w]);
        // El grano de esta fila: una fila al azar de la tabla, corrida al
        // azar, distinta en cada frame.
        let pick = hash_u32((y as u32).wrapping_mul(0x85EB_CA77) ^ self.frame.wrapping_mul(0xC2B2_AE35));
        let grain_row = &self.grain[(pick as usize & (GRAIN - 1)) * GRAIN..][..GRAIN];
        let grain_shift = (pick >> 16) as usize & (GRAIN - 1);
        SCRATCH.with_borrow_mut(|s| {
            for v in [&mut s.r, &mut s.g, &mut s.b, &mut s.ur, &mut s.ug, &mut s.ub, &mut s.shift] {
                v.resize(w, 0.0);
            }
            s.packed.resize(w, 0);
            s.grain.clear();
            while s.grain.len() < w {
                let from = if s.grain.is_empty() { grain_shift } else { 0 };
                let take = (GRAIN - from).min(w - s.grain.len());
                s.grain.extend_from_slice(&grain_row[from..from + take]);
            }
            let noise = &s.grain[..w];
            let (r, g, b) = (&mut s.r[..w], &mut s.g[..w], &mut s.b[..w]);
            // 1) Leer la fila con la vineta ya puesta, antes del brillo:
            // `v = 1 - 0.25 smoothstep(0.35, 1.1, hypot(nx, 1.15 ny))` (con
            // el reciproco del ancho: sin division), y donde algo emite se
            // atenua, `v + (1 - v) e`: los faroles y el neon de la orilla
            // no se apagan con las esquinas.
            for (x, px) in row.chunks_exact(4).enumerate() {
                let q = (((nx2[x] + vy2).sqrt() - 0.35) * (1.0 / 0.75)).clamp(0.0, 1.0);
                let v = 1.0 - 0.25 * q * q * (3.0 - 2.0 * q);
                let e = (255 - px[3]) as f32 * (1.0 / 255.0);
                let k = (v + (1.0 - v) * e) * (1.0 / 255.0);
                r[x] = px[0] as f32 * k;
                g[x] = px[1] as f32 * k;
                b[x] = px[2] as f32 * k;
            }
            // 2) Con turbo, la aberracion: el rojo se corre hacia afuera y
            // el azul hacia adentro `s = 2.5 turbo smoothstep(0.5, 1, r) nx`
            // pixeles (el centro y las naves quedan limpios). El corrimiento
            // es fraccionario, interpolado entre dos muestras (crece suave
            // con el turbo, sin saltar de pixel en pixel), y se mezcla solo
            // el 60 % del canal corrido: un velo de color, no una franja.
            // Solo en las columnas de la corona `r > 0.5`.
            if boost > 0.01 {
                // Las columnas con `nx^2 <= 0.25 - ny^2` no se tocan: un
                // tramo `[xa, xb)` al centro (vacio si `ny^2 >= 0.25`).
                let inner = (0.25 - ny2).max(0.0).sqrt();
                let xa = nx.partition_point(|&v| v < -inner);
                let xb = nx.partition_point(|&v| v <= inner).max(xa);
                let amp = ABERRATION_PX * boost;
                let shift = &mut s.shift[..w];
                for x in 0..w {
                    let t = (((nx2[x] + ny2).sqrt() - 0.5) * 2.0).clamp(0.0, 1.0);
                    shift[x] = amp * t * t * (3.0 - 2.0 * t) * nx[x];
                }
                pad_copy(r, &mut s.pr);
                pad_copy(b, &mut s.pb);
                for (lo, hi) in [(0, xa), (xb, w)] {
                    if hi > lo {
                        shift_blend(&s.pr, &shift[lo..hi], -1.0, lo, &mut r[lo..hi]);
                        shift_blend(&s.pb, &shift[lo..hi], 1.0, lo, &mut b[lo..hi]);
                    }
                }
            }
            // 3) El brillo, en modo pantalla suave: `c + g (1 - c) gain`,
            // con `g` bilineal entre los texeles de la fila. Primero se
            // amplia la fila al ancho de la pantalla y despues se suma (un
            // ciclo sin lecturas cruzadas, que se vectoriza). Las filas del
            // brillo que no llegan a medio paso de 8 bits se saltan.
            if lit {
                for v in [&mut s.lr, &mut s.lg, &mut s.lb] {
                    v.resize(gw, 0.0);
                }
                for (i, (a, b)) in self.glow[y0 * gw..(y0 + 1) * gw].iter().zip(&self.glow[y1 * gw..(y1 + 1) * gw]).enumerate() {
                    let c = a.lerp(*b, ty);
                    (s.lr[i], s.lg[i], s.lb[i]) = (c.x, c.y, c.z);
                }
                let (ur, ug, ub) = (&mut s.ur[..w], &mut s.ug[..w], &mut s.ub[..w]);
                for (src, dst) in [(&s.lr, &mut *ur), (&s.lg, &mut *ug), (&s.lb, &mut *ub)] {
                    self.widen(&src[..gw], dst);
                }
                for x in 0..w {
                    r[x] += ur[x] * (1.0 - r[x].min(1.0)) * gain;
                    g[x] += ug[x] * (1.0 - g[x].min(1.0)) * gain;
                    b[x] += ub[x] * (1.0 - b[x].min(1.0)) * gain;
                }
            }
            // 4) El humo negro delante de la camara, con la opacidad de la
            // rejilla del brillo (ver `prepare`) ampliada como un canal mas.
            if self.smoky {
                s.ls.resize(gw, 0.0);
                for (i, (a, b)) in self.smoke[y0 * gw..(y0 + 1) * gw].iter().zip(&self.smoke[y1 * gw..(y1 + 1) * gw]).enumerate() {
                    s.ls[i] = a + (b - a) * ty;
                }
                s.us.resize(w, 0.0);
                self.widen(&s.ls[..gw], &mut s.us[..w]);
                for (x, &a) in s.us[..w].iter().enumerate() {
                    r[x] += (0.02 - r[x]) * a;
                    g[x] += (0.018 - g[x]) * a;
                    b[x] += (0.016 - b[x]) * a;
                }
            }
            // 5) Las lineas de velocidad en la corona `r > 0.45` (antes del
            // acabado: se gradua con todo).
            spokes.row(y as f32 + 0.5 - h as f32 * 0.5, w as f32 * 0.5, r, g, b);
            // 6) El acabado, en 0 a 255.
            let k = 255.0 * fade;
            for x in 0..w {
                let l = (0.2126 * r[x] + 0.7152 * g[x] + 0.0722 * b[x]).clamp(0.0, 1.0);
                // Sombras al indigo, `(0.03, 0.01, 0.07) (1 - l)^2 0.6`, y
                // altas al sodio, `(0.04, 0.02, -0.02) l^2`.
                let (dark, high) = ((1.0 - l) * (1.0 - l) * 0.6, l * l);
                let cr = (r[x] + 0.03 * dark + 0.04 * high).clamp(0.0, 1.0);
                let cg = (g[x] + 0.01 * dark + 0.02 * high).clamp(0.0, 1.0);
                let cb = (b[x] + 0.07 * dark - 0.02 * high).clamp(0.0, 1.0);
                // Curva S por canal, fija en 0, 0.5 y 1:
                // `c + 0.12 c (1 - c)(2c - 1)`. Despues, la saturacion.
                let curve = |c: f32| l + (c + 0.12 * c * (1.0 - c) * (2.0 * c - 1.0) - l) * sat;
                // Grano: `(n - 0.5) 0.025 (1 - 0.7 l)`, con `n` de la tabla.
                let grain = (noise[x] as f32 * (1.0 / 255.0) - 0.5) * 0.025 * (1.0 - 0.7 * l);
                let bias = grain * 255.0;
                r[x] = curve(cr) * k + bias;
                g[x] = curve(cg) * k + bias;
                b[x] = curve(cb) * k + bias;
            }
            // 7) Escribir, opaco. Primero cada pixel se arma como un `u32`
            // en un ciclo sin conversiones de punto flotante a entero (que
            // en Rust saturan y no se vectorizan bien): sumarle `2^23` a un
            // valor de 0 a 255 lo deja redondeado al entero mas cercano en
            // los bits bajos de la mantisa, y se toman con una mascara.
            // Despues se copia a la fila.
            let packed = &mut s.packed[..w];
            // (`max` y `min` antes que `clamp`: con un NaN dan 0).
            let byte = |v: f32| (v.max(0.0).min(255.0) + 8_388_608.0).to_bits() & 0xFF;
            for (((o, &r), &g), &b) in packed.iter_mut().zip(r.iter()).zip(g.iter()).zip(b.iter()) {
                *o = byte(r) | byte(g) << 8 | byte(b) << 16 | 0xFF00_0000;
            }
            for (px, v) in row.chunks_exact_mut(4).zip(packed.iter()) {
                px.copy_from_slice(&v.to_le_bytes());
            }
        });
    }
}

/// Una linea de velocidad de este frame, en pixeles: sale del centro de
/// la pantalla en la direccion unitaria `(dx, dy)`, va de `tail` a `head`
/// pixeles del centro y suma `amp` del celeste.
#[derive(Clone, Copy, Debug)]
struct Spoke {
    dx: f32,
    dy: f32,
    tail: f32,
    head: f32,
    amp: f32,
}

/// Hasta cuantos pixeles a cada lado lee la aberracion (su corrimiento no
/// pasa de `ABERRATION_PX = 2.5`, asi que bastan 3).
const TAPS: usize = 3;

/// Copia `src` a `dst` con `TAPS` pixeles del borde repetidos a cada lado.
fn pad_copy(src: &[f32], dst: &mut Vec<f32>) {
    let (Some(&first), Some(&last)) = (src.first(), src.last()) else {
        dst.clear();
        return;
    };
    dst.clear();
    dst.extend(std::iter::repeat_n(first, TAPS));
    dst.extend_from_slice(src);
    dst.extend(std::iter::repeat_n(last, TAPS));
}

/// Un canal corrido `sign shift[i]` pixeles con interpolacion lineal y
/// mezclado al 60 % sobre `out` (los pixeles `x0..x0 + out.len()`). La
/// muestra en `x + p`, con `p = sign s` fraccionaria (`|p| <= 2.5`), es
/// `lerp(c[x + i], c[x + i + 1], p - i)` con `i = floor(p)`. El corrimiento
/// cambia despacio y en un solo sentido a lo ancho, asi que `i` es el
/// mismo en tramos largos (unos seis por fila): en cada tramo se leen dos
/// pedazos seguidos de la fila y no un indice por pixel (se vectoriza).
fn shift_blend(padded: &[f32], shift: &[f32], sign: f32, x0: usize, out: &mut [f32]) {
    let n = out.len().min(shift.len());
    let floor_at = |i: usize| (sign * shift[i]).floor().clamp(-(TAPS as f32), TAPS as f32 - 1.0);
    let mut i = 0;
    while i < n {
        let base = floor_at(i);
        // El corrimiento es monotono a lo ancho de la fila (`nx` por algo
        // que crece con `|nx|`): el fin del tramo se busca por biseccion.
        let j = i + 1 + shift[i + 1..n].partition_point(|&v| (sign * v).floor().clamp(-(TAPS as f32), TAPS as f32 - 1.0) == base);
        // `padded[x0 + x + TAPS + base]` es el pixel `x0 + x + base`.
        let k = (base as i32 + TAPS as i32) as usize;
        let (a, b) = (&padded[x0 + i + k..x0 + j + k], &padded[x0 + i + k + 1..x0 + j + k + 1]);
        for ((o, &sh), (&va, &vb)) in out[i..j].iter_mut().zip(&shift[i..j]).zip(a.iter().zip(b)) {
            let f = (sign * sh - base).clamp(0.0, 1.0);
            let v = va + (vb - va) * f;
            *o += (v - *o) * ABERRATION_MIX;
        }
        i = j;
    }
}

/// Las lineas de velocidad de este frame. La vuelta se parte en `SPOKES`
/// rayos (en coordenadas normalizadas, asi en la pantalla ancha salen mas
/// tendidos) y cada uno puede llevar una estela, un tramo `[cola, cabeza]`
/// en radio que corre hacia afuera. Cada rayo vive en ciclos de `LIFE`
/// segundos con su propio desfase (no cambian todos a la vez, que
/// parpadearia como estroboscopio): en cada ciclo un hash decide si se
/// prende, entra y sale suave (`sin(pi fase)`), la cola nace a su propio
/// radio (de 0.5 a 0.8, siempre en la corona `r > 0.45`) y avanza de 0.5 a
/// 0.8 a su propio paso. Con turbo se prende uno de cada ~10 (hash > 0.9),
/// fuerte y en toda la vuelta; sin turbo, arriba del 80 % de la maxima,
/// pocos (hash > 0.97), tenues, mas largos y solo en las orillas de los
/// lados (`|cos a| > 0.8`): rayas que salen disparadas hacia la orilla y
/// no un abanico fijo desde el centro.
///
/// Son pocas (~30), asi que no se evaluan por pixel: cada fila calcula
/// donde la cruza cada una y pinta solo esos pixeles.
pub struct Spokes {
    lines: Vec<Spoke>,
}

impl Spokes {
    fn new(p: &Params, width: usize, height: usize) -> Self {
        let turbo_amp = 0.35 * p.boost_s.clamp(0.0, 1.0);
        let speed_amp = 0.12 * smoothstep(0.8, 1.0, p.speed);
        let mut lines = Vec::new();
        if turbo_amp < 0.003 && speed_amp < 0.001 {
            return Self { lines };
        }
        let (cx, cy) = (width as f32 * 0.5, height as f32 * 0.5);
        let unit = |h: u32| (h >> 8) as f32 * (1.0 / 16_777_216.0);
        let clock = p.wall.max(0.0) / LIFE;
        for id in 0..SPOKES {
            // El ciclo de este rayo y la fase adentro de el.
            let cycle = clock + unit(hash_u32(id ^ 0x2F6B_D3A1));
            let (generation, phase) = (cycle.floor() as u32, cycle - cycle.floor());
            let h = hash_u32(id.wrapping_mul(0x9E37_79B1) ^ generation.wrapping_mul(0x85EB_CA77));
            let u = unit(h);
            let (sin, cos) = ((id as f32 + 0.5) * (TAU / SPOKES as f32)).sin_cos();
            let turbo = u > 0.9 && turbo_amp > 0.0;
            let speed = u > 0.97 && cos.abs() > 0.8 && speed_amp > 0.0;
            if !turbo && !speed {
                continue;
            }
            let amp = if turbo { turbo_amp } else { 0.0f32 }.max(if speed { speed_amp } else { 0.0 }) * (std::f32::consts::PI * phase).sin();
            if amp <= 1e-3 {
                continue;
            }
            let h2 = hash_u32(h);
            let tail = 0.5 + 0.3 * unit(h2) + (0.5 + 0.3 * unit(hash_u32(h2))) * phase;
            let len = if turbo { 0.15 } else { 0.3 } + 0.2 * unit(hash_u32(h2 ^ 0x51ED));
            // De normalizado a pixeles: `(r cos a cx, r sin a cy)`, que
            // mide `r L` pixeles con `L = hypot(cos a cx, sin a cy)`.
            let (ex, ey) = (cos * cx, sin * cy);
            let l = ex.hypot(ey).max(1e-3);
            lines.push(Spoke { dx: ex / l, dy: ey / l, tail: tail * l, head: (tail + len) * l, amp });
        }
        Self { lines }
    }

    /// Suma las lineas que cruzan la fila `py` (pixeles bajo el centro) a
    /// los canales. Cada pixel mide su distancia a la recta,
    /// `d = |px dy - py dx|`, y cuanto avanza sobre ella, `t = px dx + py dy`:
    /// la linea cubre `cover(d - ancho)` con una media anchura (distancia
    /// al eje) de 0.6 a 1.6 px que engorda hacia la cabeza (la raya mide
    /// de 1.2 a 3.2 px mas el antialias), y adentro de su tramo sube en linea de la
    /// cola a la cabeza (que se corta en 6 px). Los pixeles que puede tocar
    /// salen de despejar `x` en `|d| < ancho + 1` y en `cola < t < cabeza`.
    fn row(&self, py: f32, cx: f32, r: &mut [f32], g: &mut [f32], b: &mut [f32]) {
        const REACH: f32 = 2.6;
        let w = r.len();
        for line in &self.lines {
            let (dx, dy) = (line.dx, line.dy);
            // El tramo de `px` donde la recta pasa a menos de REACH.
            let (mut lo, mut hi) = if dy.abs() > 1e-4 {
                let (a, b) = ((py * dx - REACH) / dy, (py * dx + REACH) / dy);
                (a.min(b), a.max(b))
            } else if (py * dx).abs() < REACH {
                (f32::MIN, f32::MAX)
            } else {
                continue;
            };
            // Y donde cae entre la cola y la cabeza.
            if dx.abs() > 1e-4 {
                let (a, b) = ((line.tail - py * dy) / dx, (line.head - py * dy) / dx);
                lo = lo.max(a.min(b) - REACH);
                hi = hi.min(a.max(b) + REACH);
            } else if !(line.tail - REACH..line.head + REACH).contains(&(py * dy)) {
                continue;
            }
            let x0 = (lo + cx - 0.5).floor().max(0.0);
            let x1 = (hi + cx + 0.5).ceil().min(w as f32);
            if x1 <= x0 {
                continue;
            }
            let inv = 1.0 / (line.head - line.tail).max(1.0);
            for x in x0 as usize..x1 as usize {
                let px = x as f32 + 0.5 - cx;
                let d = (px * dy - py * dx).abs();
                let t = px * dx + py * dy;
                let along = ((t - line.tail) * inv).clamp(0.0, 1.0);
                let tip = ((line.head - t) * (1.0 / 6.0)).clamp(0.0, 1.0);
                let width = 0.6 + along;
                let add = line.amp * along * tip * (width + 0.5 - d).clamp(0.0, 1.0);
                r[x] += SPOKE_COLOR.x * add;
                g[x] += SPOKE_COLOR.y * add;
                b[x] += SPOKE_COLOR.z * add;
            }
        }
    }
}

/// Ruido de valor periodico: cuatro octavas de `4, 8, 16, 32` celdas por
/// lado, cada una a la mitad de la amplitud de la anterior. Cada octava
/// hashea sus esquinas con el indice modulo sus celdas, asi la textura se
/// repite sin costura. Se estira el contraste alrededor de 0.5 (la suma de
/// octavas se amontona en el centro).
fn bake_noise() -> Vec<u8> {
    let mut out = vec![0u8; NOISE * NOISE];
    parallel::chunks_mut(&mut out, NOISE, NOISE * 8, |y, row| {
        for (x, o) in row.iter_mut().enumerate() {
            let (mut sum, mut amp, mut total) = (0.0, 1.0, 0.0);
            for k in 0..4 {
                let cells = NOISE_CELLS << k;
                let scale = cells as f32 / NOISE as f32;
                sum += amp * periodic(x as f32 * scale, y as f32 * scale, cells, k);
                total += amp;
                amp *= 0.5;
            }
            let v = ((sum / total - 0.5) * 1.8 + 0.5).clamp(0.0, 1.0);
            *o = (v * 255.0 + 0.5) as u8;
        }
    });
    out
}

/// Ruido de valor de 0 a 1 que se repite cada `period` celdas.
fn periodic(x: f32, y: f32, period: u32, salt: u32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as u32, yi as u32);
    let corner = |dx: u32, dy: u32| {
        let (i, j) = ((xi + dx) % period, (yi + dy) % period);
        (hash_u32(i.wrapping_mul(0x9E37_79B9) ^ j.wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35)) >> 8) as f32 * (1.0 / 16_777_216.0)
    };
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_noise_repeats_without_a_seam() {
        let post = Posproceso::new(64, 48);
        for &(u, v) in &[(0.13f32, 0.71f32), (0.999, 0.5), (0.4, 0.002)] {
            let a = post.tex(u, v);
            assert!((a - post.tex(u + 1.0, v - 2.0)).abs() < 1e-4);
            assert!((0.0..=1.0).contains(&a));
        }
        // Del borde al otro lado no hay salto mas grande que entre vecinos.
        let jump = (post.tex(0.9999, 0.3) - post.tex(0.0001, 0.3)).abs();
        assert!(jump < 0.05, "{jump}");
        assert!(post.tex(f32::NAN, 0.5).is_finite());
    }

    /// Solo lo que emite florece, el compuesto deja todo opaco y la vineta
    /// oscurece las esquinas.
    #[test]
    fn only_what_emits_blooms() {
        let (w, h) = (160, 96);
        let mut post = Posproceso::new(w, h);
        let mut rgba = vec![0u8; w * h * 4];
        for (i, p) in rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (i % w, i / w);
            lienzo::write(p, Vec3::splat(0.2));
            // Un foco que emite a la izquierda y uno igual de blanco que no
            // emite a la derecha.
            if (x as i32 - 40).abs() < 3 && (y as i32 - 48).abs() < 3 {
                lienzo::write_emit(p, Vec3::ONE, 1.0);
            }
            if (x as i32 - 120).abs() < 3 && (y as i32 - 48).abs() < 3 {
                lienzo::write(p, Vec3::ONE);
            }
        }
        post.prepare_frame(&mut rgba, &Params::default());
        post.compose(&mut rgba, &Params::default(), |_, _| {});
        assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
        let at = |x: usize, y: usize| lienzo::read(&rgba[(y * w + x) * 4..]).x;
        // A 8 px de cada foco: al lado del que emite hay mas luz.
        assert!(at(52, 48) > at(108, 48) + 0.03, "{} {}", at(52, 48), at(108, 48));
        assert!(at(1, 1) < at(w / 2, 20));
    }

    /// Cuanto cuesta cada mitad a 1280 x 720
    /// (`cargo test --release medir_posproceso -- --ignored --nocapture`).
    #[test]
    #[ignore = "mide tiempos"]
    fn medir_posproceso() {
        let (w, h) = (1280, 720);
        let mut post = Posproceso::new(w, h);
        let mut rgba: Vec<u8> = (0..w * h * 4).map(|i| (hash_u32(i as u32) >> 24) as u8 | if i % 4 == 3 { 0xF0 } else { 0 }).collect();
        let source = rgba.clone();
        let best = |f: &mut dyn FnMut()| (0..40).map(|_| {
            let t = std::time::Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1e3
        }).fold(f64::MAX, f64::min);
        let params = Params { boost_s: 1.0, speed: 1.0, smoke: 0.5, ..Default::default() };
        // Con humo: su rejilla se arma en `prepare`.
        let mut frame = source.clone();
        let prepare_smoke = best(&mut || post.prepare_frame(&mut frame, &params));
        let busy = best(&mut || post.compose(&mut rgba, &params, |_, _| {}));
        let smoke = best(&mut || post.compose(&mut rgba, &Params { boost_s: 0.0, speed: 0.0, ..params }, |_, _| {}));
        let prepare = best(&mut || post.prepare_frame(&mut frame, &Params::default()));
        // Como un frame de verdad: emite uno de cada veinte.
        let real: Vec<u8> = source.iter().enumerate().map(|(i, &v)| if i % 4 == 3 && (i / 4) % 20 != 0 { 255 } else { v }).collect();
        let mut real = real;
        let extract = best(&mut || {
            let mut sums = post.take_sums();
            sums.paint(&mut real, |_, _| {});
            post.extract(&sums);
            post.sums = sums.cells;
        });
        let pyramid = best(&mut || post.bloom.process());
        println!("extraccion {extract:.3} ms   piramide {pyramid:.3} ms   prepare con humo {prepare_smoke:.3} ms");
        let plain = best(&mut || post.compose(&mut rgba, &Params::default(), |_, _| {}));
        let turbo = best(&mut || post.compose(&mut rgba, &Params { smoke: 0.0, ..params }, |_, _| {}));
        let lines = best(&mut || post.compose(&mut rgba, &Params { speed: 1.0, ..Default::default() }, |_, _| {}));
        println!("solo turbo {turbo:.3} ms   solo humo {smoke:.3} ms   solo lineas {lines:.3} ms");
        let serial = best(&mut || {
            for (y, row) in rgba.chunks_mut(w * 4).enumerate() {
                post.compose_row(&Params::default(), &Spokes::new(&Params::default(), w, h), y, row);
            }
        });
        println!("prepare {prepare:.3} ms   compose {plain:.3} ms   con turbo y humo {busy:.3} ms   compose 1 hilo {serial:.3} ms ({:.1} ns/px) con {} hilos", serial * 1e6 / (w * h) as f64, parallel::workers());
    }

    /// El camino rapido de `widen` (cuatro pixeles por texel) da lo mismo
    /// que el general con las columnas precalculadas.
    #[test]
    fn widen_paths_agree() {
        let post = Posproceso::new(320, 180);
        assert_eq!(post.width, 4 * post.gw);
        let line: Vec<f32> = (0..post.gw).map(|i| (hash_u32(i as u32) >> 8) as f32 / 16_777_216.0).collect();
        let mut fast = vec![0.0; post.width];
        post.widen(&line, &mut fast);
        for (x, &v) in fast.iter().enumerate() {
            let (a, b) = (line[post.g0[x] as usize], line[post.g1[x] as usize]);
            let general = a + (b - a) * post.gt[x];
            assert!((v - general).abs() < 1e-5, "x = {x}: {v} {general}");
        }
    }

    /// La aberracion corre el canal sin salirse de la fila: con
    /// corrimiento entero copia al vecino, con medio pixel promedia, y lo
    /// mezcla al 60 %.
    #[test]
    fn the_aberration_shifts_by_fractions() {
        let src: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let mut padded = Vec::new();
        pad_copy(&src, &mut padded);
        assert_eq!(padded.len(), 16 + 2 * TAPS);
        // Corrimiento de +2 px leyendo hacia la derecha (`sign = 1`).
        let mut out = src.clone();
        shift_blend(&padded, &[2.0; 16], 1.0, 0, &mut out);
        assert!((out[5] - (5.0 + (7.0 - 5.0) * ABERRATION_MIX)).abs() < 1e-5, "{}", out[5]);
        // Medio pixel hacia la izquierda: el promedio de `x - 1` y `x`.
        let mut out = src.clone();
        shift_blend(&padded, &[0.5; 16], -1.0, 0, &mut out);
        assert!((out[5] - (5.0 + (4.5 - 5.0) * ABERRATION_MIX)).abs() < 1e-5, "{}", out[5]);
        // En los bordes repite el pixel de la orilla y no se sale.
        let mut out = src.clone();
        shift_blend(&padded, &[-2.5; 16], 1.0, 0, &mut out);
        assert!(out.iter().all(|v| v.is_finite()) && (out[0] - 0.0).abs() < 1e-5);
        // Un corrimiento que crece a lo ancho (como en la pantalla) da lo
        // mismo que muestra por muestra.
        let shift: Vec<f32> = (0..16).map(|i| (i as f32 - 7.5) / 3.0).collect();
        let mut out = src.clone();
        shift_blend(&padded, &shift, 1.0, 0, &mut out);
        for (i, (&o, &sh)) in out.iter().zip(&shift).enumerate() {
            let at = (i as f32 + sh).clamp(0.0, 15.0);
            assert!((o - (src[i] + (at - src[i]) * ABERRATION_MIX)).abs() < 1e-4, "{i}: {o} {at}");
        }
    }

    #[test]
    fn the_spokes_live_in_the_crown() {
        let (w, h) = (640usize, 360usize);
        // Sin turbo ni velocidad no hay lineas.
        assert!(Spokes::new(&Params::default(), w, h).lines.is_empty());
        // Con turbo hay unas ~30; pintadas en toda la pantalla, algo se
        // prende, nada pasa de su fuerza y adentro de la corona no hay.
        let turbo = Spokes::new(&Params { boost_s: 1.0, wall: 0.6, ..Default::default() }, w, h);
        assert!((10..60).contains(&turbo.lines.len()), "{}", turbo.lines.len());
        let mut lit = 0;
        for y in 0..h {
            let (mut r, mut g, mut b) = (vec![0.0; w], vec![0.0; w], vec![0.0; w]);
            let py = y as f32 + 0.5 - h as f32 * 0.5;
            turbo.row(py, w as f32 * 0.5, &mut r, &mut g, &mut b);
            for x in 0..w {
                assert!(b[x].is_finite() && b[x] <= 0.3 * 2.0 + 1e-4, "{}", b[x]);
                let (nx, ny) = ((x as f32 + 0.5) / (w as f32 * 0.5) - 1.0, py / (h as f32 * 0.5));
                if nx.hypot(ny) < 0.45 {
                    assert_eq!(b[x], 0.0, "({x}, {y})");
                }
                lit += (b[x] > 0.05) as usize;
            }
        }
        assert!(lit > 500, "{lit}");
    }
}

