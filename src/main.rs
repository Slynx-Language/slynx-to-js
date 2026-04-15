use std::path::PathBuf;
mod compiler;
mod js;

use clap::Parser;
use color_eyre::eyre::Result;
pub use compiler::*;
pub use js::*;
#[derive(Debug, Parser)]
struct Cli {
    #[arg(short, long)]
    target: String,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let path = PathBuf::from(cli.target);
    let _ctx = slynx::compile_to_ir(path)?;
    Ok(())
}
