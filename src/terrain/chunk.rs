//! Generacion de un chunk: una malla de `n x n` cuadros sobre un pedazo de
//! una cara del cubo, proyectada a la esfera y desplazada por la funcion de
//! altura del planeta.

use super::{ChunkKey, HeightField};
use crate::math::{DVec3, Vec2, Vec3, Vec4};
use crate::scene::Mesh;

/// Las seis caras: normal, eje u y eje v, con `u x v = normal`. Asi los
/// triangulos que avanzan primero en u y luego en v quedan antihorarios
/// vistos desde afuera.
const FACES: [([f64; 3], [f64; 3], [f64; 3]); 6] = [
    ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
];

pub struct Chunk {
    pub key: ChunkKey,
    /// Vertices relativos a `center`: numeros chicos, precisos en `f32`.
    pub mesh: Mesh,
    /// Centro del chunk en el espacio del planeta.
    pub center: DVec3,
    /// Radio de la esfera envolvente alrededor de `center`.
    pub radius: f32,
    /// Distancia maxima de un vertice al centro del planeta.
    pub max_radius: f32,
    pub last_used: u64,
}

/// Punto `(u, v)` en `[-1, 1]` de una cara del cubo, proyectado a la esfera
/// unitaria.
///
/// No basta con normalizar: asi las celdas cerca de las esquinas del cubo
/// quedan mucho mas chicas que en el centro de la cara. La proyeccion de
/// "cubo esferizado" reparte el area de forma mucho mas pareja.
pub fn cube_to_sphere(face: u8, u: f64, v: f64) -> DVec3 {
    let (n, a, b) = FACES[face as usize];
    let p = DVec3::new(
        n[0] + a[0] * u + b[0] * v,
        n[1] + a[1] * u + b[1] * v,
        n[2] + a[2] * u + b[2] * v,
    );
    let (x2, y2, z2) = (p.x * p.x, p.y * p.y, p.z * p.z);
    DVec3::new(
        p.x * (1.0 - y2 / 2.0 - z2 / 2.0 + y2 * z2 / 3.0).sqrt(),
        p.y * (1.0 - z2 / 2.0 - x2 / 2.0 + z2 * x2 / 3.0).sqrt(),
        p.z * (1.0 - x2 / 2.0 - y2 / 2.0 + x2 * y2 / 3.0).sqrt(),
    )
}

/// Indices de los vertices de cada borde, en sentido antihorario visto desde
/// afuera: abajo (de izquierda a derecha), derecha (de abajo a arriba),
/// arriba (de derecha a izquierda) e izquierda (de arriba a abajo).
fn edge_loops(n: usize) -> [Vec<u32>; 4] {
    let row = n + 1;
    let at = |i: usize, j: usize| (j * row + i) as u32;
    [
        (0..=n).map(|i| at(i, 0)).collect(),
        (0..=n).map(|j| at(n, j)).collect(),
        (0..=n).rev().map(|i| at(i, n)).collect(),
        (0..=n).rev().map(|j| at(0, j)).collect(),
    ]
}

/// Triangulos de un chunk de `n x n` cuadros mas sus faldones. Todos los
/// chunks comparten esta lista.
pub fn indices(n: usize) -> Vec<[u32; 3]> {
    let row = n + 1;
    let mut out = Vec::with_capacity(2 * n * n + 8 * n);
    for j in 0..n {
        for i in 0..n {
            let a = (j * row + i) as u32;
            let b = a + 1;
            let c = a + row as u32;
            let d = c + 1;
            out.push([a, b, d]);
            out.push([a, d, c]);
        }
    }
    // Faldones: cada borde se une con su copia hundida. Al recorrer el borde
    // en sentido antihorario, (borde, hundido, siguiente) mira hacia afuera.
    let skirts = (row * row) as u32;
    for (edge, verts) in edge_loops(n).iter().enumerate() {
        let base = skirts + (edge * row) as u32;
        for k in 0..n {
            let (e0, e1) = (verts[k], verts[k + 1]);
            let (s0, s1) = (base + k as u32, base + k as u32 + 1);
            out.push([e0, s0, e1]);
            out.push([e1, s0, s1]);
        }
    }
    out
}

pub fn generate(key: ChunkKey, radius: f64, field: &dyn HeightField, n: usize) -> Chunk {
    let (u0, v0, u1, v1) = key.uv_bounds();
    // Muestras con un anillo extra alrededor: las normales del borde salen de
    // vecinos reales y coinciden con las del chunk de al lado.
    let stride = n + 3;
    let mut points = Vec::with_capacity(stride * stride);
    let mut samples = Vec::with_capacity(stride * stride);
    for j in 0..stride {
        let v = v0 + (v1 - v0) * (j as f64 - 1.0) / n as f64;
        for i in 0..stride {
            let u = u0 + (u1 - u0) * (i as f64 - 1.0) / n as f64;
            let dir = cube_to_sphere(key.face, u, v);
            let sample = field.sample(dir.as_vec3());
            points.push(dir * (radius + sample.height as f64));
            samples.push(sample);
        }
    }
    let center = points[(n / 2 + 1) * stride + n / 2 + 1];

    let row = n + 1;
    let count = row * row + 4 * row;
    let mut mesh = Mesh {
        positions: Vec::with_capacity(count),
        normals: Vec::with_capacity(count),
        uvs: Vec::with_capacity(count),
        colors: Vec::with_capacity(count),
        tangents: Vec::with_capacity(count),
        ..Default::default()
    };
    let (min_h, max_h) = field.height_range();
    let span = (max_h - min_h).max(1e-6);
    let mut max_radius = 0.0f64;
    for j in 0..row {
        for i in 0..row {
            let k = (j + 1) * stride + i + 1;
            let p = points[k];
            // Diferencias centrales: la normal sigue el relieve real.
            let du = points[k + 1] - points[k - 1];
            let dv = points[k + stride] - points[k - stride];
            let normal = du.cross(dv).normalize();
            let up = p.normalize();
            max_radius = max_radius.max(p.length());
            mesh.positions.push((p - center).as_vec3());
            mesh.normals.push(normal.as_vec3());
            // uv: altura normalizada a [0, 1] y pendiente (0 plano, 1 vertical).
            mesh.uvs.push(Vec2::new(
                (samples[k].height - min_h) / span,
                (1.0 - normal.dot(up)) as f32,
            ));
            mesh.colors.push(samples[k].attrs);
            let tangent = du.normalize().as_vec3();
            mesh.tangents.push(Vec4::new(tangent.x, tangent.y, tangent.z, 1.0));
        }
    }

    // Faldones: bajan unas veces el tamano de un cuadro, lo suficiente para
    // tapar la grieta con un vecino de otro nivel.
    let quad = radius * std::f64::consts::FRAC_PI_2 / (1u64 << key.level) as f64 / n as f64;
    let depth = quad * 3.0 + span as f64 * 0.05;
    for verts in edge_loops(n) {
        for index in verts {
            let i = index as usize;
            let p = mesh.positions[i].as_dvec3() + center;
            let lowered = p - p.normalize() * depth;
            let (normal, uv, color, tangent) = (mesh.normals[i], mesh.uvs[i], mesh.colors[i], mesh.tangents[i]);
            mesh.positions.push((lowered - center).as_vec3());
            mesh.normals.push(normal);
            mesh.uvs.push(uv);
            mesh.colors.push(color);
            mesh.tangents.push(tangent);
        }
    }

    let bound = mesh.positions.iter().map(|p| p.length()).fold(0.0, f32::max);
    mesh.bounds_center = Vec3::ZERO;
    mesh.bounds_radius = bound;
    Chunk {
        key,
        mesh,
        center,
        radius: bound,
        max_radius: max_radius as f32,
        last_used: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::Smooth;
    use super::*;

    #[test]
    fn cube_to_sphere_lands_on_the_unit_sphere() {
        for face in 0..6 {
            for (u, v) in [(-1.0, -1.0), (0.3, -0.7), (1.0, 1.0), (0.0, 0.0)] {
                assert!((cube_to_sphere(face, u, v).length() - 1.0).abs() < 1e-12);
            }
        }
    }

    /// Todos los triangulos miran hacia afuera: los de la superficie en la
    /// direccion del radio, los faldones lejos del centro del chunk.
    #[test]
    fn surface_and_skirt_triangles_face_outward() {
        for face in 0..6 {
            let key = ChunkKey { face, level: 2, x: 1, y: 2 };
            let chunk = generate(key, 10.0, &Smooth, 8);
            let tris = indices(8);
            let row = 9;
            let p = |i: u32| chunk.mesh.positions[i as usize].as_dvec3() + chunk.center;
            for (t, [a, b, c]) in tris.iter().enumerate() {
                let (pa, pb, pc) = (p(*a), p(*b), p(*c));
                let normal = (pb - pa).cross(pc - pa);
                let centroid = (pa + pb + pc) / 3.0;
                if t < 2 * 8 * 8 {
                    assert!(normal.dot(centroid) > 0.0, "cara {face}: superficie hacia adentro");
                } else {
                    // Fuera del chunk, medido en el plano tangente.
                    let out = centroid - chunk.center;
                    let up = chunk.center.normalize();
                    let horizontal = out - up * out.dot(up);
                    assert!(normal.dot(horizontal) > 0.0, "cara {face}: faldon hacia adentro");
                }
            }
            assert_eq!(chunk.mesh.positions.len(), row * row + 4 * row);
        }
    }
}
