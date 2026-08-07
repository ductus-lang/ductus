//! Parse Ductus source with tree-sitter and reject files with syntax errors.

use tree_sitter::{Node, Parser, Tree};

#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    /// UTF-8 byte offset of the error span's start.
    pub start_byte: usize,
    /// UTF-8 byte offset of the error span's end.
    pub end_byte: usize,
    pub message: String,
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "syntax error at line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for SyntaxError {}

/// Parse `source` into a tree, keeping it even when syntax errors are present.
pub fn parse_lenient(source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ductus::LANGUAGE.into())
        .expect("ductus grammar should load");
    parser
        .parse(source, None)
        .expect("tree-sitter always returns a tree without a timeout")
}

/// Parse `source`, returning the tree only when it contains no errors.
pub fn parse(source: &str) -> Result<Tree, SyntaxError> {
    let tree = parse_lenient(source);
    match syntax_errors(&tree, source).into_iter().next() {
        Some(error) => Err(error),
        None => Ok(tree),
    }
}

/// All ERROR and MISSING nodes in `tree`, in document order.
pub fn syntax_errors(tree: &Tree, source: &str) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    collect_errors(tree.root_node(), source, &mut errors);
    errors
}

fn collect_errors(node: Node, source: &str, errors: &mut Vec<SyntaxError>) {
    if !node.has_error() {
        return;
    }
    if node.is_error() || node.is_missing() {
        errors.push(syntax_error(node, source));
        return;
    }
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    for child in children {
        collect_errors(child, source, errors);
    }
}

fn syntax_error(node: Node, source: &str) -> SyntaxError {
    let position = node.start_position();
    let message = if node.is_missing() {
        format!("missing {}", node.kind())
    } else {
        let text = node.utf8_text(source.as_bytes()).unwrap_or_default();
        let mut snippet: String = text.chars().take(20).collect();
        if snippet.len() < text.len() {
            snippet.push('…');
        }
        format!("unexpected input: {snippet:?}")
    };
    SyntaxError {
        line: position.row + 1,
        column: position.column + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_simple_assignment() {
        let tree = parse("u = 100;\n").unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn parses_a_glyph_block() {
        let source = "glyph o : U+006F {\n  advance = 100;\n  contour c = a .. b .. cycle;\n}\n";
        assert!(parse(source).is_ok());
    }

    #[test]
    fn reports_missing_semicolon_with_location() {
        let error = parse("u = 100\nv = 2;\n").unwrap_err();
        assert_eq!(error.line, 2, "error should point into the second line");
    }

    #[test]
    fn reports_unexpected_input() {
        let error = parse("u = 100 200;\n").unwrap_err();
        assert_eq!(error.line, 1);
        assert!(error.message.contains("unexpected") || error.message.contains("missing"));
    }

    #[test]
    fn syntax_errors_is_empty_for_clean_source() {
        let tree = parse_lenient("u = 100;\nv = 2;\n");
        assert!(syntax_errors(&tree, "u = 100;\nv = 2;\n").is_empty());
    }

    #[test]
    fn syntax_errors_collects_multiple_broken_statements() {
        let source = "u = 100 200;\nv = 300 400;\n";
        let tree = parse_lenient(source);
        let errors = syntax_errors(&tree, source);
        assert!(errors.len() >= 2, "expected >=2 errors, got {errors:?}");
        assert_eq!(errors[0].line, 1);
        assert_eq!(errors[1].line, 2);
    }

    #[test]
    fn syntax_error_carries_byte_range() {
        let source = "u = 100 200;\n";
        let tree = parse_lenient(source);
        let errors = syntax_errors(&tree, source);
        let error = &errors[0];
        assert!(error.start_byte < error.end_byte);
        assert!(error.end_byte <= source.len());
    }
}
