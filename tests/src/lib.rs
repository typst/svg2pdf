#[rustfmt::skip]
mod integration;
mod api;

use std::cmp::max;
use std::fs;
use std::path::{Path, PathBuf};

use image::io::Reader;
use image::{Rgba, RgbaImage};
use once_cell::sync::Lazy;
use oxipng::{InFile, OutFile};
use pdfium_render::pdfium::Pdfium;
use pdfium_render::prelude::{PdfColor, PdfRenderConfig};
use usvg::Tree;

use svg2pdf::Options;

/// The global fontdb instance.
static FONTDB: Lazy<std::sync::Mutex<fontdb::Database>> = Lazy::new(|| {
    let mut fontdb = fontdb::Database::new();
    fontdb.load_fonts_dir("fonts");

    fontdb.set_serif_family("Noto Serif");
    fontdb.set_sans_serif_family("Noto Sans");
    fontdb.set_cursive_family("Yellowtail");
    fontdb.set_fantasy_family("Sedgwick Ave Display");
    fontdb.set_monospace_family("Noto Mono");

    std::sync::Mutex::new(fontdb)
});

/// The global pdfium instance.
static PDFIUM: Lazy<std::sync::Mutex<Pdfium>> = Lazy::new(|| {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            "./pdfium/",
        ))
        .unwrap(),
    );
    std::sync::Mutex::new(pdfium)
});

/// Converts a PDF into a png image.
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

/// Converts an SVG string into a usvg Tree
pub fn read_svg(svg_string: &str) -> Tree {
    let options = usvg::Options::default();
    Tree::from_str(svg_string, &options, &FONTDB.lock().unwrap()).unwrap()
}

/// Converts an image into a PDF and returns the PDF as well as a rendered version
/// of it.
pub fn convert_svg(svg_path: &Path, options: Options) -> (Vec<u8>, RgbaImage) {
    let svg = fs::read_to_string(svg_path).unwrap();
    let tree = read_svg(&svg);
    let pdf = svg2pdf::to_pdf(&tree, options, &FONTDB.lock().unwrap());
    let image = render_pdf(pdf.as_slice());
    (pdf, image)
}

/// Saves an RGBA image to a path.
pub fn save_image(image: &RgbaImage, path: &Path) {
    image.save_with_format(path, image::ImageFormat::Png).unwrap();

    oxipng::optimize(
        &InFile::Path(path.into()),
        &OutFile::from_path(path.into()),
        &oxipng::Options::max_compression(),
    )
    .unwrap();
}

/// Checks if two pixels are different.
fn is_pix_diff(pixel1: &Rgba<u8>, pixel2: &Rgba<u8>) -> bool {
    if pixel1.0[3] == 0 && pixel2.0[3] == 0 {
        return false;
    }

    pixel1.0[0] != pixel2.0[0]
        || pixel1.0[1] != pixel2.0[1]
        || pixel1.0[2] != pixel2.0[2]
        || pixel1.0[3] != pixel2.0[3]
}

const REPLACE: bool = false;
const PDF: bool = false;

pub fn get_diff(
    expected_image: &RgbaImage,
    actual_image: &RgbaImage,
) -> (RgbaImage, i32) {
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

    (diff_image, pixel_diff)
}

pub fn get_svg_path(test_name: &str) -> PathBuf {
    PathBuf::from("svg").join(String::from(test_name) + ".svg")
}

pub fn get_ref_path(test_name: &str) -> PathBuf {
    PathBuf::from("ref").join(String::from(test_name) + ".png")
}

pub fn get_diff_path(test_name: &str) -> PathBuf {
    PathBuf::from("diff").join(String::from(test_name) + ".png")
}

pub fn get_pdf_path(test_name: &str) -> PathBuf {
    PathBuf::from("pdf").join(String::from(test_name) + ".pdf")
}

/// Runs a single test instance.
pub fn run_test(test_name: &str, options: Options) -> i32 {
    let svg_path = get_svg_path(test_name);
    let ref_path = get_ref_path(test_name);
    let diff_path = get_diff_path(test_name);
    let pdf_path = get_pdf_path(test_name);

    run_test_impl(&svg_path, &ref_path, &diff_path, &pdf_path, options)
}
pub fn run_test_impl(
    svg_path: &Path,
    ref_path: &Path,
    diff_path: &Path,
    pdf_path: &Path,
    options: Options,
) -> i32 {
    let (pdf, actual_image) = convert_svg(&svg_path, options);

    // Just as a convenience, if the test is supposed to run but there doesn't exist
    // a reference image yet, we create a new one. This allows us to conveniently generate
    // new reference images for test cases.
    if !ref_path.exists() {
        fs::create_dir_all(ref_path.parent().unwrap()).unwrap();
        save_image(&actual_image, &ref_path);
        return 1;
    }

    if PDF {
        fs::create_dir_all(pdf_path.parent().unwrap()).unwrap();
        fs::write(pdf_path, pdf).unwrap();
    }

    let expected_image = Reader::open(&ref_path).unwrap().decode().unwrap().into_rgba8();

    let (diff_image, pixel_diff) = get_diff(&expected_image, &actual_image);

    if pixel_diff > 0 {
        fs::create_dir_all(&diff_path.parent().unwrap()).unwrap();

        diff_image
            .save_with_format(&diff_path, image::ImageFormat::Png)
            .unwrap();

        if REPLACE {
            save_image(&actual_image, &ref_path);
        }
    }

    pixel_diff
}
