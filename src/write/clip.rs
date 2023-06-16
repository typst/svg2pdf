use crate::util::Context;
use crate::write::group;
use pdf_writer::PdfWriter;
use std::rc::Rc;
use usvg::NodeKind;

pub fn alloc_clip_path(
    clip_path: Rc<usvg::ClipPath>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    match *(*clip_path).root.borrow() {
        NodeKind::Group(ref group) => {
            let (_, group_ref) =
                group::create_x_object(group, &(*clip_path).root, writer, ctx);
            let name = ctx.alloc_soft_mask(group_ref);
            name
        }
        _ => unreachable!(),
    }
}
