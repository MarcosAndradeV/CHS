use std::collections::HashMap;

use crate::chslexer::*;

#[derive(Debug, Default)]
pub struct Program<'src> {
    pub externs: Vec<ExternFunc<'src>>,
    pub global_vars: Vec<GlobalVar<'src>>,
    pub funcs: Vec<Func<'src>>,
}

#[derive(Debug)]
pub enum Names {
    ExternFunc(usize),
    Func(usize),
    GlobalVar(usize),
    Const(usize),
    Var(VarId),
    Arg(usize),
}

#[derive(Debug, Default)]
pub struct NameSpace<'src>(pub Vec<HashMap<&'src str, Names>>);

pub type VarId = usize;

#[derive(Debug)]
pub struct ExternFunc<'src> {
    pub used: bool,
    pub name: Token<'src>,
    pub args: Vec<Type>,
    pub is_variadic: bool,
    pub ret: Type,
}

#[derive(Debug)]
pub struct GlobalVar<'src> {
    pub token: Token<'src>,
    pub ty: Type,
    pub is_vec: bool,
    pub value: Option<ConstExpr<'src>>,
}

#[derive(Debug, Clone)]
pub struct Var<'src> {
    pub used: bool,
    pub loc: Loc<'src>,
    pub ty: Type,
}

#[derive(Debug, Default, Clone)]
pub struct Exec<'src> {
    pub vars: Vec<Var<'src>>,
    pub block_count: usize,
    pub body: Vec<Stmt<'src>>,
}

#[derive(Debug, Default, Clone)]
pub struct Body<'src> {
    pub vars: Vec<Var<'src>>,
    pub block_count: usize,
    pub stmts: Vec<Stmt<'src>>,
}

#[derive(Debug, Default, Clone)]
pub struct Func<'src> {
    pub used: bool,
    pub name: Token<'src>,
    pub args: Vec<&'src str>,
    pub args_types: Vec<Type>,
    pub ret_type: Type,

    pub body: Body<'src>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Type {
    #[default]
    Void,

    SBits(u8),
    UBits(u8),
    Bool,
    Char,

    PtrTo(Box<Self>),
    Array(u64, Box<Self>),
}

#[derive(Debug, Clone)]
pub enum Stmt<'src> {
    Store {
        target: Expr<'src>,
        rhs: Expr<'src>,
    },
    AssignGlobalVar {
        var: (Token<'src>, usize),
        rhs: Expr<'src>,
    },
    AssignVar {
        loc: Loc<'src>,
        var: VarId,
        rhs: Expr<'src>,
    },
    Return(Loc<'src>, Option<Expr<'src>>),
    Unop {
        result: VarId,
        operator: Token<'src>,
        operand: Expr<'src>,
    },
    Binop {
        result: VarId,
        operator: Token<'src>,
        lhs: Expr<'src>,
        rhs: Expr<'src>,
    },
    Funcall {
        result: Option<VarId>,
        caller: Token<'src>,
        args: Vec<Expr<'src>>,
    },
    Syscall {
        result: Option<VarId>,
        args: Vec<Expr<'src>>,
    },
    JZ(Expr<'src>, usize),
    JNZ(Expr<'src>, usize),
    Jmp(usize),
    Block(usize),
}

#[derive(Clone, Debug)]
pub enum ConstExpr<'src> {
    IntLit(u64),
    StrLit(Token<'src>),
}

#[derive(Debug, Clone)]
pub enum Expr<'src> {
    Void(Loc<'src>),
    IntLit(Loc<'src>, u64),
    CharLit(Loc<'src>, char),
    StrLit(Token<'src>),

    Var(Loc<'src>, VarId),
    Arg(Loc<'src>, usize),
    Global(Token<'src>, usize),

    Deref(Loc<'src>, VarId),
    Ref(Loc<'src>, VarId),

    GlobalDeref(Token<'src>, usize),
}

impl<'src> Body<'src> {
    pub fn push(&mut self, value: Stmt<'src>) {
        self.stmts.push(value);
    }
    pub fn push_block(&mut self, bid: usize) {
        self.push(Stmt::Block(bid));
    }
    pub fn next_block(&mut self) -> usize {
        let bid = self.block_count;
        self.block_count += 1;
        bid
    }
}

impl Func<'_> {
    pub fn push_block(&mut self, bid: usize) {
        self.body.push(Stmt::Block(bid));
    }
    pub fn next_block(&mut self) -> usize {
        let bid = self.body.block_count;
        self.body.block_count += 1;
        bid
    }
}

pub fn type_of_expr<'src>(
    expr: &Expr<'src>,
    global_vars: &[GlobalVar<'src>],
    vars: &[Var<'src>],
) -> Result<Type, AppError> {
    match expr {
        Expr::Void(..) => Ok(Type::Void),
        Expr::IntLit(..) => Ok(Type::SBits(32)),
        Expr::CharLit(..) => Ok(Type::Char),
        Expr::StrLit(..) => Ok(Type::PtrTo(Box::new(Type::Char))),
        Expr::Var(token, var_id) => Ok(vars[*var_id].ty.clone()),
        Expr::Arg(token, id) => todo!(),
        Expr::Global(token, uid) => Ok(global_vars[*uid].ty.clone()),
        Expr::Deref(token, var_id) => match &vars[*var_id].ty {
            Type::PtrTo(t) => Ok(t.as_ref().to_owned()),
            t => todo!("{t:?}"),
        },
        Expr::Ref(token, var_id) => Ok(Type::PtrTo(Box::new(vars[*var_id].ty.clone()))),
        Expr::GlobalDeref(token, uid) => match &global_vars[*uid].ty {
            Type::PtrTo(t) => Ok(t.as_ref().to_owned()),
            _ => todo!(),
        },
    }
}

impl<'src> NameSpace<'src> {
    pub fn push_scope(&mut self) {
        self.0.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        assert!(self.0.len() != 1);
        self.0.pop();
    }

    pub fn insert_var_index(&mut self, k: &'src str, v: Names) -> Option<Names> {
        self.0.last_mut().and_then(|scope| scope.insert(k, v))
    }

    pub fn get(&self, k: &'src str) -> Option<&Names> {
        for scope in self.0.iter().rev() {
            let v = scope.get(k);
            if v.is_some() {
                return v;
            }
        }
        None
    }
}

impl<'src> Var<'src> {
    pub fn new(loc: Loc<'src>, ty: Type) -> Self {
        Self {
            used: false,
            loc,
            ty,
        }
    }
}

impl<'src> Expr<'src> {
    pub fn loc(&self) -> Loc<'src> {
        match self {
            Expr::Void(loc) => *loc,
            Expr::IntLit(loc, _) => *loc,
            Expr::CharLit(loc, _) => *loc,
            Expr::StrLit(token) => token.loc,
            Expr::Var(loc, _) => *loc,
            Expr::Arg(loc, _) => *loc,
            Expr::Global(token, _) => token.loc,
            Expr::Deref(loc, _) => *loc,
            Expr::Ref(loc, _) => *loc,
            Expr::GlobalDeref(token, _) => token.loc,
        }
    }
}

impl<'src> std::fmt::Display for Expr<'src> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Void(_) => write!(f, "void"),
            Expr::IntLit(_, lit) => write!(f, "{lit}"),
            Expr::CharLit(_, lit) => write!(f, "{lit}"),
            Expr::StrLit(_) => write!(f, "@str"),
            Expr::Var(_, id) => write!(f, "Var({id})"),
            Expr::Arg(_, id) => write!(f, "Arg({id})"),
            Expr::Global(token, _) => write!(f, "{token}"),
            Expr::Deref(_, id) => write!(f, "Deref({id})"),
            Expr::Ref(_, id) => write!(f, "Ref({id})"),
            Expr::GlobalDeref(_, uid) => write!(f, "GlobalDeref({uid})"),
        }
    }
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Type::Void => todo!("Cannot get size of void"),
            Type::Char => 1,
            Type::SBits(b) | Type::UBits(b) => *b as usize >> 3,
            Type::Array(n, ty) => ty.size() * (*n as usize),
            _ => 8,
        }
    }
    pub fn get_inner(&self) -> &Self {
        match self {
            Type::PtrTo(ty) => ty.as_ref(),
            _ => self,
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(..))
    }

    pub fn array_to_ptr_type(self) -> Type {
        match self {
            Self::Array(_ , t) => Self::PtrTo(t),
            _ => self
        }
    }
}

impl<'src> std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SBits(b) => write!(f, "s{b}"),
            Self::UBits(b) => write!(f, "u{b}"),
            Self::Bool => write!(f, "bool"),
            Self::Char => write!(f, "char"),
            Self::Void => write!(f, "void"),
            Self::PtrTo(ty) => write!(f, "*{ty}"),
            Self::Array(n, ty) => write!(f, "[{n}]{ty}"),
        }
    }
}

use crate::*;

pub fn print_program(program: &Program) {
    for ext in &program.externs {
        let ExternFunc {
            used,
            name,
            args,
            is_variadic,
            ret,
        } = &ext;
        print!("[{}] extern fn {}(", if *used { 'X' } else { ' ' }, name);
        for (i, arg) in args.iter().enumerate() {
            print!("{}", arg);
            if i < args.len() - 1 {
                print!(", ");
            }
        }
        if *is_variadic {
            print!(", ...");
        }
        print!(")");
        print!("-> {}", ret);
        println!(";");
    }

    for func in &program.funcs {
        print_func(func);
    }
}

fn print_func(func: &Func) {
    print!(
        "[{}] fn {}(",
        if func.used { 'X' } else { ' ' },
        func.name.source
    );
    for (i, _) in func.args.iter().enumerate() {
        print!("Var({i})");
        if i < func.args.len() - 1 {
            print!(", ");
        }
    }
    println!(") -> {:?} {{", func.ret_type);

    for (id, var) in func.body.vars.iter().enumerate() {
        println!("[{}] Var({id})", if var.used { 'X' } else { ' ' })
    }

    for stmt in &func.body.stmts {
        print_stmt(&func.body.vars, stmt);
    }

    println!("}}");
}

fn print_stmt(vars: &[Var<'_>], stmt: &Stmt) {
    print!("    ");
    match stmt {
        Stmt::Store { target, rhs } => {
            println!("Store({}) = {};", target, rhs);
        }
        Stmt::AssignVar { var, rhs, .. } => {
            println!("Var({}) = {};", var, rhs);
        }
        Stmt::AssignGlobalVar { var, rhs } => {
            println!("GlobalVar({}) = {};", var.0, rhs);
        }
        Stmt::Return(_, expr) => match expr {
            Some(e) => println!("return {};", e),
            None => println!("return;"),
        },
        Stmt::Unop {
            result,
            operator,
            operand,
        } => {
            println!("Var({}) = {}{};", result, operator, operand);
        }
        Stmt::Binop {
            result,
            operator,
            lhs,
            rhs,
        } => {
            println!("Var({}) = {} {} {};", result, lhs, operator, rhs);
        }
        Stmt::Funcall {
            result,
            caller,
            args,
        } => {
            if let Some(res) = result {
                print!("Var({}) = ", res);
            }
            print!("{}(", caller);
            for (i, arg) in args.iter().enumerate() {
                match arg {
                    Expr::Var(_, var_id) => {
                        print!("Var({})", var_id);
                    }
                    Expr::Deref(_, var_id) => {
                        print!("Var({})^", var_id);
                    }
                    _ => {
                        print!("{}", arg);
                    }
                }
                if i < args.len() - 1 {
                    print!(", ");
                }
            }
            println!(");");
        }
        Stmt::Syscall { result, args } => {
            if let Some(res) = result {
                print!("Var({}) = ", res);
            }
            print!("syscall(");
            for (i, arg) in args.iter().enumerate() {
                print!("{}", arg);
                if i < args.len() - 1 {
                    print!(", ");
                }
            }
            println!(");");
        }
        Stmt::JZ(cond, block) => {
            println!("jz {} goto block_{};", cond, block);
        }
        Stmt::JNZ(cond, block) => {
            println!("jnz {} goto block_{};", cond, block);
        }
        Stmt::Jmp(block) => {
            println!("goto block_{};", block);
        }
        Stmt::Block(block) => {
            println!("block_{}:", block);
        }
    }
}
