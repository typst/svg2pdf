#[rustfmt::skip]
mod library;
mod cli;

use std::cmp::max;
use std::fs;
use std::path::Path;

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

/// Runs a single test instance.
pub fn run_test(svg_path: &str, ref_path: &str, diff_path: &str, replace: bool) -> i32 {
    let (_, actual_image) = convert_svg(&fs::read_to_string(svg_path).unwrap());

    // Just as a convenience, if the test is supposed to run but there doesn't exist
    // a reference image yet, we create a new one. This allows us to conveniently generate
    // new reference images for test cases.
    if !Path::new(ref_path).exists() {
        fs::create_dir_all(Path::new(ref_path).parent().unwrap()).unwrap();
        save_image(&actual_image, Path::new(ref_path));
        return 1;
    }

    let expected_image = Reader::open(ref_path).unwrap().decode().unwrap().into_rgba8();

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
