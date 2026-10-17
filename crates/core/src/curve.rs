//! Curves: profiles, outlines and sections, stored as evenly enough sampled points.
//!
//! Exact NURBS are used where they matter (smoothing through corners, via `curvo`); the result
//! is sampled once, at the quality the evaluation asked for. That keeps transforms, lofts and
//! toolpaths simple and fast, which is what interactive tuning needs.

use crate::geom::{P3, Xform, dist3, lerp3, signed_area_xy, unit3};
use curvo::prelude::*;
use nalgebra::Point3;

/// The most points `smooth` will fit a curve through (the fit solves a dense system).
pub const MAX_SMOOTH_POINTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Curve {
    pub points: Vec<P3>,
    /// A closed curve's last point connects back to its first (the first point is not repeated).
    pub closed: bool,
    /// Set while the curve is exactly a circle in a horizontal plane, so distances to it can be
    /// computed exactly (and fast) instead of from its sample points.
    pub circle: Option<Circle>,
    /// Set when the curve winds around a centre, like a spiral: where each point is, in turns.
    /// Waves and interlocking use it to line up one turn with the next.
    pub winding: Option<Winding>,
}

/// An exact circle: centre (x, y) and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: [f64; 2],
    pub r: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Winding {
    /// Each point's position around `center` in turns (angle ÷ 360°, counted from the start).
    pub turn: Vec<f64>,
    /// How many points make one turn; the points are evenly spaced in angle.
    pub per_turn: usize,
    pub center: [f64; 2],
}

impl Curve {
    pub fn new(points: Vec<P3>, closed: bool) -> Curve {
        Curve {
            points,
            closed,
            circle: None,
            winding: None,
        }
    }

    /// A circle around the origin in the XY plane, starting on +X, counter-clockwise.
    pub fn circle(r: f64, samples: usize) -> Curve {
        Curve {
            circle: Some(Circle {
                center: [0.0; 2],
                r,
            }),
            ..Curve::new(ring(r, samples.max(3)), true)
        }
    }

    /// A regular polygon with a corner on +X, counter-clockwise. `r` is the corner radius.
    pub fn polygon(sides: usize, r: f64) -> Curve {
        Curve::new(ring(r, sides.max(3)), true)
    }

    /// A star's corners, alternating tip (`outer`) and inner corner (`inner`), counter-clockwise
    /// from a tip on +X.
    pub fn star(tips: usize, outer: f64, inner: f64) -> Curve {
        let points = (0..2 * tips)
            .map(|i| {
                let a = i as f64 * std::f64::consts::PI / tips as f64;
                let r = if i % 2 == 0 { outer } else { inner };
                [r * a.cos(), r * a.sin(), 0.0]
            })
            .collect();
        Curve::new(points, true)
    }

    /// A smooth curve through every point (a cubic NURBS interpolation, periodic for closed
    /// curves), sampled `per_point` times per point, evenly in curve parameter.
    pub fn smooth(&self, per_point: usize) -> Result<Curve, String> {
        let pts = self.distinct_points();
        if pts.len() < 3 {
            return Ok(self.clone()); // two points make a line; nothing to round
        }
        if pts.len() > MAX_SMOOTH_POINTS {
            return Err(format!(
                "can only smooth through up to {MAX_SMOOTH_POINTS} points, this curve has {} (it is probably smooth already)",
                pts.len()
            ));
        }
        let degree = if pts.len() >= 4 { 3 } else { 2 };
        let input: Vec<Point3<f64>> = pts.iter().map(|p| Point3::new(p[0], p[1], p[2])).collect();
        let fitted = if self.closed {
            NurbsCurve3D::<f64>::interpolate_periodic(&input, degree, KnotStyle::Centripetal)
        } else {
            NurbsCurve3D::<f64>::interpolate(&input, degree)
        }
        .map_err(|e| format!("could not fit a smooth curve through these points ({e})"))?;

        let (start, end) = fitted.knots_domain();
        let per_point = per_point.max(1);
        let (count, last) = if self.closed {
            (pts.len() * per_point, pts.len() * per_point)
        } else {
            ((pts.len() - 1) * per_point + 1, (pts.len() - 1) * per_point)
        };
        let points: Vec<P3> = (0..count)
            .map(|j| {
                let q = fitted.point_at(start + (end - start) * j as f64 / last as f64);
                [q.x, q.y, q.z]
            })
            .collect();
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return Err("could not fit a smooth curve through these points".into());
        }
        Ok(Curve::new(points, self.closed))
    }

    /// The points without repeats: consecutive points closer than a micrometre are merged, and
    /// a closed curve's last point is dropped if it equals the first.
    fn distinct_points(&self) -> Vec<P3> {
        let mut out: Vec<P3> = Vec::with_capacity(self.points.len());
        for p in &self.points {
            if out.last().is_none_or(|q| dist3(*q, *p) > 1e-6) {
                out.push(*p);
            }
        }
        if self.closed
            && out.len() > 1
            && out
                .first()
                .zip(out.last())
                .is_some_and(|(a, b)| dist3(*a, *b) <= 1e-6)
        {
            out.pop();
        }
        out
    }

    /// Total length (including the closing segment of a closed curve).
    pub fn length(&self) -> f64 {
        self.segments().map(|(a, b)| dist3(a, b)).sum()
    }

    /// Each segment as (start, end), including the closing segment of a closed curve.
    pub fn segments(&self) -> impl Iterator<Item = (P3, P3)> + '_ {
        let closing = if self.closed {
            self.points.last().zip(self.points.first())
        } else {
            None
        };
        self.points
            .windows(2)
            .map(|w| (w[0], w[1]))
            .chain(closing.map(|(a, b)| (*a, *b)))
    }

    /// `n` points spaced evenly along the curve, starting at its first point.
    pub fn resample(&self, n: usize) -> Curve {
        let n = n.max(2);
        let total = self.length();
        if total <= 0.0 || self.points.len() < 2 {
            let p = self.points.first().copied().unwrap_or([0.0; 3]);
            return Curve::new(vec![p; n], self.closed);
        }
        // A closed curve's n points split it into n equal pieces; an open one's into n − 1.
        let pieces = if self.closed { n } else { n - 1 };
        let mut out = Vec::with_capacity(n);
        let mut segs = self.segments();
        let mut seg = segs.next();
        let mut walked = 0.0; // length before the current segment
        for i in 0..n {
            let target = total * i as f64 / pieces as f64;
            loop {
                let Some((a, b)) = seg else {
                    // Rounding left us just short of the end.
                    out.push(self.points.last().copied().unwrap_or([0.0; 3]));
                    break;
                };
                let len = dist3(a, b);
                if walked + len >= target {
                    let t = if len > 0.0 {
                        ((target - walked) / len).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    out.push(lerp3(a, b, t));
                    break;
                }
                walked += len;
                seg = segs.next();
            }
        }
        Curve::new(out, self.closed)
    }

    /// The curve moved, turned or scaled. It stays an exact circle under moves, turns, mirrors
    /// and uniform scaling, and keeps its winding (with the centre moved along).
    pub fn transformed(&self, x: &Xform) -> Curve {
        let mut points: Vec<P3> = self.points.iter().map(|p| x.apply(*p)).collect();
        let mirrored = x.mirrors() && self.closed;
        if mirrored {
            // Keep the seam on the same point but restore counter-clockwise travel.
            if let Some(rest) = points.get_mut(1..) {
                rest.reverse();
            }
        }
        let moved = |c: [f64; 2]| {
            let p = x.apply([c[0], c[1], 0.0]);
            [p[0], p[1]]
        };
        let circle = match (self.circle, x.xy_similarity()) {
            (Some(c), Some(s)) => Some(Circle {
                center: moved(c.center),
                r: c.r * s,
            }),
            _ => None,
        };
        let winding = if mirrored {
            None
        } else {
            self.winding.as_ref().map(|w| Winding {
                center: moved(w.center),
                ..w.clone()
            })
        };
        Curve {
            points,
            closed: self.closed,
            circle,
            winding,
        }
    }

    /// How many turns a winding curve makes (0 for other curves).
    pub fn turns(&self) -> f64 {
        self.winding
            .as_ref()
            .and_then(|w| w.turn.last().copied())
            .unwrap_or(0.0)
    }

    /// Positive when the curve runs counter-clockwise seen from above.
    pub fn signed_area(&self) -> f64 {
        signed_area_xy(&self.points)
    }

    /// Average height of the points.
    pub fn mean_z(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        self.points.iter().map(|p| p[2]).sum::<f64>() / self.points.len() as f64
    }

    /// A rectangle `width` × `height` around the origin, counter-clockwise, starting in the
    /// middle of its +X side. With `radius` > 0 the corners are rounded (`per_corner` points
    /// each).
    pub fn rectangle(width: f64, height: f64, radius: f64, per_corner: usize) -> Curve {
        let (hw, hh) = (width / 2.0, height / 2.0);
        let r = radius.clamp(0.0, hw.min(hh));
        let mut points = vec![[hw, 0.0, 0.0]];
        // Corner centres, counter-clockwise from the top right, with the angle each arc starts at.
        let corners = [
            ([hw - r, hh - r], 0.0),
            ([-hw + r, hh - r], 90.0),
            ([-hw + r, -hh + r], 180.0),
            ([hw - r, -hh + r], 270.0),
        ];
        for (c, start) in corners {
            if r <= 0.0 {
                points.push([c[0], c[1], 0.0]);
            } else {
                let n = per_corner.max(2);
                for i in 0..=n {
                    let a = (start + 90.0 * i as f64 / n as f64).to_radians();
                    points.push([c[0] + r * a.cos(), c[1] + r * a.sin(), 0.0]);
                }
            }
        }
        let mut c = Curve::new(points, true);
        c.points.dedup_by(|a, b| dist3(*a, *b) < 1e-9);
        c
    }

    /// An ellipse around the origin with radii `rx` and `ry`, counter-clockwise from +X.
    pub fn ellipse(rx: f64, ry: f64, samples: usize) -> Curve {
        let points = ring(1.0, samples.max(3))
            .into_iter()
            .map(|p| [p[0] * rx, p[1] * ry, 0.0])
            .collect();
        Curve::new(points, true)
    }

    /// An open arc of radius `r` around the origin from `start` to `end` degrees
    /// (counter-clockwise when `end` > `start`).
    pub fn arc(r: f64, start: f64, end: f64, samples: usize) -> Curve {
        let n = samples.max(2);
        let points = (0..n)
            .map(|i| {
                let a = (start + (end - start) * i as f64 / (n - 1) as f64).to_radians();
                [r * a.cos(), r * a.sin(), 0.0]
            })
            .collect();
        Curve::new(points, false)
    }

    /// An arc from `start` to `end` (degrees) with as many points as keep its chords within
    /// `tolerance` of the true arc, at least 12 segments: EasyGCode's sampling of its bend
    /// stool's rings, whose arithmetic (in radians) this follows.
    pub fn arc_within(
        r: f64,
        start: f64,
        end: f64,
        tolerance: f64,
        max_points: usize,
    ) -> Result<Curve, String> {
        let a = start.to_radians();
        let sweep = (end - start).to_radians();
        let mut dth = 2.0 * (1.0 - tolerance / r.max(1e-9)).max(-1.0).acos();
        if !dth.is_finite() || dth <= 0.0 {
            dth = 0.2;
        }
        let steps = (sweep.abs() / dth).ceil();
        if steps.is_nan() || steps >= max_points as f64 {
            return Err(format!(
                "a tolerance of {tolerance} mm needs too many points on this arc; use a larger tolerance"
            ));
        }
        let steps = (steps as usize).max(12);
        let points = (0..=steps)
            .map(|s| {
                let angle = a + (sweep * s as f64) / steps as f64;
                [r * angle.cos(), r * angle.sin(), 0.0]
            })
            .collect();
        Ok(Curve::new(points, false))
    }

    /// The point a fraction `t` (0..1) of the way along the curve, by length. A closed curve
    /// wraps around (1 is back at the start).
    pub fn point_at(&self, t: f64) -> P3 {
        let total = self.length();
        let first = self.points.first().copied().unwrap_or([0.0; 3]);
        if total <= 0.0 || !t.is_finite() {
            return first;
        }
        let t = if self.closed {
            t.rem_euclid(1.0)
        } else {
            t.clamp(0.0, 1.0)
        };
        // An open curve's ends exactly, without rounding drift from walking along it.
        if !self.closed && t >= 1.0 {
            return self.points.last().copied().unwrap_or(first);
        }
        let target = total * t;
        let mut walked = 0.0;
        for (a, b) in self.segments() {
            let len = dist3(a, b);
            if walked + len >= target {
                return if len > 0.0 {
                    lerp3(a, b, ((target - walked) / len).clamp(0.0, 1.0))
                } else {
                    a
                };
            }
            walked += len;
        }
        if self.closed {
            first
        } else {
            self.points.last().copied().unwrap_or(first)
        }
    }

    /// Points dividing the curve into `count` pieces of equal length (Grasshopper's Divide
    /// Curve): `count` + 1 points on an open curve, `count` on a closed one.
    pub fn divide(&self, count: usize) -> Vec<P3> {
        let count = count.max(1);
        self.resample(if self.closed { count } else { count + 1 })
            .points
    }

    /// The same curve run the other way. A closed curve keeps its start point.
    pub fn flipped(&self) -> Curve {
        let mut points = self.points.clone();
        if self.closed {
            if let Some(rest) = points.get_mut(1..) {
                rest.reverse();
            }
        } else {
            points.reverse();
        }
        Curve::new(points, self.closed)
    }

    /// A closed curve starting a fraction `t` of the way along it (Grasshopper's Seam). The new
    /// start point is added exactly where `t` falls.
    pub fn with_seam(&self, t: f64) -> Result<Curve, String> {
        if !self.closed {
            return Err("only a closed curve has a seam to move".into());
        }
        let total = self.length();
        if total <= 0.0 || self.points.len() < 3 {
            return Ok(Curve::new(self.points.clone(), true));
        }
        let target = total * t.rem_euclid(1.0);
        let n = self.points.len();
        let mut walked = 0.0;
        for i in 0..n {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            let len = dist3(a, b);
            if walked + len > target || i == n - 1 {
                let f = if len > 0.0 {
                    ((target - walked) / len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let start = lerp3(a, b, f);
                let mut points = vec![start];
                // Then every original point after the split, wrapping round to the split again.
                for k in 1..=n {
                    let p = self.points[(i + k) % n];
                    if dist3(p, start) > 1e-9 && !(k == n && dist3(p, start) <= 1e-9) {
                        points.push(p);
                    }
                }
                points.dedup_by(|x, y| dist3(*x, *y) < 1e-9);
                if points.len() > 1 && points.last().is_some_and(|q| dist3(*q, start) < 1e-9) {
                    points.pop();
                }
                return Ok(Curve::new(points, true));
            }
            walked += len;
        }
        Ok(self.clone())
    }

    /// Between `a` (t = 0) and `b` (t = 1), point by point (Grasshopper's Tween Curve). Curves
    /// with different point counts are first resampled evenly to the larger count.
    pub fn tween(a: &Curve, b: &Curve, t: f64) -> Result<Curve, String> {
        if a.closed != b.closed {
            return Err("both curves must be closed, or both open".into());
        }
        if a.points.is_empty() || b.points.is_empty() {
            return Err("both curves need points".into());
        }
        let n = a.points.len().max(b.points.len());
        let (ra, rb);
        let pa = if a.points.len() == n {
            &a.points
        } else {
            ra = a.resample(n);
            &ra.points
        };
        let pb = if b.points.len() == n {
            &b.points
        } else {
            rb = b.resample(n);
            &rb.points
        };
        let points = pa.iter().zip(pb).map(|(p, q)| lerp3(*p, *q, t)).collect();
        // Spirals stay spirals: both ends wind the same way, point for point.
        let winding = match (&a.winding, &b.winding) {
            (Some(w), Some(_)) if a.points.len() == n && b.points.len() == n => Some(w.clone()),
            _ => None,
        };
        Ok(Curve {
            points,
            closed: a.closed,
            circle: None,
            winding,
        })
    }

    /// The curve moved sideways by `distance` in the XY plane: outward for a closed curve,
    /// to the right of travel for an open one (negative goes the other way). Corners are
    /// mitred, which is right for smooth curves and modest distances; a distance larger than a
    /// curve's tightest bend makes it fold over.
    pub fn offset(&self, distance: f64) -> Curve {
        if distance == 0.0 {
            return self.clone();
        }
        self.offset_by(&|_| distance)
    }

    /// [`Curve::offset`] by a different distance at each point (`distance(point)`, read where
    /// the point is before it moves).
    pub fn offset_by(&self, distance: &dyn Fn(P3) -> f64) -> Curve {
        let n = self.points.len();
        if n < 2 {
            return self.clone();
        }
        let sign = if self.closed && self.signed_area() < 0.0 {
            -1.0
        } else {
            1.0
        };
        // The right-hand normal of each segment, in XY.
        let normal = |a: P3, b: P3| unit3([b[1] - a[1], -(b[0] - a[0]), 0.0]);
        let segs: Vec<Option<P3>> = (0..n)
            .map(|i| {
                if self.closed || i + 1 < n {
                    normal(self.points[i], self.points[(i + 1) % n])
                } else {
                    None
                }
            })
            .collect();
        let points = (0..n)
            .map(|i| {
                let p = self.points[i];
                let before = if i > 0 {
                    segs[i - 1]
                } else if self.closed {
                    segs[n - 1]
                } else {
                    None
                };
                let after = segs.get(i).copied().flatten();
                let d = sign * distance(p);
                match (before, after) {
                    (Some(a), Some(b)) => match unit3([a[0] + b[0], a[1] + b[1], 0.0]) {
                        Some(m) => {
                            // Mitre: as far as needed to keep both sides at `distance`, capped.
                            let k = (m[0] * a[0] + m[1] * a[1]).max(0.25);
                            [p[0] + m[0] * d / k, p[1] + m[1] * d / k, p[2]]
                        }
                        None => [p[0] + a[0] * d, p[1] + a[1] * d, p[2]],
                    },
                    (Some(a), None) | (None, Some(a)) => [p[0] + a[0] * d, p[1] + a[1] * d, p[2]],
                    (None, None) => p,
                }
            })
            .collect();
        Curve::new(points, self.closed)
    }
}

/// `n` points evenly round a circle of radius `r`, the first on +X.
fn ring(r: f64, n: usize) -> Vec<P3> {
    (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [r * a.cos(), r * a.sin(), 0.0]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arcs_within_a_tolerance_follow_easygcodes_chord_rule() {
        // r = 5, tolerance 0.05: each chord spans 2 acos(1 - 0.01) of arc, so a ring that stops
        // a 2 mm line short of a full turn (2 pi - 0.4) needs ceil(5.883 / 0.2838) = 21 chords.
        let sweep = 360.0 - (2.0f64 / 5.0).to_degrees();
        let a = Curve::arc_within(5.0, 90.0, 90.0 + sweep, 0.05, 10_000).unwrap();
        assert_eq!(a.points.len(), 22);
        let first = a.points[0];
        assert!((first[0]).abs() < 1e-12 && (first[1] - 5.0).abs() < 1e-12);
        // Every chord's middle stays within the tolerance of the circle.
        for w in a.points.windows(2) {
            let m = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            assert!(5.0 - m[0].hypot(m[1]) <= 0.05 + 1e-12);
        }
        // At least 12 chords, however loose the tolerance.
        assert_eq!(
            Curve::arc_within(5.0, 0.0, 10.0, 1.0, 10_000)
                .unwrap()
                .points
                .len(),
            13
        );
        assert!(Curve::arc_within(500.0, 0.0, 360.0, 1e-9, 1000).is_err());
    }

    #[test]
    fn circle_is_round_and_closed() {
        let c = Curve::circle(10.0, 360);
        assert_eq!(c.points.len(), 360);
        assert!(c.closed);
        assert!((c.length() - 20.0 * std::f64::consts::PI).abs() < 0.01);
        assert!(c.signed_area() > 0.0);
    }

    #[test]
    fn star_alternates_tips_and_inner_corners() {
        let s = Curve::star(5, 10.0, 4.0);
        assert_eq!(s.points.len(), 10);
        let r = |p: P3| p[0].hypot(p[1]);
        assert!((r(s.points[0]) - 10.0).abs() < 1e-12);
        assert!((r(s.points[1]) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn smooth_passes_through_the_corners() {
        let s = Curve::star(7, 45.0, 36.9);
        let sm = s.smooth(16).unwrap();
        assert_eq!(sm.points.len(), 14 * 16);
        // Every 16th sample is near a corner (centripetal parameters are not exactly uniform,
        // but the first sample is exactly the first corner).
        assert!(dist3(sm.points[0], s.points[0]) < 1e-9);
        let max_r = sm
            .points
            .iter()
            .map(|p| p[0].hypot(p[1]))
            .fold(0.0, f64::max);
        assert!(max_r < 45.5, "{max_r}");
    }

    #[test]
    fn smooth_handles_degenerate_input() {
        let dot = Curve::new(vec![[1.0, 1.0, 0.0]; 5], true);
        assert_eq!(dot.smooth(8).unwrap(), dot);
        let line = Curve::new(vec![[0.0; 3], [1.0, 0.0, 0.0]], false);
        assert_eq!(line.smooth(8).unwrap(), line);
        let tri = Curve::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], true);
        assert!(tri.smooth(8).is_ok());
        let collinear = Curve::new((0..6).map(|i| [i as f64, 0.0, 0.0]).collect(), false);
        assert!(collinear.smooth(4).is_ok());
        let nan = Curve::new(
            vec![
                [f64::NAN; 3],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 2.0, 0.0],
            ],
            true,
        );
        let _ = nan.smooth(4); // an error or a curve, never a crash
        let big = Curve::circle(10.0, MAX_SMOOTH_POINTS + 1);
        assert!(big.smooth(2).is_err());
    }

    #[test]
    fn resample_spaces_points_evenly() {
        let sq = Curve::polygon(4, 10.0);
        let r = sq.resample(40);
        assert_eq!(r.points.len(), 40);
        let lens: Vec<f64> = r.segments().map(|(a, b)| dist3(a, b)).collect();
        let (lo, hi) = lens
            .iter()
            .fold((f64::MAX, 0.0f64), |(lo, hi), l| (lo.min(*l), hi.max(*l)));
        // Corners get cut, so pieces vary a little.
        assert!(hi / lo < 1.5, "{lo} {hi}");
        let open = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0]], false);
        let r = open.resample(11);
        assert!(dist3(r.points[5], [5.0, 0.0, 0.0]) < 1e-12);
        assert!(dist3(r.points[10], [10.0, 0.0, 0.0]) < 1e-12);
    }

    #[test]
    fn circles_stay_exact_under_moves_turns_and_uniform_scaling() {
        let c = Curve::circle(10.0, 64);
        let moved = c
            .transformed(&Xform::translate(5.0, -2.0, 3.0))
            .transformed(&Xform::rotate_z(90.0));
        let exact = moved.circle.unwrap();
        assert!((exact.center[0] - 2.0).abs() < 1e-12 && (exact.center[1] - 5.0).abs() < 1e-12);
        assert_eq!(exact.r, 10.0);
        assert_eq!(
            c.transformed(&Xform::scale(3.0, 3.0, 1.0))
                .circle
                .unwrap()
                .r,
            30.0
        );
        assert!(c.transformed(&Xform::scale(3.0, 1.0, 1.0)).circle.is_none());
        assert!(Curve::polygon(6, 10.0).circle.is_none());
        assert!(c.smooth(2).is_err() || c.smooth(2).unwrap().circle.is_none());
    }

    #[test]
    fn rectangles_ellipses_and_arcs() {
        let r = Curve::rectangle(40.0, 20.0, 0.0, 4);
        assert_eq!(
            r.points,
            vec![
                [20.0, 0.0, 0.0],
                [20.0, 10.0, 0.0],
                [-20.0, 10.0, 0.0],
                [-20.0, -10.0, 0.0],
                [20.0, -10.0, 0.0]
            ]
        );
        assert!((r.length() - 120.0).abs() < 1e-9 && r.signed_area() > 0.0);
        let rounded = Curve::rectangle(40.0, 20.0, 5.0, 8);
        let expected = 2.0 * (30.0 + 10.0) + std::f64::consts::TAU * 5.0;
        assert!(
            (rounded.length() - expected).abs() < 0.1,
            "{}",
            rounded.length()
        );
        let e = Curve::ellipse(10.0, 5.0, 64);
        assert_eq!(e.points[16], [e.points[16][0], 5.0, 0.0]);
        let a = Curve::arc(10.0, 0.0, 90.0, 10);
        assert!(!a.closed && dist3(a.points[9], [0.0, 10.0, 0.0]) < 1e-12);
    }

    #[test]
    fn points_along_seams_and_direction() {
        let sq = Curve::polygon(4, 10.0); // corners on the axes
        let side = dist3(sq.points[0], sq.points[1]);
        assert!(dist3(sq.point_at(0.125), lerp3(sq.points[0], sq.points[1], 0.5)) < 1e-9);
        assert_eq!(sq.point_at(1.0), sq.points[0]);
        assert_eq!(sq.divide(4).len(), 4);
        let line = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0]], false);
        assert_eq!(line.divide(5).len(), 6);
        let seamed = sq.with_seam(0.125).unwrap();
        assert_eq!(seamed.points.len(), 5);
        assert!(dist3(seamed.points[0], sq.point_at(0.125)) < 1e-9);
        assert!((seamed.length() - 4.0 * side).abs() < 1e-9);
        assert!(line.with_seam(0.5).is_err());
        let f = sq.flipped();
        assert_eq!(f.points[0], sq.points[0]);
        assert!(f.signed_area() < 0.0);
        assert_eq!(line.flipped().points[0], [10.0, 0.0, 0.0]);
    }

    #[test]
    fn tweens_blend_point_by_point() {
        let a = Curve::circle(10.0, 32);
        let b = Curve::circle(20.0, 32).transformed(&Xform::translate(0.0, 0.0, 10.0));
        let mid = Curve::tween(&a, &b, 0.5).unwrap();
        assert!(dist3(mid.points[0], [15.0, 0.0, 5.0]) < 1e-12);
        let sq = Curve::polygon(4, 10.0);
        assert_eq!(Curve::tween(&a, &sq, 0.5).unwrap().points.len(), 32);
        let open = Curve::new(vec![[0.0; 3], [1.0, 0.0, 0.0]], false);
        assert!(Curve::tween(&a, &open, 0.5).is_err());
    }

    #[test]
    fn offsets_go_outward() {
        let c = Curve::circle(10.0, 256);
        let out = c.offset(2.0);
        assert!(
            out.points
                .iter()
                .all(|p| (p[0].hypot(p[1]) - 12.0).abs() < 0.01)
        );
        let inward = c.flipped().offset(2.0); // clockwise: still outward
        assert!(
            inward
                .points
                .iter()
                .all(|p| (p[0].hypot(p[1]) - 12.0).abs() < 0.01)
        );
        let sq = Curve::new(
            vec![
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        );
        let big = sq.offset(1.0);
        assert!(
            dist3(big.points[0], [-1.0, -1.0, 0.0]) < 1e-9,
            "{:?}",
            big.points[0]
        );
        let line = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0]], false);
        assert!(dist3(line.offset(1.0).points[0], [0.0, -1.0, 0.0]) < 1e-12);
    }

    #[test]
    fn mirroring_keeps_counter_clockwise_travel() {
        let c = Curve::star(5, 10.0, 4.0);
        let m = c.transformed(&Xform::scale(-1.0, 1.0, 1.0));
        assert!(m.signed_area() > 0.0);
        assert_eq!(m.points[0], [-10.0, 0.0, 0.0]);
    }
}
