use std::{fmt, path::PathBuf, sync::Arc};

pub struct Lexer<'src> {
    source: &'src str,
    data: Vec<char>,
    pos: usize,
    byte_pos: usize,
    loc: Loc,
    peeked: Option<Token>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            data: source.chars().collect(),
            loc: Loc::new(1, 1),
            pos: 0,
            byte_pos: 0,
            peeked: None,
        }
    }

    pub fn next(&mut self) -> Token {
        if let Some(peek) = self.peeked.take() {
            peek
        } else {
            self.next_token()
        }
    }

    pub fn peek(&mut self) -> &Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token());
        }
        self.peeked.as_ref().unwrap()
    }

    fn advance(&mut self) -> char {
        let ch = self.read_char();
        self.byte_pos += ch.len_utf8();
        self.pos += 1;
        self.loc.next(ch);
        ch
    }

    fn read_char(&mut self) -> char {
        let pos = self.pos;
        if pos >= self.data.len() {
            '\0'
        } else {
            self.data[pos]
        }
    }

    fn next_token(&mut self) -> Token {
        while self.pos <= self.data.len() {
            let begin_byte = self.byte_pos;
            let ch = self.advance();
            let loc = self.loc;

            let tok = match ch {
                '/' if self.read_char() == '/' => {
                    while self.advance() != '\n' {}
                    continue;
                }
                '#' => {
                    loop {
                        let ch = self.read_char();
                        if ch.is_alphanumeric() || ch == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Token::new(
                        TokenKind::Directive,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '-' if self.read_char() == '>' => {
                    self.advance();
                    Token::new(
                        TokenKind::Arrow,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '=' if self.read_char() == '=' => {
                    self.advance();
                    Token::new(
                        TokenKind::Eq,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '+' if self.read_char() == '=' => {
                    self.advance();
                    Token::new(
                        TokenKind::PlusAssign,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '<' if self.read_char() == '=' => {
                    self.advance();
                    Token::new(
                        TokenKind::LtEq,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '>' if self.read_char() == '=' => {
                    self.advance();
                    Token::new(
                        TokenKind::GtEq,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '!' if self.read_char() == '=' => {
                    self.advance();
                    Token::new(
                        TokenKind::NotEq,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '&' if self.read_char() == '&' => {
                    self.advance();
                    Token::new(
                        TokenKind::DoubleAmpersand,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '|' if self.read_char() == '|' => {
                    self.advance();
                    Token::new(
                        TokenKind::DoublePipe,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                ':' if self.read_char() == ':' => {
                    self.advance();
                    Token::new(
                        TokenKind::DoubleColon,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                '.' if self.read_char() == '.' && self.read_char() == '.' => {
                    self.advance();
                    self.advance();
                    Token::new(
                        TokenKind::Ellipsis,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    )
                }
                ch if ch.is_alphabetic() || ch == '_' => return self.lex_identfier(begin_byte),
                '0'..='9' => return self.lex_number(begin_byte),
                '"' => return self.lex_string(begin_byte),

                ',' => Token::new(
                    TokenKind::Comma,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                ';' => Token::new(
                    TokenKind::SemiColon,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                ':' => Token::new(
                    TokenKind::Colon,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '\\' => Token::new(
                    TokenKind::BackSlash,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '=' => Token::new(
                    TokenKind::Assign,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '<' => Token::new(
                    TokenKind::Lt,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '>' => Token::new(
                    TokenKind::Gt,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '!' => Token::new(
                    TokenKind::Bang,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '+' => Token::new(
                    TokenKind::Plus,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '-' => Token::new(
                    TokenKind::Minus,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '.' => Token::new(
                    TokenKind::Dot,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '*' => Token::new(
                    TokenKind::Asterisk,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '/' => Token::new(
                    TokenKind::Slash,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '%' => Token::new(
                    TokenKind::Mod,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '$' => Token::new(
                    TokenKind::Dollar,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '&' => Token::new(
                    TokenKind::Ampersand,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '^' => Token::new(
                    TokenKind::Caret,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '|' => Token::new(
                    TokenKind::Pipe,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '(' => Token::new(
                    TokenKind::OpenParen,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                ')' => Token::new(
                    TokenKind::CloseParen,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '[' => Token::new(
                    TokenKind::OpenBracket,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                ']' => Token::new(
                    TokenKind::CloseBracket,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '{' => Token::new(
                    TokenKind::OpenCurly,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),
                '}' => Token::new(
                    TokenKind::CloseCurly,
                    loc,
                    self.source[begin_byte..self.byte_pos].into(),
                ),

                ch if ch.is_whitespace() => continue,
                '\0' => return Token::new(TokenKind::EOF, self.loc, "\0".into()),
                _ => {
                    return Token::new(
                        TokenKind::UnexpectedCharacter,
                        self.loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    );
                }
            };
            return tok;
        }

        Token::new(TokenKind::EOF, self.loc, "".into())
    }

    fn lex_identfier(&mut self, begin_byte: usize) -> Token {
        let loc = self.loc;
        loop {
            let ch = self.read_char();
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let ident = &self.source[begin_byte..self.byte_pos];
        let mut kind = TokenKind::Identifier;
        match ident {
            "var" => kind = TokenKind::VarKeyword,
            "fn" => kind = TokenKind::FnKeyword,
            "type" => kind = TokenKind::TypeKeyword,
            "for" => kind = TokenKind::ForKeyword,
            "foreach" => kind = TokenKind::ForEachKeyword,
            "module" => kind = TokenKind::ModuleKeyword,
            "new" => kind = TokenKind::NewKeyword,
            "trait" => kind = TokenKind::TraitKeyword,
            "impl" => kind = TokenKind::ImplKeyword,
            _ => {}
        }
        Token::new(kind, loc, ident.into())
    }

    // fn lex_number(&mut self, begin_byte: usize) -> Token {
    //     let loc = self.loc;
    //     let kind = TokenKind::IntegerNumber;

    //     while let '0'..='9' = self.read_char() {
    //         self.advance();
    //     }

    //     Token::new(kind, loc, self.source[begin_byte..self.byte_pos].into())
    // }

    fn lex_number(&mut self, begin_byte: usize) -> Token {
        let loc = self.loc;
        let mut end = begin_byte;
        let mut base = 10;

        // Check for base prefix (0x, 0b, 0o)
        // if self.read_char() == '0' {
        let next = self.read_char();
        match next {
            'x' | 'X' => {
                base = 16;
                self.advance(); // 0
                self.advance(); // x
            }
            'b' | 'B' => {
                base = 2;
                self.advance(); // 0
                self.advance(); // b
            }
            'o' | 'O' => {
                base = 8;
                self.advance(); // 0
                self.advance(); // o
            }
            _ => {}
        }
        // }

        // Read digits according to base
        loop {
            let c = self.read_char();
            let valid = match base {
                2 => matches!(c, '0' | '1'),
                8 => matches!(c, '0'..='7'),
                10 => c.is_ascii_digit(),
                16 => c.is_ascii_hexdigit(),
                _ => false,
            };
            if !valid {
                break;
            }
            self.advance();
        }

        end = self.byte_pos;

        // Parse suffix (letters/numbers after digits)
        let mut suffix = String::new();
        loop {
            let c = self.read_char();
            if c.is_ascii_alphanumeric() {
                suffix.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let num_str = &self.source[begin_byte..end]
            .trim_start_matches("0x")
            .trim_start_matches("0X")
            .trim_start_matches("0b")
            .trim_start_matches("0B")
            .trim_start_matches("0o")
            .trim_start_matches("0O");
        let kind = match (base, suffix.as_str()) {
            (2 | 8 | 10 | 16, "" | "i32") => TokenKind::Int(NumberBase::from(base)),
            (2 | 8 | 10 | 16, "i64") => TokenKind::Int64(NumberBase::from(base)),
            (2 | 8 | 10 | 16, "u32") => TokenKind::UInt(NumberBase::from(base)),
            (2 | 8 | 10 | 16, "u64") => TokenKind::UInt64(NumberBase::from(base)),
            _ => TokenKind::InvalidNumber,
        };

        Token::new(kind, loc, (*num_str).into())
    }

    fn lex_string(&mut self, begin_byte: usize) -> Token {
        // let mut buffer = String::new();
        let loc = self.loc;
        loop {
            let ch = self.read_char();
            match ch {
                '"' => {
                    self.advance();
                    break;
                }
                '\0' => {
                    return Token::new(
                        TokenKind::UnterminatedStringLiteral,
                        loc,
                        self.source[begin_byte..self.byte_pos].into(),
                    );
                }
                '\\' => {
                    self.advance();
                    let esc = self.read_char();
                    match esc {
                        'r' => {}  // buffer.push('\r'),
                        'n' => {}  // buffer.push('\n'),
                        '"' => {}  // buffer.push('"'),
                        '\'' => {} // buffer.push('\''),
                        '\\' => {} // buffer.push('\\'),
                        '0' => {}  // buffer.push('\0'),
                        _ => {
                            return Token::new(
                                TokenKind::InvalidEscapeSequence,
                                loc,
                                self.source[begin_byte..self.byte_pos].into(),
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
            self.source[begin_byte..self.byte_pos].into(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub loc: Loc,
    // source: &'static str,
    pub source: String,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TokenKind::EOF => write!(f, "EOF"),
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
            TokenKind::StringLiteral => write!(f, "{}", self.source.escape_default()),
            TokenKind::CharacterLiteral => write!(f, "{}", self.source.escape_default()),
            TokenKind::FnKeyword => write!(f, "keyword fn"),
            TokenKind::TypeKeyword => write!(f, "keyword type"),
            TokenKind::ForKeyword => write!(f, "keyword for"),
            TokenKind::ModuleKeyword => write!(f, "keyword module"),
            _ => write!(f, "{}", self.source),
        }
    }
}

impl Token {
    pub fn source(&self) -> &str {
        // unsafe { transmute::<&'static str, &str>(self.source) }
        &self.source
    }

    pub fn new(kind: TokenKind, loc: Loc, source: String) -> Self {
        Self {
            kind,
            loc,
            // source: unsafe { transmute::<&str, &'static str>(source) },
            source,
        }
    }

    pub fn is_eof(&self) -> bool {
        matches!(self.kind, TokenKind::EOF)
    }

    pub fn is_ident(&self) -> bool {
        matches!(self.kind, TokenKind::Identifier)
    }

    pub fn unescape(&self) -> String {
        match self.kind {
            TokenKind::StringLiteral => token_string_unescape(self.source()),
            _ => todo!(),
        }
    }
}
pub fn token_string_unescape(source: &str) -> String {
    let mut buffer = String::new();
    let mut esc = false;
    let mut src = source.chars();
    src.next();
    for ch in src {
        match ch {
            ch if esc => {
                match ch {
                    'r' => buffer.push('\r'),
                    'n' => buffer.push('\n'),
                    '"' => buffer.push('"'),
                    '\'' => buffer.push('\''),
                    '\\' => buffer.push('\\'),
                    '0' => buffer.push('\0'),
                    _ => return buffer,
                }
                esc = false;
            }
            '"' => return buffer,
            '\\' => {
                esc = true;
                continue;
            }
            _ => buffer.push(ch),
        }
    }
    buffer
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    #[default]
    EOF,
    UnexpectedCharacter,
    InvalidEscapeSequence,
    UnterminatedStringLiteral,

    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenCurly,
    CloseCurly,

    Identifier,
    FnKeyword,
    VarKeyword,
    TypeKeyword,
    ForKeyword,
    ForEachKeyword,
    ModuleKeyword,
    NewKeyword,
    TraitKeyword,
    ImplKeyword,
    Directive,

    RealNumber,
    StringLiteral,
    CharacterLiteral,

    Dot,
    Ellipsis,
    Comma,
    Colon,
    DoubleColon,
    SemiColon,
    Arrow,
    BackSlash,

    Assign,
    PlusAssign,
    Bang,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    Mod,
    Ampersand,
    Pipe,
    Caret,
    DoubleAmpersand,
    DoublePipe,

    Dollar,
    InvalidNumber,

    Int64(NumberBase),
    UInt(NumberBase),
    UInt64(NumberBase),
    Int(NumberBase),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberBase {
    B,
    O,
    D,
    X,
}

impl From<i32> for NumberBase {
    fn from(value: i32) -> Self {
        match value {
            2 => Self::B,
            8 => Self::O,
            10 => Self::D,
            16 => Self::X,
            _ => panic!("Unkwon base"),
        }
    }
}

impl From<NumberBase> for u32 {
    fn from(val: NumberBase) -> Self {
        match val {
            NumberBase::B => 2,
            NumberBase::O => 8,
            NumberBase::D => 10,
            NumberBase::X => 16,
        }
    }
}

impl TokenKind {
    pub fn is_int_num(&self) -> bool {
        matches!(
            self,
            Self::Int(_) | Self::Int64(_) | Self::UInt(_) | Self::UInt64(_)
        )
    }

    pub fn is_assign_kind(&self) -> bool {
        matches!(self, Self::Assign | Self::PlusAssign)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Loc {
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl Loc {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub fn next_column(&mut self) {
        self.col += 1;
    }

    pub fn next_line(&mut self) {
        self.line += 1;
        self.col = 1;
    }

    pub fn next(&mut self, c: char) {
        match c {
            '\n' => self.next_line(),
            '\t' => {
                let ts = 8;
                self.col = (self.col / ts) * ts + ts;
            }
            c if c.is_control() => {}
            _ => {
                // For proper UTF-8 support, we could use unicode-width crate
                // to get the display width of characters, but for simplicity
                // we'll treat all non-control characters as width 1
                self.next_column();
            }
        }
    }
}

/// AI generated
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_identifiers() {
        let source = "café µ_var 变量 αβγ";
        let mut lexer = Lexer::new(source);

        let token1 = lexer.next();
        assert_eq!(token1.kind, TokenKind::Identifier);
        assert_eq!(token1.source(), "café");

        let token2 = lexer.next();
        assert_eq!(token2.kind, TokenKind::Identifier);
        assert_eq!(token2.source(), "µ_var");

        let token3 = lexer.next();
        assert_eq!(token3.kind, TokenKind::Identifier);
        assert_eq!(token3.source(), "变量");

        let token4 = lexer.next();
        assert_eq!(token4.kind, TokenKind::Identifier);
        assert_eq!(token4.source(), "αβγ");
    }

    #[test]
    fn test_utf8_strings() {
        let source = r#""Hello 世界" "Café ☕" "🦀 Rust""#;
        let mut lexer = Lexer::new(source);

        let token1 = lexer.next();
        assert_eq!(token1.kind, TokenKind::StringLiteral);
        assert_eq!(token1.unescape(), "Hello 世界");

        let token2 = lexer.next();
        assert_eq!(token2.kind, TokenKind::StringLiteral);
        assert_eq!(token2.unescape(), "Café ☕");

        let token3 = lexer.next();
        assert_eq!(token3.kind, TokenKind::StringLiteral);
        assert_eq!(token3.unescape(), "🦀 Rust");
    }

    #[test]
    fn test_unicode_whitespace() {
        // Test various Unicode whitespace characters
        let source = "var1\u{00A0}var2\u{2000}var3\u{3000}var4"; // NBSP, EN QUAD, IDEOGRAPHIC SPACE
        let mut lexer = Lexer::new(source);

        let tokens: Vec<Token> = std::iter::from_fn(|| {
            let token = lexer.next();
            if token.is_eof() { None } else { Some(token) }
        })
        .collect();

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].source(), "var1");
        assert_eq!(tokens[1].source(), "var2");
        assert_eq!(tokens[2].source(), "var3");
        assert_eq!(tokens[3].source(), "var4");
    }

    #[test]
    fn test_utf8_tokenization_debug() {
        let source = "café = \"Hello 世界\"";
        let mut lexer = Lexer::new(source);

        let mut tokens = Vec::new();
        loop {
            let token = lexer.next();
            if token.is_eof() {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }

        // Print tokens for debugging
        for (i, token) in tokens.iter().enumerate() {
            println!("Token {}: {:?} -> '{}'", i, token.kind, token.source());
        }

        // Verify we get the expected tokens
        assert_eq!(tokens.len(), 4); // identifier, assign, string, eof
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].source(), "café");
        assert_eq!(tokens[1].kind, TokenKind::Assign);
        assert_eq!(tokens[2].kind, TokenKind::StringLiteral);
        assert_eq!(tokens[2].unescape(), "Hello 世界");
        assert_eq!(tokens[3].kind, TokenKind::EOF);
    }

    #[test]
    fn test_utf8_edge_cases() {
        // Test zero-width space - it's not considered whitespace by Rust's is_whitespace()
        // so it's treated as an unexpected character. This is expected behavior.
        let source = "var\u{200B}name"; // zero-width space
        let mut lexer = Lexer::new(source);
        let token1 = lexer.next();
        assert_eq!(token1.kind, TokenKind::Identifier);
        assert_eq!(token1.source(), "var");

        let token2 = lexer.next();
        assert_eq!(token2.kind, TokenKind::UnexpectedCharacter);
        assert_eq!(token2.source(), "\u{200B}");

        let token3 = lexer.next();
        assert_eq!(token3.kind, TokenKind::Identifier);
        assert_eq!(token3.source(), "name");
    }

    #[test]
    fn test_mixed_scripts() {
        // Test mixing different scripts in identifiers
        let source = "english_العربية_中文_русский";
        let mut lexer = Lexer::new(source);
        let token = lexer.next();
        assert_eq!(token.kind, TokenKind::Identifier);
        assert_eq!(token.source(), "english_العربية_中文_русский");
    }

    #[test]
    fn test_utf8_string_escapes() {
        // Test string with UTF-8 characters and basic escape sequences
        let source = r#""Hello\n世界\"end""#;
        let mut lexer = Lexer::new(source);
        let token = lexer.next();
        assert_eq!(token.kind, TokenKind::StringLiteral);
        assert_eq!(token.unescape(), "Hello\n世界\"end");
    }

    #[test]
    fn test_unicode_numbers_as_identifiers() {
        // Unicode numeric characters are treated as unexpected characters in this lexer
        // since they're not alphabetic. This is expected behavior.
        let source = "٠١٢٣"; // Arabic-Indic digits
        let mut lexer = Lexer::new(source);
        let token = lexer.next();
        assert_eq!(token.kind, TokenKind::UnexpectedCharacter);
        assert_eq!(token.source(), "٠");
    }

    #[test]
    fn test_combining_characters() {
        // Test combining characters in identifiers
        let source = "café"; // e with combining acute accent
        let mut lexer = Lexer::new(source);
        let token = lexer.next();
        assert_eq!(token.kind, TokenKind::Identifier);
        assert_eq!(token.source(), "café");
    }

    #[test]
    fn test_right_to_left_text() {
        // Test right-to-left languages
        let source = "مرحبا = \"السلام عليكم\"";
        let mut lexer = Lexer::new(source);

        let identifier = lexer.next();
        assert_eq!(identifier.kind, TokenKind::Identifier);
        assert_eq!(identifier.source(), "مرحبا");

        let assign = lexer.next();
        assert_eq!(assign.kind, TokenKind::Assign);

        let string = lexer.next();
        assert_eq!(string.kind, TokenKind::StringLiteral);
        assert_eq!(string.unescape(), "السلام عليكم");
    }

    #[test]
    fn test_elipsis_token() {
        let source = "...";
        let mut lexer = Lexer::new(source);
        let token = lexer.next();
        assert_eq!(token.kind, TokenKind::Ellipsis);
        assert_eq!(token.source(), "...");
    }
}
