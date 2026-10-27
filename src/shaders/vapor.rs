//! Pira deshaciendose: Gargantua no la estira como una goma, la hace hervir.
//!
//! La marea del agujero calienta a Pira por dentro: la corteza se funde, la
//! roca se evapora y el planeta se encoge mientras pierde lo que se lleva
//! el brazo del disco (`gargantua::Stream`). Este volumen es lo que hierve
//! encima:
//!
//! - **Corona**: una capa delgada de vapor incandescente sobre lo que queda
//!   de la superficie, con llamas que suben (ruido estirado hacia afuera) y
//!   que la marea abulta hacia el agujero, adelante en la orbita: por ahi el
//!   gas sale al brazo.
//! - **El ultimo soplo**: cuando ya no queda planeta, lo que quedaba es un
//!   resplandor de gas tan caliente y delgado que brilla sin tapar; se
//!   alarga hacia el brazo, cae hacia el agujero y se apaga en segundos.
//!
//! Se marcha el rayo recto (como el planeta, que tampoco se curva) dentro de
//! la esfera que envuelve todo, de adelante hacia atras: cada muestra emite
//! con el color de cuerpo negro de su temperatura y tapa lo de atras. La
//! forma es un perfil suave erosionado por ruido: junto a la superficie el
//! gas es denso y caliente, y hacia afuera solo quedan jirones de humo. Las
//! cuentas van en radios del planeta.

use crate::math::blackbody::planckian_rgb;
use crate::math::{Perlin, Vec3, smoothstep};

/// Segundos que tarda Pira en deshacerse: a esta edad ya no queda planeta.
pub const MELT: f32 = 16.0;
/// Pasado esto la nube ya se fue al disco.
pub const VAPOR_LIFE: f32 = MELT + 4.0;
/// Opacidad por radio de planeta recorrido, a densidad 1.
const KAPPA: f32 = 9.0;
/// Paso de la marcha, en radios, y cuantas muestras como mucho.
const STEP: f32 = 0.07;
const MAX_STEPS: usize = 20;

#[derive(Clone, Copy, Debug)]
pub struct Vapor {
    /// Centro de Pira (donde esta o donde estaba), relativo a la camara.
    pub center: Vec3,
    pub radius: f32,
    /// Hacia el agujero y hacia adelante en la orbita (unitarios, normales).
    pub toward: Vec3,
    pub ahead: Vec3,
    /// Segundos desde que empezo a hervir.
    pub age: f32,
    /// Luz del disco que le llega.
    pub light: Vec3,
}

/// Lo que no cambia en todo el frame.
struct Shape {
    core: f32,
    /// Escala de altura de la corona y cuanto gas tiene.
    height: f32,
    corona: f32,
    /// Hacia donde se abulta y cuanto.
    lean: Vec3,
    bulge: f32,
    /// La nube del final: cuanto gas y su radio.
    puff: f32,
    puff_radius: f32,
    /// Cuanto se alarga en la orbita y cuanto cayo hacia el agujero.
    stretch: f32,
    fall: f32,
    /// Radio de la esfera que envuelve todo (sin alargar) y su centro.
    reach: f32,
    middle: Vec3,
}

impl Vapor {
    /// La nube `age` segundos despues de empezar, o nada si todavia no
    /// empieza o ya se fue.
    pub fn at(center: Vec3, radius: f32, toward: Vec3, ahead: Vec3, age: f32, light: Vec3) -> Option<Self> {
        (0.0..VAPOR_LIFE).contains(&age).then_some(Self {
            center,
            radius,
            toward,
            ahead,
            age,
            light,
        })
    }

    /// Lo que queda del planeta, en radios: se evapora cada vez mas rapido.
    pub fn core(age: f32) -> f32 {
        let u = (age / MELT).clamp(0.0, 1.0);
        1.0 - 0.95 * u * u * (1.6 - 0.6 * u)
    }

    fn shape(&self) -> Shape {
        let age = self.age;
        let core = Self::core(age);
        let late = (age - MELT).max(0.0);
        let height = (0.035 + 0.05 * smoothstep(0.0, MELT, age)) * (0.5 + 0.5 * core);
        let bulge = 3.0 * smoothstep(1.0, 8.0, age);
        let puff_radius = 0.22 + 0.05 * late;
        let lean = (self.toward + self.ahead * 0.5).normalize();
        let corona = smoothstep(0.2, 3.0, age) * smoothstep(0.02, 0.2, core);
        let puff = smoothstep(MELT - 3.0, MELT, age)
            * (-late / 1.2).exp()
            * smoothstep(VAPOR_LIFE, VAPOR_LIFE - 1.5, age);
        // La corona llega a unas cuantas alturas; del lado del agujero, mas.
        // Solo se envuelve lo que todavia existe.
        let top = core + 4.0 * height;
        let far = if corona > 0.01 { core + 4.0 * height * (1.0 + bulge) } else { 0.0 };
        let cloud = if puff > 0.01 { puff_radius * 2.2 } else { 0.0 };
        let reach = (0.5 * (far + top)).max(cloud) + 0.05;
        Shape {
            core,
            height,
            corona,
            lean,
            bulge,
            puff,
            puff_radius,
            stretch: 1.0 + 2.0 * smoothstep(MELT - 1.0, MELT + 8.0, age),
            fall: 0.8 * smoothstep(MELT - 1.0, MELT + 10.0, age),
            reach,
            middle: lean * (0.5 * (far - top)),
        }
    }

    /// Densidad (0 a 1) y calor (0 a 1) de la corona en `x`, en radios desde
    /// el centro de Pira y ya sin alargar en la orbita, y cuanto brilla ahi
    /// el ultimo soplo (gas tan caliente y delgado que casi no tapa).
    #[inline]
    fn density(&self, s: &Shape, x: Vec3, noise: &Perlin) -> (f32, f32, f32) {
        // La marea: del lado del agujero la corona llega mas lejos, como una
        // lengua que se estira hacia el brazo.
        let r = x.length();
        let a = x.dot(s.lean);
        let tongue = smoothstep(0.3, 1.0, a / r.max(1e-3));
        let lift = (r - s.core).max(0.0) / (s.height * (1.0 + s.bulge * tongue * tongue));
        let corona = s.corona * (-lift).exp();
        let puff = s.puff * (-(r / s.puff_radius) * (r / s.puff_radius)).exp();
        if corona.max(puff) < 0.02 {
            return (0.0, 0.0, 0.0);
        }
        let mut density = 0.0;
        let mut heat: f32 = 0.0;
        if corona >= 0.02 {
            // Llamas que suben: el ruido se estira hacia afuera y corre con
            // el tiempo; hacia el agujero el gas fluye mas rapido.
            let out = x / r.max(1e-3);
            let rise = (r - s.core) * 1.2 - self.age * (0.25 + 0.35 * a.max(0.0));
            let warp = noise.noise(out * 1.8 + Vec3::new(rise * 0.5, 3.7, 1.3));
            let flames = noise.fbm(out * 4.0 + Vec3::new(warp * 0.8, rise, warp * 0.5), 3);
            let n = smoothstep(-0.3, 0.35, flames);
            // Erosion: donde hay poco gas solo quedan las lenguas de fuego.
            density = (corona * 1.3 - (1.0 - n) * 0.8).clamp(0.0, 1.0);
            heat = (corona * (0.45 + 0.55 * n)).min(1.0);
        }
        let mut glow = 0.0;
        if puff >= 0.02 {
            // El ultimo soplo es gas caliente y suave: grumos que se
            // revuelven, sin bordes.
            let swirl = noise.fbm(x * 2.4 + Vec3::new(0.0, self.age * 0.15, 0.0), 2);
            glow = puff * (0.3 + 1.2 * smoothstep(-0.3, 0.5, swirl));
        }
        (density, heat, glow)
    }

    /// Mezcla la nube sobre `color` en la direccion `dir` (unitaria) antes
    /// de `depth`.
    pub fn apply(&self, color: Vec3, dir: Vec3, depth: f32, noise: &Perlin) -> Vec3 {
        let s = self.shape();
        // Alargada en la orbita: en el espacio de la nube se deshace y queda
        // una esfera.
        let unstretch = |v: Vec3| v - s.lean * (v.dot(s.lean) * (1.0 - 1.0 / s.stretch));
        let center = self.center + s.lean * (self.radius * s.fall);
        let origin = unstretch(-center) / self.radius;
        let step = unstretch(dir) / self.radius;
        let oc = origin - s.middle;
        let a = step.length_squared();
        let b = oc.dot(step);
        let disc = b * b - a * (oc.length_squared() - s.reach * s.reach);
        if disc <= 0.0 {
            return color;
        }
        let root = disc.sqrt();
        let (t0, t1) = (((-b - root) / a).max(0.0), ((-b + root) / a).min(depth));
        if t1 <= t0 {
            return color;
        }
        let span = (t1 - t0) * a.sqrt();
        let steps = ((span / STEP).ceil() as usize).clamp(4, MAX_STEPS);
        let dt = (t1 - t0) / steps as f32;
        let ds = span / steps as f32;
        // Se enfria mientras el brazo se la lleva.
        let cool = 1.0 - 0.5 * smoothstep(MELT, VAPOR_LIFE, self.age);
        let mut transmittance = 1.0;
        let mut light = Vec3::ZERO;
        let puff_color = planckian_rgb(3600.0 * cool) * 0.5;
        for k in 0..steps {
            let x = origin + step * (t0 + (k as f32 + 0.5) * dt);
            let (density, heat, glow) = self.density(&s, x, noise);
            light += puff_color * (glow * ds * transmittance);
            if density <= 0.0 {
                continue;
            }
            let alpha = 1.0 - (-density * KAPPA * ds).exp();
            // Del rojo oscuro del humo al amarillo junto a la superficie;
            // el disco lo ilumina del lado que le da.
            let hot = heat * heat * cool;
            let temperature = 1500.0 + 2500.0 * hot;
            let facing = smoothstep(-0.5, 1.0, x.dot(self.toward) / (x.length() + 0.2));
            let source = planckian_rgb(temperature) * (0.04 + 3.4 * hot * hot)
                + self.light * (0.02 + 0.12 * facing);
            light += source * (alpha * transmittance);
            transmittance *= 1.0 - alpha;
            if transmittance < 0.01 {
                break;
            }
        }
        color * transmittance + light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CENTER: Vec3 = Vec3::new(0.0, 0.0, -1000.0);

    fn cloud(age: f32) -> Vapor {
        Vapor::at(CENTER, 100.0, Vec3::X, Vec3::Y, age, Vec3::ONE).unwrap()
    }

    /// Luz de la nube a lo largo de `n` rayos que pasan junto a `offset`.
    fn glow(vapor: &Vapor, offset: Vec3, noise: &Perlin) -> f32 {
        (0..12)
            .map(|k| {
                let wiggle = Vec3::new(k as f32 * 2.0 - 11.0, 0.0, k as f32 * 4.0);
                let dir = (CENTER + offset + wiggle).normalize();
                vapor.apply(Vec3::ZERO, dir, f32::INFINITY, noise).max_element()
            })
            .sum::<f32>()
            / 12.0
    }

    #[test]
    fn the_planet_shrinks_while_it_boils() {
        assert_eq!(Vapor::core(0.0), 1.0);
        assert!(Vapor::core(MELT * 0.5) > 0.6);
        assert!(Vapor::core(MELT) < 0.06);
        let noise = Perlin::new(1);
        // Al empezar casi no hay corona; a media comida, brilla junto al
        // borde del planeta.
        let rim = |age: f32| glow(&cloud(age), Vec3::Y * (Vapor::core(age) * 100.0 + 3.0), &noise);
        assert!(rim(0.1) < 0.01);
        assert!(rim(8.0) > 0.1, "{}", rim(8.0));
    }

    #[test]
    fn the_tide_pulls_the_gas_toward_the_hole() {
        let noise = Perlin::new(1);
        let vapor = cloud(8.0);
        let edge = Vapor::core(8.0) * 100.0;
        // Del lado del agujero (+X, un poco adelante +Y) la corona llega mas
        // lejos que del otro lado.
        let toward = glow(&vapor, Vec3::new(edge + 25.0, 12.0, 0.0), &noise);
        let away = glow(&vapor, Vec3::new(-edge - 25.0, -12.0, 0.0), &noise);
        assert!(toward > away * 2.0 && toward > 0.05, "{toward} {away}");
    }

    #[test]
    fn the_last_of_it_glows_and_fades() {
        let noise = Perlin::new(1);
        let middle = |age: f32| glow(&cloud(age), Vec3::ZERO, &noise);
        assert!(middle(MELT + 1.0) > 0.05, "{}", middle(MELT + 1.0));
        // Brilla sin tapar lo de atras.
        let behind = cloud(MELT + 1.0).apply(Vec3::ONE, CENTER.normalize(), f32::INFINITY, &noise);
        assert!(behind.min_element() > 0.9, "{behind:?}");
        assert!(middle(VAPOR_LIFE - 1.0) < middle(MELT + 1.0) * 0.3);
        assert!(Vapor::at(CENTER, 100.0, Vec3::X, Vec3::Y, VAPOR_LIFE + 1.0, Vec3::ONE).is_none());
    }

    #[test]
    fn something_in_front_of_the_cloud_hides_it() {
        let noise = Perlin::new(1);
        let vapor = cloud(MELT);
        let color = vapor.apply(Vec3::splat(0.5), CENTER.normalize(), 500.0, &noise);
        assert_eq!(color, Vec3::splat(0.5));
    }
}
