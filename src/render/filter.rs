use crate::render::Render;
use crate::util::context::Context;
use pdf_writer::{Chunk, Content};
use std::sync::Arc;
use tiny_skia::{Size, Transform};
use usvg::{AspectRatio, Group, ImageKind, Node, NonZeroRect, ViewBox, Visibility};

/// Render a group with filters as an image
pub fn render(
    group: &Group,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) -> Option<()> {
    let ts = Transform::from_scale(ctx.options.raster_scale, ctx.options.raster_scale);

    // We have somewhat of a chicken-and-egg problem here: We need to create a new empty
    // group and append the current node as a child, so that bounding boxes take
    // transformations of the original node into account. However, resvg only allows
    // calculation of bounding boxes from a tree, so we need to wrap it in a tree.
    // But we don't know the size of the tree yet, so we initialize it with some
    // dummy values in the beginning and then set the proper values afterwards.
    let (mut tree, bbox, scaled_bbox) = {
        let mut root = Group::default();
        let mut sub_root = Group { transform: ts, ..Default::default() };

        sub_root.children.push(Node::Group(Box::from(group.clone())));
        root.children.push(Node::Group(Box::from(sub_root)));
        let mut tree = usvg::Tree {
            size: Size::from_wh(1.0, 1.0).unwrap(),
            view_box: ViewBox {
                rect: NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap(),
                aspect: Default::default(),
            },
            root,
        };
        tree.calculate_bounding_boxes();

        let bbox = group.layer_bounding_box?.transform(group.transform)?;
        let scaled_bbox = tree.root.layer_bounding_box?;

        (tree, bbox, scaled_bbox)
    };

    // TODO: Add a check so that huge regions don't crash svg2pdf (see huge-region.svg test case)
    let pixmap_size = scaled_bbox.size();

    let mut pixmap = tiny_skia::Pixmap::new(
        pixmap_size.width().round() as u32,
        pixmap_size.height().round() as u32,
    )?;

    tree.size = pixmap_size;
    tree.view_box = ViewBox { rect: scaled_bbox, aspect: Default::default() };

    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

    let encoded_image = pixmap.encode_png().ok()?;
    let img_node = Node::Image(Box::from(usvg::Image {
        id: "".to_string(),
        visibility: Visibility::Visible,
        view_box: ViewBox { rect: bbox, aspect: AspectRatio::default() },
        rendering_mode: Default::default(),
        kind: ImageKind::PNG(Arc::new(encoded_image)),
        abs_transform: Default::default(),
        bounding_box: None,
    }));

    img_node.render(chunk, content, ctx, accumulated_transform);

    Some(())
}
