//! The `.easycad` recipe language (see `docs/definition-language.md`): recipe text → syntax
//! tree, and new setting values written back into the text. No geometry here.

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod rewrite;

pub use ast::{Arg, BinOp, Binding, Expr, Literal, Output, Param, Program, Span, Unit};
pub use error::ParseError;
pub use parser::{RESERVED, parse};
pub use rewrite::{format_literal, format_number, set_values};

#[cfg(test)]
mod tests;
