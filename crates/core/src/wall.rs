//! Which way the wall goes up at each point of a toolpath: from the bead underneath (one turn
//! earlier on a spiral, else the nearest point of the layer before) to this one, square to the
//! direction of travel. A preview tilts the nozzle and the beads' cross-sections along it, as a
//! robot arm would hold the nozzle along the wall; printing never uses it (a Cartesian printer's
//! nozzle stays upright). On the bed it stays upright, whatever lies flat there (a closed shade's
//! disc) too: it leans along the wall only as the wall leaves the bed.

use crate::geom::{P3, add3, dot3, lerp3, scale3, sub3, unit3};
use crate::toolpath::Toolpath;
use std::collections::HashMap;

const UP: P3 = [0.0, 0.0, 1.0];

/// One unit vector per point of `path`: up the wall there. Straight up where there's nothing
/// underneath (the first layer or turn, travel ends) or where it can't tell, and on the bed:
/// within two of the widest beads above the lowest printed point, it eases from upright to along
/// the wall (an S-curve), so a flat bottom prints flat and the nozzle only leans once the wall
/// has left the bed.
pub fn up_the_wall(path: &Toolpath) -> Vec<P3> {
    let n = path.points.len();
    let mut below: Vec<Option<P3>> = vec![None; n];
    under_on_spirals(path, &mut below);
    under_by_layers(path, &mut below);
    // Where the bead underneath is (almost) the same point, as on a flat closing turn that runs
    // round on top of the spiral's end, the direction carries on from the point before.
    let mut out = Vec::with_capacity(n);
    let mut last = UP;
    for (i, under) in below.iter().enumerate() {
        let Some(q) = *under else {
            last = UP;
            out.push(UP);
            continue;
        };
        let mut v = sub3(path.points[i], q);
        if let Some(t) = travel_direction(path, i) {
            v = sub3(v, scale3(t, dot3(v, t)));
        }
        if dot3(v, v) > 1e-6
            && let Some(v) = unit3(v)
        {
            last = v;
        }
        out.push(last);
    }
    // Upright on the bed, along the wall once clear of it.
    let printed = || (0..n).filter(|&i| !path.travel.get(i).copied().unwrap_or(true));
    let floor = printed()
        .map(|i| path.points[i][2])
        .fold(f64::INFINITY, f64::min);
    let ease = 2.0 * printed().map(|i| path.bead_of(i).0).fold(0.0, f64::max);
    if floor.is_finite() && ease > 0.0 {
        for (i, v) in out.iter_mut().enumerate() {
            let t = ((path.points[i][2] - floor) / ease).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            if t < 1.0
                && let Some(u) = unit3(add3(scale3(UP, 1.0 - t), scale3(*v, t)))
            {
                *v = u;
            }
        }
    }
    out
}

/// The direction of travel at point `i` (from the point before to the point after).
fn travel_direction(path: &Toolpath, i: usize) -> Option<P3> {
    let pts = &path.points;
    let a = pts[i.saturating_sub(1)];
    let b = pts[(i + 1).min(pts.len() - 1)];
    unit3(sub3(b, a))
}

/// On a spiral, the bead underneath is the same place one turn earlier: where `turn` is one
/// less, interpolated between the two points around it. Runs of rising `turn` are separate
/// spirals; a first turn has nothing underneath on its own spiral.
fn under_on_spirals(path: &Toolpath, below: &mut [Option<P3>]) {
    let turn = &path.turn;
    if turn.len() != path.points.len() {
        return;
    }
    let mut run_start = 0;
    let mut j = 0;
    for i in 0..turn.len() {
        if turn[i] <= 0.0 || (i > 0 && turn[i] < turn[i - 1] - 1e-9) {
            run_start = i;
            j = i;
            continue;
        }
        let target = turn[i] - 1.0;
        if target < turn[run_start] - 1e-9 || path.travel.get(i).copied().unwrap_or(false) {
            continue;
        }
        j = j.max(run_start);
        while j + 1 < i && turn[j + 1] <= target {
            j += 1;
        }
        let (a, b) = (turn[j], turn[j + 1]);
        let u = if b - a > 1e-12 {
            ((target - a) / (b - a)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Only a bead lower down is underneath: a flat spiral (a disc) has its last turn beside it.
        let q = lerp3(path.points[j], path.points[j + 1], u);
        if q[2] < path.points[i][2] - 1e-3 {
            below[i] = Some(q);
        }
    }
}

/// Elsewhere, the nearest point on the layer before's extruding moves, found through a grid
/// over the bed.
fn under_by_layers(path: &Toolpath, below: &mut [Option<P3>]) {
    let n = path.points.len();
    let starts = &path.layer_starts;
    for l in 1..starts.len() {
        let (a, b) = (starts[l - 1], starts[l]);
        let c = starts.get(l + 1).copied().unwrap_or(n);
        if a >= b || b >= c || c > n {
            continue;
        }
        // Cells two of the layer before's widest beads across (its own beads, not the whole
        // print's: one wide primer line would make every layer's cells, and its search, big).
        let cell = ((a + 1)..b)
            .filter(|&k| !path.travel.get(k).copied().unwrap_or(false))
            .map(|k| path.bead_of(k).0)
            .fold(0.0, f64::max)
            .max(0.5)
            * 2.0;
        let key = |p: P3| ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64);
        // The layer before's extruding moves, by grid cell (each move in the cells of its ends).
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for k in (a + 1)..b {
            if path.travel.get(k).copied().unwrap_or(false) {
                continue;
            }
            let (ka, kb) = (key(path.points[k - 1]), key(path.points[k]));
            grid.entry(ka).or_default().push(k);
            if kb != ka {
                grid.entry(kb).or_default().push(k);
            }
        }
        for (i, slot) in below.iter_mut().enumerate().take(c).skip(b) {
            if slot.is_some() || path.travel.get(i).copied().unwrap_or(false) {
                continue;
            }
            let p = path.points[i];
            let (cx, cy) = key(p);
            let mut best: Option<(f64, P3)> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for &k in grid
                        .get(&(cx.saturating_add(dx), cy.saturating_add(dy)))
                        .into_iter()
                        .flatten()
                    {
                        let q = nearest_on_segment(path.points[k - 1], path.points[k], p);
                        let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, q));
                        }
                    }
                }
            }
            *slot = best.map(|(_, q)| q);
        }
    }
}

/// The point of segment a–b nearest to p, measured across the bed (x and y only).
fn nearest_on_segment(a: P3, b: P3, p: P3) -> P3 {
    let ab = [b[0] - a[0], b[1] - a[1], 0.0];
    let len2 = dot3(ab, ab);
    if len2 < 1e-18 {
        return a;
    }
    let ap = [p[0] - a[0], p[1] - a[1], 0.0];
    let u = (dot3(ap, ab) / len2).clamp(0.0, 1.0);
    add3(a, scale3(sub3(b, a), u))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Quality, step_values};
    use easycad_lang::{Literal, set_values};

    fn lampshade(values: &[(&str, Literal)]) -> std::rc::Rc<Toolpath> {
        let template = include_str!("../../../web/templates/lampshade.easycad");
        let values: Vec<(String, Literal)> = values
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let (e, _) = step_values(&set_values(template, &values).unwrap(), Quality::Final);
        assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
        e.print.unwrap().path
    }

    #[test]
    fn the_wall_goes_up_the_throat_and_out_along_an_open_bell() {
        let p = lampshade(&[
            ("shape", Literal::Text("bell".into())),
            ("max_angle", Literal::Number(90.0)),
            ("slicing", Literal::Text("surface".into())),
            ("brim", Literal::Bool(false)),
        ]);
        let up = up_the_wall(&p);
        assert_eq!(up.len(), p.points.len());
        let top = p.points.iter().map(|q| q[2]).fold(0.0, f64::max);
        for (i, q) in p.points.iter().enumerate() {
            let v = up[i];
            assert!((dot3(v, v) - 1.0).abs() < 1e-9);
            let out = [q[0], q[1], 0.0];
            let r = out[0].hypot(out[1]);
            // The throat (a straight cylinder, below 15 mm): straight up.
            if q[2] > 5.0 && q[2] < 15.0 {
                assert!(v[2] > 0.999, "{i}: {v:?}");
            }
            // The last turns on the rim, where it runs flat: outward, about level.
            if p.turn[i] > 1.0 && (q[2] - top).abs() < 0.5 && r > 70.0 {
                assert!(v[2].abs() < 0.2 && dot3(v, out) / r > 0.95, "{i}: {v:?}");
            }
        }
    }

    #[test]
    fn layers_without_a_spiral_find_the_bead_underneath() {
        // The bend stool prints layer by layer: nothing underneath its first layer, and every
        // later extruding point has a bead below it within a few line widths.
        let template = include_str!("../../../web/templates/bend-stool.easycad");
        let (e, _) = step_values(template, Quality::Final);
        assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
        let p = e.print.unwrap().path;
        let up = up_the_wall(&p);
        let second = p.layer_starts[1];
        assert!(up[..second].iter().all(|v| *v == UP));
        let leaning = up[second..].iter().filter(|v| v[2] < 0.999).count();
        assert!(leaning > 0, "its legs lean");
        assert!(up.iter().all(|v| v[2] > 0.0), "never downwards");
    }

    /// The owner's closed cone (13 Nov 2026): its disc lies flat on the bed, so the nozzle stands
    /// upright over all of it (a disc's spiral has its last turn beside it, not underneath), and
    /// on the fillet it leans out only as the wall leaves the bed, along the wall well clear of it.
    #[test]
    fn a_flat_bottom_prints_upright_and_the_wall_leans_once_off_the_bed() {
        let p = lampshade(&[
            ("opening", Literal::Number(350.0)),
            ("transition", Literal::Number(151.0)),
            ("throat", Literal::Number(40.0)),
            ("fillet", Literal::Number(40.0)),
            ("closed", Literal::Bool(true)),
            ("closed_fillet", Literal::Number(30.0)),
        ]);
        let up = up_the_wall(&p);
        let disc = p.points.iter().find(|q| q[2] > 0.0).unwrap()[2];
        let on_disc: Vec<usize> = (0..p.points.len())
            .filter(|&i| !p.travel[i] && p.points[i][2] == disc)
            .collect();
        assert!(on_disc.len() > 100);
        assert!(on_disc.iter().all(|&i| up[i] == UP), "the disc: upright");
        // Every point's lean grows with its height off the bed: under a bead's width, nearly
        // upright; well clear of it, along the cone's wall (it leans about 46° there).
        for (i, q) in p.points.iter().enumerate() {
            let lean = up[i][2].clamp(-1.0, 1.0).acos().to_degrees();
            if q[2] - disc < 1.0 {
                assert!(lean < 5.0, "{i} at {:.2}: {lean:.1}°", q[2]);
            }
            if q[2] > 60.0 && q[2] < 120.0 && !p.travel[i] {
                assert!((lean - 46.0).abs() < 8.0, "{i} at {:.2}: {lean:.1}°", q[2]);
            }
        }
    }
}
