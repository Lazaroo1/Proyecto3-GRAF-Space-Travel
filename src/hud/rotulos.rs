//! Los rotulos: palabras forjadas, tejidas o pintadas en CPU.
//!
//! Cada estilo recibe el campo de distancias de la palabra (`letras::Word`)
//! y devuelve dos capas del tamano de la palabra: el **cuerpo** (RGBA, se
//! pone encima de la imagen) y la **luz** (RGB, se suma). El HUD las sube
//! como texturas y solo las pega; asi la luz puede latir sin volver a
//! hornear nada.
//!
//! - `forged` (OCASO): hierro negro con filo de bronce y grietas de lava.
//!   Todo sale de la distancia `d` al borde: el filo es la banda
//!   `-rim < d < 0` con un perfil de bisel que inclina la normal; la cara
//!   queda hundida (sombra del filo del lado de la luz); las grietas son la
//!   curva de nivel `-d = D(p)` con `D` quebrada por un ruido de aristas,
//!   asi corren a lo largo de los trazos gruesos como en una pieza que se
//!   raja al enfriarse, y alrededor se encienden las fisuras de un Worley.
//!   La luz es el halo `e^(-d/k)` con lenguas que suben (la misma distancia
//!   leida mas abajo) y, entre las letras, orbitas, runas y ojos de linea.
//! - `woven` (GUATEMALA): las letras cortadas de un tejido (la faja de la
//!   transicion en dos registros, con su trama) y bordadas con hilo dorado.
//! - `painted` (los minijuegos): rotulo de camioneta, amarillo a naranja,
//!   con filo oscuro, brillo arriba y el canto rojo hacia abajo a la derecha.

use crate::hud::letras::Word;
use crate::math::{Perlin, Vec3, hash_u32, smoothstep};
use crate::parallel;
use crate::render::transition;

/// Un rotulo horneado.
pub struct Layers {
    pub w: usize,
    pub h: usize,
    /// RGBA sin premultiplicar.
    pub body: Vec<u8>,
    /// Lo que se suma (RGBA con alfa 255), o vacio si el estilo no brilla.
    pub light: Vec<u8>,
}

#[inline]
fn lattice(x: i32, y: i32, seed: u32) -> f32 {
    let h = hash_u32((x as u32).wrapping_mul(0x9E37_79B9) ^ hash_u32((y as u32).wrapping_mul(0x85EB_CA6B) ^ seed));
    (h >> 8) as f32 / 16_777_216.0
}

/// Ruido de valor en `[0, 1]`: suave, o con aristas (`creased`, interpolado
/// en linea recta: las curvas de nivel salen quebradas).
#[inline]
fn value(x: f32, y: f32, seed: u32, creased: bool) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (mut fx, mut fy) = (x - xi, y - yi);
    if !creased {
        fx = fx * fx * (3.0 - 2.0 * fx);
        fy = fy * fy * (3.0 - 2.0 * fy);
    }
    let (xi, yi) = (xi as i32, yi as i32);
    let top = lattice(xi, yi, seed) + (lattice(xi + 1, yi, seed) - lattice(xi, yi, seed)) * fx;
    let bottom = lattice(xi, yi + 1, seed) + (lattice(xi + 1, yi + 1, seed) - lattice(xi, yi + 1, seed)) * fx;
    top + (bottom - top) * fy
}

#[inline]
fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * value(x * freq + 13.7 * o as f32, y * freq + 7.3 * o as f32, seed.wrapping_add(o), false);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Del rojo oscuro al blanco amarillento, como algo que se calienta.
#[inline]
fn heat(e: f32) -> Vec3 {
    let e = e.clamp(0.0, 1.0);
    let red = Vec3::new(0.55, 0.04, 0.0);
    let orange = Vec3::new(1.0, 0.38, 0.04);
    let yellow = Vec3::new(1.0, 0.8, 0.3);
    let white = Vec3::new(1.0, 0.97, 0.82);
    if e < 0.4 {
        red.lerp(orange, e / 0.4) * (0.25 + 0.75 * e / 0.4)
    } else if e < 0.75 {
        orange.lerp(yellow, (e - 0.4) / 0.35)
    } else {
        yellow.lerp(white, (e - 0.75) / 0.25)
    }
}

#[inline]
fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

// Lineas ---------------------------------------------------------------------

/// Un lienzo de tinta: cuanto hay de linea en cada pixel (0 a 1).
struct Ink {
    w: usize,
    h: usize,
    data: Vec<f32>,
}

impl Ink {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, data: vec![0.0; w * h] }
    }

    /// Un trazo de `a` a `b` de `width` pixeles (con el borde suave).
    fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, strength: f32) {
        let r = width * 0.5 + 1.0;
        let (x0, x1) = ((a.0.min(b.0) - r).floor().max(0.0) as usize, ((a.0.max(b.0) + r).ceil() as usize).min(self.w.saturating_sub(1)));
        let (y0, y1) = ((a.1.min(b.1) - r).floor().max(0.0) as usize, ((a.1.max(b.1) + r).ceil() as usize).min(self.h.saturating_sub(1)));
        let (bx, by) = (b.0 - a.0, b.1 - a.1);
        let len2 = (bx * bx + by * by).max(1e-6);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5 - a.0, y as f32 + 0.5 - a.1);
                let t = ((px * bx + py * by) / len2).clamp(0.0, 1.0);
                let d = (px - bx * t).hypot(py - by * t) - width * 0.5;
                let v = (0.5 - d).clamp(0.0, 1.0) * strength;
                let cell = &mut self.data[y * self.w + x];
                *cell = cell.max(v);
            }
        }
    }

    /// Una linea quebrada.
    fn path(&mut self, points: &[(f32, f32)], width: f32, strength: f32) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], width, strength);
        }
    }

    /// Una elipse inclinada `tilt` radianes; con `dash`, de rayitas.
    fn ellipse(&mut self, c: (f32, f32), r: (f32, f32), tilt: f32, width: f32, strength: f32, dash: bool) {
        let steps = ((r.0 + r.1) * 0.9) as usize + 24;
        let (sin, cos) = tilt.sin_cos();
        let at = |k: usize| {
            let a = k as f32 / steps as f32 * std::f32::consts::TAU;
            let (x, y) = (a.cos() * r.0, a.sin() * r.1);
            (c.0 + x * cos - y * sin, c.1 + x * sin + y * cos)
        };
        for k in 0..steps {
            if dash && (k / 3) % 2 == 1 {
                continue;
            }
            self.line(at(k), at(k + 1), width, strength);
        }
    }

    /// Un punto lleno.
    fn dot(&mut self, c: (f32, f32), radius: f32, strength: f32) {
        self.line(c, (c.0 + 0.01, c.1), radius * 2.0, strength);
    }

    /// Un ojo de linea: dos arcos, el iris y la pupila. `w` es su ancho.
    fn eye(&mut self, c: (f32, f32), w: f32, width: f32, strength: f32) {
        let n = 14;
        for side in [-1.0f32, 1.0] {
            let lid: Vec<(f32, f32)> = (0..=n)
                .map(|k| {
                    let t = k as f32 / n as f32 * 2.0 - 1.0;
                    (c.0 + t * w * 0.5, c.1 + side * (1.0 - t * t) * w * 0.27)
                })
                .collect();
            self.path(&lid, width, strength);
        }
        self.ellipse(c, (w * 0.16, w * 0.16), 0.0, width, strength, false);
        self.dot(c, w * 0.055, strength);
    }

    /// Una runa (`RUNES`) de alto `size` centrada en `c`.
    fn rune(&mut self, index: usize, c: (f32, f32), size: f32, width: f32, strength: f32) {
        for stroke in RUNES[index % RUNES.len()] {
            let points: Vec<(f32, f32)> = stroke.iter().map(|&(x, y)| (c.0 + (x - 0.5) * size * 0.62, c.1 + (y - 0.5) * size)).collect();
            self.path(&points, width, strength);
        }
    }
}

/// Runas del futhark antiguo como trazos en un cuadro de 1 x 1 (`y` hacia
/// abajo): fehu, raido, kaunan, algiz, tiwaz, othala, ansuz y gebo.
const RUNES: [&[&[(f32, f32)]]; 8] = [
    &[&[(0.2, 0.0), (0.2, 1.0)], &[(0.2, 0.55), (0.85, 0.2)], &[(0.2, 0.3), (0.7, 0.0)]],
    &[&[(0.2, 1.0), (0.2, 0.0), (0.8, 0.25), (0.2, 0.5), (0.85, 1.0)]],
    &[&[(0.8, 0.1), (0.25, 0.5), (0.8, 0.9)]],
    &[&[(0.5, 1.0), (0.5, 0.0)], &[(0.1, 0.0), (0.5, 0.45), (0.9, 0.0)]],
    &[&[(0.5, 1.0), (0.5, 0.0)], &[(0.1, 0.35), (0.5, 0.0), (0.9, 0.35)]],
    &[&[(0.1, 1.0), (0.9, 0.3), (0.5, 0.0), (0.1, 0.3), (0.9, 1.0)]],
    &[&[(0.25, 0.0), (0.25, 1.0)], &[(0.25, 0.0), (0.85, 0.35)], &[(0.25, 0.3), (0.85, 0.65)]],
    &[&[(0.1, 0.05), (0.9, 0.95)], &[(0.9, 0.05), (0.1, 0.95)]],
];

// Forjado --------------------------------------------------------------------

/// Hacia donde esta la luz que pega en el metal: arriba a la izquierda y
/// hacia quien mira.
const KEY: Vec3 = Vec3::new(-0.42, -0.62, 0.66);

/// OCASO: hierro con filo de bronce y grietas de lava. `grow` engorda las
/// letras (en centesimas del alto de las mayusculas).
pub fn forged(word: &Word, grow: f32, seed: u32) -> Layers {
    let (w, h) = (word.w, word.h);
    let cap = word.bottom - word.top;
    let u = cap / 100.0;
    let grow = grow * u;
    let rim = 2.3 * u;
    let cells = Perlin::new(seed ^ 0xC4A5);
    let ink = decor(word, u, grow);
    let key = KEY.normalize();
    let mut out = vec![0u8; w * h * 8];
    parallel::chunks_mut(&mut out, w * 8, 4, |y, row| {
        let fy = y as f32 + 0.5;
        for x in 0..w {
            let fx = x as f32 + 0.5;
            let d = word.at(fx, fy) - grow;
            let line = ink.data[y * w + x];
            let mut color = Vec3::ZERO;
            let mut alpha = 0.0;
            let mut light = Vec3::ZERO;
            // La sombra que dejan las letras, corrida hacia abajo.
            let shade = word.at(fx - 1.6 * u, fy - 2.6 * u) - grow;
            let cast = 0.62 * (-shade.max(0.0) / (4.5 * u)).exp();
            if d > 0.7 {
                alpha = cast;
            }
            if d < 0.7 {
                let (nx, ny) = word.normal(fx, fy);
                let depth = -d;
                let grain = fbm(fx / (0.55 * u), fy / (0.55 * u), seed, 3);
                let blotch = fbm(fx / (9.0 * u), fy / (9.0 * u), seed ^ 77, 3);
                let cover = smoothstep(0.7, -0.7, d);
                if depth < rim * 1.25 {
                    // El filo: sube, corre plano y baja hacia la cara, y
                    // despues una ranura oscura.
                    let t = (depth / rim).max(0.0);
                    let tilt = if t < 0.38 { 0.85 * (1.0 - t / 0.38).powf(0.6) } else if t < 0.62 { 0.0 } else { -0.8 * ((t - 0.62) / 0.38).min(1.0) };
                    let n = Vec3::new(nx * tilt, ny * tilt, 1.0).normalize();
                    let diffuse = n.dot(key).max(0.0);
                    let half = (key + Vec3::Z).normalize();
                    let spec = n.dot(half).max(0.0).powf(26.0);
                    let bronze = Vec3::new(0.60, 0.385, 0.225) * (0.7 + 0.6 * grain) * (0.8 + 0.4 * blotch);
                    color = bronze * (0.2 + 1.05 * diffuse) + Vec3::new(1.0, 0.82, 0.58) * spec * 0.85;
                    // La lava le pega por debajo: un tinte calido.
                    color += Vec3::new(0.32, 0.1, 0.02) * (0.35 + 0.65 * ny.max(0.0)) * 0.5;
                    let groove = smoothstep(0.92, 1.05, t) * smoothstep(1.28, 1.1, t);
                    color = color * (1.0 - 0.8 * groove);
                } else {
                    // La cara: hierro granulado, hundida bajo el filo.
                    let inner = depth - rim * 1.25;
                    let speck = lattice(x as i32, y as i32, seed ^ 5);
                    let iron = Vec3::new(0.12, 0.112, 0.122).lerp(Vec3::new(0.33, 0.3, 0.295), grain * grain) * (0.75 + 0.5 * blotch);
                    let iron = iron * (0.9 + 0.2 * speck);
                    // El filo tapa la luz del lado de donde viene y la
                    // rebota del otro.
                    let facing = -(nx * key.x + ny * key.y) / key.x.hypot(key.y);
                    let wall = (-inner / (2.6 * u)).exp();
                    let lit = 1.0 - 0.6 * wall * facing.max(0.0) + 0.55 * wall * (-facing).max(0.0);
                    color = iron * lit;

                    // Las grietas: la curva de nivel `depth = D(p)` con `D`
                    // quebrada, a pedazos (`piece`), mas anchas abajo.
                    let jag = value(fx / (5.5 * u), fy / (5.5 * u), seed ^ 31, true) + 0.5 * value(fx / (2.1 * u), fy / (2.1 * u), seed ^ 32, true);
                    let target = rim * 1.25 + (0.7 + 3.3 * jag) * u;
                    let c = (inner + rim * 1.25 - target).abs();
                    let low = (fy - word.top) / cap;
                    let piece = fbm(fx / (26.0 * u) + nx * 1.3, fy / (26.0 * u) + ny * 1.3, seed ^ 9, 2);
                    let piece = smoothstep(0.5 - 0.1 * low, 0.6 - 0.1 * low, piece);
                    let core = smoothstep(1.0 * u, 0.15 * u, c) * piece.sqrt();
                    let glow = (-c / (3.0 * u)).exp() * piece;
                    // Las fisuras: los bordes de las celdas de un Worley,
                    // encendidas solo cerca de una grieta.
                    let (f1, f2) = cells.cells(Vec3::new(fx / (7.0 * u), fy / (7.0 * u), 3.3));
                    let fissure = smoothstep(0.09, 0.015, f2 - f1);
                    let vein = fissure * (-c / (4.0 * u)).exp() * piece;
                    let e = (core * 0.64 + glow * 0.3 + vein * 0.42).min(1.0);
                    // Quemado alrededor y oscuro en las fisuras frias.
                    color = color * (1.0 - 0.45 * fissure * (1.0 - vein)) * (1.0 - 0.35 * glow);
                    color += heat(e) * e.min(1.0);
                    light += heat(e) * (core * 0.22 + glow * 0.14);
                    // Los ojos grabados: una linea de bronce.
                    color = color.lerp(Vec3::new(0.78, 0.52, 0.3), line * 0.9);
                }
                alpha = cover + (1.0 - cover) * cast;
                color = color * (cover / alpha.max(1e-3));
            }
            if d > 0.0 {
                // El halo y las lenguas que suben (la distancia leida mas
                // abajo, con el aire revuelto).
                let air = fbm(fx / (14.0 * u), fy / (20.0 * u), seed ^ 61, 3);
                let halo = (-d / (5.0 * u)).exp();
                light += Vec3::new(1.0, 0.4, 0.1) * halo * (0.3 + 0.25 * air);
                let sway = (fbm(fx / (10.0 * u), fy / (30.0 * u), seed ^ 62, 2) - 0.5) * 9.0 * u;
                let below = (word.at(fx + sway, fy + 11.0 * u) - grow).max(0.0);
                let tongue = (-below / (7.0 * u)).exp() * smoothstep(0.35, 0.75, fbm(fx / (6.0 * u), fy / (16.0 * u), seed ^ 63, 3));
                light += Vec3::new(0.95, 0.2, 0.04) * tongue * 0.16;
                // Las orbitas y las runas, solo fuera de las letras.
                light += Vec3::new(1.0, 0.66, 0.38) * line * 0.6 * smoothstep(0.3, 1.6, d);
            }
            let px = &mut row[x * 8..x * 8 + 8];
            px[0] = byte(color.x);
            px[1] = byte(color.y);
            px[2] = byte(color.z);
            px[3] = byte(alpha);
            // La luz se comprime para que no se queme: 1 - e^-x.
            px[4] = byte(1.0 - (-light.x * 1.6).exp());
            px[5] = byte(1.0 - (-light.y * 1.6).exp());
            px[6] = byte(1.0 - (-light.z * 1.6).exp());
            px[7] = 255;
        }
    });
    split(out, w, h)
}

/// Separa las dos capas que se hornearon juntas (8 bytes por pixel).
fn split(out: Vec<u8>, w: usize, h: usize) -> Layers {
    let mut body = Vec::with_capacity(w * h * 4);
    let mut light = Vec::with_capacity(w * h * 4);
    for px in out.chunks_exact(8) {
        body.extend_from_slice(&px[..4]);
        light.extend_from_slice(&px[4..]);
    }
    Layers { w, h, body, light }
}

/// Lo que va de linea alrededor de OCASO: orbitas con sus planetas, runas
/// y, en las O, un sello y un ojo (el motivo del juego), mas los ojitos
/// grabados en sus trazos.
fn decor(word: &Word, u: f32, grow: f32) -> Ink {
    let mut ink = Ink::new(word.w, word.h);
    let cap = word.bottom - word.top;
    let cy = (word.top + word.bottom) * 0.5;
    let thin = (0.55 * u).max(1.0);
    let centers: Vec<f32> = word.spans.iter().map(|&(a, b)| (a + b) * 0.5).collect();
    let n = centers.len();
    if n == 0 {
        return ink;
    }
    // Una orbita inclinada alrededor de cada letra, con un planeta.
    for (i, &cx) in centers.iter().enumerate() {
        let wide = (word.spans[i].1 - word.spans[i].0) * 0.5;
        let h1 = hash_u32(i as u32 * 31 + 7);
        let r = |k: u32| (hash_u32(h1 ^ k) >> 8) as f32 / 16_777_216.0;
        let radius = (wide * (1.12 + 0.3 * r(1)), cap * (0.2 + 0.14 * r(2)));
        let tilt = -0.42 + 0.5 * r(3);
        let c = (cx + (r(4) - 0.5) * 0.2 * cap, cy + (r(5) - 0.5) * 0.3 * cap);
        ink.ellipse(c, radius, tilt, thin, 0.55, i % 2 == 1);
        let a = r(6) * std::f32::consts::TAU;
        let (sin, cos) = tilt.sin_cos();
        let (px, py) = (a.cos() * radius.0, a.sin() * radius.1);
        ink.dot((c.0 + px * cos - py * sin, c.1 + px * sin + py * cos), 1.5 * u, 1.0);
    }
    // Runas arriba y abajo de los huecos, y entre las letras.
    for (i, &cx) in centers.iter().enumerate() {
        ink.rune(i * 3, (cx, cy - 0.27 * cap), 0.085 * cap, thin * 1.2, 0.9);
        ink.rune(i * 3 + 5, (cx, cy + 0.3 * cap), 0.075 * cap, thin * 1.2, 0.9);
    }
    // La primera O: un sello (circulo con cruz). La ultima: el ojo.
    let first = (centers[0], cy);
    ink.ellipse(first, (0.085 * cap, 0.085 * cap), 0.0, thin, 0.8, false);
    for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        ink.line((first.0 + dx * 0.06 * cap, first.1 + dy * 0.06 * cap), (first.0 + dx * 0.15 * cap, first.1 + dy * 0.15 * cap), thin, 0.8);
    }
    let last = (centers[n - 1], cy);
    ink.eye(last, 0.3 * cap, thin * 1.2, 1.0);
    ink.ellipse(last, (0.2 * cap, 0.2 * cap), 0.0, thin, 0.45, true);
    ink.ellipse(last, (0.26 * cap, 0.1 * cap), 0.0, thin, 0.4, false);
    // Los ojitos grabados en los trazos de las dos O: a media altura, en el
    // centro de cada trazo (lo que mide el trazo se lee de la distancia).
    for &i in &[0, n - 1] {
        let (a, b) = word.spans[i];
        for from_left in [true, false] {
            let mut run: Option<(f32, f32)> = None;
            let steps = ((b - a) * 0.5) as usize;
            for k in 0..steps {
                let x = if from_left { a - grow + k as f32 } else { b + grow - k as f32 };
                let inside = word.at(x, cy) - grow < 0.0;
                match (&mut run, inside) {
                    (None, true) => run = Some((x, x)),
                    (Some(r), true) => r.1 = x,
                    (Some(_), false) => break,
                    _ => {}
                }
            }
            if let Some((s, e)) = run {
                let thick = (e - s).abs();
                if thick > 6.0 * u {
                    ink.eye(((s + e) * 0.5, cy), thick * 0.62, thin, 1.0);
                }
            }
        }
    }
    ink
}

// Tejido ---------------------------------------------------------------------

/// GUATEMALA: letras cortadas de un tejido y bordadas.
pub fn woven(word: &Word) -> Layers {
    let (w, h) = (word.w, word.h);
    let cap = word.bottom - word.top;
    let u = cap / 100.0;
    let gold = Vec3::new(0.98, 0.78, 0.2);
    let navy = Vec3::new(0.05, 0.06, 0.2);
    let mut body = vec![0u8; w * h * 4];
    parallel::chunks_mut(&mut body, w * 4, 4, |y, row| {
        let fy = y as f32 + 0.5;
        for x in 0..w {
            let fx = x as f32 + 0.5;
            let d = word.at(fx, fy);
            // De adentro hacia afuera: tejido, hilo dorado, filo azul.
            let border = 3.4 * u;
            let outline = border + 2.2 * u;
            let shadow = word.at(fx - 2.4 * u, fy - 3.4 * u) - outline;
            let cast = 0.6 * smoothstep(5.0 * u, -1.0 * u, shadow);
            if d > outline + 1.0 {
                row[x * 4 + 3] = byte(cast);
                continue;
            }
            let (nx, ny) = word.normal(fx, fy);
            // La tela: dos fajas, una sobre otra, con la trama encima.
            let v = ((fy - word.top) / cap).clamp(0.0, 0.999);
            let register = (v * 2.0).fract();
            let along = fx / (cap * 0.5 * 5.0) + if v < 0.5 { 0.0 } else { 0.37 };
            let mut cloth = transition::sash(along / 4.0, register, 0.02);
            let weave = 0.9 + 0.1 * ((fx / (0.9 * u)).sin() * (fy / (0.9 * u)).sin()).signum();
            cloth = cloth * weave;
            // Abultada: luz arriba a la izquierda, sombra abajo.
            let edge = (d / (5.0 * u)).exp().min(1.0);
            cloth = cloth * (1.0 - 0.35 * edge * (nx * 0.5 + ny * 0.8).max(0.0)) + Vec3::splat(0.22) * edge * (-(nx * 0.5 + ny * 0.8)).max(0.0);
            // El hilo: puntadas a lo largo del borde (rayitas cruzadas).
            let stitch = 0.82 + 0.18 * ((fx + fy) / (1.1 * u)).sin().signum();
            let t = d / border;
            let round = 1.0 - (2.0 * t - 1.0) * (2.0 * t - 1.0);
            let thread = gold * stitch * (0.62 + 0.5 * round.max(0.0) - 0.25 * (nx * 0.5 + ny * 0.8));
            let mut color = cloth.lerp(thread, smoothstep(-0.6, 0.6, d));
            color = color.lerp(navy, smoothstep(border - 0.6, border + 0.6, d));
            let cover = smoothstep(outline + 0.7, outline - 0.7, d);
            let alpha = cover + (1.0 - cover) * cast;
            let color = color * (cover / alpha.max(1e-3));
            row[x * 4] = byte(color.x);
            row[x * 4 + 1] = byte(color.y);
            row[x * 4 + 2] = byte(color.z);
            row[x * 4 + 3] = byte(alpha);
        }
    });
    Layers { w, h, body, light: Vec::new() }
}

/// GUATEMALA tejida, con un lazo de papel picado colgado encima: banderas
/// de papel de china de colores, caladas (circulos, un rombo con su centro,
/// picos) y con la orilla de abajo en ondas, sobre una pita que cuelga.
pub fn garlanded(word: &Word) -> Layers {
    let letters = woven(word);
    let (w, h) = (letters.w, letters.h);
    let cap = word.bottom - word.top;
    let flag_h = cap * 0.4;
    let sag = cap * 0.1;
    // Cuanto crece la imagen hacia arriba para que quepa el lazo.
    let extra = (flag_h + sag + cap * 0.06 - word.top).max(0.0).ceil() as usize + 2;
    let mut body = vec![0u8; w * (h + extra) * 4];
    body[w * extra * 4..].copy_from_slice(&letters.body);
    let (x0, x1) = (w as f32 * 0.015, w as f32 * 0.985);
    let count = 15.0;
    let papers = [
        Vec3::new(0.93, 0.16, 0.5),
        Vec3::new(0.1, 0.72, 0.72),
        Vec3::new(0.99, 0.8, 0.14),
        Vec3::new(0.98, 0.45, 0.1),
        Vec3::new(0.2, 0.68, 0.28),
        Vec3::new(0.52, 0.2, 0.7),
        Vec3::new(0.16, 0.4, 0.88),
    ];
    let rows = (extra as f32 + word.top).ceil() as usize;
    for y in 0..rows.min(h + extra) {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let t = (fx - x0) / (x1 - x0);
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let string = 2.0 + cap * 0.02 + sag * 4.0 * t * (1.0 - t);
            let px = &mut body[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let under = px[3] as f32 / 255.0;
            // La bandera de esta columna, en sus coordenadas (0 a 1).
            let index = (t * count).floor();
            let u = (t * count).fract() / 0.86;
            let v = (fy - string) / flag_h;
            let soft = 1.5 / flag_h;
            let mut alpha = 0.0;
            let mut color = Vec3::ZERO;
            if u < 1.0 && (0.0..1.0).contains(&v) {
                let waves = 0.9 + 0.1 * (u * 5.0 * std::f32::consts::PI).sin().abs();
                let mut paper = smoothstep(waves + soft, waves - soft, v) * smoothstep(0.0, soft * 2.0, u.min(1.0 - u));
                // Lo calado.
                let dots = ((u * 6.0).fract() - 0.5).hypot((v - 0.17) * 6.0) - 0.28;
                paper *= smoothstep(-0.04, 0.04, dots);
                let diamond = (u - 0.5).abs() + (v - 0.52).abs() * 0.75;
                paper *= 1.0 - smoothstep(0.24, 0.22, diamond) * smoothstep(0.1, 0.12, diamond);
                let teeth = (((u * 7.0).fract() - 0.5).abs() * 0.16 + (v - 0.8).abs()) - 0.07;
                paper *= smoothstep(-0.01, 0.01, teeth);
                let side = ((u - 0.5).abs() - 0.36).abs() + (v - 0.52).abs() * 0.6;
                paper *= smoothstep(0.05, 0.07, side);
                alpha = paper * 0.96;
                color = papers[index as usize % papers.len()] * (1.05 - 0.25 * v);
            }
            // La pita.
            let line = smoothstep(1.6, 0.6, (fy - string).abs());
            color = color.lerp(Vec3::new(0.96, 0.92, 0.84), line);
            alpha = alpha.max(line);
            if alpha <= 0.0 {
                continue;
            }
            // Detras de las letras, si se enciman.
            let shown = alpha * (1.0 - under);
            let total = under + shown;
            for (c, value) in [color.x, color.y, color.z].into_iter().enumerate() {
                let old = px[c] as f32 / 255.0;
                px[c] = byte((old * under + value * shown) / total.max(1e-3));
            }
            px[3] = byte(total);
        }
    }
    Layers { w, h: h + extra, body, light: Vec::new() }
}

// Pintado --------------------------------------------------------------------

/// Los colores de un rotulo pintado: arriba y abajo de la letra, el canto y
/// el filo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub top: Vec3,
    pub bottom: Vec3,
    pub side: Vec3,
    pub edge: Vec3,
}

impl Paint {
    /// Amarillo a naranja con el canto rojo: el de los titulos.
    pub const SUN: Paint = Paint { top: Vec3::new(1.0, 0.88, 0.36), bottom: Vec3::new(1.0, 0.56, 0.12), side: Vec3::new(0.72, 0.1, 0.16), edge: Vec3::new(0.16, 0.03, 0.07) };
    /// Crema con el canto azul: el de los numeros.
    pub const CREAM: Paint = Paint { top: Vec3::new(1.0, 0.97, 0.88), bottom: Vec3::new(0.98, 0.84, 0.62), side: Vec3::new(0.12, 0.24, 0.55), edge: Vec3::new(0.04, 0.05, 0.16) };
    /// Turquesa: los avisos buenos.
    pub const JADE: Paint = Paint { top: Vec3::new(0.62, 1.0, 0.86), bottom: Vec3::new(0.1, 0.72, 0.6), side: Vec3::new(0.05, 0.3, 0.36), edge: Vec3::new(0.02, 0.09, 0.12) };
    /// Rosa mexicano: los avisos de peligro.
    pub const ROSE: Paint = Paint { top: Vec3::new(1.0, 0.72, 0.8), bottom: Vec3::new(0.94, 0.22, 0.42), side: Vec3::new(0.45, 0.04, 0.2), edge: Vec3::new(0.14, 0.02, 0.08) };
}

/// Rotulo de camioneta: la letra de color de arriba a abajo con un brillo,
/// su filo oscuro y el canto hacia abajo a la derecha.
pub fn painted(word: &Word, paint: Paint) -> Layers {
    let (w, h) = (word.w, word.h);
    let cap = word.bottom - word.top;
    let u = cap / 100.0;
    let edge = (3.2 * u).max(1.2);
    let deep = 7.0 * u;
    let mut body = vec![0u8; w * h * 4];
    parallel::chunks_mut(&mut body, w * 4, 4, |y, row| {
        let fy = y as f32 + 0.5;
        for x in 0..w {
            let fx = x as f32 + 0.5;
            let d = word.at(fx, fy);
            // El canto: la letra corrida en diagonal, lo mas cerca que pase.
            let mut side = f32::MAX;
            for k in 1..=6 {
                let s = deep * k as f32 / 6.0;
                side = side.min(word.at(fx - s * 0.7, fy - s));
            }
            let solid = d.min(side) - edge;
            let shadow = word.at(fx - deep * 0.7 - 2.0 * u, fy - deep - 3.0 * u) - edge;
            let cast = 0.5 * smoothstep(6.0 * u, -1.0 * u, shadow);
            if solid > 1.0 {
                row[x * 4 + 3] = byte(cast);
                continue;
            }
            let v = ((fy - word.top) / cap).clamp(0.0, 1.0);
            let mut face = paint.top.lerp(paint.bottom, smoothstep(0.15, 0.95, v));
            // El brillo: una franja clara pegada al borde de arriba.
            let (_, ny) = word.normal(fx, fy);
            let shine = smoothstep(-5.5 * u, -2.0 * u, d) * smoothstep(-0.6 * u, -2.0 * u, d) * (-ny).max(0.0);
            face = face.lerp(Vec3::ONE, 0.7 * shine);
            // De adentro hacia afuera: cara, filo, canto, filo.
            let mut color = paint.side * (0.75 + 0.25 * (1.0 - v));
            color = color.lerp(paint.edge, smoothstep(-0.7, 0.7, side.min(d)));
            color = color.lerp(paint.edge, smoothstep(edge + 0.7, edge - 0.7, d));
            color = color.lerp(face, smoothstep(0.7, -0.7, d));
            let cover = smoothstep(0.7, -0.7, solid);
            let alpha = cover + (1.0 - cover) * cast;
            let color = color * (cover / alpha.max(1e-3));
            row[x * 4] = byte(color.x);
            row[x * 4 + 1] = byte(color.y);
            row[x * 4 + 2] = byte(color.z);
            row[x * 4 + 3] = byte(alpha);
        }
    });
    Layers { w, h, body, light: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::letras::font_bytes;

    /// Pone un rotulo sobre un fondo y lo guarda (para verlo sin ventana).
    fn save(layers: &Layers, name: &str, back: [f32; 3]) {
        let Ok(dir) = std::env::var("OCASO_PNG") else { return };
        let mut rgba = vec![255u8; layers.w * layers.h * 4];
        for i in 0..layers.w * layers.h {
            let a = layers.body[i * 4 + 3] as f32 / 255.0;
            for c in 0..3 {
                // Un fondo con algo de dibujo, para ver la sombra.
                let stripe = if (i % layers.w / 40 + i / layers.w / 40) % 2 == 0 { 1.0 } else { 0.8 };
                let mut v = back[c] * stripe * (1.0 - a) + layers.body[i * 4 + c] as f32 / 255.0 * a;
                if !layers.light.is_empty() {
                    v += layers.light[i * 4 + c] as f32 / 255.0;
                }
                rgba[i * 4 + c] = byte(v);
            }
        }
        std::fs::create_dir_all(&dir).unwrap();
        let path = std::path::Path::new(&dir).join(name);
        crate::render::png::write_rgba(&path, layers.w as u32, layers.h as u32, &rgba).unwrap();
    }

    /// Los tres rotulos en PNG (`OCASO_PNG=carpeta cargo test --release
    /// rotulos_en_png -- --ignored --nocapture`).
    #[test]
    #[ignore = "guarda fotos"]
    fn rotulos_en_png() {
        let size: f32 = std::env::var("OCASO_ALTO").ok().and_then(|v| v.parse().ok()).unwrap_or(330.0);
        let start = std::time::Instant::now();
        let roman = font_bytes(crate::hud::ROMAN_FONT).expect("una letra romana");
        let word = Word::new(&roman, "OCASO", size, size * 0.09, size * 0.3).unwrap();
        println!("OCASO {}x{} campo en {:.0} ms", word.w, word.h, start.elapsed().as_secs_f64() * 1000.0);
        let start = std::time::Instant::now();
        let layers = forged(&word, 2.5, 7);
        println!("forjado en {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
        save(&layers, "ocaso.png", [0.2, 0.07, 0.16]);

        let fat = font_bytes(crate::hud::FAT_FONT).expect("una letra gorda");
        let word = Word::new(&fat, "GUATEMALA", size * 0.7, size * 0.02, size * 0.12).unwrap();
        let start = std::time::Instant::now();
        save(&garlanded(&word), "guatemala.png", [0.1, 0.25, 0.5]);
        println!("tejido en {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
        for (name, text, paint) in [("surfers.png", "Talega Surfers!", Paint::SUN), ("puntos.png", "2 590", Paint::CREAM), ("gano.png", "¡Ganaste!", Paint::JADE), ("perdio.png", "Te agarraron", Paint::ROSE), ("digitos.png", "5185 puntos Q-17 3º 100 m", Paint::CREAM)] {
            let figures = font_bytes(crate::hud::DIGIT_FONT);
            let word = Word::with_digits(&fat, figures.as_deref(), text, size * 0.3, 0.0, size * 0.06).unwrap();
            save(&painted(&word, paint), name, [0.07, 0.08, 0.2]);
        }
    }
}
