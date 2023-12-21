use pdf_writer::{Chunk, Content};
use usvg::{Node, NodeKind, Transform, Tree};

use crate::util::context::Context;
use crate::util::helper::TransformExt;

pub mod clip_path;
pub mod gradient;
pub mod group;
#[cfg(feature = "image")]
pub mod image;
pub mod mask;
pub mod path;
pub mod pattern;
#[cfg(feature = "filters")]
pub mod filter;

/// Write a tree into a stream. Assumes that the stream belongs to transparency group and has the
/// right bounding boxes.
pub fn tree_to_stream(
    tree: &Tree,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    initial_transform: Transform,
) {
    content.save_state();
    let initial_transform = initial_transform.pre_concat(ctx.get_view_box_transform());
    content.transform(initial_transform.to_pdf_transform());

    tree.root.render(chunk, content, ctx, initial_transform);
    content.restore_state();
}

trait Render {
    fn render(
        &self,
        chunk: &mut Chunk,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    );
}

impl Render for Node {
    fn render(
        &self,
        chunk: &mut Chunk,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        match *self.borrow() {
            NodeKind::Path(ref path) => {
                path::render(self, path, chunk, content, ctx, accumulated_transform)
            }
            NodeKind::Group(ref group) => {
                group::render(self, group, chunk, content, ctx, accumulated_transform)
            }
            #[cfg(feature = "image")]
            NodeKind::Image(ref image) => image::render(image, chunk, content, ctx),
            // Texts should be converted beforehand.
            NodeKind::Text(ref text) => {
                if let Some(ref node) = text.flattened {
                    node.render(chunk, content, ctx, accumulated_transform);
                }
            }
        }
    }
}
