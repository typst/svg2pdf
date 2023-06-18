use crate::util::{Context, TransformExt};
use crate::write::{group, path};
use pdf_writer::{Content, PdfWriter};
use usvg::utils::view_box_to_transform;
use usvg::{Node, NodeKind, Tree};

pub fn tree_to_stream(
    tree: &Tree,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    content: &mut Content,
) {
    content.save_state();
    // Apply the base transformation to convert the svg viewport + viewbox into
    // the PDF coordinate system.
    apply_dpi_transform(ctx, content);
    apply_viewport_transforms(ctx, content);
    apply_viewbox_transforms(ctx, content);

    let _ = &tree.root.render(writer, content, ctx);

    content.restore_state();
}

fn apply_dpi_transform(ctx: &Context, content: &mut Content) {
    content.transform([ctx.dpi_factor(), 0.0, 0.0, ctx.dpi_factor(), 0.0, 0.0]);
}

fn apply_viewport_transforms(ctx: &Context, content: &mut Content) {
    // First translate to align the origins and then invert the y-axis
    content.transform([1.0, 0.0, 0.0, -1.0, 0.0, ctx.size.height() as f32]);
}

fn apply_viewbox_transforms(ctx: &Context, content: &mut Content) {
    // Delegate to usvg function
    content.transform(
        view_box_to_transform(ctx.viewbox.rect, ctx.viewbox.aspect, ctx.size)
            .get_transform(),
    );
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
