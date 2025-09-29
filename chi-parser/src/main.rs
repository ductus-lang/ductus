use clap::Parser;
use std::fs;
use std::path::PathBuf;

mod ast_parser;
mod evaluator;
mod interpreter;
mod ir;
mod svg_generator;
mod value;

use interpreter::Interpreter;

#[derive(Parser)]
#[command(name = "chi-parser")]
#[command(about = "A parser for .chi files using tree-sitter")]
struct Args {
    /// Path to the .rp file to parse
    file: PathBuf,

    /// Output SVG file path
    #[arg(short, long, default_value = "output.svg")]
    output: PathBuf,

    /// Enable debug mode to show point labels
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Read the file content
    let source_code = match fs::read_to_string(&args.file) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading file '{}': {}", args.file.display(), err);
            std::process::exit(1);
        }
    };

    // Create a new parser
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_chi::LANGUAGE.into())
        .expect("Error loading chi grammar");

    // Parse the file content
    let tree = match parser.parse(&source_code, None) {
        Some(tree) => tree,
        None => {
            eprintln!("Error parsing file '{}'", args.file.display());
            std::process::exit(1);
        }
    };

    // Print the syntax tree for debugging
    // println!("AST: {}", tree.root_node().to_sexp());

    // Parse to IR and display it for debugging
    let ast_parser = crate::ast_parser::AstParser::new();
    let program = ast_parser.parse(tree.root_node(), &source_code)?;
    println!("IR: {}", program);

    // Interpret the AST and generate SVG
    let mut interpreter = Interpreter::new();
    interpreter.interpret(tree.root_node(), &source_code)?;

    let svg_content = interpreter.generate_svg(args.debug);

    // Write SVG to output file
    fs::write(&args.output, svg_content)?;
    println!("SVG generated: {}", args.output.display());

    Ok(())
}
