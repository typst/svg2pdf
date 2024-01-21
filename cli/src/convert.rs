use crate::args::ConvertCommand;
use std::path::{Path, PathBuf};
use svg2pdf::Options;
use usvg::{TreeParsing, TreeTextToPath};

/// Execute a font listing command.
pub fn _convert(command: ConvertCommand) -> Result<(), String> {
    convert_(&command.input, command.output, command.dpi)
}

pub fn convert_(
    input: &PathBuf,
    output: Option<PathBuf>,
    dpi: f32,
) -> Result<(), String> {
    // Prepare the font database.
    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    // Convert the file.
    let name = Path::new(input.file_name().ok_or("Input path does not point to a file")?);
    let output = output.unwrap_or_else(|| name.with_extension("pdf"));

    let svg = std::fs::read_to_string(input).map_err(|_| "Failed to load SVG file")?;

    let options = usvg::Options::default();

    let mut tree = usvg::Tree::from_str(&svg, &options).map_err(|err| err.to_string())?;
    tree.convert_text(&fontdb);

    let pdf = svg2pdf::convert_tree(&tree, Options { dpi, ..Options::default() });

    std::fs::write(output, pdf).map_err(|_| "Failed to write PDF file")?;

    Ok(())
}
