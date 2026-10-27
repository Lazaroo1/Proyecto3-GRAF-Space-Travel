//! Funciones de altura de los planetas.
//!
//! Todas reciben una direccion unitaria en el espacio del planeta y devuelven
//! la altura en unidades del mundo mas unos atributos para el shader. Son
//! deterministas: la misma semilla da exactamente el mismo planeta.

use super::{HeightField, TerrainSample};
use crate::math::{Perlin, Vec3, hash_u32, smoothstep};

/// Campo de crateres: el espacio se divide en celdas cubicas y cada celda
/// tiene, con cierta probabilidad, un crater con centro y tamano al azar.
///
/// Un punto solo necesita revisar su celda y las 26 vecinas: el radio de
/// influencia de un crater nunca pasa de una celda. Es la misma idea que el
/// ruido celular (Worley), con un perfil de crater en vez de una distancia.
pub struct Craters {
    /// Celdas por unidad de radio del planeta.
    pub frequency: f32,
    /// Probabilidad de que una celda tenga crater.
    pub chance: f32,
    /// Radios minimo y maximo, en celdas (el maximo no debe pasar de 0.6).
    pub min_radius: f32,
    pub max_radius: f32,
    /// Profundidad del crater mas grande, en unidades del mundo.
    pub depth: f32,
    pub seed: u32,
}

impl Craters {
    /// Altura que agregan los crateres en `dir` y cuanto borde de crater hay
    /// ahi (0 a 1, para el shader).
    pub fn sample(&self, dir: Vec3) -> (f32, f32) {
        let q = dir * self.frequency;
        let base = (q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32);
        let mut height = 0.0;
        let mut rim = 0.0f32;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let cell = (base.0 + dx, base.1 + dy, base.2 + dz);
                    let h = cell_hash(cell, self.seed);
                    if unit(h) >= self.chance {
                        continue;
                    }
                    let h1 = hash_u32(h);
                    let h2 = hash_u32(h1);
                    let h3 = hash_u32(h2);
                    let h4 = hash_u32(h3);
                    let center = Vec3::new(
                        cell.0 as f32 + unit(h1),
                        cell.1 as f32 + unit(h2),
                        cell.2 as f32 + unit(h3),
                    );
                    // Mas crateres chicos que grandes.
                    let size = unit(h4);
                    let r = self.min_radius + (self.max_radius - self.min_radius) * size * size;
                    let d = (q - center).length() / r;
                    if d >= 1.6 {
                        continue;
                    }
                    // Cuenco parabolico con fondo plano y un anillo elevado
                    // en el borde que se desvanece hacia afuera.
                    let ring = (-((d - 1.0) / 0.22).powi(2)).exp();
                    let shape = (d * d - 1.0).clamp(-0.6, 0.0) + 0.35 * ring;
                    let scale = r / self.max_radius;
                    height += shape * self.depth * scale;
                    rim = rim.max(ring * scale);
                }
            }
        }
        (height, rim)
    }

    /// Altura minima y maxima que pueden sumar (dos crateres encimados).
    pub fn range(&self) -> (f32, f32) {
        (-1.2 * self.depth, 0.7 * self.depth)
    }
}

#[inline]
fn cell_hash(cell: (i32, i32, i32), seed: u32) -> u32 {
    hash_u32(
        (cell.0 as u32).wrapping_mul(0x8DA6_B343)
            ^ (cell.1 as u32).wrapping_mul(0xD816_3841)
            ^ (cell.2 as u32).wrapping_mul(0xCB1A_B31F)
            ^ seed,
    )
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Requiem: mundo helado y muerto. Los mares se congelaron en llanuras
/// planas; tierra adentro se levantan cordilleras de crestas afiladas y
/// todo esta marcado por crateres.
///
/// Atributos: `x` hielo (llanuras congeladas), `y` nieve en las alturas,
/// `z` borde de crater.
pub struct RequiemField {
    noise: Perlin,
    craters: Craters,
    small: Craters,
}

impl RequiemField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            craters: Craters {
                frequency: 7.0,
                chance: 0.35,
                min_radius: 0.2,
                max_radius: 0.6,
                depth: 4.0,
                seed: seed ^ 0x51ED,
            },
            small: Craters {
                frequency: 24.0,
                chance: 0.25,
                min_radius: 0.15,
                max_radius: 0.5,
                depth: 1.2,
                seed: seed ^ 0xC0FE,
            },
        }
    }
}

impl HeightField for RequiemField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let continent = n.fbm(dir * 1.3 + Vec3::new(3.1, -7.2, 1.7), 4);
        let land = smoothstep(-0.02, 0.14, continent);
        // Crestas al cubo: picos afilados y valles anchos, solo tierra adentro.
        let ridges = n.ridged(dir * 3.4 + Vec3::new(-4.4, 2.2, 9.1), 5);
        let mountains = ridges * ridges * ridges * smoothstep(0.08, 0.4, continent);
        let detail = n.fbm(dir * 16.0 + Vec3::new(1.3, 5.5, -2.7), 3);
        let (big, big_rim) = self.craters.sample(dir);
        let (small, small_rim) = self.small.sample(dir);
        let relief = land * (1.5 + 1.2 * detail) + mountains * 13.0;
        // El hielo rellena a medias los crateres de las llanuras.
        let craters = (big + small) * (0.35 + 0.65 * land);
        let height = relief + craters + (1.0 - land) * 0.15 * detail;
        TerrainSample {
            height,
            attrs: Vec3::new(
                1.0 - land,
                smoothstep(6.0, 10.0, relief),
                big_rim.max(small_rim),
            ),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        let (lo_a, hi_a) = self.craters.range();
        let (lo_b, hi_b) = self.small.range();
        (lo_a + lo_b - 0.3, 1.5 + 1.2 + 13.0 + hi_a + hi_b + 0.3)
    }
}

/// Maximo suave: como `a.max(b)` pero sin el quiebre donde se cruzan.
#[inline]
fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (a - b) / k).clamp(0.0, 1.0);
    b + (a - b) * h + k * h * (1.0 - h)
}

/// Pira: la estrella la esta devorando. Tiene acoplamiento de marea, asi
/// que siempre le da la misma cara (el lado -X del planeta): de ese lado la
/// roca se derrite y los valles son mares de lava; del otro, la costra se
/// enfria pero sigue agrietada.
///
/// Atributos: `x` fundido (mar de lava), `y` calor (cercania a la cara de la
/// estrella), `z` crestas.
pub struct PiraField {
    noise: Perlin,
}

impl PiraField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
        }
    }
}

impl HeightField for PiraField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let heat = smoothstep(-0.25, 0.9, -dir.x);
        let ridges = n.ridged(dir * 2.6 + Vec3::new(2.3, -1.1, 4.7), 5);
        let base = n.fbm(dir * 1.6 + Vec3::new(-6.1, 3.3, 0.4), 4);
        let detail = n.fbm(dir * 14.0 + Vec3::new(0.7, 8.2, -3.9), 2);
        let rock = base * 4.0 + ridges * ridges * 9.0 + detail * 0.8;
        // El mar de lava sube del lado de la estrella y es plano: lo que queda
        // por debajo de su nivel se cubre.
        let lava_level = -1.5 + 6.0 * heat;
        let molten = smoothstep(lava_level + 0.5, lava_level - 0.3, rock);
        TerrainSample {
            height: smooth_max(rock, lava_level, 0.6),
            attrs: Vec3::new(molten, heat, ridges),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        // La lava rellena todo lo que queda bajo su nivel minimo (-1.5); el
        // maximo suave puede pasarse del mayor por `k / 4`.
        (-1.6, 4.0 + 9.0 + 0.8 + 0.2)
    }
}

/// Frecuencia de la red de arterias de Jardin, en ciclos por radio.
pub const ARTERY_FREQUENCY: f32 = 2.6;
const ARTERY_OFFSET: Vec3 = Vec3::new(11.3, -5.7, 2.9);

/// Red de arterias de Jardin: cerca de 1 sobre las curvas donde un ruido de
/// Perlin vale cero. En una esfera esas curvas forman una red cerrada que
/// parece organica. El terreno la usa para levantar las arterias y el shader
/// para hacerlas brillar: con el mismo ruido, el relieve y la luz coinciden.
#[inline]
pub fn artery(noise: &Perlin, dir: Vec3) -> f32 {
    1.0 - noise.noise(dir * ARTERY_FREQUENCY + ARTERY_OFFSET).abs()
}

/// Jardin: un planeta vivo. La superficie es tejido con pliegues, recorrido
/// por una red de arterias elevadas y salpicado de poros. Los pliegues usan
/// domain warping (el ruido se muestrea en un punto desplazado por otro
/// ruido), que da formas organicas en vez de colinas regulares.
///
/// Atributos: `x` arteria, `y` poro, `z` pliegue (0 valle, 1 cresta).
pub struct JardinField {
    noise: Perlin,
    /// El ruido de las arterias usa la semilla global, la misma que reciben
    /// los shaders.
    arteries: Perlin,
}

impl JardinField {
    pub fn new(seed: u32, global_seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            arteries: Perlin::new(global_seed),
        }
    }
}

impl HeightField for JardinField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let warp = Vec3::new(
            n.noise(dir * 1.7 + Vec3::new(1.0, 2.0, 3.0)),
            n.noise(dir * 1.7 + Vec3::new(-4.0, 0.5, 2.5)),
            n.noise(dir * 1.7 + Vec3::new(3.3, -2.2, -1.1)),
        ) * 0.35;
        let folds = n.fbm(dir * 2.2 + warp, 5);
        let ridge = smoothstep(0.955, 1.0, artery(&self.arteries, dir));
        let (f1, _) = n.cells(dir * 9.0);
        let pore = smoothstep(0.35, 0.12, f1);
        TerrainSample {
            height: folds * 18.0 + ridge * 2.3 - pore * 4.5,
            attrs: Vec3::new(ridge, pore, (folds * 0.8 + 0.5).clamp(0.0, 1.0)),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        (-18.0 - 4.5, 18.0 + 2.3)
    }
}

/// Cementerio: roca muerta marcada por impactos. Los crateres son de los
/// restos del exodo que cayeron, y las zanjas largas, de naves que se
/// arrastraron al estrellarse.
///
/// Atributos: `x` borde de crater, `y` zanja, `z` base del terreno.
pub struct CementerioField {
    noise: Perlin,
    craters: Craters,
    small: Craters,
}

impl CementerioField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            craters: Craters {
                frequency: 6.0,
                chance: 0.45,
                min_radius: 0.25,
                max_radius: 0.6,
                depth: 5.0,
                seed: seed ^ 0xA11,
            },
            small: Craters {
                frequency: 18.0,
                chance: 0.4,
                min_radius: 0.15,
                max_radius: 0.55,
                depth: 1.6,
                seed: seed ^ 0xB22,
            },
        }
    }
}

impl HeightField for CementerioField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let base = n.fbm(dir * 1.8 + Vec3::new(5.0, -2.0, 7.5), 5);
        let (big, big_rim) = self.craters.sample(dir);
        let (small, small_rim) = self.small.sample(dir);
        let gouge = smoothstep(
            0.93,
            0.99,
            n.ridged(dir * 3.0 + Vec3::new(-8.0, 1.0, 2.0), 2),
        );
        TerrainSample {
            height: base * 4.0 + big + small - gouge * 2.5,
            attrs: Vec3::new(big_rim.max(small_rim), gouge, base * 0.5 + 0.5),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        let (lo_a, hi_a) = self.craters.range();
        let (lo_b, hi_b) = self.small.range();
        (-4.0 + lo_a + lo_b - 2.5, 4.0 + hi_a + hi_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directions() -> impl Iterator<Item = Vec3> {
        (0..4000).map(|i| {
            let t = i as f32 * 0.37;
            Vec3::new(
                t.sin() * (t * 0.31).cos(),
                (t * 0.77).cos(),
                t.cos() * (t * 0.13).sin(),
            )
            .normalize()
        })
    }

    /// Cada planeta se queda dentro del rango que declara (el culling por
    /// horizonte depende de eso) y la misma semilla da el mismo terreno.
    #[test]
    fn every_planet_stays_inside_its_declared_range_and_is_deterministic() {
        let fields: Vec<(Box<dyn HeightField>, Box<dyn HeightField>)> = vec![
            (
                Box::new(RequiemField::new(7)),
                Box::new(RequiemField::new(7)),
            ),
            (Box::new(PiraField::new(7)), Box::new(PiraField::new(7))),
            (
                Box::new(JardinField::new(7, 1)),
                Box::new(JardinField::new(7, 1)),
            ),
            (
                Box::new(CementerioField::new(7)),
                Box::new(CementerioField::new(7)),
            ),
        ];
        for (index, (a, b)) in fields.iter().enumerate() {
            let (lo, hi) = a.height_range();
            for dir in directions() {
                let s = a.sample(dir);
                assert!(
                    s.height >= lo && s.height <= hi,
                    "campo {index}: {} fuera de [{lo}, {hi}]",
                    s.height
                );
                assert_eq!(s.height, b.sample(dir).height);
            }
        }
    }

    #[test]
    fn craters_are_continuous() {
        let craters = Craters {
            frequency: 7.0,
            chance: 0.5,
            min_radius: 0.2,
            max_radius: 0.6,
            depth: 4.0,
            seed: 3,
        };
        for i in 0..20_000 {
            let t = i as f32 * 0.0013;
            let dir = Vec3::new(t.cos(), (t * 3.1).sin() * 0.4, t.sin()).normalize();
            let next = Vec3::new(
                (t + 1e-4).cos(),
                ((t + 1e-4) * 3.1).sin() * 0.4,
                (t + 1e-4).sin(),
            )
            .normalize();
            let (a, _) = craters.sample(dir);
            let (b, _) = craters.sample(next);
            assert!((a - b).abs() < 0.05, "salto de {a} a {b}");
        }
    }
}

/// Parpado, la luna de Vigia: cordilleras de cristal y lagos quietos. Le da
/// siempre la misma cara a Vigia (la -X); de ese lado hay un lago grande
/// (`MIRROR_LAKE`) donde se refleja el ojo.
///
/// Atributos: `x` lago (1 en el agua), `y` cristal (en las crestas altas),
/// `z` orilla.
pub struct ParpadoField {
    noise: Perlin,
}

/// Donde se para quien llega a ver el ojo en el lago (la orilla) y el
/// centro del lago, en el espacio de la luna: desde la orilla Vigia queda a
/// unos 13 grados sobre el horizonte, del otro lado del agua, y su reflejo
/// cabe abajo.
pub const MIRROR_SHORE: Vec3 = Vec3::new(-0.3306, 0.9167, 0.2244);
pub const MIRROR_LAKE: Vec3 = Vec3::new(-0.4679, 0.8584, 0.2102);
/// Radio angular del lago grande.
const MIRROR_RADIUS: f32 = 0.14;

impl ParpadoField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
        }
    }
}

impl HeightField for ParpadoField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let hills = n.fbm(dir * 3.0 + Vec3::new(2.1, -1.7, 4.4), 4);
        // Cordilleras de cristal: crestas afiladas (a la cuarta) donde una
        // mascara grande las deja crecer.
        let ridges = n.ridged(dir * 2.4 + Vec3::new(-3.3, 5.1, 0.7), 4);
        let ranges = smoothstep(-0.05, 0.3, n.fbm(dir * 1.2 + Vec3::new(7.7, 1.1, -2.3), 3));
        let peaks = ridges.powi(4) * ranges;
        // El lago grande y otros chicos donde el ruido baja. El agua llena
        // todo lo que queda bajo cero: un espejo plano.
        let lake = MIRROR_LAKE.normalize();
        let angle = dir.dot(lake).clamp(-1.0, 1.0).acos();
        let mirror = smoothstep(MIRROR_RADIUS + 0.12, MIRROR_RADIUS - 0.03, angle);
        let pools = smoothstep(-0.22, -0.38, n.fbm(dir * 2.0 + Vec3::new(-6.2, 3.3, 8.8), 3));
        let basin = mirror.max(pools);
        let ground = 1.6 + 1.5 * hills + 26.0 * peaks * (1.0 - basin) - 5.0 * basin;
        TerrainSample {
            height: ground.max(0.0),
            attrs: Vec3::new(
                smoothstep(0.02, -0.3, ground),
                smoothstep(0.2, 0.55, peaks),
                smoothstep(1.6, 0.1, ground),
            ),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        (0.0, 1.6 + 1.5 + 26.0 + 0.5)
    }
}

/// Epitafio, la luna de Requiem: hielo viejo lleno de crateres. Usa el
/// mismo shader que Requiem (atributos: `x` hielo, que no tiene; `y` nieve
/// en lo alto; `z` borde de crater).
pub struct EpitafioField {
    noise: Perlin,
    craters: Craters,
}

impl EpitafioField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            craters: Craters {
                frequency: 9.0,
                chance: 0.45,
                min_radius: 0.15,
                max_radius: 0.6,
                depth: 3.0,
                seed: seed ^ 0xE91A,
            },
        }
    }
}

impl HeightField for EpitafioField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let relief = self.noise.fbm(dir * 3.5 + Vec3::new(1.9, -4.1, 2.7), 4);
        let (craters, rim) = self.craters.sample(dir);
        let height = 1.5 + 1.5 * relief + craters;
        TerrainSample {
            height,
            attrs: Vec3::new(0.0, smoothstep(2.0, 3.5, height), rim),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        let (lo, hi) = self.craters.range();
        (lo, 3.0 + hi + 0.2)
    }
}
