use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::diag::DiagnosticReporter;
use crate::syntax::ast as s;
use crate::types::{EnumVariant, StructField, Type, TypeDatabase, TypeID};
use lex_just_parse::lexer::{Loc, Token, TokenKind, TokenSource};
use s::map_type;

use super::errors::SemanticError;

#[derive(Clone, Debug)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<TypeID>,
    pub return_type: TypeID,
    pub mangled_name: String,
    pub has_va_args: bool,
}

pub struct SymbolTable {
    pub functions: HashMap<String, Vec<FunctionSignature>>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    pub fn add(&mut self, sig: FunctionSignature, db: &TypeDatabase) -> Result<(), ()> {
        let entry = self.functions.entry(sig.name.clone()).or_default();
        for existing in entry.iter() {
            if existing.params.len() == sig.params.len() {
                let mut matches = true;
                for (p1, p2) in existing.params.iter().zip(sig.params.iter()) {
                    if db.resolve(*p1) != db.resolve(*p2) {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    return Err(());
                }
            }
        }
        entry.push(sig);
        Ok(())
    }

    pub fn resolve(
        &self,
        name: &str,
        arg_types: &[TypeID],
        db: &TypeDatabase,
    ) -> Option<&FunctionSignature> {
        if let Some(overloads) = self.functions.get(name) {
            for overload in overloads {
                if overload.params.len() == arg_types.len() {
                    let mut matches = true;
                    for (p, arg) in overload.params.iter().zip(arg_types.iter()) {
                        if db.resolve(*p) != db.resolve(*arg) {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        return Some(overload);
                    }
                }
            }
        }
        None
    }
}

pub struct TypeChecker<'a> {
    pub type_db: TypeDatabase,
    pub symtab: SymbolTable,
    pub reporter: &'a mut DiagnosticReporter,
    scopes: Vec<HashMap<String, TypeID>>,
    expected_return_type: TypeID,
    pub declared_libraries: std::collections::HashSet<String>,
    pub struct_decls: HashMap<String, s::StructDecl>,
    pub enum_decls: HashMap<String, s::EnumDecl>,
    pub resolved_fns: HashMap<String, s::FunctionDecl>,
    pub struct_templates: HashMap<String, s::StructDecl>,
    pub enum_templates: HashMap<String, s::EnumDecl>,
    pub function_templates: HashMap<String, Vec<s::FunctionDecl>>,
    pub monomorphized_functions: HashMap<(String, Vec<TypeID>), String>,
    pub new_monomorphized_items: Vec<s::FileItem>,
    pub stack_backed_vars: std::collections::HashSet<String>,
    pub monomorphization_depth: usize,
}

impl<'a> TypeChecker<'a> {
    pub fn new(reporter: &'a mut DiagnosticReporter) -> Self {
        let type_db = TypeDatabase::new();
        let expected_return_type = type_db.void();
        Self {
            type_db,
            symtab: SymbolTable::new(),
            reporter,
            scopes: vec![HashMap::new()],
            expected_return_type,
            declared_libraries: std::collections::HashSet::new(),
            struct_decls: HashMap::new(),
            enum_decls: HashMap::new(),
            resolved_fns: HashMap::new(),
            struct_templates: HashMap::new(),
            enum_templates: HashMap::new(),
            function_templates: HashMap::new(),
            monomorphized_functions: HashMap::new(),
            new_monomorphized_items: Vec::new(),
            stack_backed_vars: std::collections::HashSet::new(),
            monomorphization_depth: 0,
        }
    }

    pub fn check(&mut self, items: &mut Vec<s::FileItem>) -> bool {
        self.pass1_gather_signatures(items);
        if self.reporter.has_errors() {
            return false;
        }
        self.pass2_check_bodies(items);
        items.append(&mut self.new_monomorphized_items);
        !self.reporter.has_errors()
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn insert_var(&mut self, name: String, ty: TypeID) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }

    fn lookup_var(&self, name: &str) -> Option<TypeID> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(*ty);
            }
        }
        None
    }

    fn mangle_name(name: &str, params: &[TypeID], db: &TypeDatabase) -> String {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        for p in params {
            db.type_to_string(*p).hash(&mut hasher);
        }
        format!("{}_{:x}", name, hasher.finish())
    }

    fn get_const_int(&self, expr: &s::Expr) -> Option<i64> {
        match &expr.kind {
            s::ExprKind::Integer(integer_literal) => Some(integer_literal.value as i64),
            s::ExprKind::Unary(unary_expr) if unary_expr.op == s::Op::Neg => {
                if let s::ExprKind::Integer(integer_literal) = &unary_expr.right.kind {
                    Some(-(integer_literal.value as i64))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn pass1_gather_signatures(&mut self, items: &mut [s::FileItem]) {
        for item in items.iter() {
            if let s::FileItem::Directive(s::Directive::Library { name, .. }) = item {
                self.declared_libraries.insert(name.source().to_string());
            }
        }

        // Stage 1.1: Register placeholders for all structs and enums
        for item in items.iter() {
            match item {
                s::FileItem::Struct(decl) => {
                    let name = decl.name.source().to_string();
                    if decl.generic_params.is_some() {
                        self.struct_templates.insert(name.clone(), decl.clone());
                        self.type_db
                            .insert_named_type(name.clone(), Type::Struct { name, fields: None });
                        continue;
                    }
                    self.struct_decls.insert(name.clone(), decl.clone());
                    if self.type_db.lookup_by_name(&name).is_none() {
                        self.type_db
                            .insert_named_type(name.clone(), Type::Struct { name, fields: None });
                    }
                }
                s::FileItem::Enum(decl) => {
                    let name = decl.name.source().to_string();
                    if decl.generic_params.is_some() {
                        self.enum_templates.insert(name.clone(), decl.clone());
                        self.type_db.insert_named_type(
                            name.clone(),
                            Type::Enum {
                                name: name.clone(),
                                repr: self.type_db.int(),
                                variants: Vec::new(),
                            },
                        );
                        continue;
                    }
                    self.enum_decls.insert(name.clone(), decl.clone());
                    if self.type_db.lookup_by_name(&name).is_none() {
                        let repr = decl
                            .inner_type
                            .as_ref()
                            .map(|t| map_type(t, &mut self.type_db))
                            .unwrap_or(self.type_db.int());
                        self.type_db.insert_named_type(
                            name.clone(),
                            Type::Enum {
                                name,
                                repr,
                                variants: Vec::new(),
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        // Stage 1.2: Resolve field/variant details for structs and enums
        for item in items.iter() {
            match item {
                s::FileItem::Struct(decl) => {
                    if decl.generic_params.is_some() {
                        continue;
                    }
                    let name = decl.name.source().to_string();
                    let struct_id = self.type_db.lookup_by_name(&name).unwrap();
                    let mut fields = Vec::new();
                    for field in &decl.fields {
                        let field_ty = map_type(&field.typ, &mut self.type_db);
                        fields.push(StructField {
                            name: field.name.source().to_string(),
                            ty: field_ty,
                        });
                    }
                    if let Err(e) = self.type_db.set_struct_fields(struct_id, fields) {
                        self.reporter.report(decl.name.loc, e);
                    }
                }
                s::FileItem::Enum(decl) => {
                    if decl.generic_params.is_some() {
                        continue;
                    }
                    let name = decl.name.source().to_string();
                    let enum_id = self.type_db.lookup_by_name(&name).unwrap();
                    let mut variants = HashSet::new();
                    let mut counter = 0;
                    for variant in &decl.variants {
                        match variant {
                            s::EnumVariant::Name(tok) => {
                                let loc = tok.loc;
                                if !variants.insert(EnumVariant {
                                    name: tok.source().to_string(),
                                    default_value: counter,
                                    payload: None,
                                }) {
                                    self.reporter.report(loc, "Duplicated enum variant");
                                }
                            }
                            s::EnumVariant::DefaultValue(tok, lit) => {
                                let loc = tok.loc;
                                if !variants.insert(EnumVariant {
                                    name: tok.source().to_string(),
                                    default_value: {
                                        counter = lit.value;
                                        counter
                                    },
                                    payload: None,
                                }) {
                                    self.reporter
                                        .report(loc, "Duplicated enum variant or variant value");
                                }
                            }
                            s::EnumVariant::TuplePayload(tok, types) => {
                                let loc = tok.loc;
                                let elem_tys: Vec<TypeID> = types
                                    .iter()
                                    .map(|t| map_type(t, &mut self.type_db))
                                    .collect();
                                let payload_ty = self.type_db.tuple(elem_tys);
                                if !variants.insert(EnumVariant {
                                    name: tok.source().to_string(),
                                    default_value: counter,
                                    payload: Some(payload_ty),
                                }) {
                                    self.reporter.report(loc, "Duplicated enum variant");
                                }
                            }
                            s::EnumVariant::StructPayload(tok, fields) => {
                                let loc = tok.loc;
                                let struct_name = format!("{}_{}", name, tok.source());
                                let struct_fields: Vec<StructField> = fields
                                    .iter()
                                    .map(|f| StructField {
                                        name: f.name.source().to_string(),
                                        ty: map_type(&f.typ, &mut self.type_db),
                                    })
                                    .collect();
                                let payload_ty = self.type_db.insert_named_type(
                                    struct_name.clone(),
                                    Type::Struct {
                                        name: struct_name,
                                        fields: Some(struct_fields),
                                    },
                                );
                                if !variants.insert(EnumVariant {
                                    name: tok.source().to_string(),
                                    default_value: counter,
                                    payload: Some(payload_ty),
                                }) {
                                    self.reporter.report(loc, "Duplicated enum variant");
                                }
                            }
                        }
                        counter += 1;
                    }
                    if let Err(e) = self
                        .type_db
                        .set_enum_variants(enum_id, variants.into_iter().collect())
                    {
                        self.reporter.report(decl.name.loc, e);
                    }
                }
                _ => {}
            }
        }

        // Stage 1.2.5: Resolve type declarations (aliases and distinct types)
        for item in items.iter() {
            if let s::FileItem::TypeDecl(td) = item {
                let name = td.name.source().to_string();
                let base_id = map_type(&td.base_type, &mut self.type_db);
                if let Some(existing_id) = self.type_db.lookup_by_name(&name) {
                    if td.is_distinct {
                        if let Some(t) = self.type_db.types.get_mut(&existing_id) {
                            *t = Type::Distinct {
                                name: name.clone(),
                                base: base_id,
                            };
                        }
                    } else {
                        self.type_db.aliases.insert(existing_id, base_id);
                    }
                } else {
                    if td.is_distinct {
                        self.type_db.insert_named_type(
                            name.clone(),
                            Type::Distinct {
                                name: name.clone(),
                                base: base_id,
                            },
                        );
                    } else {
                        self.type_db.names.insert(name.clone(), base_id);
                    }
                }
            }
        }

        // Stage 1.3: Gather function signatures
        for item in items.iter_mut() {
            if let s::FileItem::FunctionDecl(decl) = item {
                let name = decl.signature.name.source().to_string();
                if decl.generic_params.is_some() {
                    // if self.function_templates.insert(name, vec![decl.clone()]).is_some() {
                    //     self.reporter
                    //         .report(decl.signature.name.loc, "overload of templates");
                    // }
                    self.function_templates
                        .entry(name)
                        .or_default()
                        .push(decl.clone());
                    continue;
                }
                let is_main = name == "main";
                if is_main && !decl.signature.parameters.is_empty() {
                    self.reporter
                        .report(decl.signature.name.loc, "main function take 0 arguments");
                }
                let mut params = Vec::new();
                for p in &decl.signature.parameters {
                    params.push(map_type(&p.typ, &mut self.type_db));
                }
                let return_type = decl
                    .signature
                    .return_type
                    .as_ref()
                    .map(|t| map_type(t, &mut self.type_db))
                    .unwrap_or_else(|| self.type_db.void());
                if is_main && return_type != self.type_db.void() {
                    self.reporter
                        .report(decl.signature.name.loc, "main function must return `void`");
                }

                let mut is_foreign = false;
                let mut foreign_lib = None;
                let mut link_name = None;
                for directive in &decl.directives {
                    match directive {
                        s::FunctionDirective::Foreign(loc, lib_token) => {
                            is_foreign = true;
                            foreign_lib = Some((loc, lib_token.source()));
                        }
                        s::FunctionDirective::LinkName(name) => {
                            link_name = Some(name.source().trim_matches('"').to_string());
                        }
                        s::FunctionDirective::Private => {}
                    }
                }

                if let Some((loc, lib_name)) = foreign_lib
                    && !self.declared_libraries.contains(lib_name)
                {
                    self.reporter.report(
                        *loc,
                        format!(
                            "Library '{}' not declared/imported for foreign function",
                            lib_name
                        ),
                    );
                }

                if is_foreign && decl.body.is_some() {
                    self.reporter.report(
                        decl.signature.name.loc,
                        "foreign functions cannot have a body",
                    );
                }
                let mangled_name = if is_main {
                    name.clone()
                } else if let Some(link_name) = link_name
                    && is_foreign
                {
                    link_name.clone()
                } else {
                    Self::mangle_name(&name, &params, &self.type_db)
                };

                decl.resolved_name = Some(mangled_name.clone());
                self.resolved_fns.insert(mangled_name.clone(), decl.clone());

                let sig = FunctionSignature {
                    name: name.clone(),
                    params,
                    return_type,
                    mangled_name,
                    has_va_args: decl.signature.va_args,
                };

                if self.symtab.add(sig, &self.type_db).is_err() {
                    let err = SemanticError::FunctionAlreadyDefined {
                        loc: decl.signature.name.loc,
                        name,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }
            }
        }
    }

    fn pass2_check_bodies(&mut self, items: &mut [s::FileItem]) {
        for item in items.iter_mut() {
            match item {
                s::FileItem::FunctionDecl(decl) => {
                    if decl.generic_params.is_some() {
                        continue;
                    }
                    if let Some(body) = &mut decl.body {
                        self.stack_backed_vars.clear();
                        self.expected_return_type = decl
                            .signature
                            .return_type
                            .as_ref()
                            .map(|t| map_type(t, &mut self.type_db))
                            .unwrap_or_else(|| self.type_db.void());
                        self.push_scope();
                        for param in &decl.signature.parameters {
                            let ty = map_type(&param.typ, &mut self.type_db);
                            self.insert_var(param.name.source().to_string(), ty);
                        }
                        self.check_block(body);
                        self.pop_scope();
                    }
                }
                // s::FileItem::Directive(s::Directive::Run(block)) => {
                //     self.expected_return_type = self.type_db.void();
                //     self.check_block(block);
                // }
                _ => {}
            }
        }
    }

    fn check_block(&mut self, block: &mut s::BlockStmt) {
        self.push_scope();
        for stmt in &mut block.stmts {
            self.check_stmt(stmt);
        }
        self.pop_scope();
    }

    fn check_stmt(&mut self, stmt: &mut s::Stmt) {
        match stmt {
            s::Stmt::ExprStmt(expr) => {
                self.infer_expr(expr);
            }
            s::Stmt::VarDecl(decl) => {
                let expr_ty = self.infer_expr(&mut decl.expr);
                let declared_ty = decl
                    .var_type
                    .as_ref()
                    .map(|t| map_type(t, &mut self.type_db));

                if decl.names.len() == 1 {
                    let name = &decl.names[0];
                    let final_ty = if let Some(decl_ty) = declared_ty {
                        if expr_ty != self.type_db.void()
                            && self.type_db.unify(decl_ty, expr_ty).is_err()
                        {
                            if let Some(decl_ty) = self.coerce(expr_ty, decl_ty) {
                                decl.expr.resolved_type = Some(decl_ty);
                                let actual_canon = self.type_db.resolve(expr_ty);
                                let expected_canon = self.type_db.resolve(decl_ty);
                                if matches!(self.type_db.get_type(expected_canon), Type::Slice(..))
                                    && matches!(self.type_db.get_type(actual_canon), Type::Array(..))
                                {
                                    if self.is_local_array_var(&decl.expr) {
                                        self.reporter.report(
                                            name.loc,
                                            "Implicit coercion of standard local arrays to slices is only allowed for function arguments".to_string()
                                        );
                                    }
                                }
                            } else {
                                let expected_str = self.type_db.type_to_string(decl_ty);
                                let found_str = self.type_db.type_to_string(expr_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: name.loc,
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                        };
                        decl_ty
                    } else if expr_ty == self.type_db.void() {
                        self.reporter
                            .report(name.loc, "Cannot infer type from void expression");
                        self.type_db.void()
                    } else {
                        expr_ty
                    };
                    self.insert_var(name.source().to_string(), final_ty);
                    let is_array = matches!(self.type_db.get_type(self.type_db.resolve(final_ty)), Type::Array(..));
                    let is_stack_slice = matches!(self.type_db.get_type(self.type_db.resolve(final_ty)), Type::Slice(..))
                        && self.is_expr_stack_backed(&decl.expr);
                    if is_array || is_stack_slice {
                        self.stack_backed_vars.insert(name.source().to_string());
                    }
                } else {
                    let target_ty = if let Some(decl_ty) = declared_ty {
                        if expr_ty != self.type_db.void()
                            && self.type_db.unify(decl_ty, expr_ty).is_err()
                        {
                            let expected_str = self.type_db.type_to_string(decl_ty);
                            let found_str = self.type_db.type_to_string(expr_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: decl.names[0].loc,
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                        decl_ty
                    } else {
                        expr_ty
                    };

                    let resolved_target = self.resolve(target_ty);
                    match self.type_db.get_type(resolved_target).clone() {
                        Type::Tuple(elements) => {
                            if elements.len() != decl.names.len() {
                                self.reporter.report(
                                    decl.names[0].loc,
                                    format!(
                                        "Cannot destructure tuple of size {} into {} variables",
                                        elements.len(),
                                        decl.names.len()
                                    ),
                                );
                            } else {
                                for (i, name) in decl.names.iter().enumerate() {
                                    let elem_ty = elements[i];
                                    self.insert_var(name.source().to_string(), elem_ty);
                                    let is_array = matches!(self.type_db.get_type(self.type_db.resolve(elem_ty)), Type::Array(..));
                                    if is_array {
                                        self.stack_backed_vars.insert(name.source().to_string());
                                    }
                                }
                            }
                        }
                        _ => {
                            self.reporter.report(
                                decl.names[0].loc,
                                format!(
                                    "Cannot destructure non-tuple type `{}`",
                                    self.type_db.type_to_string(resolved_target)
                                ),
                            );
                        }
                    }
                }
            }
            s::Stmt::Return(loc, expr_opt) => {
                let ty = if let Some(expr) = expr_opt {
                    let inferred = self.infer_expr(expr);
                    let canonical_ret = self.type_db.resolve(self.expected_return_type);
                    if matches!(self.type_db.get_type(canonical_ret), Type::Slice(..)) {
                        if self.is_expr_stack_backed(expr) {
                            self.reporter.report(
                                *loc,
                                "Returning a slice derived from a local, stack-allocated array is unsafe".to_string()
                            );
                        }
                    }
                    inferred
                } else {
                    self.type_db.void()
                };
                if self.type_db.unify(self.expected_return_type, ty).is_err() {
                    let mut coerced = false;
                    if let Some(expr) = expr_opt {
                        if let Some(coerced_ty) = self.coerce(ty, self.expected_return_type) {
                            expr.resolved_type = Some(coerced_ty);
                            coerced = true;
                        }
                    }
                    if !coerced {
                        let expected_str = self.type_db.type_to_string(self.expected_return_type);
                        let found_str = self.type_db.type_to_string(ty);
                        let err = SemanticError::TypeMismatch {
                            loc: *loc,
                            expected: expected_str,
                            found: found_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }
                }
            }
            s::Stmt::Block(block) => self.check_block(block),
            s::Stmt::IfStmt(s::IfStmt::If { cond, true_body }) => {
                let cond_ty = self.infer_expr(cond);
                let bool_ty = self.type_db.bool();
                if self.type_db.unify(bool_ty, cond_ty).is_err() {
                    let expected_str = self.type_db.type_to_string(bool_ty);
                    let found_str = self.type_db.type_to_string(cond_ty);
                    let err = SemanticError::TypeMismatch {
                        loc: cond.loc(),
                        expected: expected_str,
                        found: found_str,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }
                self.check_block(true_body);
            }
            s::Stmt::IfStmt(s::IfStmt::IfElse {
                cond,
                true_body,
                false_body,
            }) => {
                let cond_ty = self.infer_expr(cond);
                let bool_ty = self.type_db.bool();
                if self.type_db.unify(bool_ty, cond_ty).is_err() {
                    let expected_str = self.type_db.type_to_string(bool_ty);
                    let found_str = self.type_db.type_to_string(cond_ty);
                    let err = SemanticError::TypeMismatch {
                        loc: cond.loc(),
                        expected: expected_str,
                        found: found_str,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }
                self.check_block(true_body);
                self.check_block(false_body);
            }
            s::Stmt::Call(call) => {
                let mut dummy = s::Expr::new(s::ExprKind::Call(call.clone()));
                self.infer_expr(&mut dummy);
                if let s::ExprKind::Call(resolved_call) = dummy.kind {
                    *call = resolved_call;
                }
            }
            s::Stmt::ForStmt(s::ForStmt::ForLoop(block)) => self.check_block(block),
            s::Stmt::ForStmt(s::ForStmt::ForCond { cond, body }) => {
                let cond_ty = self.infer_expr(cond);
                let bool_ty = self.type_db.bool();
                if self.type_db.unify(bool_ty, cond_ty).is_err() {
                    let expected_str = self.type_db.type_to_string(bool_ty);
                    let found_str = self.type_db.type_to_string(cond_ty);
                    let err = SemanticError::TypeMismatch {
                        loc: cond.loc(),
                        expected: expected_str,
                        found: found_str,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }
                self.check_block(body);
            }
            s::Stmt::Break(_) | s::Stmt::Continue(_) => {}
            s::Stmt::Defer(_, inner) => {
                self.check_stmt(inner);
            }
            s::Stmt::Switch(switch_stmt) => {
                let cond_ty = self.infer_expr(&mut switch_stmt.cond);
                for branch in &mut switch_stmt.branches {
                    let mut pattern_vars = Vec::new();
                    let pattern_ty = match &mut branch.pattern.kind {
                        s::ExprKind::Call(call) => {
                            if let s::ExprKind::Member(mem) = &mut call.callee.kind {
                                let base_ty = self.infer_expr(&mut mem.object);
                                let canonical_base = self.resolve(base_ty);
                                if let Type::Enum {
                                    name: _enum_name,
                                    variants,
                                    ..
                                } = self.type_db.get_type(canonical_base).clone()
                                {
                                    let prop_name = mem.property.source();
                                    if let Some(variant) =
                                        variants.iter().find(|v| v.name == prop_name)
                                    {
                                        if let Some(payload_ty) = variant.payload {
                                            let canonical_payload =
                                                self.type_db.resolve(payload_ty);
                                            match self.type_db.get_type(canonical_payload).clone() {
                                                Type::Tuple(elements) => {
                                                    if call.arguments.len() != elements.len() {
                                                        self.reporter.report(call.loc, format!("Expected {} bindings for variant payload, found {}", elements.len(), call.arguments.len()));
                                                    } else {
                                                        for (i, arg) in
                                                            call.arguments.iter().enumerate()
                                                        {
                                                            if let s::ExprKind::Identifier(ident) =
                                                                &arg.kind
                                                            {
                                                                pattern_vars.push((
                                                                    ident.source().to_string(),
                                                                    elements[i],
                                                                ));
                                                            } else {
                                                                self.reporter.report(arg.loc(), "Pattern variables must be simple identifiers".to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                                Type::Struct {
                                                    fields: Some(fields),
                                                    ..
                                                } => {
                                                    if call.arguments.len() != fields.len() {
                                                        self.reporter.report(call.loc, format!("Expected {} bindings for variant payload, found {}", fields.len(), call.arguments.len()));
                                                    } else {
                                                        for (i, arg) in
                                                            call.arguments.iter().enumerate()
                                                        {
                                                            if let s::ExprKind::Identifier(ident) =
                                                                &arg.kind
                                                            {
                                                                pattern_vars.push((
                                                                    ident.source().to_string(),
                                                                    fields[i].ty,
                                                                ));
                                                            } else {
                                                                self.reporter.report(arg.loc(), "Pattern variables must be simple identifiers".to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            self.reporter.report(
                                                call.loc,
                                                format!(
                                                    "Variant '{}' does not have a payload",
                                                    prop_name
                                                ),
                                            );
                                        }
                                        canonical_base
                                    } else {
                                        self.type_db.void()
                                    }
                                } else {
                                    self.type_db.void()
                                }
                            } else {
                                self.infer_expr(&mut branch.pattern)
                            }
                        }
                        _ => self.infer_expr(&mut branch.pattern),
                    };

                    if self.type_db.unify(cond_ty, pattern_ty).is_err() {
                        let cond_str = self.type_db.type_to_string(cond_ty);
                        let pattern_str = self.type_db.type_to_string(pattern_ty);
                        let err = SemanticError::TypeMismatch {
                            loc: branch.pattern.loc(),
                            expected: cond_str,
                            found: pattern_str,
                        };
                        self.reporter.report(err.loc(), err.to_string());
                    }

                    self.scopes.push(HashMap::new());
                    for (var_name, var_ty) in pattern_vars {
                        self.scopes.last_mut().unwrap().insert(var_name, var_ty);
                    }
                    self.check_stmt(&mut branch.body);
                    self.scopes.pop();
                }
                if let Some(ref mut default_stmt) = switch_stmt.default {
                    self.check_stmt(default_stmt);
                }
            }
            s::Stmt::ForEach(fe) => {
                let iter_ty = self.infer_expr(&mut fe.iter_expr);
                let resolved_iter = self.resolve(iter_ty);
                let elem_ty = if resolved_iter == self.type_db.string() {
                    self.type_db.u8()
                } else {
                    match self.type_db.get_type(resolved_iter).clone() {
                        Type::Array(elem, _) => elem,
                        Type::Slice(elem) => elem,
                        Type::Pointer(inner) => {
                            let resolved_inner = self.resolve(inner);
                            if resolved_inner == self.type_db.string() {
                                self.type_db.u8()
                            } else {
                                match self.type_db.get_type(resolved_inner).clone() {
                                    Type::Array(elem, _) => elem,
                                    Type::Slice(elem) => elem,
                                    _ => {
                                        let iter_str = self.type_db.type_to_string(iter_ty);
                                        let err = format!("Cannot iterate over type {}", iter_str);
                                        self.reporter.report(fe.iter_expr.loc(), err);
                                        self.type_db.void()
                                    }
                                }
                            }
                        }
                        _ => {
                            let iter_str = self.type_db.type_to_string(iter_ty);
                            let err = format!("Cannot iterate over type {}", iter_str);
                            self.reporter.report(fe.iter_expr.loc(), err);
                            self.type_db.void()
                        }
                    }
                };

                self.push_scope();
                self.insert_var(fe.var_name.source().to_string(), elem_ty);
                self.check_block(&mut fe.body);
                self.pop_scope();
            }
        }
    }

    fn coerce(&mut self, expr_ty: TypeID, decl_ty: TypeID) -> Option<TypeID> {
        let decl_resolved = self.resolve(decl_ty);
        let expr_resolved = self.resolve(expr_ty);

        if expr_resolved == self.type_db.noreturn() {
            return Some(decl_ty);
        }

        let decl_type = self.type_db.get_type(decl_resolved).clone();
        let expr_type = self.type_db.get_type(expr_resolved).clone();

        match (decl_type, expr_type) {
            (Type::Slice(expected_elem), Type::Array(actual_elem, _))
                if self.type_db.unify(expected_elem, actual_elem).is_ok() =>
            {
                Some(decl_ty)
            }
            _ => None,
        }
    }

    fn infer_expr(&mut self, expr: &mut s::Expr) -> TypeID {
        let ty = match &mut expr.kind {
            s::ExprKind::Integer(_) => self.type_db.int(),
            s::ExprKind::Bool(_) => self.type_db.bool(),
            s::ExprKind::Float(_) => self.type_db.float(),
            s::ExprKind::StringLiteral(_) => self.type_db.string(),
            s::ExprKind::Null(_) => self.type_db.pointer(self.type_db.void()),
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(s), _) => {
                s.resolved_type = Some(self.infer_expr(s));
                self.type_db.any()
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Array(arr), _) => {
                for s in arr.iter_mut() {
                    s.resolved_type = Some(self.infer_expr(s));
                }
                self.type_db.array(self.type_db.any(), arr.len())
            }
            s::ExprKind::Tuple(exprs, _) => {
                let mut elem_tys = Vec::new();
                for expr in exprs {
                    elem_tys.push(self.infer_expr(expr));
                }
                self.type_db.tuple(elem_tys)
            }
            s::ExprKind::Identifier(ident) => {
                if let Some(ty) = self.lookup_var(ident.source()) {
                    ty
                } else if let Some(overloads) = self.symtab.functions.get(ident.source()) {
                    if overloads.len() == 1 {
                        let sig = &overloads[0];
                        self.type_db.fn_pointer(sig.params.clone(), sig.return_type)
                    } else {
                        let err = SemanticError::AmbiguousFunctionCall {
                            loc: ident.loc,
                            name: ident.source().to_string(),
                        };
                        self.reporter.report(err.loc(), err.to_string());
                        self.type_db.void()
                    }
                } else if let Some(ty) = self.type_db.lookup_by_name(ident.source()) {
                    ty
                } else {
                    let err = SemanticError::UndefinedVariable {
                        loc: ident.loc,
                        name: ident.source().to_string(),
                    };
                    self.reporter.report(err.loc(), err.to_string());
                    self.type_db.void()
                }
            }
            s::ExprKind::Binary(bin) => {
                let left_ty = self.infer_expr(&mut bin.left);
                let right_ty = self.infer_expr(&mut bin.right);

                let left_canon = self.resolve(left_ty);
                let right_canon = self.resolve(right_ty);
                let left_is_ptr = matches!(self.type_db.get_type(left_canon), Type::Pointer(_));
                let right_is_ptr = matches!(self.type_db.get_type(right_canon), Type::Pointer(_));
                let left_is_int = left_canon == self.type_db.int() || left_canon == self.type_db.u8();
                let right_is_int = right_canon == self.type_db.int() || right_canon == self.type_db.u8();

                let is_ptr_arithmetic = match bin.op {
                    s::Op::Add => {
                        (left_is_ptr && right_is_int) || (left_is_int && right_is_ptr)
                    }
                    s::Op::Sub => {
                        (left_is_ptr && right_is_int) || (left_is_ptr && right_is_ptr)
                    }
                    _ => false,
                };

                if !is_ptr_arithmetic
                    && left_ty != self.type_db.void()
                    && right_ty != self.type_db.void()
                    && self.type_db.unify(left_ty, right_ty).is_err()
                {
                    let expected_str = self.type_db.type_to_string(left_ty);
                    let found_str = self.type_db.type_to_string(right_ty);
                    let err = SemanticError::TypeMismatch {
                        loc: bin.right.loc(),
                        expected: expected_str,
                        found: found_str,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }

                if is_ptr_arithmetic {
                    if bin.op == s::Op::Add && right_is_ptr {
                        right_ty
                    } else if bin.op == s::Op::Sub && left_is_ptr && right_is_ptr {
                        self.type_db.int()
                    } else {
                        left_ty
                    }
                } else {
                    match (
                        bin.op,
                        self.type_db.get_type(left_ty),
                        self.type_db.get_type(right_ty),
                    ) {
                        (
                            s::Op::Eq
                            | s::Op::NotEq
                            | s::Op::Lt
                            | s::Op::LtEq
                            | s::Op::Gt
                            | s::Op::GtEq,
                            Type::Primitive(_) | Type::Enum { .. } | Type::Pointer(_),
                            Type::Primitive(_) | Type::Enum { .. } | Type::Pointer(_),
                        ) => self.type_db.bool(),
                        (_, Type::Primitive(_), Type::Primitive(_)) => left_ty,
                        _ => {
                            if let Some(resolved_sig) =
                                self.try_resolve_signature(bin.op.get_name(), &[left_ty, right_ty])
                            {
                                bin.use_operator_overload = Some(resolved_sig.mangled_name.clone());
                                resolved_sig.return_type
                            } else {
                                let left_str = self.type_db.type_to_string(left_ty);
                                let right_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::NoOverloadFound {
                                    loc: bin.right.loc(),
                                    name: format!("{:?}", bin.op),
                                    expected: format!("({}, {})", left_str, right_str),
                                };
                                self.reporter.report(err.loc(), err.to_string());
                                self.type_db.void()
                            }
                        }
                    }
                }
            }
            s::ExprKind::Call(call) => {
                let mut is_indirect = false;
                let callee_name = match &call.callee.kind {
                    s::ExprKind::GenericInst(base, args, _) => {
                        if let s::ExprKind::Identifier(ident) = &base.kind {
                            let name = ident.source().to_string();
                            if self.lookup_var(&name).is_some() {
                                is_indirect = true;
                                None
                            } else {
                                Some((name, Some(args.clone())))
                            }
                        } else {
                            is_indirect = true;
                            None
                        }
                    }
                    s::ExprKind::Identifier(ident) => {
                        let name = ident.source().to_string();
                        if self.lookup_var(&name).is_some() {
                            is_indirect = true;
                            None
                        } else {
                            Some((name, None))
                        }
                    }
                    _ => {
                        is_indirect = true;
                        None
                    }
                };

                if is_indirect {
                    let callee_ty = self.infer_expr(&mut call.callee);
                    let canonical = self.resolve(callee_ty);
                    if let Type::FnPointer { params, return_type } = self.type_db.get_type(canonical).clone() {
                        let mut arg_types = Vec::new();
                        for arg in &mut call.arguments {
                            arg_types.push(self.infer_expr(arg));
                        }
                        if arg_types.len() != params.len() {
                            self.reporter.report(call.loc, format!("Expected {} arguments for indirect call, found {}", params.len(), arg_types.len()));
                        } else {
                            for (p, arg) in params.iter().zip(arg_types.iter()) {
                                if !self.types_match(*p, *arg) {
                                    let expected_str = self.type_db.type_to_string(*p);
                                    let found_str = self.type_db.type_to_string(*arg);
                                    self.reporter.report(call.loc, format!("Type mismatch: expected {}, found {}", expected_str, found_str));
                                }
                            }
                        }
                        return return_type;
                    } else {
                        self.reporter.report(call.loc, "Expected function pointer type for indirect call");
                        return self.type_db.void();
                    }
                }

                let (callee_name, explicit_type_args) = match callee_name {
                    Some((name, args)) => (name, args),
                    None => unreachable!(),
                };

                let mut arg_types = Vec::new();
                for arg in &mut call.arguments {
                    arg_types.push(self.infer_expr(arg));
                }

                let mut concrete_overload_exists = false;
                if explicit_type_args.is_none() && let Some(overloads) = self.symtab.functions.get(&callee_name) {
                    for sig in overloads {
                        if arg_types.len() <= sig.params.len() {
                            let mut matches = true;
                            for (p, arg) in sig.params.iter().zip(arg_types.iter()) {
                                if !self.types_match(*p, *arg) {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches && let Some(decl) = self.resolved_fns.get(&sig.mangled_name)
                            {
                                let mut all_have_defaults = true;
                                for i in arg_types.len()..sig.params.len() {
                                    if decl.signature.parameters[i].value.is_none() {
                                        all_have_defaults = false;
                                        break;
                                    }
                                }
                                if all_have_defaults {
                                    concrete_overload_exists = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                let mut callee_name = callee_name;
                if !concrete_overload_exists && self.function_templates.contains_key(&callee_name) {
                    let templates = self.function_templates.get(&callee_name).unwrap();
                    let mut matched_template = None;
                    let mut resolved_type_args = Vec::new();

                    for decl in templates {
                        let generic_params = decl.generic_params.as_ref().unwrap();

                        // Check parameter counts
                        if arg_types.len() > decl.signature.parameters.len() {
                            continue;
                        }
                        if let Some(ref explicit_args) = explicit_type_args
                            && explicit_args.len() != generic_params.len()
                        {
                            continue;
                        }

                        // Try matching/unification
                        let mut temp_db = self.type_db.clone();
                        let mut infer_map = HashMap::new();

                        if let Some(ref explicit_args) = explicit_type_args {
                            for (i, param) in generic_params.iter().enumerate() {
                                let ty_id = map_type(&explicit_args[i], &mut temp_db);
                                infer_map.insert(param.source().to_string(), ty_id);
                            }
                        } else {
                            for param in generic_params {
                                let infer_var = temp_db.new_inference_var();
                                infer_map.insert(param.source().to_string(), infer_var);
                            }
                        }

                        let mut matches = true;
                        for (i, p) in decl.signature.parameters.iter().enumerate() {
                            if i < arg_types.len() {
                                let expected_param_ty =
                                    map_type_with_substs(&p.typ, &infer_map, &mut temp_db);
                                let expected_canon = temp_db.resolve(expected_param_ty);
                                let arg_canon = temp_db.resolve(arg_types[i]);
                                let unified = match (
                                    temp_db.get_type(expected_canon).clone(),
                                    temp_db.get_type(arg_canon).clone(),
                                ) {
                                    (Type::Slice(slice_inner), Type::Array(array_inner, _)) => {
                                        temp_db.unify(slice_inner, array_inner)
                                    }
                                    _ => temp_db.unify(expected_param_ty, arg_types[i]),
                                };
                                if unified.is_err() {
                                    matches = false;
                                    break;
                                }
                            }
                        }

                        if matches {
                            // Check default values for remaining parameters
                            for i in arg_types.len()..decl.signature.parameters.len() {
                                if decl.signature.parameters[i].value.is_none() {
                                    matches = false;
                                    break;
                                }
                            }
                        }

                        if matches {
                            let mut type_args = Vec::new();
                            for param in generic_params {
                                let resolved_var =
                                    temp_db.resolve(*infer_map.get(param.source()).unwrap());
                                type_args.push(type_id_to_ast(resolved_var, &temp_db));
                            }
                            matched_template = Some(decl.clone());
                            resolved_type_args = type_args;
                            break;
                        }
                    }

                    if let Some(decl) = matched_template {
                        let concrete_fn_name =
                            self.monomorphize_function(&callee_name, &decl, &resolved_type_args);
                        call.callee.kind = s::ExprKind::Identifier(Token::new(
                            TokenKind::Identifier,
                            call.callee.loc(),
                            lex_just_parse::lexer::TokenSource::from(concrete_fn_name.as_str()),
                        ));
                        callee_name = concrete_fn_name;
                    }
                }

                let resolved_sig = self.try_resolve_signature(&callee_name, &arg_types);

                if let Some(sig) = resolved_sig {
                    call.resolved_name = Some(sig.mangled_name.clone());
                    for (i, arg) in call.arguments.iter_mut().enumerate() {
                        let arg_ty = arg.resolved_type.unwrap();
                        if sig.has_va_args && i >= sig.params.len() {
                            let resolved_arg = self.resolve(arg_ty);
                            arg.resolved_type = Some(resolved_arg);
                            continue;
                        }
                        let param_ty = sig.params[i];
                        let _ = self.type_db.unify(param_ty, arg_ty);
                        let resolved_param = self.resolve(param_ty);
                        let resolved_arg = self.resolve(arg_ty);
                        if resolved_param != resolved_arg {
                            if self.types_match(resolved_param, resolved_arg) {
                                arg.resolved_type = Some(resolved_param);
                            }
                        } else {
                            arg.resolved_type = Some(resolved_param);
                        }
                    }

                    let mut defaults_to_inject = Vec::new();
                    {
                        let decl = self.resolved_fns.get(&sig.mangled_name).unwrap();
                        for i in arg_types.len()..sig.params.len() {
                            let default_expr = decl.signature.parameters[i].value.as_ref().unwrap();
                            defaults_to_inject.push((
                                i,
                                decl.signature.parameters[i].name.loc,
                                default_expr.clone(),
                            ));
                        }
                    }

                    for (i, param_loc, mut cloned_expr) in defaults_to_inject {
                        let default_ty = self.infer_expr(&mut cloned_expr);
                        if self.type_db.unify(sig.params[i], default_ty).is_err() {
                            let expected_str = self.type_db.type_to_string(sig.params[i]);
                            let found_str = self.type_db.type_to_string(default_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: param_loc,
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                        call.arguments.push(cloned_expr);
                    }
                    sig.return_type
                } else {
                    use std::fmt::Write;
                    let mut expected = String::from("(");
                    for (i, t) in arg_types.iter().enumerate() {
                        if i > 0 {
                            expected.push(',');
                        }
                        let _ = write!(&mut expected, "{}", self.type_db.type_to_string(*t));
                    }
                    expected.push(')');
                    if let Some(overloads) = self.symtab.functions.get(&callee_name) {
                        expected.push_str(". Available overloads: ");
                        for (idx, o) in overloads.iter().enumerate() {
                            if idx > 0 {
                                expected.push_str(" | ");
                            }
                            expected.push('(');
                            for (j, p) in o.params.iter().enumerate() {
                                if j > 0 {
                                    expected.push(',');
                                }
                                expected.push_str(&self.type_db.type_to_string(*p));
                            }
                            expected.push(')');
                        }
                    }
                    let err = SemanticError::NoOverloadFound {
                        loc: call.loc,
                        name: callee_name,
                        expected,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                    self.type_db.void()
                }
            }
            s::ExprKind::Assign(assign) => {
                let right_ty = self.infer_expr(&mut assign.right);
                if let s::ExprKind::Identifier(ident) = &assign.left.kind {
                    if let Some(left_ty) = self.lookup_var(ident.source()) {
                        if self.type_db.unify(left_ty, right_ty).is_err() {
                            if let Some(left_ty) = self.coerce(left_ty, right_ty) {
                                assign.right.resolved_type = Some(left_ty);
                                let actual_canon = self.type_db.resolve(right_ty);
                                let expected_canon = self.type_db.resolve(left_ty);
                                if matches!(self.type_db.get_type(expected_canon), Type::Slice(..))
                                    && matches!(self.type_db.get_type(actual_canon), Type::Array(..))
                                {
                                    if self.is_local_array_var(&assign.right) {
                                        self.reporter.report(
                                            assign.right.loc(),
                                            "Implicit coercion of standard local arrays to slices is only allowed for function arguments".to_string()
                                        );
                                    }
                                }
                            } else {
                                let expected_str = self.type_db.type_to_string(left_ty);
                                let found_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: assign.right.loc(),
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                        }
                        let canon_left = self.type_db.resolve(left_ty);
                        if matches!(self.type_db.get_type(canon_left), Type::Slice(..)) {
                            if self.is_expr_stack_backed(&assign.right) {
                                self.stack_backed_vars.insert(ident.source().to_string());
                            } else {
                                self.stack_backed_vars.remove(ident.source());
                            }
                        }
                        right_ty
                    } else {
                        let err = SemanticError::UndefinedVariable {
                            loc: ident.loc,
                            name: ident.source().to_string(),
                        };
                        self.reporter.report(err.loc(), err.to_string());
                        self.type_db.void()
                    }
                } else if let s::ExprKind::Member(mem) = &assign.left.kind {
                    let mut dummy_left = s::Expr::new(s::ExprKind::Member(mem.clone()));
                    let left_ty = self.infer_expr(&mut dummy_left);

                    let mut dummy_object = mem.object.clone();
                    let base_ty = self.infer_expr(&mut dummy_object);
                    let mut canonical_base = self.resolve(base_ty);
                    while let Type::Pointer(inner) = self.type_db.get_type(canonical_base).clone() {
                        canonical_base = self.resolve(inner);
                    }
                    if let Type::Enum { .. } = self.type_db.get_type(canonical_base) {
                        self.reporter.report(
                            assign.left.loc(),
                            format!("Cannot assign to enum variant '{}'", mem.property.source()),
                        );
                    }

                    if self.type_db.unify(left_ty, right_ty).is_err() {
                        if let Some(left_ty) = self.coerce(left_ty, right_ty) {
                            assign.right.resolved_type = Some(left_ty);
                        } else {
                            let expected_str = self.type_db.type_to_string(left_ty);
                            let found_str = self.type_db.type_to_string(right_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: assign.right.loc(),
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                    }
                    right_ty
                } else if let s::ExprKind::Index(idx) = &assign.left.kind {
                    let mut dummy_left = s::Expr::new(s::ExprKind::Index(idx.clone()));
                    let left_ty = self.infer_expr(&mut dummy_left);
                    if self.type_db.unify(left_ty, right_ty).is_err() {
                        if let Some(left_ty) = self.coerce(left_ty, right_ty) {
                            assign.right.resolved_type = Some(left_ty);
                        } else {
                            let expected_str = self.type_db.type_to_string(left_ty);
                            let found_str = self.type_db.type_to_string(right_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: assign.right.loc(),
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                    }
                    right_ty
                } else if let s::ExprKind::Unary(unary) = &assign.left.kind {
                    if unary.op == s::Op::Deref {
                        let mut dummy_left = s::Expr::new(s::ExprKind::Unary(unary.clone()));
                        let left_ty = self.infer_expr(&mut dummy_left);
                        if self.type_db.unify(left_ty, right_ty).is_err() {
                            if let Some(left_ty) = self.coerce(left_ty, right_ty) {
                                assign.right.resolved_type = Some(left_ty);
                            } else {
                                let expected_str = self.type_db.type_to_string(left_ty);
                                let found_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: assign.right.loc(),
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                        }
                        right_ty
                    } else {
                        self.reporter.report(
                            assign.left.loc(),
                            "Complex assignment left-hand side not supported",
                        );
                        self.type_db.void()
                    }
                } else if let s::ExprKind::Tuple(_, loc) = &assign.left.kind {
                    let left_loc = *loc;
                    let targets = match &mut assign.left.kind {
                        s::ExprKind::Tuple(targets, _) => targets,
                        _ => unreachable!(),
                    };
                    let canonical_right = self.resolve(right_ty);
                    match self.type_db.get_type(canonical_right).clone() {
                        Type::Tuple(elements) => {
                            if elements.len() != targets.len() {
                                self.reporter.report(
                                    left_loc,
                                    format!(
                                        "Cannot assign tuple of size {} to {} variables",
                                        elements.len(),
                                        targets.len()
                                    ),
                                );
                            } else {
                                for (i, target) in targets.iter_mut().enumerate() {
                                    let is_lval = matches!(
                                        &target.kind,
                                        s::ExprKind::Identifier(_)
                                            | s::ExprKind::Member(_)
                                            | s::ExprKind::Index(_)
                                            | s::ExprKind::Unary(s::UnaryExpr {
                                                op: s::Op::Deref,
                                                ..
                                            })
                                    );
                                    if !is_lval {
                                        self.reporter.report(
                                            target.loc(),
                                            "Left-hand side of assignment must be a variable or member",
                                        );
                                    }

                                    let target_ty = self.infer_expr(target);
                                    if self.type_db.unify(target_ty, elements[i]).is_err() {
                                        let expected_str = self.type_db.type_to_string(target_ty);
                                        let found_str = self.type_db.type_to_string(elements[i]);
                                        let err = SemanticError::TypeMismatch {
                                            loc: target.loc(),
                                            expected: expected_str,
                                            found: found_str,
                                        };
                                        self.reporter.report(err.loc(), err.to_string());
                                    }
                                }
                            }
                        }
                        _ => {
                            self.reporter.report(
                                left_loc,
                                format!(
                                    "Cannot destructure non-tuple type `{}`",
                                    self.type_db.type_to_string(canonical_right)
                                ),
                            );
                        }
                    }
                    right_ty
                } else {
                    self.reporter.report(
                        assign.left.loc(),
                        "Complex assignment left-hand side not supported",
                    );
                    self.type_db.void()
                }
            }
            s::ExprKind::Unary(unary) => {
                let right_ty = self.infer_expr(&mut unary.right);
                match unary.op {
                    s::Op::Neg => {
                        let canonical_right = self.resolve(right_ty);
                        if canonical_right == self.type_db.float() {
                            canonical_right
                        } else {
                            let int_ty = self.type_db.int();
                            if self.type_db.unify(int_ty, right_ty).is_err() {
                                let expected_str = self.type_db.type_to_string(int_ty);
                                let found_str = self.type_db.type_to_string(right_ty);
                                let err = SemanticError::TypeMismatch {
                                    loc: unary.right.loc(),
                                    expected: expected_str,
                                    found: found_str,
                                };
                                self.reporter.report(err.loc(), err.to_string());
                            }
                            int_ty
                        }
                    }
                    s::Op::Not => {
                        let bool_ty = self.type_db.bool();
                        if self.type_db.unify(bool_ty, right_ty).is_err() {
                            let expected_str = self.type_db.type_to_string(bool_ty);
                            let found_str = self.type_db.type_to_string(right_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: unary.right.loc(),
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                        bool_ty
                    }
                    s::Op::Refer => self.type_db.pointer(right_ty),
                    s::Op::Deref => {
                        let canonical = self.resolve(right_ty);
                        match self.type_db.get_type(canonical).clone() {
                            Type::Pointer(inner) => inner,
                            _ => {
                                let found_str = self.type_db.type_to_string(right_ty);
                                self.reporter.report(
                                    unary.right.loc(),
                                    format!("Cannot dereference non-pointer type: {}", found_str),
                                );
                                self.type_db.void()
                            }
                        }
                    }
                    _ => right_ty,
                }
            }
            s::ExprKind::Cast(target_type, expr, loc) => {
                let right_ty = self.infer_expr(expr);
                let dest_ty = map_type(target_type, &mut self.type_db);
                if right_ty != self.type_db.void() && dest_ty != self.type_db.void() {
                    let underlying_src = self.type_db.get_underlying_type(right_ty);
                    let underlying_dest = self.type_db.get_underlying_type(dest_ty);
                    if underlying_src != underlying_dest {
                        if !self.is_castable_type(right_ty) {
                            let src_str = self.type_db.type_to_string(right_ty);
                            self.reporter.report(
                                *loc,
                                format!("Cannot cast from non-castable type: {}", src_str),
                            );
                        }
                        if !self.is_castable_type(dest_ty) {
                            let dest_str = self.type_db.type_to_string(dest_ty);
                            self.reporter.report(
                                *loc,
                                format!("Cannot cast to non-castable type: {}", dest_str),
                            );
                        }
                    }
                }
                dest_ty
            }
            s::ExprKind::AutoCast(expr, loc) => {
                let right_ty = self.infer_expr(expr);
                if right_ty != self.type_db.void() && !self.is_castable_type(right_ty) {
                    let src_str = self.type_db.type_to_string(right_ty);
                    self.reporter.report(
                        *loc,
                        format!("Cannot auto-cast from non-castable type: {}", src_str),
                    );
                }
                self.type_db.new_inference_var()
            }
            s::ExprKind::GenericInst(base, type_args, loc) => {
                let mut ty = self.type_db.void();
                if let s::ExprKind::Identifier(ident) = &base.kind {
                    let base_name = ident.source();
                    if self.enum_templates.contains_key(base_name) {
                        ty = self.monomorphize_enum(base_name, type_args);
                    } else if self.struct_templates.contains_key(base_name) {
                        ty = self.monomorphize_struct(base_name, type_args);
                    } else {
                        self.reporter
                            .report(*loc, "Generic functions can only be called directly");
                    }
                } else {
                    self.reporter
                        .report(*loc, "Generic functions can only be called directly");
                }
                ty
            }
            s::ExprKind::StructLiteral(lit) => {
                let mut name = lit.name.source().to_string();
                if lit.type_args.is_some() {
                    let type_args = lit.type_args.as_ref().unwrap();
                    let struct_id = self.monomorphize_struct(&name, type_args);
                    let concrete_name = self.type_db.type_to_string(struct_id);
                    lit.name = Token::new(
                        TokenKind::Identifier,
                        lit.name.loc,
                        lex_just_parse::lexer::TokenSource::from(concrete_name.as_str()),
                    );
                    name = concrete_name;
                } else if self.struct_templates.contains_key(&name) {
                    let decl = self.struct_templates.get(&name).cloned().unwrap();
                    let generic_params = decl.generic_params.as_ref().unwrap();
                    let mut arg_ids = Vec::new();
                    for _ in generic_params {
                        arg_ids.push(self.type_db.new_inference_var());
                    }
                    let struct_id = self.monomorphize_struct_with_ids(&name, &arg_ids);
                    let concrete_name = self.type_db.type_to_string(struct_id);
                    lit.name = Token::new(
                        TokenKind::Identifier,
                        lit.name.loc,
                        lex_just_parse::lexer::TokenSource::from(concrete_name.as_str()),
                    );
                    name = concrete_name;
                }
                if let Some(struct_id) = self.type_db.lookup_by_name(&name) {
                    let canonical = self.resolve(struct_id);
                    match self.type_db.get_type(canonical).clone() {
                        Type::Struct {
                            fields: Some(fields),
                            ..
                        } => {
                            let mut init_fields = HashMap::new();
                            for f in &mut lit.fields {
                                let init_ty = self.infer_expr(&mut f.value);
                                init_fields
                                    .insert(f.name.source().to_string(), (init_ty, f.name.loc));
                            }

                            for def_field in &fields {
                                if let Some((init_ty, loc)) = init_fields.remove(&def_field.name) {
                                    if self.type_db.unify(def_field.ty, init_ty).is_err() {
                                        let expected_str =
                                            self.type_db.type_to_string(def_field.ty);
                                        let found_str = self.type_db.type_to_string(init_ty);
                                        let err = SemanticError::TypeMismatch {
                                            loc,
                                            expected: expected_str,
                                            found: found_str,
                                        };
                                        self.reporter.report(err.loc(), err.to_string());
                                    }
                                } else {
                                    let decl_opt = self.struct_decls.get(&name).cloned();
                                    let mut has_default = false;
                                    if let Some(ref decl) = decl_opt
                                        && let Some(field_decl) = decl
                                            .fields
                                            .iter()
                                            .find(|fd| fd.name.source() == def_field.name)
                                        && let Some(ref default_expr) = field_decl.value
                                    {
                                        has_default = true;
                                        let mut cloned_expr = default_expr.clone();
                                        let default_ty = self.infer_expr(&mut cloned_expr);

                                        if self.type_db.unify(def_field.ty, default_ty).is_err() {
                                            let expected_str =
                                                self.type_db.type_to_string(def_field.ty);
                                            let found_str = self.type_db.type_to_string(default_ty);
                                            let err = SemanticError::TypeMismatch {
                                                loc: field_decl.name.loc,
                                                expected: expected_str,
                                                found: found_str,
                                            };
                                            self.reporter.report(err.loc(), err.to_string());
                                        }

                                        lit.fields.push(s::FieldInit {
                                            name: field_decl.name.clone(),
                                            value: cloned_expr,
                                        });
                                    }

                                    if !has_default {
                                        self.reporter.report(
                                            lit.name.loc,
                                            format!(
                                                "Missing field '{}' in struct literal '{}'",
                                                def_field.name, name
                                            ),
                                        );
                                    }
                                }
                            }

                            for (extra_field, (_, loc)) in init_fields {
                                self.reporter.report(
                                    loc,
                                    format!(
                                        "Struct '{}' has no field named '{}'",
                                        name, extra_field
                                    ),
                                );
                            }

                            canonical
                        }
                        _ => {
                            self.reporter.report(
                                lit.name.loc,
                                format!("'{}' is not registered as a struct", name),
                            );
                            self.type_db.void()
                        }
                    }
                } else {
                    self.reporter
                        .report(lit.name.loc, format!("Undefined struct '{}'", name));
                    self.type_db.void()
                }
            }
            s::ExprKind::Array(arr) => {
                if let Some(hint) = &arr.type_hint {
                    let hint_ty = map_type(hint, &mut self.type_db);
                    expr.resolved_type = Some(hint_ty);
                }
                if arr.elements.is_empty() && expr.resolved_type.is_none() {
                    let inner_var = self.type_db.new_inference_var();
                    self.type_db.array(inner_var, 0)
                } else {
                    let (first_ty, skip) = if let Some(resolved_type) = expr.resolved_type {
                        (resolved_type, 0)
                    } else {
                        let first_ty = self.infer_expr(&mut arr.elements[0]);
                        expr.resolved_type = Some(first_ty);
                        (first_ty, 1)
                    };
                    for elem in arr.elements.iter_mut().skip(skip) {
                        let elem_ty = self.infer_expr(elem);
                        if self.type_db.unify(first_ty, elem_ty).is_err() {
                            let expected_str = self.type_db.type_to_string(first_ty);
                            let found_str = self.type_db.type_to_string(elem_ty);
                            let err = SemanticError::TypeMismatch {
                                loc: elem.loc(),
                                expected: expected_str,
                                found: found_str,
                            };
                            self.reporter.report(err.loc(), err.to_string());
                        }
                    }
                    self.type_db.array(first_ty, arr.elements.len())
                }
            }
            s::ExprKind::Index(idx) => {
                let base_ty = self.infer_expr(&mut idx.array);
                let index_ty = self.infer_expr(&mut idx.index);

                let int_ty = self.type_db.int();
                if self.type_db.unify(int_ty, index_ty).is_err() {
                    let expected_str = self.type_db.type_to_string(int_ty);
                    let found_str = self.type_db.type_to_string(index_ty);
                    let err = SemanticError::TypeMismatch {
                        loc: idx.index.loc(),
                        expected: expected_str,
                        found: found_str,
                    };
                    self.reporter.report(err.loc(), err.to_string());
                }

                let canonical_base = self.resolve(base_ty);
                match self.type_db.get_type(canonical_base) {
                    Type::Array(element_ty, len) => {
                        let len = *len;
                        if let Some(idx_val) = self.get_const_int(&idx.index) {
                            if idx_val < 0 || idx_val >= (len as i64) {
                                self.reporter.report(
                                    idx.loc,
                                    format!("Index {} is out of bounds for array of length {}", idx_val, len)
                                );
                            }
                        }
                        *element_ty
                    }
                    Type::Slice(element_ty) => *element_ty,
                    Type::Pointer(element_ty) => *element_ty,
                    _ => {
                        self.reporter.report(
                            idx.loc,
                            format!(
                                "Cannot index into non-array type '{}'",
                                self.type_db.type_to_string(canonical_base)
                            ),
                        );
                        self.type_db.void()
                    }
                }
            }
            s::ExprKind::Member(mem) => {
                let base_ty = self.infer_expr(&mut mem.object);
                let mut canonical_base = self.resolve(base_ty);
                while let Type::Pointer(inner) = self.type_db.get_type(canonical_base).clone() {
                    canonical_base = self.resolve(inner);
                }
                let prop_name = mem.property.source();

                let underlying_base = self.type_db.get_underlying_type(canonical_base);
                match self.type_db.get_type(underlying_base) {
                    Type::Struct {
                        name,
                        fields: Some(fields),
                    } => {
                        if let Some(f) = fields.iter().find(|field| field.name == prop_name) {
                            f.ty
                        } else {
                            self.reporter.report(
                                mem.property.loc,
                                format!("Field '{}' not found in struct '{}'", prop_name, name),
                            );
                            self.type_db.void()
                        }
                    }
                    Type::Enum { name, variants, .. } => {
                        if let Some(variant) = variants.iter().find(|v| v.name == prop_name) {
                            if variant.payload.is_some() {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!("Variant '{}' of enum '{}' requires a payload and must be constructed via call", prop_name, name),
                                );
                            }
                            canonical_base
                        } else {
                            self.reporter.report(
                                mem.property.loc,
                                format!("Variant '{}' not found in enum '{}'", prop_name, name),
                            );
                            self.type_db.void()
                        }
                    }
                    Type::Array(element_ty, _) => {
                        if prop_name == "len" {
                            self.type_db.int()
                        } else if prop_name == "data" {
                            self.type_db.pointer(*element_ty)
                        } else {
                            self.reporter.report(
                                mem.property.loc,
                                format!(
                                    "Arrays only have fields 'len' and 'data', found '{}'",
                                    prop_name
                                ),
                            );
                            self.type_db.void()
                        }
                    }
                    Type::Slice(element_ty) => {
                        if prop_name == "len" {
                            self.type_db.int()
                        } else if prop_name == "data" {
                            self.type_db.pointer(*element_ty)
                        } else {
                            self.reporter.report(
                                mem.property.loc,
                                format!(
                                    "Slices only have fields 'len' and 'data', found '{}'",
                                    prop_name
                                ),
                            );
                            self.type_db.void()
                        }
                    }
                    Type::Pointer(_inner) => {
                        unreachable!("Pointer type should have been auto-dereferenced in the loop");
                    }
                    Type::String(inner) => {
                        let inner_canon = self.resolve(*inner);
                        if let Type::Struct {
                            name: _,
                            fields: Some(fields),
                        } = self.type_db.get_type(inner_canon).clone()
                        {
                            if let Some(f) = fields.iter().find(|field| field.name == prop_name) {
                                f.ty
                            } else {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!("Field '{}' not found in string", prop_name),
                                );
                                self.type_db.void()
                            }
                        } else {
                            unreachable!()
                        }
                    }
                    Type::Tuple(elements) => {
                        if let Ok(idx) = prop_name.parse::<usize>() {
                            if idx < elements.len() {
                                elements[idx]
                            } else {
                                self.reporter.report(
                                    mem.property.loc,
                                    format!(
                                        "Tuple index '{}' out of bounds (tuple size is {})",
                                        idx,
                                        elements.len()
                                    ),
                                );
                                self.type_db.void()
                            }
                        } else {
                            self.reporter.report(
                                mem.property.loc,
                                format!(
                                    "Tuple fields must be integer indices, found '{}'",
                                    prop_name
                                ),
                            );
                            self.type_db.void()
                        }
                    }
                    _ => {
                        self.reporter.report(
                            mem.property.loc,
                            format!(
                                "Cannot access member of non-struct/non-array type '{}'",
                                self.type_db.type_to_string(canonical_base)
                            ),
                        );
                        self.type_db.void()
                    }
                }
            }
            s::ExprKind::TypeInfo(ast_ty, _) => {
                let target_ty = map_type(ast_ty, &mut self.type_db);
                let canonical_target = self.type_db.resolve(target_ty);
                self.type_db.queried_types.insert(canonical_target);
                let type_info_id = self.type_db.type_info();
                self.type_db.pointer(type_info_id)
            }
            s::ExprKind::Unsafe(inner, _) => {
                self.infer_expr(inner)
            }
        };
        expr.resolved_type = Some(ty);
        ty
    }

    fn try_resolve_signature(
        &mut self,
        callee_name: &str,
        arg_types: &[TypeID],
    ) -> Option<FunctionSignature> {
        let mut resolved_sig = None;
        if let Some(overloads) = self.symtab.functions.get(callee_name) {
            for sig in overloads {
                if (arg_types.len() <= sig.params.len())
                    || ((arg_types.len() >= sig.params.len()) && sig.has_va_args)
                {
                    let mut matches = true;
                    for (p, arg) in sig.params.iter().zip(arg_types.iter()) {
                        if !self.types_match(*p, *arg) {
                            matches = false;
                            break;
                        }
                    }
                    if !matches {
                        continue;
                    }

                    // Check if the remaining params have default values
                    let decl = self.resolved_fns.get(&sig.mangled_name).unwrap();
                    let mut all_have_defaults = true;
                    for i in arg_types.len()..sig.params.len() {
                        if decl.signature.parameters[i].value.is_none() {
                            all_have_defaults = false;
                            break;
                        }
                    }
                    if all_have_defaults {
                        resolved_sig = Some(sig.clone());
                        break;
                    }
                }
            }
        }
        resolved_sig
    }

    fn is_local_array_var(&self, expr: &s::Expr) -> bool {
        match &expr.kind {
            s::ExprKind::Unsafe(..) => false,
            s::ExprKind::Identifier(ident) => {
                if let Some(ty) = self.lookup_var(ident.source()) {
                    let canonical = self.type_db.resolve(ty);
                    matches!(self.type_db.get_type(canonical), Type::Array(..))
                } else {
                    false
                }
            }
            s::ExprKind::Cast(_, inner, _) | s::ExprKind::AutoCast(inner, _) => {
                self.is_local_array_var(inner)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(inner), _) => {
                self.is_local_array_var(inner)
            }
            _ => false,
        }
    }

    fn is_expr_stack_backed(&self, expr: &s::Expr) -> bool {
        match &expr.kind {
            s::ExprKind::Unsafe(..) => false,
            s::ExprKind::Array(..) => true,
            s::ExprKind::Identifier(ident) => {
                self.stack_backed_vars.contains(ident.source())
            }
            s::ExprKind::Cast(_, inner, _) | s::ExprKind::AutoCast(inner, _) => {
                self.is_expr_stack_backed(inner)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Scalar(inner), _) => {
                self.is_expr_stack_backed(inner)
            }
            s::ExprKind::AnyCast(s::AnyCastExpr::Array(arr), _) => {
                arr.iter().any(|item| self.is_expr_stack_backed(item))
            }
            s::ExprKind::Tuple(exprs, _) => {
                exprs.iter().any(|item| self.is_expr_stack_backed(item))
            }
            _ => false,
        }
    }

    fn types_match(&self, expected: TypeID, actual: TypeID) -> bool {
        let expected = self.type_db.resolve(expected);
        let actual = self.type_db.resolve(actual);
        if expected == actual {
            return true;
        }
        if actual == self.type_db.noreturn() {
            return true;
        }
        if let (Some((base1, args1)), Some((base2, args2))) = (
            self.type_db.generic_instantiations.get(&expected),
            self.type_db.generic_instantiations.get(&actual),
        ) {
            if base1 == base2 && args1.len() == args2.len() {
                return args1
                    .iter()
                    .zip(args2.iter())
                    .all(|(a1, a2)| self.types_match(*a1, *a2));
            }
        }
        let t_expected = self.type_db.get_type(expected);
        let t_actual = self.type_db.get_type(actual);

        match (t_expected, t_actual) {
            (Type::TypeVar(_), _) | (_, Type::TypeVar(_)) => true,
            (Type::Pointer(e), Type::Pointer(a)) => {
                let void_id = self.type_db.void();
                *e == void_id || *a == void_id || self.types_match(*e, *a)
            }
            (Type::Slice(e), Type::Slice(a)) => self.types_match(*e, *a),
            (Type::Array(e, e_len), Type::Array(a, a_len)) => {
                *e_len == *a_len && self.types_match(*e, *a)
            }
            (Type::Slice(expected_elem), Type::Array(actual_elem, _)) => {
                self.types_match(*expected_elem, *actual_elem)
            }
            (Type::FnPointer { params: p_expected, return_type: ret_expected }, Type::FnPointer { params: p_actual, return_type: ret_actual }) => {
                p_expected.len() == p_actual.len()
                    && p_expected.iter().zip(p_actual.iter()).all(|(&e, &a)| self.types_match(e, a))
                    && self.types_match(*ret_expected, *ret_actual)
            }
            _ => false,
        }
    }

    fn is_castable_type(&self, ty: TypeID) -> bool {
        let underlying = self.type_db.get_underlying_type(ty);
        if self.type_db.is_primitive_castable(underlying) {
            return true;
        }
        matches!(
            self.type_db.get_type(underlying),
            Type::Pointer(_) | Type::Enum { .. } | Type::TypeVar(_) | Type::FnPointer { .. }
        )
    }

    fn resolve(&mut self, id: TypeID) -> TypeID {
        let canonical = self.type_db.resolve(id);
        if let Some((base_name, arg_ids)) =
            self.type_db.generic_instantiations.get(&canonical).cloned()
        {
            let resolved_args: Vec<TypeID> =
                arg_ids.iter().map(|&a| self.type_db.resolve(a)).collect();
            let inst_name = s::mangle_instantiation_name(&base_name, &resolved_args, &self.type_db);
            let has_fields = if let Some(existing_id) = self.type_db.lookup_by_name(&inst_name) {
                let canonical_existing = self.type_db.resolve(existing_id);
                matches!(
                    self.type_db.get_type(canonical_existing),
                    Type::Struct {
                        fields: Some(_),
                        ..
                    }
                )
            } else {
                false
            };

            if !has_fields && self.struct_templates.contains_key(&base_name) {
                self.monomorphize_struct_with_ids(&base_name, &resolved_args);
            }
        }
        self.type_db.resolve(canonical)
    }

    fn monomorphize_struct_with_ids(&mut self, name: &str, arg_ids: &[TypeID]) -> TypeID {
        self.monomorphization_depth += 1;
        if self.monomorphization_depth > 100 {
            self.reporter.report(
                Loc::default(),
                "Monomorphization depth limit exceeded (possible infinite recursion in generics)".to_string()
            );
            self.monomorphization_depth -= 1;
            return self.type_db.void();
        }
        let decl = self
            .struct_templates
            .get(name)
            .cloned()
            .expect("struct template not found");
        let generic_params = decl.generic_params.as_ref().unwrap();

        let mut substs = HashMap::new();
        for (i, param) in generic_params.iter().enumerate() {
            substs.insert(param.source().to_string(), arg_ids[i]);
        }

        let inst_name = s::mangle_instantiation_name(name, arg_ids, &self.type_db);

        if let Some(id) = self.type_db.lookup_by_name(&inst_name) {
            let canonical = self.type_db.resolve(id);
            if let Type::Struct {
                fields: Some(_), ..
            } = self.type_db.get_type(canonical)
            {
                self.monomorphization_depth -= 1;
                return id;
            }
        }

        let struct_id = if let Some(id) = self.type_db.lookup_by_name(&inst_name) {
            id
        } else {
            self.type_db.insert_named_type(
                inst_name.clone(),
                Type::Struct {
                    name: inst_name.clone(),
                    fields: None,
                },
            )
        };
        self.type_db
            .register_generic_instantiation(struct_id, name.to_string(), arg_ids.to_vec());

        let mut fields = Vec::new();
        let mut ast_fields = Vec::new();

        let mut ast_substs = HashMap::new();
        for (i, param) in generic_params.iter().enumerate() {
            let ast_ty = type_id_to_ast(arg_ids[i], &self.type_db);
            ast_substs.insert(param.source().to_string(), ast_ty);
        }

        for field in &decl.fields {
            let mapped_ty = map_type_with_substs(&field.typ, &substs, &mut self.type_db);
            fields.push(StructField {
                name: field.name.source().to_string(),
                ty: mapped_ty,
            });
            let subst_ty = subst_type(&field.typ, &ast_substs);
            ast_fields.push(s::Field {
                name: field.name.clone(),
                typ: subst_ty,
                value: field.value.as_ref().map(|v| subst_expr(v, &ast_substs)),
            });
        }

        let _ = self.type_db.set_struct_fields(struct_id, fields);

        let monomorphized_decl = s::StructDecl {
            name: Token::new(
                TokenKind::Identifier,
                decl.name.loc,
                lex_just_parse::lexer::TokenSource::from(inst_name.as_str()),
            ),
            generic_params: None,
            directives: decl.directives.clone(),
            fields: ast_fields,
        };

        self.struct_decls
            .insert(inst_name.clone(), monomorphized_decl.clone());
        self.new_monomorphized_items
            .push(s::FileItem::Struct(monomorphized_decl));

        self.monomorphization_depth -= 1;
        struct_id
    }

    fn monomorphize_struct(&mut self, name: &str, type_args: &[s::Type]) -> TypeID {
        let mut arg_ids = Vec::new();
        for arg in type_args {
            arg_ids.push(map_type(arg, &mut self.type_db));
        }
        self.monomorphize_struct_with_ids(name, &arg_ids)
    }

    fn monomorphize_enum_with_ids(&mut self, name: &str, arg_ids: &[TypeID]) -> TypeID {
        self.monomorphization_depth += 1;
        if self.monomorphization_depth > 100 {
            self.reporter.report(
                Loc::default(),
                "Monomorphization depth limit exceeded (possible infinite recursion in generics)".to_string()
            );
            self.monomorphization_depth -= 1;
            return self.type_db.void();
        }
        let decl = self
            .enum_templates
            .get(name)
            .cloned()
            .expect("enum template not found");
        let generic_params = decl.generic_params.as_ref().unwrap();

        let mut substs = HashMap::new();
        for (i, param) in generic_params.iter().enumerate() {
            substs.insert(param.source().to_string(), arg_ids[i]);
        }

        let inst_name = s::mangle_instantiation_name(name, arg_ids, &self.type_db);

        if let Some(id) = self.type_db.lookup_by_name(&inst_name) {
            let canonical = self.type_db.resolve(id);
            if let Type::Enum { variants, .. } = self.type_db.get_type(canonical) {
                if !variants.is_empty() {
                    self.monomorphization_depth -= 1;
                    return id;
                }
            }
        }

        let repr = decl
            .inner_type
            .as_ref()
            .map(|t| map_type_with_substs(t, &substs, &mut self.type_db))
            .unwrap_or(self.type_db.int());

        let enum_id = if let Some(id) = self.type_db.lookup_by_name(&inst_name) {
            id
        } else {
            self.type_db.insert_named_type(
                inst_name.clone(),
                Type::Enum {
                    name: inst_name.clone(),
                    repr,
                    variants: Vec::new(),
                },
            )
        };
        self.type_db
            .register_generic_instantiation(enum_id, name.to_string(), arg_ids.to_vec());

        let mut variants = Vec::new();
        let mut ast_variants = Vec::new();

        let mut ast_substs = HashMap::new();
        for (i, param) in generic_params.iter().enumerate() {
            let ast_ty = type_id_to_ast(arg_ids[i], &self.type_db);
            ast_substs.insert(param.source().to_string(), ast_ty);
        }

        let mut counter = 0;
        for variant in &decl.variants {
            match variant {
                s::EnumVariant::Name(tok) => {
                    variants.push(EnumVariant {
                        name: tok.source().to_string(),
                        default_value: counter,
                        payload: None,
                    });
                    ast_variants.push(s::EnumVariant::Name(tok.clone()));
                }
                s::EnumVariant::DefaultValue(tok, lit) => {
                    variants.push(EnumVariant {
                        name: tok.source().to_string(),
                        default_value: {
                            counter = lit.value;
                            counter
                        },
                        payload: None,
                    });
                    ast_variants.push(s::EnumVariant::DefaultValue(tok.clone(), lit.clone()));
                }
                s::EnumVariant::TuplePayload(tok, types) => {
                    let elem_tys: Vec<TypeID> = types
                        .iter()
                        .map(|t| map_type_with_substs(t, &substs, &mut self.type_db))
                        .collect();
                    let payload_ty = self.type_db.tuple(elem_tys);
                    variants.push(EnumVariant {
                        name: tok.source().to_string(),
                        default_value: counter,
                        payload: Some(payload_ty),
                    });
                    let subst_tys: Vec<s::Type> =
                        types.iter().map(|t| subst_type(t, &ast_substs)).collect();
                    ast_variants.push(s::EnumVariant::TuplePayload(tok.clone(), subst_tys));
                }
                s::EnumVariant::StructPayload(tok, fields) => {
                    let struct_name = format!("{}_{}", inst_name, tok.source());
                    let struct_fields: Vec<StructField> = fields
                        .iter()
                        .map(|f| StructField {
                            name: f.name.source().to_string(),
                            ty: map_type_with_substs(&f.typ, &substs, &mut self.type_db),
                        })
                        .collect();
                    let payload_ty = self.type_db.insert_named_type(
                        struct_name.clone(),
                        Type::Struct {
                            name: struct_name,
                            fields: Some(struct_fields),
                        },
                    );
                    variants.push(EnumVariant {
                        name: tok.source().to_string(),
                        default_value: counter,
                        payload: Some(payload_ty),
                    });

                    let mut subst_fields = Vec::new();
                    for field in fields {
                        let subst_ty = subst_type(&field.typ, &ast_substs);
                        subst_fields.push(s::Field {
                            name: field.name.clone(),
                            typ: subst_ty,
                            value: field.value.as_ref().map(|v| subst_expr(v, &ast_substs)),
                        });
                    }
                    ast_variants.push(s::EnumVariant::StructPayload(tok.clone(), subst_fields));
                }
            }
            counter += 1;
        }

        let _ = self.type_db.set_enum_variants(enum_id, variants);

        let monomorphized_decl = s::EnumDecl {
            name: Token::new(
                TokenKind::Identifier,
                decl.name.loc,
                lex_just_parse::lexer::TokenSource::from(inst_name.as_str()),
            ),
            generic_params: None,
            directives: decl.directives.clone(),
            inner_type: decl.inner_type.as_ref().map(|t| subst_type(t, &ast_substs)),
            variants: ast_variants,
        };

        self.enum_decls
            .insert(inst_name.clone(), monomorphized_decl.clone());
        self.new_monomorphized_items
            .push(s::FileItem::Enum(monomorphized_decl));

        self.monomorphization_depth -= 1;
        enum_id
    }

    fn monomorphize_enum(&mut self, name: &str, type_args: &[s::Type]) -> TypeID {
        let mut arg_ids = Vec::new();
        for arg in type_args {
            arg_ids.push(map_type(arg, &mut self.type_db));
        }
        self.monomorphize_enum_with_ids(name, &arg_ids)
    }

    fn monomorphize_function(
        &mut self,
        name: &str,
        decl: &s::FunctionDecl,
        type_args: &[s::Type],
    ) -> String {
        self.monomorphization_depth += 1;
        if self.monomorphization_depth > 100 {
            self.reporter.report(
                decl.signature.name.loc,
                "Monomorphization depth limit exceeded (possible infinite recursion in generics)".to_string()
            );
            self.monomorphization_depth -= 1;
            return name.to_string();
        }
        let generic_params = decl.generic_params.as_ref().unwrap();

        let mut substs = HashMap::new();
        for (i, param) in generic_params.iter().enumerate() {
            substs.insert(param.source().to_string(), type_args[i].clone());
        }

        let mut parameters = Vec::new();
        let mut param_types = Vec::new();
        for p in &decl.signature.parameters {
            let subst_ty = subst_type(&p.typ, &substs);
            let mapped_ty = map_type(&subst_ty, &mut self.type_db);
            parameters.push(s::FunctionParameter {
                name: p.name.clone(),
                typ: subst_ty,
                value: p.value.as_ref().map(|v| subst_expr(v, &substs)),
            });
            param_types.push(mapped_ty);
        }

        let mut arg_ids = Vec::new();
        for arg in type_args {
            arg_ids.push(map_type(arg, &mut self.type_db));
        }
        let inst_name =
            Self::mangle_function_instantiation_name(name, &arg_ids, &param_types, &self.type_db);

        if self.resolved_fns.contains_key(&inst_name) {
            self.monomorphization_depth -= 1;
            return inst_name;
        }

        let return_type = decl
            .signature
            .return_type
            .as_ref()
            .map(|t| subst_type(t, &substs));

        let mapped_return_type = return_type
            .as_ref()
            .map(|t| map_type(t, &mut self.type_db))
            .unwrap_or_else(|| self.type_db.void());

        let signature = s::FunctionSignature {
            name: Token::new(
                TokenKind::Identifier,
                decl.signature.name.loc,
                TokenSource::from(inst_name.as_str()),
            ),
            parameters: parameters.clone(),
            return_type,
            va_args: decl.signature.va_args,
        };

        let mut body = decl.body.as_ref().map(|b| {
            let mut new_stmts = Vec::new();
            for s in &b.stmts {
                new_stmts.push(subst_stmt(s, &substs));
            }
            s::BlockStmt {
                loc: b.loc,
                stmts: new_stmts,
            }
        });

        // Typecheck the monomorphized body immediately (before cloning to monomorphized_decl)
        if let Some(ref mut b) = body {
            self.push_scope();
            for p in &parameters {
                let ty = map_type(&p.typ, &mut self.type_db);
                self.insert_var(p.name.source().to_string(), ty);
            }

            let old_return = self.expected_return_type;
            self.expected_return_type = mapped_return_type;
            self.check_block(b);
            self.expected_return_type = old_return;
            self.pop_scope();
        }

        let monomorphized_decl = s::FunctionDecl {
            signature,
            generic_params: None,
            directives: decl.directives.clone(),
            body: body.clone(),
            resolved_name: Some(inst_name.clone()),
        };

        let sig = FunctionSignature {
            name: inst_name.clone(),
            params: param_types,
            return_type: mapped_return_type,
            mangled_name: inst_name.clone(),
            has_va_args: decl.signature.va_args,
        };

        if self.symtab.add(sig, &self.type_db).is_err() {
            // Already defined
        }

        self.resolved_fns
            .insert(inst_name.clone(), monomorphized_decl.clone());
        self.new_monomorphized_items
            .push(s::FileItem::FunctionDecl(monomorphized_decl));

        self.monomorphization_depth -= 1;
        inst_name
    }

    fn mangle_function_instantiation_name(
        base_name: &str,
        type_args: &[TypeID],
        param_types: &[TypeID],
        db: &TypeDatabase,
    ) -> String {
        let mut name = base_name.to_string();
        for arg in type_args {
            name.push('_');
            let arg_str = db.type_to_string(*arg);
            let safe_arg = arg_str
                .replace('*', "ptr")
                .replace('[', "arr")
                .replace([']', ' '], "");
            name.push_str(&safe_arg);
        }
        let mut hasher = DefaultHasher::new();
        for p in param_types {
            db.type_to_string(*p).hash(&mut hasher);
        }
        format!("{}_{:x}", name, hasher.finish())
    }
}

fn subst_type(ast_ty: &s::Type, substs: &HashMap<String, s::Type>) -> s::Type {
    match ast_ty {
        s::Type::Scalar(token) => {
            if let Some(t) = substs.get(token.source()) {
                t.clone()
            } else {
                ast_ty.clone()
            }
        }
        s::Type::Pointer(count, inner) => {
            s::Type::Pointer(*count, Box::new(subst_type(inner, substs)))
        }
        s::Type::Array(inner, size) => s::Type::Array(Box::new(subst_type(inner, substs)), *size),
        s::Type::Slice(inner) => s::Type::Slice(Box::new(subst_type(inner, substs))),
        s::Type::GenericInst(base, args) => {
            let new_base = Box::new(subst_type(base, substs));
            let mut new_args = Vec::new();
            for arg in args {
                new_args.push(subst_type(arg, substs));
            }
            s::Type::GenericInst(new_base, new_args)
        }
        s::Type::Tuple(elements, loc) => {
            let mut new_elements = Vec::new();
            for elem in elements {
                new_elements.push(subst_type(elem, substs));
            }
            s::Type::Tuple(new_elements, *loc)
        }
        s::Type::FnPointer { parameters, return_type, loc } => {
            let mut new_params = Vec::new();
            for param in parameters {
                new_params.push(s::VarTypeValue {
                    name: param.name.clone(),
                    typ: subst_type(&param.typ, substs),
                    value: param.value.as_ref().map(|v| subst_expr(v, substs)),
                });
            }
            let new_ret = return_type.as_ref().map(|ret| Box::new(subst_type(ret, substs)));
            s::Type::FnPointer {
                parameters: new_params,
                return_type: new_ret,
                loc: *loc,
            }
        }
    }
}

fn subst_expr(expr: &s::Expr, substs: &HashMap<String, s::Type>) -> s::Expr {
    let new_kind = match &expr.kind {
        s::ExprKind::StructLiteral(lit) => {
            let new_type_args = lit
                .type_args
                .as_ref()
                .map(|args| args.iter().map(|arg| subst_type(arg, substs)).collect());
            let mut new_fields = Vec::new();
            for f in &lit.fields {
                new_fields.push(s::FieldInit {
                    name: f.name.clone(),
                    value: subst_expr(&f.value, substs),
                });
            }
            s::ExprKind::StructLiteral(s::StructLiteralExpr {
                name: lit.name.clone(),
                type_args: new_type_args,
                fields: new_fields,
            })
        }
        s::ExprKind::Array(arr) => {
            let mut new_elements = Vec::new();
            for el in &arr.elements {
                new_elements.push(subst_expr(el, substs));
            }
            s::ExprKind::Array(s::ArrayExpr {
                loc: arr.loc,
                elements: new_elements,
                type_hint: arr.type_hint.clone(),
            })
        }
        s::ExprKind::Call(call) => {
            let new_callee = Box::new(subst_expr(&call.callee, substs));
            let mut new_args = Vec::new();
            for arg in &call.arguments {
                new_args.push(subst_expr(arg, substs));
            }
            s::ExprKind::Call(s::CallExpr {
                loc: call.loc,
                callee: new_callee,
                arguments: new_args,
                resolved_name: call.resolved_name.clone(),
            })
        }
        s::ExprKind::Index(idx) => s::ExprKind::Index(s::IndexExpr {
            loc: idx.loc,
            array: Box::new(subst_expr(&idx.array, substs)),
            index: Box::new(subst_expr(&idx.index, substs)),
        }),
        s::ExprKind::Member(mem) => s::ExprKind::Member(s::MemberExpr {
            object: Box::new(subst_expr(&mem.object, substs)),
            property: mem.property.clone(),
        }),
        s::ExprKind::Binary(bin) => s::ExprKind::Binary(s::BinaryExpr {
            op: bin.op.clone(),
            left: Box::new(subst_expr(&bin.left, substs)),
            right: Box::new(subst_expr(&bin.right, substs)),
            use_operator_overload: bin.use_operator_overload.clone(),
        }),
        s::ExprKind::Unary(un) => s::ExprKind::Unary(s::UnaryExpr {
            op: un.op,
            right: Box::new(subst_expr(&un.right, substs)),
        }),
        s::ExprKind::Assign(ass) => s::ExprKind::Assign(s::AssignExpr {
            assign_kind: ass.assign_kind,
            left: Box::new(subst_expr(&ass.left, substs)),
            right: Box::new(subst_expr(&ass.right, substs)),
        }),
        s::ExprKind::TypeInfo(ty, loc) => {
            s::ExprKind::TypeInfo(Box::new(subst_type(ty, substs)), *loc)
        }
        s::ExprKind::Cast(ty, inner, loc) => s::ExprKind::Cast(
            subst_type(ty, substs),
            Box::new(subst_expr(inner, substs)),
            *loc,
        ),
        s::ExprKind::AutoCast(inner, loc) => {
            s::ExprKind::AutoCast(Box::new(subst_expr(inner, substs)), *loc)
        }
        s::ExprKind::Unsafe(inner, loc) => {
            s::ExprKind::Unsafe(Box::new(subst_expr(inner, substs)), *loc)
        }
        s::ExprKind::GenericInst(base, args, loc) => {
            let new_base = Box::new(subst_expr(base, substs));
            let new_args = args.iter().map(|arg| subst_type(arg, substs)).collect();
            s::ExprKind::GenericInst(new_base, new_args, *loc)
        }
        _ => expr.kind.clone(),
    };
    s::Expr {
        kind: new_kind,
        resolved_type: None,
    }
}

fn subst_stmt(stmt: &s::Stmt, substs: &HashMap<String, s::Type>) -> s::Stmt {
    match stmt {
        s::Stmt::ExprStmt(expr) => s::Stmt::ExprStmt(subst_expr(expr, substs)),
        s::Stmt::Call(call) => {
            let new_callee = Box::new(subst_expr(&s::Expr::new(call.callee.kind.clone()), substs));
            let s::ExprKind::Identifier(ident) = new_callee.kind else {
                panic!("Expected callee to be identifier after substitution");
            };
            let mut new_args = Vec::new();
            for arg in &call.arguments {
                new_args.push(subst_expr(arg, substs));
            }
            s::Stmt::Call(s::CallExpr {
                loc: call.loc,
                callee: Box::new(s::Expr::new(s::ExprKind::Identifier(ident))),
                arguments: new_args,
                resolved_name: call.resolved_name.clone(),
            })
        }
        s::Stmt::Block(block) => {
            let mut new_stmts = Vec::new();
            for s in &block.stmts {
                new_stmts.push(subst_stmt(s, substs));
            }
            s::Stmt::Block(s::BlockStmt {
                loc: block.loc,
                stmts: new_stmts,
            })
        }
        s::Stmt::VarDecl(decl) => {
            let new_type = decl.var_type.as_ref().map(|t| subst_type(t, substs));
            let new_expr = subst_expr(&decl.expr, substs);
            s::Stmt::VarDecl(s::VarDeclStmt {
                names: decl.names.clone(),
                var_type: new_type,
                expr: new_expr,
            })
        }
        s::Stmt::Return(loc, val) => {
            s::Stmt::Return(*loc, val.as_ref().map(|v| subst_expr(v, substs)))
        }
        s::Stmt::IfStmt(if_stmt) => match if_stmt {
            s::IfStmt::If { cond, true_body } => {
                let mut new_stmts = Vec::new();
                for s in &true_body.stmts {
                    new_stmts.push(subst_stmt(s, substs));
                }
                s::Stmt::IfStmt(s::IfStmt::If {
                    cond: subst_expr(cond, substs),
                    true_body: s::BlockStmt {
                        loc: true_body.loc,
                        stmts: new_stmts,
                    },
                })
            }
            s::IfStmt::IfElse {
                cond,
                true_body,
                false_body,
            } => {
                let mut new_true_stmts = Vec::new();
                for s in &true_body.stmts {
                    new_true_stmts.push(subst_stmt(s, substs));
                }
                let mut new_false_stmts = Vec::new();
                for s in &false_body.stmts {
                    new_false_stmts.push(subst_stmt(s, substs));
                }
                s::Stmt::IfStmt(s::IfStmt::IfElse {
                    cond: subst_expr(cond, substs),
                    true_body: s::BlockStmt {
                        loc: true_body.loc,
                        stmts: new_true_stmts,
                    },
                    false_body: s::BlockStmt {
                        loc: false_body.loc,
                        stmts: new_false_stmts,
                    },
                })
            }
        },
        s::Stmt::ForStmt(for_stmt) => match for_stmt {
            s::ForStmt::ForLoop(body) => {
                let mut new_stmts = Vec::new();
                for s in &body.stmts {
                    new_stmts.push(subst_stmt(s, substs));
                }
                s::Stmt::ForStmt(s::ForStmt::ForLoop(s::BlockStmt {
                    loc: body.loc,
                    stmts: new_stmts,
                }))
            }
            s::ForStmt::ForCond { cond, body } => {
                let mut new_stmts = Vec::new();
                for s in &body.stmts {
                    new_stmts.push(subst_stmt(s, substs));
                }
                s::Stmt::ForStmt(s::ForStmt::ForCond {
                    cond: subst_expr(cond, substs),
                    body: s::BlockStmt {
                        loc: body.loc,
                        stmts: new_stmts,
                    },
                })
            }
        },
        s::Stmt::ForEach(fe) => {
            let mut new_stmts = Vec::new();
            for s in &fe.body.stmts {
                new_stmts.push(subst_stmt(s, substs));
            }
            s::Stmt::ForEach(s::ForEachStmt {
                var_name: fe.var_name.clone(),
                iter_expr: subst_expr(&fe.iter_expr, substs),
                body: s::BlockStmt {
                    loc: fe.body.loc,
                    stmts: new_stmts,
                },
            })
        }
        s::Stmt::Switch(switch_stmt) => {
            let new_cond = subst_expr(&switch_stmt.cond, substs);
            let mut new_branches = Vec::new();
            for b in &switch_stmt.branches {
                new_branches.push(s::SwitchBranch {
                    pattern: subst_expr(&b.pattern, substs),
                    body: subst_stmt(&b.body, substs),
                });
            }
            let new_default = switch_stmt
                .default
                .as_ref()
                .map(|d| Box::new(subst_stmt(d, substs)));
            s::Stmt::Switch(s::SwitchStmt {
                loc: switch_stmt.loc,
                cond: new_cond,
                branches: new_branches,
                default: new_default,
            })
        }
        s::Stmt::Defer(loc, inner) => s::Stmt::Defer(*loc, Box::new(subst_stmt(inner, substs))),
        _ => stmt.clone(),
    }
}

fn dummy_token(name: &str) -> Token {
    Token::new(
        TokenKind::Identifier,
        Loc::default(),
        lex_just_parse::lexer::TokenSource::from(name),
    )
}

fn type_id_to_ast(id: TypeID, db: &TypeDatabase) -> s::Type {
    let canonical = db.resolve(id);
    if canonical == db.void() {
        s::Type::Scalar(dummy_token("void"))
    } else if canonical == db.int() {
        s::Type::Scalar(dummy_token("int"))
    } else if canonical == db.u8() {
        s::Type::Scalar(dummy_token("u8"))
    } else if canonical == db.bool() {
        s::Type::Scalar(dummy_token("bool"))
    } else if canonical == db.string() {
        s::Type::Scalar(dummy_token("string"))
    } else {
        match db.get_type(canonical) {
            Type::Pointer(inner) => s::Type::Pointer(1, Box::new(type_id_to_ast(*inner, db))),
            Type::Array(inner, size) => s::Type::Array(Box::new(type_id_to_ast(*inner, db)), *size),
            Type::Slice(inner) => s::Type::Slice(Box::new(type_id_to_ast(*inner, db))),
            Type::Struct { name, .. } => s::Type::Scalar(dummy_token(name)),
            Type::Enum { name, .. } => s::Type::Scalar(dummy_token(name)),
            Type::Distinct { name, .. } => s::Type::Scalar(dummy_token(name)),
            Type::Tuple(elems) => {
                let mut ast_elems = Vec::new();
                for &elem in elems {
                    ast_elems.push(type_id_to_ast(elem, db));
                }
                s::Type::Tuple(ast_elems, Loc::default())
            }
            Type::FnPointer { params, return_type } => {
                let mut parameters = Vec::new();
                for (i, &param) in params.iter().enumerate() {
                    parameters.push(s::VarTypeValue {
                        name: dummy_token(&format!("p{}", i)),
                        typ: type_id_to_ast(param, db),
                        value: None,
                    });
                }
                let ret_ast = if *return_type == db.void() {
                    None
                } else {
                    Some(Box::new(type_id_to_ast(*return_type, db)))
                };
                s::Type::FnPointer {
                    parameters,
                    return_type: ret_ast,
                    loc: Loc::default(),
                }
            }
            Type::Primitive(..) | Type::TypeVar(..) | Type::Any(..) | Type::String(..) => {
                s::Type::Scalar(dummy_token("void"))
            }
        }
    }
}

fn map_type_with_substs(
    ast_ty: &s::Type,
    substs: &HashMap<String, TypeID>,
    db: &mut TypeDatabase,
) -> TypeID {
    match ast_ty {
        s::Type::Scalar(token) => {
            if let Some(&ty_id) = substs.get(token.source()) {
                ty_id
            } else {
                map_type(ast_ty, db)
            }
        }
        s::Type::Pointer(count, inner) => {
            let mut inner_id = map_type_with_substs(inner, substs, db);
            for _ in 0..*count {
                inner_id = db.pointer(inner_id);
            }
            inner_id
        }
        s::Type::Array(inner, size) => {
            let inner_id = map_type_with_substs(inner, substs, db);
            db.array(inner_id, *size)
        }
        s::Type::Slice(inner) => {
            let inner_id = map_type_with_substs(inner, substs, db);
            db.slice(inner_id)
        }
        s::Type::GenericInst(base, args) => {
            let base_name = match &**base {
                s::Type::Scalar(token) => token.source(),
                _ => panic!("Generic instantiation base must be a scalar name"),
            };
            let mut arg_ids = Vec::new();
            for arg in args {
                arg_ids.push(map_type_with_substs(arg, substs, db));
            }
            let inst_name = s::mangle_instantiation_name(base_name, &arg_ids, db);
            let id = if let Some(id) = db.lookup_by_name(&inst_name) {
                id
            } else {
                db.insert_named_type(
                    inst_name.clone(),
                    Type::Struct {
                        name: inst_name,
                        fields: None,
                    },
                )
            };
            db.register_generic_instantiation(id, base_name.to_string(), arg_ids);
            id
        }
        s::Type::Tuple(elements, _) => {
            let mut mapped_elems = Vec::new();
            for elem in elements {
                mapped_elems.push(map_type_with_substs(elem, substs, db));
            }
            db.tuple(mapped_elems)
        }
        s::Type::FnPointer { parameters, return_type, .. } => {
            let mut param_ids = Vec::new();
            for param in parameters {
                param_ids.push(map_type_with_substs(&param.typ, substs, db));
            }
            let ret_id = match return_type {
                Some(ret) => map_type_with_substs(ret, substs, db),
                None => db.void(),
            };
            db.fn_pointer(param_ids, ret_id)
        }
    }
}
