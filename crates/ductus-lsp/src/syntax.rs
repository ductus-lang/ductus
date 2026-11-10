//! Small helpers over the tree-sitter syntax tree.

use tree_sitter::Node;

pub fn text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

/// A name bound by a statement: the identifier, and the declaration it
/// belongs to. For `params` and `metrics` blocks the declaration is the
/// single entry, so that "bound before the reference" holds per entry.
#[derive(Debug, Clone, Copy)]
pub struct Binding<'tree> {
    pub name: Node<'tree>,
    pub declaration: Node<'tree>,
}

/// Every name `statement` binds in its scope, in source order. Anchors bind
/// nothing; `for` and `def` bind only inside their own bodies.
pub fn bindings<'tree>(statement: Node<'tree>) -> Vec<Binding<'tree>> {
    let named = |name: Option<Node<'tree>>| {
        name.map(|name| Binding {
            name,
            declaration: statement,
        })
    };
    match statement.kind() {
        "assignment"
        | "node_declaration"
        | "contour_declaration"
        | "guide_declaration"
        | "param_declaration" => named(statement.child_by_field_name("name"))
            .into_iter()
            .collect(),
        "destructuring_assignment" => statement
            .child_by_field_name("pattern")
            .map(|pattern| {
                let mut cursor = pattern.walk();
                pattern
                    .named_children(&mut cursor)
                    .filter(|child| child.kind() == "identifier")
                    .map(|name| Binding {
                        name,
                        declaration: statement,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "params_block" | "metrics_block" => {
            let mut cursor = statement.walk();
            statement
                .named_children(&mut cursor)
                .flat_map(bindings)
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Named children of `node`, collected so callers can recurse freely.
pub fn children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
