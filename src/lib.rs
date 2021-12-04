//! Convert SVG files to PDFs.

use std::collections::HashMap;

use pdf_writer::writers::{ExponentialFunction, FormXObject, Resources};
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, TextStr, Writer};
use usvg::{NodeExt, NodeKind, Stop, Tree};

mod defer;
mod render;
mod scale;

use defer::*;
use render::*;
pub use scale::*;

/// Set size and scaling preferences for the conversion.
pub struct Options {
    /// Specific dimensions the SVG can be forced to fill. This size will also
    /// be used if the SVG does not have a native size.
    pub viewport: Option<(f64, f64)>,
    /// Whether to respect the SVG's native size, even if the viewport is set.
    pub respect_native_size: bool,
    /// Override the scaling mode of the SVG within its viewport.
    pub aspect_ratio: Option<usvg::AspectRatio>,
    /// The Dots per Inch to assume for the conversion to PDF's printers points.
    /// Common values include `72.0` (1pt = 1px; Adobe and macOS) and `96.0`
    /// (Microsoft) for standard resolution screens and multiples of `300.0` for
    /// print quality.
    pub dpi: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            viewport: None,
            respect_native_size: true,
            aspect_ratio: None,
            dpi: 72.0,
        }
    }
}

/// Data is needed during the preparation of the file.
struct Context<'a> {
    /// The SVG tree.
    tree: &'a Tree,
    /// The bounding box of the PDF page.
    bbox: &'a Rect,
    /// Converter to the PDF coordinate system.
    c: CoordToPdf,
    /// References for functions for gradient color and alpha values.
    function_map: HashMap<String, (Ref, Option<Ref>)>,
    /// The next indirect reference id.
    next_id: i32,
    /// The next pattern id, to be used as e.g. `p1`.
    next_pattern: u32,
    /// The next graphics state id, to be used as e.g. `gs2`.
    next_graphic: u32,
    /// The next XObject id, to be used as e.g. `xo3`.
    next_xobject: u32,
    /// The next shading id, to be used as e.g. `sh5`.
    next_shading: u32,
    /// Patterns which have been used but not yet written to the file.
    pending_gradients: Vec<PendingGradient>,
    /// Patterns which have been used but not yet written to the file.
    pending_patterns: Vec<(u32, Ref)>,
    /// Graphics states which have been used but not yet written to the file.
    pending_graphics: Vec<PendingGS>,
    /// XObjects that have been both written as indirect objects and referenced
    /// but still need to be registered with the `Resources` dictionary.
    pending_xobjects: Vec<(u32, Ref)>,
    /// IDs of nodes which need to be written to the root of the document as a
    /// transparency group along with their metadata.
    pending_groups: HashMap<String, PendingGroup>,
    /// This array stores the lengths of the pending vectors and allows to push
    /// each of their elements onto the closes `Resources` dictionary.
    checkpoints: Vec<[usize; 4]>,
    /// The mask that needs to be applied at the start of a path drawing
    /// operation.
    initial_mask: Option<String>,
}

impl<'a> Context<'a> {
    /// Create a new context.
    fn new(tree: &'a Tree, bbox: &'a Rect, c: CoordToPdf) -> Self {
        Self {
            tree,
            bbox,
            c,
            function_map: HashMap::new(),
            next_id: 1,
            next_pattern: 0,
            next_graphic: 0,
            next_xobject: 0,
            next_shading: 0,
            pending_gradients: vec![],
            pending_patterns: vec![],
            pending_graphics: vec![],
            pending_xobjects: vec![],
            pending_groups: HashMap::new(),
            checkpoints: vec![],
            initial_mask: None,
        }
    }

    /// Push a new context frame for the pending objects.
    fn push(&mut self) {
        self.checkpoints.push([
            self.pending_gradients.len(),
            self.pending_patterns.len(),
            self.pending_graphics.len(),
            self.pending_xobjects.len(),
        ]);
    }

    /// Pop a context frame and write all pending objects onto an `Resources`
    /// dictionary.
    fn pop(&mut self, resources: &mut Resources) {
        let [gradients, patterns, graphics, xobjects] = self.checkpoints.pop().unwrap();

        let pending_gradients = self.pending_gradients.split_off(gradients);
        let pending_patterns = self.pending_patterns.split_off(patterns);
        write_gradients(
            &pending_gradients,
            &pending_patterns,
            &self.function_map,
            resources,
        );

        let pending_graphics = self.pending_graphics.split_off(graphics);
        write_graphics(&pending_graphics, resources);

        let pending_xobjects = self.pending_xobjects.split_off(xobjects);
        write_xobjects(&pending_xobjects, resources);
    }

    /// Allocate a new indirect reference id.
    fn alloc_ref(&mut self) -> Ref {
        let reference = Ref::new(self.next_id);
        self.next_id += 1;
        reference
    }

    /// Allocate a new pattern id.
    fn alloc_pattern(&mut self) -> u32 {
        let num = self.next_pattern;
        self.next_pattern += 1;
        num
    }

    /// Allocate a new graphics state id.
    fn alloc_gs(&mut self) -> u32 {
        let num = self.next_graphic;
        self.next_graphic += 1;
        num
    }

    /// Allocate a new XObject id.
    fn alloc_xobject(&mut self) -> u32 {
        let num = self.next_xobject;
        self.next_xobject += 1;
        num
    }

    /// Allocate a new shading id.
    fn alloc_shading(&mut self) -> u32 {
        let num = self.next_shading;
        self.next_shading += 1;
        num
    }
}

/// Convert an SVG source string to a PDF buffer.
pub fn convert(svg: &str, opt: Options) -> Option<Vec<u8>> {
    let mut usvg_opts = usvg::Options::default();
    if let Some((width, height)) = opt.viewport {
        usvg_opts.default_size = usvg::Size::new(width, height)?;
    }
    let tree = Tree::from_str(svg, &usvg_opts.to_ref()).map_err(|e| dbg!(e)).ok()?;
    from_tree(&tree, opt)
}

/// Convert an usvg source tree to a PDF buffer.
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

    let bbox = Rect::new(0.0, 0.0, c.px_to_pt(viewport.0), c.px_to_pt(viewport.1));
    let mut ctx = Context::new(&tree, &bbox, c);

    let mut writer = PdfWriter::new();
    let catalog_id = ctx.alloc_ref();
    let page_tree_id = ctx.alloc_ref();
    let page_id = ctx.alloc_ref();
    let content_id = ctx.alloc_ref();

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).count(1).kids([page_id]);

    for element in tree.defs().children() {
        match *element.borrow() {
            NodeKind::LinearGradient(ref lg) => {
                register_functions(&mut writer, &mut ctx, &lg.id, &lg.base.stops);
            }
            NodeKind::RadialGradient(ref rg) => {
                register_functions(&mut writer, &mut ctx, &rg.id, &rg.base.stops);
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

            let mut group =
                form_xobject(&mut writer, gp.reference, &content, gp.bbox, true);

            if let Some(matrix) = gp.matrix {
                group.matrix(matrix);
            }

            let mut resources = group.resources();
            ctx.pop(&mut resources);
            resources.finish();
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

    writer.stream(content_id, &content);
    writer.document_info(ctx.alloc_ref()).producer(TextStr("svg2pdf"));

    Some(writer.finish())
}

/// Write a content stream for a node.
fn content_stream<'a>(
    node: &usvg::Node,
    writer: &mut PdfWriter,
    ctx: &mut Context<'a>,
) -> Vec<u8> {
    let mut content = Content::new();
    let num = ctx.alloc_gs();

    if let Some(reference) = ctx
        .initial_mask
        .as_ref()
        .and_then(|id| ctx.pending_groups.get(id).map(|g| g.reference))
    {
        content.set_parameters(Name(format!("gs{}", num).as_bytes()));
        ctx.pending_graphics.push(PendingGS::soft_mask(reference, num));
    }

    for element in node.children() {
        if &element == node {
            continue;
        }

        match *element.borrow() {
            NodeKind::Defs => continue,
            NodeKind::Path(ref path) => {
                path.render(&element, writer, &mut content, ctx);
            }
            NodeKind::Group(ref group) => {
                group.render(&element, writer, &mut content, ctx);
            }
            NodeKind::Image(ref image) => {
                image.render(&element, writer, &mut content, ctx);
            }
            _ => {}
        }
    }

    content.finish()
}

/// Draw a clipping path into a content stream.
fn apply_clip_path(path_id: Option<&String>, content: &mut Content, ctx: &mut Context) {
    if let Some(clip_path) = path_id.and_then(|id| ctx.tree.defs_by_id(id)) {
        if let NodeKind::ClipPath(ref path) = *clip_path.borrow() {
            apply_clip_path(path.clip_path.as_ref(), content, ctx);

            for child in clip_path.children() {
                match *child.borrow() {
                    NodeKind::Path(ref path) => {
                        draw_path(&path.data.0, path.transform, content, &ctx.c);
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

/// Prepare a mask to be written to the file. This will calculate the metadata
/// and create a `pending_group`.
fn apply_mask(
    mask_id: Option<&String>,
    bbox: usvg::Rect,
    pdf_bbox: Rect,
    ctx: &mut Context,
) -> Option<Ref> {
    if let Some(mask_node) = mask_id.and_then(|id| ctx.tree.defs_by_id(id)) {
        if let NodeKind::Mask(ref mask) = *mask_node.borrow() {
            let reference = ctx.alloc_ref();
            let (bbox, matrix) = if mask.content_units == usvg::Units::UserSpaceOnUse {
                (*ctx.bbox, None)
            } else {
                let point = mask_node.transform().apply(mask.rect.x(), mask.rect.y());
                let (x, y) = ctx.c.point(point);
                let transform =
                    [1.0, 0.0, 0.0, 1.0, bbox.x() as f32 + x, bbox.y() as f32 + y];
                (pdf_bbox, Some(transform))
            };

            apply_mask(mask.mask.as_ref(), mask.rect, pdf_bbox, ctx);

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

/// A color helper function that stores colors with values between 0.0 and 1.0.
#[derive(Debug, Clone, Copy)]
struct RgbaColor {
    /// Red.
    r: f32,
    /// Green.
    g: f32,
    /// Blue.
    b: f32,
    /// Alpha.
    a: f32,
}

impl RgbaColor {
    /// Create a new color.
    fn new(r: f32, g: f32, b: f32, a: f32) -> RgbaColor {
        RgbaColor { r, g, b, a }
    }

    /// Create a new color from u8 color components between 0.0 and 255.0.
    fn from_u8(r: u8, g: u8, b: u8, a: u8) -> RgbaColor {
        RgbaColor::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Create a RGB array for use in PDF.
    fn to_array(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }
}

impl From<usvg::Color> for RgbaColor {
    fn from(color: usvg::Color) -> Self {
        Self::from_u8(color.red, color.green, color.blue, color.alpha)
    }
}

/// Write the functions for a gradient with its stops. Also registers them with
/// the context and can create an alpha gradient function.
fn register_functions(
    writer: &mut PdfWriter,
    ctx: &mut Context,
    id: &str,
    stops: &[Stop],
) {
    let func_ref = ctx.alloc_ref();
    stops_to_function(writer, func_ref, stops, false);

    let alpha_ref = if stops
        .iter()
        .any(|stop| stop.opacity.value() < 1.0 || stop.color.alpha < 255)
    {
        let stops = stops
            .iter()
            .cloned()
            .map(|mut stop| {
                stop.color.alpha = (stop.color.alpha as f64 * stop.opacity.value()) as u8;
                stop
            })
            .collect::<Vec<_>>();

        let alpha_ref = ctx.alloc_ref();
        stops_to_function(writer, alpha_ref, &stops, true);

        Some(alpha_ref)
    } else {
        None
    };

    ctx.function_map.insert(id.to_string(), (func_ref, alpha_ref));
}

/// Convert a list of stops to a function and write it.
fn stops_to_function(
    writer: &mut PdfWriter,
    id: Ref,
    stops: &[Stop],
    alpha: bool,
) -> bool {
    let range =
        IntoIterator::into_iter([0.0f32, 1.0f32])
            .cycle()
            .take(if alpha { 2 } else { 6 });

    let set_colors =
        |exp: &mut ExponentialFunction, a_color: RgbaColor, b_color: RgbaColor| {
            if alpha {
                exp.c0([a_color.a]);
                exp.c1([b_color.a]);
            } else {
                exp.c0(a_color.to_array());
                exp.c1(b_color.to_array());
            }
        };

    if stops.is_empty() {
        return false;
    } else if stops.len() == 1 {
        let mut exp = writer.exponential_function(id);
        let stop = stops[0];
        let color = RgbaColor::from(stop.color);

        exp.domain([0.0, 1.0]);
        exp.range(range);
        set_colors(&mut exp, color, color);

        exp.n(1.0);
        return true;
    }

    let mut stitching = writer.stitching_function(id);
    stitching.domain([0.0, 1.0]);
    stitching.range(range.clone());

    let mut func_array = stitching.insert(Name(b"Functions")).array();
    let mut bounds = Vec::new();
    let mut encode = Vec::with_capacity(2 * (stops.len() - 1));

    let stops = if stops[0].offset.value() != 0.0 {
        let mut appended = stops[0].clone();
        appended.offset = usvg::StopOffset::new(0.0);

        let mut res = vec![appended];
        res.extend_from_slice(stops);
        res
    } else {
        stops.to_vec()
    };

    for window in stops.windows(2) {
        let (a, b) = (window[0], window[1]);
        let (a_color, b_color) = (RgbaColor::from(a.color), RgbaColor::from(b.color));
        bounds.push(b.offset.value() as f32);
        let mut exp = ExponentialFunction::start(func_array.push());
        exp.domain([0.0, 1.0]);
        exp.range(range.clone());
        set_colors(&mut exp, a_color, b_color);

        exp.n(1.0);

        encode.extend([0.0, 1.0]);
    }

    func_array.finish();
    bounds.pop();
    stitching.bounds(bounds);
    stitching.encode(encode);

    true
}

/// Create and return the writer for an transparency group form XObject.
fn form_xobject<'a>(
    writer: &'a mut PdfWriter,
    reference: Ref,
    content: &'a [u8],
    bbox: Rect,
    has_color: bool,
) -> FormXObject<'a> {
    let mut form = writer.form_xobject(reference, content);
    form.bbox(bbox);

    let mut group = form.group();
    group.transparency();
    group.isolated(true);
    group.knockout(false);

    let space = group.color_space();
    if has_color {
        space.device_rgb();
    } else {
        space.device_gray();
    }

    group.finish();
    form
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn files() {
        let paths = fs::read_dir("tests").unwrap();
        for path in paths {
            let path = path.unwrap();
            let base_name = path.file_name().to_string_lossy().to_string();

            println!("{}", base_name);

            let doc = fs::read_to_string(path.path()).unwrap();
            let mut options = Options::default();
            options.dpi = 72.0;
            let buf = convert(&doc, options).unwrap();

            let len = base_name.len();
            let file_name = format!("{}.pdf", &base_name[0 .. len - 4]);

            std::fs::write(format!("target/{}", file_name), buf).unwrap();
        }
    }
}
