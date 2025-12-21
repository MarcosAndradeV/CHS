#![allow(unused)]

use std::collections::HashMap;
use std::fs::{self, exists};
use std::path::PathBuf;
use std::process::Command;

use ir::{Func, Program};

use crate::ast::{Module, Name};
use crate::cli::Cli;
use crate::ir::{ExternFunc, GlobalVar};
use crate::lower_ast::lower_ast_to_ir;
use crate::parser::parse_module;
use crate::utils::*;

mod ast;
mod chslexer;
mod cli;
mod generator;
mod ir;
mod lower_ast;
mod opt;
mod parser;
mod type_checker;
mod utils;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let c = Compiler::new();
    if app(&c).is_err() {
        c.report_errors();
    }
}

fn app<'src>(c: &'src Compiler<'src>) -> Result<(), ()> {
    let cli = Cli::parse()?;

    match cli.command {
        cli::Command::Help => {
            cli.usage();
            return Ok(());
        }
        cli::Command::Compile { input_path } => {
            let exe_path = compile(c, input_path)?;
            println!("Generated {}", exe_path.display());
        }
        cli::Command::CompileRun { input_path } => {
            let exe_path = compile(c, input_path)?;
            run(exe_path);
        }
        cli::Command::Version => {
            println!("chsc version: {VERSION}");
            return Ok(());
        }
    }

    Ok(())
}

fn run(exe_path: PathBuf) {
    let output = Command::new(&exe_path).output().unwrap();
    print!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn compile<'src>(c: &'src Compiler<'src>, file_path: String) -> Result<PathBuf, ()> {
    if !exists(&c.libchs_a).expect("Can't check existence of file libchs.a") {
        let output = Command::new("cc")
            .arg("-c")
            .arg(format!("{}/chs.c", &c.runtime_path))
            .arg("-static")
            .arg("-o")
            .arg(format!("{}/libchs.a", &c.runtime_path))
            .arg("-O3")
            .output()
            .unwrap();

        if !output.status.success() {
            c.compiler_error(format!(
                "cc failed: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))?;
        }
    }
    let file_path = c.add_file_path(file_path);
    let source = c.read_source_file(&file_path)?.unwrap();
    parse_module(c, &file_path, &source)?;
    lower_ast_to_ir(c)?;
    opt::strip_unused_functions(c.program.borrow_mut());
    opt::strip_unused_variables(c.program.borrow_mut());
    if c.has_errors() {
        return Err(());
    }
    let input_path = PathBuf::from(file_path);
    let o_path = input_path.with_extension("o");
    let exe_path = input_path.with_extension("");
    match parse_backend() {
        Backend::FASM => {
            let asm_path = input_path.with_extension("asm");
            let asm_code = generator::fasm_generator::generate(c, true).unwrap();
            fs::write(&asm_path, asm_code.to_string())
                .map_err(|e| AppError::FileError {
                    path: asm_path.to_string_lossy().to_string(),
                    error: e,
                })
                .unwrap();

            let output = Command::new("fasm")
                .arg(&asm_path)
                .arg(&o_path)
                .output()
                .unwrap();

            if !output.status.success() {
                c.compiler_error(format!(
                    "fasm failed: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ))?;
            }

            let output = Command::new("cc")
                .arg(&o_path)
                .arg("-o")
                .arg(&exe_path)
                .arg("-l:libchs.a")
                .arg(format!("-L{}", c.runtime_path))
                .output()
                .unwrap();

            if !output.status.success() {
                c.compiler_error(format!(
                    "cc failed: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ))?;
            }
        }
    }
    Ok(exe_path)
}

#[derive(Default)]
struct Compiler<'src> {
    backend: Backend,
    os: Os,
    arch: Arch,
    stdlib_path: String,
    runtime_path: String,
    libchs_a: String,
    diag: MyRefCell<Vec<Diag>>,
    sources: MyRefCell<Vec<String>>,
    file_paths: MyRefCell<Vec<String>>,
    modules: MyRefCell<HashMap<&'src str, Module<'src>>>,
    global_name_space: MyRefCell<HashMap<&'src str, Name<'src>>>,
    program: MyRefCell<Program<'src>>,
}

impl<'src> Compiler<'src> {
    fn new() -> Self {
        let runtime_path = get_runtime_path();
        Self {
            libchs_a: format!("{}/libchs.a", &runtime_path),
            stdlib_path: get_stdlib_path(),
            runtime_path,
            ..Default::default()
        }
    }

    fn compiler_error<T>(&self, format: String) -> Result<T, ()> {
        self.diag.borrow_mut().push(Diag::Error(format));
        Err(())
    }

    fn read_source_file(&'src self, file_path: &'src str) -> Result<Option<&'src str>, ()> {
        validate_input_file(self, file_path)?;
        if self.has_module(file_path) {
            return Ok(None);
        }
        match std::fs::read_to_string(file_path) {
            Ok(source) => {
                let source = self.add_source(source);
                Ok(Some(source))
            }
            Err(e) => self.compiler_error(format!("Unexpected error while reading file: {e}")),
        }
    }

    fn add_source(&'src self, source: String) -> &'src String {
        let sources = self.sources.borrow_mut();
        let len = sources.len();
        sources.push(source.clone());
        unsafe { sources.as_ptr().add(len).as_ref().unwrap() }
    }

    fn add_file_path(&'src self, file_path: String) -> &'src String {
        let file_paths = self.file_paths.borrow_mut();
        let len = file_paths.len();
        file_paths.push(
            // Using full path
            std::fs::canonicalize(&file_path)
                .map(|f| f.to_str().unwrap().to_string())
                .unwrap_or(file_path),
        );
        unsafe { file_paths.as_ptr().add(len).as_ref().unwrap() }
    }

    fn report_errors(&self) {
        let diag = self.diag.borrow_mut();
        if !diag.is_empty() {
            for d in diag.drain(..) {
                match d {
                    Diag::Error(e) => eprintln!("{e}"),
                }
            }
            std::process::exit(1);
        }
    }

    fn has_errors(&self) -> bool {
        let diag = self.diag.borrow();
        diag.iter().find(|d| matches!(d, Diag::Error(..))).is_some()
    }

    fn add_module(&self, file_path: &'src str, module: Module<'src>) {
        self.modules.borrow_mut().insert(file_path, module);
    }

    fn has_module(&self, file_path: &str) -> bool {
        self.modules.borrow().contains_key(file_path)
    }

    fn add_program_extern(&self, r#extern: ExternFunc<'src>) -> usize {
        let externs = &mut self.program.borrow_mut().externs;
        let uid = externs.len();
        externs.push(r#extern);
        uid
    }

    fn add_program_func(&self, r#fn: Func<'src>) -> usize {
        let funcs = &mut self.program.borrow_mut().funcs;
        let uid = funcs.len();
        funcs.push(r#fn);
        uid
    }

    fn add_program_global_var(&self, global_var: GlobalVar<'src>) -> usize {
        let global_vars = &mut self.program.borrow_mut().global_vars;
        let uid = global_vars.len();
        global_vars.push(global_var);
        uid
    }

    fn get_program_global_vars(&self) -> &[GlobalVar<'src>] {
        &self.program.borrow().global_vars
    }

    fn get_program_funcs(&self) -> &[Func<'src>] {
         &self.program.borrow().funcs
    }

    fn get_program_externs(&self) -> &[ExternFunc<'src>] {
      &self.program.borrow().externs
    }
}

enum Diag {
    Error(String),
}

#[derive(Default)]
struct MyRefCell<T: Default> {
    inner: T,
}

impl<T: Default> MyRefCell<T> {
    fn borrow(&self) -> &T {
        unsafe { &*(&self.inner as *const T) }
    }

    fn borrow_mut(&self) -> &mut T {
        unsafe { ((&self.inner as *const T) as *mut T).as_mut().unwrap() }
    }
}
