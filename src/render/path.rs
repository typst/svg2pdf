use crate::render::group::create_x_object;
use crate::util::helper::{ColorExt, NameExt, RectExt, TransformExt, SRGB};
use pdf_writer::types::ColorSpaceOperand::Pattern;
use pdf_writer::types::{
    ColorSpaceOperand, LineCapStyle, LineJoinStyle, PaintType, TilingType,
};
use pdf_writer::{Content, Finish, PdfWriter};

use std::rc::Rc;

use crate::util::context::Context;
use usvg::utils::view_box_to_transform;
use usvg::{Fill, NodeKind, Rect, Size, Transform, Units};
use usvg::{FillRule, LineCap, LineJoin, Paint, PathSegment, Visibility};
use usvg::{Node, Stroke};

pub(crate) fn render(
    path: &usvg::Path,
    node: &Node,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    if path.visibility != Visibility::Visible {
        return;
    }

    ctx.context_frame.push();
    ctx.context_frame.append_transform(&path.transform);

    content.save_state();
    content.transform(ctx.context_frame.full_transform().as_array());
    content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));

    let stroke_opacity = path.stroke.as_ref().map(|s| s.opacity.get() as f32);
    let fill_opacity = path.fill.as_ref().map(|f| f.opacity.get() as f32);

    if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
        let name = ctx.deferrer.add_opacity(stroke_opacity, fill_opacity);
        content.set_parameters(name.as_name());
    }

    if let Some(stroke) = &path.stroke {
        set_stroke(stroke, content);
    }

    if let Some(fill) = &path.fill {
        set_fill(fill, &node, content, writer, ctx);
    }

    draw_path(path.data.segments(), content);
    finish_path(path.stroke.as_ref(), path.fill.as_ref(), content);

    content.restore_state();
    ctx.context_frame.pop();
}

pub fn draw_path(path_data: impl Iterator<Item = PathSegment>, content: &mut Content) {
    for operation in path_data {
        match operation {
            PathSegment::MoveTo { x, y } => content.move_to(x as f32, y as f32),
            PathSegment::LineTo { x, y } => content.line_to(x as f32, y as f32),
            PathSegment::CurveTo { x1, y1, x2, y2, x, y } => content
                .cubic_to(x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32),
            PathSegment::ClosePath => content.close_path(),
        };
    }
}

fn finish_path(stroke: Option<&Stroke>, fill: Option<&Fill>, content: &mut Content) {
    match (stroke, fill.map(|f| f.rule)) {
        (Some(_), Some(FillRule::NonZero)) => content.fill_nonzero_and_stroke(),
        (Some(_), Some(FillRule::EvenOdd)) => content.fill_even_odd_and_stroke(),
        (None, Some(FillRule::NonZero)) => content.fill_nonzero(),
        (None, Some(FillRule::EvenOdd)) => content.fill_even_odd(),
        (Some(_), None) => content.stroke(),
        (None, None) => content.end_path(),
    };
}

fn set_stroke(stroke: &Stroke, content: &mut Content) {
    content.set_line_width(stroke.width.get() as f32);
    content.set_miter_limit(stroke.miterlimit.get() as f32);

    match stroke.linecap {
        LineCap::Butt => content.set_line_cap(LineCapStyle::ButtCap),
        LineCap::Round => content.set_line_cap(LineCapStyle::RoundCap),
        LineCap::Square => content.set_line_cap(LineCapStyle::ProjectingSquareCap),
    };

    match stroke.linejoin {
        LineJoin::Miter => content.set_line_join(LineJoinStyle::MiterJoin),
        LineJoin::Round => content.set_line_join(LineJoinStyle::RoundJoin),
        LineJoin::Bevel => content.set_line_join(LineJoinStyle::BevelJoin),
    };

    if let Some(dasharray) = &stroke.dasharray {
        content.set_dash_pattern(dasharray.iter().map(|&x| x as f32), stroke.dashoffset);
    }

    match &stroke.paint {
        Paint::Color(c) => {
            content.set_stroke_color(c.as_array());
        }
        Paint::Pattern(_) => todo!(),
        _ => {} //_ => todo!(),
    }
}

fn set_fill(
    fill: &Fill,
    parent: &Node,
    content: &mut Content,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) {
    let paint = &fill.paint;

    match paint {
        Paint::Color(c) => {
            content.set_fill_color(c.as_array());
        }
        Paint::Pattern(p) => {
            let pattern_name = create_pattern(p.clone(), parent, writer, ctx);
            content.set_fill_color_space(Pattern);
            content.set_fill_pattern(None, pattern_name.as_name());
        }
        _ => {}
    }
}

fn create_pattern(
    pattern: Rc<usvg::Pattern>,
    parent: &Node,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (pattern_name, pattern_id) = ctx.deferrer.add_pattern();
    ctx.deferrer.push();

    let parent_bbox = ctx.plain_bbox(parent);

    // Content units object bounding box should only be used if no viewbox is declared.
    let use_content_units_obb =
        pattern.content_units == Units::ObjectBoundingBox && pattern.view_box.is_none();

    let pattern_rect =
        if pattern.units == Units::ObjectBoundingBox || use_content_units_obb {
            pattern.rect.bbox_transform(parent_bbox)
        } else {
            pattern.rect
        };

    match *pattern.root.borrow() {
        NodeKind::Group(ref group) => {
            let mut pattern_matrix = ctx.context_frame.full_transform();
            pattern_matrix.append(&pattern.transform);
            // Make sure that the pattern moves accordingly when a different x/y value is set for the pattern
            pattern_matrix.append(&Transform::new(
                1.0,
                0.0,
                0.0,
                1.0,
                pattern_rect.x(),
                pattern_rect.y(),
            ));
            // All transformations up until now will be applied to the pattern by setting the matrix argument of the pattern,
            // so we create a completely new context frame here which doesn't contain any of the transformations up until now
            ctx.context_frame.push_new();

            if use_content_units_obb {
                // Again, the x/y is already accounted for in the pattern matrix, so we only need to scale the height/width. Otherwise,
                // the x/y would be applied twice.
                ctx.context_frame.append_transform(&Transform::new_scale(
                    parent_bbox.width(),
                    parent_bbox.height(),
                ));
            }

            if let Some(view_box) = pattern.view_box {
                ctx.context_frame.append_transform(&view_box_to_transform(
                    view_box.rect,
                    view_box.aspect,
                    Size::new(pattern_rect.width(), pattern_rect.height()).unwrap(),
                ));
            }

            let (x_object_name, _) = create_x_object(&pattern.root, group, writer, ctx);

            let mut pattern_content = Content::new();
            pattern_content.x_object(x_object_name.as_name());
            let pattern_content_stream = pattern_content.finish();

            let mut tiling_pattern =
                writer.tiling_pattern(pattern_id, &pattern_content_stream);

            let mut resources = tiling_pattern.resources();
            ctx.deferrer.pop(&mut resources);
            resources.finish();

            // We already account for the x/y of the pattern by appending it to the matrix above, so here we just need to take the height / width
            // in consideration
            let final_bbox = pdf_writer::Rect::new(
                0.0,
                0.0,
                pattern_rect.width() as f32,
                pattern_rect.height() as f32,
            );

            tiling_pattern
                .tiling_type(TilingType::ConstantSpacing)
                .paint_type(PaintType::Colored)
                .bbox(final_bbox)
                .matrix(pattern_matrix.as_array())
                .x_step(final_bbox.x2 - final_bbox.x1)
                .y_step(final_bbox.y2 - final_bbox.y1);

            ctx.context_frame.pop();
            pattern_name
        }
        _ => unreachable!(),
    }
}
