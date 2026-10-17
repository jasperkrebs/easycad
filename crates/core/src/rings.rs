//! Ring fills, the way EasyGCode fills its bottoms and seats (`ringFill` in its geometry.js):
//! concentric rings inside an outline, one line width apart, linked into one continuous line
//! as a staircase, a zipper or a spiral.

use crate::curve::Curve;
use crate::geom::{P3, dist3, lerp3, signed_area_xy};

/// The most rings one fill may have.
const MAX_RINGS: usize = 4000;

/// Rings inside a closed outline, from the outline itself inward, `spacing` apart. Scaled
/// toward the outline's centre (EasyGCode's default ring fill: exact for circles), or, with
/// `inset`, true insets that keep every side `spacing` from the last (for outlines that aren't
/// round), stopping when the next ring would collapse or cross itself.
pub fn rings(
    outline: &Curve,
    spacing: f64,
    inset: bool,
    max_points: usize,
) -> Result<Vec<Curve>, String> {
    if !outline.closed || outline.points.len() < 3 {
        return Err("needs a closed outline with at least 3 points".into());
    }
    if !(spacing.is_finite() && spacing > 0.0) {
        return Err(format!("`spacing` must be more than 0, got {spacing}"));
    }
    let mut out = vec![Curve::new(outline.points.clone(), true)];
    let mut total = outline.points.len();
    if inset {
        let sign = signed_area_xy(&outline.points).signum();
        for k in 1..MAX_RINGS {
            let Some(ring) = inset_ring(&outline.points, k as f64 * spacing) else {
                break;
            };
            let area = signed_area_xy(&ring);
            if ring.len() < 3 || area.signum() != sign || area.abs() < 1e-4 || crosses_itself(&ring)
            {
                break;
            }
            total += ring.len();
            if total > max_points {
                return Err("these rings need too many points; use a larger spacing".into());
            }
            out.push(Curve::new(ring, true));
        }
        return Ok(out);
    }
    let c = outline.centroid().unwrap_or([0.0; 3]);
    let r_max = outline
        .points
        .iter()
        .map(|p| (p[0] - c[0]).hypot(p[1] - c[1]))
        .fold(0.0, f64::max);
    let mut k = 1;
    while (k as f64) * spacing <= r_max - spacing / 2.0 && k < MAX_RINGS {
        let f = (r_max - k as f64 * spacing) / r_max;
        let ring: Vec<P3> = outline
            .points
            .iter()
            .map(|p| [c[0] + (p[0] - c[0]) * f, c[1] + (p[1] - c[1]) * f, p[2]])
            .collect();
        total += ring.len();
        if total > max_points {
            return Err("these rings need too many points; use a larger spacing".into());
        }
        out.push(Curve::new(ring, true));
        k += 1;
    }
    Ok(out)
}

/// The outline moved `d` inward, side by side: each side's line moved in, corners where
/// neighbouring lines meet, and sides that would turn inside out dropped (their neighbours then
/// meet directly). `None` when nothing is left.
fn inset_ring(points: &[P3], d: f64) -> Option<Vec<P3>> {
    let ccw = signed_area_xy(points) >= 0.0;
    // Each side as (start, direction, inward normal), skipping repeated points.
    let mut sides: Vec<(P3, [f64; 2], [f64; 2])> = Vec::new();
    let n = points.len();
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len < 1e-9 {
            continue;
        }
        let dir = [dx / len, dy / len];
        let normal = if ccw {
            [-dir[1], dir[0]]
        } else {
            [dir[1], -dir[0]]
        };
        sides.push((
            [a[0] + normal[0] * d, a[1] + normal[1] * d, a[2]],
            dir,
            normal,
        ));
    }
    // Drop sides that come out reversed, until none do.
    for _ in 0..n {
        let m = sides.len();
        if m < 3 {
            return None;
        }
        let corners: Vec<Option<P3>> = (0..m)
            .map(|i| meet(&sides[(i + m - 1) % m], &sides[i]))
            .collect();
        let mut reversed = None;
        for i in 0..m {
            let (Some(a), Some(b)) = (corners[i], corners[(i + 1) % m]) else {
                continue;
            };
            let along = (b[0] - a[0]) * sides[i].1[0] + (b[1] - a[1]) * sides[i].1[1];
            if along < -1e-9 {
                reversed = Some(i);
                break;
            }
        }
        match reversed {
            Some(i) => {
                sides.remove(i);
            }
            None => {
                return Some((0..m).map(|i| corners[i].unwrap_or(sides[i].0)).collect());
            }
        }
    }
    None
}

/// Where the moved lines of two neighbouring sides meet (`None` when they're parallel, in
/// which case the second side's own start is used).
fn meet(a: &(P3, [f64; 2], [f64; 2]), b: &(P3, [f64; 2], [f64; 2])) -> Option<P3> {
    let (p, r) = (a.0, a.1);
    let (q, s) = (b.0, b.1);
    let cross = r[0] * s[1] - r[1] * s[0];
    if cross.abs() < 1e-9 {
        return Some(q);
    }
    let t = ((q[0] - p[0]) * s[1] - (q[1] - p[1]) * s[0]) / cross;
    Some([p[0] + r[0] * t, p[1] + r[1] * t, q[2]])
}

/// Whether a closed ring crosses itself (segments sorted by x, so only overlapping ones are
/// compared).
fn crosses_itself(ring: &[P3]) -> bool {
    let n = ring.len();
    let mut segs: Vec<(f64, f64, usize)> = (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            (a[0].min(b[0]), a[0].max(b[0]), i)
        })
        .collect();
    segs.sort_by(|x, y| x.0.total_cmp(&y.0));
    for (k, &(_, hi, i)) in segs.iter().enumerate() {
        for &(lo2, _, j) in &segs[k + 1..] {
            if lo2 > hi {
                break;
            }
            let adjacent = (i + 1) % n == j || (j + 1) % n == i;
            if !adjacent && segments_cross(ring[i], ring[(i + 1) % n], ring[j], ring[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}

fn segments_cross(a: P3, b: P3, c: P3, d: P3) -> bool {
    let orient =
        |p: P3, q: P3, r: P3| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (d1, d2) = (orient(a, b, c), orient(a, b, d));
    let (d3, d4) = (orient(c, d, a), orient(c, d, b));
    (d1 > 1e-12 && d2 < -1e-12 || d1 < -1e-12 && d2 > 1e-12)
        && (d3 > 1e-12 && d4 < -1e-12 || d3 < -1e-12 && d4 > 1e-12)
}

/// How rings are linked into one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// Each ring stops a line width short of a full turn and steps across to the next, where
    /// the next one starts: the seam moves back one line width per ring.
    Staircase,
    /// Each ring is cut open across a slot one line width wide at the seam and runs the other
    /// way from the ring before; the steps between rings run along the slot's sides.
    Zipper,
    /// One spiral that blends each ring into the next; from the centre, it opens with a turn
    /// out of the middle.
    Spiral,
}

impl Link {
    pub fn parse(name: &str) -> Result<Link, String> {
        match name {
            "staircase" => Ok(Link::Staircase),
            "zipper" => Ok(Link::Zipper),
            "spiral" => Ok(Link::Spiral),
            other => Err(format!(
                "`style` must be \"staircase\", \"zipper\" or \"spiral\", got \"{other}\""
            )),
        }
    }
}

/// Rings linked into one continuous open line in their order (rings from `rings`, reversed,
/// run from the centre out). Every ring gets the same number of points, enough for half a
/// millimetre between them on the largest (at least 48), starting where a ray from the centre
/// through the largest ring's first point crosses it: that's the seam, and where staircase steps
/// and the zipper's slot are.
pub fn link_rings(
    rings: &[&Curve],
    style: Link,
    width: f64,
    max_points: usize,
) -> Result<Curve, String> {
    if rings.is_empty() {
        return Err("needs at least one ring".into());
    }
    if let Some(i) = rings.iter().position(|r| !r.closed || r.points.len() < 3) {
        return Err(format!(
            "ring {} isn't a closed curve of 3 or more points",
            i + 1
        ));
    }
    if !(width.is_finite() && width > 0.0) {
        return Err(format!("`width` must be more than 0, got {width}"));
    }
    let largest = rings
        .iter()
        .max_by(|a, b| a.signed_area().abs().total_cmp(&b.signed_area().abs()))
        .copied()
        .ok_or("needs at least one ring")?;
    let c = largest.centroid().unwrap_or([0.0; 3]);
    let first = largest.points[0];
    let ray = {
        let (dx, dy) = (first[0] - c[0], first[1] - c[1]);
        let len = dx.hypot(dy);
        if len < 1e-9 {
            [0.0, 1.0]
        } else {
            [dx / len, dy / len]
        }
    };
    let n = 48usize.max((largest.length() / 0.5).ceil() as usize);
    if n.saturating_mul(rings.len()).saturating_add(n) > max_points {
        return Err("linking these rings needs too many points".into());
    }
    let ccw = largest.signed_area() >= 0.0;
    let rings: Vec<Vec<P3>> = rings
        .iter()
        .map(|r| {
            let r = if (r.signed_area() >= 0.0) == ccw {
                (*r).clone()
            } else {
                r.flipped()
            };
            from_seam(&r, c, ray).resample(n).points
        })
        .collect();
    let mut out: Vec<P3> = Vec::new();
    match style {
        Link::Staircase => {
            let mut start = 0;
            for ring in &rings {
                let per = Curve::new(ring.clone(), true).length();
                let gap = ((width / per.max(1e-9) * n as f64).round() as usize).clamp(1, n - 2);
                for t in 0..=n - gap {
                    out.push(ring[(start + t) % n]);
                }
                start = (start + n - gap) % n;
            }
        }
        Link::Zipper => {
            let hw = width / 2.0;
            let across = |p: P3| (p[0] - c[0]) * -ray[1] + (p[1] - c[1]) * ray[0];
            let along = |p: P3| (p[0] - c[0]) * ray[0] + (p[1] - c[1]) * ray[1];
            let in_slot = |p: P3| along(p) > 0.0 && across(p).abs() < hw;
            for (k, ring) in rings.iter().enumerate() {
                let a = (1..n).find(|&j| !in_slot(ring[j]));
                let b = (1..n).rev().find(|&j| !in_slot(ring[j]));
                let (Some(a), Some(b)) = (a, b) else { continue };
                if b < a {
                    continue;
                }
                // The exact points where the ring leaves and re-enters the slot.
                let cut = |inside: P3, outside: P3| -> P3 {
                    let (si, so) = (across(inside), across(outside));
                    let target = hw * so.signum();
                    let f = if (so - si).abs() > 1e-12 {
                        ((target - si) / (so - si)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    lerp3(inside, outside, f)
                };
                let mut arc = vec![cut(ring[a - 1], ring[a])];
                arc.extend_from_slice(&ring[a..=b]);
                arc.push(cut(ring[(b + 1) % n], ring[b]));
                if k % 2 == 1 {
                    arc.reverse();
                }
                out.extend(arc);
            }
        }
        Link::Spiral => {
            let area = |r: &Vec<P3>| signed_area_xy(r).abs();
            let outward = rings.len() < 2 || area(&rings[0]) <= area(&rings[rings.len() - 1]);
            if outward {
                // An opening turn out of the middle onto the first ring.
                let first = &rings[0];
                for (j, p) in first
                    .iter()
                    .enumerate()
                    .chain(std::iter::once((n, &first[0])))
                {
                    let f = j as f64 / n as f64;
                    out.push([c[0] + (p[0] - c[0]) * f, c[1] + (p[1] - c[1]) * f, p[2]]);
                }
            } else {
                out.push(rings[0][0]);
            }
            for w in rings.windows(2) {
                for j in 1..=n {
                    let f = j as f64 / n as f64;
                    out.push(lerp3(w[0][j % n], w[1][j % n], f));
                }
            }
        }
    }
    out.dedup_by(|x, y| dist3(*x, *y) < 1e-9);
    if out.len() < 2 {
        return Err(
            "the rings left nothing to print (they're narrower than the line width)".into(),
        );
    }
    Ok(Curve::new(out, false))
}

/// The ring started where the ray from `c` along `ray` crosses it farthest out (a point added
/// there), or at its point farthest along the ray if it doesn't cross.
fn from_seam(ring: &Curve, c: P3, ray: [f64; 2]) -> Curve {
    let pts = &ring.points;
    let n = pts.len();
    let along = |p: P3| (p[0] - c[0]) * ray[0] + (p[1] - c[1]) * ray[1];
    let across = |p: P3| (p[0] - c[0]) * -ray[1] + (p[1] - c[1]) * ray[0];
    let mut best: Option<(f64, usize, P3)> = None;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (sa, sb) = (across(a), across(b));
        if (sa > 0.0) == (sb > 0.0) && sa != 0.0 && sb != 0.0 {
            continue;
        }
        let f = if (sb - sa).abs() > 1e-15 {
            (-sa / (sb - sa)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let p = lerp3(a, b, f);
        let t = along(p);
        if t > 0.0 && best.is_none_or(|bst| t > bst.0) {
            best = Some((t, i, p));
        }
    }
    let (i, start) = match best {
        Some((_, i, p)) => (i, p),
        None => {
            let i = (0..n)
                .max_by(|&x, &y| along(pts[x]).total_cmp(&along(pts[y])))
                .unwrap_or(0);
            (i, pts[i])
        }
    };
    let mut points = vec![start];
    for k in 1..=n {
        let p = pts[(i + k) % n];
        if dist3(p, start) > 1e-9 {
            points.push(p);
        }
    }
    points.dedup_by(|x, y| dist3(*x, *y) < 1e-9);
    Curve::new(points, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_rings_are_one_spacing_apart_on_a_circle() {
        let c = Curve::circle(10.0, 96);
        let r = rings(&c, 2.0, false, 100_000).unwrap();
        // k · 2 ≤ 10 − 1 → k up to 4, plus the outline.
        assert_eq!(r.len(), 5);
        let radius = |c: &Curve| c.points[0][0].hypot(c.points[0][1]);
        assert!((radius(&r[4]) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn insets_keep_their_sides_apart_and_stop_before_collapsing() {
        let sq = Curve::new(
            vec![
                [0.0, 0.0, 0.0],
                [20.0, 0.0, 0.0],
                [20.0, 10.0, 0.0],
                [0.0, 10.0, 0.0],
            ],
            true,
        );
        let r = rings(&sq, 1.0, true, 100_000).unwrap();
        // 10 mm tall: insets of 1..4 mm leave a strip; 5 mm would collapse it.
        assert_eq!(
            r.len(),
            5,
            "{:?}",
            r.iter().map(|c| c.points.len()).collect::<Vec<_>>()
        );
        let b = r[2].bounds();
        assert!((b.min[0] - 2.0).abs() < 1e-9 && (b.max[1] - 8.0).abs() < 1e-9);
        // An L shape: the inner corner's sides drop out cleanly.
        let l = Curve::new(
            vec![
                [0.0, 0.0, 0.0],
                [30.0, 0.0, 0.0],
                [30.0, 10.0, 0.0],
                [10.0, 10.0, 0.0],
                [10.0, 30.0, 0.0],
                [0.0, 30.0, 0.0],
            ],
            true,
        );
        let r = rings(&l, 2.0, true, 100_000).unwrap();
        assert!(r.len() >= 2);
        assert!(r.iter().all(|c| !crosses_itself(&c.points)));
    }

    #[test]
    fn linked_rings_make_one_line() {
        let c = Curve::circle(10.0, 96);
        let r = rings(&c, 1.0, false, 100_000).unwrap();
        let refs: Vec<&Curve> = r.iter().rev().collect();
        for style in [Link::Staircase, Link::Zipper, Link::Spiral] {
            let line = link_rings(&refs, style, 1.0, 1_000_000).unwrap();
            assert!(!line.closed);
            // No step between points is longer than about a line width.
            let worst = line
                .segments()
                .map(|(a, b)| dist3(a, b))
                .fold(0.0, f64::max);
            assert!(worst < 1.6, "{style:?}: {worst}");
        }
        // The spiral from the centre starts in the middle and ends on the outline.
        let s = link_rings(&refs, Link::Spiral, 1.0, 1_000_000).unwrap();
        assert!(s.points[0][0].hypot(s.points[0][1]) < 1e-9);
        let end = s.points.last().unwrap();
        assert!((end[0].hypot(end[1]) - 10.0).abs() < 0.05);
        assert!(Link::parse("swirl").is_err());
    }
}
