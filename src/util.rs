use pdf_writer::{Rect, Ref};
use usvg::{Tree, ViewBox, Size};

pub trait TransformExt {
    fn get_transform(&self) -> [f32; 6];
}

impl TransformExt for usvg::Transform {
    fn get_transform(&self) -> [f32; 6] {
        [
            self.a as f32,
            self.b as f32,
            self.c as f32,
            self.d as f32,
            self.e as f32,
            self.f as f32,
        ]
    }
}

pub struct Context {
    next_id: i32,
    dpi: f32,
    pub viewbox: ViewBox,
    pub size: Size,
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree) -> Self {
        Self {
            next_id: 1,
            dpi: 72.0,
            viewbox: tree.view_box,
            size: tree.size,
        }
    }

    pub fn dpi_factor(&self) -> f32 {
        72.0 / self.dpi
    }

    pub fn get_media_box(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.size.width() as f32 * self.dpi_factor(),
            self.size.height() as f32 * self.dpi_factor(),
        )
    }

    /// Allocate a new indirect reference id.
    pub fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_id);
        self.next_id += 1;
        reference
    }
}
