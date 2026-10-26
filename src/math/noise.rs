//! Ruido de gradiente 3D (Perlin mejorado, 2002) con semilla, y sus fractales.
//!
//! # Por que gradiente y no valor
//! El ruido de valor del Proyecto 2 interpola numeros aleatorios puestos en
//! los nodos de una grilla: los maximos caen en los nodos y, visto de cerca,
//! se nota el enrejado. El de gradiente interpola **pendientes**: vale cero
//! en cada nodo y las formas quedan entre ellos, sin alinearse con los ejes.
//! Para terreno que se ve a pocos metros, la diferencia se nota.
//!
//! # Semilla
//! La tabla de permutacion se baraja con Fisher-Yates usando un generador
//! determinista: la misma semilla da exactamente el mismo planeta.

use super::Vec3;

pub struct Perlin {
    perm: [u8; 512],
}

impl Perlin {
    pub fn new(seed: u32) -> Self {
        let mut p: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut state = seed ^ 0x9E37_79B9;
        for i in (1..256).rev() {
            state = hash_u32(state.wrapping_add(i as u32));
            let j = (state % (i as u32 + 1)) as usize;
            p.swap(i, j);
        }
        Self {
            perm: std::array::from_fn(|i| p[i & 255]),
        }
    }

    /// Ruido de gradiente en `[-1, 1]` aproximadamente, continuo y con
    /// derivada continua.
    #[inline]
    pub fn noise(&self, p: Vec3) -> f32 {
        let (ix, iy, iz) = (floor_i(p.x), floor_i(p.y), floor_i(p.z));
        let xi = (ix & 255) as usize;
        let yi = (iy & 255) as usize;
        let zi = (iz & 255) as usize;
        let (x, y, z) = (p.x - ix as f32, p.y - iy as f32, p.z - iz as f32);
        let (u, v, w) = (fade(x), fade(y), fade(z));

        let perm = &self.perm;
        let a = perm[xi] as usize + yi;
        let aa = perm[a] as usize + zi;
        let ab = perm[a + 1] as usize + zi;
        let b = perm[xi + 1] as usize + yi;
        let ba = perm[b] as usize + zi;
        let bb = perm[b + 1] as usize + zi;

        lerp(
            w,
            lerp(
                v,
                lerp(u, grad(perm[aa], x, y, z), grad(perm[ba], x - 1.0, y, z)),
                lerp(
                    u,
                    grad(perm[ab], x, y - 1.0, z),
                    grad(perm[bb], x - 1.0, y - 1.0, z),
                ),
            ),
            lerp(
                v,
                lerp(
                    u,
                    grad(perm[aa + 1], x, y, z - 1.0),
                    grad(perm[ba + 1], x - 1.0, y, z - 1.0),
                ),
                lerp(
                    u,
                    grad(perm[ab + 1], x, y - 1.0, z - 1.0),
                    grad(perm[bb + 1], x - 1.0, y - 1.0, z - 1.0),
                ),
            ),
        )
    }

    /// Ruido browniano fraccional: suma de octavas, cada una al doble de
    /// frecuencia y a la mitad de amplitud. Devuelve `[-1, 1]` aproximadamente.
    ///
    /// Cada octava se desplaza un poco: el ruido de gradiente vale cero en los
    /// nodos enteros, y sin el desplazamiento todas las octavas valdrian cero
    /// en el mismo punto y se veria un "nudo" ahi.
    #[inline]
    pub fn fbm(&self, p: Vec3, octaves: u32) -> f32 {
        let mut sum = 0.0;
        let mut amplitude = 0.5;
        let mut frequency = 1.0;
        let mut norm = 0.0;
        for octave in 0..octaves {
            let shift = Vec3::new(17.13, 31.71, 7.77) * octave as f32;
            sum += amplitude * self.noise(p * frequency + shift);
            norm += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }
        sum / norm
    }

    /// Multifractal con crestas: `1 - |ruido|` dobla los valles en picos
    /// afilados, como cordilleras. Devuelve `[0, 1]`.
    pub fn ridged(&self, p: Vec3, octaves: u32) -> f32 {
        let mut sum = 0.0;
        let mut amplitude = 0.5;
        let mut frequency = 1.0;
        let mut norm = 0.0;
        for octave in 0..octaves {
            let shift = Vec3::new(5.31, 11.17, 23.9) * octave as f32;
            let ridge = 1.0 - self.noise(p * frequency + shift).abs();
            sum += amplitude * ridge * ridge;
            norm += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }
        sum / norm
    }
}

/// Suavizado quintico de Perlin: derivadas primera y segunda continuas.
#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

/// Los 12 gradientes de las aristas de un cubo, mas 4 repetidos para llegar a
/// 16 (el hash se recorta con `& 15`). Es la misma tabla que codifican las
/// ramas de la version original de Perlin, pero sin ramas: una lectura y un
/// producto punto, que el procesador no tiene que adivinar.
const GRADIENTS: [[f32; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, -1.0],
];

/// Producto punto con el gradiente que elige el hash.
#[inline]
fn grad(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let g = &GRADIENTS[(hash & 15) as usize];
    g[0] * x + g[1] * y + g[2] * z
}

/// Parte entera hacia menos infinito, sin pasar por `f32::floor`.
#[inline(always)]
fn floor_i(x: f32) -> i32 {
    let i = x as i32;
    i - i32::from(x < i as f32)
}

/// Finalizador de MurmurHash3: mezcla bien los bits de un entero.
#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_noise_and_different_seed_differs() {
        let p = Vec3::new(1.37, -4.2, 0.55);
        assert_eq!(Perlin::new(7).noise(p), Perlin::new(7).noise(p));
        assert_ne!(Perlin::new(7).noise(p), Perlin::new(8).noise(p));
    }

    /// La tabla de gradientes debe dar exactamente lo mismo que las ramas
    /// originales de Perlin.
    #[test]
    fn gradient_table_matches_the_branching_original() {
        fn original(hash: u8, x: f32, y: f32, z: f32) -> f32 {
            let h = hash & 15;
            let u = if h < 8 { x } else { y };
            let v = if h < 4 {
                y
            } else if h == 12 || h == 14 {
                x
            } else {
                z
            };
            (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
        }
        for hash in 0..=255u8 {
            let (x, y, z) = (0.31, -0.77, 0.52);
            assert_eq!(grad(hash, x, y, z), original(hash, x, y, z));
        }
        assert_eq!(floor_i(-0.5), -1);
        assert_eq!(floor_i(2.0), 2);
        assert_eq!(floor_i(-3.0), -3);
    }

    #[test]
    fn noise_is_continuous_and_bounded() {
        let n = Perlin::new(42);
        let mut max = 0.0f32;
        for i in 0..20_000 {
            let p = Vec3::new(i as f32 * 0.0137, i as f32 * 0.0071 - 3.0, (i % 97) as f32 * 0.11);
            let a = n.noise(p);
            let b = n.noise(p + Vec3::splat(1e-4));
            assert!((a - b).abs() < 1e-2, "salto en {p:?}");
            max = max.max(a.abs());
        }
        assert!(max <= 1.1 && max > 0.3);
    }
}
