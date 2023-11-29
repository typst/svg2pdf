use pdf_writer::types::{BlendMode, LineCapStyle, LineJoinStyle, MaskType};
use pdf_writer::{Content, Name, Rect};
use usvg::{
    BBox, LineCap, LineJoin, Node, NodeExt, NodeKind, NonZeroRect, Size, Transform,
};

use crate::render::gradient::Stop;

/// Extension trait to convert [Colors](usvg::Color) into PDF colors.
pub trait ColorExt {
    fn to_pdf_color(&self) -> [f32; 3];
}

impl ColorExt for usvg::Color {
    fn to_pdf_color(&self) -> [f32; 3] {
        [self.red as f32 / 255.0, self.green as f32 / 255.0, self.blue as f32 / 255.0]
    }
}

/// Extension trait to convert a [Transform] into PDF transforms.
pub trait TransformExt {
    fn to_pdf_transform(&self) -> [f32; 6];
}

impl TransformExt for Transform {
    fn to_pdf_transform(&self) -> [f32; 6] {
        [self.sx, self.ky, self.kx, self.sy, self.tx, self.ty]
    }
}

/// Extension trait to convert a [String] into a [Name]
pub trait NameExt {
    fn to_pdf_name(&self) -> Name;
}

impl NameExt for String {
    fn to_pdf_name(&self) -> Name {
        Name(self.as_bytes())
    }
}

/// Extension trait to turn a [`usvg` Rect](usvg::Rect) into a [PDF Rect](Rect)
pub trait RectExt {
    fn to_pdf_rect(&self) -> Rect;
}

impl RectExt for NonZeroRect {
    fn to_pdf_rect(&self) -> Rect {
        Rect::new(self.x(), self.y(), self.x() + self.width(), self.y() + self.height())
    }
}

/// Extension trait to turn a [`usvg` BlendMode](usvg::BlendMode) into a [PDF Blendmode](BlendMode)
pub trait BlendModeExt {
    fn to_pdf_blend_mode(&self) -> BlendMode;
}

impl BlendModeExt for usvg::BlendMode {
    fn to_pdf_blend_mode(&self) -> BlendMode {
        match self {
            usvg::BlendMode::Normal => BlendMode::Normal,
            usvg::BlendMode::Multiply => BlendMode::Multiply,
            usvg::BlendMode::Screen => BlendMode::Screen,
            usvg::BlendMode::Overlay => BlendMode::Overlay,
            usvg::BlendMode::Darken => BlendMode::Darken,
            usvg::BlendMode::Lighten => BlendMode::Lighten,
            usvg::BlendMode::ColorDodge => BlendMode::ColorDodge,
            usvg::BlendMode::ColorBurn => BlendMode::ColorBurn,
            usvg::BlendMode::HardLight => BlendMode::HardLight,
            usvg::BlendMode::SoftLight => BlendMode::SoftLight,
            usvg::BlendMode::Difference => BlendMode::Difference,
            usvg::BlendMode::Exclusion => BlendMode::Exclusion,
            usvg::BlendMode::Hue => BlendMode::Hue,
            usvg::BlendMode::Saturation => BlendMode::Saturation,
            usvg::BlendMode::Color => BlendMode::Color,
            usvg::BlendMode::Luminosity => BlendMode::Luminosity,
        }
    }
}

pub trait MaskTypeExt {
    fn to_pdf_mask_type(&self) -> MaskType;
}

impl MaskTypeExt for usvg::MaskType {
    fn to_pdf_mask_type(&self) -> MaskType {
        match self {
            usvg::MaskType::Alpha => MaskType::Alpha,
            usvg::MaskType::Luminance => MaskType::Luminosity,
        }
    }
}

pub trait LineCapExt {
    fn to_pdf_line_cap(&self) -> LineCapStyle;
}

impl LineCapExt for LineCap {
    fn to_pdf_line_cap(&self) -> LineCapStyle {
        match self {
            LineCap::Butt => LineCapStyle::ButtCap,
            LineCap::Round => LineCapStyle::RoundCap,
            LineCap::Square => LineCapStyle::ProjectingSquareCap,
        }
    }
}

pub trait LineJoinExt {
    fn to_pdf_line_join(&self) -> LineJoinStyle;
}

impl LineJoinExt for LineJoin {
    fn to_pdf_line_join(&self) -> LineJoinStyle {
        match self {
            LineJoin::Miter => LineJoinStyle::MiterJoin,
            //TODO: is it possible to implement this in PDF?
            LineJoin::MiterClip => LineJoinStyle::MiterJoin,
            LineJoin::Round => LineJoinStyle::RoundJoin,
            LineJoin::Bevel => LineJoinStyle::BevelJoin,
        }
    }
}

pub trait StopExt {
    fn opacity_stops(&self) -> Stop<1>;
    fn color_stops(&self) -> Stop<3>;
}

impl StopExt for usvg::Stop {
    fn opacity_stops(&self) -> Stop<1> {
        Stop {
            color: [self.opacity.get()],
            offset: self.offset.get(),
        }
    }

    fn color_stops(&self) -> Stop<3> {
        Stop {
            color: self.color.to_pdf_color(),
            offset: self.offset.get(),
        }
    }
}

pub trait GroupExt {
    fn is_isolated(&self) -> bool;
}

impl GroupExt for usvg::Group {
    // We use this instead of usvg's should_isolate method because that one also includes
    // clip paths, which shouldn't strictly be necessary but only bloats the file size in PDF.
    fn is_isolated(&self) -> bool {
        // According to the SVG spec, any of these makes a group isolated.
        self.isolate
            || self.mask.is_some()
            || self.blend_mode != usvg::BlendMode::Normal
            || !self.filters.is_empty()
            || self.opacity.get() != 1.0
    }
}

// Taken from resvg
/// Calculate the rect of an image after it is scaled using a view box.
#[cfg(feature = "image")]
pub fn image_rect(view_box: &usvg::ViewBox, img_size: Size) -> NonZeroRect {
    let new_size = fit_view_box(img_size, view_box);
    let (x, y) = usvg::utils::aligned_pos(
        view_box.aspect.align,
        view_box.rect.x(),
        view_box.rect.y(),
        view_box.rect.width() - new_size.width(),
        view_box.rect.height() - new_size.height(),
    );

    new_size.to_non_zero_rect(x, y)
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

/// Calculate the bbox of a node as a [Rect](NonZeroRect).
pub fn plain_bbox(node: &Node, with_stroke: bool) -> NonZeroRect {
    plain_bbox_without_default(node, with_stroke)
        .unwrap_or(NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap())
}

/// Calculate the bbox of a node as a [Rect](NonZeroRect).
pub fn plain_bbox_without_default(node: &Node, with_stroke: bool) -> Option<NonZeroRect> {
    calc_node_bbox(node, Transform::default(), with_stroke)
        .and_then(|b| b.to_non_zero_rect())
}

// Taken from resvg
/// Calculate the bbox of a node with a given transform.
fn calc_node_bbox(node: &Node, ts: Transform, with_stroke: bool) -> Option<BBox> {
    match *node.borrow() {
        NodeKind::Path(ref path) => path
            .data
            .bounds()
            .transform(ts)
            .map(|old_rect| {
                // Adapted from resvg
                if let Some(stroke) = &path.stroke {
                    if with_stroke {
                        let w = stroke.width.get()
                            / if ts.is_identity() {
                                2.0
                            } else {
                                2.0 / (ts.sx * ts.sy - ts.ky * ts.kx).abs().sqrt()
                            };
                        usvg::Rect::from_xywh(
                            old_rect.x() - w,
                            old_rect.y() - w,
                            old_rect.width() + 2.0 * w,
                            old_rect.height() + 2.0 * w,
                        )
                        .unwrap_or(usvg::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap())
                    } else {
                        old_rect
                    }
                } else {
                    old_rect
                }
            })
            .map(BBox::from),
        NodeKind::Image(ref img) => img.view_box.rect.transform(ts).map(BBox::from),
        NodeKind::Group(_) => {
            let mut bbox = BBox::default();

            for child in node.children() {
                let child_transform = ts.pre_concat(child.transform());
                if let Some(c_bbox) = calc_node_bbox(&child, child_transform, with_stroke)
                {
                    bbox = bbox.expand(c_bbox);
                }
            }

            // Make sure bbox was changed.
            if bbox.is_default() {
                return None;
            }

            Some(bbox)
        }
        NodeKind::Text(_) => None,
    }
}

pub fn clip_to_rect(rect: NonZeroRect, content: &mut Content) {
    content.rect(rect.x(), rect.y(), rect.width(), rect.height());
    content.close_path();
    content.clip_nonzero();
    content.end_path();
}
