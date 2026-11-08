//! What the plastic actually does: a simulation of the beads laid down, on a heightfield.
//!
//! A toolpath plans each move's bead (a line width and a layer height) and extrudes that bead's
//! volume. Where it lands depends on what's already there: the nozzle pushes the plastic into
//! the gap between itself and whatever is below it, so the same volume in a smaller gap comes
//! out wider, and in a larger gap narrower (or, with nothing under it at all, hanging in the
//! air). This keeps one heightfield of everything printed so far (a grid over the bed, the top
//! of the plastic in each cell) and, move by move in print order, works out each bead's real
//! height (the gap under the nozzle) and width (its volume spread over that height, by the same
//! stadium bead model as the G-code), then adds the bead to the heightfield. The idea is from
//! jüngerkühn's TOOLPATHS (a "global heightfield"); the code is our own.
//!
//! Each printed part starts on an empty bed (a part is its own G-code file).

use crate::bead::bead_area;
use crate::geom::P3;
use crate::toolpath::Toolpath;
use std::f64::consts::PI;

/// The bead came out much wider than planned: the layer below is too close (or the extrusion
/// too much), so the plastic squeezes out sideways.
pub const WIDE: u8 = 1;
/// Nothing under the nozzle for well over a layer: the bead hangs in the air (a bridge, or an
/// overhang past what the layer below holds up).
pub const UNSUPPORTED: u8 = 2;
/// The nozzle is at or below the plastic already printed there: it ploughs through it.
pub const COLLISION: u8 = 4;

/// A bead counts as wide at this many times its planned width.
pub const WIDE_RATIO: f64 = 1.5;
/// Unsupported: the gap under the nozzle is over this many planned layer heights.
pub const UNSUPPORTED_RATIO: f64 = 1.75;
/// A collision: the gap is under this share of the planned layer height.
pub const COLLISION_RATIO: f64 = 0.2;

/// "Just laid" never reaches back further than this (in quarter cells).
const RECENT_LIMIT: u32 = 100_000;

/// The most cells a part's heightfield may have (16 MB of f32).
const MAX_CELLS: usize = 4_000_000;
/// The most samples along the moves of one part; past it, the cells grow.
const MAX_SAMPLES: f64 = 3_000_000.0;

/// The simulated bead of every point of a toolpath (the move that ends there; zeros for travel).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Beads {
    pub widths: Vec<f32>,
    pub heights: Vec<f32>,
    /// `WIDE`, `UNSUPPORTED` and `COLLISION`, or'ed together.
    pub flags: Vec<u8>,
}

/// What the simulation found, over everything simulated.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Printing moves simulated.
    pub moves: usize,
    /// Moves flagged wide, unsupported, or colliding.
    pub wide: usize,
    pub unsupported: usize,
    pub collisions: usize,
    /// The widest bead relative to its plan, and the narrowest.
    pub max_ratio: f64,
    pub min_ratio: f64,
    /// The heightfield's cell size (mm), the finest in any part.
    pub cell: f64,
}

/// Simulates the toolpaths. `parts[i]` says which printed part path `i` belongs to (paths of
/// one part are simulated in order on one heightfield; `None` gets its own). `multiplier` is
/// the printer's extrusion multiplier (more plastic per move than planned when over 1).
pub fn simulate(
    paths: &[&Toolpath],
    parts: &[Option<usize>],
    multiplier: f64,
) -> Result<(Vec<Beads>, Summary), String> {
    if !(multiplier.is_finite() && multiplier > 0.0) {
        return Err(format!(
            "the extrusion multiplier must be more than 0, got {multiplier}"
        ));
    }
    let mut out: Vec<Beads> = paths
        .iter()
        .map(|p| Beads {
            widths: vec![0.0; p.points.len()],
            heights: vec![0.0; p.points.len()],
            flags: vec![0; p.points.len()],
        })
        .collect();
    let mut summary = Summary {
        min_ratio: f64::INFINITY,
        ..Summary::default()
    };
    // Group the paths by part, in order.
    let mut groups: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    for (i, part) in (0..paths.len()).map(|i| (i, parts.get(i).copied().flatten())) {
        match groups.iter_mut().find(|(p, _)| p.is_some() && *p == part) {
            Some((_, list)) => list.push(i),
            None => groups.push((part, vec![i])),
        }
    }
    for (_, members) in &groups {
        let group: Vec<&Toolpath> = members.iter().map(|&i| paths[i]).collect();
        let Some(mut field) = Field::for_paths(&group) else {
            continue;
        };
        summary.cell = if summary.cell > 0.0 {
            summary.cell.min(field.cell)
        } else {
            field.cell
        };
        for (&i, path) in members.iter().zip(&group) {
            field.run(path, multiplier, &mut out[i], &mut summary);
        }
    }
    if summary.moves == 0 {
        summary.min_ratio = 0.0;
    }
    Ok((out, summary))
}

/// The heightfield of one part: the top of the plastic over each cell of a grid on the bed.
struct Field {
    x0: f64,
    y0: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    top: Vec<f32>,
    /// What each cell was before the bead that last raised it, and when (how far the nozzle had
    /// printed, in quarter cells): the nozzle doesn't stand on the bead it laid a moment ago,
    /// only on what was there before.
    under: Vec<f32>,
    when: Vec<u32>,
    /// How far the nozzle has printed, in quarter cells (1 at the start).
    clock: u32,
    /// The same, exactly (mm).
    odometer: f64,
    /// The clock when the layer being printed started: beads of one layer lie side by side, so
    /// the nozzle stands on what was there before the layer, not on the layer itself (a loop
    /// closing on its own start, a brim line next to the last).
    layer_clock: u32,
}

impl Field {
    /// A grid over the printing moves (and a bead's width round them), a quarter of the
    /// narrowest planned bead per cell, coarser when the part is too big or too long for that.
    fn for_paths(paths: &[&Toolpath]) -> Option<Field> {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        let (mut widest, mut narrowest, mut length) = (0.0f64, f64::INFINITY, 0.0f64);
        for p in paths {
            for i in 1..p.points.len() {
                if p.travel.get(i).copied().unwrap_or(true) {
                    continue;
                }
                let (w, _) = p.bead_of(i);
                widest = widest.max(w);
                narrowest = narrowest.min(w);
                for q in [p.points[i - 1], p.points[i]] {
                    lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                    hi = [hi[0].max(q[0]), hi[1].max(q[1])];
                }
                length += dist_xy(p.points[i - 1], p.points[i]);
            }
        }
        if !(narrowest.is_finite() && narrowest > 0.0) {
            return None;
        }
        // Room for beads up to four times their planned width either side.
        let margin = 2.0 * widest + 1.0;
        let (sx, sy) = (hi[0] - lo[0] + 2.0 * margin, hi[1] - lo[1] + 2.0 * margin);
        let mut cell = narrowest / 4.0;
        cell = cell.max((sx * sy / MAX_CELLS as f64).sqrt());
        cell = cell.max(length / MAX_SAMPLES);
        if !(cell.is_finite() && cell > 0.0) {
            return None;
        }
        let nx = ((sx / cell).ceil() as usize).max(1);
        let ny = ((sy / cell).ceil() as usize).max(1);
        Some(Field {
            x0: lo[0] - margin,
            y0: lo[1] - margin,
            cell,
            nx,
            ny,
            top: vec![0.0; nx.checked_mul(ny)?],
            under: vec![0.0; nx * ny],
            when: vec![0; nx * ny],
            clock: 1,
            odometer: 0.0,
            layer_clock: 1,
        })
    }

    /// The highest plastic within `r` of (x, y) (the bed is 0), not counting the bead coming
    /// out of the nozzle right now (laid within `recent`), nor plastic of this same layer at the
    /// nozzle's own level (above `level`): beads of one layer lie side by side. A vase's turn
    /// below, a layer lower, still counts.
    fn floor(&self, x: f64, y: f64, r: f64, recent: u32, level: f64) -> f64 {
        let mut best = 0.0f32;
        let just = self.clock.saturating_sub(recent);
        let level = level as f32;
        self.each_cell(x - r, y - r, x + r, y + r, |cx, cy, k| {
            if (cx - x).hypot(cy - y) <= r + self.cell * 0.5 {
                let mine = self.when[k] > just
                    || (self.when[k] >= self.layer_clock && self.top[k] > level);
                best = best.max(if mine { self.under[k] } else { self.top[k] });
            }
        });
        f64::from(best)
    }

    /// Calls `f(centre x, centre y, index)` for every cell overlapping the box.
    fn each_cell(&self, xa: f64, ya: f64, xb: f64, yb: f64, mut f: impl FnMut(f64, f64, usize)) {
        let ix = |x: f64| (((x - self.x0) / self.cell).floor().max(0.0) as usize).min(self.nx - 1);
        let iy = |y: f64| (((y - self.y0) / self.cell).floor().max(0.0) as usize).min(self.ny - 1);
        for j in iy(ya)..=iy(yb) {
            let cy = self.y0 + (j as f64 + 0.5) * self.cell;
            for i in ix(xa)..=ix(xb) {
                let cx = self.x0 + (i as f64 + 0.5) * self.cell;
                f(cx, cy, j * self.nx + i);
            }
        }
    }

    /// Lays the bead `w` wide and `h` tall, its top at `z`, along a to b: every cell within half
    /// the width of the line rises to the bead's top there (a flat top, rounded sides).
    fn lay(&mut self, a: [f64; 2], b: [f64; 2], z: f64, w: f64, h: f64, recent: u32) {
        let half = w / 2.0;
        let r = (w.min(h) / 2.0).max(1e-9);
        let flat = (half - r).max(0.0);
        let (xa, xb) = (a[0].min(b[0]) - half, a[0].max(b[0]) + half);
        let (ya, yb) = (a[1].min(b[1]) - half, a[1].max(b[1]) + half);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let mut updates: Vec<(usize, f32)> = Vec::new();
        self.each_cell(xa, ya, xb, yb, |cx, cy, k| {
            let t = if len2 > 0.0 {
                (((cx - a[0]) * dx + (cy - a[1]) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = (cx - (a[0] + dx * t)).hypot(cy - (a[1] + dy * t));
            if d > half {
                return;
            }
            let height = if d <= flat {
                z
            } else {
                let e = d - flat;
                z - r + (r * r - e * e).max(0.0).sqrt()
            };
            updates.push((k, height as f32));
        });
        self.odometer += (b[0] - a[0]).hypot(b[1] - a[1]);
        self.clock = 1 + (self.odometer / (0.25 * self.cell)).min(4e9) as u32;
        let since = self.clock.saturating_sub(recent);
        for (k, height) in updates {
            if k < self.top.len() && height > self.top[k] {
                // A cell raised again while still recent keeps what was under the first bead.
                if self.when[k] < since || self.when[k] == 0 {
                    self.under[k] = self.top[k];
                }
                self.top[k] = height;
                self.when[k] = self.clock;
            }
        }
    }

    /// Simulates one toolpath on this field.
    fn run(&mut self, path: &Toolpath, multiplier: f64, out: &mut Beads, summary: &mut Summary) {
        self.layer_clock = self.clock + 1;
        for i in 1..path.points.len() {
            if path.layer_starts.contains(&i) {
                self.layer_clock = self.clock + 1;
            }
            if path.travel.get(i).copied().unwrap_or(true) {
                continue;
            }
            let (a, b) = (path.points[i - 1], path.points[i]);
            let (w0, h0) = path.bead_of(i);
            if !(w0 > 0.0 && h0 > 0.0) {
                continue;
            }
            // The plastic this move puts down, per mm of travel.
            let volume = bead_area(w0, h0) * path.flow.get(i).copied().unwrap_or(1.0) * multiplier;
            if volume.is_nan() || volume <= 0.0 {
                continue;
            }
            let n = ((dist_xy(a, b) / self.cell).ceil() as usize).clamp(1, 100_000);
            let (mut sum_w, mut sum_h, mut flags) = (0.0, 0.0, 0u8);
            let probe = (0.25 * w0).max(self.cell * 0.5);
            // The bead just laid stays out of the floor for two bead widths of travel.
            let recent = ((2.0 * w0 / (0.25 * self.cell)).ceil() as u32 + 4).min(RECENT_LIMIT);
            for k in 0..n {
                let (t0, t1) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
                let tm = (t0 + t1) / 2.0;
                let at = |t: f64| {
                    [
                        a[0] + (b[0] - a[0]) * t,
                        a[1] + (b[1] - a[1]) * t,
                        a[2] + (b[2] - a[2]) * t,
                    ]
                };
                let m = at(tm);
                let gap = m[2] - self.floor(m[0], m[1], probe, recent, m[2] - 0.5 * h0);
                let (w, h, f) = bead_in_gap(volume, gap, w0, h0);
                flags |= f;
                sum_w += w;
                sum_h += h;
                let (p, q) = (at(t0), at(t1));
                // Plastic squeezed out wider than twice the plan is pushed aside rather than
                // laid flat (and one collision would otherwise spread into the next).
                self.lay([p[0], p[1]], [q[0], q[1]], m[2], w.min(2.0 * w0), h, recent);
            }
            let (w, h) = (sum_w / n as f64, sum_h / n as f64);
            // A move extruding under half a bead (a ramp running out on a vase's closing turn)
            // has too little plastic to squeeze out or plough: only a gap under it counts.
            if path.flow.get(i).copied().unwrap_or(1.0) < 0.5 {
                flags &= UNSUPPORTED;
            }
            out.widths[i] = w as f32;
            out.heights[i] = h as f32;
            out.flags[i] = flags;
            summary.moves += 1;
            summary.wide += usize::from(flags & WIDE != 0);
            summary.unsupported += usize::from(flags & UNSUPPORTED != 0);
            summary.collisions += usize::from(flags & COLLISION != 0);
            summary.max_ratio = summary.max_ratio.max(w / w0);
            summary.min_ratio = summary.min_ratio.min(w / w0);
        }
        if path.points.len() > 1 {
            // The first point carries the first move's bead, as the drawing does.
            out.widths[0] = out.widths[1];
            out.heights[0] = out.heights[1];
        }
    }
}

/// The bead `volume` (mm² of plastic per mm of travel) makes in a gap `gap` under the nozzle,
/// planned `w0` × `h0`: its height is the gap, its width whatever holds the volume at that
/// height (the stadium model: area = (w − h)·h + π·(h/2)²). Too close, and it's flattened to a
/// fifth of a layer at most (the nozzle ploughs); too far, and it hangs at its planned size.
pub fn bead_in_gap(volume: f64, gap: f64, w0: f64, h0: f64) -> (f64, f64, u8) {
    let mut flags = 0;
    let h = if gap < COLLISION_RATIO * h0 {
        flags |= COLLISION;
        COLLISION_RATIO * h0
    } else if gap > UNSUPPORTED_RATIO * h0 {
        flags |= UNSUPPORTED;
        h0
    } else {
        gap
    };
    // (w − h)·h + π·h²/4 = volume  ⇒  w = volume / h + h · (1 − π/4); never narrower than tall
    // (then it's a round bead, of the same area).
    let mut w = volume / h + h * (1.0 - PI / 4.0);
    if w < h {
        w = (4.0 * volume / PI).sqrt();
    }
    if w > WIDE_RATIO * w0 {
        flags |= WIDE;
    }
    (w, h, flags)
}

fn dist_xy(a: P3, b: P3) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::toolpath::{MAX_PATH_POINTS, Settings, from_curves, stack};

    fn layers(width: f64, layer: f64, count: usize, z_scale: f64) -> Toolpath {
        let line = Curve::new(vec![[0.0, 0.0, 0.0], [40.0, 0.0, 0.0]], false);
        let one = from_curves(
            &[&line],
            Settings::new(width, layer, 20.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let mut path = stack(&one, count, 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        for p in &mut path.points {
            p[2] *= z_scale;
        }
        path
    }

    /// The model itself: the planned gap gives the planned bead; half the gap, about twice as
    /// wide; the volume is kept either way.
    #[test]
    fn a_bead_fills_the_gap_it_has() {
        let v = bead_area(2.0, 0.6);
        let (w, h, f) = bead_in_gap(v, 0.6, 2.0, 0.6);
        assert!((w - 2.0).abs() < 1e-9 && (h - 0.6).abs() < 1e-9 && f == 0);
        let (w, h, f) = bead_in_gap(v, 0.3, 2.0, 0.6);
        assert!((bead_area(w, h) - v).abs() < 1e-9);
        assert!(w > 3.5 && f & WIDE != 0, "{w}");
        assert_eq!(bead_in_gap(v, 0.05, 2.0, 0.6).2 & COLLISION, COLLISION);
        assert_eq!(bead_in_gap(v, 2.0, 2.0, 0.6).2 & UNSUPPORTED, UNSUPPORTED);
    }

    /// Straight layers stacked one layer height apart print as planned, all the way up.
    #[test]
    fn layers_one_layer_apart_print_as_planned() {
        let path = layers(2.0, 0.6, 5, 1.0);
        let (beads, s) = simulate(&[&path], &[None], 1.0).unwrap();
        assert_eq!(s.moves, 5);
        assert_eq!((s.wide, s.unsupported, s.collisions), (0, 0, 0));
        for i in (1..path.points.len()).filter(|&i| !path.travel[i]) {
            assert!(
                (beads[0].widths[i] - 2.0).abs() < 0.05,
                "{:?}",
                beads[0].widths
            );
            assert!(
                (beads[0].heights[i] - 0.6).abs() < 0.05,
                "{:?}",
                beads[0].heights
            );
        }
    }

    /// Layers squeezed to 60 % of their height (the same plastic per layer) come out wider from
    /// the second layer up; the first, on the bed, is squeezed too.
    #[test]
    fn squeezed_layers_come_out_wider() {
        let path = layers(2.0, 0.6, 4, 0.6);
        let (beads, s) = simulate(&[&path], &[None], 1.0).unwrap();
        let printed: Vec<f32> = (1..path.points.len())
            .filter(|&i| !path.travel[i])
            .map(|i| beads[0].widths[i])
            .collect();
        assert!(printed.iter().all(|&w| w > 3.0), "{printed:?}");
        assert_eq!(s.wide, 4);
        assert!(s.max_ratio > 1.5);
    }

    /// A layer printed with nothing under it (here, the second layer 3 layers up) hangs; a
    /// layer at the same height as the one before ploughs through it.
    #[test]
    fn gaps_and_collisions_are_flagged() {
        let mut path = layers(2.0, 0.6, 2, 1.0);
        let last = path.points.len() - 1;
        for i in (0..path.points.len())
            .rev()
            .take_while(|&i| i == last || !path.travel[i + 1])
        {
            path.points[i][2] = 2.4;
        }
        let (_, s) = simulate(&[&path], &[None], 1.0).unwrap();
        assert!(s.unsupported > 0, "{s:?}");
        let flat = layers(2.0, 0.6, 2, 0.0001);
        let (_, s) = simulate(&[&flat], &[None], 1.0).unwrap();
        assert!(s.collisions > 0, "{s:?}");
    }

    /// Two printed parts each start on an empty bed: the lid printed where the base was isn't
    /// sitting on it.
    #[test]
    fn each_part_starts_on_an_empty_bed() {
        let a = layers(2.0, 0.6, 1, 1.0);
        let (beads, s) = simulate(&[&a, &a], &[Some(0), Some(1)], 1.0).unwrap();
        assert_eq!((s.collisions, s.wide), (0, 0));
        assert_eq!(beads[0], beads[1]);
        // The same part twice: the second copy lands on the first.
        let (_, s) = simulate(&[&a, &a], &[Some(0), Some(0)], 1.0).unwrap();
        assert!(s.collisions > 0);
        assert!(simulate(&[&a], &[None], 0.0).is_err());
    }
}

/// What the simulation finds in each template, and how long it takes:
/// `cargo test --release -p easycad-core survey -- --ignored --nocapture`.
#[cfg(test)]
mod survey {
    #[test]
    #[ignore]
    fn templates() {
        for name in [
            "bend-stool",
            "lampshade",
            "vessel",
            "salt-shaker",
            "spiral-disc",
            "thread-connector",
        ] {
            let src =
                std::fs::read_to_string(format!("../../web/templates/{name}.easycad")).unwrap();
            let e = crate::eval::evaluate(&src, crate::eval::Quality::Final);
            let paths: Vec<&crate::toolpath::Toolpath> =
                e.drawing.paths.iter().map(|p| p.as_ref()).collect();
            let t = std::time::Instant::now();
            let (_, s) = super::simulate(&paths, &e.drawing.path_parts, 1.0).unwrap();
            println!("{name}: {:?} in {:?}", s, t.elapsed());
        }
    }
}
