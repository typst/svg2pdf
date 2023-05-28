use crate::color::SRGB;
use crate::util::{Context, TransformExt};
use pdf_writer::types::ColorSpaceOperand;
use pdf_writer::{Content, PdfWriter};
use usvg::{Node, NodeKind, Tree};
use usvg::utils::view_box_to_transform;

pub fn tree_to_stream(tree: &Tree, writer: &mut PdfWriter, ctx: &mut Context) -> Vec<u8> {
    let mut content = Content::new();

    content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));

    content.save_state();
    // Apply the base transformation to convert the svg viewport + viewbox into
    // the PDF coordinate system.
    apply_dpi_transform(ctx, &mut content);
    apply_viewport_transforms(ctx, &mut content);
    apply_viewbox_transforms(ctx, &mut content);

    node_to_stream(&tree.root, writer, ctx, &mut content);

    content.restore_state();
    content.finish()
}

pub fn node_to_stream(
    node: &Node,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    content: &mut Content,
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
