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

#[cfg(test)]
mod tests {
    use super::*;

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
