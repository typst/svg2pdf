use pdf_writer::Ref;

/// Just a wrapper struct so we don't need to always cast f64 to f32.
#[derive(Copy, Clone)]
pub struct Viewport((f32, f32));

impl Viewport {
    pub fn new(width: f32, height: f32) -> Self {
        Viewport((width, height))
    }

    pub fn width(&self) -> f32 {
        self.0 .0
    }

    pub fn height(&self) -> f32 {
        self.0 .1
    }
}

pub struct Context {
    next_id: i32,
    pub viewport: Viewport,
}

impl Context {
    /// Create a new context.
    pub(crate) fn new(viewport: Viewport) -> Self {
        Self { next_id: 1, viewport }
    }

    /// Allocate a new indirect reference id.
    pub fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_id);
        self.next_id += 1;
        reference
    }
}
