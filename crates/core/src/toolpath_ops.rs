//! Tuning a finished toolpath, the way EasyGCode tunes its walls: speeds and extrusion that
//! change from zone to zone, speed that ramps up at a set acceleration, fan levels and dwells,
//! and spikes (EasyGCode's staples) pulled out of the wall. Each takes fields, so where things
//! happen is written in the recipe (a height, a band around the seam, a distance from a curve).

use crate::field::Field;
use crate::geom::{P3, dist3, lerp3, sub3, unit3};
use crate::toolpath::{Event, EventKind, MAX_PATH_POINTS, Toolpath};

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
                EventKind::Dwell(_) => (usize::MAX, 0.0),
            })
            .collect();
        assert_eq!(levels, vec![(0, 40.0), (1, 100.0), (2, 40.0)]);
        assert!(fan(&p, &Field::constant(120.0)).is_err());
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
}
