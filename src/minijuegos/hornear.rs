//! Hornear sprites: un rasterizador propio y chico que dibuja un modelo (las
//! naves de Blender) desde varios angulos una sola vez, al abrir un
//! minijuego. Despues el juego solo escala y pega imagenes: dibujar los
//! 28 000 triangulos de cada nave en cada frame costaria lo que el 3D.
//!
//! - **Proyeccion ortografica**: de lejos una nave casi no tiene
//!   perspectiva, y asi un mismo sprite sirve a cualquier distancia. La
//!   camara mira la nave desde atras (giro 0) y un poco desde arriba; cada
//!   sprite es otro giro.
//! - **Raster con buffer de profundidad y supersampling**: cada triangulo se
//!   recorre en su caja con coordenadas baricentricas (funciones de borde),
//!   a 2x2 muestras por pixel; al final cada pixel promedia sus muestras y
//!   la fraccion cubierta es su alfa (bordes suaves). El sombreado es
//!   diferido: el raster solo guarda que triangulo quedo mas cerca y sus
//!   pesos baricentricos, y cada muestra se sombrea una vez al final (una
//!   nave tiene varias capas de piezas encimadas) interpolando lo que haga
//!   falta: la normal, el color, la pieza y la posicion en el modelo.
//! - **Sombreado** con las clases de pieza de `obj::Surface` (pintura con
//!   laca, metal, vidrio, LEDs, hule), con una luz de luna fria, la luz
//!   naranja de los postes desde abajo y un cielo de noche que se refleja.
//!   La botella es vidrio ambar a contraluz con la cerveza adentro.
//! - **Las LEDs van aparte** (`glow`): su luz encima de la nave, con el
//!   nucleo blanco (donde la LED llena el pixel y sus vecinos), y un halo de
//!   dos escalas (un difuminado corto y uno ancho) que sale del contorno,
//!   como neon. El byte de alfa del `glow` dice como se anima cada LED (ver
//!   `led_tag`), asi el juego las hace correr, respirar o encenderse al
//!   frenar sin volver a hornear.
//! - **Mipmaps**: cada nivel promedia 2 x 2 del anterior (premultiplicado),
//!   y el juego lee el que corresponde al tamano en la pantalla (trilineal
//!   entre dos): de lejos la nave se promedia en vez de centellear.
//! - **Mapeo de tonos** al guardar (`1 - e^(-x)` y gamma 2.2): los colores
//!   del `.mtl` son lineales.

use std::sync::OnceLock;

use crate::math::{Vec3, fast_exp, fast_pow, hash_u32, smoothstep};
use crate::parallel;
use crate::scene::{Mesh, Surface};

/// Como se sombrea una malla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Las naves botella: solo el color de cada cara, todo pulido (y el
    /// vidrio ambar por su color).
    Bottle,
    /// Los vehiculos: color y clase de pieza en la `uv`.
    Vehicle,
}

/// Como se anima la luz de una LED: el patron va en los dos bits altos del
/// byte de alfa del `glow` y la fase (de 0 a 63) en los seis bajos.
pub const PATTERN_STATIC: u8 = 0;
/// Ambar: una persecucion de feria que corre a lo largo de la nave.
pub const PATTERN_CHASE: u8 = 1;
/// Magenta, cian o azul: respiran.
pub const PATTERN_BREATHE: u8 = 2;
/// Rojas que miran hacia atras: los stops, que se encienden al frenar.
pub const PATTERN_STOP: u8 = 3;

/// Un nivel de mip: la mitad de ancho y alto que el anterior.
#[derive(Clone, Debug)]
pub struct Level {
    pub w: usize,
    pub h: usize,
    /// El color y la luz de las LEDs de cada pixel juntos (ver
    /// `Sprite::texels`).
    pub texels: Vec<[u8; 8]>,
    /// Por fila, el tramo de columnas con algo y el opaco (como
    /// `Sprite::spans` y `Sprite::solid`).
    pub spans: Vec<(u16, u16)>,
    pub solid: Vec<(u16, u16)>,
}

/// Una imagen horneada, RGBA8 premultiplicada.
#[derive(Clone, Debug)]
pub struct Sprite {
    pub width: usize,
    pub height: usize,
    /// La nave sin la luz de sus LEDs (premultiplicada por su alfa).
    pub pixels: Vec<[u8; 4]>,
    /// La luz de las LEDs que se suma encima (rgb de 0 a 1, sin alfa: es
    /// luz) y en el byte de alfa su patron y su fase (`PATTERN_*`). Vacio si
    /// el sprite no tiene LEDs (la botella, los barriletes).
    pub glow: Vec<[u8; 4]>,
    /// Los dos juntos, ocho bytes por pixel (`pixels` y despues `glow`):
    /// el juego lee los dos de cada pixel de una vez (una sola busqueda en
    /// memoria). Vacio en los barriletes.
    pub texels: Vec<[u8; 8]>,
    /// Por fila, el tramo de columnas `[desde, hasta)` que tiene algo (la
    /// imagen lleva margen para el resplandor y, de lado, la nave no llena
    /// su caja): quien la pega no muestrea lo que esta vacio.
    pub spans: Vec<(u16, u16)>,
    /// Por fila `y0`, el tramo mas largo `[desde, hasta)` de columnas `x0`
    /// donde los cuatro pixeles que lee el bilineal (`x0`, `x0 + 1` en las
    /// filas `y0` y `y0 + 1`) son opacos: ahi la nave tapa todo lo de
    /// atras y quien la pega no necesita pintar el fondo.
    pub solid: Vec<(u16, u16)>,
    /// Donde toca el suelo el centro de la nave, en pixeles del sprite.
    pub anchor: (f32, f32),
    /// Pixeles por metro.
    pub scale: f32,
    /// Los mips desde el nivel 1 (vacio en los barriletes, que traen los
    /// suyos aparte).
    pub levels: Vec<Level>,
}

/// Lo que da un sprite en un punto: el color premultiplicado (rgba de 0 a
/// 1) y la luz de las LEDs (rgb) ya animada (por la tabla de su patron).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Texel {
    pub body: [f32; 4],
    pub light: [f32; 3],
}

impl Texel {
    /// `self + (o - self) t`.
    #[inline(always)]
    pub fn lerp(self, o: Texel, t: f32) -> Texel {
        Texel {
            body: std::array::from_fn(|k| self.body[k] + (o.body[k] - self.body[k]) * t),
            light: std::array::from_fn(|k| self.light[k] + (o.light[k] - self.light[k]) * t),
        }
    }
}

/// La tabla de las LEDs: cuanto brilla cada patron y fase en este frame,
/// indexada por el byte de alfa del `glow` tal cual (`patron << 6 | fase`).
pub type Lut = [f32; 256];

/// Una tabla con todo a 1 (las LEDs fijas, sin animar).
#[cfg(test)]
pub const LUT_ONE: Lut = [1.0; 256];

/// Que niveles de mip se leen para un `lod` (ver `Sprite::pick`): el fino,
/// cuanto del siguiente y la escala de cada uno (`1 / 2^l`). Se saca una
/// vez por nave y no por pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pick {
    l0: usize,
    t: f32,
    s0: f32,
    s1: f32,
}

/// El pixel `(x, y)` de una imagen RGBA8 de `w x h` con interpolacion
/// bilineal, de 0 a 1 (afuera de la imagen es cero).
#[inline]
fn bilinear(pixels: &[[u8; 4]], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    let x = x - 0.5;
    let y = y - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i32, y0 as i32);
    let s = 1.0 / 255.0;
    // Adentro (casi siempre): los cuatro de una vez, sin revisar cada uno,
    // con los pesos `(1 - fx)(1 - fy)`, `fx (1 - fy)`, ...
    if x0 >= 0 && y0 >= 0 && (x0 as usize) + 1 < w && (y0 as usize) + 1 < h && pixels.len() >= w * h {
        let i = y0 as usize * w + x0 as usize;
        let (a, b) = (pixels[i], pixels[i + 1]);
        let (c, d) = (pixels[i + w], pixels[i + w + 1]);
        let (wa, wb, wc, wd) = ((1.0 - fx) * (1.0 - fy) * s, fx * (1.0 - fy) * s, (1.0 - fx) * fy * s, fx * fy * s);
        return std::array::from_fn(|k| a[k] as f32 * wa + b[k] as f32 * wb + c[k] as f32 * wc + d[k] as f32 * wd);
    }
    let at = |xi: i32, yi: i32| -> [f32; 4] {
        if xi < 0 || yi < 0 || xi >= w as i32 || yi >= h as i32 {
            return [0.0; 4];
        }
        match pixels.get(yi as usize * w + xi as usize) {
            Some(p) => [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32],
            None => [0.0; 4],
        }
    };
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        out[k] = (top + (bottom - top) * fy) * s;
    }
    out
}

/// El bilineal de una imagen de ocho bytes por pixel (`texels`) en
/// `(x, y)`: los ocho canales de los cuatro pixeles juntos, con los pesos
/// `(1 - fx)(1 - fy)`, `fx (1 - fy)`, ... de cada esquina (el compilador
/// hace los ocho canales en paralelo en un registro).
///
/// La luz de las LEDs se anima en cada esquina antes de mezclar:
/// `luz = sum w_i luz_i lut[patron_i]`. Si se animara con el patron del
/// pixel mas cercano, la tabla saltaria de golpe en cada frontera de pixel
/// (de 0.55 a 1, o de 0.35 a 1 en la persecucion) y, agrandada, la luz se
/// veria en cuadros de borde duro. Afuera de la imagen es cero.
#[inline(always)]
fn bilinear8(texels: &[[u8; 8]], w: usize, h: usize, x: f32, y: f32, lut: &Lut) -> Texel {
    let x = x - 0.5;
    let y = y - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i32, y0 as i32);
    let quad = if x0 >= 0 && y0 >= 0 && (x0 as usize) + 1 < w && (y0 as usize) + 1 < h {
        let i = y0 as usize * w + x0 as usize;
        match (texels.get(i..i + 2), texels.get(i + w..i + w + 2)) {
            (Some(top), Some(bottom)) => [top[0], top[1], bottom[0], bottom[1]],
            _ => [[0; 8]; 4],
        }
    } else {
        let at = |xi: i32, yi: i32| -> [u8; 8] {
            if xi < 0 || yi < 0 || xi >= w as i32 || yi >= h as i32 {
                return [0; 8];
            }
            texels.get(yi as usize * w + xi as usize).copied().unwrap_or([0; 8])
        };
        [at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1)]
    };
    let f = |t: [u8; 8]| t.map(|v| v as f32);
    let (a, b, c, d) = (f(quad[0]), f(quad[1]), f(quad[2]), f(quad[3]));
    let s = 1.0 / 255.0;
    // El cuerpo en dos pasos (`arriba = a + (b - a) fx`, `abajo = c + (d -
    // c) fx`, `arriba + (abajo - arriba) fy`): cuatro esquinas opacas dan
    // alfa 1 exacto.
    let mut body = [0.0f32; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        body[k] = (top + (bottom - top) * fy) * s;
    }
    // La luz con el peso de cada esquina por su tabla.
    let m = [
        (1.0 - fx) * (1.0 - fy) * s * lut[quad[0][7] as usize],
        fx * (1.0 - fy) * s * lut[quad[1][7] as usize],
        (1.0 - fx) * fy * s * lut[quad[2][7] as usize],
        fx * fy * s * lut[quad[3][7] as usize],
    ];
    let light = std::array::from_fn(|k| a[k + 4] * m[0] + b[k + 4] * m[1] + c[k + 4] * m[2] + d[k + 4] * m[3]);
    Texel { body, light }
}

impl Sprite {
    /// El pixel (x, y) con interpolacion bilineal, premultiplicado, de 0 a 1.
    #[inline]
    pub fn sample(&self, x: f32, y: f32) -> [f32; 4] {
        bilinear(&self.pixels, self.width, self.height, x, y)
    }

    /// El nivel `l` (0 la imagen misma) en `(x, y)` medidos en pixeles del
    /// nivel 0: en el nivel `l` son `(x, y) s`, con `s = 1 / 2^l` (cada
    /// pixel del nivel cubre `2^l` del 0, con su centro en `2^l (i + 0.5)`).
    #[inline(always)]
    fn level_texel(&self, l: usize, s: f32, x: f32, y: f32, lut: &Lut) -> Texel {
        if l == 0 {
            return bilinear8(&self.texels, self.width, self.height, x, y, lut);
        }
        match self.levels.get(l - 1) {
            Some(lv) => bilinear8(&lv.texels, lv.w, lv.h, x * s, y * s, lut),
            None => Texel::default(),
        }
    }

    /// Que niveles lee `lod` (ver `lod_split`), con la escala de cada uno.
    pub fn pick(&self, lod: f32) -> Pick {
        let (l0, t) = self.lod_split(lod);
        let scale = |l: usize| 1.0 / (1u64 << l.min(40)) as f32;
        Pick { l0, t, s0: scale(l0), s1: scale(l0 + 1) }
    }

    /// El nivel mas fino que se usa con `lod` y cuanto del siguiente, con
    /// `lod` recortado a los niveles que hay. La mezcla es "brilineal":
    /// `smoothstep(0.3, 0.7, fract(lod))`, asi cerca de cada nivel se lee
    /// uno solo (la mitad de las lecturas) y entre dos se funden sin salto.
    #[inline(always)]
    fn lod_split(&self, lod: f32) -> (usize, f32) {
        let top = self.levels.len();
        let lod = if lod > 0.0 { lod.min(top as f32) } else { 0.0 };
        let l0 = lod.floor() as usize;
        let t = smoothstep(0.3, 0.7, lod - l0 as f32);
        if t >= 1.0 && l0 < top { (l0 + 1, 0.0) } else { (l0, t) }
    }

    /// El color y la luz de las LEDs (animada con `lut`) en `(x, y)`
    /// (pixeles del nivel 0) con los niveles de `pick`: trilineal, el
    /// bilineal del nivel fino y el del siguiente mezclados por `t`.
    #[inline(always)]
    pub fn sample_pick(&self, p: &Pick, x: f32, y: f32, lut: &Lut) -> Texel {
        let a = self.level_texel(p.l0, p.s0, x, y, lut);
        if p.t <= 0.0 {
            return a;
        }
        let b = self.level_texel(p.l0 + 1, p.s1, x, y, lut);
        a.lerp(b, p.t)
    }

    /// `sample_pick` con el mip que toca a `lod` (`log2` de cuantos
    /// pixeles del sprite caen en uno de la pantalla).
    #[cfg(test)]
    pub fn sample_lod(&self, x: f32, y: f32, lod: f32, lut: &Lut) -> Texel {
        self.sample_pick(&self.pick(lod), x, y, lut)
    }

    /// El tramo de `x` (pixeles del nivel 0) donde algo de las filas `y`
    /// de `y_lo` a `y_hi` puede no ser cero con el mip de `lod` (el color
    /// y la luz de las LEDs): la union de los tramos de las filas que lee
    /// el bilineal en el nivel fino y en el siguiente. El bilineal lee las
    /// filas `floor(y - 0.5)` y la siguiente, y en `x` las columnas
    /// `floor(x - 0.5)` y la siguiente, asi que un tramo `[a, b)` del nivel
    /// `l` da `((a - 0.5) 2^l, (b + 0.5) 2^l)`. Vacio si no hay nada.
    pub fn span_rows(&self, y_lo: f32, y_hi: f32, lod: f32) -> (f32, f32) {
        let (l0, t) = self.lod_split(lod);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let levels = if t > 0.0 { l0..l0 + 2 } else { l0..l0 + 1 };
        for l in levels {
            let spans = if l == 0 { &self.spans } else { match self.levels.get(l - 1) { Some(lv) => &lv.spans, None => continue } };
            let k = (1u32 << l) as f32;
            let (a, b) = ((y_lo / k - 0.5).floor().max(0.0), ((y_hi / k - 0.5).floor() + 1.0).min(spans.len() as f32 - 1.0));
            let mut yi = a;
            while yi <= b {
                let (sa, sb) = spans[yi as usize];
                if sb > sa {
                    lo = lo.min((sa as f32 - 0.5) * k);
                    hi = hi.max((sb as f32 + 0.5) * k);
                }
                yi += 1.0;
            }
        }
        if hi <= lo { (0.0, 0.0) } else { (lo, hi) }
    }

    /// El tramo de `x` (pixeles del nivel 0) donde lo que da `sample_lod`
    /// con `lod` es opaco del todo en todas las filas de `y_lo` a `y_hi`:
    /// en cada nivel que se lee (el fino y, si se mezcla, el siguiente), la
    /// interseccion de su `solid` en las filas `y0 = floor(y / 2^l - 0.5)`
    /// que caen (con `x0 = floor(x / 2^l - 0.5)` en `[desde, hasta)`, `x` va
    /// de `(desde + 0.5) 2^l` a `(hasta + 0.5) 2^l`). Para una fila de la
    /// pantalla que cruza el sprite inclinada. Vacio si alguna fila no
    /// tiene tramo opaco.
    pub fn solid_rows(&self, y_lo: f32, y_hi: f32, lod: f32) -> (f32, f32) {
        let (l0, t) = self.lod_split(lod);
        let levels = if t > 0.0 { l0..l0 + 2 } else { l0..l0 + 1 };
        let (mut lo, mut hi) = (f32::MIN, f32::MAX);
        for l in levels {
            let solid = if l == 0 { &self.solid } else { match self.levels.get(l - 1) { Some(lv) => &lv.solid, None => return (0.0, 0.0) } };
            let k = (1u32 << l) as f32;
            let (a, b) = ((y_lo / k - 0.5).floor(), (y_hi / k - 0.5).floor());
            if !(a >= 0.0) || !(b >= a) || b as usize >= solid.len() {
                return (0.0, 0.0);
            }
            let (mut sa, mut sb) = (0u16, u16::MAX);
            for yi in a as usize..=b as usize {
                let (ra, rb) = solid[yi];
                sa = sa.max(ra);
                sb = sb.min(rb);
            }
            if sb <= sa {
                return (0.0, 0.0);
            }
            lo = lo.max((sa as f32 + 0.5) * k);
            hi = hi.min((sb as f32 + 0.5) * k);
        }
        if hi <= lo { (0.0, 0.0) } else { (lo, hi) }
    }
}

/// Muestras por lado de cada pixel.
const SUPER: usize = 2;
/// El margen de la imagen (pixeles) para el resplandor de las LEDs: el halo
/// ancho llega a unos 21 pixeles y se recorta suave.
const MARGIN: usize = 16;
/// Hasta que lado se hacen mips (el ultimo nivel ya mide menos).
const MIP_MIN: usize = 8;

/// Un sprite por hornear: la malla, como se sombrea, el giro y la altura
/// de la vista (radianes), cuanto mide una unidad del modelo en metros,
/// los pixeles por metro y la perspectiva: a cuantos metros de la cola esta
/// la camara (`eye`; 0 es de lejos, ortografica) y cuantos metros arriba
/// del centro de la nave queda el punto de fuga en la imagen (`rise`, el
/// horizonte).
#[derive(Clone, Copy)]
pub struct Job<'a> {
    pub mesh: &'a Mesh,
    pub finish: Finish,
    pub yaw: f32,
    pub elevation: f32,
    pub meters: f32,
    pub pixels_per_meter: f32,
    pub eye: f32,
    pub rise: f32,
}

/// Hornea todos los sprites de `jobs` en una sola pasada en paralelo (cada
/// hilo toma el siguiente que falte: los de lado tardan mas que los de
/// atras).
pub fn bake_all(jobs: &[Job]) -> Vec<Sprite> {
    let mut sprites: Vec<Option<Sprite>> = vec![None; jobs.len()];
    parallel::chunks_mut(&mut sprites, 1, 1, |i, slot| {
        let j = jobs[i];
        slot[0] = Some(bake_one(j.mesh, j.finish, j.yaw, j.elevation, j.meters, j.pixels_per_meter, j.eye, j.rise));
    });
    sprites.into_iter().map(|s| s.expect("sprite horneado")).collect()
}

/// Hornea `mesh` girada cada angulo de `yaws` (radianes; 0 se ve desde
/// atras, positivo gira la nariz hacia la derecha de la imagen), vista
/// `elevation` radianes desde arriba, a `pixels_per_meter`. `meters` es
/// cuanto mide una unidad del modelo. Los angulos se hornean en paralelo.
#[cfg(test)]
pub fn bake(mesh: &Mesh, finish: Finish, yaws: &[f32], elevation: f32, meters: f32, pixels_per_meter: f32) -> Vec<Sprite> {
    let jobs: Vec<Job> = yaws.iter().map(|&yaw| Job { mesh, finish, yaw, elevation, meters, pixels_per_meter, eye: 0.0, rise: 0.0 }).collect();
    bake_all(&jobs)
}

/// Una nave para dibujarla en 3D de cerca (ver `carrera`): un sprite es una
/// imagen sin perspectiva y, de cerca y a un lado de la camara, una nave
/// larga girada se veia atravesada en la calle. La malla va en metros, con
/// el origen en el suelo bajo su centro y la nariz hacia `-z` (como al
/// hornear).
///
/// Se sombrea por vertice: la luz del horneado solo depende de la normal
/// vista desde su camara (la vista es fija, `+z`), asi que cada vertice se
/// sombrea una vez por frame (`vertex`) y cada pixel interpola su luz
/// lineal y su emision y les pone el tono (`pixel`). Una camioneta pegada
/// a la camara cubre cientos de miles de pixeles y tiene unos 15 000
/// vertices; sus caras son planas, asi que sale igual.
pub struct Solid {
    pub finish: Finish,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<Vec3>,
    pub indices: Vec<[u32; 3]>,
    /// La clase de pieza y su valor de cada vertice (su `uv`; -1 en las
    /// botellas, que no la traen).
    parts: Vec<(f32, f32)>,
    /// Donde empieza su largo (`z` mas chica), su largo y su alto (m).
    z0: f32,
    length: f32,
    pub tall: f32,
}

impl Solid {
    pub fn new(mesh: &Mesh, finish: Finish, meters: f32) -> Self {
        let bottom = mesh_bottom(mesh);
        let positions: Vec<Vec3> = mesh.positions.iter().map(|&p| Vec3::new(p.x, p.y - bottom, p.z) * meters).collect();
        let (lo, hi) = positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
        let parts = (0..positions.len())
            .map(|i| {
                let uv = mesh.uvs.get(i).copied().unwrap_or_default();
                if finish == Finish::Vehicle { (uv.x, uv.y) } else { (-1.0, uv.y) }
            })
            .collect();
        let n = positions.len();
        let fill = |v: &[Vec3], fallback: Vec3| (0..n).map(|i| v.get(i).copied().unwrap_or(fallback)).collect::<Vec<_>>();
        Self {
            finish,
            normals: fill(&mesh.normals, Vec3::Y),
            colors: fill(&mesh.colors, Vec3::splat(0.7)),
            positions,
            indices: mesh.indices.iter().copied().filter(|t| t.iter().all(|&i| (i as usize) < n)).collect(),
            parts,
            z0: lo.z,
            length: (hi.z - lo.z).max(1e-6),
            tall: (hi.y - lo.y).max(1e-6),
        }
    }

    /// El vertice `i` con `n` su normal vista como la ve la camara del
    /// horneado (z hacia ella): su luz lineal, lo que emite (lineal, sin
    /// animar) y su byte de patron y fase (0 si no es LED). Es lo de una
    /// muestra de `bake_one`.
    pub fn vertex(&self, i: usize, n: Vec3) -> (Vec3, Vec3, u8) {
        let (pos, color) = (self.positions[i], self.colors[i]);
        let (class, value) = self.parts[i];
        // (La celda de las burbujas solo la usa la botella.)
        let cell = if self.finish == Finish::Bottle {
            let q = pos * (120.0 / self.length);
            (hash_u32((q.x.floor() as i32 as u32).wrapping_mul(0x9E37_79B9) ^ (q.y.floor() as i32 as u32).wrapping_mul(0x85EB_CA6B) ^ (q.z.floor() as i32 as u32).wrapping_mul(0xC2B2_AE35)) >> 8) as f32 * (1.0 / 16_777_216.0)
        } else {
            1.0
        };
        let inside = Inside { level: pos.y / self.tall, cell };
        let (lit, e) = shade::<Fast>(self.finish, class, value, color, n, inside);
        let tag = if self.finish == Finish::Vehicle && Surface::from_code(class) == Surface::Light { led_tag(color, self.normals[i], pos, self.z0, self.length) } else { 0 };
        (lit, e, tag)
    }
}

/// Un pixel de una nave en 3D (ver `Solid`): de la luz lineal y la emision
/// interpoladas y el byte de su LED, el color con tono y la luz de la LED
/// con tono (sin animar: el juego la multiplica por su tabla), como el
/// cuerpo y el `glow` de un sprite donde la LED llena el pixel (con su
/// nucleo blanco; el halo va aparte).
#[inline]
pub fn pixel(lit: Vec3, e: Vec3, tag: u8) -> (Vec3, Vec3) {
    let base = tone_with::<Fast>(lit);
    if !(e.max_element() > 1e-6) {
        return (base, Vec3::ZERO);
    }
    let led = if tag >> 6 == PATTERN_STOP {
        STOP_RED * (tone_with::<Fast>(lit + e) - base).max_element()
    } else {
        let lum = 0.2126 * e.x + 0.7152 * e.y + 0.0722 * e.z;
        tone_with::<Fast>(lit + e.lerp(Vec3::splat(lum * 1.2), 0.4)) - base
    };
    (base, led.max(Vec3::ZERO))
}

/// Lo que una LED le da a su halo: su luz por lo que brilla su patron en
/// promedio (`HALO_WEIGHT`).
#[inline]
pub fn halo_source(e: Vec3, tag: u8) -> Vec3 {
    e * HALO_WEIGHT[(tag >> 6) as usize & 3]
}

/// El halo de las LEDs de una imagen de `w` x `h` (ver `bake_one`): de la
/// luz de cada LED (`src`, ya por su `HALO_WEIGHT`), cuanto cubre la nave
/// (`alpha`) y donde hay LED (`led`, 0 o 1). Los radios del horneado (2 y
/// 7 pixeles del sprite) van por `radius` (pixeles de esta imagen por
/// pixel del sprite). Devuelve el halo ya con su tono (`tone_vivid`).
pub(crate) fn led_halo(src: &[Vec3], alpha: &[f32], led: &[Vec3], w: usize, h: usize, radius: f32) -> Vec<Vec3> {
    let r = |px: f32| ((px * radius).round() as usize).max(1);
    let mut short = src.to_vec();
    for _ in 0..2 {
        short = box_blur(&short, w, h, r(2.0));
    }
    let mut wide = src.to_vec();
    for _ in 0..3 {
        wide = box_blur(&wide, w, h, r(7.0));
    }
    let density = box_blur(led, w, h, r(7.0));
    (0..w * h)
        .map(|i| {
            let crowd = smoothstep(0.05, 0.3, density[i].x);
            tone_vivid_with::<Fast>(short[i] * (0.55 * (1.0 - 0.3 * crowd)) + wide[i] * (0.08 * (1.0 - 0.8 * crowd) * (1.0 - 0.85 * alpha[i])))
        })
        .collect()
}

/// Lo que el sombreado de la botella necesita saber de la muestra: a que
/// altura del modelo esta (0 abajo, 1 arriba) y un numero al azar de su
/// celda en el modelo (las burbujas).
#[derive(Clone, Copy)]
struct Inside {
    level: f32,
    cell: f32,
}

fn bake_one(mesh: &Mesh, finish: Finish, yaw: f32, elevation: f32, meters: f32, ppm: f32, eye: f32, rise: f32) -> Sprite {
    // Giro de la nave (alrededor de y) y despues la camara que mira desde
    // arriba (alrededor de x).
    let (sy, cy) = yaw.sin_cos();
    let (se, ce) = elevation.sin_cos();
    let rotate = |p: Vec3| {
        // La nariz (-z) gira hacia +x con yaw positivo.
        let x = p.x * cy - p.z * sy;
        let z = p.x * sy + p.z * cy;
        // Inclinar la vista: la camara sube, el modelo se ve desde arriba.
        let y2 = p.y * ce - z * se;
        let z2 = p.y * se + z * ce;
        Vec3::new(x, y2, z2)
    };
    let k = meters * ppm * SUPER as f32;
    // La perspectiva (la propia, de cerca): la cola (lo mas cerca de la
    // camara) queda como esta y lo que esta `dz` metros mas adelante se
    // achica `eye / (eye + dz)` hacia el punto de fuga, `rise` metros arriba
    // del centro (el horizonte). Sin esto una nave larga vista de cerca se
    // veia del mismo ancho adelante que atras, y el ojo la leia agrandada de
    // adelante. La profundidad (`z`) no cambia: solo ordena.
    let (blo, bhi) = mesh.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let center = rotate((blo + bhi) * 0.5);
    let tail = mesh.positions.iter().map(|&p| rotate(p).z).fold(f32::MIN, f32::max);
    let vanish = (center.x, center.y + rise / meters.max(1e-6));
    let project = |p: Vec3| {
        let q = rotate(p);
        if eye <= 0.0 {
            return q;
        }
        let f = eye / (eye + (tail - q.z).max(0.0) * meters);
        Vec3::new(vanish.0 + (q.x - vanish.0) * f, vanish.1 + (q.y - vanish.1) * f, q.z)
    };
    let projected: Vec<Vec3> = mesh.positions.iter().map(|&p| project(p)).collect();
    let (lo, hi) = projected.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    // Las medidas del modelo sin girar: el largo para la fase de las LEDs y
    // el alto para la cerveza de la botella.
    let (mlo, mhi) = mesh.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let length = (mhi.z - mlo.z).max(1e-6);
    let tall = (mhi.y - mlo.y).max(1e-6);
    // Margen para el resplandor de las LEDs.
    let margin = (MARGIN * SUPER) as f32;
    let width_s = (((hi.x - lo.x) * k + 2.0 * margin).ceil().max(0.0) as usize).div_ceil(SUPER) * SUPER;
    let height_s = (((hi.y - lo.y) * k + 2.0 * margin).ceil().max(0.0) as usize).div_ceil(SUPER) * SUPER;
    let to_image = |p: Vec3| ((p.x - lo.x) * k + margin, (hi.y - p.y) * k + margin);
    // El suelo bajo el centro: el punto mas bajo del modelo, en x = 0.
    // (Sin perspectiva: el ancla es donde la nave se para en la pista, y
    // la cola, que no cambia, queda donde estaba.)
    let ground = rotate(Vec3::new(0.0, mesh_bottom(mesh), 0.0));
    let anchor_s = to_image(ground);
    // Cada vertice en la imagen: x, y en muestras y profundidad (mayor es
    // mas cerca).
    let screen: Vec<(f32, f32, f32)> = projected
        .iter()
        .map(|&p| {
            let (x, y) = to_image(p);
            (x, y, p.z)
        })
        .collect();

    // El raster: por muestra, la profundidad, el triangulo mas cercano y sus
    // pesos baricentricos (el tercero es `1 - wa - wb`).
    let samples = width_s * height_s;
    let mut depth = vec![f32::MIN; samples];
    let mut hit = vec![(u32::MAX, 0.0f32, 0.0f32); samples];
    for (t, tri) in mesh.indices.iter().enumerate() {
        let [a, b, c] = tri.map(|i| screen.get(i as usize).copied().unwrap_or((0.0, 0.0, f32::MIN)));
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if !(area.abs() >= 1e-6) {
            continue;
        }
        let x0 = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
        let y0 = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
        let x1 = (a.0.max(b.0).max(c.0).ceil().max(0.0) as usize).min(width_s);
        let y1 = (a.1.max(b.1).max(c.1).ceil().max(0.0) as usize).min(height_s);
        let inv = 1.0 / area;
        for y in y0..y1 {
            let py = y as f32 + 0.5;
            for x in x0..x1 {
                let px = x as f32 + 0.5;
                // Funciones de borde: el peso de cada vertice.
                let wa = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) * inv;
                let wb = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) * inv;
                let wc = 1.0 - wa - wb;
                if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                    continue;
                }
                let z = a.2 * wa + b.2 * wb + c.2 * wc;
                let s = y * width_s + x;
                if z <= depth[s] {
                    continue;
                }
                depth[s] = z;
                hit[s] = (t as u32, wa, wb);
            }
        }
    }
    // Cada muestra cubierta se sombrea una vez: lo que se necesita se
    // interpola con sus pesos. Las LEDs guardan ademas su patron y fase.
    let mut lit = vec![Vec3::ZERO; samples];
    let mut glow = vec![Vec3::ZERO; samples];
    let mut tag = vec![None::<u8>; samples];
    for s in 0..samples {
        let (t, wa, wb) = hit[s];
        let Some(tri) = mesh.indices.get(t as usize) else { continue };
        let [ia, ib, ic] = tri.map(|i| i as usize);
        let wc = 1.0 - wa - wb;
        let mix = |v: &[Vec3], fallback: Vec3| {
            let at = |i: usize| v.get(i).copied().unwrap_or(fallback);
            at(ia) * wa + at(ib) * wb + at(ic) * wc
        };
        let model_normal = mix(&mesh.normals, Vec3::Y);
        let mut normal = rotate(model_normal);
        normal = if normal.length_squared() > 1e-12 { normal.normalize() } else { Vec3::Z };
        // Caras que dan la espalda (mallas abiertas): la normal hacia la
        // camara.
        if normal.z < 0.0 {
            normal = -normal;
        }
        let color = mix(&mesh.colors, Vec3::splat(0.7));
        let pos = mix(&mesh.positions, Vec3::ZERO);
        let uv = mesh.uvs.get(ia).copied().unwrap_or_default();
        let class = if finish == Finish::Vehicle { uv.x } else { -1.0 };
        // La celda de 1/120 del largo donde cae la muestra: un numero al
        // azar por celda (las burbujas de la cerveza).
        let q = pos * (120.0 / length);
        let cell = (hash_u32((q.x.floor() as i32 as u32).wrapping_mul(0x9E37_79B9) ^ (q.y.floor() as i32 as u32).wrapping_mul(0x85EB_CA6B) ^ (q.z.floor() as i32 as u32).wrapping_mul(0xC2B2_AE35)) >> 8) as f32
            * (1.0 / 16_777_216.0);
        let inside = Inside { level: (pos.y - mlo.y) / tall, cell };
        let (surface, emission) = shade::<Exact>(finish, class, uv.y, color, normal, inside);
        lit[s] = surface;
        glow[s] = emission;
        if finish == Finish::Vehicle && Surface::from_code(class) == Surface::Light {
            tag[s] = Some(led_tag(color, model_normal, pos, mlo.z, length));
        }
    }

    // Cada pixel promedia sus muestras: el color, el alfa, la luz de las
    // LEDs y que fraccion de el es LED; y el patron de la muestra de LED que
    // mas brilla.
    let (width, height) = (width_s / SUPER, height_s / SUPER);
    let n = width * height;
    let mut color = vec![Vec3::ZERO; n];
    let mut alpha = vec![0.0f32; n];
    let mut light = vec![Vec3::ZERO; n];
    let mut mask = vec![Vec3::ZERO; n];
    let mut tags: Vec<Option<(u8, f32)>> = vec![None; n];
    let per = 1.0 / (SUPER * SUPER) as f32;
    for y in 0..height {
        for x in 0..width {
            let (mut c, mut a, mut g, mut leds) = (Vec3::ZERO, 0.0, Vec3::ZERO, 0.0);
            let mut best: Option<(u8, f32)> = None;
            for sy in 0..SUPER {
                for sx in 0..SUPER {
                    let s = (y * SUPER + sy) * width_s + x * SUPER + sx;
                    if hit[s].0 != u32::MAX {
                        c += lit[s];
                        a += 1.0;
                        g += glow[s];
                    }
                    if let Some(t) = tag[s] {
                        leds += 1.0;
                        let v = glow[s].max_element();
                        if best.is_none_or(|(_, b)| v > b) {
                            best = Some((t, v));
                        }
                    }
                }
            }
            let i = y * width + x;
            color[i] = c * per;
            alpha[i] = a * per;
            light[i] = g * per;
            mask[i] = Vec3::splat(leds * per);
            tags[i] = best;
        }
    }
    let any_led = light.iter().any(|l| l.max_element() > 0.0);
    // La luz de las LEDs con su cobertura (`E = luz alfa`) y el nucleo: donde
    // la LED llena el pixel y sus vecinos (el promedio de 3 x 3 de la
    // fraccion de LED), `core = smoothstep(0.5, 1, m)`, se va hacia un
    // blanco de su misma luminancia, `E' = lerp(E, 1.2 lum(E), 0.4 core)`:
    // el centro de una LED fuerte satura a blanco como en una foto, y la
    // orilla conserva el color. (Con 0.65 el nucleo velaba la laca de
    // alrededor; con `smoothstep(0.75, 1, m)` el foco quedaba de un pixel.)
    // Los stops no: van rojos, `(1, 0.15, 0.1)` por el maximo de su luz,
    // asi al frenar pican hacia rojo y no hacia blanco rosado (el tono con
    // gamma sube los canales chicos: un rojo fuerte salia rosa).
    let emit: Vec<Vec3> = (0..n).map(|i| light[i] * alpha[i]).collect();
    let glow_px: Vec<[u8; 4]> = if any_led {
        let core = box_blur(&mask, width, height, 1);
        // El halo no se anima (si tomara el patron de la LED mas cercana,
        // donde se tocan los halos de dos LEDs con fases distintas saldria
        // una costura): sale de la luz de cada LED por lo que brilla su
        // patron en promedio (`HALO_WEIGHT`). Dos escalas: uno corto (caja
        // de radio 2, dos pasadas) y uno ancho (radio 7, tres: casi una
        // gaussiana).
        let halo_src: Vec<Vec3> = (0..n).map(|i| emit[i] * tags[i].map_or(1.0, |(t, _)| HALO_WEIGHT[(t >> 6) as usize & 3])).collect();
        let mut short = halo_src.clone();
        for _ in 0..2 {
            short = box_blur(&short, width, height, 2);
        }
        let mut wide = halo_src;
        for _ in 0..3 {
            wide = box_blur(&wide, width, height, 7);
        }
        // Donde las LEDs se amontonan (las columnas de focos de la
        // camioneta) los halos se suman en un velo que lava la laca: con la
        // densidad (la fraccion de pixeles con LED en 15 x 15) el halo baja,
        // `1 - 0.8 smoothstep(0.05, 0.3, densidad)` el ancho y
        // `1 - 0.3 smoothstep(...)` el corto. El ancho ademas florece afuera
        // de la silueta y no encima de la lamina, `1 - 0.85 alfa`: encima
        // aclaraba el azul marino del techo a gris lavanda.
        let lit: Vec<Vec3> = mask.iter().map(|m| Vec3::splat(if m.x > 0.0 { 1.0 } else { 0.0 })).collect();
        let density = box_blur(&lit, width, height, 7);
        (0..n)
            .map(|i| {
                let e = emit[i];
                let pattern = tags[i].map_or(PATTERN_STATIC, |(t, _)| t >> 6);
                let base = tone(color[i]);
                let led = if pattern == PATTERN_STOP {
                    STOP_RED * (tone(color[i] + e) - base).max_element()
                } else {
                    let lum = 0.2126 * e.x + 0.7152 * e.y + 0.0722 * e.z;
                    tone(color[i] + e.lerp(Vec3::splat(lum * 1.2), 0.4 * smoothstep(0.5, 1.0, core[i].x))) - base
                };
                let crowd = smoothstep(0.05, 0.3, density[i].x);
                let halo = short[i] * (0.55 * (1.0 - 0.3 * crowd)) + wide[i] * (0.08 * (1.0 - 0.8 * crowd) * (1.0 - 0.85 * alpha[i]));
                // El halo se apaga a 6 pixeles de la orilla de la imagen
                // (el ancho llega a ~21 y el margen es de 16): sin esto se
                // cortaba en escalones sobre el cielo.
                let (x, y) = (i % width, i / width);
                let edge = x.min(y).min(width - 1 - x).min(height - 1 - y) as f32;
                let g = led + tone_vivid(halo) * smoothstep(0.0, 6.0, edge);
                // Lo de menos de 2/255 se resta (no se corta): asi el halo
                // llega a cero sin escalon donde termina el tramo de cada
                // fila (`spans_of`).
                let cut = |v: f32| ((v - 2.0 / 255.0) * (255.0 / 253.0)).max(0.0);
                let t = tags[i].map_or(0, |(t, _)| t);
                [byte(cut(g.x)), byte(cut(g.y)), byte(cut(g.z)), t]
            })
            .collect()
    } else {
        Vec::new()
    };
    let pixels: Vec<[u8; 4]> = (0..n)
        .map(|i| {
            // Premultiplicado: el color de la nave ya trae su cobertura.
            let c = tone(color[i]);
            [byte(c.x), byte(c.y), byte(c.z), byte(alpha[i])]
        })
        .collect();
    let spans = spans_of(&pixels, &glow_px, width, height);
    let solid = solid_of(&pixels, width, height);
    // Los dos juntos para el juego (ver `Sprite::texels`).
    let texels: Vec<[u8; 8]> = (0..n)
        .map(|i| {
            let (p, g) = (pixels[i], glow_px.get(i).copied().unwrap_or([0; 4]));
            [p[0], p[1], p[2], p[3], g[0], g[1], g[2], g[3]]
        })
        .collect();
    let levels = mips(&texels, width, height);
    Sprite {
        width,
        height,
        pixels,
        glow: glow_px,
        texels,
        spans,
        solid,
        anchor: (anchor_s.0 / SUPER as f32, anchor_s.1 / SUPER as f32),
        scale: ppm,
        levels,
    }
}

/// Por fila `y0`, el tramo mas largo `[desde, hasta)` de columnas `x0`
/// donde los cuatro pixeles que lee el bilineal son opacos (ver
/// `Sprite::solid`).
fn solid_of(pixels: &[[u8; 4]], width: usize, height: usize) -> Vec<(u16, u16)> {
    let opaque = |x: usize, y: usize| pixels.get(y * width + x).is_some_and(|p| p[3] == 255);
    (0..height)
        .map(|y| {
            let (mut best, mut start) = ((0u16, 0u16), None);
            for x in 0..=width.saturating_sub(1) {
                let full = x + 1 < width && y + 1 < height && opaque(x, y) && opaque(x + 1, y) && opaque(x, y + 1) && opaque(x + 1, y + 1);
                match (full, start) {
                    (true, None) => start = Some(x),
                    (false, Some(a)) => {
                        if x - a > (best.1 - best.0) as usize {
                            best = (a.min(u16::MAX as usize) as u16, x.min(u16::MAX as usize) as u16);
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
            best
        })
        .collect()
}

/// Por fila, el tramo de columnas `[desde, hasta)` con algo: color, alfa o
/// luz de LEDs (a la luz ya se le resto 2/255 al hornear: la cola del halo
/// ancho, que no se ve, alargaria cada fila).
fn spans_of(pixels: &[[u8; 4]], glow: &[[u8; 4]], width: usize, height: usize) -> Vec<(u16, u16)> {
    (0..height)
        .map(|y| {
            let any = |x: usize| {
                let i = y * width + x;
                pixels.get(i).is_some_and(|p| p.iter().any(|&v| v != 0)) || glow.get(i).is_some_and(|g| g[0].max(g[1]).max(g[2]) > 0)
            };
            match ((0..width).find(|&x| any(x)), (0..width).rev().find(|&x| any(x))) {
                (Some(a), Some(b)) => (a.min(u16::MAX as usize) as u16, (b + 1).min(u16::MAX as usize) as u16),
                _ => (0, 0),
            }
        })
        .collect()
}

/// Los mips: cada nivel promedia 2 x 2 del anterior, premultiplicado (lo
/// que cae afuera de una imagen impar cuenta como transparente), hasta que
/// un lado mide menos de `MIP_MIN`. La luz de las LEDs se reduce con
/// `0.5 promedio + 0.5 maximo`: un foco de un pixel no se apaga a un cuarto
/// en cada nivel (de lejos la nave sigue destellando), y su patron es el
/// del pixel que mas brilla.
fn mips(texels: &[[u8; 8]], width: usize, height: usize) -> Vec<Level> {
    let mut levels: Vec<Level> = Vec::new();
    let (mut w, mut h) = (width, height);
    let mut tx = texels.to_vec();
    while w >= MIP_MIN && h >= MIP_MIN {
        let (nw, nh) = (w.div_ceil(2), h.div_ceil(2));
        let at = |buf: &[[u8; 8]], x: usize, y: usize| if x < w && y < h { buf.get(y * w + x).copied().unwrap_or([0; 8]) } else { [0; 8] };
        let mut next = vec![[0u8; 8]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let quad = [at(&tx, 2 * x, 2 * y), at(&tx, 2 * x + 1, 2 * y), at(&tx, 2 * x, 2 * y + 1), at(&tx, 2 * x + 1, 2 * y + 1)];
                let mut out = [0u8; 8];
                for k in 0..4 {
                    let sum: u32 = quad.iter().map(|t| t[k] as u32).sum();
                    out[k] = ((sum + 2) / 4) as u8;
                }
                let mut top = 0u8;
                for t in &quad {
                    let v = t[4].max(t[5]).max(t[6]);
                    if v > top {
                        top = v;
                        out[7] = t[7];
                    }
                }
                for k in 4..7 {
                    let sum: u32 = quad.iter().map(|t| t[k] as u32).sum();
                    let max = quad.iter().map(|t| t[k]).max().unwrap_or(0) as f32;
                    out[k] = (sum as f32 * 0.125 + max * 0.5 + 0.5).min(255.0) as u8;
                }
                next[y * nw + x] = out;
            }
        }
        let body: Vec<[u8; 4]> = next.iter().map(|t| [t[0], t[1], t[2], t[3]]).collect();
        let glow: Vec<[u8; 4]> = next.iter().map(|t| [t[4], t[5], t[6], t[7]]).collect();
        let spans = spans_of(&body, &glow, nw, nh);
        let solid = solid_of(&body, nw, nh);
        levels.push(Level { w: nw, h: nh, texels: next.clone(), spans, solid });
        tx = next;
        w = nw;
        h = nh;
    }
    levels
}

/// El patron y la fase de una LED (el byte de alfa del `glow`): el patron
/// por su color, normalizado por su maximo, y su orientacion en el modelo
/// (`PATTERN_*`):
///
/// - ambar (`r > 0.8`, `g` de 0.35 a 0.75 y `b < 0.3`): persecucion;
/// - magenta, cian o azul: respira;
/// - rojo (`g` y `b` bajo 0.25) mirando hacia atras (la normal del modelo
///   con `z > 0.3`; la nariz es `-z`): stop;
/// - lo demas (blancas, verdes, rojas de los lados): fija.
///
/// La fase es `fract(6 z / largo + 0.25 signo(x)) 63`: seis vueltas a lo
/// largo de la nave, y los dos lados corridos un cuarto (la persecucion
/// zigzaguea de un lado al otro).
fn led_tag(color: Vec3, model_normal: Vec3, pos: Vec3, z0: f32, length: f32) -> u8 {
    let m = color.max_element();
    let pattern = if !(m > 1e-4) {
        PATTERN_STATIC
    } else {
        let (r, g, b) = (color.x / m, color.y / m, color.z / m);
        if r > 0.8 && (0.35..=0.75).contains(&g) && b < 0.3 {
            PATTERN_CHASE
        } else if g < 0.25 && b < 0.25 {
            if model_normal.z > 0.3 { PATTERN_STOP } else { PATTERN_STATIC }
        } else if (r > 0.5 && b > 0.5 && g < 0.45) || (g > 0.5 && b > 0.5 && r < 0.45) || (b > 0.9 && r < 0.5 && g < 0.6) {
            PATTERN_BREATHE
        } else {
            PATTERN_STATIC
        }
    };
    let side = if pos.x > 0.0 { 0.25 } else if pos.x < 0.0 { -0.25 } else { 0.0 };
    let f = (pos.z - z0) / length * 6.0 + side;
    let phase = ((f - f.floor()) * 63.0).round().clamp(0.0, 63.0) as u8;
    (pattern << 6) | phase
}

/// La altura del punto mas bajo del modelo.
fn mesh_bottom(mesh: &Mesh) -> f32 {
    mesh.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min)
}

#[inline]
fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Mapeo de tonos y gamma: de luz lineal a lo que se ve.
#[inline]
fn tone(c: Vec3) -> Vec3 {
    tone_with::<Exact>(c)
}

/// Las cuentas caras del sombreado: exactas (al hornear, una vez) o
/// rapidas (las naves en 3D de cada frame, `math::fast_pow`, error de unos
/// 3e-4: no se distingue en 8 bits).
pub(crate) trait Maths {
    fn exp(x: f32) -> f32;
    fn pow(x: f32, p: f32) -> f32;
    /// La curva de `tone` en un canal.
    fn tone(v: f32) -> f32 {
        Self::pow(1.0 - Self::exp(-v.max(0.0) * 1.4), 1.0 / 2.2)
    }
}

pub(crate) struct Exact;
pub(crate) struct Fast;

impl Maths for Exact {
    #[inline(always)]
    fn exp(x: f32) -> f32 {
        x.exp()
    }
    #[inline(always)]
    fn pow(x: f32, p: f32) -> f32 {
        x.powf(p)
    }
}

impl Maths for Fast {
    #[inline(always)]
    fn exp(x: f32) -> f32 {
        fast_exp(x)
    }
    #[inline(always)]
    fn pow(x: f32, p: f32) -> f32 {
        fast_pow(x, p)
    }
    /// De una tabla indexada con los bits del `f32` (el exponente y los 7
    /// bits altos de la mantisa: tramos de 1/128 relativo de `2^-16` a
    /// `2^5`, con el mismo detalle en lo oscuro que en lo claro), en el
    /// centro de cada tramo. Menos de un paso de 8 bits de error.
    #[inline(always)]
    fn tone(v: f32) -> f32 {
        const FIRST: i32 = 111 << 7;
        const SIZE: usize = 21 << 7;
        static LUT: OnceLock<Vec<f32>> = OnceLock::new();
        let lut = LUT.get_or_init(|| {
            (0..SIZE)
                .map(|i| if i == 0 { 0.0 } else { Exact::tone(f32::from_bits((((i as i32 + FIRST) as u32) << 16) | 0x8000)) })
                .collect()
        });
        let i = ((v.max(0.0).to_bits() >> 16) as i32 - FIRST).clamp(0, SIZE as i32 - 1);
        lut[i as usize]
    }
}

/// `tone` con las cuentas de `M`.
#[inline]
fn tone_with<M: Maths>(c: Vec3) -> Vec3 {
    Vec3::new(M::tone(c.x), M::tone(c.y), M::tone(c.z))
}

/// El tono de un halo conservando su color: el del maximo, `tone(max)`,
/// por el color normalizado con una gamma suave, `(c / max)^0.6`. Con el
/// tono por canal la gamma sube los canales chicos y un halo tenue sale
/// casi blanco (el magenta, lavanda; sobre el azul de la camioneta, un velo
/// lila).
#[inline]
fn tone_vivid(c: Vec3) -> Vec3 {
    tone_vivid_with::<Exact>(c)
}

/// `tone_vivid` con las cuentas de `M`.
#[inline]
fn tone_vivid_with<M: Maths>(c: Vec3) -> Vec3 {
    let top = c.max_element();
    if !(top > 1e-6) {
        return Vec3::ZERO;
    }
    let t = M::tone(top);
    let r = |v: f32| M::pow(v.max(0.0) / top, 0.6);
    Vec3::new(r(c.x), r(c.y), r(c.z)) * t
}

/// El rojo de los stops (ver `bake_one`).
const STOP_RED: Vec3 = Vec3::new(1.0, 0.15, 0.1);

/// Cuanto pesa en el halo (que no se anima) la luz de cada patron: lo que
/// brilla en promedio (`PATTERN_*`: fija, persecucion, respira y stop sin
/// frenar).
const HALO_WEIGHT: [f32; 4] = [1.0, 0.75, 0.85, 0.6];

/// La luz del horneado (en los ejes de la imagen: x a la derecha, y arriba,
/// z hacia la camara): la luna fria desde arriba a la izquierda y la luz
/// naranja de los postes desde abajo y adelante.
const MOON: Vec3 = Vec3::new(-0.45, 0.75, 0.5);
const MOON_LIGHT: Vec3 = Vec3::new(0.55, 0.62, 0.8);
const LAMP: Vec3 = Vec3::new(0.35, -0.2, 0.9);
const LAMP_LIGHT: Vec3 = Vec3::new(0.9, 0.5, 0.18);
const AMBIENT: Vec3 = Vec3::new(0.05, 0.055, 0.08);

/// El cielo de noche que se refleja: oscuro arriba, el resplandor naranja
/// de la ciudad abajo y una franja tenue de luz de los postes. (Con la
/// franja a 0.6 los vidrios, que la reflejan de lleno, salian color crema.)
fn environment<M: Maths>(r: Vec3) -> Vec3 {
    let up = r.y;
    let sky = Vec3::new(0.04, 0.05, 0.1).lerp(Vec3::new(0.25, 0.14, 0.08), ((-up) * 2.0).clamp(0.0, 1.0));
    let band = M::exp(-((up - 0.15) / 0.08).powi(2));
    sky + Vec3::new(1.0, 0.7, 0.4) * (0.12 * band)
}

/// Si un color del `.mtl` de una botella es su vidrio cafe (no traen la
/// clase en la `uv`): rojizo (`r > 1.4 g`), mas verde que azul
/// (`g > 1.3 b`) y oscuro (luminancia bajo 0.12: la tinta cafe de la
/// etiqueta, de 0.2, no es vidrio).
fn amber_glass(c: Vec3) -> bool {
    c.x > 1.4 * c.y && c.y > 1.3 * c.z && c.luminance() < 0.12
}

/// El color de una muestra y la luz que emite (aparte, para el resplandor).
fn shade<M: Maths>(finish: Finish, class: f32, value: f32, albedo: Vec3, n: Vec3, inside: Inside) -> (Vec3, Vec3) {
    let v = Vec3::Z;
    let r = n * (2.0 * n.dot(v)) - v;
    let fresnel = 0.04 + 0.96 * (1.0 - n.dot(v).max(0.0)).powi(5);
    // Las direcciones de las luces y sus medios vectores con la vista (que
    // es fija, `+z`), una vez.
    static LIGHTS: OnceLock<[Vec3; 4]> = OnceLock::new();
    let [moon, lamp, half_moon, half_lamp] = *LIGHTS.get_or_init(|| {
        let (m, l) = (MOON.normalize(), LAMP.normalize());
        [m, l, (m + Vec3::Z).normalize(), (l + Vec3::Z).normalize()]
    });
    let diffuse = |albedo: Vec3| albedo * (AMBIENT + MOON_LIGHT * n.dot(moon).max(0.0) + LAMP_LIGHT * n.dot(lamp).max(0.0) * 0.6);
    let specular = |rough: f32| {
        let power = 2.0 / (rough * rough).max(0.003) - 2.0;
        let blinn = |h: Vec3| M::pow(n.dot(h).max(0.0), power) * (power + 8.0) / 25.0;
        MOON_LIGHT * blinn(half_moon) * 0.5 + LAMP_LIGHT * blinn(half_lamp) * 0.4
    };
    if finish == Finish::Bottle {
        // (Las botellas no traen la clase de pieza en la `uv`: el vidrio
        // sale solo por su color, `amber_glass`.)
        if amber_glass(albedo) {
            // Vidrio ambar a contraluz: casi nada de difuso, la luz que lo
            // atraviesa en las orillas (donde el rayo cruza mas vidrio):
            // `1.28 smoothstep(0.6, 1, 1 - n.z)^2` pegado a la silueta (con
            // `(1 - n.z)^2` todo el costado, que desde atras se ve de
            // canto, salia dorado como plastico: con la gamma, 0.15 de luz
            // ya es un tono medio) y hacia adentro, `0.05 (1 - n.z)`, el
            // cafe oscuro del vidrio. De frente (el fondo de la botella, que
            // mira a la camara) pasa la luz de la cerveza,
            // `(1, 0.55, 0.15) 0.25 n.z`: si no, el fondo era un disco negro.
            // Mas el cielo con Fresnel, tenido de ambar por el vidrio (sin
            // tenir, el cielo violeta sobre el cafe lo dejaba verde oliva),
            // y una veta de brillo larga (la luz de un poste que se refleja
            // en el cilindro, `smoothstep(0.93, 0.99, n . l)`). Abajo del
            // 60 % del alto va la cerveza: el contraluz pasa menos y flotan
            // burbujas (una celda de cada 250 da un punto blanco).
            let rim = (1.0 - n.z).max(0.0);
            let edge = smoothstep(0.6, 1.0, rim);
            let beer = inside.level < 0.6;
            let amber = Vec3::new(1.0, 0.55, 0.15);
            let back = amber * ((1.28 * edge * edge + 0.05 * rim + 0.25 * n.z.max(0.0)) * if beer { 0.7 } else { 1.0 });
            let streak = smoothstep(0.93, 0.99, n.dot(Vec3::new(-0.55, 0.15, 0.82).normalize()));
            let bubble = if beer && inside.cell < 0.004 { 0.6 } else { 0.0 };
            let sky = environment::<M>(r) * Vec3::new(1.0, 0.75, 0.45) * (fresnel * 0.8);
            let c = diffuse(albedo) * 0.15 + back + sky + Vec3::new(1.0, 0.9, 0.75) * (streak * 0.9) + Vec3::splat(bubble);
            return (c, Vec3::ZERO);
        }
        // La etiqueta y la tapa: pintura con un especular bajo.
        return (diffuse(albedo) * 1.3 + specular(0.3) * 0.3 + environment::<M>(r) * (fresnel * 0.6), Vec3::ZERO);
    }
    match Surface::from_code(class) {
        Surface::Light => (albedo * 0.2, albedo * value),
        Surface::Metal => {
            let rough = (value * 0.7).max(0.14);
            (albedo * (environment::<M>(r) * 1.4 + specular(rough) * 1.5), Vec3::ZERO)
        }
        // El vidrio: el cielo con Fresnel y un brillo ancho y tenue. (Con un
        // especular fino y fuerte, `specular(0.08) 0.5`, un vidrio plano
        // de cara a la luz de los postes, que aqui es una direccion y no un
        // punto, se encendia entero: la ventana de atras del colectivo
        // salia color crema.)
        Surface::Glass => (albedo * 0.5 + environment::<M>(r) * (0.15 + 0.85 * fresnel) + specular(0.3) * 0.12, Vec3::ZERO),
        Surface::Rubber => (diffuse(albedo), Vec3::ZERO),
        Surface::Paint => {
            let rough = value.clamp(0.2, 0.8);
            let gloss = ((0.5 - rough) / 0.2).clamp(0.0, 1.0);
            let base = diffuse(albedo) * 1.6 + specular(rough) * albedo.max(Vec3::splat(0.04));
            if gloss <= 0.0 {
                return (base, Vec3::ZERO);
            }
            (base + (environment::<M>(r) * fresnel + specular(0.1) * 0.3) * gloss, Vec3::ZERO)
        }
    }
}

/// Promedio corrido de `2 * radius + 1` pixeles por filas y por columnas
/// (el borde se repite). Con una suma que corre: al pasar al pixel
/// siguiente entra uno y sale otro, asi cuesta lo mismo con cualquier
/// radio (los barriletes de 512 lo usan con radio 10).
pub(crate) fn box_blur(image: &[Vec3], width: usize, height: usize, radius: usize) -> Vec<Vec3> {
    if width == 0 || height == 0 || image.len() < width * height {
        return image.to_vec();
    }
    let r = radius as isize;
    let scale = 1.0 / (2 * radius + 1) as f32;
    let mut rows = vec![Vec3::ZERO; image.len()];
    for y in 0..height {
        let line = &image[y * width..(y + 1) * width];
        let at = |i: isize| line[i.clamp(0, width as isize - 1) as usize];
        let mut sum = Vec3::ZERO;
        for k in -r..=r {
            sum = sum + at(k);
        }
        for x in 0..width as isize {
            rows[y * width + x as usize] = sum * scale;
            sum = sum + at(x + r + 1) - at(x - r);
        }
    }
    let mut out = vec![Vec3::ZERO; image.len()];
    let row = |y: isize| y.clamp(0, height as isize - 1) as usize * width;
    let mut sums = vec![Vec3::ZERO; width];
    for k in -r..=r {
        let base = row(k);
        for (s, &v) in sums.iter_mut().zip(&rows[base..base + width]) {
            *s = *s + v;
        }
    }
    for y in 0..height as isize {
        let (enter, leave) = (row(y + r + 1), row(y - r));
        for x in 0..width {
            out[y as usize * width + x] = sums[x] * scale;
            sums[x] = sums[x] + rows[enter + x] - rows[leave + x];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec2;

    /// Una caja de 4 x 1 x 2 (largo en z) con una LED roja atras.
    fn crate_mesh() -> Mesh {
        let mut mesh = Mesh::default();
        let corners = [
            Vec3::new(-0.5, 0.0, -2.0),
            Vec3::new(0.5, 0.0, -2.0),
            Vec3::new(0.5, 1.0, -2.0),
            Vec3::new(-0.5, 1.0, -2.0),
            Vec3::new(-0.5, 0.0, 2.0),
            Vec3::new(0.5, 0.0, 2.0),
            Vec3::new(0.5, 1.0, 2.0),
            Vec3::new(-0.5, 1.0, 2.0),
        ];
        for (i, &p) in corners.iter().enumerate() {
            mesh.positions.push(p);
            mesh.normals.push((p - Vec3::new(0.0, 0.5, 0.0)).normalize());
            mesh.colors.push(if i >= 4 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.2, 0.4, 0.9) });
            mesh.uvs.push(if i >= 4 { Vec2::new(3.0, 2.0) } else { Vec2::new(0.0, 0.3) });
        }
        let faces = [[0, 1, 2], [0, 2, 3], [4, 6, 5], [4, 7, 6], [0, 4, 5], [0, 5, 1], [3, 2, 6], [3, 6, 7], [0, 3, 7], [0, 7, 4], [1, 5, 6], [1, 6, 2]];
        mesh.indices = faces.to_vec();
        mesh
    }

    #[test]
    fn a_box_seen_from_behind_and_from_the_side() {
        let mesh = crate_mesh();
        let sprites = bake(&mesh, Finish::Vehicle, &[0.0, std::f32::consts::FRAC_PI_2], 0.0, 1.0, 20.0);
        let (back, side) = (&sprites[0], &sprites[1]);
        let margin = 2.0 * MARGIN as f32;
        // De atras mide 1 de ancho; de lado, 4 (mas el margen del resplandor).
        assert!((back.width as f32 - (20.0 + margin)).abs() <= 2.0, "{}", back.width);
        assert!((side.width as f32 - (80.0 + margin)).abs() <= 2.0, "{}", side.width);
        // El centro es opaco, la orilla del margen transparente.
        let center_i = (back.height / 2) * back.width + back.width / 2;
        let center = back.pixels[center_i];
        assert_eq!(center[3], 255);
        assert_eq!(back.pixels[0][3], 0);
        // De atras se ve la cara con la LED roja (z = +2, hacia la camara):
        // la luz va en el `glow`, roja encendida con su nucleo (que se va
        // hacia blanco, pero sigue roja), y el resplandor sale del contorno
        // (sin alfa).
        let g = back.glow[center_i];
        assert!(center[0] as u32 + g[0] as u32 > 200 && g[0] as u32 > g[2] as u32 + 60, "{center:?} {g:?}");
        let outside = (back.height / 2) * back.width + MARGIN / 2;
        assert!(back.glow[outside][0] > 5 && back.pixels[outside][3] == 0, "{:?}", back.glow[outside]);
        // La cara de atras es roja y mira hacia atras: stop.
        assert_eq!(g[3] >> 6, PATTERN_STOP);
        // El suelo bajo el centro: abajo al centro.
        assert!((back.anchor.0 - back.width as f32 / 2.0).abs() < 1.5);
        assert!((back.anchor.1 - (back.height as f32 - MARGIN as f32)).abs() < 1.5);
        // Los mips bajan a la mitad hasta menos de 8 y conservan el alfa
        // promedio (premultiplicado).
        assert!(!side.levels.is_empty());
        let mut w = side.width;
        for lv in &side.levels {
            assert_eq!(lv.w, w.div_ceil(2));
            assert_eq!(lv.texels.len(), lv.w * lv.h);
            assert_eq!(lv.spans.len(), lv.h);
            w = lv.w;
        }
        let last = side.levels.last().map_or(side.width, |l| l.w.min(l.h));
        assert!(last < MIP_MIN, "{last}");
        let mean = |p: &[[u8; 4]]| p.iter().map(|p| p[3] as f32).sum::<f32>() / p.len() as f32;
        let l1 = &side.levels[0];
        let l1_body: Vec<[u8; 4]> = l1.texels.iter().map(|t| [t[0], t[1], t[2], t[3]]).collect();
        let want = mean(&side.pixels) * (side.width * side.height) as f32 / (l1.w * l1.h * 4) as f32;
        assert!((mean(&l1_body) - want).abs() < 1.5, "{} {want}", mean(&l1_body));
        // El trilineal da lo mismo que el bilineal en lod 0, y entre dos
        // niveles queda entre los dos.
        let (x, y) = (side.width as f32 * 0.5 + 0.3, side.height as f32 * 0.5 + 0.2);
        let (t0, s0) = (side.sample_lod(x, y, 0.0, &LUT_ONE), side.sample(x, y));
        for k in 0..4 {
            assert!((t0.body[k] - s0[k]).abs() < 1.0 / 255.0, "{t0:?} {s0:?}");
        }
        let (a, b, m) = (side.sample_lod(x, y, 1.0, &LUT_ONE), side.sample_lod(x, y, 2.0, &LUT_ONE), side.sample_lod(x, y, 1.5, &LUT_ONE));
        for k in 0..4 {
            assert!((m.body[k] - 0.5 * (a.body[k] + b.body[k])).abs() < 1e-4);
        }
        // Cerca de cada nivel, uno solo (brilineal).
        assert_eq!(side.sample_lod(x, y, 1.2, &LUT_ONE), a);
        assert_eq!(side.sample_lod(x, y, 1.8, &LUT_ONE), b);
        // Lo que lee el bilineal cae adentro de `span_rows`.
        for lod in [0.0f32, 0.7, 2.3] {
            for yi in 0..side.height {
                let yf = yi as f32 + 0.5;
                let (lo, hi) = side.span_rows(yf, yf, lod);
                for xi in 0..side.width {
                    let xf = xi as f32 + 0.5;
                    if xf < lo || xf > hi {
                        let t = side.sample_lod(xf, yf, lod, &LUT_ONE);
                        assert!(t.body.iter().all(|&v| v == 0.0) && t.light.iter().all(|&v| v <= 2.0 / 255.0), "lod {lod} ({xi}, {yi}) fuera de [{lo}, {hi}]: {t:?}");
                    }
                }
            }
        }
    }

    /// Cuanto cuesta leer un sprite (`cargo test --release medir_lectura
    /// -- --ignored --nocapture`).
    #[test]
    #[ignore = "mide tiempos"]
    fn medir_lectura() {
        let mesh = crate_mesh();
        let sprites = bake(&mesh, Finish::Vehicle, &[0.3], 0.1, 1.0, 60.0);
        let s = &sprites[0];
        let n = 2_000_000;
        let (w, h) = (s.width as f32, s.height as f32);
        for lod in [0.0f32, 0.5] {
            let t = std::time::Instant::now();
            let mut acc = 0.0;
            for i in 0..n {
                let x = (i % 977) as f32 / 977.0 * w;
                let y = (i % 613) as f32 / 613.0 * h;
                let v = s.sample_lod(x, y, lod, &LUT_ONE);
                acc += v.body[0] + v.light[1];
            }
            println!("sample_lod {lod}: {:.1} ns  ({acc})", t.elapsed().as_secs_f64() * 1e9 / n as f64);
        }
        let t = std::time::Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            let x = (i % 977) as f32 / 977.0 * w;
            let y = (i % 613) as f32 / 613.0 * h;
            let v = bilinear8(&s.texels, s.width, s.height, x, y, &LUT_ONE);
            acc += v.body[0] + v.light[1];
        }
        println!("bilinear8: {:.1} ns  ({acc})", t.elapsed().as_secs_f64() * 1e9 / n as f64);
        let t = std::time::Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            let x = (i % 977) as f32 / 977.0 * w;
            let y = (i % 613) as f32 / 613.0 * h;
            acc += s.sample(x, y)[0];
        }
        println!("sample: {:.1} ns  ({acc})", t.elapsed().as_secs_f64() * 1e9 / n as f64);
    }

    /// El patron de cada color de LED.
    #[test]
    fn the_leds_get_their_pattern_by_color() {
        let back = Vec3::new(0.0, 0.0, 1.0);
        let side = Vec3::new(1.0, 0.0, 0.0);
        let tag = |c: Vec3, n: Vec3| led_tag(c, n, Vec3::new(0.3, 0.0, 1.0), -2.0, 4.0) >> 6;
        assert_eq!(tag(Vec3::new(1.0, 0.55, 0.05), side), PATTERN_CHASE);
        assert_eq!(tag(Vec3::new(1.0, 0.1, 0.8), side), PATTERN_BREATHE);
        assert_eq!(tag(Vec3::new(0.1, 0.9, 1.0), side), PATTERN_BREATHE);
        assert_eq!(tag(Vec3::new(0.1, 0.2, 1.0), side), PATTERN_BREATHE);
        assert_eq!(tag(Vec3::new(1.0, 0.05, 0.05), back), PATTERN_STOP);
        assert_eq!(tag(Vec3::new(1.0, 0.05, 0.05), side), PATTERN_STATIC);
        assert_eq!(tag(Vec3::new(1.0, 1.0, 1.0), back), PATTERN_STATIC);
        assert_eq!(tag(Vec3::ZERO, back), PATTERN_STATIC);
        // La fase cabe en seis bits.
        for k in 0..50 {
            let t = led_tag(Vec3::new(1.0, 0.5, 0.1), side, Vec3::new(k as f32 * 0.1 - 2.5, 0.0, k as f32 * 0.37 - 9.0), -2.0, 4.0);
            assert_eq!(t >> 6, PATTERN_CHASE);
            assert!(t & 63 <= 63);
        }
        // El vidrio cafe de la Gallo y no la tinta cafe de su etiqueta.
        assert!(amber_glass(Vec3::new(0.16, 0.067, 0.032)));
        assert!(!amber_glass(Vec3::new(0.42, 0.14, 0.08)));
        assert!(!amber_glass(Vec3::new(0.64, 0.055, 0.065)));
    }

    /// La luz se anima en cada esquina antes de mezclar: de una LED fija a
    /// un stop apagado (tabla 0) la luz baja derecho, sin salto en la
    /// frontera de los pixeles, y el cuerpo opaco sigue con alfa 1 exacto.
    #[test]
    fn the_led_light_blends_its_animation() {
        let on = [0, 0, 0, 255, 200, 100, 50, PATTERN_STATIC << 6];
        let off = [0, 0, 0, 255, 200, 100, 50, (PATTERN_STOP << 6) | 17];
        let texels = vec![on, off, on, off];
        let mut lut = [1.0; 256];
        for f in 0..64 {
            lut[((PATTERN_STOP as usize) << 6) | f] = 0.0;
        }
        for k in 0..=20 {
            let x = 0.5 + k as f32 * 0.05;
            let t = bilinear8(&texels, 2, 2, x, 1.0, &lut);
            let want = 200.0 / 255.0 * (1.0 - (x - 0.5));
            assert!((t.light[0] - want).abs() < 1e-4, "{x}: {t:?}");
            assert!(t.body[3] == 1.0 || x > 1.0, "{x}: {t:?}");
        }
        // El tono vivo conserva el color: un halo rojo tenue sigue rojo.
        let h = tone_vivid(Vec3::new(0.2, 0.01, 0.01));
        assert!(h.x > 0.4 && h.y < 0.2 * h.x, "{h:?}");
        assert_eq!(tone_vivid(Vec3::ZERO), Vec3::ZERO);
    }

    /// La caja con suma que corre da lo mismo que sumar la ventana entera
    /// en cada pixel (con el borde repetido).
    #[test]
    fn the_running_box_blur_matches_the_plain_sum() {
        let (w, h, r) = (13usize, 9usize, 3isize);
        let image: Vec<Vec3> = (0..w * h).map(|i| Vec3::new((i * 7 % 11) as f32, (i * 3 % 5) as f32, (i % 2) as f32)).collect();
        let fast = box_blur(&image, w, h, r as usize);
        let at = |x: isize, y: isize| image[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
        let n = ((2 * r + 1) * (2 * r + 1)) as f32;
        for y in 0..h as isize {
            for x in 0..w as isize {
                let mut sum = Vec3::ZERO;
                for dy in -r..=r {
                    for dx in -r..=r {
                        sum = sum + at(x + dx, y + dy);
                    }
                }
                let want = sum * (1.0 / n);
                assert!((fast[y as usize * w + x as usize] - want).length() < 1e-4, "({x}, {y})");
            }
        }
    }
}
