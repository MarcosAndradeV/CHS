#![allow(clippy::result_unit_err)]

use std::process::Command;
use std::process::exit;

use compiler::CompilerProcess;
use compiler::diag::ChsResult;

fn main() {
    if run_args().is_err() {
        exit(1);
    }
}

pub fn run_args() -> ChsResult<()> {
    let bin_path = std::env::args().next().unwrap_or_else(|| "chs".to_string());
    let bin_name = std::path::Path::new(&bin_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("chs");

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help(bin_name);
        exit(0);
    }

    let mut cp = CompilerProcess::new();
    let mut run = false;
    let mut has_sources = false;
    cp.add_default_search_paths()?;

    let mut args_iter = args.into_iter();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--check" => {
                cp.just_check = true;
            }
            "--verbose" | "-v" => {
                cp.verbose = true;
            }
            "--output" | "-o" => {
                if let Some(output_val) = args_iter.next() {
                    cp.target_name = output_val;
                } else {
                    eprintln!("Error: Expected value after output flag");
                    return Err(());
                }
            }
            "--search-path" | "-S" => {
                if let Some(search_path) = args_iter.next() {
                    cp.add_search_path(search_path.into())?;
                } else {
                    eprintln!("Error: Expected value after -S flag");
                    return Err(());
                }
            }
            "-r" | "--run" => {
                run = true;
            }
            arg if arg.starts_with("-") => {
                eprintln!("Error: Unknown option: {}", arg);
                eprintln!("Use --help for usage information.");
                return Err(());
            }
            _ => {
                cp.add_source(arg.into())?;
                has_sources = true;
            }
        }
    }

    if !has_sources {
        eprintln!("Error: No input files specified.");
        eprintln!("Usage: {} [OPTIONS] <source_files...>", bin_name);
        eprintln!("Use --help for more information.");
        return Err(());
    }

    cp.compile()?;
    if run {
        match Command::new(format!(".build/{}", cp.target_name)).status() {
            Ok(exit_status) => {
                exit(exit_status.code().unwrap_or_default());
            }
            Err(e) => {
                println!("{e}");
                exit(1);
            }
        }
    }
    Ok(())
}

fn print_help(bin_name: &str) {
    println!(
        "chs - Compiler for the Chs programming language\n\n\
Usage: {} [OPTIONS] <source_files...>\n\n\
Arguments:\n\
  <source_files...>      The source file(s) to compile (.chs)\n\n\
Options:\n\
  -o, --output <VAL>     Specify the target output executable/object name (default: \"out\")\n\
  -S, --search-path <DIR> Add a directory to the search path for module resolution\n\
      --check            Check the program for errors without compiling or generating code\n\
  -r, --run              Run the compiled executable after compilation\n\
  -v, --verbose          Enable verbose compiler output\n\
  -h, --help             Print help information",
        bin_name
    );
}
