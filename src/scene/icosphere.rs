//! Esfera geodesica: un icosaedro subdividido.
//!
//! Cada nivel parte cada triangulo en cuatro y empuja los puntos medios a la
//! esfera. A diferencia de la esfera por meridianos y paralelos, los
//! triangulos quedan casi del mismo tamano en todas partes: no se amontonan
//! en los polos.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use super::Mesh;
use crate::math::{Vec2, Vec3};

/// Esfera de radio 1 con `subdivisions` niveles: `20 * 4^n` triangulos.
pub fn icosphere(subdivisions: u32) -> Mesh {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut positions: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();

    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    for _ in 0..subdivisions {
        // Cada arista se parte una sola vez aunque la compartan dos caras.
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut midpoint = |a: u32, b: u32, positions: &mut Vec<Vec3>| -> u32 {
            *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let m = (positions[a as usize] + positions[b as usize]) * 0.5;
                positions.push(m.normalize());
                (positions.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = midpoint(a, b, &mut positions);
            let bc = midpoint(b, c, &mut positions);
            let ca = midpoint(c, a, &mut positions);
            next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = next;
    }

    let uvs = positions
        .iter()
        .map(|p| Vec2::new(0.5 + p.z.atan2(p.x) / TAU, 0.5 - p.y.asin() / PI))
        .collect();
    let mut mesh = Mesh {
        normals: positions.clone(),
        colors: vec![Vec3::ONE; positions.len()],
        tangents: vec![super::mesh::NO_TANGENT; positions.len()],
        uvs,
        positions,
        indices: faces,
        ..Default::default()
    };
    mesh.update_bounds();
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_point_outward_and_count_matches() {
        let mesh = icosphere(2);
        assert_eq!(mesh.indices.len(), 20 * 16);
        for &[a, b, c] in &mesh.indices {
            let (pa, pb, pc) = (
                mesh.positions[a as usize],
                mesh.positions[b as usize],
                mesh.positions[c as usize],
            );
            let normal = (pb - pa).cross(pc - pa);
            assert!(normal.dot(pa + pb + pc) > 0.0, "cara hacia adentro");
        }
        assert!((mesh.bounds_radius - 1.0).abs() < 1e-5);
    }
}
