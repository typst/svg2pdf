use std::path::{Path, PathBuf};
use std::sync::Arc;
use svg2pdf::{ConversionOptions, PageOptions};

pub fn convert_(
    input: &PathBuf,
    output: Option<PathBuf>,
    conversion_options: ConversionOptions,
    page_options: PageOptions,
) -> Result<(), String> {
    if let Ok(()) = log::set_logger(&LOGGER) {
        log::set_max_level(log::LevelFilter::Warn);
    }

    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    fontdb.set_serif_family("Times New Roman");
    fontdb.set_sans_serif_family("Arial");
    fontdb.set_cursive_family("Comic Sans MS");
    fontdb.set_fantasy_family("Impact");
    fontdb.set_monospace_family("Courier New");

    #[cfg(feature = "text")]
    let options = usvg::Options {
        fontdb: Arc::new(fontdb),
        ..usvg::Options::default()
    };

    #[cfg(not(feature = "text"))]
    let options = usvg::Options::default();

    // Convert the file.
    let name = Path::new(input.file_name().ok_or("Input path does not point to a file")?);
    let output = output.unwrap_or_else(|| name.with_extension("pdf"));

    let svg = std::fs::read_to_string(input).map_err(|_| "Failed to load SVG file")?;

    let tree = usvg::Tree::from_str(&svg, &options).map_err(|err| err.to_string())?;

    let pdf = svg2pdf::to_pdf(&tree, conversion_options, page_options);

    std::fs::write(output, pdf).map_err(|_| "Failed to write PDF file")?;

    Ok(())
}

// Taken from resvg
/// A simple stderr logger.
static LOGGER: SimpleLogger = SimpleLogger;
struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::LevelFilter::Warn
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let target = if !record.target().is_empty() {
                record.target()
            } else {
                record.module_path().unwrap_or_default()
            };

            let line = record.line().unwrap_or(0);
            let args = record.args();

            match record.level() {
                log::Level::Error => {
                    eprintln!("Error (in {}:{}): {}", target, line, args)
                }
                log::Level::Warn => {
                    eprintln!("Warning (in {}:{}): {}", target, line, args)
                }
                log::Level::Info => eprintln!("Info (in {}:{}): {}", target, line, args),
                log::Level::Debug => {
                    eprintln!("Debug (in {}:{}): {}", target, line, args)
                }
                log::Level::Trace => {
                    eprintln!("Trace (in {}:{}): {}", target, line, args)
                }
            }
        }
    }

    fn flush(&self) {}
}
