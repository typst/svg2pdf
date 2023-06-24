pub mod clip;
pub mod group;
pub mod path;

use crate::util::Context;
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, NodeKind, Tree};

pub fn tree_to_stream(
    tree: &Tree,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    // Root of tree is always a group, so we can just directly iterate over all of the children
    for el in tree.root.children() {
        el.render(writer, content, ctx);
    }
}

pub trait Render {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context);
}


impl Render for Node {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context) {
        match *self.borrow() {
            NodeKind::Path(ref path) => path::render(path, writer, content, ctx),
            NodeKind::Group(ref group) => {
                group::render(group, self, writer, content, ctx)
            }
            _ => {} // _ => unimplemented!()
        }
    }
}
