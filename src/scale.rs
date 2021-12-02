//! Provide transformations between PDF and SVG coordinate systems.

use usvg::{Align, AspectRatio, ViewBox};

/// Convert point data between two coordinate systems.
#[derive(Debug, Copy, Clone)]
pub struct CoordToPdf {
    factor_x: f64,
    factor_y: f64,
    offset_x: f64,
    offset_y: f64,
    height_y: f64,
    dpi: f64,
}

impl CoordToPdf {
    /// Create a new coordinate transform from the ViewBox of the SVG file to
    /// some viewport. A certain scaling mode can be forced by setting
    /// `aspect_ratio`.
    pub fn new(
        viewport: (f64, f64),
        dpi: f64,
        viewbox: ViewBox,
        aspect_ratio: Option<AspectRatio>,
    ) -> Self {
        let mut factor_x: f64;
        let mut factor_y: f64;
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        let original_ratio = viewbox.rect.width() / viewbox.rect.height();
        let viewport_ratio = viewport.0 / viewport.1;

        let aspect = if let Some(aspect) = aspect_ratio {
            if aspect.defer { viewbox.aspect } else { aspect }
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
            height_y: viewport.1,
            dpi,
        }
    }

    /// Convert from x SVG source coordinates to PDF coordinates.
    pub fn x(&self, x: f64) -> f32 {
        self.px_to_pt(x * self.factor_x + self.offset_x)
    }

    /// Convert from x PDF coordinates to SVG source coordinates.
    pub fn svg_x(&self, x: f32) -> f64 {
        (self.pt_to_px(x) - self.offset_x) / self.factor_x
    }

    /// Convert from y SVG source coordinates to PDF coordinates.
    pub fn y(&self, y: f64) -> f32 {
        self.px_to_pt(self.height_y - (y * self.factor_y + self.offset_y))
    }

    /// Convert from y PDF coordinates to SVG source coordinates.
    pub fn svg_y(&self, y: f32) -> f64 {
        (self.pt_to_px(y) - self.offset_y) / self.factor_y
    }

    /// Convert from pixels to PDF points, disregarding any offsets or
    /// axis-specific scales.
    pub fn px_to_pt(&self, px: f64) -> f32 {
        (px * 72.0 / self.dpi) as f32
    }

    /// Convert from PDF points to pixels, disregarding any offsets or
    /// axis-specific scales.
    pub fn pt_to_px(&self, pt: f32) -> f64 {
        pt as f64 * self.dpi / 72.0
    }

    /// Get the offset from the X axis.
    pub fn offset_x(&self) -> f64 {
        self.offset_x
    }

    /// Get the offset from the Y axis.
    pub fn offset_y(&self) -> f64 {
        self.offset_y
    }

    /// Get the factor for the X axis.
    pub fn factor_x(&self) -> f64 {
        self.factor_x
    }

    /// Get the factor for the Y axis.
    pub fn factor_y(&self) -> f64 {
        self.factor_y
    }

    /// Get the Dots Per Inch.
    pub fn dpi(&self) -> f64 {
        self.dpi
    }
}
