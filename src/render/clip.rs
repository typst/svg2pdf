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

    let name = match *clip_path.root.borrow() {
        NodeKind::Group(ref group) => {
            // TODO: Find a better way to do this
            if let Some(recursive_clip_path) = &clip_path.clip_path {
                let new_group = usvg::Group {
                    clip_path: Some(recursive_clip_path.clone()),
                    ..Default::default()
                };

                let new_node = Node::new(NodeKind::Group(new_group.clone()));
                new_node.append(clip_path.root.make_deep_copy());

                let (_, group_ref) =
                    group::create_x_object(&new_node, &new_group, writer, ctx);

                ctx.alloc_soft_mask(group_ref)
            } else {
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
        }
        _ => unreachable!(),
    };

    ctx.context_frame.pop();
    name
}
