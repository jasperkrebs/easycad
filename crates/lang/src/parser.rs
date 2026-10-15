//! Recursive-descent parser for `.easycad` recipes. One statement per line; a line that starts
//! with `|>` continues the previous step, and newlines inside brackets are ignored.

use crate::ast::{Arg, BinOp, Binding, Expr, Literal, Output, Param, Program, Span, Unit};
use crate::error::ParseError;
use crate::lexer::{Tok, Token, lex};

/// Words that start statements or modify settings; they can't name a step.
pub const RESERVED: &[&str] = &[
    "easycad", "project", "printer", "note", "group", "param", "show", "export", "print", "true",
    "false", "in", "of", "step", "as", "with",
];

/// Parses a whole recipe.
pub fn parse(src: &str) -> Result<Program, ParseError> {
    let tokens = lex(src)?;
    Parser {
        src,
        tokens,
        pos: 0,
        group: None,
        nesting: 0,
    }
    .program()
}

struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    group: Option<String>,
    /// How many expressions are open right now (see [`MAX_DEPTH`]).
    nesting: u32,
}

type R<T> = Result<T, ParseError>;

impl Parser<'_> {
    // ----------------------------------------------------------------- token helpers

    fn peek(&self) -> &Tok {
        self.tokens.get(self.pos).map_or(&Tok::Eof, |t| &t.tok)
    }

    fn peek_at(&self, offset: usize) -> &Tok {
        self.tokens
            .get(self.pos + offset)
            .map_or(&Tok::Eof, |t| &t.tok)
    }

    fn token(&self) -> Option<&Token> {
        self.tokens.get(self.pos).or_else(|| self.tokens.last())
    }

    fn span(&self) -> Span {
        self.token().map_or(Span::default(), |t| t.span)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == tok {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: &Tok, what: &str) -> R<Token> {
        if self.peek() == tok {
            self.bump()
                .ok_or_else(|| self.error(format!("expected {what}")))
        } else {
            Err(self.error(format!("expected {what}, found {}", self.peek().describe())))
        }
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        ParseError::at(self.span(), message)
    }

    fn ident(&mut self, what: &str) -> R<(String, Span)> {
        let span = self.span();
        match self.peek().clone() {
            Tok::Ident(name) => {
                self.pos += 1;
                Ok((name, span))
            }
            other => Err(self.error(format!("expected {what}, found {}", other.describe()))),
        }
    }

    fn text(&mut self, what: &str) -> R<String> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.pos += 1;
                Ok(s)
            }
            other => Err(self.error(format!(
                "expected {what} in quotes, found {}",
                other.describe()
            ))),
        }
    }

    fn end_of_statement(&mut self) -> R<()> {
        match self.peek() {
            Tok::Newline => {
                self.pos += 1;
                Ok(())
            }
            Tok::Eof => Ok(()),
            other => Err(self.error(format!(
                "expected the end of the line, found {}",
                other.describe()
            ))),
        }
    }

    // ----------------------------------------------------------------- statements

    fn program(mut self) -> R<Program> {
        let mut program = Program::default();
        loop {
            while self.eat(&Tok::Newline) {}
            let span = self.span();
            let Tok::Ident(word) = self.peek().clone() else {
                if *self.peek() == Tok::Eof {
                    return Ok(program);
                }
                return Err(self.error(format!(
                    "expected a step like `name = …` or a setting `param …`, found {}",
                    self.peek().describe()
                )));
            };
            if *self.peek_at(1) == Tok::Eq {
                program.bindings.push(self.binding()?);
            } else {
                self.pos += 1;
                match word.as_str() {
                    "easycad" => {
                        let Some(Token {
                            tok: Tok::Number(_),
                            start,
                            end,
                            ..
                        }) = self.token().cloned()
                        else {
                            return Err(self.error(format!(
                                "expected a version like `easycad 0.1`, found {}",
                                self.peek().describe()
                            )));
                        };
                        self.pos += 1;
                        let text = self.src.get(start..end).unwrap_or_default();
                        program.version = Some(text.to_string());
                    }
                    "project" => program.project = Some(self.text("the project name")?),
                    "printer" => program.printer = Some(self.text("the printer name")?),
                    "note" => program.notes.push(self.text("the note")?),
                    "group" => self.group = Some(self.text("the group name")?),
                    "param" => program.params.push(self.param(span)?),
                    "show" => {
                        let mut names = vec![self.ident("a step name to show")?];
                        while self.eat(&Tok::Comma) {
                            names.push(self.ident("a step name to show")?);
                        }
                        program.outputs.push(Output::Show { names, span });
                    }
                    "export" => {
                        let expr = self.expr()?;
                        match self.peek() {
                            Tok::Ident(w) if w == "as" => self.pos += 1,
                            other => {
                                return Err(self.error(format!(
                                    "expected `as \"file name\"`, found {}",
                                    other.describe()
                                )));
                            }
                        }
                        let file = self.text("the file name")?;
                        program.outputs.push(Output::Export { expr, file, span });
                    }
                    "print" => {
                        let expr = self.expr()?;
                        let printer = match self.peek() {
                            Tok::Ident(w) if w == "with" => {
                                self.pos += 1;
                                Some(self.expr()?)
                            }
                            _ => None,
                        };
                        program.outputs.push(Output::Print {
                            expr,
                            printer,
                            span,
                        });
                    }
                    other => {
                        return Err(ParseError::at(
                            span,
                            format!(
                                "`{other}` isn't a statement; to define a step write `{other} = …`"
                            ),
                        ));
                    }
                }
            }
            self.end_of_statement()?;
        }
    }

    fn binding(&mut self) -> R<Binding> {
        let (name, span) = self.ident("a step name")?;
        if RESERVED.contains(&name.as_str()) {
            return Err(ParseError::at(
                span,
                format!("`{name}` is a reserved word; pick another name"),
            ));
        }
        self.expect(&Tok::Eq, "`=`")?;
        let expr = self.expr()?;
        Ok(Binding { name, expr, span })
    }

    fn param(&mut self, span: Span) -> R<Param> {
        let (name, name_span) = self.ident("the setting's name")?;
        if RESERVED.contains(&name.as_str()) {
            return Err(ParseError::at(
                name_span,
                format!("`{name}` is a reserved word; pick another name"),
            ));
        }
        self.expect(&Tok::Eq, "`=` and a value")?;

        let value_start = self.token().map_or(0, |t| t.start);
        let value = match self.peek().clone() {
            Tok::Minus => {
                self.pos += 1;
                match self.peek().clone() {
                    Tok::Number(n) => {
                        self.pos += 1;
                        Literal::Number(-n)
                    }
                    other => {
                        return Err(self.error(format!(
                            "expected a number after `-`, found {}",
                            other.describe()
                        )));
                    }
                }
            }
            Tok::Number(n) => {
                self.pos += 1;
                Literal::Number(n)
            }
            Tok::Str(s) => {
                self.pos += 1;
                Literal::Text(s)
            }
            Tok::Ident(w) if w == "true" || w == "false" => {
                self.pos += 1;
                Literal::Bool(w == "true")
            }
            other => {
                return Err(self.error(format!(
                    "a setting's value must be a number, true/false or text, found {}",
                    other.describe()
                )));
            }
        };
        let value_end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(value_start, |t| t.end);
        let unit = self.unit();

        let mut param = Param {
            name,
            value,
            unit,
            range: None,
            step: None,
            choices: None,
            label: None,
            group: self.group.clone(),
            attrs: Vec::new(),
            span,
            value_range: (value_start, value_end),
        };
        loop {
            match self.peek().clone() {
                Tok::Newline | Tok::Eof => break,
                Tok::Ident(w) if w == "in" => {
                    let span = self.span();
                    self.pos += 1;
                    let lo = self.signed_number("the range start")?;
                    self.expect(&Tok::DotDot, "`..` between the range ends")?;
                    let hi = self.signed_number("the range end")?;
                    if lo > hi {
                        return Err(ParseError::at(
                            span,
                            format!("the range {lo}..{hi} runs backwards"),
                        ));
                    }
                    param.range = Some((lo, hi));
                }
                Tok::Ident(w) if w == "step" => {
                    self.pos += 1;
                    param.step = Some(self.signed_number("the step size")?);
                }
                Tok::Ident(w) if w == "of" => {
                    self.pos += 1;
                    param.choices = Some(self.choices()?);
                }
                Tok::Str(label) => {
                    self.pos += 1;
                    param.label = Some(label);
                }
                Tok::Attr(attr) => {
                    self.pos += 1;
                    param.attrs.push(attr);
                }
                other => {
                    return Err(self.error(format!(
                        "unexpected {} in this setting (expected `in a..b`, `step s`, `of [...]`, a \"Label\" or @attribute)",
                        other.describe()
                    )));
                }
            }
        }
        Ok(param)
    }

    fn unit(&mut self) -> Unit {
        match self.peek().clone() {
            Tok::Percent => {
                self.pos += 1;
                Unit::Percent
            }
            Tok::Ident(w) => match Unit::from_word(&w) {
                Some(u) => {
                    self.pos += 1;
                    u
                }
                None => Unit::None,
            },
            _ => Unit::None,
        }
    }

    fn signed_number(&mut self, what: &str) -> R<f64> {
        let negative = self.eat(&Tok::Minus);
        match self.peek().clone() {
            Tok::Number(n) => {
                self.pos += 1;
                Ok(if negative { -n } else { n })
            }
            other => Err(self.error(format!("expected {what}, found {}", other.describe()))),
        }
    }

    fn choices(&mut self) -> R<Vec<Literal>> {
        self.expect(&Tok::LBracket, "`[` to start the list of choices")?;
        let mut out = Vec::new();
        while *self.peek() != Tok::RBracket {
            out.push(match self.peek().clone() {
                Tok::Str(s) => Literal::Text(s),
                Tok::Ident(w) => Literal::Text(w),
                Tok::Number(n) => Literal::Number(n),
                other => return Err(self.error(format!("expected a choice, found {}", other.describe()))),
            });
            self.pos += 1;
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RBracket, "`]` to end the list of choices")?;
        Ok(out)
    }

    // ----------------------------------------------------------------- expressions

    fn expr(&mut self) -> R<Expr> {
        Ok(self.pipeline()?.expr)
    }

    /// `a |> f(x) |> g` is `g(f(a, x))`. A pipe may start the next line: `wall\n  |> cap()`.
    fn pipeline(&mut self) -> R<Node> {
        let mut lhs = self.additive()?;
        loop {
            let mut ahead = 0;
            while *self.peek_at(ahead) == Tok::Newline {
                ahead += 1;
            }
            if *self.peek_at(ahead) != Tok::Pipe {
                return Ok(lhs);
            }
            self.pos += ahead + 1;
            let (name, span) = self.ident("a step after `|>`")?;
            let mut depth = lhs.depth;
            let mut args = vec![Arg {
                name: None,
                value: lhs.expr,
            }];
            if *self.peek() == Tok::LParen {
                let (more, more_depth) = self.call_args()?;
                args.extend(more);
                depth = depth.max(more_depth);
            }
            lhs = node(Expr::Call { name, args, span }, depth + 1)?;
        }
    }

    fn additive(&mut self) -> R<Node> {
        let mut lhs = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            let span = self.span();
            self.pos += 1;
            let rhs = self.multiplicative()?;
            lhs = binary(op, lhs, rhs, span)?;
        }
    }

    fn multiplicative(&mut self) -> R<Node> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                _ => return Ok(lhs),
            };
            let span = self.span();
            self.pos += 1;
            let rhs = self.unary()?;
            lhs = binary(op, lhs, rhs, span)?;
        }
    }

    /// Minus binds looser than `^`, as in maths: `-t^2` is `-(t^2)`. Every nested expression
    /// passes through here, so this is also where runaway nesting is stopped.
    fn unary(&mut self) -> R<Node> {
        if self.nesting >= MAX_DEPTH {
            return Err(self.error("this expression is nested too deeply"));
        }
        self.nesting += 1;
        let out = if *self.peek() == Tok::Minus {
            let span = self.span();
            self.pos += 1;
            self.unary().and_then(|inner| {
                node(
                    Expr::Neg {
                        expr: Box::new(inner.expr),
                        span,
                    },
                    inner.depth + 1,
                )
            })
        } else {
            self.power()
        };
        self.nesting -= 1;
        out
    }

    /// `^` groups to the right: `2^3^2` is `2^(3^2)`.
    fn power(&mut self) -> R<Node> {
        let base = self.primary()?;
        if *self.peek() != Tok::Caret {
            return Ok(base);
        }
        let span = self.span();
        self.pos += 1;
        let exponent = self.unary()?;
        binary(BinOp::Pow, base, exponent, span)
    }

    fn primary(&mut self) -> R<Node> {
        let span = self.span();
        let leaf = |expr| Ok(Node { expr, depth: 1 });
        match self.peek().clone() {
            Tok::Number(n) => {
                self.pos += 1;
                let unit = self.unit();
                leaf(Expr::Number {
                    value: unit.to_base(n),
                    span,
                })
            }
            Tok::Str(s) => {
                self.pos += 1;
                leaf(Expr::Text { value: s, span })
            }
            Tok::Ident(w) if w == "true" || w == "false" => {
                self.pos += 1;
                leaf(Expr::Bool {
                    value: w == "true",
                    span,
                })
            }
            Tok::Ident(name) => {
                self.pos += 1;
                if *self.peek() == Tok::LParen {
                    let (args, depth) = self.call_args()?;
                    node(Expr::Call { name, args, span }, depth + 1)
                } else {
                    leaf(Expr::Ident { name, span })
                }
            }
            Tok::LBracket => {
                self.pos += 1;
                let mut items = Vec::new();
                let mut depth = 0;
                while *self.peek() != Tok::RBracket {
                    let item = self.pipeline()?;
                    depth = depth.max(item.depth);
                    items.push(item.expr);
                    if !self.eat(&Tok::Comma) {
                        break;
                    }
                }
                self.expect(&Tok::RBracket, "`]` to close the list")?;
                node(Expr::List { items, span }, depth + 1)
            }
            Tok::LParen => {
                self.pos += 1;
                let inner = self.pipeline()?;
                self.expect(&Tok::RParen, "`)`")?;
                Ok(inner)
            }
            other => Err(self.error(format!("expected a value, found {}", other.describe()))),
        }
    }

    /// `(a, b, name: c)`: the arguments and the depth of the deepest one.
    fn call_args(&mut self) -> R<(Vec<Arg>, u32)> {
        self.expect(&Tok::LParen, "`(`")?;
        let mut args = Vec::new();
        let mut depth = 0;
        let mut named_seen = false;
        while *self.peek() != Tok::RParen {
            let name = if matches!(self.peek(), Tok::Ident(_)) && *self.peek_at(1) == Tok::Colon {
                let (name, _) = self.ident("an argument name")?;
                self.pos += 1; // the colon
                named_seen = true;
                Some(name)
            } else if named_seen {
                return Err(
                    self.error("unnamed values must come before named ones (`name: value`)")
                );
            } else {
                None
            };
            let value = self.pipeline()?;
            depth = depth.max(value.depth);
            args.push(Arg {
                name,
                value: value.expr,
            });
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.expect(&Tok::RParen, "`)` to close the call")?;
        Ok((args, depth))
    }
}

/// The deepest an expression may nest (brackets, operators, pipes). Real recipes stay far
/// below this; the limit keeps hostile input from overflowing the stack.
const MAX_DEPTH: u32 = 128;

/// An expression plus the depth of its tree, so no expression can grow deeper than
/// [`MAX_DEPTH`] (evaluating and dropping trees is recursive).
struct Node {
    expr: Expr,
    depth: u32,
}

fn node(expr: Expr, depth: u32) -> R<Node> {
    if depth > MAX_DEPTH {
        return Err(ParseError::at(
            expr.span(),
            "this expression is too long or nested too deeply; split it into named steps",
        ));
    }
    Ok(Node { expr, depth })
}

fn binary(op: BinOp, lhs: Node, rhs: Node, span: Span) -> R<Node> {
    let depth = lhs.depth.max(rhs.depth) + 1;
    node(
        Expr::Binary {
            op,
            lhs: Box::new(lhs.expr),
            rhs: Box::new(rhs.expr),
            span,
        },
        depth,
    )
}
