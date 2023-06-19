use crate::util::Context;
use crate::write::{group, path};
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, NodeKind, Tree};

pub fn tree_to_stream(
    tree: &Tree,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    content: &mut Content,
) {
    let _ = &tree.root.render(writer, content, ctx);
}

pub trait Render {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context);
}

impl Render for Node {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context) {
        match *self.borrow() {
            NodeKind::Path(ref path) => path::render(path, content, ctx),
            NodeKind::Group(ref group) => {
                group::render(group, &self, writer, content, ctx)
            }
            _ => {} // _ => unimplemented!()
        }
    }
}
