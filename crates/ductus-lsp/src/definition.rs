//! Go to definition, resolved on the syntax tree.
//!
//! Spec.md §8: bindings are immutable, so a scope binds a name at most once;
//! inner scopes (glyph, `def` and `for` bodies) may shadow outer ones; and a
//! name must be bound before it is used. So resolution walks outward through
//! the enclosing scopes and takes the first binding that ends before the
//! reference. A `def` body runs at call time, so from inside one the file's
//! top level is searched without that ordering restriction.

use crate::importer::{self, Read};
use crate::scope;
use crate::syntax::{self, text};
use std::collections::HashSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Tree};

#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub path: PathBuf,
    /// In bytes, in the target file.
    pub range: Range<usize>,
}

/// The definition of the identifier (or import path) at byte `offset` of
/// the file at `path`.
pub fn definition(
    tree: &Tree,
    source: &str,
    path: &Path,
    offset: usize,
    read: Read,
) -> Option<Target> {
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;
    let parent = node.parent()?;
    let here = |node: Node| {
        Some(Target {
            path: path.to_path_buf(),
            range: node.byte_range(),
        })
    };

    if node.kind() == "string" && parent.kind() == "import_statement" {
        let file = importer::resolve(path, text(node, source).trim_matches('"'));
        read(&file)?;
        return Some(Target {
            path: file,
            range: 0..0,
        });
    }
    if node.kind() != "identifier" {
        return None;
    }

    let name = text(node, source);
    match parent.kind() {
        "member_expression" if parent.child_by_field_name("property") == Some(node) => None,
        "call_expression" => find_def(tree, source, path, name, read),
        "return_spec" => {
            let body = parent.parent()?.child_by_field_name("body")?;
            find_in(body, name, None, source).and_then(here)
        }
        _ if is_binding_site(node, parent) => here(node),
        _ => {
            let mut limit = Some(node.start_byte());
            let mut child = node;
            while let Some(scope) = child.parent() {
                match scope.kind() {
                    "block" | "glyph_block" => {
                        if let Some(found) = find_in(scope, name, limit, source) {
                            return here(found);
                        }
                        let owned = syntax::owner_bindings(scope);
                        if let Some(found) = owned.into_iter().find(|n| text(*n, source) == name) {
                            return here(found);
                        }
                        let owner = scope.parent().filter(|_| scope.kind() == "block");
                        if owner.is_some_and(|owner| owner.kind() == "def_declaration") {
                            limit = None;
                        }
                    }
                    "source_file" => {
                        return first_top_level(scope, source, path, limit, read, name, false);
                    }
                    _ => {}
                }
                child = scope;
            }
            None
        }
    }
}

/// The `def` named `name` in the file at `path` or what it imports.
pub fn find_def(tree: &Tree, source: &str, path: &Path, name: &str, read: Read) -> Option<Target> {
    first_top_level(tree.root_node(), source, path, None, read, name, true)
}

/// The first top-level binding (or `def`) of `name`, imports included.
fn first_top_level(
    root: Node,
    source: &str,
    path: &Path,
    limit: Option<usize>,
    read: Read,
    name: &str,
    is_def: bool,
) -> Option<Target> {
    let mut found = Vec::new();
    let mut visited = HashSet::from([path.to_path_buf()]);
    scope::top_level(root, source, path, limit, read, &mut visited, &mut found);
    found
        .into_iter()
        .find(|named| named.is_def == is_def && named.name == name)
        .map(|named| named.target)
}

/// Identifiers that are themselves the binding: definition is the name.
fn is_binding_site(node: Node, parent: Node) -> bool {
    match parent.kind() {
        "tuple_pattern" | "parameter_list" => true,
        "assignment"
        | "node_declaration"
        | "contour_declaration"
        | "guide_declaration"
        | "anchor_declaration"
        | "param_declaration"
        | "for_statement"
        | "def_declaration"
        | "glyph_block" => parent.child_by_field_name("name") == Some(node),
        _ => false,
    }
}

/// The binding of `name` among the statements of `scope`, ignoring
/// declarations that end after `limit`.
fn find_in<'tree>(
    scope: Node<'tree>,
    name: &str,
    limit: Option<usize>,
    source: &str,
) -> Option<Node<'tree>> {
    syntax::children(scope)
        .into_iter()
        .flat_map(syntax::bindings)
        .filter(|binding| limit.is_none_or(|limit| binding.declaration.end_byte() <= limit))
        .map(|binding| binding.name)
        .find(|binding| text(*binding, source) == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::parse::parse_lenient;
    use std::collections::HashMap;

    const MAIN: &str = "/virtual/main.ductus";

    /// Resolve the `nth` occurrence of `needle` in `source` and return the
    /// target's file and the byte offset of its start.
    fn resolve_in(
        files: &[(&str, &str)],
        source: &str,
        needle: &str,
        nth: usize,
    ) -> Option<(String, usize)> {
        let files: HashMap<PathBuf, String> = files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.to_string()))
            .collect();
        let read = |path: &Path| files.get(path).cloned();
        let offset = source
            .match_indices(needle)
            .nth(nth)
            .unwrap_or_else(|| panic!("no occurrence {nth} of {needle:?}"))
            .0;
        let tree = parse_lenient(source);
        definition(&tree, source, Path::new(MAIN), offset, &read)
            .map(|target| (target.path.display().to_string(), target.range.start))
    }

    fn resolve(source: &str, needle: &str, nth: usize) -> Option<usize> {
        resolve_in(&[], source, needle, nth).map(|(path, start)| {
            assert_eq!(path, MAIN);
            start
        })
    }

    /// Byte offset of the `nth` occurrence of `needle`.
    fn at(source: &str, needle: &str, nth: usize) -> usize {
        source.match_indices(needle).nth(nth).unwrap().0
    }

    #[test]
    fn top_level_binding() {
        let source = "stem = 40;\nhalf = stem / 2;\n";
        assert_eq!(resolve(source, "stem", 1), Some(at(source, "stem", 0)));
    }

    #[test]
    fn binding_site_is_its_own_definition() {
        let source = "stem = 40;\n";
        assert_eq!(resolve(source, "stem", 0), Some(0));
    }

    #[test]
    fn param_and_metric() {
        let source = "params {\n  weight = 400 in 100..900;\n}\nmetrics {\n  x = 500;\n  cap = x + 200;\n}\nstem = weight + x;\n";
        assert_eq!(resolve(source, "weight", 1), Some(at(source, "weight", 0)));
        assert_eq!(resolve(source, "x", 2), Some(at(source, "x", 0)));
        assert_eq!(resolve(source, "x", 1), Some(at(source, "x", 0)));
    }

    #[test]
    fn glyph_local_shadows_top_level() {
        let source = "w = 1;\nglyph o {\n  w = 2;\n  advance = w;\n}\n";
        assert_eq!(resolve(source, "w", 2), Some(at(source, "w", 1)));
    }

    #[test]
    fn rebinding_reads_the_outer_name() {
        let source = "x = 1;\nglyph o {\n  x = x + 1;\n}\n";
        assert_eq!(resolve(source, "x", 2), Some(at(source, "x", 0)));
    }

    #[test]
    fn use_before_binding_resolves_to_nothing() {
        let source = "a = b;\nb = 1;\n";
        assert_eq!(resolve(source, "b", 0), None);
    }

    #[test]
    fn glyph_locals_are_invisible_to_other_glyphs() {
        let source = "glyph a {\n  lsb = 1;\n}\nglyph b {\n  advance = lsb;\n}\n";
        assert_eq!(resolve(source, "lsb", 1), None);
    }

    #[test]
    fn contour_and_node_names() {
        let source = "glyph o {\n  node n = (0, 0) dir up pen pen(1);\n  contour c = n.pos -- (1, 0) -- cycle;\n  anchor t = point(c, 0);\n}\n";
        assert_eq!(resolve(source, "n.pos", 0), Some(at(source, "n =", 0)));
        assert_eq!(resolve(source, "c, 0", 0), Some(at(source, "c =", 0)));
    }

    #[test]
    fn member_property_has_no_definition() {
        let source = "glyph o {\n  node pos = (0, 0) dir up pen pen(1);\n  p = pos.pos;\n}\n";
        let property = at(source, ".pos", 0) + 1;
        let tree = parse_lenient(source);
        let read = |_: &Path| None;
        assert_eq!(
            definition(&tree, source, Path::new(MAIN), property, &read),
            None
        );
    }

    #[test]
    fn for_loop_variable() {
        let source = "i = 0;\nfor i in 1..3 {\n  draw(i);\n}\n";
        assert_eq!(resolve(source, "i)", 0), Some(at(source, "i in", 0)));
    }

    #[test]
    fn def_parameter_and_late_top_level_binding() {
        let source = "def f(c) -> p {\n  p = c + late;\n}\nlate = 1;\n";
        assert_eq!(resolve(source, "c +", 0), Some(at(source, "c)", 0)));
        assert_eq!(resolve(source, "late;", 0), Some(at(source, "late = ", 0)));
    }

    #[test]
    fn def_call_and_return_spec() {
        let source = "def f(c) -> p {\n  p = c;\n}\nq = f(1);\n";
        assert_eq!(resolve(source, "f(1)", 0), Some(at(source, "f(c)", 0)));
        assert_eq!(resolve(source, "p {", 0), Some(at(source, "p = c", 0)));
    }

    #[test]
    fn binding_from_an_import() {
        let common = "unit = 10;\ndef bowl(c) -> c {\n}\n";
        let files = [("/virtual/lib/common.ductus", common)];
        let source = "import \"lib/common.ductus\";\nstem = 4 * unit;\nb = bowl(1);\n";
        let found = resolve_in(&files, source, "unit", 0);
        assert_eq!(found, Some(("/virtual/lib/common.ductus".to_string(), 0)));
        let found = resolve_in(&files, source, "bowl", 0);
        let bowl = at(common, "bowl", 0);
        assert_eq!(
            found,
            Some(("/virtual/lib/common.ductus".to_string(), bowl))
        );
    }

    #[test]
    fn import_cycles_terminate() {
        let files = [("/virtual/main.ductus", "import \"main.ductus\";\n")];
        let source = "import \"main.ductus\";\nx = missing;\n";
        assert_eq!(resolve_in(&files, source, "missing", 0), None);
    }

    #[test]
    fn import_path_opens_the_file() {
        let files = [("/virtual/common.ductus", "unit = 10;\n")];
        let source = "import \"common.ductus\";\n";
        let found = resolve_in(&files, source, "common", 0);
        assert_eq!(found, Some(("/virtual/common.ductus".to_string(), 0)));
    }
}
