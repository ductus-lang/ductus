use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "chi", about = "Chi — parametric type design language")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a source file for syntax errors.
    Check {
        /// Path to the .chi file
        file: PathBuf,
    },
    /// Evaluate a source file and render it to SVG.
    Build {
        /// Path to the .chi file
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
            chi_core::parse::parse(&source)
                .map_err(|error| format!("{}: {}", file.display(), error))?;
            Ok(format!("{}: OK", file.display()))
        }
        Command::Build {
            file,
            glyph,
            set,
            debug,
            output,
        } => {
            let source = read(&file)?;
            let overrides = parse_overrides(&set)?;

            let tree = chi_core::parse::parse(&source)
                .map_err(|error| format!("{}: {}", file.display(), error))?;
            let program = chi_core::lower::lower(&tree, &source)
                .map_err(|error| format!("{}: {}", file.display(), error))?;
            let importer = FsImporter {
                root: file.parent().unwrap_or(Path::new(".")).to_path_buf(),
            };
            let document = chi_core::eval::evaluate(&program, &overrides, &importer)
                .map_err(|error| format!("{}: {}", file.display(), error))?;

            let options = chi_core::render::RenderOptions { glyph, debug };
            let svg = chi_core::render::render(&document, &options)
                .map_err(|error| format!("{}: {}", file.display(), error))?;

            std::fs::write(&output, svg)
                .map_err(|error| format!("error writing {}: {}", output.display(), error))?;
            Ok(format!("{} -> {}", file.display(), output.display()))
        }
    }
}

/// Resolves imports on the filesystem, relative to the importing file
/// (or to the root source file's directory for top-level imports). The
/// canonical id is the canonicalized path, so the same file reached via
/// different relative routes is still imported once.
struct FsImporter {
    root: PathBuf,
}

impl chi_core::eval::Importer for FsImporter {
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

fn read(file: &PathBuf) -> Result<String, String> {
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
