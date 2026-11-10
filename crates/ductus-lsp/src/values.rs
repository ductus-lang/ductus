//! Evaluated values of bindings, read from an evaluated document.
//!
//! The document keeps numbers, params, metrics, and each glyph's nodes,
//! contours, guides, anchors and advance. Other values (points, pens, plain
//! paths, top-level nodes) are not kept, and bindings in `def` and `for`
//! bodies have no single value, so those have none here.

use crate::syntax::text;
use ductus_core::doc::{Document, Glyph};
use kurbo::Point;
use tree_sitter::Node;

/// The value `declaration` gives `name`, as text.
pub fn value(document: &Document, declaration: Node, name: &str, source: &str) -> Option<String> {
    let in_metrics = declaration
        .parent()
        .is_some_and(|parent| parent.kind() == "metrics_block");
    match (glyph_of(declaration, source)?, declaration.kind()) {
        (_, "param_declaration") => {
            let param = document.params.iter().find(|param| param.name == name)?;
            Some(format!(
                "{} in {}..{}",
                number(param.value),
                number(param.min),
                number(param.max)
            ))
        }
        (None, "assignment") if in_metrics => document.metrics.get(name).map(number),
        (None, "assignment" | "destructuring_assignment") => numeric(&document.bindings, name),
        (Some(glyph), kind) => {
            let glyph = document
                .glyphs
                .iter()
                .find(|candidate| candidate.name == glyph)?;
            glyph_value(glyph, kind, name)
        }
        _ => None,
    }
}

fn glyph_value(glyph: &Glyph, kind: &str, name: &str) -> Option<String> {
    match kind {
        "assignment" if name == "advance" => Some(number(glyph.advance)),
        "assignment" | "destructuring_assignment" => numeric(&glyph.bindings, name),
        "node_declaration" => find(&glyph.nodes, name).map(|node| {
            format!(
                "pos {}, dir {}°, width {}",
                point(node.pos),
                number(node.dir),
                number(node.width())
            )
        }),
        "contour_declaration" => find(&glyph.contours, name)
            .map(|path| format!("closed path, {}", points(path.points.len()))),
        "guide_declaration" => find(&glyph.guides, name).map(|path| {
            let shape = if path.closed { "closed" } else { "open" };
            format!("{shape} path, {}", points(path.points.len()))
        }),
        "anchor_declaration" => find(&glyph.anchors, name).map(|at| point(*at)),
        _ => None,
    }
}

/// `Some(None)` at the top level, `Some(Some(glyph))` in a glyph body, and
/// `None` in a `def` or `for` body, where a binding has no single value.
fn glyph_of(declaration: Node, source: &str) -> Option<Option<String>> {
    let mut node = declaration.parent();
    while let Some(ancestor) = node {
        match ancestor.kind() {
            "block" => return None,
            "glyph_block" => {
                let name = ancestor.child_by_field_name("name")?;
                return Some(Some(text(name, source).trim_matches('"').to_string()));
            }
            _ => node = ancestor.parent(),
        }
    }
    Some(None)
}

fn numeric(bindings: &[(String, f64)], name: &str) -> Option<String> {
    find(bindings, name).map(|value| number(*value))
}

fn find<'a, T>(named: &'a [(String, T)], name: &str) -> Option<&'a T> {
    named
        .iter()
        .find(|(found, _)| found == name)
        .map(|(_, value)| value)
}

/// At most four decimals, without trailing zeros: `680`, `0.0678`.
pub fn number(value: f64) -> String {
    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    match trimmed {
        "-0" => "0".to_string(),
        _ => trimmed.to_string(),
    }
}

fn point(at: Point) -> String {
    format!("({}, {})", number(at.x), number(at.y))
}

fn points(count: usize) -> String {
    format!("{count} point{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax;
    use ductus_core::parse::parse_lenient;
    use ductus_core::{eval, lower};
    use std::collections::HashMap;

    const SOURCE: &str = "params {\n  weight = 400 in 100..900;\n}\n\
        metrics {\n  x = 500;\n  over = 10 + weight / 200;\n}\n\
        stem = 40 + 0.14 * weight;\n\
        spot = (1, 2);\n\
        def f(c) -> p {\n  p = c * 2;\n}\n\
        glyph o {\n  advance = 2 * stem;\n  lsb = advance / 3;\n  \
        node n = (10, 20) dir up pen pen(stem);\n  \
        contour main = (0, 0) -- (1, 0) -- (1, 1) -- cycle;\n  \
        guide g = (0, 0) -- (5, 5);\n  anchor top = (250, 700);\n}\n";

    /// The value of the binding whose name starts at the first `needle`.
    fn value_at(needle: &str) -> Option<String> {
        let tree = parse_lenient(SOURCE);
        let program = lower::lower(&tree, SOURCE).unwrap();
        let document = eval::evaluate(&program, &HashMap::new(), &eval::NoImporter).unwrap();
        let start = SOURCE.find(needle).unwrap();
        let name = tree
            .root_node()
            .descendant_for_byte_range(start, start)
            .unwrap();
        value(
            &document,
            name.parent().unwrap(),
            syntax::text(name, SOURCE),
            SOURCE,
        )
    }

    #[test]
    fn top_level_values() {
        assert_eq!(value_at("weight =").as_deref(), Some("400 in 100..900"));
        assert_eq!(value_at("over").as_deref(), Some("12"));
        assert_eq!(value_at("stem =").as_deref(), Some("96"));
    }

    #[test]
    fn glyph_values() {
        assert_eq!(value_at("advance =").as_deref(), Some("192"));
        assert_eq!(value_at("lsb").as_deref(), Some("64"));
        assert_eq!(
            value_at("n =").as_deref(),
            Some("pos (10, 20), dir 90°, width 96")
        );
        assert_eq!(value_at("main").as_deref(), Some("closed path, 3 points"));
        assert_eq!(value_at("g =").as_deref(), Some("open path, 2 points"));
        assert_eq!(value_at("top").as_deref(), Some("(250, 700)"));
    }

    #[test]
    fn values_the_document_does_not_keep() {
        assert_eq!(value_at("spot"), None);
        assert_eq!(value_at("p = c"), None);
    }

    #[test]
    fn numbers_read_compactly() {
        assert_eq!(number(680.0), "680");
        assert_eq!(number(0.06781), "0.0678");
        assert_eq!(number(-0.00001), "0");
    }
}
