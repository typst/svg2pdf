mod context;
mod transform;
mod write;

use context::*;
use write::*;

use pdf_writer::{Content, Finish, PdfWriter, Rect, Ref, TextStr};
use usvg::Tree;

/// A color helper function that stores colors with values between 0.0 and 1.0.
#[derive(Debug, Clone, Copy)]
struct RgbColor {
    /// Red.
    r: f32,
    /// Green.
    g: f32,
    /// Blue.
    b: f32,
}

impl RgbColor {
    /// Create a new color.
    fn new(r: f32, g: f32, b: f32) -> RgbColor {
        RgbColor { r, g, b }
    }

    /// Create a new color from u8 color components between 0.0 and 255.0.
    fn from_u8(r: u8, g: u8, b: u8) -> RgbColor {
        RgbColor::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    /// Create a RGB array for use in PDF.
    fn to_array(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
}

impl From<usvg::Color> for RgbColor {
    fn from(color: usvg::Color) -> Self {
        Self::from_u8(color.red, color.green, color.blue)
    }
}

pub fn convert_tree(tree: &Tree) -> Vec<u8> {
    let mut ctx = Context::new(Viewport::new(
        tree.view_box.rect.width() as f32,
        tree.view_box.rect.height() as f32,
    ));

    let mut writer = PdfWriter::new();
    let catalog_id = ctx.alloc_ref();
    let page_tree_id = ctx.alloc_ref();
    let page_id = ctx.alloc_ref();
    let content_id = ctx.alloc_ref();

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).count(1).kids([page_id]);

    let mut page = writer.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, ctx.viewport.width(), ctx.viewport.height()));
    page.parent(page_tree_id);
    page.contents(content_id);
    page.finish();

    let content = render::convert_stream(&tree.root, &mut writer, &mut ctx);

    let mut stream = writer.stream(content_id, &content);
    stream.finish();

    let document_info_id = ctx.alloc_ref();
    writer.document_info(document_info_id).producer(TextStr("svg2pdf"));

    writer.finish()
}
