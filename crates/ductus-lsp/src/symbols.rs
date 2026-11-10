//! The document outline: params, metrics, defs, glyphs and top-level
//! bindings, with each glyph's own bindings nested under it.

use crate::syntax::{self, Binding, text};
use lsp_types::SymbolKind;
use std::ops::Range;
use tree_sitter::{Node, Tree};

#[derive(Debug, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    /// The whole declaration, in bytes.
    pub range: Range<usize>,
    /// The name, in bytes.
    pub selection: Range<usize>,
    pub children: Vec<Symbol>,
}

pub fn symbols(tree: &Tree, source: &str) -> Vec<Symbol> {
    syntax::children(tree.root_node())
        .into_iter()
        .flat_map(|statement| top_level(statement, source))
        .collect()
}

/// True when `query`'s characters occur in `name` in order, ignoring
/// case: `bnp` matches `broad_nib_pen`. An empty query matches all.
pub fn matches(query: &str, name: &str) -> bool {
    let mut name = name.chars().map(|c| c.to_ascii_lowercase());
    query
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .all(|wanted| name.any(|c| c == wanted))
}

fn top_level(statement: Node, source: &str) -> Vec<Symbol> {
    match statement.kind() {
        "params_block" => binding_symbols(statement, source, SymbolKind::CONSTANT, Some("param")),
        "metrics_block" => binding_symbols(statement, source, SymbolKind::CONSTANT, Some("metric")),
        "def_declaration" => {
            let field = |name| statement.child_by_field_name(name);
            let Some(name) = field("name") else {
                return Vec::new();
            };
            let detail = match (field("parameters"), field("returns")) {
                (Some(parameters), Some(returns)) => Some(format!(
                    "{} -> {}",
                    text(parameters, source),
                    text(returns, source)
                )),
                _ => None,
            };
            vec![symbol(
                statement,
                name,
                source,
                SymbolKind::FUNCTION,
                detail,
            )]
        }
        "glyph_block" => {
            let Some(name) = statement.child_by_field_name("name") else {
                return Vec::new();
            };
            let detail = statement
                .child_by_field_name("unicode")
                .map(|unicode| text(unicode, source).to_string());
            let mut glyph = symbol(statement, name, source, SymbolKind::CLASS, detail);
            glyph.name = glyph.name.trim_matches('"').to_string();
            glyph.children = syntax::children(statement)
                .into_iter()
                .flat_map(|child| glyph_child(child, source))
                .collect();
            vec![glyph]
        }
        "import_statement" => statement
            .child_by_field_name("path")
            .map(|path| {
                let mut import = symbol(statement, path, source, SymbolKind::FILE, None);
                import.name = import.name.trim_matches('"').to_string();
                import
            })
            .into_iter()
            .collect(),
        "node_declaration" => {
            binding_symbols(statement, source, SymbolKind::VARIABLE, Some("node"))
        }
        _ => binding_symbols(statement, source, SymbolKind::VARIABLE, None),
    }
}

fn glyph_child(statement: Node, source: &str) -> Vec<Symbol> {
    let detail = match statement.kind() {
        "contour_declaration" => Some("contour"),
        "guide_declaration" => Some("guide"),
        "node_declaration" => Some("node"),
        "anchor_declaration" => {
            return statement
                .child_by_field_name("name")
                .map(|name| {
                    let detail = Some("anchor".to_string());
                    symbol(statement, name, source, SymbolKind::VARIABLE, detail)
                })
                .into_iter()
                .collect();
        }
        _ => None,
    };
    binding_symbols(statement, source, SymbolKind::VARIABLE, detail)
}

fn binding_symbols(
    statement: Node,
    source: &str,
    kind: SymbolKind,
    detail: Option<&str>,
) -> Vec<Symbol> {
    syntax::bindings(statement)
        .into_iter()
        .map(|Binding { name, declaration }| {
            symbol(declaration, name, source, kind, detail.map(str::to_string))
        })
        .collect()
}

fn symbol(
    declaration: Node,
    name: Node,
    source: &str,
    kind: SymbolKind,
    detail: Option<String>,
) -> Symbol {
    Symbol {
        name: text(name, source).to_string(),
        detail,
        kind,
        range: declaration.byte_range(),
        selection: name.byte_range(),
        children: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::parse::parse_lenient;

    const SOURCE: &str = r#"import "common.ductus";
params {
  weight = 400 in 100..900;
}
metrics {
  x = 500;
}
stem = 40 + 0.14 * weight;
(a, b) = pair(1);
def bar(c, r) -> p {
  p = c;
}
glyph o : U+006F {
  advance = 500;
  node n = (0, 0) dir up pen pen(stem);
  contour main = (0, 0) -- (1, 0) -- cycle;
  guide g = (0, 0) -- (1, 1);
  anchor top = (250, 700);
  for i in 1..2 { hidden = i; }
}
"#;

    fn outline(symbols: &[Symbol]) -> Vec<String> {
        symbols
            .iter()
            .map(|symbol| {
                let detail = symbol.detail.as_deref().unwrap_or("-");
                format!("{} {:?} {}", symbol.name, symbol.kind, detail)
            })
            .collect()
    }

    #[test]
    fn outlines_a_file() {
        let symbols = symbols(&parse_lenient(SOURCE), SOURCE);
        assert_eq!(
            outline(&symbols),
            [
                "common.ductus File -",
                "weight Constant param",
                "x Constant metric",
                "stem Variable -",
                "a Variable -",
                "b Variable -",
                "bar Function (c, r) -> p",
                "o Class U+006F",
            ]
        );
        assert_eq!(
            outline(&symbols[7].children),
            [
                "advance Variable -",
                "n Variable node",
                "main Variable contour",
                "g Variable guide",
                "top Variable anchor",
            ]
        );
    }

    #[test]
    fn selection_is_the_name_inside_the_declaration() {
        let symbols = symbols(&parse_lenient(SOURCE), SOURCE);
        let stem = &symbols[3];
        assert_eq!(&SOURCE[stem.selection.clone()], "stem");
        assert_eq!(&SOURCE[stem.range.clone()], "stem = 40 + 0.14 * weight;");
    }

    #[test]
    fn query_matches_a_subsequence_ignoring_case() {
        assert!(matches("", "stem"));
        assert!(matches("bnp", "broad_nib_pen"));
        assert!(matches("STEM", "stem_width"));
        assert!(!matches("pnb", "broad_nib_pen"));
        assert!(!matches("stems", "stem"));
    }

    #[test]
    fn survives_syntax_errors() {
        let source = "a = 1;\nb = = 2;\nglyph o {\n  c = 3;\n}\n";
        let names: Vec<String> = symbols(&parse_lenient(source), source)
            .into_iter()
            .map(|symbol| symbol.name)
            .collect();
        assert!(names.contains(&"a".to_string()), "{names:?}");
        assert!(names.contains(&"o".to_string()), "{names:?}");
    }
}
