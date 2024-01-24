use std::rc::Rc;

use pdf_writer::{Chunk, Content, Filter, Finish};
use usvg::{Group, SharedMask, Transform, Units};

use super::group;
use crate::util::context::Context;
use crate::util::helper::{
    bbox_to_non_zero_rect, clip_to_rect, MaskTypeExt, NameExt, RectExt, TransformExt,
};

/// Render a mask into a content stream.
pub fn render(
    group: &Group,
    mask: SharedMask,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
) {
    content.set_parameters(create(group, mask, chunk, ctx).to_pdf_name());
}

/// Turn a mask into an graphics state object. Returns the name (= the name in the `Resources` dictionary) of
/// the mask
pub fn create(
    parent: &Group,
    mask: SharedMask,
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> Rc<String> {
    let mask = mask.borrow();

    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let mut content = Content::new();
    content.save_state();

    if let Some(recursive_mask) = &mask.mask {
        render(parent, recursive_mask.clone(), chunk, &mut content, ctx);
    }

    let parent_svg_bbox = bbox_to_non_zero_rect(parent.bounding_box);

    let actual_rect = match mask.units {
        Units::ObjectBoundingBox => mask.rect.bbox_transform(parent_svg_bbox),
        Units::UserSpaceOnUse => mask.rect,
    };

    // In addition to setting the bounding box, we also apply a clip path to the mask rect to
    // circumvent a bug in Firefox where the bounding box is not applied properly for some transforms.
    // If we don't do this, the "half-width-region-with-rotation.svg" test case won't render properly.
    clip_to_rect(actual_rect, &mut content);

    let mut accumulated_transform = Transform::default();
    if mask.content_units == Units::ObjectBoundingBox {
        content.transform(Transform::from_bbox(parent_svg_bbox).to_pdf_transform());
        accumulated_transform = Transform::from_bbox(parent_svg_bbox);
    }

    group::render(&mask.root, chunk, &mut content, ctx, accumulated_transform);

    content.restore_state();
    let content_stream = ctx.finish_content(content);

    let mut x_object = chunk.form_xobject(x_ref, &content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object
        .group()
        .transparency()
        .isolated(false)
        .knockout(false)
        .color_space()
        .icc_based(ctx.deferrer.srgb_ref());

    x_object.bbox(actual_rect.to_pdf_rect());
    x_object.finish();

    let gs_ref = ctx.alloc_ref();
    let mut gs = chunk.ext_graphics(gs_ref);
    gs.soft_mask().subtype(mask.kind.to_pdf_mask_type()).group(x_ref);

    ctx.deferrer.add_graphics_state(gs_ref)
}
