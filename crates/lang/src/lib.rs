//! The `.easycad` recipe language (see `docs/definition-language.md`): recipe text → syntax
//! tree, new setting values written back into the text, and the outline and text edits the
//! graph view works with. No geometry here.

pub mod ast;
pub mod edit;
pub mod error;
pub mod lexer;
pub mod outline;
pub mod parser;
pub mod rewrite;

pub use ast::{Arg, BinOp, Binding, Expr, Literal, Output, Param, Program, Span, Unit};
pub use error::ParseError;
pub use outline::{Outline, outline};
pub use parser::{RESERVED, parse};
pub use rewrite::{format_literal, format_number, set_values};

#[cfg(test)]
mod outline_tests;
#[cfg(test)]
mod tests;
