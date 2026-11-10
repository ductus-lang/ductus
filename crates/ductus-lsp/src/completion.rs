//! Completion by context: statement keywords where a statement starts,
//! names and functions inside expressions, fields after a `.`.
//!
//! The context comes from the text before the cursor, as in signature
//! help: what is being typed is unfinished, and the syntax tree around it
//! is an ERROR node. The tree is only asked which scope the cursor is in.

use crate::builtins::{self, Builtin};
use crate::importer::Read;
use crate::scope::{self, Named};
use crate::signature::signature;
use crate::syntax;
use lsp_types::CompletionItemKind;
use std::path::Path;
use tree_sitter::Tree;

#[derive(Debug, PartialEq)]
pub struct Item {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub doc: Option<String>,
    /// For functions: the call with a placeholder per argument, in LSP
    /// snippet syntax.
    pub snippet: Option<String>,
    /// Lower groups sort first.
    pub group: u8,
}

/// Statements that can start in each kind of scope.
const TOP_LEVEL: &[&str] = &["params", "metrics", "def", "glyph", "import", "node", "for"];
const GLYPH: &[&str] = &["node", "contour", "guide", "anchor", "for"];
const BLOCK: &[&str] = &["node", "for"];

/// Spec.md §2: the prelude's direction constants.
const PRELUDE: &[(&str, &str)] = &[
    ("right", "0"),
    ("up", "90"),
    ("left", "180"),
    ("down", "270"),
];

const POINT_FIELDS: &[(&str, &str)] = &[("x", "number"), ("y", "number")];
const NODE_FIELDS: &[(&str, &str)] = &[
    ("pos", "point"),
    ("dir", "number, degrees"),
    ("width", "number"),
    ("left", "point"),
    ("right", "point"),
    ("tip", "point"),
    ("tail", "point"),
    ("tipcap", "path"),
    ("tailcap", "path"),
];

pub fn complete(tree: &Tree, source: &str, path: &Path, offset: usize, read: Read) -> Vec<Item> {
    let clean = syntax::without_comments(&source[..offset]);
    let word_start = clean
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_')
        .len();
    let before = clean[..word_start].trim_end();
    let last = before.len().checked_sub(1);
    match last.map(|index| (index, before.as_bytes()[index])) {
        Some((index, b'.')) => fields(tree, source, path, offset, read, &before[..index]),
        None | Some((_, b';' | b'}')) => statement(tree, source, path, offset, read, before),
        Some((index, b'{')) if !opens_direction(tree, index) => {
            statement(tree, source, path, offset, read, before)
        }
        _ => expression(tree, source, path, offset, read),
    }
}

/// True for the `{` of a direction spec, `a{up}`, as opposed to a block's.
fn opens_direction(tree: &Tree, index: usize) -> bool {
    tree.root_node()
        .descendant_for_byte_range(index, index + 1)
        .filter(|node| node.kind() == "{")
        .and_then(|node| node.parent())
        .is_some_and(|parent| parent.kind() == "direction_spec")
}

/// At the start of a statement: the keywords that can start one here, and
/// the calls a statement can be: defs, and `draw` and `fill` outside glyphs.
fn statement(
    tree: &Tree,
    source: &str,
    path: &Path,
    offset: usize,
    read: Read,
    before: &str,
) -> Vec<Item> {
    let blocks = enclosing_blocks(before);
    let keywords = match blocks.first().copied() {
        // These blocks declare new names; nothing to offer.
        Some("params" | "metrics") => return Vec::new(),
        Some("glyph") => GLYPH,
        Some(_) => BLOCK,
        None => TOP_LEVEL,
    };
    let in_glyph = blocks.contains(&"glyph");
    let mut items: Vec<Item> = keywords
        .iter()
        .map(|keyword| keyword_item(keyword))
        .collect();
    let visible = scope::visible(tree, source, path, offset, read);
    items.extend(visible.iter().filter(|named| named.is_def).map(def_item));
    if !in_glyph {
        let sketch = ["draw", "fill"]
            .iter()
            .filter_map(|name| builtins::find(name));
        items.extend(sketch.map(builtin_item));
    }
    items
}

/// The keyword opening each block that encloses the end of `text`,
/// innermost first: `glyph`, `def`, `for`, `params` or `metrics`. Read
/// from the text because a block being typed has no closing `}` yet, and
/// the syntax tree then has no block either.
fn enclosing_blocks(text: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut depth = 0;
    for (index, byte) in text.bytes().enumerate().rev() {
        match byte {
            b'}' => depth += 1,
            b'{' if depth > 0 => depth -= 1,
            b'{' => {
                let header = text[..index]
                    .rfind([';', '{', '}'])
                    .map_or(0, |end| end + 1);
                let keyword = text[header..index].split_whitespace().next();
                blocks.push(keyword.unwrap_or_default());
            }
            _ => {}
        }
    }
    blocks
}

/// Inside an expression: names in scope, functions, prelude constants.
fn expression(tree: &Tree, source: &str, path: &Path, offset: usize, read: Read) -> Vec<Item> {
    let visible = scope::visible(tree, source, path, offset, read);
    let mut items: Vec<Item> = visible
        .iter()
        .map(|named| match named.is_def {
            true => def_item(named),
            false => value_item(named),
        })
        .collect();
    let functions = builtins::BUILTINS
        .iter()
        .filter(|builtin| !builtin.variadic);
    items.extend(functions.map(builtin_item));
    let shadowed = |name: &str| {
        visible
            .iter()
            .any(|named| named.name == name && !named.is_def)
    };
    items.extend(
        PRELUDE
            .iter()
            .filter(|(name, _)| !shadowed(name))
            .map(|(name, value)| Item {
                label: name.to_string(),
                kind: CompletionItemKind::CONSTANT,
                detail: Some(format!("{name} = {value}")),
                doc: None,
                snippet: None,
                group: 3,
            }),
    );
    items
}

/// After `object.`: the fields of what `object` is, when that can be told
/// from the source, else every field.
fn fields(
    tree: &Tree,
    source: &str,
    path: &Path,
    offset: usize,
    read: Read,
    before_dot: &str,
) -> Vec<Item> {
    let start = before_dot
        .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        .len();
    let object = &before_dot[start..];
    if !object.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        // A number, `0.`, or something like `(a + b).`.
        return Vec::new();
    }
    let groups: &[&[(&str, &str)]] = match object.rsplit_once('.') {
        Some((_, "pos" | "left" | "right" | "tip" | "tail")) => &[POINT_FIELDS],
        Some((_, "x" | "y" | "dir" | "width" | "tipcap" | "tailcap")) => &[],
        Some(_) => &[POINT_FIELDS, NODE_FIELDS],
        None => {
            let visible = scope::visible(tree, source, path, offset, read);
            let named = visible
                .iter()
                .find(|named| named.name == object && !named.is_def);
            match named.map(|named| named.what) {
                Some("node") => &[NODE_FIELDS],
                Some("param" | "metric" | "contour" | "guide") => &[],
                _ => &[POINT_FIELDS, NODE_FIELDS],
            }
        }
    };
    let owner = |fields: &[(&str, &str)]| match fields.len() {
        2 => "point",
        _ => "node",
    };
    groups
        .iter()
        .flat_map(|fields| {
            fields.iter().map(move |(name, of)| Item {
                label: name.to_string(),
                kind: CompletionItemKind::FIELD,
                detail: Some(format!("{}: {of}", owner(fields))),
                doc: None,
                snippet: None,
                group: 0,
            })
        })
        .collect()
}

fn keyword_item(keyword: &str) -> Item {
    Item {
        label: keyword.to_string(),
        kind: CompletionItemKind::KEYWORD,
        detail: None,
        doc: None,
        snippet: None,
        group: 0,
    }
}

fn value_item(named: &Named) -> Item {
    let kind = match named.what {
        "param" | "metric" => CompletionItemKind::CONSTANT,
        _ => CompletionItemKind::VARIABLE,
    };
    Item {
        label: named.name.clone(),
        kind,
        detail: Some(named.summary.clone()),
        doc: None,
        snippet: None,
        group: 1,
    }
}

fn def_item(named: &Named) -> Item {
    let parameters: Vec<&str> = named.parameters.iter().map(String::as_str).collect();
    Item {
        label: named.name.clone(),
        kind: CompletionItemKind::FUNCTION,
        detail: Some(named.summary.trim_end_matches('{').trim_end().to_string()),
        doc: named.doc.clone(),
        snippet: Some(snippet(&named.name, &parameters)),
        group: 2,
    }
}

fn builtin_item(builtin: &Builtin) -> Item {
    let longest = builtin.overloads.last().copied().unwrap_or_default();
    let shortest = builtin.overloads.first().copied().unwrap_or_default();
    Item {
        label: builtin.name.to_string(),
        kind: CompletionItemKind::FUNCTION,
        detail: Some(signature(builtin.name, longest, "").label),
        doc: Some(builtin.doc.to_string()),
        snippet: Some(snippet(builtin.name, shortest)),
        group: 2,
    }
}

/// `name(${1:a}, ${2:b})`.
fn snippet(name: &str, parameters: &[&str]) -> String {
    let placeholders: Vec<String> = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("${{{}:{parameter}}}", index + 1))
        .collect();
    format!("{name}({})", placeholders.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::parse::parse_lenient;

    /// The labels offered at the `|` in `source`.
    fn labels(source: &str) -> Vec<String> {
        items(source).into_iter().map(|item| item.label).collect()
    }

    fn items(source: &str) -> Vec<Item> {
        let offset = source.find('|').unwrap();
        let source = source.replacen('|', "", 1);
        let tree = parse_lenient(&source);
        let read = |_: &Path| None;
        complete(
            &tree,
            &source,
            Path::new("/virtual/main.ductus"),
            offset,
            &read,
        )
    }

    fn has(labels: &[String], wanted: &[&str]) -> bool {
        wanted
            .iter()
            .all(|wanted| labels.iter().any(|label| label == wanted))
    }

    fn has_none(labels: &[String], unwanted: &[&str]) -> bool {
        unwanted
            .iter()
            .all(|unwanted| !labels.iter().any(|label| label == unwanted))
    }

    const PREAMBLE: &str = "params {\n  weight = 400 in 100..900;\n}\nstem = 1;\n\
                            # Four nodes.\ndef bowl(c, r) -> n {\n  n = c;\n}\n";

    #[test]
    fn top_level_statement() {
        let found = labels(&format!("{PREAMBLE}gl|"));
        assert!(
            has(
                &found,
                &["params", "glyph", "import", "draw", "fill", "bowl"]
            ),
            "{found:?}"
        );
        assert!(has_none(&found, &["contour", "stem", "min"]), "{found:?}");
    }

    #[test]
    fn glyph_statement() {
        let found = labels(&format!("{PREAMBLE}glyph o {{\n  advance = 1;\n  co|"));
        assert!(
            has(&found, &["contour", "guide", "anchor", "node", "bowl"]),
            "{found:?}"
        );
        assert!(has_none(&found, &["glyph", "params", "draw"]), "{found:?}");
    }

    #[test]
    fn statement_in_a_loop_inside_a_glyph() {
        let found = labels("glyph o {\n  for i in 1..2 {\n    |\n  }\n}\n");
        assert!(has(&found, &["node", "for"]), "{found:?}");
        assert!(has_none(&found, &["contour", "draw", "glyph"]), "{found:?}");
        let found = labels("for i in 1..2 {\n  |\n}\n");
        assert!(has(&found, &["node", "draw"]), "{found:?}");
    }

    #[test]
    fn params_block_offers_nothing() {
        assert_eq!(labels("params {\n  |"), Vec::<String>::new());
    }

    #[test]
    fn expression_names_functions_and_prelude() {
        let found = labels(&format!(
            "{PREAMBLE}glyph o {{\n  w = 2;\n  advance = st|\n  late = 3;\n}}\n"
        ));
        assert!(
            has(&found, &["w", "stem", "weight", "bowl", "min", "pen", "up"]),
            "{found:?}"
        );
        assert!(has_none(&found, &["late", "draw", "contour"]), "{found:?}");
    }

    #[test]
    fn direction_spec_is_an_expression() {
        let found = labels("glyph o {\n  contour c = (0, 0){u| .. (1, 0) .. cycle;\n}\n");
        assert!(has(&found, &["up", "left"]), "{found:?}");
        assert!(has_none(&found, &["contour"]), "{found:?}");
    }

    #[test]
    fn a_shadowed_prelude_constant_is_offered_once() {
        let found = labels("up = 1;\nx = |");
        assert_eq!(found.iter().filter(|label| *label == "up").count(), 1);
    }

    #[test]
    fn fields_by_what_the_object_is() {
        let node = "glyph o {\n  node n = (0, 0) dir up pen pen(1);\n  p = n.|\n}\n";
        let found = labels(node);
        assert!(
            has(&found, &["pos", "left", "tipcap"]) && has_none(&found, &["x"]),
            "{found:?}"
        );
        let found = labels("glyph o {\n  node n = (0, 0) dir up pen pen(1);\n  p = n.left.|\n}\n");
        assert_eq!(found, ["x", "y"]);
        assert_eq!(
            labels("params {\n  weight = 1 in 0..2;\n}\nx = weight.|"),
            Vec::<String>::new()
        );
        assert_eq!(
            labels("x = q.|").len(),
            POINT_FIELDS.len() + NODE_FIELDS.len()
        );
        assert_eq!(labels("x = 0.|"), Vec::<String>::new());
    }

    #[test]
    fn names_in_a_glyph_still_being_typed() {
        let source =
            "stem = 1;\nglyph o {\n  w = 2;\n  node n = (0, 0) dir up pen pen(1);\n  p = |";
        let found = labels(source);
        assert!(has(&found, &["stem", "w", "n"]), "{found:?}");
        let found = labels(&source.replace("p = |", "p = n.|"));
        assert!(has_none(&found, &["x"]), "{found:?}");
    }

    #[test]
    fn snippets_and_docs() {
        let found = items(&format!("{PREAMBLE}x = |"));
        let bowl = found.iter().find(|item| item.label == "bowl").unwrap();
        assert_eq!(bowl.snippet.as_deref(), Some("bowl(${1:c}, ${2:r})"));
        assert_eq!(bowl.detail.as_deref(), Some("def bowl(c, r) -> n"));
        assert_eq!(bowl.doc.as_deref(), Some("Four nodes."));
        let pen = found.iter().find(|item| item.label == "pen").unwrap();
        assert_eq!(pen.snippet.as_deref(), Some("pen(${1:w})"));
        assert_eq!(pen.detail.as_deref(), Some("pen(w, h, angle)"));
    }

    #[test]
    fn comments_are_ignored() {
        let found = labels("x = 1; # a.|");
        assert!(has(&found, &["params", "glyph"]), "{found:?}");
    }
}
