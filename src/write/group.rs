use crate::util::{calc_node_bbox, Context, TransformExt};
use crate::write::render::Render;
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref};
use usvg::{Node, NodeExt, Transform};
use crate::write::clip::alloc_clip_path;

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

    child_content.save_state();
    child_content.transform(group.transform.get_transform());

    if let Some(clip_path) = &group.clip_path {
        let name = alloc_clip_path(clip_path.clone(), writer, ctx);
        child_content.set_parameters(Name(name.as_bytes()));
    }

    for child in node.children() {
        child.render(writer, &mut child_content, ctx);
    }

    child_content.restore_state();

    let mut child_content_stream = child_content.finish();

    let mut x_object = writer.form_xobject(reference, &child_content_stream);
    ctx.pop_context(&mut x_object.resources());

    // TODO: Figure out a more elegant way to calculate the bbox?
    let bbox = calc_node_bbox(&node, group.transform)
        .and_then(|b| b.to_rect())
        .unwrap_or_else(|| usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap());;

    let mut group = x_object.group();
    group.transparency().isolated(true).knockout(false).color_space().srgb();
    group.finish();

    x_object.bbox(Rect::new(bbox.x() as f32, bbox.y() as f32,
                            (bbox.x() + bbox.width()) as f32, (bbox.y() + bbox.height()) as f32));
    x_object.finish();
    (name, reference)
}
