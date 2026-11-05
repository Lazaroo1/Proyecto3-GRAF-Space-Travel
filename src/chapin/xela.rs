//! La Luna de Xelaju: la luna de los Cuchumatanes (solo existe en la
//! version chapina). Palida y quieta, como la de la cancion.
//!
//! - **Mares y crateres**: el color sale de la foto de la Luna (LROC, en
//!   `assets/textures`), mas gris y palida; el relieve, de dos campos de
//!   crateres (`Craters`, de `terrain::shapes`) sobre lomas suaves.
//! - **Luz de luna**: el regolito no se ve como un plato de Lambert. Con
//!   **Lommel-Seeliger** (`mu0 / (mu0 + mu)`, con `mu0` el coseno hacia la
//!   luz y `mu` hacia la camara) el disco lleno se ve parejo hasta el borde,
//!   como la Luna de verdad, y el terminador sigue marcando los crateres.
//! - **El huipil**: franjas de un tejido de Quetzaltenango grabadas en la
//!   luna: rombos con su punto, grecas de dientes y lineas, en una grilla de
//!   puntadas (como un brocado). Cada franja tiene un numero entero de
//!   puntadas por vuelta (cierra sin costura) y cada puntada es un cuadrito
//!   redondeado. Brillan de noche; de dia apenas se ven como hilo grabado.
//!   Cuando una puntada mide menos que un par de pixeles se dibuja su
//!   promedio: de lejos la franja brilla pareja y no parpadea.
//! - **La marimba** (el lugar, N): una marimba doble grande sobre el polvo,
//!   con su fila de teclas naturales y la de alteradas mas alta atras, los
//!   cajones resonadores debajo (mas largos en los graves), las patas y el
//!   faldon de tela tipica al frente, con las mismas franjas del huipil de
//!   la luna. Cerca de ella, Enter abre el minijuego (`minijuegos::marimba`).
//!   Las teclas son de hormigo con la veta a lo largo; de noche los hilos
//!   del faldon brillan como los de la luna.

use std::f32::consts::TAU;
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::math::{DVec3, Perlin, Vec3, smoothstep};
use crate::scene::Mesh;
use crate::scene::builder::MeshBuilder;
use crate::scene::props::{Instance, PropKind, at_site, upright_rotation};
use crate::shaders::{Fragment, PropStyle, Uniforms, detail, pbr};
use crate::terrain::shapes::Craters;
use crate::terrain::{HeightField, TerrainSample};

pub fn spec() -> Spec {
    Spec {
        label: "Luna de Xelaju",
        material: Chapin::Xela,
        atmosphere: None,
        rings: None,
        motes: None,
    }
}

/// El suelo de la luna. Atributos: `x` borde de crater.
pub struct Moon {
    noise: Perlin,
    craters: Craters,
    small: Craters,
}

impl Moon {
    pub fn new(seed: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            craters: Craters {
                frequency: 5.0,
                chance: 0.5,
                min_radius: 0.2,
                max_radius: 0.6,
                depth: 1.6,
                seed: seed ^ 0x3E1,
            },
            small: Craters {
                frequency: 15.0,
                chance: 0.45,
                min_radius: 0.15,
                max_radius: 0.55,
                depth: 0.5,
                seed: seed ^ 0xA7A,
            },
        }
    }
}

impl HeightField for Moon {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let swell = self.noise.fbm(dir * 2.5 + Vec3::new(1.7, -3.1, 0.6), 4);
        let (big, big_rim) = self.craters.sample(dir);
        let (small, small_rim) = self.small.sample(dir);
        TerrainSample {
            height: 0.8 * swell + big + small,
            attrs: Vec3::new(big_rim.max(small_rim), 0.0, 0.0),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        let (lo_a, hi_a) = self.craters.range();
        let (lo_b, hi_b) = self.small.range();
        (-0.8 + lo_a + lo_b, 0.8 + hi_a + hi_b)
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(Moon::new(seed))
}

/// Una franja del huipil: latitud del centro (radianes), puntadas por
/// vuelta, filas y el motivo.
struct Band {
    latitude: f32,
    around: f32,
    rows: f32,
    motif: fn(i32, i32) -> Option<usize>,
    /// Fraccion de puntadas encendidas (el promedio de lejos).
    fill: f32,
}

/// Hilos: morado, magenta, oro y turquesa.
const THREAD: [Vec3; 4] = [
    Vec3::new(0.42, 0.12, 0.9),
    Vec3::new(1.0, 0.16, 0.5),
    Vec3::new(1.0, 0.66, 0.2),
    Vec3::new(0.2, 0.85, 0.8),
];

/// La franja ancha: lineas de oro arriba y abajo, puntos de magenta, y una
/// cadena de rombos morados con un punto turquesa al centro.
fn diamonds(i: i32, j: i32) -> Option<usize> {
    let c = i.rem_euclid(8) - 4;
    match j {
        0 | 10 => Some(2),
        2 | 8 if i % 2 == 0 => Some(1),
        3..=7 => {
            let d = c.abs() + (j - 5).abs();
            if d == 2 || (d == 4 && c.abs() < 4) {
                Some(0)
            } else if d == 0 {
                Some(3)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Las franjas angostas: una greca de dientes entre dos lineas.
fn zigzag(i: i32, j: i32) -> Option<usize> {
    let tooth = 1 + (i.rem_euclid(6) - 3).abs().min(2);
    match j {
        0 | 4 => Some(1),
        _ if j == tooth => Some(3),
        _ => None,
    }
}

const BANDS: [Band; 3] = [
    Band { latitude: 0.0, around: 168.0, rows: 11.0, motif: diamonds, fill: 0.27 },
    Band { latitude: 0.62, around: 120.0, rows: 5.0, motif: zigzag, fill: 0.53 },
    Band { latitude: -0.62, around: 120.0, rows: 5.0, motif: zigzag, fill: 0.53 },
];

/// El tejido en la direccion `p` (unitaria, en el espacio de la luna):
/// cuanto brilla y de que color. `pixel` es lo que mide un pixel en
/// radianes sobre la luna.
fn huipil(p: Vec3, pixel: f32) -> Vec3 {
    let latitude = p.y.clamp(-1.0, 1.0).asin();
    let longitude = p.z.atan2(p.x) / TAU + 0.5;
    for band in &BANDS {
        // Puntadas cuadradas: a lo alto miden lo mismo que a lo largo en el
        // centro de la franja.
        let step = TAU / band.around * band.latitude.cos();
        let row = (latitude - band.latitude) / step + band.rows * 0.5;
        if !(0.0..band.rows).contains(&row) {
            continue;
        }
        let column = longitude * band.around;
        let (i, j) = (column.floor(), row.floor());
        let size = step / pixel;
        let resolved = smoothstep(1.5, 3.0, size);
        let mut color = Vec3::ZERO;
        if let Some(thread) = (band.motif)(i as i32, j as i32) {
            // Cuadrito redondeado con un hueco entre puntadas.
            let (fx, fy) = (column - i - 0.5, row - j - 0.5);
            let soft = (0.5 / size).max(0.04);
            color = THREAD[thread] * smoothstep(soft, -soft, fx.abs().max(fy.abs()) - 0.36);
        }
        let average = (THREAD[0] + THREAD[1] + THREAD[2] + THREAD[3]) * (0.25 * band.fill * 0.55);
        return average.lerp(color, resolved);
    }
    Vec3::ZERO
}

const DUST: Vec3 = Vec3::new(0.7, 0.71, 0.74);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let radius = f.local.length().max(1.0);
    let p = f.local / radius;
    let footprint = f.pos.length() * u.pixel_angle;
    // La foto de la Luna, gris y palida.
    let photo = u.textures.images.moon.sphere(p, footprint / (TAU * radius));
    let grey = photo.lerp(Vec3::splat(photo.luminance()), 0.75);
    let albedo = (grey * 1.35).min(Vec3::splat(0.9)) * DUST;
    let rock = detail::rock(f, u, 0.35, footprint, 0.35);
    let n = rock.normal;
    let l = f.light_dir(u);
    let v = f.view_dir();
    // Lommel-Seeliger: el disco lleno parejo hasta el borde.
    // Lambert ya multiplica por mu0: con la luz dividida entre mu0 + mu
    // queda Lommel-Seeliger (el 2.3 iguala el brillo de frente).
    let (mu0, mu) = (n.dot(l).max(0.0), n.dot(v).max(0.0));
    let light = u.sun_light.lerp(Vec3::splat(u.sun_light.luminance()), 0.5);
    let lunar = light * (2.3 * std::f32::consts::PI / 3.6 / (mu0 + mu).max(0.05));
    let lit = pbr::shade_light(f, u, albedo, 0.95, 0.0, n, rock.ao, lunar);
    // El huipil: brilla de noche; de dia es hilo grabado.
    let ground = f.object_to_world.mul_vec(p);
    let night = smoothstep(0.12, -0.2, ground.dot(l));
    let thread = huipil(p, footprint / radius);
    let etched = 1.0 - 0.3 * thread.luminance().min(1.0) * (1.0 - night);
    let glow = thread * ((0.03 + 0.5 * night) * u.gains.argos);
    lit * etched + glow
}

// La marimba ------------------------------------------------------------------

/// Como se llega a ver la marimba: cerca y un poco arriba (la luna mide 48
/// de radio: a 90 de distancia, como en los otros lugares, ya no se ve).
/// La vista mira 45 adelante (como en los otros lugares), que aqui ya es
/// pasando el horizonte: la mirada se baja hasta la marimba.
pub const SITE_VIEW: (f64, f64, f64) = (8.0, 9.0, -14.0);
/// Que tan adelante del lugar esta la marimba y su tamano (el modelo mide
/// 3 de largo, como una de verdad en metros: esta es de casi once).
const MARIMBA_AHEAD: f64 = 10.0;
const MARIMBA_SCALE: f32 = 3.6;
/// Un poco a la derecha, para que la nave no la tape al llegar.
const MARIMBA_RIGHT: f64 = 6.0;

pub fn site(default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    let (normal, forward) = default;
    Some(("La marimba", normal, forward, place))
}

/// La marimba, con el faldon (+z del modelo) hacia quien llega.
fn place(instances: &mut Vec<Instance>, radius: f64, field: &dyn crate::terrain::HeightField, _seed: u32, frame: (DVec3, DVec3, DVec3)) {
    let (n, forward, right) = frame;
    let dir = (n + (forward * MARIMBA_AHEAD + right * MARIMBA_RIGHT) / radius).normalize();
    let base = upright_rotation(dir, 0.0);
    let (x0, z0) = (base.rotate(DVec3::X), base.rotate(DVec3::Z));
    // De frente a donde llega la nave (atras del lugar).
    let toward = -forward * (MARIMBA_AHEAD + SITE_VIEW.1) - right * MARIMBA_RIGHT;
    let toward = (toward - dir * toward.dot(dir)).normalize();
    let yaw = toward.dot(x0).atan2(toward.dot(z0));
    instances.push(at_site(radius, field, frame, PropKind::Marimba, MARIMBA_RIGHT, MARIMBA_AHEAD, MARIMBA_SCALE, yaw, 0.0));
}

/// Materiales de la marimba (por color de vertice).
const BAR: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const FRAME: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const CLOTH: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const BOX: Vec3 = Vec3::new(1.0, 1.0, 0.0);
/// Arriba de las teclas naturales y de las alteradas.
const NATURALS_TOP: f32 = 0.92;
const SHARPS_TOP: f32 = 1.0;
/// El faldon: de donde a donde cuelga y el tamano de una puntada.
const SKIRT: (f32, f32) = (0.88, 0.3);
const STITCH: f32 = 0.04;

/// La marimba doble: teclas naturales con la punta de quien toca alineada
/// (-z) y mas largas en los graves (-x); las alteradas en una fila mas alta
/// hacia el frente, en grupos de dos y tres como en un teclado; un cajon
/// resonador debajo de cada natural; el marco, las patas y el faldon.
pub fn mesh(detail: usize) -> Mesh {
    let mut b = MeshBuilder::new();
    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
    let naturals = [10, 18, 28][detail];
    let span = 2.9;
    let pitch = span / naturals as f32;
    let width = pitch * 0.84;
    let length = |t: f32| 0.62 - 0.3 * t;
    for i in 0..naturals {
        let t = i as f32 / (naturals - 1) as f32;
        let x = -span * 0.5 + pitch * (i as f32 + 0.5);
        let l = length(t);
        b.color(BAR);
        b.cuboid(Vec3::new(x, NATURALS_TOP - 0.02, -0.5 + l * 0.5), axes, Vec3::new(width * 0.5, 0.02, l * 0.5));
        // El cajon resonador: mas largo en los graves.
        b.color(BOX);
        let depth = 0.62 - 0.5 * t;
        b.cuboid(Vec3::new(x, 0.84 - depth * 0.5, -0.5 + l * 0.5), axes, Vec3::new(width * 0.42, depth * 0.5, 0.05));
        // Las alteradas: despues de la 1a, 2a, 4a, 5a y 6a natural de cada
        // septima (do#, re#, fa#, sol#, la#).
        if detail >= 1 && i + 1 < naturals && matches!(i % 7, 0 | 1 | 3 | 4 | 5) {
            let ls = l * 0.75;
            b.color(BAR);
            b.cuboid(Vec3::new(x + pitch * 0.5, SHARPS_TOP - 0.02, 0.42 - ls * 0.5), axes, Vec3::new(width * 0.42, 0.02, ls * 0.5));
        }
    }
    b.color(FRAME);
    // Los largueros de cada fila y los costados.
    b.cuboid(Vec3::new(0.0, 0.85, -0.52), axes, Vec3::new(1.5, 0.05, 0.025));
    b.cuboid(Vec3::new(0.0, 0.85, 0.12), axes, Vec3::new(1.5, 0.05, 0.025));
    b.cuboid(Vec3::new(0.0, 0.94, 0.44), axes, Vec3::new(1.5, 0.04, 0.025));
    for x in [-1.5, 1.5] {
        b.cuboid(Vec3::new(x, 0.88, -0.04), axes, Vec3::new(0.03, 0.07, 0.5));
    }
    // Las patas y el travesano de abajo.
    for (x, z) in [(-1.42, -0.48), (1.42, -0.48), (-1.42, 0.42), (1.42, 0.42)] {
        b.cuboid(Vec3::new(x, 0.42, z), axes, Vec3::new(0.035, 0.42, 0.035));
    }
    if detail >= 1 {
        b.cuboid(Vec3::new(0.0, 0.16, -0.48), axes, Vec3::new(1.42, 0.025, 0.02));
    }
    // El faldon de tela que cuelga al frente.
    b.color(CLOTH);
    let (top, bottom) = SKIRT;
    b.cuboid(Vec3::new(0.0, (top + bottom) * 0.5, 0.48), axes, Vec3::new(1.52, (top - bottom) * 0.5, 0.006));
    b.build()
}

/// El dibujo del faldon en la puntada (i, j) (fila 0 arriba): una linea de
/// oro, una greca de dientes y una cadena de rombos morados con su punto de
/// oro, sobre rojo. Puntadas grandes: se leen desde donde se llega.
fn skirt_thread(i: i32, j: i32) -> Option<usize> {
    match j {
        0 => Some(2),
        2..=6 => zigzag(i, j - 2),
        8..=14 => {
            let c = i.rem_euclid(8) - 4;
            match c.abs() + (j - 11).abs() {
                3 => Some(0),
                1 => Some(1),
                0 => Some(2),
                _ => None,
            }
        }
        _ => None,
    }
}

const HORMIGO: Vec3 = Vec3::new(0.3, 0.1, 0.04);
const WOOD: Vec3 = Vec3::new(0.16, 0.07, 0.035);
const HUIPIL_RED: Vec3 = Vec3::new(0.5, 0.03, 0.06);

pub fn shade_prop(f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    let p = f.local;
    let code = |v: Vec3| (v.x > 0.5, v.y > 0.5, v.z > 0.5);
    let is = |material: Vec3| code(f.color) == code(material);
    let ndv = f.normal.dot(f.view_dir()).abs().max(0.25);
    let soft = f.pos.length() * u.pixel_angle / (ndv * style.scale);
    let up = f.object_to_world.mul_vec(Vec3::Y);
    let night = smoothstep(0.12, -0.2, up.dot(f.light_dir(u)));
    let (albedo, rough, glow) = if is(BAR) {
        // Hormigo con la veta a lo largo de la tecla; cada tecla con su tono.
        let streak = u.noise.noise(Vec3::new(p.x * 60.0, p.y * 6.0, p.z * 5.0)) * 0.5 + 0.5;
        let key = u.noise.noise(Vec3::new(p.x * 9.0, 3.1, 0.7)) * 0.5 + 0.5;
        (HORMIGO * (0.7 + 0.5 * streak) * (0.85 + 0.3 * key), 0.35, Vec3::ZERO)
    } else if is(CLOTH) {
        let (top, _) = SKIRT;
        let (column, row) = ((p.x + 2.0) / STITCH, (top - p.y) / STITCH);
        let (i, j) = (column.floor(), row.floor());
        let size = STITCH / soft;
        let resolved = smoothstep(1.5, 3.0, size);
        let mut color = HUIPIL_RED;
        let mut thread = Vec3::ZERO;
        if let Some(k) = skirt_thread(i as i32, j as i32) {
            let (fx, fy) = (column - i - 0.5, row - j - 0.5);
            let s = (0.5 / size).max(0.04);
            let stitch = smoothstep(s, -s, fx.abs().max(fy.abs()) - 0.36);
            thread = THREAD[k] * stitch;
            color = HUIPIL_RED.lerp(THREAD[k], stitch);
        }
        // De lejos, el promedio de la tela.
        let average = HUIPIL_RED.lerp((THREAD[0] + THREAD[1] + THREAD[2] + THREAD[3]) * 0.25, 0.3);
        let albedo = average.lerp(color, resolved);
        let thread = (average - HUIPIL_RED).max(Vec3::ZERO).lerp(thread, resolved);
        (albedo * 0.8, 0.9, thread * ((0.04 + 0.6 * night) * u.gains.argos))
    } else if is(BOX) {
        let streak = u.noise.noise(Vec3::new(p.x * 30.0, p.y * 3.0, p.z * 30.0)) * 0.5 + 0.5;
        (WOOD * (0.8 + 0.4 * streak), 0.7, Vec3::ZERO)
    } else {
        let streak = u.noise.noise(Vec3::new(p.x * 4.0, p.y * 25.0, p.z * 25.0)) * 0.5 + 0.5;
        (WOOD * 1.2 * (0.8 + 0.4 * streak), 0.6, Vec3::ZERO)
    };
    let light = sun.lerp(Vec3::splat(sun.luminance()), 0.5);
    pbr::shade_light(f, u, albedo, rough, 0.0, f.normal, 1.0, light) + glow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_moon_stays_in_range_and_is_deterministic() {
        let (a, b) = (Moon::new(9), Moon::new(9));
        let (lo, hi) = a.height_range();
        for i in 0..4000 {
            let t = i as f32 * 0.37;
            let dir = Vec3::new(t.sin() * (t * 0.31).cos(), (t * 0.77).cos(), t.cos() * (t * 0.13).sin()).normalize();
            let s = a.sample(dir);
            assert!(s.height >= lo && s.height <= hi, "{} fuera de [{lo}, {hi}]", s.height);
            assert_eq!(s.height, b.sample(dir).height);
        }
    }

    /// El tejido cierra la vuelta sin costura y fuera de las franjas no hay
    /// nada.
    #[test]
    fn the_huipil_closes_around_and_stays_in_its_bands() {
        for band in &BANDS {
            let around = band.around as i32;
            for j in 0..band.rows as i32 {
                for i in 0..8 {
                    assert_eq!((band.motif)(i, j), (band.motif)(i + around, j), "{i} {j}");
                }
            }
        }
        assert_eq!(huipil(Vec3::Y, 0.001), Vec3::ZERO);
        assert_eq!(huipil(Vec3::new(0.0, 0.35, 1.0).normalize(), 0.001), Vec3::ZERO);
    }
}
