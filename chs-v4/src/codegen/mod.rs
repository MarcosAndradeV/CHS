use std::fmt::Write;

use crate::ir::tac::{Instr, Proc, Program};
use crate::types::Database;

pub fn genereate(
    db: &Database,
    program: Program,
    target: Target,
    backend: Backend,
) -> Result<String, CodegenError> {
    match target {
        Target::X86_64_LINUX => genereate_x86_64_linux(db, program, backend),
    }
}

pub fn genereate_x86_64_linux(
    db: &Database,
    program: Program,
    backend: Backend,
) -> Result<String, CodegenError> {
    let mut out = String::new();
    match backend {
        Backend::C => todo!(),
        Backend::FASM => {
            _ = writeln!(out, "format ELF64");
            _ = writeln!(out, "section '.text' executable");
        },
    }
    for proc in program.procs {
        genereate_proc_x86_64_linux(&mut out, db, proc, backend)?;
    }
    Ok(out)
}

pub fn genereate_proc_x86_64_linux(
    out: &mut impl Write,
    _db: &Database,
    proc: Proc,
    backend: Backend,
) -> Result<(), CodegenError> {
    match backend {
        Backend::C => todo!(),
        Backend::FASM => {
            _ = writeln!(out, "public _{} as '{}'", proc.name, proc.name);
            _ = writeln!(out, "_{}:", proc.name);
            _ = writeln!(out, "\tpush rbp");
            _ = writeln!(out, "\tmov rbp, rsp");
        },
    }

    for instr in proc.instrs {
        match instr {
            Instr::Ret => {
                generate_ret_x86_64_linux(out, backend);
            }
        }
    }

    // generate_ret_x86_64_linux(out, backend);
    Ok(())
}

fn generate_ret_x86_64_linux(out: &mut impl Write, backend: Backend) {
    match backend {
        Backend::C => todo!(),
        Backend::FASM => {
            _ = writeln!(out, "\tmov rsp, rbp");
            _ = writeln!(out, "\tpop rbp");
            _ = writeln!(out, "\tret");
        },
    }
}

pub struct CodegenError(pub String);

impl std::fmt::Debug for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CodegenError {}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub enum Target {
    X86_64_LINUX,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub enum Backend {
    C,
    FASM,
}
