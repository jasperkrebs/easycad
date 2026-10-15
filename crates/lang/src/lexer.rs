//! Turns recipe text into tokens. Newlines matter (one statement per line), except inside
//! brackets, where a long call can wrap freely.

use crate::ast::Span;
use crate::error::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Number(f64),
    Ident(String),
    Str(String),
    /// `@assumed` → `Attr("assumed")`
    Attr(String),
    Percent,
    Eq,
    Comma,
    Colon,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    DotDot,
    Pipe,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Newline,
    Eof,
}

impl Tok {
    /// How the token reads in an error message.
    pub fn describe(&self) -> String {
        match self {
            Tok::Number(n) => format!("the number {n}"),
            Tok::Ident(s) => format!("`{s}`"),
            Tok::Str(s) => format!("the text \"{s}\""),
            Tok::Attr(s) => format!("`@{s}`"),
            Tok::Newline => "the end of the line".into(),
            Tok::Eof => "the end of the file".into(),
            other => format!("`{}`", other.symbol()),
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            Tok::Percent => "%",
            Tok::Eq => "=",
            Tok::Comma => ",",
            Tok::Colon => ":",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::DotDot => "..",
            Tok::Pipe => "|>",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Caret => "^",
            _ => "?",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
    /// Byte range in the source.
    pub start: usize,
    pub end: usize,
}

pub fn lex(src: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let mut tokens = Vec::new();
    let (mut line, mut col) = (1u32, 1u32);
    let mut depth: u32 = 0;
    let mut i = 0;

    while let Some(&(start, c)) = chars.get(i) {
        let span = Span { line, col };
        let next = chars.get(i + 1).map(|&(_, c)| c);
        let push = |tok: Tok, len_chars: usize, tokens: &mut Vec<Token>| {
            let end = chars.get(i + len_chars).map_or(src.len(), |&(b, _)| b);
            tokens.push(Token {
                tok,
                span,
                start,
                end,
            });
        };

        match c {
            '\n' => {
                if depth == 0 {
                    push(Tok::Newline, 1, &mut tokens);
                }
                i += 1;
                line += 1;
                col = 1;
                continue;
            }
            // A byte-order mark (some Windows editors add one) is just whitespace.
            ' ' | '\t' | '\r' | '\u{feff}' => {}
            // `90°` reads as `90 deg`.
            '°' => push(Tok::Ident("deg".into()), 1, &mut tokens),
            '#' => {
                while let Some(&(_, c)) = chars.get(i) {
                    if c == '\n' {
                        break;
                    }
                    i += 1;
                    col += 1;
                }
                continue;
            }
            '0'..='9' | '.' if c != '.' || next.is_some_and(|n| n.is_ascii_digit()) => {
                let mut j = i;
                let mut seen_dot = false;
                while let Some(&(_, d)) = chars.get(j) {
                    let after = chars.get(j + 1).map(|&(_, c)| c);
                    if d.is_ascii_digit() {
                        j += 1;
                    } else if d == '.' && !seen_dot && after.is_some_and(|a| a.is_ascii_digit()) {
                        seen_dot = true;
                        j += 1;
                    } else if (d == 'e' || d == 'E')
                        && (after.is_some_and(|a| a.is_ascii_digit())
                            || (matches!(after, Some('+' | '-'))
                                && chars.get(j + 2).is_some_and(|&(_, c)| c.is_ascii_digit())))
                    {
                        j += 2;
                        while chars.get(j).is_some_and(|&(_, c)| c.is_ascii_digit()) {
                            j += 1;
                        }
                        break;
                    } else {
                        break;
                    }
                }
                let end = chars.get(j).map_or(src.len(), |&(b, _)| b);
                let text = src.get(start..end).unwrap_or_default();
                let value: f64 = text
                    .parse()
                    .map_err(|_| ParseError::at(span, format!("`{text}` is not a number")))?;
                tokens.push(Token {
                    tok: Tok::Number(value),
                    span,
                    start,
                    end,
                });
                col += (j - i) as u32;
                i = j;
                continue;
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut j = i;
                while chars
                    .get(j)
                    .is_some_and(|&(_, c)| c.is_alphanumeric() || c == '_')
                {
                    j += 1;
                }
                let end = chars.get(j).map_or(src.len(), |&(b, _)| b);
                let word = src.get(start..end).unwrap_or_default().to_string();
                tokens.push(Token {
                    tok: Tok::Ident(word),
                    span,
                    start,
                    end,
                });
                col += (j - i) as u32;
                i = j;
                continue;
            }
            '@' => {
                let mut j = i + 1;
                while chars
                    .get(j)
                    .is_some_and(|&(_, c)| c.is_alphanumeric() || c == '_')
                {
                    j += 1;
                }
                if j == i + 1 {
                    return Err(ParseError::at(
                        span,
                        "`@` must be followed by a word, like `@assumed`",
                    ));
                }
                let end = chars.get(j).map_or(src.len(), |&(b, _)| b);
                let word = src.get(start + 1..end).unwrap_or_default().to_string();
                tokens.push(Token {
                    tok: Tok::Attr(word),
                    span,
                    start,
                    end,
                });
                col += (j - i) as u32;
                i = j;
                continue;
            }
            '"' => {
                let mut j = i + 1;
                let mut text = String::new();
                loop {
                    match chars.get(j) {
                        None | Some((_, '\n')) => {
                            return Err(ParseError::at(
                                span,
                                "this text is missing its closing `\"`",
                            ));
                        }
                        Some((_, '"')) => break,
                        Some((_, '\\')) => {
                            match chars.get(j + 1) {
                                Some((_, 'n')) => text.push('\n'),
                                Some((_, c)) => text.push(*c),
                                None => {}
                            }
                            j += 2;
                        }
                        Some((_, c)) => {
                            text.push(*c);
                            j += 1;
                        }
                    }
                }
                let end = chars.get(j + 1).map_or(src.len(), |&(b, _)| b);
                tokens.push(Token {
                    tok: Tok::Str(text),
                    span,
                    start,
                    end,
                });
                col += (j + 1 - i) as u32;
                i = j + 1;
                continue;
            }
            '|' if next == Some('>') => {
                push(Tok::Pipe, 2, &mut tokens);
                i += 2;
                col += 2;
                continue;
            }
            '.' if next == Some('.') => {
                push(Tok::DotDot, 2, &mut tokens);
                i += 2;
                col += 2;
                continue;
            }
            '(' | '[' | '{' => {
                depth += 1;
                push(
                    match c {
                        '(' => Tok::LParen,
                        '[' => Tok::LBracket,
                        _ => Tok::LBrace,
                    },
                    1,
                    &mut tokens,
                );
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                push(
                    match c {
                        ')' => Tok::RParen,
                        ']' => Tok::RBracket,
                        _ => Tok::RBrace,
                    },
                    1,
                    &mut tokens,
                );
            }
            '%' => push(Tok::Percent, 1, &mut tokens),
            '=' => push(Tok::Eq, 1, &mut tokens),
            ',' => push(Tok::Comma, 1, &mut tokens),
            ':' => push(Tok::Colon, 1, &mut tokens),
            '+' => push(Tok::Plus, 1, &mut tokens),
            '-' => push(Tok::Minus, 1, &mut tokens),
            '*' => push(Tok::Star, 1, &mut tokens),
            '/' => push(Tok::Slash, 1, &mut tokens),
            '^' => push(Tok::Caret, 1, &mut tokens),
            other => {
                return Err(ParseError::at(
                    span,
                    format!("unexpected character `{other}`"),
                ));
            }
        }
        i += 1;
        col += 1;
    }
    tokens.push(Token {
        tok: Tok::Eof,
        span: Span { line, col },
        start: src.len(),
        end: src.len(),
    });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn numbers_ranges_and_units() {
        assert_eq!(
            kinds("0.3..1.5 18 % 2e3"),
            vec![
                Tok::Number(0.3),
                Tok::DotDot,
                Tok::Number(1.5),
                Tok::Number(18.0),
                Tok::Percent,
                Tok::Number(2000.0),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn newlines_inside_brackets_are_ignored() {
        assert_eq!(
            kinds("f(1,\n 2)\nx"),
            vec![
                Tok::Ident("f".into()),
                Tok::LParen,
                Tok::Number(1.0),
                Tok::Comma,
                Tok::Number(2.0),
                Tok::RParen,
                Tok::Newline,
                Tok::Ident("x".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn comments_strings_and_attributes() {
        assert_eq!(
            kinds("\"Lid \\\"gap\\\"\" @assumed # note\n"),
            vec![
                Tok::Str("Lid \"gap\"".into()),
                Tok::Attr("assumed".into()),
                Tok::Newline,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn degree_sign_and_byte_order_mark() {
        assert_eq!(
            kinds("\u{feff}90°"),
            vec![Tok::Number(90.0), Tok::Ident("deg".into()), Tok::Eof]
        );
    }

    #[test]
    fn errors_have_positions() {
        let err = lex("x = 1\ny = \"open").unwrap_err();
        assert_eq!((err.line, err.col), (2, 5));
        let err = lex("a = $").unwrap_err();
        assert_eq!((err.line, err.col), (1, 5));
    }
}
