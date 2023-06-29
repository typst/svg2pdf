use crate::render::{path, tree_to_stream};
use crate::util::context::Context;
use crate::util::helper::{image_rect, NameExt, RectExt, TransformExt};

use image::{GenericImageView, ImageFormat, ImageOutputFormat};
use miniz_oxide::deflate::{compress_to_vec_zlib, CompressionLevel};
use pdf_writer::{Content, Filter, Finish, PdfWriter};
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

    let soft_mask_id = ctx.deferrer.alloc_ref();

    let (image_size, image_buffer, filter, alpha_mask) = match &image.kind {
        ImageKind::JPEG(content) => {
            let result = prepare_image(
                content.as_slice(),
                ImageFormat::Jpeg,
                ImageOutputFormat::Jpeg(100),
            );
            (result.0, result.1, Filter::DctDecode, None)
        }
        ImageKind::PNG(content) => {
            // We flip the image vertically because when applying the PDF base transformation the y axis will be flipped,
            // so we need to undo that
            let image = image::load_from_memory_with_format(content, ImageFormat::Png)
                .unwrap()
                .flipv();

            let compression_level = CompressionLevel::DefaultLevel as u8;
            let encoded_buffer =
                compress_to_vec_zlib(image.to_rgb8().as_raw(), compression_level);

            let mask = image.color().has_alpha().then(|| {
                let alphas: Vec<_> = image.pixels().map(|p| (p.2).0[3]).collect();
                compress_to_vec_zlib(&alphas, compression_level)
            });

            let image_size =
                Size::new(image.width() as f64, image.height() as f64).unwrap();
            (image_size, encoded_buffer, Filter::FlateDecode, mask)
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

    let (image_name, image_id) = ctx.deferrer.add_x_object();

    let mut image_x_object = writer.image_xobject(image_id, &image_buffer);
    image_x_object.filter(filter);
    image_x_object.width(image_size.width() as i32);
    image_x_object.height(image_size.height() as i32);
    image_x_object.color_space().device_rgb();
    image_x_object.bits_per_component(8);
    if alpha_mask.is_some() {
        image_x_object.s_mask(soft_mask_id);
    }
    image_x_object.finish();

    if let Some(encoded) = &alpha_mask {
        let mut s_mask = writer.image_xobject(soft_mask_id, encoded);
        s_mask.filter(filter);
        s_mask.width(image_size.width() as i32);
        s_mask.height(image_size.height() as i32);
        s_mask.color_space().device_gray();
        s_mask.bits_per_component(8);
    }

    ctx.context_frame.append_transform(&image.transform);
    let image_rect = image_rect(&image.view_box, image_size);

    let soft_mask = clip_outer(image.view_box.rect, writer, ctx);

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

fn prepare_image(
    content: &[u8],
    input_format: ImageFormat,
    output_format: ImageOutputFormat,
) -> (Size, Vec<u8>) {
    // We flip the image vertically because when applying the PDF base transformation the y axis will be flipped,
    // so we need to undo that
    let image = image::load_from_memory_with_format(content, input_format)
        .unwrap()
        .flipv();
    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = Cursor::new(&mut buffer);
    image.write_to(&mut writer, output_format).unwrap();

    let image_size = Size::new(image.width() as f64, image.height() as f64).unwrap();
    (image_size, buffer)
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
