use crate::color::{RgbColor, SRGB};
use crate::util::{Context, NameExt, RenderContext, TransformExt};
use crate::write::group::create_x_object;
use pdf_writer::types::ColorSpaceOperand::Pattern;
use pdf_writer::types::{
    ColorSpaceOperand, LineCapStyle, LineJoinStyle, PaintType, TilingType,
};
use pdf_writer::{Content, Finish, PdfWriter};

use std::rc::Rc;

use usvg::Stroke;
use usvg::{Fill, NodeKind, Transform};
use usvg::{FillRule, LineCap, LineJoin, Paint, PathSegment, Visibility};

pub(crate) fn render(
    path: &usvg::Path,
    content: &mut Content,
    ctx: &mut Context,
    writer: &mut PdfWriter,
) {
    if path.visibility != Visibility::Visible {
        return;
    }

    content.save_state();

    ctx.context_frame.push();
    ctx.context_frame.append_transform(&path.transform);

    content.transform(ctx.context_frame.transform().get_transform());

    content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));

    let stroke_opacity = path.stroke.as_ref().map(|s| s.opacity.get() as f32);
    let fill_opacity = path.fill.as_ref().map(|f| f.opacity.get() as f32);

    if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
        let name = ctx.alloc_opacity(stroke_opacity, fill_opacity);
        content.set_parameters(name.as_name());
    }

    if let Some(stroke) = &path.stroke {
        set_stroke(stroke, content);
    }

    if let Some(fill) = &path.fill {
        set_fill(fill, content, writer, ctx);
    }

    draw_path(path.data.segments(), content);
    finish_path(path.stroke.as_ref(), path.fill.as_ref(), content);

    ctx.context_frame.pop();
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
        (Some(_), _) => content.stroke(),
        (None, _) => content.end_path(),
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
            content.set_stroke_color(RgbColor::from(*c).as_array());
        }
        Paint::Pattern(_) => todo!(),
        _ => {} //_ => todo!(),
    }
}

fn set_fill(
    fill: &Fill,
    content: &mut Content,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) {
    let paint = &fill.paint;

    match paint {
        Paint::Color(c) => {
            content.set_fill_color(RgbColor::from(*c).as_array());
        }
        Paint::Pattern(p) => {
            let pattern_name = create_pattern(p.clone(), writer, ctx);
            content.set_fill_color_space(Pattern);
            content.set_fill_pattern(None, pattern_name.as_name());
        }
        _ => {}
    }
}

fn create_pattern(
    pattern: Rc<usvg::Pattern>,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> String {
    let (pattern_name, pattern_id) = ctx.alloc_named_pattern();
    let old_transform = ctx.context_frame.transform();
    ctx.context_frame.push();
    ctx.context_frame.push();
    ctx.context_frame.set_transform(Transform::default());
    ctx.context_frame.append_transform(&pattern.transform);
    ctx.context_frame.set_render_context(RenderContext::Pattern);

    ctx.push_context();

    match *pattern.root.borrow() {
        NodeKind::Group(ref group) => {
            let (x_object_name, _) = create_x_object(group, &pattern.root, writer, ctx);

            let mut pattern_content = Content::new();
            pattern_content.x_object(x_object_name.as_name());
            let pattern_content_stream = pattern_content.finish();

            let mut tiling_pattern =
                writer.tiling_pattern(pattern_id, &pattern_content_stream);

            let mut resources = tiling_pattern.resources();
            ctx.pop_context(&mut resources);
            resources.finish();
            let final_bbox = ctx.usvg_rect_to_pdf_rect(&pattern.rect);

            tiling_pattern
                .tiling_type(TilingType::ConstantSpacing)
                .paint_type(PaintType::Colored)
                .bbox(final_bbox)
                .matrix(old_transform.get_transform())
                .x_step(final_bbox.x2 - final_bbox.x1)
                .y_step(final_bbox.y2 - final_bbox.y1);

            ctx.context_frame.pop();
            ctx.context_frame.pop();
            pattern_name
        }
        _ => unreachable!(),
    }
}
