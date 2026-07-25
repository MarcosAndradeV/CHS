use std::{
    env::temp_dir,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    process::{Command, exit},
};

use clap::{Parser, Subcommand};
use compiler::CompilerProcess;
use diagnostic::ChsResult;

const BIN_NAME: &str = env!("CARGO_BIN_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ChsResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build {
            path,
            verbose,
            output,
        } => {
            let mut cp = CompilerProcess::new();
            cp.add_default_search_paths()?;
            cp.add_source(path)?;
            cp.set_verbose(verbose);
            cp.set_target_name(output);
            cp.compile()?;
        }
        Commands::Run { path } => {
            let mut cp = CompilerProcess::new();
            cp.add_default_search_paths()?;
            let mut default_hasher = DefaultHasher::new();
            path.hash(&mut default_hasher);
            let target_name = temp_dir().join(format!("chs_{}", default_hasher.finish() as u32));
            cp.set_target_name(target_name);
            cp.add_source(path)?;
            cp.compile()?;
            let status = Command::new(cp.target_name()).status()?;
            if status.success() {
                exit(0);
            } else {
                exit(1);
            }
        }
        Commands::Clear => {
            _ = fs::remove_dir_all(".build");
        }
        Commands::Version => {
            println!("version: {BIN_NAME}-{PKG_VERSION}",);
        }
    }
    Ok(())
}

#[derive(Parser)]
#[command(name = "chs")]
#[command(about = "Chs managing tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile mdoule and dependencies
    #[command(visible_alias = "b")]
    Build {
        path: PathBuf,
        #[arg(short)]
        verbose: bool,
        #[arg(short, long, default_value = "out")]
        output: PathBuf,
    },
    /// Compile and run program
    #[command(visible_alias = "r")]
    Run { path: PathBuf },
    /// Clear the project artifacts
    Clear,
    /// Print version
    #[command(version)]
    Version,
}
