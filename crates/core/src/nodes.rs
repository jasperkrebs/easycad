//! The node library: every step a recipe can call, with its inputs and docs.
//!
//! The catalog the app shows (and the agent will read) is generated from this table, so the
//! docs can't drift from the code. Units: lengths in mm, angles in degrees.

use crate::curve::Curve;
use crate::eval::Ctx;
use crate::geom::Xform;
use crate::surface::Surface;
use crate::toolpath::{self, MAX_PATH_POINTS};
use crate::value::Value;
use std::rc::Rc;

/// How an input takes a list. An *item* input that receives a list runs the node once per item
/// (Grasshopper's list matching); a *list* input takes the whole list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Item,
    List,
}

#[derive(Debug)]
pub struct Input {
    pub name: &'static str,
    /// For the docs: `Number`, `Curve`, `Geometry`, …
    pub kind: &'static str,
    pub access: Access,
    /// `None`: the input is required.
    pub default: Option<f64>,
    pub doc: &'static str,
}

pub struct Node {
    pub name: &'static str,
    pub group: &'static str,
    pub inputs: &'static [Input],
    pub output: &'static str,
    pub doc: &'static str,
    pub run: fn(&mut Ctx, &Args) -> Result<Value, String>,
}

const fn req(name: &'static str, kind: &'static str, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Item,
        default: None,
        doc,
    }
}

const fn opt(name: &'static str, kind: &'static str, default: f64, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::Item,
        default: Some(default),
        doc,
    }
}

const fn all(name: &'static str, kind: &'static str, doc: &'static str) -> Input {
    Input {
        name,
        kind,
        access: Access::List,
        default: None,
        doc,
    }
}

const GEOMETRY: &str = "Geometry";

/// Every node, in catalog order.
pub static NODES: &[Node] = &[
    // ------------------------------------------------------------------ numbers & lists
    Node {
        name: "range",
        group: "Numbers & lists",
        inputs: &[
            req("start", "Number", "first number"),
            req("end", "Number", "last number"),
            req("count", "Count", "how many numbers (rounded)"),
        ],
        output: "List<Number>",
        doc: "`count` evenly spaced numbers from `start` to `end`, both included. `range(0, 1, count: 5)` is [0, 0.25, 0.5, 0.75, 1].",
        run: range,
    },
    Node {
        name: "lerp",
        group: "Numbers & lists",
        inputs: &[
            req("a", "Number", "value at t = 0"),
            req("b", "Number", "value at t = 1"),
            req("t", "Number", "fraction"),
        ],
        output: "Number",
        doc: "Blends from `a` to `b`: a + (b − a)·t.",
        run: lerp,
    },
    Node {
        name: "count",
        group: "Numbers & lists",
        inputs: &[all("list", "List", "any list")],
        output: "Number",
        doc: "How many items a list has.",
        run: count,
    },
    Node {
        name: "min",
        group: "Numbers & lists",
        inputs: &[req("a", "Number", ""), req("b", "Number", "")],
        output: "Number",
        doc: "The smaller of two numbers.",
        run: |_, a| Ok(Value::Number(a.num(0)?.min(a.num(1)?))),
    },
    Node {
        name: "max",
        group: "Numbers & lists",
        inputs: &[req("a", "Number", ""), req("b", "Number", "")],
        output: "Number",
        doc: "The larger of two numbers.",
        run: |_, a| Ok(Value::Number(a.num(0)?.max(a.num(1)?))),
    },
    Node {
        name: "clamp",
        group: "Numbers & lists",
        inputs: &[
            req("value", "Number", ""),
            req("low", "Number", ""),
            req("high", "Number", ""),
        ],
        output: "Number",
        doc: "Keeps `value` between `low` and `high`.",
        run: clamp,
    },
    Node {
        name: "mod",
        group: "Numbers & lists",
        inputs: &[req("a", "Number", ""), req("b", "Number", "")],
        output: "Number",
        doc: "Remainder of a ÷ b, always between 0 and b (for b > 0).",
        run: modulo,
    },
    Node {
        name: "abs",
        group: "Numbers & lists",
        inputs: &[req("x", "Number", "")],
        output: "Number",
        doc: "The value without its sign.",
        run: |_, a| Ok(Value::Number(a.num(0)?.abs())),
    },
    Node {
        name: "sqrt",
        group: "Numbers & lists",
        inputs: &[req("x", "Number", "0 or more")],
        output: "Number",
        doc: "Square root.",
        run: sqrt,
    },
    Node {
        name: "round",
        group: "Numbers & lists",
        inputs: &[req("x", "Number", "")],
        output: "Number",
        doc: "Nearest whole number.",
        run: |_, a| Ok(Value::Number(a.num(0)?.round())),
    },
    Node {
        name: "floor",
        group: "Numbers & lists",
        inputs: &[req("x", "Number", "")],
        output: "Number",
        doc: "Whole number at or below x.",
        run: |_, a| Ok(Value::Number(a.num(0)?.floor())),
    },
    Node {
        name: "ceil",
        group: "Numbers & lists",
        inputs: &[req("x", "Number", "")],
        output: "Number",
        doc: "Whole number at or above x.",
        run: |_, a| Ok(Value::Number(a.num(0)?.ceil())),
    },
    Node {
        name: "sin",
        group: "Numbers & lists",
        inputs: &[req("angle", "Angle", "degrees")],
        output: "Number",
        doc: "Sine of an angle in degrees: sin(90) is 1.",
        run: |_, a| Ok(Value::Number(a.num(0)?.to_radians().sin())),
    },
    Node {
        name: "cos",
        group: "Numbers & lists",
        inputs: &[req("angle", "Angle", "degrees")],
        output: "Number",
        doc: "Cosine of an angle in degrees.",
        run: |_, a| Ok(Value::Number(a.num(0)?.to_radians().cos())),
    },
    Node {
        name: "tan",
        group: "Numbers & lists",
        inputs: &[req("angle", "Angle", "degrees")],
        output: "Number",
        doc: "Tangent of an angle in degrees.",
        run: |_, a| Ok(Value::Number(a.num(0)?.to_radians().tan())),
    },
    // ------------------------------------------------------------------ curves
    Node {
        name: "circle",
        group: "Curves",
        inputs: &[req("r", "Length", "radius")],
        output: "Curve",
        doc: "A circle around the origin, flat on the bed, starting on +X and running counter-clockwise.",
        run: circle,
    },
    Node {
        name: "polygon",
        group: "Curves",
        inputs: &[
            req("sides", "Count", "3 or more"),
            req("r", "Length", "corner radius"),
        ],
        output: "Curve",
        doc: "A regular polygon around the origin with a corner on +X.",
        run: polygon,
    },
    Node {
        name: "star",
        group: "Curves",
        inputs: &[
            req("points", "Count", "number of tips"),
            req("outer", "Length", "tip radius"),
            req("inner", "Length", "inner corner radius"),
        ],
        output: "Curve",
        doc: "A star's corners around the origin, alternating tips and inner corners, first tip on +X. Sharp; add `|> smooth` to round it.",
        run: star,
    },
    Node {
        name: "smooth",
        group: "Curves",
        inputs: &[req("curve", "Curve", "a curve with corners")],
        output: "Curve",
        doc: "A smooth curve through every point of `curve` (a cubic NURBS, closed if the curve is closed).",
        run: smooth,
    },
    // ------------------------------------------------------------------ transforms
    Node {
        name: "scale",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, "what to scale"),
            opt("factor", "Number", 1.0, "uniform scale"),
            opt("x", "Number", 1.0, "extra scale in X"),
            opt("y", "Number", 1.0, "extra scale in Y"),
            opt("z", "Number", 1.0, "extra scale in Z"),
        ],
        output: GEOMETRY,
        doc: "Scales about the origin. `scale(c, 2)` doubles everything; `scale(c, y: 0.5)` squashes Y only.",
        run: scale,
    },
    Node {
        name: "rotate",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, "what to rotate"),
            req(
                "angle",
                "Angle",
                "degrees, counter-clockwise seen from above",
            ),
        ],
        output: GEOMETRY,
        doc: "Rotates about the vertical (Z) axis through the origin.",
        run: rotate,
    },
    Node {
        name: "move",
        group: "Transforms",
        inputs: &[
            req("geometry", GEOMETRY, "what to move"),
            opt("x", "Length", 0.0, ""),
            opt("y", "Length", 0.0, ""),
            opt("z", "Length", 0.0, ""),
        ],
        output: GEOMETRY,
        doc: "Moves by x, y and z. `move(c, z: 10)` lifts by 10 mm.",
        run: translate,
    },
    // ------------------------------------------------------------------ surfaces
    Node {
        name: "loft",
        group: "Surfaces",
        inputs: &[all(
            "sections",
            "List<Curve>",
            "two or more curves, bottom to top",
        )],
        output: "Surface",
        doc: "A smooth surface through the section curves, in order. Each section's first point is its seam.",
        run: loft,
    },
    Node {
        name: "cap",
        group: "Surfaces",
        inputs: &[req("surface", "Surface", "a loft of closed sections")],
        output: "Solid",
        doc: "Closes both ends of a lofted tube with flat caps, making a solid.",
        run: cap,
    },
    // ------------------------------------------------------------------ toolpaths
    Node {
        name: "vase_spiral",
        group: "Toolpaths",
        inputs: &[
            req(
                "wall",
                "Surface",
                "a loft of closed sections stacked upward",
            ),
            req("layer", "Length", "layer height: the rise per turn"),
            req("width", "Length", "line width"),
            opt("speed", "Speed", 40.0, "print speed in mm/s"),
        ],
        output: "Toolpath",
        doc: "One continuous spiral up the wall (vase mode, as in EasyGCode): starts on the bed with extrusion ramping up, snaps to whole layers, and ends with a flat turn that ramps extrusion down.",
        run: vase_spiral,
    },
    Node {
        name: "wave",
        group: "Toolpaths",
        inputs: &[
            req("geometry", GEOMETRY, "a toolpath, curve or surface"),
            req("amplitude", "Length", "how far the wave pushes in and out"),
            req(
                "count",
                "Number",
                "waves per turn (toolpath) or per loop (curve)",
            ),
            opt("phase", "Number", 0.0, "shift, in waves"),
        ],
        output: GEOMETRY,
        doc: "Pushes points in and out sideways in a sine wave. On a toolpath, whole counts stack into straight ribs, a fraction makes each layer shift (0.5 alternates, for a weave).",
        run: wave,
    },
];

/// Looks a node up by name.
pub fn find(name: &str) -> Option<&'static Node> {
    NODES.iter().find(|n| n.name == name)
}

/// `name(input: Kind, …) -> Output`, for messages and the catalog.
pub fn signature(node: &Node) -> String {
    let inputs: Vec<String> = node
        .inputs
        .iter()
        .map(|i| match i.default {
            Some(d) => format!(
                "{}: {} = {}",
                i.name,
                i.kind,
                easycad_lang::format_number(d)
            ),
            None => format!("{}: {}", i.name, i.kind),
        })
        .collect();
    format!("{}({}) -> {}", node.name, inputs.join(", "), node.output)
}

/// The node catalog as plain text: one signature, description and input notes per node.
pub fn catalog() -> String {
    let mut out = String::new();
    let mut group = "";
    for node in NODES {
        if node.group != group {
            group = node.group;
            out.push_str(&format!("\n## {group}\n\n"));
        }
        out.push_str(&signature(node));
        out.push('\n');
        out.push_str(&format!("    {}\n", node.doc));
        let notes: Vec<String> = node
            .inputs
            .iter()
            .filter(|i| !i.doc.is_empty())
            .map(|i| format!("{}: {}", i.name, i.doc))
            .collect();
        if !notes.is_empty() {
            out.push_str(&format!("    ({})\n", notes.join("; ")));
        }
    }
    out.trim_start().to_string()
}

// ---------------------------------------------------------------------------------------------
// Calling a node: list replication and typed access to the inputs
// ---------------------------------------------------------------------------------------------

/// Runs `node` on `args` (one per input), replicating over lists.
pub fn call(ctx: &mut Ctx, node: &'static Node, args: Vec<Value>) -> Result<Value, String> {
    let replicable = |i: usize, v: &Value| -> bool {
        match (node.inputs.get(i).map(|x| x.access), v) {
            (Some(Access::Item), Value::List(_)) => true,
            // A list of lists runs a list input once per inner list.
            (Some(Access::List), Value::List(l)) => {
                !l.items.is_empty() && l.items.iter().all(|x| matches!(x, Value::List(_)))
            }
            _ => false,
        }
    };
    replicate(ctx, &args, &replicable, &mut |ctx, values| {
        let out = (node.run)(ctx, &Args { node, values })?;
        if !out.is_finite() {
            return Err("the result has invalid coordinates (infinite or not a number); check the values going in".into());
        }
        Ok(out)
    })
}

/// What [`replicate`] runs once per set of items.
pub type Apply<'a> = dyn FnMut(&mut Ctx, &[Value]) -> Result<Value, String> + 'a;

/// Runs `f` once per item wherever `replicable` says an argument's list should be split up,
/// matching lists by the longest-list rule (shorter lists repeat their last item). Nested
/// lists replicate level by level.
pub fn replicate(
    ctx: &mut Ctx,
    args: &[Value],
    replicable: &dyn Fn(usize, &Value) -> bool,
    f: &mut Apply<'_>,
) -> Result<Value, String> {
    let lists: Vec<(usize, Rc<crate::value::List>)> = args
        .iter()
        .enumerate()
        .filter(|(i, v)| replicable(*i, v))
        .filter_map(|(i, v)| {
            if let Value::List(l) = v {
                Some((i, l.clone()))
            } else {
                None
            }
        })
        .collect();
    if lists.is_empty() {
        return f(ctx, args);
    }
    if lists.iter().any(|(_, l)| l.items.is_empty()) {
        return Value::list(Vec::new());
    }
    let n = lists.iter().map(|(_, l)| l.items.len()).max().unwrap_or(0);
    ctx.spend(n)?;
    let mut out = Vec::with_capacity(n);
    let mut sub = args.to_vec();
    for k in 0..n {
        for (i, l) in &lists {
            if let (Some(slot), Some(item)) =
                (sub.get_mut(*i), l.items.get(k.min(l.items.len() - 1)))
            {
                *slot = item.clone();
            }
        }
        out.push(replicate(ctx, &sub, replicable, f)?);
    }
    Value::list(out)
}

/// A node's inputs, after replication: item inputs hold single values.
pub struct Args<'a> {
    node: &'static Node,
    values: &'a [Value],
}

impl Args<'_> {
    fn name(&self, i: usize) -> &'static str {
        self.node.inputs.get(i).map_or("?", |x| x.name)
    }

    fn get(&self, i: usize) -> Result<&Value, String> {
        self.values
            .get(i)
            .ok_or_else(|| format!("missing `{}`", self.name(i)))
    }

    fn wrong(&self, i: usize, wanted: &str, got: &Value) -> String {
        format!("`{}` must be {wanted}, got {}", self.name(i), got.kind())
    }

    pub fn num(&self, i: usize) -> Result<f64, String> {
        match self.get(i)? {
            Value::Number(n) if n.is_finite() => Ok(*n),
            Value::Number(_) => Err(format!("`{}` is not a finite number", self.name(i))),
            other => Err(self.wrong(i, "a number", other)),
        }
    }

    /// A number that must be more than zero.
    pub fn positive(&self, i: usize) -> Result<f64, String> {
        let v = self.num(i)?;
        if v > 0.0 {
            Ok(v)
        } else {
            Err(format!("`{}` must be more than 0, got {v}", self.name(i)))
        }
    }

    /// A count: rounded to a whole number, which must be within `min..=max`.
    pub fn count(&self, i: usize, min: usize, max: usize) -> Result<usize, String> {
        let v = self.num(i)?.round();
        if v < min as f64 || v > max as f64 {
            return Err(format!(
                "`{}` must be between {min} and {max}, got {v}",
                self.name(i)
            ));
        }
        Ok(v as usize)
    }

    pub fn curve(&self, i: usize) -> Result<&Rc<Curve>, String> {
        match self.get(i)? {
            Value::Curve(c) => Ok(c),
            other => Err(self.wrong(i, "a curve", other)),
        }
    }

    pub fn surface(&self, i: usize) -> Result<&Rc<Surface>, String> {
        match self.get(i)? {
            Value::Surface(s) => Ok(s),
            other => Err(self.wrong(i, "a surface (from loft)", other)),
        }
    }

    /// A list input's items (a single value counts as a list of one).
    pub fn items(&self, i: usize) -> Result<&[Value], String> {
        match self.get(i)? {
            Value::List(l) => Ok(&l.items),
            single => Ok(std::slice::from_ref(single)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Node implementations
// ---------------------------------------------------------------------------------------------

/// The most numbers one `range` may make.
const MAX_RANGE: usize = 100_000;

fn range(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (start, end) = (a.num(0)?, a.num(1)?);
    let n = a.count(2, 1, MAX_RANGE)?;
    ctx.spend(n)?;
    let step = if n > 1 {
        (end - start) / (n - 1) as f64
    } else {
        0.0
    };
    // The last value is exactly `end`, without rounding drift.
    Value::list(
        (0..n)
            .map(|i| {
                Value::Number(if i + 1 == n && n > 1 {
                    end
                } else {
                    start + step * i as f64
                })
            })
            .collect(),
    )
}

fn lerp(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (x, y, t) = (a.num(0)?, a.num(1)?, a.num(2)?);
    Ok(Value::Number(x + (y - x) * t))
}

fn count(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    Ok(Value::Number(a.items(0)?.len() as f64))
}

fn clamp(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (v, lo, hi) = (a.num(0)?, a.num(1)?, a.num(2)?);
    if lo > hi {
        return Err(format!("`low` ({lo}) is above `high` ({hi})"));
    }
    Ok(Value::Number(v.clamp(lo, hi)))
}

fn modulo(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (x, y) = (a.num(0)?, a.num(1)?);
    if y == 0.0 {
        return Err("`b` must not be 0".into());
    }
    Ok(Value::Number(x.rem_euclid(y)))
}

fn sqrt(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let x = a.num(0)?;
    if x < 0.0 {
        return Err(format!("can't take the square root of {x}"));
    }
    Ok(Value::Number(x.sqrt()))
}

fn circle(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let r = a.positive(0)?;
    let n = ctx.samples(96, 192);
    ctx.spend(n)?;
    Ok(Value::Curve(Rc::new(Curve::circle(r, n))))
}

fn polygon(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let sides = a.count(0, 3, 1000)?;
    let r = a.positive(1)?;
    ctx.spend(sides)?;
    Ok(Value::Curve(Rc::new(Curve::polygon(sides, r))))
}

fn star(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let tips = a.count(0, 2, 500)?;
    let outer = a.positive(1)?;
    let inner = a.num(2)?;
    if inner < 0.0 {
        return Err(format!("`inner` must be 0 or more, got {inner}"));
    }
    ctx.spend(2 * tips)?;
    Ok(Value::Curve(Rc::new(Curve::star(tips, outer, inner))))
}

fn smooth(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let per = ctx.samples(8, 16);
    ctx.spend(c.points.len().saturating_mul(per))?;
    Ok(Value::Curve(Rc::new(c.smooth(per)?)))
}

fn transform(ctx: &mut Ctx, g: &Value, x: &Xform, z_scale: f64) -> Result<Value, String> {
    match g {
        Value::Curve(c) => {
            ctx.spend(c.points.len())?;
            Ok(Value::Curve(Rc::new(c.transformed(x))))
        }
        Value::Surface(s) => {
            ctx.spend(s.point_count())?;
            Ok(Value::Surface(Rc::new(s.transformed(x))))
        }
        Value::Toolpath(p) => {
            ctx.spend(p.points.len())?;
            Ok(Value::Toolpath(Rc::new(p.transformed(x, z_scale)?)))
        }
        other => Err(format!(
            "works on curves, surfaces and toolpaths, got {}",
            other.kind()
        )),
    }
}

fn scale(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let f = a.num(1)?;
    let (x, y, z) = (f * a.num(2)?, f * a.num(3)?, f * a.num(4)?);
    transform(ctx, a.get(0)?, &Xform::scale(x, y, z), z)
}

fn rotate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    transform(ctx, a.get(0)?, &Xform::rotate_z(a.num(1)?), 1.0)
}

fn translate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (x, y, z) = (a.num(1)?, a.num(2)?, a.num(3)?);
    transform(ctx, a.get(0)?, &Xform::translate(x, y, z), 1.0)
}

fn loft(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let items = a.items(0)?;
    let mut curves = Vec::with_capacity(items.len());
    for (i, v) in items.iter().enumerate() {
        match v {
            Value::Curve(c) => curves.push(c.as_ref()),
            other => {
                return Err(format!(
                    "section {} is {}, not a curve",
                    i + 1,
                    other.kind()
                ));
            }
        }
    }
    let m = curves.iter().map(|c| c.points.len()).max().unwrap_or(0);
    ctx.spend(curves.len().saturating_mul(m))?;
    Ok(Value::Surface(Rc::new(Surface::loft(&curves)?)))
}

fn cap(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let s = a.surface(0)?;
    if !s.closed {
        return Err("needs a surface made of closed sections".into());
    }
    ctx.spend(s.point_count())?;
    Ok(Value::Surface(Rc::new(Surface {
        capped: true,
        ..s.as_ref().clone()
    })))
}

fn vase_spiral(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let wall = a.surface(0)?;
    let (layer, width, speed) = (a.num(1)?, a.num(2)?, a.num(3)?);
    let path = toolpath::vase_spiral(
        wall,
        layer,
        width,
        speed,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

fn wave(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (amplitude, count, phase) = (a.num(1)?, a.num(2)?, a.num(3)?);
    match a.get(0)? {
        Value::Toolpath(p) => {
            let path = toolpath::wave_path(
                p,
                amplitude,
                count,
                phase,
                ctx.remaining().min(MAX_PATH_POINTS),
            )?;
            ctx.spend(path.points.len())?;
            Ok(Value::Toolpath(Rc::new(path)))
        }
        Value::Curve(c) => {
            let points = toolpath::wave_ring(
                &c.points,
                c.closed,
                amplitude,
                count,
                phase,
                ctx.remaining(),
            )?;
            ctx.spend(points.len())?;
            Ok(Value::Curve(Rc::new(Curve {
                points,
                closed: c.closed,
            })))
        }
        Value::Surface(s) => {
            let mut sections = Vec::with_capacity(s.sections.len());
            for sec in &s.sections {
                let points =
                    toolpath::wave_ring(sec, s.closed, amplitude, count, phase, ctx.remaining())?;
                ctx.spend(points.len())?;
                sections.push(points);
            }
            Ok(Value::Surface(Rc::new(Surface {
                sections,
                ..s.as_ref().clone()
            })))
        }
        other => Err(format!(
            "works on toolpaths, curves and surfaces, got {}",
            other.kind()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Quality;

    #[test]
    fn names_are_unique_and_documented() {
        for (i, n) in NODES.iter().enumerate() {
            assert!(
                NODES[..i].iter().all(|m| m.name != n.name),
                "duplicate node {}",
                n.name
            );
            assert!(!n.doc.is_empty(), "{} has no doc", n.name);
            assert!(
                !easycad_lang::RESERVED.contains(&n.name),
                "{} is a reserved word",
                n.name
            );
        }
        let cat = catalog();
        assert!(cat.starts_with("## Numbers & lists"));
        assert!(cat.contains("vase_spiral(wall: Surface, layer: Length, width: Length, speed: Speed = 40) -> Toolpath"));
    }

    #[test]
    fn replication_follows_the_longest_list() {
        let mut ctx = Ctx::new(Quality::Preview);
        let node = find("lerp").unwrap();
        let n = |x: f64| Value::Number(x);
        let ts = Value::list(vec![n(0.0), n(0.5), n(1.0)]).unwrap();
        let ends = Value::list(vec![n(10.0), n(20.0)]).unwrap();
        let out = call(&mut ctx, node, vec![n(0.0), ends, ts]).unwrap();
        // b: [10, 20, 20 (repeats its last item)], t: [0, 0.5, 1]
        assert_eq!(out, Value::list(vec![n(0.0), n(10.0), n(20.0)]).unwrap());
        let empty = Value::list(vec![]).unwrap();
        assert_eq!(
            call(&mut ctx, node, vec![n(0.0), n(1.0), empty.clone()]).unwrap(),
            empty
        );
    }

    #[test]
    fn lists_of_lists_loft_once_per_inner_list() {
        let mut ctx = Ctx::new(Quality::Preview);
        let c = |z: f64| {
            Value::Curve(Rc::new(
                Curve::circle(5.0, 8).transformed(&Xform::translate(0.0, 0.0, z)),
            ))
        };
        let pair = || Value::list(vec![c(0.0), c(10.0)]).unwrap();
        let out = call(
            &mut ctx,
            find("loft").unwrap(),
            vec![Value::list(vec![pair(), pair()]).unwrap()],
        )
        .unwrap();
        let Value::List(l) = out else {
            panic!("expected a list")
        };
        assert_eq!(l.items.len(), 2);
        assert!(matches!(l.items[0], Value::Surface(_)));
    }

    #[test]
    fn helpful_errors() {
        let mut ctx = Ctx::new(Quality::Preview);
        let n = |x: f64| Value::Number(x);
        let star = find("star").unwrap();
        assert_eq!(
            call(&mut ctx, star, vec![n(7.0), n(-1.0), n(1.0)]).unwrap_err(),
            "`outer` must be more than 0, got -1"
        );
        assert_eq!(
            call(&mut ctx, star, vec![n(1.0), n(5.0), n(1.0)]).unwrap_err(),
            "`points` must be between 2 and 500, got 1"
        );
        let curve = Value::Curve(Rc::new(Curve::circle(1.0, 8)));
        assert_eq!(
            call(&mut ctx, star, vec![curve, n(5.0), n(1.0)]).unwrap_err(),
            "`points` must be a number, got a curve"
        );
        assert!(
            call(
                &mut ctx,
                find("range").unwrap(),
                vec![n(0.0), n(1.0), n(1e9)]
            )
            .is_err()
        );
        assert_eq!(
            call(
                &mut ctx,
                find("scale").unwrap(),
                vec![n(1.0), n(2.0), n(1.0), n(1.0), n(1.0)]
            )
            .unwrap_err(),
            "works on curves, surfaces and toolpaths, got a number"
        );
    }

    #[test]
    fn range_hits_both_ends() {
        let mut ctx = Ctx::new(Quality::Preview);
        let out = call(
            &mut ctx,
            find("range").unwrap(),
            vec![Value::Number(0.0), Value::Number(0.3), Value::Number(4.0)],
        )
        .unwrap();
        let Value::List(l) = out else {
            panic!("expected a list")
        };
        assert_eq!(l.items.last(), Some(&Value::Number(0.3)));
        assert_eq!(l.items.len(), 4);
    }
}
