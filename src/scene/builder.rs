//! Constructor de mallas procedurales.
//!
//! Los modelos de la superficie (monolitos, hongos, corales, naufragios) se
//! arman con codigo a partir de primitivas: cajas, troncos de cono,
//! elipsoides y tubos. Cada primitiva genera posiciones, normales, uv y
//! tangentes; las tangentes apuntan hacia donde crece la `u` de la textura,
//! que es lo que necesita el mapeo de normales.
//!
//! Las primitivas facetadas duplican los vertices de cada cara (cada cara con
//! su normal plana: estilo low poly); las lisas los comparten (normal
//! promedio: superficies organicas).

use super::Mesh;
use crate::math::{Vec2, Vec3, Vec4};

pub struct MeshBuilder {
    mesh: Mesh,
    color: Vec3,
    /// Con `Some(s)`, las caras planas reciben uv en unidades del mundo (una
    /// repeticion de la textura cada `s`) en vez de 0 a 1 por cara.
    uv_world: Option<f32>,
}

/// Dos ejes perpendiculares a `axis` (unitario), para armar anillos.
pub fn perpendiculars(axis: Vec3) -> (Vec3, Vec3) {
    let helper = if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = helper.cross(axis).normalize();
    (u, axis.cross(u))
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            mesh: Mesh::default(),
            color: Vec3::ONE,
            uv_world: None,
        }
    }

    /// Color de los vertices que se agreguen despues.
    pub fn color(&mut self, color: Vec3) -> &mut Self {
        self.color = color;
        self
    }

    /// Uv de las caras planas que se agreguen despues: `Some(s)` repite la
    /// textura cada `s` unidades (las placas de un casco no se estiran en
    /// las caras largas); `None` va de 0 a 1 en cada cara.
    pub fn uv_world(&mut self, size: Option<f32>) -> &mut Self {
        self.uv_world = size;
        self
    }

    /// Cambia el color de los vertices `range` (los indices que devuelven
    /// las primitivas).
    pub fn paint(&mut self, range: std::ops::Range<u32>, color: Vec3) {
        for c in &mut self.mesh.colors[range.start as usize..range.end as usize] {
            *c = color;
        }
    }

    /// Uv de un punto de una cara plana que empieza en `origin`, con
    /// `tangent` (unitaria) como eje u y `bitangent` como eje v.
    fn planar_uv(&self, p: Vec3, origin: Vec3, tangent: Vec3, bitangent: Vec3, fallback: Vec2) -> Vec2 {
        match self.uv_world {
            Some(size) => Vec2::new((p - origin).dot(tangent) / size, (p - origin).dot(bitangent) / size),
            None => fallback,
        }
    }

    fn vertex(&mut self, p: Vec3, n: Vec3, uv: Vec2, tangent: Vec4) -> u32 {
        let m = &mut self.mesh;
        m.positions.push(p);
        m.normals.push(n);
        m.uvs.push(uv);
        m.colors.push(self.color);
        m.tangents.push(tangent);
        (m.positions.len() - 1) as u32
    }

    /// Cuadrilatero plano con su propia normal. Los vertices van en sentido
    /// antihorario visto desde afuera: `p0` es uv (0,0), `p1` (1,0), `p2`
    /// (1,1) y `p3` (0,1). La tangente sigue a `p1 - p0`.
    pub fn quad(&mut self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3) {
        let normal = (p1 - p0).cross(p3 - p0).normalize();
        let tangent = (p1 - p0).normalize();
        let bitangent = normal.cross(tangent);
        let t = Vec4::new(tangent.x, tangent.y, tangent.z, 1.0);
        let uv = |s: &Self, p, fallback| s.planar_uv(p, p0, tangent, bitangent, fallback);
        let (uv0, uv1) = (uv(self, p0, Vec2::new(0.0, 0.0)), uv(self, p1, Vec2::new(1.0, 0.0)));
        let (uv2, uv3) = (uv(self, p2, Vec2::new(1.0, 1.0)), uv(self, p3, Vec2::new(0.0, 1.0)));
        let a = self.vertex(p0, normal, uv0, t);
        let b = self.vertex(p1, normal, uv1, t);
        let c = self.vertex(p2, normal, uv2, t);
        let d = self.vertex(p3, normal, uv3, t);
        self.mesh.indices.push([a, b, c]);
        self.mesh.indices.push([a, c, d]);
    }

    /// Triangulo plano.
    pub fn triangle(&mut self, p0: Vec3, p1: Vec3, p2: Vec3) {
        let normal = (p1 - p0).cross(p2 - p0).normalize();
        let tangent = (p1 - p0).normalize();
        let bitangent = normal.cross(tangent);
        let t = Vec4::new(tangent.x, tangent.y, tangent.z, 1.0);
        let uv0 = self.planar_uv(p0, p0, tangent, bitangent, Vec2::new(0.0, 0.0));
        let uv1 = self.planar_uv(p1, p0, tangent, bitangent, Vec2::new(1.0, 0.0));
        let uv2 = self.planar_uv(p2, p0, tangent, bitangent, Vec2::new(0.5, 1.0));
        let a = self.vertex(p0, normal, uv0, t);
        let b = self.vertex(p1, normal, uv1, t);
        let c = self.vertex(p2, normal, uv2, t);
        self.mesh.indices.push([a, b, c]);
    }

    /// Caja orientada: `axes` son los ejes de la caja (unitarios y
    /// perpendiculares, con `x cross y = z`) y `half` sus medias medidas.
    /// Cada cara tiene uv de 0 a 1, con `v` hacia arriba en las laterales.
    /// Devuelve el primer vertice: las caras salen de cuatro en cuatro en
    /// el orden frente (+z), atras, derecha, izquierda, arriba y abajo.
    pub fn cuboid(&mut self, center: Vec3, axes: [Vec3; 3], half: Vec3) -> u32 {
        let first = self.mesh.positions.len() as u32;
        let [x, y, z] = [axes[0] * half.x, axes[1] * half.y, axes[2] * half.z];
        let corner = |sx: f32, sy: f32, sz: f32| center + x * sx + y * sy + z * sz;
        // Frente (+z), atras (-z), derecha (+x), izquierda (-x), arriba, abajo.
        self.quad(corner(-1.0, -1.0, 1.0), corner(1.0, -1.0, 1.0), corner(1.0, 1.0, 1.0), corner(-1.0, 1.0, 1.0));
        self.quad(corner(1.0, -1.0, -1.0), corner(-1.0, -1.0, -1.0), corner(-1.0, 1.0, -1.0), corner(1.0, 1.0, -1.0));
        self.quad(corner(1.0, -1.0, 1.0), corner(1.0, -1.0, -1.0), corner(1.0, 1.0, -1.0), corner(1.0, 1.0, 1.0));
        self.quad(corner(-1.0, -1.0, -1.0), corner(-1.0, -1.0, 1.0), corner(-1.0, 1.0, 1.0), corner(-1.0, 1.0, -1.0));
        self.quad(corner(-1.0, 1.0, 1.0), corner(1.0, 1.0, 1.0), corner(1.0, 1.0, -1.0), corner(-1.0, 1.0, -1.0));
        self.quad(corner(-1.0, -1.0, -1.0), corner(1.0, -1.0, -1.0), corner(1.0, -1.0, 1.0), corner(-1.0, -1.0, 1.0));
        first
    }

    /// Tronco de cono de `base` a `base + axis * height`, con radios `r0`
    /// abajo y `r1` arriba (`r1 = 0` es un cono). Facetado o liso.
    #[allow(clippy::too_many_arguments)]
    pub fn frustum(
        &mut self,
        base: Vec3,
        axis: Vec3,
        height: f32,
        r0: f32,
        r1: f32,
        segments: usize,
        smooth: bool,
        caps: (bool, bool),
    ) {
        let (u, w) = perpendiculars(axis);
        let top = base + axis * height;
        let ring = |i: usize, r: f32, center: Vec3| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            center + (u * a.cos() + w * a.sin()) * r
        };
        // Inclinacion de la pared: la normal se inclina hacia el eje del cono.
        let slope = (r0 - r1) / height;
        if smooth {
            let first = self.mesh.positions.len() as u32;
            // En unidades del mundo: u a lo largo de la circunferencia media.
            let (u_span, v_span) = match self.uv_world {
                Some(size) => (std::f32::consts::PI * (r0 + r1) / size, height / size),
                None => (1.0, 1.0),
            };
            for i in 0..=segments {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                let radial = u * a.cos() + w * a.sin();
                let normal = (radial + axis * slope).normalize();
                let tangent = (-u * a.sin() + w * a.cos()).normalize();
                let t = Vec4::new(tangent.x, tangent.y, tangent.z, 1.0);
                let s = i as f32 / segments as f32 * u_span;
                self.vertex(base + radial * r0, normal, Vec2::new(s, 0.0), t);
                self.vertex(top + radial * r1, normal, Vec2::new(s, v_span), t);
            }
            for i in 0..segments as u32 {
                let (a, b) = (first + 2 * i, first + 2 * i + 1);
                let (c, d) = (a + 2, b + 2);
                self.mesh.indices.push([a, c, d]);
                self.mesh.indices.push([a, d, b]);
            }
        } else {
            for i in 0..segments {
                let (p0, p1) = (ring(i, r0, base), ring(i + 1, r0, base));
                if r1 > 1e-5 {
                    self.quad(p0, p1, ring(i + 1, r1, top), ring(i, r1, top));
                } else {
                    self.triangle(p0, p1, top);
                }
            }
        }
        if caps.0 {
            for i in 0..segments {
                self.triangle(base, ring(i + 1, r0, base), ring(i, r0, base));
            }
        }
        if caps.1 && r1 > 1e-5 {
            for i in 0..segments {
                self.triangle(top, ring(i, r1, top), ring(i + 1, r1, top));
            }
        }
    }

    /// Elipsoide liso centrado en `center` con semiejes `radii` (en ejes del
    /// objeto). Con `upper_only` solo se arma la mitad de arriba, cerrada por
    /// abajo con un disco (el sombrero de un hongo).
    pub fn ellipsoid(&mut self, center: Vec3, radii: Vec3, rings: usize, segments: usize, upper_only: bool) {
        let first = self.mesh.positions.len() as u32;
        let rows = rings + 1;
        let lowest = if upper_only { std::f32::consts::FRAC_PI_2 } else { std::f32::consts::PI };
        for r in 0..rows {
            let theta = r as f32 / rings as f32 * lowest;
            let (st, ct) = theta.sin_cos();
            for s in 0..=segments {
                let phi = s as f32 / segments as f32 * std::f32::consts::TAU;
                let (sp, cp) = phi.sin_cos();
                let unit = Vec3::new(st * cp, ct, st * sp);
                let p = center + unit * radii;
                // Normal de un elipsoide: el gradiente de (x/a)^2 + ...
                let normal = Vec3::new(unit.x / radii.x, unit.y / radii.y, unit.z / radii.z).normalize();
                let tangent = Vec3::new(-sp, 0.0, cp);
                let t = Vec4::new(tangent.x, tangent.y, tangent.z, 1.0);
                self.vertex(p, normal, Vec2::new(s as f32 / segments as f32, r as f32 / rings as f32), t);
            }
        }
        let stride = segments as u32 + 1;
        let rings = rings as u32;
        for r in 0..rings {
            for s in 0..segments as u32 {
                let a = first + r * stride + s;
                let b = a + stride;
                // En los polos toda una fila es el mismo punto y uno de los
                // dos triangulos se aplasta: no se agrega.
                if r > 0 {
                    self.mesh.indices.push([a, a + 1, b + 1]);
                }
                if r + 1 < rings || upper_only {
                    self.mesh.indices.push([a, b + 1, b]);
                }
            }
        }
        if upper_only {
            // Disco que cierra el sombrero por debajo, mirando hacia abajo.
            let rim = first + rings * stride;
            let below = Vec3::new(0.0, -1.0, 0.0);
            let t = Vec4::new(1.0, 0.0, 0.0, 1.0);
            let middle = self.vertex(center, below, Vec2::new(0.5, 0.5), t);
            let first_rim = self.mesh.positions.len() as u32;
            for s in 0..=segments {
                let p = self.mesh.positions[(rim + s as u32) as usize];
                self.vertex(p, below, Vec2::new(0.5, 0.5), t);
            }
            for s in 0..segments as u32 {
                self.mesh.indices.push([middle, first_rim + s, first_rim + s + 1]);
            }
        }
    }

    /// Tubo liso a lo largo de `path`, con un radio por punto. Los anillos se
    /// orientan con transporte paralelo para que el tubo no se retuerza.
    pub fn tube(&mut self, path: &[Vec3], radii: &[f32], segments: usize) {
        if path.len() < 2 {
            return;
        }
        let first = self.mesh.positions.len() as u32;
        let dir0 = (path[1] - path[0]).normalize();
        let (mut u, _) = perpendiculars(dir0);
        for (k, (&p, &r)) in path.iter().zip(radii).enumerate() {
            let dir = if k + 1 < path.len() {
                (path[k + 1] - path[k.saturating_sub(1)]).normalize()
            } else {
                (path[k] - path[k - 1]).normalize()
            };
            // Transporte paralelo: quitarle a u la componente sobre el nuevo eje.
            u = (u - dir * u.dot(dir)).normalize();
            let w = dir.cross(u);
            for s in 0..=segments {
                let a = s as f32 / segments as f32 * std::f32::consts::TAU;
                let radial = u * a.cos() + w * a.sin();
                let tangent = -u * a.sin() + w * a.cos();
                let t = Vec4::new(tangent.x, tangent.y, tangent.z, 1.0);
                self.vertex(p + radial * r, radial, Vec2::new(s as f32 / segments as f32, k as f32), t);
            }
        }
        let stride = segments as u32 + 1;
        for k in 0..(path.len() - 1) as u32 {
            for s in 0..segments as u32 {
                let a = first + k * stride + s;
                let b = a + stride;
                // tangente x direccion del camino apunta hacia afuera.
                self.mesh.indices.push([a, b + 1, b]);
                self.mesh.indices.push([a, a + 1, b + 1]);
            }
        }
    }

    pub fn build(mut self) -> Mesh {
        self.mesh.update_bounds();
        self.mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las caras de una caja cerrada miran hacia afuera: la normal geometrica
    /// de cada triangulo se aleja del centro.
    #[test]
    fn closed_primitives_face_outward() {
        let mut b = MeshBuilder::new();
        b.cuboid(Vec3::ZERO, [Vec3::X, Vec3::Y, Vec3::Z], Vec3::new(1.0, 2.0, 0.5));
        b.frustum(Vec3::new(5.0, 0.0, 0.0), Vec3::Y, 3.0, 1.0, 0.4, 7, false, (true, true));
        b.frustum(Vec3::new(-5.0, 0.0, 0.0), Vec3::Y, 3.0, 1.0, 0.4, 7, true, (true, true));
        b.ellipsoid(Vec3::new(0.0, 6.0, 0.0), Vec3::new(2.0, 1.0, 2.0), 6, 10, false);
        let mesh = b.build();
        let centers = [Vec3::ZERO, Vec3::new(5.0, 1.5, 0.0), Vec3::new(-5.0, 1.5, 0.0), Vec3::new(0.0, 6.0, 0.0)];
        for [a, bb, c] in &mesh.indices {
            let (pa, pb, pc) = (
                mesh.positions[*a as usize],
                mesh.positions[*bb as usize],
                mesh.positions[*c as usize],
            );
            let normal = (pb - pa).cross(pc - pa);
            if normal.length() < 1e-9 {
                continue;
            }
            let centroid = (pa + pb + pc) / 3.0;
            let center = *centers
                .iter()
                .min_by(|x, y| (**x - centroid).length().total_cmp(&(**y - centroid).length()))
                .unwrap();
            assert!(normal.dot(centroid - center) > 0.0, "triangulo hacia adentro en {centroid:?}");
        }
    }

    #[test]
    fn tubes_face_away_from_their_axis() {
        let mut b = MeshBuilder::new();
        let path: Vec<Vec3> = (0..5).map(|k| Vec3::new(0.0, k as f32, 0.0)).collect();
        b.tube(&path, &[1.0, 0.9, 0.8, 0.7, 0.6], 8);
        let mesh = b.build();
        for [a, bb, c] in &mesh.indices {
            let (pa, pb, pc) = (
                mesh.positions[*a as usize],
                mesh.positions[*bb as usize],
                mesh.positions[*c as usize],
            );
            let centroid = (pa + pb + pc) / 3.0;
            let outward = Vec3::new(centroid.x, 0.0, centroid.z);
            assert!((pb - pa).cross(pc - pa).dot(outward) > 0.0);
        }
    }
}
