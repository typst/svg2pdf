use pdf_writer::Name;

pub const SRGB: Name = Name(b"srgb");

/// A color helper function that stores colors with values between 0.0 and 1.0.
#[derive(Debug, Clone, Copy)]
pub struct RgbColor {
    /// Red.
    r: f32,
    /// Green.
    g: f32,
    /// Blue.
    b: f32,
}

impl RgbColor {
    /// Create a new color.
    pub(crate) fn new(r: f32, g: f32, b: f32) -> RgbColor {
        RgbColor { r, g, b }
    }

    /// Create a new color from u8 color components between 0.0 and 255.0.
    pub fn from_u8(r: u8, g: u8, b: u8) -> RgbColor {
        RgbColor::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    /// Create a RGB array for use in PDF.
    pub fn to_array(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
}

impl From<usvg::Color> for RgbColor {
    fn from(color: usvg::Color) -> Self {
        Self::from_u8(color.red, color.green, color.blue)
    }
}
