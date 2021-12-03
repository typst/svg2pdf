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
    matrix: [f64; 6],
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
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    /// Convert from x SVG source coordinates to PDF coordinates.
    pub fn x(&self, x: f64) -> f32 {
        self.px_to_pt(self.apply_x(x) * self.factor_x + self.offset_x)
    }

    /// Convert from y SVG source coordinates to PDF coordinates.
    pub fn y(&self, y: f64) -> f32 {
        self.px_to_pt(self.height_y - (self.apply_y(y) * self.factor_y + self.offset_y))
    }

    /// Convert from SVG source coordinates to PDF coordinates.
    pub fn point(&self, point: (f64, f64)) -> (f32, f32) {
        let (x, y) = self.apply(point);
        (
            self.px_to_pt(x * self.factor_x + self.offset_x),
            self.px_to_pt(self.height_y - (y * self.factor_y + self.offset_y)),
        )
    }

    /// Convert from x PDF coordinates to SVG source coordinates.
    pub fn svg_x(&self, x: f32) -> f64 {
        (self.pt_to_px(x) - self.offset_x) / self.factor_x
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

    /// Get the transformation matrix for this converter.
    pub fn matrix(&self) -> [f32; 6] {
        let correct = (self.dpi / 72.0) as f32;
        [
            self.factor_x as f32 * correct,
            0.0,
            0.0,
            -self.factor_y as f32 * correct,
            self.offset_x as f32 * correct,
            (self.offset_y + self.height_y) as f32 * correct,
        ]
    }

    /// Get the transformation matrix for this converter but without accounting
    /// for either DPI or that the PDF coordinate system is flipped. This is
    /// useful for converting between two SVG coordinate systems.
    pub fn uncorrected_matrix(&self) -> [f64; 6] {
        [
            self.factor_x,
            0.0,
            0.0,
            self.factor_y,
            self.offset_x,
            self.offset_y,
        ]
    }

    /// Transform a rectangle from SVG to PDF formats.
    pub fn pdf_rect(&self, rect: usvg::Rect) -> pdf_writer::Rect {
        let (x1, y1) = self.point((rect.x(), rect.y() + rect.height()));
        let (x2, y2) = self.point((rect.x() + rect.width(), rect.y()));
        pdf_writer::Rect::new(x1, y1, x2, y2)
    }

    /// Apply a transformation matrix to a point.
    fn apply(&self, point: (f64, f64)) -> (f64, f64) {
        (
            point.0 * self.matrix[0] + point.1 * self.matrix[1] + self.matrix[4],
            point.0 * self.matrix[2] + point.1 * self.matrix[3] + self.matrix[5],
        )
    }

    /// Apply a transformation matrix to a point, this is okay for diagonal
    /// transformations.
    fn apply_x(&self, x: f64) -> f64 {
        x * self.matrix[0] + self.matrix[4]
    }

    /// Apply a transformation matrix to a point, this is okay for diagonal
    /// transformations.
    fn apply_y(&self, y: f64) -> f64 {
        y * self.matrix[3] + self.matrix[5]
    }

    /// Set a pre-transformation, overriding the old one.
    pub fn transform(&mut self, matrix: [f64; 6]) {
        self.matrix = matrix
    }

    /// Set the identity transform
    pub fn identity(&mut self) {
        self.matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    }
}
