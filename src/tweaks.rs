//! Panel de ajustes en vivo (T): cambia parametros mientras el juego corre.
//!
//! En la demostracion se puede pedir cambiar un parametro y explicar que
//! pasa: el radio de un planeta (se regenera su terreno alrededor de la
//! camara, su atmosfera y sus objetos), su periodo orbital o de giro (sin
//! saltos: se corrige la fase), la intensidad de su shader, la semilla de
//! los objetos, el latido de la estrella, la exposicion, el bloom o el
//! factor de LOD. Flechas arriba/abajo eligen la fila, izquierda/derecha
//! cambian el valor y Shift da pasos diez veces mas grandes.

use crate::app::App;

/// Filas del panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Body,
    Radius,
    Orbit,
    Spin,
    Intensity,
    Seed,
    Pulse,
    Exposure,
    Bloom,
    Lod,
}

const ROWS: [Row; 10] = [
    Row::Body,
    Row::Radius,
    Row::Orbit,
    Row::Spin,
    Row::Intensity,
    Row::Seed,
    Row::Pulse,
    Row::Exposure,
    Row::Bloom,
    Row::Lod,
];

pub struct Tweaks {
    pub open: bool,
    row: usize,
    /// Cuerpo que se esta ajustando.
    body: usize,
}

impl Default for Tweaks {
    fn default() -> Self {
        Self {
            open: false,
            row: 0,
            body: 1,
        }
    }
}

impl Tweaks {
    pub fn select(&mut self, delta: i32) {
        self.row = (self.row as i32 + delta).rem_euclid(ROWS.len() as i32) as usize;
    }

    /// Cambia el valor de la fila elegida `steps` pasos (negativo baja).
    pub fn adjust(&mut self, app: &mut App, steps: i32) {
        let k = steps as f64;
        let body = self.body;
        // Multiplicativo: cada paso es un 5% (o 10%) del valor actual.
        let scale = |percent: f64| (1.0 + percent / 100.0).powf(k);
        match ROWS[self.row] {
            Row::Body => {
                let count = app.system.bodies.len() as i32;
                self.body = (self.body as i32 + steps.signum()).rem_euclid(count) as usize;
            }
            Row::Radius => {
                let radius = app.system.bodies[body].radius * scale(5.0);
                app.system.set_radius(body, radius);
            }
            Row::Orbit => {
                let period = app.system.bodies[body].orbit_period * scale(10.0);
                let t = app.time();
                app.system.set_orbit_period(body, period, t);
            }
            Row::Spin => {
                let period = app.system.bodies[body].spin_period * scale(10.0);
                let t = app.time();
                app.system.set_spin_period(body, period, t);
            }
            Row::Intensity => {
                let material = app.system.bodies[body].material;
                if let Some(gain) = app.gains.for_material(&material) {
                    *gain = (*gain + 0.1 * steps as f32).clamp(0.0, 5.0);
                }
            }
            Row::Seed => {
                let seed = app.system.prop_seed.wrapping_add(steps as u32);
                app.system.scatter_props(seed);
            }
            Row::Pulse => {
                let star = &mut app.system.bodies[0];
                star.pulse = (star.pulse + 0.004 * k).clamp(0.0, 0.2);
            }
            Row::Exposure => {
                app.renderer.exposure = (app.renderer.exposure * scale(10.0) as f32).clamp(0.05, 20.0);
            }
            Row::Bloom => {
                app.renderer.bloom_intensity = (app.renderer.bloom_intensity + 0.05 * steps as f32).clamp(0.0, 3.0);
            }
            Row::Lod => {
                app.lod.factor = (app.lod.factor + 0.2 * k).clamp(0.4, 8.0);
            }
        }
    }

    /// Lineas del panel; la elegida va marcada con `>`.
    pub fn lines(&self, app: &App) -> Vec<String> {
        let body = &app.system.bodies[self.body];
        let mut gains = app.gains;
        let gain = gains.for_material(&body.material).map_or("-".to_string(), |g| format!("{g:.1}"));
        let period = |p: f64| if p == 0.0 { "-".to_string() } else { format!("{p:.0} s") };
        let values = [
            format!("cuerpo: {}", body.name),
            format!("radio: {:.1}", body.radius),
            format!("periodo orbital: {}", period(if body.orbit_radius == 0.0 { 0.0 } else { body.orbit_period })),
            format!("periodo de giro: {}", period(body.spin_period)),
            format!("intensidad del shader: {gain}"),
            format!("semilla de objetos: {}", app.system.prop_seed),
            format!("latido de la estrella: {:.3}", app.system.bodies[0].pulse),
            format!("exposicion: {:.2}", app.renderer.exposure),
            format!("bloom: {:.2}", app.renderer.bloom_intensity),
            format!("factor de LOD: {:.1}", app.lod.factor),
        ];
        let mut lines = vec!["AJUSTES EN VIVO   (T cierra)".to_string()];
        lines.extend(
            values
                .iter()
                .enumerate()
                .map(|(i, v)| format!("{} {v}", if i == self.row { ">" } else { " " })),
        );
        lines.push("arriba/abajo: fila   izq/der: cambiar   Shift: x10".to_string());
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_changes_the_radius_period_and_intensity_of_the_chosen_body() {
        let mut app = App::new(64, 36).unwrap();
        let mut tweaks = Tweaks::default();
        let body = tweaks.body;
        let radius = app.system.bodies[body].radius;
        tweaks.select(1);
        tweaks.adjust(&mut app, 2);
        assert!((app.system.bodies[body].radius - radius * 1.05f64.powi(2)).abs() < 1e-9);
        let period = app.system.bodies[body].orbit_period;
        let before = app.system.bodies[body].position(app.time());
        tweaks.select(1);
        tweaks.adjust(&mut app, -1);
        assert!(app.system.bodies[body].orbit_period < period);
        assert!((app.system.bodies[body].position(app.time()) - before).length() < 1e-6, "sin saltos");
        tweaks.select(2);
        tweaks.adjust(&mut app, 5);
        let material = app.system.bodies[body].material;
        assert!((app.gains.for_material(&material).copied().unwrap() - 1.5).abs() < 1e-6);
        assert!(tweaks.lines(&app).iter().any(|l| l.starts_with("> intensidad")));
    }
}
