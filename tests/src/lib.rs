use image::RgbaImage;
use lazy_static::lazy_static;
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::PdfRenderConfig;
use std::path::{Path, PathBuf};
use usvg::{Tree, TreeParsing, TreeTextToPath};
use walkdir::WalkDir;

pub const SVG_DIR: &str = "svgs";
pub const REF_DIR: &str = "references";
pub const DIFF_DIR: &str = "diffs";

pub const SKIPPED_FILES: [&str; 56] = [
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
    "svgs/resvg/paint-servers/radialGradient/negative-r.svg"
];

lazy_static! {
    pub static ref SVG_FILES: Vec<PathBuf> = {
        WalkDir::new(SVG_DIR)
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
    raw_path: PathBuf
}

impl TestFile {
    pub fn new(path: &Path) -> Self {
        let mut stripped_path = path.with_extension("svg");

        if stripped_path.starts_with(SVG_DIR) {
            stripped_path = PathBuf::from(stripped_path.strip_prefix(SVG_DIR).unwrap());
        }   else if stripped_path.starts_with(REF_DIR) {
            stripped_path = PathBuf::from(stripped_path.strip_prefix(REF_DIR).unwrap());
        }

        TestFile {
            raw_path: stripped_path
        }
    }

    fn convert_path(
        &self,
        prefix: &Path,
        extension: &str,
    ) -> PathBuf {
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

impl TestRunner {
    pub fn new() -> Self {
        Self {
            pdfium: Pdfium::new(
                Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
                    "./pdfium_lib/",
                ))
                    .unwrap(),
            ),
        }
    }

    pub fn render_pdf(&self, pdf: &[u8]) -> RgbaImage {
        let document = self.pdfium.load_pdf_from_byte_slice(pdf, None);

        let render_config = PdfRenderConfig::new().scale_page_by_factor(2.5);

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
    fontdb.load_system_fonts();

    let mut tree = Tree::from_str(svg_string, &options).unwrap();
    tree.convert_text(&fontdb);
    tree
}

pub fn svg_to_image(svg_string: &str, test_runner: &TestRunner) -> RgbaImage {
    let tree = read_svg(svg_string);
    let pdf = svg2pdf::convert_tree(&tree);
    test_runner.render_pdf(pdf.as_slice())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use crate::TestFile;

    #[test]
    fn file_path_from_svg_works_correctly() {
        let path = Path::new("svgs/resvg/structure/svg/zero-size.svg");
        let file_path = TestFile::new(path);

        assert_eq!(file_path.as_raw_path(), PathBuf::from("resvg/structure/svg/zero-size.svg"));
        assert_eq!(file_path.as_svg_path(), PathBuf::from("svgs/resvg/structure/svg/zero-size.svg"));
        assert_eq!(file_path.as_references_path(), PathBuf::from("references/resvg/structure/svg/zero-size.png"));
    }

    #[test]
    fn file_path_from_raw_works_correctly() {
        let path = Path::new("resvg/structure/svg/zero-size.svg");
        let file_path = TestFile::new(path);

        assert_eq!(file_path.as_raw_path(), PathBuf::from("resvg/structure/svg/zero-size.svg"));
        assert_eq!(file_path.as_svg_path(), PathBuf::from("svgs/resvg/structure/svg/zero-size.svg"));
        assert_eq!(file_path.as_references_path(), PathBuf::from("references/resvg/structure/svg/zero-size.png"));
    }

    #[test]
    fn file_path_from_reference_works_correctly() {
        let path = Path::new("references/resvg/structure/svg/zero-size.png");
        let file_path = TestFile::new(path);

        assert_eq!(file_path.as_raw_path(), PathBuf::from("resvg/structure/svg/zero-size.svg"));
        assert_eq!(file_path.as_svg_path(), PathBuf::from("svgs/resvg/structure/svg/zero-size.svg"));
        assert_eq!(file_path.as_references_path(), PathBuf::from("references/resvg/structure/svg/zero-size.png"));
    }
}