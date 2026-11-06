//! La Ciudad de Guatemala de noche vista desde 400 m: la textura que pasa
//! debajo de la pista de la carrera del Periferico.
//!
//! - **Una textura en metros que se repite**: 2048 x 2048 texeles de 3 m
//!   (un cuadro de 6144 m). Se hornea una vez al abrir el juego, por filas
//!   en paralelo, y todo lo que lleva (zonas, barrancos, calles) es
//!   periodico con el cuadro para que no se vea la costura.
//! - **Zonas con Voronoi**: celdas de 768 m (8 x 8 en el cuadro) con un
//!   punto al azar cada una. El texel pertenece a la del punto mas cercano
//!   (`d1`), y donde la segunda queda casi igual de cerca (la distancia a
//!   la mediatriz, `(d2^2 - d1^2) / (2 |s2 - s1|)`, chica) pasa una
//!   calzada. Cada zona tiene su angulo de cuadricula, su cuadra, su
//!   densidad y cuantas luces son LED (blancas) en vez de sodio (naranja).
//! - **Barrancos**: las curvas de nivel cero de un ruido de valor periodico
//!   (`|n| < 0.07`): franjas negras que serpentean y parten la ciudad,
//!   como los barrancos de la capital, con alguna casita suelta adentro.
//! - **Calles y faroles**: en coordenadas de la zona giradas, `q = p / cuadra`
//!   y la distancia al borde de la cuadra es
//!   `e = (0.5 - max(|fract(q.x) - 0.5|, |fract(q.y) - 0.5|)) * cuadra`.
//!   Un farol cada 28 m a lo largo de la calle suma una gaussiana de luz.
//!   Adentro de las cuadras hay ventanas encendidas al azar.
//! - **Hitos**: la pista de La Aurora con sus luces de orilla, umbrales y
//!   la calle de rodaje azul, y el estadio con su cancha y sus torres.
//! - **Mips** por promedio 2 x 2 en luz lineal (decodificar, promediar,
//!   codificar) hasta 1 x 1: de lejos se lee el nivel que corresponde al
//!   tamano del pixel en el suelo y la ciudad no centellea.
//! - **Codificacion gamma 2** en un byte por canal: `b = sqrt(v / 4) * 255`
//!   y `v = (b / 255)^2 * 4`. Guarda luz de 0 a 4 con mas detalle en lo
//!   oscuro, que es casi toda la ciudad.

use crate::math::{Vec3, hash_u32, smoothstep};
use crate::parallel;

/// Metros por texel del nivel 0 y texeles por lado.
pub const METROS: f32 = 3.0;
const SIZE: usize = 2048;
/// Lo que mide el cuadro que se repite (m).
pub const TILE: f32 = METROS * SIZE as f32;
/// Lado de las celdas de las zonas (m) y cuantas caben en el cuadro.
const ZONE: f32 = 768.0;
const ZONES: i32 = 8;
/// La luz mas fuerte que se guarda.
const RANGE: f32 = 4.0;

struct Nivel {
    size: usize,
    /// Texeles por metro de este nivel.
    inv: f32,
    tex: Vec<[u8; 4]>,
}

/// La ciudad horneada con sus mips.
pub struct Ciudad {
    levels: Vec<Nivel>,
    /// De byte a luz lineal: `(b / 255)^2 * 4`.
    decode: [f32; 256],
}

#[inline]
fn encode(v: f32) -> u8 {
    ((v.clamp(0.0, RANGE) / RANGE).sqrt() * 255.0 + 0.5) as u8
}

#[inline]
fn pack(c: Vec3) -> [u8; 4] {
    [encode(c.x), encode(c.y), encode(c.z), 255]
}

/// Un hash a `[0, 1)` con los 24 bits altos (lo usa tambien la carrera).
#[inline]
pub(crate) fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// Hash de dos enteros y una sal.
#[inline]
fn hash2(a: i32, b: i32, salt: u32) -> u32 {
    hash_u32((a as u32).wrapping_mul(0x9E37_79B9) ^ (b as u32).wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35))
}

/// Ruido de valor periodico de -1 a 1: como el de la pista, pero los
/// indices de la red se toman modulo `period`, asi `vn_wrap(x + period, y)`
/// da lo mismo que `vn_wrap(x, y)`.
#[inline]
fn vn_wrap(x: f32, y: f32, period: i32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i32, yi as i32);
    let corner = |dx: i32, dy: i32| unit(hash2((xi + dx).rem_euclid(period), (yi + dy).rem_euclid(period), 0x0B5E)) * 2.0 - 1.0;
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

/// Donde va el texel `(x, y)` de un nivel de lado `size` (potencia de 2,
/// al menos 4) guardado en bloques de 4 x 4: los 16 texeles de un bloque
/// (64 bytes) caen en la misma linea de cache, asi una fila de la pantalla
/// que cruza la textura en diagonal no trae una linea por texel.
#[inline]
fn tile_index(size: usize, x: usize, y: usize) -> usize {
    (((y >> 2) * (size >> 2) + (x >> 2)) << 4) | ((y & 3) << 2) | (x & 3)
}

/// Arma un nivel de lado `size` con `f(x, y)` por texel, en paralelo y
/// ya guardado en bloques de 4 x 4 (si mide al menos 4): una tira de 4
/// filas de texeles es un pedazo contiguo de `4 size` texeles, y adentro
/// el texel `(x, y)` va en `(x / 4) * 16 + (y % 4) * 4 + x % 4`.
fn build_level(size: usize, min_per_worker: usize, f: impl Fn(usize, usize) -> [u8; 4] + Sync) -> Vec<[u8; 4]> {
    let mut tex = vec![[0u8; 4]; size * size];
    if size >= 4 {
        parallel::chunks_mut(&mut tex, 4 * size, min_per_worker, |band, chunk| {
            for dy in 0..4 {
                let y = band * 4 + dy;
                for x in 0..size {
                    chunk[((x >> 2) << 4) | (dy << 2) | (x & 3)] = f(x, y);
                }
            }
        });
    } else {
        for y in 0..size {
            for x in 0..size {
                tex[y * size + x] = f(x, y);
            }
        }
    }
    tex
}

/// El texel `(x, y)` de un nivel de lado `size` armado por `build_level`.
#[inline]
fn fetch(tex: &[[u8; 4]], size: usize, x: usize, y: usize) -> [u8; 4] {
    if size >= 4 { tex[tile_index(size, x, y)] } else { tex[y * size + x] }
}

/// Lleva una diferencia de posiciones al cuadro centrado en 0 (la ciudad
/// se repite cada `TILE` metros).
#[inline]
fn wrap(d: f32) -> f32 {
    d - (d / TILE).round() * TILE
}

/// Una gaussiana de radio `r` (m): `exp(-(d / r)^2)`.
#[inline]
fn gauss(dx: f32, dz: f32, r: f32) -> f32 {
    (-(dx * dx + dz * dz) / (r * r)).exp()
}

/// La luz lineal de la ciudad en el punto `(x, z)` en metros.
fn texel(x: f32, z: f32) -> Vec3 {
    // a) Zonas: Voronoi con un punto al azar por celda.
    let (gx, gz) = ((x / ZONE).floor() as i32, (z / ZONE).floor() as i32);
    let mut best = (f32::MAX, f32::MAX);
    let mut site = [(0.0f32, 0.0f32); 2];
    let mut cell = [(0i32, 0i32); 2];
    for dj in -1..=1 {
        for di in -1..=1 {
            let (ci, cj) = (gx + di, gz + dj);
            let (wi, wj) = (ci.rem_euclid(ZONES), cj.rem_euclid(ZONES));
            let h = hash2(wi, wj, 1);
            let sx = (ci as f32 + 0.15 + 0.7 * unit(h)) * ZONE;
            let sz = (cj as f32 + 0.15 + 0.7 * unit(hash_u32(h ^ 0x51ED))) * ZONE;
            let d2 = (x - sx) * (x - sx) + (z - sz) * (z - sz);
            if d2 < best.0 {
                best = (d2, best.0);
                site = [(sx, sz), site[0]];
                cell = [(wi, wj), cell[0]];
            } else if d2 < best.1 {
                best.1 = d2;
                site[1] = (sx, sz);
                cell[1] = (wi, wj);
            }
        }
    }
    let h = hash2(cell[0].0, cell[0].1, 7);
    let zone10 = cell[0] == (3, 4);
    let angle = [0.0f32, 12.0, 27.0, 45.0][(h & 3) as usize].to_radians();
    let block = 70.0 + 40.0 * ((h >> 2) & 255) as f32 / 255.0;
    let mut dens = 0.3 + 0.7 * ((h >> 10) & 255) as f32 / 255.0;
    let mut led = 0.15 + 0.35 * ((h >> 18) & 255) as f32 / 255.0;
    if zone10 {
        dens = 1.0;
        led = 0.7;
    }
    // La distancia exacta a la mediatriz entre las dos zonas mas cercanas.
    let (sep_x, sep_z) = (site[1].0 - site[0].0, site[1].1 - site[0].1);
    let sep = sep_x.hypot(sep_z).max(1.0);
    let bisector = (best.1 - best.0) / (2.0 * sep);

    // b) Barrancos: donde el ruido periodico pasa por cero.
    let n = vn_wrap(x / 1536.0, z / 1536.0, 4) + 0.5 * vn_wrap(x / 768.0 + 7.3, z / 768.0 + 7.3, 8);
    let ravine = smoothstep(0.08, 0.05, n.abs());
    // Manchas oscuras adentro de las zonas (predios, parques, colonias
    // ralas): ruido periodico a dos escalas que baja la densidad local.
    let patch = 0.6 * vn_wrap(x / 256.0, z / 256.0, 24) + 0.4 * vn_wrap(x / 96.0, z / 96.0, 64);
    let dens_here = dens * (0.12 + 0.88 * smoothstep(-0.4, 0.25, patch));

    let mut ground = Vec3::new(0.004, 0.004, 0.006);
    let mut lights = Vec3::ZERO;

    // c) Calles: la cuadricula de la zona, en coordenadas relativas a su
    // punto (asi una zona partida por la costura del cuadro sigue igual).
    let (lx, lz) = (x - site[0].0, z - site[0].1);
    let (s, c) = angle.sin_cos();
    let (rx, rz) = (lx * c + lz * s, -lx * s + lz * c);
    let (qx, qz) = (rx / block, rz / block);
    let (fx, fz) = (qx - qx.floor() - 0.5, qz - qz.floor() - 0.5);
    let e = (0.5 - fx.abs().max(fz.abs())) * block;
    // La calle mas cercana: la de x constante si se esta mas cerca de ese
    // borde, y la coordenada que corre a lo largo de ella.
    let (along, street) = if fx.abs() > fz.abs() { (rz, qx.round() as i32 * 2) } else { (rx, qz.round() as i32 * 2 + 1) };
    if e < 3.0 {
        ground = Vec3::new(0.01, 0.008, 0.006);
    }
    // Un farol cada 28 m, corrido hasta 4 m al azar, y uno de cada cinco
    // apagado: asi la cuadricula no se ve de tela.
    let lamp = (along / 28.0).floor() as i32;
    let lh = hash2(street ^ (h as i32), lamp, 3);
    let jitter = (unit(hash_u32(lh)) - 0.5) * 8.0;
    let df = ((along - (lamp as f32 + 0.5) * 28.0 - jitter).abs()).hypot(e - 1.5);
    if unit(hash_u32(lh ^ 0x0FF)) > 0.2 * (2.0 - dens_here) {
        let lamp_color = if unit(lh) < led { Vec3::new(0.75, 0.85, 1.0) * 2.6 } else { Vec3::new(1.0, 0.52, 0.16) * 3.2 };
        lights = lights + lamp_color * ((-(df / 3.2) * (df / 3.2)).exp() * dens_here);
    }

    // e) Adentro de las cuadras: ventanas encendidas en subceldas de 6 m.
    if e >= 3.0 {
        let (si, sj) = ((rx / 6.0).floor(), (rz / 6.0).floor());
        let wh = hash2(si as i32 ^ (h as i32), sj as i32, 5);
        if unit(wh) < 0.16 * dens_here {
            let (wx, wz) = (rx - (si + 0.5) * 6.0, rz - (sj + 0.5) * 6.0);
            let window = if wh & 0xF < 13 { Vec3::new(1.0, 0.75, 0.45) * 0.5 } else { Vec3::new(0.5, 0.6, 1.0) * 0.25 };
            lights = lights + window * gauss(wx, wz, 1.6);
        }
    }
    // Un velo de sodio muy tenue (la luz que rebota en el aire bajo).
    lights = lights + Vec3::new(1.0, 0.5, 0.18) * (0.002 * dens_here);

    // d) Las calzadas entre zonas: asfalto oscuro con un rosario de faroles
    // de sodio a cada lado y carros (faros blancos de un lado y stops rojos
    // del otro). Se leen como hileras de puntos, no como rayas.
    if bisector < 9.5 {
        // `along` se mide desde el punto medio de los dos sitios y con un
        // sentido que no depende de que lado se este (el de la zona menor
        // manda): asi es periodico con el cuadro y los dos lados lo
        // comparten.
        let first = cell[0] < cell[1];
        let flip = if first { 1.0 } else { -1.0 };
        let (tx, tz) = (-sep_z / sep * flip, sep_x / sep * flip);
        let (mx, mz) = ((site[0].0 + site[1].0) * 0.5, (site[0].1 + site[1].1) * 0.5);
        let along = (x - mx) * tx + (z - mz) * tz;
        let (lo, hi) = if first { (cell[0], cell[1]) } else { (cell[1], cell[0]) };
        let key = (lo.0 * ZONES + lo.1) * 64 + hi.0 * ZONES + hi.1;
        let salt = key as f32 * 0.37;
        // Media calzada de 3.5 a 6.5 m que cambia despacio a lo largo, y
        // tramos mas o menos alumbrados.
        let half = 5.0 + 1.5 * vn_wrap(along / 220.0, salt, 4096);
        let strength = 0.55 + 0.45 * vn_wrap(along / 90.0 + 31.0, salt, 4096);
        let road = smoothstep(half, half - 1.0, bisector);
        ground = ground.lerp(Vec3::new(0.008, 0.007, 0.006), road);
        lights = lights * (1.0 - road);
        // Un farol cada 30 m en la orilla de cada lado (los de un lado
        // corridos 15 m) y uno de cada ocho apagado.
        let offset = if first { 0.0 } else { 15.0 };
        let slot = ((along - offset) / 30.0).floor();
        let lh = hash2(key, slot as i32 * 2 + first as i32, 9);
        if unit(lh) > 0.12 {
            let da = along - offset - (slot + 0.5) * 30.0;
            lights = lights + Vec3::new(1.0, 0.55, 0.2) * (2.6 * strength * gauss(da, bisector - (half - 0.8), 2.6));
        }
        // Los carros, en un carril por lado.
        if bisector < half {
            let cslot = (along / 12.0).floor();
            let ch = hash2(key, cslot as i32 * 2 + first as i32, 11);
            if ch & 3 == 0 {
                let car = if first { Vec3::new(1.0, 0.95, 0.85) * 2.2 } else { Vec3::new(1.0, 0.06, 0.03) * 1.8 };
                let da = along - (cslot + 0.5) * 12.0 + (unit(hash_u32(ch)) - 0.5) * 4.0;
                lights = lights + car * gauss(da * 0.5, bisector - half * 0.45, 1.3);
            }
        }
    }

    // Zona 10: las luces rojas de aviacion en los edificios altos. El hash
    // va con el texel modulo el cuadro, para que se repita.
    let size = SIZE as i32;
    let th = hash2(((x / METROS).floor() as i32).rem_euclid(size), ((z / METROS).floor() as i32).rem_euclid(size), 13);
    if zone10 && unit(th) < 0.0004 {
        lights = lights + Vec3::new(1.0, 0.05, 0.02) * 3.0;
    }

    // El barranco apaga todo, menos alguna casita.
    let dark = Vec3::new(0.002, 0.003, 0.003);
    ground = ground.lerp(dark, ravine);
    lights = lights * (1.0 - ravine);
    if unit(th ^ 0xA5A5_5A5A) < 0.002 {
        lights = lights + Vec3::new(1.0, 0.7, 0.4) * (0.5 * ravine);
    }

    let mut color = ground + lights;
    landmarks(x, z, &mut color);
    color
}

/// La Aurora y el estadio, uno de cada uno por cuadro.
fn landmarks(x: f32, z: f32, color: &mut Vec3) {
    // La pista de aterrizaje: 2800 x 45 m girada 20 grados, en su campo
    // oscuro, con luces de orilla cada 60 m, el umbral verde en una punta y
    // el rojo en la otra, y la calle de rodaje azul a 150 m.
    let (dx, dz) = (wrap(x - 1500.0), wrap(z - 4200.0));
    let (s, c) = 20.0f32.to_radians().sin_cos();
    let (a, b) = (dx * c + dz * s, -dx * s + dz * c);
    if a.abs() < 1650.0 && b.abs() < 320.0 {
        let field = smoothstep(1650.0, 1550.0, a.abs()) * smoothstep(320.0, 260.0, b.abs());
        let mut airport = Vec3::new(0.005, 0.006, 0.005);
        if a.abs() < 1400.0 && b.abs() < 22.5 {
            airport = Vec3::new(0.006, 0.006, 0.008);
        }
        // Luces de orilla.
        let edge_a = a - ((a / 60.0).round() * 60.0);
        if a.abs() < 1400.0 {
            airport = airport + Vec3::new(1.0, 1.0, 0.95) * (2.5 * gauss(edge_a, b.abs() - 22.5, 2.0));
            // La calle de rodaje.
            let taxi_a = a - ((a / 30.0).round() * 30.0);
            airport = airport + Vec3::new(0.25, 0.45, 1.0) * (1.5 * gauss(taxi_a, b - 150.0, 2.0));
        }
        // Los umbrales: una fila de luces de lado a lado en cada punta.
        if b.abs() < 26.0 {
            let row_b = b - ((b / 6.0).round() * 6.0);
            airport = airport + Vec3::new(0.2, 1.0, 0.3) * (2.0 * gauss(row_b, a + 1400.0, 2.0));
            airport = airport + Vec3::new(1.0, 0.1, 0.05) * (2.0 * gauss(row_b, a - 1400.0, 2.0));
        }
        // La terminal, calida, del otro lado.
        if (-500.0..500.0).contains(&a) {
            airport = airport + Vec3::new(1.0, 0.75, 0.45) * (0.6 * gauss(0.0, b + 240.0, 12.0));
        }
        *color = color.lerp(airport, field);
    }
    // El estadio: elipse de 110 x 80 m de semiejes, cancha verde, graderio
    // calido y cuatro torres en las diagonales.
    let (dx, dz) = (wrap(x - 4400.0), wrap(z - 1600.0));
    let r = (dx / 110.0).hypot(dz / 80.0);
    if r < 1.6 {
        if r < 1.0 {
            let inside = if r < 0.62 { Vec3::new(0.15, 0.6, 0.2) * 0.6 } else { Vec3::new(1.0, 0.8, 0.5) * 0.8 };
            *color = inside;
        }
        for (tx, tz) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let (px, pz) = (tx * 110.0 * 0.85, tz * 80.0 * 0.85);
            *color = *color + Vec3::new(1.0, 1.0, 0.95) * (4.0 * gauss(dx - px, dz - pz, 3.0));
        }
    }
}

impl Ciudad {
    /// Hornea la ciudad y sus mips.
    pub fn new() -> Self {
        let mut decode = [0.0f32; 256];
        for (b, d) in decode.iter_mut().enumerate() {
            let v = b as f32 / 255.0;
            *d = v * v * RANGE;
        }
        let base = build_level(SIZE, 1, |x, y| pack(texel((x as f32 + 0.5) * METROS, (y as f32 + 0.5) * METROS)));
        let mut levels = vec![Nivel { size: SIZE, inv: 1.0 / METROS, tex: base }];
        while levels.last().is_some_and(|l| l.size > 1) {
            let (prev, src) = levels.last().map_or((&[][..], 1), |l| (&l.tex[..], l.size));
            let half = src / 2;
            // Desde el nivel 2, un poco mas de luz: las luces sueltas
            // promediadas con lo oscuro se apagarian de lejos. Se aplica una
            // vez al armar el nivel 2 y los de arriba lo heredan al promediar.
            let gain = if levels.len() == 2 { 1.08 } else { 1.0 } * 0.25;
            let tex = build_level(half, 64, |x, y| {
                let mut sum = [0.0f32; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = fetch(prev, src, 2 * x + dx, 2 * y + dy);
                    for k in 0..3 {
                        sum[k] += decode[p[k] as usize];
                    }
                }
                [encode(sum[0] * gain), encode(sum[1] * gain), encode(sum[2] * gain), 255]
            });
            let size = levels.len();
            levels.push(Nivel { size: half, inv: 1.0 / (METROS * (1u32 << size) as f32), tex });
        }
        Self { levels, decode }
    }

    /// El texel `(x, y)` del nivel `level`, guardado en bloques.
    #[inline]
    fn texel_at(&self, level: usize, x: usize, y: usize) -> [u8; 4] {
        let l = &self.levels[level];
        fetch(&l.tex, l.size, x, y)
    }

    /// Bilineal en el nivel `level`, con la ciudad repetida.
    #[inline]
    fn bilinear(&self, level: usize, x: f32, z: f32) -> Vec3 {
        let l = &self.levels[level];
        let mask = l.size - 1;
        let scale = l.inv;
        let (u, v) = (x * scale - 0.5, z * scale - 0.5);
        let (u0, v0) = (u.floor(), v.floor());
        let (fx, fy) = (u - u0, v - v0);
        let (x0, y0) = ((u0 as i32 as usize) & mask, (v0 as i32 as usize) & mask);
        let (x1, y1) = ((x0 + 1) & mask, (y0 + 1) & mask);
        let d = &self.decode;
        let t = |x: usize, y: usize| {
            let p = self.texel_at(level, x, y);
            Vec3::new(d[p[0] as usize], d[p[1] as usize], d[p[2] as usize])
        };
        let top = t(x0, y0).lerp(t(x1, y0), fx);
        let bottom = t(x0, y1).lerp(t(x1, y1), fx);
        top.lerp(bottom, fy)
    }

    /// La luz lineal en `(x, z)` metros con el nivel de detalle `lod`
    /// (fraccionario). Es trilineal "angosta": solo se mezclan los dos mips
    /// vecinos cuando la parte fraccionaria esta entre 0.3 y 0.7
    /// (`f' = (f - 0.3) / 0.4`); afuera de esa franja se lee uno solo. Como
    /// el `lod` cambia suave en la pantalla, la transicion sigue sin
    /// costura y se leen la mitad de texeles en promedio.
    #[inline]
    pub fn sample(&self, x: f32, z: f32, lod: f32) -> Vec3 {
        let top = (self.levels.len() - 1) as f32;
        let lod = lod.clamp(0.0, top);
        let l0 = lod.floor() as usize;
        let f = ((lod - l0 as f32 - 0.3) * 2.5).clamp(0.0, 1.0);
        if f <= 0.0 || l0 + 1 >= self.levels.len() {
            return self.bilinear(l0, x, z);
        }
        if f >= 1.0 {
            return self.bilinear(l0 + 1, x, z);
        }
        self.bilinear(l0, x, z).lerp(self.bilinear(l0 + 1, x, z), f)
    }
}

impl Default for Ciudad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_periodic_noise_repeats() {
        for &(x, y) in &[(0.3f32, 1.7f32), (2.9, 3.4), (-1.2, 0.5)] {
            let a = vn_wrap(x, y, 4);
            let b = vn_wrap(x + 4.0, y - 8.0, 4);
            assert!((a - b).abs() < 1e-5, "{a} {b}");
            assert!((-1.0..=1.0).contains(&a));
        }
    }

    /// Todo el texel se repite con el cuadro: zonas, barrancos, calles,
    /// calzadas (medidas desde el punto medio de sus dos sitios), ventanas,
    /// hitos y las luces de aviacion.
    #[test]
    fn the_city_texel_is_periodic() {
        let mut h = 0x1234_5678u32;
        let mut next = || {
            h = hash_u32(h);
            unit(h) * TILE
        };
        for _ in 0..400 {
            let (x, z) = (next(), next());
            let a = texel(x, z);
            for (dx, dz) in [(TILE, 0.0), (0.0, TILE), (-TILE, TILE)] {
                let b = texel(x + dx, z + dz);
                assert!((a - b).length() < 1e-2 * (1.0 + a.length()), "({x}, {z}): {a:?} contra {b:?}");
            }
        }
    }

    /// Guarda el nivel 1 con tono en `ciudad.png` para mirarla
    /// (`cargo test --release dump_city -- --ignored`).
    #[test]
    #[ignore = "escribe una imagen"]
    fn dump_city() {
        let city = Ciudad::new();
        let size = city.levels[1].size;
        let mut rgba = Vec::with_capacity(size * size * 4);
        for p in (0..size * size).map(|i| city.texel_at(1, i % size, i / size)) {
            for k in 0..3 {
                let v = city.decode[p[k] as usize];
                rgba.push(((1.0 - (-1.3 * v).exp()).powf(1.0 / 2.2) * 255.0) as u8);
            }
            rgba.push(255);
        }
        let path = std::env::temp_dir().join("ciudad.png");
        crate::render::png::write_rgba(&path, size as u32, size as u32, &rgba).expect("png");
        println!("{}", path.display());
    }

    #[test]
    fn the_city_has_lights_ravines_and_repeats() {
        let city = Ciudad::new();
        assert_eq!(city.levels.len(), 12);
        // Se repite sin costura.
        let a = city.sample(123.4, 987.6, 0.0);
        let b = city.sample(123.4 + TILE, 987.6 - TILE, 0.0);
        assert!((a - b).length() < 1e-4);
        // El promedio (el mip de 1 x 1) es una ciudad de noche: naranja y
        // ni negra ni lavada.
        let mean = city.sample(0.0, 0.0, 11.0);
        assert!(mean.x > 0.03 && mean.x < 0.6, "{mean:?}");
        assert!(mean.x > mean.z, "{mean:?}");
        // Hay texeles oscuros (barrancos) y luces fuertes.
        let base = &city.levels[0].tex;
        let dark = base.iter().filter(|p| p[0] < 20).count() as f32 / base.len() as f32;
        let bright = base.iter().filter(|p| p[0] > 180).count();
        assert!(dark > 0.02, "{dark}");
        assert!(bright > 1000, "{bright}");
    }
}
