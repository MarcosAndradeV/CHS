#![allow(dead_code)]
use std::{
    fs,
    io::{self, Write as _},
    path::PathBuf,
    process::{Command, exit},
};

use argh::FromArgs;

use crate::{
    codegen::{Backend, Target, genereate},
    ir::lower_ast_to_tac,
    parser::parse,
    sema::analyse_module,
};

mod codegen;
mod ir;
mod parser;
mod sema;
mod types;

#[derive(FromArgs)]
/// chs - The CHS programing language tool
struct CliArgs {
    #[argh(subcommand)]
    command: CliCommand,
}

#[derive(FromArgs, Debug)]
#[argh(name = "compile")]
#[argh(subcommand)]
/// compile
struct Compile {
    #[argh(positional)]
    /// input file
    file_path: String,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum CliCommand {
    Compile(Compile),
}

fn main() -> Result<(), ()> {
    let cli: CliArgs = argh::from_env();
    match cli.command {
        CliCommand::Compile(Compile { file_path }) => compile(file_path)?,
    }
    Ok(())
}

fn compile(file_path: String) -> Result<(), ()> {
    let file_path = PathBuf::from(file_path);
    let source = fs::read_to_string(&file_path).map_err(|err| {
        eprintln!("{err}");
        exit(1)
    })?;
    let m = parse(file_path.clone(), &source).map_err(|err| eprintln!("{err}"))?;
    dbg!(&m);
    let db = analyse_module(&m).map_err(|err| eprintln!("{err}"))?;
    dbg!(&db);
    let program = lower_ast_to_tac(&db, m).map_err(|err| eprintln!("{err}"))?;
    dbg!(&program);
    let output = genereate(&db, program, Target::X86_64_LINUX, Backend::FASM)
        .map_err(|err| eprintln!("{err}"))?;
    dbg!(&output);

    fs::create_dir_all("build").map_err(|err| eprintln!("{err}"))?;
    let output_asm_path = "build/out.asm";
    let output_o_path = "build/out.o";
    let output_path = "build/a.out";
    fs::write(output_asm_path, output).map_err(|err| {
        eprintln!("{err}");
        exit(1)
    })?;
    let output = Command::new("fasm")
        .arg(output_asm_path)
        .arg(output_o_path)
        .output()
        .map_err(|err| {
            eprintln!("{err}");
            exit(1)
        })?;
    println!("status: {}", output.status);
    _ = io::stdout().write_all(&output.stdout);
    _ = io::stderr().write_all(&output.stderr);

    let output = Command::new("cc")
        .arg("-o")
        .arg(output_path)
        .arg(output_o_path)
        .output()
        .map_err(|err| {
            eprintln!("{err}");
            exit(1)
        })?;
    println!("status: {}", output.status);
    _ = io::stdout().write_all(&output.stdout);
    _ = io::stderr().write_all(&output.stderr);
    Ok(())
}
