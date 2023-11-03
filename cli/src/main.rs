mod args;
mod convert;
mod fonts;

use crate::args::{CliArguments, Command};
use clap::Parser;
use std::{
    io::{self, Write},
    process,
};
use termcolor::{ColorChoice, ColorSpec, StandardStream, WriteColor};

fn main() {
    if let Err(msg) = run() {
        print_error(&msg).unwrap();
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = CliArguments::parse();

    // If an input argument was provided, convert the svg file to pdf.
    if let Some(input) = args.input {
        return convert::convert_(&input, args.output, args.dpi);
    };

    // Otherwise execute the command provided if any.
    if let Some(command) = args.command {
        match command {
            Command::Fonts(command) => crate::fonts::fonts(&command)?,
        }
    } else {
        return Err("no command was provided".to_string());
    };

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
