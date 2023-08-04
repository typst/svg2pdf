/*! The context holds information that we might need access to while converting the SVG.

This includes information like for example what the size of the SVG is, whether content should be
compressed and access to an instance of the deferrer + allocator.
*/

use pdf_writer::{Content, Ref};
use usvg::utils::view_box_to_transform;
use usvg::{Size, Transform, Tree, ViewBox};

use crate::util::defer::Deferrer;
use crate::util::helper::deflate;
use crate::Options;

/// Holds all of the necessary information for the conversion process.
pub struct Context {
    /// The view box of the SVG.
    pub view_box: ViewBox,
    /// The size of the SVG.
    pub size: Size,
    /// The initial transform that should be applied before rendering everything.
    pub initial_transform: Transform,
    /// An instance of the deferrer.
    pub deferrer: Deferrer,
    /// Options that where passed by the user.
    pub options: Options,
}

impl Context {
    /// Create a new context.
    pub fn new(
        tree: &Tree,
        options: Options,
        initial_transform: Transform,
        start_ref: Option<i32>,
    ) -> Self {
        Self {
            view_box: tree.view_box,
            size: tree.size,
            initial_transform,
            deferrer: Deferrer::new_with_start_ref(start_ref.unwrap_or(1)),
            options,
        }
    }

    /// Allocate a new reference.
    pub fn alloc_ref(&mut self) -> Ref {
        self.deferrer.alloc_ref()
    }

    // Get the viewbox transform
    pub fn get_viewbox_transform(&self) -> Transform {
        view_box_to_transform(self.view_box.rect, self.view_box.aspect, self.size)
    }

    /// Returns a [`usvg` Rect](usvg::Rect) with the dimensions of the whole SVG.
    pub fn get_rect(&self) -> usvg::Rect {
        usvg::Rect::new(0.0, 0.0, self.size.width(), self.size.height()).unwrap()
    }

    /// Just a helper method so that we don't have to manually compress the content if this was
    /// set in the [Options] struct.
    pub fn finish_content(&self, content: Content) -> Vec<u8> {
        if self.options.compress {
            deflate(&content.finish())
        } else {
            content.finish()
        }
    }
}
