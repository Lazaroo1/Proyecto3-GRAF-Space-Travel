//! Cintas: lineas con ancho fijo en pantalla, armadas con triangulos que
//! miran a la camara. Pasan por el mismo pipeline que todo lo demas, asi
//! que tienen profundidad (un planeta las tapa) y brillan con el bloom.
//!
//! En cada punto la cinta se abre hacia los lados, perpendicular a la linea
//! y a la direccion de vision, con medio ancho de `w * angulo_pixel * d`
//! (`d` es la distancia a la camara): un objeto de ese tamano mide `w`
//! pixeles a esa distancia, asi que la cinta mide lo mismo de cerca y de
//! lejos.

use super::Mesh;
use crate::math::{Vec2, Vec3};

/// Agrega a `mesh` una cinta por `points` (relativos a la camara) de
/// `width` pixeles de ancho. En el uv, `x` va de 0 a 1 a lo largo de la
/// cinta y `y` de 0 a 1 de un borde al otro.
pub fn ribbon(mesh: &mut Mesh, points: &[Vec3], width: f32, pixel_angle: f32, color: Vec3) {
    let n = points.len();
    if n < 2 {
        return;
    }
    let first = mesh.positions.len() as u32;
    let mut last_side = Vec3::X;
    for (i, &p) in points.iter().enumerate() {
        let along = points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)];
        // Perpendicular a la linea y a la vision; si son paralelas (la linea
        // apunta a la camara) se conserva el lado anterior.
        let cross = along.cross(p);
        let side = if cross.length_squared() > 1e-12 {
            cross.normalize()
        } else {
            last_side
        };
        last_side = side;
        let half = side * (0.5 * width * pixel_angle * p.length());
        let facing = if p.length_squared() > 0.0 { -p.normalize() } else { Vec3::Z };
        let u = i as f32 / (n - 1) as f32;
        for (offset, v) in [(-half, 0.0), (half, 1.0)] {
            mesh.positions.push(p + offset);
            mesh.normals.push(facing);
            mesh.uvs.push(Vec2::new(u, v));
            mesh.colors.push(color);
        }
    }
    for i in 0..(n - 1) as u32 {
        let a = first + 2 * i;
        // Las dos caras: el raster descarta la que queda de espaldas.
        mesh.indices.push([a, a + 2, a + 3]);
        mesh.indices.push([a, a + 3, a + 1]);
        mesh.indices.push([a, a + 3, a + 2]);
        mesh.indices.push([a, a + 1, a + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ribbon_keeps_its_screen_width_at_any_distance() {
        let mut mesh = Mesh::default();
        let points = [Vec3::new(0.0, 0.0, -10.0), Vec3::new(5.0, 0.0, -10.0), Vec3::new(5.0, 0.0, -1000.0)];
        ribbon(&mut mesh, &points, 3.0, 0.002, Vec3::ONE);
        assert_eq!(mesh.positions.len(), 6);
        assert_eq!(mesh.indices.len(), 8);
        // Ancho en el mundo / distancia = ancho angular: igual en todos los puntos.
        for k in 0..3 {
            let (a, b) = (mesh.positions[2 * k], mesh.positions[2 * k + 1]);
            let distance = points[k].length();
            let angular = (a - b).length() / distance;
            assert!((angular - 0.006).abs() < 1e-5, "punto {k}: {angular}");
        }
    }
}
