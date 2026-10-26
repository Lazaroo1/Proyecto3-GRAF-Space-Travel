//! Mapas de normales generados al arrancar.
//!
//! Cada mapa nace de un mapa de alturas dibujado con funciones de distancia
//! (surcos tallados, remaches, marcos). La normal de cada texel sale de la
//! pendiente de la altura por diferencias centrales:
//! `n = normalize(-dh/du, -dh/dv, 1)` en el espacio tangente, con z hacia
//! afuera de la superficie. Junto a la normal va un canal de brillo: lo que
//! el shader hace emitir luz (el ojo tallado de los monolitos).

use crate::math::{Perlin, Vec2, Vec3, smoothstep};

/// Lo que se lee de un mapa en un punto.
#[derive(Clone, Copy, Debug)]
pub struct Texel {
    /// Normal unitaria en el espacio tangente.
    pub normal: Vec3,
    /// Altura: 1 en la superficie, menos en lo tallado.
    pub height: f32,
    /// Cuanto brilla, de 0 a 1.
    pub glow: f32,
}

/// Un nivel de la piramide de mipmaps.
struct Level {
    size: usize,
    /// `size - 1`: los lados son potencias de dos y repetir es enmascarar.
    mask: i32,
    /// Normal en el espacio tangente (en los niveles chicos, el promedio de
    /// las normales que cubre: se acorta donde el relieve se cancela).
    normals: Vec<Vec3>,
    /// Altura y brillo de cada texel.
    values: Vec<Vec2>,
}

impl Level {
    /// Interpolacion bilineal con repeticion. Como el lado es potencia de
    /// dos, repetir es quedarse con los bits bajos del indice (tambien para
    /// los negativos), sin divisiones.
    fn bilinear(&self, uv: Vec2) -> (Vec3, Vec2) {
        let n = self.size as f32;
        let x = uv.x * n - 0.5;
        let y = uv.y * n - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i32, y0 as i32);
        let wrap = |v: i32| (v & self.mask) as usize;
        let (xa, xb, ya, yb) = (wrap(xi), wrap(xi + 1), wrap(yi), wrap(yi + 1));
        let i = |x: usize, y: usize| y * self.size + x;
        let normal = self.normals[i(xa, ya)]
            .lerp(self.normals[i(xb, ya)], fx)
            .lerp(
                self.normals[i(xa, yb)].lerp(self.normals[i(xb, yb)], fx),
                fy,
            );
        let v = |x: usize, y: usize| self.values[i(x, y)];
        let top = v(xa, ya) + (v(xb, ya) - v(xa, ya)) * fx;
        let bottom = v(xa, yb) + (v(xb, yb) - v(xa, yb)) * fx;
        (normal, top + (bottom - top) * fy)
    }

    /// El nivel siguiente: cada texel promedia un cuadro de 2x2.
    fn half(&self) -> Self {
        let size = self.size / 2;
        let at = |x: usize, y: usize| y * self.size + x;
        let mut normals = Vec::with_capacity(size * size);
        let mut values = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let quad = [
                    at(2 * x, 2 * y),
                    at(2 * x + 1, 2 * y),
                    at(2 * x, 2 * y + 1),
                    at(2 * x + 1, 2 * y + 1),
                ];
                normals.push(
                    quad.iter()
                        .fold(Vec3::ZERO, |acc, &k| acc + self.normals[k])
                        * 0.25,
                );
                values.push(quad.iter().fold(Vec2::ZERO, |acc, &k| acc + self.values[k]) * 0.25);
            }
        }
        Self {
            size,
            mask: size as i32 - 1,
            normals,
            values,
        }
    }
}

/// Mapa de normales con su piramide de mipmaps.
///
/// # Por que mipmaps
/// Cuando un pixel cubre muchos texeles, leer uno solo es tomar una muestra
/// al azar de lo que hay debajo: al moverse la camara el relieve parpadea.
/// Cada nivel promedia el anterior, y se lee el nivel donde un texel mide
/// lo mismo que el pixel (mezclando los dos mas cercanos). Las normales de
/// un surco se inclinan hacia los dos lados: al promediarlas se cancelan y
/// de lejos la piedra se ve lisa, que es lo correcto. El brillo promediado
/// conserva la luz total: de lejos el ojo es una mancha tenue, no un punto
/// que parpadea.
pub struct NormalMap {
    levels: Vec<Level>,
}

impl NormalMap {
    /// Mapa de `size x size` (potencia de dos) a partir de una funcion que da
    /// la altura y el brillo en `[0, 1]^2`. `relief` es cuantas veces el ancho
    /// de la textura mide un desnivel de 1.
    pub fn generate(size: usize, relief: f32, texel: impl Fn(f32, f32) -> (f32, f32)) -> Self {
        let values: Vec<Vec2> = (0..size * size)
            .map(|i| {
                let (h, glow) = texel(
                    (i % size) as f32 / size as f32,
                    (i / size) as f32 / size as f32,
                );
                Vec2::new(h, glow)
            })
            .collect();
        let height = |x: isize, y: isize| {
            let x = x.rem_euclid(size as isize) as usize;
            let y = y.rem_euclid(size as isize) as usize;
            values[y * size + x].x
        };
        let scale = relief * size as f32 * 0.5;
        let normals = (0..size * size)
            .map(|i| {
                let (x, y) = ((i % size) as isize, (i / size) as isize);
                let du = height(x + 1, y) - height(x - 1, y);
                let dv = height(x, y + 1) - height(x, y - 1);
                Vec3::new(-du * scale, -dv * scale, 1.0).normalize()
            })
            .collect();
        assert!(
            size.is_power_of_two(),
            "el lado de un mapa de normales es potencia de dos"
        );
        let mut levels = vec![Level {
            size,
            mask: size as i32 - 1,
            normals,
            values,
        }];
        while levels.last().is_some_and(|l| l.size > 4) {
            let next = levels.last().map(Level::half);
            levels.extend(next);
        }
        Self { levels }
    }

    /// Lo que hay en `uv` (con repeticion). `footprint` es cuanto mide un
    /// pixel en unidades de uv: elige el nivel de mipmap.
    pub fn sample(&self, uv: Vec2, footprint: f32) -> Texel {
        let texels = footprint * self.levels[0].size as f32;
        let lod = texels.max(1.0).log2().min((self.levels.len() - 1) as f32);
        let level = lod as usize;
        let blend = lod - level as f32;
        let (mut normal, mut value) = self.levels[level].bilinear(uv);
        if blend > 0.0 && level + 1 < self.levels.len() {
            let (n1, v1) = self.levels[level + 1].bilinear(uv);
            normal = normal.lerp(n1, blend);
            value = value + (v1 - value) * blend;
        }
        Texel {
            normal: normal.normalize(),
            height: value.x,
            glow: value.y,
        }
    }
}

/// Version que se repite sin costura de una funcion cualquiera en `[0, 1]^2`:
/// mezcla la funcion con sus copias corridas una vuelta, pesadas para que
/// en cada borde valga lo mismo que en el opuesto.
fn tileable(f: impl Fn(f32, f32) -> f32, u: f32, v: f32) -> f32 {
    f(u, v) * (1.0 - u) * (1.0 - v)
        + f(u - 1.0, v) * u * (1.0 - v)
        + f(u, v - 1.0) * (1.0 - u) * v
        + f(u - 1.0, v - 1.0) * u * v
}

/// Surco en V: 1 sobre la curva (distancia 0) y 0 a partir de `width`.
#[inline]
fn groove(distance: f32, width: f32) -> f32 {
    smoothstep(width, 0.0, distance.abs())
}

/// El glifo de los monolitos de Requiem: un ojo tallado en la piedra. Una
/// almendra de dos arcos, el iris, la pupila hundida, una lagrima que baja y
/// un marco alrededor. Brilla el ojo; el marco solo esta tallado.
/// Coordenadas: u de 0 a 1, v de 0 (abajo) a 1 (arriba).
pub fn eye_glyph(size: usize) -> NormalMap {
    let noise = Perlin::new(0x0E7E);
    NormalMap::generate(size, 0.05, |u, v| {
        let p = Vec2::new(u - 0.5, v - 0.55);
        // Almendra: interseccion de dos discos desplazados en vertical. La
        // distancia con signo de una interseccion es el maximo de las dos.
        let r = 0.42;
        let offset = 0.26;
        let below = (p.x * p.x + (p.y + offset) * (p.y + offset)).sqrt() - r;
        let above = (p.x * p.x + (p.y - offset) * (p.y - offset)).sqrt() - r;
        let outline = groove(below.max(above), 0.018);
        let center = (p.x * p.x + p.y * p.y).sqrt();
        let iris = groove(center - 0.11, 0.014);
        let pupil = smoothstep(0.06, 0.035, center);
        // Lagrima: linea que baja desde el ojo.
        let tear = if p.y < -0.15 && p.y > -0.42 {
            groove(p.x, 0.012)
        } else {
            0.0
        };
        // Marco a 0.06 del borde.
        let frame = groove(u.min(1.0 - u).min(v.min(1.0 - v)) - 0.06, 0.012);
        let carved = 0.5 * outline + 0.45 * iris + 0.9 * pupil + 0.35 * tear + 0.3 * frame;
        // Piedra gastada.
        let h = 1.0 - carved + 0.02 * noise.fbm(Vec3::new(u * 18.0, v * 18.0, 0.5), 3);
        let glow = (outline * 0.8 + iris * 0.9 + pupil + tear * 0.6).min(1.0);
        (h, glow)
    })
}

/// Placas del casco de las naves naufragas: una grilla de paneles de tamano
/// variable con ranuras entre ellos, remaches en las esquinas y abolladuras.
pub fn hull_panels(size: usize) -> NormalMap {
    let noise = Perlin::new(0x4011);
    NormalMap::generate(size, 0.04, |u, v| {
        // Filas y columnas desplazadas por fila, de ancho variable.
        let rows = 4.0;
        let row = (v * rows).floor();
        let fy = v * rows - row;
        let shift = ((row * 0.37).sin() * 0.5 + 0.5) * 0.5;
        let cols = 2.0 + (row % 3.0);
        let fx = (u * cols + shift).fract();
        let seam = groove(fx.min(1.0 - fx) * (1.0 / cols), 0.008)
            .max(groove(fy.min(1.0 - fy) * (1.0 / rows), 0.008));
        let corner = Vec2::new(fx.min(1.0 - fx) / cols, fy.min(1.0 - fy) / rows);
        let rivet = smoothstep(0.012, 0.004, (corner.x - 0.02).hypot(corner.y - 0.02));
        // Las abolladuras tienen que repetirse sin costura: el casco usa la
        // textura varias veces seguidas.
        let dent = tileable(|a, b| noise.fbm(Vec3::new(a * 6.0, b * 6.0, 1.5), 3), u, v);
        (1.0 - 0.25 * seam + 0.15 * rivet + 0.08 * dent, 0.0)
    })
}

/// Texturas compartidas por los shaders.
pub struct Textures {
    pub images: std::sync::Arc<super::images::Resources>,
    pub eye_glyph: NormalMap,
    pub hull: NormalMap,
    pub organic: NormalMap,
}

impl Textures {
    pub fn generate() -> Self {
        let noise = Perlin::new(0xB105);
        let organic = NormalMap::generate(256, 0.035, |u, v| {
            let cells = |a: f32, b: f32| {
                let (f1, f2) = noise.cells(Vec3::new(a * 7.0, b * 7.0, 2.7));
                (f2 - f1).min(0.45)
            };
            let membrane = tileable(cells, u, v);
            let h = smoothstep(0.0, 0.24, membrane);
            (0.25 + 0.65 * h, smoothstep(0.25, 0.04, membrane))
        });
        Self {
            images: super::images::Resources::shared(),
            eye_glyph: eye_glyph(256),
            hull: hull_panels(256),
            organic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_heights_give_straight_up_normals_and_slopes_tilt_them() {
        let flat = NormalMap::generate(16, 1.0, |_, _| (1.0, 0.0));
        let texel = flat.sample(Vec2::new(0.3, 0.7), 0.0);
        assert!((texel.normal - Vec3::Z).length() < 1e-6 && (texel.height - 1.0).abs() < 1e-6);
        // Altura que crece en u: la normal se inclina hacia -u.
        let ramp = NormalMap::generate(16, 1.0, |u, _| ((u * std::f32::consts::TAU).sin(), 0.0));
        assert!(ramp.sample(Vec2::new(0.0, 0.5), 0.0).normal.x < -0.1);
    }

    #[test]
    fn far_away_the_relief_averages_out_and_the_glow_is_kept() {
        let glyph = eye_glyph(128);
        // Un pixel que cubre toda la textura lee el ultimo nivel: casi plano,
        // con la altura y el brillo promedio.
        let far = glyph.sample(Vec2::new(0.5, 0.55), 1.0);
        let near = glyph.sample(Vec2::new(0.5, 0.55), 0.0);
        assert!(far.normal.z > 0.95, "{far:?}");
        assert!(far.height > near.height + 0.3);
        let mean = |footprint: f32| {
            let n = 64;
            (0..n * n)
                .map(|i| {
                    glyph
                        .sample(
                            Vec2::new((i % n) as f32 / n as f32, (i / n) as f32 / n as f32),
                            footprint,
                        )
                        .glow
                })
                .sum::<f32>()
                / (n * n) as f32
        };
        assert!(
            (mean(0.0) - mean(1.0)).abs() < 0.01,
            "el brillo total se conserva"
        );
    }

    #[test]
    fn the_eye_glyph_is_carved_and_glows_in_the_middle_but_not_on_the_frame() {
        let glyph = eye_glyph(128);
        let center = glyph.sample(Vec2::new(0.5, 0.55), 0.0);
        let plain = glyph.sample(Vec2::new(0.2, 0.9), 0.0);
        let frame = glyph.sample(Vec2::new(0.5, 0.06), 0.0);
        assert!(
            center.height < plain.height - 0.5,
            "la pupila debe estar hundida"
        );
        assert!(center.glow > 0.9 && plain.glow == 0.0);
        assert!(frame.height < plain.height - 0.1 && frame.glow == 0.0);
    }
}
