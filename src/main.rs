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

    #[arg(short, long)]
    print: bool,
    #[arg(long)]
    ir: bool,
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
    pub fn compile_to<P: AsRef<Path>>(self, target: P, print: bool) -> Result<String> {
        let mut out = String::new();
        for mut file in self.included_files {
            file.read_to_string(&mut out)?;
        }
        let ir = slynx::compile_to_ir(self.target)?;
        if print {
            println!("{}", ir.format_sir());
        }
        let compiled = JsCompiler::compile(ir)?;
        out.push_str(&compiled);
        std::fs::write(target, &out)?;
        Ok(out)
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

    let out = phase.compile_to(cli.output, cli.ir)?;
    if cli.print {
        println!("{out}");
    }
    Ok(())
}
