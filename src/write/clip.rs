use crate::util::{calc_node_bbox_to_rect, Context};
use crate::write::group;
use pdf_writer::{PdfWriter};
use std::rc::Rc;
use usvg::{Node, NodeKind, Transform, Units};


pub fn create_clip_path(
    clip_path: Rc<usvg::ClipPath>,
    parent: &Node,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    ctx.context_frame.push();
    ctx.context_frame.append_transform(&clip_path.transform);

    let name = match *(*clip_path).root.borrow() {
        NodeKind::Group(ref group) => {
            // TODO: Find a better way to do this
            if let Some(recursive_clip_path) = &(*clip_path).clip_path {
                let mut new_group = usvg::Group::default();
                new_group.clip_path = Some(recursive_clip_path.clone());

                let new_node = Node::new(NodeKind::Group(new_group.clone()));
                new_node.append((*clip_path).root.make_deep_copy());

                let (_, group_ref) =
                    group::create_x_object(&new_group.clone(), &new_node, writer, ctx);
                let name = ctx.alloc_soft_mask(group_ref);
                name
            } else {
                let parent_bbox = calc_node_bbox_to_rect(&parent, Transform::default());
                ctx.context_frame.push();

                if clip_path.units == Units::ObjectBoundingBox {
                    ctx.context_frame.append_transform(&Transform::from_bbox(parent_bbox));
                }
                let (_, group_ref) =
                    group::create_x_object(group, &(*clip_path).root, writer, ctx);
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
