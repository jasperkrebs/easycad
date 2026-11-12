//! The implementations of the core steps in the table (`nodes.rs`): numbers and ranges, shapes,
//! transforms, surfaces and the toolpath strategies. The toolbox, curve tools and printing
//! steps have their own files.

use super::{Args, arg};
use crate::curve::Curve;
use crate::eval::Ctx;
use crate::field::Field;
use crate::geom::{P3, Xform};
use crate::spiral;
use crate::surface::Surface;
use crate::toolpath::{self, MAX_PATH_POINTS, Settings};
use crate::value::Value;
use std::rc::Rc;

/// The most numbers one `range` may make.
pub(super) const MAX_RANGE: usize = 100_000;

pub(super) fn range(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
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

pub(super) fn count(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    Ok(Value::Number(a.items(0)?.len() as f64))
}

pub(super) fn clamp(a: &[f64]) -> Result<f64, String> {
    let (v, lo, hi) = (arg(a, 0), arg(a, 1), arg(a, 2));
    if lo > hi {
        return Err(format!("`low` ({lo}) is above `high` ({hi})"));
    }
    Ok(v.clamp(lo, hi))
}

pub(super) fn modulo(a: &[f64]) -> Result<f64, String> {
    let (x, y) = (arg(a, 0), arg(a, 1));
    if y == 0.0 {
        return Err("`b` must not be 0".into());
    }
    Ok(x.rem_euclid(y))
}

pub(super) fn sqrt(a: &[f64]) -> Result<f64, String> {
    let x = arg(a, 0);
    if x < 0.0 {
        return Err(format!("can't take the square root of {x}"));
    }
    Ok(x.sqrt())
}

pub(super) fn distance(_: &mut Ctx, a: &Args) -> Result<Value, String> {
    if let Ok(p) = a.point(0) {
        return Ok(Value::Field(Rc::new(Field::distance(&Curve::new(
            vec![p],
            false,
        )))));
    }
    let c = a.curve(0)?;
    if c.points.is_empty() {
        return Err("the curve has no points".into());
    }
    Ok(Value::Field(Rc::new(Field::distance(c))))
}

pub(super) fn polygon(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let sides = a.count(0, 3, 1000)?;
    let r = a.positive(1)?;
    ctx.spend(sides)?;
    Ok(Value::Curve(Rc::new(Curve::polygon(sides, r))))
}

pub(super) fn star(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let tips = a.count(0, 2, 500)?;
    let outer = a.positive(1)?;
    let inner = a.num(2)?;
    if inner < 0.0 {
        return Err(format!("`inner` must be 0 or more, got {inner}"));
    }
    ctx.spend(2 * tips)?;
    Ok(Value::Curve(Rc::new(Curve::star(tips, outer, inner))))
}

pub(super) fn smooth(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let per = ctx.samples(8, 16);
    ctx.spend(c.points.len().saturating_mul(per))?;
    Ok(Value::Curve(Rc::new(c.smooth(per)?)))
}

pub(super) fn spiral(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let outer = a.num(0)?;
    let spacing = a.field(1)?;
    let tolerance = a.num(2)?;
    let min_samples = a.count(3, 0, 100_000)?;
    let (start, turns) = (a.num(4)?, a.num(5)?);
    let max = ctx.remaining().min(MAX_PATH_POINTS);
    let curve = if turns != 0.0 {
        if outer != 0.0 {
            ctx.warn("`outer` is ignored when `turns` is given".into());
        }
        let constant = if a.is_field(1) { None } else { Some(a.num(1)?) };
        spiral::spiral_turns(
            start,
            turns,
            &spacing,
            constant,
            tolerance,
            min_samples,
            max,
        )?
    } else {
        if start != 0.0 {
            return Err("`start` needs `turns`: how many turns to make from there".into());
        }
        if outer == 0.0 {
            return Err("give `outer` (the radius to end near) or `turns`".into());
        }
        spiral::spiral(outer, &spacing, tolerance, min_samples, max)?
    };
    ctx.spend(curve.points.len())?;
    Ok(Value::Curve(Rc::new(curve)))
}

pub(super) fn interlock(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let c = a.curve(0)?;
    let bumps = a.count(1, 1, 999)?;
    let width = a.positive(2)?;
    let stretch = a.num(3)?;
    if stretch < 0.0 {
        return Err("`stretch` must be 0 (leave it) or a radius".into());
    }
    let smooth = a.flag(4)?;
    let touch = a.num(5)?;
    if !smooth && (touch - 1.0).abs() > 1e-12 {
        return Err("`touch` needs `smooth: true`".into());
    }
    ctx.spend(c.points.len() * if smooth { 40 } else { 1 })?;
    let (curve, info) = if smooth {
        spiral::interlock_smooth(c, bumps, width, touch)?
    } else {
        spiral::interlock(c, bumps, width)?
    };
    let curve = if stretch > 0.0 {
        spiral::end_exactly_at(curve, stretch)
    } else {
        curve
    };
    if info.bumps != bumps {
        ctx.warn(format!(
            "bumps per turn must be odd so a peak always faces the next turn's dip; using {}",
            info.bumps
        ));
    }
    if info.overlap > 0.25 * width {
        ctx.warn(format!(
            "the spread is too short for the wiggles to meet exactly: neighbouring turns overlap by up to {} mm; widen the fade",
            easycad_lang::format_number((info.overlap * 1000.0).round() / 1000.0)
        ));
    }
    Ok(Value::Curve(Rc::new(curve)))
}

/// `place`: a part where it goes in an assembly. A toolpath keeps its points (it prints where
/// it is) and is only shown there; anything else is moved there.
pub(super) fn place(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let x = Xform::placed(a.num(1)?, a.flag(2)?);
    match a.get(0)? {
        Value::Toolpath(p) => {
            let mut placed = (**p).clone();
            placed.view = Some(x);
            Ok(Value::Toolpath(Rc::new(placed)))
        }
        other => transform(ctx, other, &x, 1.0),
    }
}

pub(super) fn transform(
    ctx: &mut Ctx,
    g: &Value,
    x: &Xform,
    z_scale: f64,
) -> Result<Value, String> {
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
        Value::Point(p) => Ok(Value::Point(x.apply(*p))),
        Value::Plane(p) => p
            .transformed(x)
            .map(Value::Plane)
            .ok_or_else(|| "this transform flattens the plane".into()),
        other => Err(format!(
            "works on curves, surfaces, toolpaths, points and planes, got {}",
            other.kind()
        )),
    }
}

/// asin/acos in degrees, for inputs within -1..1.
pub(super) fn inverse_trig(x: f64, f: fn(f64) -> f64) -> Result<f64, String> {
    if (-1.0..=1.0).contains(&x) {
        Ok(f(x).to_degrees())
    } else {
        Err(format!("needs a value between -1 and 1, got {x}"))
    }
}

pub(super) fn loft(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let curves = curves_of(a.items(0)?, "section")?;
    let m = curves.iter().map(|c| c.points.len()).max().unwrap_or(0);
    ctx.spend(curves.len().saturating_mul(m))?;
    Ok(Value::Surface(Rc::new(Surface::loft(&curves)?)))
}

/// The curves in a list input, or a message naming the first item that isn't one.
pub(super) fn curves_of<'v>(items: &'v [Value], what: &str) -> Result<Vec<&'v Curve>, String> {
    items
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            Value::Curve(c) => Ok(c.as_ref()),
            other => Err(format!("{what} {} is {}, not a curve", i + 1, other.kind())),
        })
        .collect()
}

pub(super) fn cap(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let s = a.surface(0)?;
    if !s.closed {
        return Err("needs a surface made of closed sections".into());
    }
    if s.looped {
        return Err("is already a solid (from thicken)".into());
    }
    ctx.spend(s.point_count())?;
    Ok(Value::Surface(Rc::new(Surface {
        capped: true,
        ..s.as_ref().clone()
    })))
}

pub(super) fn toolpath(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let curves = curves_of(a.items(0)?, "item")?;
    let settings = Settings::from_inputs(a.num(1)?, a.num(2)?, a.num(3)?, flow(a, 4)?)?;
    let path = toolpath::from_curves(&curves, settings, ctx.remaining().min(MAX_PATH_POINTS))?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

pub(super) fn vase_spiral(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let wall = a.surface(0)?;
    let settings = Settings::from_inputs(a.num(2)?, a.num(1)?, a.num(3)?, flow(a, 4)?)?;
    let mut path = toolpath::vase_spiral(
        wall,
        settings,
        a.flag(5)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    if a.flag(6)? {
        path.steady_start();
    }
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

pub(super) fn vase_revolve(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let profile = a.curve(0)?;
    let (rise, width, flow) = (a.field(1)?, a.field(2)?, a.field(4)?);
    let read = |f: &Rc<Field>, what: &'static str| {
        let f = f.clone();
        move |p: P3| -> Result<f64, String> {
            let v = f.at(p);
            if v.is_finite() {
                Ok(v)
            } else {
                Err(format!("`{what}` has no value at height {:.2}", p[2]))
            }
        }
    };
    let (rise, width, flow) = (
        read(&rise, "layer"),
        read(&width, "width"),
        read(&flow, "flow"),
    );
    let path = toolpath::vase_revolve(
        profile,
        &toolpath::PerTurn {
            rise: &rise,
            width: &width,
            flow: &flow,
        },
        &toolpath::Revolve {
            speed: a.num(3)?,
            bead_layer: a.num(5)?,
            tolerance: a.num(6)?,
            seam: a.num(7)?,
            flat_top: a.flag(8)?,
            steady_start: a.flag(9)?,
            along_surface: a.flag(10)?,
        },
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

/// A `flow` input: 0 (not set) or a volumetric flow in mm³/s.
pub(super) fn flow(a: &Args, i: usize) -> Result<f64, String> {
    let q = a.num(i)?;
    if q < 0.0 {
        return Err(format!("`flow` must be 0 (not set) or more, got {q}"));
    }
    Ok(q)
}

pub(super) fn helix(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let circle = a.curve(0)?;
    let (pitch, height) = (a.num(1)?, a.num(2)?);
    let settings = Settings::from_inputs(a.num(3)?, pitch, a.num(4)?, flow(a, 5)?)?;
    let mut path = toolpath::helix(
        circle,
        pitch,
        height,
        settings,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    if a.flag(6)? {
        path.steady_start();
    }
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(path)))
}

pub(super) fn stack(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let n = a.count(1, 0, 10_000)?;
    let out = toolpath::stack(
        path,
        n,
        a.num(2)?,
        a.num(3)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}

pub(super) fn join(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let paths = a
        .items(0)?
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            Value::Toolpath(p) => Ok(p.as_ref()),
            other => Err(format!(
                "item {} is {}, not a toolpath",
                i + 1,
                other.kind()
            )),
        })
        .collect::<Result<Vec<_>, String>>()?;
    if paths.is_empty() {
        return Err("needs at least one toolpath".into());
    }
    let out = toolpath::join(
        &paths,
        a.num(1)?,
        a.num(2)?,
        a.flag(3)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}

pub(super) fn wave(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let (amplitude, count, phase) = (a.field(1)?, a.num(2)?, a.num(3)?);
    let max = ctx.remaining().min(MAX_PATH_POINTS);
    let out = match a.get(0)? {
        Value::Toolpath(p) => Value::Toolpath(Rc::new(toolpath::wave_path(
            p, &amplitude, count, phase, max,
        )?)),
        Value::Curve(c) => Value::Curve(Rc::new(toolpath::wave_curve(
            c, &amplitude, count, phase, max,
        )?)),
        Value::Surface(s) => {
            let mut sections = Vec::with_capacity(s.sections.len());
            for sec in &s.sections {
                let ring = Curve::new(sec.clone(), s.closed);
                let waved = toolpath::wave_curve(&ring, &amplitude, count, phase, ctx.remaining())?;
                ctx.spend(waved.points.len())?;
                sections.push(waved.points);
            }
            Value::Surface(Rc::new(Surface {
                sections,
                revolution: None,
                ..s.as_ref().clone()
            }))
        }
        other => {
            return Err(format!(
                "works on toolpaths, curves and surfaces, got {}",
                other.kind()
            ));
        }
    };
    if let Value::Toolpath(p) = &out {
        ctx.spend(p.points.len())?;
    }
    Ok(out)
}
