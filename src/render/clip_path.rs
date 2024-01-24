use std::rc::Rc;

use pdf_writer::types::MaskType;
use pdf_writer::{Chunk, Content, Filter, Finish};
use usvg::tiny_skia_path::PathSegment;
use usvg::{FillRule, Group, Node, SharedClipPath, Transform, Units, Visibility};

use super::group;
use super::path::draw_path;
use crate::util::context::Context;
use crate::util::helper::{bbox_to_non_zero_rect, NameExt, RectExt, TransformExt};

/// Render a clip path into a content stream.
pub fn render(
    group: &Group,
    clip_path: SharedClipPath,
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

    let is_simple_clip_path = is_simple_clip_path(&clip_path.borrow().root);
    let clip_rules = collect_clip_rules(&clip_path.borrow().root);

    if is_simple_clip_path
        && (clip_rules.iter().all(|f| *f == FillRule::NonZero)
        // For even odd, there must be at most one shape in the group, because
        // overlapping shapes with evenodd render wrongly in PDF
            || (clip_rules.iter().all(|f| *f == FillRule::EvenOdd)
                && clip_rules.len() == 1))
    {
        create_simple_clip_path(
            group,
            clip_path,
            content,
            clip_rules.first().copied().unwrap_or(FillRule::NonZero),
        );
    } else {
        content.set_parameters(
            create_complex_clip_path(group, clip_path, chunk, ctx).to_pdf_name(),
        );
    }
}

fn is_simple_clip_path(group: &Group) -> bool {
    group.children.iter().all(|n| {
        match n {
            Node::Group(ref group) => {
                // We can only intersect one clipping path with another one, meaning that we
                // can convert nested clip paths if a second clip path is defined on the clip
                // path itself, but not if it is defined on a child.
                group.clip_path.is_none() && is_simple_clip_path(group)
            }
            _ => true,
        }
    })
}

fn collect_clip_rules(group: &Group) -> Vec<FillRule> {
    let mut clip_rules = vec![];
    group.children.iter().for_each(|n| match n {
        Node::Path(ref path) => {
            if let Some(fill) = &path.fill {
                clip_rules.push(fill.rule);
            }
        }
        Node::Text(ref text) => {
            if let Some(group) = text.flattened.as_deref() {
                clip_rules.extend(collect_clip_rules(group))
            }
        }
        Node::Group(ref group) => {
            clip_rules.extend(collect_clip_rules(group));
        }
        _ => {}
    });

    clip_rules
}

fn create_simple_clip_path(
    parent: &Group,
    clip_path: SharedClipPath,
    content: &mut Content,
    clip_rule: FillRule,
) {
    let clip_path = clip_path.borrow();

    if let Some(clip_path) = &clip_path.clip_path {
        create_simple_clip_path(parent, clip_path.clone(), content, clip_rule);
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
                Transform::from_bbox(bbox_to_non_zero_rect(parent.bounding_box))
            });

    let mut segments = vec![];
    extend_segments_from_group(&clip_path.root, &base_transform, &mut segments);
    draw_path(segments.into_iter(), content);

    if clip_rule == FillRule::NonZero {
        content.clip_nonzero();
    } else {
        content.clip_even_odd();
    }
    content.end_path();
}

fn extend_segments_from_group(
    group: &Group,
    transform: &Transform,
    segments: &mut Vec<PathSegment>,
) {
    for child in &group.children {
        match child {
            Node::Path(ref path) => {
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
                            segments.push(PathSegment::CubicTo(
                                points[0], points[1], points[2],
                            ));
                        }
                        PathSegment::Close => segments.push(PathSegment::Close),
                    })
                }
            }
            Node::Group(ref group) => {
                let group_transform = transform.pre_concat(group.transform);
                extend_segments_from_group(group, &group_transform, segments);
            }
            Node::Text(ref text) => {
                // TODO: Need to change this once unconverted text is supported
                if let Some(ref group) = text.flattened {
                    extend_segments_from_group(group, transform, segments);
                }
            }
            // Images are not valid in a clip path. Text will be converted into shapes beforehand.
            _ => {}
        }
    }
}

fn create_complex_clip_path(
    parent: &Group,
    clip_path: SharedClipPath,
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> Rc<String> {
    let clip_path = clip_path.borrow();

    ctx.deferrer.push();
    let x_object_reference = ctx.alloc_ref();

    let mut content = Content::new();
    content.save_state();

    if let Some(recursive_clip_path) = &clip_path.clip_path {
        render(parent, recursive_clip_path.clone(), chunk, &mut content, ctx);
    }

    content.transform(clip_path.transform.to_pdf_transform());

    let pdf_bbox = bbox_to_non_zero_rect(parent.bounding_box).to_pdf_rect();

    if clip_path.units == Units::ObjectBoundingBox {
        let parent_svg_bbox = bbox_to_non_zero_rect(parent.bounding_box);
        content.transform(Transform::from_bbox(parent_svg_bbox).to_pdf_transform());
    }

    group::render(&clip_path.root, chunk, &mut content, ctx, Transform::default());

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
