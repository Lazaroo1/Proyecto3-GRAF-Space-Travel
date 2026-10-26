//! Contadores del frame: lo que el renderer hizo y cuanto tardo cada etapa.
//!
//! Sirven para demostrar el culling y los niveles de detalle: no basta con que
//! la imagen se vea igual, hay que mostrar que el trabajo cambio.

/// Destino de los triangulos que entraron a la etapa de triangulos.
#[derive(Clone, Copy, Debug, Default)]
pub struct TriCounters {
    /// Triangulos de los objetos que pasaron el culling por objeto.
    pub input: u32,
    /// Completamente fuera del frustum.
    pub frustum: u32,
    /// De espaldas a la camara (o degenerados).
    pub backface: u32,
    /// Cruzaban el plano cercano o la banda de guarda y se recortaron.
    pub clipped: u32,
    /// Tan pequenos que no cubren ningun centro de pixel.
    pub small: u32,
    /// Llegaron al raster.
    pub emitted: u32,
}

impl TriCounters {
    pub fn accumulate(&mut self, o: &Self) {
        self.input += o.input;
        self.frustum += o.frustum;
        self.backface += o.backface;
        self.clipped += o.clipped;
        self.small += o.small;
        self.emitted += o.emitted;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub objects: u32,
    pub objects_culled: u32,
    pub vertices: u32,
    pub triangles: TriCounters,
    pub pixels_shaded: u64,
    pub ms_vertex: f32,
    pub ms_triangles: f32,
    pub ms_raster: f32,
    pub ms_shade: f32,
    pub ms_bloom: f32,
    pub ms_post: f32,
    pub ms_total: f32,
}
