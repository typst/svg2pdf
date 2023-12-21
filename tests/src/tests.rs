use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;
use image::io::Reader;
use image::{Rgba, RgbaImage};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use svg2pdf_tests::*;

#[derive(Parser, Debug)]
#[clap(about, version)]
struct Args {
    #[clap(short, long)]
    replace: bool,
    #[clap(short, long)]
    verbose: bool,
}
#[derive(PartialEq, Eq)]
enum TestStatus {
    Success,
    Failure,
    Skipped,
}

impl Display for TestStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            TestStatus::Success => write!(f, "SUCCESS"),
            TestStatus::Failure => write!(f, "FAILURE"),
            TestStatus::Skipped => write!(f, "SKIPPED"),
        }
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    let references: Vec<TestFile> =
        (*REF_FILES).iter().map(|f| TestFile::new(f)).collect();
    let svg_files: Vec<TestFile> =
        (*SVG_FILES).iter().map(|f| TestFile::new(f)).collect();
    let runner = Runner::default();

    let _ = fs::remove_dir_all(DIFF_DIR);

    let mut successful_tests: Vec<&TestFile> = vec![];
    let mut failure_tests: Vec<&TestFile> = vec![];
    let mut skipped_tests: Vec<&TestFile> = vec![];

    println!("Testing {} files in total.", svg_files.len());

    #[cfg(debug_assertions)]
    println!("Running the tests in debug mode may take a long time.");

    for svg_file in &svg_files {
        if !references.contains(svg_file) {
            let _ = print_test_case_result(TestStatus::Skipped, svg_file, args.verbose);
            skipped_tests.push(svg_file);
            continue;
        }

        let expected_image = Reader::open(svg_file.as_ref_path())
            .unwrap()
            .decode()
            .unwrap()
            .into_rgba8();
        let (_, actual_image) = runner
            .convert_svg(&fs::read_to_string(svg_file.as_svg_path()).unwrap(), &runner);

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
            let _ = print_test_case_result(TestStatus::Failure, svg_file, args.verbose);
            failure_tests.push(svg_file);
            fs::create_dir_all(svg_file.as_diff_path().parent().unwrap()).unwrap();
            diff_image
                .save_with_format(svg_file.as_diff_path(), image::ImageFormat::Png)
                .unwrap();

            if args.replace {
                save_image(&actual_image, &svg_file.as_ref_path());
            }
        } else {
            successful_tests.push(svg_file);
            let _ = print_test_case_result(TestStatus::Success, svg_file, args.verbose);
        }
    }

    StandardStream::stdout(ColorChoice::Always).reset().unwrap();

    println!("SUMMARY");
    println!("TOTAL - {}", svg_files.len());
    println!("SUCCESS - {}", successful_tests.len());
    println!("FAILURE - {}", failure_tests.len());
    println!("SKIPPED - {}", skipped_tests.len());

    if !failure_tests.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
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

fn print_test_case_result(
    test_status: TestStatus,
    file: &TestFile,
    verbose: bool,
) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);

    if !verbose && test_status == TestStatus::Success {
        return Ok(());
    }

    match test_status {
        TestStatus::Success => {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?
        }
        TestStatus::Failure => {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?
        }
        TestStatus::Skipped => {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)))?
        }
    }
    writeln!(&mut stdout, "{} - {}", test_status, file.as_svg_path().to_str().unwrap())
}
