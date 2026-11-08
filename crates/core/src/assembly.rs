//! Parts placed together (`place`): whether their beads run into each other, where they're
//! shown in the assembly. Shades screwed onto each other sit a line width plus the fit apart, so
//! they pass; beads overlapping by more than a quarter of their width, at the same height, are
//! reported.

use crate::geom::P3;
use crate::toolpath::Toolpath;
use std::collections::HashMap;
use std::rc::Rc;

/// How close two beads may come, across, as a share of their mean width.
const ACROSS: f64 = 0.75;
/// How close in height they must be to touch, as a share of their mean layer height.
const UP: f64 = 0.9;
/// Cells a move is filed under at most (a longer move is checked by its ends only).
const MAX_CELLS: usize = 512;

/// One message per pair of parts that run into each other: their names and the height where
/// they first do.
pub fn collisions(parts: &[(String, Rc<Toolpath>)]) -> Vec<String> {
    let shown: Vec<Vec<Bead>> = parts.iter().map(|(_, p)| beads(p)).collect();
    let mut out = Vec::new();
    for a in 0..parts.len() {
        for b in (a + 1)..parts.len() {
            if let Some(z) = first_touch(&shown[a], &shown[b]) {
                out.push(format!(
                    "`{}` and `{}` run into each other at {} mm high",
                    parts[a].0,
                    parts[b].0,
                    easycad_lang::format_number(z.round())
                ));
            }
        }
    }
    out
}

/// A printed move where it's shown: from, to, its width and layer height.
#[derive(Clone, Copy)]
struct Bead {
    a: P3,
    b: P3,
    w: f64,
    h: f64,
}

fn beads(p: &Toolpath) -> Vec<Bead> {
    let at = |i: usize| match &p.view {
        Some(x) => x.apply(p.points[i]),
        None => p.points[i],
    };
    (1..p.points.len())
        .filter(|&i| !p.travel.get(i).copied().unwrap_or(false))
        .map(|i| {
            let (w, h) = p.bead_of(i);
            Bead {
                a: at(i - 1),
                b: at(i),
                w,
                h,
            }
        })
        .collect()
}

/// The lowest height where a bead of `a` comes too close to one of `b`, if any.
fn first_touch(a: &[Bead], b: &[Bead]) -> Option<f64> {
    let widest = a.iter().chain(b).map(|m| m.w.max(m.h)).fold(1.0, f64::max);
    let cell = widest * 1.5;
    let key = |p: P3| {
        [
            (p[0] / cell).floor() as i64,
            (p[1] / cell).floor() as i64,
            (p[2] / cell).floor() as i64,
        ]
    };
    // `b`'s moves, filed under every cell their bounding box covers.
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (k, m) in b.iter().enumerate() {
        let (lo, hi) = (key(min3(m.a, m.b)), key(max3(m.a, m.b)));
        let count = (0..3)
            .map(|d| (hi[d] - lo[d] + 1) as usize)
            .product::<usize>();
        if count > MAX_CELLS {
            for end in [m.a, m.b] {
                grid.entry(key(end)).or_default().push(k);
            }
            continue;
        }
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    grid.entry([x, y, z]).or_default().push(k);
                }
            }
        }
    }
    let mut lowest: Option<f64> = None;
    for m in a {
        for p in [m.a, m.b] {
            let c = key(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(list) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) else {
                            continue;
                        };
                        for &k in list {
                            let o = b[k];
                            let (q, across) = nearest_across(o.a, o.b, p);
                            let up = (q[2] - p[2]).abs();
                            if across < ACROSS * (m.w + o.w) / 2.0 && up < UP * (m.h + o.h) / 2.0 {
                                let z = p[2].min(q[2]);
                                lowest = Some(lowest.map_or(z, |l: f64| l.min(z)));
                            }
                        }
                    }
                }
            }
        }
    }
    lowest
}

/// The point of move a–b nearest to p across the bed, and how far that is across.
fn nearest_across(a: P3, b: P3, p: P3) -> (P3, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let u = if len2 > 1e-18 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = [a[0] + dx * u, a[1] + dy * u, a[2] + (b[2] - a[2]) * u];
    (q, (p[0] - q[0]).hypot(p[1] - q[1]))
}

fn min3(a: P3, b: P3) -> P3 {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

fn max3(a: P3, b: P3) -> P3 {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}
