//! Rename a binding, refusing renames that would change what any name
//! means.
//!
//! The edits are applied in memory and the result is resolved again:
//! every renamed site must still reach the renamed binding, every existing
//! name spelled like the new one must reach what it reached before (no
//! capture by an inner binding, no hiding of an outer or prelude one), and
//! the renamed binding's scope must not bind the new name twice, since
//! bindings are immutable.

use crate::definition::{Target, definition};
use crate::importer::{self, Read};
use crate::references::references;
use crate::syntax::{self, text};
use ductus_core::eval::BUILTIN_FUNCTIONS;
use ductus_core::parse::parse_lenient;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Byte ranges to replace with the new name, per file, in document order.
pub type Edits = BTreeMap<PathBuf, Vec<Range<usize>>>;

/// The range of the identifier at `offset` when it names a binding that
/// can be renamed.
pub fn prepare(
    tree: &tree_sitter::Tree,
    source: &str,
    path: &Path,
    offset: usize,
    read: Read,
) -> Option<Range<usize>> {
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;
    if node.kind() != "identifier" {
        return None;
    }
    let target = definition(tree, source, path, offset, read)?;
    (!target.range.is_empty()).then(|| node.byte_range())
}

pub fn rename(binding: &Target, new: &str, files: &[PathBuf], read: Read) -> Result<Edits, String> {
    let source = read(&binding.path).ok_or("cannot read the renamed file")?;
    let old = &source[binding.range.clone()];
    if !is_identifier(new) {
        return Err(format!("'{new}' is not a valid name"));
    }
    let tree = parse_lenient(&source);
    let is_def = tree
        .root_node()
        .descendant_for_byte_range(binding.range.start, binding.range.end)
        .and_then(|node| node.parent())
        .is_some_and(|parent| parent.kind() == "def_declaration");
    if is_def && BUILTIN_FUNCTIONS.contains(&new) {
        return Err(format!("'{new}' is a built-in function"));
    }
    if old == new {
        return Ok(Edits::new());
    }

    let sites = references(binding, files, read);
    let mut edits = Edits::new();
    for site in &sites {
        edits
            .entry(site.path.clone())
            .or_default()
            .push(site.range.clone());
    }

    let mut renamed = HashMap::new();
    for (path, ranges) in &edits {
        let text = read(path).ok_or_else(|| format!("cannot read {}", path.display()))?;
        renamed.insert(path.clone(), apply(&text, ranges, new));
    }
    let after = |path: &Path| renamed.get(path).cloned().or_else(|| read(path));
    let shift = Shift {
        edits: &edits,
        delta: new.len() as isize - old.len() as isize,
    };
    let renamed_binding = shift.target(binding);

    let mut checked: Vec<&PathBuf> = edits.keys().collect();
    checked.extend(files);
    checked.sort();
    checked.dedup();
    for file in checked {
        let (Some(before_text), Some(after_text)) = (read(file), after(file)) else {
            continue;
        };
        let sites_here: Vec<&Target> = sites.iter().filter(|site| &site.path == file).collect();
        if sites_here.is_empty() && !before_text.contains(new) {
            continue;
        }
        let before_tree = parse_lenient(&before_text);
        let after_tree = parse_lenient(&after_text);
        let resolve_after = |offset| definition(&after_tree, &after_text, file, offset, &after);

        for site in sites_here {
            let offset = shift.offset(file, site.range.start);
            if resolve_after(offset).as_ref() != Some(&renamed_binding) {
                return Err(changes_meaning(old, new, file, &after_text, offset));
            }
        }
        for node in syntax::identifiers(before_tree.root_node(), &before_text, new) {
            let start = node.start_byte();
            let before = definition(&before_tree, &before_text, file, start, read)
                .map(|target| shift.target(&target));
            let offset = shift.offset(file, start);
            if resolve_after(offset) != before {
                return Err(changes_meaning(old, new, file, &after_text, offset));
            }
        }
    }

    if let Some(clash) = clash(&renamed_binding, is_def, new, files, &after) {
        let text = after(&clash.path).unwrap_or_default();
        return Err(format!(
            "'{new}' is already bound in that scope, at {}",
            place(&clash.path, &text, clash.range.start)
        ));
    }
    Ok(edits)
}

/// Spec.md §2: keywords cannot name a binding.
const KEYWORDS: &[&str] = &[
    "params", "in", "glyph", "contour", "guide", "anchor", "node", "dir", "pen", "cycle", "corner",
    "def", "for", "if", "else", "and", "or", "not", "import", "shifted", "scaled", "xscaled",
    "yscaled", "rotated", "slanted", "reversed", "metrics", "draw", "fill",
];

/// Spec.md §2: `[a-zA-Z_][a-zA-Z0-9_]*`, and not a keyword.
fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_')
        && !KEYWORDS.contains(&name)
}

fn apply(text: &str, ranges: &[Range<usize>], new: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for range in ranges {
        out.push_str(&text[copied..range.start]);
        out.push_str(new);
        copied = range.end;
    }
    out.push_str(&text[copied..]);
    out
}

/// Maps offsets in the original files to offsets after the edits.
struct Shift<'a> {
    edits: &'a Edits,
    /// Length change of each edit.
    delta: isize,
}

impl Shift<'_> {
    fn offset(&self, path: &Path, offset: usize) -> usize {
        let before = self.edits.get(path).map_or(0, |ranges| {
            ranges.iter().filter(|range| range.start < offset).count()
        });
        (offset as isize + before as isize * self.delta) as usize
    }

    fn target(&self, target: &Target) -> Target {
        Target {
            path: target.path.clone(),
            range: self.offset(&target.path, target.range.start)
                ..self.offset(&target.path, target.range.end),
        }
    }
}

/// Another binding of `name` in the renamed binding's scope, if any.
fn clash(
    binding: &Target,
    is_def: bool,
    name: &str,
    files: &[PathBuf],
    read: Read,
) -> Option<Target> {
    let source = read(&binding.path)?;
    let tree = parse_lenient(&source);
    let node = tree
        .root_node()
        .descendant_for_byte_range(binding.range.start, binding.range.end)?;

    let scope = scope_of(node)?;
    if scope.kind() != "source_file" {
        return syntax::children(scope)
            .into_iter()
            .flat_map(syntax::bindings)
            .map(|binding| binding.name)
            .chain(syntax::owner_bindings(scope))
            .find(|found| text(*found, &source) == name && found.byte_range() != binding.range)
            .map(|found| Target {
                path: binding.path.clone(),
                range: found.byte_range(),
            });
    }

    // Imports merge files into one top level, so check the top level of
    // every file that includes the binding's.
    let mut roots: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
    roots.push(&binding.path);
    for root in roots {
        let mut found = Vec::new();
        let mut visited = HashSet::new();
        top_level(root, name, is_def, read, &mut visited, &mut found);
        if found.contains(binding)
            && let Some(clash) = found.into_iter().find(|target| target != binding)
        {
            return Some(clash);
        }
    }
    None
}

/// The scope a binding name is bound in.
fn scope_of(name: Node) -> Option<Node> {
    let parent = name.parent()?;
    match parent.kind() {
        "for_statement" => return parent.child_by_field_name("body"),
        "parameter_list" => return parent.parent()?.child_by_field_name("body"),
        _ => {}
    }
    let mut node = parent;
    loop {
        if matches!(node.kind(), "block" | "glyph_block" | "source_file") {
            return Some(node);
        }
        node = node.parent()?;
    }
}

/// Every top-level binding (or `def`) of `name` in `path` and what it
/// imports, include-once.
fn top_level(
    path: &Path,
    name: &str,
    is_def: bool,
    read: Read,
    visited: &mut HashSet<PathBuf>,
    found: &mut Vec<Target>,
) {
    if !visited.insert(path.to_path_buf()) {
        return;
    }
    let Some(source) = read(path) else {
        return;
    };
    let tree = parse_lenient(&source);
    for statement in syntax::children(tree.root_node()) {
        let names = match statement.kind() {
            "import_statement" => {
                if let Some(imported) = statement.child_by_field_name("path") {
                    let file = importer::resolve(path, text(imported, &source).trim_matches('"'));
                    top_level(&file, name, is_def, read, visited, found);
                }
                continue;
            }
            "def_declaration" if is_def => {
                statement.child_by_field_name("name").into_iter().collect()
            }
            _ if !is_def => syntax::bindings(statement)
                .into_iter()
                .map(|binding| binding.name)
                .collect(),
            _ => Vec::new(),
        };
        found.extend(
            names
                .into_iter()
                .filter(|node| text(*node, &source) == name)
                .map(|node| Target {
                    path: path.to_path_buf(),
                    range: node.byte_range(),
                }),
        );
    }
}

fn changes_meaning(old: &str, new: &str, path: &Path, text: &str, offset: usize) -> String {
    format!(
        "renaming '{old}' to '{new}' would change what the name at {} refers to",
        place(path, text, offset)
    )
}

/// `file.ductus:line`.
fn place(path: &Path, text: &str, offset: usize) -> String {
    let line = text[..offset.min(text.len())].matches('\n').count() + 1;
    let file = path.file_name().unwrap_or(path.as_os_str());
    format!("{}:{line}", file.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: &str = "/virtual/lib.ductus";
    const MAIN: &str = "/virtual/main.ductus";

    /// Rename the binding at the first occurrence of `at` in `in_file` to
    /// `new`; return the renamed texts.
    fn run(
        files: &[(&str, &str)],
        in_file: &str,
        at: &str,
        new: &str,
    ) -> Result<HashMap<String, String>, String> {
        let texts: HashMap<PathBuf, String> = files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.to_string()))
            .collect();
        let read = |path: &Path| texts.get(path).cloned();
        let path = Path::new(in_file);
        let source = &texts[path];
        let offset = source.find(at).unwrap();
        let tree = parse_lenient(source);
        let binding = definition(&tree, source, path, offset, &read).unwrap();
        let paths: Vec<PathBuf> = texts.keys().cloned().collect();
        let edits = rename(&binding, new, &paths, &read)?;
        Ok(edits
            .iter()
            .map(|(path, ranges)| {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                (name, apply(&texts[path], ranges, new))
            })
            .collect())
    }

    #[test]
    fn renames_across_files() {
        let lib = "unit = 10;\ndef f(c) -> p {\n  p = c * unit;\n}\n";
        let main = "import \"lib.ductus\";\nx = unit + f(1);\n";
        let renamed = run(&[(LIB, lib), (MAIN, main)], MAIN, "unit +", "grid").unwrap();
        assert_eq!(
            renamed["lib.ductus"],
            "grid = 10;\ndef f(c) -> p {\n  p = c * grid;\n}\n"
        );
        assert_eq!(
            renamed["main.ductus"],
            "import \"lib.ductus\";\nx = grid + f(1);\n"
        );
    }

    #[test]
    fn renames_a_def_and_its_return_name() {
        let main = "def f(c) -> p {\n  p = c;\n}\nx = f(1);\n";
        let renamed = run(&[(MAIN, main)], MAIN, "f(c)", "g").unwrap();
        assert_eq!(
            renamed["main.ductus"],
            "def g(c) -> p {\n  p = c;\n}\nx = g(1);\n"
        );
        let renamed = run(&[(MAIN, main)], MAIN, "p {", "q").unwrap();
        assert_eq!(
            renamed["main.ductus"],
            "def f(c) -> q {\n  q = c;\n}\nx = f(1);\n"
        );
    }

    #[test]
    fn longer_and_shorter_names_keep_offsets_right() {
        let main = "a = 1;\nb = a + a;\nglyph o {\n  advance = a * b;\n}\n";
        let renamed = run(&[(MAIN, main)], MAIN, "a =", "width").unwrap();
        assert_eq!(
            renamed["main.ductus"],
            "width = 1;\nb = width + width;\nglyph o {\n  advance = width * b;\n}\n"
        );
    }

    #[test]
    fn refuses_keywords_and_non_identifiers() {
        let main = "a = 1;\n";
        for new in ["glyph", "cycle", "2a", "a b", ""] {
            assert!(run(&[(MAIN, main)], MAIN, "a", new).is_err(), "{new:?}");
        }
    }

    #[test]
    fn refuses_a_def_named_like_a_builtin() {
        let main = "def f(c) -> p {\n  p = c;\n}\n";
        let error = run(&[(MAIN, main)], MAIN, "f(c)", "sqrt").unwrap_err();
        assert!(error.contains("built-in"), "{error}");
    }

    #[test]
    fn refuses_a_clash_in_the_same_scope() {
        let main = "a = 1;\nb = 2;\n";
        let error = run(&[(MAIN, main)], MAIN, "a", "b").unwrap_err();
        assert!(error.contains("already bound"), "{error}");
    }

    #[test]
    fn refuses_a_clash_with_a_loop_variable() {
        let main = "for i in 1..2 {\n  j = i;\n}\n";
        let error = run(&[(MAIN, main)], MAIN, "j", "i").unwrap_err();
        assert!(error.contains("already bound"), "{error}");
    }

    #[test]
    fn refuses_a_clash_through_an_import() {
        let lib = "a = 1;\n";
        let main = "import \"lib.ductus\";\nb = 2;\n";
        let error = run(&[(LIB, lib), (MAIN, main)], LIB, "a", "b").unwrap_err();
        assert!(error.contains("main.ductus:2"), "{error}");
    }

    #[test]
    fn refuses_capture_by_an_inner_binding() {
        let main = "w = 1;\nglyph o {\n  v = 2;\n  advance = w;\n}\n";
        let error = run(&[(MAIN, main)], MAIN, "w", "v").unwrap_err();
        assert!(error.contains("main.ductus:4"), "{error}");
    }

    #[test]
    fn refuses_hiding_an_outer_binding() {
        let main = "stem = 1;\nglyph o {\n  w = 2;\n  advance = stem;\n}\n";
        let error = run(&[(MAIN, main)], MAIN, "w =", "stem").unwrap_err();
        assert!(error.contains("main.ductus:4"), "{error}");
    }

    #[test]
    fn allows_shadowing_that_changes_nothing() {
        let main = "stem = 1;\nglyph o {\n  advance = stem;\n  w = 2;\n}\n";
        let renamed = run(&[(MAIN, main)], MAIN, "w =", "stem").unwrap();
        assert_eq!(
            renamed["main.ductus"],
            "stem = 1;\nglyph o {\n  advance = stem;\n  stem = 2;\n}\n"
        );
    }

    #[test]
    fn refuses_hiding_a_prelude_name() {
        let main = "x = 1;\nglyph o {\n  node n = (0, 0) dir up pen pen(x);\n}\n";
        let error = run(&[(MAIN, main)], MAIN, "x", "up").unwrap_err();
        assert!(error.contains("main.ductus:3"), "{error}");
    }
}
