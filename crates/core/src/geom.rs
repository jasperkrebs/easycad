//! Small geometry helpers shared by the Phase 0 demo. Plain arrays keep the hot loops simple.

/// A 2D point `[x, y]` in mm.
pub type P2 = [f64; 2];
/// A 3D point `[x, y, z]` in mm, Z up.
pub type P3 = [f64; 3];

/// Straight-line distance between two 3D points.
pub fn dist3(a: P3, b: P3) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
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
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
    ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt()
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
    fn degenerate_ring_is_none() {
        assert_eq!(signed_distance_to_ring([0.0, 0.0], &SQUARE[..2]), None);
    }

    #[test]
    fn dist3_is_euclidean() {
        assert_eq!(dist3([0.0, 0.0, 0.0], [3.0, 4.0, 12.0]), 13.0);
    }
}
