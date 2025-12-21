use std::collections::HashSet;
use std::rc::Rc;

use crate::Compiler;
use crate::ast::{
    Const, Exec, Expr, ExternFunc, Func, GlobalVar, Module, Name, Precedence, Stmt, Type,
};
use crate::chslexer::*;
use crate::utils::{AppError, validate_input_file};

pub fn parse_module<'src>(
    c: &'src Compiler<'src>,
    file_path: &'src str,
    source: &'src str,
) -> Result<(), ()> {
    let mut lexer = PeekableLexer::new(file_path, source);
    lexer.set_is_keyword_fn(|s| {
        matches!(
            s,
            "fn" | "return"
                | "extern"
                | "var"
                | "syscall"
                | "while"
                | "if"
                | "else"
                | "import"
                | "const"
        )
    });

    let mut module = Module {
        name: file_path,
        ..Default::default()
    };

    loop {
        let token = lexer.next_token();
        if token.is_eof() {
            break;
        }
        match token.kind {
            TokenKind::MacroCall if token.source == "@exec" => {
                let stmt = parse_stmt(&c, &mut lexer)?;
                module.add_exec(Exec { token, stmt });
            }
            TokenKind::Keyword if token.source == "fn" => {
                let r#fn = parse_fn(&c, &mut lexer)?;
                let loc = r#fn.name.loc;
                if let Some(name) = module.add_fn(r#fn, c) {
                    c.compiler_error::<()>(
                        format!("{}: Redefinition of {}", loc, name.get_str(c),),
                    );
                };
            }
            TokenKind::Keyword if token.source == "extern" => {
                let r#extern = parse_extern_fn(c, &mut lexer)?;
                let loc = r#extern.name.loc;
                if let Some(name) = module.add_extern_fn(r#extern, c) {
                    c.compiler_error::<()>(
                        format!("{}: Redefinition of {}", loc, name.get_str(c),),
                    );
                };
            }
            TokenKind::Keyword if token.source == "import" => {
                let file_path = expect(
                    c,
                    &mut lexer,
                    TokenKind::StringLiteral,
                    Some("File path not provided"),
                )?
                .unescape();
                expect(c, &mut lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
                let file_path = match validate_input_file(c, &file_path) {
                    Ok(_) => c.add_file_path(file_path),
                    Err(_) => {
                        c.diag.borrow_mut().pop();
                        let file_path = format!("{}/{file_path}", c.stdlib_path);
                        validate_input_file(c, &file_path)?;
                        c.add_file_path(file_path)
                    }
                };
                let Some(source) = c.read_source_file(&file_path)? else {
                    continue;
                };
                parse_module(c, file_path, source)?;
            }
            TokenKind::Keyword if token.source == "var" => {
                let name = expect(
                    c,
                    &mut lexer,
                    TokenKind::Identifier,
                    Some("Expected variable name"),
                )?;
                let r#type = parse_type(c, &mut lexer)?;
                let expr = if inspect(c, &mut lexer, &[TokenKind::Assign])? {
                    lexer.next_token();
                    let expr =
                        parse_expr(c, &mut lexer, &[TokenKind::SemiColon], Precedence::Lowest)?;
                    Some(expr)
                } else {
                    None
                };
                expect(c, &mut lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
                let loc = name.loc;
                if let Some(name) = module.add_global_vars(
                    GlobalVar {
                        name,
                        r#type,
                        expr,
                    },
                    c,
                ) {
                    c.compiler_error::<()>(
                        format!("{}: Redefinition of {}", loc, name.get_str(c),),
                    );
                };
            }
            TokenKind::Keyword if token.source == "const" => {
                let name = expect(
                    c,
                    &mut lexer,
                    TokenKind::Identifier,
                    Some("Expected variable name"),
                )?;
                expect(c, &mut lexer, TokenKind::Assign, Some("Expected `=`"))?;
                let expr = parse_expr(c, &mut lexer, &[TokenKind::SemiColon], Precedence::Lowest)?;
                expect(c, &mut lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
                let loc = name.loc;
                if let Some(name) = module.add_consts(Const { name, expr }, c) {
                    c.compiler_error::<()>(format!(
                        "{}: Redefinition of {}",
                        loc,
                        name.get_str(&c),
                    ));
                };
            }
            _ => c.compiler_error(unexpected_token(
                token,
                Some("Expected a top level definition"),
            ))?,
        }
    }

    c.add_module(file_path, module);
    Ok(())
}

fn parse_extern_fn<'src>(
    c: &'src Compiler,
    lexer: &mut PeekableLexer<'src>,
) -> Result<ExternFunc<'src>, ()> {
    let token = lexer.next_token();
    if token.source != "fn" {
        c.compiler_error(unexpected_token(token, Some("Expect `fn` after `extern`")))?;
    }
    let name = expect(
        c,
        lexer,
        TokenKind::Identifier,
        Some("Expected function name"),
    )?;
    let mut args = vec![];
    let mut ret_type = None;
    let mut is_variadic = false;

    expect(c, lexer, TokenKind::OpenParen, Some("Expected `(`"))?;
    loop {
        let ptoken = lexer.peek_token();
        match ptoken.kind {
            TokenKind::CloseParen => {
                break;
            }
            TokenKind::Comma => {
                lexer.next_token();
            }
            _ => {
                if inspect(c, lexer, &[TokenKind::Splat])? {
                    lexer.next_token();
                    is_variadic = true;
                    break;
                }
                let r#type = parse_type(c, lexer)?;
                args.push(r#type);
            }
        }
    }

    expect(c, lexer, TokenKind::CloseParen, None::<&str>)?;
    if inspect(c, lexer, &[TokenKind::Arrow])? {
        lexer.next_token();
        ret_type = Some(parse_type(c, lexer)?);
    }

    expect(c, lexer, TokenKind::SemiColon, Some("Expected `;`"))?;

    Ok(ExternFunc {
        name,
        args,
        ret_type,
        is_variadic,
    })
}

fn parse_fn<'src>(c: &'src Compiler, lexer: &mut PeekableLexer<'src>) -> Result<Func<'src>, ()> {
    let name = expect(
        c,
        lexer,
        TokenKind::Identifier,
        Some("Expected function name"),
    )?;
    let mut args = vec![];
    let mut ret_type = None;

    expect(c, lexer, TokenKind::OpenParen, Some("Expected `(`"))?;
    loop {
        let ptoken = lexer.peek_token();
        match ptoken.kind {
            TokenKind::CloseParen => {
                lexer.next_token();
                if inspect(c, lexer, &[TokenKind::Arrow])? {
                    lexer.next_token();
                    ret_type = Some(parse_type(c, lexer)?);
                }
                break;
            }
            TokenKind::Comma => {
                lexer.next_token();
            }
            TokenKind::Identifier => {
                let name = lexer.next_token();
                let r#type = parse_type(c, lexer)?;
                args.push((name, r#type));
            }
            _ => {
                let token = lexer.next_token();
                c.compiler_error(unexpected_token(token, Some("Expected argument")))?;
            }
        }
    }

    let body = parse_stmt(c, lexer)?;

    Ok(Func {
        name,
        args,
        ret_type,
        body,
    })
}

fn parse_stmt<'src>(c: &'src Compiler, lexer: &mut PeekableLexer<'src>) -> Result<Stmt<'src>, ()> {
    let ptoken = lexer.peek_token();
    match ptoken.kind {
        TokenKind::Keyword if ptoken.source == "return" => {
            let loc = lexer.next_token().loc;
            let expr = if inspect(c, lexer, &[TokenKind::SemiColon])? {
                None
            } else {
                Some(parse_expr(
                    c,
                    lexer,
                    &[TokenKind::SemiColon],
                    Precedence::Lowest,
                )?)
            };
            expect(c, lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
            return Ok(Stmt::Return { loc, expr });
        }
        TokenKind::Keyword if ptoken.source == "var" => {
            lexer.next_token();
            let name = expect(
                c,
                lexer,
                TokenKind::Identifier,
                Some("Expected variable name"),
            )?;
            let r#type = parse_type(c, lexer)?;
            let expr = if inspect(c, lexer, &[TokenKind::Assign])? {
                lexer.next_token();
                Some(parse_expr(
                    c,
                    lexer,
                    &[TokenKind::SemiColon],
                    Precedence::Lowest,
                )?)
            } else {
                None
            };
            expect(c, lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
            return Ok(Stmt::VarDecl {
                name,
                r#type,
                expr,
            });
        }
        TokenKind::Keyword if ptoken.source == "while" => {
            let loc = lexer.next_token().loc;
            expect(c, lexer, TokenKind::OpenParen, Some("Expected `(`"))?;
            let cond = parse_expr(c, lexer, &[TokenKind::CloseParen], Precedence::Lowest)?;
            expect(c, lexer, TokenKind::CloseParen, Some("Expected `)`"))?;
            let body = parse_stmt(c, lexer)?;
            return Ok(Stmt::While {
                loc,
                cond,
                body: Box::new(body),
            });
        }
        TokenKind::Keyword if ptoken.source == "if" => {
            let loc = lexer.next_token().loc;
            expect(c, lexer, TokenKind::OpenParen, Some("Expected `(`"))?;
            let cond = parse_expr(c, lexer, &[TokenKind::CloseParen], Precedence::Lowest)?;
            expect(c, lexer, TokenKind::CloseParen, Some("Expected `)`"))?;
            let true_branch = parse_stmt(c, lexer)?;
            let else_branch = if lexer.peek_token().source == "else" {
                lexer.next_token();
                let else_branch = parse_stmt(c, lexer)?;
                Some(Box::new(else_branch))
            } else {
                None
            };
            return Ok(Stmt::If {
                loc,
                cond,
                true_branch: Box::new(true_branch),
                else_branch,
            });
        }
        TokenKind::OpenBrace => {
            let loc = lexer.next_token().loc;
            let mut body = vec![];
            loop {
                let ptoken = lexer.peek_token();
                match ptoken.kind {
                    TokenKind::CloseBrace => {
                        lexer.next_token();
                        break;
                    }
                    _ => {
                        let stmt = parse_stmt(c, lexer)?;
                        body.push(stmt);
                    }
                }
            }
            return Ok(Stmt::Block(loc, body));
        }
        _ => {
            let loc = ptoken.loc;
            let expr = parse_expr(
                c,
                lexer,
                &[TokenKind::SemiColon, TokenKind::Assign],
                Precedence::Lowest,
            )?;
            if next_token_is(c, lexer, &[TokenKind::Assign])? {
                let loc = lexer.next_token().loc;
                let rhs = parse_expr(c, lexer, &[TokenKind::SemiColon], Precedence::Lowest)?;
                expect(c, lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
                return Ok(Stmt::Assign {
                    loc,
                    lhs: expr,
                    rhs,
                });
            }
            expect(c, lexer, TokenKind::SemiColon, Some("Expected `;`"))?;
            return Ok(Stmt::Expr { loc, expr });
        } // _ => {
          //     let token = lexer.next_token();
          //     unexpected_token(token, Some("This is not a valid statment!"))?
          // }
    }
}

fn parse_expr<'src>(
    c: &'src Compiler,
    lexer: &mut PeekableLexer<'src>,
    stop: &[TokenKind],
    precedence: Precedence,
) -> Result<Expr<'src>, ()> {
    let mut curr_precedence = precedence;
    let left_token = lexer.next_token();
    let mut left = match left_token.kind {
        TokenKind::IntegerNumber => {
            curr_precedence = Precedence::Highest;
            Expr::IntLit(left_token.loc, left_token.source.parse().unwrap())
        }
        TokenKind::StringLiteral => {
            curr_precedence = Precedence::Highest;
            Expr::StrLit(left_token)
        }
        TokenKind::Identifier => {
            curr_precedence = Precedence::Highest;
            if matches!(left_token.source, "true" | "false") {
                Expr::BoolLit(left_token.loc, left_token.source.parse().unwrap())
            } else {
                Expr::Ident(left_token)
            }
        }
        TokenKind::Asterisk => {
            curr_precedence = Precedence::RefDeref;
            let expr = parse_expr(c, lexer, stop, Precedence::Lowest)?;
            Expr::Deref(left_token.loc, Box::new(expr))
        }
        TokenKind::Ampersand => {
            curr_precedence = Precedence::RefDeref;
            let expr = parse_expr(c, lexer, stop, Precedence::Lowest)?;
            Expr::Ref(left_token.loc, Box::new(expr))
        }
        TokenKind::OpenParen => {
            curr_precedence = Precedence::Highest;
            let expr = parse_expr(c, lexer, &[TokenKind::CloseParen], Precedence::Lowest)?;
            expect(c, lexer, TokenKind::CloseParen, Some("Expected `)`"))?;
            expr
        }
        TokenKind::Keyword if left_token.source == "syscall" => {
            curr_precedence = Precedence::Call;
            expect(c, lexer, TokenKind::OpenParen, Some("Expected `(`"))?;
            let args = collect_args(c, lexer)?;
            Expr::Syscall {
                loc: left_token.loc,
                args,
            }
        }
        _ => c.compiler_error(unexpected_token(
            left_token,
            Some("This is not a valid expr!"),
        ))?,
    };
    loop {
        let ptoken = lexer.peek_token();
        match (&left, ptoken.kind) {
            (Expr::Ident(left_token), TokenKind::OpenParen) => {
                curr_precedence = Precedence::Call;
                let loc = lexer.next_token().loc;
                let args = collect_args(c, lexer)?;
                left = Expr::Call {
                    loc,
                    caller: Box::new(left),
                    args,
                };
            }
            (_, TokenKind::OpenBracket) => {
                curr_precedence = Precedence::Call;
                let loc = lexer.next_token().loc;
                let base = left;
                let index = parse_expr(c, lexer, &[TokenKind::CloseBracket], Precedence::Lowest)?;
                expect(c, lexer, TokenKind::CloseBracket, Some("Expected `]`"))?;
                left = Expr::Index {
                    loc,
                    base: Box::new(base),
                    index: Box::new(index),
                };
            }
            _ => break,
        }
    }
    while !next_token_is(c, lexer, stop)? && (precedence < curr_precedence) {
        let token = lexer.next_token();
        curr_precedence = if let Some(precedence) = Precedence::from_token_kind(&token.kind) {
            precedence
        } else {
            c.compiler_error(unexpected_token(token, Some("Expected operator.")))?
        };
        let right = parse_expr(c, lexer, stop, curr_precedence)?;
        left = Expr::Binop {
            operator: token,
            lhs: Box::new(left),
            rhs: Box::new(right),
        }
    }
    Ok(left)
}

fn collect_args<'src>(
    c: &'src Compiler,
    lexer: &mut PeekableLexer<'src>,
) -> Result<Vec<Expr<'src>>, ()> {
    let mut args = vec![];
    loop {
        let ptoken = lexer.peek_token();
        match ptoken.kind {
            TokenKind::CloseParen => {
                lexer.next_token();
                break;
            }
            TokenKind::Comma => {
                lexer.next_token();
            }
            _ => {}
        }
        let arg = parse_expr(
            c,
            lexer,
            &[TokenKind::CloseParen, TokenKind::Comma],
            Precedence::Lowest,
        )?;
        args.push(arg);
    }
    Ok(args)
}

fn parse_type<'src>(c: &'src Compiler, lexer: &mut PeekableLexer<'src>) -> Result<Type<'src>, ()> {
    let token = lexer.next_token();
    match token.kind {
        TokenKind::Identifier => Ok(Type::Name(token)),
        TokenKind::Asterisk => Ok(Type::PtrTo(Box::new(parse_type(c, lexer)?))),
        TokenKind::OpenBracket => {
            let expr = parse_expr(c, lexer, &[TokenKind::CloseBracket], Precedence::Lowest)?;
            expect(c, lexer, TokenKind::CloseBracket, None::<&str>)?;
            let ty = parse_type(c, lexer)?;
            Ok(Type::Array(expr, Box::new(ty)))
        }
        _ => c.compiler_error(unexpected_token(token, Some("This is not a valid type!"))),
    }
}

fn next_token_is<'src>(
    c: &'src Compiler,
    lexer: &mut PeekableLexer<'src>,
    kind: &[TokenKind],
) -> Result<bool, ()> {
    let token = lexer.peek_token();
    if token.is_eof() {
        c.compiler_error(unexpected_end_of_file(token))?;
    }
    Ok(kind.contains(&token.kind))
}

fn unexpected_token(token: Token<'_>, msg: Option<impl ToString>) -> String {
    if let Some(msg) = msg {
        AppError::ParseError {
            path: token.loc.to_string(),
            error: format!("Unexpected token {}, {}", token, msg.to_string()),
        }
    } else {
        AppError::ParseError {
            path: token.loc.to_string(),
            error: format!("Unexpected token {}", token),
        }
    }
    .to_string()
}

fn undefined<T>(token: Token<'_>) -> Result<T, AppError> {
    Err(AppError::ParseError {
        path: token.loc.to_string(),
        error: format!("Udefined name {}", token),
    })
}

fn inspect<'src>(
    c: &'src Compiler,
    lexer: &mut PeekableLexer<'src>,
    kinds: &[TokenKind],
    // msg: Option<impl ToString>,
) -> Result<bool, ()> {
    let token = lexer.peek_token();
    if token.is_eof() {
        c.compiler_error(unexpected_end_of_file(token))?;
    }
    Ok(kinds.contains(&token.kind))
}

fn unexpected_end_of_file(token: &Token<'_>) -> String {
    AppError::ParseError {
        path: token.loc.to_string(),
        error: format!("Unexpected end of file"),
    }
    .to_string()
}

fn expect<'src>(
    c: &'src Compiler,
    lexer: &mut PeekableLexer<'src>,
    kind: TokenKind,
    msg: Option<impl ToString>,
) -> Result<Token<'src>, ()> {
    let token = lexer.next_token();
    if token.kind == kind {
        return Ok(token);
    }
    c.compiler_error(unexpected_token(token, msg))
}
