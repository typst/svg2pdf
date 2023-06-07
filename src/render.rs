//! Provide rendering capabilities for SVG's primitives.

use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::types::{
    ColorSpaceOperand, LineCapStyle, LineJoinStyle, PaintType, ProcSet, ShadingType,
    TilingType,
};
use pdf_writer::writers::Shading;
use pdf_writer::{Content, Filter, Finish, Name, PdfWriter, Rect, Ref, Writer};
use usvg::{
    tiny_skia_path::{PathSegment, Point},
    Align, AspectRatio, FillRule, ImageKind, LineCap, LineJoin, Node, NodeExt, Paint,
    Pattern, Transform, Units, ViewBox, Visibility,
};

#[cfg(any(feature = "png", feature = "jpeg"))]
use {
    image::io::Reader as ImageReader,
    image::{DynamicImage, ImageFormat, Luma, Rgb, Rgba},
    pdf_writer::writers::ImageXObject,
};

use super::{
    apply_clip_path, apply_mask, content_stream, form_xobject, Context, Options,
    RgbColor, SRGB,
};
use crate::defer::{PendingGS, PendingGradient};
use crate::scale::CoordToPdf;
use crate::{convert_tree_into, deflate};

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
            .and_then(|b| b.to_non_zero_rect())
            .unwrap_or_else(|| usvg::NonZeroRect::from_ltrb(0.0, 0.0, 1.0, 1.0).unwrap());

        let (fill_gradient, fill_g_alpha) =
            get_gradient(self.fill.as_ref().map(|fill| &fill.paint), ctx);

        let (stroke_gradient, stroke_g_alpha) =
            get_gradient(self.stroke.as_ref().map(|stroke| &stroke.paint), ctx);

        if fill_g_alpha.is_some() || stroke_g_alpha.is_some() {
            render_path_partial(
                self,
                bbox,
                true,
                false,
                fill_gradient,
                None,
                fill_g_alpha,
                None,
                writer,
                content,
                ctx,
            );
            render_path_partial(
                self,
                bbox,
                false,
                true,
                None,
                stroke_gradient,
                None,
                stroke_g_alpha,
                writer,
                content,
                ctx,
            );
        } else {
            render_path_partial(
                self,
                bbox,
                true,
                true,
                fill_gradient,
                stroke_gradient,
                fill_g_alpha,
                stroke_g_alpha,
                writer,
                content,
                ctx,
            )
        }
    }
}

fn render_path_partial(
    path: &usvg::Path,
    bbox: usvg::NonZeroRect,
    fill: bool,
    stroke: bool,
    fill_gradient: Option<Gradient>,
    stroke_gradient: Option<Gradient>,
    fill_g_alpha: Option<Ref>,
    stroke_g_alpha: Option<Ref>,
    writer: &mut PdfWriter,
    content: &mut Content,
    ctx: &mut Context,
) {
    // In order to apply non-uniform transparency, e.g. in a gradient, we
    // have to create a Soft Mask in an external graphics state dictionary.
    //
    // The operator for setting the graphics state overrides the previous
    // Soft Mask. Because we want the masks to intersect instead, we wrap
    // the path in a transparency group instead.
    let mut xobj_content = if let Some(alpha_func) = fill_g_alpha {
        let smask_form_ref =
            prep_shading(alpha_func, fill_gradient.as_ref().unwrap(), bbox, writer, ctx);

        Some(start_wrap(smask_form_ref, content, ctx))
    } else if let Some(alpha_func) = stroke_g_alpha {
        let smask_form_ref = prep_shading(
            alpha_func,
            stroke_gradient.as_ref().unwrap(),
            bbox,
            writer,
            ctx,
        );

        Some(start_wrap(smask_form_ref, content, ctx))
    } else {
        content.save_state();
        None
    };

    // Exchange the references for the inner content stream if there was an
    // alpha value.
    let content = if let Some((xobj_content, _)) = xobj_content.as_mut() {
        xobj_content
    } else {
        content
    };

    content.set_fill_color_space(ColorSpaceOperand::Named(SRGB));
    content.set_stroke_color_space(ColorSpaceOperand::Named(SRGB));

    let stroke_opacity = path.stroke.as_ref().map(|s| s.opacity.get());
    let fill_opacity = path.fill.as_ref().map(|f| f.opacity.get());

    // Write a graphics state for stroke and fill opacity.
    if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
        let num = ctx.alloc_gs();
        content.set_parameters(Name(format!("gs{}", num).as_bytes()));
        ctx.pending_graphics
            .push(PendingGS::opacity(stroke_opacity, fill_opacity, num));
    }

    if stroke {
        if let Some(stroke) = &path.stroke {
            content.set_line_width(
                ctx.c.px_to_pt(stroke.width.get() * ctx.c.compute_scale()),
            );

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

            content.set_miter_limit(stroke.miterlimit.get());

            if let Some(dasharray) = &stroke.dasharray {
                content.set_dash_pattern(dasharray.iter().cloned(), stroke.dashoffset);
            }

            match &stroke.paint {
                Paint::Color(c) => {
                    content.set_stroke_color(RgbColor::from(*c).to_array());
                }
                Paint::LinearGradient(_)
                | Paint::RadialGradient(_)
                | Paint::Pattern(_) => {
                    let item = &stroke.paint;
                    content.set_stroke_color_space(ColorSpaceOperand::Pattern);
                    let num = ctx.alloc_pattern();
                    let name = format!("p{}", num);

                    match item {
                        Paint::RadialGradient(_) | Paint::LinearGradient(_) => {
                            let pattern = stroke_gradient.unwrap();

                            ctx.pending_gradients.push(PendingGradient::from_gradient(
                                pattern, bbox, num, &ctx.c,
                            ));
                        }
                        Paint::Pattern(ref pattern) => {
                            prep_pattern(pattern, num, bbox, writer, ctx);
                        }
                        _ => unreachable!(),
                    }

                    content.set_stroke_pattern(None, Name(name.as_bytes()));
                }
            }
        }
    }

    if fill {
        let paint = path.fill.as_ref().map(|fill| &fill.paint);
        match paint {
            Some(Paint::Color(c)) => {
                content.set_fill_color(RgbColor::from(*c).to_array());
            }
            Some(Paint::LinearGradient(_))
            | Some(Paint::RadialGradient(_))
            | Some(Paint::Pattern(_)) => {
                content.set_fill_color_space(ColorSpaceOperand::Pattern);

                let num = ctx.alloc_pattern();
                let name = format!("p{}", num);

                match paint {
                    Some(Paint::RadialGradient(_)) | Some(Paint::LinearGradient(_)) => {
                        let pattern = fill_gradient.unwrap();

                        ctx.pending_gradients.push(PendingGradient::from_gradient(
                            pattern, bbox, num, &ctx.c,
                        ));
                    }
                    Some(Paint::Pattern(ref pattern)) => {
                        prep_pattern(pattern, num, bbox, writer, ctx);
                    }
                    _ => unreachable!(),
                }

                content.set_fill_pattern(None, Name(name.as_bytes()));
            }

            None => {}
        }
    }

    draw_path(path.data.segments(), path.transform, content, &ctx.c);

    match (path.fill.as_ref().map(|f| f.rule), fill, path.stroke.is_some() && stroke) {
        (Some(FillRule::NonZero), true, true) => content.fill_nonzero_and_stroke(),
        (Some(FillRule::EvenOdd), true, true) => content.fill_even_odd_and_stroke(),
        (Some(FillRule::NonZero), true, false) => content.fill_nonzero(),
        (Some(FillRule::EvenOdd), true, false) => content.fill_even_odd(),
        (None, _, true) | (_, false, true) => content.stroke(),
        (None, _, false) | (_, false, false) => content.end_path(),
    };

    // We only backed up the graphics state if there was no alpha
    // transparency so we only restore it in that case.
    if fill_g_alpha.is_none() && stroke_g_alpha.is_none() {
        content.restore_state();
    }

    let pdf_bbox = ctx.c.bbox();

    // Write the Form XObject if there was a gradient with alpha values.
    if let Some((xobj_content, path_no)) = xobj_content {
        let path_ref = ctx.alloc_ref();
        let data = if ctx.compress {
            deflate(&xobj_content.finish())
        } else {
            xobj_content.finish()
        };

        let mut form =
            form_xobject(writer, path_ref, &data, pdf_bbox, ctx.compress, true);
        let mut resources = form.resources();
        ctx.pop(&mut resources);
        ctx.pending_xobjects.push((path_no, path_ref));
    }
}

/// Convert usvg's transforms to PDF matrices.
fn transform_to_matrix(transform: Transform) -> [f32; 6] {
    [transform.sx, transform.ky, transform.kx, transform.sy, transform.tx, transform.ty]
}

/// Retrieve the pattern and alpha values for a paint.
fn get_gradient(paint: Option<&Paint>, ctx: &Context) -> (Option<Gradient>, Option<Ref>) {
    // Retrieve the fill gradient description struct if the fill is a
    // gradient.
    let gradient =
        if let Some(paint) = paint { Gradient::from_paint(paint) } else { None };

    // Get the alpha function for the gradient if there is some.
    let alpha_func_id = match paint {
        Some(Paint::LinearGradient(ref gradient)) => &gradient.id,
        Some(Paint::RadialGradient(ref gradient)) => &gradient.id,
        Some(Paint::Pattern(ref pattern)) => &pattern.id,
        _ => {
            return (gradient, None);
        }
    };

    let alpha_func = ctx.function_map.get(alpha_func_id).and_then(|x| x.1);

    (gradient, alpha_func)
}

/// Write the alpha shading Form XObject using a function. Returns an indirect
/// reference to a Luminance-shaded XObject.
fn prep_shading(
    alpha_func: Ref,
    gradient: &Gradient,
    bbox: usvg::NonZeroRect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) -> Ref {
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
    let mut shading = Shading::start(writer.indirect(shading_ref));

    shading.shading_type(gradient.shading_type);
    shading.color_space().d65_gray();
    shading.function(alpha_func);
    shading.coords(
        gradient
            .transformed_coords(&ctx.c, bbox)
            .into_iter()
            .take(if gradient.shading_type == ShadingType::Axial { 4 } else { 6 }),
    );
    shading.extend([true, true]);
    shading.finish();

    // Write the Form XObject for with the luminance-encoded alpha
    // values for the Soft Mask.
    let mut smask_form = form_xobject(
        writer,
        smask_form_ref,
        &shading_content,
        ctx.c.pdf_rect(bbox),
        false,
        false,
    );

    let mut resources = smask_form.resources();
    resources.proc_sets([ProcSet::Pdf, ProcSet::ImageGrayscale]);
    resources.shadings().pair(Name(shading_name.as_bytes()), shading_ref);

    smask_form_ref
}

/// Start wrapping a content stream in an Form XObject to combine graphics state
/// applicability.
fn start_wrap(
    smask_ref: Ref,
    content: &mut Content,
    ctx: &mut Context,
) -> (Content, u32) {
    // Number of the inner transparency group
    let path_ref = ctx.alloc_xobject();

    // Write the reference to the transparency group containing the path
    // to the original content stream. For all following operations, we
    // will populate a content stream for this group.
    content.x_object(Name(format!("xo{}", path_ref).as_bytes()));

    // Apply the Graphics State with the Soft Mask first thing in the
    // new content stream.
    let gs_num = ctx.alloc_gs();
    let gs_name = format!("gs{}", gs_num);
    ctx.push();
    ctx.pending_graphics.push(PendingGS::soft_mask(smask_ref, gs_num));

    let mut path_content = Content::new();
    path_content.set_parameters(Name(gs_name.as_bytes()));

    (path_content, path_ref)
}

/// Write a pattern to the file for use for filling or stroking.
fn prep_pattern(
    pattern: &Pattern,
    num: u32,
    bbox: usvg::NonZeroRect,
    writer: &mut PdfWriter,
    ctx: &mut Context,
) {
    let rect = match pattern.units {
        Units::UserSpaceOnUse => pattern.rect,
        Units::ObjectBoundingBox => usvg::NonZeroRect::from_ltrb(
            pattern.rect.x() * bbox.width() + bbox.x(),
            pattern.rect.y() * bbox.height() + bbox.y(),
            pattern.rect.width() * bbox.width(),
            pattern.rect.height() * bbox.height(),
        )
        .unwrap(),
    };

    let matrix = transform_to_matrix(pattern.transform);
    let pdf_rect = ctx.c.pdf_rect(rect);

    let mut inner_transform = if let Some(viewbox) = pattern.view_box {
        CoordToPdf::new((rect.width(), rect.height()), ctx.c.dpi(), viewbox, None)
            .uncorrected_transformation()
    } else if pattern.content_units == Units::ObjectBoundingBox {
        let viewbox = ViewBox {
            rect: usvg::NonZeroRect::from_ltrb(0.0, 0.0, 1.0, 1.0).unwrap(),
            aspect: AspectRatio { defer: false, align: Align::None, slice: false },
        };

        CoordToPdf::new((bbox.width(), bbox.height()), ctx.c.dpi(), viewbox, None)
            .uncorrected_transformation()
    } else {
        Transform::identity()
    };

    ctx.push();

    inner_transform.tx += rect.x();
    inner_transform.ty += rect.y();

    let old = ctx.c.concat_transform(inner_transform);

    let pattern_stream = content_stream(&pattern.root, writer, ctx);
    ctx.c.set_transform(old);

    let pattern_ref = ctx.alloc_ref();
    let mut pdf_pattern = writer.tiling_pattern(pattern_ref, &pattern_stream);
    pdf_pattern
        .tiling_type(TilingType::ConstantSpacing)
        .paint_type(PaintType::Colored);

    pdf_pattern
        .bbox(pdf_rect)
        .x_step(pdf_rect.x2 - pdf_rect.x1)
        .y_step(pdf_rect.y2 - pdf_rect.y1);

    if ctx.compress {
        pdf_pattern.filter(Filter::FlateDecode);
    }

    let mut resources = pdf_pattern.resources();
    ctx.pop(&mut resources);
    resources.finish();

    pdf_pattern.matrix(matrix);
    ctx.pending_patterns.push((num, pattern_ref))
}

impl Render for usvg::Group {
    fn render(
        &self,
        node: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
        ctx.push();

        let group_ref = ctx.alloc_ref();

        let bbox = node
            .calculate_bbox()
            .and_then(|b| b.to_non_zero_rect())
            .unwrap_or_else(|| usvg::NonZeroRect::from_ltrb(0.0, 0.0, 1.0, 1.0).unwrap());

        let pdf_bbox = ctx.c.bbox();
        let old = ctx.c.concat_transform(self.transform);

        let child_content = content_stream(node, writer, ctx);

        // Every group is an isolated transparency group, it needs to be painted
        // onto its own canvas.
        let mut form =
            form_xobject(writer, group_ref, &child_content, pdf_bbox, ctx.compress, true);

        let mut resources = form.resources();
        ctx.pop(&mut resources);

        let num = ctx.alloc_xobject();
        let name = format!("xo{}", num);
        content.save_state();
        resources.finish();
        form.finish();

        apply_clip_path(self.clip_path.clone(), content, writer, ctx);

        if let Some(reference) = apply_mask(self.mask.clone(), bbox, pdf_bbox, ctx) {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));
            ctx.pending_graphics.push(PendingGS::soft_mask(reference, num));
        }

        if self.opacity.get() != 1.0 {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));
            ctx.pending_graphics
                .push(PendingGS::fill_opacity(self.opacity.get(), num));
        }

        ctx.c.set_transform(old);

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

            #[cfg(any(feature = "png", feature = "jpeg", feature = "gif"))]
            let set_image_props = |image: &mut ImageXObject,
                                   raster_size: &mut Option<(u32, u32)>,
                                   decoded: &DynamicImage,
                                   grey: bool| {
                let color = decoded.color();
                *raster_size = Some((decoded.width(), decoded.height()));
                image.width(decoded.width() as i32);
                image.height(decoded.height() as i32);
                image.bits_per_component(
                    (color.bits_per_pixel() / color.channel_count() as u16) as i32,
                );

                let space = image.color_space();
                if !grey && color.has_color() {
                    space.device_rgb();
                } else {
                    space.device_gray();
                }
            };

            #[cfg(any(feature = "png", feature = "jpeg", feature = "gif"))]
            let mut raster_size: Option<(u32, u32)> = None;
            let rect = self.view_box.rect;

            #[cfg(any(feature = "png", feature = "gif"))]
            let mut apply_transparent = |decoded: DynamicImage| {
                let color = decoded.color();

                let bits = color.bits_per_pixel();
                let channels = color.channel_count() as u16;
                let image_bytes: Vec<u8> = match (channels, bits / channels > 8) {
                    (1, false) => {
                        decoded.to_luma8().pixels().flat_map(|&Luma(c)| c).collect()
                    }
                    (1, true) => decoded
                        .to_luma16()
                        .pixels()
                        .flat_map(|&Luma(x)| x)
                        .flat_map(|x| x.to_be_bytes())
                        .collect(),
                    (3 | 4, false) => {
                        decoded.to_rgb8().pixels().flat_map(|&Rgb(c)| c).collect()
                    }
                    (3 | 4, true) => decoded
                        .to_rgb16()
                        .pixels()
                        .flat_map(|&Rgb(c)| c)
                        .flat_map(|x| x.to_be_bytes())
                        .collect(),
                    _ => panic!("unknown number of channels={channels}"),
                };
                let compressed = compress_to_vec_zlib(&image_bytes, 8);

                let mut image = writer.image_xobject(image_ref, &compressed);
                set_image_props(&mut image, &mut raster_size, &decoded, false);
                image.filter(Filter::FlateDecode);

                // The alpha channel has to be written separately, as a Soft
                // Mask.
                if color.has_alpha() {
                    let mask_id = ctx.alloc_ref();
                    image.pair(Name(b"SMask"), mask_id);
                    image.finish();

                    let bits = color.bits_per_pixel();
                    let channels = color.channel_count() as u16;
                    let alpha_bytes: Vec<u8> = if bits / channels > 8 {
                        decoded
                            .to_rgba16()
                            .pixels()
                            .flat_map(|&Rgba([.., a])| a.to_be_bytes())
                            .collect()
                    } else {
                        decoded.to_rgba8().pixels().map(|&Rgba([.., a])| a).collect()
                    };

                    let compressed = compress_to_vec_zlib(&alpha_bytes, 8);
                    let mut mask = writer.image_xobject(mask_id, &compressed);
                    let mut void = None;

                    set_image_props(&mut mask, &mut void, &decoded, true);
                    mask.filter(Filter::FlateDecode);
                }
            };

            match &self.kind {
                #[cfg(feature = "jpeg")]
                ImageKind::JPEG(buf) => {
                    let cursor = std::io::Cursor::new(buf.as_ref());
                    let decoded = if let Ok(decoded) =
                        ImageReader::with_format(cursor, ImageFormat::Jpeg).decode()
                    {
                        decoded
                    } else {
                        return;
                    };

                    let mut image = writer.image_xobject(image_ref, buf);
                    set_image_props(&mut image, &mut raster_size, &decoded, false);
                    image.filter(Filter::DctDecode);
                }
                #[cfg(feature = "png")]
                ImageKind::PNG(buf) => {
                    let cursor = std::io::Cursor::new(buf.as_ref());
                    apply_transparent(
                        if let Ok(decoded) =
                            ImageReader::with_format(cursor, ImageFormat::Png).decode()
                        {
                            decoded
                        } else {
                            return;
                        },
                    );
                }
                #[cfg(feature = "gif")]
                ImageKind::GIF(buf) => {
                    let cursor = std::io::Cursor::new(buf.as_ref());
                    apply_transparent(
                        if let Ok(decoded) =
                            ImageReader::with_format(cursor, ImageFormat::Gif).decode()
                        {
                            decoded
                        } else {
                            return;
                        },
                    );
                }
                ImageKind::SVG(tree) => {
                    // An SVG image means that the file gets embedded in a
                    // completely isolated fashion, thus we convert its tree
                    // recursively here.
                    let opt = Options {
                        viewport: Some((rect.width(), rect.height())),
                        aspect: Some(self.view_box.aspect),
                        dpi: ctx.c.dpi(),
                        compress: ctx.compress,
                    };

                    ctx.next_id = convert_tree_into(tree, opt, writer, image_ref).get();
                }
                #[cfg(any(
                    not(feature = "jpeg"),
                    not(feature = "png"),
                    not(feature = "gif")
                ))]
                _ => {}
            }

            // Common operations for raster image formats.
            #[cfg(any(feature = "png", feature = "jpeg", feature = "gif"))]
            let image_ref = if let Some((width, height)) = raster_size {
                let mut content = Content::new();
                let xobj_name = Name(b"EmbRaster");
                let converter = CoordToPdf::new(
                    (rect.width(), rect.height()),
                    ctx.c.dpi(),
                    ViewBox {
                        rect: usvg::NonZeroRect::from_ltrb(
                            0.0,
                            0.0,
                            width as f32,
                            height as f32,
                        )
                        .unwrap(),
                        aspect: AspectRatio::default(),
                    },
                    Some(self.view_box.aspect),
                );

                content.save_state();
                content.transform([
                    width as f32 * converter.factor_x(),
                    0.0,
                    0.0,
                    height as f32 * converter.factor_y(),
                    converter.offset_x(),
                    converter.offset_y(),
                ]);
                content.x_object(xobj_name);
                content.restore_state();

                let content = content.finish();
                let external_ref = ctx.alloc_ref();

                let mut xobject = writer.form_xobject(external_ref, &content);
                let mut resources = xobject.resources();
                resources.proc_sets([ProcSet::ImageColor, ProcSet::ImageGrayscale]);
                resources.x_objects().pair(xobj_name, image_ref);
                resources.finish();

                xobject.bbox(Rect::new(0.0, 0.0, rect.width(), rect.height()));

                let scaling = 72.0 / ctx.c.dpi();
                let transform = self.transform.pre_scale(scaling, scaling);
                xobject.matrix(transform_to_matrix(transform));

                external_ref
            } else {
                image_ref
            };

            let num = ctx.alloc_xobject();
            ctx.pending_xobjects.push((num, image_ref));
            let name = format!("xo{}", num);

            let (x, y) = ctx.c.point((rect.x(), rect.y() + rect.height()));
            content.transform([1.0, 0.0, 0.0, 1.0, x, y]);
            content.x_object(Name(name.as_bytes()));
        }
    }
}

/// Draw a path into a content stream. Does close the path but not perform any
/// drawing operators.
pub fn draw_path(
    path_data: impl Iterator<Item = PathSegment>,
    transform: Transform,
    content: &mut Content,
    c: &CoordToPdf,
) {
    for operation in path_data {
        match operation {
            PathSegment::MoveTo(point) => {
                let (x, y) = c.point(apply(&transform, point));
                content.move_to(x, y);
            }
            PathSegment::LineTo(point) => {
                let (x, y) = c.point(apply(&transform, point));
                content.line_to(x, y);
            }
            PathSegment::QuadTo(p1, p2) => {
                let (x1, y1) = c.point(apply(&transform, p1));
                let (x3, y3) = c.point(apply(&transform, p2));
                content.cubic_to_final(x1, y1, x3, y3);
            }
            PathSegment::CubicTo(p1, p2, p3) => {
                let (x1, y1) = c.point(apply(&transform, p1));
                let (x2, y2) = c.point(apply(&transform, p2));
                let (x3, y3) = c.point(apply(&transform, p3));
                content.cubic_to(x1, y1, x2, y2, x3, y3);
            }
            PathSegment::Close => {
                content.close_path();
            }
        }
    }
}

/// Describes a pattern in use for some object.
#[derive(Clone)]
pub(crate) struct Gradient {
    /// The SVG id of the pattern that can also be used to retrieve its
    /// functions.
    pub(crate) id: String,
    /// The type of gradient.
    pub(crate) shading_type: ShadingType,
    /// The coordinates of the gradient.
    pub(crate) coords: [f32; 6],
    /// Whether to transform the coords to the bounding box of the element or
    /// keep them in the page coordinate system.
    pub(crate) transform_coords: bool,
}

impl Gradient {
    fn from_paint(paint: &Paint) -> Option<Self> {
        match paint {
            Paint::LinearGradient(ref lg) => Some(Self {
                id: lg.id.clone(),
                shading_type: ShadingType::Axial,
                coords: [lg.x1, lg.y1, lg.x2, lg.y2, 0.0, 0.0],
                transform_coords: lg.base.units == usvg::Units::ObjectBoundingBox,
            }),
            Paint::RadialGradient(ref rg) => Some(Self {
                id: rg.id.clone(),
                shading_type: ShadingType::Radial,
                coords: [rg.fx, rg.fy, rg.cx, rg.cy, 0.0, rg.r.get()],
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
        bbox: usvg::NonZeroRect,
    ) -> [f32; 6] {
        let max = if bbox.width() > bbox.height() { bbox.width() } else { bbox.height() };

        let coords = if self.transform_coords {
            let (x1, y1) = c.point((
                bbox.x() + self.coords[0] * bbox.width(),
                bbox.y() + self.coords[1] * bbox.height(),
            ));
            let (x2, y2) = c.point((
                bbox.x() + self.coords[2] * bbox.width(),
                bbox.y() + self.coords[3] * bbox.height(),
            ));
            [
                x1,
                y1,
                x2,
                y2,
                c.px_to_pt(self.coords[4] * max),
                c.px_to_pt(self.coords[5] * max),
            ]
        } else {
            let (x1, y1) = c.point((self.coords[0], self.coords[1]));
            let (x2, y2) = c.point((self.coords[2], self.coords[3]));
            [x1, y1, x2, y2, c.px_to_pt(self.coords[4]), c.px_to_pt(self.coords[5])]
        };

        if self.shading_type == ShadingType::Axial {
            [coords[0], coords[1], coords[2], coords[3], 0.0, 0.0]
        } else {
            [coords[0], coords[1], coords[4], coords[2], coords[3], coords[5]]
        }
    }
}

// Applies transform to selected coordinates.
#[inline]
pub(crate) fn apply(tr: &Transform, p: impl Into<Point>) -> (f32, f32) {
    let Point { x, y } = p.into();
    let new_x = tr.sx * x + tr.kx * y + tr.tx;
    let new_y = tr.sy * y + tr.ky * x + tr.ty;
    (new_x, new_y)
}
