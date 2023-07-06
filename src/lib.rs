mod render;
mod util;

use crate::util::helper::{dpi_ratio, NameExt};
use pdf_writer::{Content, Finish, PdfWriter, Ref, TextStr};
use usvg::{Size, Transform, Tree};
use util::context::Context;

#[derive(Copy, Clone)]
pub struct Options {
    pub dpi: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self { dpi: 72.0 }
    }
}

fn pdf_base_transform(dpi: f32, size: Size) -> Transform {
    let dpi_transform =
        Transform::new_scale(dpi_ratio(dpi) as f64, dpi_ratio(dpi) as f64);
    let viewport_transform = Transform::new(1.0, 0.0, 0.0, -1.0, 0.0, size.height());

    let mut base_transform = dpi_transform;
    base_transform.append(&viewport_transform);
    base_transform
}

pub fn convert_tree(tree: &Tree, options: Options) -> Vec<u8> {
    let mut ctx =
        Context::new(tree, options, pdf_base_transform(options.dpi, tree.size), None);
    let mut writer = PdfWriter::new();

    let catalog_id = ctx.deferrer.alloc_ref();
    let page_tree_id = ctx.deferrer.alloc_ref();
    let page_id = ctx.deferrer.alloc_ref();
    let content_id = ctx.deferrer.alloc_ref();

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).count(1).kids([page_id]);

    // Generate main content
    ctx.deferrer.push();
    let tree_x_object = render::tree_to_x_object(tree, &mut writer, &mut ctx);
    let mut content = Content::new();
    content.x_object(tree_x_object.as_name());

    let content_stream = content.finish();
    let stream = writer.stream(content_id, &content_stream);
    stream.finish();

    let mut page = writer.page(page_id);
    let mut page_resources = page.resources();
    ctx.deferrer.pop(&mut page_resources);
    page_resources.finish();

    page.media_box(ctx.get_media_box());
    page.parent(page_tree_id);
    page.contents(content_id);
    page.finish();

    let document_info_id = ctx.deferrer.alloc_ref();
    writer.document_info(document_info_id).producer(TextStr("svg2pdf"));

    writer.finish()
}

pub fn convert_tree_into(
    tree: &Tree,
    options: Options,
    writer: &mut PdfWriter,
    start_ref: Ref,
) -> Ref {
    let mut ctx = Context::new(
        tree,
        options,
        pdf_base_transform(options.dpi, tree.size),
        Some(start_ref.get()),
    );

    let x_object_id = ctx.deferrer.alloc_ref();
    ctx.deferrer.push();

    let tree_x_object = render::tree_to_x_object(tree, writer, &mut ctx);
    let mut content = Content::new();
    content.x_object(tree_x_object.as_name());
    let content_stream = content.finish();
    let mut x_object = writer.form_xobject(x_object_id, &content_stream);
    x_object.bbox(ctx.get_media_box());
    x_object.matrix([
        1.0 / (ctx.get_media_box().x2 - ctx.get_media_box().x1),
        0.0,
        0.0,
        1.0 / (ctx.get_media_box().y2 - ctx.get_media_box().y1),
        0.0,
        0.0,
    ]);
    let mut resources = x_object.resources();
    ctx.deferrer.pop(&mut resources);

    ctx.deferrer.alloc_ref()
}
