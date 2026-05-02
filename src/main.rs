use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
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
    #[arg(short, long)]
    output: String,
    #[arg(short)]
    include_files: Vec<String>,
}

struct CompilationPhase {
    included_files: Vec<File>,
    target: PathBuf,
}

impl CompilationPhase {
    pub fn new<P: Into<PathBuf>>(target: P, files: Vec<File>) -> Self {
        Self {
            included_files: files,
            target: target.into(),
        }
    }
    pub fn compile_to<P: AsRef<Path>>(self, target: P) -> Result<()> {
        let mut out = String::new();
        for mut file in self.included_files {
            file.read_to_string(&mut out)?;
        }
        let compiled = JsCompiler::compile(slynx::compile_to_ir(self.target)?)?;
        out.push_str(&compiled);
        std::fs::write(target, out)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let files = cli
        .include_files
        .iter()
        .map(|v| std::fs::OpenOptions::new().read(true).open(v))
        .collect::<Result<_, _>>()?;
    let phase = CompilationPhase::new(cli.target, files);
    phase.compile_to(cli.output)
}
