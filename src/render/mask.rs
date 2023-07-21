use std::rc::Rc;

use pdf_writer::types::MaskType;
use pdf_writer::{Content, Filter, Finish, PdfWriter};
use usvg::{Mask, Node, NodeKind, Transform, Units};

use super::group;
use crate::util::context::Context;
use crate::util::helper::{plain_bbox, NameExt, RectExt, TransformExt};

/// Render a mask into a content stream.
pub fn render(
    node: &Node,
    mask: Rc<Mask>,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    content.set_parameters(create(node, mask, writer, ctx).as_name());
}

/// Turn a mask into an graphics state object. Returns the name (= the name in the `Resources` dictionary) of
/// the mask
pub fn create(
    parent: &Node,
    mask: Rc<Mask>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Rc<String> {
    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let mut content = Content::new();
    content.save_state();

    if let Some(recursive_mask) = &mask.mask {
        render(parent, recursive_mask.clone(), writer, &mut content, ctx);
    }

    let parent_svg_bbox = plain_bbox(parent);

    let pdf_bbox = match mask.units {
        Units::ObjectBoundingBox => mask.rect.bbox_transform(parent_svg_bbox),
        Units::UserSpaceOnUse => mask.rect,
    }
    .as_pdf_rect();

    match *mask.root.borrow() {
        NodeKind::Group(ref group) => {
            if mask.content_units == Units::ObjectBoundingBox {
                content.transform(Transform::from_bbox(parent_svg_bbox).as_array());
            }
            content.x_object(group::create(&mask.root, group, writer, ctx).as_name());
        }
        _ => unreachable!(),
    };

    content.restore_state();
    let content_stream = ctx.finish_content(content);

    let mut x_object = writer.form_xobject(x_ref, &content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object
        .group()
        .transparency()
        .isolated(true)
        .knockout(false)
        .color_space()
        .srgb();

    x_object.bbox(pdf_bbox);
    x_object.finish();

    let mask_type = match mask.kind {
        usvg::MaskType::Alpha => MaskType::Alpha,
        usvg::MaskType::Luminance => MaskType::Luminosity,
    };

    let gs_ref = ctx.alloc_ref();
    let mut gs = writer.ext_graphics(gs_ref);
    gs.soft_mask().subtype(mask_type).group(x_ref);

    ctx.deferrer.add_graphics_state(gs_ref)
}
