use crate::render::image;
use crate::util::context::Context;
use pdf_writer::{Chunk, Content};
use std::sync::Arc;
use tiny_skia::{Rect, Size, Transform};
use usvg::{AspectRatio, Group, ImageKind, Node, ViewBox, Visibility};

/// Render a group with filters as an image
pub fn render(
    group: &Group,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
) -> Option<()> {
    // TODO: Add a check so that huge regions don't crash svg2pdf (see huge-region.svg test case)

    let layer_bbox = group.layer_bounding_box()?;
    let group_bbox = group.bounding_box().unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap());
    let initial_transform = Transform::from_translate(
        group_bbox.x() - layer_bbox.x(),
        group_bbox.y() - layer_bbox.y(),
    );
    let pixmap_size = Size::from_wh(
        layer_bbox.width() * ctx.options.raster_scale,
        layer_bbox.height() * ctx.options.raster_scale,
    )?;

    let mut pixmap = tiny_skia::Pixmap::new(
        pixmap_size.width().round() as u32,
        pixmap_size.height().round() as u32,
    )?;

    resvg::render_node(
        &Node::Group(Box::new(group.clone())),
        Transform::from_scale(ctx.options.raster_scale, ctx.options.raster_scale)
            .pre_concat(initial_transform)
            .pre_concat(group.abs_transform()),
        &mut pixmap.as_mut(),
    );

    pixmap.save_png("out.png");

    let encoded_image = pixmap.encode_png().ok()?;

    image::render(
        Visibility::Visible,
        &ImageKind::PNG(Arc::new(encoded_image)),
        ViewBox { rect: layer_bbox, aspect: AspectRatio::default() },
        chunk, content, ctx
    );

    Some(())
}
