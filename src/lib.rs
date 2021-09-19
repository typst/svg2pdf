use image::io::Reader as ImageReader;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, Rgba};
use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::types::{ColorSpace, LineCapStyle, LineJoinStyle, MaskType, ShadingType};
use pdf_writer::writers::{
    ExponentialFunction, ExtGraphicsState, Image, Resources, ShadingPattern,
};
use pdf_writer::{Content, Filter, Finish, Name, PdfWriter, Rect, Ref, TextStr};
use std::collections::HashMap;
use usvg::{
    Align, AspectRatio, FillRule, ImageKind, LineCap, LineJoin, Node, NodeExt, NodeKind,
    Paint, PathSegment, Stop, Tree, ViewBox, Visibility,
};

pub struct Options {
    viewport: Option<(f64, f64)>,
    respect_native_size: bool,
    aspect_ratio: Option<usvg::AspectRatio>,
    dpi: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            viewport: None,
            respect_native_size: true,
            aspect_ratio: None,
            dpi: 96.0,
        }
    }
}

struct CoordToPdf {
    factor_x: f64,
    factor_y: f64,
    offset_x: f64,
    offset_y: f64,
    height_y: f64,
    dpi: f64,
}

impl CoordToPdf {
    fn new(
        viewport: (f64, f64),
        dpi: f64,
        viewbox: ViewBox,
        aspect_ratio: Option<usvg::AspectRatio>,
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
        }
    }

    /// Convert from x SVG source coordinates to PDF coordinates.
    fn x(&self, x: f64) -> f32 {
        self.px_to_pt(x * self.factor_x + self.offset_x)
    }

    /// Convert from x PDF coordinates to SVG source coordinates.
    fn svg_x(&self, x: f32) -> f64 {
        println!("factor {}, offset {}", self.factor_x, self.offset_x);
        (self.pt_to_px(x) - self.offset_x) / self.factor_x
    }

    /// Convert from y SVG source coordinates to PDF coordinates.
    fn y(&self, y: f64) -> f32 {
        self.px_to_pt(self.height_y - (y * self.factor_y + self.offset_y))
    }

    /// Convert from y PDF coordinates to SVG source coordinates.
    fn svg_y(&self, y: f32) -> f64 {
        (self.pt_to_px(y) - self.offset_y) / self.factor_y
    }

    fn px_to_pt(&self, px: f64) -> f32 {
        (px * 72.0 / self.dpi) as f32
    }

    fn pt_to_px(&self, pt: f32) -> f64 {
        pt as f64 * self.dpi / 72.0
    }
}

struct PendingPattern {
    id: String,
    num: u32,
    shading_type: ShadingType,
    bbox: usvg::Rect,
    coords: [f64; 6],
    transform_coords: bool,
}

struct PendingGS {
    num: u32,
    stroke_opacity: Option<f32>,
    fill_opacity: Option<f32>,
    soft_mask: Option<Ref>,
}

#[derive(Clone)]
struct PendingGroup {
    reference: Ref,
    bbox: Rect,
    matrix: Option<[f32; 6]>,
    initial_mask: Option<String>,
}

struct Context<'a> {
    tree: &'a Tree,
    bbox: &'a Rect,
    c: &'a CoordToPdf,
    function_map: HashMap<String, Ref>,
    next_id: &'a mut i32,
    next_pattern: u32,
    next_graphic: u32,
    next_xobject: u32,
    dpi: f64,
    pending_patterns: Vec<PendingPattern>,
    pending_graphics: Vec<PendingGS>,
    pending_xobjects: Vec<(u32, Ref)>,
    pending_groups: HashMap<String, PendingGroup>,
    checkpoints: Vec<[usize; 3]>,
    initial_mask: Option<String>,
}

impl<'a> Context<'a> {
    fn new(
        tree: &'a Tree,
        bbox: &'a Rect,
        c: &'a CoordToPdf,
        dpi: f64,
        next_id: &'a mut i32,
    ) -> Self {
        Self {
            tree,
            bbox,
            c,
            function_map: HashMap::new(),
            next_id,
            next_pattern: 0,
            next_graphic: 0,
            next_xobject: 0,
            dpi,
            pending_patterns: vec![],
            pending_graphics: vec![],
            pending_xobjects: vec![],
            pending_groups: HashMap::new(),
            checkpoints: vec![],
            initial_mask: None,
        }
    }

    fn push(&mut self) {
        self.checkpoints.push([
            self.pending_patterns.len(),
            self.pending_graphics.len(),
            self.pending_xobjects.len(),
        ]);
    }

    fn pop(&mut self, resources: &mut Resources) {
        let [patterns, graphics, xobjects] = self.checkpoints.pop().unwrap();

        let pending_patterns = self.pending_patterns.split_off(patterns);
        write_patterns(&pending_patterns, self.c, &self.function_map, resources);

        let pending_graphics = self.pending_graphics.split_off(graphics);
        write_graphics(&pending_graphics, resources);

        let pending_xobjects = self.pending_xobjects.split_off(xobjects);
        write_xobjects(&pending_xobjects, resources);
    }

    fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(*self.next_id);
        *self.next_id += 1;
        reference
    }

    fn alloc_pattern(&mut self) -> u32 {
        let num = self.next_pattern;
        self.next_pattern += 1;
        num
    }

    fn alloc_gs(&mut self) -> u32 {
        let num = self.next_graphic;
        self.next_graphic += 1;
        num
    }

    fn alloc_xobject(&mut self) -> u32 {
        let num = self.next_xobject;
        self.next_xobject += 1;
        num
    }
}

pub fn convert(svg: &str, opt: Options) -> Option<Vec<u8>> {
    let mut usvg_opts = usvg::Options::default();
    if let Some((width, height)) = opt.viewport {
        usvg_opts.default_size = usvg::Size::new(width, height)?;
    }
    let tree = Tree::from_str(svg, &usvg_opts.to_ref()).map_err(|e| dbg!(e)).ok()?;
    from_tree(&tree, opt)
}

pub fn from_tree(tree: &Tree, opt: Options) -> Option<Vec<u8>> {
    let native_size = tree.svg_node().size;
    let viewport = if let Some((width, height)) = opt.viewport {
        if opt.respect_native_size {
            (native_size.width(), native_size.height())
        } else {
            (width, height)
        }
    } else {
        (native_size.width(), native_size.height())
    };

    let c = CoordToPdf::new(
        viewport,
        opt.dpi,
        tree.svg_node().view_box,
        opt.aspect_ratio,
    );

    let mut writer = PdfWriter::new();
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let content_id = Ref::new(4);

    let mut next_id = 5;

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).kids([page_id]);

    dbg!(viewport);

    let bbox = dbg!(Rect::new(
        0.0,
        0.0,
        c.px_to_pt(viewport.0),
        c.px_to_pt(viewport.1)
    ));

    let mut ctx = Context::new(&tree, &bbox, &c, opt.dpi, &mut next_id);

    for element in tree.defs().children() {
        match *element.borrow() {
            NodeKind::LinearGradient(ref lg) => {
                let func_ref = ctx.alloc_ref();

                stops_to_function(&mut writer, func_ref, &lg.base.stops);
                ctx.function_map.insert(lg.id.clone(), func_ref);
            }
            NodeKind::RadialGradient(ref rg) => {
                let func_ref = ctx.alloc_ref();

                stops_to_function(&mut writer, func_ref, &rg.base.stops);
                ctx.function_map.insert(rg.id.clone(), func_ref);
            }
            _ => {}
        }
    }

    ctx.push();
    let content = content_stream(&tree.root(), &mut writer, &mut ctx);

    for (id, gp) in ctx.pending_groups.clone() {
        let mask_node = tree.defs_by_id(&id).unwrap();
        let borrowed = mask_node.borrow();
        if let NodeKind::Mask(_) = *borrowed {
            ctx.push();
            ctx.initial_mask = gp.initial_mask;
            let content = content_stream(&mask_node, &mut writer, &mut ctx);

            let mut group = writer.form_xobject(gp.reference, &content);
            group.bbox(gp.bbox);
            if let Some(matrix) = gp.matrix {
                group.matrix(matrix);
            }
            let mut resources = group.resources();
            ctx.pop(&mut resources);
            resources.finish();
            group
                .group()
                .transparency()
                .color_space(ColorSpace::DeviceRgb)
                .isolated(true);
        }
    }
    ctx.initial_mask = None;

    let mut page = writer.page(page_id);
    page.media_box(bbox);
    page.parent(page_tree_id);
    page.contents(content_id);

    let mut resources = page.resources();
    ctx.pop(&mut resources);
    resources.finish();

    page.finish();

    writer.stream(content_id, content);

    Some(writer.finish(catalog_id))
}

fn content_stream<'a>(
    node: &usvg::Node,
    writer: &mut PdfWriter,
    ctx: &mut Context<'a>,
) -> Vec<u8> {
    let mut content = Content::new();

    let num = ctx.alloc_gs();
    if let Some(id) = ctx.initial_mask.as_ref() {
        content.set_parameters(Name(format!("gs{}", num).as_bytes()));
        ctx.pending_graphics.push(PendingGS {
            stroke_opacity: None,
            fill_opacity: None,
            num,
            soft_mask: ctx.pending_groups.get(id).map(|g| g.reference),
        });
    }

    for element in node.children() {
        if &element == node {
            continue;
        }

        match *element.borrow() {
            NodeKind::Defs => {
                continue;
            }
            NodeKind::Path(ref path) => path.render(&element, writer, &mut content, ctx),
            NodeKind::Group(ref group) => {
                group.render(&element, writer, &mut content, ctx)
            }
            NodeKind::Image(ref image) => {
                image.render(&element, writer, &mut content, ctx)
            }

            _ => {}
        }
    }

    content.finish()
}

trait Render {
    fn render(
        &self,
        element: &Node,
        writer: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    );
}

impl Render for usvg::Path {
    fn render(
        &self,
        element: &Node,
        _: &mut PdfWriter,
        content: &mut Content,
        ctx: &mut Context,
    ) {
        if self.visibility != Visibility::Visible {
            return;
        }

        content.save_state();

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

        if stroke_opacity.unwrap_or(1.0) != 1.0 || fill_opacity.unwrap_or(1.0) != 1.0 {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));
            ctx.pending_graphics.push(PendingGS {
                stroke_opacity,
                fill_opacity,
                num,
                soft_mask: None,
            });
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
                    content.set_stroke_rgb(
                        c.red as f32 / 255.0,
                        c.green as f32 / 255.0,
                        c.blue as f32 / 255.0,
                    );
                }
                _ => todo!(),
            }
        }

        if let Some(fill) = &self.fill {
            match &fill.paint {
                Paint::Color(c) => {
                    content.set_fill_rgb(
                        c.red as f32 / 255.0,
                        c.green as f32 / 255.0,
                        c.blue as f32 / 255.0,
                    );
                }
                Paint::Link(id) => {
                    let item = ctx.tree.defs_by_id(id).unwrap();
                    match *item.borrow() {
                        NodeKind::LinearGradient(ref lg) => {
                            let num = ctx.alloc_pattern();
                            let name = format!("p{}", num);
                            ctx.pending_patterns.push(PendingPattern {
                                id: lg.id.clone(),
                                num,
                                shading_type: ShadingType::Axial,
                                bbox: element.calculate_bbox().unwrap_or_else(|| {
                                    usvg::Rect::new(0.0, 0.0, 0.0, 0.0).unwrap()
                                }),
                                coords: [lg.x1, lg.y1, lg.x2, lg.y2, 0.0, 0.0],
                                transform_coords: lg.base.units
                                    == usvg::Units::ObjectBoundingBox,
                            });


                            content.set_fill_color_space(ColorSpace::Pattern);
                            content.set_fill_pattern(None, Name(name.as_bytes()));
                        }
                        NodeKind::RadialGradient(ref rg) => {
                            let num = ctx.alloc_pattern();
                            let name = format!("p{}", num);
                            ctx.pending_patterns.push(PendingPattern {
                                id: rg.id.clone(),
                                num,
                                shading_type: ShadingType::Radial,
                                bbox: element.calculate_bbox().unwrap_or_else(|| {
                                    usvg::Rect::new(0.0, 0.0, 0.0, 0.0).unwrap()
                                }),
                                coords: [rg.fx, rg.fy, rg.cx, rg.cy, 0.0, rg.r.value()],
                                transform_coords: rg.base.units
                                    == usvg::Units::ObjectBoundingBox,
                            });

                            content.set_fill_color_space(ColorSpace::Pattern);
                            content.set_fill_pattern(None, Name(name.as_bytes()));
                        }
                        _ => todo!(),
                    };
                }
            }
        }

        draw_path(&self.data.0, content, ctx.c);

        match (self.fill.as_ref().map(|f| f.rule), self.stroke.is_some()) {
            (Some(FillRule::NonZero), true) => content.fill_and_stroke_nonzero(),
            (Some(FillRule::EvenOdd), true) => content.fill_and_stroke_even_odd(),
            (Some(FillRule::NonZero), false) => content.fill_nonzero(),
            (Some(FillRule::EvenOdd), false) => content.fill_even_odd(),
            (None, true) => content.stroke(),
            (None, false) => content,
        };

        content.restore_state();
    }
}

impl Render for usvg::Group {
    fn render(
        &self,
        element: &Node,
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

        let child_content = content_stream(&element, writer, ctx);

        let mut form = writer.form_xobject(group_ref, &child_content);

        let bbox = element
            .calculate_bbox()
            .unwrap_or_else(|| usvg::Rect::new(0.0, 0.0, 0.0, 0.0).unwrap());
        let pdf_bbox = Rect::new(0.0, 0.0, bbox.width() as f32, bbox.height() as f32);
        form.bbox(pdf_bbox);

        form.group()
            .transparency()
            .color_space(ColorSpace::DeviceRgb)
            .isolated(true)
            .knockout(false);

        let mut resources = form.resources();
        ctx.pop(&mut resources);

        let num = ctx.alloc_xobject();
        let name = format!("xo{}", num);
        content.save_state();

        apply_clip_path(self.clip_path.as_ref(), content, ctx);

        if let Some(reference) =
            apply_mask(self.mask.as_ref(), bbox, pdf_bbox, content, ctx)
        {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));
            ctx.pending_graphics.push(PendingGS {
                num,
                fill_opacity: None,
                stroke_opacity: None,
                soft_mask: Some(reference),
            });
        }

        if self.opacity.value() != 1.0 {
            let num = ctx.alloc_gs();
            content.set_parameters(Name(format!("gs{}", num).as_bytes()));

            ctx.pending_graphics.push(PendingGS {
                num,
                fill_opacity: Some(self.opacity.value() as f32),
                stroke_opacity: None,
                soft_mask: None,
            });
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

                    println!(
                        "viewbox {:?}, rwidth in pt {}, width {:?}, height {:?}",
                        self.view_box,
                        ctx.c.px_to_pt(rect.width()),
                        decoded.width(),
                        decoded.height()
                    );

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

                    if color.has_alpha() {
                        let mask_id = ctx.alloc_ref();
                        image.pair(Name(b"SMask"), mask_id);
                        drop(image);

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
                    let opt = Options {
                        viewport: Some((rect.width(), rect.height())),
                        respect_native_size: false,
                        aspect_ratio: Some(self.view_box.aspect),
                        dpi: ctx.dpi,
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
                            ctx.c.px_to_pt(rect.width()),
                            ctx.c.px_to_pt(rect.height()),
                        ))
                        .reference()
                        .page_no(0)
                        .file()
                        .description(TextStr("Embedded SVG image"))
                        .embedded_file(file_embedd_num);
                }
            }

            let image_ref = if let Some((width, height)) = raster_size {
                let mut content = Content::new();
                let xobj_name = Name(b"EmbRaster");
                let converter = CoordToPdf::new(
                    (rect.width(), rect.height()),
                    ctx.dpi,
                    ViewBox {
                        rect: usvg::Rect::new(0.0, 0.0, width as f64, height as f64)
                            .unwrap(),
                        aspect: AspectRatio::default(),
                    },
                    Some(self.view_box.aspect),
                );

                content.save_state();
                content.concat_matrix([
                    (width as f64 * converter.factor_x) as f32,
                    0.0,
                    0.0,
                    (height as f64 * converter.factor_y) as f32,
                    converter.offset_x as f32,
                    converter.offset_y as f32,
                ]);
                content.x_object(xobj_name);
                content.restore_state();


                let content = content.finish();
                let external_ref = ctx.alloc_ref();

                let mut xobject = writer.form_xobject(external_ref, &content);
                xobject.resources().x_objects().pair(xobj_name, image_ref);
                xobject.bbox(dbg!(Rect::new(
                    0.0,
                    0.0,
                    rect.width() as f32,
                    rect.height() as f32,
                )));
                let scaling = (72.0 / ctx.dpi) as f32;
                xobject.matrix([scaling, 0.0, 0.0, scaling, 0.0, 0.0]);

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

fn apply_clip_path(path_id: Option<&String>, content: &mut Content, ctx: &mut Context) {
    if let Some(clip_path) = path_id.and_then(|id| ctx.tree.defs_by_id(id)) {
        if let NodeKind::ClipPath(ref path) = *clip_path.borrow() {
            apply_clip_path(path.clip_path.as_ref(), content, ctx);

            for child in clip_path.children() {
                match *child.borrow() {
                    NodeKind::Path(ref path) => {
                        draw_path(&path.data.0, content, ctx.c);
                        content.clip_nonzero();
                        content.end_path();
                    }
                    NodeKind::ClipPath(_) => {}
                    _ => unreachable!(),
                }
            }
        } else {
            unreachable!();
        }
    }
}

fn apply_mask(
    mask_id: Option<&String>,
    bbox: usvg::Rect,
    pdf_bbox: Rect,
    content: &mut Content,
    ctx: &mut Context,
) -> Option<Ref> {
    if let Some(mask_node) = mask_id.and_then(|id| ctx.tree.defs_by_id(id)) {
        if let NodeKind::Mask(ref mask) = *mask_node.borrow() {
            let reference = ctx.alloc_ref();
            let (bbox, matrix) = if mask.content_units == usvg::Units::UserSpaceOnUse {
                (*ctx.bbox, None)
            } else {
                (
                    pdf_bbox,
                    Some([
                        1.0,
                        0.0,
                        0.0,
                        1.0,
                        bbox.x() as f32 + ctx.c.x(mask.rect.x()),
                        bbox.y() as f32 + ctx.c.y(mask.rect.y()),
                    ]),
                )
            };
            apply_mask(mask.mask.as_ref(), mask.rect, pdf_bbox, content, ctx);

            ctx.pending_groups.insert(mask.id.clone(), PendingGroup {
                reference,
                bbox,
                matrix,
                initial_mask: mask.mask.clone(),
            });

            Some(reference)
        } else {
            unreachable!()
        }
    } else {
        None
    }
}

fn draw_path(path_data: &[PathSegment], content: &mut Content, c: &CoordToPdf) {
    for &operation in path_data {
        match operation {
            PathSegment::MoveTo { x, y } => {
                content.move_to(c.x(x), c.y(y));
            }
            PathSegment::LineTo { x, y } => {
                content.line_to(c.x(x), c.y(y));
            }
            PathSegment::CurveTo { x1, y1, x2, y2, x, y } => {
                content.cubic_to(c.x(x1), c.y(y1), c.x(x2), c.y(y2), c.x(x), c.y(y));
            }
            PathSegment::ClosePath => {
                content.close_path();
            }
        }
    }
}

fn stops_to_function(writer: &mut PdfWriter, id: Ref, stops: &[Stop]) -> bool {
    if stops.is_empty() {
        return false;
    } else if stops.len() == 1 {
        let mut exp = writer.exponential_function(id);
        let stop = stops[0];

        exp.domain([0.0, 1.0]);
        exp.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        let color = [stop.color.red, stop.color.green, stop.color.blue]
            .iter()
            .map(|&x| x as f32 / 255.0)
            .collect::<Vec<_>>();
        exp.c0(color.iter().copied());
        exp.c1(color);
        exp.n(1.0);
        return true;
    }

    let mut stitching = writer.stitching_function(id);
    stitching.domain([0.0, 1.0]);
    stitching.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    let mut func_array = stitching.key(Name(b"Functions")).array();
    let mut bounds = Vec::new();
    let mut encode = Vec::with_capacity(2 * (stops.len() - 1));

    for window in stops.windows(2) {
        let (a, b) = (window[0], window[1]);
        bounds.push(b.offset.value() as f32);
        let mut exp = ExponentialFunction::new(func_array.obj());
        exp.domain([0.0, 1.0]);
        exp.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        exp.c0([a.color.red, a.color.green, a.color.blue]
            .iter()
            .map(|&x| x as f32 / 255.0));
        exp.c1([b.color.red, b.color.green, b.color.blue]
            .iter()
            .map(|&x| x as f32 / 255.0));
        exp.n(1.0);

        encode.extend([0.0, 1.0]);
    }

    func_array.finish();
    bounds.pop();
    stitching.bounds(bounds);
    stitching.encode(encode);

    true
}

fn write_patterns(
    pending_patterns: &[PendingPattern],
    c: &CoordToPdf,
    function_map: &HashMap<String, Ref>,
    resources: &mut Resources,
) {
    if pending_patterns.is_empty() {
        return;
    }

    let mut patterns = resources.key(Name(b"Pattern")).dict();

    for pending in pending_patterns.iter() {
        let name = format!("p{}", pending.num);
        let pattern_name = Name(name.as_bytes());
        let mut pattern = ShadingPattern::new(patterns.key(pattern_name));

        let mut shading = pattern.shading();
        shading.shading_type(pending.shading_type);
        shading.color_space(ColorSpace::DeviceRgb);

        let max = if pending.bbox.width() > pending.bbox.height() {
            pending.bbox.width()
        } else {
            pending.bbox.height()
        };

        let coords = if pending.transform_coords {
            [
                c.x(pending.bbox.x() + pending.coords[0] * pending.bbox.width()),
                c.y(pending.bbox.y() + pending.coords[1] * pending.bbox.height()),
                c.x(pending.bbox.x() + pending.coords[2] * pending.bbox.width()),
                c.y(pending.bbox.y() + pending.coords[3] * pending.bbox.height()),
                c.px_to_pt(pending.coords[4] * max),
                c.px_to_pt(pending.coords[5] * max),
            ]
        } else {
            [
                c.x(pending.coords[0]),
                c.y(pending.coords[1]),
                c.x(pending.coords[2]),
                c.y(pending.coords[3]),
                c.px_to_pt(pending.coords[4]),
                c.px_to_pt(pending.coords[5]),
            ]
        };

        if pending.shading_type == ShadingType::Axial {
            shading.coords(coords[0 .. 4].iter().copied());
        } else {
            shading.coords([
                coords[0], coords[1], coords[4], coords[2], coords[3], coords[5],
            ]);
        }
        shading.function(function_map[&pending.id]);
        shading.extend([true, true]);
        shading.finish();
    }

    patterns.finish();
}

fn write_graphics(pending_graphics: &[PendingGS], resources: &mut Resources) {
    if pending_graphics.is_empty() {
        return;
    }

    let mut ext_gs = resources.key(Name(b"ExtGState")).dict();
    for gs in pending_graphics {
        let mut ext_g =
            ExtGraphicsState::new(ext_gs.key(Name(format!("gs{}", gs.num).as_bytes())));

        if let Some(stroke_opacity) = gs.stroke_opacity {
            ext_g.stroking_alpha(stroke_opacity);
        }

        if let Some(fill_opacity) = gs.fill_opacity {
            ext_g.non_stroking_alpha(fill_opacity);
        }

        if let Some(smask_id) = gs.soft_mask {
            let mut soft_mask = ext_g.soft_mask();
            soft_mask.subtype(MaskType::Luminosity);
            soft_mask.group(smask_id);
            soft_mask.finish();
        }
    }
    ext_gs.finish();
}

fn write_xobjects(pending_xobjects: &[(u32, Ref)], resources: &mut Resources) {
    if pending_xobjects.is_empty() {
        return;
    }

    let mut xobjects = resources.x_objects();
    for (num, ref_id) in pending_xobjects {
        let name = format!("xo{}", num);
        xobjects.pair(Name(name.as_bytes()), *ref_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let doc = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 36 36"><path fill="#AF7E57" d="M15.898 34.875h-1.365s-1.27-3.348-1.074-5.736l2.801.549c.526 1.571-.362 5.187-.362 5.187zm4.144 0h1.365s1.27-3.348 1.074-5.736l-2.801.549c-.526 1.571.362 5.187.362 5.187z"/><path fill="#FA743E" d="M13.792 35.149c.102-.243.318-.694.341-.955.023-.261.034-.354.24-.426.347-.121 1.436-.087 1.623-.016.186.071.189.263.216.424s.114.424.227.64c.114.216.409.697.261 1.034-.062.14-.405.138-1.507.138s-1.417-.026-1.492-.118-.011-.477.091-.721zm8.356 0c-.102-.243-.318-.694-.341-.955-.023-.261-.034-.354-.24-.426-.347-.121-1.436-.087-1.623-.016-.187.071-.189.263-.216.424s-.114.424-.227.64-.409.697-.261 1.034c.062.14.405.138 1.507.138 1.102 0 1.417-.026 1.492-.118s.011-.477-.091-.721z"/><path fill="#AF7E57" d="M8.646 4.146c.011.127.871 1.535 1.323 2.635.452 1.1.71 2.198.948 2.448.476.499 2.092 1.879 3.117 3.343s.724 2.503.06 2.922c-.88.555-1.803.204-2.437-.788-.451-.706-1.951-3.209-2.739-4.601-.306-.541-.796-1.512-1.218-2.574-.397-1-.444-2.168-.504-2.312-.174-.425-1.12-.977-1.154-1.407 0 0 2.567-.095 2.604.334zm18.708 0c-.011.127-.871 1.535-1.323 2.635-.452 1.1-.71 2.198-.948 2.448-.476.499-2.092 1.879-3.117 3.343s-.724 2.503-.06 2.922c.88.555 1.803.204 2.437-.788.451-.706 1.951-3.209 2.739-4.601.304-.54.795-1.511 1.217-2.573.397-1 .444-2.168.504-2.312.175-.425 1.121-.977 1.155-1.406 0-.002-2.567-.097-2.604.332z"/><path fill="#99AAB5" d="M36 3.5c0 .276-.224.5-.5.5H.5C.224 4 0 3.776 0 3.5S.224 3 .5 3h35c.276 0 .5.224.5.5z"/><path fill="#292F33" d="M34 0c-.553 0-1 .448-1 1 0-.552-.447-1-1-1-.553 0-1 .448-1 1v5c0 .552.447 1 1 1 .553 0 1-.448 1-1 0 .552.447 1 1 1 .553 0 1-.448 1-1V1c0-.552-.447-1-1-1zM4 0c-.553 0-1 .448-1 1 0-.552-.447-1-1-1S1 .448 1 1v5c0 .552.447 1 1 1s1-.448 1-1c0 .552.447 1 1 1s1-.448 1-1V1c0-.552-.447-1-1-1z"/><path fill="#FA743E" d="M22.172 12.286c-.742.419-1.283 1.257-1.492 1.617-.665.117-1.151.3-1.151.3H16.47s-.486-.183-1.151-.3c-.209-.36-.75-1.197-1.492-1.617 0 0-2.121.937-2.746 1.521.21.537.492 1.047.716 1.417.4 1.092 1.502 2.527 1.626 2.777.389.78.741 4.5.741 4.5L18 24l3.835-1.5s.352-3.72.741-4.5c.125-.25 1.227-1.685 1.626-2.777.224-.37.506-.879.716-1.417-.625-.583-2.746-1.52-2.746-1.52z"/><g fill="#AF7E57"><path d="M27.096 4.363c-.33-.157-.476-.428-.377-.719l.003-.01c.099-.291.291-.346.738-.339 0 0 1.521.076 2.437.712 0 0-.456.304-.699.56v.01c-.185.541-1.894-.115-2.102-.214z"/><path d="M27.843 3.47c-.137-.033-.221-.173-.188-.31l.087-.357c.033-.137.173-.221.31-.188s.221.173.188.31l-.087.357c-.034.137-.173.221-.31.188zm-.445-.87c-.163-.055-.342.034-.397.197l-.279.837c-.055.164.168-.068.332-.013.163.055.342-.033.397-.196l.144-.427c.055-.164-.034-.342-.197-.398z"/><path d="M28.384 3.399c-.137-.033-.221-.173-.188-.31l.082-.334c.033-.137.173-.221.31-.188s.221.173.188.31l-.082.334c-.034.137-.173.221-.31.188z"/><path d="M28.9 3.424c-.137-.033-.221-.173-.188-.31l.068-.277c.033-.137.173-.221.31-.188s.221.173.188.31l-.068.277c-.034.137-.173.221-.31.188z"/><path d="M29.419 3.447c-.137-.033-.221-.173-.188-.31l.04-.162c.033-.137.173-.221.31-.188s.221.173.188.31l-.04.162c-.034.137-.174.222-.31.188z"/></g><g fill="#AF7E57"><path d="M8.904 4.363c.33-.157.476-.428.377-.719l-.003-.01c-.099-.29-.291-.346-.738-.339 0 0-1.521.076-2.437.712 0 0 .454.314.702.57.098.29 1.362.137 2.099-.214z"/><path d="M8.157 3.47c.137-.033.221-.173.188-.31l-.087-.357c-.033-.137-.173-.221-.31-.188-.137.034-.221.173-.188.31l.087.357c.034.137.173.221.31.188zm.445-.87c.163-.055.342.034.397.197l.279.837c.055.164-.168-.068-.332-.013-.163.055-.342-.033-.397-.196l-.144-.427c-.055-.164.034-.342.197-.398z"/><path d="M7.616 3.399c.137-.033.221-.173.188-.31l-.082-.334c-.033-.137-.173-.221-.31-.188-.136.034-.221.173-.187.31l.082.334c.033.137.172.221.309.188z"/><path d="M7.1 3.424c.137-.033.221-.173.188-.31l-.068-.277c-.033-.137-.172-.221-.309-.188s-.221.173-.188.31l.068.277c.033.137.172.221.309.188z"/><path d="M6.581 3.447c.137-.033.221-.173.188-.31l-.039-.162c-.034-.136-.173-.221-.31-.188-.137.034-.221.173-.188.31l.04.162c.033.137.173.222.309.188z"/></g><path fill="#DD551F" d="M21.84 24.5L18 23l-3.84 1.5s-.82 1.917-.96 5.188h3.06c.88-.979 1.2-3.604 1.2-3.604h1.08s.32 2.625 1.2 3.604h3.06c-.14-3.271-.96-5.188-.96-5.188z"/><path fill="#C1694F" d="M14.136 22.5h7.729v2h-7.729z"/><path fill="#14171A" d="M19 22.399h.65v2.209H19z"/><circle fill="#14171A" cx="18.5" cy="23.458" r=".167"/><path fill="#14171A" d="M18.021 23.458c0 .092-.075.167-.167.167-.092 0-.167-.075-.167-.167 0-.092.075-.167.167-.167.092.001.167.075.167.167z"/><circle fill="#14171A" cx="17.208" cy="23.458" r=".167"/><circle fill="#14171A" cx="16.562" cy="23.458" r=".167"/><circle fill="#14171A" cx="15.917" cy="23.458" r=".167"/><circle fill="#14171A" cx="15.271" cy="23.458" r=".167"/><circle fill="#14171A" cx="14.625" cy="23.458" r=".167"/><circle fill="#14171A" cx="17.208" cy="23.458" r=".167"/><circle fill="#657786" cx="16.553" cy="23.458" r=".148"/><path fill="#657786" d="M16.318 23.31h.235v.297h-.235z"/><path fill="#657786" d="M16.729 24.846h-.758v-2.697h.758v.35h-.409v1.997h.409z"/><path fill="#60352A" d="M19.591 3.84c-.823-.266-2.758-.194-3.242.436-1.258.024-2.734 1.161-2.928 2.661-.192 1.485.235 2.174.387 3.291.172 1.265.883 1.669 1.452 1.839.818 1.081 1.688 1.034 3.147 1.034 2.851 0 4.209-1.908 4.329-5.148.073-1.96-1.077-3.444-3.145-4.113z"/><path fill="#AF7E57" d="M16.778 12.286v2.444c0 .675.547 1.222 1.222 1.222.675 0 1.222-.547 1.222-1.222v-2.444h-2.444z"/><path fill="#AF7E57" d="M21.221 8.091c-.276-.382-.629-.69-1.403-.798.29.133.569.593.605.847.036.254.073.46-.157.206-.921-1.018-1.924-.617-2.917-1.239-.694-.434-.905-.915-.905-.915s-.085.641-1.137 1.294c-.305.189-.669.611-.871 1.234-.145.448-.1.847-.1 1.529 0 1.991 1.641 3.666 3.666 3.666s3.666-1.689 3.666-3.666c-.002-1.238-.132-1.722-.447-2.158z"/><path fill="#915A34" d="M19.629 12.286h-3.258s.407.814 1.629.814 1.629-.814 1.629-.814zm-1.222-1.019h-.815c-.113 0-.204-.091-.204-.204s.091-.204.204-.204h.815c.113 0 .204.091.204.204s-.091.204-.204.204z"/><path fill="#662113" d="M16.371 9.842c-.225 0-.407-.182-.407-.407v-.408c0-.225.182-.407.407-.407.225 0 .407.182.407.407v.407c0 .226-.182.408-.407.408zm3.258 0c-.225 0-.407-.182-.407-.407v-.408c0-.225.182-.407.407-.407.225 0 .407.182.407.407v.407c0 .226-.182.408-.407.408z"/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/hello.pdf", buf).unwrap();
    }

    #[test]
    fn test_gradient() {
        let doc = r##"<svg width="120" height="240" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="a" x1="0" x2="1" y1="0" y2="1"><stop offset="0%" stop-color="red"/><stop offset="50%" stop-opacity="0"/><stop offset="100%" stop-color="#00f"/></linearGradient></defs><rect x="10" y="120" rx="15" ry="15" width="100" height="100" fill="url(#a)"/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/gradient.pdf", buf).unwrap();
    }

    #[test]
    fn test_radial_gradient() {
        let doc = r##"<svg viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><defs><radialGradient id="a"><stop offset="10%" stop-color="gold"/><stop offset="95%" stop-color="red"/></radialGradient></defs><circle cx="5" cy="5" r="4" fill="url(#a)"/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/radial-gradient.pdf", buf).unwrap();
    }

    #[test]
    fn test_group() {
        let doc = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g opacity=".5"><circle fill="#00f" r="40"/><path fill="red" d="M0 0h80v60H0z"/></g></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/group.pdf", buf).unwrap();
    }

    #[test]
    fn test_clip_path() {
        let doc = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><clipPath id="myClip"><circle cx="40" cy="35" r="35"/></clipPath><path id="heart" d="M10,30 A20,20,0,0,1,50,30 A20,20,0,0,1,90,30 Q90,60,50,90 Q10,60,10,30 Z" clip-path="url(#myClip)"/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/clip-path.pdf", buf).unwrap();
    }

    #[test]
    fn test_mask() {
        let doc = r##"<svg viewBox="-10 -10 120 120" xmlns="http://www.w3.org/2000/svg"><mask id="a"><path fill="#fff" d="M0 0h100v100H0z"/><path d="M10 35a20 20 0 0 1 40 0 20 20 0 0 1 40 0q0 30-40 60-40-30-40-60Z"/></mask><path fill="orange" d="M-10 110h120V-10z"/><circle cx="50" cy="50" r="50" mask="url(#a)"/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/mask.pdf", buf).unwrap();
    }

    #[test]
    fn test_image() {
        let doc = r##"<svg xmlns="http://www.w3.org/2000/svg" width="5" height="5" viewBox="0 0 5 5" xmlns:xlink="http://www.w3.org/1999/xlink"><image width="5" height="5" xlink:href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAHElEQVQI12P4//8/w38GIAXDIBKE0DHxgljNBAAO9TXL0Y4OHwAAAABJRU5ErkJggg=="/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/image.pdf", buf).unwrap();
    }

    #[test]
    fn test_image_2() {
        let doc = r##"<svg xmlns="http://www.w3.org/2000/svg" width="5" height="5" viewBox="0 0 5 5" xmlns:xlink="http://www.w3.org/1999/xlink"><image width="5" height="5" preserveAspectRatio="xMaxYMid slice" xlink:href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAcAAAAFCAIAAAAG+GGPAAAACXBIWXMAAA7EAAAOxAGVKw4bAAAAeUlEQVQImQFuAJH/AJLg//3+/8rryv9BNu3t0f///3/b/wD/hRz9pX7h4QD+/v7i4gz09KTpjkcA8z81MSYf+vDpf9v//f3wHx8c/0E2AP+GHeXeTOXgHf/9/eHhAPmziP+FGwB/2//8/O32yIT/Qjf8vqH//v6C2Psy2Ep/9lvaZwAAAABJRU5ErkJggg=="/></svg>"##;
        let buf = convert(doc, Options::default()).unwrap();
        std::fs::write("target/image-2.pdf", buf).unwrap();
    }
}
