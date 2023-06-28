use pdf_writer::{Name, Rect};
use usvg::{FuzzyEq, Node, NodeExt, NodeKind, PathBbox, PathData, Size, Transform};

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

pub trait NameExt {
    fn as_name(&self) -> Name;
}

impl NameExt for String {
    fn as_name(&self) -> Name {
        Name(self.as_bytes())
    }
}

pub trait RectExt {
    fn as_pdf_rect(&self, base_transform: &Transform) -> Rect;
}

impl RectExt for usvg::Rect {
    fn as_pdf_rect(&self, transform: &Transform) -> Rect {
        let transformed = self.transform(transform).unwrap();
        Rect::new(
            transformed.x() as f32,
            transformed.y() as f32,
            (transformed.x() + transformed.width()) as f32,
            (transformed.y() + transformed.height()) as f32,
        )
    }
}

// Taken from resvg
pub(crate) fn calc_node_bbox(node: &Node, ts: Transform) -> Option<PathBbox> {
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

// Taken from resvg
/// Calculates an image rect depending on the provided view box.
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
