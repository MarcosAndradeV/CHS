use lex_just_parse::lexer::Loc;
use std::fmt;

#[derive(Debug)]
pub enum SemanticError {
    TypeMismatch {
        loc: Loc,
        expected: String,
        found: String,
    },
    NoOverloadFound {
        loc: Loc,
        name: String,
        expected: String,
    },
    AmbiguousFunctionCall {
        loc: Loc,
        name: String,
    },
    FunctionAlreadyDefined {
        loc: Loc,
        name: String,
    },
    FunctionAlreadyDefinedNoTemplate {
        loc: Loc,
        name: String,
    },
    UndefinedVariable {
        loc: Loc,
        name: String,
    },
    Other {
        loc: Loc,
        message: String,
    },
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeMismatch {
                expected, found, ..
            } => {
                write!(
                    f,
                    "Type mismatch: expected {:?}, found {:?}",
                    expected, found
                )
            }
            Self::NoOverloadFound { name, expected, .. } => {
                write!(
                    f,
                    "No overload found for '{}' that matches '{}'",
                    name, expected
                )
            }
            Self::AmbiguousFunctionCall { name, .. } => {
                write!(f, "Ambiguous call to overloaded function '{}'", name)
            }
            Self::FunctionAlreadyDefined { name, .. } => {
                write!(
                    f,
                    "Function '{}' is already defined with these parameters",
                    name
                )
            }
            Self::FunctionAlreadyDefinedNoTemplate { name, .. } => {
                write!(
                    f,
                    "Function '{}' is already defined and cannot be a template function",
                    name
                )
            }
            Self::UndefinedVariable { name, .. } => {
                write!(f, "Undefined variable '{}'", name)
            }
            Self::Other { message, .. } => {
                write!(f, "{}", message)
            }
        }
    }
}

impl SemanticError {
    pub fn loc(&self) -> Loc {
        match self {
            Self::TypeMismatch { loc, .. } => *loc,
            Self::NoOverloadFound { loc, .. } => *loc,
            Self::AmbiguousFunctionCall { loc, .. } => *loc,
            Self::FunctionAlreadyDefined { loc, .. } => *loc,
            Self::FunctionAlreadyDefinedNoTemplate { loc, .. } => *loc,
            Self::UndefinedVariable { loc, .. } => *loc,
            Self::Other { loc, .. } => *loc,
        }
    }
}
