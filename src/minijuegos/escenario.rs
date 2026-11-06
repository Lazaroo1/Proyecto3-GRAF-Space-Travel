//! La escenografia chapina de la carrera del Periferico que se hornea al
//! abrirla: los barriletes gigantes de Sumpango.
//!
//! - **El barrilete** es una rueda de papel de china en anillos: una orilla
//!   de bloques de colores, una greca maya, ocho gajos con motivos de la
//!   faja tejida (`transition::motif`) y al centro un sol de rayos con el
//!   quetzal o el volcan. Encima, las canas del armazon (cuatro diametros y
//!   un aro) se ven a contraluz, mas oscuras.
//! - **Contraluz**: el papel no se alumbra de frente, deja pasar la luz de
//!   atras. Lo que se ve es lo que emite: `color^1.6 (0.35 + 0.8 e^(-r^2 / 0.25))`
//!   (mas fuerte al centro, donde esta la luz; la potencia satura el papel,
//!   que con el tono saldria pastel) por la textura del papel: arrugas y
//!   fibras de baja frecuencia que multiplican (no suman gris: el papel no
//!   se ensucia ni pierde su color).
//! - **Halo**: la luz de esa emision difuminada (dos cajas de radio
//!   `lado / 48`, que juntas se parecen a una gaussiana), tenida de una luz
//!   calida saturada con algo del color del diseno, y sumada afuera del
//!   disco con alfa 0: el papel brilla hacia afuera. (Con el color
//!   difuminado tal cual, la mezcla de todos los papeles daba un aro beige.)
//! - **Mips**: se hornea a 512 x 512 con 2 x 2 muestras por pixel y se
//!   promedia de 2 en 2 (premultiplicado) hasta 32: de lejos el barrilete
//!   se lee de la imagen chica, ya promediada, y no titila.

use std::f32::consts::{PI, TAU};

use super::carrera::{tone, value_noise};
use super::hornear::{self, Sprite};
use crate::math::{Vec3, smoothstep};
use crate::parallel;
use crate::render::transition;

/// Los colores del papel de china: magenta, amarillo, turquesa, naranja,
/// verde y morado (luz lineal).
pub const PAPEL: [Vec3; 6] = [
    Vec3::new(0.95, 0.1, 0.55),
    Vec3::new(1.0, 0.8, 0.1),
    Vec3::new(0.1, 0.75, 0.8),
    Vec3::new(1.0, 0.45, 0.05),
    Vec3::new(0.2, 0.75, 0.2),
    Vec3::new(0.45, 0.15, 0.7),
];
/// Cuantos disenos se hornean y de que tamano.
pub const KITE_DESIGNS: usize = 4;
pub const KITE_SIZE: usize = 512;
/// Hasta donde llega la imagen en coordenadas del barrilete: el disco es
/// `r < 1`, lo de afuera es para el halo (los flecos se pintan en vivo).
pub const KITE_EXTENT: f32 = 1.15;
/// El mip mas chico.
const SMALLEST: usize = 32;
/// Los filetes blancos del papel.
const WHITE: Vec3 = Vec3::new(0.96, 0.93, 0.86);
/// La luz calida de atras que tine el halo (lineal, muy saturada: el tono
/// la aclara).
const GLOW: Vec3 = Vec3::new(1.0, 0.35, 0.06);

/// Hornea el barrilete `design` a `size` x `size` (512 en el juego) y su
/// cadena de mips hasta 32. Cada sprite es RGBA8 premultiplicado y ya con
/// tono; el halo va sumado con alfa 0 (el color pasa del alfa). El ancla es
/// el centro y la escala son pixeles por unidad del barrilete.
pub fn barrilete(design: usize, size: usize) -> Vec<Sprite> {
    let size = size.max(SMALLEST);
    let n = size * size;
    let texel = 2.0 * KITE_EXTENT / size as f32;
    // La emision lineal premultiplicada y el alfa, con 2 x 2 muestras.
    let mut lin = vec![[0.0f32; 4]; n];
    parallel::chunks_mut(&mut lin, size, 8, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let (mut sum, mut cov) = (Vec3::ZERO, 0.0);
            for sy in 0..2 {
                for sx in 0..2 {
                    let px = -KITE_EXTENT + (x as f32 + 0.25 + 0.5 * sx as f32) * texel;
                    let py = KITE_EXTENT - (y as f32 + 0.25 + 0.5 * sy as f32) * texel;
                    if px * px + py * py < 1.0 {
                        sum += backlit(design, px, py, texel);
                        cov += 1.0;
                    }
                }
            }
            *out = [sum.x * 0.25, sum.y * 0.25, sum.z * 0.25, cov * 0.25];
        }
    });
    // El halo: la emision difuminada dos veces.
    let emission: Vec<Vec3> = lin.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
    let radius = (size / 48).max(1);
    let halo = hornear::box_blur(&hornear::box_blur(&emission, size, size, radius), size, size, radius);
    // El tinte del halo: la luz calida con un 30 % del color del diseno,
    // normalizado a luminancia 1 (el halo conserva la luz que tenia).
    let tint = GLOW.lerp(PAPEL[(2 * design + 1) % 6], 0.3);
    let tint = tint * (1.0 / tint.luminance().max(1e-3));
    // Con tono: el papel con su alfa y el halo afuera (`1 - alfa`).
    let mut level = lin;
    parallel::chunks_mut(&mut level, size, 8, |y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            let a = p[3];
            let paper = if a > 0.0 { tone(Vec3::new(p[0], p[1], p[2]) * (1.0 / a)) * a } else { Vec3::ZERO };
            let c = paper + tone(tint * (0.4 * halo[y * size + x].luminance())) * (1.0 - a);
            *p = [c.x, c.y, c.z, a];
        }
    });
    let mut mips = Vec::new();
    let mut side = size;
    loop {
        mips.push(to_sprite(&level, side));
        if side / 2 < SMALLEST {
            break;
        }
        level = halve(&level, side);
        side /= 2;
    }
    mips
}

/// Los cuatro disenos, cada uno con sus mips.
pub fn barriletes() -> Vec<Vec<Sprite>> {
    (0..KITE_DESIGNS).map(|d| barrilete(d, KITE_SIZE)).collect()
}

/// La luz que deja pasar el papel en `(x, y)` (unidades del barrilete, `y`
/// arriba, adentro del disco): el color del papel, las canas a contraluz
/// (el color por 0.35), la luz de atras y la textura del papel,
/// `1 + 0.10 arruga + 0.05 fibra`: la arruga es ruido de unas 5 celdas por
/// unidad y la fibra se estira a lo ancho (3 por 36 celdas).
fn backlit(design: usize, x: f32, y: f32, texel: f32) -> Vec3 {
    let r = x.hypot(y);
    let a = y.atan2(x);
    let mut c = paper(design, x, y, r, a, texel);
    // Las canas: cuatro diametros (la distancia a la recta por el centro
    // con angulo `th` es `|x sin th - y cos th|`) y el aro en 0.95.
    let mut cane = (r - 0.95).abs();
    for k in 0..4 {
        let (s, co) = (k as f32 * PI / 4.0).sin_cos();
        cane = cane.min((x * s - y * co).abs());
    }
    let on = smoothstep(0.012 + 0.5 * texel, 0.012 - 0.5 * texel, cane);
    // El papel de color es mas puro que su color de pantalla: con el tono (la
    // gamma levanta el canal bajo) un amarillo `(1, 0.8, 0.1)` saldria
    // crema. Se satura con `c^1.6` antes de alumbrarlo.
    c = Vec3::new(c.x.max(0.0).powf(1.6), c.y.max(0.0).powf(1.6), c.z.max(0.0).powf(1.6)) * (1.0 - 0.65 * on);
    let light = 0.35 + 0.8 * (-r * r / 0.25).exp();
    let texture = 1.0 + 0.10 * value_noise(5.0 * x + 11.0, 5.0 * y) + 0.05 * value_noise(3.0 * x, 36.0 * y + 7.0);
    (c * (light * texture)).max(Vec3::ZERO)
}

/// El papel por anillos (`r` y el angulo `a` de `(x, y)`):
/// - de 0.88 a 1, bloques de color cada `2 pi / 36` con filetes blancos en
///   0.88 y en 1;
/// - de 0.70 a 0.88, la greca maya veinte veces alrededor;
/// - de 0.42 a 0.70, ocho gajos de color y blanco, cada uno con su motivo
///   (la `y` del motivo hacia afuera);
/// - adentro de 0.42, rayos de sol amarillos y naranja con el quetzal o el
///   volcan encima.
fn paper(design: usize, x: f32, y: f32, r: f32, a: f32, texel: f32) -> Vec3 {
    let s = 0.5 * texel;
    let filet = |c: Vec3, at: f32, half: f32| c.lerp(WHITE, smoothstep(half + s, half - s, (r - at).abs()));
    let turn = (a + PI) * (1.0 / TAU);
    if r > 0.88 {
        let block = (turn * 36.0) as usize;
        let c = PAPEL[(block + design) % 6];
        return filet(filet(c, 0.88, 0.012), 1.0, 0.02);
    }
    if r > 0.70 {
        // La greca sobre un fondo hondo: `u` a lo largo (un cuadro cada
        // veinteavo de vuelta) y `v` a lo ancho del anillo.
        let mut c = PAPEL[(design + 5) % 6] * 0.85;
        let u = (turn * 20.0).fract() - 0.5;
        let v = (r - 0.79) / 0.18;
        transition::motif(4, &mut c, u, v, texel / 0.18);
        return filet(c, 0.70, 0.008);
    }
    if r > 0.42 {
        let j = ((turn * 8.0) as usize).min(7);
        let mut c = if j % 2 == 0 { PAPEL[(j + design) % 6] } else { WHITE };
        // Las coordenadas del gajo: el arco desde su centro (a la derecha
        // vista desde afuera) y la distancia al radio del medio.
        let center = -PI + (j as f32 + 0.5) * TAU / 8.0;
        let da = a - center;
        let da = da - (da * (1.0 / TAU)).round() * TAU;
        let (u, v) = (-da * r / 0.3, (r - 0.56) / 0.3);
        transition::motif(design + j, &mut c, u, v, texel / 0.3);
        return filet(c, 0.42, 0.008);
    }
    // El sol: el borde de cada rayo se suaviza lo que cambia `sin(16 a)` en
    // medio texel: medio texel a radio `r` son `texel / 2r` radianes, y
    // `sin(16 a)` cambia hasta 16 por radian: `8 texel / r`.
    let w = (8.0 * texel / r.max(0.02)).min(1.0);
    let mut c = Vec3::new(1.0, 0.45, 0.05).lerp(Vec3::new(1.0, 0.8, 0.1), smoothstep(-w, w, (16.0 * a).sin()));
    transition::motif(if design % 2 == 0 { 0 } else { 1 }, &mut c, x / 0.8, y / 0.8, texel / 0.8);
    c
}

/// Promedio 2 x 2 de una imagen premultiplicada de `side` x `side`.
fn halve(image: &[[f32; 4]], side: usize) -> Vec<[f32; 4]> {
    let half = side / 2;
    let mut out = vec![[0.0f32; 4]; half * half];
    for y in 0..half {
        for x in 0..half {
            let at = |dx: usize, dy: usize| image[(2 * y + dy) * side + 2 * x + dx];
            let (a, b, c, d) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
            out[y * half + x] = std::array::from_fn(|k| 0.25 * (a[k] + b[k] + c[k] + d[k]));
        }
    }
    out
}

/// De la imagen en flotantes al sprite RGBA8. No lleva luz de LEDs
/// (`glow` vacio): quien lo pega la saca de su color (`0.45 max(rgb)`).
fn to_sprite(image: &[[f32; 4]], side: usize) -> Sprite {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    Sprite {
        width: side,
        height: side,
        pixels: image.iter().map(|p| [byte(p[0]), byte(p[1]), byte(p[2]), byte(p[3])]).collect(),
        glow: Vec::new(),
        texels: Vec::new(),
        spans: vec![(0, side.min(u16::MAX as usize) as u16); side],
        solid: vec![(0, 0); side],
        anchor: (side as f32 * 0.5, side as f32 * 0.5),
        scale: side as f32 / (2.0 * KITE_EXTENT),
        levels: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El barrilete: opaco al centro, transparente en la esquina, con halo
    /// afuera del disco, mips que bajan a la mitad hasta 32 y conservan el
    /// alfa promedio.
    #[test]
    fn a_kite_has_its_disk_its_halo_and_its_mips() {
        let mips = barrilete(1, 128);
        assert_eq!(mips.iter().map(|m| m.width).collect::<Vec<_>>(), vec![128, 64, 32]);
        let big = &mips[0];
        let px = |s: &Sprite, x: usize, y: usize| s.pixels[y * s.width + x];
        assert_eq!(px(big, 64, 64)[3], 255);
        assert_eq!(px(big, 0, 0), [0, 0, 0, 0]);
        // Justo afuera del disco (r ~ 1.05) no hay alfa pero si luz.
        let edge = (64.0 + 1.05 / (2.0 * KITE_EXTENT) * 128.0) as usize;
        let halo = px(big, edge, 64);
        assert_eq!(halo[3], 0);
        assert!(halo[0] as u32 + halo[1] as u32 + halo[2] as u32 > 0, "{halo:?}");
        // El alfa promedio se conserva de un mip al siguiente.
        let mean = |s: &Sprite| s.pixels.iter().map(|p| p[3] as f32).sum::<f32>() / s.pixels.len() as f32;
        assert!((mean(&mips[0]) - mean(&mips[2])).abs() < 1.5, "{} {}", mean(&mips[0]), mean(&mips[2]));
        // El disco es pi / (2 * 1.15)^2 de la imagen.
        let want = PI / (2.0 * KITE_EXTENT).powi(2) * 255.0;
        assert!((mean(big) - want).abs() < 3.0, "{} {want}", mean(big));
        for m in &mips {
            assert_eq!(m.pixels.len(), m.width * m.height);
            assert!(m.spans.len() == m.height && m.glow.is_empty());
        }
    }

    /// Los cuatro disenos son distintos y la luz no da NaN.
    #[test]
    fn the_designs_differ_and_the_light_is_finite() {
        let a = barrilete(0, 64);
        let b = barrilete(3, 64);
        assert!(a[0].pixels != b[0].pixels);
        for k in 0..200 {
            let t = k as f32 * 0.031;
            let (x, y) = (t.cos() * (k as f32 / 200.0), t.sin() * (k as f32 / 200.0));
            for d in 0..KITE_DESIGNS {
                let c = backlit(d, x, y, 0.01);
                assert!(c.x.is_finite() && c.y.is_finite() && c.z.is_finite() && c.x >= 0.0);
            }
        }
        assert!(backlit(0, 0.0, 0.0, 0.01).x.is_finite());
    }
}
