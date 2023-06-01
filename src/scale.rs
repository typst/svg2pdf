//! Provide transformations between PDF and SVG coordinate systems.

use pdf_writer::Rect;
use usvg::{Align, AspectRatio, NonZeroRect, Transform, ViewBox};
use crate::render::apply;

/// Convert point data between two coordinate systems.
#[derive(Debug, Copy, Clone)]
pub struct CoordToPdf {
    factor_x: f32,
    factor_y: f32,
    offset_x: f32,
    offset_y: f32,
    viewport: (f32, f32),
    dpi: f32,
    transform: Transform,
}

impl CoordToPdf {
    /// Create a new coordinate transform from the ViewBox of the SVG file to
    /// some viewport. A certain scaling mode can be forced by setting
    /// `aspect_ratio`.
    pub fn new(
        viewport: (f32, f32),
        dpi: f32,
        viewbox: ViewBox,
        aspect_ratio: Option<AspectRatio>,
    ) -> Self {
        let mut factor_x: f32;
        let mut factor_y: f32;
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        let original_ratio = viewbox.rect.width() / viewbox.rect.height();
        let viewport_ratio = viewport.0 / viewport.1;

        let aspect = if let Some(aspect) = aspect_ratio {
            if aspect.defer {
                viewbox.aspect
            } else {
                aspect
            }
        } else {
            viewbox.aspect
        };

        if aspect.slice == (original_ratio < viewport_ratio) {
            // Scale to fit width.
            factor_x = viewport.0 / viewbox.rect.width();
            factor_y = factor_x;
        } else {
            // Scale to fit height.
            factor_y = viewport.1 / viewbox.rect.height();
            factor_x = factor_y;
        }

        match aspect.align {
            Align::None => {
                factor_x = viewport.0 / viewbox.rect.width();
                factor_y = viewport.1 / viewbox.rect.height();
            }
            Align::XMinYMax => {}
            Align::XMidYMax => {
                offset_x = (viewport.0 - viewbox.rect.width() * factor_x) / 2.0;
            }
            Align::XMaxYMax => {
                offset_x = viewport.0 - viewbox.rect.width() * factor_x;
            }
            Align::XMinYMid => {
                offset_y = (viewport.1 - viewbox.rect.height() * factor_y) / 2.0;
            }
            Align::XMinYMin => {
                offset_y = viewport.1 - viewbox.rect.height() * factor_y;
            }
            Align::XMidYMid => {
                offset_x = (viewport.0 - viewbox.rect.width() * factor_x) / 2.0;
                offset_y = (viewport.1 - viewbox.rect.height() * factor_y) / 2.0;
            }
            Align::XMidYMin => {
                offset_x = (viewport.0 - viewbox.rect.width() * factor_x) / 2.0;
                offset_y = viewport.1 - viewbox.rect.height() * factor_y;
            }
            Align::XMaxYMid => {
                offset_x = viewport.0 - viewbox.rect.width() * factor_x;
                offset_y = (viewport.1 - viewbox.rect.height() * factor_y) / 2.0;
            }
            Align::XMaxYMin => {
                offset_x = viewport.0 - viewbox.rect.width() * factor_x;
                offset_y = viewport.1 - viewbox.rect.height() * factor_y;
            }
        }

        offset_x -= viewbox.rect.x() * factor_x;
        offset_y -= viewbox.rect.y() * factor_y;

        CoordToPdf {
            factor_x,
            factor_y,
            offset_x,
            offset_y,
            viewport,
            dpi,
            transform: Transform::identity(),
        }
    }

    /// Convert from SVG source coordinates to PDF coordinates.
    pub fn point(&self, point: (f32, f32)) -> (f32, f32) {
        self.point_raw(self.apply(point))
    }

    /// Convert from SVG source coordinates to PDF coordinates, disregarding transforms.
    pub fn point_raw(&self, point: (f32, f32)) -> (f32, f32) {
        let (x, y) = point;
        (
            self.px_to_pt(x * self.factor_x + self.offset_x),
            self.px_to_pt(self.viewport.1 - (y * self.factor_y + self.offset_y)),
        )
    }

    /// Get the PDF bounding box for the SVG file.
    pub fn bbox(&self) -> Rect {
        Rect::new(
            0.0,
            0.0,
            self.px_to_pt(self.viewport.0),
            self.px_to_pt(self.viewport.1),
        )
    }

    /// Convert from pixels to PDF points, disregarding any offsets or
    /// axis-specific scales.
    pub fn px_to_pt(&self, px: f32) -> f32 {
        px * 72.0 / self.dpi
    }

    /// Get the offset from the X axis.
    pub fn offset_x(&self) -> f32 {
        self.offset_x
    }

    /// Get the offset from the Y axis.
    pub fn offset_y(&self) -> f32 {
        self.offset_y
    }

    /// Get the factor for the X axis.
    pub fn factor_x(&self) -> f32 {
        self.factor_x
    }

    /// Get the factor for the Y axis.
    pub fn factor_y(&self) -> f32 {
        self.factor_y
    }

    /// Get the Dots Per Inch.
    pub fn dpi(&self) -> f32 {
        self.dpi
    }

    /// Get the transformation for this converter but without accounting
    /// for either DPI or that the PDF coordinate system is flipped. This is
    /// useful for converting between two SVG coordinate systems.
    pub fn uncorrected_transformation(&self) -> Transform {
        Transform::from_row(
            self.factor_x,
            0.0,
            0.0,
            self.factor_y,
            self.offset_x,
            self.offset_y,
        )
    }

    /// Transform a rectangle from SVG to PDF formats.
    pub fn pdf_rect(&self, rect: NonZeroRect) -> pdf_writer::Rect {
        let (x1, y1) = self.point((rect.x(), rect.y() + rect.height()));
        let (x2, y2) = self.point((rect.x() + rect.width(), rect.y()));
        pdf_writer::Rect::new(x1, y1, x2, y2)
    }

    /// Apply a transformation to a point.
    fn apply(&self, point: (f32, f32)) -> (f32, f32) {
        apply(&self.transform, point)
    }

    /// Compute the scale (e.g. to adapt the stroke width).
    pub fn compute_scale(&self) -> f32 {
        let complete_transform = self.transform.post_concat(
            Transform::from_scale(self.factor_x, self.factor_y));

        let (x_scale, y_scale) = complete_transform.get_scale();

        if x_scale.is_finite() && y_scale.is_finite() {
            let scale = x_scale.max(y_scale);
            if scale > 0.0 {
                return scale;
            }
        }

        1.0
    }

    /// Set a pre-transformation, overriding the old one.
    pub fn concat_transform(&mut self, add_transform: Transform) -> Transform {
        let old = self.transform;
        self.transform = self.transform.pre_concat(add_transform);
        old
    }

    pub fn set_transform(&mut self, transform: Transform) -> Transform {
        let old = self.transform;
        self.transform = transform;
        old
    }

    pub fn get_transform(&self) -> Transform {
        self.transform
    }
}
