//! Toolpaths: the nozzle's path in print order, with what each move does.
//!
//! A toolpath is made from any curves (`from_curves`), or by a strategy such as `vase_spiral`,
//! and can be stacked (`stack`), joined (`join`) and patterned (`wave_path`). Every move ends at
//! a point and is either a travel (no extrusion) or extrudes a fraction of a full bead, using
//! EasyGCode's bead model with the width, layer height and speed of its part. A move can carry
//! its own layer height (non-planar layers are thicker in some places than others), and a part
//! can be printed at a set volumetric flow instead of a set speed: then each move's speed is the
//! flow divided by its bead's cross-section, as in EasyGCode's flow mode.
//!
//! `vase_spiral` follows EasyGCode's vase-mode conventions (its lamp/vessel generators in
//! gcode.js): one continuous helix that rises one layer per turn; the opening turn holds the
//! section the second turn starts on while extrusion ramps 0 → 100 % (averaged over each
//! segment); the height snaps to whole layers; and a flat closing turn ramps extrusion back to 0
//! and ends on the seam.

use crate::bead::bead_area;
use crate::curve::Curve;
use crate::field::Field;
use crate::geom::{Bounds, P2, P3, Xform, dist3, lerp3, signed_area_xy, signed_distance_near};
use crate::surface::Surface;
use serde::Serialize;

/// PLA, for the mass estimate.
const PLA_DENSITY_G_PER_MM3: f64 = 1.24e-3;
/// Above this a toolpath gets too heavy to compute and draw interactively.
pub const MAX_PATH_POINTS: usize = 1_000_000;
/// Travel speed assumed for time estimates (EasyGCode's default, 6000 mm/min).
const TRAVEL_SPEED_MM_S: f64 = 100.0;

/// How one part of a toolpath prints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Settings {
    /// Line width in mm.
    pub width: f64,
    /// Layer height in mm.
    pub layer: f64,
    /// Print speed in mm/s (with `flow`: the speed of a full-height bead).
    pub speed: f64,
    /// Volumetric flow in mm³/s. When set, each move's speed follows from it: flow ÷ bead area.
    pub flow: Option<f64>,
}

impl Settings {
    pub fn new(width: f64, layer: f64, speed: f64) -> Result<Settings, String> {
        check("width", width, 0.05, 20.0, " mm")?;
        check("layer", layer, 0.02, 10.0, " mm")?;
        check("speed", speed, 0.1, 1000.0, " mm/s")?;
        Ok(Settings {
            width,
            layer,
            speed,
            flow: None,
        })
    }

    /// Printed at a set volumetric flow (mm³/s) instead of a set speed.
    pub fn with_flow(width: f64, layer: f64, flow: f64) -> Result<Settings, String> {
        check("width", width, 0.05, 20.0, " mm")?;
        check("layer", layer, 0.02, 10.0, " mm")?;
        check("flow", flow, 0.1, 1000.0, " mm³/s")?;
        let speed = flow / bead_area(width, layer);
        if speed > 1000.0 {
            return Err(format!(
                "a flow of {} mm³/s is {} mm/s with this bead; lower the flow",
                easycad_lang::format_number(flow),
                easycad_lang::format_number(speed.round())
            ));
        }
        Ok(Settings {
            width,
            layer,
            speed,
            flow: Some(flow),
        })
    }

    /// By speed, or by flow when `flow` is more than 0 (how the steps take them).
    pub fn from_inputs(width: f64, layer: f64, speed: f64, flow: f64) -> Result<Settings, String> {
        if flow > 0.0 {
            Settings::with_flow(width, layer, flow)
        } else {
            Settings::new(width, layer, speed)
        }
    }

    /// The bead's cross-section in mm².
    pub fn area(&self) -> f64 {
        bead_area(self.width, self.layer)
    }

    /// The speed (mm/s) of a move whose bead has this cross-section.
    pub fn speed_for(&self, area: f64) -> f64 {
        match self.flow {
            Some(q) => q / area.max(1e-6),
            None => self.speed,
        }
    }

    /// The G-code feed (mm/min) of a move whose bead has this cross-section, computed the way
    /// EasyGCode does in flow mode (flow · 60 / area), so both round identically.
    pub fn feed_for(&self, area: f64) -> f64 {
        match self.flow {
            Some(q) => (q * 60.0) / area.max(1e-6),
            None => self.speed * 60.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toolpath {
    /// Nozzle positions in print order. The first is reached by a travel move.
    pub points: Vec<P3>,
    /// Whether the move that ends at each point is a travel (no extrusion).
    pub travel: Vec<bool>,
    /// Extrusion of the move that ends at each point, as a fraction of a full bead (0..1).
    pub flow: Vec<f64>,
    /// Position along a spiral in turns (turn `k` at fraction `u` of the way round is `k + u`),
    /// so patterns line up from one turn to the next. 0 where there is no spiral.
    pub turn: Vec<f64>,
    /// Which of `parts` prints the move that ends at each point.
    pub part: Vec<u16>,
    /// The layer height of the move that ends at each point, where it differs from its part's
    /// (non-planar layers); 0 means the part's layer height.
    pub height: Vec<f64>,
    pub parts: Vec<Settings>,
    /// Index of the first point of each layer (the fan comes on after the first).
    pub layer_starts: Vec<usize>,
    /// Points per turn when the whole path is one regular spiral (for the overhang check).
    pub samples_per_turn: usize,
    /// Number of layers (for a spiral: turns, not counting the closing turn).
    pub layers: usize,
}

/// Print statistics shown in the app (camelCase names for JavaScript).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolpathStats {
    pub layers: usize,
    /// Height of the highest extruding move.
    pub printed_height_mm: f64,
    /// Length of the extruding moves.
    pub path_length_mm: f64,
    pub travel_length_mm: f64,
    pub bead_area_mm2: f64,
    /// Extruded volume, with ramps counted at their actual flow.
    pub volume_mm3: f64,
    pub mass_g_pla: f64,
    pub time_s: f64,
    /// Highest volumetric flow of any move.
    pub flow_mm3_s: f64,
    /// Whether speeds follow from a set volumetric flow (the first part's).
    pub by_flow: bool,
    /// Steepest outward lean between consecutive layers, in degrees from vertical.
    pub max_overhang_deg: f64,
    pub samples_per_turn: usize,
    pub path_points: usize,
    pub width_mm: f64,
    pub layer_mm: f64,
    pub speed_mm_s: f64,
    pub bounds: Bounds,
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

fn too_many(needed: usize, max_points: usize, advice: &str) -> Result<(), String> {
    if needed > max_points.min(MAX_PATH_POINTS) {
        return Err(format!(
            "this toolpath needs too many points ({needed}); {advice}"
        ));
    }
    Ok(())
}

impl Toolpath {
    fn empty(parts: Vec<Settings>, capacity: usize) -> Toolpath {
        Toolpath {
            points: Vec::with_capacity(capacity),
            travel: Vec::with_capacity(capacity),
            flow: Vec::with_capacity(capacity),
            turn: Vec::with_capacity(capacity),
            part: Vec::with_capacity(capacity),
            height: Vec::with_capacity(capacity),
            parts,
            layer_starts: vec![0],
            samples_per_turn: 0,
            layers: 1,
        }
    }

    fn push(&mut self, p: P3, travel: bool, flow: f64, turn: f64, part: u16) {
        self.push_with_height(p, travel, flow, turn, part, 0.0);
    }

    /// A move with its own layer height (`height` 0: the part's).
    fn push_with_height(
        &mut self,
        p: P3,
        travel: bool,
        flow: f64,
        turn: f64,
        part: u16,
        height: f64,
    ) {
        self.points.push(p);
        self.travel.push(travel);
        self.flow.push(if travel { 0.0 } else { flow });
        self.turn.push(turn);
        self.part.push(part);
        self.height.push(if travel { 0.0 } else { height });
    }

    /// Copies point `i` of `other`, shifted by `dz`, with its part index moved by `part_offset`.
    fn push_from(&mut self, other: &Toolpath, i: usize, dz: f64, part_offset: u16) {
        let p = other.points.get(i).copied().unwrap_or([0.0; 3]);
        self.push_with_height(
            [p[0], p[1], p[2] + dz],
            other.travel.get(i).copied().unwrap_or(true),
            other.flow.get(i).copied().unwrap_or(0.0),
            other.turn.get(i).copied().unwrap_or(0.0),
            other.part.get(i).copied().unwrap_or(0) + part_offset,
            other.height.get(i).copied().unwrap_or(0.0),
        );
    }

    /// The settings of the part that prints move `i`.
    pub fn settings(&self, i: usize) -> Settings {
        let part = self.part.get(i).copied().unwrap_or(0) as usize;
        self.parts
            .get(part)
            .or(self.parts.first())
            .copied()
            .unwrap_or(Settings {
                width: 0.4,
                layer: 0.2,
                speed: 40.0,
                flow: None,
            })
    }

    /// The bead cross-section (mm²) of move `i`: its part's width with its own layer height.
    pub fn area(&self, i: usize) -> f64 {
        let s = self.settings(i);
        let h = self
            .height
            .get(i)
            .copied()
            .filter(|h| *h > 0.0)
            .unwrap_or(s.layer);
        bead_area(s.width, h)
    }

    /// The layer height used for lifts between layers and parts.
    fn layer_height(&self) -> f64 {
        self.parts.iter().map(|s| s.layer).fold(0.0, f64::max)
    }

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
        let (mut length, mut travel, mut volume, mut time) = (0.0, 0.0, 0.0, 0.0);
        let mut flow_max: f64 = 0.0;
        let mut top = f64::NEG_INFINITY;
        let mut bounds = Bounds::empty();
        for (i, w) in self.points.windows(2).enumerate() {
            let d = dist3(w[0], w[1]);
            if self.travel.get(i + 1).copied().unwrap_or(false) {
                travel += d;
                time += d / TRAVEL_SPEED_MM_S;
            } else {
                let s = self.settings(i + 1);
                let area = self.area(i + 1);
                let speed = s.speed_for(area);
                length += d;
                volume += area * d * self.flow.get(i + 1).copied().unwrap_or(1.0);
                time += d / speed;
                flow_max = flow_max.max(speed * area);
                top = top.max(w[1][2]);
                bounds.add(w[0]);
                bounds.add(w[1]);
            }
        }
        let first = self.parts.first().copied().unwrap_or(Settings {
            width: 0.0,
            layer: 0.0,
            speed: 0.0,
            flow: None,
        });
        ToolpathStats {
            layers: self.layers,
            printed_height_mm: if top.is_finite() { top } else { 0.0 },
            path_length_mm: length,
            travel_length_mm: travel,
            bead_area_mm2: first.area(),
            volume_mm3: volume,
            mass_g_pla: volume * PLA_DENSITY_G_PER_MM3,
            time_s: time,
            flow_mm3_s: flow_max,
            by_flow: first.flow.is_some(),
            max_overhang_deg: self.max_overhang_deg(),
            samples_per_turn: self.samples_per_turn,
            path_points: self.points.len(),
            width_mm: first.width,
            layer_mm: first.layer,
            speed_mm_s: first.speed,
            bounds: if bounds.is_empty() {
                Bounds::of(&self.points)
            } else {
                bounds
            },
        }
    }

    /// Steepest overhang: how far a point on one layer sticks out past the layer below, turned
    /// into an angle from vertical. Sampled on up to 24 evenly spaced layer pairs of a regular
    /// spiral. Point `j` of one turn sits right above point `j` of the turn below, so only that
    /// part of the lower ring is searched.
    fn max_overhang_deg(&self) -> f64 {
        let m = self.samples_per_turn;
        if self.layers < 2 || m < 3 {
            return 0.0;
        }
        let layer = self.settings(1).layer;
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
                    worst = worst.max(d.atan2(layer).to_degrees());
                }
            }
        }
        worst
    }
}

fn signed_area_2d(points: &[P2]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        / 2.0
}

/// One continuous vase-mode spiral up a wall of closed sections, refusing to make more than
/// `max_points` points.
pub fn vase_spiral(
    wall: &Surface,
    settings: Settings,
    max_points: usize,
) -> Result<Toolpath, String> {
    let layer = settings.layer;
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
    too_many(
        needed,
        max_points,
        "use a larger layer height or a simpler outline",
    )?;

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
    // The wall is stretched over the snapped height so the whole surface is printed.
    let section_at = |rise: f64| v_at(z0 + height * rise / total);

    let mut path = Toolpath::empty(vec![settings], needed);
    path.samples_per_turn = m;
    path.layers = layers;
    let steps = m as f64;
    // Opening turn: holds the section the second turn starts on, climbing from the bottom to
    // one layer while extrusion ramps 0 → 100 % (each segment at its midpoint value).
    let first = section_at(layer.min(total));
    for j in 0..m {
        let u = j as f64 / steps;
        let mut p = wall.point_at(first, j);
        p[2] = z0 + layer * u;
        let ramp = (u + (j as f64 - 1.0) / steps) / 2.0;
        path.push(p, j == 0, ramp, u, 0);
    }
    // The spiral: z rises one layer per turn.
    for k in 1..layers {
        for j in 0..m {
            let u = j as f64 / steps;
            let rise = layer * (k as f64 + u);
            let mut p = wall.point_at(section_at(rise), j);
            p[2] = z0 + rise;
            // The move into the first point of turn 1 still finishes the ramp.
            let flow = if k == 1 && j == 0 {
                (1.0 + (steps - 1.0) / steps) / 2.0
            } else {
                1.0
            };
            path.push(p, false, flow, k as f64 + u, 0);
        }
    }
    // The fan comes on after the ramp turn.
    if layers > 1 {
        path.layer_starts.push(m + 1);
    }
    // Closing turn: flat at the top while extrusion ramps back to zero, ending on the seam so
    // the rim closes cleanly.
    let top = (n - 1) as f64;
    for j in 0..=m {
        let u = j as f64 / steps;
        let mut p = wall.point_at(top, j % m);
        p[2] = z0 + total;
        let flow = if j == 0 {
            if layers == 1 {
                (1.0 + (steps - 1.0) / steps) / 2.0
            } else {
                1.0
            }
        } else {
            (1.0 - (u + (j as f64 - 1.0) / steps) / 2.0).max(0.0)
        };
        path.push(p, false, flow, layers as f64 + u, 0);
    }
    Ok(path)
}

/// Prints curves as they are, in order: each one starts with a travel and sits one layer height
/// above where it's drawn (a curve drawn at z = 0 is printed on the bed). Closed curves return
/// to their first point.
pub fn from_curves(
    curves: &[&Curve],
    settings: Settings,
    max_points: usize,
) -> Result<Toolpath, String> {
    if curves.is_empty() {
        return Err("needs at least one curve".into());
    }
    let needed = curves
        .iter()
        .map(|c| c.points.len() + usize::from(c.closed))
        .sum::<usize>();
    too_many(needed, max_points, "use fewer or simpler curves")?;
    let mut path = Toolpath::empty(vec![settings], needed);
    for curve in curves {
        let turn = |i: usize| {
            curve
                .winding
                .as_ref()
                .and_then(|w| w.turn.get(i).copied())
                .unwrap_or(0.0)
        };
        for (i, p) in curve.points.iter().enumerate() {
            path.push([p[0], p[1], p[2] + settings.layer], i == 0, 1.0, turn(i), 0);
        }
        if curve.closed
            && let Some(p) = curve.points.first()
        {
            path.push([p[0], p[1], p[2] + settings.layer], false, 1.0, turn(0), 0);
        }
    }
    if let [only] = curves
        && let Some(w) = &only.winding
    {
        path.samples_per_turn = w.per_turn;
    }
    Ok(path)
}

/// The same toolpath printed `count` times, each copy one layer higher. Between copies the
/// nozzle lifts one layer clear of the copy just printed, travels back to the start and lowers
/// (EasyGCode's spiral-disc layer change).
pub fn stack(path: &Toolpath, count: usize, max_points: usize) -> Result<Toolpath, String> {
    let lh = path.layer_height();
    let needed = path.points.len().saturating_add(2).saturating_mul(count);
    too_many(needed, max_points, "use fewer layers")?;
    let mut out = Toolpath::empty(path.parts.clone(), needed);
    out.layer_starts.clear();
    for k in 0..count {
        let dz = lh * k as f64;
        if let (Some(prev), Some(start)) = (out.points.last().copied(), path.points.first()) {
            out.layer_starts.push(out.points.len());
            let clear = start[2] + dz + lh;
            out.push([prev[0], prev[1], clear], true, 0.0, 0.0, 0);
            out.push([start[0], start[1], clear], true, 0.0, 0.0, 0);
        } else {
            out.layer_starts.push(0);
        }
        for i in 0..path.points.len() {
            out.push_from(path, i, dz, 0);
        }
    }
    out.layers = path.layers * count;
    if count == 1 {
        out.samples_per_turn = path.samples_per_turn;
    }
    Ok(out)
}

/// Toolpaths printed one after another. Between them the nozzle lifts clear of both ends,
/// travels over and lowers.
pub fn join(paths: &[&Toolpath], max_points: usize) -> Result<Toolpath, String> {
    let needed = paths.iter().map(|p| p.points.len() + 2).sum::<usize>();
    too_many(needed, max_points, "use fewer or simpler toolpaths")?;
    let mut out = Toolpath::empty(Vec::new(), needed);
    out.layer_starts.clear();
    out.layers = 0;
    for path in paths {
        let offset =
            u16::try_from(out.parts.len()).map_err(|_| "too many joined toolpaths".to_string())?;
        if let (Some(prev), Some(start)) = (out.points.last().copied(), path.points.first()) {
            let clear = prev[2].max(start[2]) + path.layer_height();
            out.push([prev[0], prev[1], clear], true, 0.0, 0.0, offset);
            out.push([start[0], start[1], clear], true, 0.0, 0.0, offset);
        }
        let base = out.points.len();
        out.layer_starts
            .extend(path.layer_starts.iter().map(|s| s + base));
        for i in 0..path.points.len() {
            out.push_from(path, i, 0.0, offset);
        }
        out.parts.extend(path.parts.iter().copied());
        out.layers += path.layers;
    }
    if let [only] = paths {
        out.samples_per_turn = only.samples_per_turn;
    }
    Ok(out)
}

/// Pushes points sideways by `amplitude · sin(2π · (count · s + phase))`, perpendicular to the
/// path in the XY plane, where `s` is the position along the path in turns (for a toolpath) or
/// in loops (for a curve: 0 → 1 by length), and the amplitude can vary over space. `outward` is
/// +1 for counter-clockwise loops (the right-hand side is outside) and −1 for clockwise ones.
fn displace(
    points: &[P3],
    s: &[f64],
    closed: bool,
    outward: f64,
    amplitude: &Field,
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
            let wave =
                (std::f64::consts::TAU * (count * s.get(i).copied().unwrap_or(0.0) + phase)).sin();
            let d = amplitude.at(p) * wave;
            [
                p[0] + outward * d * ty / len,
                p[1] - outward * d * tx / len,
                p[2],
            ]
        })
        .collect()
}

/// Waves a toolpath. Segments are split first when needed, so each wave gets at least 12
/// points; travels, ramps and parts are kept.
pub fn wave_path(
    path: &Toolpath,
    amplitude: &Field,
    count: f64,
    phase: f64,
    max_points: usize,
) -> Result<Toolpath, String> {
    check_wave(count, phase)?;
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
    too_many(
        total,
        max_points,
        "use fewer waves or a larger layer height",
    )?;
    let mut out = Toolpath::empty(path.parts.clone(), total);
    out.layer_starts.clear();
    let mut starts = path.layer_starts.iter().peekable();
    for i in 0..path.points.len() {
        // A layer starts with the move into its first point, split pieces included.
        if starts.peek() == Some(&&i) {
            out.layer_starts.push(out.points.len());
            starts.next();
        }
        if i > 0 && split > 1 && !path.travel.get(i).copied().unwrap_or(false) {
            let (q, p) = (path.points[i - 1], path.points[i]);
            let (tq, t) = (
                path.turn.get(i - 1).copied().unwrap_or(0.0),
                path.turn.get(i).copied().unwrap_or(0.0),
            );
            for k in 1..split {
                let a = k as f64 / split as f64;
                let flow = path.flow.get(i).copied().unwrap_or(1.0);
                out.push_with_height(
                    lerp3(q, p, a),
                    false,
                    flow,
                    tq + (t - tq) * a,
                    path.part.get(i).copied().unwrap_or(0),
                    path.height.get(i).copied().unwrap_or(0.0),
                );
            }
        }
        out.push_from(path, i, 0.0, 0);
    }
    // A spiral is one long open path, but each turn is a loop that tells which side is outside.
    let first_turn = out
        .points
        .get(..(m * split).min(out.points.len()))
        .unwrap_or(&[]);
    let outward = if signed_area_xy(first_turn) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    out.points = displace(
        &out.points,
        &out.turn,
        false,
        outward,
        amplitude,
        count,
        phase,
    );
    out.samples_per_turn = if path.samples_per_turn > 0 {
        m * split
    } else {
        0
    };
    out.layers = path.layers;
    Ok(out)
}

/// Waves a ring of points (a curve or one section of a surface), resampled first if it has
/// fewer than 12 points per wave. A winding curve (a spiral) waves per turn, like a toolpath.
pub fn wave_curve(
    curve: &Curve,
    amplitude: &Field,
    count: f64,
    phase: f64,
    max_points: usize,
) -> Result<Curve, String> {
    check_wave(count, phase)?;
    if let Some(w) = &curve.winding {
        let points = displace(
            &curve.points,
            &w.turn,
            curve.closed,
            outward(curve),
            amplitude,
            count,
            phase,
        );
        return Ok(Curve {
            points,
            circle: None,
            ..curve.clone()
        });
    }
    let wanted = ((12.0 * count.abs()).ceil() as usize).max(curve.points.len());
    if wanted > max_points {
        return Err(format!(
            "this wave needs too many points ({wanted}); use fewer waves"
        ));
    }
    let base = if wanted > curve.points.len() {
        curve.resample(wanted)
    } else {
        Curve::new(curve.points.clone(), curve.closed)
    };
    // Position along the curve, 0 → 1 by length.
    let total = base.length().max(1e-12);
    let mut walked = 0.0;
    let mut s = Vec::with_capacity(base.points.len());
    for (i, p) in base.points.iter().enumerate() {
        if i > 0 {
            walked += dist3(base.points[i - 1], *p);
        }
        s.push(walked / total);
    }
    let points = displace(
        &base.points,
        &s,
        base.closed,
        outward(&base),
        amplitude,
        count,
        phase,
    );
    Ok(Curve::new(points, base.closed))
}

fn outward(curve: &Curve) -> f64 {
    if signed_area_xy(&curve.points) >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

fn check_wave(count: f64, phase: f64) -> Result<(), String> {
    check("count", count, -2000.0, 2000.0, "")?;
    check("phase", phase, -1e6, 1e6, "")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn settings(width: f64, layer: f64, speed: f64) -> Settings {
        Settings::new(width, layer, speed).unwrap()
    }

    fn constant(v: f64) -> Field {
        Field::constant(v)
    }

    #[test]
    fn spiral_snaps_to_whole_layers_and_rises_monotonically() {
        let w = wall(0.8, 90.0);
        let p = vase_spiral(&w, settings(0.6, 0.3, 40.0), MAX_PATH_POINTS).unwrap();
        // 160 / 0.3 = 533.3 → 533 whole layers, like EasyGCode's height snapping.
        assert_eq!(p.layers, 533);
        let z: Vec<f64> = p.points.iter().map(|q| q[2]).collect();
        assert_eq!(z.first().copied(), Some(0.0));
        assert!(z.windows(2).all(|w| w[1] >= w[0]));
        assert!((z.last().copied().unwrap() - 159.9).abs() < 1e-9);
        assert_eq!(p.points.len(), (533 + 1) * 224 + 1);
        assert_eq!(p.travel.iter().filter(|t| **t).count(), 1);
        assert_eq!(p.layer_starts, vec![0, 225]);
        // The cap ends on the top section's seam with no extrusion left.
        let (end, seam) = (*p.points.last().unwrap(), w.sections[31][0]);
        assert!((end[0] - seam[0]).hypot(end[1] - seam[1]) < 1e-9);
        assert!((p.flow.last().copied().unwrap() - 1.0 / 448.0).abs() < 1e-12);
    }

    #[test]
    fn ramps_average_each_segment_like_easygcode() {
        let p = vase_spiral(&wall(1.0, 0.0), settings(0.6, 0.3, 40.0), MAX_PATH_POINTS).unwrap();
        let m = 224.0;
        // Segment s of the ramp turn ends at u = s/m and extrudes (u + (s−1)/m) / 2.
        assert_eq!(p.flow[1], 1.0 / (2.0 * m));
        assert_eq!(p.flow[224], (1.0 + (m - 1.0) / m) / 2.0);
        assert_eq!(p.flow[225], 1.0);
        // The ramp turn climbs from the bed to one layer.
        assert!((p.points[112][2] - 0.15).abs() < 1e-12);
        let s = p.stats();
        let full = s.path_length_mm * s.bead_area_mm2;
        assert!(s.volume_mm3 < full && s.volume_mm3 > 0.95 * full);
    }

    #[test]
    fn overhangs() {
        let o = |top: f64| {
            vase_spiral(&wall(top, 0.0), settings(0.6, 0.3, 40.0), MAX_PATH_POINTS)
                .unwrap()
                .stats()
                .max_overhang_deg
        };
        assert!(o(1.0) < 0.5, "{}", o(1.0));
        // Radius grows 45 mm over 160 mm of height: about atan(45/160) ≈ 15.7° at the tips.
        assert!(o(2.0) > 10.0 && o(2.0) < 25.0, "{}", o(2.0));
        assert!(o(0.5) < 0.5);
    }

    #[test]
    fn bad_walls_and_settings_are_errors() {
        let w = wall(1.0, 0.0);
        assert!(Settings::new(0.6, 0.0, 40.0).is_err());
        assert!(Settings::new(0.6, f64::NAN, 40.0).is_err());
        assert!(Settings::new(0.6, 0.3, 0.0).is_err());
        let ok = settings(0.6, 0.3, 40.0);
        let flat = Surface {
            sections: vec![w.sections[0].clone(), w.sections[0].clone()],
            closed: true,
            capped: false,
        };
        assert!(
            vase_spiral(&flat, ok, MAX_PATH_POINTS)
                .unwrap_err()
                .contains("same height")
        );
        let mut upside_down = w.clone();
        upside_down.sections.reverse();
        assert!(
            vase_spiral(&upside_down, ok, MAX_PATH_POINTS)
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
            vase_spiral(&tall, settings(0.6, 0.02, 40.0), MAX_PATH_POINTS)
                .unwrap_err()
                .contains("too many points")
        );
    }

    #[test]
    fn curves_print_one_layer_up_with_travel_between() {
        let rings = [Curve::circle(10.0, 8), Curve::circle(12.0, 8)];
        let p = from_curves(
            &rings.iter().collect::<Vec<_>>(),
            settings(2.0, 0.6, 20.0),
            MAX_PATH_POINTS,
        )
        .unwrap();
        assert_eq!(p.points.len(), 18); // 8 + closing point, twice
        assert_eq!(p.travel.iter().filter(|t| **t).count(), 2);
        assert!(p.travel[9]);
        assert!(p.points.iter().all(|q| q[2] == 0.6));
        let s = p.stats();
        assert!((s.path_length_mm - (rings[0].length() + rings[1].length())).abs() < 1e-9);
        assert!((s.travel_length_mm - 2.0).abs() < 1e-9);
    }

    #[test]
    fn stacked_layers_lift_clear_and_return_to_the_start() {
        let one = from_curves(
            &[&Curve::circle(10.0, 8)],
            settings(2.0, 0.6, 20.0),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let p = stack(&one, 3, MAX_PATH_POINTS).unwrap();
        assert_eq!(p.points.len(), 9 * 3 + 2 * 2);
        assert_eq!(p.layer_starts, vec![0, 9, 20]);
        // Lift one layer above the next layer, over to the start, then down onto it.
        let near = |a: P3, b: P3| dist3(a, b) < 1e-9;
        assert!(near(p.points[9], [10.0, 0.0, 1.8]));
        assert!(near(p.points[10], [10.0, 0.0, 1.8]));
        assert!(near(p.points[11], [10.0, 0.0, 1.2]));
        assert!(p.travel[9] && p.travel[10] && p.travel[11] && !p.travel[12]);
        assert_eq!(p.layers, 3);
    }

    #[test]
    fn joined_paths_keep_their_own_settings() {
        let a = from_curves(
            &[&Curve::circle(10.0, 8)],
            settings(2.0, 0.6, 20.0),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let b = from_curves(
            &[&Curve::circle(20.0, 8)],
            settings(1.0, 0.3, 40.0),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let j = join(&[&a, &b], MAX_PATH_POINTS).unwrap();
        assert_eq!(j.parts.len(), 2);
        assert_eq!(j.settings(j.points.len() - 1).width, 1.0);
        assert_eq!(j.settings(3).width, 2.0);
        assert_eq!(j.layer_starts, vec![0, 11]);
    }

    #[test]
    fn whole_waves_stack_and_half_waves_alternate() {
        let c = Curve::circle(40.0, 96);
        let top = c.transformed(&Xform::translate(0.0, 0.0, 30.0));
        let w = Surface::loft(&[&c, &top]).unwrap();
        let p = vase_spiral(&w, settings(0.8, 0.5, 30.0), MAX_PATH_POINTS).unwrap();
        let r = |q: P3| q[0].hypot(q[1]);
        let flutes = wave_path(&p, &constant(1.0), 9.0, 0.0, MAX_PATH_POINTS).unwrap();
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
        let weave = wave_path(&p, &constant(1.0), 9.5, 0.0, MAX_PATH_POINTS).unwrap();
        let (a, b) = (layer(&weave, 10), layer(&weave, 11));
        let spread = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
        assert!(spread > 1.9, "{spread}");
        // Ramps are kept: each split segment repeats the flow of the segment it came from.
        let mut expected = Vec::new();
        for (i, x) in p.flow.iter().enumerate() {
            if i > 0 {
                expected.push(*x);
            }
            expected.push(*x);
        }
        assert_eq!(weave.flow, expected);
        assert_eq!(weave.layer_starts, vec![0, 2 * 96 + 1]);
    }

    #[test]
    fn wave_size_can_vary_over_space() {
        // Enough points that the wave needs no resampling (which would cut the circle's corners).
        let c = Curve::circle(10.0, 128);
        // A field that is 1 inside a big square covering the right half (x from 0 to 200) and 0
        // outside it.
        let square = Curve::polygon(4, 100.0 * 2f64.sqrt())
            .transformed(&Xform::rotate_z(45.0))
            .transformed(&Xform::translate(100.0, 0.0, 0.0));
        let right = Field::map(
            |a| Ok(if a[0] < 0.0 { 1.0 } else { 0.0 }),
            vec![std::rc::Rc::new(Field::distance(&square))],
        );
        let w = wave_curve(&c, &right.unwrap(), 8.0, 0.25, 1000).unwrap();
        let r = |q: P3| q[0].hypot(q[1]);
        assert!(
            w.points
                .iter()
                .filter(|q| q[0] < -0.5)
                .all(|q| (r(*q) - 10.0).abs() < 1e-9)
        );
        assert!(
            w.points
                .iter()
                .any(|q| q[0] > 0.5 && (r(*q) - 10.0).abs() > 0.5)
        );
    }

    #[test]
    fn wave_curve_pushes_outward() {
        let c = Curve::circle(10.0, 8);
        let w = wave_curve(&c, &constant(1.0), 4.0, 0.25, 1000).unwrap();
        assert_eq!(w.points.len(), 48);
        // phase 0.25 puts a peak at the seam.
        assert!((w.points[0][0].hypot(w.points[0][1]) - 11.0).abs() < 1e-9);
        let cw = Curve::new(c.points.iter().rev().copied().collect(), true);
        let w = wave_curve(&cw, &constant(1.0), 4.0, 0.25, 1000).unwrap();
        assert!((w.points[0][0].hypot(w.points[0][1]) - 11.0).abs() < 1e-9);
    }
}
