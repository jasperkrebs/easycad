use crate::ast::Span;

/// A syntax error with a 1-based position, phrased so a person (or the agent) can fix it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}, column {col}: {message}")]
pub struct ParseError {
    pub line: u32,
    pub col: u32,
    pub message: String,
}

impl ParseError {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        ParseError {
            line: span.line,
            col: span.col,
            message: message.into(),
        }
    }
}
