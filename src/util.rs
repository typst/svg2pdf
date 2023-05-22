use pdf_writer::Ref;

pub trait TransformExt {
    fn get_transform(&self) -> [f32; 6];
}

impl TransformExt for usvg::Transform {
    fn get_transform(&self) -> [f32; 6] {
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

/// Just a wrapper struct so we don't need to always cast f64 to f32.
#[derive(Copy, Clone)]
pub struct Viewport((f32, f32, f32, f32));

impl Viewport {
    pub fn new(x:f32, y:f32, width: f32, height: f32) -> Self {
        Viewport((x, y, width, height))
    }

    pub fn x(&self) -> f32 {
        self.0.0
    }

    pub fn y(&self) -> f32 {
        self.0.1
    }

    pub fn width(&self) -> f32 {
        self.0.2
    }

    pub fn height(&self) -> f32 {
        self.0.3
    }
}

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

pub struct Context {
    next_id: i32,
    dpi: f32,
    pub viewport: Viewport,
}

impl Context {
    /// Create a new context.
    pub fn new(viewport: Viewport) -> Self {
        Self { next_id: 1, dpi: 72.0, viewport }
    }

    pub fn dpi_factor(&self) -> f32 {
        72.0 / self.dpi
    }

    /// Allocate a new indirect reference id.
    pub fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_id);
        self.next_id += 1;
        reference
    }
}
