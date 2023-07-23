use std::rc::Rc;

use pdf_writer::types::{PaintType, TilingType};
use pdf_writer::{Content, Filter, PdfWriter};
use usvg::utils::view_box_to_transform;
use usvg::{NodeKind, NonZeroRect, Size, Transform, Units};

use super::group;
use crate::util::context::Context;
use crate::util::helper::TransformExt;

/// Turn a pattern into a Pattern object. Returns the name (= the name in the `Resources` dictionary) of
/// the pattern
pub fn create(
    pattern: Rc<usvg::Pattern>,
    parent_bbox: &NonZeroRect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    matrix: Transform,
) -> Rc<String> {
    let pattern_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    // Content units object bounding box should only be used if no view box is declared.
    let use_content_units_object_bounding_box =
        pattern.content_units == Units::ObjectBoundingBox && pattern.view_box.is_none();

    let pattern_rect = if pattern.units == Units::ObjectBoundingBox
        || use_content_units_object_bounding_box
    {
        pattern.rect.bbox_transform(*parent_bbox)
    } else {
        pattern.rect
    };

    match *pattern.root.borrow() {
        NodeKind::Group(ref group) => {
            let pattern_matrix =
                matrix.pre_concat(pattern.transform).pre_concat(Transform::from_row(
                    1.0,
                    0.0,
                    0.0,
                    1.0,
                    pattern_rect.x(),
                    pattern_rect.y(),
                ));

            let mut pattern_content = Content::new();
            pattern_content.save_state();

            if use_content_units_object_bounding_box {
                // The x/y is already accounted for in the pattern matrix, so we only need to scale the height/width. Otherwise,
                // the x/y would be applied twice.
                pattern_content.transform(
                    Transform::from_scale(parent_bbox.width(), parent_bbox.height())
                        .as_array(),
                );
            }

            if let Some(view_box) = pattern.view_box {
                let pattern_transform = view_box_to_transform(
                    view_box.rect,
                    view_box.aspect,
                    Size::from_wh(pattern_rect.width(), pattern_rect.height()).unwrap(),
                );
                pattern_content.transform(pattern_transform.as_array());
            }

            group::render(
                &pattern.root,
                group,
                writer,
                &mut pattern_content,
                ctx,
                Transform::default(),
            );

            pattern_content.restore_state();

            let pattern_content_stream = ctx.finish_content(pattern_content);

            let mut tiling_pattern =
                writer.tiling_pattern(pattern_ref, &pattern_content_stream);

            if ctx.options.compress {
                tiling_pattern.filter(Filter::FlateDecode);
            }

            ctx.deferrer.pop(&mut tiling_pattern.resources());

            // We already account for the x/y of the pattern by appending it to the matrix above, so here we just need to take the height / width
            // in consideration
            let final_bbox = pdf_writer::Rect::new(
                0.0,
                0.0,
                pattern_rect.width(),
                pattern_rect.height(),
            );

            tiling_pattern
                .tiling_type(TilingType::ConstantSpacing)
                .paint_type(PaintType::Colored)
                .bbox(final_bbox)
                .matrix(pattern_matrix.as_array())
                .x_step(final_bbox.x2 - final_bbox.x1)
                .y_step(final_bbox.y2 - final_bbox.y1);

            ctx.deferrer.add_pattern(pattern_ref)
        }
        _ => unreachable!(),
    }
}
