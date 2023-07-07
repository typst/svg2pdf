use crate::util::context::Context;
use crate::util::helper::{image_rect, NameExt, TransformExt};

use image::{ColorType, DynamicImage, ImageFormat, Luma, Rgb, Rgba};
use miniz_oxide::deflate::{compress_to_vec_zlib, CompressionLevel};
use pdf_writer::{Content, Filter, Finish, PdfWriter};

use crate::{convert_tree_into, Options};
use usvg::{ImageKind, Size, Transform, Tree, Visibility};

pub(crate) fn render(
    image: &usvg::Image,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    if image.visibility != Visibility::Visible {
        return;
    }

    let (image_name, image_size) = match &image.kind {
        ImageKind::JPEG(content) => {
            let dynamic_image =
                image::load_from_memory_with_format(content, ImageFormat::Jpeg).unwrap();
            create_raster_image(
                writer,
                ctx,
                content,
                Filter::DctDecode,
                &dynamic_image,
                None,
            )
        }
        ImageKind::PNG(content) => {
            let dynamic_image =
                image::load_from_memory_with_format(content, ImageFormat::Png).unwrap();
            let (samples, filter, alpha_mask) = handle_transparent_image(&dynamic_image);
            create_raster_image(
                writer,
                ctx,
                &samples,
                filter,
                &dynamic_image,
                alpha_mask.as_deref(),
            )
        }
        ImageKind::GIF(content) => {
            let dynamic_image =
                image::load_from_memory_with_format(content, ImageFormat::Gif).unwrap();
            let (samples, filter, alpha_mask) = handle_transparent_image(&dynamic_image);
            create_raster_image(
                writer,
                ctx,
                &samples,
                filter,
                &dynamic_image,
                alpha_mask.as_deref(),
            )
        }
        ImageKind::SVG(tree) => create_svg_image(tree, writer, ctx),
    };

    let image_rect = image_rect(&image.view_box, image_size);

    content.save_state();
    content.transform(image.transform.as_array());
    clip_image_to_rect(image.view_box.rect, content);
    content
        .transform(Transform::new_translate(image_rect.x(), image_rect.y()).as_array());
    content.transform(
        Transform::new(
            image_rect.width(),
            0.0,
            0.0,
            -image_rect.height(),
            0.0,
            image_rect.height(),
        )
        .as_array(),
    );
    content.x_object(image_name.as_name());
    content.restore_state();
}

fn handle_transparent_image(image: &DynamicImage) -> (Vec<u8>, Filter, Option<Vec<u8>>) {
    let color = image.color();
    let bits = color.bits_per_pixel();
    let channels = color.channel_count() as u16;

    let encoded_image: Vec<u8> = match (channels, bits / channels > 8) {
        (1 | 2, false) => image.to_luma8().pixels().flat_map(|&Luma(c)| c).collect(),
        (1 | 2, true) => image
            .to_luma16()
            .pixels()
            .flat_map(|&Luma(x)| x)
            .flat_map(|x| x.to_be_bytes())
            .collect(),
        (3 | 4, false) => image.to_rgb8().pixels().flat_map(|&Rgb(c)| c).collect(),
        (3 | 4, true) => image
            .to_rgb16()
            .pixels()
            .flat_map(|&Rgb(c)| c)
            .flat_map(|x| x.to_be_bytes())
            .collect(),
        _ => panic!("unknown number of channels={channels}"),
    };

    let encoded_mask: Option<Vec<u8>> = if color.has_alpha() {
        if bits / channels > 8 {
            Some(
                image
                    .to_rgba16()
                    .pixels()
                    .flat_map(|&Rgba([.., a])| a.to_be_bytes())
                    .collect(),
            )
        } else {
            Some(image.to_rgba8().pixels().map(|&Rgba([.., a])| a).collect())
        }
    } else {
        None
    };

    let compression_level = CompressionLevel::DefaultLevel as u8;
    let compressed_image = compress_to_vec_zlib(&encoded_image, compression_level);
    let compressed_mask =
        encoded_mask.map(|m| compress_to_vec_zlib(&m, compression_level));

    (compressed_image, Filter::FlateDecode, compressed_mask)
}

fn create_raster_image(
    writer: &mut PdfWriter,
    ctx: &mut Context,
    samples: &[u8],
    filter: Filter,
    dynamic_image: &DynamicImage,
    alpha_mask: Option<&[u8]>,
) -> (String, Size) {
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

    let image_size =
        Size::new(dynamic_image.width() as f64, dynamic_image.height() as f64).unwrap();
    let (image_name, image_id) = ctx.deferrer.add_x_object();

    let mut image_x_object = writer.image_xobject(image_id, samples);
    image_x_object.filter(filter);
    image_x_object.width(dynamic_image.width() as i32);
    image_x_object.height(dynamic_image.height() as i32);

    let color_space = image_x_object.color_space();
    if color.has_color() {
        color_space.device_rgb();
    } else {
        color_space.device_gray();
    }

    image_x_object.bits_per_component(calculate_bits_per_component(color));
    if let Some(soft_mask_id) = alpha_mask {
        image_x_object.s_mask(soft_mask_id);
    }
    image_x_object.finish();
    (image_name, image_size)
}

fn calculate_bits_per_component(color_type: ColorType) -> i32 {
    (color_type.bits_per_pixel() / color_type.channel_count() as u16) as i32
}

fn create_svg_image(
    tree: &Tree,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> (String, Size) {
    let (image_name, image_ref) = ctx.deferrer.add_x_object();
    let next_ref = convert_tree_into(tree, Options::default(), writer, image_ref);
    ctx.deferrer.set_next_ref(next_ref.get());
    (image_name, tree.size)
}

fn clip_image_to_rect(rect: usvg::Rect, content: &mut Content) {
    content.rect(
        rect.x() as f32,
        rect.y() as f32,
        rect.width() as f32,
        rect.height() as f32,
    );
    content.close_path();
    content.clip_nonzero();
    content.end_path();
}
