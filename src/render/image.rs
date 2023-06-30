use crate::render::{path, tree_to_stream};
use crate::util::context::Context;
use crate::util::helper::{image_rect, NameExt, RectExt, TransformExt};

use image::{ColorType, DynamicImage, GenericImageView, ImageFormat, ImageOutputFormat};
use miniz_oxide::deflate::{compress_to_vec_zlib, CompressionLevel};
use pdf_writer::{Content, Filter, Finish, PdfWriter, Ref};
use std::io::Cursor;
use std::rc::Rc;

use usvg::{
    Color, Fill, ImageKind, Node, Paint, PathData, Size, Transform, Tree, Visibility,
};
use crate::render::group::make_transparency_group;

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

    let (dynamic_image, samples, filter, alpha_mask) = match &image.kind {
        ImageKind::JPEG(content) => {
            let image = prepare_image(content, ImageFormat::Jpeg);
            let samples = image_to_samples(&image, ImageOutputFormat::Jpeg(100));
            (image, samples, Filter::DctDecode, None)
        }
        ImageKind::PNG(content) => {
            // We flip the image vertically because when applying the PDF base transformation the y axis will be flipped,
            // so we need to undo that
            let image = prepare_image(content, ImageFormat::Png);

            let map_alpha = |image: &DynamicImage| image.pixels().map(|p| (p.2).0[3]).collect();
            let (encoded_image, encoded_mask) = match &image {
                DynamicImage::ImageRgb8(_) => (image.to_rgb8().as_raw(), None),
                DynamicImage::ImageRgba8(_) => (image.to_rgb8().as_raw(), Some(map_alpha(&image))),
                DynamicImage::ImageRgb16(_) => (image.to_rgb16().as_raw(), None),
                DynamicImage::ImageRgba16(_) => (image.to_rgb16().as_raw(), Some(map_alpha(&image))),
                DynamicImage::ImageLuma8(_) => (image.to_luma8().as_raw(), None),
                DynamicImage::ImageLumaA8(_) => (image.to_luma8().as_raw(), Some(map_alpha(&image))),
                DynamicImage::ImageLuma16(_) => (image.to_luma16().as_raw(), None),
                DynamicImage::ImageLumaA16(_) => (image.to_luma16().as_raw(), Some(map_alpha(&image))),
                DynamicImage::ImageRgb32F(_) => (image.to_rgb32f().as_raw(), None),
                DynamicImage::ImageRgba32F(_) => (image.to_rgb32f().as_raw(), Some(map_alpha(&image))),
                _ => return
            };

            let compression_level = CompressionLevel::DefaultLevel as u8;
            let compressed_image = compress_to_vec_zlib(encoded_image, compression_level);
            let compressed_mask = encoded_mask.map(|m| compress_to_vec_zlib(&m, compression_level));

            (image, compressed_image, Filter::FlateDecode, compressed_mask)
        }
        ImageKind::SVG(tree) => {
            render_svg(image, tree, writer, content, ctx);
            return;
        }
        _ => {
            return;
        }
    };

    ctx.context_frame.push();

    ctx.context_frame.append_transform(&image.transform);
    let image_size = Size::new(dynamic_image.width() as f64, dynamic_image.height() as f64).unwrap();
    let image_rect = image_rect(&image.view_box, image_size);

    let soft_mask = clip_outer(image.view_box.rect, writer, ctx);

    let image_name = create_image_x_object(writer, ctx, &samples, filter, &dynamic_image, alpha_mask.as_deref());

    content.save_state();
    content.set_parameters(soft_mask.as_name());
    ctx.context_frame
        .append_transform(&Transform::new_translate(image_rect.x(), image_rect.y()));
    ctx.context_frame
        .append_transform(&Transform::new_scale(image_rect.width(), image_rect.height()));
    content.transform(ctx.context_frame.full_transform().as_array());
    content.x_object(image_name.as_name());
    content.restore_state();

    ctx.context_frame.pop();
}

fn create_image_x_object(writer: &mut PdfWriter,
                        ctx: &mut Context,
                        samples: &[u8],
                         filter: Filter,
                         dynamic_image: &DynamicImage,
                         alpha_mask: Option<&[u8]>) -> String {
    let color = dynamic_image.color();
    let alpha_mask = alpha_mask.map(|mask_bytes| {
        let soft_mask_id = ctx.deferrer.alloc_ref();
        let mut s_mask = writer.image_xobject(soft_mask_id, mask_bytes);
        s_mask.filter(filter);
        s_mask.width(dynamic_image.width() as i32);
        s_mask.height(dynamic_image.height() as i32);
        s_mask.color_space().device_gray();
        s_mask.bits_per_component(calculate_bits_per_component(color));
        soft_mask_id
    });

    let (image_name, image_id) = ctx.deferrer.add_x_object();

    let mut image_x_object = writer.image_xobject(image_id, &samples);
    image_x_object.filter(filter);
    image_x_object.width(dynamic_image.width() as i32);
    image_x_object.height(dynamic_image.height() as i32);

    let color_space =image_x_object.color_space();
    if color.has_color() {
        color_space.device_rgb();
    }   else {
        color_space.device_gray();
    }

    image_x_object.bits_per_component(calculate_bits_per_component(color));
    if let Some(soft_mask_id) = alpha_mask {
        image_x_object.s_mask(soft_mask_id);
    }
    image_x_object.finish();
    image_name
}

fn calculate_bits_per_component(color_type: ColorType) -> i32 {
    (color_type.bits_per_pixel() / color_type.channel_count() as u16) as i32
}

fn image_to_samples(image: &DynamicImage, output_format: ImageOutputFormat) -> Vec<u8> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = Cursor::new(&mut buffer);
    image.write_to(&mut writer, output_format).unwrap();
    buffer
}

fn prepare_image(
    content: &[u8],
    input_format: ImageFormat
) -> DynamicImage {
    // We flip the image vertically because when applying the PDF base transformation the y axis will be flipped,
    // so we need to undo that
    image::load_from_memory_with_format(content, input_format)
        .unwrap()
        .flipv()
}

fn render_svg(
    image: &usvg::Image,
    tree: &Tree,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    ctx.context_frame.push();
    ctx.context_frame.append_transform(&image.transform);
    let soft_mask = clip_outer(image.view_box.rect, writer, ctx);
    content.save_state();
    content.set_parameters(soft_mask.as_name());
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
    content.restore_state();
    ctx.context_frame.pop();
}

fn clip_outer(rect: usvg::Rect, writer: &mut PdfWriter, ctx: &mut Context) -> String {
    let mask_reference = ctx.deferrer.alloc_ref();

    ctx.deferrer.push();
    let pdf_bbox = rect.as_pdf_rect(&ctx.context_frame.full_transform());

    let mut content = Content::new();
    content.save_state();

    let fill = Fill::from_paint(Paint::Color(Color::new_rgb(0, 0, 0)));
    let path = usvg::Path {
        fill: Some(fill),
        data: Rc::new(PathData::from_rect(rect)),
        ..Default::default()
    };

    path::render(
        &path,
        &usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap(),
        writer,
        &mut content,
        ctx,
    );

    content.restore_state();

    let content_stream = content.finish();
    let mut x_object = writer.form_xobject(mask_reference, &content_stream);

    ctx.deferrer.pop(&mut x_object.resources());

    make_transparency_group(&mut x_object);

    x_object.bbox(pdf_bbox);
    x_object.finish();

    ctx.deferrer.add_soft_mask(mask_reference)
}
