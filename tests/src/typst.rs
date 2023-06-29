use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::fs;

use svg2pdf_tests::*;

fn main() {
    let existing_svg_references: Vec<TestFile> =
        (*REF_FILES).iter().map(|f| TestFile::new(f)).collect();


    let result = (*SVG_FILES)
        .iter()
        .map(|f| TestFile::new(f))
        .filter(|f| existing_svg_references.contains(f))
        .map(|f| format!("\"{}\"", f.as_svg_path().to_str().unwrap()))
        .collect::<Vec<String>>()
        .join(",\n");

    fs::write("typst.typ", result).unwrap();

}
