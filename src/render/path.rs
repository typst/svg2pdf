use pdf_writer::types::ColorSpaceOperand;
use pdf_writer::types::ColorSpaceOperand::Pattern;
use pdf_writer::{Chunk, Content, Finish};
use usvg::tiny_skia_path::PathSegment;
use usvg::{Fill, FillRule, Node, Opacity, Paint, PaintOrder};
use usvg::{Path, Visibility};
use usvg::{Stroke, Transform};

use super::{gradient, pattern};
use crate::util::context::Context;
use crate::util::defer::SRGB;
use crate::util::helper::{
    bbox_to_non_zero_rect, ColorExt, LineCapExt, LineJoinExt, NameExt,
};

/// Render a path into a content stream.
pub fn render(
    node: &Node,
    path: &Path,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if path.visibility != Visibility::Visible {
        return;
    }

    // In order to support different stroke and fill orders as well as "advanced" paths
    // such as pattern fills/strokes with opacities and linear gradient strokes/fills with
    // stop opacities, we always render strokes and fills separately, at the cost of slightly
    // higher file sizes depending on the SVG.
    match path.paint_order {
        PaintOrder::FillAndStroke => {
            fill(path, node, chunk, content, ctx, accumulated_transform);
            stroke(path, node, chunk, content, ctx, accumulated_transform);
        }
        PaintOrder::StrokeAndFill => {
            stroke(path, node, chunk, content, ctx, accumulated_transform);
            fill(path, node, chunk, content, ctx, accumulated_transform);
        }
    }
}

/// Draws a path into a content stream. Note that this does not perform any stroking/filling,
/// it only creates a subpath.
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
                // cubic.
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

fn stroke(
    path: &Path,
    node: &Node,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if path.data.bounds().width() == 0.0 && path.data.bounds().height() == 0.0 {
        return;
    }

    if let Some(stroke) = path.stroke.as_ref() {
        let paint = &stroke.paint;
        let path_bbox = bbox_to_non_zero_rect(node.bounding_box());

        content.save_state();

        match paint {
            Paint::Color(c) => {
                set_opacity_gs(chunk, content, ctx, Some(stroke.opacity), None);
                content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));
                content.set_stroke_color(c.to_pdf_color());
            }
            Paint::Pattern(p) => {
                // Instead of setting the opacity via an external graphics state, we to it
                // by passing the opacity on to the pattern. The reason is that, for example
                // if we use a pattern as a stroke and set a stroke-opacity of 0.5, when rendering
                // the pattern, the opacity would only apply to strokes in that pattern, instead of
                // the whole pattern itself. This is why we need to handle this case differently.
                let pattern_name = pattern::create(
                    p.clone(),
                    &path_bbox,
                    chunk,
                    ctx,
                    accumulated_transform,
                    Some(stroke.opacity),
                );
                content.set_stroke_color_space(Pattern);
                content.set_stroke_pattern(None, pattern_name.to_pdf_name());
            }
            Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
                // In XPDF, the opacity will only be applied to the gradient if we also set the
                // fill opacity. Unfortunately, in muPDF it still doesn't work.
                set_opacity_gs(
                    chunk,
                    content,
                    ctx,
                    Some(stroke.opacity),
                    Some(stroke.opacity),
                );

                if let Some(soft_mask) =
                    gradient::create_shading_soft_mask(paint, &path_bbox, chunk, ctx)
                {
                    content.set_parameters(soft_mask.to_pdf_name());
                }

                let pattern_name = gradient::create_shading_pattern(
                    paint,
                    &path_bbox,
                    chunk,
                    ctx,
                    &accumulated_transform,
                );
                content.set_stroke_color_space(Pattern);
                content.set_stroke_pattern(None, pattern_name.to_pdf_name());
            }
        }

        content.set_line_width(stroke.width.get());
        content.set_miter_limit(stroke.miterlimit.get());
        content.set_line_cap(stroke.linecap.to_pdf_line_cap());
        content.set_line_join(stroke.linejoin.to_pdf_line_join());

        if let Some(dasharray) = &stroke.dasharray {
            content.set_dash_pattern(dasharray.iter().cloned(), stroke.dashoffset);
        } else {
            content.set_dash_pattern(vec![], 0.0);
        }

        draw_path(path.data.segments(), content);
        finish_path(Some(stroke), None, content);
        content.restore_state();
    }
}

fn fill(
    path: &Path,
    node: &Node,
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    accumulated_transform: Transform,
) {
    if path.data.bounds().width() == 0.0 || path.data.bounds().height() == 0.0 {
        return;
    }

    if let Some(fill) = path.fill.as_ref() {
        let paint = &fill.paint;
        let path_bbox = bbox_to_non_zero_rect(node.bounding_box());

        content.save_state();

        match paint {
            Paint::Color(c) => {
                set_opacity_gs(chunk, content, ctx, None, Some(fill.opacity));
                content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
                content.set_fill_color(c.to_pdf_color());
            }
            Paint::Pattern(p) => {
                // See note in the `stroke` function.
                let pattern_name = pattern::create(
                    p.clone(),
                    &path_bbox,
                    chunk,
                    ctx,
                    accumulated_transform,
                    Some(fill.opacity),
                );
                content.set_fill_color_space(Pattern);
                content.set_fill_pattern(None, pattern_name.to_pdf_name());
            }
            Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
                set_opacity_gs(chunk, content, ctx, None, Some(fill.opacity));

                if let Some(soft_mask) =
                    gradient::create_shading_soft_mask(paint, &path_bbox, chunk, ctx)
                {
                    content.set_parameters(soft_mask.to_pdf_name());
                };

                let pattern_name = gradient::create_shading_pattern(
                    paint,
                    &path_bbox,
                    chunk,
                    ctx,
                    &accumulated_transform,
                );
                content.set_fill_color_space(Pattern);
                content.set_fill_pattern(None, pattern_name.to_pdf_name());
            }
        }

        draw_path(path.data.segments(), content);
        finish_path(None, Some(fill), content);
        content.restore_state();
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

fn set_opacity_gs(
    chunk: &mut Chunk,
    content: &mut Content,
    ctx: &mut Context,
    stroke_opacity: Option<Opacity>,
    fill_opacity: Option<Opacity>,
) {
    let fill_opacity = fill_opacity.unwrap_or(Opacity::ONE).get();
    let stroke_opacity = stroke_opacity.unwrap_or(Opacity::ONE).get();

    if fill_opacity == 1.0 && stroke_opacity == 1.0 {
        return;
    }

    let gs_ref = ctx.alloc_ref();
    let mut gs = chunk.ext_graphics(gs_ref);
    gs.non_stroking_alpha(fill_opacity)
        .stroking_alpha(stroke_opacity)
        .finish();
    content.set_parameters(ctx.deferrer.add_graphics_state(gs_ref).to_pdf_name());
}
