//! Small helpers over the tree-sitter syntax tree.

use tree_sitter::Node;

/// Spec.md §2. Keywords cannot name a binding; `pen`, `draw` and `fill`
/// are also built-in functions.
pub const KEYWORDS: &[&str] = &[
    "params", "in", "glyph", "contour", "guide", "anchor", "node", "dir", "pen", "cycle", "corner",
    "def", "for", "if", "else", "and", "or", "not", "import", "shifted", "scaled", "xscaled",
    "yscaled", "rotated", "slanted", "reversed", "metrics", "draw", "fill",
];

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

/// Names a block's owner binds for the block only: a `for` loop variable,
/// or a `def`'s parameters. Empty for any other node.
pub fn owner_bindings(block: Node) -> Vec<Node> {
    let Some(owner) = block.parent().filter(|_| block.kind() == "block") else {
        return Vec::new();
    };
    match owner.kind() {
        "for_statement" => owner.child_by_field_name("name").into_iter().collect(),
        "def_declaration" => owner
            .child_by_field_name("parameters")
            .map(children)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Every identifier under `root` spelled `name`, in document order.
pub fn identifiers<'tree>(root: Node<'tree>, source: &str, name: &str) -> Vec<Node<'tree>> {
    let mut found = Vec::new();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.kind() == "identifier" && text(node, source) == name {
            found.push(node);
        }
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return found;
            }
        }
    }
}

/// `text` with every `#` comment replaced by spaces, so byte offsets
/// still line up with `text`. A `#` inside a string (a glyph name) is
/// taken for a comment too; strings never hold code.
pub fn without_comments(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| match line.find('#') {
            Some(hash) => {
                let end = line.len() - usize::from(line.ends_with('\n'));
                let blank = " ".repeat(end - hash);
                format!("{}{blank}{}", &line[..hash], &line[end..])
            }
            None => line.to_string(),
        })
        .collect()
}

/// The comment lines directly above `node`, without their `#`.
pub fn leading_comments(node: Node, source: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut below = node;
    while let Some(comment) = below.prev_sibling().filter(|c| c.kind() == "comment") {
        if comment.end_position().row + 1 < below.start_position().row {
            break;
        }
        let line = text(comment, source).trim_start_matches('#');
        lines.push(line.strip_prefix(' ').unwrap_or(line));
        below = comment;
    }
    lines.reverse();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Named children of `node`, collected so callers can recurse freely.
pub fn children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
