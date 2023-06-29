use image::RgbaImage;
use lazy_static::lazy_static;
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::{PdfColor, PdfRenderConfig};
use std::path::{Path, PathBuf};
use usvg::{Tree, TreeParsing, TreeTextToPath};
use walkdir::WalkDir;
use svg2pdf::Options;

pub const SVG_DIR: &str = "svgs";
pub const REF_DIR: &str = "references";
pub const DIFF_DIR: &str = "diffs";

pub const SKIPPED_FILES: [&str; 128] = [
    // These files crash svg2pdf so we always skip them.
    "svgs/resvg/structure/svg/zero-size.svg",
    "svgs/resvg/structure/svg/not-UTF-8-encoding.svg",
    "svgs/resvg/structure/svg/negative-size.svg",
    // These files don't work correctly in resvg (https://razrfalcon.github.io/resvg-test-suite/svg-support-table.html)
    // or are marked as "undefined behavior", so we skip them as well
    "svgs/resvg/shapes/rect/cap-values.svg",
    "svgs/resvg/shapes/rect/ch-values.svg",
    "svgs/resvg/shapes/rect/ic-values.svg",
    "svgs/resvg/shapes/rect/lh-values.svg",
    "svgs/resvg/shapes/rect/q-values.svg",
    "svgs/resvg/shapes/rect/rem-values.svg",
    "svgs/resvg/shapes/rect/rlh-values.svg",
    "svgs/resvg/shapes/rect/vi-and-vb-values.svg",
    "svgs/resvg/shapes/rect/vmin-and-vmax-values.svg",
    "svgs/resvg/shapes/rect/vw-and-vh-values.svg",
    "svgs/resvg/structure/image/float-size.svg",
    "svgs/resvg/structure/image/no-height-on-svg.svg",
    "svgs/resvg/structure/image/no-width-and-height-on-svg.svg",
    "svgs/resvg/structure/image/no-width-on-svg.svg",
    "svgs/resvg/structure/image/url-to-png.svg",
    "svgs/resvg/structure/image/url-to-svg.svg",
    "svgs/resvg/structure/style/external-CSS.svg",
    "svgs/resvg/structure/style/important.svg",
    "svgs/resvg/structure/svg/funcIRI-parsing.svg",
    "svgs/resvg/structure/svg/invalid-id-attribute-1.svg",
    "svgs/resvg/structure/svg/invalid-id-attribute-2.svg",
    "svgs/resvg/structure/svg/not-UTF-8-encoding.svg",
    "svgs/resvg/structure/svg/xlink-to-an-external-file.svg",
    "svgs/resvg/painting/fill/#RGBA.svg",
    "svgs/resvg/painting/fill/#RRGGBBAA.svg",
    "svgs/resvg/painting/fill/icc-color.svg",
    "svgs/resvg/painting/fill/rgb-int-int-int.svg",
    "svgs/resvg/painting/fill/rgba-0-127-0-50percent.svg",
    "svgs/resvg/painting/fill/valid-FuncIRI-with-a-fallback-ICC-color.svg",
    "svgs/resvg/painting/marker/on-ArcTo.svg",
    "svgs/resvg/painting/marker/target-with-subpaths-2.svg",
    "svgs/resvg/painting/marker/with-viewBox-1.svg",
    "svgs/resvg/painting/paint-order/fill-markers-stroke.svg",
    "svgs/resvg/painting/paint-order/stroke-markers.svg",
    "svgs/resvg/painting/stroke-dasharray/negative-sum.svg",
    "svgs/resvg/painting/stroke-dasharray/negative-values.svg",
    "svgs/resvg/painting/stroke-linejoin/arcs.svg",
    "svgs/resvg/painting/stroke-linejoin/miter-clip.svg",
    "svgs/resvg/painting/stroke-width/negative.svg",
    "svgs/resvg/masking/clip/simple-case.svg",
    "svgs/resvg/masking/clipPath/on-the-root-svg-without-size.svg",
    "svgs/resvg/masking/mask/color-interpolation=linearRGB.svg",
    "svgs/resvg/masking/mask/recursive-on-child.svg",
    "svgs/resvg/paint-servers/linearGradient/invalid-gradientTransform.svg",
    "svgs/resvg/paint-servers/pattern/invalid-patternTransform.svg",
    "svgs/resvg/paint-servers/pattern/overflow=visible.svg",
    "svgs/resvg/paint-servers/radialGradient/fr=-1.svg",
    "svgs/resvg/paint-servers/radialGradient/fr=0.2.svg",
    "svgs/resvg/paint-servers/radialGradient/fr=0.5.svg",
    "svgs/resvg/paint-servers/radialGradient/fr=0.7.svg",
    "svgs/resvg/paint-servers/radialGradient/invalid-gradientTransform.svg",
    "svgs/resvg/paint-servers/radialGradient/invalid-gradientUnits.svg",
    "svgs/resvg/paint-servers/radialGradient/negative-r.svg",
    "svgs/resvg/text/alignment-baseline/after-edge.svg",
    "svgs/resvg/text/alignment-baseline/baseline.svg",
    "svgs/resvg/text/alignment-baseline/hanging-on-vertical.svg",
    "svgs/resvg/text/alignment-baseline/ideographic.svg",
    "svgs/resvg/text/alignment-baseline/text-after-edge.svg",
    "svgs/resvg/text/direction/rtl.svg",
    "svgs/resvg/text/direction/rtl-with-vertical-writing-mode.svg",
    "svgs/resvg/text/dominant-baseline/reset-size.svg",
    "svgs/resvg/text/dominant-baseline/use-script.svg",
    "svgs/resvg/text/font/simple-case.svg",
    "svgs/resvg/text/font-size/negative-size.svg",
    "svgs/resvg/text/font-size-adjust/simple-case.svg",
    "svgs/resvg/text/font-stretch/extra-condensed.svg",
    "svgs/resvg/text/font-stretch/inherit.svg",
    "svgs/resvg/text/font-stretch/narrower.svg",
    "svgs/resvg/text/font-weight/650.svg",
    "svgs/resvg/text/glyph-orientation-horizontal/simple-case.svg",
    "svgs/resvg/text/glyph-orientation-vertical/simple-case.svg",
    "svgs/resvg/text/kerning/0.svg",
    "svgs/resvg/text/letter-spacing/large-negative.svg",
    "svgs/resvg/text/text/complex-grapheme-split-by-tspan.svg",
    "svgs/resvg/text/text/complex-graphemes-and-coordinates-list.svg",
    "svgs/resvg/text/text/compound-emojis-and-coordinates-list.svg",
    "svgs/resvg/text/text/compound-emojis.svg",
    "svgs/resvg/text/text/emojis.svg",
    "svgs/resvg/text/text/rotate-on-Arabic.svg",
    "svgs/resvg/text/text/x-and-y-with-multiple-values-and-arabic-text.svg",
    "svgs/resvg/text/text/xml-lang=ja.svg",
    "svgs/resvg/text/text-anchor/coordinates-list.svg",
    "svgs/resvg/text/text-decoration/indirect.svg",
    "svgs/resvg/text/text-decoration/style-resolving-4.svg",
    "svgs/resvg/text/text-rendering/geometricPrecision.svg",
    "svgs/resvg/text/textPath/complex.svg",
    "svgs/resvg/text/textPath/method=stretch.svg",
    "svgs/resvg/text/textPath/side=right.svg",
    "svgs/resvg/text/textPath/spacing=auto.svg",
    "svgs/resvg/text/textPath/with-baseline-shift-and-rotate.svg",
    "svgs/resvg/text/textPath/with-filter.svg",
    "svgs/resvg/text/textPath/with-invalid-path-and-xlink-href.svg",
    "svgs/resvg/text/textPath/with-path.svg",
    "svgs/resvg/text/tref/link-to-an-external-file-element.svg",
    "svgs/resvg/text/tspan/with-clip-path.svg",
    "svgs/resvg/text/tspan/with-mask.svg",
    "svgs/resvg/text/tspan/with-opacity.svg",
    "svgs/resvg/text/word-spacing/large-negative.svg",
    "svgs/resvg/text/writing-mode/tb-and-punctuation.svg",
    "svgs/resvg/text/writing-mode/tb-with-rotate-and-underline.svg",
    "svgs/resvg/text/writing-mode/tb-with-rotate.svg",
    // We don't support externally embedded images
    "svgs/resvg/structure/image/external-gif.svg",
    "svgs/resvg/structure/image/external-jpeg.svg",
    "svgs/resvg/structure/image/external-png.svg",
    "svgs/resvg/structure/image/external-svg.svg",
    "svgs/resvg/structure/image/external-svg-with-transform.svg",
    "svgs/resvg/structure/image/external-svgz.svg",
    "svgs/resvg/structure/image/float-size.svg",
    "svgs/resvg/structure/image/no-height.svg",
    "svgs/resvg/structure/image/no-height-on-svg.svg",
    "svgs/resvg/structure/image/no-width.svg",
    "svgs/resvg/structure/image/no-width-and-height.svg",
    "svgs/resvg/structure/image/no-width-and-height-on-svg.svg",
    "svgs/resvg/structure/image/no-width-on-svg.svg",
    "svgs/resvg/structure/image/raster-image-and-size-with-odd-numbers.svg",
    "svgs/resvg/structure/image/recursive-1.svg",
    "svgs/resvg/structure/image/recursive-2.svg",
    "svgs/resvg/structure/image/url-to-png.svg",
    "svgs/resvg/structure/image/url-to-svg.svg",
    "svgs/resvg/structure/image/width-and-height-set-to-auto.svg",
    "svgs/resvg/structure/image/zero-height.svg",
    "svgs/resvg/structure/image/zero-width.svg",
    // For some reason, jpg images are rendered slightly on the CLI (upon inspection
    // they look exactly the same though, so this probably is due to some difference
    // how jpgs are handled on each operating system in pdfium. For this reason, we skip
    // them. In order to still test whether jpgs generally work, three tests with a solid
    // color jpg image were added to the custom tests.
    "svgs/resvg/structure/image/embedded-jpeg-as-image-jpeg.svg",
    "svgs/resvg/structure/image/embedded-jpeg-as-image-jpg.svg",
    "svgs/resvg/structure/image/embedded-jpeg-without-mime.svg",
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

    pub fn as_references_path(&self) -> PathBuf {
        self.convert_path(Path::new(REF_DIR), "png")
    }

    pub fn as_diffs_path(&self) -> PathBuf {
        self.convert_path(Path::new(DIFF_DIR), "png")
    }
}

pub struct TestRunner {
    pdfium: Pdfium,
}

impl Default for TestRunner {
    fn default() -> Self {
        Self {
            pdfium: Pdfium::new(
                Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
                    "./pdfium_lib/",
                ))
                .unwrap(),
            ),
        }
    }
}

impl TestRunner {
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
}

pub fn read_svg(svg_string: &str) -> Tree {
    let options = usvg::Options::default();
    let mut fontdb = fontdb::Database::new();
    fontdb.load_font_file("fonts/NotoSans-Regular.ttf").unwrap();
    fontdb.load_font_file("fonts/NotoSans-Bold.ttf").unwrap();
    fontdb.load_font_file("fonts/NotoSans-Italic.ttf").unwrap();

    let mut tree = Tree::from_str(svg_string, &options).unwrap();
    tree.convert_text(&fontdb);
    tree
}

pub fn svg_to_image(svg_string: &str, test_runner: &TestRunner) -> RgbaImage {
    let tree = read_svg(svg_string);
    let pdf = svg2pdf::convert_tree(&tree, Options {dpi: 72.0 * 2.5});
    test_runner.render_pdf(pdf.as_slice())
}

#[cfg(test)]
mod tests {
    use crate::TestFile;
    use std::path::{Path, PathBuf};

    #[test]
    fn file_path_from_svg_works_correctly() {
        let path = Path::new("svgs/resvg/structure/svg/zero-size.svg");
        let file_path = TestFile::new(path);

        assert_eq!(
            file_path.as_raw_path(),
            PathBuf::from("resvg/structure/svg/zero-size.svg")
        );
        assert_eq!(
            file_path.as_svg_path(),
            PathBuf::from("svgs/resvg/structure/svg/zero-size.svg")
        );
        assert_eq!(
            file_path.as_references_path(),
            PathBuf::from("references/resvg/structure/svg/zero-size.png")
        );
    }

    #[test]
    fn file_path_from_raw_works_correctly() {
        let path = Path::new("resvg/structure/svg/zero-size.svg");
        let file_path = TestFile::new(path);

        assert_eq!(
            file_path.as_raw_path(),
            PathBuf::from("resvg/structure/svg/zero-size.svg")
        );
        assert_eq!(
            file_path.as_svg_path(),
            PathBuf::from("svgs/resvg/structure/svg/zero-size.svg")
        );
        assert_eq!(
            file_path.as_references_path(),
            PathBuf::from("references/resvg/structure/svg/zero-size.png")
        );
    }

    #[test]
    fn file_path_from_reference_works_correctly() {
        let path = Path::new("references/resvg/structure/svg/zero-size.png");
        let file_path = TestFile::new(path);

        assert_eq!(
            file_path.as_raw_path(),
            PathBuf::from("resvg/structure/svg/zero-size.svg")
        );
        assert_eq!(
            file_path.as_svg_path(),
            PathBuf::from("svgs/resvg/structure/svg/zero-size.svg")
        );
        assert_eq!(
            file_path.as_references_path(),
            PathBuf::from("references/resvg/structure/svg/zero-size.png")
        );
    }
}
