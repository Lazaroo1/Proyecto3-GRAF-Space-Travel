//! Mallas en perspectiva para los minijuegos: un rasterizador chico, en
//! CPU, para lo que se ve de cerca (las camionetas de Talega Surfers al
//! pasar a la par o al correr encima). Un sprite horneado es una imagen
//! plana: de cerca se ve como una tarjeta sin costado y, desde arriba,
//! corta (le falta la perspectiva). Aqui cada triangulo se proyecta con la
//! camara del juego.
//!
//! - **Recorte** contra el plano cercano (`z = NEAR` en la camara, que mira
//!   de frente): el triangulo se corta (Sutherland-Hodgman) y queda uno,
//!   dos o ninguno, con sus atributos interpolados.
//! - **Por filas**: cada triangulo se apunta en las bandas de filas que
//!   toca y la fila `y` solo revisa los de su banda. En la fila, cada
//!   arista `E(x, y) = (x - xa)(yb - ya) - (y - ya)(xb - xa)` es lineal en
//!   `x`: el tramo cubierto es donde las tres tienen el signo del area.
//! - **Perspectiva correcta**: en la pantalla lo lineal es `1/z` y
//!   `atributo / z`; en cada pixel `z = 1 / sum(l_i / z_i)` y cada atributo
//!   `sum(l_i a_i / z_i) z`.
//! - **Profundidad**: se compara con la de la fila (la calle, las casas, el
//!   Arco), y lo que se pinta deja la suya para lo que venga despues.

use crate::math::Vec3;

/// El plano cercano (m).
const NEAR: f32 = 0.25;
/// Filas por banda.
const BAND: usize = 8;
/// Filas por banda de `Indexed` (sus triangulos son chicos: con bandas
/// angostas cada fila revisa menos que no la tocan).
const IBAND: usize = 4;

/// La camara: donde esta, su focal en pixeles y el centro de la imagen
/// (mira de frente, hacia +z).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub pos: Vec3,
    pub focal: f32,
    pub cx: f32,
    pub horizon: f32,
}

/// Un vertice en el mundo con lo que se interpola.
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub world: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

/// Lo que llega a un pixel.
#[derive(Clone, Copy, Debug)]
pub struct Fragment {
    pub world: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
    pub depth: f32,
    /// Del triangulo: la clase de pieza, su valor y de que objeto es.
    pub class: f32,
    pub value: f32,
    pub tag: u32,
}

/// Lo mas cercano en un pixel (ver `Raster::row_nearest`).
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub depth: f32,
    pub tri: u32,
    pub k0: f32,
    pub k1: f32,
}

impl Default for Hit {
    fn default() -> Self {
        Self { depth: f32::MAX, tri: 0, k0: 0.0, k1: 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
struct ScreenTri {
    x: [f32; 3],
    y: [f32; 3],
    inv_z: [f32; 3],
    v: [Vertex; 3],
    class: f32,
    value: f32,
    tag: u32,
    y0: f32,
    y1: f32,
}

#[derive(Default)]
pub struct Raster {
    tris: Vec<ScreenTri>,
    bands: Vec<Vec<u32>>,
    height: usize,
}

impl Raster {
    /// Empieza un frame de `height` filas.
    pub fn clear(&mut self, height: usize) {
        self.tris.clear();
        self.height = height;
        let n = height.div_ceil(BAND);
        self.bands.resize_with(n, Vec::new);
        for b in &mut self.bands {
            b.clear();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Un triangulo (en el mundo), con la clase de su pieza, su valor y de
    /// que objeto es.
    pub fn add(&mut self, view: &View, v: [Vertex; 3], class: f32, value: f32, tag: u32) {
        let z = |p: Vec3| p.z - view.pos.z;
        let inside = [z(v[0].world) >= NEAR, z(v[1].world) >= NEAR, z(v[2].world) >= NEAR];
        if inside.iter().all(|&i| i) {
            self.push(view, v, class, value, tag);
            return;
        }
        if !inside.iter().any(|&i| i) {
            return;
        }
        // Recorte contra el plano cercano.
        let lerp = |a: &Vertex, b: &Vertex, t: f32| Vertex {
            world: a.world + (b.world - a.world) * t,
            normal: a.normal + (b.normal - a.normal) * t,
            color: a.color + (b.color - a.color) * t,
        };
        let mut poly: Vec<Vertex> = Vec::with_capacity(4);
        for i in 0..3 {
            let (a, b) = (&v[i], &v[(i + 1) % 3]);
            let (za, zb) = (z(a.world), z(b.world));
            if za >= NEAR {
                poly.push(*a);
            }
            if (za >= NEAR) != (zb >= NEAR) {
                poly.push(lerp(a, b, (NEAR - za) / (zb - za)));
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            self.push(view, [poly[0], poly[k], poly[k + 1]], class, value, tag);
        }
    }

    fn push(&mut self, view: &View, v: [Vertex; 3], class: f32, value: f32, tag: u32) {
        let mut x = [0.0f32; 3];
        let mut y = [0.0f32; 3];
        let mut inv_z = [0.0f32; 3];
        for i in 0..3 {
            let p = v[i].world - view.pos;
            let k = view.focal / p.z;
            x[i] = view.cx + p.x * k;
            y[i] = view.horizon - p.y * k;
            inv_z[i] = 1.0 / p.z;
        }
        let area = (x[1] - x[0]) * (y[2] - y[0]) - (y[1] - y[0]) * (x[2] - x[0]);
        if area.abs() < 1e-4 {
            return;
        }
        let (y0, y1) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
        if y1 < 0.0 || y0 > self.height as f32 {
            return;
        }
        let id = self.tris.len() as u32;
        self.tris.push(ScreenTri { x, y, inv_z, v, class, value, tag, y0, y1 });
        let (b0, b1) = ((y0.max(0.0) as usize) / BAND, ((y1.max(0.0) as usize) / BAND).min(self.bands.len().saturating_sub(1)));
        for b in b0..=b1 {
            if let Some(band) = self.bands.get_mut(b) {
                band.push(id);
            }
        }
    }

    /// Pinta la fila `y` (de `width` pixeles) con `shade`, que da el color y
    /// lo que emite de cada fragmento; `put(pixel, color, emision)` lo
    /// escribe. Solo donde queda mas cerca que `depth`, que se actualiza.
    pub fn row(&self, y: usize, width: usize, depth: &mut [f32], shade: impl Fn(&Fragment) -> (Vec3, f32), mut put: impl FnMut(usize, Vec3, f32)) {
        let Some(band) = self.bands.get(y / BAND) else { return };
        let yf = y as f32 + 0.5;
        for &id in band {
            let t = &self.tris[id as usize];
            if yf < t.y0 || yf > t.y1 {
                continue;
            }
            let (x, ty) = (&t.x, &t.y);
            // Las aristas opuestas a cada vertice: `E_i = A_i x + B_i` en
            // esta fila; la baricentrica `l_i = E_i / area`.
            let area = (x[1] - x[0]) * (ty[2] - ty[0]) - (ty[1] - ty[0]) * (x[2] - x[0]);
            let edge = |a: usize, b: usize| {
                let (xa, ya, xb, yb) = (x[a], ty[a], x[b], ty[b]);
                // E(p) = (xb - xa)(py - ya) - (yb - ya)(px - xa), con el
                // signo del area para el vertice opuesto.
                let ax = -(yb - ya);
                let b = (xb - xa) * (yf - ya) + (yb - ya) * xa;
                (ax / area, b / area)
            };
            let es = [edge(1, 2), edge(2, 0), edge(0, 1)];
            let (mut lo, mut hi) = (0.0f32, width as f32);
            let mut empty = false;
            for &(a, b) in &es {
                // l = a x + b >= 0.
                if a.abs() < 1e-12 {
                    if b < -1e-6 {
                        empty = true;
                    }
                } else if a > 0.0 {
                    lo = lo.max(-b / a);
                } else {
                    hi = hi.min(-b / a);
                }
            }
            if empty || hi <= lo {
                continue;
            }
            let x0 = ((lo - 0.5).ceil().max(0.0)) as usize;
            let x1 = ((hi - 0.5).floor().min(width as f32 - 1.0)) as isize;
            if x1 < x0 as isize {
                continue;
            }
            for px in x0..=x1 as usize {
                let xf = px as f32 + 0.5;
                let l = [es[0].0 * xf + es[0].1, es[1].0 * xf + es[1].1, es[2].0 * xf + es[2].1];
                let w = l[0] * t.inv_z[0] + l[1] * t.inv_z[1] + l[2] * t.inv_z[2];
                if w <= 0.0 {
                    continue;
                }
                let z = 1.0 / w;
                if z >= depth[px] {
                    continue;
                }
                let k = [l[0] * t.inv_z[0] * z, l[1] * t.inv_z[1] * z, l[2] * t.inv_z[2] * z];
                let mix = |f: fn(&Vertex) -> Vec3| f(&t.v[0]) * k[0] + f(&t.v[1]) * k[1] + f(&t.v[2]) * k[2];
                let frag = Fragment { world: mix(|v| v.world), normal: mix(|v| v.normal), color: mix(|v| v.color), depth: z, class: t.class, value: t.value, tag: t.tag };
                let (c, e) = shade(&frag);
                depth[px] = z;
                put(px, c, e);
            }
        }
    }
}

/// Una malla indexada que se dibuja cada frame (las naves de la carrera que
/// pasan cerca): como `Raster`, pero cada vertice se proyecta una sola vez
/// (los triangulos guardan sus indices, no copias de sus atributos), cada
/// triangulo deja listas sus aristas ya divididas entre su area
/// (`l_i = a_i x + c_i y + d_i`, asi la fila solo suma) y los que no tocan
/// el centro de ningun pixel no se apuntan. Se dibuja diferido: la fila
/// busca lo mas cercano en cada pixel (`row_nearest`) y solo eso se
/// sombrea (`fragment`). Los buffers se quedan de un frame al otro.
#[derive(Default)]
pub struct Indexed {
    world: Vec<Vec3>,
    /// La normal (solo para descartar lo que da la espalda) y dos
    /// atributos que se interpolan.
    normal: Vec<Vec3>,
    attr_a: Vec<Vec3>,
    attr_b: Vec<Vec3>,
    /// La marca de cada vertice (la del primero llega a los pixeles de su
    /// triangulo).
    vtag: Vec<u8>,
    /// En la pantalla: `x`, `y` y `1/z` (0 si queda detras del plano
    /// cercano).
    proj: Vec<[f32; 3]>,
    tris: Vec<ITri>,
    bands: Vec<Vec<u32>>,
    width: usize,
    height: usize,
    view: Option<View>,
    /// Las filas y columnas que tocan sus triangulos.
    bounds: (f32, f32, f32, f32),
}

/// Lo que llega a un pixel de `Indexed`: el punto, sus dos atributos
/// interpolados y la marca de su triangulo.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub world: Vec3,
    pub a: Vec3,
    pub b: Vec3,
    pub tag: u8,
}

#[derive(Clone, Copy, Debug)]
struct ITri {
    v: [u32; 3],
    tag: u8,
    a: [f32; 3],
    /// `1 / a_i` (0 si la arista es horizontal).
    ia: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
    iz: [f32; 3],
    y0: f32,
    y1: f32,
}

/// Un punto de la camara en la pantalla: `x`, `y` y `1/z` (todo 0 si
/// queda detras del plano cercano).
#[inline]
fn project(view: &View, world: Vec3) -> [f32; 3] {
    let p = world - view.pos;
    if p.z >= NEAR {
        let iz = 1.0 / p.z;
        let k = view.focal * iz;
        [view.cx + p.x * k, view.horizon - p.y * k, iz]
    } else {
        [0.0, 0.0, 0.0]
    }
}

impl Indexed {
    /// Empieza un frame con la camara `view` en una imagen de `width` x
    /// `height`.
    pub fn begin(&mut self, view: View, width: usize, height: usize) {
        self.world.clear();
        self.normal.clear();
        self.attr_a.clear();
        self.attr_b.clear();
        self.vtag.clear();
        self.proj.clear();
        self.tris.clear();
        self.width = width;
        self.height = height;
        self.view = Some(view);
        let n = height.div_ceil(IBAND);
        self.bands.resize_with(n, Vec::new);
        for b in &mut self.bands {
            b.clear();
        }
        self.bounds = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    }

    pub fn reserve(&mut self, vertices: usize, triangles: usize) {
        self.world.reserve(vertices);
        self.normal.reserve(vertices);
        self.attr_a.reserve(vertices);
        self.attr_b.reserve(vertices);
        self.vtag.reserve(vertices);
        self.proj.reserve(vertices);
        self.tris.reserve(triangles);
    }

    /// Un vertice (en la camara) con su normal y sus dos atributos: devuelve
    /// su indice.
    pub fn vertex(&mut self, world: Vec3, normal: Vec3, a: Vec3, b: Vec3) -> u32 {
        let view = self.view.unwrap_or(View { pos: Vec3::ZERO, focal: 1.0, cx: 0.0, horizon: 0.0 });
        self.world.push(world);
        self.normal.push(normal);
        self.attr_a.push(a);
        self.attr_b.push(b);
        self.vtag.push(0);
        self.proj.push(project(&view, world));
        (self.world.len() - 1) as u32
    }

    /// Pone de una vez `n` vertices: `f(i)` da el punto (en la camara), la
    /// normal, los dos atributos y la marca del `i`. En paralelo, en tramos.
    pub fn fill(&mut self, n: usize, f: impl Fn(usize) -> (Vec3, Vec3, Vec3, Vec3, u8) + Sync) {
        let view = self.view.unwrap_or(View { pos: Vec3::ZERO, focal: 1.0, cx: 0.0, horizon: 0.0 });
        let base = self.world.len();
        self.world.resize(base + n, Vec3::ZERO);
        self.normal.resize(base + n, Vec3::ZERO);
        self.attr_a.resize(base + n, Vec3::ZERO);
        self.attr_b.resize(base + n, Vec3::ZERO);
        self.vtag.resize(base + n, 0);
        self.proj.resize(base + n, [0.0; 3]);
        const CHUNK: usize = 1024;
        let world = crate::parallel::DisjointMut::new(&mut self.world[base..]);
        let normal = crate::parallel::DisjointMut::new(&mut self.normal[base..]);
        let attr_a = crate::parallel::DisjointMut::new(&mut self.attr_a[base..]);
        let attr_b = crate::parallel::DisjointMut::new(&mut self.attr_b[base..]);
        let vtag = crate::parallel::DisjointMut::new(&mut self.vtag[base..]);
        let proj = crate::parallel::DisjointMut::new(&mut self.proj[base..]);
        crate::parallel::for_each(n.div_ceil(CHUNK), |c| {
            let from = c * CHUNK;
            let len = CHUNK.min(n - from);
            // SAFETY: cada tramo es distinto.
            let (w, nm, a, b, t, p) = unsafe { (world.range(from, len), normal.range(from, len), attr_a.range(from, len), attr_b.range(from, len), vtag.range(from, len), proj.range(from, len)) };
            for k in 0..len {
                (w[k], nm[k], a[k], b[k], t[k]) = f(from + k);
                p[k] = project(&view, w[k]);
            }
        });
    }

    /// Las filas y columnas que tocan sus triangulos, `(y0, y1, x0, x1)`.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        (!self.tris.is_empty()).then_some(self.bounds)
    }

    /// Un triangulo de tres vertices ya puestos; a sus pixeles les llega la
    /// marca del primero. Lo que cruza el plano cercano se recorta.
    pub fn triangle(&mut self, v: [u32; 3]) {
        let tag = self.vtag[v[0] as usize];
        // De espaldas: si sus tres normales miran hacia atras de la camara,
        // lo tapa lo que la nave tiene enfrente (la mitad de los
        // triangulos; probado contra dibujarlos todos, la imagen sale igual).
        let pos = self.view.map_or(Vec3::ZERO, |w| w.pos);
        if v.iter().all(|&i| self.normal[i as usize].dot(self.world[i as usize] - pos) > 0.0) {
            return;
        }
        let inside = v.map(|i| self.proj[i as usize][2] > 0.0);
        if inside[0] && inside[1] && inside[2] {
            self.setup(v, tag);
            return;
        }
        if !(inside[0] || inside[1] || inside[2]) {
            return;
        }
        let Some(view) = self.view else { return };
        let z = |s: &Self, i: u32| s.world[i as usize].z - view.pos.z;
        let mut poly: [u32; 4] = [0; 4];
        let mut n = 0;
        for k in 0..3 {
            let (a, b) = (v[k], v[(k + 1) % 3]);
            let (za, zb) = (z(self, a), z(self, b));
            if za >= NEAR {
                poly[n] = a;
                n += 1;
            }
            if (za >= NEAR) != (zb >= NEAR) && n < 4 {
                let t = (NEAR - za) / (zb - za);
                let (ia, ib) = (a as usize, b as usize);
                let world = self.world[ia] + (self.world[ib] - self.world[ia]) * t;
                // Justo en el plano: un pelito adelante, que se proyecte.
                let world = Vec3::new(world.x, world.y, view.pos.z + NEAR + 1e-4);
                let normal = self.normal[ia] + (self.normal[ib] - self.normal[ia]) * t;
                let a = self.attr_a[ia] + (self.attr_a[ib] - self.attr_a[ia]) * t;
                let b = self.attr_b[ia] + (self.attr_b[ib] - self.attr_b[ia]) * t;
                poly[n] = self.vertex(world, normal, a, b);
                n += 1;
            }
        }
        for k in 1..n.saturating_sub(1) {
            self.setup([poly[0], poly[k], poly[k + 1]], tag);
        }
    }

    fn setup(&mut self, v: [u32; 3], tag: u8) {
        let p = v.map(|i| self.proj[i as usize]);
        let (x, y) = ([p[0][0], p[1][0], p[2][0]], [p[0][1], p[1][1], p[2][1]]);
        let (y0, y1) = (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]));
        let (x0, x1) = (x[0].min(x[1]).min(x[2]), x[0].max(x[1]).max(x[2]));
        // Afuera de la imagen, o sin el centro de ningun pixel adentro de su
        // caja (los chiquitos de lejos): no se ve.
        if y1 < 0.0 || y0 > self.height as f32 || x1 < 0.0 || x0 > self.width as f32 {
            return;
        }
        if (y0 - 0.5).ceil() > (y1 - 0.5).floor() || (x0 - 0.5).ceil() > (x1 - 0.5).floor() {
            return;
        }
        let area = (x[1] - x[0]) * (y[2] - y[0]) - (y[1] - y[0]) * (x[2] - x[0]);
        if !(area.abs() >= 1e-6) {
            return;
        }
        let inv = 1.0 / area;
        // La arista opuesta al vertice `i`, de `ea` a `eb`:
        // `E = (xb - xa)(py - ya) - (yb - ya)(px - xa)`, y `l_i = E / area`.
        let mut a = [0.0; 3];
        let mut ia = [0.0; 3];
        let mut c = [0.0; 3];
        let mut d = [0.0; 3];
        for (i, (ea, eb)) in [(1usize, 2usize), (2, 0), (0, 1)].into_iter().enumerate() {
            let (xa, ya, xb, yb) = (x[ea], y[ea], x[eb], y[eb]);
            a[i] = -(yb - ya) * inv;
            ia[i] = if a[i].abs() > 1e-12 { 1.0 / a[i] } else { 0.0 };
            c[i] = (xb - xa) * inv;
            d[i] = ((yb - ya) * xa - (xb - xa) * ya) * inv;
        }
        let id = self.tris.len() as u32;
        self.tris.push(ITri { v, tag, a, ia, c, d, iz: [p[0][2], p[1][2], p[2][2]], y0, y1 });
        self.bounds = (self.bounds.0.min(y0), self.bounds.1.max(y1), self.bounds.2.min(x0), self.bounds.3.max(x1));
        let last = self.bands.len().saturating_sub(1);
        let (b0, b1) = (((y0.max(0.0) as usize) / IBAND).min(last), ((y1.max(0.0) as usize) / IBAND).min(last));
        for band in &mut self.bands[b0..=b1] {
            band.push(id);
        }
    }

    /// Lo mas cercano de la fila `y` en cada pixel, sin sombrear: en
    /// `hits[x]` la profundidad, el triangulo y sus dos primeros pesos ya
    /// corregidos por la perspectiva. `hits` tiene que traer `f32::MAX` de
    /// profundidad donde no hay nada. Devuelve el tramo `[desde, hasta)` de
    /// columnas que toco (vacio si ninguna).
    pub fn row_nearest(&self, y: usize, hits: &mut [Hit]) -> (usize, usize) {
        let Some(band) = self.bands.get(y / IBAND) else { return (0, 0) };
        let yf = y as f32 + 0.5;
        let width = self.width.min(hits.len());
        let (mut touched_lo, mut touched_hi) = (usize::MAX, 0usize);
        for &id in band {
            let t = &self.tris[id as usize];
            if yf < t.y0 || yf > t.y1 {
                continue;
            }
            let b = [t.c[0] * yf + t.d[0], t.c[1] * yf + t.d[1], t.c[2] * yf + t.d[2]];
            let (mut lo, mut hi) = (0.0f32, width as f32);
            let mut empty = false;
            for i in 0..3 {
                // `l_i = a_i x + b_i >= 0`.
                if t.ia[i] == 0.0 {
                    empty |= b[i] < -1e-6;
                } else if t.a[i] > 0.0 {
                    lo = lo.max(-b[i] * t.ia[i]);
                } else {
                    hi = hi.min(-b[i] * t.ia[i]);
                }
            }
            if empty || hi <= lo {
                continue;
            }
            let x0 = ((lo - 0.5).ceil().max(0.0)) as usize;
            let x1 = ((hi - 0.5).floor().min(width as f32 - 1.0)) as isize;
            if x1 < x0 as isize {
                continue;
            }
            let x1 = x1 as usize;
            touched_lo = touched_lo.min(x0);
            touched_hi = touched_hi.max(x1 + 1);
            // `l_i / z_i` y `1/z` son lineales en `x`: se avanzan sumando.
            let xf = x0 as f32 + 0.5;
            let mut a0 = (t.a[0] * xf + b[0]) * t.iz[0];
            let mut a1 = (t.a[1] * xf + b[1]) * t.iz[1];
            let mut w = a0 + a1 + (t.a[2] * xf + b[2]) * t.iz[2];
            let (da0, da1) = (t.a[0] * t.iz[0], t.a[1] * t.iz[1]);
            let dw = da0 + da1 + t.a[2] * t.iz[2];
            for hit in &mut hits[x0..=x1] {
                if w > 0.0 {
                    let z = 1.0 / w;
                    if z < hit.depth {
                        *hit = Hit { depth: z, tri: id, k0: a0 * z, k1: a1 * z };
                    }
                }
                a0 += da0;
                a1 += da1;
                w += dw;
            }
        }
        if touched_hi > touched_lo { (touched_lo, touched_hi) } else { (0, 0) }
    }

    /// Lo que cae en un pixel que dio `row_nearest`.
    pub fn sample(&self, hit: &Hit) -> Sample {
        let t = &self.tris[hit.tri as usize];
        let k = [hit.k0, hit.k1, 1.0 - hit.k0 - hit.k1];
        let [i0, i1, i2] = t.v.map(|i| i as usize);
        let mix = |v: &[Vec3]| v[i0] * k[0] + v[i1] * k[1] + v[i2] * k[2];
        Sample { world: mix(&self.world), a: mix(&self.attr_a), b: mix(&self.attr_b), tag: t.tag }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La malla indexada cubre lo mismo que `Raster` (un cuadrado a 10 m) y
    /// recorta lo que cruza el plano cercano.
    #[test]
    fn the_indexed_mesh_covers_like_the_raster() {
        let view = View { pos: Vec3::ZERO, focal: 100.0, cx: 50.0, horizon: 50.0 };
        let mut m = Indexed::default();
        m.begin(view, 100, 100);
        let n = Vec3::new(0.0, 0.0, -1.0);
        let q = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| m.vertex(Vec3::new(x, y, 10.0), n, Vec3::ONE, Vec3::ZERO));
        m.triangle([q[0], q[1], q[2]]);
        m.triangle([q[0], q[2], q[3]]);
        let mut hits = vec![Hit::default(); 100];
        let (lo, hi) = m.row_nearest(50, &mut hits);
        assert_eq!((lo, hi), (40, 60));
        assert!(hits[40..60].iter().all(|h| (h.depth - 10.0).abs() < 1e-3));
        let f = m.sample(&hits[50]);
        assert!((f.world.z - 10.0).abs() < 1e-3 && f.world.x.abs() < 0.1);
        // Un piso que pasa por debajo de la camara: se recorta y se ve.
        let mut m = Indexed::default();
        m.begin(view, 100, 100);
        let p = [(-5.0, -3.0), (5.0, -3.0), (0.0, 20.0)].map(|(x, z)| m.vertex(Vec3::new(x, -1.0, z), Vec3::Y, Vec3::ONE, Vec3::ZERO));
        m.triangle(p);
        let mut hits = vec![Hit::default(); 100];
        let (lo, hi) = m.row_nearest(99, &mut hits);
        assert!(hi > lo);
        assert!(hits[lo..hi].iter().filter(|h| h.depth < f32::MAX).all(|h| h.depth >= NEAR - 1e-3));
    }

    /// Un cuadrado a 10 m de frente cubre lo que debe, con su profundidad, y
    /// uno que cruza el plano cercano se recorta sin romperse.
    #[test]
    fn a_square_rasterizes_with_depth_and_clips_at_the_near_plane() {
        let view = View { pos: Vec3::ZERO, focal: 100.0, cx: 50.0, horizon: 50.0 };
        let vert = |x: f32, y: f32, z: f32| Vertex { world: Vec3::new(x, y, z), normal: Vec3::new(0.0, 0.0, -1.0), color: Vec3::ONE };
        let mut r = Raster::default();
        r.clear(100);
        let (a, b, c, d) = (vert(-1.0, -1.0, 10.0), vert(1.0, -1.0, 10.0), vert(1.0, 1.0, 10.0), vert(-1.0, 1.0, 10.0));
        r.add(&view, [a, b, c], 0.0, 0.0, 1);
        r.add(&view, [a, c, d], 0.0, 0.0, 1);
        let mut depth = vec![f32::MAX; 100];
        let mut hits = Vec::new();
        r.row(50, 100, &mut depth, |f| (f.color, 0.0), |x, _, _| hits.push(x));
        // De x = 40 a 60 (2 m a 10 m con focal 100 son 20 px).
        assert_eq!(hits.iter().min(), Some(&40));
        assert_eq!(hits.iter().max(), Some(&59));
        // Sin huecos en la diagonal compartida.
        assert!((40..60).all(|x| hits.contains(&x)));
        assert!((depth[50] - 10.0).abs() < 1e-3);
        // Detras de algo mas cerca no se pinta.
        let mut near = vec![5.0; 100];
        let mut none = 0;
        r.row(50, 100, &mut near, |f| (f.color, 0.0), |_, _, _| none += 1);
        assert_eq!(none, 0);
        // Un piso que pasa por debajo de la camara: se recorta y se pinta.
        let mut r = Raster::default();
        r.clear(100);
        r.add(&view, [vert(-5.0, -1.0, -3.0), vert(5.0, -1.0, -3.0), vert(0.0, -1.0, 20.0)], 0.0, 0.0, 2);
        let mut depth = vec![f32::MAX; 100];
        let mut n = 0;
        r.row(99, 100, &mut depth, |f| (f.color, 0.0), |_, _, _| n += 1);
        assert!(n > 0);
        assert!(depth.iter().filter(|d| d.is_finite() && **d < f32::MAX).all(|d| *d >= NEAR - 1e-3));
    }
}
