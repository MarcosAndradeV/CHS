#![allow(clippy::enum_variant_names)]

use lex_just_parse::lexer::*;
use std::path::Path;

use types as t;

#[derive(Debug, Clone)]
pub enum Type {
    Scalar(Token),
    Pointer(u32, Box<Type>),
    Array(Box<Type>, usize),
    Slice(Box<Type>),
    GenericInst(Box<Type>, Vec<Type>),
    Tuple(Vec<Type>, Loc),
    FnPointer {
        parameters: Vec<FunctionPointerParameter>,
        return_type: Option<Box<Type>>,
        loc: Loc,
    },
}

impl Type {
    pub fn loc(&self) -> Loc {
        match self {
            Type::Scalar(token) => token.loc,
            Type::Pointer(_, inner) => inner.loc(),
            Type::Array(inner, _) => inner.loc(),
            Type::Slice(inner) => inner.loc(),
            Type::GenericInst(base, _) => base.loc(),
            Type::Tuple(_, loc) => *loc,
            Type::FnPointer { loc, .. } => *loc,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FileItem {
    FunctionDecl(FunctionDecl), // fn main()
    Import(ImportDecl),
    Directive(Directive),
    Struct(StructDecl),
    Enum(EnumDecl),
    TypeDecl(TypeDecl),
    VarDecl(VarDeclStmt),
    ThreadLocal(Vec<VarDeclStmt>),
    OperatorOverload(OperatorDecl)
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: Token,
    pub is_distinct: bool,
    pub base_type: Type,
}

#[derive(Debug, Clone)]
pub enum Directive {
    // Run(BlockStmt), // #run { message("Hi") }
    Library { name: Token, library: Library },
}

#[derive(Debug, Clone)]
pub enum LibraryKind {
    Static,
    Dynlib,
}

impl LibraryKind {
    /// Returns `true` if the library kind is [`Static`].
    ///
    /// [`Static`]: LibraryKind::Static
    #[must_use]
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static)
    }
}

#[derive(Debug, Clone)]
pub struct Library {
    pub link_name: String,
    pub kind: LibraryKind,
}

/// Represents executable statements within a function or block.
#[derive(Debug, Clone)]
pub enum Stmt {
    ExprStmt(Expr),
    Call(CallExpr),
    Block(BlockStmt),
    VarDecl(VarDeclStmt),
    Return(Loc, Option<Expr>),
    ForStmt(ForStmt),
    ForEach(ForEachStmt),
    IfStmt(IfStmt),
    Break(Loc),
    Continue(Loc),
    Defer(Loc, Box<Stmt>),
    Switch(SwitchStmt),
}

#[derive(Debug, Clone)]
pub struct SwitchStmt {
    pub loc: Loc,
    pub cond: Expr,
    pub branches: Vec<SwitchBranch>,
    pub default: Option<Box<Stmt>>,
}

#[derive(Debug, Clone)]
pub struct SwitchBranch {
    pub pattern: Expr,
    pub body: Stmt,
}

impl Stmt {
    pub fn loc(&self) -> Loc {
        match self {
            Stmt::ExprStmt(expr_stmt) => expr_stmt.loc(),
            Stmt::Call(call_expr) => call_expr.loc,
            Stmt::Block(block_stmt) => block_stmt.loc,
            Stmt::VarDecl(var_decl) => var_decl.names[0].loc,
            Stmt::Return(loc, _) => *loc,
            Stmt::ForStmt(for_stmt) => for_stmt.loc(),
            Stmt::ForEach(for_each_stmt) => for_each_stmt.loc(),
            Stmt::IfStmt(if_stmt) => if_stmt.loc(),
            Stmt::Break(loc) => *loc,
            Stmt::Continue(loc) => *loc,
            Stmt::Defer(loc, _) => *loc,
            Stmt::Switch(switch_stmt) => switch_stmt.loc,
        }
    }
}

/// Represents an import declaration `import path/to/module`.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: Token,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Token,
    pub generic_params: Option<Vec<Token>>,
    pub directives: Vec<EnumDirective>,
    pub inner_type: Option<Type>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub enum EnumVariant {
    Name(Token),                         // FOO
    DefaultValue(Token, IntegerLiteral), // BAR = 1,
}

impl EnumVariant {
    pub fn token(&self) -> &Token {
        match self {
            EnumVariant::Name(tok) => tok,
            EnumVariant::DefaultValue(tok, _) => tok,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: Token,
    pub generic_params: Option<Vec<Token>>,
    pub directives: Vec<StructDirective>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct FunctionPointerParameter {
    pub name: Token,
    pub typ: Type,
}

#[derive(Debug, Clone)]
pub struct FunctionParameter {
    pub name: Token,
    pub typ: Type,
    pub value: Option<Expr>,
    pub is_variadic: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Token,
    pub typ: Type,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: Token,
    pub parameters: Vec<FunctionParameter>,
    pub return_type: Option<Type>,
    pub va_args: bool,
}

#[derive(Debug, Clone)]
pub struct OperatorDecl {
    pub signature: FunctionSignature,
    pub body: Option<BlockStmt>,
    pub op: Op,
    pub resolved_name: String,
}

/// Represents a function declaration, including its signature and body.
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub signature: FunctionSignature,
    pub generic_params: Option<Vec<Token>>,
    pub directives: Vec<FunctionDirective>,
    pub body: Option<BlockStmt>,
    pub resolved_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FunctionDirective {
    Foreign(Loc, Token),
    LinkName(Token),
    Private,
}

#[derive(Debug, Clone)]
pub enum StructDirective {
    Test(Loc),
}

#[derive(Debug, Clone)]
pub enum EnumDirective {
    Test(Loc),
}

#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub loc: Loc,
    pub stmts: Vec<Stmt>,
}
impl BlockStmt {
    pub fn new(loc: Loc) -> Self {
        Self {
            loc,
            stmts: Vec::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty()
    }
}

#[derive(Debug, Clone)]
pub enum IfStmt {
    If {
        cond: Expr,
        true_body: BlockStmt,
    },
    IfElse {
        cond: Expr,
        true_body: BlockStmt,
        false_body: BlockStmt,
    },
}

impl IfStmt {
    pub fn loc(&self) -> Loc {
        match self {
            IfStmt::If { cond, .. } => cond.loc(),
            IfStmt::IfElse { cond, .. } => cond.loc(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ForStmt {
    ForLoop(BlockStmt),
    ForCond { cond: Expr, body: BlockStmt },
}

impl ForStmt {
    pub fn loc(&self) -> Loc {
        match self {
            ForStmt::ForLoop(block_stmt) => block_stmt.loc,
            ForStmt::ForCond { cond, .. } => cond.loc(),
        }
    }
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct ForEachStmt {
    pub var_name: Token,
    pub iter_expr: Expr,
    pub body: BlockStmt,
}

impl ForEachStmt {
    pub fn loc(&self) -> Loc {
        self.var_name.loc
    }
}

#[derive(Debug, Clone)]
pub struct VarDeclStmt {
    pub names: Vec<Token>,
    pub var_type: Option<Type>,
    pub expr: Expr,
    pub is_thread_local: bool,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub resolved_type: Option<t::TypeID>,
}

impl Expr {
    pub fn new(kind: ExprKind) -> Self {
        Self {
            kind,
            resolved_type: None,
        }
    }

    pub fn loc(&self) -> Loc {
        self.kind.loc()
    }

    pub fn name(&self) -> &str {
        self.kind.name()
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    StructLiteral(StructLiteralExpr),
    Identifier(Token),
    StringLiteral(Token),
    Integer(IntegerLiteral),
    Bool(BoolLiteral),
    Float(FloatLiteral),
    Array(ArrayExpr),
    Call(CallExpr),
    Index(IndexExpr),
    Member(MemberExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
    Null(Loc),
    TypeInfo(Box<Type>, Loc),
    Cast(Type, Box<Expr>, Loc),
    AutoCast(Box<Expr>, Loc),
    AnyCast(AnyCastExpr, Loc),
    GenericInst(Box<Expr>, Vec<Type>, Loc),
    Tuple(Vec<Expr>, Loc),
    Unsafe(Box<Expr>, Loc),
    Default(Loc),
}

impl ExprKind {
    pub fn loc(&self) -> Loc {
        match self {
            ExprKind::Null(loc) => *loc,
            ExprKind::StructLiteral(lit) => lit.name.loc,
            ExprKind::Identifier(identifier) => identifier.loc,
            ExprKind::StringLiteral(string_literal) => string_literal.loc,
            ExprKind::Integer(integer_literal) => integer_literal.loc,
            ExprKind::Bool(bool_literal) => bool_literal.loc,
            ExprKind::Float(float_literal) => float_literal.loc,
            ExprKind::Array(new_array_expr) => new_array_expr.loc,
            ExprKind::Call(call_expr) => call_expr.loc,
            ExprKind::Index(index_expr) => index_expr.loc,
            ExprKind::Member(member_expr) => member_expr.property.loc,
            ExprKind::Binary(binary_expr) => binary_expr.right.loc(),
            ExprKind::Unary(unary_expr) => unary_expr.right.loc(),
            ExprKind::Assign(assign_expr) => assign_expr.right.loc(),
            ExprKind::TypeInfo(_, loc) => *loc,
            ExprKind::Cast(_, _, loc) => *loc,
            ExprKind::AutoCast(_, loc) => *loc,
            ExprKind::GenericInst(_, _, loc) => *loc,
            ExprKind::Tuple(_, loc) => *loc,
            ExprKind::AnyCast(_, loc) => *loc,
            ExprKind::Unsafe(_, loc) => *loc,
            ExprKind::Default(loc) => *loc,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ExprKind::Identifier(..) => "identifier",
            ExprKind::StructLiteral(_) => "struct literal",
            ExprKind::StringLiteral(_) => "string literal",
            ExprKind::Integer(..) => "integer literal",
            ExprKind::Bool(..) => "bool literal",
            ExprKind::Float(..) => "float literal",
            ExprKind::Array(_) => "array",
            ExprKind::Call(_) => "call",
            ExprKind::Index(_) => "index",
            ExprKind::Member(_) => "member",
            ExprKind::Binary(_) => "binary",
            ExprKind::Unary(_) => "unary",
            ExprKind::Assign(_) => "assign",
            ExprKind::Null(_) => "null",
            ExprKind::TypeInfo(..) => "type_info",
            ExprKind::Cast(..) => "cast",
            ExprKind::AutoCast(..) => "auto_cast",
            ExprKind::GenericInst(..) => "generic instantiation",
            ExprKind::Tuple(..) => "tuple",
            ExprKind::AnyCast(..) => "anycast",
            ExprKind::Unsafe(..) => "unsafe",
            ExprKind::Default(..) => "default",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AnyCastExpr {
    Scalar(Box<Expr>),
    Array(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub struct BoolLiteral {
    pub loc: Loc,
    pub value: bool,
}

#[derive(Debug, Clone)]
pub struct IntegerLiteral {
    pub loc: Loc,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct FloatLiteral {
    pub loc: Loc,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ArrayExpr {
    pub loc: Loc,
    pub elements: Vec<Expr>,
    pub type_hint: Option<Type>,
}

#[derive(Debug, Clone)]
pub struct StructLiteralExpr {
    pub name: Token,
    pub type_args: Option<Vec<Type>>,
    pub fields: Vec<FieldInit>,
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: Token,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub loc: Loc,
    pub callee: Box<Expr>,
    pub positional_arguments: Vec<Expr>,
    pub named_arguments: Vec<NamedArg>,
    pub resolved_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NamedArg {
    pub name: Token,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct IndexExpr {
    pub loc: Loc,
    pub array: Box<Expr>,
    pub index: Box<Expr>,
}

// object.property
#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    pub property: Token,
}

#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub left: Box<Expr>,
    pub assign_kind: AssignKind,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: Op,
    pub right: Box<Expr>,
    pub use_operator_overload: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: Op,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Eq,
    Neg,
    Not,
    NotEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Refer,
    Deref,
}

impl BinaryExpr {
    pub fn op_loc(&self) -> Loc {
        self.right.loc()
    }
}

impl Op {
    pub fn get_name(&self) -> &'static str {
        match self {
            Self::Add => "operator_add",
            Self::Sub => "operator_sub",
            Self::Mul => "operator_mul",
            Self::Div => "operator_div",
            Self::Mod => "operator_mod",
            Self::Lt => "operator_lt",
            Self::LtEq => "operator_lteq",
            Self::Gt => "operator_gt",
            Self::GtEq => "operator_gteq",
            Self::Eq => "operator_eq",
            Self::Neg => "operator_neg",
            Self::Not => "operator_not",
            Self::NotEq => "operator_noteq",
            Self::And => "operator_and",
            Self::Or => "operator_or",
            Self::BitAnd => "operator_bitand",
            Self::BitOr => "operator_bitor",
            Self::BitXor => "operator_bitxor",
            Self::Refer => "operator_refer",
            Self::Deref => "operator_deref",
        }
    }

    pub fn is_binary(&self) -> bool {
        match self {
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Mod
            | Self::Lt
            | Self::LtEq
            | Self::Gt
            | Self::GtEq
            | Self::Eq
            | Self::NotEq
            | Self::And
            | Self::BitAnd
            | Self::BitOr
            | Self::BitXor => true,

            Self::Or | Self::Neg | Self::Not | Self::Refer | Self::Deref => false,
        }
    }

    pub fn is_unary(&self) -> bool {
        match self {
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Mod
            | Self::Lt
            | Self::LtEq
            | Self::Gt
            | Self::GtEq
            | Self::Eq
            | Self::NotEq
            | Self::And
            | Self::BitAnd
            | Self::BitOr
            | Self::BitXor => false,

            Self::Or | Self::Neg | Self::Not | Self::Refer | Self::Deref => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignKind {
    Default,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
    Assignment,

    LogicalOr,
    LogicalAnd,
    Equality,

    Comparison,
    BitwiseOr,
    BitwiseXor,
    BitwiseAnd,
    // BitShift,

    // BitWiseNotLogicalNot,
    AddSub,
    MulDivMod,
    Neg,

    Call,
    Member,
    Index,
}

pub fn token_to_precedence(token: &Token) -> Precedence {
    match token.kind {
        TokenKind::Dot => Precedence::Member,
        TokenKind::DoubleColon => Precedence::Member,
        TokenKind::OpenParen => Precedence::Call,
        TokenKind::OpenBracket => Precedence::Index,
        TokenKind::Plus | TokenKind::Minus => Precedence::AddSub,
        TokenKind::Asterisk | TokenKind::Slash | TokenKind::Mod => Precedence::MulDivMod,
        TokenKind::DoubleAmpersand => Precedence::LogicalAnd,
        TokenKind::DoublePipe => Precedence::LogicalOr,
        TokenKind::Ampersand => Precedence::BitwiseAnd,
        TokenKind::Pipe => Precedence::BitwiseOr,
        TokenKind::Caret => Precedence::BitwiseXor,
        t if t.is_assign_kind() => Precedence::Assignment,
        TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => Precedence::Comparison,
        TokenKind::EqEq | TokenKind::NotEq => Precedence::Equality,
        _ => Precedence::Lowest,
    }
}

pub fn mangle_instantiation_name(
    base_name: &str,
    args: &[t::TypeID],
    db: &t::TypeDatabase,
) -> String {
    let mut name = base_name.to_string();
    for arg in args {
        name.push('_');
        let arg_str = db.type_to_string(*arg);
        let safe_arg = arg_str
            .replace('*', "ptr")
            .replace('[', "arr")
            .replace([']', ' '], "");
        name.push_str(&safe_arg);
    }
    name
}

pub fn map_type(ast_ty: &Type, db: &mut t::TypeDatabase, get_module_name: &dyn Fn(&Path) -> Option<String>) -> t::TypeID {
    fn map_type_name(token: &Token, db: &mut t::TypeDatabase, get_module_name: &dyn Fn(&Path) -> Option<String>) -> t::TypeID {
        let name = token.source();
        let ref_file = Path::new(*token.loc.file_path());
        if let Some(m) = get_module_name(ref_file) {
            let namespaced = format!("{}.{}", m, name);
            if let Some(id) = db.lookup_by_name(&namespaced) {
                return id;
            }
        }
        if let Some(id) = db.lookup_by_name(name) {
            id
        } else {
            let name_to_register = if let Some(m) = get_module_name(ref_file) {
                format!("{}.{}", m, name)
            } else {
                name.to_string()
            };
            db.insert_named_type(
                name_to_register.clone(),
                t::Type::Struct {
                    name: name_to_register,
                    fields: None,
                },
            )
        }
    }
    match ast_ty {
        Type::Tuple(elements, _) => {
            let mut mapped_elems = Vec::new();
            for elem in elements {
                mapped_elems.push(map_type(elem, db, get_module_name));
            }
            db.tuple(mapped_elems)
        }
        Type::Pointer(count, inner_ast) => {
            let mut inner = map_type(inner_ast, db, get_module_name);
            for _ in 0..*count {
                inner = db.pointer(inner);
            }
            inner
        }
        Type::Scalar(token) => map_type_name(token, db, get_module_name),
        Type::Array(inner_ast, size) => {
            let inner_ty = map_type(inner_ast, db, get_module_name);
            db.array(inner_ty, *size)
        }
        Type::Slice(inner_ast) => {
            let inner_ty = map_type(inner_ast, db, get_module_name);
            db.slice(inner_ty)
        }
        Type::GenericInst(base, args) => {
            let base_name = match &**base {
                Type::Scalar(token) => token.source(),
                _ => panic!("Generic instantiation base must be a scalar name"),
            };
            let mut arg_ids = Vec::new();
            for arg in args {
                arg_ids.push(map_type(arg, db, get_module_name));
            }
            let ref_file = Path::new(*base.loc().file_path());
            let resolved_base_name = if let Some(m) = get_module_name(ref_file) {
                let namespaced = format!("{}.{}", m, base_name);
                if db.lookup_by_name(&namespaced).is_some() {
                    namespaced
                } else {
                    base_name.to_string()
                }
            } else {
                base_name.to_string()
            };

            let inst_name = mangle_instantiation_name(&resolved_base_name, &arg_ids, db);
            let id = if let Some(id) = db.lookup_by_name(&inst_name) {
                id
            } else {
                let is_enum = if let Some(base_id) = db.lookup_by_name(&resolved_base_name) {
                    let canon_base = db.resolve(base_id);
                    matches!(db.get_type(canon_base), t::Type::Enum { .. })
                } else {
                    false
                };

                if is_enum {
                    db.insert_named_type(
                        inst_name.clone(),
                        t::Type::Enum {
                            name: inst_name,
                            repr: db.int(),
                            variants: Vec::new(),
                        },
                    )
                } else {
                    db.insert_named_type(
                        inst_name.clone(),
                        t::Type::Struct {
                            name: inst_name,
                            fields: None,
                        },
                    )
                }
            };
            db.register_generic_instantiation(id, resolved_base_name, arg_ids);
            id
        }
        Type::FnPointer {
            parameters,
            return_type,
            ..
        } => {
            let mut param_ids = Vec::new();
            for param in parameters {
                param_ids.push(map_type(&param.typ, db, get_module_name));
            }
            let ret_id = match return_type {
                Some(ret) => map_type(ret, db, get_module_name),
                None => db.void(),
            };
            db.fn_pointer(param_ids, ret_id)
        }
    }
}
