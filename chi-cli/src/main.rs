use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { file } => {
            let source = match std::fs::read_to_string(&file) {
                Ok(source) => source,
                Err(error) => {
                    eprintln!("error reading {}: {}", file.display(), error);
                    return ExitCode::FAILURE;
                }
            };
            match chi_core::parse::parse(&source) {
                Ok(_) => {
                    println!("{}: OK", file.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{}: {}", file.display(), error);
                    ExitCode::FAILURE
                }
            }
        }
    }
}
