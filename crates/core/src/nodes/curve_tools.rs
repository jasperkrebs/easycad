//! Steps added so EasyGCode's projects can be drawn from general pieces: choosing between
//! values, looking things up in lists, running sums, more curve tools (rounded corners,
//! simplifying, splines, pieces, joins, points and directions along a curve, squircles),
//! centring on the bed, and changing a toolpath's bead. The table entries are in `nodes.rs`.

use super::{Args, curves_of};
use crate::curve::Curve;
use crate::curve_ops;
use crate::eval::Ctx;
use crate::field::Field;
use crate::geom::{Bounds, Xform};
use crate::toolpath::{MAX_PATH_POINTS, Settings, Toolpath};
use crate::value::Value;
use std::rc::Rc;

// ------------------------------------------------------------------------------- lists & logic

/// `yes` when `test` is true (or a number other than 0), `no` otherwise. A field test picks
/// point by point between numbers or fields.
pub(super) fn pick(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    if a.is_field(0) {
        let (t, y, n) = (a.field(0)?, a.field(1)?, a.field(2)?);
        let f = Field::map(
            |v| {
                let at = |i: usize| v.get(i).copied().unwrap_or(0.0);
                Ok(if at(0) != 0.0 { at(1) } else { at(2) })
            },
            vec![t, y, n],
        )?;
        return Ok(Value::Field(Rc::new(f)));
    }
    let chosen = if a.flag(0)? { 1 } else { 2 };
    a.values
        .get(chosen)
        .cloned()
        .ok_or_else(|| "missing value".into())
}

/// Where `value` first appears in the list, counting from 0 (as `item` counts).
pub(super) fn index_of(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let items = a.items(0)?;
    let wanted = a.get(1)?;
    let same = |v: &Value| match (v, wanted) {
        (Value::Number(x), Value::Number(y)) => (x - y).abs() <= 1e-9 * x.abs().max(1.0),
        (Value::Text(x), Value::Text(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        _ => false,
    };
    match items.iter().position(same) {
        Some(i) => Ok(Value::Number(i as f64)),
        None => Err(format!("{} is not in the list", wanted.summary())),
    }
}

/// Running totals: [a, a + b, a + b + c, …]. (Grasshopper: Mass Addition, partial results)
pub(super) fn accumulate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let items = a.items(0)?;
    ctx.spend(items.len())?;
    let mut total = 0.0;
    let mut out = Vec::with_capacity(items.len());
    for (i, v) in items.iter().enumerate() {
        match v {
            Value::Number(x) => {
                total += x;
                out.push(Value::Number(total));
            }
            other => return Err(format!("item {} is {}, not a number", i + 1, other.kind())),
        }
    }
    Value::list(out)
}

/// The sum of a list of numbers. (Grasshopper: Mass Addition)
pub(super) fn sum(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let mut total = 0.0;
    for (i, v) in a.items(0)?.iter().enumerate() {
        match v {
            Value::Number(x) => total += x,
            other => return Err(format!("item {} is {}, not a number", i + 1, other.kind())),
        }
    }
    Ok(Value::Number(total))
}

/// The numbers of a list input.
fn numbers(a: &Args, i: usize) -> Result<Vec<f64>, String> {
    a.items(i)?
        .iter()
        .enumerate()
        .map(|(k, v)| match v {
            Value::Number(x) => Ok(*x),
            other => Err(format!("item {} is {}, not a number", k + 1, other.kind())),
        })
        .collect()
}

/// The smallest and largest number of a list. (Grasshopper: Bounds)
pub(super) fn bounds(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let xs = numbers(a, 0)?;
    ctx.spend(xs.len())?;
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if xs.is_empty() || !lo.is_finite() || !hi.is_finite() {
        return Err("needs a list of numbers".into());
    }
    Value::list(vec![Value::Number(lo), Value::Number(hi)])
}

/// `graph(xs, ys, x)`: the piecewise-linear graph through the points, read at `x` (a number,
/// or a field for a value at every point). (Grasshopper: Graph Mapper)
pub(super) fn graph(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (xs, ys) = (numbers(a, 0)?, numbers(a, 1)?);
    ctx.spend(xs.len())?;
    let g = crate::field::Graph::new(xs, ys)?;
    if a.is_field(2) {
        return Ok(Value::Field(Rc::new(Field::graph(g, a.field(2)?)?)));
    }
    Ok(Value::Number(g.at(a.num(2)?)))
}

// ------------------------------------------------------------------------------- curves

/// `circle(r, tolerance)`: with a tolerance, as many points as keep the circle's flats within
/// it of the true circle, at least 24 (EasyGCode's chord-tolerance sampling); otherwise the
/// evaluation's quality decides.
pub(super) fn circle(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let r = a.positive(0)?;
    let tolerance = a.num(1)?;
    let n = if tolerance > 0.0 {
        let mut dth = 2.0 * (1.0 - tolerance / r.max(1e-6)).max(-1.0).acos();
        if !dth.is_finite() || dth <= 0.0 {
            dth = 0.2;
        }
        let n = (std::f64::consts::TAU / dth).ceil();
        if n > MAX_PATH_POINTS as f64 {
            return Err(format!(
                "a tolerance of {tolerance} mm needs too many points on this circle; use a larger tolerance"
            ));
        }
        24.max(n as usize)
    } else if tolerance < 0.0 {
        return Err(format!(
            "`tolerance` must be 0 (by quality) or more, got {tolerance}"
        ));
    } else {
        ctx.samples(96, 192)
    };
    ctx.spend(n)?;
    Ok(Value::Curve(Rc::new(Curve::circle(r, n))))
}

pub(super) fn superellipse(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (rx, ry) = (a.positive(0)?, a.positive(1)?);
    let n = a.num(2)?;
    if !(0.1..=100.0).contains(&n) {
        return Err(format!("`exponent` must be between 0.1 and 100, got {n}"));
    }
    let samples = ctx.samples(180, 720);
    ctx.spend(samples)?;
    Ok(Value::Curve(Rc::new(curve_ops::superellipse(
        rx, ry, n, samples,
    ))))
}

pub(super) fn catmull_rom(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let points = super::toolbox::points_of(a, 0)?;
    if points.len() < 2 {
        return Err("needs at least 2 points".into());
    }
    let per = a.count(2, 1, 1000)?;
    ctx.spend(points.len().saturating_mul(per))?;
    Ok(Value::Curve(Rc::new(curve_ops::catmull_rom(
        &points,
        a.flag(1)?,
        per,
    ))))
}

pub(super) fn fillet(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let r = a.num(1)?;
    if r < 0.0 {
        return Err(format!("`radius` must be 0 or more, got {r}"));
    }
    let tolerance = if ctx.samples(0, 1) == 1 { 0.01 } else { 0.05 };
    let out = c.filleted(r, tolerance, ctx.remaining().min(MAX_PATH_POINTS))?;
    ctx.spend(out.points.len())?;
    Ok(Value::Curve(Rc::new(out)))
}

pub(super) fn simplify(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let tolerance = a.num(1)?;
    if tolerance.is_nan() || tolerance < 0.0 {
        return Err(format!("`tolerance` must be 0 or more, got {tolerance}"));
    }
    ctx.spend(c.points.len())?;
    Ok(Value::Curve(Rc::new(c.simplified(tolerance))))
}

pub(super) fn subcurve(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    ctx.spend(c.points.len() * 2 + 2)?;
    Ok(Value::Curve(Rc::new(c.subcurve(a.num(1)?, a.num(2)?)?)))
}

pub(super) fn join_curves(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let pieces = curves_of(a.items(0)?, "piece")?;
    if pieces.is_empty() {
        return Err("needs at least one curve".into());
    }
    let tolerance = a.num(1)?;
    if tolerance.is_nan() || tolerance < 0.0 {
        return Err(format!("`tolerance` must be 0 or more, got {tolerance}"));
    }
    ctx.spend(pieces.iter().map(|c| c.points.len() + 1).sum())?;
    Ok(Value::Curve(Rc::new(curve_ops::join_curves(
        &pieces,
        tolerance.max(1e-9),
    )?)))
}

pub(super) fn tangent(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    a.curve(0)?
        .tangent_at(a.num(1)?)
        .map(Value::Point)
        .ok_or_else(|| "the curve has no length, so no direction".into())
}

pub(super) fn normal(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    a.curve(0)?
        .normal_at(a.num(1)?)
        .map(Value::Point)
        .ok_or_else(|| "the curve has no length in plan, so no sideways direction".into())
}

pub(super) fn closest(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    if c.points.is_empty() {
        return Err("the curve has no points".into());
    }
    Ok(Value::Number(c.closest(a.point(1)?).0))
}

pub(super) fn centroid(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    a.curve(0)?
        .centroid()
        .map(Value::Point)
        .ok_or_else(|| "the curve has no points".into())
}

// ------------------------------------------------------------------------------- transforms

/// Everything moved together in X and Y so the bounding box of all of it is centred on the
/// origin (the bed centre when printed). One value gives one back; a list gives a list.
pub(super) fn center(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let items = a.items(0)?;
    let mut b = Bounds::empty();
    for (i, v) in items.iter().enumerate() {
        match v {
            Value::Curve(c) => b.union(&c.bounds()),
            Value::Surface(s) => s.sections.iter().flatten().for_each(|p| b.add(*p)),
            Value::Toolpath(p) => b.union(&extruded_bounds(p)),
            Value::Point(p) => b.add(*p),
            other => {
                return Err(format!(
                    "item {} is {}; only curves, surfaces, toolpaths and points can be centred",
                    i + 1,
                    other.kind()
                ));
            }
        }
    }
    if b.is_empty() {
        return Ok(a.get(0)?.clone());
    }
    let shift = Xform::translate(
        -(b.min[0] + b.max[0]) / 2.0,
        -(b.min[1] + b.max[1]) / 2.0,
        0.0,
    );
    let moved = items
        .iter()
        .map(|v| super::transform(ctx, v, &shift, 1.0))
        .collect::<Result<Vec<_>, _>>()?;
    match a.get(0)? {
        Value::List(_) => Value::list(moved),
        _ => moved
            .into_iter()
            .next()
            .ok_or_else(|| "nothing to centre".into()),
    }
}

/// The bounds of a toolpath's points (travels included: they must fit on the bed too).
fn extruded_bounds(p: &Toolpath) -> Bounds {
    Bounds::of(&p.points)
}

// ------------------------------------------------------------------------------- toolpaths

/// The same moves extruded for another bead: `width` × `layer` (0 keeps each). Nothing moves;
/// only how much plastic each move gets changes, and at a set flow its speed. The layer is a
/// height for extrusion only: stacking and hops still go by the toolpath's own layer height.
pub(super) fn bead(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let (w, l) = (a.num(1)?, a.num(2)?);
    if w < 0.0 || l < 0.0 {
        return Err("`width` and `layer` must be 0 (keep) or more".into());
    }
    let parts = path
        .parts
        .iter()
        .map(|s| {
            let width = if w > 0.0 { w } else { s.width };
            let layer = if l > 0.0 { l } else { s.layer };
            // Checks the new bead (and its speed at a set flow), but keeps the part's layer.
            Settings::from_inputs(width, layer, s.speed, s.flow.unwrap_or(0.0))?;
            Settings::from_inputs(width, s.layer, s.speed, s.flow.unwrap_or(0.0))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ctx.spend(path.points.len())?;
    let mut out = Toolpath {
        parts,
        ..path.as_ref().clone()
    };
    if l > 0.0 {
        for (h, travel) in out.height.iter_mut().zip(&path.travel) {
            *h = if *travel { 0.0 } else { l };
        }
    }
    Ok(Value::Toolpath(Rc::new(out)))
}
