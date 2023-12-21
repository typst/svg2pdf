use std::cmp::max;
use std::rc::Rc;
use std::sync::Arc;

use pdf_writer::writers::Group;
use pdf_writer::{Chunk, Content, Filter, Finish};
use usvg::{
    AspectRatio, BBox, ImageKind, Node, NodeExt, NodeKind, NonZeroRect, Size, Transform,
    Tree, ViewBox, Visibility,
};

use super::{clip_path, mask, Render};
use crate::util::context::Context;
use crate::util::helper::{
    BlendModeExt, GroupExt, NameExt, NewNodeExt, RectExt, TransformExt,
};

/// Render a group into a content stream.
pub fn render(
    node: &Node,
    group: &usvg::Group,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if !group.filters.is_empty() {
        render_group_with_filters(node, chunk, content, ctx, accumulated_transform);
    } else if group.is_isolated() {
        content.save_state();
        let gs_ref = ctx.alloc_ref();
        let mut gs = chunk.ext_graphics(gs_ref);
        gs.non_stroking_alpha(group.opacity.get())
            .stroking_alpha(group.opacity.get())
            .blend_mode(group.blend_mode.to_pdf_blend_mode());

        gs.finish();
        content.set_parameters(ctx.deferrer.add_graphics_state(gs_ref).to_pdf_name());

        // We don't need to pass the accumulated transform here because if a pattern appears in a
        // XObject, it will be mapped to the coordinate space of where the XObject was invoked, meaning
        // that it will also be affected by the transforms in the content stream. If we passed on the
        // accumulated transform, they would be applied twice.
        content.x_object(
            create_x_object(node, group, chunk, ctx, Transform::default()).to_pdf_name(),
        );
        content.restore_state();
    } else {
        create_to_stream(node, group, chunk, content, ctx, accumulated_transform);
    }
}

fn render_group_with_filters(
    node: &Node,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if let Some(mut bbox) = node.stroke_bounding_box().and_then(|r| r.to_non_zero_rect())
    {
        let size = bbox
            .size()
            .to_int_size()
            .scale_by(ctx.options.raster_effects as f32)
            .unwrap();

        let ts = Transform::from_scale(
            ctx.options.raster_effects as f32,
            ctx.options.raster_effects as f32,
        );

        let mut pixmap =
            tiny_skia::Pixmap::new(max(1, size.width()), max(1, size.height())).unwrap();
        if let Some(rtree) = resvg::Tree::from_usvg_node(&node) {
            rtree.render(ts, &mut pixmap.as_mut());

            let encoded_image = pixmap.encode_png().unwrap();
            pixmap.save_png("./out.png").unwrap();
            let img_node = Node::new(NodeKind::Image(usvg::Image {
                id: "".to_string(),
                visibility: Visibility::Visible,
                view_box: ViewBox { rect: bbox, aspect: AspectRatio::default() },
                rendering_mode: Default::default(),
                kind: ImageKind::PNG(Arc::new(encoded_image)),
                abs_transform: Default::default(),
                bounding_box: None,
            }));

            img_node.render(chunk, content, ctx, accumulated_transform);
        }
    }
}

/// Turn a group into an XObject. Returns the name (= the name in the `Resources` dictionary) of
/// the group
fn create_x_object(
    node: &Node,
    group: &usvg::Group,
    chunk: &mut Chunk,
    ctx: &mut Context,
    accumulated_transform: Transform,
) -> Rc<String> {
    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let pdf_bbox = node
        .stroke_bbox_rect()
        .transform(group.transform)
        .unwrap()
        .to_pdf_rect();

    let mut content = Content::new();

    create_to_stream(node, group, chunk, &mut content, ctx, accumulated_transform);

    let content_stream = ctx.finish_content(content);

    let mut x_object = chunk.form_xobject(x_ref, &content_stream);
    ctx.deferrer.pop(&mut x_object.resources());

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    x_object
        .group()
        .transparency()
        .isolated(group.is_isolated())
        .knockout(false)
        .color_space()
        .icc_based(ctx.deferrer.srgb_ref());

    x_object.bbox(pdf_bbox);
    x_object.finish();

    ctx.deferrer.add_x_object(x_ref)
}

/// Write a group into a content stream. Opacities will be ignored. If opacities are needed,
/// you should use the `create` method instead.
fn create_to_stream(
    node: &Node,
    group: &usvg::Group,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    content.save_state();
    content.transform(group.transform.to_pdf_transform());
    let accumulated_transform = accumulated_transform.pre_concat(group.transform);

    if let Some(mask) = &group.mask {
        mask::render(node, mask.clone(), chunk, content, ctx);
    }

    if let Some(clip_path) = &group.clip_path {
        clip_path::render(node, clip_path.clone(), chunk, content, ctx);
    }

    for child in node.children() {
        child.render(chunk, content, ctx, accumulated_transform);
    }

    content.restore_state();
}
