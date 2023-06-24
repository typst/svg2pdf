use crate::render::group;
use crate::util::helper::NameExt;
use crate::util::{calc_node_bbox, Context};
use pdf_writer::{Content, Finish, PdfWriter};
use std::rc::Rc;
use usvg::{ClipPath, Node, NodeKind, Transform, Units};

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
    clip_path: Rc<ClipPath>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    ctx.deferrer.push();
    let x_object_reference = ctx.deferrer.alloc_ref();

    let mut content = Content::new();
    content.save_state();

    if let Some(recursive_clip_path) = &clip_path.clip_path {
        render(parent, recursive_clip_path.clone(), writer, &mut content, ctx);
    }

    ctx.context_frame.push();
    ctx.context_frame.append_transform(&clip_path.transform);

    let pdf_bbox = ctx.pdf_bbox(parent);

    match *clip_path.root.borrow() {
        NodeKind::Group(ref group) => {
            let parent_svg_bbox = calc_node_bbox(parent, Transform::default())
                .unwrap()
                .to_rect()
                .unwrap();

            if clip_path.units == Units::ObjectBoundingBox {
                ctx.context_frame
                    .append_transform(&Transform::from_bbox(parent_svg_bbox));
            }
            let (group_name, _) =
                group::create_x_object(&clip_path.root, group, writer, ctx);
            content.x_object(group_name.as_name());
        }
        _ => unreachable!(),
    };

    ctx.context_frame.pop();

    content.restore_state();
    let content_stream = content.finish();

    let mut x_object = writer.form_xobject(x_object_reference, &content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    x_object.bbox(pdf_bbox);
    x_object.finish();

    ctx.deferrer.add_soft_mask(x_object_reference)
}
