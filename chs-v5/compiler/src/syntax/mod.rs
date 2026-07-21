use std::path::Path;

use lex_just_parse::lexer::*;
use lex_just_parse::parser::*;
use lex_just_parse::try_parse;

use crate::diag::*;
use crate::syntax::ast::*;

pub mod ast;

pub fn parse_file(
    file_path: &Path,
    source: &str,
    reporter: &mut DiagnosticReporter,
) -> ChsResult<FileAst> {
    let mut lex = Lexer::new(file_path.to_string_lossy(), source).with_keywords(&[
        "fn", "var", "if", "else", "for", "return", "in", "true", "false", "break", "continue",
        "foreach", "import", "struct", "enum", "null", "defer", "switch", "cast", "autocast", "type",
    ]);

    if let Parser::Success(lex, items) = many(&mut lex, |lex| parse_decl(lex, reporter)) {
        if !lex.peek().is_eof() {
            let p = lex.peek();
            reporter.report(
                p.loc,
                format!("Unexpected token at top level: {}", p.source()),
            );
            return Err(());
        }
        return Ok(FileAst { items });
    }
    Err(())
}

#[derive(Debug)]
pub struct FileAst {
    pub items: Vec<FileItem>,
}

fn parse_decl<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FileItem, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::Keyword if token.source() == "fn" => {
            lex.next();
            let f = try_parse!(lex, parse_fn(lex, reporter));
            Parser::Success(lex, FileItem::FunctionDecl(f))
        }
        TokenKind::Keyword if token.source() == "struct" => {
            lex.next();
            let s = try_parse!(lex, parse_struct(lex, reporter));
            Parser::Success(lex, FileItem::Struct(s))
        }
        TokenKind::Keyword if token.source() == "enum" => {
            lex.next();
            let e = try_parse!(lex, parse_enum(lex, reporter));
            Parser::Success(lex, FileItem::Enum(e))
        }
        TokenKind::Keyword if token.source() == "import" => {
            lex.next();
            let string_tok = lex.next();
            if string_tok.kind != TokenKind::StringLiteral {
                reporter.report(string_tok.loc, "Expected string literal after import");
                return Parser::Fail(lex, ());
            }
            Parser::Success(lex, FileItem::Import(ImportDecl { path: string_tok }))
        }
        TokenKind::Keyword if token.source() == "type" => {
            lex.next();
            let name = try_parse!(lex, parse_identifier(lex));
            let is_distinct = if lex.peek().kind == TokenKind::Directive && lex.peek().source() == "#distinct" {
                lex.next(); // Consume "#distinct"
                true
            } else {
                false
            };
            let base_type = try_parse!(lex, parse_type(lex, reporter));
            Parser::Success(
                lex,
                FileItem::TypeDecl(TypeDecl {
                    name,
                    is_distinct,
                    base_type,
                }),
            )
        }
        TokenKind::Directive => {
            let token = lex.next();
            match token.source() {
                // "#run" => {
                //     let block = try_parse!(lex, parse_block_stmt(lex, reporter));
                //     return Parser::Success(lex, FileItem::Directive(Directive::Run(block)));
                // }
                "#operator" => {
                    let mut name = lex.next();
                    let op = match name.kind {
                        TokenKind::Plus => Op::Add,
                        TokenKind::Minus => Op::Sub,
                        TokenKind::Asterisk => Op::Mul,
                        TokenKind::Slash => Op::Div,
                        TokenKind::Mod => Op::Mod,
                        TokenKind::Lt => Op::Lt,
                        TokenKind::LtEq => Op::LtEq,
                        TokenKind::Gt => Op::Gt,
                        TokenKind::GtEq => Op::GtEq,
                        TokenKind::EqEq => Op::Eq,
                        TokenKind::NotEq => Op::NotEq,
                        _ => {
                            reporter.report(lex.peek().loc, "expected operator not supported");
                            return Parser::Fail(lex, ());
                        }
                    };
                    name.source = TokenSource(op.get_name());

                    let (parameters, va_args) = try_parse!(lex, parse_parameters(lex, reporter));

                    if va_args {
                        reporter.report(
                            lex.peek().loc,
                            "operator overload cannot have variadic arguments",
                        );
                        return Parser::Fail(lex, ());
                    }

                    if op.is_binary() && parameters.len() != 2 {
                        reporter.report(
                            lex.peek().loc,
                            "operator overload of binary must have 2 arguments",
                        );
                        return Parser::Fail(lex, ());
                    }

                    if op.is_unary() && parameters.len() != 1 {
                        reporter.report(
                            lex.peek().loc,
                            "operator overload of unary must have 1 argument",
                        );
                        return Parser::Fail(lex, ());
                    }

                    let mut return_type = None;
                    if lex.peek().kind == TokenKind::Arrow {
                        lex.next();
                        let typ = try_parse!(lex, parse_type(lex, reporter));
                        return_type = Some(typ);
                    }

                    let signature = FunctionSignature {
                        name,
                        parameters,
                        return_type,
                        va_args,
                    };

                    let directives = Vec::new();
                    while lex.peek().kind == TokenKind::Directive {
                        let directive = lex.next();
                        match directive.source() {
                            _ => {
                                reporter.report(
                                    directive.loc,
                                    format!("unknown operator directive: {}", directive.source()),
                                );
                                return Parser::Fail(lex, ());
                            }
                        }
                    }

                    let mut body = None;
                    if lex.peek().kind == TokenKind::OpenCurly {
                        let (l, b) = try_parse!(parse_block_stmt(lex, reporter));
                        lex = l;
                        body = Some(b);
                    }

                    let f = FunctionDecl {
                        signature,
                        generic_params: None,
                        directives,
                        body,
                        resolved_name: None,
                    };
                    return Parser::Success(lex, FileItem::FunctionDecl(f));
                }
                "#library" => {
                    let name = try_parse!(lex, parse_identifier(lex));
                    try_parse!(lex, expect(lex, TokenKind::OpenCurly, reporter));

                    let mut link_name = None;
                    let mut kind = None;

                    while lex.peek().kind != TokenKind::CloseCurly && lex.peek().kind != TokenKind::EOF {
                        let key_tok = try_parse!(lex, parse_identifier(lex));
                        try_parse!(lex, expect(lex, TokenKind::Eq, reporter));
                        let val_tok = try_parse!(lex, parse_string_literal(lex));
                        let val_str = val_tok.source().trim_matches('"').to_string();

                        match key_tok.source() {
                            "link_name" => {
                                link_name = Some(val_str);
                            }
                            "kind" => {
                                let k = match val_str.as_str() {
                                    "static" => LibraryKind::Static,
                                    "dynlib" => LibraryKind::Dynlib,
                                    s => {
                                        reporter.report(
                                            val_tok.loc,
                                            format!("unexpected library kind `{}`", s),
                                        );
                                        return Parser::Fail(lex, ());
                                    }
                                };
                                kind = Some(k);
                            }
                            other => {
                                reporter.report(
                                    key_tok.loc,
                                    format!("unexpected library attribute `{}`", other),
                                );
                                return Parser::Fail(lex, ());
                            }
                        }

                        if lex.peek().kind == TokenKind::Comma {
                            lex.next();
                        }
                    }

                    try_parse!(lex, expect(lex, TokenKind::CloseCurly, reporter));

                    let link_name = if let Some(ln) = link_name {
                        ln
                    } else {
                        reporter.report(name.loc, "library missing 'link_name' attribute");
                        return Parser::Fail(lex, ());
                    };

                    let kind = kind.unwrap_or(LibraryKind::Dynlib);

                    let library = Library { link_name, kind };
                    return Parser::Success(
                        lex,
                        FileItem::Directive(Directive::Library { name, library }),
                    );
                }
                _ => (),
            }
            reporter.report(
                token.loc,
                format!("Unknown top-level directive: {}", token.source()),
            );
            Parser::Fail(lex, ())
        }
        TokenKind::EOF => Parser::Fail(lex, ()),
        _ => {
            let p = lex.peek();
            reporter.report(p.loc, format!("unexpected token `{}`", p.source()));
            Parser::Fail(lex, ())
        }
    }
}

fn parse_enum<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, EnumDecl, ()> {
    let name = try_parse!(lex, parse_identifier(lex));

    let generic_params = if lex.peek().kind == TokenKind::OpenBracket {
        let gp = try_parse!(lex, parse_generic_params(lex, reporter));
        Some(gp)
    } else {
        None
    };

    let inner_type = if lex.peek().kind == TokenKind::Colon {
        lex.next();
        Some(try_parse!(lex, parse_type(lex, reporter)))
    } else {
        None
    };

    let mut directives = Vec::new();
    while lex.peek().kind == TokenKind::Directive {
        let value = lex.next();
        match value.source() {
            "#test" => directives.push(EnumDirective::Test(value.loc)),
            _ => {
                reporter.report(
                    value.loc,
                    format!("unknown enum directive: {}", value.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    let variants = try_parse!(lex, parse_enum_variants(lex, reporter));

    Parser::Success(
        lex,
        EnumDecl {
            name,
            generic_params,
            directives,
            inner_type,
            variants,
        },
    )
}

fn parse_enum_variants<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<EnumVariant>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let variants = try_parse!(
        lex,
        sep_by(lex, |l| parse_enum_variant(l, reporter), parse_comma)
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, variants)
}

fn parse_generic_params<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Token>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenBracket {
        reporter.report(
            token.loc,
            format!("expected OpenBracket, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    let mut params = Vec::new();
    loop {
        let peek = lex.peek();
        if peek.kind == TokenKind::CloseBracket {
            lex.next();
            break;
        }
        let dollar_peek = lex.peek().clone();
        try_parse!(lex, expect(lex, TokenKind::Dollar, reporter));
        let ident_tok = try_parse!(lex, parse_identifier(lex));
        let name = format!("${}", ident_tok.source());
        let param = Token::new(
            TokenKind::Identifier,
            dollar_peek.loc,
            TokenSource::from(name.as_str()),
        );
        params.push(param);
        let next = lex.peek();
        if next.kind == TokenKind::Comma {
            lex.next();
        } else if next.kind == TokenKind::CloseBracket {
            lex.next();
            break;
        } else {
            reporter.report(
                next.loc,
                format!("expected Comma or CloseBracket, found {:?}", next.kind),
            );
            return Parser::Fail(lex, ());
        }
    }
    Parser::Success(lex, params)
}

fn parse_struct<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, StructDecl, ()> {
    let name = try_parse!(lex, parse_identifier(lex));

    let generic_params = if lex.peek().kind == TokenKind::OpenBracket {
        let gp = try_parse!(lex, parse_generic_params(lex, reporter));
        Some(gp)
    } else {
        None
    };

    let mut directives = Vec::new();
    while lex.peek().kind == TokenKind::Directive {
        let value = lex.next();
        match value.source() {
            "#test" => directives.push(StructDirective::Test(value.loc)),
            _ => {
                reporter.report(
                    value.loc,
                    format!("unknown struct directive: {}", value.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    let fields = try_parse!(lex, parse_fields(lex, reporter));

    Parser::Success(
        lex,
        StructDecl {
            name,
            generic_params,
            directives,
            fields,
        },
    )
}

fn parse_fields<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Field>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let fields = try_parse!(
        lex,
        sep_by(lex, |l| parse_var_type_value(l, reporter), parse_comma)
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, fields)
}

fn parse_fn<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FunctionDecl, ()> {
    let name = try_parse!(lex, parse_identifier(lex));

    let generic_params = if lex.peek().kind == TokenKind::OpenBracket {
        let gp = try_parse!(lex, parse_generic_params(lex, reporter));
        Some(gp)
    } else {
        None
    };

    let (parameters, va_args) = try_parse!(lex, parse_parameters(lex, reporter));

    let mut return_type = None;
    if lex.peek().kind == TokenKind::Arrow {
        lex.next();
        let typ = try_parse!(lex, parse_type(lex, reporter));
        return_type = Some(typ);
    }

    let mut directives = Vec::new();
    let mut is_foreign = false;
    while lex.peek().kind == TokenKind::Directive {
        let directive = lex.next();
        match directive.source() {
            "#foreign" => {
                let name = try_parse!(lex, parse_identifier(lex));
                directives.push(FunctionDirective::Foreign(directive.loc, name));
                is_foreign = true;
            }
            "#link_name" => {
                let name = try_parse!(lex, parse_string_literal(lex));
                directives.push(FunctionDirective::LinkName(name));
            }
            "#private" => {
                directives.push(FunctionDirective::Private);
            }
            _ => {
                reporter.report(
                    directive.loc,
                    format!("unknown function directive: {}", directive.source()),
                );
                return Parser::Fail(lex, ());
            }
        }
    }

    if va_args && !is_foreign {
        reporter.report(
            name.loc,
            "non foreign functions cannot have variadic arguments",
        );
        return Parser::Fail(lex, ());
    }

    let signature = FunctionSignature {
        name,
        parameters,
        return_type,
        va_args,
    };

    let mut body = None;
    if lex.peek().kind == TokenKind::OpenCurly {
        let (l, b) = try_parse!(parse_block_stmt(lex, reporter));
        lex = l;
        body = Some(b);
    }

    Parser::Success(
        lex,
        FunctionDecl {
            signature,
            generic_params,
            directives,
            body,
            resolved_name: None,
        },
    )
}

fn parse_block_stmt<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, BlockStmt, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    let mut block = BlockStmt::new(token.loc);

    block.stmts = try_parse!(
        lex,
        sep_by(
            lex,
            |lex| {
                if lex.peek().kind == TokenKind::CloseCurly {
                    return Parser::Fail(lex, ());
                }
                parse_stmt(lex, reporter)
            },
            parse_semicolon
        )
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, block)
}

fn parse_stmt<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Stmt, ()> {
    let ptoken = lex.peek().clone();
    match ptoken.kind {
        TokenKind::Identifier => {
            let ident = try_parse!(lex, parse_identifier(lex));
            let peek = lex.peek();
            match peek.kind {
                TokenKind::OpenParen => {
                    let loc = peek.loc;
                    let arguments = try_parse!(lex, parse_arguments(lex, reporter));
                    Parser::Success(
                        lex,
                        Stmt::Call(CallExpr {
                            loc,
                            callee: Box::new(Expr::new(ExprKind::Identifier(ident))),
                            arguments,
                            resolved_name: None,
                        }),
                    )
                }
                _ => {
                    // e.g. `arr = 5` or `arr.foo`
                    let mut left = Expr::new(ExprKind::Identifier(ident.clone()));
                    loop {
                        let p = token_to_precedence(lex.peek());
                        if Precedence::Lowest >= p {
                            break;
                        }
                        let next_left = try_parse!(lex, parse_infix(lex, left, reporter));
                        left = next_left;
                    }

                    if lex.peek().kind == TokenKind::Comma {
                        let mut exprs = vec![left];
                        let mut last_loc = ident.loc;
                        while lex.peek().kind == TokenKind::Comma {
                            lex.next(); // consume comma
                            let (next_lex, expr) = try_parse!(parse_expr_with_precedence(
                                lex,
                                Precedence::Assignment,
                                reporter
                            ));
                            lex = next_lex;
                            last_loc = expr.loc();
                            exprs.push(expr);
                        }
                        let left_tuple = Expr::new(ExprKind::Tuple(exprs, last_loc));

                        let op_token = lex.peek().clone();
                        if let Some(assign_kind) = token_to_assign_kind(op_token.kind) {
                            lex.next(); // consume assign operator
                            let (next_lex, first_right) = try_parse!(parse_expr_with_precedence(
                                lex,
                                Precedence::Lowest,
                                reporter
                            ));
                            let mut current_lex = next_lex;

                            let right = if current_lex.peek().kind == TokenKind::Comma {
                                let mut rexprs = vec![first_right];
                                let mut rlast_loc = op_token.loc;
                                while current_lex.peek().kind == TokenKind::Comma {
                                    current_lex.next();
                                    let (next_lex, expr) = try_parse!(parse_expr_with_precedence(
                                        current_lex,
                                        Precedence::Lowest,
                                        reporter
                                    ));
                                    current_lex = next_lex;
                                    rlast_loc = expr.loc();
                                    rexprs.push(expr);
                                }
                                Expr::new(ExprKind::Tuple(rexprs, rlast_loc))
                            } else {
                                first_right
                            };

                            let assign_expr = Expr::new(ExprKind::Assign(AssignExpr {
                                left: Box::new(left_tuple),
                                assign_kind,
                                right: Box::new(right),
                            }));
                            Parser::Success(current_lex, Stmt::ExprStmt(assign_expr))
                        } else {
                            reporter
                                .report(op_token.loc, "expected assignment operator after tuple");
                            Parser::Fail(lex, ())
                        }
                    } else {
                        Parser::Success(lex, Stmt::ExprStmt(left))
                    }
                }
            }
        }
        TokenKind::Keyword if ptoken.source() == "var" => {
            // var name1, name2, ... [: type] = value;
            lex.next();
            let mut names = Vec::new();
            let first_name = try_parse!(lex, parse_identifier(lex));
            names.push(first_name);

            while lex.peek().kind == TokenKind::Comma {
                lex.next();
                let name = try_parse!(lex, parse_identifier(lex));
                names.push(name);
            }

            let mut var_type = None;
            if lex.peek().kind == TokenKind::Colon {
                lex.next();
                let typ = try_parse!(lex, parse_type(lex, reporter));
                var_type = Some(typ);
            }

            try_parse!(lex, expect(lex, TokenKind::Eq, reporter));

            let expr = try_parse!(lex, parse_expr(lex, reporter));

            let stmt = Stmt::VarDecl(VarDeclStmt {
                names,
                var_type,
                expr,
            });
            Parser::Success(lex, stmt)
        }
        TokenKind::OpenCurly => {
            let body = try_parse!(lex, parse_block_stmt(lex, reporter));
            Parser::Success(lex, Stmt::Block(body))
        }
        TokenKind::Keyword if ptoken.source() == "if" => {
            lex.next();
            let cond = try_parse!(lex, parse_expr(lex, reporter));
            let true_body = try_parse!(lex, parse_block_stmt(lex, reporter));
            let peek = lex.peek();
            let stmt = if peek.kind == TokenKind::Keyword && peek.source() == "else" {
                lex.next();
                let peek = lex.peek();
                // `else if` hack
                let false_body = if peek.kind == TokenKind::Keyword && peek.source() == "if" {
                    let next_if = try_parse!(lex, parse_stmt(lex, reporter));
                    BlockStmt {
                        loc: next_if.loc(),
                        stmts: vec![next_if],
                    }
                } else {
                    try_parse!(lex, parse_block_stmt(lex, reporter))
                };

                Stmt::IfStmt(IfStmt::IfElse {
                    cond,
                    true_body,
                    false_body,
                })
            } else {
                Stmt::IfStmt(IfStmt::If { cond, true_body })
            };
            Parser::Success(lex, stmt)
        }
        TokenKind::Keyword if ptoken.source() == "foreach" => {
            lex.next();
            if lex.peek().kind == TokenKind::Identifier {
                let var_name = lex.next();
                let peek = lex.peek();
                if peek.kind == TokenKind::Keyword && peek.source() == "in" {
                    lex.next();
                    let iter_expr = try_parse!(lex, parse_expr(lex, reporter));
                    let body = try_parse!(lex, parse_block_stmt(lex, reporter));
                    return Parser::Success(
                        lex,
                        Stmt::ForEach(ForEachStmt {
                            var_name,
                            iter_expr,
                            body,
                        }),
                    );
                }
            }
            reporter.report(
                ptoken.loc,
                "Expect Identifier: foreach statements must use this format `foreach x in xs`",
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Keyword if ptoken.source() == "for" => {
            lex.next();
            if lex.peek().kind == TokenKind::OpenCurly {
                let body = try_parse!(lex, parse_block_stmt(lex, reporter));
                return Parser::Success(lex, Stmt::ForStmt(ForStmt::ForLoop(body)));
            }
            let cond = try_parse!(lex, parse_expr(lex, reporter));
            let body = try_parse!(lex, parse_block_stmt(lex, reporter));
            Parser::Success(lex, Stmt::ForStmt(ForStmt::ForCond { cond, body }))
        }
        TokenKind::Keyword if ptoken.source() == "break" => {
            let token = lex.next();
            Parser::Success(lex, Stmt::Break(token.loc))
        }
        TokenKind::Keyword if ptoken.source() == "continue" => {
            let token = lex.next();
            Parser::Success(lex, Stmt::Continue(token.loc))
        }
        TokenKind::Keyword if ptoken.source() == "return" => {
            let loc = lex.next().loc;
            if lex.peek().kind == TokenKind::SemiColon {
                return Parser::Success(lex, Stmt::Return(loc, None));
            }
            let first_expr = try_parse!(lex, parse_expr(lex, reporter));
            if lex.peek().kind == TokenKind::Comma {
                let mut exprs = vec![first_expr];
                while lex.peek().kind == TokenKind::Comma {
                    lex.next();
                    let expr = try_parse!(lex, parse_expr(lex, reporter));
                    exprs.push(expr);
                }
                let tuple_expr = Expr::new(ExprKind::Tuple(exprs, loc));
                Parser::Success(lex, Stmt::Return(loc, Some(tuple_expr)))
            } else {
                Parser::Success(lex, Stmt::Return(loc, Some(first_expr)))
            }
        }
        TokenKind::Keyword if ptoken.source() == "defer" => {
            let loc = lex.next().loc;
            let inner_stmt = try_parse!(lex, parse_stmt(lex, reporter));
            Parser::Success(lex, Stmt::Defer(loc, Box::new(inner_stmt)))
        }
        TokenKind::Keyword if ptoken.source() == "switch" => {
            let loc = lex.next().loc;
            let cond = try_parse!(lex, parse_expr(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::OpenCurly, reporter));

            let mut branches = Vec::new();
            let mut default = None;

            while lex.peek().kind != TokenKind::CloseCurly && lex.peek().kind != TokenKind::EOF {
                let pattern = try_parse!(lex, parse_expr(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::Arrow, reporter));
                let body = try_parse!(lex, parse_stmt(lex, reporter));

                if lex.peek().kind == TokenKind::Comma || lex.peek().kind == TokenKind::SemiColon {
                    lex.next();
                }

                let is_wildcard = if let ExprKind::Identifier(ref ident) = pattern.kind {
                    ident.source() == "_"
                } else {
                    false
                };

                if is_wildcard {
                    if default.is_some() {
                        reporter.report(pattern.loc(), "Multiple default cases in switch");
                    }
                    default = Some(Box::new(body));
                } else {
                    branches.push(SwitchBranch { pattern, body });
                }
            }

            try_parse!(lex, expect(lex, TokenKind::CloseCurly, reporter));

            Parser::Success(
                lex,
                Stmt::Switch(SwitchStmt {
                    loc,
                    cond,
                    branches,
                    default,
                }),
            )
        }
        _ => {
            let first_expr = try_parse!(lex, parse_expr(lex, reporter));
            if lex.peek().kind == TokenKind::Comma {
                let mut exprs = vec![first_expr];
                let mut last_loc = ptoken.loc;
                while lex.peek().kind == TokenKind::Comma {
                    lex.next(); // consume comma
                    let (next_lex, expr) = try_parse!(parse_expr_with_precedence(
                        lex,
                        Precedence::Assignment,
                        reporter
                    ));
                    lex = next_lex;
                    last_loc = expr.loc();
                    exprs.push(expr);
                }
                let left_tuple = Expr::new(ExprKind::Tuple(exprs, last_loc));

                let op_token = lex.peek().clone();
                if let Some(assign_kind) = token_to_assign_kind(op_token.kind) {
                    lex.next(); // consume assign operator
                    let (next_lex, first_right) = try_parse!(parse_expr_with_precedence(
                        lex,
                        Precedence::Lowest,
                        reporter
                    ));
                    let mut current_lex = next_lex;

                    let right = if current_lex.peek().kind == TokenKind::Comma {
                        let mut rexprs = vec![first_right];
                        let mut rlast_loc = op_token.loc;
                        while current_lex.peek().kind == TokenKind::Comma {
                            current_lex.next();
                            let (next_lex, expr) = try_parse!(parse_expr_with_precedence(
                                current_lex,
                                Precedence::Lowest,
                                reporter
                            ));
                            current_lex = next_lex;
                            rlast_loc = expr.loc();
                            rexprs.push(expr);
                        }
                        Expr::new(ExprKind::Tuple(rexprs, rlast_loc))
                    } else {
                        first_right
                    };

                    let assign_expr = Expr::new(ExprKind::Assign(AssignExpr {
                        left: Box::new(left_tuple),
                        assign_kind,
                        right: Box::new(right),
                    }));
                    Parser::Success(current_lex, Stmt::ExprStmt(assign_expr))
                } else {
                    reporter.report(op_token.loc, "expected assignment operator after tuple");
                    Parser::Fail(lex, ())
                }
            } else {
                Parser::Success(lex, Stmt::ExprStmt(first_expr))
            }
        }
    }
}

fn parse_identifier<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, Token, ()> {
    let token = lex.peek();
    if token.kind != TokenKind::Identifier {
        return Parser::Fail(lex, ());
    }
    let token = lex.next();
    Parser::Success(lex, token)
}

fn parse_ellipsis<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    let token = lex.peek();
    if token.kind != TokenKind::Ellipsis {
        return Parser::Fail(lex, ());
    }
    lex.next();
    Parser::Success(lex, ())
}

fn parse_string_literal<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, Token, ()> {
    let token = lex.peek();
    if token.kind != TokenKind::StringLiteral {
        return Parser::Fail(lex, ());
    }
    let token = lex.next();
    Parser::Success(lex, token)
}

fn expect<'lex>(
    lex: RefLexer<'lex>,
    kind: TokenKind,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, (), ()> {
    let token = lex.next();
    if token.kind != kind {
        reporter.report(
            token.loc,
            format!("expected {:?}, found {:?}", kind, token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, ())
}

fn parse_struct_literal_fields<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<FieldInit>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenCurly {
        reporter.report(
            token.loc,
            format!("expected OpenCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let fields = try_parse!(
        lex,
        sep_by(
            lex,
            |mut lex| {
                let name = try_parse!(lex, parse_identifier(lex));
                if lex.peek().kind == TokenKind::Colon {
                    lex.next();
                }
                let value = try_parse!(lex, parse_expr(lex, reporter));
                Parser::Success(lex, FieldInit { name, value })
            },
            parse_comma
        )
    );
    let token = lex.next();
    if token.kind != TokenKind::CloseCurly {
        reporter.report(
            token.loc,
            format!("expected CloseCurly, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, fields)
}

fn parse_arguments<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Expr>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenParen {
        reporter.report(
            token.loc,
            format!("expected OpenParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let args = try_parse!(lex, sep_by(lex, |l| parse_expr(l, reporter), parse_comma));
    let token = lex.next();
    if token.kind != TokenKind::CloseParen {
        reporter.report(
            token.loc,
            format!("expected CloseParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, args)
}

#[allow(dead_code)]
fn parse_array<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Vec<Expr>, ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenBracket {
        reporter.report(
            token.loc,
            format!("expected OpenBracket, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }

    let array = try_parse!(lex, sep_by(lex, |l| parse_expr(l, reporter), parse_comma));
    let token = lex.next();
    if token.kind != TokenKind::CloseBracket {
        reporter.report(
            token.loc,
            format!("expected CloseBracket, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, array)
}

fn parse_var_type_value<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, VarTypeValue, ()> {
    let name = try_parse!(lex, parse_identifier(lex));
    if lex.peek().kind == TokenKind::Colon {
        lex.next();
    }
    let typ = try_parse!(lex, parse_type(lex, reporter));
    let value = if lex.peek().kind == TokenKind::Eq {
        lex.next();
        Some(try_parse!(lex, parse_expr(lex, reporter)))
    } else {
        None
    };
    Parser::Success(lex, VarTypeValue { name, typ, value })
}

fn parse_enum_variant<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, EnumVariant, ()> {
    let name = try_parse!(lex, parse_identifier(lex));
    let peek = lex.peek();
    if peek.kind == TokenKind::Eq {
        lex.next();
        let value = try_parse!(lex, parse_number(lex, reporter));
        Parser::Success(lex, EnumVariant::DefaultValue(name, value))
    }
    // else if peek.kind == TokenKind::OpenParen {
    //     lex.next(); // Consume '('
    //     let mut types = Vec::new();
    //     loop {
    //         if lex.peek().kind == TokenKind::CloseParen {
    //             break;
    //         }
    //         let ty = try_parse!(lex, parse_type(lex, reporter));
    //         types.push(ty);
    //         let next = lex.peek();
    //         if next.kind == TokenKind::Comma {
    //             lex.next();
    //         } else if next.kind == TokenKind::CloseParen {
    //             break;
    //         } else {
    //             reporter.report(
    //                 next.loc,
    //                 format!(
    //                     "expected Comma or CloseParen in variant payload, found {:?}",
    //                     next.kind
    //                 ),
    //             );
    //             return Parser::Fail(lex, ());
    //         }
    //     }
    //     try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
    //     Parser::Success(lex, EnumVariant::TuplePayload(name, types))
    // }
    // else if peek.kind == TokenKind::OpenCurly {
    //     let fields = try_parse!(lex, parse_fields(lex, reporter));
    //     Parser::Success(lex, EnumVariant::StructPayload(name, fields))
    // }
    else {
        Parser::Success(lex, EnumVariant::Name(name))
    }
}

fn parse_parameters<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, (Vec<FunctionParameter>, bool), ()> {
    let token = lex.next();
    if token.kind != TokenKind::OpenParen {
        reporter.report(
            token.loc,
            format!("expected OpenParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    let (parameters, va_args) = try_parse!(
        lex,
        sep_by(lex, |l| parse_var_type_value(l, reporter), parse_comma).and_then(
            |lex, parameters| {
                match parse_ellipsis(lex) {
                    Parser::Success(lex, _) => Parser::Success(lex, (parameters, true)),
                    Parser::Fail(lex, _) => Parser::Success(lex, (parameters, false)),
                }
            }
        )
    );

    let token = lex.next();
    if token.kind != TokenKind::CloseParen {
        reporter.report(
            token.loc,
            format!("expected CloseParen, found {:?}", token.kind),
        );
        return Parser::Fail(lex, ());
    }
    Parser::Success(lex, (parameters, va_args))
}

fn parse_comma<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::Comma {
        lex.next();
        return Parser::Success(lex, ());
    }
    Parser::Fail(lex, ())
}

fn parse_semicolon<'lex>(lex: RefLexer<'lex>) -> Parser<'lex, (), ()> {
    if lex.peek().kind == TokenKind::SemiColon {
        lex.next();
        return Parser::Success(lex, ());
    }
    Parser::Fail(lex, ())
}

fn parse_type<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Type, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::OpenParen => {
            let start_tok = lex.next();
            let mut types = Vec::new();
            if lex.peek().kind == TokenKind::CloseParen {
                lex.next();
                Parser::Success(lex, Type::Tuple(types, start_tok.loc))
            } else {
                loop {
                    let ty = try_parse!(lex, parse_type(lex, reporter));
                    types.push(ty);
                    let next = lex.peek();
                    if next.kind == TokenKind::Comma {
                        lex.next();
                    } else if next.kind == TokenKind::CloseParen {
                        lex.next();
                        break;
                    } else {
                        reporter.report(
                            next.loc,
                            format!(
                                "expected Comma or CloseParen in tuple type, found {:?}",
                                next.kind
                            ),
                        );
                        return Parser::Fail(lex, ());
                    }
                }
                if types.len() == 1 {
                    Parser::Success(lex, types.remove(0))
                } else {
                    Parser::Success(lex, Type::Tuple(types, start_tok.loc))
                }
            }
        }
        TokenKind::Asterisk => {
            lex.next();
            let mut pointer_count = 1;
            while lex.peek().kind == TokenKind::Asterisk {
                lex.next();
                pointer_count += 1;
            }
            let inner = try_parse!(lex, parse_type(lex, reporter));
            Parser::Success(lex, Type::Pointer(pointer_count, Box::new(inner)))
        }
        TokenKind::OpenBracket => {
            lex.next();
            if lex.peek().kind == TokenKind::CloseBracket {
                lex.next();
                let inner = try_parse!(lex, parse_type(lex, reporter));
                Parser::Success(lex, Type::Slice(Box::new(inner)))
            } else {
                let size_token = try_parse!(lex, parse_number(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::CloseBracket, reporter));
                let inner = try_parse!(lex, parse_type(lex, reporter));
                Parser::Success(lex, Type::Array(Box::new(inner), size_token.value as usize))
            }
        }
        TokenKind::Dollar => {
            let dollar_tok = lex.next();
            let ident_tok = try_parse!(lex, parse_identifier(lex));
            let name = format!("${}", ident_tok.source());
            let token = Token::new(
                TokenKind::Identifier,
                dollar_tok.loc,
                TokenSource::from(name.as_str()),
            );
            Parser::Success(lex, Type::Scalar(token))
        }
        TokenKind::Identifier => {
            let ident = lex.next();
            let mut base_type = Type::Scalar(ident);
            if lex.peek().kind == TokenKind::Dollar {
                lex.next(); // Consume '$'
                try_parse!(lex, expect(lex, TokenKind::OpenBracket, reporter));
                let mut type_args = Vec::new();
                loop {
                    let arg = try_parse!(lex, parse_type(lex, reporter));
                    type_args.push(arg);
                    let next = lex.peek();
                    if next.kind == TokenKind::Comma {
                        lex.next();
                    } else if next.kind == TokenKind::CloseBracket {
                        lex.next();
                        break;
                    } else {
                        reporter.report(
                            next.loc,
                            format!("expected Comma or CloseBracket, found {:?}", next.kind),
                        );
                        return Parser::Fail(lex, ());
                    }
                }
                base_type = Type::GenericInst(Box::new(base_type), type_args);
            }
            Parser::Success(lex, base_type)
        }
        TokenKind::Keyword if token.source() == "fn" => {
            let fn_tok = lex.next();
            let (parameters, _va_args) = try_parse!(lex, parse_parameters(lex, reporter));
            let mut return_type = None;
            if lex.peek().kind == TokenKind::Arrow {
                lex.next();
                let typ = try_parse!(lex, parse_type(lex, reporter));
                return_type = Some(Box::new(typ));
            }
            Parser::Success(
                lex,
                Type::FnPointer {
                    parameters,
                    return_type,
                    loc: fn_tok.loc,
                },
            )
        }
        _ => {
            reporter.report(
                token.loc,
                format!("expected type name, '*' or '[', found {:?}", token.kind),
            );
            Parser::Fail(lex, ())
        }
    }
}

fn parse_expr<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    parse_expr_with_precedence(lex, Precedence::Lowest, reporter)
}

fn parse_expr_with_precedence<'lex>(
    mut lex: RefLexer<'lex>,
    precedence: Precedence,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    let (l, mut left) = try_parse!(parse_prefix(lex, reporter));
    lex = l;

    loop {
        let ptoken = lex.peek();
        let p = token_to_precedence(ptoken);
        if precedence >= p {
            break;
        }

        let (l, next_left) = try_parse!(parse_infix(lex, left, reporter));
        lex = l;
        left = next_left;
    }

    Parser::Success(lex, left)
}

fn parse_prefix<'lex>(
    mut lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    let ptoken = lex.peek();
    match ptoken.kind {
        TokenKind::Number(_) => {
            let num_lit = try_parse!(lex, parse_number(lex, reporter));
            let expr = Expr::new(ExprKind::Integer(num_lit));
            Parser::Success(lex, expr)
        }
        TokenKind::RealNumber => {
            let float_lit = try_parse!(lex, parse_float(lex, reporter));
            let expr = Expr::new(ExprKind::Float(float_lit));
            Parser::Success(lex, expr)
        }
        TokenKind::Keyword if ptoken.source() == "null" => {
            let loc = lex.next().loc;
            Parser::Success(lex, Expr::new(ExprKind::Null(loc)))
        }
        TokenKind::Keyword if ptoken.source() == "autocast" => {
            let cast_tok = lex.next();
            let (lex, expr) =
                try_parse!(parse_expr_with_precedence(lex, Precedence::Neg, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::AutoCast(Box::new(expr), cast_tok.loc)),
            )
        }
        TokenKind::Keyword if ptoken.source() == "cast" => {
            let cast_tok = lex.next();
            try_parse!(lex, expect(lex, TokenKind::OpenParen, reporter));
            let target_type = try_parse!(lex, parse_type(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
            let (lex, expr) =
                try_parse!(parse_expr_with_precedence(lex, Precedence::Neg, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Cast(target_type, Box::new(expr), cast_tok.loc)),
            )
        }
        TokenKind::Dot => {
            let token = lex.next();
            reporter.report(
                token.loc,
                "prefix enum variants (implicit member expressions) are not supported yet",
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Directive => {
            let token = lex.next();
            if token.source() == "#type_info" && lex.peek().kind == TokenKind::OpenParen {
                lex.next(); // Consume '('
                let ty = try_parse!(lex, parse_type(lex, reporter));
                try_parse!(lex, expect(lex, TokenKind::CloseParen, reporter));
                return Parser::Success(
                    lex,
                    Expr::new(ExprKind::TypeInfo(Box::new(ty), token.loc)),
                );
            } else if token.source() == "#anycast" {
                if lex.peek().kind == TokenKind::OpenBracket {
                    let array = try_parse!(lex, parse_array(lex, reporter));
                    return Parser::Success(
                        lex,
                        Expr::new(ExprKind::AnyCast(AnyCastExpr::Array(array), token.loc)),
                    );
                }
                let expr = try_parse!(lex, parse_expr(lex, reporter));
                return Parser::Success(
                    lex,
                    Expr::new(ExprKind::AnyCast(
                        AnyCastExpr::Scalar(expr.into()),
                        token.loc,
                    )),
                );
            } else if token.source() == "#unsafe" {
                let expr = try_parse!(lex, parse_expr(lex, reporter));
                return Parser::Success(
                    lex,
                    Expr::new(ExprKind::Unsafe(Box::new(expr), token.loc)),
                );
            }
            reporter.report(
                token.loc,
                format!("unsupported directive '{}'", token.source()),
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Identifier => {
            let token = lex.next();
            let expr = Expr::new(ExprKind::Identifier(token));
            Parser::Success(lex, expr)
        }
        TokenKind::StringLiteral => {
            let token = lex.next();
            let expr = Expr::new(ExprKind::StringLiteral(token));
            Parser::Success(lex, expr)
        }
        TokenKind::Keyword if ptoken.source() == "true" || ptoken.source() == "false" => {
            let token = lex.next();
            let expr = Expr::new(ExprKind::Bool(BoolLiteral {
                loc: token.loc,
                value: token.source() == "true",
            }));
            Parser::Success(lex, expr)
        }
        TokenKind::OpenBracket => {
            let loc = ptoken.loc;
            let elements = try_parse!(lex, parse_array(lex, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Array(ArrayExpr {
                    loc,
                    elements,
                    type_hint: None,
                })),
            )
        }
        TokenKind::OpenParen => {
            let open_tok = lex.next();
            if lex.peek().kind == TokenKind::CloseParen {
                lex.next();
                Parser::Success(lex, Expr::new(ExprKind::Tuple(Vec::new(), open_tok.loc)))
            } else {
                let mut exprs = Vec::new();
                let mut has_comma = false;
                loop {
                    let expr = try_parse!(lex, parse_expr(lex, reporter));
                    exprs.push(expr);
                    let next = lex.peek();
                    if next.kind == TokenKind::Comma {
                        lex.next();
                        has_comma = true;
                    } else if next.kind == TokenKind::CloseParen {
                        lex.next();
                        break;
                    } else {
                        reporter.report(
                            next.loc,
                            format!(
                                "expected Comma or CloseParen in tuple expression, found {:?}",
                                next.kind
                            ),
                        );
                        return Parser::Fail(lex, ());
                    }
                }
                if exprs.len() == 1 && !has_comma {
                    Parser::Success(lex, exprs.remove(0))
                } else {
                    Parser::Success(lex, Expr::new(ExprKind::Tuple(exprs, open_tok.loc)))
                }
            }
        }
        TokenKind::Minus | TokenKind::Bang | TokenKind::Ampersand | TokenKind::Asterisk => {
            let token = lex.next();
            let op = match token.kind {
                TokenKind::Minus => Op::Neg,
                TokenKind::Bang => Op::Not,
                TokenKind::Ampersand => Op::Refer,
                TokenKind::Asterisk => Op::Deref,
                _ => unreachable!(),
            };
            let (lex, right) =
                try_parse!(parse_expr_with_precedence(lex, Precedence::Neg, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Unary(UnaryExpr {
                    op,
                    right: Box::new(right),
                })),
            )
        }
        TokenKind::EOF => {
            let token = lex.next();
            reporter.report(token.loc, "expected expression but got EOF");
            Parser::Fail(lex, ())
        }
        _ => Parser::Fail(lex, ()),
    }
}

fn parse_number<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, IntegerLiteral, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::Number(base) => {
            let token = lex.next();
            let value = match u64::from_str_radix(token.source(), base.radix()) {
                Ok(ok) => ok,
                Err(err) => {
                    reporter.report(token.loc, format!("invalid integer: {}", err));
                    return Parser::Fail(lex, ());
                }
            };
            Parser::Success(
                lex,
                IntegerLiteral {
                    loc: token.loc,
                    value,
                },
            )
        }
        _ => Parser::Fail(lex, ()),
    }
}

fn parse_float<'lex>(
    lex: RefLexer<'lex>,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, FloatLiteral, ()> {
    let token = lex.peek();
    match token.kind {
        TokenKind::RealNumber => {
            let token = lex.next();
            let value = match token.source().parse::<f64>() {
                Ok(ok) => ok,
                Err(err) => {
                    reporter.report(token.loc, format!("invalid float literal: {}", err));
                    return Parser::Fail(lex, ());
                }
            };
            Parser::Success(
                lex,
                FloatLiteral {
                    loc: token.loc,
                    value,
                },
            )
        }
        _ => Parser::Fail(lex, ()),
    }
}

fn token_to_assign_kind(kind: TokenKind) -> Option<AssignKind> {
    match kind {
        TokenKind::Assign | TokenKind::Eq => Some(AssignKind::Default),
        TokenKind::PlusEq => Some(AssignKind::Add),
        TokenKind::MinusEq => Some(AssignKind::Sub),
        TokenKind::AsteriskEq => Some(AssignKind::Mul),
        TokenKind::SlashEq => Some(AssignKind::Div),
        TokenKind::ModEq => Some(AssignKind::Mod),
        _ => None,
    }
}

fn parse_infix<'lex>(
    mut lex: RefLexer<'lex>,
    left: Expr,
    reporter: &mut DiagnosticReporter,
) -> Parser<'lex, Expr, ()> {
    let token = lex.peek().clone();
    match token.kind {
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Asterisk
        | TokenKind::Slash
        | TokenKind::Mod
        | TokenKind::Lt
        | TokenKind::LtEq
        | TokenKind::Gt
        | TokenKind::GtEq
        | TokenKind::EqEq
        | TokenKind::NotEq => {
            lex.next();
            let op = match token.kind {
                TokenKind::Plus => Op::Add,
                TokenKind::Minus => Op::Sub,
                TokenKind::Asterisk => Op::Mul,
                TokenKind::Slash => Op::Div,
                TokenKind::Mod => Op::Mod,
                TokenKind::Lt => Op::Lt,
                TokenKind::LtEq => Op::LtEq,
                TokenKind::Gt => Op::Gt,
                TokenKind::GtEq => Op::GtEq,
                TokenKind::EqEq => Op::Eq,
                TokenKind::NotEq => Op::NotEq,
                _ => unreachable!(),
            };
            let precedence = token_to_precedence(&token);
            let (lex, right) = try_parse!(parse_expr_with_precedence(lex, precedence, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Binary(BinaryExpr {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                    use_operator_overload: None,
                })),
            )
        }
        TokenKind::DoubleAmpersand
        | TokenKind::DoublePipe
        | TokenKind::Ampersand
        | TokenKind::Pipe
        | TokenKind::Caret => {
            lex.next();
            let op = match token.kind {
                TokenKind::DoubleAmpersand => Op::And,
                TokenKind::DoublePipe => Op::Or,
                TokenKind::Ampersand => Op::BitAnd,
                TokenKind::Pipe => Op::BitOr,
                TokenKind::Caret => Op::BitXor,
                _ => unreachable!(),
            };
            let precedence = token_to_precedence(&token);
            let (lex, right) = try_parse!(parse_expr_with_precedence(lex, precedence, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Binary(BinaryExpr {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                    use_operator_overload: None,
                })),
            )
        }
        t if t.is_assign_kind() => {
            lex.next();
            let assign_kind = match token_to_assign_kind(token.kind) {
                Some(k) => k,
                None => {
                    reporter.report(token.loc, format!("invalid assignment: {}", token.source()));
                    return Parser::Fail(lex, ());
                }
            };
            let (mut current_lex, first_right) = try_parse!(parse_expr_with_precedence(
                lex,
                Precedence::Lowest,
                reporter
            ));
            let right = if current_lex.peek().kind == TokenKind::Comma {
                let mut exprs = vec![first_right];
                let mut last_loc = token.loc;
                while current_lex.peek().kind == TokenKind::Comma {
                    current_lex.next();
                    let (next_lex, expr) = try_parse!(parse_expr_with_precedence(
                        current_lex,
                        Precedence::Lowest,
                        reporter
                    ));
                    current_lex = next_lex;
                    last_loc = expr.loc();
                    exprs.push(expr);
                }
                Expr::new(ExprKind::Tuple(exprs, last_loc))
            } else {
                first_right
            };
            Parser::Success(
                current_lex,
                Expr::new(ExprKind::Assign(AssignExpr {
                    left: Box::new(left),
                    assign_kind,
                    right: Box::new(right),
                })),
            )
        }
        TokenKind::DoubleColon => {
            reporter.report(
                token.loc,
                "associated paths or namespace lookup using '::' is not supported yet",
            );
            Parser::Fail(lex, ())
        }
        TokenKind::Dot => {
            lex.next(); // Consume '.'

            if let ExprKind::Identifier(name) = &left.kind {
                if lex.peek().kind == TokenKind::OpenBracket {
                    let loc = lex.peek().loc;
                    let elements = try_parse!(lex, parse_array(lex, reporter));
                    return Parser::Success(
                        lex,
                        Expr::new(ExprKind::Array(ArrayExpr {
                            loc,
                            elements,
                            type_hint: Some(Type::Scalar(name.clone())),
                        })),
                    );
                }
                if lex.peek().kind == TokenKind::OpenCurly {
                    let fields = try_parse!(lex, parse_struct_literal_fields(lex, reporter,));
                    return Parser::Success(
                        lex,
                        Expr::new(ExprKind::StructLiteral(StructLiteralExpr {
                            name: name.clone(),
                            type_args: None,
                            fields,
                        })),
                    );
                }
            }

            let peek = lex.peek();
            let property = if peek.kind == TokenKind::Identifier {
                lex.next()
            } else if let TokenKind::Number(_) = peek.kind {
                lex.next()
            } else {
                reporter.report(
                    peek.loc,
                    format!(
                        "expected identifier or number for member, found {:?}",
                        peek.kind
                    ),
                );
                return Parser::Fail(lex, ());
            };
            Parser::Success(
                lex,
                Expr::new(ExprKind::Member(MemberExpr {
                    object: Box::new(left),
                    property,
                })),
            )
        }
        TokenKind::Dollar => {
            let token = lex.next(); // Consume '$'
            try_parse!(lex, expect(lex, TokenKind::OpenBracket, reporter));
            let type_args = try_parse!(
                lex,
                sep_by(lex, |lex| parse_type(lex, reporter), parse_comma)
            );
            try_parse!(lex, expect(lex, TokenKind::CloseBracket, reporter));

            if let ExprKind::Identifier(name) = &left.kind {
                if lex.peek().kind == TokenKind::Dot {
                    lex.next();
                    let fields = try_parse!(lex, parse_struct_literal_fields(lex, reporter));
                    return Parser::Success(
                        lex,
                        Expr::new(ExprKind::StructLiteral(StructLiteralExpr {
                            name: name.clone(),
                            type_args: Some(type_args),
                            fields,
                        })),
                    );
                }
            }

            Parser::Success(
                lex,
                Expr::new(ExprKind::GenericInst(
                    Box::new(left.clone()),
                    type_args,
                    token.loc,
                )),
            )
        }
        TokenKind::OpenParen => {
            let (lex, arguments) = try_parse!(parse_arguments(lex, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Call(CallExpr {
                    loc: left.loc(),
                    callee: Box::new(left),
                    arguments,
                    resolved_name: None,
                })),
            )
        }
        TokenKind::OpenBracket => {
            lex.next(); // Consume '['
            let loc = token.loc;
            let index = try_parse!(lex, parse_expr(lex, reporter));
            try_parse!(lex, expect(lex, TokenKind::CloseBracket, reporter));
            Parser::Success(
                lex,
                Expr::new(ExprKind::Index(IndexExpr {
                    loc,
                    array: Box::new(left),
                    index: Box::new(index),
                })),
            )
        }
        _ => {
            reporter.report(
                token.loc,
                format!("unexpected token in expression: {}", token.source()),
            );
            Parser::Fail(lex, ())
        }
    }
}
