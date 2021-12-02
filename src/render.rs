//! Provide rendering capabilities for SVG's primitives.

use image::io::Reader as ImageReader;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, Rgba};
use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::types::{ColorSpace, LineCapStyle, LineJoinStyle, ShadingType};
use pdf_writer::writers::{Image, Shading};
use pdf_writer::{Content, Filter, Finish, Name, PdfWriter, Rect, TextStr};
use usvg::{
    AspectRatio, FillRule, ImageKind, LineCap, LineJoin, Node, NodeExt, NodeKind, Paint,
    PathSegment, Transform, ViewBox, Visibility,
};

use super::{
    apply_clip_path, apply_mask, content_stream, form_xobject, from_tree, Context,
    Options, RgbaColor,
};
use crate::defer::{PendingGS, PendingPattern};
use crate::scale::CoordToPdf;

/// Write the appropriate instructions for a node into the content stream.
///
/// The method may use its `PdfWriter` to write auxillary indirect objects such
/// as Form XObjects and use the context to enque pending references to them in
/// corresponding object's `Resources` dictionary.
pub(crate) trait Render {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    );
}

impl Render for usvg::Path {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
        if self.visibility != Visibility::Visible {
            return;
        }

        let bbox = node
            .calculate_bbox()
            .unwrap_or_else(|| usvg::Rect::new(0.0, 0.0, 0.0, 0.0).unwrap());

        let pdf_bbox = Rect::new(
            ctx.c.x(bbox.x()),
            ctx.c.y(bbox.y() + bbox.height()),
            ctx.c.x(bbox.x() + bbox.width()),
            ctx.c.y(bbox.y()),
        );

        let paint = self.fill.as_ref().map(|fill| &fill.paint);

        // Retrieve the fill gradient description struct if the fill is a
        // gradient.
        let fill_gradient_pattern = if let Some(Paint::Link(id)) = paint {
            let node = ctx.tree.defs_by_id(id).unwrap();
            Pattern::from_node(node)
        } else {
            None
        };

        // Get the alpha function for the gradient if there is some.
        let alpha_func = if let Some(Paint::Link(id)) = paint {
            ctx.function_map.get(id).and_then(|x| x.1)
        } else {
            content.save_state();
            None
        };

        // In order to apply non-uniform transparency, e.g. in a gradient, we
        // have to create a Soft Mask in an external graphics state dictionary.
        //
        // The operator for setting the graphics state overrides the previous
        // Soft Mask. Because we want the masks to intersect instead, we wrap
        // the path in a transparency group instead.
        let mut xobj_content = if let Some(alpha_func) = alpha_func {
            // Number of the inner transparency group
            let path_ref = ctx.alloc_xobject();

            // Reference and content stream of the Form XObject containing the
            // Soft Mask shading as a Luminance gradient.
            let smask_form_ref = ctx.alloc_ref();
            let mut shading_content = Content::new();

            // We draw the gradient with the shading operator instead of
            // registering a pattern, so we allocate a shading number for the
            // `Resources` dictionary.
            let shading_num = ctx.alloc_shading();
            let shading_name = format!("sh{}", shading_num);
            shading_content.shading(Name(shading_name.as_bytes()));
            let shading_content = shading_content.finish();

            // Reference for the indirect Shading dictionary.
            let shading_ref = ctx.alloc_ref();
            let mut shading = Shading::new(writer.indirect(shading_ref));
            let pattern = fill_gradient_pattern.clone().unwrap();

            shading.shading_type(pattern.shading_type);
            shading.color_space(ColorSpace::DeviceGray);

            shading.function(alpha_func);
            shading.coords(
                IntoIterator::into_iter(pattern.transformed_coords(ctx.c, bbox)).take(
                    if pattern.shading_type == ShadingType::Axial {
                        4
                    } else {
                        6
                    },
                ),
            );
            shading.extend([true, true]);
            shading.finish();

            // Write the Form XObject for with the luminance-encoded alpha
            // values for the Soft Mask.
            let mut smask_form = form_xobject(
                writer,
                smask_form_ref,
                &shading_content,
                pdf_bbox,
                ColorSpace::DeviceGray,
            );

            smask_form
                .resources()
                .shadings()
                .pair(Name(shading_name.as_bytes()), shading_ref);

            // Write the reference to the transparency group containing the path
            // to the original content stream. For all following operations, we
            // will populate a content stream for this group.
            content.x_object(Name(format!("xo{}", path_ref).as_bytes()));

            // Apply the Graphics State with the Soft Mask first thing in the
            // new content stream.
            let gs_num = ctx.alloc_gs();
            let gs_name = format!("gs{}", gs_num);
            ctx.pending_graphics
                .push(PendingGS::soft_mask(smask_form_ref, gs_num));

            let mut path_content = Content::new();
            path_content.set_parameters(Name(gs_name.as_bytes()));

            Some((path_content, path_ref))
        } else {
            None
        };

        // Exchange the references for the inner content stream if there was an
        // alpha value.
        let content = if let Some((xobj_content, _)) = xobj_content.as_mut() {
            xobj_content
        } else {
            content
        };

        // Combine alpha and opacity values.
        let stroke_opacity = self.stroke.as_ref().map(|s| {
            let mut res = s.opacity.value() as f32;
            if let Paint::Color(c) = s.paint {
                res *= c.alpha as f32 / 255.0;
            }
            res
        });

        let fill_opacity = self.fill.as_ref().map(|f| {
            let mut res = f.opacity.value() as f32;
            if let Paint::Color(c) = f.paint {
                res *= c.alpha as f32 / 255.0;
            }
            res
        });

        // Write a graphics state for stroke and fill opacity.
        if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));
            ctx.pending_graphics.push(PendingGS::opacity(
                stroke_opacity,
                fill_opacity,
                num,
            ));
        }

        if let Some(stroke) = &self.stroke {
            content.set_line_width(ctx.c.px_to_pt(stroke.width.value()));
            match stroke.linecap {
                LineCap::Butt => content.set_line_cap(LineCapStyle::ButtCap),
                LineCap::Round => content.set_line_cap(LineCapStyle::RoundCap),
                LineCap::Square => {
                    content.set_line_cap(LineCapStyle::ProjectingSquareCap)
                }
            };

            match stroke.linejoin {
                LineJoin::Miter => content.set_line_join(LineJoinStyle::MiterJoin),
                LineJoin::Round => content.set_line_join(LineJoinStyle::RoundJoin),
                LineJoin::Bevel => content.set_line_join(LineJoinStyle::BevelJoin),
            };

            content.set_miter_limit(stroke.miterlimit.value() as f32);

            if let Some(dasharray) = &stroke.dasharray {
                content.set_dash_pattern(
                    dasharray.iter().map(|&x| x as f32),
                    stroke.dashoffset,
                );
            }

            match stroke.paint {
                Paint::Color(c) => {
                    let [r, g, b] = RgbaColor::from(c).to_array();
                    content.set_stroke_rgb(r, g, b);
                }
                _ => todo!(),
            }
        }

        match self.fill.as_ref().map(|fill| &fill.paint) {
            Some(Paint::Color(c)) => {
                let [r, g, b] = RgbaColor::from(*c).to_array();
                content.set_fill_rgb(r, g, b);
            }
            Some(Paint::Link(id)) => {
                let item = ctx.tree.defs_by_id(id).unwrap();
                match *item.borrow() {
                    NodeKind::RadialGradient(_) | NodeKind::LinearGradient(_) => {
                        let pattern = fill_gradient_pattern.unwrap();
                        let num = ctx.alloc_pattern();
                        let name = format!("p{}", num);

                        ctx.pending_patterns.push(PendingPattern::from_pattern(
                            pattern, bbox, num, ctx.c,
                        ));

                        content.set_fill_color_space(ColorSpace::Pattern);
                        content.set_fill_pattern(None, Name(name.as_bytes()));
                    }
                    _ => todo!(),
                };
            }
            None => {}
        }

        draw_path(&self.data.0, self.transform, content, ctx.c);

        match (self.fill.as_ref().map(|f| f.rule), self.stroke.is_some()) {
            (Some(FillRule::NonZero), true) => content.fill_nonzero_and_stroke(),
            (Some(FillRule::EvenOdd), true) => content.fill_even_odd_and_stroke(),
            (Some(FillRule::NonZero), false) => content.fill_nonzero(),
            (Some(FillRule::EvenOdd), false) => content.fill_even_odd(),
            (None, true) => content.stroke(),
            (None, false) => content,
        };

        // We only backed up the graphics state if there was no alpha
        // transparency so we only restore it in that case.
        if alpha_func.is_none() {
            content.restore_state();
        }

        // Write the Form XObject if there was a gradient with alpha values.
        if let Some((xobj_content, path_no)) = xobj_content {
            let path_ref = ctx.alloc_ref();
            let data = xobj_content.finish();
            form_xobject(writer, path_ref, &data, pdf_bbox, ColorSpace::DeviceRgb);
            ctx.pending_xobjects.push((path_no, path_ref));
        }
    }
}

impl Render for usvg::Group {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
        if !self.filter.is_empty() {
            todo!();
            return;
        }

        ctx.push();

        let group_ref = ctx.alloc_ref();
        let child_content = content_stream(&node, writer, ctx);

        let bbox = node
            .calculate_bbox()
            .unwrap_or_else(|| usvg::Rect::new(0.0, 0.0, 0.0, 0.0).unwrap());
        let pdf_bbox = Rect::new(
            ctx.c.x(bbox.x()),
            ctx.c.y(bbox.y()),
            ctx.c.x(bbox.x() + bbox.width()),
            ctx.c.y(bbox.y() + bbox.height()),
        );

        // Every group is an isolated transparency group, it needs to be painted
        // onto its own canvas.
        let mut form = form_xobject(
            writer,
            group_ref,
            &child_content,
            pdf_bbox,
            ColorSpace::DeviceRgb,
        );

        let mut resources = form.resources();
        ctx.pop(&mut resources);

        let num = ctx.alloc_xobject();
        let name = format!("xo{}", num);
        content.save_state();

        apply_clip_path(self.clip_path.as_ref(), content, ctx);

        if let Some(reference) = apply_mask(self.mask.as_ref(), bbox, pdf_bbox, ctx) {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));

            ctx.pending_graphics.push(PendingGS::soft_mask(reference, num));
        }

        if self.opacity.value() != 1.0 {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));

            ctx.pending_graphics
                .push(PendingGS::fill_opacity(self.opacity.value() as f32, num));
        }

        content.x_object(Name(name.as_bytes()));
        content.restore_state();
        ctx.pending_xobjects.push((num, group_ref));
    }
}

impl Render for usvg::Image {
    fn render(
        &self,
        _: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
        {
            if self.visibility != Visibility::Visible {
                return;
            }

            let image_ref = ctx.alloc_ref();
            let set_image_props = |
                image: &mut Image,
                raster_size: &mut Option<(u32, u32)>,
                decoded: &DynamicImage,
                grey: bool,
            | {
                let color = decoded.color();
                *raster_size = Some((decoded.width(), decoded.height()));
                image
                    .width(decoded.width() as i32)
                    .height(decoded.height() as i32)
                    .color_space(if !grey && color.has_color() {
                        ColorSpace::DeviceRgb
                    } else {
                        ColorSpace::DeviceGray
                    })
                    .bits_per_component(
                        (color.bits_per_pixel() / color.channel_count() as u16) as i32,
                    );
            };

            let mut raster_size: Option<(u32, u32)> = None;
            let rect = self.view_box.rect;

            match &self.kind {
                ImageKind::JPEG(buf) => {
                    let cursor = std::io::Cursor::new(buf);
                    let decoded = if let Ok(decoded) =
                        ImageReader::with_format(cursor, ImageFormat::Jpeg).decode()
                    {
                        decoded
                    } else {
                        return;
                    };

                    let mut image = writer.image(image_ref, buf);
                    set_image_props(&mut image, &mut raster_size, &decoded, false);
                    image.filter(Filter::DctDecode);
                }
                ImageKind::PNG(buf) => {
                    let cursor = std::io::Cursor::new(buf);
                    let decoded = if let Ok(decoded) =
                        ImageReader::with_format(cursor, ImageFormat::Png).decode()
                    {
                        decoded
                    } else {
                        return;
                    };

                    let color = decoded.color();

                    let image_bytes: Vec<u8> =
                        if (color.bits_per_pixel() / color.channel_count() as u16) > 8 {
                            decoded
                                .to_rgb16()
                                .pixels()
                                .flat_map(|&Rgb(c)| c)
                                .flat_map(|x| x.to_be_bytes())
                                .collect()
                        } else {
                            decoded.to_rgb8().pixels().flat_map(|&Rgb(c)| c).collect()
                        };


                    let compressed = compress_to_vec_zlib(&image_bytes, 8);

                    let mut image = writer.image(image_ref, &compressed);
                    set_image_props(&mut image, &mut raster_size, &decoded, false);
                    image.filter(Filter::FlateDecode);

                    // The alpha channel has to be written separately, as a Soft
                    // Mask.
                    if color.has_alpha() {
                        let mask_id = ctx.alloc_ref();
                        image.pair(Name(b"SMask"), mask_id);
                        image.finish();

                        let alpha_bytes: Vec<u8> = if (color.bits_per_pixel()
                            / color.channel_count() as u16)
                            > 8
                        {
                            decoded
                                .to_rgba16()
                                .pixels()
                                .flat_map(|&Rgba([.., a])| a.to_be_bytes())
                                .collect()
                        } else {
                            decoded.to_rgba8().pixels().map(|&Rgba([.., a])| a).collect()
                        };

                        let compressed = compress_to_vec_zlib(&alpha_bytes, 8);
                        let mut mask = writer.image(mask_id, &compressed);
                        let mut void = None;

                        set_image_props(&mut mask, &mut void, &decoded, true);

                        mask.filter(Filter::FlateDecode);
                    }
                }
                ImageKind::SVG(tree) => {
                    // An SVG image means that the file gets embedded in a
                    // completely isolated fashion, thus we convert its tree
                    // recursively here.
                    let opt = Options {
                        viewport: Some((rect.width(), rect.height())),
                        respect_native_size: false,
                        aspect_ratio: Some(self.view_box.aspect),
                        dpi: ctx.c.dpi(),
                    };

                    let bytes = match from_tree(tree, opt) {
                        Some(bytes) => bytes,
                        None => return,
                    };
                    let byte_len = bytes.len();
                    let compressed = compress_to_vec_zlib(&bytes, 8);

                    let file_embedd_num = ctx.alloc_ref();
                    let mut embedded = writer.embedded_file(file_embedd_num, &compressed);
                    embedded
                        .subtype(Name(b"application#2Fpdf"))
                        .filter(Filter::FlateDecode);
                    embedded.params().size(byte_len as i32);
                    embedded.finish();

                    writer
                        .form_xobject(image_ref, &[])
                        .bbox(Rect::new(
                            0.0,
                            0.0,
                            ctx.c.px_to_pt(rect.x() + rect.width()),
                            ctx.c.px_to_pt(rect.y() + rect.height()),
                        ))
                        .reference()
                        .page_number(0)
                        .file()
                        .description(TextStr("Embedded SVG image"))
                        .embedded_file(file_embedd_num);
                }
            }

            // Common operations for raster image formats.
            let image_ref = if let Some((width, height)) = raster_size {
                let mut content = Content::new();
                let xobj_name = Name(b"EmbRaster");
                let converter = CoordToPdf::new(
                    (rect.width(), rect.height()),
                    ctx.c.dpi(),
                    ViewBox {
                        rect: usvg::Rect::new(0.0, 0.0, width as f64, height as f64)
                            .unwrap(),
                        aspect: AspectRatio::default(),
                    },
                    Some(self.view_box.aspect),
                );

                content.save_state();
                content.transform([
                    (width as f64 * converter.factor_x()) as f32,
                    0.0,
                    0.0,
                    (height as f64 * converter.factor_y()) as f32,
                    converter.offset_x() as f32,
                    converter.offset_y() as f32,
                ]);
                content.x_object(xobj_name);
                content.restore_state();

                let content = content.finish();
                let external_ref = ctx.alloc_ref();

                let mut xobject = writer.form_xobject(external_ref, &content);
                xobject.resources().x_objects().pair(xobj_name, image_ref);
                xobject.bbox(Rect::new(
                    0.0,
                    0.0,
                    (rect.x() + rect.width()) as f32,
                    (rect.y() + rect.height()) as f32,
                ));

                let scaling = 72.0 / ctx.c.dpi();
                let mut transform = self.transform.clone();
                transform.scale(scaling, scaling);
                xobject.matrix([
                    transform.a as f32,
                    transform.b as f32,
                    transform.c as f32,
                    transform.d as f32,
                    transform.e as f32,
                    transform.f as f32,
                ]);

                external_ref
            } else {
                image_ref
            };

            let num = ctx.alloc_xobject();
            ctx.pending_xobjects.push((num, image_ref));
            let name = format!("xo{}", num);

            content.move_to(ctx.c.x(rect.x()), ctx.c.y(rect.y()));
            content.x_object(Name(name.as_bytes()));
        }
    }
}

/// Draw a path into a content stream. Does close the path but not perform any
/// drawing operators.
pub fn draw_path(
    path_data: &[PathSegment],
    transform: Transform,
    content: &mut Content,
    c: &CoordToPdf,
) {
    for &operation in path_data {
        match operation {
            PathSegment::MoveTo { x, y } => {
                let (x, y) = transform.apply(x, y);
                content.move_to(c.x(x), c.y(y));
            }
            PathSegment::LineTo { x, y } => {
                let (x, y) = transform.apply(x, y);
                content.line_to(c.x(x), c.y(y));
            }
            PathSegment::CurveTo { x1, y1, x2, y2, x, y } => {
                let (x1, y1) = transform.apply(x1, y1);
                let (x2, y2) = transform.apply(x2, y2);
                let (x, y) = transform.apply(x, y);

                content.cubic_to(c.x(x1), c.y(y1), c.x(x2), c.y(y2), c.x(x), c.y(y));
            }
            PathSegment::ClosePath => {
                content.close_path();
            }
        }
    }
}

/// Describes a pattern in use for some object.
#[derive(Clone)]
pub(crate) struct Pattern {
    /// The SVG id of the pattern that can also be used to retreive its
    /// functions.
    pub(crate) id: String,
    /// The type of gradient.
    pub(crate) shading_type: ShadingType,
    /// The coordinates of the gradient.
    pub(crate) coords: [f64; 6],
    /// Whether to transform the coords to the bounding box of the element or
    /// keep them in the page coordinate system.
    pub(crate) transform_coords: bool,
}

impl Pattern {
    fn from_node(node: Node) -> Option<Self> {
        match *node.borrow() {
            NodeKind::LinearGradient(ref lg) => Some(Self {
                id: lg.id.clone(),
                shading_type: ShadingType::Axial,
                coords: [lg.x1, lg.y1, lg.x2, lg.y2, 0.0, 0.0],
                transform_coords: lg.base.units == usvg::Units::ObjectBoundingBox,
            }),
            NodeKind::RadialGradient(ref rg) => Some(Self {
                id: rg.id.clone(),
                shading_type: ShadingType::Radial,
                coords: [rg.fx, rg.fy, rg.cx, rg.cy, 0.0, rg.r.value()],
                transform_coords: rg.base.units == usvg::Units::ObjectBoundingBox,
            }),
            _ => None,
        }
    }

    /// Apply the transformation and reorder the coordinates depending on the
    /// shading type.
    pub(crate) fn transformed_coords(
        &self,
        c: &CoordToPdf,
        bbox: usvg::Rect,
    ) -> [f32; 6] {
        let max = if bbox.width() > bbox.height() {
            bbox.width()
        } else {
            bbox.height()
        };

        let coords = if self.transform_coords {
            [
                c.x(bbox.x() + self.coords[0] * bbox.width()),
                c.y(bbox.y() + self.coords[1] * bbox.height()),
                c.x(bbox.x() + self.coords[2] * bbox.width()),
                c.y(bbox.y() + self.coords[3] * bbox.height()),
                c.px_to_pt(self.coords[4] * max),
                c.px_to_pt(self.coords[5] * max),
            ]
        } else {
            [
                c.x(self.coords[0]),
                c.y(self.coords[1]),
                c.x(self.coords[2]),
                c.y(self.coords[3]),
                c.px_to_pt(self.coords[4]),
                c.px_to_pt(self.coords[5]),
            ]
        };

        if self.shading_type == ShadingType::Axial {
            [coords[0], coords[1], coords[2], coords[3], 0.0, 0.0]
        } else {
            [
                coords[0], coords[1], coords[4], coords[2], coords[3], coords[5],
            ]
        }
    }
}
