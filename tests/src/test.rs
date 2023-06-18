use clap::Parser;
use image::io::Reader;
use image::{Rgba, RgbaImage};
use std::fmt::Formatter;
use std::io::Write;
use std::process::ExitCode;
use std::{fmt, fs, io};
use svg2pdf_tests::*;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(Parser, Debug)]
#[clap(about, version)]
struct Args {
    #[clap(short, long)]
    replace: bool,
}

enum TestStatus {
    SUCCESS,
    FAILURE,
    SKIPPED,
}

impl fmt::Display for TestStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TestStatus::SUCCESS => write!(f, "SUCCESS"),
            TestStatus::FAILURE => write!(f, "FAILURE"),
            TestStatus::SKIPPED => write!(f, "SKIPPED"),
        }
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    let references: Vec<TestFile> =
        (&*REF_FILES).iter().map(|f| TestFile::new(f)).collect();
    let svg_files: Vec<TestFile> =
        (&*SVG_FILES).iter().map(|f| TestFile::new(f)).collect();
    let test_runner = TestRunner::new();

    let _ = fs::remove_dir_all(DIFF_DIR);

    let mut successful_tests: Vec<&TestFile> = vec![];
    let mut failure_tests: Vec<&TestFile> = vec![];
    let mut skipped_tests: Vec<&TestFile> = vec![];

    println!("Testing {} files in total.", svg_files.len());

    for svg_file in &svg_files {
        if !references.contains(svg_file) {
            let _ = print_test_case_result(TestStatus::SKIPPED, svg_file);
            skipped_tests.push(&svg_file);
            continue;
        }

        let expected_image = Reader::open(svg_file.as_references_path())
            .unwrap()
            .decode()
            .unwrap()
            .into_rgba8();
        let actual_image = svg_to_image(
            &fs::read_to_string(svg_file.as_svg_path()).unwrap(),
            &test_runner,
        );

        let (width, height) = expected_image.dimensions();
        let mut diff_image = RgbaImage::new(width * 3, height);

        let mut diff = false;

        for (x, y, expected_pixel) in expected_image.enumerate_pixels() {
            let actual_pixel = actual_image.get_pixel(x, y);
            diff_image.put_pixel(x, y, *expected_pixel);
            diff_image.put_pixel(x + 2 * width, y, *actual_pixel);
            if is_pix_diff(expected_pixel, actual_pixel) {
                diff = true;
                diff_image.put_pixel(x + width, y, Rgba([255, 0, 0, 255]));
            } else {
                diff_image.put_pixel(x + width, y, Rgba([0, 0, 0, 255]))
            }
        }

        if diff {
            let _ = print_test_case_result(TestStatus::FAILURE, svg_file);
            failure_tests.push(svg_file);
            fs::create_dir_all(svg_file.as_diffs_path().parent().unwrap()).unwrap();
            diff_image
                .save_with_format(svg_file.as_diffs_path(), image::ImageFormat::Png)
                .unwrap();

            if args.replace {
                actual_image.save_with_format(svg_file.as_references_path(), image::ImageFormat::Png).unwrap();
            }
        } else {
            successful_tests.push(svg_file);
            let _ = print_test_case_result(TestStatus::SUCCESS, svg_file);
        }
    }

    StandardStream::stdout(ColorChoice::Always).reset().unwrap();

    println!("SUMMARY");
    println!("TOTAL - {}", svg_files.len());
    println!("SUCCESS - {}", successful_tests.len());
    println!("FAILURE - {}", failure_tests.len());
    println!("SKIPPED - {}", skipped_tests.len());

    if failure_tests.len() > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn is_pix_diff(pixel1: &Rgba<u8>, pixel2: &Rgba<u8>) -> bool {
    pixel1.0[0] != pixel2.0[0]
        || pixel1.0[1] != pixel2.0[1]
        || pixel1.0[2] != pixel2.0[2]
        || pixel1.0[3] != pixel2.0[3]
}

fn print_test_case_result(test_status: TestStatus, file: &TestFile) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    match test_status {
        TestStatus::SUCCESS => {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?
        }
        TestStatus::FAILURE => {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?
        }
        TestStatus::SKIPPED => {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)))?
        }
    }
    writeln!(&mut stdout, "{} - {}", test_status, file.as_svg_path().to_str().unwrap())
}
