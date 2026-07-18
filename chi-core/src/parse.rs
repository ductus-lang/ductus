//! Parse Chi source with tree-sitter and reject files with syntax errors.

use tree_sitter::{Node, Parser, Tree};

#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
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

/// Parse `source`, returning the tree only when it contains no errors.
pub fn parse(source: &str) -> Result<Tree, SyntaxError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_chi::LANGUAGE.into())
        .expect("chi grammar should load");
    let tree = parser
        .parse(source, None)
        .expect("tree-sitter always returns a tree without a timeout");

    match first_error(tree.root_node(), source) {
        Some(error) => Err(error),
        None => Ok(tree),
    }
}

fn first_error(node: Node, source: &str) -> Option<SyntaxError> {
    if !node.has_error() {
        return None;
    }
    if node.is_error() || node.is_missing() {
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
        return Some(SyntaxError {
            line: position.row + 1,
            column: position.column + 1,
            message,
        });
    }
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    children
        .into_iter()
        .find_map(|child| first_error(child, source))
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
}
