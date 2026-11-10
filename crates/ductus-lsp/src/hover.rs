//! Hover: a name's declaration, and its value in the last evaluation.

use crate::builtins;
use crate::definition::definition;
use crate::importer::Read;
use crate::signature::{def_label, signature};
use crate::syntax::text;
use crate::values;
use ductus_core::doc::Document;
use ductus_core::parse::parse_lenient;
use std::ops::Range;
use std::path::Path;
use tree_sitter::{Node, Tree};

/// The prelude's direction constants, Spec.md §2.
const PRELUDE: &[(&str, &str)] = &[
    ("right", "0"),
    ("up", "90"),
    ("left", "180"),
    ("down", "270"),
];

/// The evaluated document values come from, and whether the text has
/// changed since with errors.
pub struct Evaluated<'a> {
    pub document: &'a Document,
    pub stale: bool,
}

/// Markdown for the identifier at `offset`, and the identifier's range.
pub fn hover(
    tree: &Tree,
    source: &str,
    path: &Path,
    offset: usize,
    read: Read,
    evaluated: Option<Evaluated>,
) -> Option<(String, Range<usize>)> {
    let node = tree.root_node().descendant_for_byte_range(offset, offset)?;
    if node.kind() != "identifier" {
        return None;
    }
    let parent = node.parent()?;
    if parent.kind() == "member_expression" && parent.child_by_field_name("property") == Some(node)
    {
        return None;
    }
    let name = text(node, source);
    let markdown = match definition(tree, source, path, offset, read) {
        Some(target) if !target.range.is_empty() => {
            let target_source = read(&target.path)?;
            let target_tree = parse_lenient(&target_source);
            let name_node = target_tree
                .root_node()
                .descendant_for_byte_range(target.range.start, target.range.end)?;
            declared(name_node, name, &target_source, evaluated)?
        }
        _ => builtin(name).or_else(|| prelude(name))?,
    };
    Some((markdown, node.byte_range()))
}

/// The hover for the binding whose name is `name_node`.
fn declared(
    name_node: Node,
    name: &str,
    source: &str,
    evaluated: Option<Evaluated>,
) -> Option<String> {
    let mut declaration = name_node.parent()?;
    match declaration.kind() {
        "def_declaration" => {
            let signature = def_label(declaration, source)?;
            return Some(with_doc(code(&signature.label), signature.doc.as_deref()));
        }
        "parameter_list" => {
            let def = def_label(declaration.parent()?, source)?;
            return Some(format!("{}\nparameter `{name}`", code(&def.label)));
        }
        "for_statement" => {
            return Some(format!(
                "{}\nloop variable",
                code(&first_line(declaration, source))
            ));
        }
        "tuple_pattern" => declaration = declaration.parent()?,
        _ => {}
    }
    let mut markdown = code(&first_line(declaration, source));
    if let Some(Evaluated { document, stale }) = evaluated
        && let Some(value) = values::value(document, declaration, name, source)
    {
        markdown.push_str(&format!("\n**{value}**"));
        if stale {
            markdown.push_str("\n\n*From the last evaluation without errors.*");
        }
    }
    Some(markdown)
}

fn builtin(name: &str) -> Option<String> {
    let builtin = builtins::find(name)?;
    let labels: Vec<String> = builtin
        .overloads
        .iter()
        .map(|parameters| signature(name, parameters, "").label)
        .collect();
    Some(with_doc(code(&labels.join("\n")), Some(builtin.doc)))
}

fn prelude(name: &str) -> Option<String> {
    let (_, value) = PRELUDE.iter().find(|(constant, _)| *constant == name)?;
    Some(format!(
        "{}\ndirection constant from the prelude, in degrees",
        code(&format!("{name} = {value}"))
    ))
}

/// The declaration's first line, with `…` when it goes on, and without a
/// block's opening `{`.
fn first_line<'a>(declaration: Node, source: &'a str) -> std::borrow::Cow<'a, str> {
    let text = text(declaration, source);
    let first = text.lines().next().unwrap_or_default().trim();
    let first = first.strip_suffix('{').unwrap_or(first).trim_end();
    match text.trim_end().contains('\n') && !text.trim_end().ends_with('}') {
        true => format!("{first} …").into(),
        false => first.into(),
    }
}

fn code(text: &str) -> String {
    format!("```ductus\n{text}\n```")
}

fn with_doc(markdown: String, doc: Option<&str>) -> String {
    match doc {
        Some(doc) => format!("{markdown}\n{doc}"),
        None => markdown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::{eval, lower};
    use std::collections::HashMap;

    const SOURCE: &str = "params {\n  weight = 400 in 100..900;\n}\n\
        stem = 40 + 0.14 * weight;\n\
        # Doubles.\ndef f(c) -> p {\n  p = c * 2;\n}\n\
        glyph o {\n  advance = f(stem);\n  \
        contour main = (0, 0)\n    -- (1, 0)\n    -- cycle;\n  \
        node n = (0, 0) dir up pen pen(1);\n  w = n.left;\n}\n\
        for i in 1..2 {\n  draw(circle((i, 0), 1));\n}\n";

    /// Hover at the `nth` occurrence of `needle`, evaluated or not.
    fn hover_at(needle: &str, nth: usize, evaluate: bool) -> Option<String> {
        let tree = parse_lenient(SOURCE);
        let document = evaluate.then(|| {
            let program = lower::lower(&tree, SOURCE).unwrap();
            eval::evaluate(&program, &HashMap::new(), &eval::NoImporter).unwrap()
        });
        let evaluated = document.as_ref().map(|document| Evaluated {
            document,
            stale: false,
        });
        let offset = SOURCE.match_indices(needle).nth(nth).unwrap().0;
        let read = |_: &Path| Some(SOURCE.to_string());
        hover(
            &tree,
            SOURCE,
            Path::new("/virtual/main.ductus"),
            offset,
            &read,
            evaluated,
        )
        .map(|(markdown, _)| markdown)
    }

    #[test]
    fn binding_with_its_value() {
        let found = hover_at("stem", 1, true).unwrap();
        assert_eq!(found, "```ductus\nstem = 40 + 0.14 * weight;\n```\n**96**");
    }

    #[test]
    fn without_an_evaluation_only_the_declaration() {
        let found = hover_at("stem", 1, false).unwrap();
        assert_eq!(found, "```ductus\nstem = 40 + 0.14 * weight;\n```");
    }

    #[test]
    fn param_and_glyph_values() {
        assert!(
            hover_at("weight", 1, true)
                .unwrap()
                .ends_with("**400 in 100..900**")
        );
        assert!(hover_at("advance", 0, true).unwrap().ends_with("**192**"));
        let main = hover_at("main", 0, true).unwrap();
        assert!(
            main.starts_with("```ductus\ncontour main = (0, 0) …\n```"),
            "{main}"
        );
        assert!(main.ends_with("**closed path, 2 points**"), "{main}");
    }

    #[test]
    fn def_builtin_and_prelude() {
        let def = hover_at("f(stem)", 0, true).unwrap();
        assert_eq!(def, "```ductus\nf(c) -> p\n```\nDoubles.");
        let pen = hover_at("pen(1)", 0, true).unwrap();
        assert!(pen.starts_with("```ductus\npen(w)\npen(w, h)\npen(w, h, angle)\n```"));
        let up = hover_at("up pen", 0, true).unwrap();
        assert!(up.starts_with("```ductus\nup = 90\n```"), "{up}");
    }

    #[test]
    fn parameters_and_loop_variables_have_no_value() {
        assert_eq!(
            hover_at("c * 2", 0, true).unwrap(),
            "```ductus\nf(c) -> p\n```\nparameter `c`"
        );
        assert_eq!(
            hover_at("i, 0", 0, true).unwrap(),
            "```ductus\nfor i in 1..2\n```\nloop variable"
        );
    }

    #[test]
    fn member_properties_have_no_hover() {
        assert_eq!(hover_at("left", 0, true), None);
    }

    #[test]
    fn stale_values_say_so() {
        let tree = parse_lenient(SOURCE);
        let program = lower::lower(&tree, SOURCE).unwrap();
        let document = eval::evaluate(&program, &HashMap::new(), &eval::NoImporter).unwrap();
        let offset = SOURCE.match_indices("stem").nth(1).unwrap().0;
        let read = |_: &Path| Some(SOURCE.to_string());
        let evaluated = Some(Evaluated {
            document: &document,
            stale: true,
        });
        let (markdown, _) = hover(
            &tree,
            SOURCE,
            Path::new("/virtual/main.ductus"),
            offset,
            &read,
            evaluated,
        )
        .unwrap();
        assert!(markdown.ends_with("*From the last evaluation without errors.*"));
    }
}
