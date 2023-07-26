use std::rc::Rc;

use pdf_writer::{Content, Filter, Finish, PdfWriter};
use usvg::{BlendMode, Node, Transform};

use super::{clip_path, mask, Render};
use crate::util::context::Context;
use crate::util::helper::{plain_bbox, BlendModeExt, NameExt, RectExt, TransformExt};

/// Render a group into a content stream.
pub fn render(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if group.opacity.get() != 1.0 || group.blend_mode != BlendMode::Normal {
        content.save_state();
        let gs_ref = ctx.alloc_ref();
        let mut gs = writer.ext_graphics(gs_ref);
        gs.non_stroking_alpha(group.opacity.get())
            .stroking_alpha(group.opacity.get())
            // TODO: Blend modes don't quite work correctly yet.
            .blend_mode(group.blend_mode.to_pdf_blend_mode());

        gs.finish();
        content.set_parameters(ctx.deferrer.add_graphics_state(gs_ref).as_name());

        content.x_object(
            create_x_object(node, group, writer, ctx, accumulated_transform).as_name(),
        );
        content.restore_state();
    } else {
        create_to_stream(node, group, writer, content, ctx, accumulated_transform);
    }
}

/// Turn a group into an XObject. Returns the name (= the name in the `Resources` dictionary) of
/// the group
fn create_x_object(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    accumulated_transform: Transform,
) -> Rc<String> {
    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let pdf_bbox = plain_bbox(node).transform(group.transform).unwrap().as_pdf_rect();

    let mut content = Content::new();

    create_to_stream(node, group, writer, &mut content, ctx, accumulated_transform);

    let content_stream = ctx.finish_content(content);

    let mut x_object = writer.form_xobject(x_ref, &content_stream);
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
        .srgb();

    x_object.bbox(pdf_bbox);
    x_object.finish();

    ctx.deferrer.add_x_object(x_ref)
}

/// Write a group into a content stream. Opacities will be ignored. If opacities are needed,
/// you should use the `create` method instead.
fn create_to_stream(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    content.save_state();
    content.transform(group.transform.as_array());
    let accumulated_transform = accumulated_transform.pre_concat(group.transform);

    if let Some(mask) = &group.mask {
        mask::render(node, mask.clone(), writer, content, ctx);
    }

    if let Some(clip_path) = &group.clip_path {
        clip_path::render(node, clip_path.clone(), writer, content, ctx);
    }

    for child in node.children() {
        child.render(writer, content, ctx, accumulated_transform);
    }

    content.restore_state();
}
