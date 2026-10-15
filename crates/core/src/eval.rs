//! Runs a recipe: every named step is evaluated once, in dependency order, and the outputs
//! (`show`, `print`) are collected for the app. Problems become diagnostics on a line; steps
//! that don't depend on a failed step still run, like Grasshopper turning one component red.

use crate::field::Field;
use crate::geom::{Bounds, P3, add3, scale3, sub3};
use crate::nodes::{self, Node};
use crate::surface::Mesh;
use crate::toolpath::{Toolpath, ToolpathStats};
use crate::value::Value;
use easycad_lang::{Arg, BinOp, Expr, Literal, Output, Program, Span, format_number};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

/// Recipes longer than this are refused (real ones are a few kB).
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
/// How many points and list items one evaluation may create in total.
const BUDGET: usize = 4_000_000;
/// The most mesh vertices, curve points and toolpath points drawn at once.
const MAX_DRAWN_POINTS: usize = 2_500_000;
/// The language version this engine reads.
pub const LANGUAGE_VERSION: &str = "0.1";

/// How finely curves are sampled: coarse while dragging a slider, fine on release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Preview,
    Final,
}

/// What a node needs from the evaluation: the quality, and a budget so no recipe can make
/// more geometry than a phone can hold.
pub struct Ctx {
    pub quality: Quality,
    remaining: usize,
    /// Messages from nodes that worked but want to say something (shown on the calling line).
    warnings: Vec<String>,
}

impl Ctx {
    pub fn new(quality: Quality) -> Ctx {
        Ctx {
            quality,
            remaining: BUDGET,
            warnings: Vec::new(),
        }
    }

    pub fn warn(&mut self, message: String) {
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }

    /// `preview` or `fine`, depending on the quality.
    pub fn samples(&self, preview: usize, fine: usize) -> usize {
        match self.quality {
            Quality::Preview => preview,
            Quality::Final => fine,
        }
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }

    pub fn spend(&mut self, n: usize) -> Result<(), String> {
        if n > self.remaining {
            return Err(format!(
                "this recipe makes too much geometry (over {BUDGET} points in total); use fewer sections, fewer points or a larger layer height"
            ));
        }
        self.remaining -= n;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A message about one place in the recipe.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub line: u32,
    pub col: u32,
    pub severity: Severity,
    pub message: String,
}

/// A setting, as the settings window needs it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamInfo {
    pub name: String,
    pub label: String,
    pub group: Option<String>,
    /// `number`, `toggle`, `choice` or `text`.
    pub kind: &'static str,
    /// The value as written, in `unit`.
    pub value: Literal,
    /// As written in the recipe: `mm`, `deg`, `%`, or empty.
    pub unit: &'static str,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub choices: Vec<Literal>,
    pub attrs: Vec<String>,
    pub line: u32,
}

/// One named step and how it went, for the step list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepInfo {
    pub name: String,
    pub line: u32,
    /// `ok`, `error` or `skipped` (a step it needs failed).
    pub status: &'static str,
    pub summary: String,
}

/// Everything to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    /// Shown surfaces and solids, merged.
    pub mesh: Mesh,
    /// Shown curves as polylines (a closed curve ends where it starts).
    pub curves: Vec<Vec<P3>>,
    /// Shown and printed toolpaths.
    pub paths: Vec<Rc<Toolpath>>,
    pub bounds: Bounds,
}

impl Default for Drawing {
    fn default() -> Self {
        Drawing {
            mesh: Mesh::default(),
            curves: Vec::new(),
            paths: Vec::new(),
            bounds: Bounds::empty(),
        }
    }
}

/// The toolpath a `print` statement names.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintInfo {
    pub stats: ToolpathStats,
    /// From `print … with printer`, else the recipe's `printer` line.
    pub printer: Option<String>,
    #[serde(skip)]
    pub path: Rc<Toolpath>,
}

/// What an `export … as "file"` statement asks for.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    /// A triangle mesh of surfaces and solids.
    Stl,
    /// Printer G-code for a toolpath.
    Gcode,
}

/// A file the recipe offers to save.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Export {
    pub file: String,
    pub kind: ExportKind,
    pub line: u32,
    /// What to write: surfaces/solids for STL, a toolpath for G-code.
    #[serde(skip)]
    pub value: Value,
}

/// The result of running a recipe. Always produced, even for broken recipes: then the
/// diagnostics say what's wrong, and whatever still works is drawn.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Evaluation {
    pub version: Option<String>,
    pub project: Option<String>,
    pub printer: Option<String>,
    pub notes: Vec<String>,
    pub params: Vec<ParamInfo>,
    pub steps: Vec<StepInfo>,
    pub diagnostics: Vec<Diagnostic>,
    pub drawing: Drawing,
    pub print: Option<PrintInfo>,
    pub exports: Vec<Export>,
}

impl Evaluation {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }
}

/// Parses and runs a recipe.
pub fn evaluate(source: &str, quality: Quality) -> Evaluation {
    let mut out = Evaluation::default();
    if source.len() > MAX_SOURCE_BYTES {
        out.diagnostics.push(Diagnostic {
            line: 1,
            col: 1,
            severity: Severity::Error,
            message: format!(
                "the recipe is too long ({} kB; the limit is {} kB)",
                source.len() / 1024,
                MAX_SOURCE_BYTES / 1024
            ),
        });
        return out;
    }
    let program = match easycad_lang::parse(source) {
        Ok(p) => p,
        Err(e) => {
            out.diagnostics.push(Diagnostic {
                line: e.line,
                col: e.col,
                severity: Severity::Error,
                message: e.message,
            });
            return out;
        }
    };
    out.version = program.version.clone();
    out.project = program.project.clone();
    out.printer = program.printer.clone();
    out.notes = program.notes.clone();
    if let Some(v) = &program.version
        && v != LANGUAGE_VERSION
    {
        out.diagnostics.push(Diagnostic {
            line: 1,
            col: 1,
            severity: Severity::Warning,
            message: format!(
                "this recipe says `easycad {v}`; this app reads version {LANGUAGE_VERSION}"
            ),
        });
    }
    Runner::new(&program, quality, &mut out).run();
    out.diagnostics.sort_by_key(|d| (d.line, d.col));
    out
}

/// Runs a recipe and returns every step that worked with its value, for tests that look at
/// the values themselves.
#[cfg(test)]
pub(crate) fn step_values(source: &str, quality: Quality) -> (Evaluation, HashMap<String, Value>) {
    let mut out = Evaluation::default();
    let Ok(program) = easycad_lang::parse(source) else {
        return (out, HashMap::new());
    };
    let mut runner = Runner::new(&program, quality, &mut out);
    runner.run();
    let values = program
        .bindings
        .iter()
        .zip(&runner.state)
        .filter_map(|(b, s)| match s {
            State::Done(v) => Some((b.name.clone(), v.clone())),
            _ => None,
        })
        .collect();
    drop(runner);
    (out, values)
}

// ---------------------------------------------------------------------------------------------

/// Names every recipe can use (a setting or step with the same name wins): `pi`, and `x`, `y`,
/// `z`, the coordinates of each point as fields (`move(z: 0.2 * x)` tilts a curve).
fn builtin(name: &str) -> Option<Value> {
    match name {
        "pi" => Some(Value::Number(std::f64::consts::PI)),
        "x" => Some(Value::Field(Rc::new(Field::coordinate(0)))),
        "y" => Some(Value::Field(Rc::new(Field::coordinate(1)))),
        "z" => Some(Value::Field(Rc::new(Field::coordinate(2)))),
        _ => None,
    }
}

/// The built-in names, for "did you mean" suggestions.
const BUILTINS: [&str; 4] = ["pi", "x", "y", "z"];

enum Slot {
    Param(Value),
    Binding(usize),
}

#[derive(Debug, Clone, PartialEq)]
enum State {
    Pending,
    Done(Value),
    Failed,
    Skipped,
    Duplicate,
}

enum Fail {
    Error(Span, String),
    /// A step this needs has failed; its error is already reported.
    Skipped,
}

struct Runner<'a> {
    program: &'a Program,
    out: &'a mut Evaluation,
    ctx: Ctx,
    names: HashMap<&'a str, Slot>,
    state: Vec<State>,
    drawn: usize,
    too_much_drawn: bool,
}

impl<'a> Runner<'a> {
    fn new(program: &'a Program, quality: Quality, out: &'a mut Evaluation) -> Self {
        Runner {
            program,
            out,
            ctx: Ctx::new(quality),
            names: HashMap::new(),
            state: vec![State::Pending; program.bindings.len()],
            drawn: 0,
            too_much_drawn: false,
        }
    }

    fn diag(&mut self, span: Span, severity: Severity, message: impl Into<String>) {
        self.out.diagnostics.push(Diagnostic {
            line: span.line,
            col: span.col,
            severity,
            message: message.into(),
        });
    }

    fn run(&mut self) {
        let program = self.program;
        let mut defined: HashMap<&str, u32> = HashMap::new();

        // Settings.
        for p in &program.params {
            if let Some(line) = defined.get(p.name.as_str()) {
                let msg = format!("`{}` is defined twice (first on line {line})", p.name);
                self.diag(p.span, Severity::Error, msg);
                continue;
            }
            defined.insert(p.name.as_str(), p.span.line);
            let value = match &p.value {
                Literal::Number(n) => Value::Number(p.unit.to_base(*n)),
                Literal::Bool(b) => Value::Bool(*b),
                Literal::Text(t) => Value::Text(t.as_str().into()),
            };
            if let (Literal::Number(n), Some((lo, hi))) = (&p.value, p.range)
                && (*n < lo || *n > hi)
            {
                let msg = format!(
                    "`{}` is {}, outside its range {}..{}",
                    p.name,
                    format_number(*n),
                    format_number(lo),
                    format_number(hi)
                );
                self.diag(p.span, Severity::Warning, msg);
            }
            if let Some(choices) = &p.choices
                && !choices.contains(&p.value)
            {
                self.diag(
                    p.span,
                    Severity::Warning,
                    format!("`{}` is not one of its choices", p.name),
                );
            }
            self.names.insert(p.name.as_str(), Slot::Param(value));
            self.out.params.push(ParamInfo {
                name: p.name.clone(),
                label: p.label.clone().unwrap_or_else(|| humanize(&p.name)),
                group: p.group.clone(),
                kind: match (&p.value, &p.choices) {
                    (_, Some(_)) => "choice",
                    (Literal::Bool(_), _) => "toggle",
                    (Literal::Text(_), _) => "text",
                    (Literal::Number(_), _) => "number",
                },
                value: p.value.clone(),
                unit: p.unit.word(),
                min: p.range.map(|r| r.0),
                max: p.range.map(|r| r.1),
                step: p.step,
                choices: p.choices.clone().unwrap_or_default(),
                attrs: p.attrs.clone(),
                line: p.span.line,
            });
        }

        // Steps.
        for (i, b) in program.bindings.iter().enumerate() {
            if let Some(line) = defined.get(b.name.as_str()) {
                let msg = format!("`{}` is defined twice (first on line {line})", b.name);
                self.diag(b.span, Severity::Error, msg);
                self.set(i, State::Duplicate);
                continue;
            }
            defined.insert(b.name.as_str(), b.span.line);
            self.names.insert(b.name.as_str(), Slot::Binding(i));
        }

        let order = self.order();
        for i in order {
            let Some(b) = program.bindings.get(i) else {
                continue;
            };
            if self.state.get(i) != Some(&State::Pending) {
                continue;
            }
            let next = match self.eval(&b.expr) {
                Ok(v) => State::Done(v),
                Err(Fail::Error(span, msg)) => {
                    self.diag(span, Severity::Error, msg);
                    State::Failed
                }
                Err(Fail::Skipped) => State::Skipped,
            };
            self.set(i, next);
        }

        self.outputs();

        for (i, b) in program.bindings.iter().enumerate() {
            let (status, summary) = match self.state.get(i) {
                Some(State::Done(v)) => ("ok", v.summary()),
                Some(State::Skipped) => ("skipped", "not run: a step it needs failed".to_string()),
                Some(State::Duplicate) => ("error", "defined twice".to_string()),
                _ => ("error", "failed".to_string()),
            };
            self.out.steps.push(StepInfo {
                name: b.name.clone(),
                line: b.span.line,
                status,
                summary,
            });
        }
    }

    fn set(&mut self, i: usize, s: State) {
        if let Some(slot) = self.state.get_mut(i) {
            *slot = s;
        }
    }

    /// Works out which steps each step reads, reports unknown names and loops, and returns an
    /// order in which every step comes after the steps it reads (Kahn's algorithm: no
    /// recursion, so any number of steps is fine).
    fn order(&mut self) -> Vec<usize> {
        let program = self.program;
        let bindings = &program.bindings;
        let n = bindings.len();
        let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, b) in bindings.iter().enumerate() {
            if self.state.get(i) == Some(&State::Duplicate) {
                continue;
            }
            let mut refs = Vec::new();
            b.expr.references(&mut refs);
            for (name, span) in refs {
                match self.names.get(name.as_str()) {
                    Some(Slot::Binding(j)) => deps[i].push(*j),
                    Some(Slot::Param(_)) => {}
                    None if builtin(&name).is_some() => {}
                    None => {
                        let msg = self.unknown_name(&name);
                        self.diag(span, Severity::Error, msg);
                        self.set(i, State::Failed);
                    }
                }
            }
        }

        let mut waiting: Vec<usize> = deps.iter().map(Vec::len).collect();
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, ds) in deps.iter().enumerate() {
            for &d in ds {
                dependents[d].push(i);
            }
        }
        let mut queue: VecDeque<usize> = (0..n).filter(|i| waiting[*i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while let Some(i) = queue.pop_front() {
            order.push(i);
            for &k in &dependents[i] {
                waiting[k] -= 1;
                if waiting[k] == 0 {
                    queue.push_back(k);
                }
            }
        }

        // Whatever is left is in a loop, or needs a step that is.
        let mut reported = vec![false; n];
        for start in 0..n {
            if waiting[start] == 0 || reported[start] {
                continue;
            }
            let mut path: Vec<usize> = Vec::new();
            let mut at: HashMap<usize, usize> = HashMap::new();
            let mut cur = start;
            loop {
                if reported[cur] {
                    break;
                }
                if let Some(&first) = at.get(&cur) {
                    let cycle: Vec<usize> = path[first..].to_vec();
                    let chain: Vec<&str> = cycle
                        .iter()
                        .chain(std::iter::once(&cur))
                        .map(|&c| bindings[c].name.as_str())
                        .collect();
                    let chain = chain.join(" → ");
                    for &c in &cycle {
                        reported[c] = true;
                        let msg = format!("`{}` depends on itself ({chain})", bindings[c].name);
                        self.diag(bindings[c].span, Severity::Error, msg);
                        self.set(c, State::Failed);
                    }
                    break;
                }
                at.insert(cur, path.len());
                path.push(cur);
                match deps[cur].iter().find(|d| waiting[**d] > 0) {
                    Some(&d) => cur = d,
                    None => break,
                }
            }
        }
        for (i, w) in waiting.iter().enumerate() {
            if *w > 0 && self.state.get(i) == Some(&State::Pending) {
                self.set(i, State::Skipped);
            }
        }
        // Steps whose inputs failed are skipped quietly; their inputs carry the error.
        order
    }

    fn unknown_name(&self, name: &str) -> String {
        let known = self.names.keys().copied().chain(BUILTINS);
        match closest(name, known) {
            Some(s) => format!("unknown name `{name}`; did you mean `{s}`?"),
            None => {
                format!("unknown name `{name}`; define it with `param {name} = …` or `{name} = …`")
            }
        }
    }

    // ------------------------------------------------------------------------- expressions

    fn lookup(&self, name: &str, span: Span) -> Result<Value, Fail> {
        match self.names.get(name) {
            Some(Slot::Param(v)) => Ok(v.clone()),
            Some(Slot::Binding(i)) => match self.state.get(*i) {
                Some(State::Done(v)) => Ok(v.clone()),
                _ => Err(Fail::Skipped),
            },
            None => builtin(name).ok_or_else(|| Fail::Error(span, self.unknown_name(name))),
        }
    }

    fn eval(&mut self, e: &Expr) -> Result<Value, Fail> {
        match e {
            Expr::Number { value, .. } => Ok(Value::Number(*value)),
            Expr::Bool { value, .. } => Ok(Value::Bool(*value)),
            Expr::Text { value, .. } => Ok(Value::Text(value.as_str().into())),
            Expr::Ident { name, span } => self.lookup(name, *span),
            Expr::List { items, span } => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval(item)?);
                }
                self.ctx
                    .spend(values.len())
                    .and_then(|()| Value::list(values))
                    .map_err(|m| Fail::Error(*span, m))
            }
            Expr::Neg { expr, span } => {
                let v = self.eval(expr)?;
                let is_list = |_: usize, v: &Value| matches!(v, Value::List(_));
                nodes::replicate(&mut self.ctx, &[v], &is_list, &mut |_, a| match a.first() {
                    Some(Value::Number(n)) => Ok(Value::Number(-n)),
                    Some(Value::Field(f)) => {
                        Field::negated(f.clone()).map(|f| Value::Field(Rc::new(f)))
                    }
                    Some(other) => Err(format!("can't put a minus in front of {}", other.kind())),
                    None => Err("missing value".into()),
                })
                .map_err(|m| Fail::Error(*span, m))
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let a = self.eval(lhs)?;
                let b = self.eval(rhs)?;
                let is_list = |_: usize, v: &Value| matches!(v, Value::List(_));
                nodes::replicate(&mut self.ctx, &[a, b], &is_list, &mut |_, ab| {
                    arithmetic(*op, ab)
                })
                .map_err(|m| Fail::Error(*span, m))
            }
            Expr::Call { name, args, span } => self.call(name, args, *span),
        }
    }

    fn call(&mut self, name: &str, args: &[Arg], span: Span) -> Result<Value, Fail> {
        let Some(node) = nodes::find(name) else {
            let msg = match closest(name, nodes::NODES.iter().map(|n| n.name)) {
                Some(s) => format!("there's no step called `{name}`; did you mean `{s}`?"),
                None => format!("there's no step called `{name}` (the Recipe tab lists them all)"),
            };
            return Err(Fail::Error(span, msg));
        };
        let slots = bind(node, args, span)?;
        let mut values = Vec::with_capacity(slots.len());
        for (input, slot) in node.inputs.iter().zip(&slots) {
            values.push(match slot {
                Some(expr) => self.eval(expr)?,
                None => input.default_value(),
            });
        }
        let result = nodes::call(&mut self.ctx, node, values);
        for warning in std::mem::take(&mut self.ctx.warnings) {
            self.diag(span, Severity::Warning, format!("{name}: {warning}"));
        }
        result.map_err(|m| Fail::Error(span, format!("{name}: {m}")))
    }

    // ------------------------------------------------------------------------- outputs

    fn outputs(&mut self) {
        let program = self.program;
        let mut shown = false;
        for output in &program.outputs {
            match output {
                Output::Show { names, .. } => {
                    for (name, span) in names {
                        shown = true;
                        match self.lookup(name, *span) {
                            Ok(v) => {
                                if !self.draw(&v) {
                                    let msg = format!(
                                        "`{name}` is {}; there's nothing to draw",
                                        v.kind()
                                    );
                                    self.diag(*span, Severity::Warning, msg);
                                }
                            }
                            Err(Fail::Error(s, m)) => self.diag(s, Severity::Error, m),
                            Err(Fail::Skipped) => {}
                        }
                    }
                }
                Output::Print {
                    expr,
                    printer,
                    span,
                } => {
                    shown = true;
                    match self.eval(expr) {
                        Ok(Value::Toolpath(p)) => {
                            self.draw(&Value::Toolpath(p.clone()));
                            if self.out.print.is_some() {
                                self.diag(
                                    *span,
                                    Severity::Warning,
                                    "only the first `print` is used for now",
                                );
                                continue;
                            }
                            let printer = match printer.as_ref().map(|e| self.eval(e)) {
                                Some(Ok(Value::Text(t))) => Some(t.to_string()),
                                Some(Ok(other)) => {
                                    let msg = format!(
                                        "the printer must be a name in quotes, got {}",
                                        other.kind()
                                    );
                                    self.diag(*span, Severity::Error, msg);
                                    None
                                }
                                Some(Err(Fail::Error(s, m))) => {
                                    self.diag(s, Severity::Error, m);
                                    None
                                }
                                Some(Err(Fail::Skipped)) | None => program.printer.clone(),
                            };
                            self.out.print = Some(PrintInfo {
                                stats: p.stats(),
                                printer,
                                path: p.clone(),
                            });
                        }
                        Ok(other) => {
                            let msg = format!(
                                "`print` needs a toolpath (from toolpath or vase_spiral), got {}",
                                other.kind()
                            );
                            self.diag(*span, Severity::Error, msg);
                        }
                        Err(Fail::Error(s, m)) => self.diag(s, Severity::Error, m),
                        Err(Fail::Skipped) => {}
                    }
                }
                Output::Export { expr, file, span } => self.export(expr, file, *span),
            }
        }
        if !shown {
            // Nothing chosen: draw the last step that has something to draw.
            let last = program
                .bindings
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, b)| match self.state.get(i) {
                    Some(State::Done(v)) if drawable(v) => {
                        Some((v.clone(), b.span, b.name.clone()))
                    }
                    _ => None,
                });
            if let Some((v, span, name)) = last {
                self.draw(&v);
                let msg = format!(
                    "nothing is shown, so the last step (`{name}`) is drawn; add `show name` to choose"
                );
                self.diag(span, Severity::Info, msg);
            }
        }
    }

    /// `export x as "file"`: offers a file to save, if the value suits the file type.
    fn export(&mut self, expr: &Expr, file: &str, span: Span) {
        let lower = file.to_lowercase();
        let kind = if lower.ends_with(".stl") {
            ExportKind::Stl
        } else if lower.ends_with(".gcode") {
            ExportKind::Gcode
        } else {
            let msg = format!(
                "can't export \"{file}\" yet: name the file .stl (for shapes) or .gcode (for a toolpath)"
            );
            self.diag(span, Severity::Warning, msg);
            return;
        };
        let value = match self.eval(expr) {
            Ok(v) => v,
            Err(Fail::Error(s, m)) => {
                self.diag(s, Severity::Error, m);
                return;
            }
            Err(Fail::Skipped) => return,
        };
        let (fits, wanted) = match kind {
            ExportKind::Stl => (has_surface(&value), "surfaces or solids"),
            ExportKind::Gcode => (matches!(value, Value::Toolpath(_)), "a toolpath"),
        };
        if !fits {
            let msg = format!("\"{file}\" needs {wanted}, got {}", value.kind());
            self.diag(span, Severity::Error, msg);
            return;
        }
        self.out.exports.push(Export {
            file: file_name(file),
            kind,
            line: span.line,
            value,
        });
    }

    /// Adds a value to the drawing. Returns whether there was anything to draw.
    fn draw(&mut self, v: &Value) -> bool {
        let fine = self.ctx.quality == Quality::Final;
        if let Some(lines) = marker(v) {
            if self.drawn + 12 > MAX_DRAWN_POINTS {
                return true;
            }
            self.drawn += 12;
            for l in lines {
                self.out.drawing.bounds.union(&Bounds::of(&l));
                self.out.drawing.curves.push(l);
            }
            return true;
        }
        let cost = match v {
            Value::Curve(c) => c.points.len() + 1,
            Value::Surface(s) => s.point_count() * if fine { 2 } else { 1 },
            Value::Toolpath(p) => p.points.len(),
            Value::List(l) => {
                let mut any = false;
                for item in &l.items {
                    any |= self.draw(item);
                }
                return any;
            }
            _ => return false,
        };
        if self.drawn + cost > MAX_DRAWN_POINTS {
            if !self.too_much_drawn {
                self.too_much_drawn = true;
                let span = Span { line: 1, col: 1 };
                self.diag(
                    span,
                    Severity::Warning,
                    "there's too much to draw, so some of it is hidden",
                );
            }
            return true;
        }
        self.drawn += cost;
        let drawing = &mut self.out.drawing;
        match v {
            Value::Curve(c) => {
                let mut points = c.points.clone();
                if c.closed
                    && let Some(first) = c.points.first()
                {
                    points.push(*first);
                }
                drawing.bounds.union(&Bounds::of(&points));
                drawing.curves.push(points);
            }
            Value::Surface(s) => {
                drawing.mesh.append(&s.mesh(if fine { 2 } else { 1 }));
                for section in &s.sections {
                    drawing.bounds.union(&Bounds::of(section));
                }
            }
            // The same toolpath can be both shown and printed: draw it once.
            Value::Toolpath(p) if !drawing.paths.iter().any(|q| Rc::ptr_eq(q, p)) => {
                drawing.bounds.union(&Bounds::of(&p.points));
                drawing.paths.push(p.clone());
            }
            _ => {}
        }
        true
    }
}

fn has_surface(v: &Value) -> bool {
    match v {
        Value::Surface(_) => true,
        Value::List(l) => l.items.iter().any(has_surface),
        _ => false,
    }
}

/// A safe file name: the last part of the path, with only plain characters.
fn file_name(file: &str) -> String {
    let last = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let clean: String = last
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '-'
            }
        })
        .take(100)
        .collect();
    if clean.trim_matches(['.', ' ']).is_empty() {
        "export".into()
    } else {
        clean
    }
}

/// How points and planes are drawn: a small cross for a point; for a plane, a square in it
/// (20 mm) with a short line along its x axis and its normal, as in Grasshopper.
fn marker(v: &Value) -> Option<Vec<Vec<P3>>> {
    match v {
        Value::Point(p) => {
            let s = 1.5;
            Some(
                (0..3)
                    .map(|k| {
                        let mut a = *p;
                        let mut b = *p;
                        a[k] -= s;
                        b[k] += s;
                        vec![a, b]
                    })
                    .collect(),
            )
        }
        Value::Plane(pl) => {
            let at = |u: f64, v: f64| add3(pl.origin, add3(scale3(pl.x, u), scale3(pl.y, v)));
            let h = 10.0;
            Some(vec![
                vec![at(-h, -h), at(h, -h), at(h, h), at(-h, h), at(-h, -h)],
                vec![pl.origin, add3(pl.origin, scale3(pl.x, h))],
                vec![pl.origin, add3(pl.origin, scale3(pl.z, h))],
            ])
        }
        _ => None,
    }
}

fn drawable(v: &Value) -> bool {
    match v {
        Value::Curve(_)
        | Value::Surface(_)
        | Value::Toolpath(_)
        | Value::Point(_)
        | Value::Plane(_) => true,
        Value::List(l) => l.items.iter().any(drawable),
        _ => false,
    }
}

/// Matches a call's arguments to the node's inputs: positional ones in order, then named ones.
fn bind<'e>(node: &Node, args: &'e [Arg], span: Span) -> Result<Vec<Option<&'e Expr>>, Fail> {
    let mut slots: Vec<Option<&Expr>> = vec![None; node.inputs.len()];
    let mut next = 0;
    for arg in args {
        let at = arg.value.span();
        let index = match &arg.name {
            None => {
                next += 1;
                if next > node.inputs.len() {
                    let msg = format!(
                        "{}: too many values; it takes {} ({})",
                        node.name,
                        node.inputs.len(),
                        nodes::signature(node)
                    );
                    return Err(Fail::Error(at, msg));
                }
                next - 1
            }
            Some(name) => match node.inputs.iter().position(|i| i.name == name) {
                Some(i) => i,
                None => {
                    let names: Vec<&str> = node.inputs.iter().map(|i| i.name).collect();
                    let hint = closest(name, names.iter().copied())
                        .map(|s| format!("; did you mean `{s}`?"))
                        .unwrap_or_default();
                    let msg = format!(
                        "{}: has no input called `{name}`{hint} (inputs: {})",
                        node.name,
                        names.join(", ")
                    );
                    return Err(Fail::Error(at, msg));
                }
            },
        };
        match slots.get_mut(index) {
            Some(slot @ None) => *slot = Some(&arg.value),
            _ => {
                let name = node.inputs.get(index).map_or("?", |i| i.name);
                return Err(Fail::Error(
                    at,
                    format!("{}: `{name}` is given twice", node.name),
                ));
            }
        }
    }
    if let Some((input, _)) = node
        .inputs
        .iter()
        .zip(&slots)
        .find(|(i, s)| s.is_none() && i.required())
    {
        let msg = format!(
            "{}: missing `{}` ({})",
            node.name,
            input.name,
            nodes::signature(node)
        );
        return Err(Fail::Error(span, msg));
    }
    Ok(slots)
}

fn arithmetic(op: BinOp, ab: &[Value]) -> Result<Value, String> {
    let (verb, symbol) = match op {
        BinOp::Add => ("add", "+"),
        BinOp::Sub => ("subtract", "-"),
        BinOp::Mul => ("multiply", "*"),
        BinOp::Div => ("divide", "/"),
        BinOp::Pow => ("raise", "^"),
    };
    match (ab.first(), ab.get(1)) {
        (Some(Value::Number(x)), Some(Value::Number(y))) => {
            let r = match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div if *y == 0.0 => return Err("division by zero".into()),
                BinOp::Div => x / y,
                BinOp::Pow => x.powf(*y),
            };
            if r.is_finite() {
                Ok(Value::Number(r))
            } else {
                Err(format!(
                    "{} {symbol} {} is not a finite number",
                    format_number(*x),
                    format_number(*y)
                ))
            }
        }
        (Some(Value::Text(x)), Some(Value::Text(y))) if op == BinOp::Add => {
            Ok(Value::Text(format!("{x}{y}").into()))
        }
        // Points are vectors too: add and subtract them, scale them by numbers.
        (Some(Value::Point(p)), Some(Value::Point(q))) if matches!(op, BinOp::Add | BinOp::Sub) => {
            Ok(Value::Point(if op == BinOp::Add {
                add3(*p, *q)
            } else {
                sub3(*p, *q)
            }))
        }
        (Some(Value::Point(p)), Some(Value::Number(k)))
            if matches!(op, BinOp::Mul | BinOp::Div) =>
        {
            if op == BinOp::Div && *k == 0.0 {
                return Err("division by zero".into());
            }
            Ok(Value::Point(scale3(
                *p,
                if op == BinOp::Mul { *k } else { 1.0 / k },
            )))
        }
        (Some(Value::Number(k)), Some(Value::Point(p))) if op == BinOp::Mul => {
            Ok(Value::Point(scale3(*p, *k)))
        }
        // With a field on either side, the result is a field: the same arithmetic, point by point.
        (
            Some(a @ (Value::Number(_) | Value::Field(_))),
            Some(b @ (Value::Number(_) | Value::Field(_))),
        ) => {
            let field = |v: &Value| match v {
                Value::Field(f) => f.clone(),
                Value::Number(n) => Rc::new(Field::constant(*n)),
                _ => Rc::new(Field::constant(f64::NAN)),
            };
            Field::binary(op, field(a), field(b)).map(|f| Value::Field(Rc::new(f)))
        }
        (Some(a), Some(b)) => {
            let geometry =
                |v: &Value| matches!(v, Value::Curve(_) | Value::Surface(_) | Value::Toolpath(_));
            let hint = match op {
                BinOp::Mul | BinOp::Div if geometry(a) || geometry(b) => {
                    "; to resize geometry use `scale(…, factor)`"
                }
                BinOp::Add | BinOp::Sub if geometry(a) || geometry(b) => {
                    "; to move geometry use `move(…, x: …, y: …, z: …)`"
                }
                _ => "",
            };
            Err(format!("can't {verb} {} and {}{hint}", a.kind(), b.kind()))
        }
        _ => Err("missing value".into()),
    }
}

/// `top_scale` → "Top scale".
fn humanize(name: &str) -> String {
    let words = name.replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The candidate closest to `name` by edit distance, if it's close enough to be a typo.
fn closest<'c>(name: &str, candidates: impl Iterator<Item = &'c str>) -> Option<&'c str> {
    if name.chars().count() > 64 {
        return None;
    }
    let limit = if name.chars().count() >= 5 { 2 } else { 1 };
    candidates
        .filter(|c| c.chars().count() <= 64)
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, c)| (*d, *c))
        .map(|(_, c)| c)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut row = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        prev = row;
    }
    prev.last().copied().unwrap_or(0)
}
