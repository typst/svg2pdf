use crate::util::{Context, TransformExt};
use crate::write::render::Render;
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, Transform};

use super::clip::apply_clip_path;
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

        if let Some(clip_path) = self.clip_path.clone() {
            ctx.register_clip_path();
            apply_clip_path(clip_path, writer, content, ctx);
            ctx.unregister_clip_path();
        }

        node_to_stream(node, writer, ctx, content);

        if !self.transform.is_default() {
            content.restore_state();
        }
    }
}
