use pdf_writer::Name;

pub const SRGB: Name = Name(b"srgb");

pub trait ColorExt {
    fn as_array(&self) -> [f32; 3];
}

impl ColorExt for usvg::Color {
    fn as_array(&self) -> [f32; 3] {
        [self.red as f32 / 255.0, self.green as f32 / 255.0, self.blue as f32 / 255.0]
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
