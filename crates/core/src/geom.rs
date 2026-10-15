//! Small geometry helpers. Plain arrays keep the hot loops simple.

/// A 2D point `[x, y]` in mm.
pub type P2 = [f64; 2];
/// A 3D point `[x, y, z]` in mm, Z up.
pub type P3 = [f64; 3];

/// Straight-line distance between two 3D points.
pub fn dist3(a: P3, b: P3) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
}

/// The point a fraction `t` of the way from `a` to `b`.
pub fn lerp3(a: P3, b: P3, t: f64) -> P3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Uniform Catmull-Rom interpolation between `p1` (t = 0) and `p2` (t = 1), with `p0` and
/// `p3` as the neighbours. The curve passes through every point and keeps straight runs
/// straight, which is what lofting through sections needs.
pub fn catmull_rom(p0: P3, p1: P3, p2: P3, p3: P3, t: f64) -> P3 {
    let (t2, t3) = (t * t, t * t * t);
    let mut out = [0.0; 3];
    for (k, o) in out.iter_mut().enumerate() {
        *o = 0.5
            * (2.0 * p1[k]
                + (p2[k] - p0[k]) * t
                + (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * t2
                + (3.0 * p1[k] - p0[k] - 3.0 * p2[k] + p3[k]) * t3);
    }
    out
}

/// An affine transform `p' = M·p + t`: scales, rotations about Z, and moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xform {
    m: [[f64; 3]; 3],
    t: P3,
}

impl Xform {
    /// Scales about the origin.
    pub fn scale(x: f64, y: f64, z: f64) -> Self {
        Xform {
            m: [[x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, z]],
            t: [0.0; 3],
        }
    }

    /// Rotates counter-clockwise (seen from above) about the Z axis through the origin.
    pub fn rotate_z(degrees: f64) -> Self {
        let (s, c) = degrees.to_radians().sin_cos();
        Xform {
            m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
            t: [0.0; 3],
        }
    }

    pub fn translate(x: f64, y: f64, z: f64) -> Self {
        Xform {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t: [x, y, z],
        }
    }

    pub fn apply(&self, p: P3) -> P3 {
        let m = &self.m;
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + self.t[0],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + self.t[1],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + self.t[2],
        ]
    }

    /// If this transform keeps circles in the XY plane circles (a move, a rotation about Z, a
    /// uniform scale in X and Y, or a mirror), the factor it scales their radius by.
    pub fn xy_similarity(&self) -> Option<f64> {
        let m = &self.m;
        let (a, b, c, d) = (m[0][0], m[0][1], m[1][0], m[1][1]);
        let uncoupled = m[0][2] == 0.0 && m[1][2] == 0.0 && m[2][0] == 0.0 && m[2][1] == 0.0;
        let tol = 1e-12 * (a.abs() + b.abs() + c.abs() + d.abs()).max(1.0);
        let rotation = (a - d).abs() <= tol && (b + c).abs() <= tol;
        let reflection = (a + d).abs() <= tol && (b - c).abs() <= tol;
        let s = a.hypot(c);
        (uncoupled && (rotation || reflection) && s > 0.0).then_some(s)
    }

    /// Whether the transform flips orientation (a mirror), which reverses curve direction.
    pub fn mirrors(&self) -> bool {
        let m = &self.m;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        det < 0.0
    }
}

/// Signed area of a closed polygon seen from above: positive when it runs counter-clockwise.
pub fn signed_area_xy(points: &[P3]) -> f64 {
    let mut area = 0.0;
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        area += a[0] * b[1] - b[0] * a[1];
    }
    area / 2.0
}

/// Signed distance from `p` to the closed polyline `ring`: positive outside, negative inside.
///
/// Used for overhangs: how far a point on the next layer sticks out past the layer below.
/// Returns `None` for a ring with fewer than 3 points.
pub fn signed_distance_to_ring(p: P2, ring: &[P2]) -> Option<f64> {
    if ring.len() < 3 {
        return None;
    }
    let mut best = f64::INFINITY;
    let mut inside = false;
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        // Even-odd crossing test for inside/outside.
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x_cross = a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if p[0] < x_cross {
                inside = !inside;
            }
        }
        best = best.min(distance_to_segment(p, *a, *b));
    }
    Some(if inside { -best } else { best })
}

/// Distance from `p` to the segment `a`–`b`.
fn distance_to_segment(p: P2, a: P2, b: P2) -> f64 {
    closest_on_segment(p, a, b).0
}

/// Distance from `p` to the segment `a`–`b`, and where the closest point is (0 at `a`, 1 at `b`).
fn closest_on_segment(p: P2, a: P2, b: P2) -> (f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
    (((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt(), t)
}

/// Like [`signed_distance_to_ring`], but only looks at the `window` segments either side of
/// index `around`: fast when `p` is known to be near that part of the ring, as a point of one
/// layer is near the same point of the layer below. `ccw` says which way the ring runs.
/// The side comes from the nearest segment's outward normal (or, at a corner, the sum of both
/// neighbouring normals), which is exact for a ring that doesn't fold back on itself nearby.
pub fn signed_distance_near(
    p: P2,
    ring: &[P2],
    around: usize,
    window: usize,
    ccw: bool,
) -> Option<f64> {
    let m = ring.len();
    if m < 3 {
        return None;
    }
    let at = |i: usize| ring.get(i % m).copied().unwrap_or([0.0; 2]);
    // Outward normal of segment i (unnormalised direction is fine for a sign, but corners add
    // two of them, so make them unit length).
    let normal = |i: usize| {
        let (a, b) = (at(i), at(i + 1));
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy).max(1e-300);
        if ccw {
            [dy / len, -dx / len]
        } else {
            [-dy / len, dx / len]
        }
    };
    let window = window.min(m / 2);
    let mut best = (f64::INFINITY, 0usize, 0.0);
    for k in 0..=2 * window {
        let i = (around + m - window + k) % m;
        let (d, t) = closest_on_segment(p, at(i), at(i + 1));
        if d < best.0 {
            best = (d, i, t);
        }
    }
    let (d, i, t) = best;
    let (corner, n) = if t <= 0.0 {
        let (a, b) = (normal(i + m - 1), normal(i));
        (at(i), [a[0] + b[0], a[1] + b[1]])
    } else if t >= 1.0 {
        let (a, b) = (normal(i), normal(i + 1));
        (at(i + 1), [a[0] + b[0], a[1] + b[1]])
    } else {
        (at(i), normal(i))
    };
    let side = (p[0] - corner[0]) * n[0] + (p[1] - corner[1]) * n[1];
    Some(if side > 0.0 { d } else { -d })
}

/// Axis-aligned bounds `[min, max]`, for framing the view and checking the bed.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Bounds {
    pub min: P3,
    pub max: P3,
}

impl Bounds {
    pub fn empty() -> Self {
        Bounds {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }

    pub fn add(&mut self, p: P3) {
        for (k, v) in p.iter().enumerate() {
            self.min[k] = self.min[k].min(*v);
            self.max[k] = self.max[k].max(*v);
        }
    }

    pub fn union(&mut self, other: &Bounds) {
        if !other.is_empty() {
            self.add(other.min);
            self.add(other.max);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min[0] > self.max[0]
    }

    pub fn of(points: &[P3]) -> Self {
        let mut b = Bounds::empty();
        points.iter().for_each(|p| b.add(*p));
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [P2; 4] = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];

    #[test]
    fn inside_is_negative_outside_is_positive() {
        assert_eq!(signed_distance_to_ring([5.0, 2.0], &SQUARE), Some(-2.0));
        assert_eq!(signed_distance_to_ring([13.0, 5.0], &SQUARE), Some(3.0));
    }

    #[test]
    fn nearby_search_agrees_with_the_full_search() {
        let ring: Vec<P2> = (0..64)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 64.0;
                let r = if i % 2 == 0 { 10.0 } else { 7.0 }; // a spiky star: corners both ways
                [r * a.cos(), r * a.sin()]
            })
            .collect();
        for (j, q) in ring.iter().enumerate() {
            for scale in [0.9, 0.99, 1.0, 1.01, 1.2] {
                let p = [q[0] * scale + 0.01, q[1] * scale];
                let full = signed_distance_to_ring(p, &ring).unwrap();
                let near = signed_distance_near(p, &ring, j, 8, true).unwrap();
                assert!(
                    (full - near).abs() < 1e-9,
                    "point {j} × {scale}: {full} vs {near}"
                );
            }
        }
        let cw: Vec<P2> = ring.iter().rev().copied().collect();
        let p = [12.0, 0.0];
        assert!(signed_distance_near(p, &cw, 63, 8, false).unwrap() > 0.0);
    }

    #[test]
    fn degenerate_ring_is_none() {
        assert_eq!(signed_distance_to_ring([0.0, 0.0], &SQUARE[..2]), None);
    }

    #[test]
    fn dist3_is_euclidean() {
        assert_eq!(dist3([0.0, 0.0, 0.0], [3.0, 4.0, 12.0]), 13.0);
    }

    #[test]
    fn catmull_rom_hits_its_ends_and_keeps_lines_straight() {
        let p = |x: f64| [x, 2.0 * x, 0.0];
        assert_eq!(catmull_rom(p(0.0), p(1.0), p(2.0), p(3.0), 0.0), p(1.0));
        assert_eq!(catmull_rom(p(0.0), p(1.0), p(2.0), p(3.0), 1.0), p(2.0));
        let mid = catmull_rom(p(0.0), p(1.0), p(2.0), p(3.0), 0.25);
        assert!(dist3(mid, p(1.25)) < 1e-12);
    }

    #[test]
    fn transforms() {
        let r = Xform::rotate_z(90.0).apply([1.0, 0.0, 5.0]);
        assert!(dist3(r, [0.0, 1.0, 5.0]) < 1e-12);
        assert_eq!(
            Xform::scale(2.0, 3.0, 4.0).apply([1.0, 1.0, 1.0]),
            [2.0, 3.0, 4.0]
        );
        assert_eq!(
            Xform::translate(1.0, 2.0, 3.0).apply([1.0, 1.0, 1.0]),
            [2.0, 3.0, 4.0]
        );
        assert!(Xform::scale(-1.0, 1.0, 1.0).mirrors());
        assert!(!Xform::rotate_z(33.0).mirrors());
        assert_eq!(Xform::translate(3.0, 4.0, 5.0).xy_similarity(), Some(1.0));
        assert!((Xform::rotate_z(33.0).xy_similarity().unwrap() - 1.0).abs() < 1e-12);
        assert_eq!(Xform::scale(2.0, 2.0, 7.0).xy_similarity(), Some(2.0));
        assert_eq!(Xform::scale(-2.0, 2.0, 1.0).xy_similarity(), Some(2.0));
        assert_eq!(Xform::scale(2.0, 1.0, 1.0).xy_similarity(), None);
    }

    #[test]
    fn area_sign_follows_direction() {
        let ccw: Vec<P3> = SQUARE.iter().map(|p| [p[0], p[1], 0.0]).collect();
        assert_eq!(signed_area_xy(&ccw), 100.0);
        let cw: Vec<P3> = ccw.iter().rev().copied().collect();
        assert_eq!(signed_area_xy(&cw), -100.0);
    }
}
