//! WebAssembly bindings for the playground: one call from source to SVG.

use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(untagged)]
enum Response {
    Ok {
        ok: bool,
        svg: String,
        params: Vec<ParamInfo>,
        glyphs: Vec<String>,
        /// Metrics, top-level bindings, and each glyph's own numbers, for
        /// the inspector.
        measures: Measures,
    },
    Err {
        ok: bool,
        error: ErrorInfo,
    },
}

#[derive(Serialize)]
struct ParamInfo {
    name: String,
    value: f64,
    default: f64,
    min: f64,
    max: f64,
}

/// Named numbers, as (name, value) pairs in the order they should be shown.
/// Glyphs are a list, not a map, so the panel keeps document order rather
/// than whatever a hash gives it.
#[derive(Serialize)]
struct Measures {
    metrics: Vec<(String, f64)>,
    bindings: Vec<(String, f64)>,
    glyphs: Vec<(String, Vec<(String, f64)>)>,
}

#[derive(Serialize)]
struct ErrorInfo {
    /// 1-based; 0 when the error has no source position.
    line: usize,
    column: usize,
    message: String,
}

/// Compile ductus source to SVG.
///
/// `overrides_json`: JSON object of param overrides, e.g. `{"weight": 700}`.
/// `glyph`: render only this glyph (empty string = all).
/// Returns a JSON string: `{ok, svg, params, glyphs}` or `{ok, error}`.
#[wasm_bindgen]
pub fn compile(source: &str, overrides_json: &str, glyph: &str, debug: bool) -> String {
    console_error_panic_hook::set_once();
    let response = match compile_inner(source, overrides_json, glyph, debug) {
        Ok(response) => response,
        Err(error) => Response::Err { ok: false, error },
    };
    serde_json::to_string(&response).expect("response serializes")
}

fn compile_inner(
    source: &str,
    overrides_json: &str,
    glyph: &str,
    debug: bool,
) -> Result<Response, ErrorInfo> {
    let overrides: HashMap<String, f64> = if overrides_json.trim().is_empty() {
        HashMap::new()
    } else {
        serde_json::from_str(overrides_json).map_err(|error| ErrorInfo {
            line: 0,
            column: 0,
            message: format!("invalid overrides: {error}"),
        })?
    };

    let tree = ductus_core::parse::parse(source).map_err(|error| ErrorInfo {
        line: error.line,
        column: error.column,
        message: error.message,
    })?;

    let program = ductus_core::lower::lower(&tree, source).map_err(|error| ErrorInfo {
        line: error.span.line,
        column: error.span.column,
        message: error.message,
    })?;

    // The playground has no filesystem; imports report a clear error.
    let document = ductus_core::eval::evaluate(&program, &overrides, &ductus_core::eval::NoImporter)
        .map_err(|error| ErrorInfo {
        line: error.span.line,
        column: error.span.column,
        message: error.message,
    })?;

    let options = ductus_core::render::RenderOptions {
        glyph: (!glyph.is_empty()).then(|| glyph.to_string()),
        debug,
    };
    let svg = ductus_core::render::render(&document, &options).map_err(|error| ErrorInfo {
        line: 0,
        column: 0,
        message: error.message,
    })?;

    Ok(Response::Ok {
        ok: true,
        svg,
        params: document
            .params
            .iter()
            .map(|param| ParamInfo {
                name: param.name.clone(),
                value: param.value,
                default: param.default,
                min: param.min,
                max: param.max,
            })
            .collect(),
        glyphs: document
            .glyphs
            .iter()
            .map(|glyph| glyph.name.clone())
            .collect(),
        measures: Measures {
            // The document's metrics are a map; sort so the panel is stable
            // between edits. Bindings already arrive in source order.
            metrics: {
                let mut metrics: Vec<(String, f64)> = document
                    .metrics
                    .values
                    .iter()
                    .map(|(name, value)| (name.clone(), *value))
                    .collect();
                metrics.sort_by(|a, b| a.0.cmp(&b.0));
                metrics
            },
            bindings: document.bindings.clone(),
            glyphs: document
                .glyphs
                .iter()
                .map(|glyph| {
                    let mut numbers = vec![("advance".to_string(), glyph.advance)];
                    numbers.extend(glyph.bindings.iter().cloned());
                    (glyph.name.clone(), numbers)
                })
                .collect(),
        },
    })
}

/// The highlight query, embedded from the sibling grammar repo at build time.
///
/// Editing `highlights.scm` requires `bun run sync-wasm` in the playground for
/// the change to take effect.
const HIGHLIGHTS_QUERY: &str = include_str!("../../../tree-sitter-ductus/queries/highlights.scm");

fn highlight_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_ductus::LANGUAGE.into(), HIGHLIGHTS_QUERY)
            .expect("highlights.scm is a valid query")
    })
}

#[derive(Serialize)]
struct Analysis {
    highlights: Vec<Highlight>,
    errors: Vec<SyntaxErrorInfo>,
}

#[derive(Serialize)]
struct Highlight {
    /// UTF-16 code-unit offsets (what CodeMirror positions are).
    from: usize,
    to: usize,
    class: String,
}

#[derive(Serialize, Debug)]
struct SyntaxErrorInfo {
    /// UTF-16 code-unit offsets.
    from: usize,
    to: usize,
    line: usize,
    column: usize,
    message: String,
}

/// Parse-only pass for the editor: syntax colors and all syntax errors at once.
///
/// Returns JSON `{ "highlights": [...], "errors": [...] }`. Offsets are UTF-16
/// code units so they map directly onto CodeMirror positions.
#[wasm_bindgen]
pub fn analyze(source: &str) -> String {
    console_error_panic_hook::set_once();
    serde_json::to_string(&analyze_inner(source)).expect("analysis serializes")
}

fn analyze_inner(source: &str) -> Analysis {
    let tree = ductus_core::parse::parse_lenient(source);
    let spans = highlight_spans(&tree, source);
    let errors = ductus_core::parse::syntax_errors(&tree, source);

    // One pass converts every byte offset (highlights and errors) to UTF-16.
    let mut targets = BTreeSet::new();
    for (start, end, _) in &spans {
        targets.insert(*start);
        targets.insert(*end);
    }
    for error in &errors {
        targets.insert(error.start_byte);
        targets.insert(error.end_byte);
    }
    let utf16 = utf16_offsets(source, &targets);

    Analysis {
        highlights: spans
            .into_iter()
            .map(|(start, end, class)| Highlight {
                from: utf16[&start],
                to: utf16[&end],
                class,
            })
            .collect(),
        errors: errors
            .into_iter()
            .map(|error| SyntaxErrorInfo {
                from: utf16[&error.start_byte],
                to: utf16[&error.end_byte],
                line: error.line,
                column: error.column,
                message: error.message,
            })
            .collect(),
    }
}

/// Byte-range highlight spans, one per captured leaf, sorted by position.
///
/// When several patterns claim the same range, the earliest pattern in
/// `highlights.scm` wins — the ordering the query was written and validated
/// with. Captured nodes are leaves, so conflicts are always identical ranges.
fn highlight_spans(tree: &Tree, source: &str) -> Vec<(usize, usize, String)> {
    let query = highlight_query();
    let names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut claimed: HashMap<(usize, usize), (usize, String)> = HashMap::new();

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let start = capture.node.start_byte();
            let end = capture.node.end_byte();
            if start == end {
                continue; // skip zero-width nodes (e.g. captured MISSING tokens)
            }
            let claim = claimed.entry((start, end)).or_insert((usize::MAX, String::new()));
            if m.pattern_index < claim.0 {
                *claim = (m.pattern_index, names[capture.index as usize].replace('.', "-"));
            }
        }
    }

    let mut spans: Vec<(usize, usize, String)> = claimed
        .into_iter()
        .map(|((start, end), (_, class))| (start, end, class))
        .collect();
    spans.sort_by_key(|(start, end, _)| (*start, *end));
    spans
}

/// Resolve each target UTF-8 byte offset to its UTF-16 code-unit index in one
/// pass. Targets fall on character boundaries (node edges or string end).
fn utf16_offsets(source: &str, targets: &BTreeSet<usize>) -> HashMap<usize, usize> {
    let mut map = HashMap::with_capacity(targets.len());
    let mut pending = targets.iter().peekable();
    let mut units = 0usize;
    for (byte, ch) in source.char_indices() {
        while let Some(&&target) = pending.peek() {
            if target <= byte {
                map.insert(target, units);
                pending.next();
            } else {
                break;
            }
        }
        units += ch.len_utf16();
    }
    // Any remaining targets sit at (or past) the end of the source.
    for &target in pending {
        map.insert(target, units);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_a_glyph() {
        let response = compile(
            "metrics { upm = 1000; asc = 750; desc = -220; }\nglyph l { advance = 100; contour c = (0,0) -- (50,0) -- (50,700) -- (0,700) -- cycle; }\n",
            "",
            "",
            false,
        );
        assert!(response.contains("\"ok\":true"), "{response}");
        assert!(response.contains("<svg"));
    }

    #[test]
    fn reports_errors_with_position() {
        let response = compile("x = undefined_thing;\n", "", "", false);
        assert!(response.contains("\"ok\":false"), "{response}");
        assert!(response.contains("\"line\":1"));
    }

    #[test]
    fn applies_overrides() {
        let source = "params { w = 100 in 0..200; }\nglyph g { advance = w; contour c = (0,0) -- (w,0) -- (w,10) -- cycle; }\n";
        let default = compile(source, "", "", false);
        let wide = compile(source, "{\"w\": 200}", "", false);
        assert!(default.contains("\"ok\":true"));
        assert!(wide.contains("\"ok\":true"));
        assert_ne!(default, wide);
    }

    /// The highlight whose ASCII text equals `text`. Panics if none — every
    /// source used here is ASCII, so UTF-16 offsets equal byte offsets.
    fn highlight_class(source: &str, text: &str) -> String {
        let analysis = analyze_inner(source);
        analysis
            .highlights
            .into_iter()
            .find(|h| &source[h.from..h.to] == text)
            .unwrap_or_else(|| panic!("no highlight for {text:?}"))
            .class
    }

    #[test]
    fn keyword_is_highlighted_as_keyword() {
        assert_eq!(highlight_class("params { w = 1 in 0..2; }\n", "params"), "keyword");
    }

    #[test]
    fn member_property_is_property_not_keyword() {
        // The regex mode's known bug: `width` in `n.width` colored as keyword.
        assert_eq!(highlight_class("x = n.width;\n", "width"), "property");
    }

    #[test]
    fn clean_source_has_no_errors() {
        assert!(analyze_inner("u = 100;\n").errors.is_empty());
    }

    #[test]
    fn two_broken_statements_yield_two_errors() {
        let analysis = analyze_inner("u = 100 200;\nv = 300 400;\n");
        assert!(
            analysis.errors.len() >= 2,
            "expected >=2 errors, got {:?}",
            analysis.errors
        );
    }

    #[test]
    fn malayalam_comment_does_not_shift_offsets() {
        // UTF-16 (JS String.length) semantics: the `number` span for 100 must
        // start at the UTF-16 length of everything before it, not the byte length.
        let source = "# മലയാളം\nu = 100;\n";
        let analysis = analyze_inner(source);
        let number = analysis
            .highlights
            .iter()
            .find(|h| h.class == "number")
            .expect("100 is highlighted as a number");
        let expected = source[..source.find("100").unwrap()].encode_utf16().count();
        assert_eq!(number.from, expected);
        assert_eq!(number.to, expected + 3);
    }
}
