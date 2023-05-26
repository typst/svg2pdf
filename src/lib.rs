mod util;
mod write;
mod color;

use util::*;
use write::*;
use color::*;

use pdf_writer::{Content, Finish, PdfWriter, Rect, Ref, TextStr, Name, writers::ColorSpace};
use usvg::Tree;

pub fn convert_tree(tree: &Tree) -> Vec<u8> {
    let mut ctx = Context::new(tree);

    let mut writer = PdfWriter::new();
    let catalog_id = ctx.alloc_ref();
    let page_tree_id = ctx.alloc_ref();
    let page_id = ctx.alloc_ref();
    let content_id = ctx.alloc_ref();

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).count(1).kids([page_id]);
    
    let mut page = writer.page(page_id);
    page.resources().color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
    page.media_box(ctx.get_media_box());
    page.parent(page_tree_id);
    page.contents(content_id);
    page.finish();

    let content = render::tree_to_stream(&tree, &mut writer, &mut ctx);

    let mut stream = writer.stream(content_id, &content);
    stream.finish();

    let document_info_id = ctx.alloc_ref();
    writer.document_info(document_info_id).producer(TextStr("svg2pdf"));

    writer.finish()
}
