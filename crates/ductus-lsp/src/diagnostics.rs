//! Problems in one document: syntax errors, then lowering and evaluation
//! errors. Evaluation runs with default params and stops at its first error;
//! without errors, the evaluated document is the result.

use crate::importer::{Read, WorkspaceImporter};
use ductus_core::ast::Span;
use ductus_core::doc::Document;
use ductus_core::{eval, lower, parse};
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use tree_sitter::Tree;

#[derive(Debug, PartialEq)]
pub struct Problem {
    /// In bytes.
    pub range: Range<usize>,
    pub message: String,
}

pub fn check(tree: &Tree, source: &str, path: &Path, read: Read) -> Result<Document, Vec<Problem>> {
    let errors = parse::syntax_errors(tree, source);
    if !errors.is_empty() {
        // Lowering an ERROR tree fails anyway; the syntax errors say why.
        return Err(errors
            .into_iter()
            .map(|error| Problem {
                range: error.start_byte..error.end_byte,
                message: error.message,
            })
            .collect());
    }
    let program = lower::lower(tree, source)
        .map_err(|error| vec![at(tree, source, error.span, error.message)])?;
    let importer = WorkspaceImporter { root: path, read };
    eval::evaluate(&program, &HashMap::new(), &importer)
        .map_err(|error| vec![at(tree, source, error.span, error.message)])
}

/// A `Span` holds only where a construct starts. Underline the smallest
/// named node there: the identifier for a binding, the declaration for a
/// statement-level complaint.
fn at(tree: &Tree, source: &str, span: Span, message: String) -> Problem {
    let line_start: usize = source
        .split_inclusive('\n')
        .take(span.line.saturating_sub(1))
        .map(str::len)
        .sum();
    let offset = (line_start + span.column.saturating_sub(1)).min(source.len());
    let range = tree
        .root_node()
        .named_descendant_for_byte_range(offset, offset)
        .filter(|node| node.kind() != "source_file")
        .map_or(offset..offset, |node| node.byte_range());
    Problem { range, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::parse::parse_lenient;

    fn problems(source: &str) -> Vec<(String, String)> {
        let tree = parse_lenient(source);
        let read = |_: &Path| None;
        check(&tree, source, Path::new("/virtual/main.ductus"), &read)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|problem| (source[problem.range].to_string(), problem.message))
            .collect()
    }

    #[test]
    fn clean_file_has_no_problems() {
        assert_eq!(problems("u = 100;\nv = u * 2;\n"), []);
    }

    #[test]
    fn reports_every_syntax_error() {
        let found = problems("u = 100 200;\nv = 300 400;\n");
        assert!(found.len() >= 2, "{found:?}");
    }

    #[test]
    fn evaluation_error_underlines_the_binding() {
        let found = problems("u = 1;\nu = 2;\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "u");
        assert!(found[0].1.contains("already defined"), "{:?}", found[0].1);
    }

    #[test]
    fn evaluation_error_after_malayalam_text() {
        let found = problems("# മലയാളം\nx = y;\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "y");
    }

    #[test]
    fn unreadable_import_is_reported_on_the_import() {
        let found = problems("import \"nowhere.ductus\";\n");
        assert_eq!(found.len(), 1);
        assert!(found[0].0.starts_with("import"), "{found:?}");
        assert!(found[0].1.contains("cannot read"), "{found:?}");
    }
}
