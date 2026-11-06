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
//!   diferido: el raster solo guarda la normal, el color y la pieza de la
//!   muestra mas cercana, y cada muestra se sombrea una vez al final (una
//!   nave tiene varias capas de piezas encimadas).
//! - **Sombreado** con las clases de pieza de `obj::Surface` (pintura con
//!   laca, metal, vidrio, LEDs, hule), con una luz de luna fria, la luz
//!   naranja de los postes desde abajo y un cielo de noche que se refleja.
//!   Las LEDs se guardan aparte, se difuminan y se suman: el resplandor sale
//!   del contorno, como neon.
//! - **Mapeo de tonos** al guardar (`1 - e^(-x)` y gamma 2.2): los colores
//!   del `.mtl` son lineales.

use crate::math::{Vec3, smoothstep};
use crate::parallel;
use crate::scene::{Mesh, Surface};

/// Como se sombrea una malla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Las naves botella: solo el color de cada cara, todo pulido.
    Bottle,
    /// Los vehiculos: color y clase de pieza en la `uv`.
    Vehicle,
}

/// Una imagen horneada, RGBA8 premultiplicada (el color puede pasar del
/// alfa: el resplandor de las LEDs se suma).
#[derive(Clone, Debug)]
pub struct Sprite {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[u8; 4]>,
    /// Cuanto emite cada pixel (de 0 a 255): solo las piezas con clase LED
    /// del `.mtl`, no la laca ni el vidrio claros. Es lo que el posproceso
    /// hace florecer. Vacio si el sprite no guarda emision (los
    /// barriletes: quien los pega la saca de su color).
    pub emit: Vec<u8>,
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
}

impl Sprite {
    /// El pixel (x, y) con interpolacion bilineal, premultiplicado, de 0 a 1.
    #[inline]
    pub fn sample(&self, x: f32, y: f32) -> [f32; 4] {
        let x = x - 0.5;
        let y = y - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |xi: i32, yi: i32| -> [f32; 4] {
            if xi < 0 || yi < 0 || xi >= self.width as i32 || yi >= self.height as i32 {
                return [0.0; 4];
            }
            let p = self.pixels[yi as usize * self.width + xi as usize];
            [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
        };
        let (x0, y0) = (x0 as i32, y0 as i32);
        // Adentro (casi siempre): los cuatro de una vez, sin revisar cada
        // uno, con los pesos `(1 - fx)(1 - fy)`, `fx (1 - fy)`, ...
        if x0 >= 0 && y0 >= 0 && (x0 as usize) + 1 < self.width && (y0 as usize) + 1 < self.height {
            let i = y0 as usize * self.width + x0 as usize;
            let (a, b) = (self.pixels[i], self.pixels[i + 1]);
            let (c, d) = (self.pixels[i + self.width], self.pixels[i + self.width + 1]);
            let s = 1.0 / 255.0;
            let (wa, wb, wc, wd) = ((1.0 - fx) * (1.0 - fy) * s, fx * (1.0 - fy) * s, (1.0 - fx) * fy * s, fx * fy * s);
            return std::array::from_fn(|k| a[k] as f32 * wa + b[k] as f32 * wb + c[k] as f32 * wc + d[k] as f32 * wd);
        }
        let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
        let mut out = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bottom = c[k] + (d[k] - c[k]) * fx;
            out[k] = (top + (bottom - top) * fy) * (1.0 / 255.0);
        }
        out
    }

    /// El tramo de `x` (en pixeles del sprite) donde `sample(x, y)` puede
    /// no ser cero: el bilineal en `x` lee las columnas `floor(x - 0.5)` y
    /// la siguiente, asi que un tramo `[a, b)` de las filas que toca da
    /// `(a - 0.5, b + 0.5)`. Vacio si la fila no tiene nada.
    pub fn span(&self, y: f32) -> (f32, f32) {
        let y0 = (y - 0.5).floor();
        let (mut lo, mut hi) = (u16::MAX, 0u16);
        for yi in [y0, y0 + 1.0] {
            if yi >= 0.0 && (yi as usize) < self.height {
                let (a, b) = self.spans[yi as usize];
                if b > a {
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
            }
        }
        if hi <= lo { (0.0, 0.0) } else { (lo as f32 - 0.5, hi as f32 + 0.5) }
    }

    /// El tramo de `x` (en pixeles del sprite) donde `sample(x, y)` es
    /// opaco del todo (ver `solid`): con `x0 = floor(x - 0.5)` en
    /// `[desde, hasta)`, `x` va de `desde + 0.5` a `hasta + 0.5`. Vacio si
    /// la fila no tiene un tramo opaco.
    pub fn solid_span(&self, y: f32) -> (f32, f32) {
        let y0 = (y - 0.5).floor();
        if !(y0 >= 0.0) || y0 as usize >= self.solid.len() {
            return (0.0, 0.0);
        }
        let (a, b) = self.solid[y0 as usize];
        if b <= a { (0.0, 0.0) } else { (a as f32 + 0.5, b as f32 + 0.5) }
    }

    /// Cuanto emite el pixel que cae en `(x, y)` (vecino mas cercano, de 0
    /// a 1): el brillo del posproceso lo difumina igual.
    #[inline]
    pub fn emit_at(&self, x: f32, y: f32) -> f32 {
        if !(x >= 0.0 && y >= 0.0) {
            return 0.0;
        }
        let (xi, yi) = (x as usize, y as usize);
        if xi >= self.width || yi >= self.height {
            return 0.0;
        }
        // (Un sprite sin emision guardada, como los barriletes, no emite.)
        self.emit.get(yi * self.width + xi).map_or(0.0, |&v| v as f32 * (1.0 / 255.0))
    }
}

/// Muestras por lado de cada pixel.
const SUPER: usize = 2;

/// Un sprite por hornear: la malla, como se sombrea, el giro y la altura
/// de la vista (radianes), cuanto mide una unidad del modelo en metros y
/// los pixeles por metro.
#[derive(Clone, Copy)]
pub struct Job<'a> {
    pub mesh: &'a Mesh,
    pub finish: Finish,
    pub yaw: f32,
    pub elevation: f32,
    pub meters: f32,
    pub pixels_per_meter: f32,
}

/// Hornea todos los sprites de `jobs` en una sola pasada en paralelo (cada
/// hilo toma el siguiente que falte: los de lado tardan mas que los de
/// atras).
pub fn bake_all(jobs: &[Job]) -> Vec<Sprite> {
    let mut sprites: Vec<Option<Sprite>> = vec![None; jobs.len()];
    parallel::chunks_mut(&mut sprites, 1, 1, |i, slot| {
        let j = jobs[i];
        slot[0] = Some(bake_one(j.mesh, j.finish, j.yaw, j.elevation, j.meters, j.pixels_per_meter));
    });
    sprites.into_iter().map(|s| s.expect("sprite horneado")).collect()
}

/// Hornea `mesh` girada cada angulo de `yaws` (radianes; 0 se ve desde
/// atras, positivo gira la nariz hacia la derecha de la imagen), vista
/// `elevation` radianes desde arriba, a `pixels_per_meter`. `meters` es
/// cuanto mide una unidad del modelo. Los angulos se hornean en paralelo.
#[cfg(test)]
pub fn bake(mesh: &Mesh, finish: Finish, yaws: &[f32], elevation: f32, meters: f32, pixels_per_meter: f32) -> Vec<Sprite> {
    let jobs: Vec<Job> = yaws.iter().map(|&yaw| Job { mesh, finish, yaw, elevation, meters, pixels_per_meter }).collect();
    bake_all(&jobs)
}

/// Un vertice ya en la imagen: x, y en muestras, profundidad (mayor es mas
/// cerca) y lo que se interpola.
#[derive(Clone, Copy)]
struct Corner {
    x: f32,
    y: f32,
    depth: f32,
    normal: Vec3,
    color: Vec3,
    class: f32,
    value: f32,
}

fn bake_one(mesh: &Mesh, finish: Finish, yaw: f32, elevation: f32, meters: f32, ppm: f32) -> Sprite {
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
    let projected: Vec<Vec3> = mesh.positions.iter().map(|&p| rotate(p)).collect();
    let (lo, hi) = projected.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    // Margen para el resplandor de las LEDs.
    let margin = (6 * SUPER) as f32;
    let width_s = (((hi.x - lo.x) * k + 2.0 * margin).ceil() as usize).div_ceil(SUPER) * SUPER;
    let height_s = (((hi.y - lo.y) * k + 2.0 * margin).ceil() as usize).div_ceil(SUPER) * SUPER;
    let to_image = |p: Vec3| ((p.x - lo.x) * k + margin, (hi.y - p.y) * k + margin);
    // El suelo bajo el centro: el punto mas bajo del modelo, en x = 0.
    let ground = rotate(Vec3::new(0.0, mesh_bottom(mesh), 0.0));
    let anchor_s = to_image(ground);

    let corners: Vec<Corner> = (0..mesh.positions.len())
        .map(|i| {
            let p = projected[i];
            let (x, y) = to_image(p);
            let n = mesh.normals.get(i).copied().unwrap_or(Vec3::Y);
            let uv = mesh.uvs.get(i).copied().unwrap_or_default();
            Corner {
                x,
                y,
                depth: p.z,
                normal: rotate(n),
                color: mesh.colors.get(i).copied().unwrap_or(Vec3::splat(0.7)),
                class: if finish == Finish::Vehicle { uv.x } else { -1.0 },
                value: uv.y,
            }
        })
        .collect();

    let samples = width_s * height_s;
    let mut depth = vec![f32::MIN; samples];
    // Lo que se guarda de la muestra mas cercana: normal, color y la pieza.
    let mut normals = vec![Vec3::ZERO; samples];
    let mut colors = vec![Vec3::ZERO; samples];
    let mut pieces = vec![(0.0f32, 0.0f32); samples];
    for tri in &mesh.indices {
        let [a, b, c] = tri.map(|i| corners[i as usize]);
        let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if area.abs() < 1e-6 {
            continue;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let x1 = (a.x.max(b.x).max(c.x).ceil() as usize).min(width_s);
        let y1 = (a.y.max(b.y).max(c.y).ceil() as usize).min(height_s);
        let inv = 1.0 / area;
        for y in y0..y1 {
            let py = y as f32 + 0.5;
            for x in x0..x1 {
                let px = x as f32 + 0.5;
                // Funciones de borde: el peso de cada vertice.
                let wa = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) * inv;
                let wb = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) * inv;
                let wc = 1.0 - wa - wb;
                if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                    continue;
                }
                let z = a.depth * wa + b.depth * wb + c.depth * wc;
                let s = y * width_s + x;
                if z <= depth[s] {
                    continue;
                }
                depth[s] = z;
                normals[s] = a.normal * wa + b.normal * wb + c.normal * wc;
                colors[s] = a.color * wa + b.color * wb + c.color * wc;
                pieces[s] = (a.class, a.value);
            }
        }
    }
    // Cada muestra cubierta se sombrea una vez.
    let mut lit = vec![Vec3::ZERO; samples];
    let mut glow = vec![Vec3::ZERO; samples];
    for s in 0..samples {
        if depth[s] == f32::MIN {
            continue;
        }
        let mut normal = normals[s].normalize();
        // Caras que dan la espalda (mallas abiertas): la normal hacia la
        // camara.
        if normal.z < 0.0 {
            normal = -normal;
        }
        let (surface, emission) = shade(finish, pieces[s].0, pieces[s].1, colors[s], normal);
        lit[s] = surface;
        glow[s] = emission;
    }

    // Cada pixel promedia sus muestras: el color, el alfa y la luz de LEDs.
    let (width, height) = (width_s / SUPER, height_s / SUPER);
    let mut color = vec![Vec3::ZERO; width * height];
    let mut alpha = vec![0.0f32; width * height];
    let mut light = vec![Vec3::ZERO; width * height];
    let per = 1.0 / (SUPER * SUPER) as f32;
    for y in 0..height {
        for x in 0..width {
            let (mut c, mut a, mut g) = (Vec3::ZERO, 0.0, Vec3::ZERO);
            for sy in 0..SUPER {
                for sx in 0..SUPER {
                    let s = (y * SUPER + sy) * width_s + x * SUPER + sx;
                    if depth[s] > f32::MIN {
                        c = c + lit[s];
                        a += 1.0;
                        g = g + glow[s];
                    }
                }
            }
            color[y * width + x] = c * per;
            alpha[y * width + x] = a * per;
            light[y * width + x] = g * per;
        }
    }
    // El resplandor: la luz de las LEDs difuminada (dos pasadas de caja).
    let mut halo = light.clone();
    for _ in 0..2 {
        halo = box_blur(&halo, width, height, 2);
    }
    let pixels: Vec<[u8; 4]> = (0..width * height)
        .map(|i| {
            // Premultiplicado: el color de la nave ya trae su cobertura.
            let c = tone(color[i] + light[i] * alpha[i]) + tone(halo[i] * 0.55);
            let a = alpha[i];
            [byte(c.x), byte(c.y), byte(c.z), byte(a)]
        })
        .collect();
    // La emision: `1 - e^(-3 max(luz))` de las LEDs (con su cobertura, que
    // ya trae `light`). Donde las LEDs se amontonan (las tiras y columnas
    // de focos de la camioneta) emite menos que un foco suelto: la
    // densidad es la fraccion de pixeles con LED en una ventana de 7 x 7,
    // y arriba de 0.2 se baja hasta a la mitad,
    // `1 - 0.5 smoothstep(0.2, 0.5, densidad)`. Si no, decenas de focos
    // juntos suman un velo blanco.
    let lit: Vec<Vec3> = light.iter().map(|l| Vec3::splat(if l.max_element() > 0.05 { 1.0 } else { 0.0 })).collect();
    let density = box_blur(&lit, width, height, 3);
    let emit = (0..width * height)
        .map(|i| {
            let e = 1.0 - (-3.0 * light[i].max_element()).exp();
            let crowd = 1.0 - 0.5 * smoothstep(0.2, 0.5, density[i].x);
            byte(e * crowd)
        })
        .collect();
    let spans = pixels
        .chunks_exact(width.max(1))
        .map(|row: &[[u8; 4]]| {
            let any = |p: &[u8; 4]| p.iter().any(|&v| v != 0);
            match (row.iter().position(any), row.iter().rposition(any)) {
                (Some(a), Some(b)) => (a.min(u16::MAX as usize) as u16, (b + 1).min(u16::MAX as usize) as u16),
                _ => (0, 0),
            }
        })
        .collect();
    let opaque = |x: usize, y: usize| pixels[y * width + x][3] == 255;
    let solid = (0..height)
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
        .collect();
    Sprite {
        width,
        height,
        pixels,
        emit,
        spans,
        solid,
        anchor: (anchor_s.0 / SUPER as f32, anchor_s.1 / SUPER as f32),
        scale: ppm,
    }
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
    let t = |v: f32| (1.0 - (-v.max(0.0) * 1.4).exp()).powf(1.0 / 2.2);
    Vec3::new(t(c.x), t(c.y), t(c.z))
}

/// La luz del horneado (en los ejes de la imagen: x a la derecha, y arriba,
/// z hacia la camara): la luna fria desde arriba a la izquierda y la luz
/// naranja de los postes desde abajo y adelante.
const MOON: Vec3 = Vec3::new(-0.45, 0.75, 0.5);
const MOON_LIGHT: Vec3 = Vec3::new(0.55, 0.62, 0.8);
const LAMP: Vec3 = Vec3::new(0.35, -0.2, 0.9);
const LAMP_LIGHT: Vec3 = Vec3::new(0.9, 0.5, 0.18);
const AMBIENT: Vec3 = Vec3::new(0.05, 0.055, 0.08);

/// El cielo de noche que se refleja: oscuro arriba, el resplandor naranja
/// de la ciudad abajo y una franja de luz de los postes.
fn environment(r: Vec3) -> Vec3 {
    let up = r.y;
    let sky = Vec3::new(0.04, 0.05, 0.1).lerp(Vec3::new(0.25, 0.14, 0.08), ((-up) * 2.0).clamp(0.0, 1.0));
    let band = (-((up - 0.15) / 0.08).powi(2)).exp();
    sky + Vec3::new(1.0, 0.7, 0.4) * (0.6 * band)
}

/// El color de una muestra y la luz que emite (aparte, para el resplandor).
fn shade(finish: Finish, class: f32, value: f32, albedo: Vec3, n: Vec3) -> (Vec3, Vec3) {
    let v = Vec3::Z;
    let r = n * (2.0 * n.dot(v)) - v;
    let fresnel = 0.04 + 0.96 * (1.0 - n.dot(v).max(0.0)).powi(5);
    let diffuse = |albedo: Vec3| {
        albedo * (AMBIENT + MOON_LIGHT * n.dot(MOON.normalize()).max(0.0) + LAMP_LIGHT * n.dot(LAMP.normalize()).max(0.0) * 0.6)
    };
    let specular = |rough: f32| {
        let power = 2.0 / (rough * rough).max(0.003) - 2.0;
        let blinn = |l: Vec3| {
            let h = (l.normalize() + v).normalize();
            n.dot(h).max(0.0).powf(power) * (power + 8.0) / 25.0
        };
        MOON_LIGHT * blinn(MOON) * 0.5 + LAMP_LIGHT * blinn(LAMP) * 0.4
    };
    if finish == Finish::Bottle {
        return (diffuse(albedo) * 1.3 + specular(0.15) * 0.6 + environment(r) * (fresnel * 0.8), Vec3::ZERO);
    }
    match Surface::from_code(class) {
        Surface::Light => (albedo * 0.2, albedo * value),
        Surface::Metal => {
            let rough = (value * 0.7).max(0.14);
            (albedo * (environment(r) * 1.4 + specular(rough) * 1.5), Vec3::ZERO)
        }
        Surface::Glass => (albedo * 0.5 + environment(r) * (0.15 + 0.85 * fresnel) + specular(0.08) * 0.5, Vec3::ZERO),
        Surface::Rubber => (diffuse(albedo), Vec3::ZERO),
        Surface::Paint => {
            let rough = value.clamp(0.2, 0.8);
            let gloss = ((0.5 - rough) / 0.2).clamp(0.0, 1.0);
            let base = diffuse(albedo) * 1.6 + specular(rough) * albedo.max(Vec3::splat(0.04));
            (base + (environment(r) * fresnel + specular(0.1) * 0.3) * gloss, Vec3::ZERO)
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

    /// Una caja de 4 x 1 x 2 (largo en z) con una LED atras.
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
        // De atras mide 1 de ancho; de lado, 4 (mas el margen del resplandor).
        assert!((back.width as f32 - (20.0 + 12.0)).abs() <= 2.0, "{}", back.width);
        assert!((side.width as f32 - (80.0 + 12.0)).abs() <= 2.0, "{}", side.width);
        // El centro es opaco, la orilla del margen transparente.
        let center = back.pixels[(back.height / 2) * back.width + back.width / 2];
        assert_eq!(center[3], 255);
        assert_eq!(back.pixels[0][3], 0);
        // De atras se ve la cara con la LED roja (z = +2, hacia la camara):
        // rojo encendido, y el resplandor sale del contorno.
        assert!(center[0] > 200 && center[2] < 120, "{center:?}");
        let outside = back.pixels[(back.height / 2) * back.width + 2];
        assert!(outside[0] > 10 && outside[3] == 0, "{outside:?}");
        // El suelo bajo el centro: abajo al centro.
        assert!((back.anchor.0 - back.width as f32 / 2.0).abs() < 1.5);
        assert!((back.anchor.1 - (back.height as f32 - 6.0)).abs() < 1.5);
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
