use std::path::{Path, PathBuf};

use fontdb::Database;
use image::RgbaImage;
use lazy_static::lazy_static;
use oxipng::{InFile, OutFile};
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::{PdfColor, PdfRenderConfig};
use usvg::{Tree, TreeParsing, TreeTextToPath};
use walkdir::WalkDir;

use svg2pdf::Options;

pub const SVG_DIR: &str = "svg";
pub const REF_DIR: &str = "ref";
pub const DIFF_DIR: &str = "diff";
pub const PDF_DIR: &str = "pdf";

pub const SKIPPED_FILES: [&str; 126] = [
    // These files crash svg2pdf so we always skip them.
    "svg/resvg/structure/svg/zero-size.svg",
    "svg/resvg/structure/svg/not-UTF-8-encoding.svg",
    "svg/resvg/structure/svg/negative-size.svg",
    // These files don't work correctly in resvg (https://razrfalcon.github.io/resvg-test-suite/svg-support-table.html)
    // or are marked as "undefined behavior", so we skip them as well
    "svg/resvg/shapes/rect/cap-values.svg",
    "svg/resvg/shapes/rect/ch-values.svg",
    "svg/resvg/shapes/rect/ic-values.svg",
    "svg/resvg/shapes/rect/lh-values.svg",
    "svg/resvg/shapes/rect/q-values.svg",
    "svg/resvg/shapes/rect/rem-values.svg",
    "svg/resvg/shapes/rect/rlh-values.svg",
    "svg/resvg/shapes/rect/vi-and-vb-values.svg",
    "svg/resvg/shapes/rect/vmin-and-vmax-values.svg",
    "svg/resvg/shapes/rect/vw-and-vh-values.svg",
    "svg/resvg/structure/image/float-size.svg",
    "svg/resvg/structure/image/no-height-on-svg.svg",
    "svg/resvg/structure/image/no-width-and-height-on-svg.svg",
    "svg/resvg/structure/image/no-width-on-svg.svg",
    "svg/resvg/structure/image/url-to-png.svg",
    "svg/resvg/structure/image/url-to-svg.svg",
    "svg/resvg/structure/style/external-CSS.svg",
    "svg/resvg/structure/style/important.svg",
    "svg/resvg/structure/svg/funcIRI-parsing.svg",
    "svg/resvg/structure/svg/funcIRI-with-invalid-characters.svg",
    "svg/resvg/structure/svg/invalid-id-attribute-1.svg",
    "svg/resvg/structure/svg/invalid-id-attribute-2.svg",
    "svg/resvg/structure/svg/not-UTF-8-encoding.svg",
    "svg/resvg/structure/svg/xlink-to-an-external-file.svg",
    "svg/resvg/painting/fill/#RGBA.svg",
    "svg/resvg/painting/fill/#RRGGBBAA.svg",
    "svg/resvg/painting/fill/icc-color.svg",
    "svg/resvg/painting/fill/rgb-int-int-int.svg",
    "svg/resvg/painting/fill/rgba-0-127-0-50percent.svg",
    "svg/resvg/painting/fill/valid-FuncIRI-with-a-fallback-ICC-color.svg",
    "svg/resvg/painting/marker/on-ArcTo.svg",
    "svg/resvg/painting/marker/target-with-subpaths-2.svg",
    "svg/resvg/painting/marker/with-viewBox-1.svg",
    "svg/resvg/painting/paint-order/fill-markers-stroke.svg",
    "svg/resvg/painting/paint-order/stroke-markers.svg",
    "svg/resvg/painting/stroke-dasharray/negative-sum.svg",
    "svg/resvg/painting/stroke-dasharray/negative-values.svg",
    "svg/resvg/painting/stroke-linejoin/arcs.svg",
    "svg/resvg/painting/stroke-linejoin/miter-clip.svg",
    "svg/resvg/painting/stroke-width/negative.svg",
    "svg/resvg/masking/clip/simple-case.svg",
    "svg/resvg/masking/clipPath/on-the-root-svg-without-size.svg",
    "svg/resvg/masking/mask/color-interpolation=linearRGB.svg",
    "svg/resvg/masking/mask/recursive-on-child.svg",
    "svg/resvg/paint-servers/linearGradient/invalid-gradientTransform.svg",
    "svg/resvg/paint-servers/pattern/invalid-patternTransform.svg",
    "svg/resvg/paint-servers/pattern/overflow=visible.svg",
    "svg/resvg/paint-servers/radialGradient/fr=-1.svg",
    "svg/resvg/paint-servers/radialGradient/fr=0.2.svg",
    "svg/resvg/paint-servers/radialGradient/fr=0.5.svg",
    "svg/resvg/paint-servers/radialGradient/fr=0.7.svg",
    "svg/resvg/paint-servers/radialGradient/invalid-gradientTransform.svg",
    "svg/resvg/paint-servers/radialGradient/invalid-gradientUnits.svg",
    "svg/resvg/paint-servers/radialGradient/negative-r.svg",
    "svg/resvg/text/alignment-baseline/after-edge.svg",
    "svg/resvg/text/alignment-baseline/baseline.svg",
    "svg/resvg/text/alignment-baseline/hanging-on-vertical.svg",
    "svg/resvg/text/alignment-baseline/ideographic.svg",
    "svg/resvg/text/alignment-baseline/text-after-edge.svg",
    "svg/resvg/text/direction/rtl.svg",
    "svg/resvg/text/direction/rtl-with-vertical-writing-mode.svg",
    "svg/resvg/text/dominant-baseline/reset-size.svg",
    "svg/resvg/text/dominant-baseline/use-script.svg",
    "svg/resvg/text/font/simple-case.svg",
    "svg/resvg/text/font-size/negative-size.svg",
    "svg/resvg/text/font-size-adjust/simple-case.svg",
    "svg/resvg/text/font-stretch/extra-condensed.svg",
    "svg/resvg/text/font-stretch/inherit.svg",
    "svg/resvg/text/font-stretch/narrower.svg",
    "svg/resvg/text/font-weight/650.svg",
    "svg/resvg/text/glyph-orientation-horizontal/simple-case.svg",
    "svg/resvg/text/glyph-orientation-vertical/simple-case.svg",
    "svg/resvg/text/kerning/0.svg",
    "svg/resvg/text/letter-spacing/large-negative.svg",
    "svg/resvg/text/text/complex-grapheme-split-by-tspan.svg",
    "svg/resvg/text/text/complex-graphemes-and-coordinates-list.svg",
    "svg/resvg/text/text/compound-emojis-and-coordinates-list.svg",
    "svg/resvg/text/text/compound-emojis.svg",
    "svg/resvg/text/text/emojis.svg",
    "svg/resvg/text/text/rotate-on-Arabic.svg",
    "svg/resvg/text/text/x-and-y-with-multiple-values-and-arabic-text.svg",
    "svg/resvg/text/text/xml-lang=ja.svg",
    "svg/resvg/text/text-anchor/coordinates-list.svg",
    "svg/resvg/text/text-decoration/indirect.svg",
    "svg/resvg/text/text-decoration/style-resolving-4.svg",
    "svg/resvg/text/text-rendering/geometricPrecision.svg",
    "svg/resvg/text/textPath/complex.svg",
    "svg/resvg/text/textPath/method=stretch.svg",
    "svg/resvg/text/textPath/side=right.svg",
    "svg/resvg/text/textPath/spacing=auto.svg",
    "svg/resvg/text/textPath/with-baseline-shift-and-rotate.svg",
    "svg/resvg/text/textPath/with-filter.svg",
    "svg/resvg/text/textPath/with-invalid-path-and-xlink-href.svg",
    "svg/resvg/text/textPath/with-path.svg",
    "svg/resvg/text/tref/link-to-an-external-file-element.svg",
    "svg/resvg/text/tspan/with-clip-path.svg",
    "svg/resvg/text/tspan/with-mask.svg",
    "svg/resvg/text/tspan/with-opacity.svg",
    "svg/resvg/text/word-spacing/large-negative.svg",
    "svg/resvg/text/writing-mode/tb-and-punctuation.svg",
    "svg/resvg/text/writing-mode/tb-with-rotate-and-underline.svg",
    "svg/resvg/text/writing-mode/tb-with-rotate.svg",
    // We don't support externally embedded images
    "svg/resvg/structure/image/external-gif.svg",
    "svg/resvg/structure/image/external-jpeg.svg",
    "svg/resvg/structure/image/external-png.svg",
    "svg/resvg/structure/image/external-svg.svg",
    "svg/resvg/structure/image/external-svg-with-transform.svg",
    "svg/resvg/structure/image/external-svgz.svg",
    "svg/resvg/structure/image/float-size.svg",
    "svg/resvg/structure/image/no-height.svg",
    "svg/resvg/structure/image/no-height-on-svg.svg",
    "svg/resvg/structure/image/no-width.svg",
    "svg/resvg/structure/image/no-width-and-height.svg",
    "svg/resvg/structure/image/no-width-and-height-on-svg.svg",
    "svg/resvg/structure/image/no-width-on-svg.svg",
    "svg/resvg/structure/image/raster-image-and-size-with-odd-numbers.svg",
    "svg/resvg/structure/image/recursive-1.svg",
    "svg/resvg/structure/image/recursive-2.svg",
    "svg/resvg/structure/image/url-to-png.svg",
    "svg/resvg/structure/image/url-to-svg.svg",
    "svg/resvg/structure/image/width-and-height-set-to-auto.svg",
    "svg/resvg/structure/image/zero-height.svg",
    "svg/resvg/structure/image/zero-width.svg",
];

lazy_static! {
    pub static ref SVG_FILES: Vec<PathBuf> = {
        WalkDir::new(SVG_DIR)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.file_name().to_str().map(|s| s.ends_with(".svg")).unwrap_or(false)
            })
            .filter(|e| !SKIPPED_FILES.contains(&e.path().to_str().unwrap()))
            .map(|e| e.into_path())
            .collect()
    };
    pub static ref REF_FILES: Vec<PathBuf> = {
        WalkDir::new(REF_DIR)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.file_name().to_str().map(|s| s.ends_with(".png")).unwrap_or(false)
            })
            .map(|e| e.into_path())
            .collect()
    };
}

#[derive(Eq, PartialEq)]
pub struct TestFile {
    raw_path: PathBuf,
}

/// An abstract representation of a test file that allows to get the paths of the same file
/// in its different formats (e.g. the original svg file, the pdf file, the reference image).
impl TestFile {
    pub fn new(path: &Path) -> Self {
        let mut stripped_path = path.with_extension("svg");

        if stripped_path.starts_with(SVG_DIR) {
            stripped_path = PathBuf::from(stripped_path.strip_prefix(SVG_DIR).unwrap());
        } else if stripped_path.starts_with(REF_DIR) {
            stripped_path = PathBuf::from(stripped_path.strip_prefix(REF_DIR).unwrap());
        }

        TestFile { raw_path: stripped_path }
    }

    fn convert_path(&self, prefix: &Path, extension: &str) -> PathBuf {
        let mut path_buf = PathBuf::new();
        path_buf.push(prefix);
        path_buf.push(self.raw_path.as_path());
        path_buf = path_buf.with_extension(extension);
        path_buf
    }

    pub fn as_raw_path(&self) -> PathBuf {
        self.raw_path.clone()
    }

    pub fn as_svg_path(&self) -> PathBuf {
        self.convert_path(Path::new(SVG_DIR), "svg")
    }

    pub fn as_ref_path(&self) -> PathBuf {
        self.convert_path(Path::new(REF_DIR), "png")
    }

    pub fn as_diff_path(&self) -> PathBuf {
        self.convert_path(Path::new(DIFF_DIR), "png")
    }

    pub fn as_pdf_path(&self) -> PathBuf {
        self.convert_path(Path::new(PDF_DIR), "pdf")
    }
}

pub struct Runner {
    pdfium: Pdfium,
    fontdb: Database,
}

impl Default for Runner {
    fn default() -> Self {
        let mut fontdb = fontdb::Database::new();
        // We need Noto Sans because many test files use it
        fontdb.load_font_file("fonts/NotoSans-Regular.ttf").unwrap();
        fontdb.load_font_file("fonts/NotoSans-Bold.ttf").unwrap();
        fontdb.load_font_file("fonts/NotoSans-Italic.ttf").unwrap();

        Self {
            pdfium: Pdfium::new(
                Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
                    "./pdfium_lib/",
                ))
                .unwrap(),
            ),
            fontdb,
        }
    }
}

impl Runner {
    pub fn render_pdf(&self, pdf: &[u8]) -> RgbaImage {
        let document = self.pdfium.load_pdf_from_byte_slice(pdf, None);

        let render_config = PdfRenderConfig::new()
            .clear_before_rendering(true)
            .set_clear_color(PdfColor::new(255, 255, 255, 0));

        document
            .unwrap()
            .pages()
            .first()
            .unwrap()
            .render_with_config(&render_config)
            .unwrap()
            .as_image()
            .into_rgba8()
    }

    pub fn read_svg(&self, svg_string: &str) -> Tree {
        let options = usvg::Options::default();
        let mut tree = Tree::from_str(svg_string, &options).unwrap();
        tree.convert_text(&self.fontdb);
        tree
    }

    pub fn convert_svg(
        &self,
        svg_string: &str,
        test_runner: &Runner,
    ) -> (Vec<u8>, RgbaImage) {
        let tree = self.read_svg(svg_string);
        // We scale the images by 2.5 so that their resolution is 500 x 500
        let pdf = svg2pdf::convert_tree(
            &tree,
            Options { dpi: 72.0 * 2.5, ..Options::default() },
        );
        let image = test_runner.render_pdf(pdf.as_slice());
        (pdf, image)
    }
}

pub fn save_image(image: &RgbaImage, path: &Path) {
    image.save_with_format(path, image::ImageFormat::Png).unwrap();

    oxipng::optimize(
        &InFile::Path(PathBuf::from(path)),
        &OutFile::Path(Some(PathBuf::from(path))),
        &oxipng::Options::max_compression(),
    )
    .unwrap();
}
