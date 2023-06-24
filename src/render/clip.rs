use crate::util::{calc_node_bbox, Context};
use crate::render::group;
use pdf_writer::{Content, PdfWriter};
use std::rc::Rc;
use usvg::{ClipPath, Node, NodeKind, Transform, Units};
use crate::util::helper::NameExt;

pub(crate) fn render(
    node: &Node,
    clip_path: Rc<ClipPath>,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    let name = create_soft_mask(node, clip_path, writer, ctx);
    content.set_parameters(name.as_name());
}

pub(crate) fn create_soft_mask(
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

            if clip_path.units == Units::ObjectBoundingBox {
                ctx.context_frame
                    .append_transform(&Transform::from_bbox(parent_bbox));
            }
            let (_, group_ref) =
                group::create_x_object(&clip_path.root, group, writer, ctx);
            let name = ctx.deferrer.add_soft_mask(group_ref);

            name
        }
        _ => unreachable!(),
    };

    ctx.context_frame.pop();
    name
}
