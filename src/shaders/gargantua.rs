//! Gargantúa: Variante predeterminada del Proyecto 2, con gas de volumen finito.
//! La tabla acelera el cielo lejano; en el disco se integra el mismo RK4 y
//! transferencia radiativa del Proyecto 2, incluyendo las nueve capas de gas.
use super::{Uniforms, sky::corona};
use crate::gargantua_p2::{
    self as p2,
    disk::{GasTexture, Medium, RadialTable},
    math::Vec3 as P2Vec,
    photon::Photon,
    relativity::RayFrame,
};
use crate::math::geodesic::{LensTable, weak_deflection};
use crate::math::{Vec3, fast_atan2};

pub struct BlackHole<'a> {
    pub center: Vec3,
    pub rs: f32,
    pub normal: Vec3,
    pub axis_a: Vec3,
    pub axis_b: Vec3,
    pub table: &'a LensTable,
    pub gas: &'a GasTexture,
    pub density: f32,
    pub outer: f32,
    pub peak: f32,
    pub cone_cos: f32,
    /// El gas de Pira entrando al disco, cuando Gargantua se la come.
    pub stream: Option<Stream>,
}

/// Pira deshecha: su gas entra al disco como un brazo en espiral. Sale de
/// donde esta Pira, baja al plano del disco, se calienta al caer y, como el
/// gas de adentro gira mas rapido, se enrolla y se abre con el tiempo hasta
/// diluirse en el disco. Cuando Pira ya no esta, lo de afuera se vacia: el
/// gas sigue cayendo.
#[derive(Clone, Debug)]
pub struct Stream {
    /// Pira, en el marco del disco (y es la normal) y en `rs`.
    start: P2Vec,
    /// Su radio y su angulo en el disco.
    radius: f32,
    angle: f32,
    /// Segundos desde que empezo a deshacerse.
    age: f32,
    /// Lo que solo depende de la edad: cuanto queda de Pira (su embudo se
    /// encoge con ella), cuanto se ensancho el brazo, hasta donde se vacio y
    /// su brillo.
    alive: f32,
    spread: f32,
    drained: f32,
    fade: f32,
    /// El eje cada tanto de radio, de `3 rs` a Pira: para decidir rapido si
    /// un paso del rayo pasa cerca.
    track: [Track; TRACK_SIZE],
}

const TRACK_SIZE: usize = 48;

/// Un punto del eje en la tabla: direccion en el plano, altura, y los
/// margenes (grosor, ancho y cuanto gira por `rs`).
#[derive(Clone, Copy, Debug, Default)]
struct Track {
    dir: (f32, f32),
    ys: f32,
    thick: f32,
    width: f32,
    slope: f32,
}

/// Segundos que el brazo recibe gas: lo que tarda Pira en deshacerse y un
/// poco mas (despues, se vacia de afuera hacia adentro).
const FEEDING: f32 = super::vapor::MELT + 2.0;
/// Pasado esto ya no queda nada del brazo: se mezclo con el disco.
pub const STREAM_LIFE: f32 = 75.0;

/// El eje del brazo a una distancia del agujero.
struct Axis {
    /// Avance desde Pira (0) hasta el borde interior del disco (1).
    u: f32,
    /// Altura y grosor, angulo y ancho (en `rs`).
    ys: f32,
    thick: f32,
    phi: f32,
    width: f32,
    /// Cuanto cambia el angulo del eje por `rs` de radio.
    slope: f32,
}

impl Stream {
    pub fn new(start: P2Vec, age: f32) -> Self {
        use crate::math::smoothstep;
        let mut stream = Self {
            start,
            radius: start.x.hypot(start.z),
            angle: fast_atan2(start.z, start.x),
            age,
            alive: super::Vapor::core(age) * smoothstep(super::vapor::MELT, super::vapor::MELT - 2.0, age),
            spread: 1.0 + age / 30.0,
            drained: ((age - FEEDING) / 18.0).clamp(0.0, 1.2),
            // Recien soltado brilla; en un minuto ya se diluyo en el disco.
            fade: smoothstep(0.0, 2.0, age)
                * (-age / 30.0).exp()
                * smoothstep(STREAM_LIFE, STREAM_LIFE - 15.0, age),
            track: [Track::default(); TRACK_SIZE],
        };
        for k in 0..TRACK_SIZE {
            let axis = stream.axis(stream.track_radius(k as f32));
            stream.track[k] = Track {
                dir: (axis.phi.cos(), axis.phi.sin()),
                ys: axis.ys,
                thick: axis.thick,
                width: axis.width,
                slope: axis.slope,
            };
        }
        stream
    }

    /// Radio de la entrada `k` (fraccionaria) de la tabla del eje.
    #[inline]
    fn track_radius(&self, k: f32) -> f32 {
        3.0 + (self.radius + 1.5 - 3.0) * k / (TRACK_SIZE - 1) as f32
    }

    /// Hasta donde llega, en `rs` desde el agujero.
    pub fn reach(&self) -> f32 {
        self.radius + 2.0
    }

    #[inline]
    fn axis(&self, r: f32) -> Axis {
        let rp = self.radius;
        let u = ((rp - r) / (rp - 3.0)).clamp(0.0, 1.0);
        // Un embudo junto a Pira (sale de todo el lado que da al agujero) y
        // despues un chorro delgado.
        let funnel = (1.0 - u).powi(8) * self.alive;
        // El gas del disco gira hacia angulos decrecientes y el de adentro
        // mas rapido: el brazo se adelanta hacia adentro y se enrolla. Sale
        // casi derecho hacia el agujero y se curva despues.
        let ratio = rp / r.max(3.0);
        let late = self.age * 0.07 * ratio * ratio.sqrt();
        let root = u.sqrt();
        let wind = u * root * (1.4 + late);
        Axis {
            u,
            // Baja de la altura de Pira al plano del disco.
            ys: self.start.y * (1.0 - u) * (1.0 - u),
            thick: 0.12 + 0.2 * (1.0 - u) + 0.9 * funnel,
            phi: self.angle - wind,
            width: (0.14 + 0.45 * u + funnel) * self.spread,
            slope: 1.5 * (root * (1.4 + late) / (rp - 3.0) + u * root * late / r.max(3.0)),
        }
    }

    /// Luz y profundidad optica del brazo a lo largo de un tramo largo
    /// (casi recto) de `a` a `b`. Lejos del agujero un paso del rayo mide
    /// mas que el grosor del brazo: muestrearlo en un punto dibujaria discos
    /// apilados. Se descarta rapido si el tramo pasa lejos; si no, se suma el
    /// brazo en pedazos del tramo: su forma (gaussiana, de la tabla del eje)
    /// y, solo donde la forma pesa, la turbulencia. El color se toma donde
    /// el tramo pasa mas cerca.
    #[inline]
    fn through(&self, a: P2Vec, b: P2Vec, gas: &GasTexture) -> Option<(Vec3, f32)> {
        if self.fade <= 0.0 {
            return None;
        }
        let mid = (a + b) * 0.5;
        let length = (b - a).length();
        let half = length * 0.5;
        let r = mid.x.hypot(mid.z);
        if r - half > self.radius + 1.5 || r + half < 3.0 {
            return None;
        }
        // Ni por encima ni por debajo de todo lo que el brazo puede ocupar.
        if mid.y.abs() > self.start.y.abs() + 3.7 + half {
            return None;
        }
        let track = self.track_at(r);
        if (mid.y - track.ys).abs() > 3.0 * track.thick + half {
            return None;
        }
        // Lejos del punto del eje a ese mismo radio (con margen por lo que
        // el radio cambia a lo largo del tramo), no lo toca.
        let off = (mid.x - track.dir.0 * r).hypot(mid.z - track.dir.1 * r);
        if off > 3.0 * track.width + half * (1.0 + track.slope * r) {
            return None;
        }
        let pieces = ((length / 0.4).ceil() as usize).clamp(1, 5);
        let piece = length / pieces as f32;
        let (mut total, mut best, mut nearest) = (0.0, 0.0, mid);
        for k in 0..pieces {
            let q = a + (b - a) * ((k as f32 + 0.5) / pieces as f32);
            let shape = self.shape(q);
            if shape > 0.01 {
                total += shape * gas.turbulence(q);
                if shape > best {
                    (best, nearest) = (shape, q);
                }
            }
        }
        if total * piece < 1e-3 {
            return None;
        }
        let rq = nearest.x.hypot(nearest.z);
        let u = ((self.radius - rq) / (self.radius - 3.0)).clamp(0.0, 1.0);
        Some((glow(u), self.fade / self.spread * 0.9 * total * piece))
    }

    /// El eje a la distancia `r`, de la tabla.
    #[inline]
    fn track_at(&self, r: f32) -> Track {
        let x = ((r - 3.0) / (self.radius + 1.5 - 3.0) * (TRACK_SIZE - 1) as f32)
            .clamp(0.0, (TRACK_SIZE - 1) as f32 - 1e-3);
        let (k, f) = (x as usize, x.fract());
        let (a, b) = (&self.track[k], &self.track[k + 1]);
        let mix = |p: f32, q: f32| p + (q - p) * f;
        Track {
            dir: (mix(a.dir.0, b.dir.0), mix(a.dir.1, b.dir.1)),
            ys: mix(a.ys, b.ys),
            thick: mix(a.thick, b.thick),
            width: mix(a.width, b.width),
            slope: mix(a.slope, b.slope),
        }
    }

    /// La forma del brazo en `q` (0 a 1), sin turbulencia: gaussiana de
    /// lado (la cuerda hasta el eje a ese radio, casi el arco) y de alto.
    #[inline]
    fn shape(&self, q: P2Vec) -> f32 {
        let r = q.x.hypot(q.z);
        if !(3.0..self.radius + 1.5).contains(&r) {
            return 0.0;
        }
        let track = self.track_at(r);
        let dy = (q.y - track.ys) / track.thick;
        if dy.abs() > 3.0 {
            return 0.0;
        }
        let (cx, cz) = (q.x - track.dir.0 * r, q.z - track.dir.1 * r);
        let d2 = (cx * cx + cz * cz) / (track.width * track.width);
        if d2 > 9.0 {
            return 0.0;
        }
        let u = ((self.radius - r) / (self.radius - 3.0)).clamp(0.0, 1.0);
        (-d2 - dy * dy).exp() * self.ends(r, u)
    }

    /// Lo que se desvanece en la punta, junto al agujero y, cuando ya no hay
    /// Pira, desde afuera hacia adentro.
    #[inline]
    fn ends(&self, r: f32, u: f32) -> f32 {
        let rp = self.radius;
        crate::math::smoothstep(rp + 1.5, rp - 0.5, r)
            * crate::math::smoothstep(3.0, 4.0, r)
            * crate::math::smoothstep(self.drained - 0.05, self.drained + 0.25, u)
    }

    /// Luz que emite y profundidad optica por `rs` de recorrido en `q`. El
    /// gas del brazo tiene la misma turbulencia que el del disco (`gas`): asi
    /// se ve hecho de lo mismo y, al ensancharse, se confunde con el.
    #[inline]
    fn sample(&self, q: P2Vec, gas: &GasTexture) -> Option<(Vec3, f32)> {
        let rp = self.radius;
        let r = q.x.hypot(q.z);
        if !(3.0..rp + 1.5).contains(&r) {
            return None;
        }
        let axis = self.axis(r);
        let dy = (q.y - axis.ys) / axis.thick;
        if dy.abs() > 3.0 {
            return None;
        }
        let angle = fast_atan2(q.z, q.x);
        let d = wrap(angle - axis.phi) * r / axis.width;
        if d.abs() > 3.0 {
            return None;
        }
        let u = axis.u;
        let shape = (-(d * d) - dy * dy).exp() * self.ends(r, u) * self.fade;
        if shape < 2e-3 {
            return None;
        }
        let density = shape * gas.turbulence_at(r, angle, q.y) / self.spread;
        Some((glow(u), density * 0.9))
    }
}

/// Luz del brazo a su avance `u`: se calienta al caer, del rojo (2400 K)
/// al blanco (7400 K), y brilla mas. De una tabla: el color de cuerpo negro
/// es caro para cada muestra.
#[inline]
fn glow(u: f32) -> Vec3 {
    const SIZE: usize = 64;
    static TABLE: std::sync::OnceLock<[Vec3; SIZE]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let u = i as f32 / (SIZE - 1) as f32;
            crate::math::blackbody::planckian_rgb(2400.0 + 5000.0 * u) * (1.6 + 3.0 * u)
        })
    });
    let x = u.clamp(0.0, 1.0) * (SIZE - 1) as f32;
    let i = (x as usize).min(SIZE - 2);
    table[i].lerp(table[i + 1], x - i as f32)
}

/// Un angulo llevado a `[-pi, pi)`.
#[inline]
fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[derive(Clone, Copy, Debug)]
pub struct Traced {
    pub captured: bool,
    pub sky_dir: Vec3,
    pub light: Vec3,
    pub transmittance: f32,
    pub depth: f32,
}

impl BlackHole<'_> {
    fn local(&self, v: Vec3) -> P2Vec {
        P2Vec::new(v.dot(self.axis_a), v.dot(self.normal), -v.dot(self.axis_b))
    }
    /// El rayo completo, sin la rejilla: la referencia de las pruebas.
    #[cfg(test)]
    pub fn trace(&self, dir: Vec3, limit: f32) -> Traced {
        let lens = self.lens(dir);
        let gas = (lens.gas && limit >= self.gas_begins()).then(|| self.gas(dir, limit));
        self.finish(&lens, gas)
    }

    /// Lo barato de un rayo: si cae, hacia donde sale al cielo (de la tabla)
    /// y si puede cruzar gas.
    fn lens(&self, dir: Vec3) -> Lens {
        let distance = self.center.length();
        let to_hole = self.center / distance;
        let cos_s = dir.dot(to_hole).clamp(-1.0, 1.0);
        let sin_s = dir.cross(to_hole).length();
        let s = fast_atan2(sin_s, cos_s);
        let e1 = -to_hole;
        let e2 = if sin_s > 1e-6 {
            (dir - to_hole * cos_s) / sin_s
        } else {
            let h = if to_hole.y.abs() < 0.9 {
                Vec3::Y
            } else {
                Vec3::X
            };
            h.cross(to_hole).normalize()
        };
        let mut lens = Lens {
            captured: false,
            sky_dir: dir,
            cos_s,
            gas: false,
        };
        let outside = self.density <= 0.0 || cos_s < self.cone_cos;
        if outside {
            if let Some((c, sn)) = self.table.far_turn(s) {
                lens.sky_dir = dir * c + (to_hole * sin_s - e2 * cos_s) * sn;
                return lens;
            }
        }
        if s > self.table.span() {
            let exit =
                sin_s.atan2(-cos_s * self.table.lapse) + weak_deflection(self.table.observer, s);
            lens.sky_dir = e1 * exit.cos() + e2 * exit.sin();
            return lens;
        }
        let ray = self.table.lookup(s);
        lens.captured = ray.path.captured;
        lens.sky_dir = e1 * ray.path.exit.cos() + e2 * ray.path.exit.sin();
        lens.gas = !outside;
        lens
    }

    /// Hasta donde hay gas, en `rs`: el disco o el brazo de Pira.
    fn extent(&self) -> f32 {
        self.stream.as_ref().map_or(self.outer, |s| self.outer.max(s.reach()))
    }

    /// Distancia a la camara desde la que puede haber gas.
    fn gas_begins(&self) -> f32 {
        (self.center.length() - self.extent() * self.rs).max(0.0)
    }

    /// Distancia a la camara pasada la cual ya no hay gas: un limite mas
    /// lejano da lo mismo que ninguno.
    fn gas_ends(&self) -> f32 {
        self.center.length() + (self.extent() * 1.01 + 1.0) * self.rs
    }

    /// Lo caro: el RK4 y la transferencia del Proyecto 2 a traves del gas,
    /// hasta la distancia `limit`.
    fn gas(&self, dir: Vec3, limit: f32) -> Gas {
        let mut gas = Gas::CLEAR;
        let origin = self.local(-self.center / self.rs);
        let mut photon = Photon::from_camera(origin, self.local(dir));
        let frame = RayFrame {
            observer_radius: origin.length(),
            lz_over_e: photon.lz_over_e,
        };
        let medium = Medium {
            peak_temperature: self.peak,
            ..Medium::VARIANT
        };
        let radial = RadialTable::shared();
        let extent = self.extent();
        // El origen puede estar muy lejos del sistema: los pasos exteriores
        // son mayores sin saltarse la envoltura de 19 rs.
        for _ in 0..p2::config::MAX_STEPS {
            if photon.captured() || photon.escaped() {
                break;
            }
            let previous = photon.pos;
            let r = previous.length();
            // Saliendo, fuera de la esfera de fotones, el radio solo crece:
            // pasada la envoltura ya no queda gas que cruzar, y la direccion
            // final al cielo ya salio de la tabla.
            if r > extent + 1.0 && previous.dot(photon.vel) > 0.0 {
                break;
            }
            let step = if r > 40.0 && previous.dot(photon.vel) < 0.0 {
                photon
                    .step_length()
                    .max((r - 35.0) * 0.1 / photon.vel.length().max(0.1))
            } else {
                p2::volume_step(&photon, medium)
            };
            photon.advance(step);
            let mid = (previous + photon.pos) * 0.5;
            let hit_distance = (mid - origin).length() * self.rs;
            if hit_distance >= limit {
                break;
            }
            if let Some(stream) = &self.stream {
                // El brazo es mas delgado que un paso lejos del agujero: se
                // muestrea en pedazos del paso (casi recto) para no dibujar
                // discos apilados.
                let length = (photon.pos - previous).length();
                let crossed = if length <= 0.25 {
                    stream.sample(mid, self.gas).map(|(e, d)| (e, d * length))
                } else {
                    stream.through(previous, photon.pos, self.gas)
                };
                if let Some((emission, depth)) = crossed {
                    let alpha = 1.0 - (-depth).exp();
                    gas.light += emission * (alpha * gas.transmittance);
                    let after = gas.transmittance * (1.0 - alpha);
                    if gas.transmittance > 0.5 && after <= 0.5 {
                        gas.depth = hit_distance;
                    }
                    gas.transmittance = after;
                }
            }
            if mid.x.hypot(mid.z) >= self.outer {
                continue;
            }
            if let Some(mut sample) = p2::disk::prepare_variant(mid, step, frame, medium, radial) {
                sample.optical_depth *= self.density;
                let (emission, alpha) = sample.shade(self.gas);
                gas.light +=
                    Vec3::new(emission.x, emission.y, emission.z) * (alpha * gas.transmittance);
                let after = gas.transmittance * (1.0 - alpha);
                if gas.transmittance > 0.5 && after <= 0.5 {
                    gas.depth = hit_distance;
                }
                gas.transmittance = after;
                if after < p2::config::MIN_TRANSMITTANCE {
                    break;
                }
            }
        }
        let peak = gas.light.max_element();
        if peak > 30.0 {
            gas.light = gas.light * (30.0 / peak);
        }
        gas
    }

    /// Junta la lente y el gas (si se calculo) de un rayo.
    fn finish(&self, lens: &Lens, gas: Option<Gas>) -> Traced {
        let mut traced = Traced {
            captured: lens.captured,
            sky_dir: lens.sky_dir,
            light: Vec3::ZERO,
            transmittance: 1.0,
            depth: f32::INFINITY,
        };
        if let Some(gas) = gas {
            traced.light = gas.light;
            traced.transmittance = gas.transmittance;
            traced.depth = gas.depth;
            // La sombra también da una profundidad a los volúmenes posteriores
            // (anillos y atmósferas): no pueden pintarse encima desde atrás.
            if traced.captured && lens.cos_s > 0.0 {
                traced.depth = traced.depth.min(self.center.length() / lens.cos_s);
            }
        }
        traced
    }

    /// Color de un pixel de cielo visto a traves del agujero. Las estrellas
    /// del fondo se ven en la direccion curvada; la corona de la estrella que
    /// colapsa esta junto al agujero, no detras, y no se curva.
    fn sky_from(&self, traced: Traced, dir: Vec3, u: &Uniforms) -> (Vec3, f32) {
        let background = if traced.captured {
            Vec3::ZERO
        } else {
            u.sky.sample(traced.sky_dir)
        };
        let glow = if u.star_glow > 0.0 {
            corona(dir, u) * u.star_glow
        } else {
            Vec3::ZERO
        };
        (
            traced.light + background * traced.transmittance + glow,
            traced.depth,
        )
    }

    /// Una superficie ya sombreada (color y distancia) vista con el agujero:
    /// si esta delante, solo la tapa el disco que este mas cerca; si esta
    /// detras y el rayo cae, la sombra la oculta. Lo que esta detras no se
    /// curva (una aproximacion): solo el cielo se curva.
    #[cfg(test)]
    pub fn surface(&self, dir: Vec3, color: Vec3, depth: f32) -> (Vec3, f32) {
        self.surface_with(dir, color, depth, |limit| self.trace(dir, limit))
    }

    fn surface_with(
        &self,
        dir: Vec3,
        color: Vec3,
        depth: f32,
        trace: impl FnOnce(f32) -> Traced,
    ) -> (Vec3, f32) {
        let distance = self.center.length();
        let cos_s = dir.dot(self.center) / distance;
        if cos_s < self.cone_cos {
            return (color, depth);
        }
        // Solo cuentan los cruces del disco que estan antes de la superficie;
        // si ademas la superficie esta detras del agujero y el rayo cae, la
        // sombra la oculta.
        let beyond = depth * cos_s > distance;
        let traced = trace(depth);
        let background = if beyond && traced.captured {
            Vec3::ZERO
        } else {
            color
        };
        (
            traced.light + background * traced.transmittance,
            depth.min(traced.depth),
        )
    }
}

/// Lo que la tabla dice de un rayo, sin integrar nada.
struct Lens {
    captured: bool,
    sky_dir: Vec3,
    cos_s: f32,
    /// Si el rayo entra al cono del disco y hay que seguirlo por el gas.
    gas: bool,
}

/// Lo que el gas le hace a un rayo: luz que suma, cuanto deja pasar y a que
/// distancia ya tapa la mitad.
#[derive(Clone, Copy, Debug)]
struct Gas {
    light: Vec3,
    transmittance: f32,
    depth: f32,
}

impl Gas {
    const CLEAR: Self = Self {
        light: Vec3::ZERO,
        transmittance: 1.0,
        depth: f32::INFINITY,
    };
}

/// Pixeles entre dos muestras del gas.
const GAS_STRIDE: usize = 2;
/// Cuanto pueden diferir cuatro muestras vecinas para interpolarlas: en
/// transmitancia y en luz (relativa a la mas brillante).
const GAS_SPREAD_TRANSMITTANCE: f32 = 0.15;
const GAS_SPREAD_LIGHT: f32 = 0.25;

/// Una muestra de la rejilla: el gas de su rayo y si ese rayo cae.
#[derive(Clone, Copy)]
struct GasSample {
    gas: Gas,
    captured: bool,
}

/// El gas del disco calculado cada `GAS_STRIDE` pixeles. Su textura es mas
/// gruesa que un pixel, asi que entre cuatro muestras parecidas se
/// interpola; en los bordes (la sombra, la orilla del disco, el anillo de
/// fotones) las muestras no se parecen y el pixel se traza completo. Los
/// pixeles que caen justo en una muestra la usan tal cual.
#[derive(Default)]
pub struct GasField {
    cols: usize,
    rows: usize,
    samples: Vec<GasSample>,
}

impl GasField {
    /// Traza el gas en los puntos de la rejilla. `direction` da el rayo
    /// (sin normalizar) del centro de un pixel.
    pub fn build(
        &mut self,
        hole: &BlackHole,
        width: usize,
        height: usize,
        direction: impl Fn(f32, f32) -> Vec3 + Sync,
    ) {
        self.cols = (width - 1).div_ceil(GAS_STRIDE) + 1;
        self.rows = (height - 1).div_ceil(GAS_STRIDE) + 1;
        let empty = GasSample {
            gas: Gas::CLEAR,
            captured: false,
        };
        self.samples.resize(self.cols * self.rows, empty);
        let cols = self.cols;
        crate::parallel::chunks_mut(&mut self.samples, cols, 1, |row, out| {
            let y = (row * GAS_STRIDE) as f32 + 0.5;
            for (k, sample) in out.iter_mut().enumerate() {
                let dir = direction((k * GAS_STRIDE) as f32 + 0.5, y).normalize();
                let lens = hole.lens(dir);
                *sample = GasSample {
                    gas: if lens.gas {
                        hole.gas(dir, f32::INFINITY)
                    } else {
                        Gas::CLEAR
                    },
                    captured: lens.captured,
                };
            }
        });
    }

    /// Como `BlackHole::trace` para el pixel `(px, py)`, con el gas de la
    /// rejilla donde se puede.
    fn trace(&self, hole: &BlackHole, dir: Vec3, limit: f32, px: f32, py: f32) -> Traced {
        let lens = hole.lens(dir);
        if !lens.gas || limit < hole.gas_begins() {
            return hole.finish(&lens, None);
        }
        // Una superficie metida en el gas corta el rayo a su distancia.
        if limit < hole.gas_ends() {
            return hole.finish(&lens, Some(hole.gas(dir, limit)));
        }
        let gas = self
            .interpolate(px, py)
            .unwrap_or_else(|| hole.gas(dir, f32::INFINITY));
        hole.finish(&lens, Some(gas))
    }

    fn interpolate(&self, px: f32, py: f32) -> Option<Gas> {
        let u = (px - 0.5) / GAS_STRIDE as f32;
        let v = (py - 0.5) / GAS_STRIDE as f32;
        let (i, j) = (u as usize, v as usize);
        if i + 1 >= self.cols || j + 1 >= self.rows {
            return None;
        }
        let (fx, fy) = (u - i as f32, v - j as f32);
        let at = |i: usize, j: usize| &self.samples[j * self.cols + i];
        if fx == 0.0 && fy == 0.0 {
            return Some(at(i, j).gas);
        }
        let corners = [at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1)];
        let captured = corners[0].captured;
        let finite = corners[0].gas.depth.is_finite();
        let (mut t_min, mut t_max) = (f32::MAX, f32::MIN);
        let (mut l_min, mut l_max) = (f32::MAX, f32::MIN);
        for c in &corners {
            if c.captured != captured || c.gas.depth.is_finite() != finite {
                return None;
            }
            t_min = t_min.min(c.gas.transmittance);
            t_max = t_max.max(c.gas.transmittance);
            let l = c.gas.light.max_element();
            l_min = l_min.min(l);
            l_max = l_max.max(l);
        }
        if t_max - t_min > GAS_SPREAD_TRANSMITTANCE
            || l_max - l_min > GAS_SPREAD_LIGHT * l_max + 0.002
        {
            return None;
        }
        let w = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
        let mut gas = Gas {
            light: Vec3::ZERO,
            transmittance: 0.0,
            depth: if finite { 0.0 } else { f32::INFINITY },
        };
        for (c, w) in corners.iter().zip(w) {
            gas.light += c.gas.light * w;
            gas.transmittance += c.gas.transmittance * w;
            if finite {
                gas.depth += c.gas.depth * w;
            }
        }
        Some(gas)
    }

    /// `BlackHole::sky` con el gas de la rejilla.
    pub fn sky(&self, hole: &BlackHole, dir: Vec3, u: &Uniforms, px: f32, py: f32) -> (Vec3, f32) {
        hole.sky_from(self.trace(hole, dir, f32::INFINITY, px, py), dir, u)
    }

    /// `BlackHole::surface` con el gas de la rejilla.
    pub fn surface(
        &self,
        hole: &BlackHole,
        dir: Vec3,
        color: Vec3,
        depth: f32,
        px: f32,
        py: f32,
    ) -> (Vec3, f32) {
        hole.surface_with(dir, color, depth, |limit| {
            self.trace(hole, dir, limit, px, py)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_gas() -> &'static GasTexture {
        static GAS: std::sync::OnceLock<GasTexture> = std::sync::OnceLock::new();
        GAS.get_or_init(|| {
            let mut gas = GasTexture::new();
            gas.update_preview(
                &crate::gargantua_p2::math::noise::NoiseTable::new(
                    crate::gargantua_p2::config::NOISE_SEED,
                ),
                0.0,
            );
            gas
        })
    }

    fn hole<'a>(table: &'a LensTable, center: Vec3) -> BlackHole<'a> {
        let normal = Vec3::Y;
        BlackHole {
            center,
            rs: 1.0,
            normal,
            axis_a: Vec3::Z,
            axis_b: Vec3::X,
            table,
            gas: test_gas(),
            density: 1.0,
            outer: 19.0,
            peak: 6500.0,
            stream: None,
            cone_cos: table.disk_cone(19.0).cos(),
        }
    }

    #[test]
    fn looking_at_the_hole_is_dark_and_the_disk_shines_beside_it() {
        // Camara a 40 rs, un poco por encima del plano del disco.
        let center = Vec3::new(0.0, -4.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let at_hole = bh.trace(center.normalize(), f32::INFINITY);
        assert!(at_hole.captured);
        // Un rayo que apunta al disco a 8 rs del centro, del lado de enfrente.
        let target = Vec3::new(8.0, 0.0, 0.0) + center;
        let hit = bh.trace(target.normalize(), f32::INFINITY);
        assert!(!hit.captured);
        assert!(
            hit.light.x + hit.light.y + hit.light.z > 0.005,
            "{:?}",
            hit.light
        );
        assert!(hit.depth < center.length() + 10.0);
    }

    #[test]
    fn the_side_coming_toward_the_camera_is_bluer_and_brighter() {
        let center = Vec3::new(0.0, -3.0, -60.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        // El gas gira a derechas alrededor de +Y: en +X va hacia -Z (hacia
        // la camara, que esta en -Z respecto al agujero... en +Z del hoyo).
        let left = bh.trace(
            (center + Vec3::new(-7.0, 0.0, 0.0)).normalize(),
            f32::INFINITY,
        );
        let right = bh.trace(
            (center + Vec3::new(7.0, 0.0, 0.0)).normalize(),
            f32::INFINITY,
        );
        let ratio = |c: Vec3| c.z / c.x.max(1e-6);
        let (bright, dim) = if left.light.x + left.light.y + left.light.z
            > right.light.x + right.light.y + right.light.z
        {
            (left, right)
        } else {
            (right, left)
        };
        assert!(
            ratio(bright.light) > ratio(dim.light),
            "el lado brillante debe ser el mas azul"
        );
    }

    #[test]
    fn a_planet_in_front_of_the_hole_is_not_covered_by_the_disk_behind_it() {
        let center = Vec3::new(0.0, -4.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let dir = (center + Vec3::new(8.0, 0.0, 0.0)).normalize();
        let planet = Vec3::new(0.2, 0.5, 0.1);
        let (color, depth) = bh.surface(dir, planet, 10.0);
        assert_eq!(color, planet);
        assert_eq!(depth, 10.0);
        // La misma superficie detras del agujero queda tapada por el disco.
        let (behind, _) = bh.surface(dir, planet, 200.0);
        assert!(behind != planet);
    }

    #[test]
    fn the_captured_sky_has_depth_to_occlude_distant_rings() {
        let center = Vec3::new(0.0, -2.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let ray = bh.trace(center.normalize(), f32::INFINITY);
        assert!(ray.captured);
        assert!(ray.depth.is_finite() && ray.depth <= center.length() + 0.1);
        assert!(ray.depth > 10.0);
    }

    #[test]
    fn the_gas_grid_matches_full_tracing() {
        // Una vista de 160x90 que encuadra el disco desde 40 rs.
        let center = Vec3::new(0.0, -2.0, -40.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let (w, h) = (160usize, 90usize);
        let forward = center.normalize();
        let right = Vec3::Y.cross(forward).normalize() * 0.6;
        let up = forward.cross(right).normalize() * 0.6 * h as f32 / w as f32;
        let direction = |x: f32, y: f32| {
            forward + right * (x / w as f32 * 2.0 - 1.0) + up * (1.0 - y / h as f32 * 2.0)
        };
        let mut field = GasField::default();
        field.build(&bh, w, h, direction);
        let (mut error, mut light) = (0.0, 0.0);
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let dir = direction(px, py).normalize();
                let exact = bh.trace(dir, f32::INFINITY);
                let grid = field.trace(&bh, dir, f32::INFINITY, px, py);
                assert_eq!(exact.captured, grid.captured);
                if x % GAS_STRIDE == 0 && y % GAS_STRIDE == 0 {
                    // Sobre una muestra, la rejilla es el rayo mismo.
                    assert_eq!(exact.light, grid.light);
                    assert_eq!(exact.transmittance, grid.transmittance);
                }
                error += (exact.light - grid.light).abs().max_element();
                light += exact.light.max_element();
            }
        }
        assert!(light > 1.0, "la vista debe ver el disco");
        assert!(error / light < 0.03, "error relativo {}", error / light);
    }

    /// Costo de un pixel segun donde cae. Correr con
    /// `cargo test --release -- --ignored --nocapture medir_gargantua`.
    #[test]
    #[ignore]
    fn medir_gargantua() {
        let center = Vec3::new(0.0, -2.0, -29.0);
        let table = LensTable::build(center.length());
        let bh = hole(&table, center);
        let axis = center.normalize();
        let (a, b) = {
            let helper = Vec3::Y;
            let a = helper.cross(axis).normalize();
            (a, axis.cross(a))
        };
        let n = 400_000;
        let time = |label: &str, spread: f32| {
            let dirs: Vec<Vec3> = (0..n)
                .map(|i| {
                    let angle = i as f32 * 2.399;
                    let radius = spread * ((i % 997) as f32 / 997.0 * 0.5 + 0.5);
                    (axis + (a * angle.cos() + b * angle.sin()) * radius).normalize()
                })
                .collect();
            let start = std::time::Instant::now();
            let mut acc = Vec3::ZERO;
            for d in &dirs {
                let t = bh.trace(*d, f32::INFINITY);
                acc += t.light + t.sky_dir;
            }
            std::hint::black_box(acc);
            println!(
                "{label:<22} {:6.1} ns por pixel",
                start.elapsed().as_secs_f64() * 1e9 / n as f64
            );
        };
        time("cerca (disco)", 0.4);
        time("cono sin disco", 0.75);
        time("tabla lejos", 1.6);
        time("campo debil", 6.0);
    }
}
