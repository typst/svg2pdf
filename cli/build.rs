use std::{env, path::Path};

use clap::{CommandFactory, ValueEnum};
use clap_complete::{generate_to, Shell};

mod args {
    include!("src/args.rs");
}

fn main() -> Result<(), std::io::Error> {
    let outdir_str = match env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => outdir,
    };

    // Put the files in the artifacts folder (/target/artifacts)
    let outdir_path = &Path::new(&outdir_str).ancestors().nth(3).unwrap();
    let artifacts_path = outdir_path.join("artifacts");
    std::fs::create_dir_all(&artifacts_path)?;

    let mut cmd = args::Args::command();

    let man = clap_mangen::Man::new(cmd.clone());
    let mut manpage_file = std::fs::File::create(artifacts_path.join("svg2pdf.1"))?;
    man.render(&mut manpage_file)?;

    for shell in Shell::value_variants() {
        generate_to(*shell, &mut cmd, "svg2pdf", &artifacts_path).unwrap();
    }

    Ok(())
}
