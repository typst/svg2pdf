use crate::context::Context;
use crate::write::render::Render;
use pdf_writer::{Content, PdfWriter};
use usvg::Node;

impl Render for usvg::Group {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
    }
}
