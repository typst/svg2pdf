use pdf_writer::{Name, Rect};
use usvg::{FuzzyEq, Node, NodeExt, NodeKind, PathBbox, PathData, Size, Transform};

pub const SRGB: Name = Name(b"srgb");

/// Extension trait to convert [Colors](usvg::Color) into PDF colors.
pub trait ColorExt {
    fn as_array(&self) -> [f32; 3];
}

impl ColorExt for usvg::Color {
    fn as_array(&self) -> [f32; 3] {
        [self.red as f32 / 255.0, self.green as f32 / 255.0, self.blue as f32 / 255.0]
    }
}

/// Extension trait to convert a [Transform] into PDF transforms.
pub trait TransformExt {
    fn as_array(&self) -> [f32; 6];
}

impl TransformExt for Transform {
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

/// Extension trait to convert a [String] into a [Name]
pub trait NameExt {
    fn as_name(&self) -> Name;
}

impl NameExt for String {
    fn as_name(&self) -> Name {
        Name(self.as_bytes())
    }
}

/// Extension trait to turn a [`usvg` Rect](usvg::Rect) into a [PDF Rect](Rect)
pub trait RectExt {
    fn as_pdf_rect(&self) -> Rect;
}

impl RectExt for usvg::Rect {
    fn as_pdf_rect(&self) -> Rect {
        Rect::new(
            self.x() as f32,
            self.y() as f32,
            (self.x() + self.width()) as f32,
            (self.y() + self.height()) as f32,
        )
    }
}

// Taken from resvg
/// Calculate the rect of an image after it is scaled using a view box.
#[cfg(feature = "image")]
pub fn image_rect(view_box: &usvg::ViewBox, img_size: Size) -> usvg::Rect {
    let new_size = fit_view_box(img_size, view_box);
    let (x, y) = usvg::utils::aligned_pos(
        view_box.aspect.align,
        view_box.rect.x(),
        view_box.rect.y(),
        view_box.rect.width() - new_size.width(),
        view_box.rect.height() - new_size.height(),
    );

    new_size.to_rect(x, y)
}

// Taken from resvg
/// Calculate the new size of a view box that is the result of applying a view box
/// to a certain size.
#[cfg(feature = "image")]
pub fn fit_view_box(size: Size, vb: &usvg::ViewBox) -> usvg::Size {
    let s = vb.rect.size();

    if vb.aspect.align == usvg::Align::None {
        s
    } else if vb.aspect.slice {
        size.expand_to(s)
    } else {
        size.scale_to(s)
    }
}

/// Calculate the scale ratio of a DPI value.
pub fn dpi_ratio(dpi: f32) -> f32 {
    dpi / 72.0
}

/// Compress data using the deflate algorithm.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    const COMPRESSION_LEVEL: u8 = 6;
    miniz_oxide::deflate::compress_to_vec_zlib(data, COMPRESSION_LEVEL)
}

/// Calculate the bbox of a node as a [Rect](usvg::Rect).
pub fn plain_bbox(node: &Node) -> usvg::Rect {
    calc_node_bbox(node, Transform::default())
        .and_then(|b| b.to_rect())
        .unwrap_or(usvg::Rect::new(0.0, 0.0, 1.0, 1.0).unwrap())
}

// Taken from resvg
/// Calculate the bbox of a node with a given transform.
fn calc_node_bbox(node: &Node, ts: Transform) -> Option<PathBbox> {
    match *node.borrow() {
        NodeKind::Path(ref path) => {
            path.data.bbox_with_transform(ts, path.stroke.as_ref())
        }
        NodeKind::Image(ref img) => {
            let path = PathData::from_rect(img.view_box.rect);
            path.bbox_with_transform(ts, None)
        }
        NodeKind::Group(_) => {
            let mut bbox = PathBbox::new_bbox();

            for child in node.children() {
                let mut child_transform = ts;
                child_transform.append(&child.transform());
                if let Some(c_bbox) = calc_node_bbox(&child, child_transform) {
                    bbox = bbox.expand(c_bbox);
                }
            }

            // Make sure bbox was changed.
            if bbox.fuzzy_eq(&PathBbox::new_bbox()) {
                return None;
            }

            Some(bbox)
        }
        NodeKind::Text(_) => None,
    }
}
