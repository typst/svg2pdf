use crate::render::Render;
use crate::util::context::Context;
use pdf_writer::{Chunk, Content};
use std::sync::Arc;
use tiny_skia::{Rect, Size, Transform};
use usvg::{AspectRatio, Group, ImageKind, Node, ViewBox, Visibility};

fn print_tree(group: &Group, level: u32) {
    println!("Level {}", level);
    println!("{:#?}", group);
    for child in &group.children {
        println!("{:#?}\n", child);

        if let Node::Group(ref g) = child {
            print_tree(g, level + 1);
        }
    }
}

/// Render a group with filters as an image
pub fn render(
    group: &Group,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) -> Option<()> {
    // TODO: Add a check so that huge regions don't crash svg2pdf (see huge-region.svg test case)

    let mut root_group = Group {
        children: vec![Node::Group(Box::new(group.clone()))],
        ..Group::default()
    };
    root_group.calculate_bounding_boxes();
    root_group.calculate_abs_transforms(Transform::default());

    let layer_bbox = root_group.layer_bounding_box?;
    let group_bbox = root_group.bounding_box.unwrap_or(Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap());
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
        &Node::Group(Box::new(root_group)),
        Transform::from_scale(ctx.options.raster_scale, ctx.options.raster_scale).pre_concat(initial_transform),
        &mut pixmap.as_mut(),
    );

    let encoded_image = pixmap.encode_png().ok()?;

    let image_node = Node::Image(Box::new(usvg::Image {
        id: "".to_string(),
        visibility: Visibility::Visible,
        view_box: ViewBox { rect: layer_bbox, aspect: AspectRatio::default() },
        rendering_mode: Default::default(),
        kind: ImageKind::PNG(Arc::new(encoded_image)),
        abs_transform: Default::default(),
        bounding_box: None,
    }));

    image_node.render(chunk, content, ctx, accumulated_transform);

    Some(())
}
