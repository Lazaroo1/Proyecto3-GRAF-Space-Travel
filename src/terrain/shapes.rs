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

/// Argos: un planeta de carne con un ojo. Aqui solo esta la carne: pliegues
/// gruesos y redondos (tubos donde un ruido torcido cruza por cero, como
/// intestinos o tendones) en dos escalas sobre bultos grandes. El ojo no esta
/// en el relieve: se mueve para mirar a la nave, asi que lo pone el vertex
/// shader (`shaders::argos::lift`), que aplana la carne donde esta el ojo y
/// levanta los parpados alrededor.
///
/// Atributos: `x` cresta de los pliegues, `y` fondo de los surcos, `z`
/// tendones (los pliegues finos).
pub struct ArgosField {
    noise: Perlin,
}

/// Lo mas que el vertex shader de Argos levanta la carne (los parpados), en
/// unidades del mundo.
pub const ARGOS_LID: f32 = 10.0;

impl ArgosField {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
        }
    }
}

/// Perfil de un tubo redondo de medio ancho `w` alrededor de donde `n`
/// cruza por cero: 1 en el centro, 0 afuera. Es `(1 - x^2)^1.5`: redondo
/// arriba y sin pared vertical al llegar al surco (una semicircunferencia
/// tiene pendiente infinita en el borde y la malla la dibuja en dientes).
#[inline]
fn tube(n: f32, w: f32) -> f32 {
    let x = (n.abs() / w).min(1.0);
    let t = 1.0 - x * x;
    t * t.sqrt()
}

impl HeightField for ArgosField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let warp = Vec3::new(
            n.noise(dir * 2.2 + Vec3::new(3.1, 0.0, 0.0)),
            n.noise(dir * 2.2 + Vec3::new(0.0, 7.4, 0.0)),
            n.noise(dir * 2.2 + Vec3::new(0.0, 0.0, 11.9)),
        );
        let q = dir + warp * 0.18;
        let lumps = n.fbm(q * 2.5 + Vec3::new(5.0, 1.0, 2.0), 3);
        let folds = tube(n.noise(q * 5.5), 0.22);
        let tendons = tube(n.noise(q * 14.0 + Vec3::splat(9.0)), 0.16);
        TerrainSample {
            height: 3.5 * lumps + 8.0 * folds + 2.5 * tendons,
            attrs: Vec3::new(folds, 1.0 - folds.max(tendons), tendons),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        // El fbm de tres octavas no pasa de 0.875; arriba, lo que levantan
        // los parpados.
        (-3.1, 3.1 + 8.0 + 2.5 + ARGOS_LID)
    }
}

/// Pira se esta despedazando: la marea de la estrella (esta mas adentro de
/// su limite de Roche para un cuerpo rigido) partio su corteza en
/// continentes que se separan, y entre ellos se abren abismos hasta el magma
/// del interior. Es lo que se cree que le pasa a WD 1145+017 b, un cuerpo
/// que se desintegra frente a su estrella.
///
/// - **Placas**: un Voronoi sobre la esfera (la placa de un punto es el
///   centro mas cercano: el de mayor producto punto), con los bordes
///   desgarrados por un ruido que mueve el punto antes de buscar su placa.
///   Cada placa tiene su relieve, su altura y su inclinacion: parecen pedazos
///   sueltos, no un solo suelo.
/// - **Abismos**: a la distancia angular `g` del borde entre las dos placas
///   mas cercanas, `g ~ (d1 - d2) / |c1 - c2|` (con `d` los productos punto y
///   `c` los centros). Donde `g` es menor que el ancho del abismo, que cambia
///   a lo largo del borde (grietas angostas y abismos anchos), el suelo cae
///   en una pared casi vertical hasta el magma, `CORE_DEPTH` mas abajo. Antes
///   de caer, la orilla se levanta un poco: la placa se dobla al romperse.
/// - **El lugar** (`SITE`): dos de las placas se ponen a proposito para que
///   un abismo pase frente a el.
///
/// Atributos: `x` magma (1 en el fondo), `y` lado dia, `z` cuanto se ha
/// bajado hacia el fondo (0 en la placa, 1 en el magma: las paredes brillan
/// mas abajo).
pub struct PiraField {
    noise: Perlin,
    plates: Vec<Plate>,
    /// El punto del borde que pasa frente al lugar: ahi el abismo es ancho.
    gorge: Vec3,
}

/// Una placa: el centro de su celda, cuanto sube o baja entera y su
/// inclinacion (un vector perpendicular al centro: la altura cambia con
/// `dir . tilt`).
#[derive(Clone, Copy, Debug)]
struct Plate {
    center: Vec3,
    lift: f32,
    tilt: Vec3,
}

/// Profundidad del magma bajo el radio del planeta (unidades del mundo).
pub const PIRA_CORE_DEPTH: f32 = 26.0;
/// La normal del lugar de Pira, en su espacio: cerca del terminador (el lado
/// de la estrella es -X, por la marea), mirando hacia ella.
pub const PIRA_SITE: Vec3 = Vec3::new(-0.42, 0.1, -0.9);
/// Placas: las del lugar mas las del resto de la esfera.
const PIRA_PLATES: usize = 20;

impl PiraField {
    pub fn new(seed: u32) -> Self {
        let site = PIRA_SITE.normalize();
        // Hacia la estrella sobre la superficie, desde el lugar.
        let toward = (Vec3::new(-1.0, 0.0, 0.0) - site * -site.x).normalize();
        let along = |angle: f32| site * angle.cos() + toward * angle.sin();
        let unit = |h: u32| (h >> 8) as f32 / 16_777_216.0;
        // El lugar queda en su placa y el borde con la siguiente pasa unos
        // 0.28 rad (31 unidades) adelante, hacia la estrella.
        let mut centers = vec![along(-0.3), along(0.86)];
        // Las demas, repartidas en la esfera (Fibonacci con desorden), lejos
        // de esas dos.
        let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
        let total = 36;
        for k in 0..total {
            let h = hash_u32(seed ^ (k as u32).wrapping_mul(0x9E37_79B9));
            let y = 1.0 - 2.0 * (k as f32 + 0.25 + 0.5 * unit(h)) / total as f32;
            let ring = (1.0 - y * y).max(0.0).sqrt();
            let a = golden * k as f32 + 0.6 * unit(hash_u32(h));
            let c = Vec3::new(ring * a.cos(), y, ring * a.sin());
            if centers.iter().all(|&o: &Vec3| o.dot(c) < 0.8) && centers.len() < PIRA_PLATES {
                centers.push(c);
            }
        }
        let plates = centers
            .iter()
            .enumerate()
            .map(|(k, &center)| {
                let h = hash_u32(seed ^ 0x9A7E ^ (k as u32).wrapping_mul(0x85EB_CA6B));
                let helper = if center.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
                let a = helper.cross(center).normalize();
                let b = center.cross(a);
                let angle = unit(h) * std::f32::consts::TAU;
                Plate {
                    center,
                    lift: -2.0 + 6.0 * unit(hash_u32(h)),
                    tilt: (a * angle.cos() + b * angle.sin()) * (4.0 + 8.0 * unit(hash_u32(h ^ 7))),
                }
            })
            .collect();
        Self {
            noise: Perlin::new(seed),
            plates,
            gorge: along(0.28),
        }
    }

    /// La placa de `dir` (ya movido) y la distancia angular al borde con la
    /// siguiente.
    fn plate_at(&self, dir: Vec3) -> (&Plate, f32) {
        let (mut first, mut second) = ((f32::MIN, 0usize), (f32::MIN, 0usize));
        for (k, plate) in self.plates.iter().enumerate() {
            let d = plate.center.dot(dir);
            if d > first.0 {
                second = first;
                first = (d, k);
            } else if d > second.0 {
                second = (d, k);
            }
        }
        let (a, b) = (&self.plates[first.1], &self.plates[second.1]);
        let gap = (first.0 - second.0) / (a.center - b.center).length().max(1e-4);
        (a, gap)
    }
}

impl HeightField for PiraField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let heat = smoothstep(-0.25, 0.9, -dir.x);
        // Bordes desgarrados: dos escalas de ruido mueven el punto.
        let warp = Vec3::new(
            n.noise(dir * 2.4 + Vec3::new(3.1, 0.0, -1.2)),
            n.noise(dir * 2.4 + Vec3::new(-4.4, 2.2, 0.7)),
            n.noise(dir * 2.4 + Vec3::new(0.9, -3.7, 5.1)),
        ) * 0.15
            + Vec3::new(
                n.noise(dir * 9.0 + Vec3::new(7.7, 1.1, 0.0)),
                n.noise(dir * 9.0 + Vec3::new(-2.0, 6.6, 3.3)),
                n.noise(dir * 9.0 + Vec3::new(5.5, -5.5, 1.9)),
            ) * 0.03;
        let (plate, gap) = self.plate_at((dir + warp).normalize());
        // El relieve de la placa: crestas, lomas y detalle, con su altura y
        // su inclinacion.
        let ridges = n.ridged(dir * 2.6 + Vec3::new(2.3, -1.1, 4.7), 5);
        let base = n.fbm(dir * 1.6 + Vec3::new(-6.1, 3.3, 0.4), 4);
        let detail = n.fbm(dir * 14.0 + Vec3::new(0.7, 8.2, -3.9), 2);
        let rock = base * 3.0 + ridges * ridges * 6.0 + detail * 0.8 + plate.lift + plate.tilt.dot(dir);
        // El ancho del abismo cambia a lo largo del borde: grietas angostas
        // y abismos anchos, mas anchos del lado de la estrella.
        let wide = smoothstep(-0.35, 0.55, n.noise(dir * 1.3 + Vec3::new(1.7, 9.1, -2.4)));
        let width = 0.035 + 0.11 * wide + 0.025 * heat + 0.06 * smoothstep(0.97, 0.998, dir.dot(self.gorge));
        // La orilla se levanta antes de caer y despues la pared baja casi
        // vertical hasta el magma.
        let lip = smoothstep(width * 2.4, width * 1.05, gap) * smoothstep(width * 0.82, width * 1.05, gap) * 1.6;
        let fall = smoothstep(width, width * 0.72, gap);
        let floor = -PIRA_CORE_DEPTH + 0.9 * n.noise(dir * 7.0 + Vec3::new(4.0, 4.0, -8.0));
        let height = (rock + lip) * (1.0 - fall) + floor * fall;
        let molten = smoothstep(0.82, 0.97, fall);
        TerrainSample {
            height,
            attrs: Vec3::new(molten, heat, fall),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        // El fondo con sus olas; arriba, el relieve (3 + 6 + 0.8), la placa
        // mas alta (4), su inclinacion (12) y el labio. (Frente al lugar el
        // abismo es mas ancho: ahi se ve el magma desde la orilla.)
        (-PIRA_CORE_DEPTH - 1.0, 3.0 + 6.0 + 0.8 + 4.0 + 12.0 + 1.6)
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
            (Box::new(ArgosField::new(7)), Box::new(ArgosField::new(7))),
            (Box::new(PiraField::new(7)), Box::new(PiraField::new(7))),
            (
                Box::new(JardinField::new(7, 1)),
                Box::new(JardinField::new(7, 1)),
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

    /// Pira esta partida: hay placas y abismos con magma, y el lugar queda
    /// sobre una placa con un abismo adelante, hacia la estrella.
    #[test]
    fn pira_is_broken_into_plates_and_its_site_overlooks_an_abyss() {
        for seed in [7, crate::config::NOISE_SEED ^ 0x9124] {
            let field = PiraField::new(seed);
            let site = PIRA_SITE.normalize();
            let toward = (Vec3::new(-1.0, 0.0, 0.0) + site * site.x).normalize();
            assert!(field.sample(site).attrs.z < 0.01, "semilla {seed}: el lugar cae en un abismo");
            let mut ahead = (1..70).map(|k| k as f32 / 110.0).map(|a| site * a.cos() + toward * a.sin());
            assert!(ahead.any(|d| field.sample(d).attrs.x > 0.9), "semilla {seed}: no hay abismo adelante");
            let molten = directions().filter(|&d| field.sample(d).attrs.x > 0.5).count();
            assert!(molten > 4000 / 25 && molten < 4000 / 3, "semilla {seed}: {molten} de 4000 en el magma");
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
