use crate::context::Context;
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, NodeKind};

pub fn convert_stream(node: &Node, writer: &mut PdfWriter, ctx: &mut Context) -> Vec<u8> {
    let mut content = Content::new();

    // Apply the base transformation to convert the svg coordinate system into
    // the PDF coordinate system.
    content.save_state();
    content.transform([1.0, 0.0, 0.0, 1.0, 0.0, ctx.viewport.height()]);
    content.transform([1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);

    for element in node.children() {
        match *element.borrow() {
            NodeKind::Path(ref path) => {
                path.render(&element, writer, &mut content, ctx);
            }
            _ => {}
        }
    }

    content.restore_state();

    content.finish()
}

pub(crate) trait Render {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    );
}
