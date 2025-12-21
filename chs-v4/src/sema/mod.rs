use crate::parser::ast;
use crate::types::{Database, Method, Module};

pub fn analyse_module(m: &ast::Module) -> Result<Database, SemaError> {
    let mut db = Database::new();
    let module = Module::register(&mut db, m.file_path.clone());

    for decl in m.decls.iter() {
        match decl {
            ast::Decl::Method(method_decl) => {
                assert!(method_decl.visibility == ast::Visibility::Public);
                assert!(method_decl.kind == ast::MethodKind::Static);
                assert!(method_decl.arguments.is_empty());
                assert!(method_decl.return_type.is_none());
                assert!(method_decl.body.is_empty());
                let _method = Method::register(&mut db, module, method_decl.name.clone())
                    .map_err(|err| SemaError(err))?;
            }
            _ => todo!(),
        }
    }

    Ok(db)
}

pub struct SemaError(pub String);

impl std::fmt::Debug for SemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for SemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SemaError {}
