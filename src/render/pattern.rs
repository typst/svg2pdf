use std::ops::Mul;
use std::rc::Rc;

use pdf_writer::{Content, Filter, PdfWriter};
use pdf_writer::types::{PaintType, TilingType};
use usvg::{NodeKind, NonZeroRect, Opacity, Size, Transform, Units};
use usvg::utils::view_box_to_transform;

use crate::util::context::Context;
use crate::util::helper::TransformExt;

use super::group;

/// Turn a pattern into a Pattern object. Returns the name (= the name in the `Resources` dictionary) of
/// the pattern
pub fn create(
    pattern: Rc<usvg::Pattern>,
    parent_bbox: &NonZeroRect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    matrix: Transform,
    initial_opacity: Option<Opacity>,
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

    if let Some(initial_opacity) = initial_opacity {
        if let NodeKind::Group(ref mut group) = *pattern.root.borrow_mut() {
            group.opacity = group.opacity.mul(initial_opacity);
        }
    }

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

            let mut content = Content::new();
            content.save_state();

            if use_content_units_object_bounding_box {
                // The x/y is already accounted for in the pattern matrix, so we only need to scale the height/width. Otherwise,
                // the x/y would be applied twice.
                content.transform(
                    Transform::from_scale(parent_bbox.width(), parent_bbox.height())
                        .to_pdf_transform(),
                );
            }

            if let Some(view_box) = pattern.view_box {
                let pattern_transform = view_box_to_transform(
                    view_box.rect,
                    view_box.aspect,
                    Size::from_wh(pattern_rect.width(), pattern_rect.height()).unwrap(),
                );
                content.transform(pattern_transform.to_pdf_transform());
            }

            group::render(
                &pattern.root,
                group,
                writer,
                &mut content,
                ctx,
                Transform::default(),
            );

            content.restore_state();

            let pattern_content_stream = ctx.finish_content(content);

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
                .matrix(pattern_matrix.to_pdf_transform())
                .x_step(final_bbox.x2 - final_bbox.x1)
                .y_step(final_bbox.y2 - final_bbox.y1);

            ctx.deferrer.add_pattern(pattern_ref)
        }
        _ => unreachable!(),
    }
}
