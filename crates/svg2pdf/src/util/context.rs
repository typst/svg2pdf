/*! The context holds information that we might need access to while converting the SVG.

This includes information like for example what the size of the SVG is, whether content should be
compressed and access to an instance of the deferrer + allocator.
*/

use pdf_writer::{Content, Ref};
use usvg::utils::view_box_to_transform;
use usvg::{NonZeroRect, Size, Transform, Tree, ViewBox};

use super::defer::Deferrer;
use super::helper::deflate;
use crate::Options;

/// Holds all of the necessary information for the conversion process.
pub struct Context {
    /// The view box of the SVG.
    pub view_box: ViewBox,
    /// The size of the SVG.
    pub size: Size,
    /// An instance of the deferrer.
    pub deferrer: Deferrer,
    /// Options that where passed by the user.
    pub options: Options,
}

impl Context {
    /// Create a new context.
    pub fn new(tree: &Tree, options: Options, start_ref: Option<i32>) -> Self {
        Self {
            view_box: tree.view_box,
            size: tree.size,
            deferrer: Deferrer::new_with_start_ref(start_ref.unwrap_or(1)),
            options,
        }
    }

    /// Allocate a new reference.
    pub fn alloc_ref(&mut self) -> Ref {
        self.deferrer.alloc_ref()
    }

    /// Get the base transform that needs to be applied before rendering everything else (
    /// i.e. the initial transform passed by the user + the view box transform to account for the
    /// view box of the SVG).
    pub fn get_view_box_transform(&self) -> Transform {
        view_box_to_transform(self.view_box.rect, self.view_box.aspect, self.size)
    }

    /// Returns a [`usvg` Rect](usvg::Rect) with the dimensions of the whole SVG.
    pub fn get_rect(&self) -> NonZeroRect {
        NonZeroRect::from_xywh(0.0, 0.0, self.size.width(), self.size.height()).unwrap()
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
