//! The recipe as the graph view sees it. Every statement keeps its place in the text: each
//! step's pipe stages and every argument's text, as byte ranges. Graph edits ([`crate::edit`])
//! use them to change exactly one piece of the text, so comments and layout everywhere else
//! stay as they were.

use crate::ast::Unit;
use crate::error::ParseError;
use crate::lexer::{Tok, Token, lex};
use crate::parser::parse;
use serde::Serialize;

/// A byte range in the source: `(start, end)`.
pub type Range = (usize, usize);

/// Where a statement sits: its first and last line (1-based), and the byte range of those
/// whole lines, through the line break at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lines {
    pub line: u32,
    pub end_line: u32,
    pub range: Range,
}

/// A setting (`param …`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamLine {
    pub name: String,
    pub name_range: Range,
    pub lines: Lines,
    /// From `param` to the statement's last token (a comment after it isn't included).
    pub range: Range,
}

/// A named step and its pipe stages: `a |> f(x) |> g` has three.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepLine {
    pub name: String,
    pub name_range: Range,
    pub lines: Lines,
    /// From the name to the statement's last token.
    pub range: Range,
    /// The whole expression after `=`.
    pub expr: Range,
    pub stages: Vec<Stage>,
}

/// One pipe stage: a call (`f(x)`, or `f` after `|>`), or, as the first stage, any expression
/// (`a + b`, `[1, 2]`, a name).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    /// The step it calls, if it's a call.
    pub call: Option<String>,
    pub range: Range,
    /// Where the called name is written.
    pub name_range: Option<Range>,
    /// Whether it gets the stage before it as its first input (all stages but the first).
    pub piped: bool,
    pub args: Vec<ArgText>,
    /// Where the closing `)` is; `None` for a call written without brackets (`|> smooth`).
    pub close: Option<usize>,
    /// The names a plain expression reads (a call's are on its arguments).
    pub refs: Vec<String>,
}

/// One argument of a call.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgText {
    /// The name of a named argument (`width: 0.6`).
    pub name: Option<String>,
    /// The value's text.
    pub value: Range,
    /// The whole argument, `name: ` included.
    pub full: Range,
    /// The names the value reads: settings, steps, or built-ins like `x`.
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind {
    Show,
    Print,
    Export,
}

/// A `show`, `print` or `export` statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputLine {
    pub kind: OutputKind,
    pub lines: Lines,
    /// `show`: the names, each with where it's written.
    pub names: Vec<(String, Range)>,
    /// `print` and `export`: the expression.
    pub expr: Option<Range>,
    pub refs: Vec<String>,
    /// `export`: the file name.
    pub file: Option<String>,
}

/// Every statement of a recipe with its place in the text.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outline {
    pub params: Vec<ParamLine>,
    pub steps: Vec<StepLine>,
    pub outputs: Vec<OutputLine>,
    /// The end of the last header line (`easycad`, `project`, `printer`, `note`), or 0.
    pub header_end: usize,
}

impl Outline {
    pub fn step(&self, name: &str) -> Option<&StepLine> {
        self.steps.iter().find(|s| s.name == name)
    }

    pub fn param(&self, name: &str) -> Option<&ParamLine> {
        self.params.iter().find(|p| p.name == name)
    }
}

/// Outlines a recipe. Only a recipe that parses has an outline; otherwise this is the parse
/// error.
pub fn outline(src: &str) -> Result<Outline, ParseError> {
    parse(src)?;
    let tokens = lex(src)?;
    let mut out = Outline::default();
    let mut i = 0;
    while let Some(t) = tokens.get(i) {
        match t.tok {
            Tok::Newline => {
                i += 1;
                continue;
            }
            Tok::Eof => break,
            _ => {}
        }
        let end = statement_end(&tokens, i);
        let body: Vec<&Token> = tokens
            .get(i..end)
            .unwrap_or_default()
            .iter()
            .filter(|t| t.tok != Tok::Newline)
            .collect();
        let lines = lines_of(src, &body, tokens.get(end));
        statement(&mut out, &body, lines);
        i = end + 1;
    }
    Ok(out)
}

/// The index of the line break (or end of file) that ends the statement starting at `i`. A
/// line starting with `|>` continues the statement above it.
fn statement_end(tokens: &[Token], i: usize) -> usize {
    let mut k = i;
    loop {
        match tokens.get(k).map(|t| &t.tok) {
            None | Some(Tok::Eof) => return k,
            Some(Tok::Newline) => {
                let mut j = k + 1;
                while tokens.get(j).is_some_and(|t| t.tok == Tok::Newline) {
                    j += 1;
                }
                if tokens.get(j).is_some_and(|t| t.tok == Tok::Pipe) {
                    k = j;
                } else {
                    return k;
                }
            }
            Some(_) => k += 1,
        }
    }
}

fn lines_of(src: &str, body: &[&Token], terminator: Option<&Token>) -> Lines {
    let (first, last) = (body.first(), body.last());
    let start = first.map_or(0, |t| line_start(src, t.start));
    let end = match terminator {
        Some(t) if t.tok == Tok::Newline => t.end,
        _ => src.len(),
    };
    Lines {
        line: first.map_or(1, |t| t.span.line),
        end_line: last.map_or(1, |t| t.span.line),
        range: (start, end),
    }
}

/// The byte offset where the line holding `at` starts.
pub fn line_start(src: &str, at: usize) -> usize {
    src.get(..at)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |p| p + 1)
}

fn ident<'a>(t: Option<&&'a Token>) -> Option<&'a str> {
    match t.map(|t| &t.tok) {
        Some(Tok::Ident(name)) => Some(name),
        _ => None,
    }
}

fn statement(out: &mut Outline, body: &[&Token], lines: Lines) {
    let Some(&first) = body.first() else {
        return;
    };
    let word = ident(body.first()).unwrap_or_default();
    if body.get(1).is_some_and(|t| t.tok == Tok::Eq) {
        let expr = body.get(2..).unwrap_or_default();
        out.steps.push(StepLine {
            name: word.to_string(),
            name_range: (first.start, first.end),
            lines,
            range: range_of(body),
            expr: range_of(expr),
            stages: stages(expr),
        });
        return;
    }
    let rest = body.get(1..).unwrap_or_default();
    match word {
        "param" => {
            if let (Some(name), Some(t)) = (ident(body.get(1)), body.get(1)) {
                out.params.push(ParamLine {
                    name: name.to_string(),
                    name_range: (t.start, t.end),
                    lines,
                    range: range_of(body),
                });
            }
        }
        "show" => out.outputs.push(OutputLine {
            kind: OutputKind::Show,
            lines,
            names: rest
                .iter()
                .filter_map(|t| match &t.tok {
                    Tok::Ident(n) => Some((n.clone(), (t.start, t.end))),
                    _ => None,
                })
                .collect(),
            expr: None,
            refs: Vec::new(),
            file: None,
        }),
        "print" | "export" => {
            let stop = if word == "print" { "with" } else { "as" };
            let expr = until_word(rest, stop);
            let file = rest.iter().rev().find_map(|t| match &t.tok {
                Tok::Str(s) if word == "export" => Some(s.clone()),
                _ => None,
            });
            out.outputs.push(OutputLine {
                kind: if word == "print" {
                    OutputKind::Print
                } else {
                    OutputKind::Export
                },
                lines,
                names: Vec::new(),
                expr: Some(range_of(expr)),
                refs: refs(expr),
                file,
            });
        }
        "easycad" | "project" | "printer" | "note" => {
            out.header_end = out.header_end.max(lines.range.1);
        }
        _ => {}
    }
}

/// The tokens before `word` (outside brackets).
fn until_word<'a, 'b>(toks: &'b [&'a Token], word: &str) -> &'b [&'a Token] {
    let mut depth = 0i32;
    for (k, t) in toks.iter().enumerate() {
        match &t.tok {
            Tok::LParen | Tok::LBracket => depth += 1,
            Tok::RParen | Tok::RBracket => depth -= 1,
            Tok::Ident(w) if depth == 0 && w == word => return toks.get(..k).unwrap_or_default(),
            _ => {}
        }
    }
    toks
}

fn range_of(toks: &[&Token]) -> Range {
    match (toks.first(), toks.last()) {
        (Some(a), Some(b)) => (a.start, b.end),
        _ => (0, 0),
    }
}

/// Splits tokens at `sep` outside brackets.
fn split<'a>(toks: &[&'a Token], sep: &Tok) -> Vec<Vec<&'a Token>> {
    let mut parts = vec![Vec::new()];
    let mut depth = 0i32;
    for &t in toks {
        match &t.tok {
            Tok::LParen | Tok::LBracket => depth += 1,
            Tok::RParen | Tok::RBracket => depth -= 1,
            tok if depth == 0 && tok == sep => {
                parts.push(Vec::new());
                continue;
            }
            _ => {}
        }
        if let Some(part) = parts.last_mut() {
            part.push(t);
        }
    }
    parts.retain(|p| !p.is_empty());
    parts
}

fn stages(expr: &[&Token]) -> Vec<Stage> {
    split(expr, &Tok::Pipe)
        .iter()
        .enumerate()
        .map(|(k, part)| stage(part, k > 0))
        .collect()
}

/// Where the bracket opened at `open` closes.
fn matching(toks: &[&Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, t) in toks.iter().enumerate().skip(open) {
        match t.tok {
            Tok::LParen | Tok::LBracket => depth += 1,
            Tok::RParen | Tok::RBracket => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

fn stage(part: &[&Token], piped: bool) -> Stage {
    let range = range_of(part);
    let name = ident(part.first()).filter(|n| *n != "true" && *n != "false");
    let head = part.first().map(|t| (t.start, t.end));
    let opens = part.get(1).is_some_and(|t| t.tok == Tok::LParen);
    let last = part.len().saturating_sub(1);
    match name {
        Some(name) if opens && matching(part, 1) == Some(last) => Stage {
            call: Some(name.to_string()),
            range,
            name_range: head,
            piped,
            args: args(part.get(2..last).unwrap_or_default()),
            close: part.get(last).map(|t| t.start),
            refs: Vec::new(),
        },
        Some(name) if piped && part.len() == 1 => Stage {
            call: Some(name.to_string()),
            range,
            name_range: head,
            piped,
            args: Vec::new(),
            close: None,
            refs: Vec::new(),
        },
        _ => Stage {
            call: None,
            range,
            name_range: None,
            piped,
            args: Vec::new(),
            close: None,
            refs: refs(part),
        },
    }
}

fn args(toks: &[&Token]) -> Vec<ArgText> {
    split(toks, &Tok::Comma)
        .iter()
        .map(|group| {
            let named =
                ident(group.first()).is_some() && group.get(1).is_some_and(|t| t.tok == Tok::Colon);
            let value = if named {
                group.get(2..).unwrap_or_default()
            } else {
                group.as_slice()
            };
            ArgText {
                name: if named {
                    ident(group.first()).map(str::to_string)
                } else {
                    None
                },
                value: range_of(value),
                full: range_of(group),
                refs: refs(value),
            }
        })
        .collect()
}

/// The names an expression reads: every name except called steps, argument names, the step
/// after a `|>`, units after numbers, and `true`/`false`.
pub(crate) fn refs(toks: &[&Token]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (k, t) in toks.iter().enumerate() {
        let Tok::Ident(name) = &t.tok else {
            continue;
        };
        let next = toks.get(k + 1).map(|t| &t.tok);
        let prev = k.checked_sub(1).and_then(|p| toks.get(p)).map(|t| &t.tok);
        let skip = name == "true"
            || name == "false"
            || matches!(next, Some(Tok::LParen | Tok::Colon))
            || matches!(prev, Some(Tok::Pipe))
            || (matches!(prev, Some(Tok::Number(_))) && Unit::from_word(name).is_some());
        if !skip && !out.contains(name) {
            out.push(name.clone());
        }
    }
    out
}
