//! Pool de hilos persistentes para las pasadas de un frame (fork-join).
//!
//! Evolucion del pool del Proyecto 2. Alla cada frame tenia pocas pasadas con
//! mucho trabajo. Aqui un frame tiene unas quince (vertices, triangulos,
//! raster, shading, cada nivel del bloom, tonemap) y varias duran decenas de
//! microsegundos. En el pool anterior los hilos dormian en una cola y se
//! despertaban uno tras otro: despertar a quince hilos costaba mas que el
//! trabajo de toda la pasada.
//!
//! Ahora:
//! - Publicar un trabajo despierta a **todos** los hilos a la vez.
//! - Al terminar, cada hilo espera activamente unos cientos de microsegundos
//!   antes de dormir: dentro de un frame, la siguiente pasada casi siempre
//!   llega antes, y el hilo la toma sin pasar por el sistema operativo.
//! - Los indices se reparten con un contador atomico: el hilo que termina un
//!   bloque toma el siguiente, asi los nucleos rapidos (P) no esperan a los
//!   lentos (E) del procesador hibrido.

use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;

type Panic = Box<dyn Any + Send>;

/// Vueltas de espera activa antes de dormir (cada una es una pausa del CPU,
/// del orden de 40 a 140 ciclos: unos cientos de microsegundos en total).
///
/// No conviene alargarla. En una laptop, los hilos que giran consumen el
/// presupuesto de potencia y le quitan recursos al hilo gemelo de su nucleo:
/// el procesador baja el reloj y todo el frame se vuelve mas lento. Medido:
/// con 10 veces mas vueltas, la mediana del frame paso de 24 a 65 ms. Por eso
/// el renderer hace pocas pasadas grandes en vez de muchas chicas.
const SPIN_LIMIT: u32 = 20_000;

/// Trabajo publicado: una funcion por indice y el contador del reparto.
struct Job<'a> {
    run: &'a (dyn Fn(usize) + Sync),
    count: usize,
    next: AtomicUsize,
    /// Hilos del pool que se unieron y aun no terminan.
    attached: AtomicUsize,
    panic: Mutex<Option<Panic>>,
}

impl Job<'_> {
    /// Toma indices hasta que se acaben. Si la funcion entra en panic, lo
    /// guarda para que el llamador lo relance y corta el reparto.
    fn work(&self) {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            loop {
                let i = self.next.fetch_add(1, Ordering::Relaxed);
                if i >= self.count {
                    break;
                }
                (self.run)(i);
            }
        }));
        if let Err(error) = result {
            self.next.store(self.count, Ordering::Relaxed);
            lock(&self.panic).get_or_insert(error);
        }
    }
}

struct Slot {
    epoch: u64,
    /// Direccion del trabajo vivo en la pila del llamador, o 0.
    job: usize,
}

struct Pool {
    slot: Mutex<Slot>,
    wake: Condvar,
    /// Copia de `slot.epoch` para esperar sin tomar el candado.
    epoch: AtomicU64,
    workers: usize,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Pool {
    fn shared() -> &'static Self {
        static POOL: OnceLock<Pool> = OnceLock::new();
        static STARTED: OnceLock<()> = OnceLock::new();
        let pool = POOL.get_or_init(|| Pool {
            slot: Mutex::new(Slot { epoch: 0, job: 0 }),
            wake: Condvar::new(),
            epoch: AtomicU64::new(0),
            workers: worker_count(),
        });
        STARTED.get_or_init(|| {
            // El hilo llamador tambien trabaja: se lanzan `workers - 1`.
            for index in 1..pool.workers {
                let _ = thread::Builder::new()
                    .name(format!("render-{index}"))
                    .spawn(move || worker(pool));
            }
        });
        pool
    }
}

/// Hilos del pool. `OCASO_HILOS` la fija a mano para medir el escalado.
///
/// Por defecto se usan 3/4 de los hilos logicos cuando hay muchos. Medido en
/// un i5-12500H (12 nucleos, 16 hilos): con 12 hilos el shading tarda lo
/// mismo que con 16 y el bloom baja de 5.5 a 2 ms. Los hilos extra del
/// hyperthreading no agregan calculo en este renderer y en las pasadas cortas
/// solo compiten por la cache de su nucleo.
fn worker_count() -> usize {
    if let Some(n) = std::env::var("OCASO_HILOS").ok().and_then(|v| v.parse().ok()) {
        if n > 0 {
            return n;
        }
    }
    let logical = thread::available_parallelism().map_or(1, |n| n.get());
    if logical >= 12 { logical * 3 / 4 } else { logical }
}

fn worker(pool: &'static Pool) {
    // Una pasada pedida desde dentro de un trabajo corre en serie.
    ACTIVE.set(true);
    let mut seen = 0u64;
    loop {
        let mut spins = 0;
        while pool.epoch.load(Ordering::Acquire) == seen {
            if spins < SPIN_LIMIT {
                std::hint::spin_loop();
                spins += 1;
                continue;
            }
            let mut slot = lock(&pool.slot);
            while slot.epoch == seen {
                slot = pool.wake.wait(slot).unwrap_or_else(|e| e.into_inner());
            }
            break;
        }
        // Unirse bajo el candado: el llamador retira el trabajo bajo el mismo
        // candado, asi que despues de retirarlo nadie mas puede unirse.
        let job = {
            let slot = lock(&pool.slot);
            seen = slot.epoch;
            if slot.job == 0 {
                continue;
            }
            // SAFETY: el llamador no destruye el trabajo hasta que `attached`
            // vuelve a cero, y este hilo lo incremento antes de soltar el candado.
            let job = unsafe { &*(slot.job as *const Job<'static>) };
            job.attached.fetch_add(1, Ordering::AcqRel);
            job
        };
        job.work();
        job.attached.fetch_sub(1, Ordering::AcqRel);
    }
}

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Marca al hilo llamador como ocupado mientras reparte trabajo.
struct ActiveWork(bool);
impl ActiveWork {
    fn enter() -> Self {
        Self(ACTIVE.replace(true))
    }
}
impl Drop for ActiveWork {
    fn drop(&mut self) {
        ACTIVE.set(self.0);
    }
}

/// Cantidad de hilos que trabajan en cada pasada (incluye al llamador).
pub fn workers() -> usize {
    Pool::shared().workers
}

/// Ejecuta `f(0..count)` repartido entre todos los hilos y espera a que
/// terminen. Si `f` entra en panic, el panic se relanza aqui.
pub fn for_each(count: usize, f: impl Fn(usize) + Sync) {
    let pool = Pool::shared();
    if count <= 1 || pool.workers <= 1 || ACTIVE.get() {
        (0..count).for_each(f);
        return;
    }
    let job = Job {
        run: &f,
        count,
        next: AtomicUsize::new(0),
        attached: AtomicUsize::new(0),
        panic: Mutex::new(None),
    };
    let address = &job as *const Job as usize;
    {
        let mut slot = lock(&pool.slot);
        slot.epoch += 1;
        slot.job = address;
        pool.epoch.store(slot.epoch, Ordering::Release);
    }
    pool.wake.notify_all();
    {
        let _active = ActiveWork::enter();
        job.work();
    }
    {
        // Retirar el trabajo (si otro llamador no publico ya el suyo encima).
        let mut slot = lock(&pool.slot);
        if slot.job == address {
            slot.job = 0;
        }
    }
    let mut spins = 0u32;
    while job.attached.load(Ordering::Acquire) != 0 {
        if spins < SPIN_LIMIT {
            std::hint::spin_loop();
            spins += 1;
        } else {
            thread::yield_now();
        }
    }
    if let Some(error) = job.panic.into_inner().unwrap_or_else(|e| e.into_inner()) {
        panic::resume_unwind(error);
    }
}

/// Puntero que se puede compartir entre hilos. Solo se usa para entregar
/// bloques disjuntos de un mismo slice.
struct SendPtr<T>(*mut T);
// A mano: `derive` exigiria `T: Copy` y el puntero se copia sin importar `T`.
impl<T> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SendPtr<T> {}
unsafe impl<T: Send> Send for SendPtr<T> {}
unsafe impl<T: Send> Sync for SendPtr<T> {}
impl<T> SendPtr<T> {
    // Un metodo (y no `ptr.0`) hace que el closure capture el struct entero,
    // que si es `Sync`, en vez del puntero crudo, que no lo es.
    fn get(self) -> *mut T {
        self.0
    }
}

/// Acceso compartido a un slice para que varios hilos escriban rangos
/// disjuntos. Sirve cuando cada trabajo toca pedazos de varios buffers a la
/// vez (un tile escribe su visibilidad, su color y su bloque del bloom).
pub struct DisjointMut<'a, T> {
    ptr: SendPtr<T>,
    len: usize,
    _borrow: std::marker::PhantomData<&'a mut [T]>,
}

// SAFETY: solo entrega rangos disjuntos (contrato de `range`) de un slice
// prestado en exclusiva, como haria `chunks_mut`.
unsafe impl<T: Send> Sync for DisjointMut<'_, T> {}

impl<'a, T: Send> DisjointMut<'a, T> {
    pub fn new(slice: &'a mut [T]) -> Self {
        Self {
            ptr: SendPtr(slice.as_mut_ptr()),
            len: slice.len(),
            _borrow: std::marker::PhantomData,
        }
    }

    /// El rango `[start, start + len)`.
    ///
    /// # Safety
    /// Dos rangos que esten vivos al mismo tiempo no deben solaparse.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn range(&self, start: usize, len: usize) -> &'a mut [T] {
        assert!(start + len <= self.len, "rango fuera del slice");
        // SAFETY: dentro del slice; la exclusividad la garantiza el llamador.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.get().add(start), len) }
    }
}

/// Procesa `data` en bloques de `chunk_len` con su indice, en paralelo. Con
/// menos de dos bloques de `min_per_worker` elementos se hace en serie:
/// despertar hilos no vale la pena para tan poco.
pub fn chunks_mut<T: Send>(
    data: &mut [T],
    chunk_len: usize,
    min_per_worker: usize,
    operation: impl Fn(usize, &mut [T]) + Sync,
) {
    assert!(chunk_len > 0, "el tamano de bloque debe ser positivo");
    let len = data.len();
    if len < 2 * min_per_worker.max(1) {
        for (index, chunk) in data.chunks_mut(chunk_len).enumerate() {
            operation(index, chunk);
        }
        return;
    }
    let ptr = SendPtr(data.as_mut_ptr());
    for_each(len.div_ceil(chunk_len), |index| {
        let start = index * chunk_len;
        let end = (start + chunk_len).min(len);
        // SAFETY: cada indice produce un rango [start, end) distinto, y
        // `data` sigue prestado en exclusiva durante toda la llamada.
        let chunk = unsafe { std::slice::from_raw_parts_mut(ptr.get().add(start), end - start) };
        operation(index, chunk);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_match_serial_processing_including_a_partial_last_chunk() {
        let source: Vec<usize> = (0..10_007).map(|i| i * i).collect();
        for chunk in [1, 7, 64, 20_000] {
            let mut output = vec![0; source.len()];
            chunks_mut(&mut output, chunk, 1, |index, block| {
                for (x, value) in block.iter_mut().enumerate() {
                    *value += source[index * chunk + x] + 1;
                }
            });
            assert!(output.iter().zip(&source).all(|(o, s)| *o == s + 1));
        }
    }

    #[test]
    fn every_index_runs_exactly_once() {
        let counts: Vec<AtomicUsize> = (0..5000).map(|_| AtomicUsize::new(0)).collect();
        for _ in 0..50 {
            for_each(counts.len(), |i| {
                counts[i].fetch_add(1, Ordering::Relaxed);
            });
        }
        assert!(counts.iter().all(|c| c.load(Ordering::Relaxed) == 50));
    }

    #[test]
    fn panics_propagate_and_the_pool_keeps_working() {
        let result = panic::catch_unwind(|| {
            for_each(64, |i| assert_ne!(i, 17, "fallo de prueba"));
        });
        assert!(result.is_err());
        let mut values = [0u8; 300];
        chunks_mut(&mut values, 3, 1, |_, block| block.fill(7));
        assert_eq!(values, [7; 300]);
    }

    #[test]
    fn nested_work_runs_serially_without_deadlock() {
        let mut values = [0; 27];
        chunks_mut(&mut values, 5, 1, |_, row| {
            chunks_mut(row, 2, 1, |_, pair| {
                for value in pair {
                    *value += 1;
                }
            });
        });
        assert_eq!(values, [1; 27]);
    }

    /// Costo de despachar una pasada vacia. Correr con
    /// `cargo test --release -- --ignored --nocapture medir_`.
    #[test]
    #[ignore]
    fn medir_despacho() {
        for _ in 0..1000 {
            for_each(64, |_| {});
        }
        let start = std::time::Instant::now();
        let rounds = 20_000;
        for _ in 0..rounds {
            for_each(64, |i| {
                std::hint::black_box(i);
            });
        }
        let us = start.elapsed().as_secs_f64() * 1e6 / rounds as f64;
        println!("despacho de una pasada vacia: {us:.2} us");
    }

    #[test]
    fn concurrent_callers_from_several_threads_complete() {
        let handles: Vec<_> = (0..4)
            .map(|t| {
                thread::spawn(move || {
                    let total = AtomicUsize::new(0);
                    for _ in 0..200 {
                        for_each(97, |i| {
                            total.fetch_add(i + t, Ordering::Relaxed);
                        });
                    }
                    total.load(Ordering::Relaxed)
                })
            })
            .collect();
        for (t, handle) in handles.into_iter().enumerate() {
            assert_eq!(handle.join().unwrap(), 200 * (96 * 97 / 2 + 97 * t));
        }
    }
}
