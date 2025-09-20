//! Main Interpreter - Coordinates the two-stage process

use crate::ast_parser::AstParser;
use crate::evaluator::{ControlPoint, Evaluator};
use tree_sitter::Node;

pub struct Interpreter {
    ast_parser: AstParser,
    evaluator: Evaluator,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            ast_parser: AstParser::new(),
            evaluator: Evaluator::new(),
        }
    }

    pub fn interpret(
        &mut self,
        node: Node,
        source: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Stage 1: Parse AST to IR
        let program = self.ast_parser.parse(node, source)?;

        // Stage 2: Evaluate IR with full context
        self.evaluator.evaluate(program)?;

        Ok(())
    }

    pub fn generate_svg(&self, debug: bool) -> String {
        self.evaluator.generate_svg(debug)
    }
}
