use crate::util::{calc_node_bbox, calc_node_bbox_to_rect, Context, Units};
use crate::write::clip::alloc_clip_path;
use crate::write::render::Render;
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref};
use usvg::{Node, Transform};

pub(crate) fn render(
    group: &usvg::Group,
    node: &Node,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    let (name, _) = create_x_object(group, node, writer, ctx);
    content.x_object(Name(name.as_bytes()));
}

pub(crate) fn create_x_object(
    group: &usvg::Group,
    node: &Node,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> (String, Ref) {
    let (name, reference) = ctx.alloc_named_x_object();

    ctx.push_context();

    let mut child_content = Content::new();

    ctx.context_frame.push(&mut child_content);
    ctx.context_frame.append_transform(&group.transform);

    let pdf_bbox = calc_node_bbox_to_rect(&node, ctx.context_frame.transform());

    if let Some(clip_path) = &group.clip_path {
        let svg_bbox = calc_node_bbox_to_rect(&node, Transform::default());
        match clip_path.units {
            usvg::Units::ObjectBoundingBox => ctx.context_frame.set_units(Units::ObjectBoundingBox(Transform::from_bbox(svg_bbox))),
            usvg::Units::UserSpaceOnUse => ctx.context_frame.set_units(Units::UserSpaceOnUse)
        }

        let name = alloc_clip_path(clip_path.clone(), writer, ctx);
        child_content.set_parameters(Name(name.as_bytes()));
    }

    if group.opacity.get() != 1.0 {
        let name = ctx.alloc_opacity(
            None,
            Some(group.opacity.get() as f32)
        );
        child_content.set_parameters(Name(name.as_bytes()));
    }

    for child in node.children() {
        child.render(writer, &mut child_content, ctx);
    }

    ctx.context_frame.pop(&mut child_content);

    let child_content_stream = child_content.finish();

    let mut x_object = writer.form_xobject(reference, &child_content_stream);
    ctx.pop_context(&mut x_object.resources());

    let mut group = x_object.group();
    group
        .transparency()
        .isolated(true)
        .knockout(false)
        .color_space()
        .srgb();
    group.finish();

    x_object.bbox(Rect::new(
        pdf_bbox.x() as f32,
        pdf_bbox.y() as f32,
        (pdf_bbox.x() + pdf_bbox.width()) as f32,
        (pdf_bbox.y() + pdf_bbox.height()) as f32,
    ));
    x_object.finish();
    (name, reference)
}
