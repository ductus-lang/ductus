//! Inlay hints: the evaluated value at the end of each computed binding.
//!
//! Only bindings the document keeps a number for, at the top level, in
//! `metrics` and in glyph bodies. A binding written as a literal, `x = 500;`,
//! gets none: the hint would repeat the code.

use crate::syntax::{self, text};
use crate::values;
use ductus_core::doc::Document;
use std::ops::Range;
use tree_sitter::{Node, Tree};

#[derive(Debug, PartialEq)]
pub struct Hint {
    /// Where the hint goes: the end of the statement, in bytes.
    pub offset: usize,
    pub label: String,
}

/// Hints for the statements that overlap `range`.
pub fn hints(tree: &Tree, source: &str, document: &Document, range: Range<usize>) -> Vec<Hint> {
    let mut statements = Vec::new();
    for statement in syntax::children(tree.root_node()) {
        match statement.kind() {
            "metrics_block" | "glyph_block" => statements.extend(syntax::children(statement)),
            _ => statements.push(statement),
        }
    }
    statements
        .into_iter()
        .filter(|statement| {
            statement.start_byte() <= range.end && statement.end_byte() >= range.start
        })
        .filter(|statement| is_computed(*statement))
        .filter_map(|statement| hint(statement, source, document))
        .collect()
}

fn is_computed(statement: Node) -> bool {
    match statement.kind() {
        "assignment" => !statement
            .child_by_field_name("value")
            .is_some_and(is_literal),
        "destructuring_assignment" => true,
        _ => false,
    }
}

/// `500` or `-20`.
fn is_literal(value: Node) -> bool {
    match value.kind() {
        "number" => true,
        "unary_expression" => value
            .child_by_field_name("operand")
            .is_some_and(|operand| operand.kind() == "number"),
        _ => false,
    }
}

fn hint(statement: Node, source: &str, document: &Document) -> Option<Hint> {
    let bindings = syntax::bindings(statement);
    let values: Vec<(&str, String)> = bindings
        .iter()
        .filter_map(|binding| {
            let name = text(binding.name, source);
            values::value(document, statement, name, source).map(|value| (name, value))
        })
        .collect();
    let label = match (statement.kind(), values.as_slice()) {
        (_, []) => return None,
        ("assignment", [(_, value)]) => value.clone(),
        _ => values
            .iter()
            .map(|(name, value)| format!("{name} = {value}"))
            .collect::<Vec<_>>()
            .join(", "),
    };
    Some(Hint {
        offset: statement.end_byte(),
        label: format!("▸ {label}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::parse::parse_lenient;
    use ductus_core::{eval, lower};
    use std::collections::HashMap;

    const SOURCE: &str = "params {\n  weight = 400 in 100..900;\n}\n\
        metrics {\n  x = 500;\n  over = 10 + weight / 200;\n}\n\
        stem = 40 + 0.14 * weight;\n\
        drop = -20;\n\
        spot = (1, 2);\n\
        def pair(a) -> (b, c) {\n  b = a;\n  c = a * 2;\n}\n\
        (h, v) = pair(stem);\n\
        glyph o {\n  advance = 2 * stem;\n  lsb = 10;\n  node n = (0, 0) dir up pen pen(1);\n}\n";

    fn all(range: Range<usize>) -> Vec<(String, String)> {
        let tree = parse_lenient(SOURCE);
        let program = lower::lower(&tree, SOURCE).unwrap();
        let document = eval::evaluate(&program, &HashMap::new(), &eval::NoImporter).unwrap();
        hints(&tree, SOURCE, &document, range)
            .into_iter()
            .map(|hint| {
                let line_start = SOURCE[..hint.offset].rfind('\n').map_or(0, |i| i + 1);
                (
                    SOURCE[line_start..hint.offset].trim().to_string(),
                    hint.label,
                )
            })
            .collect()
    }

    fn pair(line: &str, label: &str) -> (String, String) {
        (line.to_string(), label.to_string())
    }

    #[test]
    fn computed_numbers_only() {
        assert_eq!(
            all(0..SOURCE.len()),
            [
                pair("over = 10 + weight / 200;", "▸ 12"),
                pair("stem = 40 + 0.14 * weight;", "▸ 96"),
                pair("(h, v) = pair(stem);", "▸ h = 96, v = 192"),
                pair("advance = 2 * stem;", "▸ 192"),
            ]
        );
    }

    #[test]
    fn only_in_the_requested_range() {
        let start = SOURCE.find("glyph").unwrap();
        assert_eq!(
            all(start..SOURCE.len()),
            [pair("advance = 2 * stem;", "▸ 192")]
        );
    }
}
