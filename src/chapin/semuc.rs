//! Semuc Champey (la version chapina de Parpado): pozas turquesa en
//! terrazas de caliza, en el fondo de un canon entre cerros de selva. Le da
//! siempre la misma cara (-X) a Atitlan, que se levanta al final del canon y
//! se refleja en el agua.
//!
//! - **Karst de conos**: los cerros son ruido celular (Worley). Cada celda
//!   levanta un cerro redondo alrededor de su punto, `(1 - r^2)^1.5`: cima
//!   redonda, ladera empinada y sin pared vertical al llegar al suelo, como
//!   los cerros de Alta Verapaz. Debajo hay una meseta de caliza.
//! - **Los rios**: el principal es un gran circulo que pasa por el lugar, con
//!   meandros de ruido (una funcion periodica del angulo a lo largo del rio,
//!   corregida para que en el lugar vaya derecho hacia Atitlan). Los demas
//!   salen de donde un ruido vale cero; la distancia al cauce es el ruido
//!   entre su pendiente, y un paso de Newton lleva el punto al centro del
//!   cauce para que todo el ancho del rio tenga el mismo nivel. La distancia
//!   al cauce talla el canon.
//! - **Pozas escalonadas**: todo el planeta esta inclinado, `P = -H dir.F`:
//!   el agua baja hacia Atitlan. El nivel del agua es `P` redondeado hacia
//!   abajo en escalones; dentro de cada escalon el piso hace la poza: un
//!   borde de travertino que asoma sobre el agua rio abajo, un cuenco y, rio
//!   arriba, la pared por donde cae la poza siguiente. El nivel se tuerce con
//!   ruido y se curva en las orillas, asi cada borde se abomba rio abajo como
//!   los diques de travertino. El agua es plana: la altura es
//!   `max(piso, nivel)`, y lo que se ve es lo que se choca.
//! - **Agua clara**: el fondo se ve a traves del agua por el rayo refractado
//!   (Snell). La luz que atraviesa el agua se apaga por Beer-Lambert, mucho
//!   mas el rojo que el verde y el azul: poco hondo se ve la caliza turquesa
//!   palida, hondo el verde esmeralda de lo que dispersa el agua. En el fondo
//!   bailan las causticas (crestas de dos ruidos que se mueven). Encima,
//!   Fresnel de Schlick entre lo que se ve adentro y el reflejo del cielo y
//!   de Atitlan (el rayo reflejado contra su esfera, con su propio shader), y
//!   los destellos del sol en los rizos.
//! - **Cascadas**: en la pared entre dos pozas cae agua blanca: vetas de
//!   ruido estiradas en vertical que bajan con el tiempo; abajo, espuma.
//! - **Selva**: copas de arboles con Worley propio (cada punto es una copa:
//!   la normal sale del centro de la copa, asi cada una es una cupula, y cada
//!   una tiene su verde; unas pocas estan en flor). En los acantilados, la
//!   caliza gris con chorreones; junto al agua, travertino crema, mojado y
//!   con musgo.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use super::{Chapin, Placer, Spec};
use crate::math::{DVec3, Perlin, Vec2, Vec3, Vec4, hash_u32, smoothstep};
use crate::scene::builder::perpendiculars;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter};
use crate::shaders::{AtmosphereParams, Fragment, Uniforms, detail};
use crate::terrain::{HeightField, TerrainSample};

/// Radio para el que esta pensado el relieve: pasa angulos a unidades del
/// mundo.
const RADIUS: f32 = 200.0;
/// El lugar, en el espacio de la luna. Atitlan esta siempre en -X: desde
/// aqui queda a unos 17 grados sobre el horizonte, al final del canon, y en
/// la vista fija (t = 40) el sol alumbra de lado, a unos 40 grados.
const POZAS: Vec3 = Vec3::new(-0.3, 0.749, -0.591);
/// Desnivel de todo el planeta: el agua baja hacia Atitlan.
const TILT: f32 = 18.0;
/// Caida de una poza a la siguiente, hondura del agua y lo que el borde
/// asoma sobre ella.
const STEP: f32 = 1.4;
const DEPTH: f32 = 2.4;
const LIP: f32 = 0.3;
/// Meseta de caliza sobre el rio y altura de los cerros.
const PLATEAU: f32 = 10.0;
const CONE: f32 = 24.0;
/// Cerros por radio y frecuencia de la red de rios.
const CONES: f32 = 6.5;
const NETWORK: f32 = 1.6;

/// Ejes del lugar: vertical, rio abajo (hacia Atitlan) y derecha.
fn axes() -> (Vec3, Vec3, Vec3) {
    let up = POZAS.normalize();
    let toward = Vec3::new(-1.0, 0.0, 0.0);
    let flow = (toward - up * toward.dot(up)).normalize();
    (up, flow, flow.cross(up))
}

pub fn spec() -> Spec {
    Spec {
        label: "Semuc Champey",
        material: Chapin::Semuc,
        // Bruma humeda de selva: azul verdosa y baja, se junta en el canon.
        atmosphere: Some(AtmosphereParams {
            thickness: 40.0,
            scale_height: 8.0,
            rayleigh: Vec3::new(0.0012, 0.0030, 0.0046),
            mie: 0.0025,
            mie_g: 0.75,
            intensity: 1.4,
        }),
        rings: None,
        motes: Some((Motes::Spray, 90.0)),
    }
}

pub fn field(seed: u32) -> Arc<dyn HeightField> {
    Arc::new(SemucField::new(seed))
}

/// El relieve de Semuc Champey.
///
/// Atributos: `x` hondura del agua (negativa fuera de ella: cuanto asoma la
/// roca sobre el rio mas cercano), `y` fondo del canon (1 en el piso, 0
/// fuera), `z` cascada (1 en la pared por donde cae el agua).
pub struct SemucField {
    noise: Perlin,
    up: Vec3,
    flow: Vec3,
    right: Vec3,
    /// El meandro del rio principal en el lugar y su pendiente: se restan
    /// para que el rio pase justo por el lugar, derecho hacia Atitlan.
    bend: (f32, f32),
}

/// Lo que dice un rio de un punto: el piso, el nivel de su agua y cuanto
/// es canon y cascada.
struct Reach {
    floor: f32,
    water: f32,
    gorge: f32,
    fall: f32,
}

impl SemucField {
    pub fn new(seed: u32) -> Self {
        let (up, flow, right) = axes();
        let mut field = Self {
            noise: Perlin::new(seed),
            up,
            flow,
            right,
            bend: (0.0, 0.0),
        };
        let e = 1e-3;
        let at = |angle: f32| field.meander(angle.sin(), angle.cos());
        field.bend = (at(0.0), (at(e) - at(-e)) / (2.0 * e));
        field
    }

    /// Cuanto se corre el rio principal a un lado, en el angulo a lo largo
    /// del rio con seno `s` y coseno `c`: una funcion periodica, sin salto.
    fn meander(&self, s: f32, c: f32) -> f32 {
        let n = &self.noise;
        26.0 * n.noise(Vec3::new(c * 1.6, s * 1.6, 4.3)) + 9.0 * n.noise(Vec3::new(c * 4.5, s * 4.5, -2.1))
    }

    /// La meseta y los cerros, sobre la inclinacion del planeta.
    fn land(&self, dir: Vec3) -> f32 {
        let n = &self.noise;
        let (f1, _) = n.cells(dir * CONES + Vec3::new(3.7, -1.2, 5.1));
        let c = (1.0 - (f1 / 0.85).powi(2)).max(0.0);
        let tall = 0.6 + 0.4 * n.noise(dir * 3.1 + Vec3::new(-7.3, 2.2, 0.9));
        -TILT * dir.dot(self.flow) + PLATEAU + CONE * c * c.sqrt() * tall + 1.2 * n.fbm(dir * 11.0, 3)
    }

    /// El rio principal: distancia al cauce (con signo, unidades del mundo),
    /// nivel en el centro del cauce y medio ancho.
    fn main_river(&self, dir: Vec3) -> (f32, f32, f32) {
        let a = dir.dot(self.flow);
        let b = dir.dot(self.up);
        let r = (a * a + b * b).sqrt().max(1e-6);
        let (s, c) = (a / r, b / r);
        let shift = self.meander(s, c) - self.bend.0 - self.bend.1 * s;
        let across = dir.dot(self.right) * RADIUS - shift;
        let half = 16.0 + 5.0 * self.noise.noise(Vec3::new(c * 3.0, s * 3.0, 8.1));
        (across, -TILT * s, half)
    }

    /// Los otros rios: distancia al cauce donde un ruido vale cero y nivel
    /// en el centro del cauce. Cerca del lugar no hay: ahi manda el principal.
    fn network(&self, dir: Vec3) -> (f32, f32) {
        let n = &self.noise;
        let q = dir * NETWORK + Vec3::new(9.1, -3.3, 6.6);
        let v = n.noise(q);
        // Pendiente del ruido sobre la superficie, por radian.
        let (t1, t2) = perpendiculars(dir);
        let e = 0.01;
        let g1 = (n.noise(q + t1 * (NETWORK * e)) - v) / e;
        let g2 = (n.noise(q + t2 * (NETWORK * e)) - v) / e;
        let g = (g1 * g1 + g2 * g2).max(1e-4);
        let near_site = smoothstep(0.88, 0.93, dir.dot(self.up));
        let distance = v.abs() / g.sqrt() * RADIUS + 200.0 * near_site;
        // Un paso de Newton hacia el cero del ruido: el centro del cauce.
        let center = dir - (t1 * g1 + t2 * g2) * (v / g).clamp(-0.2, 0.2);
        (distance, -TILT * center.dot(self.flow))
    }

    /// El piso de la poza en `dir` y el nivel de su agua. `level` es el
    /// nivel del rio en el centro del cauce (baja rio abajo) y `x` la
    /// distancia al centro en medios anchos.
    fn pool(&self, dir: Vec3, level: f32, x: f32) -> (f32, f32, f32) {
        let n = &self.noise;
        // Bordes irregulares que se abomban rio abajo.
        let wobble = n.noise(dir * 26.0 + Vec3::new(1.3, 4.4, -2.7));
        let p = level - 0.5 * x * x + 0.45 * STEP * wobble;
        // Pozas de largo distinto: una deformacion del nivel que nunca deja
        // de subir.
        let p = p + 0.3 * STEP * (p * TAU / (2.7 * STEP)).sin();
        let k = (p / STEP).floor();
        let t = p / STEP - k;
        let water = k * STEP;
        let deep = DEPTH * smoothstep(1.05, 0.45, x);
        let lip = LIP * smoothstep(0.12, 0.0, t);
        let bowl = deep * smoothstep(0.0, 0.3, t);
        // Rio arriba, la pared sube hasta el borde de la poza siguiente.
        let wall = (STEP + LIP + deep) * smoothstep(0.93, 1.0, t);
        let fall = smoothstep(0.75, 0.97, t) * smoothstep(1.0, 0.99, t) * smoothstep(1.0, 0.6, x);
        (water + lip - bowl + wall, water, fall)
    }

    /// Un rio a `across` del cauce, con su canon de medio ancho `half` y
    /// paredes de `wall`.
    fn reach(&self, dir: Vec3, land: f32, across: f32, level: f32, half: f32, wall: f32) -> Option<Reach> {
        let d = across.abs();
        if d > half + wall {
            return None;
        }
        // La pared sale empinada del agua y se redondea arriba.
        let s = ((d - half) / wall).clamp(0.0, 1.0);
        let gorge = (1.0 - s) * (1.0 - s);
        let (floor, water, fall) = self.pool(dir, level, d / half);
        Some(Reach {
            floor: land + (floor - land) * gorge,
            water,
            gorge,
            fall: fall * gorge,
        })
    }
}

/// La hondura que se guarda lejos del agua (la roca asoma de mas).
const DRY: f32 = -8.0;
/// Lo mas bajo y lo mas alto que puede quedar el relieve.
const LOWEST: f32 = -TILT - STEP * 2.3 - 0.5 - DEPTH;
const HIGHEST: f32 = TILT + PLATEAU + CONE * 1.1 + 1.2;

impl HeightField for SemucField {
    fn sample(&self, dir: Vec3) -> TerrainSample {
        let land = self.land(dir);
        let (across, level, half) = self.main_river(dir);
        let main = self.reach(dir, land, across, level, half, 14.0);
        let (distance, level) = self.network(dir);
        let other = self.reach(dir, land, distance, level, 7.0, 9.0);
        // Donde se juntan dos rios manda el canon mas hondo.
        let river = match (main, other) {
            (Some(a), Some(b)) => Some(if b.floor < a.floor { b } else { a }),
            (a, b) => a.or(b),
        };
        let Some(r) = river else {
            return TerrainSample {
                height: land.clamp(LOWEST, HIGHEST),
                attrs: Vec3::new(DRY, 0.0, 0.0),
            };
        };
        TerrainSample {
            height: r.floor.max(r.water).clamp(LOWEST, HIGHEST),
            attrs: Vec3::new((r.water - r.floor).clamp(DRY, DEPTH + LIP), r.gorge, r.fall),
        }
    }

    fn height_range(&self) -> (f32, f32) {
        (LOWEST, HIGHEST)
    }
}

// Colores (lineales).
const TRAVERTINE: Vec3 = Vec3::new(0.62, 0.57, 0.45);
const LIMESTONE: Vec3 = Vec3::new(0.36, 0.36, 0.34);
const ALGAE: Vec3 = Vec3::new(0.16, 0.2, 0.07);
const CANOPY: Vec3 = Vec3::new(0.012, 0.05, 0.022);
const CANOPY_LIGHT: Vec3 = Vec3::new(0.05, 0.14, 0.04);
/// Copas en flor: el rosa lila del matilisguate.
const BLOSSOM: Vec3 = Vec3::new(0.42, 0.2, 0.3);
const FOAM: Vec3 = Vec3::new(0.85, 0.9, 0.88);
/// Lo que el agua dispersa (el turquesa lechoso del carbonato) y cuanto
/// apaga cada color por unidad de camino.
const SCATTER: Vec3 = Vec3::new(0.02, 0.32, 0.26);
const ABSORB: Vec3 = Vec3::new(0.6, 0.1, 0.13);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (depth, gorge, fall) = (f.color.x, f.color.y, f.color.z);
    let footprint = f.pos.length() * u.pixel_angle;
    // La orilla se mezcla en una franja del ancho de un pixel: sin escalones.
    let band = (footprint * 1.5).max(0.02);
    let wet = smoothstep(-band, band, depth);
    if wet >= 1.0 {
        return water(f, u, depth, fall, footprint);
    }
    let dry = ground(f, u, depth, gorge, fall, footprint);
    if wet <= 0.0 {
        return dry;
    }
    dry.lerp(water(f, u, depth.max(0.0), fall, footprint), wet)
}

/// Sombra y luz del sol que llega a un punto con normal `n`.
fn sunlight(f: &Fragment, u: &Uniforms, n: Vec3) -> Vec3 {
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, n));
    u.sun_light * (n.dot(f.light_dir(u)).max(0.0) * shadow * 3.6 / PI)
}

/// Del espacio de la luna al mundo.
#[inline]
fn to_world(f: &Fragment, v: Vec3) -> Vec3 {
    f.object_to_world.mul_vec(v)
}

/// Del mundo al espacio de la luna (la transpuesta de la rotacion).
#[inline]
fn to_local(f: &Fragment, v: Vec3) -> Vec3 {
    let m = &f.object_to_world.m;
    Vec3::new(
        m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
        m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
        m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
    )
}

/// Agua clara: el fondo a traves del agua, el reflejo y los destellos.
fn water(f: &Fragment, u: &Uniforms, depth: f32, fall: f32, footprint: f32) -> Vec3 {
    let t = u.time;
    let p = f.local;
    let up = p.normalize();
    let (ta, tb) = perpendiculars(up);
    // Rizos: dos capas de ruido que corren; se apagan donde un pixel cubre
    // varias ondas (si no, brillan y parpadean).
    let calm = smoothstep(0.5, 0.08, footprint);
    let n = u.noise;
    let wa = n.noise(p * 1.1 + Vec3::new(t * 0.3, 0.0, t * 0.2));
    let wb = n.noise(p * 1.1 + Vec3::new(5.2, t * 0.25, -3.1));
    let fa = n.noise(p * 3.7 + Vec3::new(-t * 0.6, 1.7, t * 0.4)) * calm;
    let fb = n.noise(p * 3.7 + Vec3::new(2.9, -t * 0.5, 7.3)) * calm;
    let rough = 0.05 + 0.3 * fall;
    let tilt = (ta * (wa + 0.6 * fa) + tb * (wb + 0.6 * fb)) * ((0.05 + 0.25 * fall) * (0.4 + 0.6 * calm));
    let normal = to_world(f, (up + tilt).normalize());
    let v = f.view_dir();
    let ndv = normal.dot(v).max(1e-3);
    let fresnel = 0.02 + 0.98 * (1.0 - ndv).powi(5);
    let reflected = environment(f, normal * (2.0 * ndv) - v, u);

    // El rayo refractado (Snell, del aire al agua) llega al fondo.
    let eta = 1.0 / 1.33;
    let k = 1.0 - eta * eta * (1.0 - ndv * ndv);
    let refracted = -v * eta + normal * (eta * ndv - k.max(0.0).sqrt());
    let down = -f.normal;
    let path = depth / refracted.dot(down).max(0.25);
    let bottom = p + to_local(f, refracted) * path;
    // La luz del sol tambien cruza el agua hasta el fondo.
    let l = f.light_dir(u);
    let sun_path = depth / l.dot(f.normal).max(0.3);
    let sun = sunlight(f, u, f.normal);
    // El ojo se acostumbra: bajo el agua la luz roja casi no cuenta y el
    // turquesa se apagaria; se mezcla con su luminancia.
    let light = sun.lerp(Vec3::splat(sun.luminance()), 0.55);
    let ambient = Vec3::new(0.05, 0.075, 0.09) + u.sun_light * 0.012;
    let caustic = caustics(bottom, t, n, footprint) * u.gains.vigia;
    let floor = TRAVERTINE * (0.7 + 0.45 * n.fbm(bottom * 0.7, 2));
    let through_sun = exp3(ABSORB * -sun_path);
    let lit_floor = floor * (light * through_sun * (0.75 + 1.4 * caustic) + ambient);
    let through = exp3(ABSORB * -path);
    let glow = SCATTER * (light * 0.45 + ambient);
    let inside = lit_floor * through + glow * (Vec3::ONE - through);
    // Espuma donde cae la cascada: manchas que se abren y se van.
    let churn = n.noise(p * 1.8 + up * (t * 1.5)) * 0.5 + 0.5;
    let foam = fall * smoothstep(0.35, 0.75, churn + fall * 0.4);
    let body = inside * (1.0 - fresnel) + reflected * fresnel;
    let lit_foam = FOAM * (sun + ambient);
    body.lerp(lit_foam, foam) + glint(f, u, normal, rough + footprint * 0.5) * (1.0 - foam)
}

/// Destello del sol en el agua: Blinn-Phong normalizado, mas ancho cuando el
/// pixel cubre muchos rizos.
fn glint(f: &Fragment, u: &Uniforms, n: Vec3, rough: f32) -> Vec3 {
    let l = f.light_dir(u);
    let h = (l + f.view_dir()).normalize();
    let shine = 2.0 / (rough * rough).max(1e-3);
    let nl = n.dot(l).max(0.0);
    let spec = (shine + 8.0) / (8.0 * PI) * n.dot(h).max(0.0).powf(shine);
    let shadow = u.shadow.map_or(1.0, |s| s.visibility(f.pos, f.normal));
    u.sun_light * (spec * nl * shadow * 0.05 * u.gains.vigia)
}

/// `e^v` por canal.
#[inline]
fn exp3(v: Vec3) -> Vec3 {
    Vec3::new(v.x.exp(), v.y.exp(), v.z.exp())
}

/// Causticas: las crestas de dos ruidos que se mueven forman la red de luz
/// que el agua enfoca en el fondo. De lejos, su promedio.
fn caustics(p: Vec3, t: f32, n: &Perlin, footprint: f32) -> f32 {
    let fade = smoothstep(0.25, 0.06, footprint);
    if fade <= 0.0 {
        return 0.3;
    }
    let warp = n.noise(p * 0.45 + Vec3::new(0.0, t * 0.2, 0.0));
    let a = 1.0 - n.noise(p * 1.6 + Vec3::new(t * 0.35, warp, 0.0)).abs();
    let b = 1.0 - n.noise(p * 2.1 + Vec3::new(warp, -t * 0.3, 3.3)).abs();
    let lines = a.powi(8) + b.powi(8);
    0.3 + (lines - 0.3) * fade
}

/// Lo que se ve desde `f` en la direccion `dir`: Atitlan si el rayo le pega
/// (con su propio shader), si no el cielo.
fn environment(f: &Fragment, dir: Vec3, u: &Uniforms) -> Vec3 {
    let sky = {
        let stars = u.sky.sample(dir);
        match u.sky_view {
            Some(view) => view.sample(dir).apply(stars),
            None => stars,
        }
    };
    let Some(giant) = &u.giant else { return sky };
    let oc = f.pos - giant.center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - giant.radius * giant.radius;
    let disc = b * b - c;
    if disc <= 0.0 {
        return sky;
    }
    let t = -b - disc.sqrt();
    if t <= 0.0 {
        return sky;
    }
    let hit = f.pos + dir * t;
    let surface = Fragment {
        ao: 1.0,
        object_to_world: giant.to_world,
        pos: hit,
        normal: (hit - giant.center) / giant.radius,
        local: giant.to_local.mul_vec(hit - giant.center),
        uv: Vec2::ZERO,
        color: Vec3::ZERO,
        tangent: Vec4::new(1.0, 0.0, 0.0, 1.0),
    };
    super::atitlan::shade(&surface, u)
}

/// Roca, travertino y selva.
fn ground(f: &Fragment, u: &Uniforms, depth: f32, gorge: f32, fall: f32, footprint: f32) -> Vec3 {
    let n = u.noise;
    let p = f.local;
    let up = p.normalize();
    let slope = f.uv.y;
    let above = -depth;
    let ragged = n.noise(p * 0.35) * 0.5;
    // Travertino junto al agua del canon, caliza gris en los acantilados y
    // selva en todo lo demas.
    let travertine = gorge * smoothstep(2.6, 1.2, above + ragged);
    let cliff = smoothstep(0.38, 0.6, slope + ragged * 0.2) * (1.0 - travertine);
    let jungle = (1.0 - travertine - cliff).max(0.0);
    let mut color = Vec3::ZERO;
    if travertine + cliff > 0.01 {
        let rock = detail::rock(f, u, 0.2, footprint, 0.8);
        let grain = 0.55 + 0.9 * rock.color.luminance() / 0.155;
        // Chorreones oscuros que bajan por la caliza: ruido estirado en vertical.
        let streaks = smoothstep(0.1, 0.6, n.noise(p * 0.9 - up * (p.dot(up) * 0.8)));
        let stone = LIMESTONE * (1.0 - 0.55 * streaks);
        // Junto al agua: mojado, mas oscuro y brillante, y con algas.
        let wetness = smoothstep(0.9, 0.0, above);
        let algae = smoothstep(1.6, 0.6, above) * (0.5 + 0.5 * n.noise(p * 1.3));
        let shore = TRAVERTINE.lerp(ALGAE, algae * 0.5) * (1.0 - 0.35 * wetness);
        let albedo = stone.lerp(shore, travertine / (travertine + cliff)) * grain;
        let rough = 0.75 - 0.55 * wetness * travertine;
        let lit = crate::shaders::pbr::shade(f, u, albedo, rough, 0.0, rock.normal, rock.ao);
        // Agua blanca que baja por la pared de la cascada.
        let falling = n.noise(p * 2.6 - up * (p.dot(up) * 1.9) + up * (u.time * 5.0)) * 0.5 + 0.5;
        let white = fall * smoothstep(0.3, 0.6, falling + fall * 0.3);
        let foam = FOAM * (sunlight(f, u, f.normal) + Vec3::splat(0.06));
        color += lit.lerp(foam, white) * (travertine + cliff);
    }
    if jungle > 0.01 {
        color += canopy(f, u, footprint) * jungle;
    }
    color
}

/// Las copas de la selva: un Worley propio que devuelve el punto mas
/// cercano, para que cada copa sea una cupula con su color.
fn canopy(f: &Fragment, u: &Uniforms, footprint: f32) -> Vec3 {
    const CROWN: f32 = 4.0;
    // De lejos, cuando una copa mide pocos pixeles, queda su promedio.
    let near = smoothstep(0.45, 0.15, footprint / CROWN);
    let (offset, edge, id) = if near > 0.0 {
        nearest(f.local / CROWN)
    } else {
        (Vec3::ZERO, 1.0, 0)
    };
    // La cupula: la normal del suelo inclinada hacia afuera del centro.
    let up = f.local.normalize();
    let side = to_world(f, offset - up * offset.dot(up));
    let leaves = detail::sample(f, &u.textures.organic, 0.3, footprint, 0.45);
    let normal = (leaves.normal + side * (1.5 * near)).normalize();
    // Cada copa con su verde y, de vez en cuando, una en flor.
    let tone = (id >> 8 & 0xFF) as f32 / 255.0;
    let patch = u.noise.fbm(f.local * 0.04, 2) * 0.5 + 0.5;
    let mut leaf = CANOPY.lerp(CANOPY_LIGHT, (0.25 + (tone - 0.5) * near * 0.8 + (patch - 0.5) * 0.9).clamp(0.0, 1.0));
    if id & 0xFF < 5 {
        // Solo en lo alto de la copa: la orilla queda verde.
        leaf = leaf.lerp(BLOSSOM, near * near * 0.7);
    }
    // Sombra entre copas y entre las hojas.
    let gap = (1.0 - near * smoothstep(0.35, 0.0, edge) * 0.85) * (0.55 + 0.45 * leaves.height);
    crate::shaders::pbr::shade(f, u, leaf * (0.8 + 0.4 * leaves.height), 0.85, 0.0, normal, gap)
}

/// El punto de Worley mas cercano a `p` (vector desde el punto), la
/// distancia al borde con la celda vecina (`f2 - f1`) y el hash de su celda.
fn nearest(p: Vec3) -> (Vec3, f32, u32) {
    let base = p.floor();
    let mut best = (Vec3::ZERO, f32::MAX, 0);
    let mut second = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                let h = hash_u32(
                    (cell.x as i32 as u32).wrapping_mul(0x8DA6_B343)
                        ^ (cell.y as i32 as u32).wrapping_mul(0xD816_3841)
                        ^ (cell.z as i32 as u32).wrapping_mul(0xCB1A_B31F),
                );
                let h2 = hash_u32(h);
                let h3 = hash_u32(h2);
                let point = cell + Vec3::new(unit(h), unit(h2), unit(h3));
                let delta = p - point;
                let d = delta.length_squared();
                if d < best.1 {
                    second = best.1;
                    best = (delta, d, h3);
                } else if d < second {
                    second = d;
                }
            }
        }
    }
    (best.0, second.sqrt() - best.1.sqrt(), best.2)
}

#[inline]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

pub const DEFORMS: bool = false;

pub fn displace(_local: Vec3, _normal: Vec3, _time: f32, _noise: &Perlin) -> Vec3 {
    Vec3::ZERO
}

/// Selva en los cerros: arboles de Peten en las laderas, nunca en el agua
/// ni en los acantilados.
pub fn rules() -> Vec<Scatter> {
    vec![Scatter {
        kind: PropKind::Arbol,
        cells: 40,
        scale: (0.8, 1.6),
        sink: 0.4,
        upright: true,
        accept: |density, s, slope| {
            smoothstep(0.9, 0.4, slope) * smoothstep(-1.5, -2.8, s.attrs.x) * smoothstep(0.45, 0.7, density) * 0.4
        },
    }]
}

pub fn site(_default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    let (up, flow, _) = axes();
    Some(("Las pozas", up.as_dvec3(), flow.as_dvec3(), place))
}

fn place(_instances: &mut Vec<Instance>, _radius: f64, _field: &dyn HeightField, _seed: u32, _frame: (DVec3, DVec3, DVec3)) {}
