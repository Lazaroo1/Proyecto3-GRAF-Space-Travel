//! Marimba procedural para la version chapina.
//!
//! Provisional: por ahora no suena.

/// La marimba: el sintetizador le pide una muestra a la vez.
pub struct Marimba {}

impl Marimba {
    pub fn new() -> Self {
        Self {}
    }

    /// La siguiente muestra (mono, entre -1 y 1 aproximadamente), a
    /// `sample_rate` muestras por segundo.
    pub fn next(&mut self, _sample_rate: f32) -> f32 {
        0.0
    }
}

impl Default for Marimba {
    fn default() -> Self {
        Self::new()
    }
}
