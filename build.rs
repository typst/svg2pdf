use std::{env, path::Path};

use clap::{CommandFactory, ValueEnum};
use clap_complete::{generate_to, Shell};

mod args {
    include!("src/cli.rs");
}

fn main() -> Result<(), std::io::Error> {
    if !cfg!(feature = "cli") {
        return Ok(());
    }

    let outdir_str = match env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => outdir,
    };

    // Put the files in the same level as the binary (e.g. /target/debug folder)
    let outdir_path = &Path::new(&outdir_str).ancestors().nth(3).unwrap();

    let mut cmd = args::Args::command();

    let man = clap_mangen::Man::new(cmd.clone());
    let mut manpage_file = std::fs::File::create(outdir_path.join("svg2pdf.1"))?;
    man.render(&mut manpage_file)?;

    for shell in Shell::value_variants() {
        generate_to(*shell, &mut cmd, "svg2pdf", &outdir_path).unwrap();
    }

    Ok(())
}
