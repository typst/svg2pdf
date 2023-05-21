use crate::context::Context;
use crate::transform::TransformExt;
use crate::write::render::Render;
use pdf_writer::types::{LineCapStyle, LineJoinStyle, ColorSpaceOperand};
use pdf_writer::{Content, PdfWriter};
use usvg::{FillRule, LineCap, LineJoin, Node, Paint, PathSegment, Visibility};
use crate::{RgbColor};

impl Render for usvg::Path {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
        if self.visibility != Visibility::Visible {
            return;
        }

        content.save_state();
        content.transform(self.transform.get_transform());

        content.set_fill_color_space(ColorSpaceOperand::DeviceRgb);
        content.set_stroke_color_space(ColorSpaceOperand::DeviceRgb);

        if let Some(stroke) = &self.stroke {
            content.set_line_width(stroke.width.get() as f32);
            content.set_miter_limit(stroke.miterlimit.get() as f32);

            match stroke.linecap {
                LineCap::Butt => content.set_line_cap(LineCapStyle::ButtCap),
                LineCap::Round => content.set_line_cap(LineCapStyle::RoundCap),
                LineCap::Square => {
                    content.set_line_cap(LineCapStyle::ProjectingSquareCap)
                }
            };

            match stroke.linejoin {
                LineJoin::Miter => content.set_line_join(LineJoinStyle::MiterJoin),
                LineJoin::Round => content.set_line_join(LineJoinStyle::RoundJoin),
                LineJoin::Bevel => content.set_line_join(LineJoinStyle::BevelJoin),
            };

            if let Some(dasharray) = &stroke.dasharray {
                content.set_dash_pattern(
                    dasharray.iter().map(|&x| x as f32),
                    stroke.dashoffset,
                );
            }

            match &stroke.paint {
                Paint::Color(c) => {
                    content.set_stroke_color(RgbColor::from(*c).to_array());
                }
                _ => todo!(),
            }
        }

        let paint = self.fill.as_ref().map(|fill| &fill.paint);

        match paint {
            Some(Paint::Color(c)) => {
                content.set_fill_color(RgbColor::from(*c).to_array());
            }
            _ => {}
        }

        draw_path(self.data.segments(), content);

        match (self.fill.as_ref().map(|f| f.rule), self.stroke.is_some()) {
            (Some(FillRule::NonZero), true) => content.fill_nonzero_and_stroke(),
            (Some(FillRule::EvenOdd), true) => content.fill_even_odd_and_stroke(),
            (Some(FillRule::NonZero), false) => content.fill_nonzero(),
            (Some(FillRule::EvenOdd), false) => content.fill_even_odd(),
            (_, true) => content.stroke(),
            (_, false) => content.end_path(),
        };

        content.restore_state();
    }
}

fn draw_path(path_data: impl Iterator<Item = PathSegment>, content: &mut Content) {
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
