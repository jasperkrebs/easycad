//! Parts made from components, expanded into one ordinary recipe (see `docs/lamp-builder.md`).
//!
//! Every name inside a component is its part's own: in the part `tube`, the component's setting
//! `shape` becomes the setting `tube.shape`, its step `path` the step `tube.path`, and so on;
//! names from outside the component (shared settings and steps, other parts' `main.end_out`)
//! stay as they are. The part's line gives its settings' values (the rest keep the component's)
//! and its inputs' expressions. What comes out runs like any recipe.

use crate::ast::{
    Arg, Binding, Component, Condition, Expr, Literal, Output, Part, PartValue, Program, Span, Unit,
};
use std::collections::HashSet;

/// A recipe with its parts expanded, plus what the app needs to know about them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Expanded {
    /// The recipe to run: shared settings and steps, then every part's own, with no components
    /// or parts left.
    pub program: Program,
    /// The parts, in file order.
    pub parts: Vec<PartInfo>,
    /// Every part's inputs (settings whose value is an expression, like another part's port).
    pub inputs: Vec<InputInfo>,
    /// Every part's ports: the steps other parts can be mounted on.
    pub ports: Vec<PortInfo>,
    /// What's wrong with the parts (the rest still runs).
    pub errors: Vec<(Span, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartInfo {
    pub name: String,
    pub component: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputInfo {
    pub part: String,
    /// `tube.mount`: the step holding its value.
    pub name: String,
    pub label: Option<String>,
    /// Its group inside the component.
    pub group: Option<String>,
    /// The name it reads, when it's one name (`main.end_out`, `socket_port`); else empty.
    pub value: String,
    /// Whether the part's line gives it (else it's the component's default).
    pub given: bool,
    /// The name the component's default reads, when it's one name (`on_socket`).
    pub default: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortInfo {
    pub part: String,
    /// `main.end_out`: the step holding its value, and what an input names to mount there.
    pub name: String,
    pub label: Option<String>,
    pub span: Span,
}

/// Expands every part (see the module's docs). A recipe with no parts comes out as it went in.
pub fn expand(program: &Program) -> Expanded {
    let mut out = Expanded {
        program: Program {
            version: program.version.clone(),
            project: program.project.clone(),
            printer: program.printer.clone(),
            notes: program.notes.clone(),
            params: program.params.clone(),
            bindings: program.bindings.clone(),
            outputs: program.outputs.clone(),
            ..Program::default()
        },
        ..Expanded::default()
    };
    let shared: HashSet<&str> = program
        .params
        .iter()
        .map(|p| p.name.as_str())
        .chain(program.bindings.iter().map(|b| b.name.as_str()))
        .collect();
    let mut names: HashSet<&str> = HashSet::new();
    for c in &program.components {
        if !names.insert(c.name.as_str()) || shared.contains(c.name.as_str()) {
            out.errors
                .push((c.span, format!("`{}` is defined twice", c.name)));
        }
    }
    let mut parts: HashSet<&str> = HashSet::new();
    for part in &program.parts {
        if shared.contains(part.name.as_str())
            || names.contains(part.name.as_str())
            || !parts.insert(part.name.as_str())
        {
            out.errors
                .push((part.span, format!("`{}` is defined twice", part.name)));
            continue;
        }
        let Some(c) = program.components.iter().find(|c| c.name == part.component) else {
            out.errors.push((
                part.span,
                format!("there's no component `{}`", part.component),
            ));
            continue;
        };
        expand_part(part, c, &mut out);
    }
    out
}

/// The names a component's part has its own copy of.
fn locals(c: &Component) -> HashSet<String> {
    let b = &c.body;
    b.params
        .iter()
        .map(|p| p.name.clone())
        .chain(b.inputs.iter().map(|i| i.name.clone()))
        .chain(b.bindings.iter().map(|x| x.name.clone()))
        .chain(b.ports.iter().map(|p| p.name.clone()))
        .collect()
}

fn expand_part(part: &Part, c: &Component, out: &mut Expanded) {
    let body = &c.body;
    let own = locals(c);
    let p = part.name.as_str();
    let full = |name: &str| format!("{p}.{name}");
    out.parts.push(PartInfo {
        name: part.name.clone(),
        component: c.name.clone(),
        span: part.span,
    });
    for arg in &part.args {
        let known = body.params.iter().any(|q| q.name == arg.name)
            || body.inputs.iter().any(|i| i.name == arg.name);
        if !known {
            out.errors.push((
                arg.span,
                format!("the component `{}` has no setting `{}`", c.name, arg.name),
            ));
        }
    }
    let arg = |name: &str| part.args.iter().find(|a| a.name == name);

    for q in &body.params {
        let mut param = q.clone();
        param.name = full(&q.name);
        param.part = Some(part.name.clone());
        param.when = q.when.as_ref().map(|w| Condition {
            name: if own.contains(&w.name) {
                full(&w.name)
            } else {
                w.name.clone()
            },
            ..w.clone()
        });
        if let Some(a) = arg(&q.name) {
            match &a.value {
                PartValue::Literal { value, unit } => {
                    param.value = in_unit(value, *unit, q.unit);
                    param.value_range = a.range;
                }
                PartValue::Expr(_) => out.errors.push((
                    a.span,
                    format!(
                        "`{}` is a setting: give it a value, like `{}: 10`",
                        a.name, a.name
                    ),
                )),
            }
        }
        out.program.params.push(param);
    }
    for i in &body.inputs {
        let (expr, given) = match arg(&i.name) {
            Some(a) => (
                match &a.value {
                    PartValue::Expr(e) => e.clone(),
                    PartValue::Literal { value, unit } => literal_expr(value, *unit, a.span),
                },
                true,
            ),
            None => (rename(&i.default, &own, p), false),
        };
        let value = match &expr {
            Expr::Ident { name, .. } => name.clone(),
            _ => String::new(),
        };
        out.inputs.push(InputInfo {
            part: part.name.clone(),
            name: full(&i.name),
            label: i.label.clone(),
            group: None,
            value,
            given,
            default: match &i.default {
                Expr::Ident { name, .. } => name.clone(),
                _ => String::new(),
            },
            span: i.span,
        });
        out.program.bindings.push(Binding {
            name: full(&i.name),
            expr,
            span: i.span,
        });
    }
    for b in &body.bindings {
        out.program.bindings.push(Binding {
            name: full(&b.name),
            expr: rename(&b.expr, &own, p),
            span: b.span,
        });
    }
    for port in &body.ports {
        out.ports.push(PortInfo {
            part: part.name.clone(),
            name: full(&port.name),
            label: port.label.clone(),
            span: port.span,
        });
        out.program.bindings.push(Binding {
            name: full(&port.name),
            expr: rename(&port.expr, &own, p),
            span: port.span,
        });
    }
    for o in &body.outputs {
        out.program.outputs.push(match o {
            Output::Show { names, span } => Output::Show {
                names: names
                    .iter()
                    .map(|(n, s)| (if own.contains(n) { full(n) } else { n.clone() }, *s))
                    .collect(),
                span: *span,
            },
            Output::Mark {
                expr,
                settings,
                label,
                span,
            } => Output::Mark {
                expr: rename(expr, &own, p),
                settings: settings
                    .iter()
                    .map(|(n, s)| (if own.contains(n) { full(n) } else { n.clone() }, *s))
                    .collect(),
                label: label.clone(),
                span: *span,
            },
            Output::Export { expr, file, span } => Output::Export {
                expr: rename(expr, &own, p),
                file: format!("{p}-{file}"),
                span: *span,
            },
            Output::Print {
                expr,
                printer,
                span,
                ..
            } => Output::Print {
                expr: rename(expr, &own, p),
                printer: printer.as_ref().map(|e| rename(e, &own, p)),
                span: *span,
                part: Some(part.name.clone()),
            },
        });
    }
    // The inputs' groups: where they're declared among the settings (the group of the last
    // setting before them in the component).
    let groups: Vec<(u32, Option<String>)> = body
        .params
        .iter()
        .map(|q| (q.span.line, q.group.clone()))
        .collect();
    let first_input = out.inputs.len() - body.inputs.len();
    for (info, i) in out.inputs[first_input..].iter_mut().zip(&body.inputs) {
        info.group = groups
            .iter()
            .rev()
            .find(|(line, _)| *line < i.span.line)
            .or_else(|| groups.first())
            .and_then(|(_, g)| g.clone());
    }
}

/// A value written in one unit, as the setting's own unit has it.
fn in_unit(value: &Literal, written: Unit, setting: Unit) -> Literal {
    match value {
        Literal::Number(n) if written != Unit::None && written != setting => {
            let per = setting.to_base(1.0);
            Literal::Number(if per != 0.0 {
                written.to_base(*n) / per
            } else {
                *n
            })
        }
        other => other.clone(),
    }
}

/// A setting's value given to an input: the same value as an expression.
fn literal_expr(value: &Literal, unit: Unit, span: Span) -> Expr {
    match value {
        Literal::Number(n) => Expr::Number {
            value: unit.to_base(*n),
            span,
        },
        Literal::Bool(b) => Expr::Bool { value: *b, span },
        Literal::Text(t) => Expr::Text {
            value: t.clone(),
            span,
        },
        Literal::List(items) => Expr::List {
            items: items
                .iter()
                .map(|i| literal_expr(i, Unit::None, span))
                .collect(),
            span,
        },
    }
}

/// The expression with the part's own names made the part's (`path` → `tube.path`).
fn rename(expr: &Expr, own: &HashSet<String>, part: &str) -> Expr {
    let r = |e: &Expr| rename(e, own, part);
    match expr {
        Expr::Ident { name, span } => Expr::Ident {
            name: if own.contains(name) {
                format!("{part}.{name}")
            } else {
                name.clone()
            },
            span: *span,
        },
        Expr::List { items, span } => Expr::List {
            items: items.iter().map(r).collect(),
            span: *span,
        },
        Expr::Call { name, args, span } => Expr::Call {
            name: name.clone(),
            args: args
                .iter()
                .map(|a| Arg {
                    name: a.name.clone(),
                    value: r(&a.value),
                })
                .collect(),
            span: *span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op: *op,
            lhs: Box::new(r(lhs)),
            rhs: Box::new(r(rhs)),
            span: *span,
        },
        Expr::Neg { expr, span } => Expr::Neg {
            expr: Box::new(r(expr)),
            span: *span,
        },
        Expr::Number { .. } | Expr::Bool { .. } | Expr::Text { .. } => expr.clone(),
    }
}
