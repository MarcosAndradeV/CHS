use crate::{
    chslexer::TokenKind,
    generator::fasm_backend::{
        Cond, DataDef, DataDirective, DataExpr, Function, Instr, Module, Register, SizeOperator,
        Value,
    },
    ir::*,
    utils::AppError, Compiler,
};

macro_rules! anno_asm {
    ($anno:expr, $f:expr, $($msg: expr),*) => {
        if $anno {
            $f.push_raw_instr(
                format!(";; {}",
                format_args!($($msg),*)
            ));
        }
    };
}

macro_rules! raw_instr {
    ($f:expr, $inst:expr) => {
        $f.push_raw_instr(format!("{}", format_args!($inst)));
    };
}



pub fn generate(c: &Compiler, use_c: bool) -> Result<Module, AppError> {
    let mut m = Module::new(use_c);
    let p = c.program.borrow();

    for r#extern in &p.externs {
        if r#extern.used {
            m.push_extrn(r#extern.name.source);
        }
    }

    for global_var in &p.global_vars {
        let size =
            global_var.ty.size()
        ;
        if let Some(c) = &global_var.value {
            match c {
                ConstExpr::IntLit(lit) => {
                    m.bss.push(DataDef::new(
                        format!("_{}", global_var.token.source),
                        DataDirective::Dd,
                        vec![DataExpr::Const(*lit)],
                    ));
                }
                ConstExpr::StrLit(token) => {
                    m.bss.push(DataDef::new(
                        format!("_{}", global_var.token.source),
                        DataDirective::Db,
                        vec![
                            DataExpr::Str(token.unescape()),
                            DataExpr::Const(0)
                        ],
                    ));
                }
            }
        } else {
            m.bss.push(DataDef::new(
                format!("_{}", global_var.token.source),
                DataDirective::Rb,
                vec![DataExpr::Const(size as u64)],
            ));
        }
    }

    for func in &p.funcs {
        if !func.used {continue;}
        let mut f = Function::new(m.link_with_c, func.name.source);

        let (size, offsets) = calculate_stack_offsets(&func.body.vars);
        f.allocate_stack(size);
        f.push_block("b");
        if func.ret_type != Type::Void {
            f.epiloge = false;
        }

        let call_convention = Register::get_call_convention();
        if func.args.len() > call_convention.len() {
            return Err(AppError::GenerationError(
                Some("Functions with more than 6 arguments are not supported yet.".to_string()),
                "Args via stack are not implemented yet".to_string(),
            ));
        }

        for (id, reg) in func
            .args
            .iter()
            .enumerate()
            .map(|arg| arg.0)
            .zip(call_convention)
        {
            let offset = offsets[id];
            f.push_raw_instr(format!("mov [rbp-{offset}], {reg}"));
        }

        let anno = true;

        let mut ctx = GenCtx {
            f: &mut f,
            m: &mut m,
            global_vars: &p.global_vars,
            vars: &func.body.vars,
            args_types: &func.args_types,
            offets: &offsets,
        };

        for stmt in &func.body.stmts {
            match stmt {
                Stmt::Return(loc, expr) => {
                    anno_asm!(anno, ctx.f, "Return@{loc}");
                    if let Some(expr) = expr {
                        mov_to_reg(&mut ctx, &expr, Register::Rax);
                    }
                    ctx.f.push_raw_instr("mov rsp, rbp");
                    ctx.f.push_raw_instr("pop rbp");
                    ctx.f.push_raw_instr("ret");
                }
                Stmt::AssignVar {
                    var: id,
                    rhs,
                    ..
                } => {
                    let r = Register::Rax;
                    mov_to_reg(&mut ctx, rhs, r);
                    let offset = ctx.get_offset(*id);
                    raw_instr!(ctx.f, "mov [rbp-{offset}], {r}");
                }
                Stmt::Store { target, rhs } => {
                    let reg = Register::Rax;
                    mov_to_reg(&mut ctx, rhs, reg);
                    match target {
                        Expr::Deref(token, id) => {
                            let offset = ctx.get_offset(*id);
                            let s = size_of_type(ctx.get_type_of_var(*id).get_inner());
                            let reg = s.register_for_size(reg);
                            if ctx.vars[*id].ty.is_array() {
                                raw_instr!(ctx.f, "lea rbx, [rbp-{offset}]");
                            } else {
                                raw_instr!(ctx.f, "mov rbx, [rbp-{offset}]");
                            }
                            raw_instr!(ctx.f, "mov {s} [rbx], {reg}");
                        }
                        Expr::GlobalDeref(token, uid) => {
                            let s = size_of_type(&ctx.global_vars[*uid].ty);
                            let reg = s.register_for_size(reg);
                            if ctx.global_vars[*uid].is_vec {
                                raw_instr!(ctx.f, "mov rbx, _{token}");
                            } else {
                                raw_instr!(ctx.f, "mov rbx, [_{token}]");
                            }
                            raw_instr!(ctx.f, "mov {s} [rbx], {reg}");
                        }
                        _ => unreachable!(),
                    }
                }
                Stmt::AssignGlobalVar { var:(token, uid), rhs } => {
                    let reg = Register::Rax;
                    mov_to_reg(&mut ctx, rhs, reg);
                    let s = size_of_type(&ctx.global_vars[*uid].ty);
                    let reg = s.register_for_size(reg);
                    if ctx.global_vars[*uid].is_vec {
                        raw_instr!(ctx.f, "mov _{token}, {reg}");
                    } else {
                        raw_instr!(ctx.f, "mov [_{token}], {reg}");
                    }
                }
                Stmt::Unop {
                    result,
                    operator,
                    operand,
                } => todo!(),
                Stmt::Binop {
                    result: id,
                    operator,
                    lhs,
                    rhs,
                } => {
                    let offset = ctx.get_offset(*id);
                    let sty = size_of_type(ctx.get_type_of_var(*id));
                    let l = Register::Rax;
                    let r = Register::Rbx;
                    let mut out = l;
                    match operator.kind {
                        TokenKind::Plus => {
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "add {l}, {r}");
                            // out = l;
                        }
                        TokenKind::Minus => {
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "sub {l}, {r}");
                            // out = l;
                        }
                        TokenKind::DoublePipe => {
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "or {l}, {r}");
                            // out = l;
                        }
                        TokenKind::DoubleAmpersand => {
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "and {l}, {r}");
                            // out = l;
                        }
                        TokenKind::Asterisk => {
                            raw_instr!(ctx.f, "xor rdx, rdx");
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "imul {r}");
                            // out = l;
                        }
                        TokenKind::Slash => {
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "cqo");
                            raw_instr!(ctx.f, "idiv {r}");
                            // out = l;
                        }
                        TokenKind::Percent => {
                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);
                            raw_instr!(ctx.f, "cqo");
                            raw_instr!(ctx.f, "idiv {r}");
                            out = Register::Rdx;
                        }
                        TokenKind::Eq => {
                            raw_instr!(ctx.f, "xor rdx, rdx");

                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);

                            raw_instr!(ctx.f, "cmp {l}, {r}");
                            raw_instr!(ctx.f, "sete dl");
                            out = Register::Rdx;
                        }
                        TokenKind::NotEq => {
                            raw_instr!(ctx.f, "xor rdx, rdx");

                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);

                            raw_instr!(ctx.f, "cmp {l}, {r}");
                            raw_instr!(ctx.f, "setne dl");
                            out = Register::Rdx;
                        }
                        TokenKind::Lt => {
                            raw_instr!(ctx.f, "xor rdx, rdx");

                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);

                            raw_instr!(ctx.f, "cmp {l}, {r}");
                            raw_instr!(ctx.f, "setl dl");
                            out = Register::Rdx;
                        }
                        TokenKind::Gt => {
                            raw_instr!(ctx.f, "xor rdx, rdx");

                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);

                            raw_instr!(ctx.f, "cmp {l}, {r}");
                            raw_instr!(ctx.f, "setg dl");
                            out = Register::Rdx;
                        }
                        TokenKind::LtEq => {
                            raw_instr!(ctx.f, "xor rdx, rdx");

                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);

                            raw_instr!(ctx.f, "cmp {l}, {r}");
                            raw_instr!(ctx.f, "setle dl");
                            out = Register::Rdx;
                        }
                        TokenKind::GtEq => {
                            raw_instr!(ctx.f, "xor rdx, rdx");

                            mov_to_reg(&mut ctx, lhs, l);
                            mov_to_reg(&mut ctx, rhs, r);

                            raw_instr!(ctx.f, "cmp {l}, {r}");
                            raw_instr!(ctx.f, "setge dl");
                            out = Register::Rdx;
                        }
                        _ => todo!("{operator}"),
                    }
                    raw_instr!(ctx.f, "mov [rbp-{offset}], {out}");
                }
                Stmt::Funcall {
                    result,
                    caller,
                    args,
                } => {
                    let call_convention = Register::get_call_convention();
                    if args.len() > call_convention.len() {
                        return Err(AppError::GenerationError(
                            Some(
                                "Functions with more than 6 arguments are not supported yet."
                                    .to_string(),
                            ),
                            "Args via stack are not implemented yet".to_string(),
                        ));
                    }

                    let regs = call_convention.into_iter();
                    for (expr, reg) in args.into_iter().zip(regs).rev() {
                        mov_to_reg(&mut ctx, expr, reg);
                    }

                    // x86_64 Linux ABI passes the amount of floating point args via al.
                    raw_instr!(ctx.f, "mov al, 0");

                    raw_instr!(ctx.f, "call _{caller}");
                    if let Some(result) = result {
                        let offset = offsets[*result];
                        raw_instr!(ctx.f, "mov [rbp-{offset}], rax");
                    }
                }
                Stmt::Syscall { result, args } => {
                    let call_convention = Register::get_syscall_call_convention();

                    if args.len() < 1 {
                        return Err(AppError::GenerationError(None, "Syscall".to_string()));
                    }

                    let args_len = args.len() - 1;

                    let regs = call_convention.into_iter();
                    for (expr, reg) in args.into_iter().zip(regs).rev() {
                        mov_to_reg(&mut ctx, expr, reg);
                    }

                    raw_instr!(ctx.f, "syscall");
                    if let Some(result) = result {
                        let offset = offsets[*result];
                        raw_instr!(ctx.f, "mov [rbp-{offset}], rax");
                    }
                }
                Stmt::Block(id) => {
                    ctx.f.push_block(format!("b{id}"));
                }
                Stmt::JZ(cond, id) => {
                    let l = Register::Rax;
                    raw_instr!(ctx.f, "xor rax, rax");
                    mov_to_reg(&mut ctx, cond, l);
                    raw_instr!(ctx.f, "test {l}, {l}");
                    raw_instr!(ctx.f, "jz .b{id}");
                }
                Stmt::JNZ(cond, id) => {
                    let l = Register::Rax;
                    raw_instr!(ctx.f, "xor rax, rax");
                    mov_to_reg(&mut ctx, cond, l);
                    raw_instr!(ctx.f, "test {l}, {l}");
                    raw_instr!(ctx.f, "jnz .b{id}");
                }
                Stmt::Jmp(id) => {
                    raw_instr!(ctx.f, "jmp .b{id}");
                }
            }
        }

        m.push_function(f);
    }
    Ok(m)
}

struct GenCtx<'ctx, 'src> {
    f: &'ctx mut Function,
    m: &'ctx mut Module,
    global_vars: &'ctx [GlobalVar<'src>],
    vars: &'ctx [Var<'src>],
    args_types: &'ctx [Type],
    offets: &'ctx [usize],
}

impl GenCtx<'_, '_> {

    fn get_offset(&self, id: usize) -> usize {
        self.offets[id]
    }

    fn get_type_of_var(&self, id: usize) -> &Type {
        &self.vars[id].ty
    }
}

fn mov_to_reg(ctx: &mut GenCtx, expr: &Expr<'_>, reg: Register) -> Result<(), AppError> {
    match expr {
        Expr::IntLit(loc, lit) => {
            raw_instr!(ctx.f, "mov {reg}, {lit}");
            Ok(())
        }
        Expr::CharLit(loc, lit) => {
            let lit = *lit as i32;
            raw_instr!(ctx.f, "mov {reg}, {lit}");
            Ok(())
        }
        Expr::Var(token, id) => {
            let offset = ctx.get_offset(*id);
            if ctx.vars[*id].ty.is_array() {
                raw_instr!(ctx.f, "lea {reg}, [rbp-{offset}]");
            } else {
                raw_instr!(ctx.f, "mov {reg}, [rbp-{offset}]");
            }
            Ok(())
        }
        Expr::Arg(token, id) => {
            let call_convention = Register::get_call_convention();
            if *id > call_convention.len() {
                return Err(AppError::GenerationError(
                    Some(
                        "Functions with more than 6 arguments are not supported yet."
                            .to_string(),
                    ),
                    "Args via stack are not implemented yet".to_string(),
                ));
            }

            let arg_reg = call_convention[*id];
            if reg != arg_reg {
                raw_instr!(ctx.f, "mov {reg}, {arg_reg}");
            }
            Ok(())
        }
        Expr::Void(loc) => todo!(),
        Expr::StrLit(token) => {
            let n = ctx.m.rodata.len();
            let name = format!("str{n}");
            ctx.m.push_rodata(DataDef::new(
                &name,
                DataDirective::Db,
                vec![DataExpr::Str(token.unescape()), DataExpr::Const(0)],
            ));
            raw_instr!(ctx.f, "mov {reg}, {name}");
            Ok(())
        }
        Expr::Global(token, uid) => {
            if ctx.global_vars[*uid].is_vec {
                raw_instr!(ctx.f, "mov {reg}, _{token}");
            } else {
                raw_instr!(ctx.f, "mov {reg}, [_{token}]");
            }
            Ok(())
        }
        Expr::Deref(token, id) => {
            let offset = ctx.get_offset(*id);
            let offset = ctx.get_offset(*id);
            let s = size_of_type(ctx.get_type_of_var(*id).get_inner());
            if ctx.vars[*id].ty.is_array() {
                raw_instr!(ctx.f, "lea {reg}, [rbp-{offset}]");
            } else {
                raw_instr!(ctx.f, "mov {reg}, [rbp-{offset}]");
            }
            match s {
                SizeOperator::Byte => {
                    raw_instr!(ctx.f, "movzx {reg}, {s} [{reg}]");
                }
                SizeOperator::Word => todo!(),
                SizeOperator::Dword => {
                    raw_instr!(ctx.f, "mov {reg}, [{reg}]");
                }
                SizeOperator::Qword => {
                    raw_instr!(ctx.f, "mov {reg}, [{reg}]");
                }
                _ => todo!(),
            }

            Ok(())
        }
        Expr::Ref(token, id) => {
            let offset = ctx.get_offset(*id);
            raw_instr!(ctx.f, "lea {reg}, [rbp-{offset}]");
            Ok(())
        }
        Expr::GlobalDeref(token, uid) => {
            let global_var = &ctx.global_vars[*uid];
            if global_var.is_vec {
                raw_instr!(ctx.f, "mov {reg}, _{token}");
            } else {
                raw_instr!(ctx.f, "mov {reg}, [_{token}]");
            }
            let s = size_of_type(&global_var.ty);
            match s {
                SizeOperator::Byte => {
                    raw_instr!(ctx.f, "movzx {reg}, {s} [{reg}]");
                }
                SizeOperator::Word => todo!(),
                SizeOperator::Dword => {
                    raw_instr!(ctx.f, "mov {reg}, [{reg}]");
                }
                SizeOperator::Qword => {
                    raw_instr!(ctx.f, "mov {reg}, [{reg}]");
                }
                _ => todo!(),
            }

            Ok(())
        }
    }
}

fn calculate_stack_offsets(vars: &[Var<'_>]) -> (usize, Vec<usize>) {
    let mut offsets = vec![0; vars.len()];
    let mut offset = 0usize;
    for (i, var) in vars.iter().enumerate() {
        if !var.used {continue;}
        let size = var.ty.size();
        offset += if size <= 8 { 8 } else { size };
        offsets[i] = offset;
    }
    (offset, offsets)
}

fn size_of_type(ty: &Type) -> SizeOperator {
    match ty {
        Type::Char => SizeOperator::Byte,
        Type::SBits(8) | Type::UBits(8) => SizeOperator::Byte,
        Type::SBits(16) | Type::UBits(16) => SizeOperator::Word,
        Type::SBits(32) | Type::UBits(32) => SizeOperator::Dword,
        Type::SBits(64) | Type::UBits(64) => SizeOperator::Qword,
        Type::Bool => SizeOperator::Dword,
        Type::PtrTo(_) | Type::Array(..) => SizeOperator::Qword,
        _ => todo!(),
    }
}
