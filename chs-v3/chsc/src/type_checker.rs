use std::collections::HashMap;

use crate::ir::{type_of_expr, Program, Stmt, Type};
use crate::utils::AppError;


pub fn check(p: &Program) -> Result<(), AppError> {
    let mut type_map = TypeMap::default();
    for f in p.funcs.iter() {
        type_map.globals_mut().insert(f.name.source, TypeItem::Func(&f.args_types, &f.ret_type));
    }

    for f in p.funcs.iter() {
        for stmt in f.body.stmts.iter() {
            for (i, v) in f.body.vars.iter().enumerate() {
                type_map.locals_mut().insert(i, TypeItem::Type(&v.ty));
            }
            match stmt {
                _ => todo!(),
            }
        }
    }
    Ok(())
}

enum TypeItem<'c> {
    Func(&'c [Type], &'c Type),
    Type(&'c Type)
}

#[derive(Default)]
struct TypeMap<'src>  {
    globals: HashMap<&'src str, TypeItem<'src>>,
    locals: HashMap<usize, TypeItem<'src>>,
}

impl<'src> TypeMap<'src> {
    fn globals_mut(&mut self) -> &mut HashMap<&'src str, TypeItem<'src>> {
        &mut self.globals
    }

    fn locals_mut(&mut self) -> &mut HashMap<usize, TypeItem<'src>> {
        &mut self.locals
    }
}
