//! El Volcan de Fuego (la version chapina de Pira): un mundo de ceniza negra
//! con volcanes que hacen erupcion y, al pie del Fuego, en un valle verde,
//! Antigua Guatemala con el Arco de Santa Catalina. Como Pira, le da siempre
//! la misma cara (-X) a la estrella: Antigua vive en una tarde eterna, con la
//! estrella baja a la espalda, y el Fuego se levanta en el terminador.
//!
//! # El relieve
//! - **Volcanes**: conos de perfil concavo, `H (1 - s)^1.7` (`s` es la
//!   distancia al centro entre el radio de la base): empinados arriba y
//!   tendidos abajo, como un estratovolcan. El crater corta la cumbre con un
//!   paraboloide invertido y un minimo suave redondea el borde. Donde dos
//!   conos se tocan (el Fuego y el Acatenango) se suman como una norma 4,
//!   `(a^4 + b^4)^(1/4)`: casi el maximo, sin quiebre, y entre los dos queda
//!   el collado de La Horqueta.
//! - **Barrancas**: canales que bajan de la cumbre, cada uno en un angulo
//!   alrededor del cono y con meandros (senos de la distancia al centro). El
//!   perfil es una V, mas honda a media ladera. Las coladas del Fuego bajan
//!   por algunas de ellas: la misma funcion que talla la barranca le dice al
//!   shader donde esta la lava.
//! - **Surcos**: entre las barrancas, muchos canales angostos: ruido en un
//!   circulo alrededor del eje del cono (periodico en el angulo) que
//!   serpentea al bajar; el fondo va donde el ruido cruza por cero. Dejan
//!   aristas en la silueta. El shader agrega otros mas finos solo en la
//!   normal.
//! - **Lomas**: fbm con crestas viejas donde un ruido grande lo deja; en los
//!   valles bajos crece lo verde.
//! - **Antigua**: en un rectangulo redondeado alrededor del lugar el relieve
//!   se aplana a la altura que tiene ahi.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::math::blackbody::planckian_rgb;
use crate::math::{DVec3, Perlin, Vec3, fast_atan2, hash_u32, smoothstep};
use crate::scene::Mesh;
use crate::scene::builder::{MeshBuilder, perpendiculars};
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter, at_site, clear_site, upright_rotation};
use crate::shaders::{AtmosphereParams, Fragment, PropStyle, Uniforms, detail, limb_haze, pbr};
use crate::terrain::{HeightField, TerrainSample};

/// Radio para el que esta pensado el relieve (el de Pira): pasa angulos a
/// unidades del mundo.
const RADIUS: f32 = 110.0;

/// Antigua (el lugar, en el espacio del planeta) y hacia donde se mira al
/// llegar: calle arriba, hacia el Fuego. La estrella queda atras a la
/// izquierda, a 30 grados sobre el horizonte.
const ANTIGUA: Vec3 = Vec3::new(-0.4997, 0.4807, -0.7206);
const UPHILL: Vec3 = Vec3::new(0.7203, -0.2315, -0.6539);

pub fn spec() -> Spec {
    Spec {
        label: "Volcan de Fuego",
        material: Chapin::Fuego,
        // Aire delgado con algo de ceniza: azul de Rayleigh arriba y un poco
        // de Mie gris junto al suelo.
        atmosphere: Some(AtmosphereParams {
            thickness: 22.0,
            scale_height: 4.5,
            rayleigh: Vec3::new(0.0025, 0.005, 0.0105),
            mie: 0.0025,
            mie_g: 0.75,
            intensity: 1.3,
        }),
        rings: None,
        motes: Some((Motes::Ash, 120.0)),
    }
}

// Los volcanes -----------------------------------------------------------------

/// Barrancas de cada volcan.
const CHANNELS: usize = 7;

/// Un volcan. Las medidas van en unidades del mundo.
struct Volcano {
    /// Centro, unitario, en el espacio del planeta.
    center: Vec3,
    /// Altura de la cumbre y radio de la base.
    height: f32,
    base: f32,
    /// Radio del crater.
    crater: f32,
    /// Cuanto fuego tiene (0 dormido).
    fire: f32,
    /// Angulo de la primera barranca alrededor del eje, y hondura de todas.
    spin: f32,
    gully: f32,
    /// Hasta donde baja la lava en cada barranca, en radios de la base (0:
    /// seca).
    flows: [f32; CHANNELS],
}

const VOLCANOES: [Volcano; 8] = [
    // El Fuego: la primera barranca apunta a Antigua; tres coladas bajan
    // por la cara que se ve desde el pueblo.
    Volcano {
        center: Vec3::new(-0.1453, 0.3363, -0.9305),
        height: 38.5,
        base: 49.3,
        crater: 3.2,
        fire: 1.0,
        spin: 0.75,
        gully: 4.0,
        flows: [0.78, 0.55, 0.0, 0.0, 0.0, 0.0, 0.9],
    },
    // El Acatenango, su gemelo dormido, al lado.
    Volcano {
        center: Vec3::new(-0.0267, 0.5277, -0.849),
        height: 33.0,
        base: 51.5,
        crater: 2.6,
        fire: 0.0,
        spin: 0.4,
        gully: 4.2,
        flows: [0.0; CHANNELS],
    },
    // El de Agua: el cono parejo del otro lado del valle.
    Volcano {
        center: Vec3::new(-0.9849, -0.1057, -0.1374),
        height: 26.4,
        base: 51.5,
        crater: 4.5,
        fire: 0.0,
        spin: 1.9,
        gully: 2.0,
        flows: [0.0; CHANNELS],
    },
    // Pacaya, chico y activo, del lado de la noche.
    Volcano {
        center: Vec3::new(0.6221, -0.4013, -0.6723),
        height: 13.2,
        base: 29.1,
        crater: 2.4,
        fire: 0.7,
        spin: 0.3,
        gully: 1.4,
        flows: [0.95, 0.0, 0.0, 0.7, 0.0, 0.0, 0.0],
    },
    // Santa Maria y, en su falda, el domo del Santiaguito.
    Volcano {
        center: Vec3::new(-0.4509, -0.7816, -0.4309),
        height: 22.4,
        base: 38.1,
        crater: 2.2,
        fire: 0.0,
        spin: 2.2,
        gully: 2.4,
        flows: [0.0; CHANNELS],
    },
    Volcano {
        center: Vec3::new(-0.2646, -0.8445, -0.4656),
        height: 7.3,
        base: 16.8,
        crater: 2.0,
        fire: 0.5,
        spin: 1.5,
        gully: 0.6,
        flows: [0.0, 0.0, 0.9, 0.0, 0.0, 0.0, 0.0],
    },
    // Tajumulco y Tacana, lejos.
    Volcano {
        center: Vec3::new(-0.5507, 0.1502, 0.8211),
        height: 23.8,
        base: 44.8,
        crater: 2.4,
        fire: 0.0,
        spin: 0.9,
        gully: 2.8,
        flows: [0.0; CHANNELS],
    },
    Volcano {
        center: Vec3::new(0.5529, 0.5529, 0.6233),
        height: 19.8,
        base: 38.1,
        crater: 2.2,
        fire: 0.0,
        spin: 2.7,
        gully: 2.4,
        flows: [0.0; CHANNELS],
    },
];

/// Exponente del perfil de los conos.
const PROFILE: f32 = 1.7;

/// Distancia al centro de un volcan (unidades del mundo, por la cuerda) y
/// angulo alrededor de su eje.
#[inline]
fn polar(v: &Volcano, dir: Vec3) -> (f32, f32) {
    let delta = dir - v.center;
    let (e1, e2) = perpendiculars(v.center);
    (delta.length() * RADIUS, fast_atan2(delta.dot(e2), delta.dot(e1)))
}

/// Angulo de la barranca `k` a la distancia `d` del centro: repartidas
/// alrededor del cono, con meandros que se abren ladera abajo.
#[inline]
fn channel_angle(v: &Volcano, k: usize, d: f32) -> f32 {
    let k = k as f32;
    let fan = v.spin + k * (TAU / CHANNELS as f32) + 0.3 * (k * 2.4 + v.spin * 3.0).sin();
    let meander = 1.3 * (0.21 * d + 2.1 * k).sin() + 0.5 * (0.53 * d + 1.3 * k + v.spin).sin();
    fan + meander * smoothstep(0.0, 8.0, d) / d.max(1.0)
}

/// La barranca mas cercana: distancia de lado (unidades del mundo) y cual es.
#[inline]
fn nearest_channel(v: &Volcano, d: f32, phi: f32) -> (f32, usize) {
    let mut best = (f32::MAX, 0);
    for k in 0..CHANNELS {
        let delta = (phi - channel_angle(v, k, d) + PI).rem_euclid(TAU) - PI;
        let side = (delta * d).abs();
        if side < best.0 {
            best = (side, k);
        }
    }
    best
}

/// Medio ancho de una colada a la distancia `d` del centro: angosta junto
/// al crater, mas ancha abajo.
#[inline]
fn flow_width(d: f32) -> f32 {
    0.3 + 0.018 * d
}

/// Cuanta lava hay en una barranca a `side` de su centro y a la distancia
/// `d` del volcan (0 a 1, con un borde de `aa`), y en que fraccion de la
/// colada va (0 en el crater, 1 en el frente).
#[inline]
fn flow(v: &Volcano, k: usize, d: f32, side: f32, aa: f32) -> (f32, f32) {
    let reach = v.flows[k] * v.base;
    if reach <= 0.0 || d > reach + aa {
        return (0.0, 1.0);
    }
    let w = flow_width(d);
    // El frente es un lobulo redondo.
    let front = (reach - d).min(w);
    let across = smoothstep(w + aa, w - aa, side + (w - front).max(0.0));
    (across, (d / reach).min(1.0))
}

/// Cuantos surcos hay alrededor de un cono: frecuencia del circulo de ruido
/// en el relieve y, mas finos, en el shader.
const RIDGES: f32 = 3.0;
const RILLS: f32 = 9.0;

/// Los surcos de una ladera: ruido en un circulo alrededor del eje (asi es
/// periodico en el angulo) que serpentea al bajar (`wander`). Vale 0 en el
/// fondo de un surco.
#[inline]
fn furrow(noise: &Perlin, v: &Volcano, d: f32, phi: f32, freq: f32, wander: f32) -> f32 {
    let ring = Vec3::new(phi.cos() * freq, phi.sin() * freq, d * wander + v.spin * 5.0);
    noise.noise(ring).abs()
}

/// Mas suave que `a.min(b)`: el borde del crater queda redondo.
#[inline]
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// Un volcan a la distancia `d` de su centro y en el angulo `phi`: altura
/// y lava (0 a 1).
fn cone(v: &Volcano, d: f32, phi: f32, noise: &Perlin) -> (f32, f32) {
    let s = d / v.base;
    let slope = v.height * (1.0 - s).max(0.0).powf(PROFILE);
    // El crater: un paraboloide invertido que corta la cumbre.
    let rim = v.height * (1.0 - v.crater / v.base).powf(PROFILE);
    let depth = v.crater * 1.1;
    let bowl = rim - depth + depth * (d / v.crater).powi(2);
    let mut h = smooth_min(slope, bowl, 0.8);
    let mut lava = 0.0f32;
    if v.fire > 0.0 && d < v.crater {
        // Lago de lava en el fondo del crater: plano.
        let lake = rim - depth * 0.7;
        if h < lake {
            h = lake;
            lava = 1.0;
        }
    }
    // Barrancas: una V, honda a media ladera.
    let (side, k) = nearest_channel(v, d, phi);
    let width = 2.0 + 0.1 * d;
    let carve = v.gully * smoothstep(v.crater, v.crater * 3.5, d) * smoothstep(1.0, 0.55, s);
    let groove = (1.0 - (side / width).min(1.0)).powf(1.4);
    h -= carve * groove;
    // Surcos entre las barrancas: angostos, mas hondos a media ladera.
    let scour = 1.0 - smoothstep(0.0, 0.3, furrow(noise, v, d, phi, RIDGES, 0.04));
    h -= scour * v.height * 0.045 * smoothstep(v.crater, v.crater * 2.5, d) * smoothstep(1.0, 0.6, s);
    let (molten, _) = flow(v, k, d, side, 0.2);
    (h.max(0.0), lava.max(molten))
}

// El relieve ------------------------------------------------------------------

/// Donde se aplana el valle de Antigua (coordenadas del lugar, unidades del
/// mundo): medio ancho, desde y hasta donde calle arriba, y lo que tarda en
/// juntarse con el relieve.
const TOWN: (f32, f32, f32, f32) = (36.0, -66.0, 4.0, 20.0);

/// Ejes de Antigua: vertical, calle arriba y a la derecha.
#[inline]
fn town_axes() -> (Vec3, Vec3, Vec3) {
    (ANTIGUA, UPHILL, UPHILL.cross(ANTIGUA))
}

/// Coordenadas de `dir` en el plano de Antigua: `x` a la derecha y `z`
/// calle arriba, en unidades del mundo. Es la inversa de `at_site`, que
/// pone cada objeto en `normalize(up + (right x + forward z) / radio)`: lo
/// pintado y los objetos quedan alineados. `None` lejos del pueblo.
#[inline]
fn street(dir: Vec3) -> Option<(f32, f32)> {
    let (up, forward, right) = town_axes();
    let h = dir.dot(up);
    if h < 0.5 {
        return None;
    }
    let k = RADIUS / h;
    Some((dir.dot(right) * k, dir.dot(forward) * k))
}

/// Cuanto es pueblo un punto (1 adentro, 0 afuera).
#[inline]
fn town_weight(dir: Vec3) -> f32 {
    let Some((x, z)) = street(dir) else { return 0.0 };
    let (half, near, far, blend) = TOWN;
    let dx = (x.abs() - half).max(0.0);
    let dz = (near - z).max(z - far).max(0.0);
    smoothstep(blend, 0.0, dx.hypot(dz))
}

/// El relieve del Volcan de Fuego.
///
/// Atributos: `x` lava, `y` ceniza (negra en el cono del Fuego, gris
/// alrededor), `z` verde (valles y faldas de los volcanes dormidos).
pub struct FuegoField {
    noise: Perlin,
    /// Altura del valle de Antigua.
    valley: f32,
}

/// Lo mas bajo y lo mas alto que puede quedar el relieve.
const LOWEST: f32 = -4.0;
const HIGHEST: f32 = 42.0;

impl FuegoField {
    pub fn new(seed: u32) -> Self {
        let mut field = Self {
            noise: Perlin::new(seed),
            valley: 0.0,
        };
        field.valley = field.relief(ANTIGUA).height;
        field
    }

    /// El relieve sin el valle de Antigua.
    fn relief(&self, dir: Vec3) -> TerrainSample {
        let n = &self.noise;
        let warp = Vec3::new(
            n.noise(dir * 2.0 + Vec3::new(4.1, 0.0, 0.0)),
            n.noise(dir * 2.0 + Vec3::new(0.0, 9.3, 0.0)),
            n.noise(dir * 2.0 + Vec3::new(0.0, 0.0, 6.7)),
        ) * 0.12;
        let q = dir + warp;
        let hills = n.fbm(q * 2.6 + Vec3::new(-3.1, 5.2, 1.7), 4);
        let highland = smoothstep(-0.05, 0.3, n.fbm(q * 1.3 + Vec3::new(2.7, -1.3, 5.1), 3));
        let ridges = n.ridged(q * 4.2 + Vec3::new(-4.4, 2.2, 0.9), 4);
        let rubble = n.fbm(dir * 24.0 + Vec3::new(0.7, 8.2, -3.9), 2);
        let land = 2.4 * hills + 8.0 * ridges * ridges * highland + 0.5 * rubble;
        let valley = smoothstep(0.15, -0.3, hills) * (1.0 - 0.7 * highland);
        // Los conos se suman como una norma 4: casi el maximo, sin quiebre.
        let (mut peaks, mut cover, mut lava, mut ash, mut forest) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for v in &VOLCANOES {
            let (d, phi) = polar(v, dir);
            if d >= v.base {
                continue;
            }
            let (h, molten) = cone(v, d, phi, n);
            peaks += h.powi(4);
            let s = d / v.base;
            cover = cover.max(smoothstep(1.0, 0.55, s));
            lava = lava.max(molten);
            // Ceniza negra arriba en los activos; bosque en las faldas, mas
            // arriba en los dormidos.
            ash = ash.max(v.fire * smoothstep(0.85, 0.5, s));
            let tree_line = 0.25 + 0.35 * v.fire + 0.2 * n.noise(dir * 9.0 + Vec3::new(1.7, -2.2, 0.4));
            forest = forest.max(smoothstep(1.0, 0.85, s) * smoothstep(tree_line, tree_line + 0.15, s));
        }
        let height = land * (1.0 - cover) + peaks.sqrt().sqrt() + 0.3 * rubble * cover;
        let green = (valley * (1.0 - cover) + forest).min(1.0) * (1.0 - ash);
        TerrainSample {
            height: height.clamp(LOWEST, HIGHEST),
            attrs: Vec3::new(lava, ash, green),
        }
    }
}

impl HeightField for FuegoField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let mut s = self.relief(dir);
        let town = town_weight(dir);
        if town > 0.0 {
            // El valle: plano, sin lava ni ceniza, con patios verdes.
            let swell = self.noise.fbm(dir * 9.0, 2) * 0.3 * (1.0 - town);
            s.height += (self.valley + swell - s.height) * town;
            s.attrs = s.attrs.lerp(Vec3::new(0.0, 0.0, 1.0), town);
        }
        s
    }

    fn height_range(&self) -> (f32, f32) {
        (LOWEST, HIGHEST)
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(FuegoField::new(seed))
}

// El shader del suelo ---------------------------------------------------------

const BASALT: Vec3 = Vec3::new(0.014, 0.0125, 0.0115);
/// Escoria oxidada de las cumbres.
const SCORIA: Vec3 = Vec3::new(0.032, 0.015, 0.009);
const SOIL: Vec3 = Vec3::new(0.032, 0.021, 0.014);
const FOREST: Vec3 = Vec3::new(0.008, 0.022, 0.008);
/// Parcelas de los valles: milpa, cafetal, potrero, rastrojo y tierra arada.
const CROPS: [Vec3; 5] = [
    Vec3::new(0.022, 0.048, 0.012),
    Vec3::new(0.01, 0.027, 0.01),
    Vec3::new(0.034, 0.052, 0.016),
    Vec3::new(0.05, 0.042, 0.018),
    Vec3::new(0.036, 0.024, 0.014),
];
/// Bruma del borde del planeta.
const HAZE: Vec3 = Vec3::new(0.32, 0.36, 0.45);
/// Las laderas de los volcanes: ceniza oscura de la cumbre del Fuego y la
/// gris que rellena surcos y barrancas, pedregal de las cumbres dormidas,
/// zacate, y el verde de las copas del bosque (en sombra y al sol).
const ASH_DARK: Vec3 = Vec3::new(0.0048, 0.0045, 0.0043);
const ASH_GREY: Vec3 = Vec3::new(0.013, 0.0125, 0.012);
const SCREE: Vec3 = Vec3::new(0.016, 0.012, 0.009);
const SCRUB: Vec3 = Vec3::new(0.017, 0.016, 0.008);
const CANOPY: Vec3 = Vec3::new(0.0045, 0.016, 0.004);
const CANOPY_LIGHT: Vec3 = Vec3::new(0.011, 0.028, 0.006);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let footprint = f.pos.length() * u.pixel_angle;
    let dir = f.local.normalize();
    let town = town_weight(dir);
    let mut color = if town < 1.0 { land(f, u, dir, footprint) } else { Vec3::ZERO };
    if town > 0.0 {
        color = color.lerp(antigua(f, u, dir, footprint), town);
    }
    // La bruma del borde es para verlo desde la orbita: de cerca lavaria la
    // ceniza negra.
    let orbit = smoothstep(120.0, 260.0, f.pos.length());
    color + limb_haze(f, HAZE, 0.2, f.normal.dot(f.light_dir(u))) * orbit
}

/// Mascara de una linea: `edge` es la distancia a su centro, `width` su
/// medio ancho y `aa` lo que mide un pixel. Si es mas delgada que un pixel se
/// ensancha y baja su pico (como las grietas de Pira): de lejos sigue siendo
/// una linea y no parpadea.
#[inline]
fn line(edge: f32, width: f32, aa: f32) -> f32 {
    smoothstep(width + aa, 0.0, edge) * (width / (width + aa)).min(1.0)
}

/// El volcan que manda en `dir` (el que lo tiene mas adentro de su base),
/// la distancia a su centro y el angulo alrededor.
fn host(dir: Vec3) -> Option<(&'static Volcano, f32, f32)> {
    let mut best: Option<&Volcano> = None;
    let mut inside = 1.0;
    for v in &VOLCANOES {
        let s = (dir - v.center).length() * RADIUS / v.base;
        if s < inside {
            inside = s;
            best = Some(v);
        }
    }
    best.map(|v| {
        let (d, phi) = polar(v, dir);
        (v, d, phi)
    })
}

/// Parcelas: las celdas de una grilla 3D (torcida con ruido) cortadas por
/// la esfera, cada una con su cultivo y con setos oscuros en los bordes. La
/// distancia a cada pared se mide sobre la superficie: se divide entre lo
/// que el eje de esa pared cruza la superficie, si no las paredes casi
/// tangentes darian setos enormes.
fn crops(p: Vec3, dir: Vec3, footprint: f32, n: &Perlin) -> Vec3 {
    const SIZE: f32 = 9.0;
    let warp = Vec3::new(n.noise(p * 0.03), n.noise(p * 0.03 + Vec3::splat(5.3)), n.noise(p * 0.03 + Vec3::splat(9.1)));
    let q = p / SIZE + warp * 0.5;
    let cell = q.floor();
    let fr = q - cell;
    let wall = |f: f32, axis: f32| f.min(1.0 - f) * SIZE / (1.0 - axis * axis).max(0.04).sqrt();
    let edge = wall(fr.x, dir.x).min(wall(fr.y, dir.y)).min(wall(fr.z, dir.z));
    let id = hash_u32(
        (cell.x as i32 as u32).wrapping_mul(0x8DA6_B343)
            ^ (cell.y as i32 as u32).wrapping_mul(0xD816_3841)
            ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F),
    );
    let crop = CROPS[(id % CROPS.len() as u32) as usize];
    // Surcos dentro de la parcela, solo de cerca.
    let rows = ((p.dot(Vec3::new(0.6, 0.5, 0.62)) + (id & 7) as f32) * 2.2).sin() * smoothstep(0.25, 0.05, footprint);
    (crop * (1.0 + 0.15 * rows)).lerp(FOREST, line(edge, 0.3, footprint) * 0.85)
}

/// Roca, ceniza y lo verde, con la lava encima.
fn land(f: &Fragment, u: &Uniforms, dir: Vec3, footprint: f32) -> Vec3 {
    let green = f.color.z;
    let n = u.noise;
    let p = f.local;
    let steep = smoothstep(0.3, 0.6, f.uv.y);
    let rock = detail::rock(f, u, 0.12, footprint, 0.8);
    let grain = 0.6 + 0.8 * rock.color.luminance() / 0.16;
    // Lomas: roca vieja arriba y tierra abajo; lo verde de los valles son
    // parcelas.
    let mut albedo = BASALT.lerp(SOIL, smoothstep(0.55, 0.3, f.uv.x) * 0.8) * grain;
    albedo = albedo.lerp(crops(p, dir, footprint, n), smoothstep(0.25, 0.6, green) * (1.0 - steep));
    let mut normal = rock.normal;
    if let Some((v, d, phi)) = host(dir) {
        // Los conos: su propia ladera, con la textura de roca solo en lo
        // empinado.
        let (slope, slope_normal) = flank(f, u, v, d, phi, footprint);
        let on_cone = smoothstep(1.0, 0.75, d / v.base);
        albedo = albedo.lerp(slope, on_cone);
        let rocky = (rock.normal - f.normal) * (0.15 + 0.6 * steep);
        normal = normal.lerp(slope_normal + rocky, on_cone).normalize();
    }
    let (glow, molten) = fire(f, u, dir, footprint);
    let lit = pbr::shade(f, u, albedo * (1.0 - molten), 0.92, 0.0, normal, rock.ao);
    lit + glow
}

/// La ladera de un volcan: arriba la cumbre pelona (ceniza oscura en los
/// activos, pedregal en los dormidos) con surcos rellenos de ceniza gris,
/// luego zacate y abajo el bosque. Las barrancas grandes del Fuego cortan
/// el bosque como franjas de ceniza gris, como se ven desde Antigua; en los
/// dormidos son solo sombra. Devuelve el albedo y la normal, inclinada por
/// los surcos finos.
fn flank(f: &Fragment, u: &Uniforms, v: &Volcano, d: f32, phi: f32, footprint: f32) -> (Vec3, Vec3) {
    let n = u.noise;
    let s = d / v.base;
    let active = v.fire > 0.0;
    // Hasta donde llega lo pelon y desde donde hay bosque, torcidos con
    // ruido: el bosque sube mas en los dormidos.
    let around = Vec3::new(phi.cos(), phi.sin(), v.spin);
    let wobble = 0.06 * n.noise(around * 3.0) + 0.03 * n.noise(f.local * 0.35);
    let bare_line = if active { 0.36 } else { 0.22 } + wobble;
    let bare = smoothstep(bare_line + 0.05, bare_line - 0.03, s);
    let woods = smoothstep(bare_line + 0.08, bare_line + 0.2, s);
    // Vetas a lo largo de la pendiente: ruido en el angulo, estirado hacia
    // abajo.
    let streak = n.noise(Vec3::new(phi.cos() * 22.0, phi.sin() * 22.0, d * 0.05)) * 0.5 + 0.5;
    let broad = n.noise(Vec3::new(phi.cos() * 5.0, phi.sin() * 5.0, d * 0.03 + 2.0)) * 0.5 + 0.5;
    // Surcos finos, solo en el sombreado: el fondo con ceniza gris y la
    // pendiente de lado inclina la normal. Se apagan cuando miden pocos
    // pixeles y bajo el bosque.
    let spacing = d.max(1.0) / RILLS;
    let fine = smoothstep(3.0, 8.0, spacing / footprint.max(1e-4)) * (1.0 - woods);
    let (mut rill, mut tilt) = (0.0, 0.0);
    if fine > 0.0 {
        let profile = |phi: f32| smoothstep(0.0, 0.3, furrow(n, v, d, phi, RILLS, 0.09));
        let e = 0.004;
        rill = (1.0 - profile(phi)) * fine;
        let depth = 0.25 + 0.012 * d;
        tilt = (profile(phi + e) - profile(phi - e)) / (2.0 * e * d.max(1.0)) * depth * fine;
    }
    // El fondo de las barrancas grandes.
    let (side, _) = nearest_channel(v, d, phi);
    let width = 2.0 + 0.1 * d;
    let bed = smoothstep(width * 0.5, width * 0.12, side) * smoothstep(v.crater * 1.5, v.crater * 4.0, d);

    let summit = if active {
        ASH_DARK.lerp(ASH_GREY, (0.25 * streak + 0.7 * rill + 0.3 * bed).min(1.0)) * (0.8 + 0.35 * broad)
    } else {
        SCREE.lerp(ASH_GREY, 0.4 * rill + 0.3 * streak) * (0.75 + 0.4 * broad)
    };
    // Escoria rojiza alrededor del crater.
    let summit = summit.lerp(SCORIA * (0.6 + 0.4 * streak), smoothstep(0.16, 0.06, s) * 0.6);
    let scrub = SCRUB.lerp(CANOPY, 0.5 * broad) * (0.8 + 0.4 * streak);
    let mut albedo = summit.lerp(scrub, 1.0 - bare).lerp(canopy(f, n, footprint, broad), woods);
    if active {
        albedo = albedo.lerp(ASH_GREY * (0.85 + 0.3 * streak), bed * 0.85);
    } else {
        albedo *= 1.0 - 0.35 * bed;
    }
    // La normal: de lado, alrededor del eje del cono.
    let (e1, e2) = perpendiculars(v.center);
    let across = e2 * phi.cos() - e1 * phi.sin();
    let normal = (f.normal - f.object_to_world.mul_vec(across) * tilt).normalize();
    (albedo, normal)
}

/// Copas de arboles: celdas de Worley, oscuras en el borde, con verdes
/// distintos por manchas. De lejos, el promedio.
fn canopy(f: &Fragment, n: &Perlin, footprint: f32, patch: f32) -> Vec3 {
    const CROWN: f32 = 1.8;
    let average = CANOPY.lerp(CANOPY_LIGHT, 0.2 + 0.4 * patch);
    let near = smoothstep(CROWN * 0.1, CROWN * 0.04, footprint);
    if near <= 0.0 {
        return average;
    }
    let (f1, f2) = n.cells(f.local / CROWN);
    let tone = n.noise(f.local * 0.9) * 0.5 + 0.5;
    let leaf = CANOPY.lerp(CANOPY_LIGHT, (0.1 + 0.7 * tone + 0.3 * (patch - 0.5)).clamp(0.0, 1.0));
    let gap = 1.0 - 0.3 * smoothstep(0.2, 0.0, f2 - f1) - 0.2 * f1.min(1.0);
    average.lerp(leaf * gap, near)
}

/// Una explosion del Fuego: cada `BURST` segundos mas o menos (con azar),
/// de fuerza distinta. Devuelve hace cuanto fue la ultima y su fuerza.
const BURST: f32 = 4.5;

#[inline]
fn last_burst(t: f32) -> (f32, f32) {
    let k = (t / BURST).floor();
    let at = |k: f32| (k + 0.7 * unit(hash_u32(k as i32 as u32 ^ 0xB0B))) * BURST;
    let k = if at(k) > t { k - 1.0 } else { k };
    (t - at(k), 0.4 + 0.6 * unit(hash_u32(k as i32 as u32 ^ 0x5EED)))
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// La luz de la lava: el lago del crater, las coladas en las barrancas y las
/// bombas incandescentes de cada explosion. Devuelve la luz que emite y
/// cuanto del suelo es lava.
fn fire(f: &Fragment, u: &Uniforms, dir: Vec3, footprint: f32) -> (Vec3, f32) {
    let t = u.time;
    let n = u.noise;
    let mut glow = Vec3::ZERO;
    let mut molten = 0.0f32;
    for v in VOLCANOES.iter().filter(|v| v.fire > 0.0) {
        let (d, phi) = polar(v, dir);
        if d > v.base {
            continue;
        }
        let (since, strength) = last_burst(t + v.spin * 3.0);
        let flash = strength * (-since * 2.2).exp() * v.fire;
        // El latido de la estrella baja por las coladas.
        let pulse = 0.8 + 0.4 * u.beat_at(d * 0.04);
        // El crater: un lago de lava que hierve.
        if d < v.crater * 1.6 {
            let churn = n.fbm(f.local * 0.9 + Vec3::new(0.0, t * 0.6, t * 0.4), 3) * 0.5 + 0.5;
            let lake = smoothstep(v.crater * 0.8, v.crater * 0.6, d);
            let temperature = 1250.0 + 300.0 * churn + 400.0 * flash;
            glow += planckian_rgb(temperature) * (lake * (2.5 + 3.0 * churn + 10.0 * flash) * pulse);
            molten = molten.max(lake);
            // Las paredes del crater se ponen al rojo.
            let wall = smoothstep(v.crater * 1.6, v.crater * 0.8, d) * (1.0 - lake);
            glow += planckian_rgb(1000.0 + 300.0 * flash) * (wall * (0.4 + 3.0 * flash));
        }
        // Las coladas: balsas de costra que bajan con la lava y, entre ellas,
        // la lava al rojo; mas fria (y mas costra) en las orillas y el frente.
        let (side, k) = nearest_channel(v, d, phi);
        let aa = footprint.max(0.02);
        let (cover, along) = flow(v, k, d, side, aa);
        if cover > 0.0 {
            let w = flow_width(d);
            let x = (side / w).min(1.0);
            let downhill = d - t * (1.4 - 0.8 * along);
            let rafts = n.noise(Vec3::new(side * 1.8, downhill * 0.5, k as f32 * 7.1));
            let fine = smoothstep(0.6, 0.15, footprint);
            let crust = (smoothstep(0.0, 0.4, rafts) * fine * (0.4 + 0.6 * along) + x * x * 0.7).min(1.0);
            let temperature = 1380.0 - 430.0 * along - 300.0 * crust;
            // El brillo baja con la cuarta potencia de la temperatura; de
            // lejos la colada es mas delgada que un pixel y se reparte en el.
            let bright = (temperature / 1380.0).powi(4) * (1.0 - 0.8 * crust);
            let amount = cover * (w / aa.max(w));
            let surge = 1.0 + 1.5 * flash * smoothstep(18.0, 4.0, d);
            glow += planckian_rgb(temperature) * (amount * bright * 7.0 * pulse * surge);
            molten = molten.max(cover);
        }
        // Bombas: puntos incandescentes que ruedan ladera abajo despues de
        // cada explosion y se enfrian.
        let reach = v.crater + 14.0;
        if d < reach && since < 4.0 && footprint < 0.4 {
            let cool = (-since * 0.9).exp() * strength;
            let roll = d - v.crater - since * 3.5;
            let (f1, _) = n.cells(Vec3::new(phi.cos() * 6.0, phi.sin() * 6.0, roll * 0.8 + since.floor() * 17.0));
            let size = 0.12 + footprint;
            let speck = smoothstep(size, size * 0.4, f1) * (0.12 / size).min(1.0);
            glow += planckian_rgb(1100.0 + 300.0 * cool) * (speck * cool * 6.0 * smoothstep(reach, v.crater, d));
        }
        // La lava alumbra la ceniza de alrededor.
        let near = smoothstep(v.base * 0.5, 0.0, d) * (0.06 + 0.5 * flash);
        glow += planckian_rgb(1000.0) * (near * 0.05 * pulse);
    }
    (glow * u.gains.pira, molten)
}

// Antigua --------------------------------------------------------------------

/// Medio ancho de la calle del Arco, de sus banquetas y de las demas calles.
const STREET: f32 = 2.8;
const SIDEWALK: f32 = 0.9;
const LANE: f32 = 2.4;
/// Las otras avenidas (a lo largo de la calle del Arco) y las calles que la
/// cruzan, por su centro.
const AVENUES: [f32; 2] = [-24.0, 24.0];
const CALLES: [f32; 2] = [-36.0, -14.0];
/// Donde esta el Arco, calle arriba.
const ARCH_Z: f32 = 2.0;
/// La alfombra de aserrin: medio ancho y de donde a donde va.
const CARPET: (f32, f32, f32) = (1.7, -30.0, -1.0);

const COBBLE: Vec3 = Vec3::new(0.13, 0.115, 0.1);
const FLAGSTONE: Vec3 = Vec3::new(0.24, 0.22, 0.2);
const PATIO: Vec3 = Vec3::new(0.08, 0.11, 0.04);

/// El suelo de Antigua: calles empedradas, banquetas, patios y la alfombra.
fn antigua(f: &Fragment, u: &Uniforms, dir: Vec3, footprint: f32) -> Vec3 {
    let Some((x, z)) = street(dir) else { return Vec3::ZERO };
    let n = u.noise;
    // Visto de lado, un pixel cubre mas suelo.
    let ndv = f.normal.dot(f.view_dir()).abs().max(0.2);
    let aa = (footprint / ndv).max(0.004);
    // Distancia (con signo) al borde de cada cosa: negativa adentro.
    let mut road = x.abs() - STREET;
    let mut walk = x.abs() - STREET - SIDEWALK;
    for c in AVENUES {
        road = road.min((x - c).abs() - LANE);
        walk = walk.min((x - c).abs() - LANE - 1.0);
    }
    for c in CALLES {
        road = road.min((z - c).abs() - LANE);
        walk = walk.min((z - c).abs() - LANE - 1.0);
    }
    let on_road = smoothstep(aa, -aa, road);
    let on_walk = smoothstep(aa, -aa, walk) * (1.0 - on_road);
    let mut albedo = PATIO * (0.8 + 0.4 * (n.fbm(f.local * 0.3, 2) * 0.5 + 0.5));
    let mut rough = 0.95;
    if on_walk > 0.0 {
        // Banqueta de lajas.
        let (gx, gz) = ((x * 0.8).fract(), (z * 1.1).fract());
        let joint = smoothstep(0.06 + aa, 0.0, gx.min(1.0 - gx).min(gz.min(1.0 - gz)) * 1.0);
        albedo = albedo.lerp(FLAGSTONE * (1.0 - 0.5 * joint), on_walk);
    }
    if on_road > 0.0 {
        // Piedras redondas: Worley; la junta es donde dos celdas se tocan.
        let (f1, f2) = n.cells(Vec3::new(x * 2.1, z * 2.1, 0.5));
        let fine = smoothstep(0.25, 0.08, aa * 2.1);
        let joint = smoothstep(0.14, 0.03, f2 - f1) * fine;
        let stone = COBBLE * (0.75 + 0.5 * (1.0 - f1) * fine + 0.25 * n.noise(Vec3::new(x * 0.7, z * 0.7, 3.0)));
        let cobbles = stone * (1.0 - 0.75 * joint);
        albedo = albedo.lerp(cobbles, on_road);
        rough = 0.8;
        if let Some((carpet, cover)) = sawdust(x, z, aa, n) {
            albedo = albedo.lerp(carpet, cover);
            rough = 1.0;
        }
    }
    pbr::shade(f, u, albedo, rough, 0.0, f.normal, 1.0)
}

/// Aserrin de colores (lineales).
const SAWDUST: [Vec3; 9] = [
    Vec3::new(0.28, 0.05, 0.42),
    Vec3::new(0.70, 0.04, 0.22),
    Vec3::new(0.88, 0.30, 0.02),
    Vec3::new(0.92, 0.66, 0.04),
    Vec3::new(0.08, 0.32, 0.05),
    Vec3::new(0.04, 0.30, 0.52),
    Vec3::new(0.62, 0.03, 0.03),
    Vec3::new(0.82, 0.78, 0.70),
    Vec3::new(0.85, 0.32, 0.42),
];
const WHITE_DUST: Vec3 = SAWDUST[7];
const PINE: Vec3 = Vec3::new(0.03, 0.09, 0.025);

/// Cubre (1 adentro) una forma a distancia con signo `d`, con borde de `aa`.
#[inline]
fn fill(d: f32, aa: f32) -> f32 {
    smoothstep(aa, -aa, d)
}

/// La alfombra de aserrin frente al Arco: un fleco de pino, una cenefa de
/// dientes y una fila de cuadros, cada uno con su medallon (una flor, una
/// estrella o un sol) sobre un fondo de color. Devuelve su color y cuanto
/// cubre.
fn sawdust(x: f32, z: f32, aa: f32, noise: &Perlin) -> Option<(Vec3, f32)> {
    let (half, z0, z1) = CARPET;
    let edge = (x.abs() - half).max(z0 - z).max(z - z1);
    if edge > aa {
        return None;
    }
    let cover = fill(edge, aa);
    let ax = x.abs();
    // El fleco de pino y las lineas blancas.
    let inner = (half - 0.3 - ax).min(z - z0 - 0.3).min(z1 - 0.3 - z);
    let mut color = PINE * (0.7 + 0.6 * (noise.noise(Vec3::new(x * 9.0, z * 9.0, 1.0)) * 0.5 + 0.5));
    color = color.lerp(WHITE_DUST, fill(-inner, aa));
    // La cenefa: dientes de dos colores.
    let band = inner - 0.08;
    let tooth = ((z * 1.6).fract() - 0.5).abs() * 2.0;
    let teeth = if band / 0.35 < tooth { SAWDUST[3] } else { SAWDUST[0] };
    color = color.lerp(teeth, fill(-band, aa));
    let field = band - 0.35;
    color = color.lerp(WHITE_DUST, fill(-field, aa));
    let field = field - 0.08;
    if field > -aa {
        return Some((color, cover));
    }
    // Los cuadros, uno tras otro calle arriba.
    let length = 4.4;
    let i = ((z - z0) / length).floor();
    let id = hash_u32(i as i32 as u32 ^ 0xA1F0);
    let pick = |k: u32| SAWDUST[(hash_u32(id ^ k) % 7) as usize];
    let pz = z - z0 - (i + 0.5) * length;
    let mut panel = pick(1);
    // Entre cuadros, una franja de ajedrez.
    let divider = pz.abs() - (length * 0.5 - 0.22);
    let check = ((x * 4.5).floor() + (pz * 4.5).floor()).rem_euclid(2.0);
    panel = panel.lerp(if check < 1.0 { WHITE_DUST } else { pick(2) }, fill(-divider, aa));
    // El medallon, en polares.
    let rho = x.hypot(pz);
    let theta = pz.atan2(x);
    let size = half - 0.95;
    let medal = match id % 3 {
        // Flor: petalos de dos colores alrededor de un centro.
        0 => {
            let petals = 6.0 + 2.0 * (id >> 4 & 3) as f32;
            let r = size * (0.55 + 0.45 * (theta * petals * 0.5).cos().abs().powf(0.7));
            let which = ((theta / TAU * petals).floor()).rem_euclid(2.0);
            let ring = if which < 1.0 { pick(3) } else { pick(4) };
            let mut c = panel.lerp(WHITE_DUST, fill(rho - r - 0.07, aa));
            c = c.lerp(ring, fill(rho - r, aa));
            c = c.lerp(pick(5), fill(rho - size * 0.35, aa));
            c.lerp(WHITE_DUST, fill(rho - size * 0.12, aa))
        }
        // Estrella de puntas en un circulo.
        1 => {
            let points = 8.0;
            let a = ((theta / TAU * points).fract() - 0.5).abs() * 2.0;
            let r = size * (0.45 + 0.5 * (1.0 - a));
            let mut c = panel.lerp(pick(6), fill(rho - size, aa));
            c = c.lerp(WHITE_DUST, fill((rho - size * 0.93).abs() - 0.04, aa));
            c = c.lerp(pick(3), fill(rho - r, aa));
            c.lerp(pick(0), fill(rho - size * 0.3, aa))
        }
        // Sol: anillos y rayos.
        _ => {
            let rays = ((theta / TAU * 16.0).fract() - 0.5).abs() * 2.0;
            let mut c = panel.lerp(pick(3), fill(rho - size * (0.7 + 0.3 * rays), aa));
            c = c.lerp(pick(2), fill(rho - size * 0.62, aa));
            c = c.lerp(WHITE_DUST, fill((rho - size * 0.45).abs() - 0.05, aa));
            c.lerp(pick(6), fill(rho - size * 0.3, aa))
        }
    };
    // De lejos, el cuadro se funde en su color medio: no parpadea.
    let far = smoothstep(0.05, 0.2, aa);
    let average = panel.lerp(pick(3), 0.3);
    let art = medal.lerp(average, far);
    // Grano del aserrin.
    let grain = 1.0 + 0.15 * noise.noise(Vec3::new(x * 30.0, z * 30.0, 7.0)) * smoothstep(0.03, 0.01, aa);
    Some((color.lerp(art, fill(field, aa)) * grain, cover))
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

// Los objetos -----------------------------------------------------------------

/// Casas sueltas en los valles verdes y planos: nunca en la lava, la ceniza
/// ni el cono.
pub fn rules() -> Vec<Scatter> {
    vec![Scatter {
        kind: PropKind::Casa,
        cells: 36,
        scale: (0.8, 1.1),
        sink: 0.3,
        upright: true,
        accept: |density, s, slope| {
            smoothstep(0.55, 0.85, s.attrs.z)
                * smoothstep(0.3, 0.12, slope)
                * (1.0 - s.attrs.y)
                * (1.0 - s.attrs.x)
                * smoothstep(0.6, 0.8, density)
                * 0.5
        },
    }]
}

pub fn site(_default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    let (up, forward, _) = town_axes();
    Some(("Antigua", up.as_dvec3().normalize(), forward.as_dvec3().normalize(), place))
}

/// Una casa o el Arco por colocar: posicion en el plano del lugar, escala y
/// hacia donde mira su fachada (en el plano).
struct Lot {
    kind: PropKind,
    x: f64,
    z: f64,
    scale: f32,
    facing: (f64, f64),
}

/// Una hilera de casas pegadas: sus fachadas en la linea de `a` a `b` y
/// mirando hacia `facing`.
fn row(lots: &mut Vec<Lot>, a: (f64, f64), b: (f64, f64), facing: (f64, f64), seed: u32) {
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    let along = ((b.0 - a.0) / length, (b.1 - a.1) / length);
    let width = 2.0 * HOUSE.x as f64;
    let mut at = 0.0;
    let mut k = 0u32;
    while at < length {
        let h = hash_u32(seed ^ k.wrapping_mul(0x9E37_79B9));
        let mut scale = 0.85 + 0.25 * unit(h) as f64;
        if at + width * scale > length {
            // La ultima se angosta un poco para cerrar la hilera, o no cabe.
            scale = (length - at) / width;
            if scale < 0.75 {
                break;
            }
        }
        let middle = at + width * scale * 0.5;
        let back = HOUSE.z as f64 * scale;
        lots.push(Lot {
            kind: PropKind::Casa,
            x: a.0 + along.0 * middle - facing.0 * back,
            z: a.1 + along.1 * middle - facing.1 * back,
            scale: scale as f32,
            facing,
        });
        at += width * scale;
        k += 1;
    }
}

/// Antigua: el Arco sobre la calle y manzanas de casas alrededor.
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn HeightField, seed: u32, frame: (DVec3, DVec3, DVec3)) {
    let (n, forward, right) = frame;
    // El pueblo esta medido para el radio de Pira; con otro radio conserva
    // sus angulos (y queda alineado con lo que pinta el shader).
    let k = radius / RADIUS as f64;
    clear_site(instances, radius, n, 90.0 * k);
    let mut lots = vec![Lot {
        kind: PropKind::Arco,
        x: 0.0,
        z: ARCH_Z as f64,
        scale: 1.0,
        facing: (0.0, -1.0),
    }];
    // Linea de fachadas de cada lado de la calle del Arco y de las avenidas.
    let main = (STREET + SIDEWALK) as f64;
    let lane = (LANE + 1.0) as f64;
    let (av, calles) = (AVENUES[1] as f64, CALLES.map(|c| c as f64));
    let (arch0, arch1) = ((ARCH_Z - ARCH.z) as f64, (ARCH_Z + ARCH.z) as f64);
    let depth = 2.0 * HOUSE.z as f64 * 1.1;
    let blocks = [(TOWN.1 as f64 + 8.0, calles[0] - lane), (calles[0] + lane, arch0)];
    for (b, &(z0, z1)) in blocks.iter().enumerate() {
        for side in [-1.0, 1.0] {
            let s = seed ^ (b as u32 * 4 + if side > 0.0 { 1 } else { 0 });
            // Frente a la calle del Arco y frente a la avenida de su lado.
            row(&mut lots, (side * main, z0), (side * main, z1), (-side, 0.0), s ^ 0x11);
            row(&mut lots, (side * (av - lane), z0), (side * (av - lane), z1), (side, 0.0), s ^ 0x22);
            // Del otro lado de la avenida.
            row(&mut lots, (side * (av + lane), z0), (side * (av + lane), z1), (-side, 0.0), s ^ 0x33);
            // Frente a las calles, entre las esquinas.
            let (x0, x1) = (side * (main + depth), side * (av - lane - depth));
            if z0 > TOWN.1 as f64 + 8.0 {
                row(&mut lots, (x0, z0), (x1, z0), (0.0, -1.0), s ^ 0x44);
            }
            row(&mut lots, (x0, z1.min(calles[0] - lane)), (x1, z1.min(calles[0] - lane)), (0.0, 1.0), s ^ 0x55);
        }
    }
    // Despues del Arco, hasta la calle del pie del volcan.
    for side in [-1.0, 1.0] {
        row(&mut lots, (side * main, arch1), (side * main, calles[1] - lane), (-side, 0.0), seed ^ 0x66);
    }
    for lot in lots {
        let (x, z) = (lot.x * k, lot.z * k);
        let dir = (n + (right * x + forward * z) / radius).normalize();
        // La fachada hacia `facing`: el giro que lleva +Z del modelo ahi.
        let base = upright_rotation(dir, 0.0);
        let (x0, z0) = (base.rotate(DVec3::X), base.rotate(DVec3::Z));
        let want = right * lot.facing.0 + forward * lot.facing.1;
        let want = (want - dir * want.dot(dir)).normalize();
        let yaw = want.dot(x0).atan2(want.dot(z0));
        instances.push(at_site(radius, field, frame, lot.kind, x, z, lot.scale, yaw, 0.05));
    }
}

// Los modelos -----------------------------------------------------------------

/// El Arco de Santa Catalina, en unidades del modelo: medio ancho, alto de
/// la cornisa y medio grueso. La calle pasa a lo largo de z; la fachada que
/// se ve al llegar mira hacia +z.
const ARCH: Vec3 = Vec3::new(6.4, 8.6, 1.6);
/// Radio del arco (medio ancho del paso), altura donde arranca y ancho de
/// la arquivolta (el marco blanco alrededor).
const SPAN: f32 = 2.8;
const SPRING: f32 = 3.2;
const ARCHIVOLT: f32 = 0.45;
/// La torre del reloj: medio ancho, alto de su cornisa y medio grueso; el
/// centro y el radio de la esfera.
const TOWER: Vec3 = Vec3::new(1.5, 11.6, 1.15);
const CLOCK: (f32, f32) = (10.3, 0.75);
/// Una casa: medio ancho (a lo largo de la calle), alto de las paredes y
/// medio fondo. La fachada mira hacia +z.
const HOUSE: Vec3 = Vec3::new(2.6, 3.4, 2.6);
/// Las ventanas de una casa: centro (a cada lado de la puerta y de alto),
/// medio ancho y medio alto. La puerta: medio ancho y alto.
const WINDOW: (f32, f32, f32, f32) = (1.65, 1.75, 0.5, 0.75);
const DOOR: (f32, f32) = (0.6, 2.4);

// Colores de vertice: marcan el material.
const PLASTER: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const TRIM: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const TILE: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const IRON: Vec3 = Vec3::new(1.0, 1.0, 0.0);
const WOOD: Vec3 = Vec3::new(0.0, 1.0, 1.0);

pub fn mesh(kind: PropKind, detail: usize) -> Mesh {
    match kind {
        PropKind::Arco => arch(detail),
        _ => house(detail),
    }
}

/// Un cuadrilatero plano con su cara hacia `out`: se ordenan los vertices
/// para que giren al reves del reloj vistos desde afuera.
fn face(b: &mut MeshBuilder, [p0, p1, p2, p3]: [Vec3; 4], out: Vec3) {
    if (p1 - p0).cross(p3 - p0).dot(out) >= 0.0 {
        b.quad(p0, p1, p2, p3);
    } else {
        b.quad(p0, p3, p2, p1);
    }
}

/// Un triangulo con su cara hacia `out`.
fn tri(b: &mut MeshBuilder, [p0, p1, p2]: [Vec3; 3], out: Vec3) {
    if (p1 - p0).cross(p2 - p0).dot(out) >= 0.0 {
        b.triangle(p0, p1, p2);
    } else {
        b.triangle(p0, p2, p1);
    }
}

/// Un techo de dos aguas de teja: cumbrera a lo largo de x a la altura
/// `ridge`, aleros en `+-eave_z` a la altura `eave_y`, de `-half` a `half`.
/// Con `soffit` tambien la cara de abajo de los aleros (se ve desde la calle).
fn gable(b: &mut MeshBuilder, half: f32, eave_y: f32, eave_z: f32, ridge: f32, soffit: bool, gable_color: Vec3) {
    for s in [-1.0f32, 1.0] {
        let quad = [
            Vec3::new(-half, eave_y, s * eave_z),
            Vec3::new(half, eave_y, s * eave_z),
            Vec3::new(half, ridge, 0.0),
            Vec3::new(-half, ridge, 0.0),
        ];
        b.color(TILE);
        face(b, quad, Vec3::new(0.0, 1.0, s));
        if soffit {
            b.color(WOOD);
            face(b, quad, Vec3::new(0.0, -1.0, -s * 0.2));
        }
        b.color(gable_color);
        tri(
            b,
            [
                Vec3::new(s * half, eave_y, -eave_z),
                Vec3::new(s * half, eave_y, eave_z),
                Vec3::new(s * half, ridge, 0.0),
            ],
            Vec3::new(s, 0.0, 0.0),
        );
    }
}

/// Una fachada del Arco en `z`, con el hueco del paso: los pilares, la
/// franja de arriba y, alrededor del arco, un abanico de cuadrilateros
/// entre la curva y un cuadrado (sus esquinas caen en los angulos multiplos
/// de 45 grados: `segments` es multiplo de 4).
fn arch_facade(b: &mut MeshBuilder, z: f32, segments: usize) {
    let out = Vec3::new(0.0, 0.0, z.signum());
    let at = |x: f32, y: f32| Vec3::new(x, y, z);
    let frame = SPAN + ARCHIVOLT;
    for s in [-1.0f32, 1.0] {
        face(b, [at(s * SPAN, 0.0), at(s * ARCH.x, 0.0), at(s * ARCH.x, SPRING), at(s * SPAN, SPRING)], out);
        face(b, [at(s * frame, SPRING), at(s * ARCH.x, SPRING), at(s * ARCH.x, ARCH.y), at(s * frame, ARCH.y)], out);
    }
    face(b, [at(-frame, SPRING + frame), at(frame, SPRING + frame), at(frame, ARCH.y), at(-frame, ARCH.y)], out);
    let arc = |a: f32| at(SPAN * a.cos(), SPRING + SPAN * a.sin());
    let square = |a: f32| {
        let (s, c) = a.sin_cos();
        let t = frame / c.abs().max(s);
        at(c * t, SPRING + s.max(0.0) * t)
    };
    for i in 0..segments {
        let a0 = PI * (1.0 - i as f32 / segments as f32);
        let a1 = PI * (1.0 - (i + 1) as f32 / segments as f32);
        face(b, [arc(a0), arc(a1), square(a1), square(a0)], out);
    }
}

/// El Arco de Santa Catalina: un cuerpo grueso sobre la calle con el paso
/// en medio, la torre del reloj encima, techos de teja y cornisas blancas.
/// Las molduras, las ventanas y el reloj los pinta el shader.
fn arch(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let segments = [4, 8, 12][detail];
    let d = ARCH.z;
    b.color(PLASTER);
    arch_facade(&mut b, d, segments);
    arch_facade(&mut b, -d, segments);
    // La boveda del paso y las jambas, mirando hacia adentro.
    let arc = |a: f32, z: f32| Vec3::new(SPAN * a.cos(), SPRING + SPAN * a.sin(), z);
    for i in 0..segments {
        let a0 = PI * i as f32 / segments as f32;
        let a1 = PI * (i + 1) as f32 / segments as f32;
        let middle = (a0 + a1) * 0.5;
        let inward = -Vec3::new(middle.cos(), middle.sin(), 0.0);
        face(&mut b, [arc(a0, d), arc(a1, d), arc(a1, -d), arc(a0, -d)], inward);
    }
    for s in [-1.0f32, 1.0] {
        let x = s * SPAN;
        face(
            &mut b,
            [Vec3::new(x, 0.0, d), Vec3::new(x, 0.0, -d), Vec3::new(x, SPRING, -d), Vec3::new(x, SPRING, d)],
            Vec3::new(-s, 0.0, 0.0),
        );
        // Los costados, donde el Arco se pega a las casas.
        let x = s * ARCH.x;
        face(
            &mut b,
            [Vec3::new(x, 0.0, d), Vec3::new(x, 0.0, -d), Vec3::new(x, ARCH.y, -d), Vec3::new(x, ARCH.y, d)],
            Vec3::new(s, 0.0, 0.0),
        );
    }
    // Techo de dos aguas sobre el cuerpo.
    gable(&mut b, ARCH.x + 0.35, ARCH.y - 0.05, d + 0.55, ARCH.y + 1.3, detail >= 1, PLASTER);
    // La torre del reloj, su cornisa y su techo de cuatro aguas.
    b.color(PLASTER);
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let body = (TOWER.y - ARCH.y) * 0.5;
    b.cuboid(Vec3::new(0.0, ARCH.y + body, 0.0), axes, Vec3::new(TOWER.x, body, TOWER.z));
    let cap = TOWER.y + 0.3;
    if detail >= 1 {
        b.color(TRIM);
        b.cuboid(Vec3::new(0.0, ARCH.y - 0.2, 0.0), axes, Vec3::new(ARCH.x + 0.2, 0.2, d + 0.2));
        b.cuboid(Vec3::new(0.0, TOWER.y + 0.15, 0.0), axes, Vec3::new(TOWER.x + 0.2, 0.15, TOWER.z + 0.2));
    }
    b.color(TILE);
    let apex = Vec3::new(0.0, cap + 1.6, 0.0);
    let (rx, rz) = (TOWER.x + 0.35, TOWER.z + 0.35);
    let corners = [
        Vec3::new(-rx, cap, rz),
        Vec3::new(rx, cap, rz),
        Vec3::new(rx, cap, -rz),
        Vec3::new(-rx, cap, -rz),
    ];
    for i in 0..4 {
        let (a, c) = (corners[i], corners[(i + 1) % 4]);
        let out = ((a + c) * 0.5 - Vec3::new(0.0, cap, 0.0)).normalize() + Vec3::Y;
        tri(&mut b, [a, c, apex], out);
    }
    if detail >= 2 {
        // Cornisa sobre el arco y la cruz de hierro de la torre.
        b.color(TRIM);
        let band = SPRING + SPAN + ARCHIVOLT + 0.1;
        b.cuboid(Vec3::new(0.0, band, 0.0), axes, Vec3::new(ARCH.x + 0.08, 0.12, d + 0.12));
        b.color(IRON);
        b.cuboid(apex + Vec3::new(0.0, 0.45, 0.0), axes, Vec3::new(0.05, 0.5, 0.05));
        b.cuboid(apex + Vec3::new(0.0, 0.6, 0.0), axes, Vec3::new(0.25, 0.05, 0.05));
    }
    b.build()
}

/// Una casa colonial de un piso: paredes de color, techo de teja a dos
/// aguas con alero, y rejas de madera en las ventanas. La puerta, el zocalo
/// y las ventanas los pinta el shader.
fn house(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    b.color(PLASTER);
    b.cuboid(Vec3::new(0.0, HOUSE.y * 0.5, 0.0), axes, Vec3::new(HOUSE.x, HOUSE.y * 0.5, HOUSE.z));
    gable(&mut b, HOUSE.x + 0.06, HOUSE.y - 0.1, HOUSE.z + 0.5, HOUSE.y + 1.1, detail >= 1, PLASTER);
    let (wx, wy, ww, wh) = WINDOW;
    if detail >= 1 {
        // Las rejas: cajas que salen de la fachada.
        b.color(IRON);
        for x in [-wx, wx] {
            b.cuboid(Vec3::new(x, wy, HOUSE.z + 0.12), axes, Vec3::new(ww, wh, 0.12));
        }
    }
    if detail >= 2 {
        // Tejadillo sobre cada reja y la cornisa bajo el alero.
        b.color(TILE);
        for x in [-wx, wx] {
            b.cuboid(Vec3::new(x, wy + wh + 0.08, HOUSE.z + 0.17), axes, Vec3::new(ww + 0.12, 0.07, 0.17));
        }
        b.color(TRIM);
        b.cuboid(Vec3::new(0.0, HOUSE.y - 0.2, HOUSE.z + 0.05), axes, Vec3::new(HOUSE.x - 0.02, 0.12, 0.05));
    }
    b.build()
}

/// Colores de las casas de Antigua (lineales).
const WALLS: [Vec3; 8] = [
    Vec3::new(0.80, 0.45, 0.06),
    Vec3::new(0.50, 0.07, 0.04),
    Vec3::new(0.07, 0.17, 0.42),
    Vec3::new(0.80, 0.74, 0.58),
    Vec3::new(0.78, 0.30, 0.15),
    Vec3::new(0.25, 0.42, 0.20),
    Vec3::new(0.18, 0.45, 0.48),
    Vec3::new(0.62, 0.36, 0.05),
];
const ARCH_YELLOW: Vec3 = Vec3::new(0.78, 0.42, 0.04);
const LIME_WHITE: Vec3 = Vec3::new(0.8, 0.78, 0.72);
const TERRACOTTA: Vec3 = Vec3::new(0.36, 0.1, 0.045);
const DARK_WOOD: Vec3 = Vec3::new(0.07, 0.035, 0.018);
const BLACK_IRON: Vec3 = Vec3::new(0.02, 0.018, 0.017);

pub fn shade_prop(kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let c = f.color;
    if c.z > 0.5 && c.y < 0.5 {
        return tiles(f, u, style, sun);
    }
    if c.x > 0.5 && c.y > 0.5 {
        // Reja: barrotes oscuros con la ventana detras.
        let bars = ((f.local.x * 6.0).fract() - 0.5).abs() * 2.0;
        let albedo = if bars < 0.35 { BLACK_IRON } else { DARK_WOOD * 0.5 };
        return pbr::shade_light(f, u, albedo, 0.6, 0.3, f.normal, 1.0, sun);
    }
    if c.y > 0.5 && c.z > 0.5 {
        return pbr::shade_light(f, u, DARK_WOOD, 0.8, 0.0, f.normal, 1.0, sun);
    }
    if c.y > 0.5 {
        return pbr::shade_light(f, u, LIME_WHITE, 0.85, 0.0, f.normal, 1.0, sun);
    }
    let wall = match kind {
        PropKind::Arco => ARCH_YELLOW,
        _ => WALLS[((style.seed * WALLS.len() as f32) as usize).min(WALLS.len() - 1)],
    };
    // La normal en los ejes del modelo: las caras que miran al frente (+z)
    // y atras (-z).
    let m = &f.object_to_world.m;
    let facing = m[0][2] * f.normal.x + m[1][2] * f.normal.y + m[2][2] * f.normal.z;
    let p = f.local;
    let paint = match kind {
        PropKind::Arco if facing.abs() > 0.7 => clock(p, facing, u.time),
        PropKind::Casa if facing > 0.7 => door(p),
        _ => None,
    };
    let albedo = paint.unwrap_or(wall);
    pbr::shade_light(f, u, albedo, 0.9, 0.0, f.normal, 1.0, sun)
}

/// El reloj de la torre del Arco, en sus dos caras: caratula blanca con
/// borde de hierro y dos agujas que avanzan con el tiempo de la simulacion
/// (el minutero da la vuelta en un minuto, la aguja de la hora en doce).
/// Visto desde atras se refleja `x`, asi las dos caras giran en el sentido
/// del reloj vistas desde afuera.
fn clock(p: Vec3, facing: f32, time: f32) -> Option<Vec3> {
    let dx = p.x * facing.signum();
    let dy = p.y - CLOCK.0;
    let r = dx.hypot(dy);
    if r > CLOCK.1 + 0.1 {
        return None;
    }
    if r > CLOCK.1 {
        return Some(BLACK_IRON);
    }
    // Una aguja de largo `length` en el angulo `angle` (desde las doce).
    let hand = |angle: f32, length: f32, width: f32| {
        let (s, c) = angle.sin_cos();
        let along = dx * s + dy * c;
        let across = (dx * c - dy * s).abs();
        smoothstep(width, width * 0.4, across) * smoothstep(-0.05, 0.0, along) * smoothstep(length, length - 0.05, along)
    };
    let minute = hand(time / 60.0 * TAU, CLOCK.1 * 0.85, 0.05);
    let hour = hand(time / 720.0 * TAU, CLOCK.1 * 0.55, 0.07);
    // Las doce marcas de las horas.
    let tick = (dy.atan2(dx) / TAU * 12.0).fract();
    let ticks = smoothstep(0.08, 0.03, (tick - 0.5).abs().min(0.5 - (tick - 0.5).abs()))
        * smoothstep(CLOCK.1 * 0.75, CLOCK.1 * 0.85, r);
    Some(LIME_WHITE.lerp(BLACK_IRON, minute.max(hour).max(ticks * 0.8)))
}

/// La fachada de una casa: puerta de madera de dos hojas en medio y el
/// zocalo pintado de un color mas oscuro abajo.
fn door(p: Vec3) -> Option<Vec3> {
    if p.x.abs() < DOOR.0 && p.y < DOOR.1 {
        // Tableros: una linea entre las hojas y marcos alrededor.
        let seam = smoothstep(0.04, 0.02, p.x.abs());
        let panel = ((p.y / DOOR.1 * 3.0).fract() - 0.5).abs();
        let frame = smoothstep(0.42, 0.47, panel);
        return Some(DARK_WOOD * (1.0 - 0.45 * seam.max(frame)));
    }
    (p.y < 0.55).then_some(DARK_WOOD * 0.9 + Vec3::new(0.05, 0.03, 0.02))
}

/// Teja de barro: canales a lo largo de la pendiente, con su relieve.
fn tiles(f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    let across = (p.x * 3.0).fract();
    let tone = 0.75 + 0.5 * unit(hash_u32((p.x * 3.0).floor() as i32 as u32 ^ ((p.y * 2.2).floor() as i32 as u32).wrapping_mul(0x9E37) ^ (style.seed * 1000.0) as u32));
    let ridge = (across * PI).sin();
    pbr::shade_light(f, u, TERRACOTTA * tone * (0.55 + 0.45 * ridge), 0.75, 0.0, f.normal, 1.0, sun)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directions() -> impl Iterator<Item = Vec3> {
        (0..4000).map(|i| {
            let t = i as f32 * 0.37;
            Vec3::new(t.sin() * (t * 0.31).cos(), (t * 0.77).cos(), t.cos() * (t * 0.13).sin()).normalize()
        })
    }

    /// Alrededor de cada volcan y del pueblo, donde el relieve cambia mas.
    fn close_ups() -> impl Iterator<Item = Vec3> {
        VOLCANOES.iter().map(|v| v.center).chain([ANTIGUA]).flat_map(|c| {
            let (e1, e2) = perpendiculars(c);
            (0..600).map(move |i| {
                let a = i as f32 * 2.399;
                let r = (i as f32 / 600.0).sqrt() * 0.5;
                (c + (e1 * a.cos() + e2 * a.sin()) * r).normalize()
            })
        })
    }

    /// El relieve se queda dentro de su rango (el culling por horizonte
    /// depende de eso) y la misma semilla da el mismo planeta.
    #[test]
    fn the_volcano_stays_in_range_and_is_deterministic() {
        let (a, b) = (FuegoField::new(7), FuegoField::new(7));
        let (lo, hi) = a.height_range();
        for dir in directions().chain(close_ups()) {
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
            assert_eq!(s.attrs, b.sample(dir).attrs);
        }
    }
}
