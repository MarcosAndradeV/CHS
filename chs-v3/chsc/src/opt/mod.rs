use crate::ir::*;

pub fn strip_unused_functions<'src>(program_ir: &mut Program<'src>) {
    let mut used_func = vec![];
    mark_used(program_ir, &mut used_func, "main");
    for name in used_func {
        if let Some(f) = program_ir.funcs.iter_mut().find(|f| f.name.source == name) {
            f.used = true;
        } else if let Some(f) = program_ir
            .externs
            .iter_mut()
            .find(|f| f.name.source == name)
        {
            f.used = true;
        }
    }
}

fn mark_used<'src>(program_ir: &Program<'src>, used_func: &mut Vec<&'src str>, root: &'src str) {
    if let Some(f) = program_ir.funcs.iter().find(|f| f.name.source == root) {
        used_func.push(f.name.source);
        for stmt in f.body.stmts.iter() {
            if let Stmt::Funcall {
                result: _,
                caller,
                args: _,
            } = stmt
            {
                mark_used(program_ir, used_func, caller.source);
            }
        }
    }
    if let Some(f) = program_ir.externs.iter().find(|f| f.name.source == root) {
        used_func.push(f.name.source);
    }
}

pub fn strip_unused_variables<'src>(program_ir: &mut Program<'src>) {
    for func in program_ir.funcs.iter_mut().filter(|f| f.used) {
        let vars = &mut func.body.vars;
        for stmt in &func.body.stmts {
            match stmt {
                Stmt::Return(loc, Some(expr)) => mark_used_var(vars, expr),
                Stmt::Store { target, rhs } => {
                    mark_used_var(vars, target);
                    mark_used_var(vars, rhs);
                }
                Stmt::AssignGlobalVar { var, rhs } => {
                    mark_used_var(vars, rhs);
                }
                Stmt::AssignVar { loc, var, rhs } => {
                    mark_used_var(vars, rhs);
                }
                Stmt::Unop {
                    result,
                    operator,
                    operand,
                } => todo!(),
                Stmt::Binop {
                    result,
                    operator,
                    lhs,
                    rhs,
                } => {
                    mark_used_var(vars, lhs);
                    mark_used_var(vars, rhs);
                }
                Stmt::Funcall {
                    result,
                    caller,
                    args,
                } => {
                    for arg in args.iter() {
                        mark_used_var(vars, arg);
                    }
                }
                Stmt::Syscall { result, args } => {
                    for arg in args.iter() {
                        mark_used_var(vars, arg);
                    }
                }
                Stmt::JZ(expr, _) => {
                      mark_used_var(vars, expr);
                }
                Stmt::JNZ(expr, _) => {
                      mark_used_var(vars, expr);
                }
                _ => {}
            }
        }
    }
}

fn mark_used_var<'src>(vars: &mut Vec<Var<'src>>, expr: &Expr<'src>) {
    match expr {
        Expr::Deref(_, id) | Expr::Ref(_, id) | Expr::Var(_, id) => {
            vars[*id].used = true;
        }
        _ => {},
    }
}
