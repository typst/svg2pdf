use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::fs;

use svg2pdf_tests::*;

#[derive(Parser, Debug)]
#[clap(about, version)]
struct Args {
    #[clap(short, long)]
    full: bool,
    #[clap(short, long)]
    subset: Option<Regex>,
    #[clap(short, long)]
    pdf: bool,
}

fn main() {
    let args = Args::parse();
    let runner = Runner::default();

    let _ = fs::remove_dir_all(PDF_DIR);

    //Only generate the ones that match the subset selected
    let filter_replace = |f: &TestFile| {
        args.subset
            .as_ref()
            .map_or(true, |r| r.is_match(f.as_raw_path().as_path().to_str().unwrap()))
    };

    let svg_files: Vec<TestFile> = if args.full || args.subset.is_some() {
        // Get all svg files
        (*SVG_FILES)
            .iter()
            .map(|f| TestFile::new(f))
            .filter(filter_replace)
            .collect()
    } else {
        // Only get svg files with existing references
        let existing_svg_references: Vec<TestFile> =
            (*REF_FILES).iter().map(|f| TestFile::new(f)).collect();
        (*SVG_FILES)
            .iter()
            .map(|f| TestFile::new(f))
            .filter(|f| existing_svg_references.contains(f))
            .filter(filter_replace)
            .collect()
    };

    let number_of_svg_files = (*svg_files).len() as u64;

    if !args.full {
        println!(
            "Regenerating {} of {} reference images...",
            number_of_svg_files,
            (*SVG_FILES).len() as u64
        );
    } else {
        println!("Generating {} reference images...", number_of_svg_files);
    }

    let progress_style = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:60.yellow} {pos:>7}/{len:7} {msg}",
    )
    .unwrap();
    let progress_bar = ProgressBar::new(number_of_svg_files);
    progress_bar.set_style(progress_style);

    for svg_file in &svg_files {
        let input_path = svg_file.as_svg_path();
        let output_path = svg_file.as_references_path();

        progress_bar.set_message(svg_file.as_raw_path().to_str().unwrap().to_string());

        let (pdf, image) = runner.convert_svg(
            &fs::read_to_string(input_path.to_str().unwrap()).unwrap(),
            &runner,
        );

        if args.pdf {
            // We just write the pdfs.
            let pdf_path = svg_file.as_pdf_path();
            fs::create_dir_all(pdf_path.as_path().parent().unwrap()).unwrap();
            fs::write(pdf_path.as_path(), pdf).unwrap();
        } else {
            // Otherwise we write the actual reference images.
            fs::create_dir_all(output_path.as_path().parent().unwrap()).unwrap();
            save_image(&image, &output_path);
        }

        progress_bar.inc(1);
    }

    progress_bar.finish();
    println!("Reference images have been generated successfully.");
}
