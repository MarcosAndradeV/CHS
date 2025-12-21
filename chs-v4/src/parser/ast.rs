use std::{fmt, path::PathBuf, sync::Arc};

use crate::parser::lexer::{Loc, Token};

#[derive(Debug, Clone)]
pub struct Module {
    pub file_path: PathBuf,
    pub decls: Vec<Decl>,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Import(ImportDecl),
    Method(MethodDecl),
    Type(TypeDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
}

impl Decl {
    pub fn as_method_decl(&self) -> Option<&MethodDecl> {
        if let Decl::Method(d) = self {
            Some(d)
        } else {
            None
        }
    }
    pub fn as_type_decl(&self) -> Option<&TypeDecl> {
        if let Decl::Type(d) = self {
            Some(d)
        } else {
            None
        }
    }
    pub fn as_trait_decl(&self) -> Option<&TraitDecl> {
        if let Decl::Trait(d) = self {
            Some(d)
        } else {
            None
        }
    }
    pub fn as_impl_decl(&self) -> Option<&ImplDecl> {
        if let Decl::Impl(d) = self {
            Some(d)
        } else {
            None
        }
    }
}

pub enum Directive {
    Static,
    Instance,
    Extern,
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(ExprStmt),
    Block(BlockStmt),
    VarDecl(VarDeclStmt),
    Return(Option<Expr>),
    ForStmt(ForStmt),
    ForEach(ForEachStmt),
    IfStmt(IfStmt),
}

#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub file_path: Arc<str>,
    pub name: Token,
}

/// ´import std/io´
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub module_path: Vec<Token>,
    pub module_name: Token,
}

#[derive(Debug, Clone)]
pub struct FunctionArgument {
    pub name: Token,
    pub r#type: Type,
}

#[derive(Debug, Clone)]
pub enum Type {
    Identifier(Token),
    Pointer(Box<Type>),
    VaArgs,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Identifier(token) => write!(f, "{}", token.source),
            Type::Pointer(token) => write!(f, "*{}", token),
            Type::VaArgs => write!(f, "..."),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub trait_name: Token,
    pub type_name: Token,
    pub methods: Vec<MethodDecl>,
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub name: Token,
    pub methods: Vec<MethodDecl>,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: Token,
    pub fields: Vec<Field>,
    pub methods: Vec<MethodDecl>,
    pub constructors: Vec<ConstructorDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Token,
    pub typ: Type,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodKind {
    Static,
    Instance,
    Extern,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub visibility: Visibility,
    pub kind: MethodKind,
    pub name: Token,
    pub arguments: Vec<FunctionArgument>,
    pub return_type: Option<Type>,
    pub body: BlockStmt,
}

#[derive(Debug, Clone)]
pub struct ConstructorDecl {
    pub loc: Loc,
    pub arguments: Vec<FunctionArgument>,
    pub body: BlockStmt,
}

#[derive(Debug, Default, Clone)]
pub struct BlockStmt {
    pub stmts: Vec<Stmt>,
}
impl BlockStmt {
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

#[derive(Debug, Clone)]
pub enum ForStmt {
    ForLoop(BlockStmt),
    ForCond {
        cond: Expr,
        body: BlockStmt,
    },
    ForCondInc {
        cond: Expr,
        inc: Expr,
        body: BlockStmt,
    },
}

#[derive(Debug, Clone)]
pub struct ForEachStmt {
    var_name: Token,
    var_typ: Option<Type>,
    iter_expr: Expr,
    body: BlockStmt,
}

#[derive(Debug, Clone)]
pub struct VarDeclStmt {
    pub name: Token,
    pub typ: Option<Type>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Identifier(Identifier),
    StringLiteral(StringLiteral),
    IntegerLiteral(IntegerLiteral),
    BoolLiteral(BoolLiteral),
    New(NewExpr),
    NewArray(NewArrayExpr),
    Call(CallExpr),
    Index(IndexExpr),
    Member(MemberExpr),
    MethodCall(MethodCallExpr),
    NamespaceAccess(NamespaceAccessExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
}

impl Expr {
    pub fn loc(&self) -> Loc {
        match self {
            Expr::Identifier(identifier) => identifier.token.loc,
            Expr::StringLiteral(string_literal) => string_literal.token.loc,
            Expr::IntegerLiteral(integer_literal) => integer_literal.loc,
            Expr::BoolLiteral(bool_literal) => bool_literal.loc,
            Expr::New(new_expr) => new_expr.loc,
            Expr::NewArray(new_array_expr) => new_array_expr.loc,
            Expr::Call(call_expr) => call_expr.loc,
            Expr::Index(index_expr) => index_expr.loc,
            Expr::MethodCall(method_call) => method_call.method.loc,
            Expr::Member(member_expr) => member_expr.property.loc,
            Expr::NamespaceAccess(namespace_access_expr) => namespace_access_expr.name.loc,
            Expr::Binary(binary_expr) => binary_expr.right.loc(),
            Expr::Unary(unary_expr) => unary_expr.right.loc(),
            Expr::Assign(assign_expr) => assign_expr.right.loc(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Expr::Identifier(..) => "identifier",
            Expr::StringLiteral(_) => "string literal",
            Expr::IntegerLiteral(..) => "integer literal",
            Expr::BoolLiteral(..) => "bool literal",
            Expr::New(_) => "new",
            Expr::NewArray(_) => "new array",
            Expr::Call(_) => "call",
            Expr::Index(_) => "index",
            Expr::MethodCall(_) => "method call",
            Expr::Member(_) => "member",
            Expr::NamespaceAccess(_) => "namespace access",
            Expr::Binary(_) => "binary",
            Expr::Unary(_) => "unary",
            Expr::Assign(_) => "assign",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct BoolLiteral {
    pub loc: Loc,
    pub value: bool,
}


#[derive(Debug, Clone)]
pub struct IntegerLiteral {
    pub loc: Loc,
    pub kind: IntegerKind,
}

#[derive(Debug, Clone)]
pub enum IntegerKind {
    Int(i32),
    Int64(i64),
    UInt(u32),
    UInt64(u64),
}

#[derive(Debug, Clone)]
pub struct NewExpr {
    pub loc: Loc,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct NewArrayExpr {
    pub loc: Loc,
    pub ty: Type,
    pub size: Arc<Expr>,
}

#[derive(Debug, Clone)]
pub struct TypeLiteral {
    pub loc: Loc,
    pub ty: Type,
    pub exprs: Arc<[Expr]>,
}

#[derive(Debug, Clone)]
pub struct MethodCallExpr {
    pub object: Box<Expr>,
    pub method: Token,
    pub arguments: Arc<[Expr]>,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub loc: Loc,
    pub callee: Box<Expr>,
    pub arguments: Arc<[Expr]>,
}

#[derive(Debug, Clone)]
pub struct IndexExpr {
    pub loc: Loc,
    pub array: Box<Expr>,
    pub index: Box<Expr>,
}

#[derive(Debug, Clone)]
pub struct MemberExpr {
    pub object: Box<Expr>,
    pub property: Token,
}

#[derive(Debug, Clone)]
pub struct NamespaceAccessExpr {
    pub name: Token,
    pub target: Token,
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
