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

#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub points: Vec<P3>,
    /// A closed curve's last point connects back to its first (the first point is not repeated).
    pub closed: bool,
}

impl Curve {
    /// A circle around the origin in the XY plane, starting on +X, counter-clockwise.
    pub fn circle(r: f64, samples: usize) -> Curve {
        let n = samples.max(3);
        let points = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                [r * a.cos(), r * a.sin(), 0.0]
            })
            .collect();
        Curve {
            points,
            closed: true,
        }
    }

    /// A regular polygon with a corner on +X, counter-clockwise. `r` is the corner radius.
    pub fn polygon(sides: usize, r: f64) -> Curve {
        Curve::circle(r, sides)
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
        Curve {
            points,
            closed: true,
        }
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
        Ok(Curve {
            points,
            closed: self.closed,
        })
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
            return Curve {
                points: vec![p; n],
                closed: self.closed,
            };
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
        Curve {
            points: out,
            closed: self.closed,
        }
    }

    pub fn transformed(&self, x: &Xform) -> Curve {
        let mut points: Vec<P3> = self.points.iter().map(|p| x.apply(*p)).collect();
        if x.mirrors() && self.closed {
            // Keep the seam on the same point but restore counter-clockwise travel.
            if let Some(rest) = points.get_mut(1..) {
                rest.reverse();
            }
        }
        Curve {
            points,
            closed: self.closed,
        }
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
        let dot = Curve {
            points: vec![[1.0, 1.0, 0.0]; 5],
            closed: true,
        };
        assert_eq!(dot.smooth(8).unwrap(), dot);
        let line = Curve {
            points: vec![[0.0; 3], [1.0, 0.0, 0.0]],
            closed: false,
        };
        assert_eq!(line.smooth(8).unwrap(), line);
        let tri = Curve {
            points: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            closed: true,
        };
        assert!(tri.smooth(8).is_ok());
        let collinear = Curve {
            points: (0..6).map(|i| [i as f64, 0.0, 0.0]).collect(),
            closed: false,
        };
        assert!(collinear.smooth(4).is_ok());
        let nan = Curve {
            points: vec![
                [f64::NAN; 3],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 2.0, 0.0],
            ],
            closed: true,
        };
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
        let open = Curve {
            points: vec![[0.0; 3], [10.0, 0.0, 0.0]],
            closed: false,
        };
        let r = open.resample(11);
        assert!(dist3(r.points[5], [5.0, 0.0, 0.0]) < 1e-12);
        assert!(dist3(r.points[10], [10.0, 0.0, 0.0]) < 1e-12);
    }

    #[test]
    fn mirroring_keeps_counter_clockwise_travel() {
        let c = Curve::star(5, 10.0, 4.0);
        let m = c.transformed(&Xform::scale(-1.0, 1.0, 1.0));
        assert!(m.signed_area() > 0.0);
        assert_eq!(m.points[0], [-10.0, 0.0, 0.0]);
    }
}
