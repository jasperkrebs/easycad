//! Curves: profiles, outlines and sections, stored as evenly enough sampled points.
//!
//! Exact NURBS are used where they matter (smoothing through corners, via `curvo`); the result
//! is sampled once, at the quality the evaluation asked for. That keeps transforms, lofts and
//! toolpaths simple and fast, which is what interactive tuning needs.

use crate::geom::{P3, Xform, dist3, lerp3, signed_area_xy};
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
    fn mirroring_keeps_counter_clockwise_travel() {
        let c = Curve::star(5, 10.0, 4.0);
        let m = c.transformed(&Xform::scale(-1.0, 1.0, 1.0));
        assert!(m.signed_area() > 0.0);
        assert_eq!(m.points[0], [-10.0, 0.0, 0.0]);
    }
}
