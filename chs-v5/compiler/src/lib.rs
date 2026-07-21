#![allow(clippy::result_unit_err)]

use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::diag::*;
use crate::syntax::ast;
use crate::syntax::ast::Library;
use crate::syntax::parse_file;

pub mod codegen;
pub mod ir;
pub mod semantics;
mod syntax;
pub mod types;

pub mod diag;

#[cfg(test)]
mod tests;

pub fn get_std_path() -> PathBuf {
    // Development path relative to Cargo.toml
    #[cfg(feature = "development")]
    {
        return PathBuf::from("../std");
    }
    if let Ok(path) = std::env::var("CHS_HOME") {
        return PathBuf::from_iter(&[path.as_str(), "std"]);
    }
    PathBuf::from("./std")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenBackend {
    Qbe,
}

pub struct CompilerProcess {
    sources: Vec<PathBuf>,
    search_paths: HashSet<PathBuf>,
    foreign_libraries: HashMap<String, Library>,
    is_main: bool,
    pub target_name: String,
    pub backend: CodegenBackend,
    pub verbose: bool,
    pub just_check: bool,
}

impl Default for CompilerProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerProcess {
    pub fn new() -> Self {
        Self {
            search_paths: HashSet::new(),
            foreign_libraries: HashMap::new(),
            sources: Vec::new(),
            is_main: true,
            target_name: String::from("out"),
            backend: CodegenBackend::Qbe,
            verbose: false,
            just_check: false,
        }
    }

    pub fn spawn_child(&self, target_name: String) -> Self {
        Self {
            target_name,
            is_main: false,
            sources: Vec::new(),
            search_paths: self.search_paths.clone(),
            foreign_libraries: self.foreign_libraries.clone(),
            verbose: self.verbose,
            backend: self.backend,
            just_check: self.just_check,
        }
    }

    pub fn add_default_search_paths(&mut self) -> ChsResult<()> {
        self.add_search_path(handle_error!(env::current_dir())?)?;
        self.add_search_path(get_std_path())?;
        self.add_search_path(get_std_path().join("runtime"))?;
        Ok(())
    }

    pub fn add_source(&mut self, source: PathBuf) -> ChsResult<()> {
        let source = fs::canonicalize(&source).unwrap_or(source);
        self.sources.push(source);
        Ok(())
    }

    pub fn add_search_path(&mut self, file_path: PathBuf) -> ChsResult<()> {
        let file_path = fs::canonicalize(&file_path).unwrap_or(file_path);
        self.search_paths.insert(file_path);
        Ok(())
    }

    fn compile_impl(
        &mut self,
        compiled_paths: &mut HashSet<PathBuf>,
        linked_objects: &mut Vec<PathBuf>,
        reporter: &mut DiagnosticReporter,
    ) -> ChsResult<Vec<ast::FunctionDecl>> {
        let mut file_queue = self.sources.clone();
        let mut merged_ast_items = Vec::new();

        while let Some(fp) = file_queue.pop() {
            let fp = fs::canonicalize(&fp).unwrap_or(fp);
            if !compiled_paths.insert(fp.clone()) {
                continue; // Already processed
            }

            let source = handle_error!(fs::read_to_string(&fp))?;
            let ast = match parse_file(&fp, &source, reporter) {
                Ok(a) => a,
                Err(_) => {
                    println!("Could not parse source `{}`", fp.display());
                    return Err(());
                }
            };

            for item in ast.items {
                if let ast::FileItem::Import(import_decl) = item {
                    let path_str = import_decl.path.source();
                    let path_str = path_str.trim_matches('"');
                    let mut resolved = None;

                    // Try relative to current file
                    let mut relative = fp.parent().unwrap_or(Path::new(".")).join(path_str);
                    let exists = relative.exists();
                    if !exists {
                        relative = relative.with_extension("chs");
                    }
                    let exists = relative.exists();
                    if exists {
                        resolved = Some(relative);
                    } else {
                        // Try search paths
                        for sp in &self.search_paths {
                            let mut candidate = sp.join(path_str);
                            let exists = candidate.exists();
                            if !exists {
                                candidate = candidate.with_extension("chs")
                            }
                            let exists = candidate.exists();
                            if exists {
                                resolved = Some(candidate);
                                break;
                            }
                        }
                    }

                    if let Some(r) = resolved {
                        let r = fs::canonicalize(&r).unwrap_or(r);
                        if r.is_dir() {
                            if !compiled_paths.contains(&r) {
                                compiled_paths.insert(r.clone());
                                // Spawn child CompilerProcess
                                let target = r.file_name().unwrap().to_string_lossy().into_owned();
                                let mut child = self.spawn_child(target);

                                // Collect all .chs files in directory
                                if let Ok(entries) = fs::read_dir(&r) {
                                    for entry in entries.flatten() {
                                        if entry.path().extension().and_then(|s| s.to_str())
                                            == Some("chs")
                                        {
                                            child.sources.push(entry.path());
                                        }
                                    }
                                }

                                if !child.sources.is_empty() {
                                    let child_exports = child.compile_impl(
                                        compiled_paths,
                                        linked_objects,
                                        reporter,
                                    )?;
                                    for mut exp in child_exports {
                                        exp.body = None;
                                        merged_ast_items.push(ast::FileItem::FunctionDecl(exp));
                                    }
                                }
                                self.foreign_libraries.extend(child.foreign_libraries);
                            }
                        } else if r.is_file() && !compiled_paths.contains(&r) {
                            file_queue.push(r);
                        }
                    } else {
                        println!("Could not resolve import: {}", path_str);
                        return Err(());
                    }
                } else {
                    merged_ast_items.push(item);
                }
            }
        }

        let mut exported_decls = Vec::new();
        for item in &merged_ast_items {
            if let ast::FileItem::FunctionDecl(decl) = item {
                if decl
                    .directives
                    .iter()
                    .find(|d| matches!(d, ast::FunctionDirective::Private))
                    .is_some()
                {
                    continue;
                }
                exported_decls.push(decl.clone());
            }
            if let ast::FileItem::Directive(ast::Directive::Library { name, library }) = item {
                self.foreign_libraries
                    .insert(name.source().to_string(), library.clone());
            }
        }

        // Now translate merged AST

        let mut type_checker = crate::semantics::TypeChecker::new(reporter);
        type_checker.declared_libraries = self.foreign_libraries.keys().cloned().collect();
        if !type_checker.check(&mut merged_ast_items) {
            return Err(());
        }

        if let Some(mut module) =
            ir::translate::translate_ast_items(&merged_ast_items, type_checker.type_db, reporter)
        {
            if reporter.has_errors() {
                return Err(());
            }

            // Optimize function IRs
            for func in module.functions.values_mut() {
                if let ir::Function::Default {
                    blocks,
                    instructions,
                    entry_block,
                    ..
                } = func
                {
                    ir::optimize_ir(blocks, instructions, entry_block);
                }
            }

            // Optimize run block IRs
            // for run_block in &mut module.run_blocks {
            //     ir::optimize_ir(
            //         &mut run_block.blocks,
            //         &mut run_block.instructions,
            //         &mut run_block.entry_block,
            //     );
            // }

            if self.just_check {
                return Ok(Vec::new());
            }

            let out_dir = env::current_dir()
                .unwrap_or(PathBuf::from("."))
                .join(PathBuf::from(".build"));
            std::fs::create_dir_all(&out_dir).unwrap();

            // Execute compile-time #run blocks
            // for (idx, run_block) in module.run_blocks.iter().enumerate() {
            //     let temp_name = format!("{}_run_{}", self.target_name, idx);
            //     let temp_ssa_file = out_dir.join(format!("{}.ssa", temp_name));
            //     let temp_s_file = out_dir.join(format!("{}.s", temp_name));
            //     let temp_exec_file = out_dir.join(&temp_name);

            //     // Create a temporary module containing the AST's types and functions
            //     let mut temp_module = ir::Module::new(module.type_db.clone());
            //     for (func_name, func) in &module.functions {
            //         if func_name != "main" {
            //             temp_module
            //                 .functions
            //                 .insert(func_name.clone(), func.clone());
            //         }
            //     }

            //     // Add the run block as the main function
            //     let run_as_main = ir::Function::Default {
            //         name: "main".to_string(),
            //         signature: ir::Signature {
            //             params: Vec::new(),
            //             return_type: module.type_db.void(),
            //         },
            //         blocks: run_block.blocks.clone(),
            //         instructions: run_block.instructions.clone(),
            //         entry_block: run_block.entry_block,
            //     };
            //     temp_module
            //         .functions
            //         .insert("main".to_string(), run_as_main);

            //     // Transpile to QBE SSA
            //     let transpiler = codegen::qbe::QbeTranspiler::new(&temp_module);
            //     let qbe_code = transpiler.transpile();
            //     std::fs::write(&temp_ssa_file, qbe_code).unwrap();

            //     // Run QBE
            //     let mut qbe_cmd = std::process::Command::new("qbe");
            //     qbe_cmd.arg("-o").arg(&temp_s_file).arg(&temp_ssa_file);
            //     let qbe_status = qbe_cmd.status().unwrap();
            //     if !qbe_status.success() {
            //         println!("Failed to compile QBE SSA code for run block with qbe.");
            //         return Err(());
            //     }

            //     // Link with GCC
            //     let mut gcc_cmd = std::process::Command::new("gcc");
            //     gcc_cmd.arg(&temp_s_file);
            //     gcc_cmd.arg("-o").arg(&temp_exec_file);

            //     // Link external/dependency objects
            //     for obj in linked_objects.iter() {
            //         gcc_cmd.arg(obj);
            //     }

            //     // Link foreign libraries
            //     for library in self.foreign_libraries.values() {
            //         let arg = if library.link_name.starts_with("lib") {
            //             format!("-l:{}", library.link_name)
            //         } else if library.kind.is_static() {
            //             format!("-l:lib{}.a", library.link_name)
            //         } else {
            //             format!("-l{}", library.link_name)
            //         };
            //         gcc_cmd.arg(arg);
            //     }

            //     // Search paths
            //     for path in &self.search_paths {
            //         gcc_cmd.arg("-I").arg(path);
            //         gcc_cmd.arg("-L").arg(path);
            //     }

            //     let gcc_status = gcc_cmd.status().unwrap();
            //     if !gcc_status.success() {
            //         println!("Failed to link run block with gcc.");
            //         return Err(());
            //     }

            //     // Execute run block binary
            //     if self.verbose {
            //         println!("Executing #run block {}...", idx);
            //     }
            //     let run_status = std::process::Command::new(&temp_exec_file)
            //         .status()
            //         .unwrap();

            //     if !run_status.success() {
            //         println!(
            //             "Compile-time run block failed with exit status {:?}",
            //             run_status.code()
            //         );
            //         return Err(());
            //     }

            //     // Clean up temporary run block files
            //     let _ = std::fs::remove_file(temp_ssa_file);
            //     let _ = std::fs::remove_file(temp_s_file);
            //     let _ = std::fs::remove_file(temp_exec_file);
            // }

            let input_file = match self.backend {
                CodegenBackend::Qbe => {
                    // Transpile to QBE SSA
                    let transpiler = codegen::qbe::QbeTranspiler::new(&module);
                    let qbe_code = transpiler.transpile();

                    let ssa_file = out_dir.join(format!("{}.ssa", self.target_name));
                    std::fs::write(&ssa_file, qbe_code).unwrap();

                    if self.verbose {
                        println!("Transpiled QBE SSA code written to {}", ssa_file.display());
                    }

                    let s_file = out_dir.join(format!("{}.s", self.target_name));

                    let mut qbe_cmd = std::process::Command::new("qbe");
                    qbe_cmd.arg("-o").arg(&s_file).arg(&ssa_file);
                    let qbe_status = qbe_cmd.status().unwrap();
                    if !qbe_status.success() {
                        println!("Failed to compile QBE SSA code with qbe.");
                        return Err(());
                    }
                    s_file
                }
            };

            let mut cmd = std::process::Command::new("gcc");

            if self.is_main {
                cmd.arg(&input_file);
                cmd.arg("-o").arg(out_dir.join(&self.target_name));
                for obj in linked_objects.iter() {
                    cmd.arg(obj);
                }
                for library in self.foreign_libraries.values() {
                    let arg = if library.link_name.starts_with("lib") {
                        format!("-l:{}", library.link_name)
                    } else if library.kind.is_static() {
                        format!("-l:lib{}.a", library.link_name)
                    } else {
                        format!("-l{}", library.link_name)
                    };
                    cmd.arg(arg);
                }
            } else {
                let obj_file = out_dir.join(format!("{}.o", self.target_name));
                cmd.arg("-c").arg(&input_file).arg("-o").arg(&obj_file);
                linked_objects.push(obj_file);
            }

            for path in &self.search_paths {
                cmd.arg("-I").arg(path);
                cmd.arg("-L").arg(path);
            }

            let status = cmd.status().unwrap();
            if status.success() {
                if self.is_main {
                    if self.verbose {
                        println!(
                            "Successfully compiled executable to .build/{}",
                            self.target_name
                        );
                    }
                } else if self.verbose {
                    println!(
                        "Successfully compiled object to .build/{}.o",
                        self.target_name
                    );
                }
                Ok(exported_decls)
            } else {
                println!("Failed to link with gcc.");
                Err(())
            }
        } else {
            Err(())
        }
    }

    pub fn compile(&mut self) -> ChsResult<Vec<ast::FunctionDecl>> {
        let mut compiled_paths = HashSet::new();
        let mut linked_objects = Vec::new();
        let mut reporter = DiagnosticReporter::new();
        let res = self.compile_impl(&mut compiled_paths, &mut linked_objects, &mut reporter);
        if reporter.has_errors() {
            reporter.print_all();
            return Err(());
        }
        res
    }
}
