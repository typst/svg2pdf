pub mod clip_path;
pub mod gradient;
pub mod group;
#[cfg(feature = "image")]
pub mod image;
pub mod mask;
pub mod path;
pub mod pattern;

use std::rc::Rc;

use crate::{initial_transform};
use pdf_writer::{Content, Filter, Finish, PdfWriter, Rect};

use usvg::{AspectRatio, Node, NodeKind, Size, Transform, Tree};

use crate::util::context::Context;
use crate::util::helper::{plain_bbox, TransformExt};

/// Turn a tree into an XObject. Returns the name (= the name in the `Resources` dictionary) of
/// the XObject. The bounding box of the resulting XObject will be [0, 0, pdf_size.width, pdf_size.height]
pub fn tree_to_x_object(
    tree: &Tree,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    pdf_size: Size,
    aspect: Option<AspectRatio>,
) -> Rc<String> {
    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let mut child_content = Content::new();
    tree_to_stream(
        tree,
        writer,
        &mut child_content,
        ctx,
        initial_transform(aspect, tree, pdf_size),
    );
    let child_content_stream = ctx.finish_content(child_content);

    let mut x_object = writer.form_xobject(x_ref, &child_content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object.bbox(Rect::new(0.0, 0.0, pdf_size.width(), pdf_size.height()));
    x_object
        .group()
        .transparency()
        .isolated(true)
        .knockout(false)
        .color_space()
        .srgb();
    x_object.finish();

    ctx.deferrer.add_x_object(x_ref)
}

/// Write a tree into a stream. Assumes that the stream belongs to an XObject
/// that is a transparency group and already has the right bounding boxes, otherwise
/// the result might be wrong.
pub fn tree_to_stream(
    tree: &Tree,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    initial_transform: Transform,
) {
    content.save_state();
    let initial_transform = initial_transform.pre_concat(ctx.get_view_box_transform());
    content.transform(initial_transform.as_array());

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
            NodeKind::Path(ref path) => path::render(
                path,
                &plain_bbox(self),
                writer,
                content,
                ctx,
                accumulated_transform,
            ),
            NodeKind::Group(ref group) => {
                group::render(self, group, writer, content, ctx, accumulated_transform)
            }
            #[cfg(feature = "image")]
            NodeKind::Image(ref image) => image::render(image, writer, content, ctx),
            _ => {}
        }
    }
}
