use crate::render::clip_path;
use crate::render::Render;
use crate::util::context::Context;
use crate::util::helper::{NameExt, RectExt};
use pdf_writer::{Content, Finish, PdfWriter, Ref};
use pdf_writer::writers::FormXObject;
use usvg::Node;

pub(crate) fn render(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    let (name, _) = create_x_object(node, group, writer, ctx);
    content.x_object(name.as_name());
}

pub(crate) fn create_x_object(
    node: &Node,
    group: &usvg::Group,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> (String, Ref) {
    let (name, reference) = ctx.deferrer.add_x_object();
    ctx.deferrer.push();

    ctx.context_frame.push();
    ctx.context_frame.append_transform(&group.transform);

    let mut child_content = Content::new();
    child_content.save_state();

    let pdf_bbox = ctx.plain_bbox(node).as_pdf_rect(&ctx.context_frame.full_transform());

    if let Some(clip_path) = &group.clip_path {
        clip_path::render(node, clip_path.clone(), writer, &mut child_content, ctx);
    }

    // TODO: This doesn't work correctly yet. If the group has an opacity and the paths directly
    // beneath it as well, the group opacity will be overwritten because the graphics state of the
    // child will override it. So if there is a group opacity, we will have to wrap everything in
    // another x object.
    if group.opacity.get() != 1.0 {
        let name = ctx.deferrer.add_opacity(Some(group.opacity.get() as f32), Some(group.opacity.get() as f32));
        child_content.set_parameters(name.as_name());
    }

    for child in node.children() {
        child.render(writer, &mut child_content, ctx);
    }

    ctx.context_frame.pop();
    child_content.restore_state();

    let child_content_stream = child_content.finish();

    let mut x_object = writer.form_xobject(reference, &child_content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    make_transparency_group(&mut x_object);

    x_object.bbox(pdf_bbox);
    x_object.finish();
    (name, reference)
}

pub fn make_transparency_group(x_object: &mut FormXObject) {
    let mut group = x_object.group();
    group
        .transparency()
        .isolated(true)
        .knockout(false)
        .color_space()
        .srgb();
    group.finish();
}
