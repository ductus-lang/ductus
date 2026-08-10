//! Formatter invariants over the real source corpus: formatting must be
//! idempotent, and it must not change what the source parses to.

use std::path::{Path, PathBuf};

/// Every `.ductus` file under `playground/examples` and `docs/src/examples`.
fn corpus() -> Vec<(PathBuf, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for directory in ["playground/examples", "docs/src/examples"] {
        let entries = std::fs::read_dir(root.join(directory)).expect("corpus directory exists");
        for entry in entries {
            let path = entry.expect("readable directory entry").path();
            if path
                .extension()
                .is_some_and(|extension| extension == "ductus")
            {
                let source = std::fs::read_to_string(&path).expect("readable source file");
                sources.push((path, source));
            }
        }
    }
    assert!(!sources.is_empty(), "the corpus should not be empty");
    sources
}

#[test]
fn formatting_is_idempotent_over_the_corpus() {
    for (path, source) in corpus() {
        let once = ductus_core::format::format(&source)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let twice = ductus_core::format::format(&once)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            once,
            twice,
            "{} does not reach a fixed point",
            path.display()
        );
    }
}

#[test]
fn formatting_preserves_the_parse_tree_over_the_corpus() {
    for (path, source) in corpus() {
        let formatted = ductus_core::format::format(&source)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let before = ductus_core::parse::parse(&source).expect("corpus file parses");
        let after = ductus_core::parse::parse(&formatted).unwrap_or_else(|error| {
            panic!(
                "{}: formatted output does not parse: {error}",
                path.display()
            )
        });
        // The S-expression covers named nodes, comments included, so this
        // also proves no comment is dropped or reordered.
        assert_eq!(
            before.root_node().to_sexp(),
            after.root_node().to_sexp(),
            "{}: formatting changed the parse",
            path.display()
        );
        // The S-expression carries node kinds only, no text: it would not
        // catch a mangled number or identifier. Compare every token too.
        assert_eq!(
            leaf_tokens(before.root_node(), &source),
            leaf_tokens(after.root_node(), &formatted),
            "{}: formatting changed a token",
            path.display()
        );
    }
}

/// Every leaf token of the tree in document order, as (kind, text) —
/// operators, keywords and punctuation included. Comment text is compared
/// without trailing whitespace, which the formatter trims.
fn leaf_tokens(node: tree_sitter::Node, source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    if node.child_count() == 0 {
        let text = node.utf8_text(source.as_bytes()).expect("valid UTF-8");
        out.push((node.kind().to_string(), text.trim_end().to_string()));
    }
    for child in node.children(&mut cursor) {
        out.extend(leaf_tokens(child, source));
    }
    out
}
