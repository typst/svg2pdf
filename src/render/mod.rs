use pdf_writer::{Chunk, Content};
use usvg::{Node, Transform, Tree};

use crate::util::context::Context;
use crate::util::helper::TransformExt;

pub mod clip_path;
#[cfg(feature = "filters")]
pub mod filter;
pub mod gradient;
pub mod group;
#[cfg(feature = "image")]
pub mod image;
pub mod mask;
pub mod path;
pub mod pattern;
pub mod text;

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

    group::render(tree.root(), chunk, content, ctx, initial_transform, None);
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
        match self {
            Node::Path(ref path) => {
                path::render(path, chunk, content, ctx, accumulated_transform)
            }
            Node::Group(ref group) => {
                group::render(group, chunk, content, ctx, accumulated_transform, None)
            }
            #[cfg(feature = "image")]
            Node::Image(ref image) => image::render(
                image.visibility(),
                image.kind(),
                image.view_box(),
                chunk,
                content,
                ctx,
            ),
            #[cfg(not(feature = "image"))]
            Node::Image(_) => {
                log::warn!("Images have been disabled in this build of svg2pdf.")
            }
            Node::Text(ref text) => {
                text::render(text, chunk, content, ctx, accumulated_transform);
                // group::render(
                //     text.flattened(),
                //     chunk,
                //     content,
                //     ctx,
                //     accumulated_transform,
                //     None,
                // );
            }
        }
    }
}
