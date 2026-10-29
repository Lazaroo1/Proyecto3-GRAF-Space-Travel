//! La version chapina (V): el mismo sistema, las mismas orbitas y la misma
//! fisica, con Guatemala. Cada cuerpo cambia de piel: su nombre, su relieve,
//! su shader, sus objetos, sus anillos, su atmosfera y su lugar. Tambien
//! cambian el cielo (otra Via Lactea), la corona del sol (dorada, con rayos),
//! la nave (botellas), el sonido (la musica de la carpeta `musica`, sin el
//! latido) y no hay final. Lo de OCASO que no tiene que ver (los arcos de
//! plasma, la cola de Pira) no se dibuja.
//!
//! | OCASO | Version chapina |
//! | --- | --- |
//! | La estrella | Kinich Ajaw, el sol maya ([kinich.rs]) |
//! | Pira | El Volcan de Fuego, con Antigua al pie ([fuego.rs]) |
//! | Jardin | Peten: la selva y Tikal ([peten.rs]) |
//! | Vigia | Atitlan: un mundo lago con tres volcanes ([atitlan.rs]) |
//! | Parpado | Semuc Champey: pozas turquesa ([semuc.rs]) |
//! | Argos | Los Cuchumatanes y San Andres Xecul ([cuchumatanes.rs]) |
//! | (nueva) | La Luna de Xelaju, luna de los Cuchumatanes ([xela.rs]) |
//! | Cementerio | La capital: camionetas y la Torre del Reformador ([capital.rs]) |
//! | El final (J) | No hay: Kinich Ajaw no se apaga ni late |
//!
//! Cada planeta vive en su archivo y da lo mismo: su `spec` (nombre,
//! atmosfera, anillos, particulas y lugar), su relieve (`field`), su shader
//! (`shade`), si mueve sus vertices (`DEFORMS`, `displace`), como reparte
//! sus objetos (`rules`, `site`) y los modelos y shaders de sus objetos
//! (`mesh`, `shade_prop`).

pub mod atitlan;
pub mod capital;
pub mod cuchumatanes;
pub mod fuego;
pub mod kinich;
pub mod peten;
pub mod semuc;
pub mod xela;

use std::sync::Arc;

use crate::math::{DVec3, Perlin, Vec3};
use crate::scene::Mesh;
use crate::scene::effects::Motes;
use crate::scene::props::{Instance, PropKind, Scatter};
use crate::scene::Rings;
use crate::shaders::{AtmosphereParams, Fragment, PropStyle, Uniforms};
use crate::terrain::HeightField;

/// El material de cada cuerpo en la version chapina.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chapin {
    Fuego,
    Peten,
    Atitlan,
    Semuc,
    Cuchumatanes,
    Xela,
    Capital,
}

/// Lo que un planeta chapin dice de si mismo.
pub struct Spec {
    /// Nombre en el HUD (sin tildes: el texto de raylib no las acepta).
    pub label: &'static str,
    pub material: Chapin,
    pub atmosphere: Option<AtmosphereParams>,
    pub rings: Option<Rings>,
    /// Particulas cerca del suelo y hasta que altura.
    pub motes: Option<(Motes, f64)>,
}

/// Coloca los objetos del lugar de un planeta (como los de OCASO).
pub type Placer = fn(&mut Vec<Instance>, f64, &dyn HeightField, u32, (DVec3, DVec3, DVec3));

/// Cada cuerpo de OCASO (por su nombre interno) y su version chapina.
pub fn spec(name: &str) -> Option<Spec> {
    match name {
        "Pira" => Some(fuego::spec()),
        "Jardin" => Some(peten::spec()),
        "Vigia" => Some(atitlan::spec()),
        "Parpado" => Some(semuc::spec()),
        "Argos" => Some(cuchumatanes::spec()),
        "Xela" => Some(xela::spec()),
        "Cementerio" => Some(capital::spec()),
        _ => None,
    }
}

/// El nombre de la estrella en la version chapina.
pub const STAR_LABEL: &str = "Kinich Ajaw";

/// El relieve de un cuerpo (por su nombre interno).
pub fn field(name: &str, seed: u32) -> Option<Arc<dyn HeightField>> {
    Some(match name {
        "Pira" => fuego::field(seed),
        "Jardin" => peten::field(seed),
        "Vigia" => atitlan::field(seed),
        "Parpado" => semuc::field(seed),
        "Argos" => cuchumatanes::field(seed),
        "Xela" => xela::field(seed),
        "Cementerio" => capital::field(seed),
        _ => return None,
    })
}

/// Reglas de reparto de objetos de un cuerpo (por su nombre interno).
pub fn rules_for(name: &str) -> Vec<Scatter> {
    match name {
        "Pira" => fuego::rules(),
        "Jardin" => peten::rules(),
        "Vigia" => atitlan::rules(),
        "Parpado" => semuc::rules(),
        "Argos" => cuchumatanes::rules(),
        "Cementerio" => capital::rules(),
        _ => Vec::new(),
    }
}

/// El lugar de un cuerpo: su nombre, su vertical y hacia donde mirar (en el
/// espacio del planeta), y quien coloca sus objetos. `default` es donde
/// estaria (el mismo del lugar de OCASO, si lo hay).
pub fn site(name: &str, default: (DVec3, DVec3)) -> Option<(&'static str, DVec3, DVec3, Placer)> {
    match name {
        "Pira" => fuego::site(default),
        "Jardin" => peten::site(default),
        "Vigia" => atitlan::site(default),
        "Parpado" => semuc::site(default),
        "Argos" => cuchumatanes::site(default),
        "Cementerio" => capital::site(default),
        _ => None,
    }
}

impl Chapin {
    /// El fragment shader del cuerpo.
    pub fn shade(self, f: &Fragment, u: &Uniforms) -> Vec3 {
        match self {
            Chapin::Fuego => fuego::shade(f, u),
            Chapin::Peten => peten::shade(f, u),
            Chapin::Atitlan => atitlan::shade(f, u),
            Chapin::Semuc => semuc::shade(f, u),
            Chapin::Cuchumatanes => cuchumatanes::shade(f, u),
            Chapin::Xela => xela::shade(f, u),
            Chapin::Capital => capital::shade(f, u),
        }
    }

    /// Si mueve sus vertices (y hay que llamar a `displace`).
    pub fn deforms(self) -> bool {
        match self {
            Chapin::Fuego => fuego::DEFORMS,
            Chapin::Peten => peten::DEFORMS,
            Chapin::Atitlan => atitlan::DEFORMS,
            Chapin::Semuc => semuc::DEFORMS,
            Chapin::Cuchumatanes => cuchumatanes::DEFORMS,
            Chapin::Xela => false,
            Chapin::Capital => capital::DEFORMS,
        }
    }

    /// El vertex shader: cuanto se mueve el vertice `local` (con normal
    /// `normal`), en el espacio del planeta.
    pub fn displace(self, local: Vec3, normal: Vec3, time: f32, noise: &Perlin) -> Vec3 {
        match self {
            Chapin::Fuego => fuego::displace(local, normal, time, noise),
            Chapin::Peten => peten::displace(local, normal, time, noise),
            Chapin::Atitlan => atitlan::displace(local, normal, time, noise),
            Chapin::Semuc => semuc::displace(local, normal, time, noise),
            Chapin::Cuchumatanes => cuchumatanes::displace(local, normal, time, noise),
            Chapin::Xela => Vec3::ZERO,
            Chapin::Capital => capital::displace(local, normal, time, noise),
        }
    }
}

/// Los modelos de los objetos chapines, cada uno en su planeta.
pub fn mesh(kind: PropKind, detail: usize) -> Mesh {
    match kind {
        PropKind::Arco | PropKind::Casa => fuego::mesh(kind, detail),
        PropKind::Templo | PropKind::Ceiba | PropKind::Arbol => peten::mesh(kind, detail),
        PropKind::Lancha => atitlan::mesh(kind, detail),
        PropKind::Iglesia | PropKind::Pino => cuchumatanes::mesh(kind, detail),
        PropKind::Camioneta | PropKind::Torre | PropKind::Edificio => capital::mesh(kind, detail),
        _ => unreachable!("{kind:?} no es un objeto chapin"),
    }
}

/// Los shaders de los objetos chapines. `sun` es la luz de la estrella que
/// les llega (cero si el planeta la tapa).
pub fn shade_prop(kind: PropKind, f: &Fragment, u: &Uniforms, style: &PropStyle, sun: Vec3) -> Vec3 {
    match kind {
        PropKind::Arco | PropKind::Casa => fuego::shade_prop(kind, f, u, style, sun),
        PropKind::Templo | PropKind::Ceiba | PropKind::Arbol => peten::shade_prop(kind, f, u, style, sun),
        PropKind::Lancha => atitlan::shade_prop(kind, f, u, style, sun),
        PropKind::Iglesia | PropKind::Pino => cuchumatanes::shade_prop(kind, f, u, style, sun),
        PropKind::Camioneta | PropKind::Torre | PropKind::Edificio => {
            capital::shade_prop(kind, f, u, style, sun)
        }
        _ => Vec3::ZERO,
    }
}
