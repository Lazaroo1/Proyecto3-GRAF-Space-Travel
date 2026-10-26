//! Lectura, bilineal y mipmaps de imagenes en CPU. RGB8 con dimensiones LE.
use crate::math::{Vec2, Vec3};
use std::{
    path::Path,
    sync::{Arc, OnceLock},
};
struct Level {
    w: usize,
    h: usize,
    data: Vec<[u8; 3]>,
}
pub struct ImageMap {
    levels: Vec<Level>,
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
impl ImageMap {
    pub fn load(path: &Path, srgb: bool) -> Result<Self, String> {
        let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if b.len() < 8 {
            return Err("imagen sin cabecera".into());
        }
        let w = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        if w == 0 || h == 0 || w > 16384 || h > 16384 || b.len() != 8 + w * h * 3 {
            return Err(format!("imagen RGB invalida: {}", path.display()));
        }
        let data = b[8..].chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        let mut levels = vec![Level { w, h, data }];
        while levels.last().unwrap().w > 1 || levels.last().unwrap().h > 1 {
            let prev = levels.last().unwrap();
            let w = (prev.w / 2).max(1);
            let h = (prev.h / 2).max(1);
            let mut data = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 3];
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let p = prev.data[(y * 2 + dy).min(prev.h - 1) * prev.w
                                + (x * 2 + dx).min(prev.w - 1)];
                            for c in 0..3 {
                                sum[c] += p[c] as u32;
                            }
                        }
                    }
                    data.push(sum.map(|v| ((v + 2) / 4) as u8));
                }
            }
            levels.push(Level { w, h, data });
        }
        Ok(Self { levels, srgb })
    }
    pub fn sample(&self, uv: Vec2, footprint: f32) -> Vec3 {
        let lod = (footprint * self.levels[0].w as f32)
            .max(1.0)
            .log2()
            .min((self.levels.len() - 1) as f32);
        let i = lod as usize;
        let t = lod - i as f32;
        let mut c = self.bilinear(i, uv);
        if t > 0.0 && i + 1 < self.levels.len() {
            c = c.lerp(self.bilinear(i + 1, uv), t);
        }
        c
    }
    fn bilinear(&self, i: usize, uv: Vec2) -> Vec3 {
        let l = &self.levels[i];
        let x = uv.x * l.w as f32 - 0.5;
        let y = uv.y * l.h as f32 - 0.5;
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let pixel = |x: i32, y: i32| {
            let p =
                l.data[y.rem_euclid(l.h as i32) as usize * l.w + x.rem_euclid(l.w as i32) as usize];
            if self.srgb {
                let t = table();
                Vec3::new(t[p[0] as usize], t[p[1] as usize], t[p[2] as usize])
            } else {
                Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 255.0
            }
        };
        pixel(ix, iy)
            .lerp(pixel(ix + 1, iy), fx)
            .lerp(pixel(ix, iy + 1).lerp(pixel(ix + 1, iy + 1), fx), fy)
    }
    pub fn sphere(&self, p: Vec3, footprint: f32) -> Vec3 {
        let p = p.normalize();
        let u = p.z.atan2(p.x) / std::f32::consts::TAU + 0.5;
        let v = p.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
        self.sample(Vec2::new(u, v.clamp(0.0001, 0.9999)), footprint)
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
    pub rock: ImageMap,
    pub rock_normal: ImageMap,
    pub rock_rough: ImageMap,
    pub rock_ao: ImageMap,
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
        let mut ship = Vec::new();
        let mut images = std::collections::HashMap::<String, Arc<ImageMap>>::new();
        for line in text.lines().filter(|s| !s.is_empty()) {
            let p: Vec<_> = line.split_whitespace().collect();
            if p.len() != 9 {
                return Err("tabla de materiales invalida".into());
            }
            let f = |i: usize| p[i].parse::<f32>().map_err(|e| e.to_string());
            let image = if p[8] == "-" {
                None
            } else {
                if !images.contains_key(p[8]) {
                    images.insert(
                        p[8].into(),
                        Arc::new(ImageMap::load(&base.join(p[8]), true)?),
                    );
                }
                images.get(p[8]).cloned()
            };
            ship.push(ShipMaterial {
                base: Vec3::new(f(0)?, f(1)?, f(2)?),
                rough: f(3)?,
                metal: f(4)?,
                emission: Vec3::new(f(5)?, f(6)?, f(7)?),
                image,
            });
        }
        Ok(Self {
            ship,
            jupiter: ImageMap::load(&base.join("jupiter.rgb"), true)?,
            moon: ImageMap::load(&base.join("moon.rgb"), true)?,
            rock: ImageMap::load(&base.join("rock-color.rgb"), true)?,
            rock_normal: ImageMap::load(&base.join("rock-normal.rgb"), false)?,
            rock_rough: ImageMap::load(&base.join("rock-rough.rgb"), false)?,
            rock_ao: ImageMap::load(&base.join("rock-ao.rgb"), false)?,
        })
    }
}
