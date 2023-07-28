pub mod clip_path;
pub mod gradient;
pub mod group;
#[cfg(feature = "image")]
pub mod image;
pub mod mask;
pub mod path;
pub mod pattern;

use pdf_writer::{Content, PdfWriter};

use usvg::{Node, NodeKind, Transform, Tree};

use crate::util::context::Context;
use crate::util::helper::{plain_bbox_without_default, TransformExt};

/// Write a tree into a stream. Assumes that the stream belongs to transparency group and has the
/// right bounding boxes.
pub fn tree_to_stream(
    tree: &Tree,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    initial_transform: Transform,
) {
    content.save_state();
    let initial_transform = initial_transform.pre_concat(ctx.get_view_box_transform());
    content.transform(initial_transform.to_pdf_transform());

    // The root of a tree is always a group, so we can directly iterate over the children
    for el in tree.root.children() {
        el.render(writer, content, ctx, initial_transform);
    }
    content.restore_state();
}

trait Render {
    fn render(
        &self,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    );
}

impl Render for Node {
    fn render(
        &self,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        match *self.borrow() {
            NodeKind::Path(ref path) => {
                path::render(
                    path,
                    plain_bbox_without_default(self, true).as_ref(),
                    writer,
                    content,
                    ctx,
                    accumulated_transform,
                )
            }
            NodeKind::Group(ref group) => {
                group::render(self, group, writer, content, ctx, accumulated_transform)
            }
            #[cfg(feature = "image")]
            NodeKind::Image(ref image) => image::render(image, writer, content, ctx),
            _ => {}
        }
    }
}
