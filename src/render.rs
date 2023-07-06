pub mod clip_path;
pub mod group;
pub mod image;
pub mod path;
pub mod pattern;

use crate::util::context::Context;
use pdf_writer::{Content, Finish, PdfWriter, Rect};
use usvg::{Node, NodeKind, Tree};
use crate::util::helper::{NameExt, TransformExt};

pub fn tree_to_x_object(
    tree: &Tree,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (name, reference) = ctx.deferrer.add_x_object();
    ctx.deferrer.push();

    let mut child_content = Content::new();

    for el in tree.root.children() {
        el.render(writer, &mut child_content, ctx);
    }

    let child_content_stream = child_content.finish();

    let mut x_object = writer.form_xobject(reference, &child_content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    x_object.bbox(Rect::new(0.0, 0.0, ctx.size.width() as f32, ctx.size.height() as f32));
    x_object.matrix(ctx.get_base_transform().as_array());
    x_object.finish();
    name
}

pub trait Render {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context);
}

impl Render for Node {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context) {
        match *self.borrow() {
            NodeKind::Path(ref path) => {
                path::render(path, &ctx.plain_bbox(self), writer, content, ctx)
            }
            NodeKind::Group(ref group) => {
                group::render(self, group, writer, content, ctx)
            }
            NodeKind::Image(ref image) => {
                image::render(self, image, writer, content, ctx)
            }
            _ => unimplemented!(),
        }
    }
}
