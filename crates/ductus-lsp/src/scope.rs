//! The names in scope: what a file's top level binds, imports included,
//! and what enclosing glyph, `def` and `for` bodies add at a point.
//!
//! Spec.md §8: a name must be bound before it is used, so value bindings
//! count only when their declaration ends before the point. An `import`
//! binds the imported file's names where it stands.

use crate::definition::Target;
use crate::importer::{self, Read};
use crate::syntax::{self, text};
use ductus_core::parse::parse_lenient;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Tree};

#[derive(Debug, Clone)]
pub struct Named {
    pub name: String,
    /// A `def`. Defs and values are separate namespaces.
    pub is_def: bool,
    /// What declares it: `param`, `metric`, `node`, `contour`, `guide`,
    /// `def`, `parameter`, `loop variable` or `binding`.
    pub what: &'static str,
    /// The first line of the declaration.
    pub summary: String,
    /// A `def`'s parameter names.
    pub parameters: Vec<String>,
    /// A `def`'s description: the comment directly above it.
    pub doc: Option<String>,
    pub target: Target,
}

/// Everything the top level `root` of the file at `path` binds, in order,
/// with imports expanded where they stand, include-once. Values count only
/// when their declaration ends before `limit`; defs always, since a call
/// inside a `def` body runs after the whole file is read.
pub fn top_level(
    root: Node,
    source: &str,
    path: &Path,
    limit: Option<usize>,
    read: Read,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<Named>,
) {
    let before = |node: Node| limit.is_none_or(|limit| node.end_byte() <= limit);
    for statement in syntax::children(root) {
        match statement.kind() {
            "import_statement" if before(statement) => {
                if let Some(imported) = statement.child_by_field_name("path") {
                    let file = importer::resolve(path, text(imported, source).trim_matches('"'));
                    file_top_level(&file, read, visited, out);
                }
            }
            "def_declaration" => {
                if let Some(name) = statement.child_by_field_name("name") {
                    out.push(named(name, statement, "def", source, path));
                }
            }
            _ => {
                for binding in syntax::bindings(statement) {
                    if before(binding.declaration) {
                        let what = what(binding.declaration);
                        out.push(named(binding.name, binding.declaration, what, source, path));
                    }
                }
            }
        }
    }
}

/// [`top_level`] of a file read through `read`, unless already visited.
pub fn file_top_level(
    path: &Path,
    read: Read,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<Named>,
) {
    if !visited.insert(path.to_path_buf()) {
        return;
    }
    let Some(source) = read(path) else {
        return;
    };
    let tree = parse_lenient(&source);
    top_level(tree.root_node(), &source, path, None, read, visited, out);
}

/// Every name visible at byte `offset`, innermost scope first. A name
/// shadowed by an inner binding appears once, as the inner one.
pub fn visible(tree: &Tree, source: &str, path: &Path, offset: usize, read: Read) -> Vec<Named> {
    let mut out = Vec::new();
    let mut limit = Some(offset);
    let mut node = Some(start_at(tree, source, offset));
    while let Some(scope) = node {
        match scope.kind() {
            // An ERROR node holds the statements of a block still being
            // typed, one without its closing `}`.
            "block" | "glyph_block" | "ERROR" => {
                let before = |node: Node| limit.is_none_or(|limit| node.end_byte() <= limit);
                for binding in syntax::children(scope)
                    .into_iter()
                    .flat_map(syntax::bindings)
                {
                    if before(binding.declaration) {
                        let what = what(binding.declaration);
                        out.push(named(binding.name, binding.declaration, what, source, path));
                    }
                }
                if let Some(owner) = scope.parent().filter(|_| scope.kind() == "block") {
                    let what = match owner.kind() {
                        "for_statement" => "loop variable",
                        _ => "parameter",
                    };
                    for name in syntax::owner_bindings(scope) {
                        out.push(named(name, owner, what, source, path));
                    }
                    if owner.kind() == "def_declaration" {
                        limit = None;
                    }
                }
            }
            "source_file" => {
                let mut visited = HashSet::from([path.to_path_buf()]);
                top_level(scope, source, path, limit, read, &mut visited, &mut out);
            }
            _ => {}
        }
        node = scope.parent();
    }
    let mut seen = HashSet::new();
    out.retain(|named| seen.insert((named.name.clone(), named.is_def)));
    out
}

/// The node to start a scope walk from at `offset`. Past the end of an
/// unfinished block, say at the end of the file, the cursor is outside the
/// ERROR node that holds the block, so start from the token before it,
/// unless that token closes a block.
fn start_at<'tree>(tree: &'tree Tree, source: &str, offset: usize) -> Node<'tree> {
    let root = tree.root_node();
    let at = root
        .descendant_for_byte_range(offset, offset)
        .unwrap_or(root);
    if at != root {
        return at;
    }
    let previous = source[..offset].trim_end().len().checked_sub(1);
    previous
        .and_then(|previous| root.descendant_for_byte_range(previous, previous))
        .filter(|token| token.kind() != "}")
        .unwrap_or(root)
}

fn what(declaration: Node) -> &'static str {
    match declaration.kind() {
        "param_declaration" => "param",
        "node_declaration" => "node",
        "contour_declaration" => "contour",
        "guide_declaration" => "guide",
        "assignment"
            if declaration
                .parent()
                .is_some_and(|parent| parent.kind() == "metrics_block") =>
        {
            "metric"
        }
        _ => "binding",
    }
}

fn named(name: Node, declaration: Node, what: &'static str, source: &str, path: &Path) -> Named {
    let is_def = what == "def";
    let parameters = match is_def {
        true => declaration
            .child_by_field_name("parameters")
            .map(syntax::children)
            .unwrap_or_default()
            .into_iter()
            .map(|parameter| text(parameter, source).to_string())
            .collect(),
        false => Vec::new(),
    };
    let doc = is_def
        .then(|| syntax::leading_comments(declaration, source))
        .flatten();
    let summary = text(declaration, source).lines().next().unwrap_or_default();
    Named {
        name: text(name, source).to_string(),
        is_def,
        what,
        summary: summary.trim().to_string(),
        parameters,
        doc,
        target: Target {
            path: path.to_path_buf(),
            range: name.byte_range(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const MAIN: &str = "/virtual/main.ductus";

    /// `name what` for every name visible at the `|` in `source`.
    fn visible_at(files: &[(&str, &str)], source: &str) -> Vec<String> {
        let offset = source.find('|').unwrap();
        let source = source.replacen('|', "", 1);
        let texts: HashMap<PathBuf, String> = files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.to_string()))
            .collect();
        let read = |path: &Path| texts.get(path).cloned();
        let tree = parse_lenient(&source);
        visible(&tree, &source, Path::new(MAIN), offset, &read)
            .into_iter()
            .map(|named| format!("{} {}", named.name, named.what))
            .collect()
    }

    #[test]
    fn innermost_first_and_only_what_is_bound_before() {
        let source = "params {\n  weight = 400 in 100..900;\n}\nmetrics {\n  x = 500;\n}\nstem = 1;\nglyph o {\n  stem = 2;\n  node n = (0, 0) dir up pen pen(1);\n  advance = |;\n  late = 3;\n}\nlater = 4;\n";
        assert_eq!(
            visible_at(&[], source),
            ["stem binding", "n node", "weight param", "x metric"]
        );
    }

    #[test]
    fn def_bodies_see_parameters_and_the_whole_top_level() {
        let source = "def f(c) -> p {\n  p = c + |;\n}\nlate = 1;\n";
        assert_eq!(
            visible_at(&[], source),
            ["c parameter", "f def", "late binding"]
        );
    }

    #[test]
    fn loop_variables_and_imports() {
        let lib = "unit = 10;\ndef bowl(c) -> c {\n}\n";
        let source = "import \"lib.ductus\";\nfor i in 1..2 {\n  draw(|);\n}\n";
        assert_eq!(
            visible_at(&[("/virtual/lib.ductus", lib)], source),
            ["i loop variable", "unit binding", "bowl def"]
        );
    }
}
