use std::rc::Rc;

use image::{ColorType, DynamicImage, ImageFormat, Luma, Rgb, Rgba};
use miniz_oxide::deflate::{compress_to_vec_zlib, CompressionLevel};
use pdf_writer::{Chunk, Content, Filter, Finish};
use usvg::{ImageKind, Size, Transform, Tree, Visibility};

use crate::util::context::Context;
use crate::util::helper;
use crate::util::helper::{image_rect, NameExt, TransformExt};
use crate::{convert_tree_into, Options};

/// Render an image into a content stream.
pub fn render(
    image: &usvg::Image,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
) {
    if image.visibility != Visibility::Visible {
        return;
    }

    // Will return the name of the image (in the Resources dictionary) and the dimensions of the
    // actual image (i.e. the actual image size, not the size in the PDF, which will always be 1x1
    // because that's how ImageXObjects are scaled by default.
    let (image_name, image_size) = match &image.kind {
        ImageKind::JPEG(content) => {
            let dynamic_image =
                image::load_from_memory_with_format(content, ImageFormat::Jpeg).unwrap();
            // JPEGs don't support alphas, so no extra processing is required.
            create_raster_image(
                chunk,
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
            // Alpha channels need to br written separately as a soft mask, hence the extra processing
            // step.
            let (samples, filter, alpha_mask) = handle_transparent_image(&dynamic_image);
            create_raster_image(
                chunk,
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
            // Alpha channels need to be written separately as a soft mask, hence the extra processing
            // step.
            let (samples, filter, alpha_mask) = handle_transparent_image(&dynamic_image);
            create_raster_image(
                chunk,
                ctx,
                &samples,
                filter,
                &dynamic_image,
                alpha_mask.as_deref(),
            )
        }
        // SVGs just get rendered recursively.
        ImageKind::SVG(tree) => create_svg_image(tree, chunk, ctx),
    };

    // Get the dimensions of the actual rect that is needed to scale the image into the image view
    // box. If the keepAspectRatio is slice, this rect will exceed the actual image view box, but
    // it will be clipped further below so that it always stays within the bounds of the actual image
    // rect.
    let image_rect = image_rect(&image.view_box, image_size);

    content.save_state();
    // Clip the image so just the part inside of the view box is actually visible.
    helper::clip_to_rect(image.view_box.rect, content);

    // Account for the x/y of the viewbox.
    content.transform(
        Transform::from_translate(image_rect.x(), image_rect.y()).to_pdf_transform(),
    );
    // Scale the image from 1x1 to the actual dimensions.
    content.transform(
        Transform::from_row(
            image_rect.width(),
            0.0,
            0.0,
            -image_rect.height(),
            0.0,
            image_rect.height(),
        )
        .to_pdf_transform(),
    );
    content.x_object(image_name.to_pdf_name());
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
    chunk: &mut Chunk,
    ctx: &mut Context,
    samples: &[u8],
    filter: Filter,
    dynamic_image: &DynamicImage,
    alpha_mask: Option<&[u8]>,
) -> (Rc<String>, Size) {
    let color = dynamic_image.color();
    let alpha_mask = alpha_mask.map(|mask_bytes| {
        let soft_mask_id = ctx.alloc_ref();
        let mut s_mask = chunk.image_xobject(soft_mask_id, mask_bytes);
        s_mask.filter(filter);
        s_mask.width(dynamic_image.width() as i32);
        s_mask.height(dynamic_image.height() as i32);
        s_mask.color_space().device_gray();
        s_mask.bits_per_component(calculate_bits_per_component(color));
        soft_mask_id
    });

    let image_size =
        Size::from_wh(dynamic_image.width() as f32, dynamic_image.height() as f32)
            .unwrap();
    let image_ref = ctx.alloc_ref();
    let image_name = ctx.deferrer.add_x_object(image_ref);

    let mut image_x_object = chunk.image_xobject(image_ref, samples);
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
    chunk: &mut Chunk,
    ctx: &mut Context,
) -> (Rc<String>, Size) {
    let image_ref = ctx.alloc_ref();
    let image_name = ctx.deferrer.add_x_object(image_ref);
    // convert_tree_into will automatically scale it in a way so that its dimensions are 1x1, like
    // regular ImageXObjects. So afterwards, we can just treat them the same.
    let next_ref = convert_tree_into(tree, Options::default(), chunk, image_ref);
    ctx.deferrer.set_next_ref(next_ref.get());
    (image_name, tree.size)
}
