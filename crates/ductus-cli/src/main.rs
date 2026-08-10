use ductus_core::doc::Document;
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "ductus", about = "Ductus — parametric type design language")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a source file for syntax errors.
    Check {
        /// Path to the .ductus file
        file: PathBuf,
    },
    /// Evaluate a source file and render it to SVG.
    Build {
        /// Path to the .ductus file
        file: PathBuf,
        /// Render only this glyph
        #[arg(long)]
        glyph: Option<String>,
        /// Override a param, e.g. --set weight=700 (repeatable)
        #[arg(long = "set", value_name = "PARAM=VALUE")]
        set: Vec<String>,
        /// Draw guides, skeleton nodes, metrics lines and handles
        #[arg(long)]
        debug: bool,
        /// Output SVG path
        #[arg(short, long, default_value = "output.svg")]
        output: PathBuf,
    },
    /// Rewrite source files into the canonical format.
    Format {
        /// Paths to .ductus files
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Write nothing; fail when a file is not formatted
        #[arg(long)]
        check: bool,
    },
    /// Evaluate a source file and print its numbers: params, metrics,
    /// derived bindings, and each glyph's advance, bindings and geometry.
    Inspect {
        /// Path to the .ductus file
        file: PathBuf,
        /// Override a param, e.g. --set weight=700 (repeatable)
        #[arg(long = "set", value_name = "PARAM=VALUE")]
        set: Vec<String>,
        /// Inspect only this glyph
        #[arg(long)]
        glyph: Option<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<String, String> {
    match cli.command {
        Command::Check { file } => {
            let source = read(&file)?;
            ductus_core::parse::parse(&source)
                .map_err(|error| format!("{}: {}", file.display(), error))?;
            Ok(format!("{}: OK", file.display()))
        }
        Command::Format { files, check } => {
            let mut changed = Vec::new();
            for file in &files {
                let source = read(file)?;
                let formatted = ductus_core::format::format(&source)
                    .map_err(|error| format!("{}: {}", file.display(), error))?;
                if formatted == source {
                    continue;
                }
                if !check {
                    std::fs::write(file, &formatted)
                        .map_err(|error| format!("error writing {}: {}", file.display(), error))?;
                }
                changed.push(file.display().to_string());
            }
            match (check, changed.is_empty()) {
                (true, false) => Err(format!("not formatted:\n{}", changed.join("\n"))),
                (true, true) => Ok("all files formatted".to_string()),
                (false, true) => Ok("no changes".to_string()),
                (false, false) => Ok(format!("reformatted:\n{}", changed.join("\n"))),
            }
        }
        Command::Inspect { file, set, glyph } => {
            let document = evaluate(&file, &set)?;
            report(&document, glyph.as_deref())
        }
        Command::Build {
            file,
            glyph,
            set,
            debug,
            output,
        } => {
            let document = evaluate(&file, &set)?;

            let options = ductus_core::render::RenderOptions { glyph, debug };
            let svg = ductus_core::render::render(&document, &options)
                .map_err(|error| format!("{}: {}", file.display(), error))?;

            std::fs::write(&output, svg)
                .map_err(|error| format!("error writing {}: {}", output.display(), error))?;
            Ok(format!("{} -> {}", file.display(), output.display()))
        }
    }
}

/// Parse, lower and evaluate a file, resolving imports from its directory.
fn evaluate(file: &Path, set: &[String]) -> Result<Document, String> {
    let source = read(file)?;
    let overrides = parse_overrides(set)?;
    let named = |error: String| format!("{}: {error}", file.display());

    let tree = ductus_core::parse::parse(&source).map_err(|error| named(error.to_string()))?;
    let program =
        ductus_core::lower::lower(&tree, &source).map_err(|error| named(error.to_string()))?;
    let importer = FsImporter {
        root: file.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    ductus_core::eval::evaluate(&program, &overrides, &importer)
        .map_err(|error| named(error.to_string()))
}

/// Every number in an evaluated document, as plain text. This is the CLI
/// half of what the playground's inspector shows; between them they are the
/// only way to read a derived value back out of a source file.
fn report(document: &Document, only: Option<&str>) -> Result<String, String> {
    let mut out = String::new();
    let row = |out: &mut String, name: &str, value: String| {
        out.push_str(&format!("  {name:<16}{value:>12}\n"));
    };

    if !document.params.is_empty() {
        out.push_str("params\n");
        for param in &document.params {
            let range = format!("  in {}..{}", number(param.min), number(param.max));
            out.push_str(&format!(
                "  {:<16}{:>12}{range}\n",
                param.name,
                number(param.value)
            ));
        }
    }

    // Metrics live in a map, so they come out by name.
    let mut metrics: Vec<(&String, &f64)> = document.metrics.values.iter().collect();
    metrics.sort_by(|a, b| a.0.cmp(b.0));
    if !metrics.is_empty() {
        out.push_str("metrics\n");
        for (name, value) in metrics {
            row(&mut out, name, number(*value));
        }
    }

    if !document.bindings.is_empty() {
        out.push_str("bindings\n");
        for (name, value) in &document.bindings {
            row(&mut out, name, number(*value));
        }
    }

    let mut found = false;
    for glyph in &document.glyphs {
        if only.is_some_and(|wanted| wanted != glyph.name) {
            continue;
        }
        found = true;
        out.push_str(&format!("\nglyph {}\n", glyph.name));
        row(&mut out, "advance", number(glyph.advance));
        for (name, value) in &glyph.bindings {
            row(&mut out, name, number(*value));
        }
        // Point counts, because a contour keeping the same count across the
        // design space is what interpolation compatibility asks of it.
        for (name, path) in &glyph.contours {
            row(&mut out, &format!("contour {name}"), points(path.points.len()));
        }
        for (name, path) in &glyph.guides {
            row(&mut out, &format!("guide {name}"), points(path.points.len()));
        }
        for (name, at) in &glyph.anchors {
            row(
                &mut out,
                &format!("anchor {name}"),
                format!("({}, {})", number(at.x), number(at.y)),
            );
        }
    }
    if let Some(wanted) = only
        && !found
    {
        return Err(format!("no glyph named '{wanted}' in this file"));
    }
    Ok(out)
}

/// Compact enough to scan a column of them: four decimals at most, and no
/// trailing zeros, so an advance reads `680` and a probe `0.0678`.
fn number(value: f64) -> String {
    let text = format!("{value:.4}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

fn points(count: usize) -> String {
    format!("{count} point{}", if count == 1 { "" } else { "s" })
}

/// Resolves imports on the filesystem, relative to the importing file
/// (or to the root source file's directory for top-level imports). The
/// canonical id is the canonicalized path, so the same file reached via
/// different relative routes is still imported once.
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

fn read(file: &Path) -> Result<String, String> {
    std::fs::read_to_string(file)
        .map_err(|error| format!("error reading {}: {}", file.display(), error))
}

fn parse_overrides(set: &[String]) -> Result<HashMap<String, f64>, String> {
    let mut overrides = HashMap::new();
    for entry in set {
        let Some((name, value)) = entry.split_once('=') else {
            return Err(format!("--set expects PARAM=VALUE, got '{entry}'"));
        };
        let value: f64 = value
            .parse()
            .map_err(|_| format!("--set {name}: '{value}' is not a number"))?;
        overrides.insert(name.to_string(), value);
    }
    Ok(overrides)
}
