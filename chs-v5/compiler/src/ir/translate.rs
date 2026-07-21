use std::collections::HashMap;

use crate::diag::DiagnosticReporter;
use crate::syntax::ast as s;
use lex_just_parse::lexer::Loc;
use s::map_type;

use super::block::BasicBlock;
use super::builder::IrBuilder;
use super::function::{Function, /*RunBlock, RunId,*/ Signature};
use super::inst::{BlockId, InstData, Instruction, Operand};
use super::module::Module;
use super::types::Type;

struct GlobalEnv {
    functions: HashMap<String, Signature>,
}

pub fn translate_ast_items(
    items: &[s::FileItem],
    type_db: crate::types::TypeDatabase,
    reporter: &mut DiagnosticReporter,
) -> Option<Module> {
    let mut env = GlobalEnv {
        functions: HashMap::new(),
    };
    let mut type_db = type_db;

    // Pass 1: Gather global declarations
    for item in items {
        if let s::FileItem::FunctionDecl(decl) = item {
            if decl.generic_params.is_some() {
                continue;
            }
            let mut params = Vec::new();
            for param in &decl.signature.parameters {
                params.push(map_type(&param.typ, &mut type_db));
            }
            let return_type = match &decl.signature.return_type {
                Some(ty) => map_type(ty, &mut type_db),
                None => type_db.void(),
            };
            let name = decl.resolved_name.clone().unwrap();
            let signature = Signature {
                name: decl.signature.name.source().to_string(),
                has_va_args: decl.signature.va_args,
                params,
                return_type,
                mangled_name: name.clone(),
            };

            match env.functions.entry(name) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    reporter.report(
                        decl.signature.name.loc,
                        format!(
                            "Function '{}' is already defined",
                            decl.resolved_name.as_ref().unwrap()
                        ),
                    );
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(signature);
                }
            }
        }
    }

    let mut module = Module::new(type_db);

    // Pass 2: Translate bodies
    for item in items {
        match item {
            s::FileItem::FunctionDecl(decl) => {
                if decl.generic_params.is_some() {
                    continue;
                }
                let name = decl.resolved_name.clone().unwrap();
                if let Some(signature) = env.functions.get(&name).cloned() {
                    let func =
                        translate_function(decl, signature, &env, &mut module.type_db, reporter);
                    module.add_function(func);
                }
            }
            // s::FileItem::Directive(s::Directive::Run(block)) => {
            //     let run_id = module.next_run_id();
            //     let run_block =
            //         translate_run_block(run_id, block, &env, &mut module.type_db, reporter);
            //     module.add_run_block(run_block);
            // }
            _ => {}
        }
    }

    if reporter.has_errors() {
        None
    } else {
        Some(module)
    }
}

#[derive(Clone, Copy)]
struct LoopTarget {
    break_block: BlockId,
    continue_block: BlockId,
    scope_depth: usize,
}

struct Translator<'a, 'b> {
    builder: IrBuilder<'a>,
    scopes: Vec<HashMap<String, (Operand, Type)>>,
    defers: Vec<Vec<s::Stmt>>,
    env: &'b GlobalEnv,
    reporter: &'b mut DiagnosticReporter,
    loop_targets: Vec<LoopTarget>,
}

impl<'a, 'b> Translator<'a, 'b> {
    fn new(
        blocks: &'a mut Vec<BasicBlock>,
        instructions: &'a mut Vec<InstData>,
        env: &'b GlobalEnv,
        type_db: &'a mut crate::types::TypeDatabase,
        reporter: &'b mut DiagnosticReporter,
    ) -> Self {
        Self {
            builder: IrBuilder::new(blocks, instructions, type_db),
            scopes: vec![HashMap::new()],
            defers: vec![Vec::new()],
            env,
            reporter,
            loop_targets: Vec::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.defers.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.defers.pop();
    }

    fn run_defers(&mut self, up_to_depth: usize) {
        let current_depth = self.defers.len();
        if current_depth == 0 {
            return;
        }
        for depth in (up_to_depth..current_depth).rev() {
            let defers = self.defers[depth].clone();
            for stmt in defers.iter().rev() {
                self.translate_stmt(stmt);
            }
        }
    }

    fn insert_var(&mut self, name: String, ptr: Operand, ty: Type) {
        self.scopes.last_mut().unwrap().insert(name, (ptr, ty));
    }

    fn lookup_var(&self, name: &str) -> Option<(Operand, Type)> {
        for scope in self.scopes.iter().rev() {
            if let Some((ptr, ty)) = scope.get(name) {
                return Some((ptr.clone(), *ty));
            }
        }
        None
    }

    fn find_function_mangled_name(
        &self,
        demangled_name: &str,
        param_count: usize,
    ) -> Option<String> {
        for (mangled_name, sig) in &self.env.functions {
            if sig.name == demangled_name && sig.params.len() == param_count {
                return Some(mangled_name.clone());
            }
        }
        None
    }

    fn emit_bounds_check(&mut self, loc: Loc, idx_op: Operand, len_op: Operand) {
        let void_ty = self.builder.type_db.void();

        let message_op = Operand::String(std::rc::Rc::from(
            format!("{}: Index out of bounds", loc).as_str(),
        ));
        let mangled_name = self.find_function_mangled_name("chs__oob_check", 3).unwrap();
        let callee = Operand::String(std::rc::Rc::from(mangled_name));
        self.builder.build_inst(
            Instruction::Call(callee, vec![message_op, idx_op, len_op].into_boxed_slice()),
            void_ty,
        );
    }

    fn get_member_offset(&mut self, base_ty: Type, prop_name: &str) -> (u32, Type) {
        let canonical = self.builder.type_db.resolve(base_ty);
        match self.builder.type_db.get_type(canonical) {
            crate::types::Type::Slice(elem) => {
                let elem = *elem;
                if prop_name == "data" {
                    (0, self.builder.type_db.pointer(elem))
                } else if prop_name == "len" {
                    (8, self.builder.type_db.int())
                } else {
                    panic!("Unknown slice property: {}", prop_name);
                }
            }
            crate::types::Type::Struct {
                fields: Some(fields),
                ..
            } => {
                let layout =
                    crate::codegen::qbe::StructLayout::compute(canonical, &self.builder.type_db);
                let idx = fields.iter().position(|f| f.name == prop_name).unwrap();
                (layout.fields[idx].offset, fields[idx].ty)
            }
            crate::types::Type::Tuple(elements) => {
                let idx: usize = prop_name.parse().unwrap();
                let layout =
                    crate::codegen::qbe::StructLayout::compute(canonical, &self.builder.type_db);
                (layout.fields[idx].offset, elements[idx])
            }
            crate::types::Type::Any(inner) => self.get_member_offset(*inner, prop_name),
            crate::types::Type::String(inner) => self.get_member_offset(*inner, prop_name),
            crate::types::Type::Distinct { base, .. } => self.get_member_offset(*base, prop_name),
            _ => panic!(
                "Expected structural type, got {:?}",
                self.builder.type_db.get_type(canonical)
            ),
        }
    }

    fn translate_lvalue(&mut self, expr: &s::Expr) -> (Operand, Type) {
        match &expr.kind {
            s::ExprKind::Identifier(ident) => {
                let name = ident.source();
                if let Some((ptr, ty)) = self.lookup_var(name) {
                    (ptr, ty)
                }
                // else if let Some(ty) = self.builder.type_db.lookup_by_name(name) {
                //     (Operand::Int(0), ty)
                // }
                else {
                    self.reporter
                        .report(ident.loc, format!("Undefined variable '{}'", name));
                    (Operand::Int(0), self.builder.type_db.void())
                }
            }
            s::ExprKind::Unary(unary) if unary.op == s::Op::Deref => {
                let (ptr, ptr_ty) = self.translate_expr(&unary.right);
                let canonical = self.builder.type_db.resolve(ptr_ty);
                let inner_ty = match self.builder.type_db.get_type(canonical) {
                    crate::types::Type::Pointer(inner) => *inner,
                    _ => {
                        self.reporter
                            .report(expr.loc(), "Expected pointer type for dereference");
                        self.builder.type_db.void()
                    }
                };
                (ptr, inner_ty)
            }
            s::ExprKind::Member(mem) => {
                let is_lval = matches!(
                    &mem.object.kind,
                    s::ExprKind::Identifier(_)
                        | s::ExprKind::Unary(s::UnaryExpr {
                            op: s::Op::Deref,
                            ..
                        })
                        | s::ExprKind::Member(_)
                        | s::ExprKind::Index(_)
                );
                let (mut obj_ptr, mut obj_ty) = if is_lval {
                    self.translate_lvalue(&mem.object)
                } else {
                    let (val, ty) = self.translate_expr(&mem.object);
                    let canonical = self.builder.type_db.resolve(ty);
                    if let crate::types::Type::Pointer(inner) =
                        self.builder.type_db.get_type(canonical)
                    {
                        (val, *inner)
                    } else {
                        let ptr = self.builder.build_alloca(ty);
                        self.builder.build_store(ty, ptr.clone(), val);
                        (ptr, ty)
                    }
                };
                let prop_name = mem.property.source();

                let mut canonical = self.builder.type_db.resolve(obj_ty);
                while let crate::types::Type::Pointer(inner) =
                    self.builder.type_db.get_type(canonical).clone()
                {
                    obj_ptr = self.builder.build_load(obj_ty, obj_ptr);
                    obj_ty = inner;
                    canonical = self.builder.type_db.resolve(obj_ty);
                }

                if let crate::types::Type::Enum { repr, variants, .. } =
                    self.builder.type_db.get_type(canonical).clone()
                {
                    if let Some(v) = variants.iter().find(|f| f.name == prop_name) {
                        (Operand::Int(v.default_value), repr)
                    } else {
                        self.reporter.report(
                            mem.property.loc,
                            format!("variant '{}' not found", prop_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else if let crate::types::Type::Array(elem_ty, _) =
                    self.builder.type_db.get_type(canonical).clone()
                {
                    if prop_name == "data" {
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);
                        let first_elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(obj_ptr, Operand::Int(0)), ptr_ty);
                        let temp_ptr = self.builder.build_alloca(ptr_ty);
                        self.builder.build_store(
                            ptr_ty,
                            temp_ptr.clone(),
                            Operand::Reg(first_elem_ptr),
                        );
                        (temp_ptr, ptr_ty)
                    } else {
                        self.reporter.report(
                            mem.property.loc,
                            format!("Array field '{}' not found", prop_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else {
                    let (offset, field_ty) = self.get_member_offset(obj_ty, prop_name);
                    let ptr_ty = self.builder.type_db.pointer(field_ty);
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(obj_ptr, offset), ptr_ty);
                    (Operand::Reg(field_ptr), field_ty)
                }
            }
            s::ExprKind::Index(idx) => {
                let loc = expr.loc();
                let (arr_ptr, arr_ty) = self.translate_lvalue(&idx.array);
                let (idx_op, _) = self.translate_expr(&idx.index);

                let canonical = self.builder.type_db.resolve(arr_ty);
                match self.builder.type_db.get_type(canonical) {
                    crate::types::Type::Pointer(elem_ty) => {
                        let elem_ty = *elem_ty;
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);
                        let loaded_ptr = self.builder.build_load(arr_ty, arr_ptr);
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(loaded_ptr, idx_op), ptr_ty);
                        (Operand::Reg(elem_ptr), elem_ty)
                    }
                    crate::types::Type::Array(elem_ty, size) => {
                        let elem_ty = *elem_ty;
                        let size = *size;
                        self.emit_bounds_check(loc, idx_op.clone(), Operand::Int(size as u64));
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(arr_ptr, idx_op), ptr_ty);
                        (Operand::Reg(elem_ptr), elem_ty)
                    }
                    crate::types::Type::Slice(elem_ty) => {
                        let elem_ty = *elem_ty;
                        let ptr_ty = self.builder.type_db.pointer(elem_ty);

                        // Load slice length (at offset 8)
                        let int_ptr_ty = self.builder.type_db.pointer(self.builder.type_db.int());
                        let len_field_ptr = self
                            .builder
                            .build_inst(Instruction::GetMemberPtr(arr_ptr.clone(), 8), int_ptr_ty);
                        let len_op = self
                            .builder
                            .build_load(self.builder.type_db.int(), Operand::Reg(len_field_ptr));

                        self.emit_bounds_check(loc, idx_op.clone(), len_op);

                        let data_ptr_ty = self.builder.type_db.pointer(ptr_ty);
                        let data_field_ptr = self
                            .builder
                            .build_inst(Instruction::GetMemberPtr(arr_ptr, 0), data_ptr_ty);
                        let data_ptr = self
                            .builder
                            .build_load(ptr_ty, Operand::Reg(data_field_ptr));
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(data_ptr, idx_op), ptr_ty);
                        (Operand::Reg(elem_ptr), elem_ty)
                    }
                    _ => {
                        self.reporter
                            .report(idx.loc, "Cannot index into non-pointer/non-array type");
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                }
            }
            s::ExprKind::Unsafe(inner, _) => self.translate_lvalue(inner),
            _ => {
                self.reporter
                    .report(expr.loc(), "Expression is not an LValue");
                (Operand::Int(0), self.builder.type_db.void())
            }
        }
    }

    fn is_block_terminated(&self) -> bool {
        if let Some(block_id) = self.builder.current_block {
            let block = &self.builder.blocks[block_id.0 as usize];
            if let Some(&last_inst_id) = block.instructions.last() {
                let inst = &self.builder.instructions[last_inst_id.0 as usize].inst;
                return matches!(
                    inst,
                    Instruction::Br(_) | Instruction::CondBr(_, _, _) | Instruction::Return(_)
                );
            }
            false
        } else {
            true // No current block means we are terminated/unreachable!
        }
    }

    fn translate_block(&mut self, block: &s::BlockStmt) {
        self.push_scope();
        for stmt in &block.stmts {
            if self.is_block_terminated() {
                break;
            }
            self.translate_stmt(stmt);
        }
        if !self.is_block_terminated() {
            let current_depth = self.defers.len();
            if current_depth > 0 {
                self.run_defers(current_depth - 1);
            }
        }
        self.pop_scope();
    }

    fn translate_stmt(&mut self, stmt: &s::Stmt) {
        match stmt {
            s::Stmt::ExprStmt(expr) => {
                self.translate_expr(expr);
            }
            s::Stmt::VarDecl(decl) => {
                let (val, expr_ty) = self.translate_expr(&decl.expr);

                let declared_ty = decl
                    .var_type
                    .as_ref()
                    .map(|t| map_type(t, self.builder.type_db));

                let final_ty = if let Some(decl_ty) = declared_ty {
                    decl_ty
                } else {
                    expr_ty
                };

                if decl.names.len() == 1 {
                    let ptr = self.builder.build_alloca(final_ty);
                    self.builder.build_store(final_ty, ptr.clone(), val);
                    self.insert_var(decl.names[0].source().to_string(), ptr, final_ty);
                } else {
                    let tuple_ptr = self.builder.build_alloca(final_ty);
                    self.builder.build_store(final_ty, tuple_ptr.clone(), val);

                    let canonical_final = self.builder.type_db.resolve(final_ty);
                    if let crate::types::Type::Tuple(elements) =
                        self.builder.type_db.get_type(canonical_final).clone()
                    {
                        for (i, name) in decl.names.iter().enumerate() {
                            let elem_ty = elements[i];
                            let ptr_ty = self.builder.type_db.pointer(elem_ty);
                            let (offset, _) = self.get_member_offset(final_ty, &i.to_string());
                            let member_ptr = self.builder.build_inst(
                                Instruction::GetMemberPtr(tuple_ptr.clone(), offset),
                                ptr_ty,
                            );
                            let elem_val =
                                self.builder.build_load(elem_ty, Operand::Reg(member_ptr));
                            let var_ptr = self.builder.build_alloca(elem_ty);
                            self.builder.build_store(elem_ty, var_ptr.clone(), elem_val);
                            self.insert_var(name.source().to_string(), var_ptr, elem_ty);
                        }
                    } else {
                        panic!("Expected tuple type for destructuring");
                    }
                }
            }
            s::Stmt::Return(_, expr_opt) => {
                let (val, ty) = if let Some(expr) = expr_opt {
                    self.translate_expr(expr)
                } else {
                    (Operand::Int(0), self.builder.type_db.void())
                };

                self.run_defers(0);

                let is_void = ty == self.builder.type_db.void();
                self.builder
                    .build_return(if is_void { None } else { Some(val) });
            }
            s::Stmt::Block(block) => {
                self.translate_block(block);
            }
            s::Stmt::IfStmt(s::IfStmt::If { cond, true_body }) => {
                let (cond_val, _) = self.translate_expr(cond);

                let true_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .build_cond_br(cond_val, true_block, merge_block);

                self.builder.set_block(true_block);
                self.translate_block(true_body);
                if !self.is_block_terminated() {
                    self.builder.build_br(merge_block);
                }

                self.builder.set_block(merge_block);
            }
            s::Stmt::IfStmt(s::IfStmt::IfElse {
                cond,
                true_body,
                false_body,
            }) => {
                let (cond_val, _) = self.translate_expr(cond);

                let true_block = self.builder.create_block();
                let false_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder
                    .build_cond_br(cond_val, true_block, false_block);

                self.builder.set_block(true_block);
                self.translate_block(true_body);
                let true_terminated = self.is_block_terminated();
                if !true_terminated {
                    self.builder.build_br(merge_block);
                }

                self.builder.set_block(false_block);
                self.translate_block(false_body);
                let false_terminated = self.is_block_terminated();
                if !false_terminated {
                    self.builder.build_br(merge_block);
                }

                if true_terminated && false_terminated {
                    self.builder.current_block = None;
                } else {
                    self.builder.set_block(merge_block);
                }
            }
            s::Stmt::ForStmt(s::ForStmt::ForCond { cond, body }) => {
                let cond_block = self.builder.create_block();
                let body_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.build_br(cond_block);

                self.builder.set_block(cond_block);
                let (cond_val, _) = self.translate_expr(cond);
                self.builder
                    .build_cond_br(cond_val, body_block, merge_block);

                self.builder.set_block(body_block);
                self.loop_targets.push(LoopTarget {
                    break_block: merge_block,
                    continue_block: cond_block,
                    scope_depth: self.scopes.len(),
                });
                self.translate_block(body);
                self.loop_targets.pop();

                if !self.is_block_terminated() {
                    self.builder.build_br(cond_block);
                }

                self.builder.set_block(merge_block);
            }
            s::Stmt::ForStmt(s::ForStmt::ForLoop(body)) => {
                let body_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.build_br(body_block);

                self.builder.set_block(body_block);
                self.loop_targets.push(LoopTarget {
                    break_block: merge_block,
                    continue_block: body_block,
                    scope_depth: self.scopes.len(),
                });
                self.translate_block(body);
                self.loop_targets.pop();

                if !self.is_block_terminated() {
                    self.builder.build_br(body_block);
                }

                self.builder.set_block(merge_block);
            }
            s::Stmt::Break(loc) => {
                if let Some(target) = self.loop_targets.last().cloned() {
                    self.run_defers(target.scope_depth);
                    self.builder.build_br(target.break_block);
                } else {
                    self.reporter
                        .report(*loc, "break statement outside of loop");
                }
            }
            s::Stmt::Continue(loc) => {
                if let Some(target) = self.loop_targets.last().cloned() {
                    self.run_defers(target.scope_depth);
                    self.builder.build_br(target.continue_block);
                } else {
                    self.reporter
                        .report(*loc, "continue statement outside of loop");
                }
            }
            s::Stmt::ForEach(fe) => {
                let (iter_val, iter_ty) = self.translate_expr(&fe.iter_expr);
                let iter_canon = self.builder.type_db.resolve(iter_ty);
                let (elem_ty, len_val, data_val) = if iter_canon == self.builder.type_db.string() {
                    let ptr = self.builder.build_alloca(iter_canon);
                    self.builder.build_store(iter_canon, ptr.clone(), iter_val);

                    let len_ptr_ty = self.builder.type_db.pointer(self.builder.type_db.int());
                    let len_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), 8), len_ptr_ty);
                    let len_val = self
                        .builder
                        .build_load(self.builder.type_db.int(), Operand::Reg(len_ptr));

                    let u8_ty = self.builder.type_db.u8();
                    let elem_ptr_ty = self.builder.type_db.pointer(u8_ty);
                    let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                    let data_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr, 0), data_ptr_ty);
                    let data_val = self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                    (u8_ty, len_val, data_val)
                } else {
                    match self.builder.type_db.get_type(iter_canon).clone() {
                        crate::types::Type::Array(elem, size) => {
                            let ptr = self.builder.build_alloca(iter_canon);
                            self.builder.build_store(iter_canon, ptr.clone(), iter_val);
                            let len_val = Operand::Int(size as u64);
                            let data_val = ptr;
                            (elem, len_val, data_val)
                        }
                        crate::types::Type::Slice(elem) => {
                            let ptr = self.builder.build_alloca(iter_canon);
                            self.builder.build_store(iter_canon, ptr.clone(), iter_val);

                            let len_ptr_ty =
                                self.builder.type_db.pointer(self.builder.type_db.int());
                            let len_ptr = self
                                .builder
                                .build_inst(Instruction::GetMemberPtr(ptr.clone(), 8), len_ptr_ty);
                            let len_val = self
                                .builder
                                .build_load(self.builder.type_db.int(), Operand::Reg(len_ptr));

                            let elem_ptr_ty = self.builder.type_db.pointer(elem);
                            let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                            let data_ptr = self
                                .builder
                                .build_inst(Instruction::GetMemberPtr(ptr, 0), data_ptr_ty);
                            let data_val =
                                self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                            (elem, len_val, data_val)
                        }
                        crate::types::Type::Pointer(inner) => {
                            let inner_canon = self.builder.type_db.resolve(inner);
                            if inner_canon == self.builder.type_db.string() {
                                let ptr = iter_val;
                                let len_ptr_ty =
                                    self.builder.type_db.pointer(self.builder.type_db.int());
                                let len_ptr = self.builder.build_inst(
                                    Instruction::GetMemberPtr(ptr.clone(), 8),
                                    len_ptr_ty,
                                );
                                let len_val = self
                                    .builder
                                    .build_load(self.builder.type_db.int(), Operand::Reg(len_ptr));

                                let u8_ty = self.builder.type_db.u8();
                                let elem_ptr_ty = self.builder.type_db.pointer(u8_ty);
                                let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                                let data_ptr = self
                                    .builder
                                    .build_inst(Instruction::GetMemberPtr(ptr, 0), data_ptr_ty);
                                let data_val =
                                    self.builder.build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                                (u8_ty, len_val, data_val)
                            } else {
                                match self.builder.type_db.get_type(inner_canon).clone() {
                                    crate::types::Type::Array(elem, size) => {
                                        let len_val = Operand::Int(size as u64);
                                        let data_val = iter_val;
                                        (elem, len_val, data_val)
                                    }
                                    crate::types::Type::Slice(elem) => {
                                        let ptr = iter_val;
                                        let len_ptr_ty = self
                                            .builder
                                            .type_db
                                            .pointer(self.builder.type_db.int());
                                        let len_ptr = self.builder.build_inst(
                                            Instruction::GetMemberPtr(ptr.clone(), 8),
                                            len_ptr_ty,
                                        );
                                        let len_val = self.builder.build_load(
                                            self.builder.type_db.int(),
                                            Operand::Reg(len_ptr),
                                        );

                                        let elem_ptr_ty = self.builder.type_db.pointer(elem);
                                        let data_ptr_ty = self.builder.type_db.pointer(elem_ptr_ty);
                                        let data_ptr = self.builder.build_inst(
                                            Instruction::GetMemberPtr(ptr, 0),
                                            data_ptr_ty,
                                        );
                                        let data_val = self
                                            .builder
                                            .build_load(elem_ptr_ty, Operand::Reg(data_ptr));
                                        (elem, len_val, data_val)
                                    }
                                    _ => panic!(
                                        "Expected array, slice or pointer to array/slice for foreach iteration"
                                    ),
                                }
                            }
                        }
                        _ => panic!(
                            "Expected array, slice or pointer to array/slice for foreach iteration"
                        ),
                    }
                };

                let int_ty = self.builder.type_db.int();
                let idx_ptr = self.builder.build_alloca(int_ty);
                self.builder
                    .build_store(int_ty, idx_ptr.clone(), Operand::Int(0));

                let cond_block = self.builder.create_block();
                let body_block = self.builder.create_block();
                let inc_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.build_br(cond_block);

                // Condition block: idx < len
                self.builder.set_block(cond_block);
                let idx_val = self.builder.build_load(int_ty, idx_ptr.clone());
                let cmp_val = self.builder.build_inst(
                    Instruction::Lt(int_ty, idx_val.clone(), len_val),
                    self.builder.type_db.bool(),
                );
                self.builder
                    .build_cond_br(Operand::Reg(cmp_val), body_block, merge_block);

                // Body block
                self.builder.set_block(body_block);

                self.push_scope();

                // Get element: data_val[idx_val]
                let elem_ptr_ty = self.builder.type_db.pointer(elem_ty);
                let elem_ptr = self
                    .builder
                    .build_inst(Instruction::GetIndexPtr(data_val, idx_val), elem_ptr_ty);
                let elem_val = self.builder.build_load(elem_ty, Operand::Reg(elem_ptr));

                let var_ptr = self.builder.build_alloca(elem_ty);
                self.builder.build_store(elem_ty, var_ptr.clone(), elem_val);

                self.insert_var(fe.var_name.source().to_string(), var_ptr, elem_ty);

                self.loop_targets.push(LoopTarget {
                    break_block: merge_block,
                    continue_block: inc_block,
                    scope_depth: self.scopes.len(),
                });

                self.translate_block(&fe.body);

                self.loop_targets.pop();
                self.pop_scope();

                if !self.is_block_terminated() {
                    self.builder.build_br(inc_block);
                }

                // Increment block: idx = idx + 1
                self.builder.set_block(inc_block);
                let current_idx = self.builder.build_load(int_ty, idx_ptr.clone());
                let next_idx = self
                    .builder
                    .build_inst(Instruction::Add(current_idx, Operand::Int(1)), int_ty);
                self.builder
                    .build_store(int_ty, idx_ptr.clone(), Operand::Reg(next_idx));
                self.builder.build_br(cond_block);

                self.builder.set_block(merge_block);
            }
            s::Stmt::Defer(_, inner_stmt) => {
                self.defers.last_mut().unwrap().push(*inner_stmt.clone());
            }
            s::Stmt::Switch(switch_stmt) => {
                let merge_block = self.builder.create_block();
                let mut all_branches_terminate = true;

                let mut check_blocks = Vec::new();
                for _ in 0..switch_stmt.branches.len() {
                    check_blocks.push(self.builder.create_block());
                }

                let default_block = if switch_stmt.default.is_some() {
                    Some(self.builder.create_block())
                } else {
                    None
                };

                let (cond_val, cond_ty) = self.translate_expr(&switch_stmt.cond);
                let canonical_cond = self.builder.type_db.resolve(cond_ty);

                let first_dest = if !check_blocks.is_empty() {
                    check_blocks[0]
                } else if let Some(def_b) = default_block {
                    def_b
                } else {
                    merge_block
                };
                self.builder.build_br(first_dest);

                for (i, branch) in switch_stmt.branches.iter().enumerate() {
                    self.builder.set_block(check_blocks[i]);

                    let mut pattern_vars = Vec::new();
                    let cmp = match &branch.pattern.kind {
                        s::ExprKind::Call(call) => {
                            if let s::ExprKind::Member(mem) = &call.callee.kind {
                                let base_ty = mem
                                    .object
                                    .resolved_type
                                    .expect("Object must have a resolved type");
                                let canonical_base = self.builder.type_db.resolve(base_ty);
                                if let crate::types::Type::Enum { repr, variants, .. } =
                                    self.builder.type_db.get_type(canonical_base).clone()
                                {
                                    let prop_name = mem.property.source();
                                    let v = variants.iter().find(|f| f.name == prop_name).unwrap();

                                    let cond_ptr = self.builder.build_alloca(canonical_cond);
                                    self.builder.build_store(
                                        canonical_cond,
                                        cond_ptr.clone(),
                                        cond_val.clone(),
                                    );
                                    let tag_val = self.builder.build_load(repr, cond_ptr.clone());

                                    let inst = Instruction::Eq(
                                        repr,
                                        tag_val,
                                        Operand::Int(v.default_value),
                                    );
                                    let cmp_val = Operand::Reg(
                                        self.builder.build_inst(inst, self.builder.type_db.bool()),
                                    );

                                    if let Some(payload_ty) = v.payload {
                                        let canonical_payload =
                                            self.builder.type_db.resolve(payload_ty);
                                        let layout = crate::codegen::qbe::EnumLayout::compute(
                                            canonical_base,
                                            &self.builder.type_db,
                                        );
                                        let u8_ptr_ty =
                                            self.builder.type_db.pointer(self.builder.type_db.u8());
                                        let casted_cond = self.builder.build_inst(
                                            Instruction::Cast(cond_ptr.clone()),
                                            u8_ptr_ty,
                                        );
                                        let payload_byte_ptr = self.builder.build_inst(
                                            Instruction::GetIndexPtr(
                                                Operand::Reg(casted_cond),
                                                Operand::Int(layout.payload_offset as u64),
                                            ),
                                            u8_ptr_ty,
                                        );
                                        let payload_ptr_ty =
                                            self.builder.type_db.pointer(payload_ty);
                                        let payload_ptr = self.builder.build_inst(
                                            Instruction::Cast(Operand::Reg(payload_byte_ptr)),
                                            payload_ptr_ty,
                                        );

                                        match self
                                            .builder
                                            .type_db
                                            .get_type(canonical_payload)
                                            .clone()
                                        {
                                            crate::types::Type::Tuple(elements) => {
                                                for (idx, arg) in call.arguments.iter().enumerate()
                                                {
                                                    if let s::ExprKind::Identifier(ident) =
                                                        &arg.kind
                                                    {
                                                        let elem_ty = elements[idx];
                                                        let elem_ptr_ty =
                                                            self.builder.type_db.pointer(elem_ty);
                                                        let (offset, _) = self.get_member_offset(
                                                            payload_ty,
                                                            &idx.to_string(),
                                                        );
                                                        let field_ptr = self.builder.build_inst(
                                                            Instruction::GetMemberPtr(
                                                                Operand::Reg(payload_ptr),
                                                                offset,
                                                            ),
                                                            elem_ptr_ty,
                                                        );
                                                        let val = self.builder.build_load(
                                                            elem_ty,
                                                            Operand::Reg(field_ptr),
                                                        );
                                                        pattern_vars.push((
                                                            ident.source().to_string(),
                                                            val,
                                                            elem_ty,
                                                        ));
                                                    }
                                                }
                                            }
                                            crate::types::Type::Struct {
                                                fields: Some(fields),
                                                ..
                                            } => {
                                                for (idx, arg) in call.arguments.iter().enumerate()
                                                {
                                                    if let s::ExprKind::Identifier(ident) =
                                                        &arg.kind
                                                    {
                                                        let field_name = &fields[idx].name;
                                                        let elem_ty = fields[idx].ty;
                                                        let elem_ptr_ty =
                                                            self.builder.type_db.pointer(elem_ty);
                                                        let (offset, _) = self.get_member_offset(
                                                            payload_ty, field_name,
                                                        );
                                                        let field_ptr = self.builder.build_inst(
                                                            Instruction::GetMemberPtr(
                                                                Operand::Reg(payload_ptr),
                                                                offset,
                                                            ),
                                                            elem_ptr_ty,
                                                        );
                                                        let val = self.builder.build_load(
                                                            elem_ty,
                                                            Operand::Reg(field_ptr),
                                                        );
                                                        pattern_vars.push((
                                                            ident.source().to_string(),
                                                            val,
                                                            elem_ty,
                                                        ));
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    cmp_val
                                } else {
                                    unreachable!()
                                }
                            } else {
                                let (pat_val, _pat_ty) = self.translate_expr(&branch.pattern);
                                let inst = Instruction::Eq(cond_ty, cond_val.clone(), pat_val);
                                Operand::Reg(
                                    self.builder.build_inst(inst, self.builder.type_db.bool()),
                                )
                            }
                        }
                        s::ExprKind::Member(mem) => {
                            let base_ty = mem
                                .object
                                .resolved_type
                                .expect("Object must have a resolved type");
                            let canonical_base = self.builder.type_db.resolve(base_ty);
                            if let crate::types::Type::Enum { repr, variants, .. } =
                                self.builder.type_db.get_type(canonical_base).clone()
                            {
                                let prop_name = mem.property.source();
                                let v = variants.iter().find(|f| f.name == prop_name).unwrap();

                                let cond_ptr = self.builder.build_alloca(canonical_cond);
                                self.builder.build_store(
                                    canonical_cond,
                                    cond_ptr.clone(),
                                    cond_val.clone(),
                                );
                                let tag_val = self.builder.build_load(repr, cond_ptr.clone());

                                let inst =
                                    Instruction::Eq(repr, tag_val, Operand::Int(v.default_value));
                                Operand::Reg(
                                    self.builder.build_inst(inst, self.builder.type_db.bool()),
                                )
                            } else {
                                let (pat_val, _pat_ty) = self.translate_expr(&branch.pattern);
                                let inst = Instruction::Eq(cond_ty, cond_val.clone(), pat_val);
                                Operand::Reg(
                                    self.builder.build_inst(inst, self.builder.type_db.bool()),
                                )
                            }
                        }
                        _ => {
                            let (pat_val, pat_ty) = self.translate_expr(&branch.pattern);
                            let canonical_pat = self.builder.type_db.resolve(pat_ty);
                            if let crate::types::Type::Enum { repr, .. } =
                                self.builder.type_db.get_type(canonical_cond).clone()
                            {
                                let cond_ptr = self.builder.build_alloca(canonical_cond);
                                self.builder.build_store(
                                    canonical_cond,
                                    cond_ptr.clone(),
                                    cond_val.clone(),
                                );
                                let tag_cond = self.builder.build_load(repr, cond_ptr);

                                let pat_ptr = self.builder.build_alloca(canonical_pat);
                                self.builder
                                    .build_store(canonical_pat, pat_ptr.clone(), pat_val);
                                let tag_pat = self.builder.build_load(repr, pat_ptr);

                                let inst = Instruction::Eq(repr, tag_cond, tag_pat);
                                Operand::Reg(
                                    self.builder.build_inst(inst, self.builder.type_db.bool()),
                                )
                            } else {
                                let inst = Instruction::Eq(cond_ty, cond_val.clone(), pat_val);
                                Operand::Reg(
                                    self.builder.build_inst(inst, self.builder.type_db.bool()),
                                )
                            }
                        }
                    };

                    let body_block = self.builder.create_block();
                    let false_dest = if i + 1 < check_blocks.len() {
                        check_blocks[i + 1]
                    } else if let Some(def_b) = default_block {
                        def_b
                    } else {
                        merge_block
                    };

                    self.builder.build_cond_br(cmp, body_block, false_dest);

                    self.builder.set_block(body_block);

                    self.scopes.push(HashMap::new());
                    for (name, val, ty) in pattern_vars {
                        let var_ptr = self.builder.build_alloca(ty);
                        self.builder.build_store(ty, var_ptr.clone(), val);
                        self.scopes.last_mut().unwrap().insert(name, (var_ptr, ty));
                    }

                    self.translate_stmt(&branch.body);

                    self.scopes.pop();

                    if !self.is_block_terminated() {
                        all_branches_terminate = false;
                        self.builder.build_br(merge_block);
                    }
                }

                if let Some(ref default_stmt) = switch_stmt.default {
                    let def_b = default_block.unwrap();
                    self.builder.set_block(def_b);
                    self.translate_stmt(default_stmt);
                    if !self.is_block_terminated() {
                        all_branches_terminate = false;
                        self.builder.build_br(merge_block);
                    }
                } else {
                    all_branches_terminate = false;
                }

                if all_branches_terminate {
                    self.builder.current_block = None;
                } else {
                    self.builder.set_block(merge_block);
                }
            }
            s::Stmt::Call(call) => {
                let dummy_expr = s::Expr::new(s::ExprKind::Call(call.clone()));
                self.translate_expr(&dummy_expr);
            }
        }
    }

    fn translate_expr(&mut self, expr: &s::Expr) -> (Operand, Type) {
        let (val, ty) = match &expr.kind {
            s::ExprKind::Null(_) => (
                Operand::Null,
                self.builder.type_db.pointer(self.builder.type_db.void()),
            ),
            s::ExprKind::Integer(lit) => (Operand::Int(lit.value), self.builder.type_db.int()),
            s::ExprKind::Bool(lit) => (Operand::Bool(lit.value), self.builder.type_db.bool()),
            s::ExprKind::Float(lit) => (Operand::Float(lit.value), self.builder.type_db.float()),
            s::ExprKind::StringLiteral(lit) => {
                let s = lit.unescape();
                (
                    Operand::String(std::rc::Rc::from(s.as_str())),
                    self.builder.type_db.string(),
                )
            }
            s::ExprKind::Identifier(ident) => {
                let name = ident.source();
                if let Some((ptr, ty)) = self.lookup_var(name) {
                    (self.builder.build_load(ty, ptr), ty)
                } else if let Some(sig) = self
                    .env
                    .functions
                    .get(name)
                    .or_else(|| self.env.functions.values().find(|sig| sig.name == name))
                {
                    let fn_pointer_ty = self
                        .builder
                        .type_db
                        .fn_pointer(sig.params.clone(), sig.return_type);
                    (
                        Operand::Global(std::rc::Rc::from(sig.mangled_name.as_str())),
                        fn_pointer_ty,
                    )
                }
                // else if let Some(ty) = self.builder.type_db.lookup_by_name(name) {
                //     (Operand::Int(0), ty)
                // }
                else {
                    self.reporter
                        .report(ident.loc, format!("Undefined variable '{}'", name));
                    (Operand::Int(0), self.builder.type_db.void())
                }
            }
            s::ExprKind::Member(_)
            | s::ExprKind::Index(_)
            | s::ExprKind::Unary(s::UnaryExpr {
                op: s::Op::Deref, ..
            }) => {
                if let s::ExprKind::Member(mem) = &expr.kind
                    && let Some(obj_ty) = mem.object.resolved_type
                {
                    let mut canonical = self.builder.type_db.resolve(obj_ty);
                    while let crate::types::Type::Pointer(inner) =
                        self.builder.type_db.get_type(canonical).clone()
                    {
                        canonical = self.builder.type_db.resolve(inner);
                    }
                    let prop_name = mem.property.source();
                    let ty_val = self.builder.type_db.get_type(canonical).clone();
                    match ty_val {
                        crate::types::Type::Enum { repr, variants, .. } => {
                            if let Some(v) = variants.iter().find(|f| f.name == prop_name) {
                                let ptr = self.builder.build_alloca(canonical);
                                self.builder.build_store(
                                    repr,
                                    ptr.clone(),
                                    Operand::Int(v.default_value),
                                );
                                return (self.builder.build_load(canonical, ptr), canonical);
                            }
                        }
                        crate::types::Type::Array(_, size) => {
                            if prop_name == "len" {
                                return (Operand::Int(size as u64), self.builder.type_db.int());
                            }
                        }
                        _ => {}
                    }
                }
                let (ptr, ty) = self.translate_lvalue(expr);
                (self.builder.build_load(ty, ptr), ty)
            }
            s::ExprKind::Binary(bin) => {
                let (left, left_ty) = self.translate_expr(&bin.left);
                let (right, right_ty) = self.translate_expr(&bin.right);

                if let Some(callee_name) = &bin.use_operator_overload {
                    let mut args = Vec::new();

                    args.push(left);
                    args.push(right);

                    if let Some(signature) = self.env.functions.get(callee_name) {
                        let ret_ty = signature.return_type;
                        let callee = Operand::String(std::rc::Rc::from(callee_name.as_str()));
                        (
                            Operand::Reg(self.builder.build_inst(
                                Instruction::Call(callee, args.into_boxed_slice()),
                                ret_ty,
                            )),
                            ret_ty,
                        )
                    } else {
                        self.reporter.report(
                            bin.op_loc(),
                            format!("Call to undefined function '{}'", callee_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else {
                    let is_comp = matches!(
                        bin.op,
                        s::Op::Eq
                            | s::Op::NotEq
                            | s::Op::Lt
                            | s::Op::LtEq
                            | s::Op::Gt
                            | s::Op::GtEq
                    );
                    let ret_ty = if is_comp {
                        self.builder.type_db.bool()
                    } else {
                        let left_canon = self.builder.type_db.resolve(left_ty);
                        let right_canon = self.builder.type_db.resolve(right_ty);
                        let left_is_ptr = matches!(
                            self.builder.type_db.get_type(left_canon),
                            crate::types::Type::Pointer(_)
                        );
                        let right_is_ptr = matches!(
                            self.builder.type_db.get_type(right_canon),
                            crate::types::Type::Pointer(_)
                        );

                        if bin.op == s::Op::Add && right_is_ptr {
                            right_ty
                        } else if bin.op == s::Op::Sub && left_is_ptr && right_is_ptr {
                            self.builder.type_db.int()
                        } else {
                            left_ty
                        }
                    };

                    let inst = match bin.op {
                        s::Op::Add => Instruction::Add(left, right),
                        s::Op::Sub => Instruction::Sub(left, right),
                        s::Op::Mul => Instruction::Mul(left, right),
                        s::Op::Div => Instruction::Div(left, right),
                        s::Op::Mod => Instruction::Mod(left, right),
                        s::Op::Eq => Instruction::Eq(left_ty, left, right),
                        s::Op::NotEq => Instruction::NotEq(left_ty, left, right),
                        s::Op::Lt => Instruction::Lt(left_ty, left, right),
                        s::Op::LtEq => Instruction::LtEq(left_ty, left, right),
                        s::Op::Gt => Instruction::Gt(left_ty, left, right),
                        s::Op::GtEq => Instruction::GtEq(left_ty, left, right),
                        s::Op::And => Instruction::And(left, right),
                        s::Op::Or => Instruction::Or(left, right),
                        s::Op::BitAnd => Instruction::BitAnd(left, right),
                        s::Op::BitOr => Instruction::BitOr(left, right),
                        s::Op::BitXor => Instruction::BitXor(left, right),
                        _ => {
                            self.reporter
                                .report(bin.op_loc(), "Unsupported binary operator");
                            Instruction::Add(left, right)
                        }
                    };
                    (Operand::Reg(self.builder.build_inst(inst, ret_ty)), ret_ty)
                }
            }
            s::ExprKind::Unary(unary) => {
                if unary.op == s::Op::Refer {
                    let (ptr, ty) = self.translate_lvalue(&unary.right);
                    let ptr_ty = self.builder.type_db.pointer(ty);
                    (ptr, ptr_ty)
                } else {
                    let (right, right_ty) = self.translate_expr(&unary.right);
                    let inst = match unary.op {
                        s::Op::Neg => Instruction::Neg(right),
                        s::Op::Not => Instruction::Not(right),
                        _ => {
                            self.reporter
                                .report(expr.loc(), "Unsupported unary operator");
                            Instruction::Neg(right)
                        }
                    };
                    (
                        Operand::Reg(self.builder.build_inst(inst, right_ty)),
                        right_ty,
                    )
                }
            }
            s::ExprKind::Cast(_, inner, loc) | s::ExprKind::AutoCast(inner, loc) => {
                let (src_val, _src_ty) = self.translate_expr(inner);
                let dest_ty = self.builder.type_db.resolve(expr.resolved_type.unwrap());

                // If it is a TypeVar, it means the type is unconstrained/ambiguous
                let canonical_dest = self.builder.type_db.resolve(dest_ty);
                if let crate::types::Type::TypeVar(_) =
                    self.builder.type_db.get_type(canonical_dest)
                {
                    self.reporter.report(
                        *loc,
                        "Ambiguous auto-cast: cannot infer target type from context",
                    );
                    (Operand::Int(0), self.builder.type_db.void())
                } else {
                    // Check if it's actually castable
                    let underlying_src = self.builder.type_db.get_underlying_type(_src_ty);
                    let underlying_dest = self.builder.type_db.get_underlying_type(dest_ty);
                    let is_cast_valid = if underlying_src == underlying_dest {
                        true
                    } else {
                        let mut is_src_castable =
                            self.builder.type_db.is_primitive_castable(underlying_src);
                        if !is_src_castable
                            && let crate::types::Type::Pointer(_) | crate::types::Type::Enum { .. } =
                                self.builder.type_db.get_type(underlying_src)
                        {
                            is_src_castable = true;
                        }

                        let mut is_dest_castable =
                            self.builder.type_db.is_primitive_castable(underlying_dest);
                        if !is_dest_castable
                            && let crate::types::Type::Pointer(_) | crate::types::Type::Enum { .. } =
                                self.builder.type_db.get_type(underlying_dest)
                        {
                            is_dest_castable = true;
                        }

                        is_src_castable && is_dest_castable
                    };
                    if !is_cast_valid {
                        let dest_str = self.builder.type_db.type_to_string(dest_ty);
                        self.reporter.report(
                            *loc,
                            format!("Cannot cast to non-castable type: {}", dest_str),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    } else {
                        // Generate the cast instruction
                        let inst = Instruction::Cast(src_val);
                        (
                            Operand::Reg(self.builder.build_inst(inst, dest_ty)),
                            dest_ty,
                        )
                    }
                }
            }
            s::ExprKind::Call(call) => {
                // if let s::ExprKind::Member(mem) = &call.callee.kind {
                //     let base_ty = mem
                //         .object
                //         .resolved_type
                //         .expect("Object must have a resolved type");
                //     let canonical_base = self.builder.type_db.resolve(base_ty);
                //     if let crate::types::Type::Enum { repr, variants, .. } =
                //         self.builder.type_db.get_type(canonical_base).clone()
                //     {
                //         let prop_name = mem.property.source();
                //         if let Some(v) = variants.iter().find(|f| f.name == prop_name) {
                //             let ptr = self.builder.build_alloca(canonical_base);
                //             self.builder.build_store(
                //                 repr,
                //                 ptr.clone(),
                //                 Operand::Int(v.default_value),
                //             );
                //             if let Some(payload_ty) = v.payload {
                //                 let canonical_payload = self.builder.type_db.resolve(payload_ty);
                //                 let layout = crate::codegen::qbe::EnumLayout::compute(
                //                     canonical_base,
                //                     &self.builder.type_db,
                //                 );
                //                 let u8_ptr_ty =
                //                     self.builder.type_db.pointer(self.builder.type_db.u8());
                //                 let casted_ptr = self
                //                     .builder
                //                     .build_inst(Instruction::Cast(ptr.clone()), u8_ptr_ty);
                //                 let payload_byte_ptr = self.builder.build_inst(
                //                     Instruction::GetIndexPtr(
                //                         Operand::Reg(casted_ptr),
                //                         Operand::Int(layout.payload_offset as u64),
                //                     ),
                //                     u8_ptr_ty,
                //                 );
                //                 let payload_ptr_ty = self.builder.type_db.pointer(payload_ty);
                //                 let payload_ptr = self.builder.build_inst(
                //                     Instruction::Cast(Operand::Reg(payload_byte_ptr)),
                //                     payload_ptr_ty,
                //                 );

                //                 match self.builder.type_db.get_type(canonical_payload).clone() {
                //                     crate::types::Type::Tuple(elements) => {
                //                         for (i, arg) in call.arguments.iter().enumerate() {
                //                             let (val, _) = self.translate_expr(arg);
                //                             let elem_ty = elements[i];
                //                             let elem_ptr_ty = self.builder.type_db.pointer(elem_ty);
                //                             let field_ptr = self.builder.build_inst(
                //                                 Instruction::GetMemberPtr(
                //                                     Operand::Reg(payload_ptr),
                //                                     std::rc::Rc::from(i.to_string()),
                //                                 ),
                //                                 elem_ptr_ty,
                //                             );
                //                             self.builder.build_store(
                //                                 elem_ty,
                //                                 Operand::Reg(field_ptr),
                //                                 val,
                //                             );
                //                         }
                //                     }
                //                     crate::types::Type::Struct {
                //                         fields: Some(fields),
                //                         ..
                //                     } => {
                //                         for (i, arg) in call.arguments.iter().enumerate() {
                //                             let (val, _) = self.translate_expr(arg);
                //                             let field_name = &fields[i].name;
                //                             let elem_ty = fields[i].ty;
                //                             let elem_ptr_ty = self.builder.type_db.pointer(elem_ty);
                //                             let field_ptr = self.builder.build_inst(
                //                                 Instruction::GetMemberPtr(
                //                                     Operand::Reg(payload_ptr),
                //                                     std::rc::Rc::from(field_name.as_str()),
                //                                 ),
                //                                 elem_ptr_ty,
                //                             );
                //                             self.builder.build_store(
                //                                 elem_ty,
                //                                 Operand::Reg(field_ptr),
                //                                 val,
                //                             );
                //                         }
                //                     }
                //                     _ => {}
                //                 }
                //             }
                //             return (self.builder.build_load(canonical_base, ptr), canonical_base);
                //         }
                //     }
                // }

                let mut args = Vec::new();
                for arg in &call.arguments {
                    let (arg_val, _) = self.translate_expr(arg);
                    args.push(arg_val);
                }

                if let Some(ref callee_name) = call.resolved_name {
                    if let Some(signature) = self.env.functions.get(callee_name) {
                        let ret_ty = signature.return_type;
                        let callee = Operand::String(std::rc::Rc::from(callee_name.as_str()));
                        let reg = self
                            .builder
                            .build_inst(Instruction::Call(callee, args.into_boxed_slice()), ret_ty);
                        if ret_ty == self.builder.type_db.noreturn() {
                            self.builder.build_return(None);
                        }
                        (Operand::Reg(reg), ret_ty)
                    } else {
                        self.reporter.report(
                            call.loc,
                            format!("Call to undefined function '{}'", callee_name),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                } else {
                    let (callee_val, callee_ty) = self.translate_expr(&call.callee);
                    let canonical = self.builder.type_db.resolve(callee_ty);
                    if let crate::types::Type::FnPointer { return_type, .. } =
                        self.builder.type_db.get_type(canonical).clone()
                    {
                        let reg = self.builder.build_inst(
                            Instruction::Call(callee_val, args.into_boxed_slice()),
                            return_type,
                        );
                        if return_type == self.builder.type_db.noreturn() {
                            self.builder.build_return(None);
                        }
                        (Operand::Reg(reg), return_type)
                    } else {
                        self.reporter.report(
                            call.loc,
                            "Expected function pointer type for indirect call".to_string(),
                        );
                        (Operand::Int(0), self.builder.type_db.void())
                    }
                }
            }
            s::ExprKind::Assign(assign) => {
                let (right_val, right_ty) = self.translate_expr(&assign.right);

                if let s::ExprKind::Tuple(targets, _) = &assign.left.kind {
                    let tuple_ptr = self.builder.build_alloca(right_ty);
                    self.builder
                        .build_store(right_ty, tuple_ptr.clone(), right_val.clone());

                    let canonical_right = self.builder.type_db.resolve(right_ty);
                    if let crate::types::Type::Tuple(elements) =
                        self.builder.type_db.get_type(canonical_right).clone()
                    {
                        for (i, target) in targets.iter().enumerate() {
                            let elem_ty = elements[i];
                            let ptr_ty = self.builder.type_db.pointer(elem_ty);
                            let (offset, _) = self.get_member_offset(right_ty, &i.to_string());
                            let member_ptr = self.builder.build_inst(
                                Instruction::GetMemberPtr(tuple_ptr.clone(), offset),
                                ptr_ty,
                            );
                            let elem_val =
                                self.builder.build_load(elem_ty, Operand::Reg(member_ptr));
                            let (target_ptr, target_ty) = self.translate_lvalue(target);
                            self.builder.build_store(target_ty, target_ptr, elem_val);
                        }
                    } else {
                        panic!("Expected tuple type for destructuring assignment");
                    }
                    (right_val, right_ty)
                } else {
                    let (ptr, left_ty) = self.translate_lvalue(&assign.left);
                    let val_to_store = match assign.assign_kind {
                        s::AssignKind::Default => right_val,
                        s::AssignKind::Add => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Add(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Sub => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Sub(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Mul => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Mul(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Div => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Div(current_val, right_val), left_ty),
                            )
                        }
                        s::AssignKind::Mod => {
                            let current_val = self.builder.build_load(left_ty, ptr.clone());
                            Operand::Reg(
                                self.builder
                                    .build_inst(Instruction::Mod(current_val, right_val), left_ty),
                            )
                        }
                    };

                    self.builder.build_store(left_ty, ptr, val_to_store.clone());
                    (val_to_store, left_ty)
                }
            }
            s::ExprKind::Tuple(elements, _) => {
                let tuple_ty = expr.resolved_type.expect("Tuple literal must be resolved");
                let ptr = self.builder.build_alloca(tuple_ty);

                let canonical = self.builder.type_db.resolve(tuple_ty);
                let element_tys = match self.builder.type_db.get_type(canonical) {
                    crate::types::Type::Tuple(elements) => elements.clone(),
                    _ => unreachable!("Must be tuple type"),
                };

                for (i, elem_expr) in elements.iter().enumerate() {
                    let elem_ty = element_tys[i];
                    let (val, _) = self.translate_expr(elem_expr);
                    let ptr_ty = self.builder.type_db.pointer(elem_ty);
                    let (offset, _) = self.get_member_offset(tuple_ty, &i.to_string());
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), offset), ptr_ty);
                    self.builder
                        .build_store(elem_ty, Operand::Reg(field_ptr), val);
                }

                (self.builder.build_load(tuple_ty, ptr), tuple_ty)
            }
            s::ExprKind::StructLiteral(lit) => {
                let struct_ty = expr.resolved_type.expect("Struct literal must be resolved");
                let ptr = self.builder.build_alloca(struct_ty);

                let canonical = self.builder.type_db.resolve(struct_ty);
                let fields = match self.builder.type_db.get_type(canonical) {
                    crate::types::Type::Struct {
                        fields: Some(fields),
                        ..
                    } => fields.clone(),
                    _ => unreachable!("Must be struct type"),
                };

                for f in &lit.fields {
                    let field_name = f.name.source();
                    let field_def = fields.iter().find(|fd| fd.name == field_name).unwrap();
                    let field_ty = field_def.ty;

                    let (val, _) = self.translate_expr(&f.value);
                    let ptr_ty = self.builder.type_db.pointer(field_ty);
                    let (offset, _) = self.get_member_offset(struct_ty, field_name);
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(ptr.clone(), offset), ptr_ty);
                    self.builder
                        .build_store(field_ty, Operand::Reg(field_ptr), val);
                }

                (self.builder.build_load(struct_ty, ptr), struct_ty)
            }
            s::ExprKind::Array(arr) => {
                let element_ty = self
                    .builder
                    .type_db
                    .get_inner_type_id(expr.resolved_type.unwrap());
                let elem_ptr_ty = self.builder.type_db.pointer(element_ty);
                let size = arr.elements.len();
                if arr.elements.is_empty() {
                    let array_ty = self.builder.type_db.array(element_ty, 0);
                    (self.builder.build_load(array_ty, Operand::Null), array_ty)
                } else {
                    let array_ty = self.builder.type_db.array(element_ty, size);
                    let ptr = self.builder.build_alloca(array_ty);
                    for (i, elem) in arr.elements.iter().enumerate() {
                        let (val, _) = self.translate_expr(elem);
                        let idx_op = Operand::Int(i as u64);
                        let elem_ptr = self
                            .builder
                            .build_inst(Instruction::GetIndexPtr(ptr.clone(), idx_op), elem_ptr_ty);
                        self.builder
                            .build_store(element_ty, Operand::Reg(elem_ptr), val);
                    }
                    (self.builder.build_load(array_ty, ptr), array_ty)
                }
            }
            s::ExprKind::TypeInfo(ast_ty, _) => {
                let target_ty = map_type(ast_ty, self.builder.type_db);
                let canonical_target = self.builder.type_db.resolve(target_ty);
                self.builder.type_db.queried_types.insert(canonical_target);
                let type_info_id = self.builder.type_db.type_info();
                let ptr_ty = self.builder.type_db.pointer(type_info_id);
                (
                    Operand::Global(std::rc::Rc::from(format!(
                        "chs_type_info_{}",
                        canonical_target.0
                    ))),
                    ptr_ty,
                )
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(scalar), _) => {
                let (val, scalar_ty) = self.translate_expr(scalar);
                let canonical_scalar = self.builder.type_db.resolve(scalar_ty);
                let val_ptr = self.builder.build_alloca(scalar_ty);
                self.builder.build_store(scalar_ty, val_ptr.clone(), val);

                self.builder.type_db.queried_types.insert(canonical_scalar);

                let any_ty = self.builder.type_db.any();
                let any_ptr = self.builder.build_alloca(any_ty);
                {
                    let type_info_ptr_ty = self
                        .builder
                        .type_db
                        .pointer(self.builder.type_db.type_info());
                    let field_ptr = self.builder.build_inst(
                        Instruction::GetMemberPtr(any_ptr.clone(), 8),
                        type_info_ptr_ty,
                    );
                    self.builder.build_store(
                        type_info_ptr_ty,
                        Operand::Reg(field_ptr),
                        Operand::Global(std::rc::Rc::from(format!(
                            "chs_type_info_{}",
                            canonical_scalar.0
                        ))),
                    );
                }
                {
                    let void_ptr_ty = self.builder.type_db.pointer(self.builder.type_db.void());
                    let field_ptr = self
                        .builder
                        .build_inst(Instruction::GetMemberPtr(any_ptr.clone(), 0), void_ptr_ty);
                    self.builder
                        .build_store(void_ptr_ty, Operand::Reg(field_ptr), val_ptr);
                }

                (self.builder.build_load(any_ty, any_ptr), any_ty)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Array(arr), _) => {
                let any_ty = self.builder.type_db.any();
                let element_ty = any_ty;
                let elem_ptr_ty = self.builder.type_db.pointer(any_ty);

                let array_ty = self.builder.type_db.array(element_ty, arr.len());
                let ptr = self.builder.build_alloca(array_ty);

                for (i, elem) in arr.iter().enumerate() {
                    let (val, scalar_ty) = self.translate_expr(elem);
                    let canonical_scalar = self.builder.type_db.resolve(scalar_ty);
                    let val_ptr = self.builder.build_alloca(scalar_ty);
                    self.builder.build_store(scalar_ty, val_ptr.clone(), val);
                    self.builder.type_db.queried_types.insert(canonical_scalar);

                    let any_ptr = self.builder.build_alloca(any_ty);
                    {
                        let type_info_ptr_ty = self
                            .builder
                            .type_db
                            .pointer(self.builder.type_db.type_info());
                        let field_ptr = self.builder.build_inst(
                            Instruction::GetMemberPtr(any_ptr.clone(), 8),
                            type_info_ptr_ty,
                        );
                        self.builder.build_store(
                            type_info_ptr_ty,
                            Operand::Reg(field_ptr),
                            Operand::Global(std::rc::Rc::from(format!(
                                "chs_type_info_{}",
                                canonical_scalar.0
                            ))),
                        );
                    }
                    {
                        let void_ptr_ty = self.builder.type_db.pointer(self.builder.type_db.void());
                        let field_ptr = self
                            .builder
                            .build_inst(Instruction::GetMemberPtr(any_ptr.clone(), 0), void_ptr_ty);
                        self.builder
                            .build_store(void_ptr_ty, Operand::Reg(field_ptr), val_ptr);
                    }

                    let idx_op = Operand::Int(i as u64);
                    let elem_ptr = self
                        .builder
                        .build_inst(Instruction::GetIndexPtr(ptr.clone(), idx_op), elem_ptr_ty);
                    self.builder
                        .build_store(element_ty, Operand::Reg(elem_ptr), any_ptr);
                }
                (self.builder.build_load(array_ty, ptr), array_ty)
            }
            s::ExprKind::Unsafe(inner, _) => self.translate_expr(inner),
            #[allow(unreachable_patterns)]
            _ => {
                self.reporter.report(
                    expr.loc(),
                    format!(
                        "Expression `{}` not supported in IR translation yet",
                        expr.name()
                    ),
                );
                (Operand::Int(0), self.builder.type_db.void())
            }
        };

        // Coercion check!
        let canonical_actual = self.builder.type_db.resolve(ty);
        if let Some(expected_ty) = expr.resolved_type {
            let canonical_expected = self.builder.type_db.resolve(expected_ty);
            if canonical_actual != canonical_expected
                && let (
                    crate::types::Type::Array(elem_ty, size),
                    crate::types::Type::Slice(slice_elem),
                ) = (
                    self.builder.type_db.get_type(canonical_actual).clone(),
                    self.builder.type_db.get_type(canonical_expected).clone(),
                )
                && self.builder.type_db.unify(elem_ty, slice_elem).is_ok()
            {
                let slice_ptr = self.builder.build_alloca(canonical_expected);
                let ptr_ty = self.builder.type_db.pointer(elem_ty);
                let data_ptr_ty = self.builder.type_db.pointer(ptr_ty);
                let data_field_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(slice_ptr.clone(), 0), data_ptr_ty);

                let array_ptr = if matches!(
                    expr.kind,
                    s::ExprKind::Array(_) | s::ExprKind::AnyCast(s::AnyCastExpr::Array(_), _)
                ) {
                    let array_ty = ty;
                    let ptr = self.builder.build_alloca(array_ty);
                    self.builder.build_store(ty, ptr.clone(), val.clone());
                    ptr
                } else {
                    let (lval_ptr, _) = self.translate_lvalue(expr);
                    lval_ptr
                };

                let first_elem_ptr = self
                    .builder
                    .build_inst(Instruction::GetIndexPtr(array_ptr, Operand::Int(0)), ptr_ty);
                self.builder.build_store(
                    ptr_ty,
                    Operand::Reg(data_field_ptr),
                    Operand::Reg(first_elem_ptr),
                );

                let int_ty = self.builder.type_db.int();
                let len_ptr_ty = self.builder.type_db.pointer(int_ty);
                let len_field_ptr = self
                    .builder
                    .build_inst(Instruction::GetMemberPtr(slice_ptr.clone(), 8), len_ptr_ty);
                self.builder.build_store(
                    int_ty,
                    Operand::Reg(len_field_ptr),
                    Operand::Int(size as u64),
                );

                return (
                    self.builder.build_load(canonical_expected, slice_ptr),
                    canonical_expected,
                );
            }
        }

        (val, ty)
    }
}

// Helper methods for Binary and Assign to get their operator Loc
impl s::BinaryExpr {
    fn op_loc(&self) -> lex_just_parse::lexer::Loc {
        self.right.loc()
    }
}

fn translate_function(
    decl: &s::FunctionDecl,
    signature: Signature,
    env: &GlobalEnv,
    type_db: &mut crate::types::TypeDatabase,
    reporter: &mut DiagnosticReporter,
) -> Function {
    let is_foreign = decl
        .directives
        .iter()
        .any(|d| matches!(d, s::FunctionDirective::Foreign(..)));
    let has_link_name = decl
        .directives
        .iter()
        .any(|d| matches!(d, s::FunctionDirective::LinkName(..)));

    let name = decl.resolved_name.clone().unwrap();
    let signature = signature.clone();
    let mut function = if is_foreign {
        let link_name = if has_link_name {
            name.clone()
        } else {
            decl.signature.name.source().to_string()
        };
        Function::foreign(name, link_name, signature)
    } else {
        Function::new(name, signature)
    };

    if let Function::Default {
        name: _,
        signature,
        blocks,
        instructions,
        entry_block,
    } = &mut function
        && let Some(body) = &decl.body
    {
        let mut translator = Translator::new(blocks, instructions, env, type_db, reporter);
        translator.builder.set_block(*entry_block);

        translator.push_scope();
        for (i, param) in decl.signature.parameters.iter().enumerate() {
            let ty = signature.params[i];
            let ptr = translator.builder.build_alloca(ty);
            translator
                .builder
                .build_store(ty, ptr.clone(), Operand::Param(i as u32));
            translator.insert_var(param.name.source().to_string(), ptr, ty);
        }

        translator.translate_block(body);

        if !translator.is_block_terminated() {
            if signature.return_type != translator.builder.type_db.void()
                && signature.return_type != translator.builder.type_db.noreturn()
            {
                translator
                    .reporter
                    .report(decl.signature.name.loc, "Function must return a value");
            }
            translator.builder.build_return(None);
        }
        translator.pop_scope();
    }

    function
}

// fn translate_run_block(
//     id: RunId,
//     block: &s::BlockStmt,
//     env: &GlobalEnv,
//     type_db: &mut crate::types::TypeDatabase,
//     reporter: &mut DiagnosticReporter,
// ) -> RunBlock {
//     let mut run_block = RunBlock::new(id);
//     let mut translator = Translator::new(
//         &mut run_block.blocks,
//         &mut run_block.instructions,
//         env,
//         type_db,
//         reporter,
//     );
//     translator.builder.set_block(run_block.entry_block);

//     translator.translate_block(block);

//     if !translator.is_block_terminated() {
//         translator.builder.build_return(None);
//     }

//     run_block
// }
