use crate::color::{RgbColor, SRGB};
use crate::util::{Context, TransformExt};
use pdf_writer::types::{ColorSpaceOperand, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Name};
use usvg::Fill;
use usvg::Stroke;
use usvg::{FillRule, LineCap, LineJoin, Paint, PathSegment, Visibility};

pub(crate) fn render(path: &usvg::Path, content: &mut Content, ctx: &mut Context) {
    if path.visibility != Visibility::Visible {
        return;
    }

    content.save_state();
    content.transform(path.transform.get_transform());

    content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));

    let stroke_opacity = path.stroke.as_ref().map(|s| s.opacity.get() as f32);
    let fill_opacity = path.fill.as_ref().map(|f| f.opacity.get() as f32);

    if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
        let name = ctx.alloc_opacity(stroke_opacity, fill_opacity);
        content.set_parameters(Name(name.as_bytes()));
    }

    if let Some(stroke) = &path.stroke {
        set_stroke(stroke, content);
    }

    if let Some(fill) = &path.fill {
        set_fill(fill, content);
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
            content.set_stroke_color(RgbColor::from(*c).to_array());
        }
        _ => {} //_ => todo!(),
    }
}

fn set_fill(fill: &Fill, content: &mut Content) {
    let paint = &fill.paint;

    match paint {
        Paint::Color(c) => {
            content.set_fill_color(RgbColor::from(*c).to_array());
        }
        _ => {}
    }
}
