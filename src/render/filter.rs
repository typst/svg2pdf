use crate::render::Render;
use crate::util::context::Context;
use pdf_writer::{Chunk, Content};
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::{Size, Transform};
use usvg::{AspectRatio, BBox, Group, ImageKind, Node, NodeExt, NodeKind, Units, ViewBox, Visibility};

fn calculate_bounding_box(node: &Node) {
    if node.has_children() {
        let mut bbox = BBox::default();
        let mut stroke_bbox = BBox::default();
        for child in node.children() {
            calculate_bounding_box(&child);

            if let Some(mut c_bbox) = child.bounding_box() {
                if let NodeKind::Group(ref group) = *child.borrow() {
                    if let Some(r) = c_bbox.transform(group.transform) {
                        c_bbox = r;
                    }
                }

                bbox = bbox.expand(c_bbox);
            }

            if let Some(mut c_bbox) = child.stroke_bounding_box() {
                if let NodeKind::Group(ref group) = *child.borrow() {
                    if let Some(r) = c_bbox.transform(group.transform) {
                        c_bbox = r;
                    }
                }

                stroke_bbox = stroke_bbox.expand(c_bbox);
            }
        }

        if let NodeKind::Group(ref mut group) = *node.borrow_mut() {
            group.bounding_box = bbox.to_rect();
            group.stroke_bounding_box = stroke_bbox.to_rect();
        }
    }

    match *node.borrow_mut() {
        NodeKind::Path(ref mut path) => {
            path.bounding_box = path.data.compute_tight_bounds();
            path.stroke_bounding_box = path.calculate_stroke_bounding_box();
            if path.stroke_bounding_box.is_none() {
                path.stroke_bounding_box = path.bounding_box;
            }
        }
        // TODO: should we account for `preserveAspectRatio`?
        NodeKind::Image(ref mut image) => image.bounding_box = Some(image.view_box.rect),
        // Have to be handled separately to prevent multiple mutable reference to the tree.
        NodeKind::Group(_) => {}
        // Will be set only during text-to-path conversion.
        NodeKind::Text(_) => {}
    }

    // Yes, subroots are not affected by the node's transform.
    node.subroots(|root| calculate_bounding_box(&root));
}

pub fn render(
    node: &Node,
    filters: &Vec<Rc<usvg::filter::Filter>>,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) -> Option<()> {
    let new_node = Node::new(NodeKind::Group(Group::default()));
    new_node.append(node.make_deep_copy());
    calculate_bounding_box(&new_node);
    let bbox = new_node.bounding_box().map(BBox::from)?;

    // Basic idea: We calculate the bounding box so that all filter effects are contained.
    // Then, we create a new pixmap with that size (optionally bigger if raster effects are set
    // to a higher resolution). Then, we translate by the top/left to make sure that the whole
    // group is actually contained within the visible area of the pixmap. Finally, we render it
    // into an image and place the image into the PDF so that it is aligned correctly.
    let bbox_rect = bbox.to_non_zero_rect()?;
    let mut actual_bbox = bbox;

    // TODO: Add a check so that huge regions don't crash svg2pdf? (see huge-region.svg test case)

    // TODO: In theory, this is not sufficient, as it is possible that a filter in a child
    // group is even bigger, and thus the bbox would have to be expanded even more. But
    // for the vast majority of SVGs, this shouldn't matter.

    // Also, this will only work reliably for groups that are not isolated (i.e. they are
    // written directly into the page stream instead of an XObject), the reason being that
    // otherwise, the bounding box of the surrounding XObject might not be big enough, since
    // calculating the bbox of a group does not take filters into account.
    for filter in filters {
        let filter_region = if filter.units == Units::UserSpaceOnUse {
            filter.rect
        } else {
            filter.rect.bbox_transform(bbox.to_non_zero_rect()?)
        };
        actual_bbox = actual_bbox.expand(filter_region)
    }

    let actual_bbox_rect = actual_bbox.to_non_zero_rect()?;

    let (left_delta, top_delta) = (
        bbox_rect.left() - actual_bbox_rect.left(),
        bbox_rect.top() - actual_bbox_rect.top(),
    );

    let pixmap_size = Size::from_wh(
        actual_bbox_rect.width() * ctx.options.raster_effects,
        actual_bbox_rect.height() * ctx.options.raster_effects,
    )?;

    let ts =
        Transform::from_scale(ctx.options.raster_effects, ctx.options.raster_effects)
            .pre_translate(left_delta, top_delta);

    let mut pixmap = tiny_skia::Pixmap::new(
        pixmap_size.width().round() as u32,
        pixmap_size.height().round() as u32,
    )?;

    if let Some(rtree) = resvg::Tree::from_usvg_node(&new_node) {
        rtree.render(ts, &mut pixmap.as_mut());

        let encoded_image = pixmap.encode_png().ok()?;

        let img_node = Node::new(NodeKind::Image(usvg::Image {
            id: "".to_string(),
            visibility: Visibility::Visible,
            view_box: ViewBox {
                rect: actual_bbox_rect,
                aspect: AspectRatio::default(),
            },
            rendering_mode: Default::default(),
            kind: ImageKind::PNG(Arc::new(encoded_image)),
            abs_transform: Default::default(),
            bounding_box: None,
        }));

        img_node.render(chunk, content, ctx, accumulated_transform);
    }

    Some(())
}
