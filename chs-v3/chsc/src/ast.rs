use std::collections::HashMap;

use crate::{
    chslexer::{Loc, Token, TokenKind}, Compiler
};

#[derive(Default)]
pub struct Module<'src> {
    pub name: &'src str,
    pub externs: Vec<ExternFunc<'src>>,

    pub funcs: Vec<Func<'src>>,
    pub global_vars: Vec<GlobalVar<'src>>,
    pub consts: Vec<Const<'src>>,
    pub execs: Vec<Exec<'src>>,
    // pub imported_funcs: Vec<Func<'src>>,
}

impl<'src> Module<'src> {
    pub fn add_fn(&mut self, r#fn: Func<'src>, c: &'src Compiler<'src>) -> Option<Name<'src>> {
        let k = r#fn.name.source;
        let len = self.funcs.len();
        self.funcs.push(r#fn);
        c.global_name_space.borrow_mut().insert(k, Name::Func(self.name, len))
    }
    pub fn add_extern_fn(
        &mut self,
        r#extern: ExternFunc<'src>,
        c: &'src Compiler<'src>,
    ) -> Option<Name<'src>> {
        let k = r#extern.name.source;
        let len = self.externs.len();
        self.externs.push(r#extern);
        c.global_name_space.borrow_mut().insert(k, Name::ExternFn(self.name, len))
    }
    pub fn add_global_vars(
        &mut self,
        var: GlobalVar<'src>,
        c: &'src Compiler<'src>,
    ) -> Option<Name<'src>> {
        let k = var.name.source;
        let len = self.global_vars.len();
        self.global_vars.push(var);
        c.global_name_space.borrow_mut().insert(k, Name::GlobalVar(self.name, len))
    }
    pub fn add_consts(&mut self, r#const: Const<'src>, c: &'src Compiler<'src>) -> Option<Name<'src>> {
        let k = r#const.name.source;
        let len = self.consts.len();
        self.consts.push(r#const);
        c.global_name_space.borrow_mut().insert(k, Name::Const(self.name, len))
    }

    pub fn add_exec(&mut self, e: Exec<'src>) {
        self.execs.push(e);
    }
}

pub enum Name<'src> {
    GlobalVar(&'src str, usize),
    Const(&'src str, usize),
    Func(&'src str, usize),
    ExternFn(&'src str, usize),
}

impl<'src> Name<'src> {
    pub fn get_str(&self, c: &'src Compiler<'src>) -> &'src str {
        match self {
            Name::GlobalVar(m, id) => c.modules.borrow()[m].global_vars[*id].name.source,
            Name::Const(m, id) => c.modules.borrow()[m].consts[*id].name.source,
            Name::Func(m, id) => c.modules.borrow()[m].funcs[*id].name.source,
            Name::ExternFn(m, id) => c.modules.borrow()[m].externs[*id].name.source,
        }
    }
}

pub struct Const<'src> {
    pub name: Token<'src>,
    pub expr: Expr<'src>,
}

pub struct GlobalVar<'src> {
    pub name: Token<'src>,
    pub r#type: Type<'src>,
    pub expr: Option<Expr<'src>>,
}

pub struct ExternFunc<'src> {
    pub name: Token<'src>,
    pub args: Vec<Type<'src>>,
    pub ret_type: Option<Type<'src>>,
    pub is_variadic: bool,
}

pub struct Exec<'src> {
    pub token: Token<'src>,
    pub stmt: Stmt<'src>,
}

pub struct Func<'src> {
    pub name: Token<'src>,
    pub args: Vec<(Token<'src>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,

    pub body: Stmt<'src>,
}

#[derive(Clone, Debug)]
pub enum Type<'src> {
    Name(Token<'src>),
    PtrTo(Box<Self>),
    Array(Expr<'src>, Box<Self>)
}

#[derive(Debug)]
pub enum Stmt<'src> {
    Return {
        loc: Loc<'src>,
        expr: Option<Expr<'src>>,
    },
    Expr {
        loc: Loc<'src>,
        expr: Expr<'src>,
    },
    VarDecl {
        name: Token<'src>,
        r#type: Type<'src>,
        expr: Option<Expr<'src>>,
    },
    Assign {
        loc: Loc<'src>,
        lhs: Expr<'src>,
        rhs: Expr<'src>,
    },
    While {
        loc: Loc<'src>,
        cond: Expr<'src>,
        body: Box<Self>,
    },
    If {
        loc: Loc<'src>,
        cond: Expr<'src>,
        true_branch: Box<Self>,
        else_branch: Option<Box<Self>>,
    },
    Block(Loc<'src>, Vec<Self>),
}

#[derive(Clone, Debug)]
pub enum Expr<'src> {
    IntLit(Loc<'src>, u64),
    BoolLit(Loc<'src>, bool),
    StrLit(Token<'src>),

    Ident(Token<'src>),
    Deref(Loc<'src>, Box<Self>),
    Ref(Loc<'src>, Box<Self>),
    Call {
        loc: Loc<'src>,
        caller: Box<Self>,
        args: Vec<Self>,
    },
    Syscall {
        loc: Loc<'src>,
        args: Vec<Self>,
    },
    Binop {
        operator: Token<'src>,
        lhs: Box<Expr<'src>>,
        rhs: Box<Expr<'src>>,
    },
    Index {
        loc: Loc<'src>,
        base: Box<Expr<'src>>,
        index: Box<Expr<'src>>,
    },
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest = 1,

    // Assignment,       // =, +=, -=, etc.
    LogicalOr,    // ||
    LogicalAnd,   // &&
    BitWiseOr,    // |
    BitWiseXor,   // ^
    BitWiseAnd,   // &
    Equality,     // ==, !=
    Comparison,   // <, >, <=, >=
    BitWiseShift, // <<, >>
    Sum,          // +, -
    Factor,       // *, /, %
    RefDeref,     // &, *
    NegNot,       // !, - (unary)
    Call,         // function calls, array indexing, field access

    Highest,
}

impl Precedence {
    pub fn from_token_kind(kind: &TokenKind) -> Option<Precedence> {
        use TokenKind::*;

        let r = match kind {
            // Assign => Self::Assignment,
            DoublePipe => Self::LogicalOr,
            DoubleAmpersand => Self::LogicalAnd,

            Eq | NotEq => Self::Equality,
            Lt | Gt | LtEq | GtEq => Self::Comparison,

            Pipe => Self::BitWiseOr,
            Caret => Self::BitWiseXor,
            Ampersand => Self::BitWiseAnd,
            ShiftLeft | ShiftRight => Self::BitWiseShift,

            Plus | Minus => Self::Sum,
            Asterisk | Slash | Percent => Self::Factor,

            Bang | Tilde => Self::NegNot,

            OpenParen | OpenBracket => Self::Call,

            Caret | Ampersand => Self::RefDeref,

            _ => return None,
        };
        Some(r)
    }
}
