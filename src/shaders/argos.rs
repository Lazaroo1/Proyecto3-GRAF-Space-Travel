//! Argos: un planeta de carne con un ojo que te sigue por todo el sistema.
//!
//! # El ojo se mueve
//! El ojo no esta pegado a la carne: gira dentro de ella para mirar a la
//! nave (`ArgosLook::eye`, en el espacio del planeta) mientras el planeta
//! sigue girando debajo. El vertex shader (`lift`) mueve la carne en la
//! direccion radial: cerca del ojo la baja hasta una esfera exacta (el
//! globo), le sube la cornea y levanta un borde de parpados alrededor. La
//! abertura es una elipse en el plano del ojo, mas ancha que alta, con los
//! parpados a lo largo del eje del planeta. Al parpadear, la piel de los
//! parpados baja y sube sobre el globo hasta juntarse en medio. Cuando la
//! nave baja a la carne, el ojo se hunde entero (`presence`) y queda carne
//! con sus ojos chicos.
//!
//! # El iris
//! En polares alrededor del eje del ojo, anclado al ojo y no a la carne:
//! pupila negra que se dilata con el latido y se cierra con la nave cerca;
//! adentro, rojo hondo con estrias finas y criptas oscuras; la collareta, un
//! anillo ondulado rojo encendido; afuera, magenta que pasa a morado con
//! fibras radiales (ruido con mucha frecuencia a lo largo del angulo y poca a
//! lo largo del radio) y hebras lila; al borde, el anillo limbico oscuro. El
//! iris brilla un poco por dentro: se ve desde cualquier lado, de dia o de
//! noche. La cornea es mojada: GGX con muy poca rugosidad y un segundo
//! brillo, el de la luz difusa del cielo, que la hace verse humeda aunque la
//! estrella no le pegue de frente.
//!
//! # Esclera y carne
//! La esclera es rosa lila palido, mas roja hacia los parpados, con venas que
//! entran desde la carne hacia el iris (ruido estirado a lo largo del radio
//! del ojo, donde cruza por cero) y laten con la estrella; con la nave cerca
//! se inyecta de sangre. La carne es granate y mojada, con los pliegues del
//! relieve: crestas claras, surcos negros y venas moradas. El borde de los
//! parpados por dentro es mucosa rosa.
//!
//! La luz de la estrella llega casi sin color (`light`): con el rojo de la
//! estrella la carne y la esclera se veian color durazno.
//!
//! La normal del globo y la pendiente de la cornea y los parpados salen de
//! la misma altura que usa el vertex shader, derivada con diferencias
//! finitas.

use super::{Fragment, Uniforms, limb_haze, pbr};
use crate::math::{Vec3, smoothstep};
use crate::terrain::shapes::ARGOS_LID;

/// Lo que el ojo hace este frame.
#[derive(Clone, Copy, Debug)]
pub struct ArgosLook {
    /// Hacia donde mira, unitario en el espacio del planeta.
    pub eye: Vec3,
    /// Radio del planeta: el del globo del ojo.
    pub radius: f32,
    /// Que tan cerca esta la nave: 0 lejos, 1 encima.
    pub near: f32,
    /// Parpadeo: 0 abierto, 1 cerrado.
    pub blink: f32,
    /// 1 con el ojo afuera, 0 hundido en la carne.
    pub presence: f32,
}

impl Default for ArgosLook {
    fn default() -> Self {
        Self {
            eye: Vec3::X,
            radius: 1.0,
            near: 0.0,
            blink: 0.0,
            presence: 1.0,
        }
    }
}

/// Radios angulares (radianes) del iris y de la abertura de los parpados,
/// cuanto se estrecha la abertura a lo alto y ancho del borde de los
/// parpados.
pub const IRIS: f32 = 0.32;
pub const OPEN: f32 = 0.52;
const ALMOND: f32 = 0.85;
const LID_WIDTH: f32 = 0.1;
/// Cuanto sube la cornea sobre el globo y la piel de los parpados cerrados,
/// en unidades del mundo.
const CORNEA: f32 = 3.5;
const LID_SKIN: f32 = 4.5;

const DEEP_RED: Vec3 = Vec3::new(0.2, 0.006, 0.03);
const RED: Vec3 = Vec3::new(0.62, 0.03, 0.09);
const HOT: Vec3 = Vec3::new(1.0, 0.14, 0.25);
const MAGENTA: Vec3 = Vec3::new(0.5, 0.04, 0.24);
const PURPLE: Vec3 = Vec3::new(0.3, 0.04, 0.36);
const LAVENDER: Vec3 = Vec3::new(0.78, 0.5, 0.95);
const SCLERA: Vec3 = Vec3::new(0.62, 0.48, 0.58);
const SCLERA_EDGE: Vec3 = Vec3::new(0.4, 0.09, 0.14);
const VEIN: Vec3 = Vec3::new(0.26, 0.01, 0.03);
const MUCOSA: Vec3 = Vec3::new(0.62, 0.16, 0.22);
const LID: Vec3 = Vec3::new(0.3, 0.06, 0.08);
const FLESH: Vec3 = Vec3::new(0.07, 0.008, 0.015);
const CREST: Vec3 = Vec3::new(0.26, 0.04, 0.055);
const CREASE: Vec3 = Vec3::new(0.03, 0.003, 0.008);
const FLESH_VEIN: Vec3 = Vec3::new(0.12, 0.015, 0.08);

/// La luz de la estrella sobre Argos, casi sin color. Tambien la usan sus
/// ojos chicos.
pub fn light(sun: Vec3) -> Vec3 {
    sun.lerp(Vec3::splat(sun.luminance()), 0.65)
}

/// Ejes del plano del ojo: `y` sigue al eje del planeta (por ahi se cierran
/// los parpados) y `x` va de lado.
#[inline]
fn frame(eye: Vec3) -> (Vec3, Vec3) {
    let helper = if eye.y.abs() < 0.95 { Vec3::Y } else { Vec3::X };
    let y = (helper - eye * eye.dot(helper)).normalize();
    (y.cross(eye), y)
}

/// Donde cae un punto respecto al ojo.
#[derive(Clone, Copy)]
struct Spot {
    /// Angulo desde el centro del ojo.
    angle: f32,
    /// Distancia en la elipse de la abertura: 0 en el centro, 1 en el
    /// borde de los parpados, mas afuera en la carne.
    d: f32,
    /// Cuanto lo tapa la piel de los parpados al parpadear (0 a 1).
    cover: f32,
    /// Donde se juntan los parpados: 1 sobre la linea, para la arruga.
    seam: f32,
}

#[inline]
fn spot(p: Vec3, look: &ArgosLook, x: Vec3, y: Vec3) -> Spot {
    let angle = p.dot(look.eye).clamp(-1.0, 1.0).acos();
    let (px, py) = (p.dot(x), p.dot(y));
    let s = (px * px + py * py).sqrt().max(1e-6);
    // Coordenadas en la elipse: a lo ancho y a lo alto.
    let u = angle * px / s / OPEN;
    let w = angle * py / s / (OPEN * ALMOND);
    let d = (u * u + w * w).sqrt();
    // Los parpados bajan y suben hasta dejar abierta solo esta altura.
    let edge = (1.0 - look.blink) * (1.0 - u * u).max(0.0).sqrt();
    let cover = smoothstep(edge - 0.015, edge + 0.035, w.abs()) * smoothstep(1.02, 0.95, d);
    let seam =
        (-(w / 0.03).powi(2)).exp() * smoothstep(0.6, 0.95, look.blink) * smoothstep(1.0, 0.9, d);
    Spot {
        angle,
        d,
        cover,
        seam,
    }
}

/// Cuanto se baja la carne hacia el globo: del todo adentro del ojo y en una
/// pendiente corta alrededor, que los parpados tapan.
#[inline]
fn sink(d: f32) -> f32 {
    smoothstep(1.15, 0.92, d)
}

/// Lo que el ojo le suma al globo: la cornea donde esta abierto, la piel de
/// los parpados donde lo tapan y el borde de los parpados (unidades del
/// mundo).
#[inline]
fn relief(s: Spot) -> f32 {
    let socket = smoothstep(1.0, 0.9, s.d);
    let cornea = CORNEA * smoothstep(IRIS * 1.3, 0.0, s.angle) * socket * (1.0 - s.cover);
    let skin = LID_SKIN * s.cover * socket * (1.0 - 0.35 * s.seam);
    let rim = ARGOS_LID * (-((s.d - 1.05) * OPEN / LID_WIDTH).powi(2)).exp();
    cornea + skin + rim
}

/// Vertex shader: cuanto mover el vertice `local` (en el espacio del
/// planeta) en la direccion radial. Cerca del ojo baja la carne hasta el
/// globo; en el borde levanta los parpados.
pub fn lift(local: Vec3, look: &ArgosLook) -> f32 {
    if look.presence <= 0.0 {
        return 0.0;
    }
    let r = local.length();
    let p = local / r;
    let (x, y) = frame(look.eye);
    let s = spot(p, look, x, y);
    look.presence * (sink(s.d) * (look.radius - r) + relief(s))
}

pub fn shade(f: &Fragment, u: &Uniforms, look: &ArgosLook) -> Vec3 {
    let n = u.noise;
    let t = u.time;
    let p = f.local.normalize();
    let (x, y) = frame(look.eye);
    let s = spot(p, look, x, y);
    let presence = look.presence;
    let socket = smoothstep(1.0, 0.94, s.d) * presence;
    let open = socket * (1.0 - s.cover);
    let footprint = f.pos.length() * u.pixel_angle / look.radius;

    // Normal: la del globo donde la carne se bajo y la del relieve afuera,
    // mas la pendiente de la cornea y los parpados (diferencias finitas).
    let to_world = |v: Vec3| f.object_to_world.mul_vec(v);
    let ground = to_world(p)
        .normalize()
        .lerp(f.normal, 1.0 - sink(s.d) * presence)
        .normalize();
    let helper = if p.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let t1 = helper.cross(p).normalize();
    let t2 = p.cross(t1);
    let step = 0.004;
    let height = relief(s);
    let slope = |dir: Vec3| {
        let q = (p + dir * step).normalize();
        (relief(spot(q, look, x, y)) - height) / (step * look.radius)
    };
    let tilt = to_world(t1 * slope(t1) + t2 * slope(t2)) * presence;
    let normal = (ground - tilt).normalize();
    let l = f.light_dir(u);
    let nl = normal.dot(l);
    let beat = u.beat_at(0.15);
    let light = light(u.sun_light);

    // La carne, en el espacio del planeta (gira con el). Cada parte se
    // calcula solo donde se ve: la carne, la esclera y el iris.
    let crease = f.color.y;
    let mut flesh = Vec3::ZERO;
    if open < 1.0 {
        let (crest, tendon) = (f.color.x, f.color.z);
        let warp = Vec3::new(n.noise(p * 3.0), n.noise(p * 3.0 + Vec3::splat(4.7)), 0.0) * 0.12;
        let flesh_vein = (1.0 - n.noise((p + warp) * 13.0 + Vec3::splat(2.0)).abs()).powi(14)
            * smoothstep(0.03, 0.006, footprint * 13.0);
        flesh = FLESH
            .lerp(CREST, (crest * 0.7 + tendon * 0.5).min(1.0))
            .lerp(CREASE, 0.95 * smoothstep(0.35, 1.0, crease));
        flesh = flesh.lerp(FLESH_VEIN * (1.0 + 0.6 * beat), flesh_vein * 0.8);
        // Por dentro del borde, mucosa rosa y mojada.
        let waterline = smoothstep(0.95, 0.99, s.d) * smoothstep(1.05, 1.0, s.d) * presence;
        flesh = flesh.lerp(MUCOSA, waterline * 0.7);
        // La piel de los parpados cerrados: lisa, con arrugas a lo largo de
        // la rendija y la arruga oscura donde se juntan.
        if s.cover * socket > 0.0 {
            let w = p.dot(y) / (OPEN * ALMOND);
            let folds = 0.5 + 0.5 * (w * 70.0 + 3.0 * n.noise(p * 9.0)).sin();
            let lid = LID * (0.8 + 0.3 * folds) * (1.0 - 0.85 * s.seam);
            flesh = flesh.lerp(lid, s.cover * socket);
        }
    }

    // El ojo, anclado a sus propios ejes.
    let mut eye_color = Vec3::ZERO;
    let mut glow = Vec3::ZERO;
    let mut in_iris = 0.0;
    if open > 0.0 {
        let ep = Vec3::new(p.dot(x), p.dot(y), p.dot(look.eye));
        let rho = s.angle / IRIS;
        let phi = ep.y.atan2(ep.x);
        let (sn, cs) = phi.sin_cos();
        in_iris = smoothstep(1.03, 0.97, rho);
        // Esclera con venas que entran desde los parpados hacia el iris: ruido
        // estirado a lo largo del radio del ojo, torcido, donde cruza por
        // cero; las finas se ramifican de las gruesas.
        let blood = 0.3 + 0.7 * look.near;
        let mut sclera = Vec3::ZERO;
        if in_iris < 1.0 {
            let meander = n.noise(ep * 4.0 + Vec3::new(1.0, 2.0, 3.0)) * 0.35;
            let reach = smoothstep(IRIS * 1.05, OPEN, s.angle);
            let line = |field: f32, width: f32| smoothstep(width, width * 0.25, field.abs());
            let thick_field = n.fbm(
                Vec3::new(cs * 7.0, sn * 7.0, s.angle * 3.0) + Vec3::splat(meander),
                2,
            );
            let fine_field = n.fbm(
                Vec3::new(cs * 19.0, sn * 19.0, s.angle * 7.0 + 4.0) + Vec3::splat(meander * 1.5),
                2,
            );
            let second = n.fbm(
                Vec3::new(cs * 11.0, sn * 11.0, s.angle * 4.0 + 9.0) + Vec3::splat(meander),
                2,
            );
            let thick = (line(thick_field, 0.05 * (0.25 + reach) * (0.5 + blood))
                + line(second, 0.04 * reach * (0.5 + blood)))
            .min(1.0)
                * smoothstep(0.03, 0.01, footprint * 7.0 / s.angle.max(0.2));
            let fine = line(fine_field, 0.035 * reach * blood)
                * smoothstep(0.015, 0.005, footprint * 19.0 / s.angle.max(0.2));
            let veins = (thick + 0.8 * fine).min(1.0) * (0.75 + 0.25 * beat);
            sclera = SCLERA
                .lerp(
                    SCLERA_EDGE,
                    smoothstep(0.6, 1.0, s.d) * (0.45 + 0.35 * blood),
                )
                .lerp(VEIN, veins);
        }

        // El iris, en polares alrededor del eje del ojo.
        let mut iris = Vec3::ZERO;
        let mut pupil_mask = 0.0;
        if in_iris > 0.0 {
            let pupil = (0.3 + 0.05 * beat) * (1.0 - 0.38 * look.near);
            let fibers = (1.0
                - n.noise(Vec3::new(cs * 26.0, sn * 26.0, rho * 2.4 - t * 0.03))
                    .abs())
            .powi(5);
            let striae = (1.0 - n.noise(Vec3::new(cs * 60.0, sn * 60.0, rho * 4.0)).abs()).powi(3);
            let strands = (1.0
                - n.noise(Vec3::new(cs * 14.0, sn * 14.0, rho * 1.6 + 5.0))
                    .abs())
            .powi(14);
            let crypt = smoothstep(
                0.1,
                0.4,
                n.noise(Vec3::new(cs * 8.0, sn * 8.0, rho * 5.0 + 9.0)),
            );
            let collar_r = 0.52 + 0.05 * n.noise(Vec3::new(cs * 3.0, sn * 3.0, 2.0));
            let collar = (-((rho - collar_r) / 0.035).powi(2)).exp();
            iris = if rho < collar_r {
                DEEP_RED.lerp(RED, smoothstep(pupil, collar_r, rho))
                    * (0.45 + 0.75 * striae)
                    * (1.0 - 0.7 * crypt * smoothstep(collar_r, pupil, rho))
            } else {
                MAGENTA.lerp(PURPLE, smoothstep(collar_r, 0.85, rho)) * (0.4 + 0.8 * fibers)
            };
            iris = iris + HOT * (collar * (0.45 + 0.45 * striae));
            iris = iris.lerp(LAVENDER, strands * smoothstep(collar_r, 0.78, rho) * 0.55);
            iris = iris * (1.0 - 0.93 * smoothstep(0.76, 0.98, rho));
            pupil_mask = smoothstep(pupil + 0.03, pupil - 0.01, rho);
            iris = iris.lerp(Vec3::new(0.005, 0.0, 0.007), pupil_mask);
        }
        eye_color = sclera.lerp(iris, in_iris);
        // El iris brilla un poco por dentro; mas con el latido y la nave.
        let heat = 0.2 + 0.18 * beat + 0.3 * look.near;
        glow = iris * (in_iris * (1.0 - pupil_mask) * heat * open);
    }

    // Luz: el ojo es un espejo mojado; la carne, mojada pero mas aspera, y
    // deja pasar un poco de luz roja por debajo del terminador.
    let albedo = flesh.lerp(eye_color, open);
    let rough = 0.26 - 0.2 * open;
    let cavity = 1.0 - 0.5 * crease * (1.0 - socket);
    // La sombra se busca un poco arriba de la superficie, sobre la normal, y
    // mas con la luz rasante: en el globo liso y en los pliegues redondos el
    // mapa de sombras hacia franjas y dientes de sierra.
    let lifted = Fragment {
        pos: f.pos + normal * ((0.3 + 0.004 * f.pos.length()) * (1.0 + 3.0 * (1.0 - nl.abs()))),
        ..*f
    };
    let mut lit = pbr::shade_light(&lifted, u, albedo, rough, 0.0, normal, cavity, light);
    let wrap = ((nl + 0.35) / 1.35).max(0.0) - nl.max(0.0);
    lit += albedo * Vec3::new(0.9, 0.2, 0.25) * light * (wrap.max(0.0) * 0.5 * (1.0 - open));
    // Brillo humedo de la cornea con la luz del cielo, de arriba.
    let v = f.view_dir();
    let sky_light = (v + to_world(Vec3::Y) * 0.7).normalize();
    let h = (sky_light + v).normalize();
    let nh = normal.dot(h).max(0.0);
    let glint = nh.powf(900.0) * 1.6 + nh.powf(60.0) * 0.05;
    lit += Vec3::new(0.95, 0.9, 1.0) * (glint * open * (0.4 + 0.6 * in_iris));
    lit + glow * u.gains.argos + limb_haze(f, Vec3::new(0.4, 0.05, 0.13), 0.3, nl)
}
