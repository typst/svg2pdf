use crate::color::SRGB;
use crate::util::{Context, TransformExt};
use pdf_writer::types::ColorSpaceOperand;
use pdf_writer::{Content, Finish, Name, PdfWriter, Ref};
use pdf_writer::writers::FormXObject;
use usvg::{Node, NodeKind, Tree};
use usvg::utils::view_box_to_transform;

pub fn tree_to_stream(tree: &Tree, writer: &mut PdfWriter, ctx: &mut Context, content: &mut Content) {

    content.save_state();
    // Apply the base transformation to convert the svg viewport + viewbox into
    // the PDF coordinate system.
    apply_dpi_transform(ctx, content);
    apply_viewport_transforms(ctx, content);
    apply_viewbox_transforms(ctx, content);

    node_to_stream(&tree.root, writer, ctx, content);

    content.restore_state();
}

pub fn node_to_stream(
    node: &Node,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    content: &mut Content
) {
    for element in node.children() {
        match *element.borrow() {
            NodeKind::Path(ref path) => path.render(&element, writer, content, ctx),
            NodeKind::Group(ref group) => group.render(&element, writer, content, ctx),
            _ => {}
        }
    }
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
    content.transform(view_box_to_transform(ctx.viewbox.rect, ctx.viewbox.aspect, ctx.size).get_transform());
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
