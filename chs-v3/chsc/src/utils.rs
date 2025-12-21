use std::ffi::OsStr;
use std::process::{exit, Command, Output};
use std::env;

use crate::Compiler;

pub const DEFAULT_STDLIB_PATH: &str = "stdlib";
pub const DEFAULT_RUNTIME_PATH: &str = "stdlib/runtime";

pub fn get_stdlib_path() -> String {
    env::var("CHS_STDLIB_PATH").unwrap_or_else(|_| DEFAULT_STDLIB_PATH.to_string())
}

pub fn get_runtime_path() -> String {
    env::var("CHS_RUNTIME_PATH").unwrap_or_else(|_| DEFAULT_RUNTIME_PATH.to_string())
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum Backend {
    #[default] FASM,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum Os {
    #[default] LINUX,
    WINDOWS,
    MACOS,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum Arch {
    #[default] X86_64,
    AARCH64,
}

pub fn parse_backend() -> Backend {
    match env::var("CHSBACKEND").as_deref() {
        Ok("FASM") => Backend::FASM,
        _ => Backend::FASM, // Default
    }
}

pub fn parse_os() -> Os {
    match env::var("CHSOS").as_deref() {
        Ok("LINUX") => Os::LINUX,
        Ok("WINDOWS") => Os::WINDOWS,
        Ok("MACOS") => Os::MACOS,
        _ => Os::LINUX, // Default
    }
}

pub fn parse_arch() -> Arch {
    match env::var("CHSARCH").as_deref() {
        Ok("X86_64") => Arch::X86_64,
        Ok("AARCH64") => Arch::AARCH64,
        _ => Arch::X86_64, // Default
    }
}


pub fn run_fasm<I, O>(input_path: I, output_path: O) -> Result<(), AppError>
where
    I: AsRef<OsStr>,
    O: AsRef<OsStr>,
{
    let input_str = input_path.as_ref().to_string_lossy();

    if !std::path::Path::new(&input_str.as_ref()).exists() {
        return Err(AppError::InvalidPath {
            path: input_str.to_string(),
        });
    }

    let mut command = Command::new("fasm");
    command.arg(input_path).arg(output_path);

    let output = command.output().map_err(|e| AppError::ProcessError {
        command: "fasm".to_string(),
        error: e,
    })?;

    if !output.status.success() {
        return Err(AppError::CompilationError {
            command: "fasm".to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

pub fn run_cc<I, O, Args, S>(input_path: I, output_path: O, args: Args) -> Result<(), AppError>
where
    I: AsRef<OsStr>,
    O: AsRef<OsStr>,
    Args: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let input_str = input_path.as_ref().to_string_lossy();

    if !std::path::Path::new(&input_str.as_ref()).exists() {
        return Err(AppError::InvalidPath {
            path: input_str.to_string(),
        });
    }

    let mut command = Command::new("cc");
    command.arg(input_path).arg("-o").arg(output_path).args(args);

    let output = command.output().map_err(|e| AppError::ProcessError {
        command: "cc".to_string(),
        error: e,
    })?;

    if !output.status.success() {
        return Err(AppError::CompilationError {
            command: "cc".to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

pub fn run_exe<I>(input_path: I) -> Result<(i32, String, String), AppError>
where
    I: AsRef<OsStr>,
{
    let output: Output = Command::new(&input_path).output().map_err(|e| {
        AppError::IoError(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to start process '{}': {}",
                input_path.as_ref().to_string_lossy(),
                e
            ),
        ))
    })?;


        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);
        Ok((code, stdout, stderr))

}

#[derive(Debug)]
pub enum AppError {
    ProcessError {
        command: String,
        error: std::io::Error,
    },
    CompilationError {
        command: String,
        stdout: String,
        stderr: String,
    },
    InvalidPath {
        path: String,
    },
    IoError(std::io::Error),
    Utf8Error(String),
    ExecutionFailed(i32, String, String), // (exit code, stderr)
    InterpreterExit(i32),
    ArgumentError(String),
    FileError {
        path: String,
        error: std::io::Error,
    },
    ParseError{
        path: String,
        error: String,
    },
    TypeError(String),
    GenerationError(Option<String>, String),
    ConfigParseError(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::ProcessError { command, error } => {
                writeln!(f, "Failed to execute '{}': {}", command, error)
            }
            AppError::CompilationError {
                command,
                stdout,
                stderr,
            } => {
                writeln!(
                    f,
                    "'{}' failed:\nStdout: {}\nStderr: {}",
                    command, stdout, stderr
                )
            }
            AppError::InvalidPath { path } => {
                writeln!(f, "Invalid path: {}", path)
            }
            AppError::IoError(e) => writeln!(f, "IO error: {}", e),
            AppError::Utf8Error(e) => writeln!(f, "UTF-8 conversion error: {}", e),
            AppError::ExecutionFailed(code, stdout, stderr) => {
                write!(
                    f,
                    "Process failed (exit code {}):\n{}{}",
                    code, stdout, stderr
                )
            }
            AppError::ArgumentError(msg) => writeln!(f, "Argument error: {}", msg),
            AppError::FileError { path, error } => writeln!(f, "File error '{}': {}", path, error),
            AppError::ParseError{path, error } => writeln!(f, "{path}: {error}"),
            AppError::TypeError(msg) => writeln!(f, "Type error: {}", msg),
            AppError::GenerationError(hint, msg) => {
                writeln!(f, "Generation Error: {msg}");
                if let Some(hint) = hint {
                    writeln!(f, "Hint: {hint}");
                }
                Ok(())
            }
            AppError::ConfigParseError(msg) => {
                writeln!(f, "Parse configuration file failed: {}", msg)
            }
            AppError::InterpreterExit(_) => {
                Ok(())
            }
        }
    }
}

impl std::error::Error for AppError {}

pub fn validate_input_file(c: &Compiler, file_path: &str) -> Result<(), ()> {
    let path = std::path::Path::new(file_path);

    if !path.exists() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "File does not exist");
        c.compiler_error(format!("File error '{}': {}", path.display(), error))?;
    }

    if !path.is_file() {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidInput, "Path is not a file");
        c.compiler_error(format!("File error '{}': {}", path.display(), error))?;
    }

    Ok(())
}

pub fn handle_app_error(error: &AppError) {
    eprint!("{error}");
}
