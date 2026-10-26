//! Jardin: el planeta vivo. Shader animado y deformacion de vertices.
//!
//! - **Arterias**: las curvas donde un ruido de Perlin vale cero forman sobre
//!   la esfera una red cerrada (la misma que levanta el terreno). Se dibujan
//!   por pixel con antialiasing: cuando un pixel cubre mas que el ancho de la
//!   linea, la linea se ensancha y se atenua en proporcion, asi conserva su
//!   brillo total y no parpadea desde la orbita.
//! - **Pulso**: las arterias laten con el mismo ritmo que la estrella, pero
//!   cada lugar recibe el latido con un retraso que cambia suave sobre la
//!   superficie: se ve una onda que recorre el planeta.
//! - **Capilares**: una red mas fina que solo aparece de cerca.
//! - La bioluminiscencia se nota mas de noche, y `Uniforms::biolum` la sube
//!   cuando la estrella se apaga.
//! - **Respiracion**: `breathe` desplaza los vertices sobre su normal con un
//!   ritmo lento; la superficie entera se hincha y se deshincha.

use std::f32::consts::TAU;

use super::{heartbeat, limb_haze, Fragment, Uniforms};
use crate::config::HEARTBEAT_PERIOD;
use crate::math::{smoothstep, Perlin, Vec3};
use crate::terrain::shapes::{artery, ARTERY_FREQUENCY};

const TISSUE_DARK: Vec3 = Vec3::new(0.10, 0.03, 0.10);
const TISSUE_MOSS: Vec3 = Vec3::new(0.035, 0.10, 0.095);
const VEIN_CYAN: Vec3 = Vec3::new(0.10, 0.95, 0.80);
const VEIN_MAGENTA: Vec3 = Vec3::new(0.95, 0.20, 0.75);
const PORE_GLOW: Vec3 = Vec3::new(0.85, 0.15, 0.55);
const HAZE: Vec3 = Vec3::new(0.55, 0.25, 0.55);
const CAPILLARY_FREQUENCY: f32 = 9.0;
/// Segundos de una respiracion completa.
const BREATH_PERIOD: f32 = 7.0;
/// Cuanto se hincha la superficie, en unidades del mundo.
const BREATH_DEPTH: f32 = 0.7;

/// Desplazamiento de un vertice sobre su normal.
pub fn breathe(local: Vec3, time: f32, noise: &Perlin) -> f32 {
    let breath = (time * TAU / BREATH_PERIOD).sin() * 0.5 + 0.5;
    // No todo se hincha igual: algunas zonas casi no se mueven.
    let variation = noise.noise(local * 0.02) * 0.5 + 0.5;
    breath * BREATH_DEPTH * (0.3 + 0.7 * variation)
}

/// Mascara de una linea donde `value` se acerca a 1, con `width` de ancho y
/// `aa` de desenfoque (los dos en unidades de `value`). Al crecer `aa` la
/// linea se ensancha y baja su pico: su brillo total se mantiene.
#[inline]
fn line(value: f32, width: f32, aa: f32) -> f32 {
    smoothstep(1.0 - width - aa, 1.0 - width + aa, value) * (width / (width + aa)).min(1.0)
}

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (_, pore, fold) = (f.color.x, f.color.y, f.color.z);
    let t = u.time;
    let n = f.normal;
    let l = f.light_dir(u);
    let ndl = n.dot(l);
    let dir = f.local.normalize();
    let radius = f.local.length().max(1.0);
    // Cuanto avanza la coordenada del ruido de un pixel al siguiente.
    let step = f.pos.length() * u.pixel_angle / radius * 1.5;

    let albedo = TISSUE_DARK.lerp(TISSUE_MOSS, fold);
    // Luz envolvente: el tejido deja pasar luz mas alla del terminador.
    let wrap = ((ndl + 0.35) / 1.35).max(0.0);
    let lit = albedo * u.sun_light * wrap + albedo * 0.02;

    // Arteria: un nucleo delgado y un halo suave alrededor; de cerca se ve
    // un tubo que brilla por dentro, no una franja plana.
    let a = artery(u.noise, dir);
    let artery_aa = (step * ARTERY_FREQUENCY).max(0.003);
    let halo = smoothstep(0.97, 1.0, a);
    let arteries = line(a, 0.008, artery_aa) + halo * halo * 0.08;
    let capillary_aa = (step * CAPILLARY_FREQUENCY).max(0.004);
    let capillaries = if capillary_aa < 0.1 {
        let value = 1.0 - u.noise.noise(dir * CAPILLARY_FREQUENCY + Vec3::new(4.0, 9.0, -2.0)).abs();
        line(value, 0.03, capillary_aa)
    } else {
        0.0
    };

    let delay = (u.noise.noise(dir * 1.2) * 0.5 + 0.5) * 2.5;
    let beat = heartbeat((t - delay) as f64, HEARTBEAT_PERIOD) as f32;
    // Cada arteria es cian o magenta: mezclarlas a medias daria lavanda
    // casi blanco.
    let vein_color = VEIN_CYAN.lerp(VEIN_MAGENTA, smoothstep(0.45, 0.55, fold));
    let veins = vein_color * (arteries * (0.3 + 1.8 * beat))
        + VEIN_CYAN * (capillaries * (0.1 + 0.45 * beat));
    // Poros: solo su borde brilla, como un anillo alrededor de la boca.
    let flicker = (t * 1.3 + f.local.x * 0.05 + f.local.z * 0.07).sin() * 0.5 + 0.5;
    let ring = pore * (1.0 - pore) * 4.0;
    let pores = PORE_GLOW * (ring * (0.06 + 0.1 * flicker));
    let night = smoothstep(0.15, -0.35, ndl);
    let glow = (veins + pores) * ((0.3 + 0.7 * night) * u.biolum);

    lit + glow + limb_haze(f, HAZE, 0.6, ndl)
}
