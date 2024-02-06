use crate::render::Render;
use crate::util::context::Context;
use pdf_writer::{Chunk, Content};
use std::sync::Arc;
use tiny_skia::{Size, Transform};
use usvg::utils::view_box_to_transform;
use usvg::{AspectRatio, Group, ImageKind, Node, NonZeroRect, ViewBox, Visibility};

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
    let layer_bbox = group.filters_bounding_box()?;
    let group_bbox = group.bounding_box?;
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
        initial_transform,
        &mut pixmap.as_mut(),
    );

    let encoded_image = pixmap.encode_png().ok()?;

    let image = usvg::Image {
        id: "".to_string(),
        visibility: Visibility::Visible,
        view_box: ViewBox { rect: layer_bbox, aspect: AspectRatio::default() },
        rendering_mode: Default::default(),
        kind: ImageKind::PNG(Arc::new(encoded_image)),
        abs_transform: Default::default(),
        bounding_box: None,
    };

    let group_node = Node::Group(Box::new(Group {
        transform: Transform::from_scale(1.0, 1.0),
        children: vec![Node::Image(Box::new(image))],
        ..Group::default()
    }));

    group_node.render(chunk, content, ctx, accumulated_transform);

    Some(())
}
