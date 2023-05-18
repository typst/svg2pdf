use crate::render::Render;
use crate::render::{clip_path, mask};
use crate::util::context::Context;
use crate::util::helper::{plain_bbox, NameExt, RectExt, TransformExt};
use pdf_writer::{Content, Filter, Finish, PdfWriter};
use std::rc::Rc;
use usvg::Node;

/// Render a group into a content stream.
pub fn render(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    content.x_object(create(node, group, writer, ctx).as_name());
}

/// Turn a group into an XObject. Returns the name (= the name in the `Resources` dictionary) of
/// the group
pub fn create(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Rc<String> {
    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let pdf_bbox = plain_bbox(node).as_pdf_rect();

    let mut child_content = Content::new();
    child_content.save_state();

    if let Some(mask) = &group.mask {
        mask::render(node, mask.clone(), writer, &mut child_content, ctx);
    }

    if let Some(clip_path) = &group.clip_path {
        clip_path::render(node, clip_path.clone(), writer, &mut child_content, ctx);
    }

    // TODO: This doesn't work correctly yet. If the group has an opacity and the paths directly
    // beneath it as well, the group opacity will be overwritten because the graphics state of the
    // child will override it since they are in the same XObject.
    if group.opacity.get() != 1.0 {
        let gs_ref = ctx.alloc_ref();
        let mut gs = writer.ext_graphics(gs_ref);
        gs.non_stroking_alpha(group.opacity.get() as f32)
            .stroking_alpha(group.opacity.get() as f32)
            .finish();
        child_content.set_parameters(ctx.deferrer.add_graphics_state(gs_ref).as_name());
    }

    for child in node.children() {
        child.render(writer, &mut child_content, ctx);
    }

    child_content.restore_state();

    let child_content_stream = ctx.finish_content(child_content);

    let mut x_object = writer.form_xobject(x_ref, &child_content_stream);
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
    x_object.matrix(group.transform.as_array());
    x_object.finish();

    ctx.deferrer.add_x_object(x_ref)
}
