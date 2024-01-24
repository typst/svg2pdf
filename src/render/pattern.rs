use std::cell::RefCell;
use std::ops::Mul;
use std::rc::Rc;

use pdf_writer::types::{PaintType, TilingType};
use pdf_writer::{Chunk, Content, Filter};
use usvg::utils::view_box_to_transform;
use usvg::{NonZeroRect, Opacity, Pattern, Size, Transform, Units};

use super::group;
use crate::util::context::Context;
use crate::util::helper::TransformExt;

/// Turn a pattern into a tiling pattern. Returns the name (= the name in the `Resources` dictionary) of
/// the pattern
pub fn create(
    pattern: Rc<RefCell<Pattern>>,
    parent_bbox: &NonZeroRect,
    chunk: &mut Chunk,
    ctx: &mut Context,
    matrix: Transform,
    initial_opacity: Option<Opacity>,
) -> Rc<String> {
    let mut pattern = pattern.borrow_mut();
    let pattern_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    // Content units object bounding box should only be used if no view box is declared.
    let content_units_obb =
        pattern.content_units == Units::ObjectBoundingBox && pattern.view_box.is_none();

    let pattern_rect = if pattern.units == Units::ObjectBoundingBox || content_units_obb {
        pattern.rect.bbox_transform(*parent_bbox)
    } else {
        pattern.rect
    };

    if let Some(initial_opacity) = initial_opacity {
        pattern.root.opacity = pattern.root.opacity.mul(initial_opacity);
    }

    let pattern_matrix = matrix.pre_concat(pattern.transform).pre_concat(
        Transform::from_row(1.0, 0.0, 0.0, 1.0, pattern_rect.x(), pattern_rect.y()),
    );

    let mut content = Content::new();
    content.save_state();

    if content_units_obb {
        // The x/y is already accounted for in the pattern matrix, so we only need to scale the height/width. Otherwise,
        // the x/y would be applied twice.
        content.transform(
            Transform::from_scale(parent_bbox.width(), parent_bbox.height())
                .to_pdf_transform(),
        );
    }

    if let Some(view_box) = pattern.view_box {
        let view_box_transform = view_box_to_transform(
            view_box.rect,
            view_box.aspect,
            Size::from_wh(pattern_rect.width(), pattern_rect.height()).unwrap(),
        );
        content.transform(view_box_transform.to_pdf_transform());
    }

    group::render(&pattern.root, chunk, &mut content, ctx, Transform::default());

    content.restore_state();

    let content_stream = ctx.finish_content(content);

    let mut tiling_pattern = chunk.tiling_pattern(pattern_ref, &content_stream);

    if ctx.options.compress {
        tiling_pattern.filter(Filter::FlateDecode);
    }

    ctx.deferrer.pop(&mut tiling_pattern.resources());

    // We already account for the x/y of the pattern by appending it to the matrix above, so here we just need to take the height / width
    // in consideration
    let final_bbox =
        pdf_writer::Rect::new(0.0, 0.0, pattern_rect.width(), pattern_rect.height());

    tiling_pattern
        .tiling_type(TilingType::ConstantSpacing)
        .paint_type(PaintType::Colored)
        .bbox(final_bbox)
        .matrix(pattern_matrix.to_pdf_transform())
        .x_step(final_bbox.x2 - final_bbox.x1)
        .y_step(final_bbox.y2 - final_bbox.y1);

    ctx.deferrer.add_pattern(pattern_ref)
}
