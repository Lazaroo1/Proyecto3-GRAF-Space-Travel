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
//!
//! Las naves chapinas (`load_vehicle`) usan mas del `.mtl`: la luz que emite
//! (`Ke`) y el brillo (`Ns`), y del nombre del material sacan de que es cada
//! pieza (pintura, metal, vidrio, luz o hule). Como no llevan texturas, eso
//! viaja en la `uv` de cada vertice (`Surface`).

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
    parse_obj_with(text, &|name: &str| (materials.get(name).copied().unwrap_or(DEFAULT_COLOR), None))
}

/// Un material del `.mtl` con lo que usan las naves chapinas.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MtlMaterial {
    pub kd: Vec3,
    pub ke: Vec3,
    pub ns: f32,
}

/// Todos los materiales del `.mtl`: color difuso, emision y brillo.
pub fn parse_mtl_full(text: &str) -> HashMap<String, MtlMaterial> {
    let mut materials: HashMap<String, MtlMaterial> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next();
        if key == Some("newmtl") {
            current = parts.next().map(str::to_string);
            if let Some(name) = &current {
                materials.insert(name.clone(), MtlMaterial { kd: DEFAULT_COLOR, ke: Vec3::ZERO, ns: 100.0 });
            }
            continue;
        }
        let Some(material) = current.as_ref().and_then(|n| materials.get_mut(n)) else { continue };
        match key {
            Some("Kd") => material.kd = parse_vec3(parts).unwrap_or(material.kd),
            Some("Ke") => material.ke = parse_vec3(parts).unwrap_or(material.ke),
            Some("Ns") => material.ns = parts.next().and_then(|v| v.parse().ok()).unwrap_or(material.ns),
            _ => {}
        }
    }
    materials
}

/// De que es una pieza de una nave chapina. Va en la `u` de sus vertices; la
/// `v` lleva la rugosidad (o, en las luces, cuanto brillan).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Pintura con laca (y los murales y las lonas, mas mates).
    Paint = 0,
    /// Cromo, plata y oro.
    Metal = 1,
    /// Vidrios: oscuros, reflejan el cielo.
    Glass = 2,
    /// Las LEDs: emiten su luz (`Ke`).
    Light = 3,
    /// Llantas.
    Rubber = 4,
}

impl Surface {
    /// Por el nombre del material y lo que dice el `.mtl`.
    pub fn of(name: &str, material: &MtlMaterial) -> Self {
        let name = name.to_lowercase();
        if material.ke.max_element() > 0.05 || name.starts_with("led") {
            Surface::Light
        } else if ["cristal", "vidrio", "reflejo"].iter().any(|k| name.contains(k)) {
            Surface::Glass
        } else if ["cromo", "plata", "oro"].iter().any(|k| name.contains(k)) {
            Surface::Metal
        } else if name.contains("goma") {
            Surface::Rubber
        } else {
            Surface::Paint
        }
    }

    pub fn from_code(code: f32) -> Self {
        match code.round() as i32 {
            1 => Surface::Metal,
            2 => Surface::Glass,
            3 => Surface::Light,
            4 => Surface::Rubber,
            _ => Surface::Paint,
        }
    }
}

/// Rugosidad (de GGX) que corresponde al exponente de Blinn-Phong `Ns`:
/// `alpha = sqrt(2 / (Ns + 2))` y la rugosidad es su raiz.
pub fn roughness_from_ns(ns: f32) -> f32 {
    (2.0 / (ns.max(0.0) + 2.0)).powf(0.25)
}

/// Lee una nave chapina: el color de cada pieza (`Kd`, o en las luces su
/// `Ke` sin el brillo) va como color de vertice y su superficie en la `uv`.
pub fn load_vehicle(path: &Path) -> Result<Mesh, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut materials = HashMap::new();
    for line in text.lines() {
        if let Some(name) = line.trim().strip_prefix("mtllib ") {
            let mtl = dir.join(name.trim());
            let mtl_text = fs::read_to_string(&mtl).map_err(|e| format!("{}: {e}", mtl.display()))?;
            materials.extend(parse_mtl_full(&mtl_text));
        }
    }
    let style = |name: &str| {
        let material = materials.get(name).copied().unwrap_or_default();
        let surface = Surface::of(name, &material);
        match surface {
            Surface::Light => {
                let peak = material.ke.max_element().max(1e-3);
                (material.ke / peak, Some(Vec2::new(surface as u8 as f32, peak)))
            }
            _ => (material.kd, Some(Vec2::new(surface as u8 as f32, roughness_from_ns(material.ns)))),
        }
    };
    let mut mesh = parse_obj_with(&text, &style).map_err(|e| format!("{}: {e}", path.display()))?;
    // No lleva mapas de normales: las tangentes no se usan.
    mesh.tangents.clear();
    Ok(mesh)
}

/// Lee el `.obj` con `style`, que da el color de cada material y, si quiere,
/// la `uv` que llevan todos sus vertices (en vez de la del archivo).
fn parse_obj_with(text: &str, style: &dyn Fn(&str) -> (Vec3, Option<Vec2>)) -> Result<Mesh, String> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut mesh = Mesh::default();
    let mut has_normal = Vec::new();
    let mut lookup: HashMap<(usize, Option<usize>, Option<usize>, usize), u32> = HashMap::new();
    let mut material_ids: HashMap<String, usize> = HashMap::new();
    let mut material = 0;
    let mut color = DEFAULT_COLOR;
    let mut fixed_uv: Option<Vec2> = None;
    let mut face = Vec::new();

    for (n, line) in text.lines().enumerate() {
        let error = |msg: &str| format!("linea {}: {msg}: {line}", n + 1);
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                positions.push(parse_vec3(parts).ok_or_else(|| error("vertice invalido"))?)
            }
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
                (color, fixed_uv) = style(&name);
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
                        mesh.uvs.push(fixed_uv.unwrap_or_else(|| vt.map_or(Vec2::ZERO, |i| uvs[i])));
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
    if !uvs.is_empty() {
        let mut tangents = vec![Vec3::ZERO; mesh.positions.len()];
        let mut bitangents = tangents.clone();
        for &[ia, ib, ic] in &mesh.indices {
            let (a, b, c) = (ia as usize, ib as usize, ic as usize);
            let e1 = mesh.positions[b] - mesh.positions[a];
            let e2 = mesh.positions[c] - mesh.positions[a];
            let d1 = mesh.uvs[b] - mesh.uvs[a];
            let d2 = mesh.uvs[c] - mesh.uvs[a];
            let det = d1.x * d2.y - d1.y * d2.x;
            if det.abs() < 1e-9 {
                continue;
            }
            let t = (e1 * d2.y - e2 * d1.y) / det;
            let bt = (e2 * d1.x - e1 * d2.x) / det;
            for i in [a, b, c] {
                tangents[i] += t;
                bitangents[i] += bt;
            }
        }
        mesh.tangents = mesh
            .normals
            .iter()
            .enumerate()
            .map(|(i, &n)| {
                let mut t = (tangents[i] - n * tangents[i].dot(n)).normalize();
                if t.length_squared() < 0.1 {
                    let h = if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
                    t = h.cross(n).normalize();
                }
                let sign = if n.cross(t).dot(bitangents[i]) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                crate::math::Vec4::new(t.x, t.y, t.z, sign)
            })
            .collect();
    }
    mesh.update_bounds();
    Ok(mesh)
}

fn parse_vec3<'a>(parts: impl Iterator<Item = &'a str>) -> Option<Vec3> {
    let v: Vec<f32> = parts
        .take(3)
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

/// Indice del `.obj` (base 1, o negativo desde el final) a base 0.
fn resolve(token: Option<&str>, len: usize) -> Option<usize> {
    let index: i64 = token?.parse().ok()?;
    let resolved = if index > 0 {
        index - 1
    } else {
        len as i64 + index
    };
    (0..len as i64)
        .contains(&resolved)
        .then_some(resolved as usize)
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
    fn mirrored_uvs_preserve_tangent_handedness() {
        let mesh =
            parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt -1 0\nvt 0 1\nf 1/1 2/2 3/3\n").unwrap();
        assert_eq!(mesh.uvs[1], Vec2::new(-1.0, 0.0));
        for t in &mesh.tangents {
            assert!((t.x + 1.0).abs() < 1e-6 && t.y.abs() < 1e-6 && t.z.abs() < 1e-6);
            assert_eq!(t.w, -1.0);
        }
    }

    #[test]
    fn vehicle_materials_carry_their_surface_in_the_uv() {
        let mtl = "newmtl cromo\nKd 0.6 0.7 0.8\nNs 592.9\nnewmtl led_rojo\nKd 1 0 0\nKe 2 0.03 0.05\n\
                   newmtl granate\nKd 0.07 0.002 0.016\nNs 32\nnewmtl vidrio_tintado\nKd 0 0 0\n";
        let materials = parse_mtl_full(mtl);
        let surface = |name: &str| Surface::of(name, &materials[name]);
        assert_eq!(surface("cromo"), Surface::Metal);
        assert_eq!(surface("led_rojo"), Surface::Light);
        assert_eq!(surface("granate"), Surface::Paint);
        assert_eq!(surface("vidrio_tintado"), Surface::Glass);
        assert_eq!(materials["led_rojo"].ke, Vec3::new(2.0, 0.03, 0.05));
        // Pulido (Ns alto) es poco rugoso; mate (Ns bajo), mucho.
        assert!(roughness_from_ns(592.9) < 0.3 && roughness_from_ns(32.0) > 0.45);
        for code in 0..5 {
            assert_eq!(Surface::from_code(code as f32) as i32, code);
        }
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
        let mesh = parse_obj(
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Rojo\nf 1 2 3\n",
            &materials,
        )
        .unwrap();
        assert_eq!(mesh.colors[0], Vec3::new(1.0, 0.0, 0.0));
    }
}
