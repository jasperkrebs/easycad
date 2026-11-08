//! The syntax tree of an `.easycad` recipe (see `docs/definition-language.md`).

use serde::{Deserialize, Serialize};

/// A position in the source, 1-based, for error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

/// A whole recipe file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    /// From the `easycad 0.1` header line.
    pub version: Option<String>,
    /// `project "…"`: the name shown in the window title and the project list.
    pub project: Option<String>,
    /// `printer "…"`: the default printer profile.
    pub printer: Option<String>,
    /// `note "…"` lines, in order.
    pub notes: Vec<String>,
    /// The settings, in file order. They become the project's settings window.
    pub params: Vec<Param>,
    /// Named steps (`name = expression`), in file order. Each one is a node.
    pub bindings: Vec<Binding>,
    /// `show`, `export` and `print` statements.
    pub outputs: Vec<Output>,
    /// `input name = expression "Label"` (only inside a component): a setting whose value is
    /// another part's port, or any expression.
    pub inputs: Vec<Input>,
    /// `port name = expression "Label"` (only inside a component): a place another part can be
    /// mounted.
    pub ports: Vec<Port>,
    /// `component name "Label" … end`: recipes within the recipe, made into parts.
    pub components: Vec<Component>,
    /// `part name = component(setting: value, …)`, in file order.
    pub parts: Vec<Part>,
}

/// `component name "Label"` … `end`: settings, inputs, steps, ports and outputs that every part
/// made from it has its own copy of (see `docs/lamp-builder.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub label: Option<String>,
    /// Its own settings, inputs, steps, ports and outputs (no components or parts inside).
    pub body: Program,
    pub span: Span,
}

/// `input name = expression "Label"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub name: String,
    pub default: Expr,
    pub label: Option<String>,
    pub span: Span,
}

/// `port name = expression "Label"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    pub name: String,
    pub expr: Expr,
    pub label: Option<String>,
    pub span: Span,
}

/// `part name = component(setting: value, input: expression, …)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub component: String,
    pub args: Vec<PartArg>,
    pub span: Span,
    /// Byte range of the whole statement, to the end of its last line (newline included).
    pub range: (usize, usize),
    /// Byte offset of the closing `)`, where a new argument goes.
    pub close: usize,
}

/// One `name: value` of a part.
#[derive(Debug, Clone, PartialEq)]
pub struct PartArg {
    pub name: String,
    pub span: Span,
    pub value: PartValue,
    /// Byte range of the value as written (a number with its unit).
    pub range: (usize, usize),
}

/// A part's argument: a setting's value as written, or an expression (for an input).
#[derive(Debug, Clone, PartialEq)]
pub enum PartValue {
    Literal { value: Literal, unit: Unit },
    Expr(Expr),
}

/// A literal setting value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Literal {
    Number(f64),
    Bool(bool),
    Text(String),
    /// A list setting: numbers, or rows of numbers (`[[0, 30], [0.5, 36]]`), edited as a table.
    List(Vec<Literal>),
}

/// A unit written after a number. Values are stored in the unit as written; [`Unit::to_base`]
/// converts to the engine's base units (mm, degrees, plain ratios).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    #[default]
    None,
    Mm,
    Cm,
    M,
    Inch,
    Deg,
    Rad,
    Percent,
}

impl Unit {
    /// Parses a unit word as written in a recipe.
    pub fn from_word(word: &str) -> Option<Unit> {
        Some(match word {
            "mm" => Unit::Mm,
            "cm" => Unit::Cm,
            "m" => Unit::M,
            "inch" => Unit::Inch,
            "deg" => Unit::Deg,
            "rad" => Unit::Rad,
            "%" => Unit::Percent,
            _ => return None,
        })
    }

    /// The unit as written in a recipe (empty for no unit).
    pub fn word(self) -> &'static str {
        match self {
            Unit::None => "",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::M => "m",
            Unit::Inch => "inch",
            Unit::Deg => "deg",
            Unit::Rad => "rad",
            Unit::Percent => "%",
        }
    }

    /// Converts a value in this unit to the engine's base unit: lengths to mm, angles to
    /// degrees, percentages to plain ratios.
    pub fn to_base(self, value: f64) -> f64 {
        match self {
            Unit::None | Unit::Mm | Unit::Deg => value,
            Unit::Cm => value * 10.0,
            Unit::M => value * 1000.0,
            Unit::Inch => value * 25.4,
            Unit::Rad => value.to_degrees(),
            Unit::Percent => value / 100.0,
        }
    }
}

/// A setting: `param name = value unit in min..max step s "Label" @attr`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    /// The value as written (in `unit`).
    pub value: Literal,
    pub unit: Unit,
    pub range: Option<(f64, f64)>,
    pub step: Option<f64>,
    /// `of [...]`: a fixed list of choices (shown as a dropdown).
    pub choices: Option<Vec<Literal>>,
    pub label: Option<String>,
    /// The `group "…"` this setting appears under.
    pub group: Option<String>,
    /// `@assumed`, `@locked`, … without the `@`.
    pub attrs: Vec<String>,
    /// `when …`: the setting only applies (and shows) while another setting has a value.
    pub when: Option<Condition>,
    pub span: Span,
    /// Byte range of the value literal in the source, so a new value can be written back
    /// without touching anything else on the line.
    pub value_range: (usize, usize),
    /// The part this setting belongs to (after expanding parts: `tube.shape` is `tube`'s).
    pub part: Option<String>,
}

/// When a setting applies: `when legs` (while `legs` is on, or not 0 or empty), `when shape =
/// "own"`, `when shape = ["sphere", "flat sphere"]` (one of these), each with `not` to turn it
/// round (`when not zipper`, `when not compensation = "off"`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Condition {
    /// The setting it depends on.
    pub name: String,
    /// The values that make it apply; empty: any value that's on (true, not 0, not empty).
    pub values: Vec<Literal>,
    pub not: bool,
}

/// A named step: `name = expression`.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

/// `show a, b`, `export x as "file"`, `print x with printer`.
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Show {
        names: Vec<(String, Span)>,
        span: Span,
    },
    Export {
        expr: Expr,
        file: String,
        span: Span,
    },
    Print {
        expr: Expr,
        printer: Option<Expr>,
        span: Span,
        /// The part it prints (after expanding parts): the G-code file's name.
        part: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// An expression. Pipes are already desugared: `a |> f(x)` is the call `f(a, x)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A number, already converted to base units.
    Number {
        value: f64,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Text {
        value: String,
        span: Span,
    },
    Ident {
        name: String,
        span: Span,
    },
    List {
        items: Vec<Expr>,
        span: Span,
    },
    Call {
        name: String,
        args: Vec<Arg>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Neg {
        expr: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Text { span, .. }
            | Expr::Ident { span, .. }
            | Expr::List { span, .. }
            | Expr::Call { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Neg { span, .. } => *span,
        }
    }

    /// Every name this expression reads (params or other steps), for dependency ordering.
    pub fn references(&self, out: &mut Vec<(String, Span)>) {
        match self {
            Expr::Ident { name, span } => out.push((name.clone(), *span)),
            Expr::List { items, .. } => items.iter().for_each(|e| e.references(out)),
            Expr::Call { args, .. } => args.iter().for_each(|a| a.value.references(out)),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.references(out);
                rhs.references(out);
            }
            Expr::Neg { expr, .. } => expr.references(out),
            Expr::Number { .. } | Expr::Bool { .. } | Expr::Text { .. } => {}
        }
    }
}

/// A call argument, positional (`name: None`) or named.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
}
