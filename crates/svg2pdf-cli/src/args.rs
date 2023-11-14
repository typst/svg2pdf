use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(about, version)]
pub struct Args {
    /// Path to read SVG file from.
    pub input: PathBuf,
    /// Path to write PDF file to.
    pub output: Option<PathBuf>,
    /// The number of SVG pixels per PDF points.
    #[clap(long, default_value = "72.0")]
    pub dpi: f32,
}
