//! Shaders propios.
//!
//! - **Fragment shaders**: cada material es una funcion que recibe los
//!   atributos ya interpolados del pixel visible (posicion, normal,
//!   coordenadas del objeto, uv, color) y devuelve su color HDR lineal.
//!   Corren en el pase de shading diferido: una vez por pixel visible.
//! - **Mapeo de normales**: los objetos de la superficie leen mapas de
//!   normales generados al arrancar (`textures`), con mipmaps.
//! - **Deformacion de vertices**: algunos materiales mueven sus vertices
//!   antes de proyectarlos (`displace`), como la respiracion de Jardin.
//! - **Volumenes**: lo que no es una superficie (anillos, atmosferas) se
//!   calcula por pixel contra la profundidad ya resuelta (`volumes`).

pub mod atmosphere;
mod cementerio;
mod detail;
pub mod gargantua;
mod images;
mod jardin;
mod nebula;
pub(crate) mod parpado;
mod tail;
mod vapor;
mod pbr;
mod pira;
mod props;
pub(crate) mod requiem;
pub mod rings;
mod ship;
mod sky;
mod sun;
pub mod textures;
mod vigia;

use crate::math::{Perlin, Vec2, Vec3, Vec4};

pub use atmosphere::{AtmosphereFrame, AtmosphereModel, AtmosphereParams, SkyView};
pub use gargantua::BlackHole;
pub use nebula::Nebula;
pub use tail::Tail;
pub use vapor::Vapor;
pub use pbr::PointLight;
pub use props::{BEACON_PERIOD, PropStyle, beacon_flash, vent_activity};
pub use rings::{RingInfo, RingStyle};
pub use ship::ShipStyle;
pub use sky::{CoronaClouds, SkyCube, shade as sky};
pub use sun::{SunStyle, heartbeat, heartbeat_phase};
pub use textures::Textures;

/// Vistas de depuracion para demostrar el trabajo del renderer.
#[derive(Clone, Copy, Debug, Default)]
pub struct DebugView {
    /// Tine cada chunk de terreno con el color de su nivel de detalle.
    pub lod_colors: bool,
    /// Dibuja las aristas de los triangulos.
    pub wireframe: bool,
    /// Muestra solo la luz que la atmosfera agrega en cada superficie.
    pub fog_only: bool,
}

/// Intensidad del efecto propio de cada shader, ajustable en vivo (1 es la
/// de diseno): la lava de Pira, la bioluminiscencia de Jardin, el ojo de
/// Vigia, el hielo y los glifos de Requiem, la chatarra de Cementerio y el
/// brillo de la estrella.
#[derive(Clone, Copy, Debug)]
pub struct Gains {
    pub sun: f32,
    pub pira: f32,
    pub jardin: f32,
    pub vigia: f32,
    pub requiem: f32,
    pub cementerio: f32,
}

impl Default for Gains {
    fn default() -> Self {
        Self {
            sun: 1.0,
            pira: 1.0,
            jardin: 1.0,
            vigia: 1.0,
            requiem: 1.0,
            cementerio: 1.0,
        }
    }
}

impl Gains {
    /// La intensidad del shader de un material de cuerpo, si tiene.
    pub fn for_material(&mut self, material: &Material) -> Option<&mut f32> {
        match material {
            Material::Sun(_) => Some(&mut self.sun),
            Material::Pira => Some(&mut self.pira),
            Material::Jardin => Some(&mut self.jardin),
            Material::Vigia | Material::Parpado => Some(&mut self.vigia),
            Material::Requiem => Some(&mut self.requiem),
            Material::Cementerio => Some(&mut self.cementerio),
            _ => None,
        }
    }
}

/// Valores comunes a todos los pixeles del frame.
pub struct Uniforms<'a> {
    pub shadow: Option<&'a crate::render::shadow::ShadowMap>,
    pub points: &'a [PointLight],
    /// Tiempo de simulacion en segundos.
    pub time: f32,
    /// Latido de la estrella en este frame (0 a 1): igual para todos los
    /// pixeles, se calcula una vez. Cero cuando la estrella ya no existe.
    pub beat: f32,
    /// Fase del latido (en latidos), latidos por segundo y si el corazon
    /// sigue vivo (1) o ya se apago (0), para lo que late con retraso.
    pub beat_phase: f32,
    pub beat_rate: f32,
    pub heart: f32,
    /// Brillo de la corona de la estrella en el cielo.
    pub star_glow: f32,
    /// Llamaradas del estertor (0 en la agonia, 1 justo antes del colapso).
    pub flares: f32,
    /// La capa de gas que solto la estrella, mientras se ve.
    pub nebula: Option<Nebula>,
    /// La cola de polvo de Pira.
    pub tail: Option<Tail>,
    /// Pira hirviendo mientras Gargantua se la come.
    pub vapor: Option<Vapor>,
    /// Gargantua, cuando ya existe.
    pub black_hole: Option<&'a BlackHole<'a>>,
    /// Centro de la estrella, relativo a la camara.
    pub sun_pos: Vec3,
    pub sun_radius: f32,
    /// Luz de la estrella que llega a los planetas (color * intensidad).
    pub sun_light: Vec3,
    pub noise: &'a Perlin,
    /// Estrellas y nebulosa horneadas.
    pub sky: &'a SkyCube,
    /// Mapas de normales de los objetos de la superficie.
    pub textures: &'a Textures,
    /// Angulo que cubre un pixel, en radianes.
    pub pixel_angle: f32,
    /// Hacia donde mira el ojo de Vigia, en el espacio de Vigia.
    pub eye_dir: Vec3,
    /// Que tan cerca esta la nave de Vigia (0 lejos, 1 rozando sus nubes):
    /// el ojo reacciona.
    pub eye_near: f32,
    /// Vigia vista desde afuera de su dibujo: los lagos de Parpado la
    /// reflejan.
    pub giant: Option<Giant>,
    /// Posicion de la nave, relativa a la camara: lo que hay en los lugares
    /// de cada planeta despierta cuando se acerca.
    pub ship: Vec3,
    /// Cuanto se ha fundido Pira mientras Gargantua se la come (0 a 1).
    pub melt: f32,
    /// Anillos de Cementerio, si existen.
    pub rings: &'a [RingInfo],
    /// Atmosferas visibles, de la mas lejana a la mas cercana.
    pub atmospheres: &'a [AtmosphereFrame<'a>],
    /// Cielo precalculado de la atmosfera en la que esta la camara, si esta
    /// dentro de alguna.
    pub sky_view: Option<&'a SkyView>,
    /// Intensidad de la bioluminiscencia de Jardin (1 normal; sube cuando la
    /// estrella se apaga).
    pub biolum: f32,
    /// Intensidad del efecto de cada shader.
    pub gains: Gains,
    /// Nubes de la corona de la estrella, tabuladas en este frame.
    pub clouds: Option<&'a CoronaClouds>,
    pub debug: DebugView,
}

/// Atributos interpolados del pixel visible.
pub struct Fragment {
    pub ao: f32,
    pub object_to_world: crate::math::Mat3,
    /// Posicion en el mundo relativa a la camara: la camara es el origen.
    pub pos: Vec3,
    /// Normal en el mundo, unitaria.
    pub normal: Vec3,
    /// Posicion en el espacio del objeto: gira con el.
    pub local: Vec3,
    /// En el terreno: altura normalizada y pendiente.
    pub uv: Vec2,
    /// Color del material, o mascaras del terreno.
    pub color: Vec3,
    /// Tangente en el mundo (sin normalizar) y signo de la bitangente.
    pub tangent: Vec4,
}

impl Uniforms<'_> {
    /// El latido `delay` segundos atras (las venas de Jardin y los corales
    /// lo reciben con retraso).
    #[inline]
    pub fn beat_at(&self, delay: f32) -> f32 {
        heartbeat_phase((self.beat_phase - delay * self.beat_rate) as f64) as f32 * self.heart
    }
}

impl Fragment {
    /// Normal perturbada por un mapa de normales: `sample` es la normal en
    /// el espacio tangente (x sobre la tangente, y sobre la bitangente, z
    /// sobre la normal) y `strength` escala la inclinacion.
    ///
    /// La base TBN se reconstruye en cada pixel: la tangente interpolada se
    /// vuelve a hacer perpendicular a la normal (Gram-Schmidt) y la
    /// bitangente sale del producto cruz con su signo.
    #[inline]
    pub fn perturb(&self, sample: Vec3, strength: f32) -> Vec3 {
        let n = self.normal;
        let t = (self.tangent.xyz() - n * n.dot(self.tangent.xyz())).normalize();
        let b = n.cross(t) * self.tangent.w;
        (t * (sample.x * strength) + b * (sample.y * strength) + n * sample.z).normalize()
    }

    /// Direccion unitaria hacia la camara.
    #[inline]
    pub fn view_dir(&self) -> Vec3 {
        (-self.pos).normalize()
    }

    /// Direccion unitaria hacia la estrella.
    #[inline]
    pub fn light_dir(&self, u: &Uniforms) -> Vec3 {
        (u.sun_pos - self.pos).normalize()
    }
}

/// Un planeta para sombrearlo desde otro lado (un reflejo): centro relativo
/// a la camara, radio y sus ejes.
#[derive(Clone, Copy, Debug)]
pub struct Giant {
    pub center: Vec3,
    pub radius: f32,
    /// De su espacio al mundo y al reves.
    pub to_world: crate::math::Mat3,
    pub to_local: crate::math::Mat3,
}

#[derive(Clone, Copy, Debug)]
pub enum Material {
    Sun(SunStyle),
    Pira,
    Jardin,
    Vigia,
    Requiem,
    Cementerio,
    /// La luna de Vigia: cristal y lagos espejo.
    Parpado,
    Ship(ShipStyle),
    /// Un objeto de la superficie, con los datos de su instancia.
    Prop(PropStyle),
    /// Una cinta de luz (la trayectoria predicha): solo emite su color.
    Line(Vec3),
}

impl Material {
    /// Si el material mueve sus vertices (y hay que llamar a `displace`).
    pub fn deforms(&self) -> bool {
        matches!(self, Material::Jardin)
    }
}

#[inline]
pub fn shade(material: &Material, f: &Fragment, u: &Uniforms) -> Vec3 {
    match material {
        Material::Sun(style) => sun::shade(f, u, style),
        Material::Pira => pira::shade(f, u),
        Material::Jardin => jardin::shade(f, u),
        Material::Vigia => vigia::shade(f, u),
        Material::Requiem => requiem::shade(f, u),
        Material::Cementerio => cementerio::shade(f, u),
        Material::Parpado => parpado::shade(f, u),
        Material::Ship(style) => ship::shade(f, u, style),
        Material::Prop(style) => props::shade(style, f, u),
        Material::Line(color) => line(f, *color),
    }
}

/// Cinta de luz: el color de sus vertices por el del material; brilla mas
/// en el centro que en los bordes (uv.y va de un borde al otro) y se apaga
/// hacia el final del camino (uv.x).
#[inline]
fn line(f: &Fragment, color: Vec3) -> Vec3 {
    let core = 1.0 - (f.uv.y * 2.0 - 1.0).abs();
    f.color * color * (core.sqrt() * (1.0 - 0.75 * f.uv.x))
}

/// Desplazamiento del vertice sobre su normal, en unidades del mundo. Solo
/// depende del tiempo y del ruido: los objetos que se paran sobre una
/// superficie que se mueve lo usan para moverse con ella.
#[inline]
pub fn displace(material: &Material, local: Vec3, time: f32, noise: &Perlin) -> f32 {
    match material {
        Material::Jardin => jardin::breathe(local, time, noise),
        _ => 0.0,
    }
}

/// Brillo del borde de un planeta visto desde lejos: cerca del limbo la
/// linea de vision atraviesa mas atmosfera. Junto a la superficie se apaga,
/// porque ahi las lomas tambien tienen bordes rasantes y se verian palidas.
#[inline]
pub fn limb_haze(f: &Fragment, color: Vec3, strength: f32, ndl: f32) -> Vec3 {
    let ndv = f.normal.dot(f.view_dir()).max(0.0);
    let rim = (1.0 - ndv).powi(3);
    let day = crate::math::smoothstep(-0.25, 0.4, ndl);
    // Distancia a la camara en radios del planeta.
    let far = crate::math::smoothstep(0.15, 0.6, f.pos.length() / f.local.length().max(1.0));
    color * (rim * day * far * strength)
}

/// Si hay algun volumen que componer este frame.
#[inline]
pub fn has_volumes(u: &Uniforms) -> bool {
    !u.rings.is_empty()
        || !u.atmospheres.is_empty()
        || u.nebula.is_some()
        || u.tail.is_some()
        || u.vapor.is_some()
}

/// Pasos de la atmosfera por pixel de cielo: ahi esta el detalle fino del
/// limbo visto desde la orbita.
const SKY_FOG_STEPS: usize = 6;

/// Mezcla los volumenes que cruza el rayo `dir` (sin normalizar) antes de
/// llegar a la distancia `depth` (infinita en el cielo). Las atmosferas solo
/// se calculan aqui para el cielo: en las superficies ya vienen de los
/// vertices.
#[inline]
pub fn volumes(color: Vec3, dir: Vec3, depth: f32, sky: bool, u: &Uniforms) -> Vec3 {
    let dir = dir.normalize();
    let mut color = color;
    if let Some(vapor) = &u.vapor {
        color = vapor.apply(color, dir, depth, u.noise);
    }
    if let Some(nebula) = &u.nebula {
        color += nebula.light(dir, depth, u.noise);
    }
    if let Some(tail) = &u.tail {
        color += tail.light(dir, depth, u.noise);
    }
    // Del anillo mas lejano al mas cercano.
    match u.rings {
        [] => {}
        [ring] => color = rings::composite(color, dir, depth, ring, u),
        many => {
            let mut order = [(0.0f32, 0usize); 4];
            let count = many.len().min(order.len());
            for (k, ring) in many.iter().take(count).enumerate() {
                order[k] = (rings::distance(ring, dir), k);
            }
            order[..count].sort_by(|a, b| b.0.total_cmp(&a.0));
            for &(t, k) in &order[..count] {
                if t.is_finite() {
                    color = rings::composite(color, dir, depth, &many[k], u);
                }
            }
        }
    }
    if sky {
        // De la mas lejana a la mas cercana; la que rodea a la camara sale
        // de la tabla del cielo en vez de integrarse.
        for (i, atmosphere) in u.atmospheres.iter().enumerate() {
            let fog = match u.sky_view {
                Some(view) if view.index == i => view.sample(dir),
                _ => atmosphere.integrate(dir, depth, u.sun_light, SKY_FOG_STEPS),
            };
            color = fog.apply(color);
        }
    }
    color
}

/// Color de depuracion de cada nivel de detalle.
pub fn lod_color(level: u32) -> Vec3 {
    const PALETTE: [Vec3; 8] = [
        Vec3::new(0.9, 0.2, 0.2),
        Vec3::new(0.95, 0.55, 0.15),
        Vec3::new(0.95, 0.9, 0.2),
        Vec3::new(0.3, 0.85, 0.3),
        Vec3::new(0.2, 0.8, 0.85),
        Vec3::new(0.25, 0.4, 0.95),
        Vec3::new(0.6, 0.3, 0.9),
        Vec3::new(0.95, 0.4, 0.8),
    ];
    PALETTE[level as usize % PALETTE.len()]
}
