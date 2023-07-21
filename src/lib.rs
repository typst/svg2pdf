/*! Convert SVG files to PDFs.

This crate allows to convert static (i.e. non-interactive) SVG files to
either standalone PDF files or Form XObjects that can be embedded in another
PDF file and used just like images.

The conversion will translate the SVG content to PDF without rasterizing them,
so no quality is lost.

## Example
This example reads an SVG file and writes the corresponding PDF back to the disk.

```
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let path = "tests/svgs/custom/integration/matplotlib/time_series.svg";
let svg = std::fs::read_to_string(path)?;

// This can only fail if the SVG is malformed. This one is not.
let pdf = svg2pdf::convert_str(&svg, svg2pdf::Options::default())?;

// ... and now you have a Vec<u8> which you could write to a file or
// transmit over the network!
std::fs::write("target/time_series.pdf", pdf)?;
# Ok(()) }
```

## Supported features
In general, a large part of the SVG specification is supported, including features like:
- Path drawing with fills and strokes
- Gradients
- Patterns
- Clip paths
- Masks
- Transformation matrices
- Respecting the `keepAspectRatio` attribute
- Raster images and nested SVGs

## Unsupported features
Among the unsupported features are currently:
- The `spreadMethod` attribute of gradients
- Filters
- Blend modes
- Raster images are not color managed but use PDF's DeviceRGB color space
- A number of features that were added in SVG2
*/

mod render;
mod util;

use pdf_writer::{Content, Filter, Finish, PdfWriter, Rect, Ref, TextStr};
use usvg::utils::view_box_to_transform;
use usvg::{Align, AspectRatio, Size, Transform, Tree, TreeParsing};

use crate::util::context::Context;
use crate::util::helper::{dpi_ratio, NameExt, RectExt};

/// Set size and scaling preferences for the conversion.
#[derive(Copy, Clone)]
pub struct Options {
    /// Specific dimensions the SVG will be forced to fill in nominal SVG
    /// pixels. If this is `Some`, the resulting PDF will always have the
    /// corresponding size converted to PostScript points according to `dpi`. If
    /// it is `None`, the PDF will take on the native size of the SVG.
    ///
    /// Normally, unsized SVGs will take on the size of the target viewport. In
    /// order to achieve the behavior in which your SVG will take its native
    /// size and the size of your viewport only if it has no native size, you
    /// need to create a [`usvg` tree](usvg::Tree) for your file in your own
    /// code. You will then need to set the `default_size` field of the
    /// [`usvg::Options`] struct to your viewport size and set this field
    /// according to `tree.svg_node().size`.
    ///
    /// _Default:_ `None`.
    pub viewport: Option<Size>,

    /// Override the scaling mode of the SVG within its viewport. Look
    /// [here][aspect] to learn about the different possible modes.
    ///
    /// _Default:_ `None`.
    ///
    /// [aspect]: https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/preserveAspectRatio
    pub aspect: Option<AspectRatio>,

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
    pub dpi: f32,

    /// Whether the content streams should be compressed.
    ///
    /// The smaller PDFs generated by this are generally more practical but it
    /// increases runtime a bit.
    ///
    /// _Default:_ `true`.
    pub compress: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            dpi: 72.0,
            viewport: None,
            aspect: None,
            compress: true,
        }
    }
}

/// Convert an SVG source string to a standalone PDF buffer.
///
/// Does not load any fonts and consequently cannot convert `text` elements. To
/// convert text, you should convert your source string to a
/// [`usvg` tree](usvg::Tree) manually,
/// [convert text with usvg](usvg::TreeTextToPath::convert_text) and then use
/// [`convert_tree`].
///
/// Returns an error if the SVG string is malformed.
pub fn convert_str(src: &str, options: Options) -> Result<Vec<u8>, usvg::Error> {
    let mut usvg_options = usvg::Options::default();
    if let Some(size) = options.viewport {
        usvg_options.default_size = size;
    }
    let tree = Tree::from_str(src, &usvg_options)?;
    Ok(convert_tree(&tree, options))
}

/// Convert a [`usvg` tree](usvg::Tree) into a standalone PDF buffer.
///
/// ## Example
/// The example below reads an SVG file, processes text within it, then converts
/// it into a PDF and finally writes it back to the file system.
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use usvg::{fontdb, TreeParsing, TreeTextToPath};
/// use svg2pdf::Options;
///
/// let input = "tests/svgs/custom/integration/matplotlib/step.svg";
/// let output = "target/step.pdf";
///
/// let svg = std::fs::read_to_string(input)?;
/// let options = usvg::Options::default();
/// let mut tree = usvg::Tree::from_str(&svg, &options)?;
///
/// let mut db = fontdb::Database::new();
/// db.load_system_fonts();
/// tree.convert_text(&db);
///
/// let pdf = svg2pdf::convert_tree(&tree, Options::default());
/// std::fs::write(output, pdf)?;
/// # Ok(()) }
/// ```
pub fn convert_tree(tree: &Tree, options: Options) -> Vec<u8> {
    let page_size = options.viewport.unwrap_or(tree.size);
    let mut ctx =
        Context::new(tree, options, initial_transform(&options, tree, page_size), None);
    let mut writer = PdfWriter::new();

    let catalog_ref = ctx.alloc_ref();
    let page_tree_ref = ctx.alloc_ref();
    let page_ref = ctx.alloc_ref();
    let content_ref = ctx.alloc_ref();

    writer.catalog(catalog_ref).pages(page_tree_ref);
    writer.pages(page_tree_ref).count(1).kids([page_ref]);

    // Generate main content
    ctx.deferrer.push();
    let tree_x_object = render::tree_to_x_object(tree, &mut writer, &mut ctx);
    let mut content = Content::new();
    content.x_object(tree_x_object.as_name());

    let content_stream = ctx.finish_content(content);
    let mut stream = writer.stream(content_ref, &content_stream);

    if ctx.options.compress {
        stream.filter(Filter::FlateDecode);
    }

    stream.finish();

    let mut page = writer.page(page_ref);
    let mut page_resources = page.resources();
    ctx.deferrer.pop(&mut page_resources);
    page_resources.finish();

    page.media_box(Rect::new(
        0.0,
        0.0,
        dpi_ratio(options.dpi) * page_size.width() as f32,
        dpi_ratio(options.dpi) * page_size.height() as f32,
    ));
    page.parent(page_tree_ref);
    page.contents(content_ref);
    page.finish();

    let document_info_id = ctx.alloc_ref();
    writer.document_info(document_info_id).producer(TextStr("svg2pdf"));

    writer.finish()
}

/// Convert a [`usvg` tree](usvg::Tree) into a Form XObject that can be used as
/// part of a larger document.
///
/// This method is intended for use in an existing [`PdfWriter`] workflow. It
/// will always return an XObject with the width and height of one printer's
/// point, just like an [`ImageXObject`](pdf_writer::writers::ImageXObject)
/// would.
///
/// The resulting object can be used by registering a name and the `id` with a
/// page's [`/XObject`](pdf_writer::writers::Resources::x_objects) resources
/// dictionary and then invoking the [`Do`](Content::x_object) operator with the
/// name in the page's content stream.
///
/// As the conversion process may need to create multiple indirect objects in
/// the PDF, this function allocates consecutive IDs starting at `id` for its
/// objects and returns the next available ID for your future writing.
///
/// ## Example
/// Write a PDF file with some text and an SVG graphic.
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use svg2pdf;
/// use pdf_writer::{Content, Finish, Name, PdfWriter, Rect, Ref, Str};
/// use usvg::TreeParsing;
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
/// let path = "tests/svgs/custom/integration/matplotlib/step.svg";
/// let svg = std::fs::read_to_string(path)?;
/// let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())?;
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
///     .transform([300.0, 0.0, 0.0, 225.0, 147.5, 385.0])
///     .x_object(svg_name);
///
/// // Write the file to the disk.
/// writer.stream(content_id, &content.finish());
/// std::fs::write("target/embedded.pdf", writer.finish())?;
/// # Ok(()) }
/// ```
pub fn convert_tree_into(
    tree: &Tree,
    options: Options,
    writer: &mut PdfWriter,
    start_ref: Ref,
) -> Ref {
    let mut ctx = Context::new(
        tree,
        options,
        initial_transform(&options, tree, tree.size),
        Some(start_ref.get()),
    );

    let x_ref = ctx.alloc_ref();
    ctx.deferrer.push();

    let tree_x_object = render::tree_to_x_object(tree, writer, &mut ctx);
    let mut content = Content::new();
    content.x_object(tree_x_object.as_name());

    let content_stream = ctx.finish_content(content);

    let mut x_object = writer.form_xobject(x_ref, &content_stream);
    x_object.bbox(ctx.get_rect().as_pdf_rect());
    // Revert the PDF transformation so that the resulting XObject is 1x1 in size.
    x_object.matrix([
        1.0 / ctx.get_rect().width() as f32,
        0.0,
        0.0,
        1.0 / ctx.get_rect().height() as f32,
        0.0,
        0.0,
    ]);

    if ctx.options.compress {
        x_object.filter(Filter::FlateDecode);
    }

    let mut resources = x_object.resources();
    ctx.deferrer.pop(&mut resources);

    ctx.alloc_ref()
}

/// Return the initial transform that is necessary for the conversion between SVG coordinates
/// and the final PDF page (including DPI and a custom viewport).
fn initial_transform(options: &Options, tree: &Tree, actual_size: Size) -> Transform {
    // Account for DPI.
    let dpi_transform = Transform::new_scale(
        dpi_ratio(options.dpi) as f64,
        dpi_ratio(options.dpi) as f64,
    );

    // Account for the custom viewport that has been passed in the Options struct. If nothing has
    // been passed, actual_size will be same as tree.size, so the transform will just be the
    // default transform.
    let custom_viewport_transform = view_box_to_transform(
        usvg::Rect::new(0.0, 0.0, tree.size.width(), tree.size.height()).unwrap(),
        options.aspect.unwrap_or(AspectRatio {
            defer: false,
            align: Align::None,
            slice: false,
        }),
        actual_size,
    );

    // Account for the direction of the y axis and the shift of the origin in the coordinate system.
    let pdf_transform = Transform::new(1.0, 0.0, 0.0, -1.0, 0.0, actual_size.height());

    let mut base_transform = dpi_transform;
    base_transform.append(&pdf_transform);
    base_transform.append(&custom_viewport_transform);
    base_transform
}
