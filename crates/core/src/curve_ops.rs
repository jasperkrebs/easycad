//! More curve tools, the ones EasyGCode's projects are drawn with: corners rounded with arcs,
//! curves simplified to a tolerance, splines through points, pieces cut out and joined, points
//! and directions found along a curve, squircles. Each is general, so any project can use it.

use crate::curve::Curve;
use crate::geom::{Bounds, P3, add3, dist3, dot3, lerp3, norm3, scale3, sub3, unit3};

impl Curve {
    /// Position along the curve (0..1 by length) of the point closest to `p`, and that
    /// point. Ties go to the earliest.
    pub fn closest(&self, p: P3) -> (f64, P3) {
        let total = self.length();
        let first = self.points.first().copied().unwrap_or(p);
        if total <= 0.0 {
            return (0.0, first);
        }
        let (mut best, mut best_t, mut best_p) = (f64::INFINITY, 0.0, first);
        let mut walked = 0.0;
        for (a, b) in self.segments() {
            let ab = sub3(b, a);
            let len2 = dot3(ab, ab);
            let f = if len2 > 0.0 {
                (dot3(sub3(p, a), ab) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = lerp3(a, b, f);
            let d = dist3(p, q);
            let len = len2.sqrt();
            if d < best - 1e-12 {
                (best, best_t, best_p) = (d, (walked + f * len) / total, q);
            }
            walked += len;
        }
        (best_t.clamp(0.0, 1.0), best_p)
    }

    /// The unit direction of travel a fraction `t` of the way along the curve (by length): the
    /// direction of the segment it falls on.
    pub fn tangent_at(&self, t: f64) -> Option<P3> {
        let total = self.length();
        if total <= 0.0 || !t.is_finite() {
            return None;
        }
        let t = if self.closed {
            t.rem_euclid(1.0)
        } else {
            t.clamp(0.0, 1.0)
        };
        let target = total * t;
        let mut walked = 0.0;
        let mut last = None;
        for (a, b) in self.segments() {
            let len = dist3(a, b);
            if len <= 0.0 {
                continue;
            }
            last = unit3(sub3(b, a));
            if walked + len >= target {
                return last;
            }
            walked += len;
        }
        last
    }

    /// The direction 90° to the right of travel in plan, a fraction `t` of the way along: for a
    /// closed curve always outward (whichever way it runs), like [`Curve::offset`].
    pub fn normal_at(&self, t: f64) -> Option<P3> {
        let d = self.tangent_at(t)?;
        let sign = if self.closed && self.signed_area() < 0.0 {
            -1.0
        } else {
            1.0
        };
        unit3([sign * d[1], -sign * d[0], 0.0])
    }

    /// The area centroid of a closed curve in plan (at its mean height); for an open curve, or
    /// one that encloses no area, the centre of its length.
    pub fn centroid(&self) -> Option<P3> {
        let first = self.points.first().copied()?;
        if self.closed && self.points.len() >= 3 {
            let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
            for (p, q) in self.segments() {
                // Relative to the first point, which keeps far-away curves precise.
                let (x0, y0) = (p[0] - first[0], p[1] - first[1]);
                let (x1, y1) = (q[0] - first[0], q[1] - first[1]);
                let cross = x0 * y1 - x1 * y0;
                a += cross;
                cx += (x0 + x1) * cross;
                cy += (y0 + y1) * cross;
            }
            if a.abs() > 1e-12 {
                return Some([
                    first[0] + cx / (3.0 * a),
                    first[1] + cy / (3.0 * a),
                    self.mean_z(),
                ]);
            }
        }
        let total = self.length();
        if total <= 0.0 {
            return Some(first);
        }
        let mut sum = [0.0; 3];
        for (a, b) in self.segments() {
            sum = add3(sum, scale3(lerp3(a, b, 0.5), dist3(a, b)));
        }
        Some(scale3(sum, 1.0 / total))
    }

    /// The piece between fractions `t0` and `t1` of the way along (by length), as an open
    /// curve. On a closed curve, `t1` before `t0` goes on through the seam.
    pub fn subcurve(&self, t0: f64, t1: f64) -> Result<Curve, String> {
        if !(t0.is_finite() && t1.is_finite()) {
            return Err("`start` and `end` must be numbers".into());
        }
        let total = self.length();
        if total <= 0.0 || self.points.len() < 2 {
            return Err("the curve has no length".into());
        }
        let (a, b) = if self.closed {
            let a = t0.rem_euclid(1.0);
            let mut b = t1.rem_euclid(1.0);
            if b <= a + 1e-12 && (t1 - t0).abs() > 1e-12 {
                b += 1.0;
            }
            (a, b)
        } else {
            (t0.clamp(0.0, 1.0), t1.clamp(0.0, 1.0))
        };
        if (b - a).abs() <= 1e-12 {
            return Err("`start` and `end` are the same place, so the piece has no length".into());
        }
        let forward = b > a;
        let (lo, hi) = if forward { (a, b) } else { (b, a) };
        // The polyline's vertices by their position along it (twice round for closed curves).
        let mut stations: Vec<(f64, P3)> = Vec::new();
        let laps = if self.closed { 2 } else { 1 };
        let mut walked = 0.0;
        for lap in 0..laps {
            for (p, q) in self.segments() {
                if lap == 0 && stations.is_empty() {
                    stations.push((0.0, p));
                }
                walked += dist3(p, q);
                stations.push((walked / total, q));
            }
        }
        let at = |t: f64| -> P3 {
            let i = stations.partition_point(|s| s.0 < t);
            match (
                i.checked_sub(1).and_then(|k| stations.get(k)),
                stations.get(i),
            ) {
                (Some(s0), Some(s1)) if s1.0 > s0.0 => {
                    lerp3(s0.1, s1.1, ((t - s0.0) / (s1.0 - s0.0)).clamp(0.0, 1.0))
                }
                (_, Some(s1)) => s1.1,
                (Some(s0), None) => s0.1,
                (None, None) => [0.0; 3],
            }
        };
        let mut points = vec![at(lo)];
        points.extend(
            stations
                .iter()
                .filter(|s| s.0 > lo + 1e-12 && s.0 < hi - 1e-12)
                .map(|s| s.1),
        );
        points.push(at(hi));
        points.dedup_by(|x, y| dist3(*x, *y) < 1e-9);
        if !forward {
            points.reverse();
        }
        Ok(Curve::new(points, false))
    }

    /// The curve with points that change its course by less than `tolerance` removed
    /// (Ramer–Douglas–Peucker): every removed point is within `tolerance` of the result.
    pub fn simplified(&self, tolerance: f64) -> Curve {
        let n = self.points.len();
        if n < 3 || tolerance <= 0.0 {
            return self.clone();
        }
        let mut keep = vec![false; n];
        if self.closed {
            // Split at the point farthest from the first, then simplify both halves.
            let far = (1..n)
                .max_by(|&i, &j| {
                    dist3(self.points[i], self.points[0])
                        .total_cmp(&dist3(self.points[j], self.points[0]))
                })
                .unwrap_or(n / 2);
            let mut ring = self.points.clone();
            ring.push(self.points[0]);
            keep[0] = true;
            keep[far] = true;
            rdp(&ring, 0, far, tolerance, &mut keep);
            let mut tail = vec![false; n + 1];
            rdp(&ring, far, n, tolerance, &mut tail);
            for (k, t) in keep.iter_mut().zip(tail) {
                *k |= t;
            }
        } else {
            keep[0] = true;
            keep[n - 1] = true;
            rdp(&self.points, 0, n - 1, tolerance, &mut keep);
        }
        let points = self
            .points
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(p, _)| *p)
            .collect();
        Curve::new(points, self.closed)
    }

    /// Every corner rounded with a circular arc of `radius` tangent to both sides (smaller where
    /// a side is too short to fit it: at most half of each side is used). Each arc gets enough
    /// points to keep within `tolerance` of the true arc. Straight runs stay as they are.
    pub fn filleted(
        &self,
        radius: f64,
        tolerance: f64,
        max_points: usize,
    ) -> Result<Curve, String> {
        let n = self.points.len();
        if radius <= 0.0 || n < 3 {
            return Ok(self.clone());
        }
        let corners: Vec<usize> = if self.closed {
            (0..n).collect()
        } else {
            (1..n - 1).collect()
        };
        let mut out: Vec<P3> = Vec::new();
        if !self.closed {
            out.push(self.points[0]);
        }
        for i in corners {
            let p = self.points[i];
            let prev = self.points[(i + n - 1) % n];
            let next = self.points[(i + 1) % n];
            let (Some(u), Some(v)) = (unit3(sub3(prev, p)), unit3(sub3(next, p))) else {
                out.push(p);
                continue;
            };
            // The angle between the two sides at the corner.
            let cos = dot3(u, v).clamp(-1.0, 1.0);
            let theta = cos.acos();
            if theta < 1e-6 || (std::f64::consts::PI - theta) < 1e-6 {
                out.push(p); // a fold or a straight run: nothing to round
                continue;
            }
            let half = theta / 2.0;
            // Distance from the corner to where the arc touches each side.
            let want = radius / half.tan();
            let room = 0.5 * dist3(prev, p).min(dist3(next, p));
            let d = want.min(room);
            let r = d * half.tan();
            let a = add3(p, scale3(u, d));
            let b = add3(p, scale3(v, d));
            let bis = unit3(add3(u, v)).unwrap_or(u);
            let c = add3(p, scale3(bis, r / half.sin()));
            let sweep = std::f64::consts::PI - theta;
            let step = 2.0 * (1.0 - tolerance.max(1e-4) / r.max(1e-9)).max(-1.0).acos();
            let segs = ((sweep / step.max(1e-3)).ceil() as usize).clamp(1, 1000);
            if out.len() + segs + 1 > max_points {
                return Err(format!(
                    "the rounded curve needs too many points (more than {max_points}); use a larger tolerance"
                ));
            }
            let (ca, cb) = (sub3(a, c), sub3(b, c));
            for k in 0..=segs {
                let f = k as f64 / segs as f64;
                // Slerp from a to b around c, keeping the radius.
                let w = slerp(ca, cb, f);
                out.push(add3(c, w));
            }
        }
        if !self.closed {
            out.push(self.points[n - 1]);
        }
        out.dedup_by(|x, y| dist3(*x, *y) < 1e-9);
        if self.closed && out.len() > 1 && dist3(out[0], out[out.len() - 1]) < 1e-9 {
            out.pop();
        }
        Ok(Curve::new(out, self.closed))
    }

    /// The bounds of the curve's points.
    pub fn bounds(&self) -> Bounds {
        Bounds::of(&self.points)
    }
}

/// Rotates `a` towards `b` by fraction `f` of the angle between them, interpolating the length.
fn slerp(a: P3, b: P3, f: f64) -> P3 {
    let (la, lb) = (norm3(a), norm3(b));
    let (Some(ua), Some(ub)) = (unit3(a), unit3(b)) else {
        return lerp3(a, b, f);
    };
    let omega = dot3(ua, ub).clamp(-1.0, 1.0).acos();
    let len = la + (lb - la) * f;
    if omega < 1e-9 {
        return scale3(ua, len);
    }
    let s = omega.sin();
    let dir = add3(
        scale3(ua, ((1.0 - f) * omega).sin() / s),
        scale3(ub, (f * omega).sin() / s),
    );
    scale3(dir, len)
}

/// Marks the points between `i` and `j` that the simplified curve must keep.
fn rdp(points: &[P3], i: usize, j: usize, tolerance: f64, keep: &mut [bool]) {
    let mut stack = vec![(i, j)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (points[a], points[b]);
        let (mut far, mut worst) = (a, -1.0);
        for (k, p) in points.iter().enumerate().take(b).skip(a + 1) {
            let d = distance_to_segment(*p, pa, pb);
            if d > worst {
                (far, worst) = (k, d);
            }
        }
        if worst > tolerance {
            if let Some(slot) = keep.get_mut(far) {
                *slot = true;
            }
            stack.push((a, far));
            stack.push((far, b));
        }
    }
}

fn distance_to_segment(p: P3, a: P3, b: P3) -> f64 {
    let ab = sub3(b, a);
    let len2 = dot3(ab, ab);
    if len2 <= 0.0 {
        return dist3(p, a);
    }
    let f = (dot3(sub3(p, a), ab) / len2).clamp(0.0, 1.0);
    dist3(p, lerp3(a, b, f))
}

/// One curve from pieces, end to end in order: each piece is turned round if its end, not its
/// start, meets the end of the one before (within `tolerance`), and the joints aren't
/// repeated. The result is closed when its end meets its start.
pub fn join_curves(pieces: &[&Curve], tolerance: f64) -> Result<Curve, String> {
    let mut out: Vec<P3> = Vec::new();
    for (i, piece) in pieces.iter().enumerate() {
        let mut pts = piece.points.clone();
        if piece.closed
            && let Some(first) = pts.first().copied()
        {
            pts.push(first);
        }
        let (Some(start), Some(end)) = (pts.first().copied(), pts.last().copied()) else {
            continue;
        };
        let Some(last) = out.last().copied() else {
            out.extend(pts);
            continue;
        };
        let (to_start, to_end) = (dist3(last, start), dist3(last, end));
        if to_end < to_start && to_end <= tolerance {
            pts.reverse();
        } else if to_start > tolerance {
            return Err(format!(
                "piece {} doesn't touch the end of the one before (the gap is {} mm); join only curves that meet end to end",
                i + 1,
                easycad_lang::format_number((to_start.min(to_end) * 1000.0).round() / 1000.0)
            ));
        }
        // The joint is already there.
        out.extend(pts.into_iter().skip(1));
    }
    if out.len() < 2 {
        return Err("needs pieces with at least 2 points between them".into());
    }
    let closed = out.len() > 2
        && out
            .first()
            .zip(out.last())
            .is_some_and(|(a, b)| dist3(*a, *b) <= tolerance);
    if closed {
        out.pop();
    }
    out.dedup_by(|x, y| dist3(*x, *y) < 1e-9);
    Ok(Curve::new(out, closed))
}

/// A uniform Catmull-Rom spline through `points`, `per_segment` points per span (EasyGCode's
/// splines: its rounded star and vessel profiles). An open spline's ends get mirrored
/// neighbours (2·p0 − p1), so it starts heading straight for the second point.
pub fn catmull_rom(points: &[P3], closed: bool, per_segment: usize) -> Curve {
    let n = points.len();
    if n < 3 {
        return Curve::new(points.to_vec(), closed);
    }
    let per = per_segment.max(1);
    let get = |i: isize| -> P3 {
        if closed {
            points[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            sub3(scale3(points[0], 2.0), points[1])
        } else if i as usize >= n {
            sub3(scale3(points[n - 1], 2.0), points[n - 2])
        } else {
            points[i as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * per + 1);
    for s in 0..spans as isize {
        for k in 0..per {
            let t = k as f64 / per as f64;
            out.push(crate::geom::catmull_rom(
                get(s - 1),
                get(s),
                get(s + 1),
                get(s + 2),
                t,
            ));
        }
    }
    if !closed {
        out.push(points[n - 1]);
    }
    Curve::new(out, closed)
}

/// A superellipse (|x/rx|ⁿ + |y/ry|ⁿ = 1) around the origin, counter-clockwise from +X:
/// n = 2 is an ellipse, larger n squarer (a squircle near 4), smaller n pinched (1 is a
/// diamond). `samples` points, spread evenly by angle around the centre.
pub fn superellipse(rx: f64, ry: f64, n: f64, samples: usize) -> Curve {
    let m = samples.max(8);
    let points = (0..m)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / m as f64;
            let (c, s) = (a.cos(), a.sin());
            // The radius along this direction where the curve crosses it.
            let k = ((c / rx).abs().powf(n) + (s / ry).abs().powf(n)).powf(-1.0 / n);
            [k * c, k * s, 0.0]
        })
        .collect();
    Curve::new(points, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Curve {
        Curve::new(
            vec![
                [0.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        )
    }

    #[test]
    fn closest_points_tangents_and_normals() {
        let sq = square();
        let (t, p) = sq.closest([5.0, -3.0, 0.0]);
        assert!((t - 0.125).abs() < 1e-12 && dist3(p, [5.0, 0.0, 0.0]) < 1e-12);
        let (t, _) = sq.closest([12.0, 5.0, 0.0]);
        assert!((t - 0.375).abs() < 1e-12);
        assert_eq!(sq.tangent_at(0.1), Some([1.0, 0.0, 0.0]));
        assert_eq!(sq.tangent_at(0.3), Some([0.0, 1.0, 0.0]));
        // Outward either way round.
        let out = sq.normal_at(0.1).unwrap();
        assert!(dist3(out, [0.0, -1.0, 0.0]) < 1e-12);
        let cw = sq.flipped();
        let out = cw.normal_at(0.9).unwrap();
        assert!(dist3(out, [0.0, -1.0, 0.0]) < 1e-12, "{out:?}");
        let line = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0]], false);
        assert!(dist3(line.normal_at(0.5).unwrap(), [0.0, -1.0, 0.0]) < 1e-12);
        assert!(Curve::new(vec![[1.0; 3]], false).tangent_at(0.5).is_none());
    }

    #[test]
    fn centroids() {
        let c = square().centroid().unwrap();
        assert!(dist3(c, [5.0, 5.0, 0.0]) < 1e-12);
        let far =
            Curve::circle(3.0, 64).transformed(&crate::geom::Xform::translate(1e5, -2e5, 4.0));
        let c = far.centroid().unwrap();
        assert!(dist3(c, [1e5, -2e5, 4.0]) < 1e-6, "{c:?}");
        let l = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], false);
        assert!(dist3(l.centroid().unwrap(), [7.5, 2.5, 0.0]) < 1e-12);
    }

    #[test]
    fn pieces_between_two_places() {
        let sq = square();
        let s = sq.subcurve(0.125, 0.375).unwrap();
        assert_eq!(
            s.points,
            vec![[5.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 5.0, 0.0]]
        );
        // Through the seam.
        let s = sq.subcurve(0.875, 0.125).unwrap();
        assert_eq!(
            s.points,
            vec![[0.0, 5.0, 0.0], [0.0, 0.0, 0.0], [5.0, 0.0, 0.0]]
        );
        let line = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0]], false);
        let back = line.subcurve(0.8, 0.2).unwrap();
        assert!(dist3(back.points[0], [8.0, 0.0, 0.0]) < 1e-12);
        assert!(dist3(back.points[1], [2.0, 0.0, 0.0]) < 1e-12);
        assert!(line.subcurve(0.5, 0.5).is_err());
    }

    #[test]
    fn simplifying_keeps_the_shape_within_the_tolerance() {
        let dense = Curve::circle(10.0, 720);
        let s = dense.simplified(0.05);
        assert!(
            s.points.len() < 100 && s.points.len() > 20,
            "{}",
            s.points.len()
        );
        for p in &dense.points {
            assert!(
                s.closest(*p)
                    .1
                    .iter()
                    .zip(p)
                    .all(|(a, b)| (a - b).abs() < 0.06)
            );
        }
        let wiggle = Curve::new(
            vec![
                [0.0; 3],
                [1.0, 0.01, 0.0],
                [2.0, -0.01, 0.0],
                [3.0, 0.0, 0.0],
            ],
            false,
        );
        assert_eq!(wiggle.simplified(0.1).points.len(), 2);
    }

    #[test]
    fn fillets_round_corners_tangentially() {
        let sq = square();
        let f = sq.filleted(2.0, 0.01, 10_000).unwrap();
        assert!(f.closed);
        // The arc passes r·(√2 − 1) from the corner it replaces.
        let worst = f
            .points
            .iter()
            .map(|p| dist3(*p, [10.0, 10.0, 0.0]))
            .fold(f64::INFINITY, f64::min);
        let expected = 2.0 * (2f64.sqrt() - 1.0);
        assert!((worst - expected).abs() < 0.02, "{worst} vs {expected}");
        // The straight sides are kept: the length is 4·(10 − 2·2) + 2π·2.
        let want = 4.0 * 6.0 + std::f64::consts::TAU * 2.0;
        assert!((f.length() - want).abs() < 0.05, "{}", f.length());
        // A radius too big for the sides is limited to what fits.
        let big = sq.filleted(100.0, 0.01, 10_000).unwrap();
        assert!(
            big.points
                .iter()
                .all(|p| p[0] >= -1e-9 && p[0] <= 10.0 + 1e-9)
        );
        let open = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], false);
        let o = open.filleted(3.0, 0.01, 10_000).unwrap();
        assert_eq!(o.points.first(), Some(&[0.0; 3]));
        assert_eq!(o.points.last(), Some(&[10.0, 10.0, 0.0]));
        assert!(sq.filleted(2.0, 0.0001, 10).is_err());
    }

    #[test]
    fn joining_pieces_end_to_end() {
        let a = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0]], false);
        let b = Curve::new(vec![[10.0, 10.0, 0.0], [10.0, 0.0, 0.0]], false); // runs backwards
        let c = Curve::new(vec![[10.0, 10.0, 0.0], [0.0, 0.0, 0.0]], false);
        let j = join_curves(&[&a, &b, &c], 1e-6).unwrap();
        assert!(j.closed);
        assert_eq!(
            j.points,
            vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]]
        );
        let gap = Curve::new(vec![[20.0, 0.0, 0.0], [30.0, 0.0, 0.0]], false);
        assert!(join_curves(&[&a, &gap], 1e-6).is_err());
    }

    #[test]
    fn catmull_rom_passes_through_the_points() {
        let pts = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        let open = catmull_rom(&pts, false, 8);
        assert_eq!(open.points.len(), 3 * 8 + 1);
        for (k, p) in pts.iter().enumerate() {
            assert!(dist3(open.points[k * 8], *p) < 1e-12);
        }
        let closed = catmull_rom(&pts, true, 8);
        assert_eq!(closed.points.len(), 32);
        assert!(closed.closed);
    }

    #[test]
    fn superellipses_from_diamond_to_squircle() {
        let e = superellipse(10.0, 5.0, 2.0, 64);
        assert!(
            e.points
                .iter()
                .all(|p| ((p[0] / 10.0).powi(2) + (p[1] / 5.0).powi(2) - 1.0).abs() < 1e-9)
        );
        let sq = superellipse(10.0, 10.0, 4.0, 64);
        let corner = sq.points[8]; // at 45°
        assert!(
            (corner[0] - 10.0 / 2f64.powf(0.25)).abs() < 1e-9,
            "{corner:?}"
        );
        assert!(sq.signed_area() > 0.0);
    }
}
