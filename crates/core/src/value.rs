//! The values that flow along a recipe's wires.

use crate::curve::Curve;
use crate::field::Field;
use crate::surface::Surface;
use crate::toolpath::Toolpath;
use easycad_lang::format_number;
use std::rc::Rc;

/// The deepest lists may nest. Real recipes use one or two levels; the limit keeps hostile
/// recipes from overflowing the stack.
pub const MAX_LIST_DEPTH: u16 = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
    Text(Rc<str>),
    List(Rc<List>),
    Curve(Rc<Curve>),
    Surface(Rc<Surface>),
    Toolpath(Rc<Toolpath>),
    /// A number that varies over space (see [`crate::field`]).
    Field(Rc<Field>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    pub items: Vec<Value>,
    /// 1 for a list of plain values, 2 for a list of lists, …
    pub depth: u16,
}

impl Value {
    /// Makes a list, refusing absurdly deep nesting.
    pub fn list(items: Vec<Value>) -> Result<Value, String> {
        let inner = items
            .iter()
            .map(|v| if let Value::List(l) = v { l.depth } else { 0 })
            .max()
            .unwrap_or(0);
        if inner >= MAX_LIST_DEPTH {
            return Err(format!(
                "lists are nested more than {MAX_LIST_DEPTH} levels deep"
            ));
        }
        Ok(Value::List(Rc::new(List {
            items,
            depth: inner + 1,
        })))
    }

    /// How the value's kind reads in a message: "a number", "a curve", …
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Number(_) => "a number",
            Value::Bool(_) => "true/false",
            Value::Text(_) => "text",
            Value::List(_) => "a list",
            Value::Curve(_) => "a curve",
            Value::Surface(s) if s.capped => "a solid",
            Value::Surface(_) => "a surface",
            Value::Toolpath(_) => "a toolpath",
            Value::Field(_) => "a field",
        }
    }

    fn plural(&self) -> &'static str {
        match self {
            Value::Number(_) => "numbers",
            Value::Bool(_) => "true/false values",
            Value::Text(_) => "texts",
            Value::List(_) => "lists",
            Value::Curve(_) => "curves",
            Value::Surface(s) if s.capped => "solids",
            Value::Surface(_) => "surfaces",
            Value::Toolpath(_) => "toolpaths",
            Value::Field(_) => "fields",
        }
    }

    /// A short description for the list of steps: `160`, `list of 46 curves`, …
    pub fn summary(&self) -> String {
        match self {
            Value::Number(n) => format_number(*n),
            Value::Bool(b) => b.to_string(),
            Value::Text(t) => format!("\"{t}\""),
            Value::List(l) => {
                let items = &l.items;
                match items.first() {
                    None => "empty list".into(),
                    Some(Value::Number(_))
                        if items.len() <= 4
                            && items.iter().all(|v| matches!(v, Value::Number(_))) =>
                    {
                        format!(
                            "[{}]",
                            items
                                .iter()
                                .map(Value::summary)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                    Some(first) if items.iter().all(|v| v.plural() == first.plural()) => {
                        format!("list of {} {}", items.len(), first.plural())
                    }
                    Some(_) => format!("list of {} values", items.len()),
                }
            }
            Value::Curve(c) if c.winding.is_some() => {
                let end = c.points.last().map_or(0.0, |p| p[0].hypot(p[1]));
                format!(
                    "spiral · {} turns · {} points · ends at r = {} mm",
                    format_number(c.turns()),
                    c.points.len(),
                    format_number((end * 100.0).round() / 100.0)
                )
            }
            Value::Curve(c) => {
                format!(
                    "{} {} · {} points",
                    if c.closed { "closed" } else { "open" },
                    if c.circle.is_some() {
                        "circle"
                    } else {
                        "curve"
                    },
                    c.points.len()
                )
            }
            Value::Surface(s) => format!(
                "{} · {} sections × {} points",
                if s.capped { "solid" } else { "surface" },
                s.sections.len(),
                s.samples()
            ),
            Value::Toolpath(p) => {
                format!("toolpath · {} layers · {} points", p.layers, p.points.len())
            }
            Value::Field(_) => "field (a number that varies over space)".into(),
        }
    }

    /// Whether every number and coordinate in the value is finite (no infinity, no NaN).
    pub fn is_finite(&self) -> bool {
        let pts = |p: &[[f64; 3]]| p.iter().flatten().all(|v| v.is_finite());
        match self {
            Value::Number(n) => n.is_finite(),
            Value::Bool(_) | Value::Text(_) => true,
            Value::List(l) => l.items.iter().all(Value::is_finite),
            Value::Curve(c) => pts(&c.points),
            Value::Surface(s) => s.sections.iter().all(|sec| pts(sec)),
            Value::Toolpath(p) => pts(&p.points),
            // Checked where the field is used, point by point.
            Value::Field(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_lists_are_refused() {
        let mut v = Value::Number(1.0);
        for _ in 0..MAX_LIST_DEPTH {
            v = Value::list(vec![v]).unwrap();
        }
        assert!(Value::list(vec![v]).is_err());
    }

    #[test]
    fn summaries() {
        let n = |x| Value::Number(x);
        assert_eq!(
            Value::list(vec![n(1.0), n(2.5)]).unwrap().summary(),
            "[1, 2.5]"
        );
        let many = Value::list((0..46).map(|i| n(i as f64)).collect()).unwrap();
        assert_eq!(many.summary(), "list of 46 numbers");
        let c = Value::Curve(Rc::new(Curve::circle(1.0, 8)));
        assert_eq!(c.summary(), "closed circle · 8 points");
        assert_eq!(
            Value::list(vec![c.clone(), n(1.0)]).unwrap().summary(),
            "list of 2 values"
        );
        assert_eq!(Value::list(vec![]).unwrap().summary(), "empty list");
    }
}
