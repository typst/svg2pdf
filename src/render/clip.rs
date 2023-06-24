use crate::util::{calc_node_bbox, Context};
use crate::render::group;
use pdf_writer::PdfWriter;
use std::rc::Rc;
use usvg::{Node, NodeKind, Transform, Units};

pub(crate) fn render(
    parent: &Node,
    clip_path: Rc<usvg::ClipPath>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    ctx.context_frame.push();
    ctx.context_frame.append_transform(&clip_path.transform);

    // TODO: Think about a more elegant way of solving this
    match *clip_path.root.borrow_mut() {
        NodeKind::Group(ref mut group) => {
            if let Some(recursive_clip_path) = &clip_path.clip_path {
                group.clip_path = Some(recursive_clip_path.clone());
            }
        }
        _ => unreachable!(),
    };

    let name = match *clip_path.root.borrow() {
        NodeKind::Group(ref group) => {
            let parent_bbox = calc_node_bbox(parent, Transform::default())
                .unwrap()
                .to_rect()
                .unwrap();
            ctx.context_frame.push();

            if clip_path.units == Units::ObjectBoundingBox {
                ctx.context_frame
                    .append_transform(&Transform::from_bbox(parent_bbox));
            }
            let (_, group_ref) =
                group::create_x_object(&clip_path.root, group, writer, ctx);
            let name = ctx.alloc_soft_mask(group_ref);

            ctx.context_frame.pop();

            name
        }
        _ => unreachable!(),
    };

    ctx.context_frame.pop();
    name
}
