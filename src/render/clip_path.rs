use std::rc::Rc;

use pdf_writer::types::MaskType;
use pdf_writer::{Chunk, Content, Filter, Finish};
use usvg::tiny_skia_path::PathSegment;
use usvg::{
    ClipPath, FillRule, Node, NodeExt, NodeKind, NonZeroRect, Transform, Units,
    Visibility,
};

use super::group;
use super::path::draw_path;
use crate::util::context::Context;
use crate::util::helper::{NameExt, RectExt, TransformExt};

/// Render a clip path into a content stream.
pub fn render(
    node: &Node,
    clip_path: Rc<ClipPath>,
    chunk: &mut Chunk,
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
    // of more complex clipping paths, even if this means that Safari will in some cases not
    // display them correctly.
    if is_simple_clip_path(&clip_path.root) {
        create_simple_clip_path(node, clip_path, content);
    } else {
        content.set_parameters(
            create_complex_clip_path(node, clip_path, chunk, ctx).to_pdf_name(),
        );
    }
}

fn is_simple_clip_path(node: &Node) -> bool {
    node.descendants().all(|n| match *n.borrow() {
        NodeKind::Path(ref path) => {
            // While there is a clipping path for EvenOdd, it will produce wrong results
            // if the clip-rule is defined on a group instead of on the children.
            path.fill.as_ref().map_or(true, |fill| fill.rule == FillRule::NonZero)
        }
        NodeKind::Text(ref text) => {
            // TODO: Need to change this once unconverted text is supported
            text.flattened.as_ref().map_or(true, is_simple_clip_path)
        }
        NodeKind::Group(ref group) => {
            // We can only intersect one clipping path with another one, meaning that we
            // can convert nested clip paths if a second clip path is defined on the clip
            // path itself, but not if it is defined on a child.
            group.clip_path.is_none()
        }
        _ => false,
    })
}

fn create_simple_clip_path(
    parent: &Node,
    clip_path: Rc<ClipPath>,
    content: &mut Content,
) {
    if let Some(clip_path) = &clip_path.clip_path {
        create_simple_clip_path(parent, clip_path.clone(), content);
    }

    // Just a dummy operation, so that in case the clip path only has hidden children the clip
    // path will still be applied and everything will be hidden.
    content.move_to(0.0, 0.0);

    let base_transform =
        clip_path
            .transform
            .pre_concat(if clip_path.units == Units::UserSpaceOnUse {
                Transform::default()
            } else {
                Transform::from_bbox(
                    parent
                        .bounding_box()
                        .and_then(|bb| bb.to_non_zero_rect())
                        .unwrap_or(NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
                )
            });

    let mut segments = vec![];
    extend_segments_from_node(&clip_path.root, &base_transform, &mut segments);
    draw_path(segments.into_iter(), content);
    content.clip_nonzero();
    content.end_path();
}

fn extend_segments_from_node(
    node: &Node,
    transform: &Transform,
    segments: &mut Vec<PathSegment>,
) {
    match *node.borrow() {
        NodeKind::Path(ref path) => {
            if path.visibility != Visibility::Hidden {
                path.data.segments().for_each(|segment| match segment {
                    PathSegment::MoveTo(mut p) => {
                        transform.map_point(&mut p);
                        segments.push(PathSegment::MoveTo(p));
                    }
                    PathSegment::LineTo(mut p) => {
                        transform.map_point(&mut p);
                        segments.push(PathSegment::LineTo(p));
                    }
                    PathSegment::QuadTo(p1, p2) => {
                        let mut points = [p1, p2];
                        transform.map_points(&mut points);
                        segments.push(PathSegment::QuadTo(points[0], points[1]));
                    }
                    PathSegment::CubicTo(p1, p2, p3) => {
                        let mut points = [p1, p2, p3];
                        transform.map_points(&mut points);
                        segments
                            .push(PathSegment::CubicTo(points[0], points[1], points[2]));
                    }
                    PathSegment::Close => segments.push(PathSegment::Close),
                })
            }
        }
        NodeKind::Group(ref group) => {
            let group_transform = transform.pre_concat(group.transform);
            for child in node.children() {
                extend_segments_from_node(&child, &group_transform, segments);
            }
        }
        NodeKind::Text(ref text) => {
            // TODO: Need to change this once unconverted text is supported
            if let Some(ref node) = text.flattened {
                extend_segments_from_node(node, transform, segments);
            }
        }
        // Images are not valid in a clip path. Text will be converted into shapes beforehand.
        _ => {}
    }
}

fn create_complex_clip_path(
    parent: &Node,
    clip_path: Rc<ClipPath>,
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> Rc<String> {
    ctx.deferrer.push();
    let x_object_reference = ctx.alloc_ref();

    let mut content = Content::new();
    content.save_state();

    if let Some(recursive_clip_path) = &clip_path.clip_path {
        render(parent, recursive_clip_path.clone(), chunk, &mut content, ctx);
    }

    content.transform(clip_path.transform.to_pdf_transform());

    let pdf_bbox = parent
        .bounding_box()
        .and_then(|bb| bb.to_non_zero_rect())
        .unwrap()
        .to_pdf_rect();

    match *clip_path.root.borrow() {
        NodeKind::Group(ref group) => {
            if clip_path.units == Units::ObjectBoundingBox {
                let parent_svg_bbox =
                    parent.bounding_box().and_then(|bb| bb.to_non_zero_rect()).unwrap();
                content
                    .transform(Transform::from_bbox(parent_svg_bbox).to_pdf_transform());
            }
            group::render(
                &clip_path.root,
                group,
                chunk,
                &mut content,
                ctx,
                Transform::default(),
            );
        }
        _ => unreachable!(),
    };

    content.restore_state();
    let content_stream = ctx.finish_content(content);

    let mut x_object = chunk.form_xobject(x_object_reference, &content_stream);

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    ctx.deferrer.pop(&mut x_object.resources());

    x_object
        .group()
        .transparency()
        .isolated(false)
        .knockout(false)
        .color_space()
        .icc_based(ctx.deferrer.srgb_ref());

    x_object.bbox(pdf_bbox);
    x_object.finish();

    let gs_ref = ctx.alloc_ref();
    let mut gs = chunk.ext_graphics(gs_ref);
    gs.soft_mask().subtype(MaskType::Alpha).group(x_object_reference);

    ctx.deferrer.add_graphics_state(gs_ref)
}
