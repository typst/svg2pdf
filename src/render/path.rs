use pdf_writer::{Content, Finish, PdfWriter};
use usvg::{Fill, FillRule, Node, Opacity, Paint, PaintOrder};
use usvg::{Path, Visibility};
use usvg::{Stroke, Transform};
use usvg::tiny_skia_path::PathSegment;

use crate::util::context::Context;
use crate::util::helper::{
    LineCapExt, LineJoinExt, NameExt, plain_bbox_without_default, TransformExt,
};

/// Render a path into a content stream.
pub fn render(
    node: &Node,
    path: &Path,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
    transform: Transform,
) {
    // Check if the path has a bbox at all.
    let Some(_) = plain_bbox_without_default(node, true) else { return; };

    if path.visibility != Visibility::Visible {
        return;
    }

    let has_stroke_opacity =
        path.stroke.as_ref().is_some_and(|stroke| stroke.opacity.get() != 1.0);

    let is_complex_stroke = |stroke: &Stroke| match &stroke.paint {
        Paint::Pattern(_) => stroke.opacity != 1.0,
        Paint::RadialGradient(rg) => rg.stops.iter().any(|stop| stop.opacity.get() < 1.0),
        Paint::LinearGradient(lg) => lg.stops.iter().any(|stop| stop.opacity.get() < 1.0),
        Paint::Color(_) => false,
    };

    let is_complex_fill = |fill: &Fill| match &fill.paint {
        Paint::Pattern(_) => fill.opacity != 1.0,
        Paint::RadialGradient(rg) => rg.stops.iter().any(|stop| stop.opacity.get() < 1.0),
        Paint::LinearGradient(lg) => lg.stops.iter().any(|stop| stop.opacity.get() < 1.0),
        Paint::Color(_) => false,
    };

    let is_complex_path = |path: &Path| {
        path.stroke.as_ref().is_some_and(is_complex_stroke)
            || path.fill.as_ref().is_some_and(is_complex_fill)
    };

    match (path.paint_order, has_stroke_opacity, is_complex_path(path)) {
        (PaintOrder::FillAndStroke, false, false) => {
            simple_path::render(
                path,
                node,
                writer,
                content,
                ctx,
                transform,
            );
        }
        // Chrome and Adobe Acrobat will clip the fill so that it is not visible under the
        // stroke, even if it has an opacity. However, in SVG, it should be visible. In order to achieve
        // consistent behaviour, we draw the stroke and fill separately if there is a stroke
        // opacity.
        (PaintOrder::FillAndStroke, true, _)
        | (PaintOrder::FillAndStroke, _, true) => {
            complex_path::render_fill(
                path,
                node,
                writer,
                content,
                ctx,
                transform,
            );

            complex_path::render_stroke(
                path,
                node,
                writer,
                content,
                ctx,
                transform,
            );
        }
        (PaintOrder::StrokeAndFill, _, _) => {
            complex_path::render_stroke(
                path,
                node,
                writer,
                content,
                ctx,
                transform,
            );

            complex_path::render_fill(
                path,
                node,
                writer,
                content,
                ctx,
                transform,
            );
        }
    }
}

mod simple_path {
    use pdf_writer::{Content, PdfWriter};
    use pdf_writer::types::ColorSpaceOperand;
    use pdf_writer::types::ColorSpaceOperand::Pattern;
    use usvg::{Fill, Node, NonZeroRect, Paint, Stroke, Transform};

    use crate::render::{gradient, pattern};
    use crate::render::path::{draw_path, finish_path, set_opacity_gs, set_stroke_properties};
    use crate::util::context::Context;
    use crate::util::helper::{ColorExt, NameExt, plain_bbox, SRGB, TransformExt};

    pub fn render(
        path: &usvg::Path,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        let stroke = path.stroke.as_ref();
        let fill = path.fill.as_ref();
        let path_bbox = plain_bbox(node, false);

        content.save_state();
        content.transform(path.transform.to_pdf_transform());
        let accumulated_transform = accumulated_transform.pre_concat(path.transform);
        draw_path(path.data.segments(), content);

        set_opacity_gs(
            writer,
            content,
            ctx,
            stroke.map(|stroke| stroke.opacity),
            fill.map(|fill| fill.opacity),
        );
        if let Some(stroke) = stroke {
            set_stroke(stroke, &path_bbox, content, writer, ctx, accumulated_transform);
        }

        if let Some(fill) = fill {
            set_fill(fill, &path_bbox, content, writer, ctx, accumulated_transform);
        }

        finish_path(stroke, fill, content);
        content.restore_state();
    }

    fn set_stroke(
        stroke: &Stroke,
        path_bbox: &NonZeroRect,
        content: &mut Content,
        writer: &mut PdfWriter,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        set_stroke_properties(content, stroke);

        let paint = &stroke.paint;

        match paint {
            Paint::Color(c) => {
                content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));
                content.set_stroke_color(c.to_pdf_color());
            }
            Paint::Pattern(p) => {
                let pattern_name = pattern::create(
                    p.clone(),
                    path_bbox,
                    writer,
                    ctx,
                    accumulated_transform,
                    None,
                );
                content.set_stroke_color_space(Pattern);
                content.set_stroke_pattern(None, pattern_name.to_pdf_name());
            }
            Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
                let pattern_name = gradient::create_shading_pattern(
                    paint,
                    path_bbox,
                    writer,
                    ctx,
                    &accumulated_transform,
                );
                content.set_stroke_color_space(Pattern);
                content.set_stroke_pattern(None, pattern_name.to_pdf_name());
            }
        }
    }

    fn set_fill(
        fill: &Fill,
        path_bbox: &NonZeroRect,
        content: &mut Content,
        writer: &mut PdfWriter,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        let paint = &fill.paint;

        match paint {
            Paint::Color(c) => {
                content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
                content.set_fill_color(c.to_pdf_color());
            }
            Paint::Pattern(p) => {
                let pattern_name = pattern::create(
                    p.clone(),
                    path_bbox,
                    writer,
                    ctx,
                    accumulated_transform,
                    None,
                );
                content.set_fill_color_space(Pattern);
                content.set_fill_pattern(None, pattern_name.to_pdf_name());
            }
            Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
                let pattern_name = gradient::create_shading_pattern(
                    paint,
                    path_bbox,
                    writer,
                    ctx,
                    &accumulated_transform,
                );
                content.set_fill_color_space(Pattern);
                content.set_fill_pattern(None, pattern_name.to_pdf_name());
            }
        }
    }
}

mod complex_path {
    use pdf_writer::{Content, Finish, PdfWriter};
    use pdf_writer::types::ColorSpaceOperand;
    use pdf_writer::types::ColorSpaceOperand::Pattern;
    use usvg::{Node, Paint, Path, Transform};

    use crate::render::{gradient, pattern};
    use crate::render::path::{draw_path, finish_path, set_opacity_gs, set_stroke_properties};
    use crate::util::context::Context;
    use crate::util::helper::{ColorExt, NameExt, plain_bbox, SRGB, TransformExt};

    pub fn render_stroke(
        path: &Path,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        if let Some(stroke) = path.stroke.as_ref() {
            let paint = &stroke.paint;
            let path_bbox = plain_bbox(node, true);

            content.save_state();
            content.transform(path.transform.to_pdf_transform());
            draw_path(path.data.segments(), content);
            let accumulated_transform = accumulated_transform.pre_concat(path.transform);

            set_stroke_properties(content, stroke);

            match paint {
                Paint::Color(c) => {
                    set_opacity_gs(writer, content, ctx, Some(stroke.opacity), None);
                    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));
                    content.set_stroke_color(c.to_pdf_color());
                }
                Paint::Pattern(p) => {
                    let pattern_name = pattern::create(
                        p.clone(),
                        &path_bbox,
                        writer,
                        ctx,
                        accumulated_transform,
                        Some(stroke.opacity),
                    );
                    content.set_stroke_color_space(Pattern);
                    content.set_stroke_pattern(None, pattern_name.to_pdf_name());
                }
                Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
                    let stroke_opacity = stroke.opacity.get();

                    // Only create a graphics state if at least one of the opacities is not 1.
                    if stroke_opacity != 1.0 {
                        let gs_ref = ctx.alloc_ref();
                        let mut gs = writer.ext_graphics(gs_ref);
                        gs.stroking_alpha(stroke_opacity).finish();
                        content.set_parameters(
                            ctx.deferrer.add_graphics_state(gs_ref).to_pdf_name(),
                        );
                    }

                    let soft_mask_name = gradient::create_shading_soft_mask(
                        paint, &path_bbox, writer, ctx,
                    );
                    let pattern_name = gradient::create_shading_pattern(
                        paint,
                        &path_bbox,
                        writer,
                        ctx,
                        &accumulated_transform,
                    );
                    content.set_parameters(soft_mask_name.to_pdf_name());
                    content.set_stroke_color_space(Pattern);
                    content.set_stroke_pattern(None, pattern_name.to_pdf_name());
                }
            }

            finish_path(Some(stroke), None, content);
            content.restore_state();
        }
    }

    pub fn render_fill(
        path: &Path,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
        accumulated_transform: Transform,
    ) {
        if let Some(fill) = path.fill.as_ref() {
            let paint = &fill.paint;
            let path_bbox = plain_bbox(node, false);

            content.save_state();
            content.transform(path.transform.to_pdf_transform());
            draw_path(path.data.segments(), content);
            let accumulated_transform = accumulated_transform.pre_concat(path.transform);

            match paint {
                Paint::Color(c) => {
                    set_opacity_gs(writer, content, ctx, None, Some(fill.opacity));
                    content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
                    content.set_fill_color(c.to_pdf_color());
                }
                Paint::Pattern(p) => {
                    let pattern_name = pattern::create(
                        p.clone(),
                        &path_bbox,
                        writer,
                        ctx,
                        accumulated_transform,
                        Some(fill.opacity),
                    );
                    content.set_fill_color_space(Pattern);
                    content.set_fill_pattern(None, pattern_name.to_pdf_name());
                }
                Paint::LinearGradient(_) | Paint::RadialGradient(_) => {
                    let fill_opacity = fill.opacity.get();

                    // Only create a graphics state if at least one of the opacities is not 1.
                    if fill_opacity != 1.0 {
                        let gs_ref = ctx.alloc_ref();
                        let mut gs = writer.ext_graphics(gs_ref);
                        gs.non_stroking_alpha(fill_opacity).finish();
                        content.set_parameters(
                            ctx.deferrer.add_graphics_state(gs_ref).to_pdf_name(),
                        );
                    }

                    let soft_mask_name = gradient::create_shading_soft_mask(
                        paint, &path_bbox, writer, ctx,
                    );
                    let pattern_name = gradient::create_shading_pattern(
                        paint,
                        &path_bbox,
                        writer,
                        ctx,
                        &accumulated_transform,
                    );
                    content.set_parameters(soft_mask_name.to_pdf_name());
                    content.set_fill_color_space(Pattern);
                    content.set_fill_pattern(None, pattern_name.to_pdf_name());
                }
            }

            finish_path(None, Some(fill), content);
            content.restore_state();
        }
    }
}

pub fn draw_path(path_data: impl Iterator<Item=PathSegment>, content: &mut Content) {
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

fn set_stroke_properties(content: &mut Content, stroke: &Stroke) {
    content.set_line_width(stroke.width.get());
    content.set_miter_limit(stroke.miterlimit.get());

    content.set_line_cap(stroke.linecap.to_pdf_line_cap());
    content.set_line_join(stroke.linejoin.to_pdf_line_join());

    if let Some(dasharray) = &stroke.dasharray {
        content.set_dash_pattern(dasharray.iter().cloned(), stroke.dashoffset);
    }
}

fn set_opacity_gs(
    writer: &mut PdfWriter,
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
    let mut gs = writer.ext_graphics(gs_ref);
    gs.non_stroking_alpha(fill_opacity)
        .stroking_alpha(stroke_opacity)
        .finish();
    content.set_parameters(ctx.deferrer.add_graphics_state(gs_ref).to_pdf_name());
}
