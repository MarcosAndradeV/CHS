use std::cell::RefCell;

use crate::chslexer::TokenKind;
use crate::ir::{self, type_of_expr};
use crate::ir::{Body, Names};
use crate::utils::AppError;
use crate::{Compiler, ast};

pub fn lower_ast_to_ir<'src>(c: &Compiler<'src>) -> Result<(), ()> {
    let mut consts = vec![];
    let mut local_name_space = ir::NameSpace::default();
    local_name_space.push_scope();

    for module in c.modules.borrow().values() {
        for c in module.consts.iter() {
            let e = const_eval(&local_name_space, &consts, &c.expr);
            let id = consts.len();
            consts.push(e);
            local_name_space.insert_var_index(c.name.source, ir::Names::Const(id));
        }

        for f in &module.externs {
            let uid = c.add_program_extern(ir::ExternFunc {
                used: false,
                name: f.name,
                args: f
                    .args
                    .iter()
                    .map(|t| convert_types(&local_name_space, &consts, t))
                    .collect(),
                is_variadic: f.is_variadic,
                ret: f
                    .ret_type
                    .as_ref()
                    .map(|t| convert_types(&local_name_space, &consts, t))
                    .unwrap_or_default(),
            });
            local_name_space.insert_var_index(f.name.source, ir::Names::ExternFunc(uid));
        }

        for f in &module.funcs {
            let r#fn = ir::Func {
                name: f.name,
                ret_type: f
                    .ret_type
                    .as_ref()
                    .map(|t| convert_types(&local_name_space, &consts, t))
                    .unwrap_or_default(),
                ..Default::default()
            };
            let uid = c.add_program_func(r#fn);
            local_name_space.insert_var_index(f.name.source, ir::Names::Func(uid));
        }

        for v in &module.global_vars {
            let ty = convert_types(&local_name_space, &consts, &v.r#type);
            let is_vec = matches!(ty, ir::Type::Array(..));
            let uid = c.add_program_global_var(ir::GlobalVar {
                token: v.name,
                is_vec,
                ty: if is_vec {ty.array_to_ptr_type()} else {ty},
                value: v
                    .expr
                    .as_ref()
                    .map(|e| const_eval(&local_name_space, &consts, e)),
            });
            local_name_space.insert_var_index(v.name.source, ir::Names::GlobalVar(uid));
        }

        if !module.execs.is_empty() {
            todo!()
        }
    }
    for module in c.modules.borrow().values() {
        for f in &module.funcs {
            let ir::Names::Func(uid) = *local_name_space.get(f.name.source).unwrap() else {
                unreachable!()
            };
            local_name_space.push_scope();

            let func = &mut c.program.borrow_mut().funcs[uid];
            for (arg, typ) in &f.args {
                let id = func.args.len();
                func.args.push(arg.source);
                let ty = convert_types(&local_name_space, &consts, typ);
                func.args_types.push(ty.clone());
                let id = func.body.vars.len();
                local_name_space.insert_var_index(arg.source, ir::Names::Var(id));
                func.body.vars.push(ir::Var {
                    used: false,
                    loc: arg.loc,
                    ty,
                });
            }
            compile_stmt(c, &consts, &mut local_name_space, func, &f.body);

            local_name_space.pop_scope();
        }
    }

    Ok(())
}

fn const_eval<'src>(
    names_index: &ir::NameSpace<'src>,
    consts: &[ir::ConstExpr<'src>],
    e: &ast::Expr<'src>,
) -> ir::ConstExpr<'src> {
    match e {
        ast::Expr::IntLit(_, lit) => ir::ConstExpr::IntLit(*lit),
        ast::Expr::StrLit(token) => ir::ConstExpr::StrLit(*token),
        ast::Expr::Ident(token) => {
            if let Some(Names::Const(id)) = names_index.get(token.source) {
                consts[*id].clone()
            } else {
                todo!()
            }
        }
        _ => todo!(),
    }
}

fn compile_stmt<'src>(
    c: &Compiler<'src>,
    consts: &[ir::ConstExpr<'src>],
    names_index: &mut ir::NameSpace<'src>,
    func: &mut ir::Func<'src>,
    stmt: &ast::Stmt<'src>,
) {
    match stmt {
        ast::Stmt::Return { loc, expr } => {
            let expr = expr
                .as_ref()
                .map(|e| compile_expr(c, consts, names_index, &mut func.body, e));
            func.body.push(ir::Stmt::Return(*loc, expr));
        }
        ast::Stmt::VarDecl { name, r#type, expr } => {
            let loc = name.loc;
            let id = func.body.vars.len();
            names_index.insert_var_index(name.source, ir::Names::Var(id));
            func.body.vars.push(ir::Var {
                used: false,
                loc: name.loc,
                ty: convert_types(names_index, consts, r#type),
            });
            if let Some(expr) = expr {
                let rhs = compile_expr(c, consts, names_index, &mut func.body, expr);
                func.body.push(ir::Stmt::AssignVar { loc, var: id, rhs });
            }
        }
        ast::Stmt::Expr { loc, expr } => {
            match expr {
                ast::Expr::Call { loc, caller, args } => {
                    let args = args
                        .into_iter()
                        .map(|arg| compile_expr(c, consts, names_index, &mut func.body, arg))
                        .collect();
                    let caller = match caller.as_ref() {
                        ast::Expr::Ident(ident) => *ident,
                        _ => todo!(),
                    };
                    func.body.push(ir::Stmt::Funcall {
                        result: None,
                        caller,
                        args,
                    });
                }
                ast::Expr::Syscall { loc, args } => {
                    let args = args
                        .into_iter()
                        .map(|arg| compile_expr(c, consts, names_index, &mut func.body, arg))
                        .collect();
                    func.body.push(ir::Stmt::Syscall { result: None, args });
                }
                _ => {}
            };
        }
        ast::Stmt::Assign { loc, lhs, rhs } => {
            match compile_expr(c, consts, names_index, &mut func.body, lhs) {
                ir::Expr::Var(_, id) => {
                    let rhs = compile_expr(c, consts, names_index, &mut func.body, rhs);
                    func.body.push(ir::Stmt::AssignVar {
                        loc: *loc,
                        var: id,
                        rhs,
                    });
                }
                ir::Expr::Global(token, id) => {
                    let rhs = compile_expr(c, consts, names_index, &mut func.body, rhs);
                    func.body.push(ir::Stmt::AssignGlobalVar {
                        var: (token, id),
                        rhs,
                    });
                }
                target @ ir::Expr::Deref(loc, id) => {
                    let rhs = compile_expr(c, consts, names_index, &mut func.body, rhs);
                    func.body.push(ir::Stmt::Store { target, rhs });
                }
                target @ ir::Expr::GlobalDeref(loc, id) => {
                    let rhs = compile_expr(c, consts, names_index, &mut func.body, rhs);
                    func.body.push(ir::Stmt::Store { target, rhs });
                }
                _ => unreachable!(),
            }
        }
        ast::Stmt::While {
            loc,
            cond,
            body: while_body,
        } => {
            let body_id = func.next_block();
            let cond_id = func.next_block();
            func.body.push(ir::Stmt::Jmp(cond_id));
            func.push_block(body_id);
            compile_stmt(c, consts, names_index, func, while_body);
            func.push_block(cond_id);
            let jnz = ir::Stmt::JNZ(
                compile_expr(c, consts, names_index, &mut func.body, cond),
                body_id,
            );
            func.body.push(jnz);
        }
        ast::Stmt::If {
            loc,
            cond,
            true_branch,
            else_branch,
        } => {
            let true_branch_id = func.next_block();
            let else_branch_id = func.next_block();
            let after_branch_id = func.next_block();

            let jz = ir::Stmt::JZ(
                compile_expr(c, consts, names_index, &mut func.body, cond),
                else_branch_id,
            );
            func.body.push(jz);
            compile_stmt(c, consts, names_index, func, true_branch);
            if let Some(else_branch) = else_branch {
                func.body.push(ir::Stmt::Jmp(after_branch_id));
                func.push_block(else_branch_id);
                compile_stmt(c, consts, names_index, func, else_branch);
                func.push_block(after_branch_id);
            } else {
                func.push_block(else_branch_id);
            }
        }
        ast::Stmt::Block(loc, stmts) => {
            names_index.push_scope();
            for stmt in stmts {
                compile_stmt(c, consts, names_index, func, stmt);
            }
            names_index.pop_scope();
        }
    }
}

fn compile_expr<'src>(
    c: &Compiler<'src>,
    consts: &[ir::ConstExpr<'src>],
    names_index: &ir::NameSpace<'src>,
    body: &mut Body<'src>,

    expr: &ast::Expr<'src>,
) -> ir::Expr<'src> {
    match expr {
        ast::Expr::IntLit(loc, lit) => ir::Expr::IntLit(*loc, *lit),
        ast::Expr::BoolLit(loc, lit) => ir::Expr::IntLit(*loc, if *lit { 1 } else { 0 }),
        ast::Expr::StrLit(token) => ir::Expr::StrLit(*token),
        ast::Expr::Ident(token) => match names_index.get(token.source) {
            Some(ir::Names::Var(id)) => ir::Expr::Var(token.loc, *id),
            Some(ir::Names::Arg(id)) => ir::Expr::Arg(token.loc, *id),
            Some(ir::Names::GlobalVar(id)) => ir::Expr::Global(*token, *id),
            Some(ir::Names::Const(id)) => match &consts[*id] {
                ir::ConstExpr::IntLit(lit) => ir::Expr::IntLit(token.loc, *lit),
                ir::ConstExpr::StrLit(token) => ir::Expr::StrLit(*token),
            },
            None => {
                todo!(
                    "{}",
                    AppError::ParseError {
                        path: token.loc.to_string(),
                        error: format!("Undefined name `{token}`")
                    }
                    .to_string()
                );
            }
            _ => todo!(),
        },
        ast::Expr::Deref(loc, expr) => match compile_expr(c, consts, names_index, body, expr) {
            ir::Expr::Var(loc, var_id) => ir::Expr::Deref(loc, var_id),
            ir::Expr::StrLit(token) => {
                let unescape = token.unescape();
                ir::Expr::CharLit(token.loc, unescape.chars().nth(0).unwrap_or('\0'))
            }
            ir::Expr::Global(token, uid) => ir::Expr::GlobalDeref(token, uid),
            _ => todo!(),
        },
        ast::Expr::Ref(loc, expr) => match compile_expr(c, consts, names_index, body, expr) {
            ir::Expr::Var(loc, var_id) => ir::Expr::Ref(loc, var_id),
            _ => todo!(),
        },
        ast::Expr::Call { loc, caller, args } => {
            let id = body.vars.len();
            let args = args
                .into_iter()
                .map(|arg| compile_expr(c, consts, names_index, body, arg))
                .collect();
            let (caller, ty) = match caller.as_ref() {
                ast::Expr::Ident(ident) => {
                    let ty = match names_index.get(ident.source) {
                        Some(ir::Names::Func(uid)) => c.get_program_funcs()[*uid].ret_type.clone(),
                        Some(ir::Names::ExternFunc(uid)) => {
                            c.get_program_externs()[*uid].ret.clone()
                        }
                        _ => todo!(),
                    };
                    (*ident, ty)
                }
                _ => todo!(),
            };
            body.push(ir::Stmt::Funcall {
                result: Some(id),
                caller,
                args,
            });
            let loc = *loc;
            body.vars.push(ir::Var {
                used: false,
                loc,
                ty,
            });

            ir::Expr::Var(loc, id)
        }
        ast::Expr::Syscall { loc, args } => {
            let args = args
                .into_iter()
                .map(|arg| compile_expr(c, consts, names_index, body, arg))
                .collect();

            let id = body.vars.len();
            body.push(ir::Stmt::Syscall {
                result: Some(id),
                args,
            });
            let loc = *loc;
            body.vars.push(ir::Var {
                used: false,
                loc,
                ty: ir::Type::UBits(64),
            });

            ir::Expr::Var(loc, id)
        }
        ast::Expr::Binop { operator, lhs, rhs } => {
            let loc = operator.loc;

            let lhs = compile_expr(c, consts, names_index, body, lhs);
            let rhs = compile_expr(c, consts, names_index, body, rhs);

            let id = body.vars.len();
            let global_vars = c.get_program_global_vars();
            let lhs_ty = type_of_expr(&lhs, global_vars, &body.vars).unwrap();
            let rhs_ty = type_of_expr(&rhs, global_vars, &body.vars).unwrap();
            let ty = match operator.kind {
                TokenKind::Plus | TokenKind::Minus => {
                    if matches!(lhs_ty, ir::Type::PtrTo(_)|ir::Type::Array(..)) {
                        lhs_ty
                    } else if matches!(rhs_ty, ir::Type::PtrTo(_)|ir::Type::Array(..)) {
                        rhs_ty
                    } else {
                        match (&lhs_ty, &rhs_ty) {
                            (ir::Type::SBits(l), ir::Type::SBits(r)) => {
                                if l > r {
                                    ir::Type::SBits(*l)
                                } else {
                                    ir::Type::SBits(*r)
                                }
                            }
                            (ir::Type::SBits(l), ir::Type::UBits(r)) => {
                                if l > r {
                                    ir::Type::UBits(*l)
                                } else {
                                    ir::Type::UBits(*r)
                                }
                            }
                            (ir::Type::UBits(l), ir::Type::SBits(r)) => {
                                if l > r {
                                    ir::Type::UBits(*l)
                                } else {
                                    ir::Type::UBits(*r)
                                }
                            }
                            (ir::Type::UBits(l), ir::Type::UBits(r)) => {
                                if l > r {
                                    ir::Type::UBits(*l)
                                } else {
                                    ir::Type::UBits(*r)
                                }
                            }
                            _ => todo!("{lhs_ty} {rhs_ty}"),
                        }
                    }
                }
                TokenKind::Asterisk | TokenKind::Slash | TokenKind::Percent => {
                    match (&lhs_ty, &rhs_ty) {
                        (ir::Type::SBits(l), ir::Type::SBits(r)) => {
                            if l > r {
                                ir::Type::SBits(*l)
                            } else {
                                ir::Type::SBits(*r)
                            }
                        }
                        (ir::Type::SBits(l), ir::Type::UBits(r)) => {
                            if l > r {
                                ir::Type::UBits(*l)
                            } else {
                                ir::Type::UBits(*r)
                            }
                        }
                        (ir::Type::UBits(l), ir::Type::SBits(r)) => {
                            if l > r {
                                ir::Type::UBits(*l)
                            } else {
                                ir::Type::UBits(*r)
                            }
                        }
                        (ir::Type::UBits(l), ir::Type::UBits(r)) => {
                            if l > r {
                                ir::Type::UBits(*l)
                            } else {
                                ir::Type::UBits(*r)
                            }
                        }
                        _ => todo!("{lhs_ty} {rhs_ty}"),
                    }
                }

                TokenKind::Ampersand => todo!(),
                TokenKind::Pipe => todo!(),
                TokenKind::Caret => todo!(),
                TokenKind::ShiftLeft => todo!(),
                TokenKind::ShiftRight => todo!(),

                TokenKind::Eq
                | TokenKind::NotEq
                | TokenKind::Gt
                | TokenKind::GtEq
                | TokenKind::Lt
                | TokenKind::LtEq => {
                    // func.body.push(ir::Stmt::ComparisonBinop {
                    //     result: id,
                    //     operator,
                    //     lhs,
                    //     rhs,
                    // });
                    ir::Type::Bool
                }

                TokenKind::DoubleAmpersand | TokenKind::DoublePipe => {
                    // func.body.push(ir::Stmt::LogicalBinop {
                    //     result: id,
                    //     operator,
                    //     lhs,
                    //     rhs,
                    // });
                    ir::Type::Bool
                }
                _ => todo!(),
            };
            let operator = *operator;
            body.push(ir::Stmt::Binop {
                result: id,
                operator,
                lhs,
                rhs,
            });

            body.vars.push(ir::Var {
                used: false,
                loc,
                ty,
            });
            ir::Expr::Var(loc, id)
        }
        ast::Expr::Index { loc, base, index } => {
            todo!()
        }
    }
}

fn convert_types<'src>(
    names_index: &ir::NameSpace<'src>,
    consts: &[ir::ConstExpr<'src>],
    ast_type: &ast::Type,
) -> ir::Type {
    match ast_type {
        ast::Type::Name(token) if token.source == "s8" => ir::Type::SBits(8),
        ast::Type::Name(token) if token.source == "u8" => ir::Type::UBits(8),
        ast::Type::Name(token) if token.source == "s16" => ir::Type::SBits(16),
        ast::Type::Name(token) if token.source == "u16" => ir::Type::UBits(16),
        ast::Type::Name(token) if token.source == "int" => ir::Type::SBits(32),
        ast::Type::Name(token) if token.source == "s32" => ir::Type::SBits(32),
        ast::Type::Name(token) if token.source == "u32" => ir::Type::UBits(32),
        ast::Type::Name(token) if token.source == "usize" => ir::Type::UBits(64),
        ast::Type::Name(token) if token.source == "ssize" => ir::Type::SBits(64),
        ast::Type::Name(token) if token.source == "u64" => ir::Type::UBits(64),
        ast::Type::Name(token) if token.source == "s64" => ir::Type::SBits(64),
        ast::Type::Name(token) if token.source == "char" => ir::Type::Char,
        ast::Type::Name(token) if token.source == "bool" => ir::Type::Bool,
        ast::Type::Name(token) if token.source == "void" => ir::Type::Void,
        ast::Type::PtrTo(t) => ir::Type::PtrTo(Box::new(convert_types(names_index, consts, t))),
        ast::Type::Array(n, t) => {
            let n = match const_eval(names_index, consts, n) {
                ir::ConstExpr::IntLit(lit) => lit,
                ir::ConstExpr::StrLit(token) => todo!(),
            };
            ir::Type::Array(n, Box::new(convert_types(names_index, consts, t)))
        }
        _ => todo!("{ast_type:?}"),
    }
}
