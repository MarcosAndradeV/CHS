use crate::ir::tac::{Instr, Proc, Program};
use crate::parser::ast;
use crate::types::{Database, ModuleId};

pub mod tac;

pub fn lower_ast_to_tac(db: &Database, m: ast::Module) -> Result<Program, LowerError> {
    let mut program = Program::new();
    for decl in m.decls {
        match decl {
            ast::Decl::Method(method_decl) => {
                let module = unsafe { db.get_module_unchecked(m.file_path.as_path()) };
                let id = ModuleId::get_method(module, db, method_decl.name.source()).unwrap();
                let mut proc = Proc::new(id, method_decl.name.source); // TODO: mangle names
                assert!(method_decl.visibility == ast::Visibility::Public);
                assert!(method_decl.kind == ast::MethodKind::Static);
                assert!(method_decl.arguments.is_empty());
                assert!(method_decl.return_type.is_none());
                assert!(method_decl.body.is_empty());
                proc.push(Instr::Ret);
                program.push(proc);
            }
            _ => todo!(),
        }
    }
    Ok(program)
}

pub struct LowerError(pub String);

impl std::fmt::Debug for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LowerError {}
