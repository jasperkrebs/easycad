//! Steps for EasyGCode's printing tricks, from general pieces: ring fills, scattered points,
//! the lean of a wall, and tuning a toolpath (speeds and extrusion by zone, acceleration,
//! fan levels, dwells, spikes). The table entries are in `nodes.rs`.

use super::{Args, curves_of};
use crate::eval::Ctx;
use crate::field::Field;
use crate::geom::P3;
use crate::rings::{self, Link};
use crate::scatter;
use crate::toolpath::MAX_PATH_POINTS;
use crate::toolpath_ops::{self, Spike};
use crate::value::Value;
use std::rc::Rc;

/// Points from a list input (point values or `[x, y, z]` lists).
fn points(a: &Args, i: usize) -> Result<Vec<P3>, String> {
    super::toolbox::points_of(a, i)
}

pub(super) fn rings(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let out = rings::rings(
        c,
        a.num(1)?,
        a.flag(2)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(out.iter().map(|r| r.points.len()).sum())?;
    Value::list(out.into_iter().map(|r| Value::Curve(Rc::new(r))).collect())
}

pub(super) fn link_rings(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let list = curves_of(a.items(0)?, "ring")?;
    let style = Link::parse(a.text(1)?)?;
    let line = rings::link_rings(
        &list,
        style,
        a.num(2)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(line.points.len())?;
    Ok(Value::Curve(Rc::new(line)))
}

pub(super) fn scatter(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let n = a.count(2, 0, scatter::MAX_SCATTER)?;
    ctx.spend(n.saturating_mul(n.max(1)) / 16 + n)?;
    let pts = scatter::scatter(a.num(0)?, a.num(1)?, n, a.num(3)?, a.flag(4)?)?;
    Value::list(pts.into_iter().map(Value::Point).collect())
}

pub(super) fn lean(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let s = a.surface(0)?;
    if !s.closed {
        return Err("needs a wall of closed sections".into());
    }
    Ok(Value::Field(Rc::new(Field::lean(&s.sections)?)))
}

pub(super) fn adjust(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    ctx.spend(path.points.len())?;
    let (speed, extrude) = (a.field(1)?, a.field(2)?);
    let out = toolpath_ops::adjust(path, &speed, &extrude)?;
    Ok(Value::Toolpath(Rc::new(out)))
}

pub(super) fn accelerate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let out = toolpath_ops::accelerate(path, a.num(1)?, ctx.remaining().min(MAX_PATH_POINTS))?;
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}

pub(super) fn fan(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    ctx.spend(path.points.len())?;
    let level = a.field(1)?;
    Ok(Value::Toolpath(Rc::new(toolpath_ops::fan(path, &level)?)))
}

pub(super) fn dwell(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let at = points(a, 1)?;
    ctx.spend(path.points.len().saturating_mul(at.len().max(1)) / 64 + 1)?;
    Ok(Value::Toolpath(Rc::new(toolpath_ops::dwell(
        path,
        &at,
        a.num(2)?,
    )?)))
}

pub(super) fn spikes(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let at = points(a, 1)?;
    let s = Spike {
        width: a.num(2)?,
        length: a.num(3)?,
        angle: a.num(4)?,
        out_speed: a.num(5)?,
        tip_speed: a.num(6)?,
        in_speed: a.num(7)?,
        dwell: a.num(8)?,
    };
    ctx.spend(path.points.len().saturating_mul(at.len().max(1)) / 64 + 1)?;
    let (out, made) = toolpath_ops::spikes(path, &at, s, ctx.remaining().min(MAX_PATH_POINTS))?;
    if made < at.len() {
        ctx.warn(format!(
            "{} of {} spikes were left out: they'd overlap another or run off the end of a printed stretch",
            at.len() - made,
            at.len()
        ));
    }
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}
