use std::io::{self, Write};
use std::path::Path;
use std::process;

use clap::Parser;
use svg2pdf::Options;
use termcolor::{ColorChoice, ColorSpec, StandardStream, WriteColor};
use usvg::{TreeParsing, TreeTextToPath};

mod args;

fn main() {
    if let Err(msg) = run() {
        print_error(&msg).unwrap();
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = args::Args::parse();

    let name =
        Path::new(args.input.file_name().ok_or("Input path does not point to a file")?);
    let output = args.output.unwrap_or_else(|| name.with_extension("pdf"));

    let svg =
        std::fs::read_to_string(&args.input).map_err(|_| "Failed to load SVG file")?;

    let options = usvg::Options::default();
    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    let mut tree = usvg::Tree::from_str(&svg, &options).map_err(|err| err.to_string())?;
    tree.convert_text(&fontdb);
    tree.calculate_bounding_boxes();

    let pdf = svg2pdf::convert_tree(
        &tree,
        Options {
            dpi: args.dpi,
            raster_effects: 1.0,
            ..Options::default()
        },
    );

    std::fs::write(output, pdf).map_err(|_| "Failed to write PDF file")?;

    Ok(())
}

fn print_error(msg: &str) -> io::Result<()> {
    let mut w = StandardStream::stderr(ColorChoice::Always);

    let mut color = ColorSpec::new();
    color.set_fg(Some(termcolor::Color::Red));
    color.set_bold(true);
    w.set_color(&color)?;
    write!(w, "error")?;

    w.reset()?;
    writeln!(w, ": {msg}.")
}
