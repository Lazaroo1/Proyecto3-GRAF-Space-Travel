//! Kinich Ajaw, el sol maya: la estrella de la version chapina. Hace todo lo
//! que hace la estrella de OCASO (late, agoniza, echa llamaradas en el
//! estertor y colapsa), pero arde en oro y en vez del ojo lleva el glifo
//! k'in, el signo maya del sol y del dia: una flor de cuatro petalos.
//!
//! # El glifo
//! Vive en el espacio de la estrella, alrededor del mismo eje donde OCASO
//! tiene el ojo (`GLYPH_AXIS`): gira con ella y a los 40 s mira de frente a
//! la vista 53_kinich. Cada punto se proyecta al plano tangente a ese eje
//! (proyeccion ortografica: visto de frente coincide con lo que ve la
//! camara, visto de lado se encoge con la esfera) y ahi se dibuja con
//! funciones de distancia con signo en 2D: cuatro petalos (elipses), el
//! centro, cuatro puntos entre los petalos y el cartucho, un cuadrado de
//! esquinas redondas (la norma 3: |x|^3 + |y|^3 = R^3).
//!
//! El k'in tiene la simetria del cuadrado, asi que el plano se dobla:
//! con `|x|`, `|y|` y ordenandolos, todo punto cae en el primer octante,
//! donde basta un petalo y un punto para dibujar los cuatro.
//!
//! # Integrado al fuego
//! El glifo no pinta color: suma o quita calor y todo pasa por la misma
//! rampa, asi que arde con el resto. Sus lineas son canales incandescentes
//! (una gaussiana sobre la distancia) por los que corre el brillo; afuera
//! de cada forma hay una sombra que la levanta como un relieve; los
//! petalos arden mas, con rayos que salen del centro, y el fondo del
//! cartucho se apaga. El plano del glifo se deforma con el mismo warp del
//! fuego, asi que las lineas tiemblan como plasma.
//!
//! Antialias: un pixel cubre `distancia * angulo_del_pixel` sobre la
//! esfera. Cada linea se ensancha a `sqrt(w^2 + pixel^2)` y baja su brillo
//! en la misma proporcion (conserva su energia): de lejos el glifo se
//! difumina en vez de parpadear.
//!
//! # El fuego
//! El mismo domain warping de dos capas que la estrella de OCASO, con una
//! rampa de oro (brasa, naranja, oro y blanco), oscurecimiento hacia el
//! borde (limb darkening, como el Sol de verdad) y carriles oscuros de
//! granulacion donde un ruido cruza por cero. El borde usa `limb_fire` de
//! la estrella, asi que las llamas siguen en la corona del cielo.
//!
//! # Latido, estertor y colapso
//! Cada latido dilata el centro del k'in y sale de el como un anillo de
//! luz. En el estertor (la estrella se enfria) el oro se va a carmesi, los
//! carriles se abren, el glifo falla por partes como una brasa que se
//! apaga y estallan llamaradas doradas. Al colapsar se calienta y, como la
//! de OCASO, se lleva al color de cuerpo negro.

use crate::math::blackbody::planckian_rgb;
use crate::math::{Vec3, smoothstep};
use crate::shaders::sun::{limb_fire, limb_frame};
use crate::shaders::{Fragment, SunStyle, Uniforms};

/// Temperatura de la agonia (la misma de la estrella de OCASO).
const CALM_TEMPERATURE: f32 = 3400.0;
/// En el estertor se enfria hasta 400 K menos.
const DYING_DROP: f32 = 400.0;
/// La rampa de oro esta en unidades de la intensidad de la superficie.
const DISC_GAIN: f32 = 0.45;
/// Donde esta el glifo, en el espacio de la estrella: donde OCASO tiene el
/// ojo, que mira a la vista de detras de Pira a los 40 s.
const GLYPH_AXIS: Vec3 = Vec3::new(-0.9684, 0.1954, 0.1551);
/// Giro del glifo alrededor de su eje: con el, en 53_kinich queda derecho.
const TWIST: f32 = -0.115;
/// El glifo solo se evalua donde `p . GLYPH_AXIS` pasa de esto.
const FACING_CUT: f32 = 0.7;
/// Hebras carmesi en lo frio, como las nubes de la corona.
const CARMINE: Vec3 = Vec3::new(0.42, 0.04, 0.05);

/// Gira `p` un angulo `angle` alrededor del eje unitario `axis` (Rodrigues).
#[inline]
fn rotate(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.dot(p) * (1.0 - c))
}

/// Rampa de oro: calor (0 a ~1.3) a luz HDR.
fn gold(heat: f32) -> Vec3 {
    const STOPS: [(f32, Vec3); 7] = [
        (0.0, Vec3::new(0.0, 0.0, 0.0)),
        (0.18, Vec3::new(0.05, 0.012, 0.003)),
        (0.38, Vec3::new(0.36, 0.085, 0.015)),
        (0.58, Vec3::new(0.85, 0.3, 0.045)),
        (0.78, Vec3::new(1.55, 0.78, 0.16)),
        (1.0, Vec3::new(2.4, 1.6, 0.55)),
        (1.3, Vec3::new(3.8, 3.1, 1.9)),
    ];
    let heat = heat.clamp(0.0, 1.3);
    for pair in STOPS.windows(2) {
        let ((h0, c0), (h1, c1)) = (pair[0], pair[1]);
        if heat <= h1 {
            return c0.lerp(c1, (heat - h0) / (h1 - h0));
        }
    }
    STOPS[6].1
}

/// Linea de ancho `w` sobre la distancia `d`, ensanchada por el pixel `fp`
/// sin cambiar su energia (la integral sobre `d` no depende de `fp`).
#[inline]
fn line(d: f32, w: f32, fp: f32) -> f32 {
    let wide = (w * w + fp * fp).sqrt();
    (-(d / wide).powi(2)).exp() * (w / wide)
}

/// Lo que el k'in hace en un punto de su plano.
struct Glyph {
    /// Canales incandescentes.
    lines: f32,
    /// Sombra afuera de cada forma: el relieve.
    shadow: f32,
    /// Dentro de los petalos.
    petals: f32,
    /// El fondo del cartucho, entre la flor y el borde.
    ground: f32,
    /// El punto del centro y el anillo oscuro que lo rodea.
    core: f32,
    gap: f32,
}

/// El k'in en `(x, y)` (en radios de la estrella), con pixel `fp` y el
/// latido `beat` que dilata su centro.
fn glyph(x: f32, y: f32, fp: f32, beat: f32) -> Glyph {
    // Petalos: elipses con centro a `D`, radios `RP_R` (hacia afuera) y
    // `RP_T` (de lado); la linea de adentro va `INSET` mas adentro.
    const D: f32 = 0.21;
    const RP_R: f32 = 0.165;
    const RP_T: f32 = 0.142;
    const INSET: f32 = 0.042;
    // El centro, los puntos de las diagonales y el cartucho.
    const RC: f32 = 0.085;
    const DOT: f32 = 0.29;
    const RD: f32 = 0.022;
    const R: f32 = 0.52;
    const W: f32 = 0.0095;

    // Al primer octante: el petalo de +x y el punto de la diagonal.
    let (mut ax, mut ay) = (x.abs(), y.abs());
    if ay > ax {
        std::mem::swap(&mut ax, &mut ay);
    }
    let rho = x.hypot(y);
    // Distancia aproximada a la elipse: el circulo de radio RP_T estirado.
    let lobe = ((ax - D) * (RP_T / RP_R)).hypot(ay);
    let petal = lobe - RP_T;
    let inner = lobe - (RP_T - INSET);
    let center = rho - (RC + 0.008 * beat);
    let dot = (ax - DOT).hypot(ay - DOT) - RD;
    let cartouche = (ax.powi(3) + ay.powi(3)).cbrt() - R;

    // Los petalos salen de abajo del centro; su linea de adentro es una U
    // abierta hacia el.
    let outside_center = smoothstep(-0.004, 0.012, center);
    let lines = (line(petal, W, fp) + line(inner, 0.8 * W, fp) * smoothstep(D - 0.07, D, ax))
        * outside_center
        + line(center, W, fp)
        + line(cartouche, 1.2 * W, fp)
        + 0.6 * line(cartouche + 0.04, 0.8 * W, fp)
        + smoothstep(fp + 0.005, -fp, dot);
    let shadow_of = |d: f32| line(d - 2.4 * W, 1.6 * W, fp);
    Glyph {
        lines,
        shadow: shadow_of(petal) * outside_center + shadow_of(cartouche),
        petals: smoothstep(fp + 0.008, -fp - 0.008, petal) * outside_center,
        ground: smoothstep(0.0, 0.03, petal) * smoothstep(0.0, -0.03, cartouche + 0.04),
        core: smoothstep(fp + 0.006, -fp - 0.006, rho - 0.032 - 0.01 * beat),
        gap: smoothstep(-0.05, -0.03, center) * smoothstep(0.0, -0.015, center),
    }
}

pub fn shade(f: &Fragment, u: &Uniforms, s: &SunStyle) -> Vec3 {
    let t = u.time;
    let n = u.noise;
    // La superficie gira con la estrella.
    let p = f.local.normalize();
    // Radio sobre el disco tal como se ve: 0 en el centro, 1 en el limbo.
    let mu = f.normal.dot(f.view_dir()).clamp(0.0, 1.0);
    let r = (1.0 - mu * mu).sqrt();
    // Lo que cubre un pixel sobre la esfera de radio 1.
    let footprint = f.pos.length() * u.pixel_angle / u.sun_radius;
    // 0 en la agonia, 1 al final del estertor (se enfria).
    let dying = smoothstep(CALM_TEMPERATURE, CALM_TEMPERATURE - DYING_DROP, s.temperature);

    // El fuego da vueltas despacio alrededor del k'in y hierve: dos capas
    // de domain warping.
    let facing = p.dot(GLYPH_AXIS);
    let q = rotate(p, GLYPH_AXIS, t * 0.02);
    let boil = Vec3::new(0.0, 0.0, t * 0.05);
    let w = q * 2.6 + boil;
    let warp = Vec3::new(
        n.noise(w),
        n.noise(w + Vec3::new(13.1, 0.0, 0.0)),
        n.noise(w + Vec3::new(0.0, 27.3, 0.0)),
    );
    let qq = q * 5.5 + warp * 0.75;
    // Humo fino sobre regiones grandes que arden mas, motas brillantes y
    // lenguas finas.
    let smoke = n.fbm(qq + boil * 0.8, 4) * 0.5 + 0.5;
    let mass = n.fbm(q * 2.0 + warp * 0.5 + Vec3::new(4.0, 9.0, t * 0.02), 2) * 0.5 + 0.5;
    let burning = smoothstep(0.38, 0.72, mass);
    let fleck = n.fbm(qq * 3.0 + Vec3::new(5.0, 1.0, -t * 0.1), 3) * 0.5 + 0.5;
    let flecks = smoothstep(0.5, 0.76, fleck) * (0.25 + 0.75 * burning);
    // Lo mas fino se apaga de lejos, donde ya no cabe en un pixel.
    let detail = smoothstep(0.03, 0.01, footprint);
    let tongue = 1.0 - n.noise(qq * 1.6 + Vec3::new(3.0, 0.0, -t * 0.12)).abs();
    let tongues = tongue.powi(6) * burning * detail;
    // Granulacion: carriles oscuros donde un ruido cruza por cero, que se
    // abren cuando la estrella muere.
    let lane = n.noise(qq * 2.2 + Vec3::new(1.0, 7.0, t * 0.08)).abs();
    let lane = smoothstep(0.0, 0.22 + 0.18 * dying, lane);
    let lanes = 1.0 - (0.22 + 0.3 * dying) * (1.0 - lane) * detail;
    let dense = smoothstep(0.3, 0.75, smoke);
    let darkening = 0.72 + 0.28 * mu.sqrt();

    // Hacia el borde arde mas; las llamas solo cuentan junto al limbo.
    let rim = smoothstep(0.55, 1.0, r);
    let limb = smoothstep(0.82, 1.0, r);
    let fire = if limb > 0.0 {
        let (_, a, b) = limb_frame(u.sun_pos);
        limb_fire(f.normal.dot(b).atan2(f.normal.dot(a)), 0.0, t, n)
    } else {
        0.0
    };
    let mut heat = (0.16 + 0.34 * dense + 0.14 * mass + 0.5 * flecks + 0.12 * tongues)
        * darkening
        * lanes
        * (1.0 - 0.2 * dying)
        + rim * (0.02 + 0.08 * mass * mass)
        + limb * (0.15 + 0.5 * fire);

    // El k'in: cabe donde facing > 0.78; el corte va un poco mas afuera.
    if facing > FACING_CUT {
        let up = (Vec3::Y - GLYPH_AXIS * GLYPH_AXIS.y).normalize();
        let e2 = rotate(up, GLYPH_AXIS, TWIST);
        let e1 = e2.cross(GLYPH_AXIS);
        // El plano del glifo tiembla con el fuego.
        let x = p.dot(e1) + warp.x * 0.016;
        let y = p.dot(e2) + warp.y * 0.016;
        // De lado, el pixel cubre mas esfera.
        let g = glyph(x, y, footprint / mu.max(0.25), u.beat_at(0.0));
        // El brillo corre por los canales.
        let flow = smoothstep(-0.5, 0.6, n.noise(q * 9.0 + Vec3::new(0.0, 0.0, t * 0.3)));
        // Rayos que salen del centro por los petalos: ruido que cambia
        // rapido con el angulo y despacio con la distancia.
        let rays = if g.petals > 0.0 {
            let rho = x.hypot(y).max(1e-4);
            let ray = n.noise(Vec3::new(x / rho * 6.0, y / rho * 6.0, rho * 6.0 - t * 0.4));
            (1.0 - ray.abs()).powi(4)
        } else {
            0.0
        };
        // En el estertor el glifo falla por partes, como una brasa.
        let falter = if u.flares > 0.0 {
            let out = n.noise(Vec3::new(x * 5.0, y * 5.0, t * 2.2));
            1.0 - 0.75 * u.flares * smoothstep(-0.15, 0.35, out)
        } else {
            1.0
        };
        let glow = g.petals * (0.1 + 0.16 * rays)
            + g.lines * (0.2 + 0.3 * flow) * (0.6 + 0.4 * dense)
            + 0.35 * g.core;
        heat += falter * glow - 0.16 * g.shadow - 0.25 * heat * (g.ground + g.gap);
    }

    // El latido sale del centro del k'in como un anillo de luz.
    let angle = facing.clamp(-1.0, 1.0).acos();
    let front = u.beat_phase.fract() * 1.8;
    let ring = (-((angle - front) * 6.0).powi(2)).exp() * (1.0 - u.beat_phase.fract());
    heat += 0.1 * ring * u.heart;
    // Llamaradas del estertor: parches donde las motas estallan de luz.
    let mut flare = 0.0;
    if u.flares > 0.0 {
        let burst = n.noise(q * 3.0 + Vec3::new(0.0, t * 0.35, 0.0));
        flare = u.flares * smoothstep(0.25, 0.6, burst);
        heat += flare * (0.2 + 0.8 * flecks + 0.5 * tongues);
    }

    let mut color = gold(heat);
    // Al morir el oro se va a carmesi; las llamaradas siguen doradas.
    let red = dying * (1.0 - flare);
    color = Vec3::new(color.x, color.y * (1.0 - 0.45 * red), color.z * (1.0 - 0.6 * red));
    // Hebras carmesi en lo frio, donde un ruido cruza por cero.
    let veil = (1.0 - n.noise(qq * 0.45 + Vec3::new(7.0, 3.0, t * 0.03)).abs()).powi(10);
    color = color.lerp(CARMINE * (0.25 + 0.9 * heat), veil * smoothstep(0.6, 0.35, heat) * 0.35);
    // Si se calienta al colapsar, el oro se vuelve de cuerpo negro.
    let hot = smoothstep(CALM_TEMPERATURE * 1.05, CALM_TEMPERATURE * 3.0, s.temperature);
    if hot > 0.0 {
        let white = planckian_rgb(s.temperature * (0.6 + 0.4 * heat)) * color.luminance();
        color = color.lerp(white, hot);
    }
    // El estertor la enfria: un poco mas roja.
    let cool = (s.temperature / CALM_TEMPERATURE).min(1.0);
    color = Vec3::new(color.x, color.y * cool, color.z * cool * cool);
    color * (DISC_GAIN * s.intensity * u.gains.sun * (1.0 + 0.16 * u.beat))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sum(g: &Glyph) -> f32 {
        g.lines + g.shadow + g.petals + g.ground + g.core + g.gap
    }

    #[test]
    fn kin_has_the_symmetry_of_the_square() {
        for i in 0..2000 {
            let a = i as f32 * 0.0137;
            let rho = (i % 97) as f32 / 97.0 * 0.6;
            let (x, y) = (rho * a.cos(), rho * a.sin());
            let base = sum(&glyph(x, y, 0.003, 0.4));
            for (gx, gy) in [(-x, y), (x, -y), (y, x), (-y, -x)] {
                let other = sum(&glyph(gx, gy, 0.003, 0.4));
                assert!((base - other).abs() < 1e-4, "({x}, {y}) -> ({gx}, {gy})");
            }
        }
    }

    #[test]
    fn kin_ends_before_the_facing_cut() {
        // El shader solo dibuja el glifo donde facing > FACING_CUT, a menos
        // de sqrt(1 - FACING_CUT^2) del eje; el warp lo mueve 0.016.
        let edge = (1.0 - FACING_CUT * FACING_CUT).sqrt() - 0.016;
        for i in 0..4000 {
            let a = i as f32 * 0.0091;
            let rho = edge + (i % 37) as f32 / 37.0 * 0.4;
            for fp in [0.001, 0.01] {
                let g = glyph(rho * a.cos(), rho * a.sin(), fp, 1.0);
                assert!(sum(&g) < 1e-3, "rho {rho}: {}", sum(&g));
            }
        }
    }

    #[test]
    fn lines_keep_their_energy_when_blurred() {
        let integral = |fp: f32| {
            (-4000..4000).map(|i| line(i as f32 * 1e-4, 0.01, fp)).sum::<f32>() * 1e-4
        };
        let sharp = integral(0.0);
        for fp in [0.005, 0.02, 0.05] {
            assert!((integral(fp) - sharp).abs() < 0.01 * sharp, "fp {fp}");
        }
    }

    #[test]
    fn gold_gets_brighter_with_heat() {
        assert_eq!(gold(0.0), Vec3::ZERO);
        let mut last = -1.0;
        for i in 0..=140 {
            let lum = gold(i as f32 * 0.01).luminance();
            assert!(lum >= last, "heat {}", i as f32 * 0.01);
            last = lum;
        }
    }
}
