#![allow(dead_code)]
#![allow(unused)]
use std::fmt;

pub fn is_keyword_default(_: &str) -> bool {
    false
}

pub fn is_binop_default(op: &TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        op,
        Assign
            | Plus
            | Minus
            | Asterisk
            | Slash
            | Eq
            | NotEq
            | Gt
            | Lt
            | Percent
            | Ampersand
            | Pipe
            | DoubleAmpersand
            | DoublePipe
    )
}

pub type IsKeywordFn = fn(ident: &str) -> bool;
pub type IsBinopFn = fn(op: &TokenKind) -> bool;

pub struct Lexer<'src> {
    source: &'src str,
    data: &'src [u8],
    pos: usize,
    loc: Loc<'src>,
    is_keyword_fn: IsKeywordFn,
    is_binop_fn: IsBinopFn,
}

pub struct PeekableLexer<'src> {
    pub peeked: Option<Token<'src>>,
    pub lexer: Lexer<'src>,
}

impl<'src> PeekableLexer<'src> {
    pub fn new(file_path: &'src str, source: &'src str) -> Self {
        Self {
            peeked: None,
            lexer: Lexer::new(file_path, source),
        }
    }
    pub fn next_token(&mut self) -> Token<'src> {
        if let Some(peek) = self.peeked.take() {
            peek
        } else {
            self.lexer.next_token()
        }
    }
    pub fn peek_token(&mut self) -> &Token<'src> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token());
        }
        self.peeked.as_ref().unwrap()
    }

    pub fn is_binop(&self, op: &TokenKind) -> bool {
        (self.lexer.is_binop_fn)(op)
    }

    pub fn set_is_keyword_fn(&mut self, f: IsKeywordFn) {
        self.lexer.set_is_keyword_fn(f);
    }

    pub fn set_is_binop_fn(&mut self, f: IsBinopFn) {
        self.lexer.set_is_binop_fn(f);
    }

    pub fn get_is_keyword_fn(&self) -> IsKeywordFn {
        self.lexer.is_keyword_fn
    }

    pub fn get_is_binop_fn(&self) -> IsBinopFn {
        self.lexer.is_binop_fn
    }
}

impl<'src> Lexer<'src> {
    pub fn new(file_path: &'src str, source: &'src str) -> Self {
        Self {
            source,
            data: source.as_bytes(),
            loc: Loc::new(file_path, 1, 1),
            pos: 0,
            is_keyword_fn: is_keyword_default,
            is_binop_fn: is_binop_default,
        }
    }

    pub fn set_is_keyword_fn(&mut self, f: IsKeywordFn) {
        self.is_keyword_fn = f;
    }

    pub fn set_is_binop_fn(&mut self, f: IsBinopFn) {
        self.is_binop_fn = f;
    }

    fn advance(&mut self) -> u8 {
        let ch = self.read_char();
        self.pos += 1;
        self.loc.next(ch);
        ch
    }

    fn peek_suffix(&self) -> Option<&'src str> {
        let begin = self.pos;
        let mut i = self.pos;
        let mut counter = 0;
        while i < self.data.len() && self.data[i].is_ascii_alphanumeric() {
            // buf.push(self.data[i] as char);
            i += 1;
            counter += 1;
            if counter > 3 {
                break;
            }
        }
        match self.source[begin..i].into() {
            "u8" | "i8" | "u16" | "i16" | "i32" | "u32" | "i64" | "u64" => {
                Some(self.source[begin..i].into())
            }
            _ => None,
        }
    }

    fn advance_n(&mut self, n: usize) {
        for _ in 0..n {
            self.advance();
        }
    }

    fn read_char(&mut self) -> u8 {
        let pos = self.pos;
        if pos >= self.data.len() {
            0
        } else {
            self.data[pos]
        }
    }

    pub fn next_token(&mut self) -> Token<'src> {
        while self.pos <= self.data.len() {
            let begin = self.pos;
            let ch = self.advance();
            let loc = self.loc;

            let tok = match ch {
                b'/' if self.read_char() == b'/' => {
                    while !matches!(self.advance(), b'\n' | b'\0') {}
                    continue;
                }
                b'#' => {
                    while !matches!(self.advance(), b'\n' | b'\0') {}
                    continue;
                }
                b'-' if self.read_char() == b'>' => {
                    self.advance();
                    Token::new(TokenKind::Arrow, loc, self.source[begin..self.pos].into())
                }
                b'=' if self.read_char() == b'=' => {
                    self.advance();
                    Token::new(TokenKind::Eq, loc, self.source[begin..self.pos].into())
                }
                b'<' if self.read_char() == b'=' => {
                    self.advance();
                    Token::new(TokenKind::LtEq, loc, self.source[begin..self.pos].into())
                }
                b'>' if self.read_char() == b'=' => {
                    self.advance();
                    Token::new(TokenKind::GtEq, loc, self.source[begin..self.pos].into())
                }
                b'!' if self.read_char() == b'=' => {
                    self.advance();
                    Token::new(TokenKind::NotEq, loc, self.source[begin..self.pos].into())
                }
                b'&' if self.read_char() == b'&' => {
                    self.advance();
                    Token::new(
                        TokenKind::DoubleAmpersand,
                        loc,
                        self.source[begin..self.pos].into(),
                    )
                }
                b'|' if self.read_char() == b'|' => {
                    self.advance();
                    Token::new(
                        TokenKind::DoublePipe,
                        loc,
                        self.source[begin..self.pos].into(),
                    )
                }
                b'<' if self.read_char() == b'<' => {
                    self.advance();
                    Token::new(
                        TokenKind::ShiftLeft,
                        loc,
                        self.source[begin..self.pos].into(),
                    )
                }
                b'>' if self.read_char() == b'>' => {
                    self.advance();
                    Token::new(
                        TokenKind::ShiftRight,
                        loc,
                        self.source[begin..self.pos].into(),
                    )
                }
                b':' if self.read_char() == b':' => {
                    self.advance();
                    Token::new(
                        TokenKind::DoubleColon,
                        loc,
                        self.source[begin..self.pos].into(),
                    )
                }
                b'.' if self.read_char() == b'.' && self.read_char() == b'.' => {
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::Splat, loc, self.source[begin..self.pos].into())
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => return self.lex_identfier_or_keyword(begin),
                b'0'..=b'9' => return self.lex_number(begin),
                b'"' => return self.lex_string(begin),
                b'@' => return self.lex_macro(begin),

                b',' => Token::new(TokenKind::Comma, loc, self.source[begin..self.pos].into()),
                b';' => Token::new(
                    TokenKind::SemiColon,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b':' => Token::new(TokenKind::Colon, loc, self.source[begin..self.pos].into()),
                b'\\' => Token::new(
                    TokenKind::BackSlash,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b'=' => Token::new(TokenKind::Assign, loc, self.source[begin..self.pos].into()),
                b'<' => Token::new(TokenKind::Lt, loc, self.source[begin..self.pos].into()),
                b'>' => Token::new(TokenKind::Gt, loc, self.source[begin..self.pos].into()),
                b'!' => Token::new(TokenKind::Bang, loc, self.source[begin..self.pos].into()),
                b'+' => Token::new(TokenKind::Plus, loc, self.source[begin..self.pos].into()),
                b'-' => Token::new(TokenKind::Minus, loc, self.source[begin..self.pos].into()),
                b'.' => Token::new(TokenKind::Dot, loc, self.source[begin..self.pos].into()),
                b'~' => Token::new(TokenKind::Tilde, loc, self.source[begin..self.pos].into()),
                b'*' => Token::new(
                    TokenKind::Asterisk,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b'/' => Token::new(TokenKind::Slash, loc, self.source[begin..self.pos].into()),
                b'%' => Token::new(TokenKind::Percent, loc, self.source[begin..self.pos].into()),
                b'$' => Token::new(TokenKind::Dollar, loc, self.source[begin..self.pos].into()),
                b'^' => Token::new(TokenKind::Caret, loc, self.source[begin..self.pos].into()),
                b'&' => Token::new(
                    TokenKind::Ampersand,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b'|' => Token::new(TokenKind::Pipe, loc, self.source[begin..self.pos].into()),
                b'(' => Token::new(
                    TokenKind::OpenParen,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b')' => Token::new(
                    TokenKind::CloseParen,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b'[' => Token::new(
                    TokenKind::OpenBracket,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b']' => Token::new(
                    TokenKind::CloseBracket,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b'{' => Token::new(
                    TokenKind::OpenBrace,
                    loc,
                    self.source[begin..self.pos].into(),
                ),
                b'}' => Token::new(
                    TokenKind::CloseBrace,
                    loc,
                    self.source[begin..self.pos].into(),
                ),

                ch if ch.is_ascii_whitespace() => continue,
                0 => return Token::new(TokenKind::EOF, self.loc, "\0".into()),
                _ => {
                    return Token::new(
                        TokenKind::UnexpectedCharacter,
                        self.loc,
                        self.source[begin..self.pos].into(),
                    );
                }
            };
            return tok;
        }

        Token::new(TokenKind::EOF, self.loc, "".into())
    }

    fn lex_identfier_or_keyword(&mut self, begin: usize) -> Token<'src> {
        let loc = self.loc;
        loop {
            let ch = self.read_char();
            match ch {
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => (),
                b'0'..=b'9' => (),
                _ => break,
            }
            self.advance();
        }
        let ident = &self.source[begin..self.pos];
        let kind = if (self.is_keyword_fn)(ident) {
            TokenKind::Keyword
        } else {
            TokenKind::Identifier
        };
        Token::new(kind, loc, ident.into())
    }

    fn lex_number(&mut self, begin: usize) -> Token<'src> {
        let loc = self.loc();
        let mut kind = TokenKind::IntegerNumber;

        while let b'0'..=b'9' = self.read_char() {
            self.advance();
        }

        let suffix_start = self.pos;
        let suffix = self.peek_suffix();

        match suffix.as_deref() {
            Some("i8") => {
                kind = TokenKind::I8Number;
                self.advance_n(3);
            }
            Some("u8") => {
                kind = TokenKind::U8Number;
                self.advance_n(3);
            }
            Some("i16") => {
                kind = TokenKind::I16Number;
                self.advance_n(3);
            }
            Some("u16") => {
                kind = TokenKind::U16Number;
                self.advance_n(3);
            }
            Some("i32") => {
                kind = TokenKind::I32Number;
                self.advance_n(3);
            }
            Some("u32") => {
                kind = TokenKind::U32Number;
                self.advance_n(3);
            }
            Some("i64") => {
                kind = TokenKind::I64Number;
                self.advance_n(3);
            }
            Some("u64") => {
                kind = TokenKind::U64Number;
                self.advance_n(3);
            }
            Some(_) => {
                kind = TokenKind::UnknowIntergerLiteral;
            }
            None => (),
        }

        Token::new(kind, loc, self.source[begin..suffix_start].into())
    }

    fn lex_string(&mut self, begin: usize) -> Token<'src> {
        // let mut buffer = String::new();
        let loc = self.loc();
        loop {
            let ch = self.read_char();
            match ch {
                b'"' => {
                    self.advance();
                    break;
                }
                b'\0' => {
                    return Token::new(
                        TokenKind::UnterminatedStringLiteral,
                        loc,
                        self.source[begin..self.pos].into(),
                    );
                }
                b'\\' => {
                    self.advance();
                    let esc = self.read_char();
                    match esc {
                        b'r' => {}  // buffer.push('\r'),
                        b'n' => {}  // buffer.push('\n'),
                        b'"' => {}  // buffer.push('"'),
                        b'\'' => {} // buffer.push('\''),
                        b'\\' => {} // buffer.push('\\'),
                        b'0' => {}  // buffer.push('\0'),
                        _ => {
                            return Token::new(
                                TokenKind::InvalidEscapeSequence,
                                loc,
                                self.source[begin..self.pos].into(),
                            );
                        }
                    }
                }
                _ => {} // buffer.push(ch as char),
            }
            self.advance();
        }

        Token::new(
            TokenKind::StringLiteral,
            loc,
            self.source[begin..self.pos].into(),
        )
    }

    pub fn loc(&self) -> Loc<'src> {
        self.loc
    }

    fn lex_macro(&mut self, begin: usize) -> Token<'src> {
        // let mut buffer = String::new();
        let mut kind = TokenKind::MacroCall;
        let loc = self.loc();

        loop {
            let ch = self.read_char();
            match ch {
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => (),
                b'0'..=b'9' => (),
                _ => break,
            }
            self.advance();
        }

        Token::new(kind, loc, self.source[begin..self.pos].into())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Token<'src> {
    pub kind: TokenKind,
    pub loc: Loc<'src>,
    pub source: &'src str,
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TokenKind::EOF => write!(f, "EOF"),
            TokenKind::Invalid => write!(f, "Invalid Token `{}`", self.source.escape_default()),
            TokenKind::UnexpectedCharacter => {
                write!(f, "Unexpected Character `{}`", self.source.escape_default())
            }
            TokenKind::InvalidEscapeSequence => {
                write!(
                    f,
                    "Invalid Escape Sequence `{}`",
                    self.source.escape_default()
                )
            }
            TokenKind::UnterminatedStringLiteral => {
                write!(
                    f,
                    "Unterminated String Literal `{}`",
                    self.source.escape_default()
                )
            }
            TokenKind::UnknowIntergerLiteral => {
                write!(
                    f,
                    "Unknown Integer Literal `{}`",
                    self.source.escape_default()
                )
            }
            TokenKind::MacroCall => {
                write!(f, "Macro({})", self.source)
            }
            TokenKind::Keyword => write!(f, "Keyword({})", self.source),
            TokenKind::StringLiteral => write!(f, "{}", self.source.escape_default()),
            TokenKind::CharacterLiteral => write!(f, "{}", self.source.escape_default()),
            _ => write!(f, "{}", self.source),
        }
    }
}

impl<'src> Token<'src> {
    pub fn new(kind: TokenKind, loc: Loc<'src>, source: &'src str) -> Self {
        Self { kind, loc, source }
    }

    pub fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::EOF)
    }

    pub fn is_invalid(&self) -> bool {
        matches!(self.kind, TokenKind::Invalid)
    }

    pub fn is_macro(&self) -> bool {
        matches!(self.kind, TokenKind::MacroCall)
    }

    pub fn is_ident(&self) -> bool {
        matches!(self.kind, TokenKind::Identifier)
    }

    pub fn unescape(&self) -> String {
        match self.kind {
            TokenKind::StringLiteral => {
                let mut buffer = String::new();
                let mut esc = false;
                let mut src = self.source.bytes();
                src.next();
                for ch in src {
                    match ch {
                        ch if esc => {
                            match ch {
                                b'r' => buffer.push('\r'),
                                b'n' => buffer.push('\n'),
                                b'"' => buffer.push('"'),
                                b'\'' => buffer.push('\''),
                                b'\\' => buffer.push('\\'),
                                b'0' => buffer.push('\0'),
                                _ => return buffer,
                            }
                            esc = false;
                        }
                        b'"' => return buffer,
                        b'\\' => {
                            esc = true;
                            continue;
                        }
                        _ => buffer.push(ch as char),
                    }
                }
                return buffer;
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    #[default]
    EOF,
    Invalid,
    UnexpectedCharacter,
    InvalidEscapeSequence,
    UnterminatedStringLiteral,
    UnknowIntergerLiteral,

    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,

    MacroCall,

    Identifier,
    Keyword,

    RealNumber,
    IntegerNumber,
    U8Number,
    I8Number,
    U16Number,
    I16Number,
    U32Number,
    I32Number,
    U64Number,
    I64Number,
    StringLiteral,
    CharacterLiteral,

    Dot,
    Tilde,
    Splat,
    Comma,
    Colon,
    DoubleColon,
    SemiColon,
    Arrow,
    BackSlash,
    Caret,

    Assign,
    Bang,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent,
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    Ampersand,
    Pipe,
    DoubleAmpersand,
    DoublePipe,

    Dollar,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Loc<'src> {
    pub file_path: &'src str,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for Loc<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file_path, self.line, self.col)
    }
}

impl<'src> Loc<'src> {
    pub fn new(file_path: &'src str, line: usize, col: usize) -> Self {
        Self {
            file_path,
            line,
            col,
        }
    }

    pub fn next_column(&mut self) {
        self.col += 1;
    }

    pub fn next_line(&mut self) {
        self.line += 1;
        self.col = 1;
    }

    pub fn next(&mut self, c: u8) {
        match c {
            b'\n' => self.next_line(),
            b'\t' => {
                let ts = 8;
                self.col = (self.col / ts) * ts + ts;
            }
            c if c.is_ascii_control() => {}
            _ => self.next_column(),
        }
    }
}
