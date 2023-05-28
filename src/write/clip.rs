use std::rc::Rc;
use crate::util::{Context, TransformExt};

use pdf_writer::{PdfWriter, Content};
use usvg::NodeKind;

use super::{path::draw_path, render::Render};

/// Draw a clipping path into a content stream.
pub fn apply_clip_path(
    path: Rc<usvg::ClipPath>,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context
) {

    if let Some(path) = path.clip_path.clone() {
        apply_clip_path(path, writer, content, ctx);
    }

    for child in (*path).root.children() {
        match *child.borrow() {
            NodeKind::Path(ref path) => {
                path.render(&child, writer, content, ctx);
            }
            NodeKind::Group(ref group) => {
                group.render(&child, writer, content, ctx);
            }
            _ => unreachable!(),
        }
    }

    content.clip_nonzero();
    content.end_path();
}