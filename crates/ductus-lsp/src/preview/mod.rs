//! Live preview: a browser page that shows the glyph under the editor's
//! cursor, re-rendered on every change from the unsaved buffer.
//!
//! The main loop owns the documents and renders frames; `http` serves the
//! page, streams the frames to it, and passes its settings back.

mod http;

pub use http::Preview;

use crate::syntax::{self, text};
use ductus_core::doc::Document;
use ductus_core::render::{self, RenderOptions};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

/// "glyph" typed on a T9 keypad.
pub const DEFAULT_PORT: u16 = 45974;

/// Which glyph the page shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    /// The glyph under the cursor.
    Follow,
    /// Every glyph in the file.
    All,
    /// One glyph, wherever the cursor is.
    Glyph(String),
}

/// What the page asks for; it sends them on every change and whenever it
/// (re)connects.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub overrides: HashMap<String, f64>,
    pub debug: bool,
    pub choice: Choice,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            overrides: HashMap::new(),
            debug: true,
            choice: Choice::Follow,
        }
    }
}

impl Settings {
    /// `{"overrides": {"weight": 700}, "debug": true, "glyph": null}`, where
    /// a `null` glyph follows the cursor and `""` shows all glyphs.
    pub fn from_json(text: &str) -> Option<Settings> {
        let value: Value = serde_json::from_str(text).ok()?;
        let overrides = value["overrides"]
            .as_object()?
            .iter()
            .filter_map(|(name, value)| Some((name.clone(), value.as_f64()?)))
            .collect();
        let choice = match &value["glyph"] {
            Value::Null => Choice::Follow,
            Value::String(name) if name.is_empty() => Choice::All,
            Value::String(name) => Choice::Glyph(name.clone()),
            _ => return None,
        };
        Some(Settings {
            overrides,
            debug: value["debug"].as_bool()?,
            choice,
        })
    }
}

/// The document the preview follows, and the glyph in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Focus {
    pub path: PathBuf,
    /// `None` for a sketch file, rendered whole.
    pub glyph: Option<String>,
}

/// The focus after the cursor moves to `offset` in the document at
/// `path`: its glyph when the cursor is in one, the file when it is a
/// sketch, and otherwise unchanged, so that editing params, a `def` or an
/// imported library keeps showing the glyph being worked on.
pub fn refocus(
    current: Option<&Focus>,
    path: &Path,
    tree: &Tree,
    source: &str,
    offset: usize,
) -> Option<Focus> {
    if let Some(glyph) = glyph_at(tree, source, offset) {
        return Some(Focus {
            path: path.to_path_buf(),
            glyph: Some(glyph),
        });
    }
    if is_sketch(tree, source) {
        return Some(Focus {
            path: path.to_path_buf(),
            glyph: None,
        });
    }
    current.cloned()
}

fn glyph_at(tree: &Tree, source: &str, offset: usize) -> Option<String> {
    let mut node = tree.root_node().descendant_for_byte_range(offset, offset);
    while let Some(ancestor) = node {
        if ancestor.kind() == "glyph_block" {
            let name = ancestor.child_by_field_name("name")?;
            return Some(text(name, source).trim_matches('"').to_string());
        }
        node = ancestor.parent();
    }
    None
}

/// A file with top-level `draw` or `fill` and no glyphs.
fn is_sketch(tree: &Tree, source: &str) -> bool {
    let statements = syntax::children(tree.root_node());
    let draws = |statement: &tree_sitter::Node| {
        statement.kind() == "call_statement"
            && statement
                .named_child(0)
                .and_then(|call| call.child_by_field_name("function"))
                .is_some_and(|function| matches!(text(function, source), "draw" | "fill"))
    };
    statements.iter().any(draws) && !statements.iter().any(|s| s.kind() == "glyph_block")
}

/// The glyph to render: `None` renders the whole file.
pub fn shown(choice: &Choice, focus: &Focus) -> Option<String> {
    match choice {
        Choice::Follow => focus.glyph.clone(),
        Choice::All => None,
        Choice::Glyph(name) => Some(name.clone()),
    }
}

/// A rendering and what the page builds its controls from.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub svg: String,
    pub glyphs: Vec<String>,
    pub params: Vec<Value>,
}

pub fn render(document: &Document, glyph: Option<&str>, debug: bool) -> Result<Rendered, String> {
    let options = RenderOptions {
        glyph: glyph.map(str::to_string),
        debug,
    };
    let svg = render::render(document, &options).map_err(|error| error.message)?;
    Ok(Rendered {
        svg,
        glyphs: document
            .glyphs
            .iter()
            .map(|glyph| glyph.name.clone())
            .collect(),
        params: document
            .params
            .iter()
            .map(|param| {
                json!({
                    "name": param.name, "min": param.min, "max": param.max,
                    "default": param.default, "value": param.value,
                })
            })
            .collect(),
    })
}

/// Why a frame has no fresh rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub message: String,
    /// 1-based, when the error has a place in the source.
    pub line: Option<usize>,
}

/// The JSON the page receives. On a failure the last rendering is kept,
/// marked stale, so the glyph stays in view while the error is fixed.
pub fn frame(
    file: &str,
    glyph: Option<&str>,
    debug: bool,
    rendered: Option<&Rendered>,
    failure: Option<&Failure>,
) -> String {
    json!({
        "file": file,
        "glyph": glyph,
        "debug": debug,
        "svg": rendered.map(|rendered| &rendered.svg),
        "glyphs": rendered.map(|rendered| &rendered.glyphs),
        "params": rendered.map(|rendered| &rendered.params),
        "stale": failure.is_some(),
        "error": failure.map(|failure| json!({ "message": failure.message, "line": failure.line })),
    })
    .to_string()
}

/// The frame before anything is in focus.
pub fn waiting() -> String {
    json!({ "message": "Move the cursor into a glyph." }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ductus_core::parse::parse_lenient;
    use ductus_core::{eval, lower};

    const SOURCE: &str = "params {\n  weight = 400 in 100..900;\n}\nstem = 1;\n\
        glyph o {\n  advance = 500;\n}\nglyph \"ka.mlym\" {\n  advance = 600;\n}\n";

    fn focus_at(current: Option<&Focus>, source: &str, needle: &str) -> Option<Focus> {
        let tree = parse_lenient(source);
        let offset = source.find(needle).unwrap();
        refocus(current, Path::new("/v/main.ductus"), &tree, source, offset)
    }

    fn glyph(name: &str) -> Focus {
        Focus {
            path: PathBuf::from("/v/main.ductus"),
            glyph: Some(name.to_string()),
        }
    }

    #[test]
    fn follows_the_glyph_under_the_cursor() {
        assert_eq!(focus_at(None, SOURCE, "advance = 500"), Some(glyph("o")));
        assert_eq!(
            focus_at(None, SOURCE, "advance = 600"),
            Some(glyph("ka.mlym"))
        );
    }

    #[test]
    fn outside_glyphs_the_focus_stays() {
        let current = glyph("o");
        assert_eq!(
            focus_at(Some(&current), SOURCE, "stem"),
            Some(current.clone())
        );
        assert_eq!(focus_at(None, SOURCE, "weight"), None);
        let library = "unit = 1;\ndef f(c) -> c {\n}\n";
        assert_eq!(focus_at(Some(&current), library, "unit"), Some(current));
    }

    #[test]
    fn a_sketch_is_shown_whole() {
        let sketch = "u = 1;\ndraw((0, 0) .. (u, u));\n";
        let focus = focus_at(None, sketch, "u = 1");
        assert_eq!(focus.map(|focus| focus.glyph), Some(None));
    }

    #[test]
    fn settings_from_the_page() {
        let settings =
            Settings::from_json(r#"{"overrides":{"weight":700},"debug":false,"glyph":null}"#);
        let settings = settings.unwrap();
        assert_eq!(settings.overrides["weight"], 700.0);
        assert!(!settings.debug);
        assert_eq!(settings.choice, Choice::Follow);
        let all = Settings::from_json(r#"{"overrides":{},"debug":true,"glyph":""}"#).unwrap();
        assert_eq!(all.choice, Choice::All);
        let pinned = Settings::from_json(r#"{"overrides":{},"debug":true,"glyph":"o"}"#).unwrap();
        assert_eq!(pinned.choice, Choice::Glyph("o".to_string()));
        assert_eq!(Settings::from_json("not json"), None);
    }

    #[test]
    fn frames_keep_the_last_rendering_on_failure() {
        let tree = parse_lenient(SOURCE);
        let program = lower::lower(&tree, SOURCE).unwrap();
        let document = eval::evaluate(&program, &HashMap::new(), &eval::NoImporter).unwrap();
        let rendered = render(&document, Some("o"), true).unwrap();
        assert!(rendered.svg.starts_with("<svg"));
        assert_eq!(rendered.glyphs, ["o", "ka.mlym"]);
        assert_eq!(rendered.params[0]["max"], 900.0);

        let failure = Failure {
            message: "undefined variable: w".to_string(),
            line: Some(3),
        };
        let frame: Value = serde_json::from_str(&frame(
            "main.ductus",
            Some("o"),
            true,
            Some(&rendered),
            Some(&failure),
        ))
        .unwrap();
        assert_eq!(frame["stale"], true);
        assert_eq!(frame["error"]["line"], 3);
        assert!(frame["svg"].as_str().unwrap().starts_with("<svg"));
    }
}
