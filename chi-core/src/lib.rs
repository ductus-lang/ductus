//! chi-core — parser, evaluator and SVG renderer for the Chi language.
//!
//! Chi is a DSL for parametric type design; see Spec.md in the
//! tree-sitter-chi repository for the language definition.

pub mod ast;
pub mod doc;
pub mod eval;
pub mod lower;
pub mod parse;
pub mod render;
pub mod shapes;
pub mod value;
