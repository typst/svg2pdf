pub mod clip_path;
pub mod gradient;
pub mod group;
#[cfg(feature = "image")]
pub mod image;
pub mod mask;
pub mod path;
pub mod pattern;

use std::rc::Rc;

use pdf_writer::{Content, Filter, Finish, PdfWriter, Rect};
use usvg::{Node, NodeKind, Tree};

use crate::util::context::Context;
use crate::util::helper::{plain_bbox, TransformExt};

/// Turn a tree into an XObject. Returns the name (= the name in the `Resources` dictionary) of
/// the XObject
pub fn tree_to_x_object(
    tree: &Tree,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Rc<String> {
    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let mut child_content = Content::new();
    child_content.save_state();
    child_content.transform(ctx.get_viewbox_transform().as_array());

    // The root of a tree is always a group, so we can directly iterate over the children
    for el in tree.root.children() {
        el.render(writer, &mut child_content, ctx);
    }

    child_content.restore_state();

    let child_content_stream = ctx.finish_content(child_content);

    let mut x_object = writer.form_xobject(x_ref, &child_content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object.bbox(Rect::new(0.0, 0.0, ctx.size.width() as f32, ctx.size.height() as f32));
    // Apply the base transform
    x_object.matrix(ctx.get_initial_transform().as_array());
    x_object.finish();

    ctx.deferrer.add_x_object(x_ref)
}

trait Render {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context);
}

impl Render for Node {
    fn render(&self, writer: &mut PdfWriter, content: &mut Content, ctx: &mut Context) {
        match *self.borrow() {
            NodeKind::Path(ref path) => {
                path::render(path, &plain_bbox(self), writer, content, ctx)
            }
            NodeKind::Group(ref group) => {
                group::render(self, group, writer, content, ctx)
            }
            #[cfg(feature = "image")]
            NodeKind::Image(ref image) => image::render(image, writer, content, ctx),
            _ => {}
        }
    }
}
