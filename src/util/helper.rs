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

    /// Create a RGB array to use in PDF.
    pub fn as_array(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
}

impl From<usvg::Color> for RgbColor {
    fn from(color: usvg::Color) -> Self {
        Self::from_u8(color.red, color.green, color.blue)
    }
}

pub trait TransformExt {
    fn as_array(&self) -> [f32; 6];
}

impl TransformExt for usvg::Transform {
    fn as_array(&self) -> [f32; 6] {
        [
            self.a as f32,
            self.b as f32,
            self.c as f32,
            self.d as f32,
            self.e as f32,
            self.f as f32,
        ]
    }
}

pub trait NameExt {
    fn as_name(&self) -> Name;
}

impl NameExt for String {
    fn as_name(&self) -> Name {
        Name(self.as_bytes())
    }
}
