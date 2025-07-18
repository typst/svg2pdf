use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

/// The character typically used to separate path components
/// in environment variables.
const ENV_PATH_SEP: char = if cfg!(windows) { ';' } else { ':' };

#[derive(Debug, Parser)]
#[clap(about, version)]
pub struct CliArguments {
    /// The command to run
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Sets the level of logging verbosity:
    /// -v = warning & error, -vv = info, -vvv = debug, -vvvv = trace
    #[clap(short, long, action = ArgAction::Count)]
    pub verbosity: u8,
    /// Path to read SVG file from.
    pub input: Option<PathBuf>,
    /// Path to write PDF file to.
    pub output: Option<PathBuf>,
    /// The number of SVG pixels per PDF points.
    #[clap(long, default_value = "72.0")]
    pub dpi: f32,
    /// Whether text should be converted to paths
    /// before embedding it into the PDF.
    #[clap(long, short, action=ArgAction::SetTrue)]
    pub text_to_paths: bool,
    /// How much raster images of rasterized effects should be scaled up.
    #[clap(long, default_value = "1.5")]
    pub raster_scale: f32,

    /// Common font arguments.
    #[command(flatten)]
    pub font: FontsArgs,
}

// What to do.
#[derive(Debug, Clone, Subcommand)]
#[command()]
pub enum Command {
    /// Lists all discovered fonts in system
    Fonts(FontsCommand),
}

/// Lists all discovered fonts in system.
#[derive(Debug, Clone, Parser)]
pub struct FontsCommand {
    /// Also lists style variants of each font family
    #[arg(long)]
    pub all: bool,

    /// Common font arguments.
    #[command(flatten)]
    pub font: FontsArgs,
}

/// Common arguments to customize available fonts.
#[derive(Debug, Clone, Parser)]
pub struct FontsArgs {
    /// Adds additional directories to search for fonts.
    ///
    /// If multiple paths are specified, they are separated by the system's path
    /// separator (`:` on Unix-like systems and `;` on Windows).
    #[arg(long = "font-path", value_name = "DIR", value_delimiter = ENV_PATH_SEP)]
    pub font_paths: Vec<PathBuf>,
}
