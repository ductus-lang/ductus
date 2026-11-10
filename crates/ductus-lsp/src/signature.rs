//! Signature help for built-in and `def` calls.
//!
//! The call is found in the text, not the syntax tree: while its arguments
//! are being typed the call is unfinished, and the tree holds an ERROR node
//! where a `call_expression` would be.

use crate::builtins;
use crate::definition::find_def;
use crate::importer::Read;
use crate::syntax::{self, text};
use ductus_core::parse::parse_lenient;
use std::ops::Range;
use std::path::Path;
use tree_sitter::{Node, Tree};

#[derive(Debug, PartialEq)]
pub struct Signature {
    pub label: String,
    /// Each parameter's place in `label`, in bytes.
    pub parameters: Vec<Range<usize>>,
    pub doc: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Help {
    pub signatures: Vec<Signature>,
    pub active_signature: usize,
    pub active_parameter: usize,
}

pub fn signature_help(
    tree: &Tree,
    source: &str,
    path: &Path,
    offset: usize,
    read: Read,
) -> Option<Help> {
    let (name, argument) = call_at(source, offset)?;
    let (signatures, variadic) = match builtins::find(&name) {
        Some(builtin) => {
            let signatures = builtin
                .overloads
                .iter()
                .map(|parameters| {
                    let mut signature = signature(&name, parameters, "");
                    signature.doc = Some(builtin.doc.to_string());
                    signature
                })
                .collect();
            (signatures, builtin.variadic)
        }
        None => (vec![def_signature(tree, source, path, &name, read)?], false),
    };
    let active_signature = signatures
        .iter()
        .position(|signature| signature.parameters.len() > argument)
        .unwrap_or(signatures.len() - 1);
    let count = signatures[active_signature].parameters.len();
    let active_parameter = match variadic {
        true => argument.min(count.saturating_sub(1)),
        false => argument,
    };
    Some(Help {
        signatures,
        active_signature,
        active_parameter,
    })
}

/// `name(a, b)` followed by `suffix`, with each parameter's range.
fn signature(name: &str, parameters: &[&str], suffix: &str) -> Signature {
    let mut label = format!("{name}(");
    let mut ranges = Vec::new();
    for (index, parameter) in parameters.iter().enumerate() {
        if index > 0 {
            label.push_str(", ");
        }
        ranges.push(label.len()..label.len() + parameter.len());
        label.push_str(parameter);
    }
    label.push(')');
    label.push_str(suffix);
    Signature {
        label,
        parameters: ranges,
        doc: None,
    }
}

fn def_signature(
    tree: &Tree,
    source: &str,
    path: &Path,
    name: &str,
    read: Read,
) -> Option<Signature> {
    let target = find_def(tree, source, path, name, read)?;
    let def_source = read(&target.path)?;
    let def_tree = parse_lenient(&def_source);
    let def = def_tree
        .root_node()
        .descendant_for_byte_range(target.range.start, target.range.end)?
        .parent()?;
    let parameters: Vec<&str> = def
        .child_by_field_name("parameters")
        .map(syntax::children)
        .unwrap_or_default()
        .into_iter()
        .map(|parameter| text(parameter, &def_source))
        .collect();
    let returns = def
        .child_by_field_name("returns")
        .map(|returns| format!(" -> {}", text(returns, &def_source)))
        .unwrap_or_default();
    let mut signature = signature(name, &parameters, &returns);
    signature.doc = leading_comments(def, &def_source);
    Some(signature)
}

/// The comment lines directly above `node`, without their `#`.
fn leading_comments(node: Node, source: &str) -> Option<String> {
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

/// The name of the call whose argument list holds `offset`, and the
/// 0-based index of the argument there.
fn call_at(source: &str, offset: usize) -> Option<(String, usize)> {
    let before = without_comments(&source[..offset]);
    let bytes = before.as_bytes();
    let mut depth = 0;
    let mut argument = 0;
    for index in (0..bytes.len()).rev() {
        match bytes[index] {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' if depth > 0 => depth -= 1,
            b'(' => {
                let name = name_before(&before[..index]).filter(|name| {
                    builtins::find(name).is_some() || !syntax::KEYWORDS.contains(name)
                });
                if let Some(name) = name {
                    return Some((name.to_string(), argument));
                }
                // A point or a grouping: all one argument of an outer call.
                argument = 0;
            }
            // A list or a mediation: also one argument.
            b'[' => argument = 0,
            b',' if depth == 0 => argument += 1,
            // The statement or block starts here, outside any call.
            b';' | b'{' => return None,
            _ => {}
        }
    }
    None
}

/// `text` with each line's `#` comment removed.
fn without_comments(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| match line.find('#') {
            Some(hash) if line.ends_with('\n') => format!("{}\n", &line[..hash]),
            Some(hash) => line[..hash].to_string(),
            None => line.to_string(),
        })
        .collect()
}

/// The identifier that ends `text`, ignoring trailing whitespace.
fn name_before(text: &str) -> Option<&str> {
    let text = text.trim_end();
    let start = text
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(0, |index| index + 1);
    let name = &text[start..];
    name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        .then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    const MAIN: &str = "/virtual/main.ductus";

    /// Signature help at the `|` in `source`, with `files` importable.
    fn help_in(files: &[(&str, &str)], source: &str) -> Option<Help> {
        let offset = source.find('|').unwrap();
        let source = source.replacen('|', "", 1);
        let texts: HashMap<PathBuf, String> = files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.to_string()))
            .chain([(PathBuf::from(MAIN), source.clone())])
            .collect();
        let read = |path: &Path| texts.get(path).cloned();
        let tree = parse_lenient(&source);
        signature_help(&tree, &source, Path::new(MAIN), offset, &read)
    }

    /// (active label, active parameter's text) at the `|`.
    fn active(source: &str) -> Option<(String, String)> {
        let help = help_in(&[], source)?;
        let signature = &help.signatures[help.active_signature];
        let parameter = signature
            .parameters
            .get(help.active_parameter)
            .map(|range| signature.label[range.clone()].to_string())
            .unwrap_or_default();
        Some((signature.label.clone(), parameter))
    }

    fn some(label: &str, parameter: &str) -> Option<(String, String)> {
        Some((label.to_string(), parameter.to_string()))
    }

    #[test]
    fn builtin_while_typing() {
        assert_eq!(active("x = min(|"), some("min(a, b)", "a"));
        assert_eq!(active("x = min(1, |"), some("min(a, b)", "b"));
        assert_eq!(active("x = min(1, |);"), some("min(a, b)", "b"));
    }

    #[test]
    fn nested_calls_points_and_lists() {
        assert_eq!(active("x = max(min(1, 2), |"), some("max(a, b)", "b"));
        assert_eq!(active("x = max(min(1, |"), some("min(a, b)", "b"));
        assert_eq!(active("c = circle((1, 2|"), some("circle(c, r)", "c"));
        assert_eq!(active("c = circle((1, 2), |"), some("circle(c, r)", "r"));
        assert_eq!(active("draw([a, b|"), some("draw(path)", "path"));
        assert_eq!(active("x = point(p, t[0, 1], |"), some("point(p, t)", ""));
    }

    #[test]
    fn overloads_and_variadics() {
        assert_eq!(active("p = pen(1, 2, |"), some("pen(w, h, angle)", "angle"));
        assert_eq!(active("p = pen(|"), some("pen(w)", "w"));
        assert_eq!(active("draw(a, b, |"), some("draw(path)", "path"));
    }

    #[test]
    fn outside_any_call() {
        assert_eq!(active("x = min(1, 2) + |"), None);
        assert_eq!(active("x = (1 + |"), None);
        assert_eq!(active("x = min(1, 2);\ny = |"), None);
        assert_eq!(active("glyph o {\n  advance = |"), None);
        assert_eq!(active("node n = (0, 0) dir (|"), None);
    }

    #[test]
    fn comments_do_not_count() {
        assert_eq!(active("x = min(1, # (a, b\n  |"), some("min(a, b)", "b"));
    }

    #[test]
    fn def_with_its_comment() {
        let source = "# A bowl of four nodes.\n# Counterclockwise.\ndef bowl(c, rx, ry) -> (n, w, s, e) {\n}\nx = bowl(1, |";
        let help = help_in(&[], source).unwrap();
        let signature = &help.signatures[0];
        assert_eq!(signature.label, "bowl(c, rx, ry) -> (n, w, s, e)");
        assert_eq!(&signature.label[signature.parameters[1].clone()], "rx");
        assert_eq!(help.active_parameter, 1);
        assert_eq!(
            signature.doc.as_deref(),
            Some("A bowl of four nodes.\nCounterclockwise.")
        );
    }

    #[test]
    fn def_from_an_import() {
        let lib = "def f(c) -> p {\n  p = c;\n}\n";
        let source = "import \"lib.ductus\";\nx = f(|";
        let help = help_in(&[("/virtual/lib.ductus", lib)], source).unwrap();
        assert_eq!(help.signatures[0].label, "f(c) -> p");
        assert_eq!(help.signatures[0].doc, None);
    }

    #[test]
    fn unknown_function_has_no_help() {
        assert_eq!(active("x = nothing(|"), None);
    }
}
