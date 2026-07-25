use std::path::PathBuf;

use diagnostic::DiagnosticReporter;

use crate::ast::FileItem;
use crate::parse_file;

#[test]
fn feature_type_defs() {
    let source = "type Point struct {
        x: int,
        y: int
    }

    type NUMBERS enum {
        ONE,
        TWO,
    }

    type Seconds float
    type Dollar #distinct float";

    let mut reporter = DiagnosticReporter::new();

    let file_path = PathBuf::from("<feature_type_defs>");
    match parse_file(file_path.as_path(), source, &mut reporter) {
        Ok(file_ast) => {
            assert!(matches!(&file_ast.items[0], FileItem::Struct(_)));
            assert!(matches!(&file_ast.items[1], FileItem::Enum(_)));
            assert!(matches!(&file_ast.items[2], FileItem::TypeDecl(_)));
            assert!(matches!(&file_ast.items[3], FileItem::TypeDecl(_)));
        }
        Err(_) => {
            assert!(false, "{}", reporter.into_string());
        }
    }
}

#[test]
fn feature_generic_types() {
    let source = "type Point struct[$T]{
        x: $T,
        y: $T
    }

    fn test(p: Point[int]){}";

    let mut reporter = DiagnosticReporter::new();

    let file_path = PathBuf::from("<feature_generic_types>");
    match parse_file(file_path.as_path(), source, &mut reporter) {
        Ok(file_ast) => {
            assert!(matches!(&file_ast.items[0], FileItem::Struct(_)));
            assert!(matches!(&file_ast.items[1], FileItem::FunctionDecl(_)));
        }
        Err(_) => {
            assert!(false, "{}", reporter.into_string());
        }
    }
}
