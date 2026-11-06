//! Figuras 2D para pintar en CPU sobre la imagen RGBA8: cajas redondeadas,
//! discos, capsulas (segmentos con radio), arcos y poligonos convexos (el
//! barrilete), cada una con su distancia
//! con signo al borde. La cobertura de cada pixel sale de esa distancia
//! (`lienzo::cover`), asi los bordes quedan suaves sin muestrear de mas, y
//! afuera puede ir un resplandor `exp(-d / alcance)`. Una `Canvas` junta las
//! figuras de un frame y `paint` las pinta fila por fila (en paralelo, desde
//! quien la llama), en orden; con profundidad, detras de lo que este mas
//! cerca. La usan los shucos y Talega Surfers.
//!
//! **Volumen**: una figura con `vol` se sombrea como si fuera de barro: su
//! distancia al borde dice que tan inclinada esta la superficie. A `e`
//! pixeles adentro del borde, con un perfil de circulo de radio `vol`, la
//! normal es `(g q, sqrt(1 - q^2))` con `q = 1 - e / vol` (en el borde mira
//! de lado, `q = 1`; a `vol` adentro mira de frente) y `g` el gradiente de
//! la distancia (hacia afuera). Con esa normal va Lambert con la luz
//! principal, la del cielo, un brillo de Blinn-Phong si es lustrosa y la
//! contraluz en la orilla (`q^4`).

use super::lienzo::{self, cover, halo, over, round_box};
use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub enum Form {
    /// Caja redondeada con medios lados y radio de esquina.
    Rect { hx: f32, hy: f32, r: f32 },
    Disc { r: f32 },
    /// Segmento de `(x, y)` a `(bx, by)` con radio `r` (una capsula).
    Line { bx: f32, by: f32, r: f32 },
    /// Arco de radio `r` y medio grueso `t` entre los angulos `from` y `to`
    /// (con y hacia abajo: de 0 a pi es la mitad de abajo).
    Arc { r: f32, t: f32, from: f32, to: f32 },
    /// Poligono convexo de hasta 8 vertices (relativos a `(x, y)`, en
    /// cualquier orden de giro).
    Poly { n: u8, pts: [(f32, f32); 8] },
    /// Elipse con semiejes `rx` y `ry`.
    Ellipse { rx: f32, ry: f32 },
}

/// Una figura: su forma, su color, cuanto cubre, que tan suave es su borde
/// (pixeles) y su resplandor (`glow` pixeles de alcance, `glow_k` fuerza).
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub x: f32,
    pub y: f32,
    pub form: Form,
    pub color: Vec3,
    pub alpha: f32,
    pub soft: f32,
    pub glow: f32,
    pub glow_k: f32,
    /// A que profundidad esta (m, 0 si va siempre encima): se esconde
    /// detras de lo que este mas cerca en el buffer de profundidad.
    pub depth: f32,
    /// El volumen (pixeles del perfil, 0 es plana), lo lustrosa (0 a 1) y
    /// cuanto emite (para el brillo del posproceso, de 0 a 1).
    pub vol: f32,
    pub gloss: f32,
    pub emit: f32,
    /// La caja que ocupa (sin el borde suave ni el resplandor) y cuanto
    /// agregarle.
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    pad: f32,
}

impl Shape {
    pub fn alpha(&mut self, a: f32) -> &mut Self {
        self.alpha = a;
        self
    }

    pub fn soft(&mut self, s: f32) -> &mut Self {
        self.soft = s.max(0.5);
        self.repad();
        self
    }

    pub fn glow(&mut self, spread: f32, k: f32) -> &mut Self {
        self.glow = spread;
        self.glow_k = k;
        self.repad();
        self
    }

    /// Que se sombree con volumen: el perfil de `r` pixeles.
    pub fn vol(&mut self, r: f32) -> &mut Self {
        self.vol = r.max(0.0);
        self
    }

    pub fn gloss(&mut self, g: f32) -> &mut Self {
        self.gloss = g;
        self
    }

    pub fn emit(&mut self, e: f32) -> &mut Self {
        self.emit = e;
        self
    }

    fn repad(&mut self) {
        self.pad = 1.0 + 1.5 * self.soft + 5.0 * self.glow;
    }

    /// Distancia con signo (pixeles) del punto al borde.
    pub fn sdf(&self, px: f32, py: f32) -> f32 {
        let (dx, dy) = (px - self.x, py - self.y);
        match self.form {
            Form::Rect { hx, hy, r } => round_box(dx, dy, hx, hy, r),
            Form::Disc { r } => dx.hypot(dy) - r,
            Form::Line { bx, by, r } => segment(px, py, (self.x, self.y), (bx, by)) - r,
            Form::Arc { r, t, from, to } => {
                let angle = dy.atan2(dx);
                if angle >= from && angle <= to {
                    (dx.hypot(dy) - r).abs() - t
                } else {
                    let end = |a: f32| {
                        let (s, c) = a.sin_cos();
                        (dx - c * r).hypot(dy - s * r)
                    };
                    end(from).min(end(to)) - t
                }
            }
            Form::Poly { n, pts } => poly(dx, dy, &pts[..n as usize]),
            Form::Ellipse { rx, ry } => ellipse(dx, dy, rx, ry),
        }
    }

    /// Hacia donde crece la distancia (unitario, hacia afuera del borde).
    pub fn grad(&self, px: f32, py: f32) -> (f32, f32) {
        let (dx, dy) = (px - self.x, py - self.y);
        let unit = |x: f32, y: f32| {
            let l = x.hypot(y);
            if l > 1e-6 { (x / l, y / l) } else { (0.0, 0.0) }
        };
        match self.form {
            Form::Rect { hx, hy, r } => {
                let r = r.min(hx).min(hy).max(0.0);
                let (qx, qy) = (dx.abs() - hx + r, dy.abs() - hy + r);
                let (sx, sy) = (dx.signum(), dy.signum());
                if qx > 0.0 && qy > 0.0 {
                    let (gx, gy) = unit(qx, qy);
                    (gx * sx, gy * sy)
                } else if qx > qy {
                    (sx, 0.0)
                } else {
                    (0.0, sy)
                }
            }
            Form::Disc { .. } => unit(dx, dy),
            Form::Line { bx, by, .. } => {
                let (abx, aby) = (bx - self.x, by - self.y);
                let len2 = abx * abx + aby * aby;
                let t = if len2 > 0.0 { ((dx * abx + dy * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
                unit(dx - abx * t, dy - aby * t)
            }
            Form::Arc { r, .. } => {
                let (gx, gy) = unit(dx, dy);
                let k = if dx.hypot(dy) >= r { 1.0 } else { -1.0 };
                (gx * k, gy * k)
            }
            Form::Ellipse { rx, ry } => unit(dx / (rx * rx), dy / (ry * ry)),
            Form::Poly { .. } => {
                let e = 0.5;
                unit(self.sdf(px + e, py) - self.sdf(px - e, py), self.sdf(px, py + e) - self.sdf(px, py - e))
            }
        }
    }
}

/// Distancia con signo a una elipse (aproximada: `k0 (k0 - 1) / k1`, exacta
/// en el borde y en un circulo).
#[inline]
pub fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> f32 {
    let (rx, ry) = (rx.max(1e-3), ry.max(1e-3));
    let k0 = (x / rx).hypot(y / ry);
    let k1 = (x / (rx * rx)).hypot(y / (ry * ry));
    if k1 < 1e-9 { -rx.min(ry) } else { k0 * (k0 - 1.0) / k1 }
}

/// La luz con que se sombrean las figuras con volumen. En la pantalla: x a
/// la derecha, y hacia abajo y z hacia quien mira.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// Hacia donde esta la luz principal (unitario) y su color.
    pub key: Vec3,
    pub key_color: Vec3,
    /// La luz que llega de todos lados (el cielo).
    pub ambient: Vec3,
    /// La contraluz: lo que brilla la orilla.
    pub rim: Vec3,
}

impl Default for Light {
    fn default() -> Self {
        Self { key: Vec3::new(-0.35, -0.55, 0.76).normalize(), key_color: Vec3::splat(0.5), ambient: Vec3::splat(0.62), rim: Vec3::ZERO }
    }
}

impl Light {
    /// El color de una figura con volumen a `d` pixeles de su borde.
    #[inline]
    fn shade(&self, sh: &Shape, half: Vec3, xf: f32, yf: f32, d: f32) -> Vec3 {
        let (gx, gy) = sh.grad(xf, yf);
        let q = (1.0 + d / sh.vol).clamp(0.0, 1.0);
        let nz = (1.0 - q * q).max(0.0).sqrt();
        let n = Vec3::new(gx * q, gy * q, nz);
        let mut c = sh.color * (self.ambient + self.key_color * n.dot(self.key).max(0.0));
        if sh.gloss > 0.0 {
            // `(n . h)^32` con cinco cuadrados.
            let mut s = n.dot(half).max(0.0);
            s *= s;
            s *= s;
            s *= s;
            s *= s;
            s *= s;
            c = c + self.key_color * (s * sh.gloss);
        }
        let r = q * q;
        c + self.rim * (r * r)
    }
}

/// Distancia con signo a un poligono convexo: adentro, la mayor de las
/// distancias (negativas) a sus lados; afuera, la distancia al lado o al
/// vertice mas cercano (exacta).
pub fn poly(x: f32, y: f32, pts: &[(f32, f32)]) -> f32 {
    let n = pts.len();
    if n < 3 {
        return f32::MAX;
    }
    // El giro: con el area con signo, cada lado sabe de que lado queda
    // adentro.
    let area: f32 = (0..n).map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1).sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let mut inside = f32::MIN;
    let mut edge = f32::MAX;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let len = ex.hypot(ey).max(1e-6);
        // Lo que esta afuera de este lado (positivo afuera).
        let out = -sign * (ex * (y - a.1) - ey * (x - a.0)) / len;
        inside = inside.max(out);
        edge = edge.min(segment(x, y, a, b));
    }
    if inside <= 0.0 { inside } else { edge }
}

/// Distancia de `(x, y)` al segmento `a b`.
pub fn segment(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (x - a.0, y - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 { ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (apx - abx * t).hypot(apy - aby * t)
}

#[derive(Default)]
pub struct Canvas {
    pub shapes: Vec<Shape>,
    /// La profundidad que llevan las figuras que se pongan (0: ninguna).
    pub depth: f32,
}

impl Canvas {
    fn push(&mut self, x: f32, y: f32, form: Form, color: Vec3, bounds: (f32, f32, f32, f32)) -> &mut Shape {
        let mut shape = Shape { x, y, form, color, alpha: 1.0, soft: 1.0, glow: 0.0, glow_k: 0.0, depth: self.depth, vol: 0.0, gloss: 0.0, emit: 0.0, x0: bounds.0, y0: bounds.1, x1: bounds.2, y1: bounds.3, pad: 0.0 };
        shape.repad();
        self.shapes.push(shape);
        self.shapes.last_mut().expect("recien puesta")
    }

    pub fn rect(&mut self, x: f32, y: f32, hx: f32, hy: f32, r: f32, color: Vec3) -> &mut Shape {
        self.push(x, y, Form::Rect { hx, hy, r }, color, (x - hx, y - hy, x + hx, y + hy))
    }

    pub fn disc(&mut self, x: f32, y: f32, r: f32, color: Vec3) -> &mut Shape {
        self.push(x, y, Form::Disc { r }, color, (x - r, y - r, x + r, y + r))
    }

    pub fn line(&mut self, ax: f32, ay: f32, bx: f32, by: f32, r: f32, color: Vec3) -> &mut Shape {
        self.push(ax, ay, Form::Line { bx, by, r }, color, (ax.min(bx) - r, ay.min(by) - r, ax.max(bx) + r, ay.max(by) + r))
    }

    pub fn arc(&mut self, x: f32, y: f32, r: f32, t: f32, from: f32, to: f32, color: Vec3) -> &mut Shape {
        let e = r + t;
        self.push(x, y, Form::Arc { r, t, from, to }, color, (x - e, y - e, x + e, y + e))
    }

    pub fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32, color: Vec3) -> &mut Shape {
        self.push(x, y, Form::Ellipse { rx, ry }, color, (x - rx, y - ry, x + rx, y + ry))
    }

    /// Un poligono convexo con sus vertices en la pantalla (hasta 8).
    pub fn poly(&mut self, points: &[(f32, f32)], color: Vec3) -> &mut Shape {
        let n = points.len().min(8);
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &points[..n] {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let (cx, cy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let mut pts = [(0.0, 0.0); 8];
        for (i, p) in points[..n].iter().enumerate() {
            pts[i] = (p.0 - cx, p.1 - cy);
        }
        self.push(cx, cy, Form::Poly { n: n as u8, pts }, color, (x0, y0, x1, y1))
    }
}

/// Pinta las figuras que tocan la fila `yf`, en orden. Con `depths` (la
/// profundidad de cada pixel de la fila), las que tienen profundidad no
/// tapan lo que esta mas cerca.
pub fn paint(shapes: &[Shape], yf: f32, row: &mut [u8], width: usize, depths: Option<&[f32]>) {
    paint_lit(shapes, yf, row, width, depths, &Light::default());
}

/// `paint` con la luz de las figuras con volumen. La emision de cada pixel
/// (en el alfa, ver `lienzo::write_emit`) se mezcla igual que el color: lo
/// que se pinta encima tapa la de atras y pone la suya.
pub fn paint_lit(shapes: &[Shape], yf: f32, row: &mut [u8], width: usize, depths: Option<&[f32]>, light: &Light) {
    let half = (light.key + Vec3::new(0.0, 0.0, 1.0)).normalize();
    for sh in shapes {
        if yf < sh.y0 - sh.pad || yf > sh.y1 + sh.pad {
            continue;
        }
        let x0 = ((sh.x0 - sh.pad).floor().max(0.0) as usize).min(width);
        let x1 = ((sh.x1 + sh.pad).ceil().max(0.0) as usize).min(width);
        for x in x0..x1 {
            if sh.depth > 0.0 && depths.is_some_and(|dp| dp.get(x).is_some_and(|&z| z < sh.depth)) {
                continue;
            }
            let xf = x as f32 + 0.5;
            let d = sh.sdf(xf, yf);
            let a = cover(d / sh.soft) * sh.alpha;
            let g = if sh.glow > 0.0 && d > 0.0 { halo(d, sh.glow) * sh.glow_k } else { 0.0 };
            if a <= 0.002 && g <= 0.002 {
                continue;
            }
            let p = &mut row[x * 4..x * 4 + 4];
            let mut c = lienzo::read(p);
            let mut e = lienzo::emission(p);
            if g > 0.0 {
                c = c + sh.color * g;
                e = (e + sh.emit * g).min(1.0);
            }
            if a > 0.0 {
                let col = if sh.vol > 0.0 { light.shade(sh, half, xf, yf, d) } else { sh.color };
                c = over(c, col, a);
                e += (sh.emit - e) * a.min(1.0);
            }
            lienzo::write_emit(p, c, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polygon_measures_inside_and_outside() {
        // Un cuadrado de 2 x 2, en los dos sentidos de giro.
        for square in [[(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)], [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)]] {
            assert!((poly(0.0, 0.0, &square) + 1.0).abs() < 1e-5);
            assert!((poly(3.0, 0.0, &square) - 2.0).abs() < 1e-5);
            assert!((poly(2.0, 2.0, &square) - 2.0f32.sqrt()).abs() < 1e-5);
        }
    }

    /// La elipse mide bien en el borde y en un circulo, y el gradiente de
    /// cada forma apunta hacia afuera.
    #[test]
    fn ellipses_measure_and_gradients_point_out() {
        assert!(ellipse(4.0, 0.0, 4.0, 2.0).abs() < 1e-5);
        assert!(ellipse(0.0, 2.0, 4.0, 2.0).abs() < 1e-5);
        assert!((ellipse(5.0, 0.0, 3.0, 3.0) - 2.0).abs() < 1e-5);
        assert!(ellipse(0.0, 0.0, 4.0, 2.0) < 0.0);
        let mut c = Canvas::default();
        c.rect(0.0, 0.0, 10.0, 5.0, 2.0, Vec3::ONE);
        c.disc(0.0, 0.0, 5.0, Vec3::ONE);
        c.line(-5.0, 0.0, 5.0, 0.0, 3.0, Vec3::ONE);
        c.ellipse(0.0, 0.0, 6.0, 3.0, Vec3::ONE);
        c.poly(&[(-4.0, -4.0), (4.0, -4.0), (4.0, 4.0), (-4.0, 4.0)], Vec3::ONE);
        for sh in &c.shapes {
            for (px, py) in [(3.0f32, 0.2f32), (-0.3, 2.5), (0.5, -2.0)] {
                let (gx, gy) = sh.grad(px, py);
                let d0 = sh.sdf(px, py);
                let d1 = sh.sdf(px + 0.01 * gx, py + 0.01 * gy);
                assert!(d1 > d0, "{:?} en ({px}, {py})", sh.form);
            }
        }
    }

    /// Una figura con volumen se ve mas clara del lado de la luz, y lo que
    /// emite deja su emision en el pixel.
    #[test]
    fn volume_shades_toward_the_light_and_emission_is_kept() {
        let mut c = Canvas::default();
        c.disc(20.0, 20.0, 16.0, Vec3::splat(0.6)).vol(16.0);
        c.disc(50.0, 20.0, 6.0, Vec3::ONE).emit(0.8);
        let light = Light { key: Vec3::new(-1.0, 0.0, 1.0).normalize(), key_color: Vec3::ONE, ambient: Vec3::splat(0.1), rim: Vec3::ZERO };
        let mut row = vec![0u8; 64 * 4];
        for p in row.chunks_exact_mut(4) {
            p[3] = 255;
        }
        paint_lit(&c.shapes, 20.5, &mut row, 64, None, &light);
        let at = |x: usize| lienzo::read(&row[x * 4..x * 4 + 4]).x;
        assert!(at(8) > at(32) + 0.1, "{} {}", at(8), at(32));
        assert!((lienzo::emission(&row[50 * 4..50 * 4 + 4]) - 0.8).abs() < 0.01);
        assert_eq!(lienzo::emission(&row[20 * 4..20 * 4 + 4]), 0.0);
    }
}
