//! Color de un cuerpo negro a partir de su temperatura.
//!
//! Portado del Proyecto 2. Las estrellas radian aproximadamente como cuerpos
//! negros, asi que su color no es un parametro artistico: lo fija la
//! temperatura. Una supergigante roja a 3500 K se ve naranja rojiza; el disco
//! de un agujero negro a decenas de miles de kelvin, blanco azulado.
//!
//! # Como se calcula
//!
//! Integrar la ley de Planck contra las funciones de igualacion de color CIE
//! en cada pixel seria carisimo. En su lugar se usa una aproximacion analitica
//! del **lugar planckiano**: la curva que los cuerpos negros dibujan en el
//! diagrama de cromaticidad CIE 1931 al variar la temperatura. La aproximacion
//! de Kim et al. la reproduce con error despreciable entre 1667 K y 25000 K.
//!
//! De la cromaticidad `(x, y)` se pasa a XYZ con `Y = 1`, y de ahi a sRGB
//! lineal con la matriz estandar D65. El color sale normalizado a luminancia
//! 1: dice **de que color** brilla, no cuanto.

use super::Vec3;

/// Limites de validez de la aproximacion de Kim et al., en kelvin.
const MIN_TEMPERATURE: f32 = 1667.0;
const MAX_TEMPERATURE: f32 = 25_000.0;

/// Color de un cuerpo negro en sRGB lineal, normalizado a luminancia 1.
pub fn planckian_rgb(temperature: f32) -> Vec3 {
    let t = temperature.clamp(MIN_TEMPERATURE, MAX_TEMPERATURE);

    // Se trabaja en `u = 1000/T` para que los coeficientes queden en un orden
    // de magnitud manejable en `f32`.
    let u = 1000.0 / t;
    let u2 = u * u;
    let u3 = u2 * u;

    let x = if t < 4000.0 {
        -0.266_123_9 * u3 - 0.234_358_9 * u2 + 0.877_695_6 * u + 0.179_910
    } else {
        -3.025_847 * u3 + 2.107_037_9 * u2 + 0.222_634_7 * u + 0.240_390
    };

    let x2 = x * x;
    let x3 = x2 * x;
    let y = if t < 2222.0 {
        -1.106_381_4 * x3 - 1.348_110_2 * x2 + 2.185_558_3 * x - 0.202_196_83
    } else if t < 4000.0 {
        -0.954_947_6 * x3 - 1.374_185_9 * x2 + 2.091_37 * x - 0.167_488_67
    } else {
        3.081_758 * x3 - 5.873_387 * x2 + 3.751_13 * x - 0.370_014_83
    };

    if y.abs() < 1.0e-4 {
        return Vec3::ONE;
    }

    // Cromaticidad -> XYZ con luminancia unitaria.
    let big_y = 1.0;
    let big_x = x / y;
    let big_z = (1.0 - x - y) / y;

    // XYZ -> sRGB lineal, matriz estandar con blanco D65.
    let rgb = Vec3::new(
        3.2406 * big_x - 1.5372 * big_y - 0.4986 * big_z,
        -0.9689 * big_x + 1.8758 * big_y + 0.0415 * big_z,
        0.0557 * big_x - 0.2040 * big_y + 1.0570 * big_z,
    );

    rgb.max(Vec3::ZERO)
}
