//! Lector propio de `.obj` y `.mtl`, extendido del lab 4.
//!
//! Lee posiciones (`v`), normales (`vn`), coordenadas de textura (`vt`),
//! caras con cualquier combinacion `v`, `v/vt`, `v//vn` o `v/vt/vn`, indices
//! negativos y poligonos de mas de tres lados (en abanico). Del `.mtl` toma el
//! color difuso `Kd` de cada material y lo guarda como color por vertice.
//!
//! En el `.obj` cada esquina de una cara combina indices independientes de
//! posicion, uv y normal. Para el pipeline, un vertice es una combinacion
//! unica de esos indices (mas el material): un cubo con caras planas tiene 8
//! posiciones pero 24 vertices, porque cada esquina lleva tres normales.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::Mesh;
use crate::math::{Vec2, Vec3};

const DEFAULT_COLOR: Vec3 = Vec3::new(0.8, 0.8, 0.8);

pub fn load_obj(path: &Path) -> Result<Mesh, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut materials = HashMap::new();
    for line in text.lines() {
        if let Some(name) = line.trim().strip_prefix("mtllib ") {
            let mtl = dir.join(name.trim());
            let mtl_text =
                fs::read_to_string(&mtl).map_err(|e| format!("{}: {e}", mtl.display()))?;
            materials.extend(parse_mtl(&mtl_text));
        }
    }
    parse_obj(&text, &materials).map_err(|e| format!("{}: {e}", path.display()))
}

/// Color difuso (`Kd`) de cada material.
pub fn parse_mtl(text: &str) -> HashMap<String, Vec3> {
    let mut colors = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("newmtl") => current = parts.next().map(str::to_string),
            Some("Kd") => {
                if let (Some(name), Some(color)) = (&current, parse_vec3(parts)) {
                    colors.insert(name.clone(), color);
                }
            }
            _ => {}
        }
    }
    colors
}

pub fn parse_obj(text: &str, materials: &HashMap<String, Vec3>) -> Result<Mesh, String> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut mesh = Mesh::default();
    let mut has_normal = Vec::new();
    let mut lookup: HashMap<(usize, Option<usize>, Option<usize>, usize), u32> = HashMap::new();
    let mut material_ids: HashMap<String, usize> = HashMap::new();
    let mut material = 0;
    let mut color = DEFAULT_COLOR;
    let mut face = Vec::new();

    for (n, line) in text.lines().enumerate() {
        let error = |msg: &str| format!("linea {}: {msg}: {line}", n + 1);
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => positions.push(parse_vec3(parts).ok_or_else(|| error("vertice invalido"))?),
            Some("vn") => normals.push(parse_vec3(parts).ok_or_else(|| error("normal invalida"))?),
            Some("vt") => {
                let u: Vec<f32> = parts.take(2).filter_map(|p| p.parse().ok()).collect();
                if u.is_empty() {
                    return Err(error("coordenada de textura invalida"));
                }
                uvs.push(Vec2::new(u[0], u.get(1).copied().unwrap_or(0.0)));
            }
            Some("usemtl") => {
                let name = parts.next().unwrap_or_default().to_string();
                color = materials.get(&name).copied().unwrap_or(DEFAULT_COLOR);
                let next_id = material_ids.len() + 1;
                material = *material_ids.entry(name).or_insert(next_id);
            }
            Some("f") => {
                face.clear();
                for token in parts {
                    let mut fields = token.split('/');
                    let v = resolve(fields.next(), positions.len())
                        .ok_or_else(|| error("indice de vertice fuera de rango"))?;
                    let vt = optional(fields.next(), uvs.len())
                        .map_err(|_| error("indice de textura fuera de rango"))?;
                    let vn = optional(fields.next(), normals.len())
                        .map_err(|_| error("indice de normal fuera de rango"))?;
                    let index = *lookup.entry((v, vt, vn, material)).or_insert_with(|| {
                        mesh.positions.push(positions[v]);
                        mesh.normals.push(vn.map_or(Vec3::ZERO, |i| normals[i]));
                        mesh.uvs.push(vt.map_or(Vec2::ZERO, |i| uvs[i]));
                        mesh.colors.push(color);
                        has_normal.push(vn.is_some());
                        (mesh.positions.len() - 1) as u32
                    });
                    face.push(index);
                }
                if face.len() < 3 {
                    return Err(error("la cara necesita al menos 3 vertices"));
                }
                for i in 1..face.len() - 1 {
                    mesh.indices.push([face[0], face[i], face[i + 1]]);
                }
            }
            _ => {}
        }
    }

    // Vertices sin normal en el archivo: promedio de las caras que los usan,
    // pesado por area (el producto cruz ya trae el area en su longitud).
    if has_normal.iter().any(|&h| !h) {
        let mut sum = vec![Vec3::ZERO; mesh.positions.len()];
        for &[a, b, c] in &mesh.indices {
            let (pa, pb, pc) = (
                mesh.positions[a as usize],
                mesh.positions[b as usize],
                mesh.positions[c as usize],
            );
            let n = (pb - pa).cross(pc - pa);
            for i in [a, b, c] {
                sum[i as usize] += n;
            }
        }
        for (i, &has) in has_normal.iter().enumerate() {
            if !has {
                mesh.normals[i] = sum[i];
            }
        }
    }
    for n in &mut mesh.normals {
        *n = n.normalize();
    }
    mesh.update_bounds();
    Ok(mesh)
}

fn parse_vec3<'a>(parts: impl Iterator<Item = &'a str>) -> Option<Vec3> {
    let v: Vec<f32> = parts.take(3).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

/// Indice del `.obj` (base 1, o negativo desde el final) a base 0.
fn resolve(token: Option<&str>, len: usize) -> Option<usize> {
    let index: i64 = token?.parse().ok()?;
    let resolved = if index > 0 { index - 1 } else { len as i64 + index };
    (0..len as i64).contains(&resolved).then_some(resolved as usize)
}

/// Campo opcional de una esquina: vacio o ausente es `None`; invalido es error.
fn optional(token: Option<&str>, len: usize) -> Result<Option<usize>, ()> {
    match token {
        None | Some("") => Ok(None),
        some => resolve(some, len).map(Some).ok_or(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Mesh, String> {
        parse_obj(text, &HashMap::new())
    }

    #[test]
    fn reads_faces_with_normals() {
        let mesh = parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n").unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.indices, vec![[0, 1, 2]]);
        assert_eq!(mesh.normals[0], Vec3::Z);
    }

    #[test]
    fn accepts_negative_indices_and_quads() {
        let mesh = parse("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf -4 -3 -2 -1\n").unwrap();
        assert_eq!(mesh.indices, vec![[0, 1, 2], [0, 2, 3]]);
        // Sin normales en el archivo: se calculan de la cara (+Z).
        assert!((mesh.normals[0] - Vec3::Z).length() < 1e-6);
    }

    #[test]
    fn rejects_out_of_range_indices() {
        assert!(parse("v 0 0 0\nf 1 2 3\n").is_err());
        assert!(parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1//4 2//1 3//1\n").is_err());
    }

    #[test]
    fn materials_become_vertex_colors() {
        let mut materials = HashMap::new();
        materials.insert("Rojo".to_string(), Vec3::new(1.0, 0.0, 0.0));
        let mesh =
            parse_obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Rojo\nf 1 2 3\n", &materials).unwrap();
        assert_eq!(mesh.colors[0], Vec3::new(1.0, 0.0, 0.0));
    }
}
