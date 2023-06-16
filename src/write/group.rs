use crate::util::{calc_node_bbox, Context, TransformExt};
use crate::write::render::Render;
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect};
use usvg::{Node, NodeExt, Transform};

pub(crate) fn render(
    group: &usvg::Group,
    node: &Node,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {

    let (name, reference) = ctx.alloc_named_x_object();

    content.save_state();
    content.transform(group.transform.get_transform());
    content.x_object(Name(name.as_bytes()));
    content.restore_state();

    ctx.push_context();

    let mut child_content = Content::new();

    for child in node.children() {
        child.render(writer, &mut child_content, ctx);
    }

    let mut child_content_stream = child_content.finish();

    let mut xobject = writer.form_xobject(reference, &child_content_stream);
    ctx.pop_context(&mut xobject.resources());

    // TODO: Figure out a more elegant way to calculate the bbox?
    let bbox = calc_node_bbox(&node, Transform::default())
        .and_then(|b| b.to_rect())
        .unwrap_or_else(|| usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap());;


    xobject.bbox(Rect::new(bbox.x() as f32, bbox.y() as f32,
                           (bbox.x() + bbox.width()) as f32, (bbox.y() + bbox.height()) as f32));
    xobject.finish();
}
