//! Pira: roca volcanica y mares de lava. Shader animado.
//!
//! - La **lava** es un campo de temperatura que fluye: ruido fractal con
//!   domain warping que se desplaza con el tiempo. Su color no es un tinte a
//!   mano: es el de un cuerpo negro a esa temperatura (1300 a 2400 K), asi
//!   que lo mas caliente sale amarillo y lo mas frio rojo oscuro.
//! - Sobre el mar flotan **placas de costra** enfriada que se mueven mas
//!   lento que la lava.
//! - En la roca, las **grietas** son las lineas donde un ruido cruza por
//!   cero (`1 - |ruido|` cerca de 1): dibujan una red como de placas rotas.
//!   Brillan con su propia luz, mas del lado que mira a la estrella, y de
//!   noche son lo unico que se ve.

use super::{limb_haze, Fragment, Uniforms};
use crate::math::blackbody::planckian_rgb;
use crate::math::{smoothstep, Vec3};

const BASALT: Vec3 = Vec3::new(0.07, 0.06, 0.06);
const ASH: Vec3 = Vec3::new(0.17, 0.13, 0.11);
const SMOKE: Vec3 = Vec3::new(0.45, 0.16, 0.06);

pub fn shade(f: &Fragment, u: &Uniforms) -> Vec3 {
    let (molten, heat, ridge) = (f.color.x, f.color.y, f.color.z);
    let t = u.time;
    let n = f.normal;
    let l = f.light_dir(u);
    let ndl = n.dot(l);
    let p = f.local;

    // Grietas con desvanecimiento: si un pixel cubre mas que su ancho, se
    // reemplazan por su brillo promedio en vez de parpadear.
    let footprint = f.pos.length() * u.pixel_angle;
    let sharp = smoothstep(4.0, 0.8, footprint);
    let crack_noise = 1.0 - u.noise.noise(p * 0.11).abs();
    let crack = smoothstep(0.93, 0.995, crack_noise) * sharp + 0.06 * (1.0 - sharp);

    let rock_albedo = BASALT.lerp(ASH, ridge * 0.6);
    let lit = rock_albedo * u.sun_light * ndl.max(0.0) + rock_albedo * 0.015;
    let glow = planckian_rgb(1200.0 + 700.0 * heat) * (crack * (0.12 + 2.4 * heat));
    let mut color = lit + glow;

    if molten > 0.0 {
        let flow = Vec3::new(t * 0.06, t * 0.015, -t * 0.04);
        let swirl = u.noise.fbm(p * 0.05 + flow, 2);
        let temp = (u.noise.fbm(p * 0.09 + flow * 1.7 + Vec3::splat(swirl * 2.0), 3) * 0.5 + 0.5).clamp(0.0, 1.0);
        let plates = smoothstep(0.6, 0.7, u.noise.fbm(p * 0.045 - flow * 0.5, 2) * 0.5 + 0.5);
        let lava = planckian_rgb(1300.0 + 1100.0 * temp) * (1.0 + 5.5 * temp * temp);
        // La costra es roca oscura con las orillas al rojo.
        let crust = lit + planckian_rgb(1250.0) * (0.25 * heat);
        color = color.lerp(lava.lerp(crust, plates), molten);
    }

    // Humo y vapor de roca en el borde del lado iluminado.
    color + limb_haze(f, SMOKE * (0.4 + 0.6 * heat), 1.0, ndl)
}
