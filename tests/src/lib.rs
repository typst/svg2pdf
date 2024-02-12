#[rustfmt::skip]
mod render;

use std::cmp::max;
use std::fs;
use std::path::{Path, PathBuf};

use image::io::Reader;
use image::{Rgba, RgbaImage};
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use oxipng::{InFile, OutFile};
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::{PdfColor, PdfRenderConfig};
use usvg::{PostProcessingSteps, Tree, TreeParsing, TreePostProc};
use walkdir::WalkDir;

use svg2pdf::Options;

pub const SVG_DIR: &str = "svg";
pub const REF_DIR: &str = "ref";
pub const DIFF_DIR: &str = "diff";
pub const PDF_DIR: &str = "pdf";

static FONTDB: Lazy<std::sync::Mutex<fontdb::Database>> = Lazy::new(|| {
    let mut fontdb = fontdb::Database::new();
    fontdb.load_fonts_dir("fonts");
    std::sync::Mutex::new(fontdb)
});

static PDFIUM: Lazy<std::sync::Mutex<Pdfium>> = Lazy::new(|| {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            "./pdfium/",
        ))
        .unwrap(),
    );
    std::sync::Mutex::new(pdfium)
});

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

pub fn render_pdf(pdf: &[u8]) -> RgbaImage {
    let pdfium = PDFIUM.lock().unwrap();
    let document = pdfium.load_pdf_from_byte_slice(pdf, None);

    let render_config = PdfRenderConfig::new()
        .clear_before_rendering(true)
        .set_clear_color(PdfColor::new(255, 255, 255, 0));

    let result = document
        .unwrap()
        .pages()
        .first()
        .unwrap()
        .render_with_config(&render_config)
        .unwrap()
        .as_image()
        .into_rgba8();
    result
}

pub fn read_svg(svg_string: &str) -> Tree {
    let options = usvg::Options::default();
    let mut tree = Tree::from_str(svg_string, &options).unwrap();
    tree.postprocess(PostProcessingSteps::default(), &FONTDB.lock().unwrap());
    tree.calculate_bounding_boxes();
    tree
}

pub fn convert_svg(svg_string: &str) -> (Vec<u8>, RgbaImage) {
    let scale_factor = 1.0;
    let tree = read_svg(svg_string);
    let pdf = svg2pdf::convert_tree(
        &tree,
        Options {
            dpi: 72.0 * scale_factor,
            raster_scale: 1.5,
            ..Options::default()
        },
    );
    let image = render_pdf(pdf.as_slice());
    (pdf, image)
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

fn is_pix_diff(pixel1: &Rgba<u8>, pixel2: &Rgba<u8>) -> bool {
    if pixel1.0[3] == 0 && pixel2.0[3] == 0 {
        return false;
    }

    pixel1.0[0] != pixel2.0[0]
        || pixel1.0[1] != pixel2.0[1]
        || pixel1.0[2] != pixel2.0[2]
        || pixel1.0[3] != pixel2.0[3]
}

pub fn render(svg_path: &str, ref_path: &str, diff_path: &str, replace: bool) -> i32 {
    let expected_image = Reader::open(ref_path).unwrap().decode().unwrap().into_rgba8();

    let (_, actual_image) = convert_svg(&fs::read_to_string(svg_path).unwrap());

    let width = max(expected_image.width(), actual_image.width());
    let height = max(expected_image.height(), actual_image.height());

    let mut diff_image = RgbaImage::new(width * 3, height);

    let mut pixel_diff = 0;

    for x in 0..width {
        for y in 0..height {
            let actual_pixel = actual_image.get_pixel_checked(x, y);
            let expected_pixel = expected_image.get_pixel_checked(x, y);

            match (actual_pixel, expected_pixel) {
                (Some(actual), Some(expected)) => {
                    diff_image.put_pixel(x, y, *expected);
                    diff_image.put_pixel(x + 2 * width, y, *actual);
                    if is_pix_diff(expected, actual) {
                        pixel_diff += 1;
                        diff_image.put_pixel(x + width, y, Rgba([255, 0, 0, 255]));
                    } else {
                        diff_image.put_pixel(x + width, y, Rgba([0, 0, 0, 255]))
                    }
                }
                (Some(actual), None) => {
                    pixel_diff += 1;
                    diff_image.put_pixel(x + 2 * width, y, *actual);
                    diff_image.put_pixel(x + width, y, Rgba([255, 0, 0, 255]));
                }
                (None, Some(expected)) => {
                    pixel_diff += 1;
                    diff_image.put_pixel(x, y, *expected);
                    diff_image.put_pixel(x + width, y, Rgba([255, 0, 0, 255]));
                }
                _ => unreachable!(),
            }
        }
    }

    if pixel_diff > 0 {
        fs::create_dir_all(Path::new(diff_path).parent().unwrap()).unwrap();

        diff_image
            .save_with_format(diff_path, image::ImageFormat::Png)
            .unwrap();

        if replace {
            save_image(&actual_image, Path::new(ref_path));
        }
    }

    pixel_diff
}
