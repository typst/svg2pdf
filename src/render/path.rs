use pdf_writer::types::ColorSpaceOperand::Pattern;
use pdf_writer::types::{ColorSpaceOperand, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Finish, PdfWriter};
use usvg::tiny_skia_path::PathSegment;
use usvg::{Fill, NonZeroRect, PaintOrder};
use usvg::{FillRule, LineCap, LineJoin, Paint, Visibility};
use usvg::{Stroke, Transform};

use super::{gradient, pattern};
use crate::util::context::Context;
use crate::util::helper::{ColorExt, NameExt, TransformExt, SRGB};

/// Render a path into a content stream.
pub fn render(
    path: &usvg::Path,
    parent_bbox: &NonZeroRect,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    let separate_fill_stroke = || {
        let mut stroked_path = path.clone();
        stroked_path.fill = None;
        let mut filled_path = path.clone();
        filled_path.stroke = None;
        (stroked_path, filled_path)
    };

    if path.paint_order == PaintOrder::FillAndStroke {
        if path.stroke.as_ref().is_some_and(|stroke| stroke.opacity.get() != 1.0) {
            // Chrome and Adobe Acrobat will clip the fill so that it is not visible under the
            // stroke. For SVG, it should be visible. In order to achieve consistent behaviour,
            // we draw the stroke and fill separately in such a case.
            let (stroked_path, filled_path) = separate_fill_stroke();
            render_impl(&filled_path, parent_bbox, writer, content, ctx, accumulated_transform);
            render_impl(&stroked_path, parent_bbox, writer, content, ctx, accumulated_transform);
        }   else {
            render_impl(path, parent_bbox, writer, content, ctx, accumulated_transform)
        }
    }   else {
        let (stroked_path, filled_path) = separate_fill_stroke();
        render_impl(&stroked_path, parent_bbox, writer, content, ctx, accumulated_transform);
        render_impl(&filled_path, parent_bbox, writer, content, ctx, accumulated_transform);
    }
}

fn render_impl(
    path: &usvg::Path,
    parent_bbox: &NonZeroRect,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if path.visibility != Visibility::Visible {
        return;
    }

    content.save_state();
    content.transform(path.transform.as_array());
    let accumulated_transform = accumulated_transform.pre_concat(path.transform);

    let stroke_opacity = path.stroke.as_ref().map(|s| s.opacity.get());
    let fill_opacity = path.fill.as_ref().map(|f| f.opacity.get());

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
        set_stroke(stroke, parent_bbox, content, writer, ctx, accumulated_transform);
    }

    if let Some(fill) = &path.fill {
        set_fill(fill, parent_bbox, content, writer, ctx, accumulated_transform);
    }

    draw_path(path.data.segments(), content);
    finish_path(path.stroke.as_ref(), path.fill.as_ref(), content);

    content.restore_state();
}

pub fn draw_path(path_data: impl Iterator<Item = PathSegment>, content: &mut Content) {
    // Taken from resvg
    fn calc(n1: f32, n2: f32) -> f32 {
        (n1 + n2 * 2.0) / 3.0
    }

    let mut p_prev = None;

    for operation in path_data {
        match operation {
            PathSegment::MoveTo(p) => {
                content.move_to(p.x, p.y);
                p_prev = Some(p);
            }
            PathSegment::LineTo(p) => {
                content.line_to(p.x, p.y);
                p_prev = Some(p);
            }
            PathSegment::QuadTo(p1, p2) => {
                // Since PDF doesn't support quad curves, we ned to convert them into
                // cubic
                let prev = p_prev.unwrap();
                content.cubic_to(
                    calc(prev.x, p1.x),
                    calc(prev.y, p1.y),
                    calc(p2.x, p1.x),
                    calc(p2.y, p1.y),
                    p2.x,
                    p2.y,
                );
                p_prev = Some(p2);
            }
            PathSegment::CubicTo(p1, p2, p3) => {
                content.cubic_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y);
                p_prev = Some(p3);
            }
            PathSegment::Close => {
                content.close_path();
            }
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
    parent_bbox: &NonZeroRect,
    content: &mut Content,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    content.set_line_width(stroke.width.get());
    content.set_miter_limit(stroke.miterlimit.get());

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
        content.set_dash_pattern(dasharray.iter().cloned(), stroke.dashoffset);
    }

    let paint = &stroke.paint;

    match paint {
        Paint::Color(c) => {
            content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));
            content.set_stroke_color(c.as_array());
        }
        Paint::Pattern(p) => {
            let pattern_name = pattern::create(
                p.clone(),
                parent_bbox,
                writer,
                ctx,
                accumulated_transform,
            );
            content.set_stroke_color_space(Pattern);
            content.set_stroke_pattern(None, pattern_name.as_name());
        }
        Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
            if let Some((pattern_name, mask)) =
                gradient::create(paint, parent_bbox, writer, ctx, &accumulated_transform)
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
    parent_bbox: &NonZeroRect,
    content: &mut Content,
    writer: &mut PdfWriter,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    let paint = &fill.paint;

    match paint {
        Paint::Color(c) => {
            content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
            content.set_fill_color(c.as_array());
        }
        Paint::Pattern(p) => {
            let pattern_name = pattern::create(
                p.clone(),
                parent_bbox,
                writer,
                ctx,
                accumulated_transform,
            );
            content.set_fill_color_space(Pattern);
            content.set_fill_pattern(None, pattern_name.as_name());
        }
        Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
            if let Some((pattern_name, mask)) =
                gradient::create(paint, parent_bbox, writer, ctx, &accumulated_transform)
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
