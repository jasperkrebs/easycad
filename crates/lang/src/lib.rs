//! The `.easycad` recipe language (see `docs/definition-language.md`): recipe text → syntax
//! tree, and new setting values (or the printer) written back into the text. No geometry here.

pub mod ast;
pub mod error;
pub mod expand;
pub mod lexer;
pub mod parser;
pub mod rewrite;

pub use ast::{
    Arg, BinOp, Binding, Component, Condition, Expr, Input, Literal, Output, Param, Part, PartArg,
    PartValue, Port, Program, Span, Unit,
};
pub use error::ParseError;
pub use expand::{Expanded, expand};
pub use parser::{RESERVED, parse};
pub use rewrite::{add_part, format_literal, format_number, remove_part, set_printer, set_values};

#[cfg(test)]
mod tests;
