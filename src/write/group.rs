use crate::util::{Context, TransformExt};
use crate::write::render::Render;
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, Transform};

use super::render::node_to_stream;

impl Render for usvg::Group {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {

        if !self.transform.is_default() {
            content.save_state();
            content.transform(self.transform.get_transform());
        }

        node_to_stream(node, writer, ctx, content);

        if !self.transform.is_default() {
            content.restore_state();
        }
    }
}
