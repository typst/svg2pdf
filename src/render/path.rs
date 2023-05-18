use crate::util::helper::{ColorExt, NameExt, TransformExt, SRGB};
use pdf_writer::types::ColorSpaceOperand::Pattern;
use pdf_writer::types::{ColorSpaceOperand, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Finish, PdfWriter};

use crate::render::{gradient, pattern};
use crate::util::context::Context;
use usvg::Fill;
use usvg::Stroke;
use usvg::{FillRule, LineCap, LineJoin, Paint, PathSegment, Visibility};

/// Render a path into a content stream.
pub fn render(
    path: &usvg::Path,
    parent_bbox: &usvg::Rect,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    if path.visibility != Visibility::Visible {
        return;
    }

    content.save_state();
    content.transform(path.transform.as_array());
    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));

    let stroke_opacity = path.stroke.as_ref().map(|s| s.opacity.get() as f32);
    let fill_opacity = path.fill.as_ref().map(|f| f.opacity.get() as f32);

    // Only create a graphics state if at least one of the opacities is not 1.
    if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
        let gs_ref = ctx.alloc_ref();
        let mut gs = writer.ext_graphics(gs_ref);
        gs.non_stroking_alpha(fill_opacity.unwrap_or(1.0))
            .stroking_alpha(stroke_opacity.unwrap_or(1.0))
            .finish();
        content.set_parameters(ctx.deferrer.add_graphics_state(gs_ref).as_name());
    }

    if let Some(stroke) = &path.stroke {
        set_stroke(stroke, parent_bbox, content, writer, ctx);
    }

    if let Some(fill) = &path.fill {
        set_fill(fill, parent_bbox, content, writer, ctx);
    }

    draw_path(path.data.segments(), content);
    finish_path(path.stroke.as_ref(), path.fill.as_ref(), content);
    content.restore_state();
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

fn set_stroke(
    stroke: &Stroke,
    parent_bbox: &usvg::Rect,
    content: &mut Content,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) {
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

    let paint = &stroke.paint;

    match paint {
        Paint::Color(c) => {
            content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));
            content.set_stroke_color(c.as_array());
        }
        Paint::Pattern(p) => {
            let pattern_name = pattern::create(p.clone(), parent_bbox, writer, ctx);
            content.set_stroke_color_space(Pattern);
            content.set_stroke_pattern(None, pattern_name.as_name());
        }
        Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
            if let Some((pattern_name, mask)) =
                gradient::create(paint, parent_bbox, writer, ctx)
            {
                // If the gradient contains stop with opacities, we need to write those separately
                // using a soft mask.
                if let Some(mask) = mask {
                    content.set_parameters(mask.as_name());
                }
                content.set_stroke_color_space(Pattern);
                content.set_stroke_pattern(None, pattern_name.as_name());
            }
        }
    }
}

fn set_fill(
    fill: &Fill,
    parent_bbox: &usvg::Rect,
    content: &mut Content,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) {
    let paint = &fill.paint;

    match paint {
        Paint::Color(c) => {
            content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
            content.set_fill_color(c.as_array());
        }
        Paint::Pattern(p) => {
            let pattern_name = pattern::create(p.clone(), parent_bbox, writer, ctx);
            content.set_fill_color_space(Pattern);
            content.set_fill_pattern(None, pattern_name.as_name());
        }
        Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
            if let Some((pattern_name, mask)) =
                gradient::create(paint, parent_bbox, writer, ctx)
            {
                if let Some(mask) = mask {
                    content.set_parameters(mask.as_name());
                }
                content.set_fill_color_space(Pattern);
                content.set_fill_pattern(None, pattern_name.as_name());
            }
        }
    }
}
