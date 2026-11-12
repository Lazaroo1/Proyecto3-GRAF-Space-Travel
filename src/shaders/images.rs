//! Lectura, bilineal y mipmaps de imagenes en CPU. Las imagenes son PNG RGB
//! de 8 bits (`render::png`, sin perdida); se leen todas a la vez, cada una
//! en su hilo.
use crate::math::{Vec2, Vec3};
use std::{
    path::Path,
    sync::{Arc, OnceLock},
};
struct Level<const N: usize> {
    w: usize,
    h: usize,
    data: Vec<[u8; N]>,
}
pub struct ImageMap {
    levels: Vec<Level<3>>,
    srgb: bool,
}
fn linear(v: u8) -> f32 {
    let x = v as f32 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn table() -> &'static [f32; 256] {
    static T: OnceLock<[f32; 256]> = OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|i| linear(i as u8)))
}
/// Lee un PNG RGB.
fn read_png(path: &Path) -> Result<Level<3>, String> {
    let (w, h, rgb) = crate::render::png::read_rgb(path)?;
    let data = rgb.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
    Ok(Level { w: w as usize, h: h as usize, data })
}

/// Lee varios PNG de `base` a la vez, cada uno en su hilo (lo que mas tarda
/// es descomprimirlos), en el orden de `names`.
fn read_all(base: &Path, names: &[&str]) -> Result<Vec<Level<3>>, String> {
    std::thread::scope(|scope| {
        let jobs: Vec<_> = names.iter().map(|name| scope.spawn(move || read_png(&base.join(name)))).collect();
        jobs.into_iter().map(|job| job.join().map_err(|_| "no se pudo leer una imagen".to_string())?).collect()
    })
}
/// Piramide de mipmaps por promedio de 2x2, canal por canal.
fn pyramid<const N: usize>(base: Level<N>) -> Vec<Level<N>> {
    let mut levels = vec![base];
    while levels.last().unwrap().w > 1 || levels.last().unwrap().h > 1 {
        let prev = levels.last().unwrap();
        let w = (prev.w / 2).max(1);
        let h = (prev.h / 2).max(1);
        let mut data = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0u32; N];
                for dy in 0..2 {
                    for dx in 0..2 {
                        let p = prev.data[(y * 2 + dy).min(prev.h - 1) * prev.w
                            + (x * 2 + dx).min(prev.w - 1)];
                        for c in 0..N {
                            sum[c] += p[c] as u32;
                        }
                    }
                }
                data.push(sum.map(|v| ((v + 2) / 4) as u8));
            }
        }
        levels.push(Level { w, h, data });
    }
    levels
}
/// Nivel de mipmap (con su fraccion) para una huella de `footprint` en uv.
#[inline]
fn mip_level(footprint: f32, width: usize, levels: usize) -> (usize, f32) {
    let lod = (footprint * width as f32)
        .max(1.0)
        .log2()
        .min((levels - 1) as f32);
    let i = lod as usize;
    (i, lod - i as f32)
}
/// Esquina de abajo a la izquierda y pesos de la bilineal en un nivel.
#[inline]
fn corners(w: usize, h: usize, uv: Vec2) -> ([usize; 4], f32, f32) {
    let x = uv.x * w as f32 - 0.5;
    let y = uv.y * h as f32 - 0.5;
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let col = |x: i32| x.rem_euclid(w as i32) as usize;
    let row = |y: i32| y.rem_euclid(h as i32) as usize * w;
    let (x0, x1, y0, y1) = (col(ix), col(ix + 1), row(iy), row(iy + 1));
    ([y0 + x0, y0 + x1, y1 + x0, y1 + x1], x - fx, y - fy)
}
impl ImageMap {
    fn new(level: Level<3>, srgb: bool) -> Self {
        Self { levels: pyramid(level), srgb }
    }
    pub fn sample(&self, uv: Vec2, footprint: f32) -> Vec3 {
        let (i, t) = mip_level(footprint, self.levels[0].w, self.levels.len());
        let mut c = self.bilinear(i, uv);
        if t > 0.0 && i + 1 < self.levels.len() {
            c = c.lerp(self.bilinear(i + 1, uv), t);
        }
        c
    }
    fn bilinear(&self, i: usize, uv: Vec2) -> Vec3 {
        let l = &self.levels[i];
        let ([a, b, c, d], fx, fy) = corners(l.w, l.h, uv);
        let pixel = |index: usize| {
            let p = l.data[index];
            if self.srgb {
                let t = table();
                Vec3::new(t[p[0] as usize], t[p[1] as usize], t[p[2] as usize])
            } else {
                Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0
            }
        };
        pixel(a)
            .lerp(pixel(b), fx)
            .lerp(pixel(c).lerp(pixel(d), fx), fy)
    }
    pub fn sphere(&self, p: Vec3, footprint: f32) -> Vec3 {
        let p = p.normalize();
        let u = p.z.atan2(p.x) / std::f32::consts::TAU + 0.5;
        let v = p.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
        self.sample(Vec2::new(u, v.clamp(0.0001, 0.9999)), footprint)
    }
}
/// Los cuatro mapas de la roca (color, normal, rugosidad y oclusion) en un
/// solo texel de 8 bytes. Separados, cada muestra triplanar leia cuatro
/// imagenes distintas; juntos, una sola linea de cache por texel.
pub struct RockMap {
    levels: Vec<Level<8>>,
}
/// Una muestra de la roca. El color sale lineal; la normal, la rugosidad y
/// la oclusion quedan como estan en sus archivos (0 a 1).
pub struct RockTexel {
    pub color: Vec3,
    pub normal: Vec3,
    pub rough: f32,
    pub ao: f32,
}
impl RockMap {
    fn new([color, normal, rough, ao]: [Level<3>; 4]) -> Result<Self, String> {
        if [&normal, &rough, &ao]
            .iter()
            .any(|l| l.w != color.w || l.h != color.h)
        {
            return Err("los mapas de la roca no miden lo mismo".into());
        }
        let data = (0..color.data.len())
            .map(|i| {
                let (c, n) = (color.data[i], normal.data[i]);
                [c[0], c[1], c[2], n[0], n[1], n[2], rough.data[i][0], ao.data[i][0]]
            })
            .collect();
        Ok(Self {
            levels: pyramid(Level {
                w: color.w,
                h: color.h,
                data,
            }),
        })
    }
    pub fn sample(&self, uv: Vec2, footprint: f32) -> RockTexel {
        let (i, t) = mip_level(footprint, self.levels[0].w, self.levels.len());
        let mut c = self.bilinear(i, uv);
        if t > 0.0 && i + 1 < self.levels.len() {
            let d = self.bilinear(i + 1, uv);
            for k in 0..8 {
                c[k] += (d[k] - c[k]) * t;
            }
        }
        RockTexel {
            color: Vec3::new(c[0], c[1], c[2]),
            normal: Vec3::new(c[3], c[4], c[5]),
            rough: c[6],
            ao: c[7],
        }
    }
    fn bilinear(&self, i: usize, uv: Vec2) -> [f32; 8] {
        let l = &self.levels[i];
        let ([a, b, c, d], fx, fy) = corners(l.w, l.h, uv);
        let srgb = table();
        let texel = |index: usize| {
            let p = l.data[index];
            let k = 1.0 / 255.0;
            [
                srgb[p[0] as usize],
                srgb[p[1] as usize],
                srgb[p[2] as usize],
                p[3] as f32 * k,
                p[4] as f32 * k,
                p[5] as f32 * k,
                p[6] as f32 * k,
                p[7] as f32 * k,
            ]
        };
        let lerp = |p: [f32; 8], q: [f32; 8], t: f32| std::array::from_fn(|k| p[k] + (q[k] - p[k]) * t);
        lerp(lerp(texel(a), texel(b), fx), lerp(texel(c), texel(d), fx), fy)
    }
}
pub struct ShipMaterial {
    pub base: Vec3,
    pub rough: f32,
    pub metal: f32,
    pub emission: Vec3,
    pub image: Option<Arc<ImageMap>>,
}
pub struct Resources {
    pub ship: Vec<ShipMaterial>,
    pub jupiter: ImageMap,
    pub moon: ImageMap,
    pub rock: RockMap,
}
impl Resources {
    pub fn shared() -> Arc<Self> {
        static R: OnceLock<Arc<Resources>> = OnceLock::new();
        R.get_or_init(|| {
            let local = std::path::PathBuf::from("assets/textures");
            let base = if local.exists() {
                local
            } else {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/textures")
            };
            Arc::new(Self::load(&base).expect("Faltan los recursos de assets/textures"))
        })
        .clone()
    }
    fn load(base: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(base.join("ship-materials.txt")).map_err(|e| e.to_string())?;
        // Todas las imagenes de una vez: las de la nave, Jupiter, la Luna y
        // los cuatro mapas de la roca.
        let mut names: Vec<&str> = Vec::new();
        for line in text.lines().filter(|s| !s.is_empty()) {
            if let Some(name) = line.split_whitespace().nth(8).filter(|n| *n != "-" && !names.contains(n)) {
                names.push(name);
            }
        }
        let ship_count = names.len();
        names.extend(["jupiter.png", "moon.png", "rock-color.png", "rock-normal.png", "rock-rough.png", "rock-ao.png"]);
        let mut levels = read_all(base, &names)?.into_iter();
        let mut images = std::collections::HashMap::<String, Arc<ImageMap>>::new();
        for name in &names[..ship_count] {
            images.insert(name.to_string(), Arc::new(ImageMap::new(levels.next().ok_or("falta una imagen")?, true)));
        }
        let mut next = || levels.next().ok_or_else(|| "falta una imagen".to_string());
        let (jupiter, moon) = (ImageMap::new(next()?, true), ImageMap::new(next()?, true));
        let rock = RockMap::new([next()?, next()?, next()?, next()?])?;
        let mut ship = Vec::new();
        for line in text.lines().filter(|s| !s.is_empty()) {
            let p: Vec<_> = line.split_whitespace().collect();
            if p.len() != 9 {
                return Err("tabla de materiales invalida".into());
            }
            let f = |i: usize| p[i].parse::<f32>().map_err(|e| e.to_string());
            let image = if p[8] == "-" { None } else { images.get(p[8]).cloned() };
            ship.push(ShipMaterial {
                base: Vec3::new(f(0)?, f(1)?, f(2)?),
                rough: f(3)?,
                metal: f(4)?,
                emission: Vec3::new(f(5)?, f(6)?, f(7)?),
                image,
            });
        }
        Ok(Self { ship, jupiter, moon, rock })
    }
}
