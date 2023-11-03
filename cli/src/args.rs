use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

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
pub struct ConvertCommand {
    /// Path to read SVG file from.
    pub input: PathBuf,
    /// Path to write PDF file to.
    pub output: Option<PathBuf>,
    /// The number of SVG pixels per PDF points.
    #[clap(long, default_value = "72.0")]
    pub dpi: f32,
}

/// Lists all discovered fonts in system.
#[derive(Debug, Clone, Parser)]
pub struct FontsCommand {
    /// Also lists style variants of each font family
    #[arg(long)]
    pub all: bool,
}
