#![allow(clippy::result_unit_err)]

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use diagnostic::*;
use syntax::ast;
use syntax::ast::Library;
use syntax::parse_file;

pub fn get_std_path() -> PathBuf {
    #[cfg(feature = "development")]
    {
        return PathBuf::from("../std");
    }
    if let Ok(path) = std::env::var("CHS_HOME") {
        return PathBuf::from(path).join("std");
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
    target_name: PathBuf,
    backend: CodegenBackend,
    verbose: bool,
    is_main: bool,
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
            foreign_libraries:HashMap::new(),
            sources: Vec::new(),
            target_name: PathBuf::from("out"),
            backend: CodegenBackend::Qbe,
            verbose: false,
            is_main: true,
        }
    }

    pub fn spawn_child(&self, target_name: impl Into<PathBuf>) -> Self {
        Self {
            target_name: target_name.into(),
            sources: Vec::new(),
            search_paths: self.search_paths.clone(),
            foreign_libraries: self.foreign_libraries.clone(),
            verbose: self.verbose,
            backend: self.backend,
            is_main: false,
        }
    }

    pub fn add_default_search_paths(&mut self) -> ChsResult<()> {
        self.add_search_path(env::current_dir()?)?;
        self.add_search_path(get_std_path())?;
        self.add_search_path(get_std_path().join("runtime"))?;
        // NOTE: This is not a search path. Just a hack
        self.add_source(get_std_path().join("runtime/module.chs"))?;
        Ok(())
    }

    pub fn add_source(&mut self, source: PathBuf) -> ChsResult<()> {
        // NOTE: On Windows, use `dunce::canonicalize` to avoid UNC path (`\\?\`) issues with GCC.
        let source = fs::canonicalize(&source).unwrap_or(source);
        self.sources.push(source);
        Ok(())
    }

    pub fn add_search_path(&mut self, file_path: PathBuf) -> ChsResult<()> {
        let file_path = fs::canonicalize(&file_path).unwrap_or(file_path);
        self.search_paths.insert(file_path);
        Ok(())
    }

    pub fn compile(&mut self) -> ChsResult<Vec<ast::FunctionDecl>> {
        let mut compiled_paths = HashSet::new();
        let mut linked_objects = Vec::new();
        let mut reporter = DiagnosticReporter::new();

        let res = self.run_pipeline(&mut compiled_paths, &mut linked_objects, &mut reporter);

        if reporter.has_errors() {
            reporter.print_all();
            bail!("Fail to compile source code");
        }
        res
    }

    /// Drives the 4-phase compilation pipeline
    fn run_pipeline(
        &mut self,
        compiled_paths: &mut HashSet<PathBuf>,
        linked_objects: &mut Vec<PathBuf>,
        reporter: &mut DiagnosticReporter,
    ) -> ChsResult<Vec<ast::FunctionDecl>> {
        // Phase 1: Frontend (Parse & Resolve)
        let (mut merged_ast_items, exported_decls) =
            self.parse_and_resolve(compiled_paths, linked_objects, reporter)?;

        // Phase 2: Middle-end (Type Checking & IR Lowering)
        let mut module = self.analyze_and_lower(&mut merged_ast_items, reporter)?;

        // Phase 3: Backend (QBE Transpilation)
        let s_file = self.codegen_qbe(&mut module)?;

        // Phase 4: Linking
        self.link_binary(s_file, linked_objects)?;

        Ok(exported_decls)
    }

    fn parse_and_resolve(
        &mut self,
        compiled_paths: &mut HashSet<PathBuf>,
        linked_objects: &mut Vec<PathBuf>,
        reporter: &mut DiagnosticReporter,
    ) -> ChsResult<(Vec<ast::FileItem>, Vec<ast::FunctionDecl>)> {
        let mut file_queue = self.sources.clone();
        let mut merged_ast_items = Vec::new();

        while let Some(fp) = file_queue.pop() {
            let fp = fs::canonicalize(&fp).unwrap_or(fp);
            if !compiled_paths.insert(fp.clone()) {
                continue; // Already processed
            }

            let source = fs::read_to_string(&fp)?;
            let ast = match parse_file(&fp, &source, reporter) {
                Ok(a) => a,
                Err(_) => {
                    bail!("Could not parse source `{}`", fp.display());
                }
            };

            for item in ast.items {
                if let ast::FileItem::Import(import_decl) = item {
                    let path_str = import_decl.path.source().trim_matches('"');
                    let mut resolved = None;

                    // 1. Try relative to current file
                    let mut relative = fp.parent().unwrap_or(Path::new(".")).join(path_str);
                    if !relative.exists() {
                        relative = relative.with_extension("chs");
                    }

                    if relative.exists() {
                        resolved = Some(relative);
                    } else {
                        // 2. Try search paths
                        for sp in &self.search_paths {
                            let mut candidate = sp.join(path_str);
                            if !candidate.exists() {
                                candidate = candidate.with_extension("chs");
                            }
                            if candidate.exists() {
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

                                // FIX: Hash the absolute path to prevent child target name collisions
                                let mut hasher = DefaultHasher::new();
                                r.hash(&mut hasher);
                                let dir_name = r
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "mod".to_string());
                                let target = format!("{}_{:x}", dir_name, hasher.finish());

                                let mut child = self.spawn_child(target);

                                if let Ok(entries) = fs::read_dir(&r) {
                                    for entry in entries.flatten() {
                                        if entry.path().extension().and_then(|s| s.to_str())
                                            == Some("chs")
                                        {
                                            file_queue.push(entry.path());
                                        }
                                    }
                                }

                                if !child.sources.is_empty() {
                                    let child_exports = child.run_pipeline(
                                        compiled_paths,
                                        linked_objects,
                                        reporter,
                                    )?;

                                    for mut exp in child_exports {
                                        exp.body = None; // Strip body for headers
                                        merged_ast_items.push(ast::FileItem::FunctionDecl(exp));
                                    }
                                }
                                self.foreign_libraries.extend(child.foreign_libraries);
                            }
                        } else if r.is_file() && !compiled_paths.contains(&r) {
                            file_queue.push(r);
                        }
                    } else {
                        reporter.report(
                            import_decl.path.loc,
                            format!("Could not resolve import: {}", path_str),
                        );
                        continue;
                    }
                } else {
                    merged_ast_items.push(item);
                }
            }
        }

        // Extract Exports and Libraries
        let mut exported_decls = Vec::new();
        for item in &merged_ast_items {
            if let ast::FileItem::FunctionDecl(decl) = item {
                let is_private = decl
                    .directives
                    .iter()
                    .any(|d| matches!(d, ast::FunctionDirective::Private));

                if !is_private {
                    exported_decls.push(decl.clone());
                }
            }
            if let ast::FileItem::Directive(ast::Directive::Library { name, library }) = item {
                self.foreign_libraries
                    .insert(name.source().to_string(), library.clone());
            }
        }

        Ok((merged_ast_items, exported_decls))
    }

    fn analyze_and_lower(
        &self,
        merged_ast_items: &mut Vec<ast::FileItem>,
        reporter: &mut DiagnosticReporter,
    ) -> ChsResult<ir::Module> {
        let mut type_checker = semantics::TypeChecker::new(reporter);
        type_checker.declared_libraries = self.foreign_libraries.keys().cloned().collect();

        if !type_checker.check(merged_ast_items) {
            bail!("Fail to type check source code");
        }

        let mut module =
            ir::translate::translate_ast_items(merged_ast_items, type_checker.type_db, reporter)?;

        if reporter.has_errors() {
            bail!("Could not translate source code to ir");
        }

        optimize(&mut module, self.is_main);
        Ok(module)
    }

    fn codegen_qbe(&self, module: &mut ir::Module) -> ChsResult<PathBuf> {
        let out_dir = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".build");

        fs::create_dir_all(&out_dir)?;
        fs::write(out_dir.join(".gitignore"), "*")?;

        if self.backend != CodegenBackend::Qbe {
            bail!("Unsupported backend.");
        }

        let transpiler = codegen::qbe::QbeTranspiler::new(module);
        let qbe_code = transpiler.transpile();

        let ssa_file = out_dir.join(self.target_name.with_extension("ssa"));
        fs::write(&ssa_file, qbe_code)?;

        if self.verbose {
            println!("Transpiled QBE SSA code written to {}", ssa_file.display());
        }

        let s_file = out_dir.join(self.target_name.with_extension("s"));
        let qbe_exe = std::env::var("QBE").unwrap_or_else(|_| "qbe".to_string());

        let mut qbe_cmd = std::process::Command::new(qbe_exe);
        qbe_cmd.arg("-o").arg(&s_file).arg(&ssa_file);

        if !qbe_cmd.status()?.success() {
            bail!("Failed to compile QBE SSA code with qbe.");
        }

        Ok(s_file)
    }

    fn link_binary(&self, s_file: PathBuf, linked_objects: &mut Vec<PathBuf>) -> ChsResult<()> {
        let out_dir = env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".build");

        // Allow overriding the C compiler via standard environment variables
        let cc = std::env::var("CC").unwrap_or_else(|_| "gcc".to_string());
        let mut cmd = std::process::Command::new(cc);

        if self.is_main {
            cmd.arg(&s_file);
            let final_output = env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&self.target_name);

            cmd.arg("-o").arg(&final_output);

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
            let obj_file = out_dir.join(self.target_name.with_extension("o"));
            cmd.arg("-c").arg(&s_file).arg("-o").arg(&obj_file);
            linked_objects.push(obj_file);
        }

        for path in &self.search_paths {
            cmd.arg("-I").arg(path);
            cmd.arg("-L").arg(path);
        }
        if self.verbose {
            println!("CMD: {:?}", cmd);
        }
        let status = cmd.status()?;

        if status.success() {
            if self.verbose {
                let msg = if self.is_main {
                    format!(
                        "Successfully compiled executable to .build/{}",
                        self.target_name.display()
                    )
                } else {
                    format!(
                        "Successfully compiled object to .build/{}.o",
                        self.target_name.display()
                    )
                };
                println!("{}", msg);
            }
            Ok(())
        } else {
            bail!("Failed to link with C compiler.");
        }
    }

    pub fn set_target_name(&mut self, target_name: impl Into<PathBuf>) {
        self.target_name = target_name.into();
    }

    pub fn target_name(&self) -> &Path {
        &self.target_name
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }
}

fn optimize(module: &mut ir::Module, is_main: bool) {
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
    ir::strip_unused_functions(module, is_main);
}
