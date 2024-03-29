use pdf_writer::{Chunk, Content, Filter, Finish, Ref};
use usvg::{Node, Transform, Tree};

use crate::util::context::Context;
use crate::util::helper::{RectExt, TransformExt};
use crate::util::resources::ResourceContainer;

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

/// Write a tree into a stream. Assumes that the stream belongs to transparency group and the object
/// that contains it has the correct bounding box set.
pub fn tree_to_stream(
    tree: &Tree,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    rc: &mut ResourceContainer,
) {
    content.save_state();

    let initial_transform =
        // From PDF coordinate system to SVG coordinate system
        Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, tree.size().height())
            // Account for view box of tree.
            .pre_concat(tree.view_box().to_transform(tree.size()));

    content.transform(initial_transform.to_pdf_transform());

    group::render(tree.root(), chunk, content, ctx, initial_transform, None, rc);
    content.restore_state();
}

/// Convert a tree into a XObject of size 1x1, similar to an image. The Ref of that
/// XObject is guaranteed to be the current highest, so you can use `ref.next()` to
/// get the next free ID.
pub fn tree_to_xobject(tree: &Tree, chunk: &mut Chunk, ctx: &mut Context) -> Ref {
    let bbox = tree.size().to_non_zero_rect(0.0, 0.0);
    let x_ref = ctx.alloc_ref();

    let mut rc = ResourceContainer::new();

    let mut content = Content::new();
    tree_to_stream(tree, chunk, &mut content, ctx, &mut rc);
    let stream = ctx.finish_content(content);

    let mut x_object = chunk.form_xobject(x_ref, &stream);
    x_object.bbox(bbox.to_pdf_rect());
    x_object.matrix([1.0 / bbox.width(), 0.0, 0.0, 1.0 / bbox.height(), 0.0, 0.0]);

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    let mut resources = x_object.resources();
    rc.finish(&mut resources);

    resources.finish();
    x_object.finish();

    x_ref
}

trait Render {
    fn render(
        &self,
        chunk: &mut Chunk,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
        rc: &mut ResourceContainer,
    );
}

impl Render for Node {
    fn render(
        &self,
        chunk: &mut Chunk,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
        rc: &mut ResourceContainer,
    ) {
        match self {
            Node::Path(ref path) => {
                path::render(path, chunk, content, ctx, rc, accumulated_transform)
            }
            Node::Group(ref group) => {
                group::render(group, chunk, content, ctx, accumulated_transform, None, rc)
            }
            #[cfg(feature = "image")]
            Node::Image(ref image) => image::render(
                image.visibility(),
                image.kind(),
                image.view_box(),
                chunk,
                content,
                ctx,
                rc,
            ),
            #[cfg(not(feature = "image"))]
            Node::Image(_) => {
                log::warn!("Images have been disabled in this build of svg2pdf.")
            }
            Node::Text(ref text) => {
                text::render(text, chunk, content, ctx, rc, accumulated_transform);
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
