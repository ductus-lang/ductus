//! ductus-docs — an mdBook preprocessor that compiles ```ductus fences into
//! figures: highlighted source on one side, the rendered SVG on the other.
//!
//! mdBook pipes `[context, book]` JSON on stdin and reads the transformed
//! book JSON from stdout; `ductus-docs supports html` answers with exit 0.
//! The book is handled as raw JSON so the only protocol dependency is
//! serde_json.
//!
//! Fence options, whitespace-separated after `ductus`:
//!
//! ```text
//! glyph=o               render only this glyph
//! set="weight=700"      param overrides for this instance (quoted, repeatable inside)
//! debug                 render guides, nodes, metrics lines and handles
//! norender              show the code only (fragments that don't compile alone)
//! hide                  append to the chapter's hidden preamble; show nothing
//! acc                   show the block *and* append it to the preamble
//! ```
//!
//! `hide` and `acc` blocks accumulate per chapter and every visible block
//! compiles as preamble + block, so tutorial fragments stay compiler-checked
//! without repeating boilerplate on the page. `norender acc` shows setup code
//! (a params block, say) without rendering it. An *empty* visible block
//! renders the preamble alone — an output-only figure for showing another
//! instance (`set=`) or view (`debug`) of geometry the chapter already
//! built. A block that fails to compile fails the book build, with the
//! chapter file and line in the message.

use ductus_core::doc::Document;
use ductus_core::render::RenderOptions;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("supports") {
        let renderer = args.get(2).map(String::as_str).unwrap_or("");
        return if renderer == "html" {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    match run() {
        Ok(book) => {
            println!("{book}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("ductus-docs: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("error reading stdin: {error}"))?;
    let (context, mut book): (Value, Value) =
        serde_json::from_str(&input).map_err(|error| format!("invalid input JSON: {error}"))?;

    let root = context.get("root").and_then(Value::as_str).unwrap_or(".");
    let src = context
        .pointer("/config/book/src")
        .and_then(Value::as_str)
        .unwrap_or("src");
    let src_dir = Path::new(root).join(src);

    // mdBook 0.5 calls the chapter list `items`; 0.4 called it `sections`.
    for key in ["items", "sections"] {
        if let Some(items) = book.get_mut(key).and_then(Value::as_array_mut) {
            process_items(items, &src_dir)?;
        }
    }
    serde_json::to_string(&book).map_err(|error| format!("error serializing book: {error}"))
}

/// Rewrite the content of every chapter, recursing into sub-chapters.
fn process_items(items: &mut [Value], src_dir: &Path) -> Result<(), String> {
    for item in items {
        let Some(chapter) = item.get_mut("Chapter") else {
            continue;
        };
        // `path` is the chapter's file relative to the src dir; draft
        // chapters have none. Its directory anchors `import` resolution.
        let rel = chapter
            .get("path")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let label = match &rel {
            Some(path) => path.display().to_string(),
            None => chapter
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("draft chapter")
                .to_string(),
        };
        let dir = rel
            .as_ref()
            .and_then(|path| src_dir.join(path).parent().map(Path::to_path_buf))
            .unwrap_or_else(|| src_dir.to_path_buf());

        if let Some(content) = chapter.get("content").and_then(Value::as_str) {
            let processed = process_chapter(content, &dir, &label)?;
            chapter["content"] = Value::String(processed);
        }
        if let Some(subs) = chapter.get_mut("sub_items").and_then(Value::as_array_mut) {
            process_items(subs, src_dir)?;
        }
    }
    Ok(())
}

// ---- Chapter processing -----------------------------------------------------

fn process_chapter(content: &str, chapter_dir: &Path, label: &str) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut preamble = String::new();
    let mut i = 0;

    while i < lines.len() {
        let Some((ticks, info)) = fence_open(lines[i]) else {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        };
        let is_ductus =
            info == "ductus" || info.starts_with("ductus ") || info.starts_with("ductus\t");
        if !is_ductus {
            // A foreign fence passes through verbatim, including any ```ductus
            // lines quoted inside it.
            out.push(lines[i].to_string());
            i += 1;
            while i < lines.len() {
                let closed = fence_close(lines[i], ticks);
                out.push(lines[i].to_string());
                i += 1;
                if closed {
                    break;
                }
            }
            continue;
        }

        let fence_line = i + 1; // 1-based, for error messages
        let options = Options::parse(&info["ductus".len()..])
            .map_err(|message| format!("{label}:{fence_line}: {message}"))?;
        i += 1;
        let start = i;
        while i < lines.len() && !fence_close(lines[i], ticks) {
            i += 1;
        }
        if i == lines.len() {
            return Err(format!(
                "{label}:{fence_line}: unterminated ```ductus block"
            ));
        }
        let block = lines[start..i].join("\n");
        i += 1; // the closing fence

        let html = process_block(&block, &options, &mut preamble, chapter_dir)
            .map_err(|error| error.report(label, fence_line))?;
        out.push(html);
    }
    Ok(out.join("\n"))
}

/// A fence opener: up to three leading spaces, then three or more backticks.
/// Returns the tick count and the trimmed info string.
fn fence_open(line: &str) -> Option<(usize, &str)> {
    let stripped = line
        .strip_prefix("   ")
        .unwrap_or(line)
        .trim_start_matches(' ');
    if line.len() - stripped.len() > 3 {
        return None; // four spaces of indent is a code block, not a fence
    }
    let ticks = stripped.chars().take_while(|&c| c == '`').count();
    if ticks < 3 {
        return None;
    }
    Some((ticks, stripped[ticks..].trim()))
}

/// A closing fence: only backticks, at least as many as the opener.
fn fence_close(line: &str, ticks: usize) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= ticks && trimmed.chars().all(|c| c == '`')
}

fn process_block(
    block: &str,
    options: &Options,
    preamble: &mut String,
    chapter_dir: &Path,
) -> Result<String, BlockError> {
    if options.hide || (options.acc && options.norender) {
        let offset = preamble.lines().count();
        preamble.push_str(block);
        preamble.push('\n');
        // Compile now, so a broken preamble is reported at its own block
        // rather than at the first visible example that uses it.
        compile(preamble, &HashMap::new(), chapter_dir)
            .map_err(|error| error.into_block(offset))?;
        return Ok(if options.hide {
            String::new()
        } else {
            format!(
                "<pre class=\"ductus-code ductus-standalone\"><code>{}</code></pre>",
                highlight(block)
            )
        });
    }

    if options.norender {
        return Ok(format!(
            "<pre class=\"ductus-code ductus-standalone\"><code>{}</code></pre>",
            highlight(block)
        ));
    }

    let full = format!("{preamble}{block}");
    let preamble_lines = preamble.lines().count();
    let document = compile(&full, &options.set, chapter_dir)
        .map_err(|error| error.into_block(preamble_lines))?;
    let render_options = RenderOptions {
        glyph: options.glyph.clone(),
        debug: options.debug,
    };
    let svg =
        ductus_core::render::render(&document, &render_options).map_err(|error| BlockError {
            line: None,
            message: error.message,
        })?;
    if options.acc {
        preamble.push_str(block);
        preamble.push('\n');
    }

    // An empty block renders the preamble alone: an output-only figure, for
    // showing another instance or view of geometry the chapter already built.
    if block.trim().is_empty() {
        let svg = svg.replace('\n', " ");
        return Ok(format!(
            "<figure class=\"ductus-example ductus-output-only\">\
             <div class=\"ductus-render\">{svg}</div>\
             </figure>"
        ));
    }
    Ok(figure(&highlight(block), &svg))
}

// ---- Fence options ----------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
struct Options {
    glyph: Option<String>,
    set: HashMap<String, f64>,
    debug: bool,
    norender: bool,
    hide: bool,
    acc: bool,
}

impl Options {
    fn parse(rest: &str) -> Result<Options, String> {
        let mut options = Options::default();
        for token in tokenize(rest)? {
            if token == "debug" {
                options.debug = true;
            } else if token == "norender" {
                options.norender = true;
            } else if token == "hide" {
                options.hide = true;
            } else if token == "acc" {
                options.acc = true;
            } else if let Some(name) = token.strip_prefix("glyph=") {
                options.glyph = Some(name.to_string());
            } else if let Some(pairs) = token.strip_prefix("set=") {
                for pair in pairs.split_whitespace() {
                    let Some((name, number)) = pair.split_once('=') else {
                        return Err(format!("set= expects PARAM=VALUE, got '{pair}'"));
                    };
                    let value: f64 = number
                        .parse()
                        .map_err(|_| format!("set= {name}: '{number}' is not a number"))?;
                    options.set.insert(name.to_string(), value);
                }
            } else {
                return Err(format!("unknown ```ductus option '{token}'"));
            }
        }
        let renders = options.debug || options.glyph.is_some() || !options.set.is_empty();
        if options.hide && (options.norender || options.acc || renders) {
            return Err("`hide` cannot be combined with other options".to_string());
        }
        if options.norender && renders {
            return Err("`norender` cannot be combined with render options".to_string());
        }
        Ok(options)
    }
}

/// Split on whitespace; a double quote spans to its matching quote and the
/// quotes themselves are dropped, so `set="a=1 b=2"` is one token.
fn tokenize(text: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in text.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if in_quotes {
        return Err("unclosed quote in ```ductus options".to_string());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

// ---- Compiling --------------------------------------------------------------

/// A failure inside one fence, with a line number when the compiler gave one.
/// Lines count within the fence's own content after preamble subtraction, so
/// `report` can point at the chapter source line. `None` marks positionless
/// errors (rendering) and preamble errors already resolved to a message.
struct BlockError {
    line: Option<usize>,
    message: String,
}

impl BlockError {
    fn report(&self, label: &str, fence_line: usize) -> String {
        match self.line {
            Some(line) => format!("{label}:{}: {}", fence_line + line, self.message),
            None => format!("{label}:{fence_line}: {}", self.message),
        }
    }
}

/// A compile failure with the 1-based line in the assembled source.
struct CompileError {
    line: usize, // 0 when the error has no position
    message: String,
}

impl CompileError {
    /// Resolve the assembled-source line against the hidden preamble: lines
    /// past the preamble belong to the fence, earlier ones are named as such.
    fn into_block(self, preamble_lines: usize) -> BlockError {
        if self.line > preamble_lines {
            BlockError {
                line: Some(self.line - preamble_lines),
                message: self.message,
            }
        } else {
            let position = if self.line > 0 {
                format!(" (line {} of the hidden preamble)", self.line)
            } else {
                String::new()
            };
            BlockError {
                line: None,
                message: format!("{}{position}", self.message),
            }
        }
    }
}

fn compile(
    source: &str,
    overrides: &HashMap<String, f64>,
    chapter_dir: &Path,
) -> Result<Document, CompileError> {
    let tree = ductus_core::parse::parse(source).map_err(|error| CompileError {
        line: error.line,
        message: error.message,
    })?;
    let program = ductus_core::lower::lower(&tree, source).map_err(|error| CompileError {
        line: error.span.line,
        message: error.message,
    })?;
    let importer = FsImporter {
        root: chapter_dir.to_path_buf(),
    };
    ductus_core::eval::evaluate(&program, overrides, &importer).map_err(|error| CompileError {
        line: error.span.line,
        message: error.message,
    })
}

/// Resolves `import` on the filesystem, relative to the chapter's directory
/// (or to the importing file for nested imports) — the CLI's importer, with
/// the chapter standing in for the root source file.
struct FsImporter {
    root: PathBuf,
}

impl ductus_core::eval::Importer for FsImporter {
    fn load(&self, path: &str, from: Option<&str>) -> Result<(String, String), String> {
        let base = match from {
            Some(from) => PathBuf::from(from)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            None => self.root.clone(),
        };
        let joined = base.join(path);
        let canonical = joined
            .canonicalize()
            .map_err(|error| format!("error reading {}: {error}", joined.display()))?;
        let source = std::fs::read_to_string(&canonical)
            .map_err(|error| format!("error reading {}: {error}", canonical.display()))?;
        Ok((canonical.to_string_lossy().into_owned(), source))
    }
}

// ---- Figure HTML ------------------------------------------------------------

/// One line of raw HTML: a blank line would end the markdown HTML block and
/// hand the rest back to the markdown parser (which would, for one, read
/// indented SVG as a code block). Newlines inside `<pre>` are `&#10;`; SVG
/// doesn't care, so its newlines become spaces.
fn figure(code_html: &str, svg: &str) -> String {
    let svg = svg.replace('\n', " ");
    format!(
        "<figure class=\"ductus-example\">\
         <pre class=\"ductus-code\"><code>{code_html}</code></pre>\
         <div class=\"ductus-render\">{svg}</div>\
         </figure>"
    )
}

// ---- Highlighting -----------------------------------------------------------

fn highlight_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        // The same query the playground uses, so the class names
        // (`ductus-keyword`, `ductus-number`, …) match its palette.
        Query::new(
            &tree_sitter_ductus::LANGUAGE.into(),
            tree_sitter_ductus::HIGHLIGHTS_QUERY,
        )
        .expect("highlights.scm is a valid query")
    })
}

/// Escaped HTML for a ductus source fragment, capture spans wrapped in
/// `<span class="ductus-...">`. Lenient parse: fragments with errors still get
/// colors for everything the parser recognized.
fn highlight(source: &str) -> String {
    let tree = ductus_core::parse::parse_lenient(source);
    let mut html = String::new();
    let mut at = 0;
    for (start, end, class) in highlight_spans(&tree, source) {
        if start < at {
            continue;
        }
        html.push_str(&escape(&source[at..start]));
        html.push_str(&format!("<span class=\"ductus-{class}\">",));
        html.push_str(&escape(&source[start..end]));
        html.push_str("</span>");
        at = end;
    }
    html.push_str(&escape(&source[at..]));
    // The figure is emitted as a single HTML line; entities keep the breaks.
    html.replace('\n', "&#10;")
}

/// Byte-range highlight spans, one per captured leaf, sorted by position.
/// When several patterns claim the same range, the latest pattern in
/// `highlights.scm` wins — the Neovim rule the query is ordered for, and
/// the same resolution as ductus-wasm's `analyze`.
fn highlight_spans(tree: &Tree, source: &str) -> Vec<(usize, usize, String)> {
    let query = highlight_query();
    let names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut claimed: HashMap<(usize, usize), (usize, String)> = HashMap::new();

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let start = capture.node.start_byte();
            let end = capture.node.end_byte();
            if start == end {
                continue; // zero-width nodes (e.g. captured MISSING tokens)
            }
            if claimed
                .get(&(start, end))
                .is_some_and(|(index, _)| *index > m.pattern_index)
            {
                continue;
            }
            claimed.insert(
                (start, end),
                (
                    m.pattern_index,
                    names[capture.index as usize].replace('.', "-"),
                ),
            );
        }
    }

    let mut spans: Vec<(usize, usize, String)> = claimed
        .into_iter()
        .map(|((start, end), (_, class))| (start, end, class))
        .collect();
    spans.sort_by_key(|(start, end, _)| (*start, *end));
    spans
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(content: &str) -> Result<String, String> {
        process_chapter(content, Path::new("."), "test.md")
    }

    const SKETCH: &str = "```ductus\nu = 100;\ndraw((0,0) .. (2u, u) .. (4u, 0));\n```";

    #[test]
    fn sketch_block_becomes_a_one_line_figure() {
        let html = chapter(SKETCH).unwrap();
        let figure = html
            .lines()
            .find(|line| line.contains("<figure"))
            .expect("a figure line");
        assert!(figure.contains("<svg"), "{figure}");
        assert!(figure.contains("</figure>"), "{figure}");
        assert!(figure.contains("ductus-keyword\">draw<") || figure.contains("ductus-function"));
    }

    #[test]
    fn text_and_foreign_fences_pass_through() {
        let content =
            "# Title\n\n```rust\nfn main() {}\n```\n\n````md\n```ductus\nnot compiled\n```\n````\n";
        assert_eq!(chapter(content).unwrap(), content.trim_end_matches('\n'));
    }

    #[test]
    fn hide_feeds_the_next_block_and_shows_nothing() {
        let content = "```ductus hide\nu = 100;\n```\n\n```ductus\ndraw((0,0) -- (u, u));\n```";
        let html = chapter(content).unwrap();
        assert!(!html.contains("u = 100"), "hidden block leaked: {html}");
        assert!(html.contains("<figure"));
    }

    #[test]
    fn norender_shows_code_without_compiling() {
        let content = "```ductus norender\nnode n = q dir 45 pen pen(w);\n```";
        let html = chapter(content).unwrap();
        assert!(html.contains("ductus-standalone"));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn errors_point_at_the_chapter_line() {
        // Line 1 is text, line 2 opens the fence, line 4 is the bad statement.
        let content = "intro\n```ductus\nu = 100;\nv = boom;\n```";
        let error = chapter(content).unwrap_err();
        assert!(error.starts_with("test.md:4:"), "{error}");
    }

    #[test]
    fn preamble_errors_report_their_own_line() {
        // The bad statement sits on chapter line 2, inside the hide block.
        let content = "```ductus hide\nx = nope;\n```";
        let error = chapter(content).unwrap_err();
        assert!(error.starts_with("test.md:2:"), "{error}");
    }

    #[test]
    fn set_selects_an_instance() {
        let source = "```ductus glyph=g set=\"w=200\"\nparams { w = 100 in 0..200; }\nglyph g { advance = w; contour c = (0,0) -- (w,0) -- (w,10) -- cycle; }\n```";
        let wide = chapter(source).unwrap();
        let default = chapter(&source.replace(" set=\"w=200\"", "")).unwrap();
        assert_ne!(wide, default);
    }

    #[test]
    fn debug_renders_the_grid() {
        let html = chapter(&SKETCH.replace("```ductus\n", "```ductus debug\n")).unwrap();
        assert!(html.contains("ductus-grid"), "{html}");
    }

    #[test]
    fn acc_shows_the_block_and_feeds_the_next_one() {
        let content = "```ductus norender acc\nu = 100;\n```\n\n```ductus acc\ndraw((0,0) -- (u, u));\n```\n\n```ductus\nfill((0,0) -- (u,0) -- 0.5[(0,0), (u,u)] -- cycle);\n```";
        let html = chapter(content).unwrap();
        assert!(
            html.contains("ductus-standalone"),
            "acc block is visible: {html}"
        );
        assert!(html.contains(">100<"), "acc block is visible: {html}");
        // Three ductus blocks, two of them rendered (norender acc has no pane).
        assert_eq!(html.matches("<svg").count(), 2, "{html}");
    }

    #[test]
    fn empty_block_is_an_output_only_figure() {
        let content = "```ductus hide\nu = 100;\ndraw((0,0) .. (u, u) .. (2u, 0));\n```\n\n```ductus debug\n```";
        let html = chapter(content).unwrap();
        assert!(html.contains("ductus-output-only"), "{html}");
        assert!(html.contains("<svg"), "{html}");
        assert!(!html.contains("<pre"), "no code pane: {html}");
    }

    #[test]
    fn unknown_options_are_rejected() {
        let error = chapter("```ductus dbug\nu = 1;\n```").unwrap_err();
        assert!(error.contains("unknown ```ductus option 'dbug'"), "{error}");
        assert!(Options::parse(" hide debug").is_err());
        assert!(Options::parse(" norender glyph=o").is_err());
    }

    #[test]
    fn unterminated_block_is_an_error() {
        let error = chapter("```ductus\nu = 1;\n").unwrap_err();
        assert!(error.contains("unterminated"), "{error}");
    }

    #[test]
    fn code_is_escaped_and_single_line() {
        let html = chapter("```ductus norender\na = 1 if 2 < 3 else 4; # <tag> & co\n```").unwrap();
        assert_eq!(html.lines().count(), 1);
        assert!(html.contains("&lt;tag&gt; &amp; co"));
    }
}
