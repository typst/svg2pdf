use crate::util::Context;
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, NodeKind, Tree};

pub fn tree_to_stream(tree: &Tree, writer: &mut PdfWriter, ctx: &mut Context) -> Vec<u8> {
    let mut content = Content::new();

    content.save_state();
    // Apply the base transformation to convert the svg coordinate system into
    // the PDF coordinate system.
    // Apply the scaling to account for dpi
    content.transform([ctx.dpi_factor(), 0.0, 0.0, ctx.dpi_factor(), 0.0, 0.0]);
    // Align the origin of the cordinate system.
    content.transform([1.0, 0.0, 0.0, 1.0, 0.0, ctx.viewport.height()]);
    // Invert the direction of the y axis.
    content.transform([1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    // Apply the x and y of the view box.
    content.transform([1.0, 0.0, 0.0, 1.0, -ctx.viewport.x(), -ctx.viewport.y()]);

    node_to_stream(&tree.root, writer, ctx, &mut content);

    content.restore_state();
    content.finish()
}

pub fn node_to_stream(node: &Node, writer: &mut PdfWriter, ctx: &mut Context, content: &mut Content) {
    for element in node.children() {
        match *element.borrow() {
            NodeKind::Path(ref path) => path.render(&element, writer, content, ctx),
            NodeKind::Group(ref group) => group.render(&element, writer, content, ctx),
            _ => {}
        }
    }
}

pub trait Render {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    );
}
