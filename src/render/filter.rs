use crate::render::Render;
use crate::util::context::Context;
use pdf_writer::{Chunk, Content};
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::{Size, Transform};
use usvg::{
    AspectRatio, BBox, Group, ImageKind, Node, NodeExt, NodeKind, NonZeroRect, Units,
    ViewBox, Visibility,
};

/// Render a group with filters as an image
pub fn render(
    node: &Node,
    filters: &Vec<Rc<usvg::filter::Filter>>,
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
    let mut tree = {
        let root =
            Node::new(NodeKind::Group(Group { transform: ts, ..Group::default() }));
        root.append(node.make_deep_copy());
        let mut tree = usvg::Tree {
            size: Size::from_wh(1.0, 1.0).unwrap(),
            view_box: ViewBox {
                rect: NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap(),
                aspect: Default::default(),
            },
            root,
        };
        tree.calculate_bounding_boxes();
        tree
    };

    let mut bbox = tree.root.bounding_box().map(BBox::from)?;

    // TODO: Add a check so that huge regions don't crash svg2pdf (see huge-region.svg test case)
    // Basic idea: We calculate the bounding box so that all filter effects are contained
    // by taking the filter rects into considerations.
    // In theory, this is not sufficient, as it is possible that a filter in a child
    // group is even bigger, and thus the bbox would have to be expanded even more. But
    // for the vast majority of SVGs, this shouldn't matter.
    // Also, this will only work reliably for groups that are not isolated (i.e. they are
    // written directly into the page stream instead of an XObject), the reason being that
    // otherwise, the bounding box of the surrounding XObject might not be big enough, since
    // calculating the bbox of a group does not take filters into account. If we ever have
    // a way of taking filters into consideration when calling tree.calculate_bounding_boxes,
    // we can fix that.
    for filter in filters {
        let filter_region = if filter.units == Units::UserSpaceOnUse {
            filter.rect
        } else {
            filter.rect.bbox_transform(bbox.to_non_zero_rect()?)
        };
        bbox = bbox.expand(filter_region)
    }

    let bbox_rect = bbox.to_non_zero_rect()?;

    let pixmap_size = Size::from_wh(
        bbox_rect.width() * ctx.options.raster_scale,
        bbox_rect.height() * ctx.options.raster_scale,
    )?;

    let mut pixmap = tiny_skia::Pixmap::new(
        pixmap_size.width().round() as u32,
        pixmap_size.height().round() as u32,
    )?;

    tree.size = pixmap_size;
    tree.view_box = ViewBox {
        rect: bbox_rect.transform(ts)?,
        aspect: Default::default(),
    };

    let rtree = resvg::Tree::from_usvg(&tree);
    rtree.render(Transform::default(), &mut pixmap.as_mut());

    let encoded_image = pixmap.encode_png().ok()?;
    let img_node = Node::new(NodeKind::Image(usvg::Image {
        id: "".to_string(),
        visibility: Visibility::Visible,
        view_box: ViewBox { rect: bbox_rect, aspect: AspectRatio::default() },
        rendering_mode: Default::default(),
        kind: ImageKind::PNG(Arc::new(encoded_image)),
        abs_transform: Default::default(),
        bounding_box: None,
    }));

    img_node.render(chunk, content, ctx, accumulated_transform);

    Some(())
}
