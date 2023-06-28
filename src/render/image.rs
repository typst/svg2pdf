use std::io::Cursor;
use image::imageops::FilterType;
use image::io::Reader;
use pdf_writer::{Content, Filter, Finish, PdfWriter, Rect};
use usvg::{ImageKind, Node, Size, Transform, Tree, Visibility};
use usvg::utils::view_box_to_transform;
use crate::render::tree_to_stream;
use crate::util::context::Context;
use crate::util::helper::{image_rect, NameExt, TransformExt};

pub(crate) fn render(
    node: &Node,
    image: &usvg::Image,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    if image.visibility != Visibility::Visible {
        return;
    }

    let (dynamic_image, image_data) = match &image.kind {
        ImageKind::JPEG(content) => {
            // We flip the image vertically because when applying the PDF base transformation the y axis will be flipped,
            // so we need to undo that
            let image = image::load_from_memory_with_format(content.as_slice(), image::ImageFormat::Jpeg).unwrap().flipv();
            let mut buffer: Vec<u8> = Vec::new();
            let mut writer = Cursor::new(&mut buffer);
            image.write_to(&mut writer, image::ImageOutputFormat::Jpeg(90)).unwrap();
            (image, buffer)
        }
        ImageKind::SVG(tree) => {
            render_svg(image, tree, writer, content, ctx);
            return;
        }
        _ => unimplemented!()
    };

    ctx.context_frame.push();

    let (image_name, image_id) = ctx.deferrer.add_x_object();

    let mut image_x_object = writer.image_xobject(image_id, &image_data);
    image_x_object.filter(Filter::DctDecode);
    image_x_object.width(dynamic_image.width() as i32);
    image_x_object.height(dynamic_image.height() as i32);
    image_x_object.color_space().device_rgb();
    image_x_object.bits_per_component(8);
    image_x_object.finish();

    ctx.context_frame.append_transform(&image.transform);
    let image_rect = image_rect(&image.view_box, Size::new(dynamic_image.width() as f64, dynamic_image.height() as f64).unwrap());

    ctx.context_frame.append_transform(&Transform::new_translate(image_rect.x(), image_rect.y()));
    ctx.context_frame.append_transform(&Transform::new_scale(image_rect.width(), image_rect.height()));
    content.save_state();
    content.transform(ctx.context_frame.full_transform().as_array());
    content.x_object(image_name.as_name());
    content.restore_state();

    ctx.context_frame.pop();

}

fn render_svg(image: &usvg::Image,
              tree: &Tree,
              writer: &mut PdfWriter,
              content: &mut Content,
              ctx: &mut Context) {
    ctx.context_frame.push();
    ctx.context_frame.append_transform(&image.transform);
    let image_rect = image_rect(&image.view_box, tree.size);
    // Account for the x/y shift of the image
    ctx.context_frame.append_transform(&Transform::new_translate(image_rect.x(), image_rect.y()));
    // Apply transformation so that the embedded svg has the same size as the image
    ctx.context_frame.append_transform(&Transform::new_scale(image_rect.width() / tree.size.width(), image_rect.height() / tree.size.height()));
    tree_to_stream(tree, writer, content, ctx);
    ctx.context_frame.pop();
}