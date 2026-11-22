//! Steps for EasyGCode's printing tricks, from general pieces: ring fills, scattered points,
//! the lean of a wall, tuning a toolpath (speeds and extrusion by zone, acceleration, fan
//! levels, dwells, spikes) and machine events (temperatures, waits for them, the printer's
//! speed and extrusion overrides: EasyGCode's foaming). The table entries are in `nodes.rs`.

use super::{Args, curves_of};
use crate::eval::Ctx;
use crate::field::Field;
use crate::geom::P3;
use crate::legs;
use crate::pendant;
use crate::rings::{self, Link};
use crate::scatter;
use crate::toolpath::{EventKind, MAX_PATH_POINTS, Temperatures};
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

pub(super) fn leg_rings(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let count = a.count(0, 1, 10_000)?;
    let width = a.positive(1)?;
    let angles = a
        .items(2)?
        .iter()
        .map(|v| match v {
            Value::Number(d) if d.is_finite() => Ok(d.to_radians()),
            other => Err(format!("`legs` must be angles, got {}", other.kind())),
        })
        .collect::<Result<Vec<f64>, String>>()?;
    let pairs = a.count(3, 0, 10_000)?;
    let seam = match a.text(6) {
        Ok("zipper") => true,
        Ok("staircase") | Err(_) => false,
        Ok(other) => {
            return Err(format!(
                "`seam` is \"staircase\" or \"zipper\", got \"{other}\""
            ));
        }
    };
    let spec = legs::LegSpec {
        count,
        spacing: width,
        angles,
        pairs,
        tip: a.num(4)?,
        fillet: a.num(5)?,
        zipper: seam,
        amount: a.num(7)?,
        gap: a.num(8)?,
        spread_at: a.num(9)?,
        r1: a.num(10)?,
        r2: a.num(11)?,
        drop: a.num(12)?,
        tolerance: a.num(13)?,
        max_points: ctx.remaining().min(MAX_PATH_POINTS),
    };
    let out = legs::leg_rings(&spec)?;
    if a.flag(14)? {
        ctx.spend(out.outline.points.len())?;
        return Ok(Value::Curve(Rc::new(out.outline)));
    }
    ctx.spend(out.rings.iter().map(|r| r.points.len()).sum())?;
    Value::list(
        out.rings
            .into_iter()
            .map(|r| Value::Curve(Rc::new(r)))
            .collect(),
    )
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
    if s.looped {
        return Err(crate::surface::LOOPED.into());
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

pub(super) fn tilt(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    ctx.spend(path.points.len().saturating_mul(4))?;
    let out = crate::tilt::tilt(path, a.num(1)?, a.num(2)?, a.num(3)?, a.flag(4)?)?;
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

pub(super) fn spokes(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    let count = a.count(2, 0, 36)?;
    ctx.spend(path.points.len() / 64 + 1)?;
    let (out, made) = toolpath_ops::spokes(
        path,
        a.num(1)?,
        count,
        a.num(3)?,
        a.num(4)?,
        ctx.remaining().min(MAX_PATH_POINTS),
    )?;
    if made < count {
        ctx.warn(format!(
            "{} of {count} spokes were left out: the path doesn't reach that height, or there isn't room to reach in from the wall",
            count - made
        ));
    }
    ctx.spend(out.points.len())?;
    Ok(Value::Toolpath(Rc::new(out)))
}

/// Temperatures from inputs `from..from + 4` (nozzle, up, mid, down); 0 is the printer's own.
fn temperatures(a: &Args, from: usize) -> Result<Temperatures, String> {
    let read = |k: usize| -> Result<Option<f64>, String> {
        let v = a.num(from + k)?;
        Ok((v != 0.0).then_some(v))
    };
    Ok(Temperatures {
        nozzle: read(0)?,
        zones: [read(1)?, read(2)?, read(3)?],
    })
}

/// Adds a machine event to the toolpath in input 0.
fn event(ctx: &mut Ctx, a: &Args, kind: EventKind, at_end: bool) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    ctx.spend(path.points.len())?;
    Ok(Value::Toolpath(Rc::new(toolpath_ops::machine_event(
        path, kind, at_end,
    )?)))
}

pub(super) fn temperature(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let temps = temperatures(a, 1)?;
    event(ctx, a, EventKind::Heat(temps), a.flag(5)?)
}

pub(super) fn wait_temperature(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let temps = temperatures(a, 1)?;
    let cool = a.flag(5)?;
    event(ctx, a, EventKind::WaitHeat { temps, cool }, a.flag(6)?)
}

/// The toolpath marked as a print aid (a brim, a skirt): every one of its moves.
pub(super) fn aid(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let path = a.toolpath(0)?;
    ctx.spend(path.points.len())?;
    let mut out = (**path).clone();
    for s in &mut out.parts {
        s.aid = true;
    }
    Ok(Value::Toolpath(Rc::new(out)))
}

pub(super) fn speed_override(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    event(ctx, a, EventKind::SpeedOverride(a.num(1)?), a.flag(2)?)
}

pub(super) fn extrusion_override(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    event(ctx, a, EventKind::ExtrusionOverride(a.num(1)?), a.flag(2)?)
}

/// Handles of a curve over 0..1: `[u, value]` or `[u, value, corner]` lists.
fn handles(a: &Args, i: usize, what: &str) -> Result<Vec<pendant::Handle>, String> {
    a.items(i)?
        .iter()
        .map(|v| {
            let Value::List(l) = v else {
                return Err(format!(
                    "`{what}`'s points must be [where 0..1, value] lists"
                ));
            };
            let n = |k: usize| match l.items.get(k) {
                Some(Value::Number(x)) => Some(*x),
                Some(Value::Bool(b)) => Some(f64::from(u8::from(*b))),
                _ => None,
            };
            match (n(0), n(1)) {
                (Some(u), Some(v)) => Ok(pendant::Handle {
                    u,
                    v,
                    sharp: n(2).is_some_and(|c| c != 0.0),
                }),
                _ => Err(format!(
                    "`{what}`'s points must be [where 0..1, value] lists"
                )),
            }
        })
        .collect()
}

pub(super) fn thread_shade(ctx: &mut Ctx, a: &Args) -> Result<Value, String> {
    let choice = |i: usize, name: &str, options: [&str; 2]| -> Result<bool, String> {
        match a.text(i) {
            Err(_) => Ok(true),
            Ok(t) if t == options[0] => Ok(true),
            Ok(t) if t == options[1] => Ok(false),
            Ok(t) => Err(format!(
                "`{name}` is \"{}\" or \"{}\", got \"{t}\"",
                options[0], options[1]
            )),
        }
    };
    let s = pendant::Settings {
        profile: handles(a, 0, "profile")?,
        height: a.num(1)?,
        wing: handles(a, 2, "wing")?,
        path: handles(a, 3, "path")?,
        path_len: a.num(4)?,
        entries: a.count(5, 1, 8)?,
        wing_zone: a.num(6)?,
        max_overhang: a.num(7)?,
        tip_gap: a.num(8)?,
        neck_gap: a.num(9)?,
        wall_gap: a.num(10)?,
        pin_gap: a.num(11)?,
        rotation: a.num(12)?,
        socket_d: a.num(13)?,
        engagement: a.num(14)?,
        grip: a.num(15)?,
        hide: a.num(16)?,
        socket_h: a.num(17)?,
        crease_top: choice(18, "top", ["crease", "full"])?,
        crease: a.num(19)?,
        layers: match a.text(20) {
            Ok("flat") => pendant::Layers::Flat,
            Err(_) | Ok("angle") => pendant::Layers::Angle,
            Ok("length") => pendant::Layers::Length,
            Ok(t) => {
                return Err(format!(
                    "`z_by` is \"flat\", \"angle\" or \"length\", got \"{t}\""
                ));
            }
        },
        line: a.num(21)?,
        layer: a.num(22)?,
        step: a.num(23)?,
        strength: a.num(24)?,
        steep: a.num(25)?,
        pivot: a.num(26)?,
        scale: a.num(27)?,
        wall_line: a.num(29)?,
        wall_layer: a.num(30)?,
        pockets: a.flag(31)?,
        leaf: a.flag(32)?,
    };
    let output = a.text(28).unwrap_or("path");
    let (points, info) = pendant::thread_shade(&s)?;
    match output {
        "path" => {
            ctx.spend(points.len())?;
            let mut line = crate::curve::Curve::new(points, false);
            line.bead = Some(info.bead);
            Ok(Value::Curve(Rc::new(line)))
        }
        "wall" => {
            ctx.spend(info.wall.len())?;
            let pts = info
                .wall
                .iter()
                .map(|&(r, z)| [r, 0.0, z - s.layer])
                .collect();
            Ok(Value::Curve(Rc::new(crate::curve::Curve::new(pts, false))))
        }
        "info" => {
            let f = |b: bool| f64::from(u8::from(b));
            let nums = [
                info.top - s.layer,
                info.layers as f64,
                f(info.clamped),
                f(info.too_many),
                f(info.limited),
                f(info.path_limited),
                info.max_lean,
                info.grip_bottom - s.layer,
                info.grip_top - s.layer,
                info.grip_r,
                f(info.grip_full),
                info.socket_shows,
                info.socket_room,
                info.top_room,
                f(info.zone_short),
                info.length,
                info.max_d,
                info.bottom_d,
                info.min_rise,
                info.max_wing_step,
                info.max_bridge_slope,
                info.min_ring_gap,
                info.min_ring_share,
                info.max_ring_share,
            ];
            Value::list(nums.iter().map(|&x| Value::Number(x)).collect())
        }
        other => Err(format!(
            "`output` is \"path\", \"wall\" or \"info\", got \"{other}\""
        )),
    }
}
