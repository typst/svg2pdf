use crate::render::group;
use crate::render::path::draw_path;
use crate::util::context::Context;
use crate::util::helper::{plain_bbox, NameExt, RectExt, TransformExt};
use pdf_writer::types::MaskType;
use pdf_writer::{Content, Filter, Finish, PdfWriter};
use std::rc::Rc;
use usvg::{ClipPath, FillRule, Node, NodeKind, Transform, Units, Visibility};

/// Render a clip path into a content stream.
pub(crate) fn render(
    node: &Node,
    clip_path: Rc<ClipPath>,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    // Unfortunately, clip paths are a bit tricky to deal with, the reason being that clip paths in
    // SVGs can be much more complex than in PDF. In SVG, clip paths can have transforms, as well as
    // nested clip paths. The objects inside of the clip path can have transforms as well, making it
    // even more difficult to deal with. Because in PDF, once you start a clip path operation you
    // cannot interrupt it, because once you pop the current graphics state, the clip path will be
    // lost since it is part of the current graphics state. However, if we have various transforms
    // on the children, we need to be able to push/pop the graphics state, so that the children's
    // transforms don't affect each other. Initially, because of this, clip paths were only implemented
    // using soft masks, but Safari has a couple of issues with rendering them properly. Not to mention
    // the fact that soft masks are obviously also more expensive. Because of this, we proceed the following
    // way: We first check whether the clip path itself is too "complex" (complex being that it fulfills
    // some attributes that make it impossible to represent them in our current setup using just native
    // PDF clip paths. If it is too complex, we fall back to using soft masks. However, if it is simple
    // enough, we just use the normal clip path operator in PDF. It should be noted that in reality,
    // only very few SVGs seem to have such complex clipping paths (they are not even rendered correctly
    // by all online converters that were tested), so in most real-life scenarios, the simple version
    // should suffice. But in order to conform with the SVG specification, we also handle the case
    // of more complex clipping paths, even if this means that Safari won't display them correctly.
    if is_simple_clip_path(clip_path.clone()) {
        create_simple_clip_path(clip_path, content);
    } else {
        content.set_parameters(
            create_complex_clip_path(node, clip_path, writer, ctx).as_name(),
        );
    }
}

fn is_simple_clip_path(clip_path: Rc<ClipPath>) -> bool {
    clip_path.transform.is_default()
        && clip_path.units == Units::UserSpaceOnUse
        && clip_path.root.descendants().all(|n| match *n.borrow() {
            NodeKind::Path(ref path) => {
                // PDFs clip path operator doesn't support the EvenOdd rule
                path.transform.is_default()
                    && path
                        .fill
                        .as_ref()
                        .map_or(true, |fill| fill.rule == FillRule::NonZero)
            }
            NodeKind::Group(ref group) => {
                group.transform.is_default()
                    && group
                        .clip_path
                        .as_ref()
                        .map_or(true, |clip_path| is_simple_clip_path(clip_path.clone()))
            }
            _ => false,
        })
        && clip_path
            .clip_path
            .as_ref()
            .map_or(true, |clip_path| is_simple_clip_path(clip_path.clone()))
}

fn create_simple_clip_path(clip_path: Rc<ClipPath>, content: &mut Content) {
    // Just a dummy operation, so that in case the clip path only has hidden children the clip
    // path will still be applied and everything will be hidden.
    content.move_to(0.0, 0.0);

    if let Some(clip_path) = &clip_path.clip_path {
        create_simple_clip_path(clip_path.clone(), content);
    }

    for node in clip_path.root.descendants() {
        if let NodeKind::Path(ref path) = *node.borrow() {
            if path.visibility != Visibility::Hidden {
                draw_path(path.data.segments(), content);
            }
        }

        if let NodeKind::Group(ref group) = *node.borrow() {
            if let Some(clip_path) = &group.clip_path {
                create_simple_clip_path(clip_path.clone(), content);
            }
        }
    }

    content.clip_nonzero();
    content.end_path();
}

// TODO: Figure out if there is a way to deduplicate and reuse parts of mask?
fn create_complex_clip_path(
    parent: &Node,
    clip_path: Rc<ClipPath>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Rc<String> {
    ctx.deferrer.push();
    let x_object_reference = ctx.alloc_ref();

    let mut content = Content::new();
    content.save_state();

    if let Some(recursive_clip_path) = &clip_path.clip_path {
        render(parent, recursive_clip_path.clone(), writer, &mut content, ctx);
    }

    content.transform(clip_path.transform.as_array());

    let pdf_bbox = plain_bbox(parent).as_pdf_rect();

    match *clip_path.root.borrow() {
        NodeKind::Group(ref group) => {
            if clip_path.units == Units::ObjectBoundingBox {
                let parent_svg_bbox = plain_bbox(parent);
                content.transform(Transform::from_bbox(parent_svg_bbox).as_array());
            }
            content
                .x_object(group::create(&clip_path.root, group, writer, ctx).as_name());
        }
        _ => unreachable!(),
    };

    content.restore_state();
    let content_stream = ctx.finish_content(content);

    let mut x_object = writer.form_xobject(x_object_reference, &content_stream);

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    ctx.deferrer.pop(&mut x_object.resources());

    x_object
        .group()
        .transparency()
        .isolated(true)
        .knockout(false)
        .color_space()
        .srgb();

    x_object.bbox(pdf_bbox);
    x_object.finish();

    let gs_ref = ctx.alloc_ref();
    let mut gs = writer.ext_graphics(gs_ref);
    gs.soft_mask().subtype(MaskType::Alpha).group(x_object_reference);

    ctx.deferrer.add_graphics_state(gs_ref)
}
