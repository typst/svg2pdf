use std::path::{Path, PathBuf};

use fontdb::Database;
use image::RgbaImage;
use lazy_static::lazy_static;
use oxipng::{InFile, OutFile};
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::{PdfColor, PdfRenderConfig};
use usvg::Tree;
use walkdir::WalkDir;

use svg2pdf::Options;

pub const SVG_DIR: &str = "svg";
pub const REF_DIR: &str = "ref";
pub const DIFF_DIR: &str = "diff";
pub const PDF_DIR: &str = "pdf";

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
        let tree = Tree::from_str(svg_string, &options, &self.fontdb).unwrap();
        tree
    }

    pub fn convert_svg(
        &self,
        svg_string: &str,
        test_runner: &Runner,
    ) -> (Vec<u8>, RgbaImage) {
        let scale_factor = 1.0;
        let tree = self.read_svg(svg_string);
        let pdf = svg2pdf::convert_tree(
            &tree,
            Options {
                dpi: 72.0 * scale_factor,
                raster_scale: 1.5,
                ..Options::default()
            },
        );
        let image = test_runner.render_pdf(pdf.as_slice());
        (pdf, image)
    }
}

pub fn save_image(image: &RgbaImage, path: &Path) {
    image.save_with_format(path, image::ImageFormat::Png).unwrap();

    oxipng::optimize(
        &InFile::Path(path.into()),
        &OutFile::from_path(path.into()),
        &oxipng::Options::max_compression(),
    )
    .unwrap();
}
