//! Tuning a finished toolpath, the way EasyGCode tunes its walls: speeds and extrusion that
//! change from zone to zone, speed that ramps up at a set acceleration, fan levels and dwells,
//! and spikes (EasyGCode's staples) pulled out of the wall. Each takes fields, so where things
//! happen is written in the recipe (a height, a band around the seam, a distance from a curve).

use crate::field::Field;
use crate::geom::{P3, dist3, lerp3, sub3, unit3};
use crate::toolpath::{Event, EventKind, MAX_PATH_POINTS, Settings, Toolpath};

/// A field's value at `p`, or a message naming it.
fn value(field: &Field, p: P3, what: &str) -> Result<f64, String> {
    let v = field.at(p);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!(
            "`{what}` has no value at ({:.2}, {:.2}, {:.2})",
            p[0], p[1], p[2]
        ))
    }
}

/// The middle of the move that ends at point `i`.
fn middle(path: &Toolpath, i: usize) -> P3 {
    let p = path.points.get(i).copied().unwrap_or([0.0; 3]);
    match i.checked_sub(1).and_then(|k| path.points.get(k)) {
        Some(q) => lerp3(*q, p, 0.5),
        None => p,
    }
}

fn is_travel(path: &Toolpath, i: usize) -> bool {
    i == 0 || path.travel.get(i).copied().unwrap_or(true)
}

/// Every printing move's speed and extrusion multiplied by the fields' values at its middle:
/// slower bumps, a faster wall, a flow that ramps over a fillet, more plastic where a wall
/// leans out. Travels are left alone.
pub fn adjust(path: &Toolpath, speed: &Field, extrude: &Field) -> Result<Toolpath, String> {
    let mut out = path.clone();
    for i in 0..out.points.len() {
        if is_travel(path, i) {
            continue;
        }
        let m = middle(path, i);
        let (s, e) = (value(speed, m, "speed")?, value(extrude, m, "extrude")?);
        if s <= 0.0 {
            return Err(format!(
                "`speed` must be more than 0 everywhere, but it's {s} at ({:.2}, {:.2}, {:.2})",
                m[0], m[1], m[2]
            ));
        }
        if e < 0.0 {
            return Err(format!(
                "`extrude` can't be negative, but it's {e} at ({:.2}, {:.2}, {:.2})",
                m[0], m[1], m[2]
            ));
        }
        if let Some(v) = out.speed.get_mut(i) {
            *v *= s;
        }
        if let Some(f) = out.flow.get_mut(i) {
            *f *= e;
        }
    }
    Ok(out)
}

/// The same moves extruded for another bead: `width` × `layer` (0 keeps each). Nothing moves;
/// only how much plastic each move gets, and at a set flow its speed, change. The layer is a
/// height for extrusion only: stacking and hops still go by the toolpath's own layer height.
/// Plain numbers set every part's bead; fields are read at the middle of each move, so a bead
/// can blend from one size into another a little at every move (a flat bottom's into a wall's).
pub fn bead(path: &Toolpath, width: &Field, layer: &Field) -> Result<Toolpath, String> {
    if let (Some(w), Some(l)) = (width.as_constant(), layer.as_constant()) {
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
        let mut out = Toolpath {
            parts,
            ..path.clone()
        };
        for i in 0..out.points.len() {
            let travel = is_travel(path, i);
            if let Some(slot) = out.width.get_mut(i).filter(|_| w > 0.0) {
                *slot = 0.0; // the part's new width
            }
            if let Some(slot) = out.height.get_mut(i).filter(|_| l > 0.0) {
                *slot = if travel { 0.0 } else { l };
            }
        }
        return Ok(out);
    }
    let mut out = path.clone();
    for i in 0..out.points.len() {
        if is_travel(path, i) {
            continue;
        }
        let m = middle(path, i);
        let (w, l) = (value(width, m, "width")?, value(layer, m, "layer")?);
        let at = format!("({:.2}, {:.2}, {:.2})", m[0], m[1], m[2]);
        if w < 0.0 || l < 0.0 {
            return Err(format!(
                "`width` and `layer` must be 0 (keep) or more, but they're {w} and {l} at {at}"
            ));
        }
        let (own_w, own_l) = path.bead_of(i);
        let (w, l) = (
            if w > 0.0 { w } else { own_w },
            if l > 0.0 { l } else { own_l },
        );
        // Checks the bead (and its speed at a set flow).
        let s = path.settings(i);
        Settings::from_inputs(w, l, s.speed, s.flow.unwrap_or(0.0))
            .map_err(|e| format!("{e} at {at}"))?;
        if let Some(slot) = out.width.get_mut(i) {
            *slot = w;
        }
        if let Some(slot) = out.height.get_mut(i) {
            *slot = l;
        }
    }
    Ok(out)
}

/// The speed (mm/s) of move `i`.
fn speed_of(path: &Toolpath, i: usize) -> f64 {
    path.settings(i).speed_for(path.area(i)) * path.speed_factor(i)
}

/// Speed that can only rise at `acceleration` (mm/s²), as EasyGCode ramps its spike and hanger
/// feeds: wherever a printing move is faster than the one before, the stretch that follows at
/// that speed (as far as it takes to get there: (v₁² − v₀²) / 2a) is cut into 8 equal pieces,
/// each as fast as the acceleration allows by its end. Slowing down is left to the printer.
pub fn accelerate(
    path: &Toolpath,
    acceleration: f64,
    max_points: usize,
) -> Result<Toolpath, String> {
    if !(acceleration.is_finite() && acceleration > 0.0) {
        return Err(format!(
            "`acceleration` must be more than 0 mm/s², got {acceleration}"
        ));
    }
    const PIECES: f64 = 8.0;
    let n = path.points.len();
    let mut out = path.emptied();
    let mut new_index = Vec::with_capacity(n);
    let mut starts = path.layer_starts.iter().peekable();
    // The ramp under way: the speed it starts from, the speed it heads for, how far it has come
    // and how far it needs.
    let mut ramp: Option<(f64, f64, f64, f64)> = None;
    let mut prev_speed: Option<f64> = None;
    let copy = |out: &mut Toolpath, i: usize, p: P3, factor: f64| {
        out.points.push(p);
        out.travel.push(path.travel.get(i).copied().unwrap_or(true));
        out.flow.push(path.flow.get(i).copied().unwrap_or(0.0));
        out.turn.push(path.turn.get(i).copied().unwrap_or(0.0));
        out.part.push(path.part.get(i).copied().unwrap_or(0));
        out.height.push(path.height.get(i).copied().unwrap_or(0.0));
        out.width.push(path.width.get(i).copied().unwrap_or(0.0));
        out.speed.push(factor);
    };
    for i in 0..n {
        if starts.peek() == Some(&&i) {
            out.layer_starts.push(out.points.len());
            starts.next();
        }
        let p = path.points[i];
        let factor = path.speed_factor(i);
        if is_travel(path, i) {
            ramp = None;
            copy(&mut out, i, p, factor);
            new_index.push(out.points.len() - 1);
            continue;
        }
        let v = speed_of(path, i);
        if let Some(v0) = prev_speed
            && v > v0 * (1.0 + 1e-9)
        {
            ramp = Some((v0, v, 0.0, (v * v - v0 * v0) / (2.0 * acceleration)));
        }
        prev_speed = Some(v);
        let Some((v0, v1, done, need)) = ramp.filter(|r| (v - r.1).abs() <= 1e-9 * r.1) else {
            ramp = None;
            copy(&mut out, i, p, factor);
            new_index.push(out.points.len() - 1);
            continue;
        };
        // The ramp is cut into 8 equal pieces; each runs at the speed reached by its end.
        let piece_end = |d: f64| (need * ((d / need * PIECES - 1e-9).ceil() / PIECES)).min(need);
        let speed_at = |d: f64| (v0 * v0 + 2.0 * acceleration * piece_end(d)).sqrt().min(v1);
        let q = path.points[i - 1];
        let len = dist3(q, p);
        let mut k = (done / need * PIECES + 1e-9).floor() + 1.0;
        while k < PIECES {
            let mark = need * k / PIECES - done;
            if mark >= len - 1e-9 {
                break;
            }
            if mark > 1e-9 {
                copy(
                    &mut out,
                    i,
                    lerp3(q, p, mark / len),
                    factor * speed_at(done + mark) / v1,
                );
            }
            k += 1.0;
        }
        let end = done + len;
        copy(&mut out, i, p, factor * speed_at(end.min(need)) / v1);
        new_index.push(out.points.len() - 1);
        ramp = (end < need - 1e-9).then_some((v0, v1, end, need));
        if out.points.len() > max_points.min(MAX_PATH_POINTS) {
            return Err("the ramps need too many points; use a higher acceleration".into());
        }
    }
    out.copy_events(path, &|i| new_index.get(i).copied());
    Ok(out)
}

/// Part fan levels (0–100 %) from a field read at each printing move's middle: the fan
/// changes wherever the level does (EasyGCode's fan only on the bumps, or a stronger fan on
/// overhangs). The automatic "fan on after the first layer" is replaced by these.
pub fn fan(path: &Toolpath, level: &Field) -> Result<Toolpath, String> {
    let mut out = path.clone();
    out.events.retain(|e| !matches!(e.kind, EventKind::Fan(_)));
    let mut current: f64 = 0.0; // the start block leaves the fan off
    for i in 0..path.points.len() {
        if is_travel(path, i) {
            continue;
        }
        let m = middle(path, i);
        let l = value(level, m, "level")?;
        if !(0.0..=100.0).contains(&l) {
            return Err(format!(
                "`level` must be between 0 and 100 %, but it's {l} at ({:.2}, {:.2}, {:.2})",
                m[0], m[1], m[2]
            ));
        }
        // Only a change the printer would notice (its 0–255 steps).
        if (l * 2.55).round() != (current * 2.55).round() {
            out.events.push(Event {
                after: i - 1,
                kind: EventKind::Fan(l),
            });
            current = l;
        }
    }
    out.events.sort_by_key(|e| e.after);
    Ok(out)
}

/// A pause of `seconds` at the printing move that ends nearest each point (EasyGCode's dwell
/// at a spike's tip, `G4`).
pub fn dwell(path: &Toolpath, at: &[P3], seconds: f64) -> Result<Toolpath, String> {
    if !(seconds.is_finite() && (0.0..=600.0).contains(&seconds)) {
        return Err(format!(
            "`seconds` must be between 0 and 600, got {seconds}"
        ));
    }
    let mut out = path.clone();
    for p in at {
        let nearest = (0..path.points.len())
            .filter(|&i| !is_travel(path, i))
            .min_by(|&a, &b| dist3(path.points[a], *p).total_cmp(&dist3(path.points[b], *p)))
            .ok_or("the toolpath has no printing moves")?;
        out.events.push(Event {
            after: nearest,
            kind: EventKind::Dwell(seconds),
        });
    }
    out.events.sort_by_key(|e| e.after);
    Ok(out)
}

/// A machine event (new temperatures, a wait for them, the printer's speed or extrusion
/// override) right before the toolpath's first printing move (after the travel to its start),
/// or with `at_end` right after its last move. Events at one place happen in the order they
/// were added. An empty toolpath keeps the event, and `join` puts it where the toolpath would
/// have been.
pub fn machine_event(path: &Toolpath, kind: EventKind, at_end: bool) -> Result<Toolpath, String> {
    match kind {
        EventKind::Heat(t) | EventKind::WaitHeat { temps: t, .. } => {
            for v in t.zones.iter().chain(std::iter::once(&t.nozzle)).flatten() {
                if !(v.is_finite() && *v > 0.0 && *v <= 350.0) {
                    return Err(format!(
                        "a temperature must be more than 0 and at most 350 °C (0 is the printer's own), got {v}"
                    ));
                }
            }
        }
        EventKind::SpeedOverride(v) | EventKind::ExtrusionOverride(v) => {
            if !(v.is_finite() && (1.0..=1000.0).contains(&v)) {
                return Err(format!("`percent` must be between 1 and 1000, got {v}"));
            }
        }
        EventKind::Fan(_) | EventKind::Dwell(_) => {}
    }
    // The printer takes whole percent (EasyGCode rounds its foaming speed the same way).
    let kind = match kind {
        EventKind::SpeedOverride(v) => EventKind::SpeedOverride((v + 0.5).floor()),
        EventKind::ExtrusionOverride(v) => EventKind::ExtrusionOverride((v + 0.5).floor()),
        other => other,
    };
    let mut out = path.clone();
    let after = if at_end {
        path.points.len().saturating_sub(1)
    } else {
        0
    };
    out.events.push(Event { after, kind });
    out.events.sort_by_key(|e| e.after);
    Ok(out)
}

/// Settings for [`spikes`].
#[derive(Debug, Clone, Copy)]
pub struct Spike {
    /// Distance between the way out and the way back, along the path.
    pub width: f64,
    /// How far the tip sticks out.
    pub length: f64,
    /// Upward tilt of the spike, degrees (0 is flat).
    pub angle: f64,
    /// Speed factors of the way out, along the tip and the way back.
    pub out_speed: f64,
    pub tip_speed: f64,
    pub in_speed: f64,
    /// Pause at the tip, seconds.
    pub dwell: f64,
}

/// Spikes pulled out of a toolpath at the places nearest each point (EasyGCode's staples):
/// the path leaves the wall `width / 2` before the place, runs out `length` (tilted up by
/// `angle`), across the tip, pauses, and comes back in `width / 2` after it, to carry on. Out is
/// to the right of travel: outward on a counter-clockwise wall (a negative length goes in).
/// Spikes that would overlap one before them are left out.
pub fn spikes(
    path: &Toolpath,
    at: &[P3],
    s: Spike,
    max_points: usize,
) -> Result<(Toolpath, usize), String> {
    if !(s.width.is_finite() && s.width > 0.0) {
        return Err(format!("`width` must be more than 0, got {}", s.width));
    }
    if !s.length.is_finite() || !s.angle.is_finite() || s.angle.abs() > 89.0 {
        return Err("`length` must be a number and `angle` between -89 and 89°".into());
    }
    for (name, v) in [
        ("out_speed", s.out_speed),
        ("tip_speed", s.tip_speed),
        ("in_speed", s.in_speed),
    ] {
        if !(v.is_finite() && v > 0.0) {
            return Err(format!("`{name}` must be more than 0, got {v}"));
        }
    }
    // Distance along the path at each point (travels count too, they never hold a spike).
    let n = path.points.len();
    let mut along = vec![0.0; n];
    for i in 1..n {
        along[i] = along[i - 1] + dist3(path.points[i - 1], path.points[i]);
    }
    // Where each spike goes: (centre distance, its move).
    let mut places: Vec<(f64, usize)> = Vec::new();
    for p in at {
        let mut best: Option<(f64, f64, usize)> = None; // (distance to p, along, move)
        for i in 1..n {
            if is_travel(path, i) {
                continue;
            }
            let (a, b) = (path.points[i - 1], path.points[i]);
            let ab = sub3(b, a);
            let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
            let f = if len2 > 0.0 {
                (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / len2)
                    .clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = dist3(*p, lerp3(a, b, f));
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, along[i - 1] + f * len2.sqrt(), i));
            }
        }
        if let Some((_, s_at, i)) = best {
            places.push((s_at, i));
        }
    }
    places.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Keep spikes whose legs stay on one printing stretch and clear of the one before.
    let half = s.width / 2.0;
    let stretch_of = |i: usize| -> (f64, f64) {
        let mut a = i;
        while a > 1 && !is_travel(path, a - 1) {
            a -= 1;
        }
        let mut b = i;
        while b + 1 < n && !is_travel(path, b + 1) {
            b += 1;
        }
        (along[a - 1], along[b])
    };
    let mut kept: Vec<f64> = Vec::new();
    for (c, i) in places {
        let (lo, hi) = stretch_of(i);
        if c - half < lo || c + half > hi {
            continue;
        }
        if kept.last().is_some_and(|k| c - half < k + half + 1e-6) {
            continue;
        }
        kept.push(c);
    }
    let count = kept.len();
    // Point on the path at a distance along it, with the move it falls on.
    let point_at = |d: f64| -> (P3, usize) {
        let i = along.partition_point(|a| *a < d).clamp(1, n - 1);
        let (a, b) = (path.points[i - 1], path.points[i]);
        let len = along[i] - along[i - 1];
        let f = if len > 0.0 {
            ((d - along[i - 1]) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (lerp3(a, b, f), i)
    };
    let lean = s.angle.to_radians();
    let (out_xy, out_z) = (s.length * lean.cos(), s.length * lean.sin());

    let mut out = path.emptied();
    let mut new_index: Vec<Option<usize>> = vec![None; n];
    let mut starts = path.layer_starts.iter().peekable();
    let mut spikes_left = kept.iter().peekable();
    let push = |out: &mut Toolpath, i: usize, p: P3, travel: bool, flow: f64, speed: f64| {
        out.points.push(p);
        out.travel.push(travel);
        out.flow.push(if travel { 0.0 } else { flow });
        out.turn.push(path.turn.get(i).copied().unwrap_or(0.0));
        out.part.push(path.part.get(i).copied().unwrap_or(0));
        out.height.push(if travel {
            0.0
        } else {
            path.height.get(i).copied().unwrap_or(0.0)
        });
        out.width.push(if travel {
            0.0
        } else {
            path.width.get(i).copied().unwrap_or(0.0)
        });
        out.speed.push(speed);
    };
    for i in 0..n {
        if starts.peek() == Some(&&i) {
            out.layer_starts.push(out.points.len());
            starts.next();
        }
        // Spikes that start on this move.
        while let Some(&&c) = spikes_left.peek() {
            if i == 0 || c - half > along[i] {
                break;
            }
            spikes_left.next();
            let (a, ia) = point_at(c - half);
            let (b, _) = point_at(c + half);
            let (_, im) = point_at(c);
            // Out: the right-hand side of travel at the spike's centre.
            let dir = unit3(sub3(path.points[im], path.points[im - 1])).unwrap_or([1.0, 0.0, 0.0]);
            let normal = [dir[1], -dir[0], 0.0];
            let tip = |p: P3| {
                [
                    p[0] + normal[0] * out_xy,
                    p[1] + normal[1] * out_xy,
                    p[2] + out_z,
                ]
            };
            let base_speed = path.speed_factor(ia);
            let flow = path.flow.get(ia).copied().unwrap_or(1.0);
            push(&mut out, ia, a, false, flow, base_speed);
            push(&mut out, ia, tip(a), false, 1.0, base_speed * s.out_speed);
            push(&mut out, ia, tip(b), false, 1.0, base_speed * s.tip_speed);
            if s.dwell > 0.0 {
                out.events.push(Event {
                    after: out.points.len() - 1,
                    kind: EventKind::Dwell(s.dwell),
                });
            }
            push(&mut out, ia, b, false, 1.0, base_speed * s.in_speed);
        }
        // The original point, unless a spike skipped over it.
        let inside = kept
            .iter()
            .any(|c| along[i] >= c - half - 1e-9 && along[i] <= c + half + 1e-9);
        if inside && !is_travel(path, i) {
            continue;
        }
        let travel = is_travel(path, i);
        push(
            &mut out,
            i,
            path.points[i],
            travel,
            path.flow.get(i).copied().unwrap_or(0.0),
            path.speed_factor(i),
        );
        new_index[i] = Some(out.points.len() - 1);
        if out.points.len() > max_points.min(MAX_PATH_POINTS) {
            return Err("the spikes need too many points".into());
        }
    }
    let spike_events = std::mem::take(&mut out.events);
    out.copy_events(path, &|i| new_index.get(i).copied().flatten());
    out.events.extend(spike_events);
    out.events.sort_by_key(|e| e.after);
    Ok((out, count))
}

/// Spokes into a toolpath around the Z axis, `count` of them, from where it first reaches
/// `height` (the first there, the rest spread evenly over the turn after): at each the line
/// leaves the wall, runs in to the middle half a line to one side, turns round a half circle
/// `inner` from the axis and runs back out half a line to the other side, back into the wall.
/// The legs lie flat at `height`; each move gets the bead of the wall move it starts from, and
/// the stretch of wall between the legs (a line width) is left out: the legs close it. A cross
/// that ties a vase's bottom to its wall. Returns the toolpath and how many spokes went in.
pub fn spokes(
    path: &Toolpath,
    height: f64,
    count: usize,
    width: f64,
    inner: f64,
    angle: Option<f64>,
    max_points: usize,
) -> Result<(Toolpath, usize), String> {
    if !(width.is_finite() && width > 0.0) {
        return Err(format!("`width` must be more than 0, got {width}"));
    }
    if !height.is_finite() || !inner.is_finite() {
        return Err("`height` and `inner` must be numbers".into());
    }
    let inner = if inner > 0.0 {
        inner.max(width / 2.0)
    } else {
        1.25 * width
    };
    let n = path.points.len();
    let mut along = vec![0.0; n];
    for i in 1..n {
        along[i] = along[i - 1] + dist3(path.points[i - 1], path.points[i]);
    }
    // Where it first reaches the height, on a printing move.
    let Some(i0) = (1..n).find(|&i| !is_travel(path, i) && path.points[i][2] >= height - 1e-9)
    else {
        return Ok((path.clone(), 0));
    };
    if count == 0 {
        return Ok((path.clone(), 0));
    }
    let point_at = |d: f64| -> (P3, usize) {
        let i = along.partition_point(|a| *a < d).clamp(1, n - 1);
        let (a, b) = (path.points[i - 1], path.points[i]);
        let len = along[i] - along[i - 1];
        let f = if len > 0.0 {
            ((d - along[i - 1]) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (lerp3(a, b, f), i)
    };
    let (a0, b0) = (path.points[i0 - 1], path.points[i0]);
    let f0 = if b0[2] > a0[2] {
        ((height - a0[2]) / (b0[2] - a0[2])).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let d0 = along[i0 - 1] + f0 * (along[i0] - along[i0 - 1]);
    // Walking on from there: the angle turned round the axis so far, unwrapped.
    let ang = |p: P3| p[1].atan2(p[0]);
    let step = std::f64::consts::TAU / count as f64;
    let wrap = |mut a: f64| {
        while a > std::f64::consts::PI {
            a -= std::f64::consts::TAU;
        }
        while a < -std::f64::consts::PI {
            a += std::f64::consts::TAU;
        }
        a
    };
    // With an angle, the first spoke goes where the path next passes it (or a spoke's turn on
    // from it), so spokes at several heights stack; without, where it reaches the height.
    let first = match angle {
        Some(deg) => {
            let p0 = point_at(d0).0;
            let way = (i0..n)
                .map(|k| wrap(ang(path.points[k]) - ang(p0)))
                .find(|d| d.abs() > 1e-9)
                .map_or(1.0, f64::signum);
            (way * wrap(deg.to_radians() - ang(p0))).rem_euclid(step)
        }
        None => 0.0,
    };
    let mut centres = if first < 1e-12 { vec![d0] } else { vec![] };
    let (mut prev, mut s_prev, mut turned) = (point_at(d0).0, d0, 0.0);
    let mut i = i0;
    while centres.len() < count && i < n {
        if is_travel(path, i) {
            break;
        }
        let p = path.points[i];
        let da = wrap(ang(p) - ang(prev)).abs();
        let want = first + step * centres.len() as f64;
        if da > 0.0 && turned + da >= want {
            let f = ((want - turned) / da).clamp(0.0, 1.0);
            let d = s_prev + f * (along[i] - s_prev);
            centres.push(d);
            prev = point_at(d).0;
            s_prev = d;
            turned = want;
            continue;
        }
        turned += da;
        prev = p;
        s_prev = along[i];
        i += 1;
    }
    // Keep spokes on one printing stretch, clear of each other, long enough to reach in.
    let half = width / 2.0;
    let kept: Vec<f64> = centres
        .into_iter()
        .filter(|&c| {
            let (m, im) = point_at(c);
            let r = m[0].hypot(m[1]);
            c - half > along[0]
                && c + half < along[n - 1]
                && !is_travel(path, im)
                && r > inner + 1.5 * width
        })
        .collect();
    let count = kept.len();
    let mut out = path.emptied();
    let mut new_index: Vec<Option<usize>> = vec![None; n];
    let mut starts = path.layer_starts.iter().peekable();
    let mut left = kept.iter().peekable();
    let push = |out: &mut Toolpath, i: usize, p: P3, travel: bool, flow: f64, speed: f64| {
        out.points.push(p);
        out.travel.push(travel);
        out.flow.push(if travel { 0.0 } else { flow });
        out.turn.push(path.turn.get(i).copied().unwrap_or(0.0));
        out.part.push(path.part.get(i).copied().unwrap_or(0));
        out.height.push(if travel {
            0.0
        } else {
            path.height.get(i).copied().unwrap_or(0.0)
        });
        out.width.push(if travel {
            0.0
        } else {
            path.width.get(i).copied().unwrap_or(0.0)
        });
        out.speed.push(speed);
    };
    for i in 0..n {
        if starts.peek() == Some(&&i) {
            out.layer_starts.push(out.points.len());
            starts.next();
        }
        while let Some(&&c) = left.peek() {
            if i == 0 || c - half > along[i] {
                break;
            }
            left.next();
            let (a, ia) = point_at(c - half);
            let (b, _) = point_at(c + half);
            let (m, _) = point_at(c);
            let r = m[0].hypot(m[1]).max(1e-9);
            let u = [m[0] / r, m[1] / r];
            // Along the wall, the way it goes.
            let tl = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
            let t = [(b[0] - a[0]) / tl, (b[1] - a[1]) / tl];
            let side = |p: P3| (p[0] - m[0]) * t[0] + (p[1] - m[1]) * t[1];
            let (la, lb) = (side(a), side(b));
            let at = |radius: f64, lateral: f64| -> P3 {
                [
                    u[0] * radius + t[0] * lateral,
                    u[1] * radius + t[1] * lateral,
                    height,
                ]
            };
            let (c_in, c_out) = (at(inner, la), at(inner, lb));
            // A line width in from the wall, down at the height.
            let toward = |from: P3, to: P3| -> P3 {
                let d = (to[0] - from[0]).hypot(to[1] - from[1]);
                let f = (width / d.max(1e-9)).min(1.0);
                [
                    from[0] + (to[0] - from[0]) * f,
                    from[1] + (to[1] - from[1]) * f,
                    height,
                ]
            };
            let speed = path.speed_factor(ia);
            let flow = path.flow.get(ia).copied().unwrap_or(1.0);
            push(&mut out, ia, a, false, flow, speed);
            push(&mut out, ia, toward(a, c_in), false, 1.0, speed);
            push(&mut out, ia, c_in, false, 1.0, speed);
            let (mid, rr) = ((la + lb) / 2.0, (lb - la).abs() / 2.0);
            let sign = if lb >= la { 1.0 } else { -1.0 };
            for k in 1..8 {
                let ph = std::f64::consts::PI * k as f64 / 8.0;
                push(
                    &mut out,
                    ia,
                    at(inner - rr * ph.sin(), mid - sign * rr * ph.cos()),
                    false,
                    1.0,
                    speed,
                );
            }
            push(&mut out, ia, c_out, false, 1.0, speed);
            push(&mut out, ia, toward(b, c_out), false, 1.0, speed);
            push(&mut out, ia, b, false, 1.0, speed);
        }
        // The original point, unless a spoke left it out.
        let inside = kept
            .iter()
            .any(|c| along[i] >= c - half - 1e-9 && along[i] <= c + half + 1e-9);
        if inside && !is_travel(path, i) {
            continue;
        }
        let travel = is_travel(path, i);
        push(
            &mut out,
            i,
            path.points[i],
            travel,
            path.flow.get(i).copied().unwrap_or(0.0),
            path.speed_factor(i),
        );
        new_index[i] = Some(out.points.len() - 1);
        if out.points.len() > max_points.min(MAX_PATH_POINTS) {
            return Err("the spokes need too many points".into());
        }
    }
    out.copy_events(path, &|i| new_index.get(i).copied().flatten());
    Ok((out, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::toolpath::{Settings, from_curves};

    fn line_path(points: Vec<P3>, speed: f64) -> Toolpath {
        from_curves(
            &[&Curve::new(points, false)],
            Settings::new(1.0, 0.5, speed).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap()
    }

    #[test]
    fn adjust_scales_speed_and_extrusion_by_zone() {
        let p = line_path(vec![[0.0; 3], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]], 20.0);
        // Half speed and 1.5× extrusion where x > 10 (the second move's middle is at 15).
        let speed = Field::map(
            |v| Ok(if v[0] > 10.0 { 0.5 } else { 1.0 }),
            vec![Rc::new(Field::coordinate(0))],
        )
        .unwrap();
        let extrude = Field::map(
            |v| Ok(if v[0] > 10.0 { 1.5 } else { 1.0 }),
            vec![Rc::new(Field::coordinate(0))],
        )
        .unwrap();
        let a = adjust(&p, &speed, &extrude).unwrap();
        assert_eq!(a.speed, vec![1.0, 1.0, 0.5]);
        assert_eq!(a.flow, vec![0.0, 1.0, 1.5]);
        assert!(adjust(&p, &Field::constant(0.0), &Field::constant(1.0)).is_err());
        assert!(adjust(&p, &Field::constant(1.0), &Field::constant(-1.0)).is_err());
    }

    use std::rc::Rc;

    #[test]
    fn speed_ramps_up_at_the_acceleration() {
        // 10 mm at 10 mm/s, then 40 mm at 30 mm/s: (30² − 10²) / (2 · 100) = 4 mm of ramp.
        let p = line_path(vec![[0.0; 3], [10.0, 0.0, 0.0], [50.0, 0.0, 0.0]], 10.0);
        let p = adjust(
            &p,
            &Field::map(
                |v| Ok(if v[0] > 10.0 { 3.0 } else { 1.0 }),
                vec![Rc::new(Field::coordinate(0))],
            )
            .unwrap(),
            &Field::constant(1.0),
        )
        .unwrap();
        let a = accelerate(&p, 100.0, MAX_PATH_POINTS).unwrap();
        // The second move is cut at 0.5, 1, …, 3.5 mm into the ramp, then runs to its end.
        assert_eq!(a.points.len(), 3 + 7);
        assert!((a.points[2][0] - 10.5).abs() < 1e-9);
        let v = |i: usize| speed_of(&a, i);
        assert!((v(2) - (100.0f64 + 2.0 * 100.0 * 0.5).sqrt()).abs() < 1e-9);
        assert!((v(a.points.len() - 1) - 30.0).abs() < 1e-9);
        assert!((1..a.points.len()).all(|i| v(i) <= 30.0 + 1e-9));
        assert!(accelerate(&p, 0.0, MAX_PATH_POINTS).is_err());
    }

    #[test]
    fn fan_changes_where_the_level_does() {
        let p = line_path(
            vec![
                [0.0; 3],
                [10.0, 0.0, 0.0],
                [20.0, 0.0, 0.0],
                [30.0, 0.0, 0.0],
            ],
            20.0,
        );
        let level = Field::map(
            |v| {
                Ok(if v[0] > 10.0 && v[0] < 20.0 {
                    100.0
                } else {
                    40.0
                })
            },
            vec![Rc::new(Field::coordinate(0))],
        )
        .unwrap();
        let f = fan(&p, &level).unwrap();
        let levels: Vec<(usize, f64)> = f
            .events
            .iter()
            .map(|e| match e.kind {
                EventKind::Fan(l) => (e.after, l),
                _ => (usize::MAX, 0.0),
            })
            .collect();
        assert_eq!(levels, vec![(0, 40.0), (1, 100.0), (2, 40.0)]);
        assert!(fan(&p, &Field::constant(120.0)).is_err());
    }

    #[test]
    fn machine_events_go_before_the_first_print_move_or_after_the_last() {
        use crate::toolpath::{Temperatures, join};
        let p = line_path(vec![[0.0; 3], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]], 20.0);
        let hot = Temperatures {
            nozzle: Some(230.0),
            zones: [None; 3],
        };
        let a = machine_event(&p, EventKind::Heat(hot), true).unwrap();
        let b = machine_event(
            &a,
            EventKind::WaitHeat {
                temps: hot,
                cool: false,
            },
            false,
        )
        .unwrap();
        let c = machine_event(&b, EventKind::SpeedOverride(150.0), true).unwrap();
        let places: Vec<(usize, &str)> = c
            .events
            .iter()
            .map(|e| {
                let what = match e.kind {
                    EventKind::Heat(_) => "heat",
                    EventKind::WaitHeat { .. } => "wait",
                    EventKind::SpeedOverride(_) => "speed",
                    _ => "?",
                };
                (e.after, what)
            })
            .collect();
        // The wait right after the travel to the start; at the end, in the order added.
        assert_eq!(places, vec![(0, "wait"), (2, "heat"), (2, "speed")]);
        // Out of range: refused, with a message.
        let bad = Temperatures {
            nozzle: Some(500.0),
            zones: [None; 3],
        };
        assert!(machine_event(&p, EventKind::Heat(bad), false).is_err());
        assert!(machine_event(&p, EventKind::ExtrusionOverride(0.0), false).is_err());
        // An empty toolpath keeps its event, and `join` puts it where it would have printed.
        let none =
            from_curves(&[], Settings::new(1.0, 0.5, 20.0).unwrap(), MAX_PATH_POINTS).unwrap();
        let marked = machine_event(&none, EventKind::SpeedOverride(100.0), true).unwrap();
        let q = line_path(vec![[30.0, 0.0, 0.0], [40.0, 0.0, 0.0]], 20.0);
        let j = join(&[&p, &marked, &q], 0.0, 0.0, false, MAX_PATH_POINTS).unwrap();
        assert_eq!(j.events.len(), 1);
        assert_eq!(j.events[0].after, 2);
    }

    #[test]
    fn dwells_go_after_the_nearest_move() {
        let p = line_path(vec![[0.0; 3], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]], 20.0);
        let d = dwell(&p, &[[9.0, 1.0, 0.0]], 0.5).unwrap();
        assert_eq!(
            d.events,
            vec![Event {
                after: 1,
                kind: EventKind::Dwell(0.5)
            }]
        );
        assert!(dwell(&p, &[[0.0; 3]], -1.0).is_err());
    }

    #[test]
    fn spikes_leave_the_wall_and_come_back() {
        // A square, counter-clockwise: out is outward.
        let sq = Curve::new(
            vec![
                [0.0, 0.0, 0.0],
                [40.0, 0.0, 0.0],
                [40.0, 40.0, 0.0],
                [0.0, 40.0, 0.0],
            ],
            true,
        );
        let p = from_curves(
            &[&sq],
            Settings::new(1.0, 0.5, 20.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let s = Spike {
            width: 2.0,
            length: 5.0,
            angle: 0.0,
            out_speed: 0.5,
            tip_speed: 0.25,
            in_speed: 0.5,
            dwell: 1.0,
        };
        let (out, made) = spikes(
            &p,
            &[[20.0, -3.0, 0.0], [20.5, 0.0, 0.0]],
            s,
            MAX_PATH_POINTS,
        )
        .unwrap();
        assert_eq!(made, 1, "the second overlaps the first");
        let near = |a: P3, b: P3| dist3(a, b) < 1e-9;
        let i = out
            .points
            .iter()
            .position(|q| near(*q, [19.0, 0.0, 0.5]))
            .unwrap();
        assert!(near(out.points[i + 1], [19.0, -5.0, 0.5]));
        assert!(near(out.points[i + 2], [21.0, -5.0, 0.5]));
        assert!(near(out.points[i + 3], [21.0, 0.0, 0.5]));
        assert_eq!(out.speed[i + 1..=i + 3], [0.5, 0.25, 0.5]);
        assert_eq!(
            out.events,
            vec![Event {
                after: i + 2,
                kind: EventKind::Dwell(1.0)
            }]
        );
        // The rest of the square is all there.
        assert!((out.stats().path_length_mm - (160.0 + 2.0 * 5.0)).abs() < 1e-9);
    }

    /// A cross into a helix: four spokes from where it reaches the height, a quarter turn apart,
    /// flat at the height, turning round near the axis, the helix carrying on unchanged.
    #[test]
    fn spokes_make_a_cross() {
        // Two turns of a 40 mm helix from z 1 to 5, a point every degree.
        let pts: Vec<P3> = (0..=720)
            .map(|d| {
                let a = (d as f64).to_radians();
                [40.0 * a.cos(), 40.0 * a.sin(), 1.0 + 2.0 * d as f64 / 360.0]
            })
            .collect();
        let path = line_path(pts.clone(), 20.0);
        let (out, made) = spokes(&path, 3.0, 4, 4.0, 0.0, None, MAX_PATH_POINTS).unwrap();
        assert_eq!(made, 4);
        // Inside the wall: only the spokes, flat at the height, as near the axis as their turns.
        let inside: Vec<&P3> = out
            .points
            .iter()
            .filter(|p| p[0].hypot(p[1]) < 39.0)
            .collect();
        assert!(
            inside.iter().all(|p| (p[2] - 3.0).abs() < 1e-9),
            "{inside:?}"
        );
        let nearest = inside
            .iter()
            .map(|p| p[0].hypot(p[1]))
            .fold(f64::INFINITY, f64::min);
        assert!((nearest - (5.0 - 2.0)).abs() < 0.05, "{nearest}");
        // Spokes a quarter turn apart: their tips' directions.
        let tips: Vec<f64> = inside
            .iter()
            .filter(|p| (p[0].hypot(p[1]) - nearest).abs() < 0.05)
            .map(|p| p[1].atan2(p[0]).to_degrees().rem_euclid(360.0))
            .collect();
        assert_eq!(tips.len(), 4, "{tips:?}");
        for w in tips.windows(2) {
            assert!(
                ((w[1] - w[0]).rem_euclid(360.0) - 90.0).abs() < 1.0,
                "{tips:?}"
            );
        }
        // The first where the nozzle reaches the height (the toolpath prints its 0.5 mm layer
        // above the curve: z 3 three quarters round, at 270°).
        assert!((tips[0] - 270.0).abs() < 1.0, "{tips:?}");
        // The helix carries on to its end, and no spokes without the height, or with none asked.
        let (end, want) = (out.points[out.points.len() - 1], pts[pts.len() - 1]);
        assert!((0..3).all(|k| (end[k] - want[k] - if k == 2 { 0.5 } else { 0.0 }).abs() < 1e-9));
        assert_eq!(
            spokes(&path, 9.0, 4, 4.0, 0.0, None, MAX_PATH_POINTS)
                .unwrap()
                .1,
            0
        );
        assert_eq!(
            spokes(&path, 3.0, 0, 4.0, 0.0, None, MAX_PATH_POINTS)
                .unwrap()
                .0
                .points
                .len(),
            path.points.len()
        );
    }

    /// With an angle, the first spoke goes where the path next passes it: crosses at two heights
    /// with one angle stack, one over the other.
    #[test]
    fn spokes_at_an_angle_stack() {
        // Three turns of a 40 mm helix from z 1 to 7.
        let pts: Vec<P3> = (0..=1080)
            .map(|d| {
                let a = (d as f64).to_radians();
                [40.0 * a.cos(), 40.0 * a.sin(), 1.0 + 2.0 * d as f64 / 360.0]
            })
            .collect();
        let path = line_path(pts, 20.0);
        let tips = |p: &Toolpath, z: f64| -> Vec<f64> {
            let inside: Vec<&P3> = p
                .points
                .iter()
                .filter(|q| q[0].hypot(q[1]) < 39.0 && (q[2] - z).abs() < 1e-9)
                .collect();
            let nearest = inside
                .iter()
                .map(|q| q[0].hypot(q[1]))
                .fold(f64::INFINITY, f64::min);
            let mut t: Vec<f64> = inside
                .iter()
                .filter(|q| (q[0].hypot(q[1]) - nearest).abs() < 0.05)
                .map(|q| q[1].atan2(q[0]).to_degrees().rem_euclid(360.0))
                .collect();
            t.sort_by(f64::total_cmp);
            t
        };
        let (one, made) = spokes(&path, 3.0, 4, 4.0, 0.0, Some(45.0), MAX_PATH_POINTS).unwrap();
        assert_eq!(made, 4);
        let (two, made) = spokes(&one, 5.0, 4, 4.0, 0.0, Some(45.0), MAX_PATH_POINTS).unwrap();
        assert_eq!(made, 4);
        let (low, high) = (tips(&two, 3.0), tips(&two, 5.0));
        assert_eq!(low.len(), 4, "{low:?}");
        for (a, want) in low.iter().zip([45.0, 135.0, 225.0, 315.0]) {
            assert!((a - want).abs() < 1.0, "{low:?}");
        }
        assert!(
            low.iter().zip(&high).all(|(a, b)| (a - b).abs() < 1.0),
            "{low:?} {high:?}"
        );
    }
}
