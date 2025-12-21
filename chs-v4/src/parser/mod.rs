#![allow(unused)]
use std::path::{Path, PathBuf};

use crate::parser::lexer::{Lexer, Token, TokenKind};

pub mod ast;
pub mod lexer;

use ast::*;

pub struct ParserError(pub String);

impl std::fmt::Debug for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParserError {}

pub struct Parser<'a> {
    file_path: PathBuf,
    lexer: &'a mut Lexer<'a>,
    current: Token,
    peek: Token,
}

#[derive(PartialEq, PartialOrd)]
enum Precedence {
    Lowest,
    Assignment,

    LogicalOr,
    LogicalAnd,
    Equality,

    Comparison,
    BitwiseOr,
    BitwiseXor,
    BitwiseAnd,
    BitShift,

    BitWiseNotLogicalNot,

    AddSub,
    MulDivMod,
    Neg,

    Call,
    Member,
    Index,
}

fn token_to_precedence(token: &Token) -> Precedence {
    match token.kind {
        TokenKind::Dot => Precedence::Member,
        TokenKind::DoubleColon => Precedence::Member,
        TokenKind::OpenParen => Precedence::Call,
        TokenKind::OpenBracket => Precedence::Index,
        TokenKind::Plus | TokenKind::Minus => Precedence::AddSub,
        TokenKind::Asterisk | TokenKind::Slash | TokenKind::Mod => Precedence::MulDivMod,
        t if t.is_assign_kind() => Precedence::Assignment,
        TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => Precedence::Comparison,
        TokenKind::Eq => Precedence::Equality,
        _ => Precedence::Lowest,
    }
}

impl<'a> Parser<'a> {
    pub fn new(file_path: PathBuf, lexer: &'a mut Lexer<'a>) -> Self {
        Self {
            file_path,
            current: lexer.next(),
            peek: lexer.next(),
            lexer,
        }
    }

    fn next_token(&mut self) {
        self.current = self.peek.clone();
        self.peek = self.lexer.next();
    }

    fn peek_precedence(&self) -> Precedence {
        token_to_precedence(&self.peek)
    }

    fn expect_current(&mut self, kind: TokenKind) -> Result<(), ParserError> {
        if self.current.kind == kind {
            Ok(())
        } else {
            Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected {expected:?}, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                expected = kind,
                found = self.current
            )))
        }
    }

    fn expect_peek(&mut self, kind: TokenKind) -> Result<(), ParserError> {
        if self.peek.kind == kind {
            self.next_token();
            Ok(())
        } else {
            Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected {expected:?}, found {found}",
                file_path = self.file_path.display(),
                loc = self.peek.loc,
                expected = kind,
                found = self.peek
            )))
        }
    }

    pub fn parse_program(mut self) -> Result<Module, ParserError> {
        let mut decls = vec![];

        while !self.current.is_eof() {
            let decl = self.parse_decl()?;
            decls.push(decl);
            self.next_token();
        }

        Ok(Module {
            file_path: self.file_path,
            decls,
        })
    }

    fn parse_decl(&mut self) -> Result<Decl, ParserError> {
        let metadata = self.parse_metadata()?;
        match self.current.source() {
            "import" => self.parse_import_decl(),
            "fn" => Ok(Decl::Method(self.parse_method_decl(metadata)?)),
            "type" => self.parse_type_decl(),
            "trait" => self.parse_trait_decl(),
            "impl" => self.parse_impl_decl(),
            _ => Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected decl, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                found = self.current
            ))),
        }
    }

    fn parse_metadata(&mut self) -> Result<Vec<Directive>, ParserError> {
        let mut metadata = Vec::new();
        while self.current.kind == TokenKind::Directive {
            match self.current.source() {
                "#private" => metadata.push(Directive::Private),
                "#public" => metadata.push(Directive::Public),
                "#static" => metadata.push(Directive::Static),
                "#instance" => metadata.push(Directive::Instance),
                "#extern" => metadata.push(Directive::Extern),
                _ => {
                    return Err(ParserError(format!(
                        "{file_path}:{loc} ParserError: Unknow directive found {found}",
                        file_path = self.file_path.display(),
                        loc = self.current.loc,
                        found = self.current
                    )));
                }
            }
            self.next_token();
        }
        Ok(metadata)
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParserError> {
        match self.current.source() {
            "var" => self.parse_var_decl_statement(),
            "for" => self.parse_for_statement(),
            "foreach" => self.parse_foreach_statement(),
            "if" => self.parse_if_statement(),
            "return" => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_import_decl(&mut self) -> Result<Decl, ParserError> {
        self.next_token();
        let mut module_path = Vec::new();
        self.expect_current(TokenKind::Identifier)?;
        module_path.push(self.current.clone());

        while self.peek.kind == TokenKind::Slash {
            self.next_token();
            self.next_token();
            self.expect_current(TokenKind::Identifier)?;
            module_path.push(self.current.clone());
        }

        let module_name = module_path.pop().unwrap();
        if self.peek.kind == TokenKind::SemiColon {
            self.next_token();
        }
        Ok(Decl::Import(ImportDecl {
            module_path,
            module_name,
        }))
    }

    fn parse_trait_methods_decl(&mut self) -> Result<Vec<MethodDecl>, ParserError> {
        let mut methods = vec![];

        if self.peek.kind == TokenKind::CloseCurly {
            self.next_token();
            return Ok(methods);
        }

        self.next_token();

        loop {
            let metadata = self.parse_metadata()?;
            let method_decl = self.parse_method_decl(metadata)?;
            methods.push(method_decl);

            if self.peek.kind == TokenKind::CloseCurly {
                self.next_token();
                break;
            }

            self.next_token();
        }

        Ok(methods)
    }

    fn parse_impl_decl(&mut self) -> Result<Decl, ParserError> {
        self.next_token();
        let trait_name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        self.next_token();
        self.expect_current(TokenKind::ForKeyword)?;
        self.next_token();
        let type_name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        self.next_token();
        self.expect_current(TokenKind::OpenCurly)?;
        let methods = self.parse_trait_methods_decl()?;
        Ok(Decl::Impl(ImplDecl {
            trait_name,
            type_name,
            methods,
        }))
    }

    fn parse_trait_decl(&mut self) -> Result<Decl, ParserError> {
        self.next_token();
        let name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        self.next_token();
        self.expect_current(TokenKind::OpenCurly)?;
        let methods = self.parse_trait_methods_decl()?;
        Ok(Decl::Trait(TraitDecl { name, methods }))
    }

    fn parse_type_decl(&mut self) -> Result<Decl, ParserError> {
        self.next_token();
        let name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        self.next_token();
        self.expect_current(TokenKind::OpenCurly)?;
        self.next_token();

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut constructors = Vec::new();

        loop {
            if self.peek.kind == TokenKind::CloseCurly {
                self.next_token();
                break;
            }

            let metadata = self.parse_metadata()?;

            match self.current.kind {
                TokenKind::FnKeyword => {
                    let method = self.parse_method_decl(metadata)?;
                    methods.push(method);
                }
                TokenKind::Identifier if self.current.source() == "init" => {
                    let constructor = self.parse_constructor()?;
                    constructors.push(constructor);
                }
                TokenKind::Identifier => {
                    let field = self.parse_field(metadata)?;
                    fields.push(field);
                }
                TokenKind::CloseCurly => break,
                _ => {
                    return Err(ParserError(format!(
                        "{file_path}:{loc} ParserError: Unexpected token found in type declaration: {found}",
                        file_path = self.file_path.display(),
                        loc = self.current.loc,
                        found = self.current
                    )));
                }
            }

            self.next_token();
        }

        Ok(Decl::Type(TypeDecl {
            name,
            fields,
            methods,
            constructors,
        }))
    }

    fn parse_field(&mut self, metadata: Vec<Directive>) -> Result<Field, ParserError> {
        let mut visibility = Visibility::Public;
        for directive in metadata {
            match directive {
                Directive::Private => visibility = Visibility::Private,
                Directive::Public => visibility = Visibility::Public,
                _ => todo!(),
            }
        }
        let name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        self.next_token();
        let typ = self.parse_type()?;
        Ok(Field {
            name,
            typ,
            visibility,
        })
    }

    fn parse_constructor(&mut self) -> Result<ConstructorDecl, ParserError> {
        let loc = self.current.loc;
        self.next_token();
        self.expect_current(TokenKind::OpenParen)?;
        let params = self.parse_function_params()?;
        self.expect_current(TokenKind::CloseParen)?;
        self.next_token();
        self.expect_current(TokenKind::OpenCurly)?;
        let body = self.parse_block_statement()?;
        self.expect_current(TokenKind::CloseCurly)?;
        Ok(ConstructorDecl {
            loc,
            arguments: params,
            body,
        })
    }

    fn parse_method_decl(&mut self, metadata: Vec<Directive>) -> Result<MethodDecl, ParserError> {
        self.next_token();
        let mut visibility = Visibility::Public;
        let mut kind = MethodKind::Static;
        for directive in metadata {
            match directive {
                Directive::Public => visibility = Visibility::Public,
                Directive::Private => visibility = Visibility::Private,
                Directive::Static => kind = MethodKind::Static,
                Directive::Instance => kind = MethodKind::Instance,
                Directive::Extern => kind = MethodKind::Extern,
            }
        }
        let name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;

        self.expect_peek(TokenKind::OpenParen)?;

        let params = self.parse_function_params()?;

        self.next_token();
        let return_type = match self.current.kind {
            TokenKind::OpenBracket | TokenKind::Identifier => {
                let res = Some(self.parse_type()?);
                if self.peek.kind == TokenKind::OpenCurly {
                    self.next_token();
                }
                res
            }
            _ => None,
        };

        let body = if (self.current.kind != TokenKind::OpenCurly) {
            BlockStmt::default()
        } else {
            self.parse_block_statement()?
        };

        Ok(MethodDecl {
            visibility,
            kind,
            name,
            arguments: params.into(),
            return_type,
            body,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParserError> {
        match self.current.kind {
            TokenKind::Identifier => {
                let ident = self
                    .parse_identifier_expr()?
                    .into_identifier(self.file_path.as_path(), &self.current)?;
                let ty = Type::Identifier(ident);
                Ok(ty)
            }
            TokenKind::Asterisk => {
                self.next_token();
                let pointee_type = self.parse_type()?;
                Ok(Type::Pointer(Box::new(pointee_type)))
            }
            TokenKind::Ellipsis => {
                // self.next_token();
                Ok(Type::VaArgs)
            }
            _ => Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected type, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                found = self.current
            ))),
        }
    }

    fn parse_function_params(&mut self) -> Result<Vec<FunctionArgument>, ParserError> {
        let mut params = vec![];

        if self.peek.kind == TokenKind::CloseParen {
            self.next_token();
            return Ok(params);
        }

        self.next_token();

        loop {
            let name = self
                .parse_identifier_expr()?
                .into_identifier(self.file_path.as_path(), &self.current)?;

            self.next_token();
            let type_decl = self.parse_type()?;

            params.push(FunctionArgument {
                name,
                r#type: type_decl,
            });

            if self.peek.kind != TokenKind::Comma {
                break;
            }
            self.next_token();
            self.next_token();
        }

        self.expect_peek(TokenKind::CloseParen)?;

        Ok(params)
    }

    fn parse_block_statement(&mut self) -> Result<BlockStmt, ParserError> {
        let mut body = vec![];
        self.next_token();

        while self.current.kind != TokenKind::CloseCurly && !self.current.is_eof() {
            let stmt = self.parse_statement()?;
            body.push(stmt);
            self.next_token();
        }

        if self.current.kind != TokenKind::CloseCurly {
            return Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected `}}`, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                found = self.current
            )));
        }
        Ok(BlockStmt { stmts: body.into() })
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, ParserError> {
        self.next_token();
        let mut expr = None;

        if self.current.kind != TokenKind::SemiColon {
            expr = Some(self.parse_expression(Precedence::Lowest)?);
            if self.peek.kind == TokenKind::SemiColon {
                self.next_token();
            }
        } else {
            self.expect_current(TokenKind::SemiColon)?;
        }

        Ok(Stmt::Return(expr))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, ParserError> {
        self.next_token();
        let cond = self.parse_expression(Precedence::Lowest)?;
        self.next_token();
        self.expect_current(TokenKind::OpenCurly)?;

        let mut body = vec![];
        self.next_token();

        while self.current.kind != TokenKind::CloseCurly && !self.current.is_eof() {
            let stmt = self.parse_statement()?;
            body.push(stmt);
            self.next_token();
        }

        if self.current.kind != TokenKind::CloseCurly {
            return Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected `}}`, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                found = self.current
            )));
        }

        if self.peek.source() == "else" {
            self.next_token();
            self.next_token();
            self.expect_current(TokenKind::OpenCurly)?;

            let mut else_body = vec![];
            self.next_token();

            while self.current.kind != TokenKind::CloseCurly && !self.current.is_eof() {
                let stmt = self.parse_statement()?;
                else_body.push(stmt);
                self.next_token();
            }

            if self.current.kind != TokenKind::CloseCurly {
                return Err(ParserError(format!(
                    "{file_path}:{loc} ParserError: Expected `}}`, found {found}",
                    file_path = self.file_path.display(),
                    loc = self.current.loc,
                    found = self.current
                )));
            }
            return Ok(Stmt::IfStmt(IfStmt::IfElse {
                cond,
                true_body: BlockStmt { stmts: body.into() },
                false_body: BlockStmt {
                    stmts: else_body.into(),
                },
            }));
        }

        Ok(Stmt::IfStmt(IfStmt::If {
            cond,
            true_body: BlockStmt { stmts: body.into() },
        }))
    }

    fn parse_for_statement(&mut self) -> Result<Stmt, ParserError> {
        self.next_token();
        match self.current.kind {
            TokenKind::OpenCurly => {
                let mut body = vec![];
                self.next_token();

                self.parse_body(&mut body)?;
                Ok(Stmt::ForStmt(ForStmt::ForLoop(BlockStmt {
                    stmts: body.into(),
                })))
            }
            _ => {
                let cond = self.parse_expression(Precedence::Lowest)?;
                self.next_token();
                let mut inc = None;
                match self.current.kind {
                    TokenKind::OpenCurly => {}
                    TokenKind::SemiColon => {
                        self.next_token();
                        inc = Some(self.parse_expression(Precedence::Lowest)?);
                    }
                    _ => {}
                }
                self.expect_current(TokenKind::OpenCurly)?;
                let mut body = vec![];
                self.next_token();

                self.parse_body(&mut body)?;

                if let Some(inc) = inc {
                    Ok(Stmt::ForStmt(ForStmt::ForCondInc {
                        cond,
                        inc,
                        body: BlockStmt { stmts: body.into() },
                    }))
                } else {
                    Ok(Stmt::ForStmt(ForStmt::ForCond {
                        cond,
                        body: BlockStmt { stmts: body.into() },
                    }))
                }
            }
        }
    }

    fn parse_body(&mut self, body: &mut Vec<Stmt>) -> Result<(), ParserError> {
        while self.current.kind != TokenKind::CloseCurly && !self.current.is_eof() {
            let stmt = self.parse_statement()?;
            body.push(stmt);
            self.next_token();
        }
        Ok(if self.current.kind != TokenKind::CloseCurly {
            return Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected `}}`, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                found = self.current
            )));
        })
    }

    fn parse_var_decl_statement(&mut self) -> Result<Stmt, ParserError> {
        self.next_token();
        let name = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;

        let mut typ = None;
        let expr = if self.peek.kind == TokenKind::Assign {
            self.next_token();
            self.next_token();
            self.parse_expression(Precedence::Assignment)?
        } else {
            self.next_token();
            typ = Some(self.parse_type()?);
            self.expect_peek(TokenKind::Assign)?;
            self.next_token();
            self.parse_expression(Precedence::Assignment)?
        };

        if self.peek.kind == TokenKind::SemiColon {
            self.next_token();
        }

        Ok(Stmt::VarDecl(VarDeclStmt { name, typ, expr }))
    }

    fn parse_expression_statement(&mut self) -> Result<Stmt, ParserError> {
        let expr = self.parse_expression(Precedence::Lowest)?;

        if self.peek.kind == TokenKind::SemiColon {
            self.next_token();
        }

        Ok(Stmt::Expr(ExprStmt { expr }))
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Result<Expr, ParserError> {
        let mut left = self.parse_prefix()?;

        while self.peek.kind != TokenKind::SemiColon && precedence < self.peek_precedence() {
            self.next_token();
            left = self.parse_infix(left)?;
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParserError> {
        match self.current.kind {
            TokenKind::NewKeyword => self.parse_new_expr(),
            TokenKind::Minus | TokenKind::Bang => self.parse_unary_expr(),
            TokenKind::Identifier if self.current.source() == "true" => {
                Ok(Expr::BoolLiteral(BoolLiteral { loc: self.current.loc, value: true }))
            }
            TokenKind::Identifier if self.current.source() == "false" => {
                Ok(Expr::BoolLiteral(BoolLiteral { loc: self.current.loc, value: false }))
            }
            TokenKind::Identifier => self.parse_identifier_expr(),
            TokenKind::StringLiteral => self.parse_string_literal_expr(),
            kind if kind.is_int_num() => self.parse_integer_literal_expr(),
            _ => Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected prefix expression, found {found}",
                file_path = self.file_path.display(),
                loc = self.current.loc,
                found = self.current
            ))),
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr, ParserError> {
        match self.current.kind {
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Asterisk
            | TokenKind::Slash
            | TokenKind::Mod
            | TokenKind::Eq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::Gt
            | TokenKind::GtEq => self.parse_binary_expression(left),
            t if t.is_assign_kind() => self.parse_assign_expression(left),
            TokenKind::Dot => self.parse_member_expression(left),
            TokenKind::DoubleColon => self.parse_namespace_expression(left),
            TokenKind::OpenParen => self.parse_call_expression(left),
            TokenKind::OpenBracket => self.parse_index_expression(left),
            _ => Ok(left),
        }
    }

    fn parse_identifier_expr(&mut self) -> Result<Expr, ParserError> {
        self.expect_current(TokenKind::Identifier)?;
        Ok(Expr::Identifier(Identifier { token: self.current.clone() }))
    }

    fn parse_string_literal_expr(&mut self) -> Result<Expr, ParserError> {
        Ok(Expr::StringLiteral(StringLiteral {
            token: self.current.clone(),
        }))
    }

    fn parse_integer_literal_expr(&mut self) -> Result<Expr, ParserError> {
        let loc = self.current.loc;
        match self.current.kind {
            TokenKind::Int(base) => {
                if let Ok(value) = i32::from_str_radix(self.current.source(), base.into()) {
                    return Ok(Expr::IntegerLiteral(IntegerLiteral{
                        loc,
                        kind: IntegerKind::Int(value)
                    }));
                }
            }
            TokenKind::Int64(base) => {
                if let Ok(value) = i64::from_str_radix(self.current.source(), base.into()) {
                    return Ok(Expr::IntegerLiteral(IntegerLiteral{
                        loc,
                        kind: IntegerKind::Int64(value)
                    }));
                }
            }
            TokenKind::UInt(base) => {
                if let Ok(value) = u32::from_str_radix(self.current.source(), base.into()) {
                    return Ok(Expr::IntegerLiteral(IntegerLiteral{
                        loc,
                        kind: IntegerKind::UInt(value)
                    }));
                }
            }
            TokenKind::UInt64(base) => {
                if let Ok(value) = u64::from_str_radix(self.current.source(), base.into()) {
                    return Ok(Expr::IntegerLiteral(IntegerLiteral{
                        loc,
                        kind: IntegerKind::UInt64(value)
                    }));
                }
            }
            _ => (),
        };

        Err(ParserError(format!(
            "{file_path}:{loc} ParserError: Could not parse {source} as 32-bit interger (try using a suffix: i32, u32, i64, u64)",
            file_path = self.file_path.display(),
            loc = self.current.loc,
            source = self.current.source(),
        )))
    }

    fn parse_assign_expression(&mut self, left: Expr) -> Result<Expr, ParserError> {
        let assign_kind = match self.current.kind {
            TokenKind::Assign => AssignKind::Default,
            TokenKind::PlusAssign => AssignKind::Add,
            _ => todo!(),
        };
        self.next_token();
        let right = self.parse_expression(Precedence::Assignment)?;

        Ok(Expr::Assign(AssignExpr {
            left: Box::new(left),
            assign_kind,
            right: Box::new(right),
        }))
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParserError> {
        let mut precedence = Precedence::Neg;
        let op = match self.current.kind {
            TokenKind::Minus => Op::Neg,
            TokenKind::Bang => {
                precedence = Precedence::BitWiseNotLogicalNot;
                Op::Not
            }
            _ => unreachable!(),
        };

        self.next_token();
        let right = self.parse_expression(precedence)?;

        Ok(Expr::Unary(UnaryExpr {
            op,
            right: Box::new(right),
        }))
    }

    fn parse_binary_expression(&mut self, left: Expr) -> Result<Expr, ParserError> {
        let op = match self.current.kind {
            TokenKind::Plus => Op::Add,
            TokenKind::Minus => Op::Sub,
            TokenKind::Asterisk => Op::Mul,
            TokenKind::Slash => Op::Div,
            TokenKind::Mod => Op::Mod,
            TokenKind::Lt => Op::Lt,
            TokenKind::LtEq => Op::LtEq,
            TokenKind::Eq => Op::Eq,
            TokenKind::Gt => Op::Gt,
            TokenKind::GtEq => Op::GtEq,
            _ => unreachable!(),
        };

        let precedence = token_to_precedence(&self.current);
        self.next_token();
        let right = self.parse_expression(precedence)?;

        Ok(Expr::Binary(BinaryExpr {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }))
    }

    fn parse_member_expression(&mut self, object: Expr) -> Result<Expr, ParserError> {
        self.next_token();
        let property = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        Ok(Expr::Member(MemberExpr {
            object: Box::new(object),
            property,
        }))
    }

    fn parse_namespace_expression(&mut self, name: Expr) -> Result<Expr, ParserError> {
        let name = name.into_identifier(self.file_path.as_path(), &self.current)?;
        self.next_token();
        let target = self
            .parse_identifier_expr()?
            .into_identifier(self.file_path.as_path(), &self.current)?;
        Ok(Expr::NamespaceAccess(NamespaceAccessExpr { name, target }))
    }

    fn parse_index_expression(&mut self, array: Expr) -> Result<Expr, ParserError> {
        let loc = self.current.loc;
        self.next_token();
        let index = self.parse_expression(Precedence::Lowest)?;
        self.expect_peek(TokenKind::CloseBracket)?;
        Ok(Expr::Index(IndexExpr {
            loc,
            array: Box::new(array),
            index: Box::new(index),
        }))
    }

    fn parse_call_expression(&mut self, callee: Expr) -> Result<Expr, ParserError> {
        let loc = self.current.loc;
        let arguments = self.parse_call_arguments()?;
        if let Expr::Member(member_expr) = callee {
            return Ok(Expr::MethodCall(MethodCallExpr {
                object: member_expr.object,
                method: member_expr.property,
                arguments: arguments.into(),
            }));
        }
        Ok(Expr::Call(CallExpr {
            loc,
            callee: Box::new(callee),
            arguments: arguments.into(),
        }))
    }

    fn parse_new_expr(&mut self) -> Result<Expr, ParserError> {
        self.next_token();
        self.expect_current(TokenKind::Identifier)?;
        let loc = self.current.loc;
        let ty = self.parse_type()?;
        if self.peek.kind == TokenKind::OpenBracket {
            self.next_token();
            self.next_token();
            let size = self.parse_integer_literal_expr()?.into();
            self.next_token();
            self.expect_current(TokenKind::CloseBracket)?;
            return Ok(Expr::NewArray(NewArrayExpr { loc, ty, size }));
        }
        Ok(Expr::New(NewExpr { loc, ty }))
    }

    fn parse_call_arguments(&mut self) -> Result<Vec<Expr>, ParserError> {
        let mut args = vec![];
        if self.peek.kind == TokenKind::CloseParen {
            self.next_token();
            return Ok(args);
        }

        self.next_token();
        args.push(self.parse_expression(Precedence::Lowest)?);

        while self.peek.kind == TokenKind::Comma {
            self.next_token();
            self.next_token();
            args.push(self.parse_expression(Precedence::Lowest)?);
        }

        self.expect_peek(TokenKind::CloseParen)?;
        Ok(args)
    }

    fn parse_foreach_statement(&mut self) -> Result<Stmt, ParserError> {
        todo!("parse_foreach_statement")
    }
}

impl Expr {
    fn into_identifier(self, file_path: &Path, token: &Token) -> Result<Token, ParserError> {
        if let Expr::Identifier(ident) = self {
            Ok(ident.token)
        } else {
            Err(ParserError(format!(
                "{file_path}:{loc} ParserError: Expected identifier, found {found:?}",
                file_path = file_path.display(),
                loc = token.loc,
                found = self
            )))
        }
    }
}

pub fn parse(file_path: PathBuf, source: &str) -> Result<Module, ParserError> {
    let mut lex = Lexer::new(source);
    let parser = Parser::new(file_path, &mut lex);
    parser.parse_program()
}
