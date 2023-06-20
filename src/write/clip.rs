use crate::util::{calc_node_bbox_to_rect, Context, Units};
use crate::write::group;
use pdf_writer::PdfWriter;
use std::rc::Rc;
use usvg::{Node, NodeKind, Transform};
use usvg::NodeKind::Group;

pub fn create_clip_path(
    clip_path: Rc<usvg::ClipPath>,
    parent: &Node,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    ctx.context_frame.append_transform(&clip_path.transform);

    let parent_bbox = calc_node_bbox_to_rect(&parent, Transform::default());
    match clip_path.units {
        usvg::Units::ObjectBoundingBox => ctx.context_frame.set_units(Units::ObjectBoundingBox(Transform::from_bbox(parent_bbox))),
        usvg::Units::UserSpaceOnUse => ctx.context_frame.set_units(Units::UserSpaceOnUse)
    }

    match *(*clip_path).root.borrow() {
        NodeKind::Group(ref group) => {

            if let Some(recursive_clip_path) = &(*clip_path).clip_path {
                let mut new_group = usvg::Group::default();
                new_group.clip_path = Some(recursive_clip_path.clone());

                let mut new_node = Node::new(NodeKind::Group(new_group.clone()));
                new_node.append((*clip_path).root.make_deep_copy());

                let (_, group_ref) =
                    group::create_x_object(&new_group.clone(), &new_node, writer, ctx);
                let name = ctx.alloc_soft_mask(group_ref);
                name
            } else {
                let (_, group_ref) =
                    group::create_x_object(group, &(*clip_path).root, writer, ctx);
                let name = ctx.alloc_soft_mask(group_ref);
                name
            }
        }
        _ => unreachable!(),
    }
}
