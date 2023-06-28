use crate::render::tree_to_stream;
use crate::util::context::Context;
use crate::util::helper::{image_rect, NameExt, TransformExt};

use pdf_writer::{Content, Filter, Finish, PdfWriter};
use std::io::Cursor;
use image::{ImageFormat, ImageOutputFormat};

use usvg::{ImageKind, Node, Size, Transform, Tree, Visibility};

pub(crate) fn render(
    _node: &Node,
    image: &usvg::Image,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    if image.visibility != Visibility::Visible {
        return;
    }

    let (image_size, image_buffer) = match &image.kind {
        ImageKind::JPEG(content) => {
            prepare_image(content.as_slice(), ImageFormat::Jpeg, ImageOutputFormat::Jpeg(100))
        },
        ImageKind::SVG(tree) => {
            render_svg(image, tree, writer, content, ctx);
            return;
        }
        _ => unimplemented!(),
    };

    ctx.context_frame.push();

    let (image_name, image_id) = ctx.deferrer.add_x_object();

    let mut image_x_object = writer.image_xobject(image_id, &image_buffer);
    image_x_object.filter(Filter::DctDecode);
    image_x_object.width(image_size.width() as i32);
    image_x_object.height(image_size.height() as i32);
    image_x_object.color_space().device_rgb();
    image_x_object.bits_per_component(8);
    image_x_object.finish();

    ctx.context_frame.append_transform(&image.transform);
    let image_rect = image_rect(
        &image.view_box,
        image_size,
    );

    ctx.context_frame
        .append_transform(&Transform::new_translate(image_rect.x(), image_rect.y()));
    ctx.context_frame
        .append_transform(&Transform::new_scale(image_rect.width(), image_rect.height()));
    content.save_state();
    content.transform(ctx.context_frame.full_transform().as_array());
    content.x_object(image_name.as_name());
    content.restore_state();

    ctx.context_frame.pop();
}

fn prepare_image(content: &[u8], input_format: ImageFormat, output_format: ImageOutputFormat) -> (Size, Vec<u8>) {
    // We flip the image vertically because when applying the PDF base transformation the y axis will be flipped,
    // so we need to undo that
    let image = image::load_from_memory_with_format(
        content,
        input_format,
    ).unwrap().flipv();
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = Cursor::new(&mut buffer);
    image
        .write_to(&mut writer, output_format)
        .unwrap();

    let image_size = Size::new(image.width() as f64, image.height() as f64).unwrap();
    (image_size, buffer)
}

fn render_svg(
    image: &usvg::Image,
    tree: &Tree,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context
) {
    ctx.context_frame.push();
    ctx.context_frame.append_transform(&image.transform);
    let image_rect = image_rect(&image.view_box, tree.size);
    // Account for the x/y shift of the image
    ctx.context_frame
        .append_transform(&Transform::new_translate(image_rect.x(), image_rect.y()));
    // Apply transformation so that the embedded svg has the same size as the image
    ctx.context_frame.append_transform(&Transform::new_scale(
        image_rect.width() / tree.size.width(),
        image_rect.height() / tree.size.height(),
    ));
    tree_to_stream(tree, writer, content, ctx);
    ctx.context_frame.pop();
}
