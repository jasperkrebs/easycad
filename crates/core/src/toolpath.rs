//! Toolpaths: the nozzle's path in print order, with how much to extrude along each segment.
//!
//! `vase_spiral` follows EasyGCode's vase-mode conventions: one continuous helix that rises one
//! layer per turn, the first turn climbing from the bottom while extrusion ramps 0 → 100 %, the
//! height snapped to whole layers, and a flat cap turn at the top that ramps extrusion back to
//! 0 and ends on the seam.
//!
//! No G-code here yet: G-code drives real machines, so it only arrives once it is golden-tested
//! against EasyGCode (Phase 1b).

use crate::bead::bead_area;
use crate::geom::{Bounds, P2, P3, Xform, dist3, lerp3, signed_area_xy, signed_distance_near};
use crate::surface::Surface;
use serde::Serialize;

/// PLA, for the mass estimate.
const PLA_DENSITY_G_PER_MM3: f64 = 1.24e-3;
/// Above this a toolpath gets too heavy to compute and draw interactively.
pub const MAX_PATH_POINTS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Toolpath {
    pub points: Vec<P3>,
    /// Extrusion of the segment that ends at each point, as a fraction of a full bead (0..1).
    /// The first entry is unused.
    pub flow: Vec<f64>,
    /// Position along the spiral in turns: layer `k` at fraction `u` of the way round is
    /// `k + u`. Patterns such as `wave` use it to line up from one layer to the next.
    pub turn: Vec<f64>,
    pub samples_per_turn: usize,
    /// Number of spiral layers (turns), not counting the flat cap turn.
    pub layers: usize,
    /// Layer height: the rise per turn.
    pub layer: f64,
    /// Line width.
    pub width: f64,
    /// Print speed in mm/s.
    pub speed: f64,
}

/// Print statistics shown in the app (camelCase names for JavaScript).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolpathStats {
    pub layers: usize,
    /// Printed height after snapping to whole layers.
    pub printed_height_mm: f64,
    pub path_length_mm: f64,
    pub bead_area_mm2: f64,
    /// Extruded volume, with the start and cap ramps counted at their actual flow.
    pub volume_mm3: f64,
    pub mass_g_pla: f64,
    pub time_s: f64,
    pub flow_mm3_s: f64,
    /// Steepest outward lean between consecutive layers, in degrees from vertical.
    pub max_overhang_deg: f64,
    pub samples_per_turn: usize,
    pub path_points: usize,
    pub width_mm: f64,
    pub layer_mm: f64,
    pub speed_mm_s: f64,
    pub bounds: Bounds,
}

fn signed_area_2d(points: &[P2]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        / 2.0
}

/// Checks that `value` is a number within `min..=max`.
fn check(name: &str, value: f64, min: f64, max: f64, unit: &str) -> Result<(), String> {
    if !value.is_finite() || value < min || value > max {
        return Err(format!(
            "`{name}` must be between {min} and {max}{unit}, got {value}"
        ));
    }
    Ok(())
}

/// One continuous vase-mode spiral up a wall of closed sections, refusing to make more than
/// `max_points` points.
pub fn vase_spiral(
    wall: &Surface,
    layer: f64,
    width: f64,
    speed: f64,
    max_points: usize,
) -> Result<Toolpath, String> {
    check("layer", layer, 0.02, 10.0, " mm")?;
    check("width", width, 0.05, 20.0, " mm")?;
    check("speed", speed, 0.1, 1000.0, " mm/s")?;
    if !wall.closed {
        return Err(
            "needs a wall made of closed sections (circles, polygons, closed outlines)".into(),
        );
    }
    let n = wall.sections.len();
    let m = wall.samples();
    if n < 2 || m < 3 {
        return Err("needs a wall with at least 2 sections of 3 or more points".into());
    }
    let levels: Vec<f64> = wall
        .sections
        .iter()
        .map(|s| s.iter().map(|p| p[2]).sum::<f64>() / s.len().max(1) as f64)
        .collect();
    let (z0, z_top) = (
        levels.first().copied().unwrap_or(0.0),
        levels.last().copied().unwrap_or(0.0),
    );
    if levels.iter().any(|z| !z.is_finite()) {
        return Err("the wall has points at an invalid height".into());
    }
    if levels.iter().all(|z| (z - z0).abs() < 1e-9) {
        return Err(
            "all sections sit at the same height; move them up, e.g. `|> move(z: height * t)`"
                .into(),
        );
    }
    if let Some(i) = levels.windows(2).position(|w| w[1] <= w[0]) {
        return Err(format!(
            "the sections must be stacked upward, but section {} is not above section {}",
            i + 2,
            i + 1
        ));
    }

    let height = z_top - z0;
    let layers = (height / layer).round().max(1.0) as usize;
    let total = layers as f64 * layer;
    let needed = (layers + 1).saturating_mul(m).saturating_add(1);
    if needed > max_points.min(MAX_PATH_POINTS) {
        return Err(format!(
            "{layers} layers × {m} points per layer is too many points to compute ({needed}); use a larger layer height or a simpler outline"
        ));
    }

    // Which section parameter `v` sits at height `z` (sections may be unevenly spaced).
    let v_at = |z: f64| -> f64 {
        let i = levels
            .partition_point(|l| *l <= z)
            .saturating_sub(1)
            .min(n - 2);
        let (lo, hi) = (
            levels.get(i).copied().unwrap_or(0.0),
            levels.get(i + 1).copied().unwrap_or(1.0),
        );
        i as f64 + ((z - lo) / (hi - lo)).clamp(0.0, 1.0)
    };

    let mut path = Toolpath {
        points: Vec::with_capacity(needed),
        flow: Vec::with_capacity(needed),
        turn: Vec::with_capacity(needed),
        samples_per_turn: m,
        layers,
        layer,
        width,
        speed,
    };
    let mut push = |p: P3, flow: f64, turn: f64| {
        path.points.push(p);
        path.flow.push(flow);
        path.turn.push(turn);
    };
    // The spiral: z rises one layer per turn. The wall is stretched over the snapped height so
    // the whole surface is printed.
    for k in 0..layers {
        for j in 0..m {
            let u = j as f64 / m as f64;
            let rise = layer * (k as f64 + u);
            let mut p = wall.point_at(v_at(z0 + height * rise / total), j);
            p[2] = z0 + rise;
            push(p, if k == 0 { u } else { 1.0 }, k as f64 + u);
        }
    }
    // Flat cap: one more turn at the top while extrusion ramps back to zero, ending on the seam
    // so the rim closes cleanly.
    let top = (n - 1) as f64;
    for j in 0..=m {
        let u = j as f64 / m as f64;
        let mut p = wall.point_at(top, j % m);
        p[2] = z0 + total;
        push(p, 1.0 - u, layers as f64 + u);
    }
    Ok(path)
}

impl Toolpath {
    /// Rotations, moves and scaling in X/Y. Scaling in Z would change the layer height, so it
    /// is refused: scale the shape before making the toolpath instead.
    pub fn transformed(&self, x: &Xform, z_scale: f64) -> Result<Toolpath, String> {
        if (z_scale - 1.0).abs() > 1e-12 {
            return Err("can't scale a toolpath in z (that changes the layer height); scale the shape before the toolpath instead".into());
        }
        Ok(Toolpath {
            points: self.points.iter().map(|p| x.apply(*p)).collect(),
            ..self.clone()
        })
    }

    pub fn stats(&self) -> ToolpathStats {
        let mut length = 0.0;
        let mut extruded = 0.0;
        for (i, w) in self.points.windows(2).enumerate() {
            let d = dist3(w[0], w[1]);
            length += d;
            extruded += d * self.flow.get(i + 1).copied().unwrap_or(1.0);
        }
        let area = bead_area(self.width, self.layer);
        let volume = extruded * area;
        ToolpathStats {
            layers: self.layers,
            printed_height_mm: self.layers as f64 * self.layer,
            path_length_mm: length,
            bead_area_mm2: area,
            volume_mm3: volume,
            mass_g_pla: volume * PLA_DENSITY_G_PER_MM3,
            time_s: length / self.speed,
            flow_mm3_s: self.speed * area,
            max_overhang_deg: self.max_overhang_deg(),
            samples_per_turn: self.samples_per_turn,
            path_points: self.points.len(),
            width_mm: self.width,
            layer_mm: self.layer,
            speed_mm_s: self.speed,
            bounds: Bounds::of(&self.points),
        }
    }

    /// Steepest overhang: how far a point on one layer sticks out past the layer below, turned
    /// into an angle from vertical. Sampled on up to 24 evenly spaced layer pairs. Point `j`
    /// of one turn sits right above point `j` of the turn below, so only that part of the lower
    /// ring is searched.
    fn max_overhang_deg(&self) -> f64 {
        let m = self.samples_per_turn;
        if self.layers < 2 || m < 3 {
            return 0.0;
        }
        let ring = |k: usize| -> Vec<P2> {
            self.points
                .get(k * m..(k + 1) * m)
                .map_or_else(Vec::new, |s| s.iter().map(|p| [p[0], p[1]]).collect())
        };
        let pairs = (self.layers - 1).min(24);
        let mut worst: f64 = 0.0;
        for s in 0..pairs {
            let k = s * (self.layers - 1) / pairs;
            let (lower, upper) = (ring(k), ring(k + 1));
            let ccw = signed_area_2d(&lower) >= 0.0;
            for (j, q) in upper.iter().enumerate() {
                if let Some(d) = signed_distance_near(*q, &lower, j, 16, ccw)
                    && d > 0.0
                {
                    worst = worst.max(d.atan2(self.layer).to_degrees());
                }
            }
        }
        worst
    }
}

/// Pushes points sideways by `amplitude · sin(2π · (count · s + phase))`, perpendicular to the
/// path in the XY plane, where `s` is the position along the path in turns (for a toolpath) or
/// in loops (for a curve: 0 → 1 by length). `outward` is +1 for counter-clockwise loops (the
/// right-hand side is outside) and −1 for clockwise ones.
fn displace(
    points: &[P3],
    s: &[f64],
    closed: bool,
    outward: f64,
    amplitude: f64,
    count: f64,
    phase: f64,
) -> Vec<P3> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let p = points.get(i).copied().unwrap_or([0.0; 3]);
            let (prev, next) = if closed {
                (points.get((i + n - 1) % n), points.get((i + 1) % n))
            } else {
                (
                    points.get(i.saturating_sub(1)),
                    points.get((i + 1).min(n - 1)),
                )
            };
            let (Some(a), Some(b)) = (prev, next) else {
                return p;
            };
            let (tx, ty) = (b[0] - a[0], b[1] - a[1]);
            let len = tx.hypot(ty);
            if len <= 1e-12 {
                return p;
            }
            let d = amplitude
                * (std::f64::consts::TAU * (count * s.get(i).copied().unwrap_or(0.0) + phase))
                    .sin();
            [
                p[0] + outward * d * ty / len,
                p[1] - outward * d * tx / len,
                p[2],
            ]
        })
        .collect()
}

/// Waves a toolpath. Segments are split first when needed, so each wave gets at least 12
/// points; the flat cap and the ramps keep their extrusion.
pub fn wave_path(
    path: &Toolpath,
    amplitude: f64,
    count: f64,
    phase: f64,
    max_points: usize,
) -> Result<Toolpath, String> {
    check_wave(amplitude, count, phase)?;
    let m = path.samples_per_turn.max(1);
    let wanted = (12.0 * count.abs()).ceil();
    let split = if (m as f64) < wanted {
        (wanted / m as f64).ceil() as usize
    } else {
        1
    };
    let total = path
        .points
        .len()
        .saturating_sub(1)
        .saturating_mul(split)
        .saturating_add(1);
    if total > max_points.min(MAX_PATH_POINTS) {
        return Err(format!(
            "this wave needs too many points ({total}); use fewer waves or a larger layer height"
        ));
    }
    let mut points = Vec::with_capacity(total);
    let mut flow = Vec::with_capacity(total);
    let mut turn = Vec::with_capacity(total);
    for i in 0..path.points.len() {
        let (p, f, t) = (
            path.points[i],
            path.flow.get(i).copied().unwrap_or(1.0),
            path.turn.get(i).copied().unwrap_or(0.0),
        );
        if i > 0 && split > 1 {
            let (q, tq) = (
                path.points[i - 1],
                path.turn.get(i - 1).copied().unwrap_or(0.0),
            );
            for k in 1..split {
                let a = k as f64 / split as f64;
                points.push(lerp3(q, p, a));
                flow.push(f);
                turn.push(tq + (t - tq) * a);
            }
        }
        points.push(p);
        flow.push(f);
        turn.push(t);
    }
    // A spiral is one long open path, but each turn is a loop that tells which side is outside.
    let first_turn = points.get(..(m * split).min(points.len())).unwrap_or(&[]);
    let outward = if signed_area_xy(first_turn) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let waved = displace(&points, &turn, false, outward, amplitude, count, phase);
    Ok(Toolpath {
        points: waved,
        flow,
        turn,
        samples_per_turn: m * split,
        ..path.clone()
    })
}

/// Waves a ring of points (a curve or one section of a surface), resampled first if it has
/// fewer than 12 points per wave.
pub fn wave_ring(
    points: &[P3],
    closed: bool,
    amplitude: f64,
    count: f64,
    phase: f64,
    max_points: usize,
) -> Result<Vec<P3>, String> {
    check_wave(amplitude, count, phase)?;
    let curve = crate::curve::Curve {
        points: points.to_vec(),
        closed,
    };
    let wanted = ((12.0 * count.abs()).ceil() as usize).max(points.len());
    if wanted > max_points {
        return Err(format!(
            "this wave needs too many points ({wanted}); use fewer waves"
        ));
    }
    let curve = if wanted > points.len() {
        curve.resample(wanted)
    } else {
        curve
    };
    // Position along the curve, 0 → 1 by length.
    let total = curve.length().max(1e-12);
    let mut walked = 0.0;
    let mut s = Vec::with_capacity(curve.points.len());
    for (i, p) in curve.points.iter().enumerate() {
        if i > 0 {
            walked += dist3(curve.points[i - 1], *p);
        }
        s.push(walked / total);
    }
    let outward = if signed_area_xy(&curve.points) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    Ok(displace(
        &curve.points,
        &s,
        closed,
        outward,
        amplitude,
        count,
        phase,
    ))
}

fn check_wave(amplitude: f64, count: f64, phase: f64) -> Result<(), String> {
    check("amplitude", amplitude, -100.0, 100.0, " mm")?;
    check("count", count, -2000.0, 2000.0, "")?;
    check("phase", phase, -1e6, 1e6, "")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;

    fn wall(top_scale: f64, twist: f64) -> Surface {
        let outline = Curve::star(7, 45.0, 45.0 * 0.82).smooth(16).unwrap();
        let sections: Vec<Curve> = (0..32)
            .map(|i| {
                let t = i as f64 / 31.0;
                outline
                    .transformed(&Xform::scale(
                        1.0 + (top_scale - 1.0) * t,
                        1.0 + (top_scale - 1.0) * t,
                        1.0,
                    ))
                    .transformed(&Xform::rotate_z(twist * t))
                    .transformed(&Xform::translate(0.0, 0.0, 160.0 * t))
            })
            .collect();
        Surface::loft(&sections.iter().collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn spiral_snaps_to_whole_layers_and_rises_monotonically() {
        let w = wall(0.8, 90.0);
        let p = vase_spiral(&w, 0.3, 0.6, 40.0, MAX_PATH_POINTS).unwrap();
        // 160 / 0.3 = 533.3 → 533 whole layers, like EasyGCode's height snapping.
        assert_eq!(p.layers, 533);
        let z: Vec<f64> = p.points.iter().map(|q| q[2]).collect();
        assert_eq!(z.first().copied(), Some(0.0));
        assert!(z.windows(2).all(|w| w[1] >= w[0]));
        assert!((z.last().copied().unwrap() - 159.9).abs() < 1e-9);
        assert_eq!(p.points.len(), (533 + 1) * 224 + 1);
        // It starts on the bottom section's seam and the cap ends on the top section's seam.
        assert_eq!(p.points[0], w.sections[0][0]);
        let (end, seam) = (*p.points.last().unwrap(), w.sections[31][0]);
        assert!((end[0] - seam[0]).hypot(end[1] - seam[1]) < 1e-9);
        assert_eq!(p.flow.last().copied(), Some(0.0));
    }

    #[test]
    fn ramps_make_volume_less_than_full_flow() {
        let s = vase_spiral(&wall(0.8, 90.0), 0.3, 0.6, 40.0, MAX_PATH_POINTS)
            .unwrap()
            .stats();
        let full = s.path_length_mm * s.bead_area_mm2;
        assert!(s.volume_mm3 < full && s.volume_mm3 > 0.95 * full);
    }

    #[test]
    fn overhangs() {
        let straight = vase_spiral(&wall(1.0, 0.0), 0.3, 0.6, 40.0, MAX_PATH_POINTS)
            .unwrap()
            .stats();
        assert!(
            straight.max_overhang_deg < 0.5,
            "{}",
            straight.max_overhang_deg
        );
        let flare = vase_spiral(&wall(2.0, 0.0), 0.3, 0.6, 40.0, MAX_PATH_POINTS)
            .unwrap()
            .stats();
        // Radius grows 45 mm over 160 mm of height: about atan(45/160) ≈ 15.7° at the tips.
        assert!(
            flare.max_overhang_deg > 10.0 && flare.max_overhang_deg < 25.0,
            "{}",
            flare.max_overhang_deg
        );
        let taper = vase_spiral(&wall(0.5, 0.0), 0.3, 0.6, 40.0, MAX_PATH_POINTS)
            .unwrap()
            .stats();
        assert!(taper.max_overhang_deg < 0.5);
    }

    #[test]
    fn bad_walls_and_settings_are_errors() {
        let w = wall(1.0, 0.0);
        assert!(vase_spiral(&w, 0.0, 0.6, 40.0, MAX_PATH_POINTS).is_err());
        assert!(vase_spiral(&w, f64::NAN, 0.6, 40.0, MAX_PATH_POINTS).is_err());
        assert!(vase_spiral(&w, 0.3, 0.6, 0.0, MAX_PATH_POINTS).is_err());
        let flat = Surface {
            sections: vec![w.sections[0].clone(), w.sections[0].clone()],
            closed: true,
            capped: false,
        };
        assert!(
            vase_spiral(&flat, 0.3, 0.6, 40.0, MAX_PATH_POINTS)
                .unwrap_err()
                .contains("same height")
        );
        let mut upside_down = w.clone();
        upside_down.sections.reverse();
        assert!(
            vase_spiral(&upside_down, 0.3, 0.6, 40.0, MAX_PATH_POINTS)
                .unwrap_err()
                .contains("stacked upward")
        );
        let tall = Surface {
            sections: vec![
                w.sections[0].clone(),
                w.sections[31]
                    .iter()
                    .map(|p| [p[0], p[1], 5000.0])
                    .collect(),
            ],
            ..w
        };
        assert!(
            vase_spiral(&tall, 0.02, 0.6, 40.0, MAX_PATH_POINTS)
                .unwrap_err()
                .contains("too many points")
        );
    }

    #[test]
    fn whole_waves_stack_and_half_waves_alternate() {
        let c = Curve::circle(40.0, 96);
        let top = c.transformed(&Xform::translate(0.0, 0.0, 30.0));
        let w = Surface::loft(&[&c, &top]).unwrap();
        let p = vase_spiral(&w, 0.5, 0.8, 30.0, MAX_PATH_POINTS).unwrap();
        let r = |q: P3| q[0].hypot(q[1]);
        let flutes = wave_path(&p, 1.0, 9.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(flutes.samples_per_turn, 192); // 96 points < 12 × 9 → each segment split in two
        let m = flutes.samples_per_turn;
        let layer = |path: &Toolpath, k: usize| -> Vec<f64> {
            path.points[k * m..(k + 1) * m]
                .iter()
                .map(|q| r(*q))
                .collect()
        };
        // Whole waves: every layer is the same, so the bumps stack into flutes.
        let (a, b) = (layer(&flutes, 10), layer(&flutes, 11));
        assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6));
        let peak = a.iter().copied().fold(0.0, f64::max);
        assert!((peak - 41.0).abs() < 0.05, "{peak}");
        // Half waves: each layer is the opposite of the one below, so they weave.
        let weave = wave_path(&p, 1.0, 9.5, 0.0, MAX_PATH_POINTS).unwrap();
        let (a, b) = (layer(&weave, 10), layer(&weave, 11));
        let spread = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(spread > 1.9, "{spread}");
        // Extrusion ramps are kept: each split segment repeats the flow of the segment it came from.
        let mut expected = Vec::new();
        for (i, x) in p.flow.iter().enumerate() {
            if i > 0 {
                expected.push(*x);
            }
            expected.push(*x);
        }
        assert_eq!(weave.flow, expected);
    }

    #[test]
    fn wave_ring_pushes_outward() {
        let c = Curve::circle(10.0, 8);
        let w = wave_ring(&c.points, true, 1.0, 4.0, 0.25, 1000).unwrap();
        assert_eq!(w.len(), 48);
        // phase 0.25 puts a peak at the seam.
        assert!((w[0][0].hypot(w[0][1]) - 11.0).abs() < 1e-9);
        let cw: Vec<P3> = c.points.iter().rev().copied().collect();
        let w = wave_ring(&cw, true, 1.0, 4.0, 0.25, 1000).unwrap();
        assert!((w[0][0].hypot(w[0][1]) - 11.0).abs() < 1e-9);
    }
}
