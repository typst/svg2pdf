/*! Convert SVG files to PDFs.

This crate allows to convert static (i.e. non-interactive) SVG files to
either standalone PDF files or Form XObjects that can be embedded in another
PDF file and used just like images.

The conversion will translate the SVG content to PDF without rasterizing it,
so no quality is lost.

## Example
This example reads an SVG file and writes the corresponding PDF back to the disk.

```rust
let svg = std::fs::read_to_string("tests/example.svg").unwrap();

// This can only fail if the SVG is malformed. This one is not.
let pdf = svg2pdf::convert_str(&svg, svg2pdf::Options::default()).unwrap();

// ... and now you have a Vec<u8> which you could write to a file or
// transmit over the network!
std::fs::write("target/example.pdf", pdf).unwrap();
```

## Supported features
- Path drawing with fills and strokes
- Gradients
- Patterns
- Clip paths
- Masks
- Transformation matrices
- Respecting the `keepAspectRatio` attribute
- Raster images and nested SVGs

Filters are not currently supported and embedded raster images are not color
managed. Instead, they use PDF's `DeviceRGB` color space.
*/

use std::collections::HashMap;

use pdf_writer::types::ProcSet;
use pdf_writer::writers::{ColorSpace, ExponentialFunction, FormXObject, Resources};
use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, TextStr, Writer};
use usvg::{NodeExt, NodeKind, Stop, Tree};

mod defer;
mod render;
mod scale;

use defer::*;
use render::*;
use scale::*;

const SRGB: Name = Name(b"srgb");

/// Set size and scaling preferences for the conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Specific dimensions the SVG will be forced to fill in nominal SVG
    /// pixels. If this is `Some`, the resulting PDF will always have the
    /// corresponding size converted to PostScript points according to `dpi`. If
    /// it is `None`, the PDF will either take on the native size of the SVG or
    /// 100 by 100 if no native size was specified (i.e. there is no `viewBox`,
    /// no `width`, and no `height` attribute).
    ///
    /// Normally, unsized SVGs will take on the size of the target viewport. In
    /// order to achieve the behavior in which your SVG will take its native
    /// size and the size of your viewport only if it has no native size, you
    /// need to create a usvg [`Tree`] for your file in your own code. You will
    /// then need to set the `default_size` field of the [`usvg::Options`]
    /// struct to your viewport size and set this field according to
    /// `tree.svg_node().size`.
    ///
    /// _Default:_ `None`.
    pub viewport: Option<(f64, f64)>,
    /// Override the scaling mode of the SVG within its viewport. Look
    /// [here][aspect] to learn about the different possible modes.
    ///
    /// _Default:_ `None`.
    ///
    /// [aspect]: https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/preserveAspectRatio
    pub aspect: Option<usvg::AspectRatio>,
    /// The dots per inch to assume for the conversion to PDF's printer's
    /// points. Common values include `72.0` (1pt = 1px; Adobe and macOS) and
    /// `96.0` (Microsoft) for standard resolution screens and multiples of
    /// `300.0` for print quality.
    ///
    /// This, of course, does not change the output quality (except for very
    /// high values, where precision might degrade due to floating point
    /// errors). Instead, it sets what the physical dimensions of one nominal
    /// pixel should be on paper when printed without scaling.
    ///
    /// _Default:_ `72.0`.
    pub dpi: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options { viewport: None, aspect: None, dpi: 72.0 }
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
        resources.color_spaces().insert(SRGB).start::<ColorSpace>().srgb();
        resources.proc_sets([ProcSet::Pdf, ProcSet::ImageColor, ProcSet::ImageGrayscale]);

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

/// Convert an SVG source string to a standalone PDF buffer.
///
/// Returns an error if the SVG string is malformed.
pub fn convert_str(src: &str, options: Options) -> Result<Vec<u8>, usvg::Error> {
    let mut usvg_opts = usvg::Options::default();
    if let Some((width, height)) = options.viewport {
        usvg_opts.default_size =
            usvg::Size::new(width.max(1.0), height.max(1.0)).unwrap();
    }
    let tree = Tree::from_str(src, &usvg_opts.to_ref())?;
    Ok(convert_tree(&tree, options))
}

/// Convert a [`usvg` tree](Tree) to a standalone PDF buffer.
pub fn convert_tree(tree: &Tree, options: Options) -> Vec<u8> {
    let (c, bbox) = get_sizings(tree, &options);
    let mut ctx = Context::new(&tree, &bbox, c);

    let mut writer = PdfWriter::new();
    let catalog_id = ctx.alloc_ref();
    let page_tree_id = ctx.alloc_ref();
    let page_id = ctx.alloc_ref();
    let content_id = ctx.alloc_ref();

    writer.catalog(catalog_id).pages(page_tree_id);
    writer.pages(page_tree_id).count(1).kids([page_id]);

    preregister(tree, &mut writer, &mut ctx);

    ctx.push();
    let content = content_stream(&tree.root(), &mut writer, &mut ctx);

    write_masks(tree, &mut writer, &mut ctx);

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

    writer.finish()
}

/// Convert a [`usvg` tree](Tree) into a Form XObject that can be used as part
/// of a larger document.
///
/// This method is intended for use in an existing [`PdfWriter`] workflow. It
/// will always return an XObject with the width and height of one printer's
/// point, just like an [`ImageXObject`](pdf_writer::writers::ImageXObject)
/// would.
///
/// The resulting object can be used by registering a name and the `id` with a
/// page's [`/XObject`](pdf_writer::writers::Resources::x_objects) resources
/// dictionary and then invoking the [`Do`](pdf_writer::Content::x_object)
/// operator with the name in the page's content stream.
///
/// As the conversion process may need to create multiple indirect objects in
/// the PDF, this function allocates consecutive IDs starting at `id` for its
/// objects and returns the next available ID for your future writing.
///
/// ## Example
/// Write a PDF file with some text and an SVG graphic.
///
/// ```rust
/// use svg2pdf;
/// use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, Str};
///
/// // Allocate the indirect reference IDs and names.
/// let catalog_id = Ref::new(1);
/// let page_tree_id = Ref::new(2);
/// let page_id = Ref::new(3);
/// let font_id = Ref::new(4);
/// let content_id = Ref::new(5);
/// let svg_id = Ref::new(6);
/// let font_name = Name(b"F1");
/// let svg_name = Name(b"S1");
///
/// // Start writing a PDF.
/// let mut writer = PdfWriter::new();
/// writer.catalog(catalog_id).pages(page_tree_id);
/// writer.pages(page_tree_id).kids([page_id]).count(1);
///
/// // Set up a simple A4 page.
/// let mut page = writer.page(page_id);
/// page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
/// page.parent(page_tree_id);
/// page.contents(content_id);
///
/// // Add the font and, more importantly, the SVG to the resource dictionary
/// // so that it can be referenced in the content stream.
/// let mut resources = page.resources();
/// resources.x_objects().pair(svg_name, svg_id);
/// resources.fonts().pair(font_name, font_id);
/// resources.finish();
/// page.finish();
///
/// // Set a predefined font, so we do not have to load anything extra.
/// writer.type1_font(font_id).base_font(Name(b"Helvetica"));
///
/// // Let's add an SVG graphic to this file.
/// // We need to load its source first and manually parse it into a usvg Tree.
/// let svg = std::fs::read_to_string("tests/example.svg").unwrap();
/// let tree = usvg::Tree::from_str(&svg, &usvg::Options::default().to_ref()).unwrap();
///
/// // Then, we will write it to the page as the 6th indirect object.
/// //
/// // This call allocates some indirect object reference IDs for itself. If we
/// // wanted to write some more indirect objects afterwards, we could use the
/// // return value as the next unused reference ID.
/// svg2pdf::convert_tree_into(&tree, svg2pdf::Options::default(), &mut writer, svg_id);
///
/// // Write a content stream with some text and our SVG.
/// let mut content = Content::new();
/// content
///     .begin_text()
///     .set_font(font_name, 16.0)
///     .next_line(108.0, 734.0)
///     .show(Str(b"Look at my wonderful vector graphic!"))
///     .end_text();
///
/// // Add our graphic.
/// content
///     .transform([300.0, 0.0, 0.0, 300.0, 147.5, 385.0])
///     .x_object(svg_name);
///
/// // Write the file to the disk.
/// writer.stream(content_id, &content.finish());
/// std::fs::write("target/embedded.pdf", writer.finish()).unwrap();
/// ```
pub fn convert_tree_into(
    tree: &Tree,
    options: Options,
    writer: &mut PdfWriter,
    id: Ref,
) -> Ref {
    let (c, bbox) = get_sizings(tree, &options);
    let mut ctx = Context::new(&tree, &bbox, c);

    ctx.next_id = id.get() + 1;

    preregister(tree, writer, &mut ctx);

    ctx.push();
    let content = content_stream(&tree.root(), writer, &mut ctx);

    write_masks(tree, writer, &mut ctx);

    let mut xobject = writer.form_xobject(id, &content);
    xobject.bbox(bbox);
    xobject.matrix([
        1.0 / (bbox.x2 - bbox.x1),
        0.0,
        0.0,
        1.0 / (bbox.y2 - bbox.y1),
        0.0,
        0.0,
    ]);

    let mut resources = xobject.resources();
    ctx.pop(&mut resources);

    ctx.alloc_ref()
}

/// Calculates the bounding box and size conversions for an usvg tree.
fn get_sizings(tree: &Tree, options: &Options) -> (CoordToPdf, Rect) {
    let native_size = tree.svg_node().size;
    let viewport = if let Some((width, height)) = options.viewport {
        (width, height)
    } else {
        (native_size.width(), native_size.height())
    };

    let c = CoordToPdf::new(
        viewport,
        options.dpi,
        tree.svg_node().view_box,
        options.aspect,
    );

    (
        c,
        Rect::new(0.0, 0.0, c.px_to_pt(viewport.0), c.px_to_pt(viewport.1)),
    )
}

fn preregister(tree: &Tree, writer: &mut PdfWriter, ctx: &mut Context) {
    for element in tree.defs().children() {
        match *element.borrow() {
            NodeKind::LinearGradient(ref lg) => {
                register_functions(writer, ctx, &lg.id, &lg.base.stops);
            }
            NodeKind::RadialGradient(ref rg) => {
                register_functions(writer, ctx, &rg.id, &rg.base.stops);
            }
            _ => {}
        }
    }
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
        space.srgb();
    } else {
        space.srgb_gray();
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
            let buf = convert_str(&doc, options).unwrap();

            let len = base_name.len();
            let file_name = format!("{}.pdf", &base_name[0 .. len - 4]);

            std::fs::write(format!("target/{}", file_name), buf).unwrap();
        }
    }
}
