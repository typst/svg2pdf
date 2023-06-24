mod util;
mod render;

use util::*;

use pdf_writer::{Content, Finish, PdfWriter, TextStr};
use usvg::Tree;

pub fn convert_tree(tree: &Tree) -> Vec<u8> {
    let mut ctx = Context::new(tree);
    let mut writer = PdfWriter::new();

    let catalog_id = ctx.deferrer.alloc_ref();
    let page_tree_id = ctx.deferrer.alloc_ref();
    let page_id = ctx.deferrer.alloc_ref();
    let content_id = ctx.deferrer.alloc_ref();

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).count(1).kids([page_id]);

    // Generate main content
    ctx.deferrer.push_context();
    let mut content = Content::new();
    render::tree_to_stream(tree, &mut writer, &mut content, &mut ctx);

    let content_stream = content.finish();
    let stream = writer.stream(content_id, &content_stream);
    stream.finish();

    let mut page = writer.page(page_id);
    let mut page_resources = page.resources();
    ctx.deferrer.pop_context(&mut page_resources);
    page_resources.finish();

    page.media_box(ctx.get_media_box());
    page.parent(page_tree_id);
    page.contents(content_id);
    page.finish();

    let document_info_id = ctx.deferrer.alloc_ref();
    writer.document_info(document_info_id).producer(TextStr("svg2pdf"));

    writer.finish()
}
