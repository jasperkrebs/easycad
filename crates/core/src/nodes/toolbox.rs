//! The Grasshopper-style toolbox: list tools, points and planes, curve operations, transforms
//! that take fields (per point), surfaces from curves, and the general vase-mode spiral. The
//! table entries live in `nodes.rs` with the rest; these are the implementations.

use super::{Args, MAX_RANGE, curves_of};
use crate::curve::Curve;
use crate::eval::Ctx;
use crate::field::Field;
use crate::geom::{P3, Plane, Xform, add3, dot3, scale3, sub3, unit3};
use crate::surface::Surface;
use crate::toolpath::{self, MAX_PATH_POINTS};
use crate::value::Value;
use easycad_lang::format_number;
use std::rc::Rc;

// ------------------------------------------------------------------------------- lists

pub(super) fn series(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (start, step) = (a.num(0)?, a.num(1)?);
    let n = a.count(2, 0, MAX_RANGE)?;
    ctx.spend(n)?;
    Value::list(
        (0..n)
            .map(|i| Value::Number(start + step * i as f64))
            .collect(),
    )
}

/// A small deterministic generator (xorshift64*), so the same seed always gives the same list.
pub(super) fn random(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let n = a.count(0, 0, MAX_RANGE)?;
    let (lo, hi) = (a.num(1)?, a.num(2)?);
    let seed = a.num(3)?;
    ctx.spend(n)?;
    let mut state: u64 = (seed.abs() * 1000.0) as u64 ^ 0x9E37_79B9_7F4A_7C15;
    if state == 0 {
        state = 0x2545_F491_4F6C_DD1D;
    }
    let mut next = || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    };
    Value::list(
        (0..n)
            .map(|_| Value::Number(lo + (hi - lo) * next()))
            .collect(),
    )
}

pub(super) fn item(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let items = a.items(0)?;
    if items.is_empty() {
        return Err("the list is empty".into());
    }
    let i = a.num(1)?.round();
    if !i.is_finite() || i.abs() > 1e12 {
        return Err("`index` is out of reach".into());
    }
    // Wraps around, like Grasshopper's List Item: -1 is the last item.
    let k = (i as i64).rem_euclid(items.len() as i64) as usize;
    items.get(k).cloned().ok_or_else(|| "no such item".into())
}

pub(super) fn reverse(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let mut items = a.items(0)?.to_vec();
    items.reverse();
    Value::list(items)
}

pub(super) fn shift(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let mut items = a.items(0)?.to_vec();
    if !items.is_empty() {
        let by = a.num(1)?.round();
        if !by.is_finite() || by.abs() > 1e12 {
            return Err("`offset` is out of reach".into());
        }
        let k = (by as i64).rem_euclid(items.len() as i64) as usize;
        items.rotate_left(k);
    }
    Value::list(items)
}

pub(super) fn flatten(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let mut out = Vec::new();
    let mut stack: Vec<&Value> = a.items(0)?.iter().rev().collect();
    while let Some(v) = stack.pop() {
        match v {
            Value::List(l) => stack.extend(l.items.iter().rev()),
            other => out.push(other.clone()),
        }
    }
    ctx.spend(out.len())?;
    Value::list(out)
}

pub(super) fn remap(a: &[f64]) -> Result<f64, String> {
    let at = |i: usize| a.get(i).copied().unwrap_or(0.0);
    let (v, lo, hi, to_lo, to_hi) = (at(0), at(1), at(2), at(3), at(4));
    if hi == lo {
        return Err("`from_low` and `from_high` must differ".into());
    }
    Ok(to_lo + (v - lo) * (to_hi - to_lo) / (hi - lo))
}

// ------------------------------------------------------------------------------- points & planes

pub(super) fn point(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    Ok(Value::Point([a.num(0)?, a.num(1)?, a.num(2)?]))
}

pub(super) fn plane(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (origin, normal) = (a.point(0)?, a.point(1)?);
    Plane::from_normal(origin, normal)
        .map(Value::Plane)
        .ok_or_else(|| "`normal` must have a length (it can't be [0, 0, 0])".into())
}

pub(super) fn sample(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    let f = a.field(0)?;
    let p = a.point(1)?;
    let v = f.at(p);
    if v.is_finite() {
        Ok(Value::Number(v))
    } else {
        Err("the field has no value at that point".into())
    }
}

// ------------------------------------------------------------------------------- curves

/// Points from a list input: point values or `[x, y, z]` lists.
pub(super) fn points_of(a: &Args, i: usize) -> Result<Vec<P3>, String> {
    a.items(i)?
        .iter()
        .enumerate()
        .map(|(k, v)| match v {
            Value::Point(p) => Ok(*p),
            Value::List(l) => {
                super::as_point(l).ok_or_else(|| format!("item {} is not a point", k + 1))
            }
            other => Err(format!("item {} is {}, not a point", k + 1, other.kind())),
        })
        .collect()
}

pub(super) fn line(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    ctx.spend(2)?;
    Ok(Value::Curve(Rc::new(Curve::new(
        vec![a.point(0)?, a.point(1)?],
        false,
    ))))
}

pub(super) fn polyline(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let points = points_of(a, 0)?;
    if points.len() < 2 {
        return Err("needs at least 2 points".into());
    }
    ctx.spend(points.len())?;
    Ok(Value::Curve(Rc::new(Curve::new(points, a.flag(1)?))))
}

pub(super) fn interpolate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let points = points_of(a, 0)?;
    if points.len() < 2 {
        return Err("needs at least 2 points".into());
    }
    let per = ctx.samples(8, 16);
    ctx.spend(points.len().saturating_mul(per))?;
    Ok(Value::Curve(Rc::new(
        Curve::new(points, a.flag(1)?).smooth(per)?,
    )))
}

pub(super) fn rectangle(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (w, h) = (a.positive(0)?, a.positive(1)?);
    let r = a.num(2)?;
    if r < 0.0 {
        return Err(format!("`radius` must be 0 or more, got {r}"));
    }
    let per = ctx.samples(8, 16);
    ctx.spend(4 * (per + 1) + 1)?;
    Ok(Value::Curve(Rc::new(Curve::rectangle(w, h, r, per))))
}

pub(super) fn ellipse(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (rx, ry) = (a.positive(0)?, a.positive(1)?);
    let n = ctx.samples(96, 192);
    ctx.spend(n)?;
    Ok(Value::Curve(Rc::new(Curve::ellipse(rx, ry, n))))
}

pub(super) fn arc(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let r = a.positive(0)?;
    let (start, end) = (a.num(1)?, a.num(2)?);
    if (end - start).abs() > 3600.0 {
        return Err("an arc can turn at most 10 times round".into());
    }
    let n = ((ctx.samples(96, 192) as f64 * (end - start).abs() / 360.0).ceil() as usize).max(2);
    ctx.spend(n)?;
    Ok(Value::Curve(Rc::new(Curve::arc(r, start, end, n))))
}

pub(super) fn divide(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let n = a.count(1, 1, MAX_RANGE)?;
    ctx.spend(n + 1)?;
    Value::list(c.divide(n).into_iter().map(Value::Point).collect())
}

pub(super) fn evaluate(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    Ok(Value::Point(a.curve(0)?.point_at(a.num(1)?)))
}

pub(super) fn length(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    Ok(Value::Number(a.curve(0)?.length()))
}

pub(super) fn flip(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    ctx.spend(c.points.len())?;
    Ok(Value::Curve(Rc::new(c.flipped())))
}

pub(super) fn seam(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    ctx.spend(c.points.len() + 1)?;
    Ok(Value::Curve(Rc::new(c.with_seam(a.num(1)?)?)))
}

pub(super) fn tween(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (c, d) = (a.curve(0)?, a.curve(1)?);
    ctx.spend(c.points.len().max(d.points.len()))?;
    Ok(Value::Curve(Rc::new(Curve::tween(c, d, a.num(2)?)?)))
}

pub(super) fn offset(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    ctx.spend(c.points.len())?;
    Ok(Value::Curve(Rc::new(c.offset(a.num(1)?))))
}

pub(super) fn resample(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let n = a.count(1, 2, MAX_PATH_POINTS)?;
    ctx.spend(n)?;
    Ok(Value::Curve(Rc::new(c.resample(n))))
}

// ------------------------------------------------------------------------------- transforms

/// The geometry with every point replaced by `f(point)`: what transforms by a field do (each
/// point moved, scaled or turned by the field's value there). Toolpaths are refused, since
/// that would change their layers.
fn map_points(
    ctx: &mut Ctx,
    g: &Value,
    what: &str,
    f: &dyn Fn(P3) -> Result<P3, String>,
) -> Result<Value, String> {
    let all = |pts: &[P3]| -> Result<Vec<P3>, String> { pts.iter().map(|p| f(*p)).collect() };
    match g {
        Value::Curve(c) => {
            ctx.spend(c.points.len())?;
            Ok(Value::Curve(Rc::new(Curve {
                points: all(&c.points)?,
                closed: c.closed,
                circle: None,
                winding: c.winding.clone(),
            })))
        }
        Value::Surface(s) => {
            ctx.spend(s.point_count())?;
            let sections = s
                .sections
                .iter()
                .map(|sec| all(sec))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Surface(Rc::new(Surface {
                sections,
                ..s.as_ref().clone()
            })))
        }
        Value::Point(p) => Ok(Value::Point(f(*p)?)),
        Value::Plane(pl) => Ok(Value::Plane(Plane {
            origin: f(pl.origin)?,
            ..*pl
        })),
        Value::Toolpath(_) => Err(format!(
            "can't {what} a toolpath point by point (that would change its layers); {what} the curves before making the toolpath"
        )),
        other => Err(format!(
            "works on curves, surfaces, points and planes, got {}",
            other.kind()
        )),
    }
}

/// A field's value at a point, or a message if it has none there.
fn value_at(field: &Field, p: P3, what: &str) -> Result<f64, String> {
    let v = field.at(p);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!(
            "`{what}` has no value at ({}, {}, {})",
            format_number(p[0]),
            format_number(p[1]),
            format_number(p[2])
        ))
    }
}

/// `move`: by fixed amounts, or point by point when any of x, y, z is a field.
pub(super) fn translate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    if (1..=3).any(|i| a.is_field(i)) {
        let (fx, fy, fz) = (a.field(1)?, a.field(2)?, a.field(3)?);
        return map_points(ctx, a.get(0)?, "move", &|p| {
            Ok([
                p[0] + value_at(&fx, p, "x")?,
                p[1] + value_at(&fy, p, "y")?,
                p[2] + value_at(&fz, p, "z")?,
            ])
        });
    }
    let (x, y, z) = (a.num(1)?, a.num(2)?, a.num(3)?);
    super::transform(ctx, a.get(0)?, &Xform::translate(x, y, z), 1.0)
}

/// `scale`: about `center`, by fixed factors or point by point when any factor is a field.
pub(super) fn scale(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let center = a.point(5)?;
    if (1..=4).any(|i| a.is_field(i)) {
        let (ff, fx, fy, fz) = (a.field(1)?, a.field(2)?, a.field(3)?, a.field(4)?);
        return map_points(ctx, a.get(0)?, "scale", &|p| {
            let f = value_at(&ff, p, "factor")?;
            let k = [
                f * value_at(&fx, p, "x")?,
                f * value_at(&fy, p, "y")?,
                f * value_at(&fz, p, "z")?,
            ];
            let d = sub3(p, center);
            Ok(add3(center, [d[0] * k[0], d[1] * k[1], d[2] * k[2]]))
        });
    }
    let f = a.num(1)?;
    let (x, y, z) = (f * a.num(2)?, f * a.num(3)?, f * a.num(4)?);
    let xform = if center == [0.0; 3] {
        Xform::scale(x, y, z)
    } else {
        Xform::scale_about(x, y, z, center)
    };
    super::transform(ctx, a.get(0)?, &xform, z)
}

/// A rotation by `degrees` about `axis` through `center`. About the vertical axis it's the
/// same exact matrix as ever (so existing recipes give identical numbers).
fn rotation(degrees: f64, axis: P3, center: P3) -> Result<Xform, String> {
    let unit = unit3(axis).ok_or("`axis` must have a length (it can't be [0, 0, 0])")?;
    let vertical = unit[0].abs() < 1e-12 && unit[1].abs() < 1e-12;
    Ok(match (vertical, center == [0.0; 3]) {
        (true, true) => Xform::rotate_z(degrees * unit[2].signum()),
        (true, false) => Xform::rotate_axis(degrees * unit[2].signum(), [0.0, 0.0, 1.0], center)
            .ok_or("bad axis")?,
        (false, _) => Xform::rotate_axis(degrees, unit, center).ok_or("bad axis")?,
    })
}

pub(super) fn rotate(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (axis, center) = (a.point(2)?, a.point(3)?);
    if a.is_field(1) {
        let angle = a.field(1)?;
        return map_points(ctx, a.get(0)?, "turn", &|p| {
            Ok(rotation(value_at(&angle, p, "angle")?, axis, center)?.apply(p))
        });
    }
    let xform = rotation(a.num(1)?, axis, center)?;
    let g = a.get(0)?;
    if matches!(g, Value::Toolpath(_))
        && unit3(axis).is_some_and(|u| u[0].abs() > 1e-12 || u[1].abs() > 1e-12)
    {
        return Err("can't tilt a toolpath (that would tilt its layers); tilt the curves before making the toolpath".into());
    }
    super::transform(ctx, g, &xform, 1.0)
}

pub(super) fn mirror(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let pl = a.plane(1)?;
    let xform = Xform::mirror(pl.origin, pl.z).ok_or("the plane has no normal")?;
    let g = a.get(0)?;
    if matches!(g, Value::Toolpath(_)) && pl.z[2].abs() > 1e-9 {
        return Err(
            "a toolpath can only be mirrored through an upright plane (its layers must stay flat)"
                .into(),
        );
    }
    super::transform(ctx, g, &xform, 1.0)
}

pub(super) fn project_along(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let pl = a.plane(1)?;
    let dir = a.point(2)?;
    let along = dot3(dir, pl.z);
    if along.abs() < 1e-9 {
        return Err("`direction` runs parallel to the plane, so nothing can reach it".into());
    }
    map_points(ctx, a.get(0)?, "project", &|p| {
        let t = dot3(sub3(pl.origin, p), pl.z) / along;
        Ok(add3(p, scale3(dir, t)))
    })
}

// ------------------------------------------------------------------------------- surfaces

pub(super) fn extrude(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let h = a.num(1)?;
    if h == 0.0 {
        return Err("`height` can't be 0".into());
    }
    ctx.spend(2 * c.points.len())?;
    Ok(Value::Surface(Rc::new(Surface::extrude(c, h)?)))
}

pub(super) fn revolve(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let profile = a.curve(0)?;
    let n = ctx.samples(96, 192);
    ctx.spend(profile.points.len().saturating_mul(n))?;
    Ok(Value::Surface(Rc::new(Surface::revolve(profile, n)?)))
}

pub(super) fn contour(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let s = a.surface(0)?;
    let layers = s.contour(a.num(1)?, a.num(2)?, ctx.remaining().min(MAX_PATH_POINTS))?;
    ctx.spend(layers.iter().map(|c| c.points.len()).sum())?;
    Value::list(
        layers
            .into_iter()
            .map(|c| Value::Curve(Rc::new(c)))
            .collect(),
    )
}

// ------------------------------------------------------------------------------- toolpaths

pub(super) fn spiralize(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let layers = curves_of(a.items(0)?, "layer")?;
    let width = a.num(1)?;
    let (path, info) = toolpath::spiralize(
        &layers,
        width,
        a.num(2)?,
        super::flow(a, 3)?,
        a.flag(4)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    if info.thickest > 0.8 * width {
        ctx.warn(format!(
            "some layers are {} mm thick, more than about 80 % of the {} mm line width; use more layers",
            format_number((info.thickest * 100.0).round() / 100.0),
            format_number(width)
        ));
    }
    if info.resampled {
        ctx.warn("the layers had different point counts, so they were resampled to match; their seams may shift".into());
    }
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}
