//! Toolpaths: the nozzle's path in print order, with what each move does.
//!
//! A toolpath is made from any curves (`from_curves`), or by a strategy such as `vase_spiral`,
//! and can be stacked (`stack`), joined (`join`) and patterned (`wave_path`). Every move ends at
//! a point and is either a travel (no extrusion) or extrudes a fraction of a full bead, using
//! EasyGCode's bead model with the width, layer height and speed of its part. A move can carry
//! its own layer height (non-planar layers are thicker in some places than others) and its own
//! width (a bead that blends from one size to another), and a part
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
    /// The line width of the move that ends at each point, where it differs from its part's (a
    /// bead that blends from one width to another); 0 means the part's width.
    pub width: Vec<f64>,
    pub parts: Vec<Settings>,
    /// Speed of the move that ends at each point, as a factor of its part's (1 = as set; at a
    /// set flow, the flow scales with it): per-zone speeds and acceleration ramps.
    pub speed: Vec<f64>,
    /// Machine events (fan changes, dwells), each right after the move that ends at a point,
    /// in order.
    pub events: Vec<Event>,
    /// Index of the first point of each layer (the fan comes on after the first).
    pub layer_starts: Vec<usize>,
    /// Points per turn when the whole path is one regular spiral (for the overhang check).
    pub samples_per_turn: usize,
    /// Number of layers (for a spiral: turns, not counting the closing turn).
    pub layers: usize,
}

/// Something the machine does between two moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    /// Happens right after the move that ends at this point.
    pub after: usize,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    /// Part fan to this level, 0–100 %.
    Fan(f64),
    /// Wait this many seconds with the nozzle where it is.
    Dwell(f64),
    /// New heater temperatures; printing carries on while they change.
    Heat(Temperatures),
    /// Wait until the heaters are hot enough (`cool` false: the one at the nozzle within 2 °C
    /// below its target) or cool enough (`cool` true: every one within 2 °C above its target).
    WaitHeat { temps: Temperatures, cool: bool },
    /// The printer's speed override in % (M220): every move's speed is scaled by it.
    SpeedOverride(f64),
    /// The printer's extrusion override in % (M221): every move's extrusion is scaled by it.
    ExtrusionOverride(f64),
}

/// Heater temperatures in °C for the machine events. `None` is the printer's own (its settings
/// in the app), so a recipe can go back to them without knowing them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Temperatures {
    /// A filament printer's nozzle; also any zone of a pellet printer that isn't given.
    pub nozzle: Option<f64>,
    /// A pellet printer's upper, middle and lower extruder zones.
    pub zones: [Option<f64>; 3],
}

impl Temperatures {
    /// The pellet printer's three zones, given the printer's own.
    pub fn zones_or(&self, own: [f64; 3]) -> [f64; 3] {
        let mut out = own;
        for (k, slot) in out.iter_mut().enumerate() {
            if let Some(t) = self.zones[k].or(self.nozzle) {
                *slot = t;
            }
        }
        out
    }

    /// A filament printer's nozzle (or, for a recipe written for zones, the lower zone, the one
    /// at the nozzle), given the printer's own.
    pub fn nozzle_or(&self, own: f64) -> f64 {
        self.nozzle.or(self.zones[2]).unwrap_or(own)
    }
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
    /// The highest part-fan level the toolpath's own events set, 0–100 % (0: it sets none).
    pub max_fan_pct: f64,
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
            width: Vec::with_capacity(capacity),
            parts,
            speed: Vec::with_capacity(capacity),
            events: Vec::new(),
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
        self.width.push(0.0);
        self.speed.push(1.0);
    }

    /// EasyGCode's clearance hop (`hopTravel` in its gcode.js) from the last point towards
    /// `dest`: straight up to `clear_z` (only if that's higher than where the nozzle is), over
    /// to above `dest`, and down to it (only if it's lower). Returns whether the move down is
    /// still to come: when `dest` is at the travel height, the move over already ends on it.
    fn hop(&mut self, dest: P3, clear_z: f64, part: u16) -> bool {
        let Some(prev) = self.points.last().copied() else {
            return true;
        };
        let z = clear_z.max(prev[2]).max(dest[2]);
        if z > prev[2] + 1e-6 {
            self.push([prev[0], prev[1], z], true, 0.0, 0.0, part);
        }
        self.push([dest[0], dest[1], z], true, 0.0, 0.0, part);
        dest[2] < z - 1e-6
    }

    /// EasyGCode's wipe (`travelWipe` in its gcode.js) towards `dest`: one straight move to
    /// `height` above it; the move down onto it is still to come.
    fn wipe(&mut self, dest: P3, height: f64, part: u16) {
        self.push([dest[0], dest[1], dest[2] + height], true, 0.0, 0.0, part);
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
        if let (Some(slot), Some(v)) = (self.speed.last_mut(), other.speed.get(i)) {
            *slot = *v;
        }
        self.copy_width(other, i);
    }

    /// Gives the last point the line width of `other`'s point `i` (travels keep none).
    fn copy_width(&mut self, other: &Toolpath, i: usize) {
        let travel = self.travel.last().copied().unwrap_or(true);
        if let (Some(slot), Some(w)) = (self.width.last_mut(), other.width.get(i))
            && !travel
        {
            *slot = *w;
        }
    }

    /// Copies `other`'s events, their points renumbered by `map` (`None`: the point wasn't
    /// copied, so its events go after the copied point before it).
    pub(crate) fn copy_events(&mut self, other: &Toolpath, map: &dyn Fn(usize) -> Option<usize>) {
        for e in &other.events {
            let after = (0..=e.after).rev().find_map(&map);
            if let Some(after) = after {
                self.events.push(Event { after, ..*e });
            }
        }
    }

    /// The same toolpath with no points (and no events or layers): parts and the rest kept,
    /// for steps that rebuild the points.
    pub(crate) fn emptied(&self) -> Toolpath {
        let mut out = Toolpath::empty(self.parts.clone(), self.points.len());
        out.layer_starts.clear();
        out.layers = self.layers;
        out.samples_per_turn = self.samples_per_turn;
        out
    }

    /// The speed factor of move `i` (1 when not set).
    pub fn speed_factor(&self, i: usize) -> f64 {
        self.speed.get(i).copied().unwrap_or(1.0)
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

    /// The bead of move `i` as (width, layer height) in mm: its own where it has them, else its
    /// part's.
    pub fn bead_of(&self, i: usize) -> (f64, f64) {
        let s = self.settings(i);
        let own = |v: &Vec<f64>| v.get(i).copied().filter(|v| *v > 0.0);
        (
            own(&self.width).unwrap_or(s.width),
            own(&self.height).unwrap_or(s.layer),
        )
    }

    /// The bead cross-section (mm²) of move `i`: its own width and layer height where it has
    /// them, else its part's.
    pub fn area(&self, i: usize) -> f64 {
        let (w, h) = self.bead_of(i);
        bead_area(w, h)
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
        // The printer's speed and extrusion overrides (M220, M221) as they stand at each move.
        let mut events: Vec<&Event> = self.events.iter().collect();
        events.sort_by_key(|e| e.after);
        let mut pending = events.into_iter().peekable();
        let (mut speed_override, mut extrusion_override) = (1.0, 1.0);
        for (i, w) in self.points.windows(2).enumerate() {
            while let Some(e) = pending.next_if(|e| e.after <= i) {
                match e.kind {
                    EventKind::SpeedOverride(pct) => speed_override = pct / 100.0,
                    EventKind::ExtrusionOverride(pct) => extrusion_override = pct / 100.0,
                    _ => {}
                }
            }
            let d = dist3(w[0], w[1]);
            if self.travel.get(i + 1).copied().unwrap_or(false) {
                travel += d;
                time += d / (TRAVEL_SPEED_MM_S * speed_override);
            } else {
                let s = self.settings(i + 1);
                let area = self.area(i + 1);
                let speed = s.speed_for(area) * self.speed_factor(i + 1) * speed_override;
                length += d;
                volume +=
                    area * d * self.flow.get(i + 1).copied().unwrap_or(1.0) * extrusion_override;
                time += d / speed;
                flow_max = flow_max.max(speed * area * extrusion_override);
                top = top.max(w[1][2]);
                bounds.add(w[0]);
                bounds.add(w[1]);
            }
        }
        time += self
            .events
            .iter()
            .map(|e| match e.kind {
                EventKind::Dwell(s) => s,
                _ => 0.0,
            })
            .sum::<f64>();
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
            max_fan_pct: self
                .events
                .iter()
                .filter_map(|e| match e.kind {
                    EventKind::Fan(level) => Some(level),
                    _ => None,
                })
                .fold(0.0, f64::max),
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
    flat_top: bool,
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
    // the rim closes cleanly. An open top stops as the spiral reaches the top instead.
    let top = (n - 1) as f64;
    let last = if flat_top { m } else { 0 };
    for j in 0..=last {
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

/// What [`spiralize`] found out about its layers, for the step's warnings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpiralInfo {
    /// Thinnest and thickest local layer height, in mm.
    pub thinnest: f64,
    pub thickest: f64,
    /// Whether layers with fewer points were resampled to match the others.
    pub resampled: bool,
}

/// One continuous spiral through a stack of closed layer curves: vase mode for any layers, flat
/// or not. The first curve is the bottom (usually on the bed); each turn blends one layer into
/// the next, and every move is extruded for its own local layer height (the vertical gap to the
/// turn below, averaged over the move), so the thin and thick sides of a tilted layer each get
/// the right amount of plastic (and, at a set flow, the right speed). Same conventions as [`vase_spiral`]: the
/// opening turn holds layer 1's shape while it climbs from the bottom curve with extrusion
/// ramping up, and a flat closing turn on the top layer ramps it back down. Flat, evenly spaced
/// layers give exactly `vase_spiral`'s toolpath. Point `j` of every layer must be the "same"
/// point (curves made from one another by tweens and moves are). Without `ramp`, the spiral
/// carries on from something printed before (EasyGCode's flat bottom rounding into a wall): the
/// opening turn blends the bottom layer into the next like every other turn, at full extrusion.
pub fn spiralize(
    layers: &[&Curve],
    width: f64,
    speed: f64,
    flow: f64,
    flat_top: bool,
    ramp: bool,
    max_points: usize,
) -> Result<(Toolpath, SpiralInfo), String> {
    if layers.len() < 2 {
        return Err("needs at least 2 layer curves: the bottom one and one above it".into());
    }
    if let Some(i) = layers.iter().position(|c| !c.closed) {
        return Err(format!(
            "layer {} is an open curve; vase mode needs closed layers",
            i + 1
        ));
    }
    if let Some(i) = layers.iter().position(|c| c.points.len() < 3) {
        return Err(format!("layer {} has fewer than 3 points", i + 1));
    }
    let m = layers.iter().map(|c| c.points.len()).max().unwrap_or(0);
    let n = layers.len() - 1;
    let needed = (n + 1).saturating_mul(m).saturating_add(1);
    too_many(needed, max_points, "use fewer layers or simpler curves")?;
    let resampled = layers.iter().any(|c| c.points.len() != m);
    let rings: Vec<Vec<P3>> = layers
        .iter()
        .map(|c| {
            if c.points.len() == m {
                c.points.clone()
            } else {
                c.resample(m).points
            }
        })
        .collect();

    // Local layer heights: how far layer k sits above layer k − 1 at each point.
    let mut h = vec![vec![0.0; m]; n + 1];
    let (mut thinnest, mut thickest) = (f64::INFINITY, 0.0f64);
    for (k, row) in h.iter_mut().enumerate().skip(1) {
        let (Some(top), Some(below)) = (rings.get(k), rings.get(k - 1)) else {
            continue;
        };
        for (j, (slot, (t, b))) in row.iter_mut().zip(top.iter().zip(below)).enumerate() {
            let d = t[2] - b[2];
            if !(d > 1e-6 && d.is_finite()) {
                return Err(format!(
                    "layer {} isn't above layer {} everywhere (at its point {} it's {} mm higher); the layers must stack upward",
                    k + 1,
                    k,
                    j + 1,
                    easycad_lang::format_number((d * 1000.0).round() / 1000.0)
                ));
            }
            *slot = d;
            thinnest = thinnest.min(d);
            thickest = thickest.max(d);
        }
    }
    let mean = h[1..].iter().flatten().sum::<f64>() / (n * m) as f64;
    let settings = Settings::from_inputs(width, mean, speed, flow)?;
    let mut path = Toolpath::empty(vec![settings], needed);
    path.samples_per_turn = m;
    path.layers = n;
    let steps = m as f64;
    let ring = |k: usize, j: usize| {
        rings
            .get(k)
            .and_then(|r| r.get(j))
            .copied()
            .unwrap_or([0.0; 3])
    };
    let height = |k: usize, j: usize| h.get(k).and_then(|r| r.get(j)).copied().unwrap_or(0.0);

    // Every point of the spiral: (position, extrusion factor, progress, local layer height).
    let mut moves: Vec<(P3, f64, f64, f64)> = Vec::with_capacity(needed);
    // Opening turn: layer 1's shape, climbing from the bottom curve as extrusion ramps up (each
    // segment at its midpoint value).
    for j in 0..m {
        let u = j as f64 / steps;
        let (a, b) = (ring(0, j), ring(1, j));
        if ramp {
            let p = [b[0], b[1], a[2] + (b[2] - a[2]) * u];
            moves.push((p, (u + (j as f64 - 1.0) / steps) / 2.0, u, height(1, j)));
        } else {
            moves.push((lerp3(a, b, u), 1.0, u, height(1, j)));
        }
    }
    // The spiral: each turn blends one layer into the next.
    for k in 1..n {
        for j in 0..m {
            let u = j as f64 / steps;
            let p = lerp3(ring(k, j), ring(k + 1, j), u);
            // The move into the first point of turn 1 still finishes the ramp.
            let flow = if ramp && k == 1 && j == 0 {
                (1.0 + (steps - 1.0) / steps) / 2.0
            } else {
                1.0
            };
            let local = height(k, j) + (height(k + 1, j) - height(k, j)) * u;
            moves.push((p, flow, k as f64 + u, local));
        }
    }
    // Closing turn: flat on the top layer while extrusion ramps back to zero, ending on the seam
    // (or, for an open top, just the move up onto the top layer's seam).
    for j in 0..=(if flat_top { m } else { 0 }) {
        let u = j as f64 / steps;
        let flow = if j == 0 {
            if ramp && n == 1 {
                (1.0 + (steps - 1.0) / steps) / 2.0
            } else {
                1.0
            }
        } else {
            (1.0 - (u + (j as f64 - 1.0) / steps) / 2.0).max(0.0)
        };
        moves.push((ring(n, j % m), flow, n as f64 + u, height(n, j % m)));
    }
    // Each move is extruded for the gap under it. Where layers tilt, that gap changes along the
    // move, so a move takes the mean of the local heights at its two ends.
    let mut last = moves.first().map_or(0.0, |m| m.3);
    for (i, (p, flow, t, h)) in moves.into_iter().enumerate() {
        path.push_with_height(p, i == 0, flow, t, 0, (last + h) / 2.0);
        last = h;
    }
    // The fan comes on after the ramp turn.
    if n > 1 {
        path.layer_starts.push(m + 1);
    }
    Ok((
        path,
        SpiralInfo {
            thinnest,
            thickest,
            resampled,
        },
    ))
}

/// Prints curves as they are, in order: each one starts with a travel and sits one layer height
/// above where it's drawn (a curve drawn at z = 0 is printed on the bed). Closed curves return
/// to their first point. No curves make an empty toolpath, which `join` skips (so a brim with
/// no rings is no brim).
pub fn from_curves(
    curves: &[&Curve],
    settings: Settings,
    max_points: usize,
) -> Result<Toolpath, String> {
    if curves.is_empty() {
        let mut empty = Toolpath::empty(vec![settings], 0);
        empty.layer_starts.clear();
        empty.layers = 0;
        return Ok(empty);
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

/// The same toolpath printed `count` times (0 is none), each copy one layer higher. Between
/// copies the nozzle travels back to the start `lift` layers above the new copy (EasyGCode's
/// clearance hop): with 1, it lifts a layer clear of the copy just printed, goes over and
/// lowers (its spiral disc); with 0, it rises to the new layer and goes straight over (its
/// spoon). With `wipe` (mm) it goes instead in one straight move to that far above the new
/// copy's start and down onto it, without stopping to lift (its bend stool).
pub fn stack(
    path: &Toolpath,
    count: usize,
    lift: f64,
    wipe: f64,
    max_points: usize,
) -> Result<Toolpath, String> {
    check("lift", lift, 0.0, 100.0, " layers")?;
    check("wipe", wipe, 0.0, 1000.0, " mm")?;
    let lh = path.layer_height();
    let needed = path.points.len().saturating_add(2).saturating_mul(count);
    too_many(needed, max_points, "use fewer layers")?;
    let mut out = Toolpath::empty(path.parts.clone(), needed);
    out.layer_starts.clear();
    if path.points.is_empty() || count == 0 {
        out.layers = 0;
        if count > 0 {
            out.events = path.events.clone();
        }
        return Ok(out);
    }
    for k in 0..count {
        let dz = lh * k as f64;
        let mut first = 0;
        match path.points.first() {
            Some(start) if !out.points.is_empty() => {
                out.layer_starts.push(out.points.len());
                let dest = [start[0], start[1], start[2] + dz];
                if wipe > 0.0 {
                    out.wipe(dest, wipe, 0);
                } else if !out.hop(dest, dest[2] + lift * lh, 0) {
                    first = 1; // the move over already ends on the copy's start
                }
            }
            _ => out.layer_starts.push(0),
        }
        let base = out.points.len();
        for i in first..path.points.len() {
            out.push_from(path, i, dz, 0);
        }
        out.copy_events(path, &|i| {
            if i >= first {
                Some(base + i - first)
            } else {
                base.checked_sub(1)
            }
        });
    }
    out.layers = path.layers * count;
    if count == 1 {
        out.samples_per_turn = path.samples_per_turn;
    }
    Ok(out)
}

/// Toolpaths printed one after another, each with its own settings. Between two parts the
/// nozzle travels clear (EasyGCode's clearance hop): straight up, over, and down to the next
/// part's start, at least `clear` mm above the bed and never lower than either end; with
/// `clear` 0, one layer (the next part's) above the higher end. With `wipe` (mm) it goes
/// instead in one straight move to that far above the next part's start and down onto it. A
/// part that starts exactly where the one before ended carries on without any travel. Empty
/// parts are skipped, but their machine events happen where they would have printed.
///
/// A part that starts no higher than a one-layer start counts as that first layer, so the part
/// fan comes on where EasyGCode turns it on: after a brim *and* the first turn of the helix
/// printed inside it (not after the first of several bottom layers), and after a first layer
/// and the primer line printed after it, before the next layer up.
pub fn join(
    paths: &[&Toolpath],
    clear: f64,
    wipe: f64,
    max_points: usize,
) -> Result<Toolpath, String> {
    check("clear", clear, 0.0, 1000.0, " mm")?;
    check("wipe", wipe, 0.0, 1000.0, " mm")?;
    let needed = paths.iter().map(|p| p.points.len() + 2).sum::<usize>();
    too_many(needed, max_points, "use fewer or simpler toolpaths")?;
    let mut out = Toolpath::empty(Vec::new(), needed);
    out.layer_starts.clear();
    out.layers = 0;
    let mut parts_used = 0;
    for path in paths {
        let Some(start) = path.points.first().copied() else {
            let after = out.points.len().saturating_sub(1);
            out.events
                .extend(path.events.iter().map(|e| Event { after, ..*e }));
            continue;
        };
        parts_used += 1;
        let offset =
            u16::try_from(out.parts.len()).map_err(|_| "too many joined toolpaths".to_string())?;
        // Points of `path` before `first` aren't copied: its opening travel, when the nozzle is
        // already there.
        let mut first = 0;
        if let Some(prev) = out.points.last().copied() {
            if dist3(prev, start) <= 1e-6 {
                first = 1;
            } else if wipe > 0.0 {
                out.wipe(start, wipe, offset);
            } else {
                let clear_z = if clear > 0.0 {
                    clear
                } else {
                    prev[2].max(start[2]) + path.layer_height()
                };
                if !out.hop(start, clear_z, offset) {
                    first = 1;
                }
            }
        }
        // A part that starts no higher than the first layer printed so far (a brim, a primer
        // line, the object inside a brim) belongs to that first layer.
        let printed = |p: &Toolpath| {
            (0..p.points.len())
                .filter(|&i| !p.travel.get(i).copied().unwrap_or(true))
                .map(|i| p.points[i][2])
                .collect::<Vec<f64>>()
        };
        let first_top = printed(&out).into_iter().fold(f64::NEG_INFINITY, f64::max);
        let starts_at = printed(path).first().copied().unwrap_or(start[2]);
        let merge = out.layer_starts.len() == 1 && starts_at <= first_top + 1e-6;
        let base = out.points.len();
        for (k, s) in path.layer_starts.iter().enumerate() {
            if k == 0 && merge {
                continue;
            }
            out.layer_starts.push(base + s.saturating_sub(first));
        }
        for i in first..path.points.len() {
            out.push_from(path, i, 0.0, offset);
        }
        out.copy_events(path, &|i| {
            if i >= first {
                Some(base + i - first)
            } else {
                base.checked_sub(1)
            }
        });
        out.parts.extend(path.parts.iter().copied());
        out.layers += path.layers - usize::from(merge && path.layers > 0);
    }
    if out.layer_starts.is_empty() && !out.points.is_empty() {
        out.layer_starts.push(0);
    }
    if parts_used == 1
        && let Some(only) = paths.iter().find(|p| !p.points.is_empty())
    {
        out.samples_per_turn = only.samples_per_turn;
    }
    Ok(out)
}

/// What [`vase_revolve`] reads once per turn, at the point of the profile at the turn's middle:
/// the turn's rise, its line width and its volumetric flow (0: by speed).
pub struct PerTurn<'a> {
    pub rise: &'a dyn Fn(P3) -> Result<f64, String>,
    pub width: &'a dyn Fn(P3) -> Result<f64, String>,
    pub flow: &'a dyn Fn(P3) -> Result<f64, String>,
}

/// [`vase_revolve`]'s settings that hold for the whole spiral.
#[derive(Debug, Clone, Copy)]
pub struct Revolve {
    /// Print speed in mm/s (when the flow is 0).
    pub speed: f64,
    /// Extrude every turn for this layer height instead of its rise (0: its rise).
    pub bead_layer: f64,
    /// How far a chord may stray from the true circle, in mm.
    pub tolerance: f64,
    /// Where each turn starts, in degrees from +X.
    pub seam: f64,
    /// A flat closing turn that ramps extrusion down (false: an open top).
    pub flat_top: bool,
}

/// One turn of [`vase_revolve`]: where it starts, how far it rises and its settings.
struct Turn {
    z: f64,
    dz: f64,
    width: f64,
}

/// A profile of revolution: radius against height, for [`vase_revolve`].
struct Profile {
    /// (radius, height), height never decreasing.
    points: Vec<[f64; 2]>,
}

impl Profile {
    /// The radius at height `z` (EasyGCode's `makeLampSampler`): between the last point at or
    /// below `z` and the next, linearly; the end radii beyond the ends.
    fn r(&self, z: f64) -> f64 {
        let pts = &self.points;
        let n = pts.len();
        let (first, last) = (pts[0], pts[n - 1]);
        if z <= first[1] {
            return first[0];
        }
        if z >= last[1] {
            return last[0];
        }
        let (mut lo, mut hi) = (0, n - 1);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if pts[mid][1] <= z {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let (a, b) = (pts[lo], pts[hi]);
        let span = b[1] - a[1];
        if span < 1e-9 {
            return b[0];
        }
        let f = ((z - a[1]) / span).clamp(0.0, 1.0);
        a[0] + (b[0] - a[0]) * f
    }
}

/// Vase mode around the Z axis, following a profile of revolution: EasyGCode's lampshade
/// (`generateLamp` in its gcode.js), whose arithmetic this follows. The profile's x is the
/// radius and its z the height; it's printed from its lowest point up.
///
/// - Turn by turn: the rise and line width are read at the turn's middle (estimated from the
///   rise at its start), the flow at its middle; a turn that would overshoot the top is cut
///   short, unless that leaves less than a quarter of its rise (then the closing turn finishes).
/// - Each turn goes once round, from the seam (`seam`, degrees from +X; EasyGCode: 90, the
///   back), counter-clockwise, with enough points that no chord strays more than `tolerance`
///   from the true circle (at least 24), the radius at each point read from the profile.
/// - The first turn holds the radius the second one starts at, while extrusion ramps up
///   0 → 100 %; a flat closing turn at the top ramps it back down (with `flat_top`; without
///   it, the spiral stops at full flow as it reaches the top: an open rim).
/// - Each turn is extruded for its own line width and, with `bead_layer` 0, its rise
///   (EasyGCode's width compensation), or for `bead_layer` (its layer-height compensation:
///   a shorter rise, the full bead).
pub fn vase_revolve(
    profile: &Curve,
    per_turn: &PerTurn,
    options: &Revolve,
    max_points: usize,
) -> Result<Toolpath, String> {
    let Revolve {
        speed,
        bead_layer,
        tolerance,
        seam,
        flat_top,
    } = *options;
    let mut points: Vec<[f64; 2]> = profile.points.iter().map(|p| [p[0], p[2]]).collect();
    if profile.closed {
        return Err("needs an open profile (radius against height), not a closed curve".into());
    }
    if points.len() < 2 {
        return Err("needs a profile of at least 2 points".into());
    }
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return Err("the profile has a point that is not a number".into());
    }
    if points.first().map(|p| p[1]) > points.last().map(|p| p[1]) {
        points.reverse();
    }
    if points.windows(2).any(|w| w[1][1] < w[0][1]) {
        return Err("the profile must rise all the way (its height may never go down)".into());
    }
    if points.iter().any(|p| p[0] <= 0.0) {
        return Err("the profile's radius (its x) must be more than 0 everywhere".into());
    }
    check("tolerance", tolerance, 0.001, 10.0, " mm")?;
    check("bead_layer", bead_layer, 0.0, 10.0, " mm")?;
    if !seam.is_finite() {
        return Err("`seam` must be a number".into());
    }
    let profile = Profile { points };
    let (z0, top) = (
        profile.points[0][1],
        profile.points[profile.points.len() - 1][1],
    );
    if top - z0 < 1e-6 {
        return Err("the profile has no height".into());
    }
    let seam = seam.to_radians();
    let on_profile = |z: f64| {
        let r = profile.r(z);
        [r * seam.cos(), r * seam.sin(), z]
    };
    let positive = |what: &str, v: f64, z: f64| -> Result<f64, String> {
        if v.is_finite() && v > 0.0 {
            Ok(v)
        } else {
            Err(format!(
                "`{what}` must be more than 0, but it's {v} at height {}",
                easycad_lang::format_number((z * 100.0).round() / 100.0)
            ))
        }
    };

    // The turns.
    let mut turns: Vec<Turn> = Vec::new();
    let mut zc = z0;
    while zc < top - 1e-6 {
        let first_rise = positive("layer", (per_turn.rise)(on_profile(zc))?, zc)?;
        let mid = top.min(zc + first_rise / 2.0);
        let rise = positive("layer", (per_turn.rise)(on_profile(mid))?, mid)?;
        check("layer", rise, 0.02, 10.0, " mm")?;
        let width = positive("width", (per_turn.width)(on_profile(mid))?, mid)?;
        let mut dz = rise;
        if zc + dz > top {
            dz = top - zc;
        }
        if dz < rise * 0.25 {
            break;
        }
        turns.push(Turn { z: zc, dz, width });
        zc += dz;
        too_many(turns.len(), max_points / 24, "use a larger layer height")?;
    }
    let (Some(first), Some(last)) = (turns.first(), turns.last()) else {
        return Err("the profile is shorter than one layer".into());
    };
    let steps_for = |r: f64| {
        let mut dth = 2.0 * (1.0 - tolerance / r.max(1e-6)).max(-1.0).acos();
        if !dth.is_finite() || dth <= 0.0 {
            dth = 0.2;
        }
        24.max((std::f64::consts::TAU / dth).ceil() as usize)
    };
    let r_first = profile.r(top.min(first.z + first.dz));
    let z_top = last.z + last.dz;
    let r_top = profile.r(z_top);
    let counts: Vec<usize> = turns
        .iter()
        .enumerate()
        .map(|(i, t)| {
            steps_for(if i == 0 {
                r_first
            } else {
                profile.r(t.z).max(profile.r(t.z + t.dz))
            })
        })
        .collect();
    let cap = if flat_top { steps_for(r_top) } else { 0 };
    let needed = counts
        .iter()
        .sum::<usize>()
        .saturating_add(cap)
        .saturating_add(1);
    too_many(needed, max_points, "use a larger tolerance or layer height")?;

    // Each turn's settings, and the closing turn's (the last turn's bead at the top's flow).
    let settings = |width: f64, dz: f64, at: f64| -> Result<Settings, String> {
        let layer = if bead_layer > 0.0 { bead_layer } else { dz };
        let flow = (per_turn.flow)(on_profile(at))?;
        if !flow.is_finite() || flow < 0.0 {
            return Err(format!("`flow` must be 0 (by speed) or more, got {flow}"));
        }
        Settings::from_inputs(width, layer, speed, flow)
    };
    let mut path = Toolpath::empty(Vec::new(), needed);
    let part_of = |path: &mut Toolpath, s: Settings| -> Result<u16, String> {
        if path.parts.last() != Some(&s) {
            path.parts.push(s);
        }
        u16::try_from(path.parts.len() - 1)
            .map_err(|_| "too many different turns; use a larger layer height".to_string())
    };
    let tau = std::f64::consts::TAU;
    let at = |r: f64, u: f64, z: f64| {
        let a = seam + tau * u;
        [r * a.cos(), r * a.sin(), z]
    };
    let start_part = part_of(
        &mut path,
        settings(first.width, first.dz, first.z + first.dz / 2.0)?,
    )?;
    path.push(at(r_first, 0.0, z0), true, 0.0, 0.0, start_part);
    for (i, (t, &steps)) in turns.iter().zip(&counts).enumerate() {
        let part = part_of(&mut path, settings(t.width, t.dz, t.z + t.dz / 2.0)?)?;
        let n = steps as f64;
        for s in 1..=steps {
            let u = s as f64 / n;
            let zp = t.z + t.dz * u;
            let (r, ramp) = if i == 0 {
                (r_first, (u + (s as f64 - 1.0) / n) / 2.0)
            } else {
                (profile.r(zp), 1.0)
            };
            path.push(at(r, u, zp), false, ramp, i as f64 + u, part);
        }
        if i == 0 {
            // The fan comes on after the ramp turn.
            path.layer_starts.push(path.points.len());
        }
    }
    // The closing turn (none for an open top: the last turn ends on the rim at full flow).
    let n = cap as f64;
    if cap > 0 {
        let part = part_of(&mut path, settings(last.width, last.dz, z_top)?)?;
        for s in 1..=cap {
            let u = s as f64 / n;
            let ramp = (1.0 - (u + (s as f64 - 1.0) / n) / 2.0).max(0.0);
            path.push(
                at(r_top, u, z_top),
                false,
                ramp,
                turns.len() as f64 + u,
                part,
            );
        }
    }
    path.layers = turns.len();
    Ok(path)
}

/// A helix around an exact circle: vase mode that rises exactly `pitch` per turn to exactly
/// `height`, without snapping to whole turns (EasyGCode's thread connector, `generateThread` in
/// its gcode.js, whose arithmetic this follows step by step). It starts on the circle's first
/// point, on the circle's plane, and climbs one pitch over the first turn while extrusion ramps
/// 0 → 100 % (each segment at its midpoint value); the rest rises evenly, any fraction of a turn
/// included; a flat closing turn at `height` ramps extrusion back to zero for a level top. The
/// circle's point count is the number of moves per turn, and its direction the way the helix
/// turns (counter-clockwise rising: a right-hand thread).
pub fn helix(
    base: &Curve,
    pitch: f64,
    height: f64,
    settings: Settings,
    max_points: usize,
) -> Result<Toolpath, String> {
    let Some(circle) = base.circle else {
        return Err(
            "needs a circle (from `circle`, maybe moved, turned or scaled evenly), not just any curve"
                .into(),
        );
    };
    let steps = base.points.len();
    let Some(first) = base.points.first().copied() else {
        return Err("the circle has no points".into());
    };
    if !base.closed || steps < 3 {
        return Err("the circle needs at least 3 points".into());
    }
    check("pitch", pitch, 0.02, 10.0, " mm")?;
    if !(height.is_finite() && height > pitch) {
        return Err(format!(
            "`height` must be more than one pitch ({} mm), got {height}",
            easycad_lang::format_number(pitch)
        ));
    }
    let turns = height / pitch;
    let rest = turns - 1.0;
    let rest_steps = ((rest * steps as f64).ceil() as usize).max(1);
    let needed = steps
        .saturating_mul(2)
        .saturating_add(rest_steps)
        .saturating_add(1);
    too_many(
        needed,
        max_points,
        "use a larger pitch or fewer points per turn",
    )?;

    let [cx, cy] = circle.center;
    let r = circle.r;
    let seam = (first[1] - cy).atan2(first[0] - cx);
    let dir = if base.signed_area() >= 0.0 { 1.0 } else { -1.0 };
    let z0 = first[2];
    let at = |a: f64, z: f64| [cx + r * a.cos(), cy + r * a.sin(), z];
    let n = steps as f64;
    let tau = std::f64::consts::TAU;

    let mut path = Toolpath::empty(vec![settings], needed);
    path.samples_per_turn = steps;
    path.layers = (turns.round() as usize).max(1);
    path.push(at(seam, z0), true, 0.0, 0.0, 0);
    // First turn: from the circle's plane up one pitch while extrusion ramps up.
    for s in 1..=steps {
        let u = s as f64 / n;
        let ramp = (u + (s as f64 - 1.0) / n) / 2.0;
        path.push(
            at(seam + dir * (tau * u), z0 + pitch * u),
            false,
            ramp,
            u,
            0,
        );
    }
    // The fan comes on after the ramp turn.
    path.layer_starts.push(steps + 1);
    // The rest at exactly one pitch per turn, any fraction of a turn included.
    for s in 1..=rest_steps {
        let t = 1.0 + (rest * s as f64) / rest_steps as f64;
        path.push(at(seam + dir * (tau * t), z0 + pitch * t), false, 1.0, t, 0);
    }
    // Closing turn: flat at the top while extrusion ramps back to zero.
    let end = seam + dir * (tau * turns);
    for s in 1..=steps {
        let u = s as f64 / n;
        let flow = (1.0 - (u + (s as f64 - 1.0) / n) / 2.0).max(0.0);
        path.push(
            at(end + dir * (tau * u), z0 + height),
            false,
            flow,
            turns + u,
            0,
        );
    }
    Ok(path)
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
    let mut new_index = Vec::with_capacity(path.points.len());
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
                if let Some(slot) = out.speed.last_mut() {
                    *slot = path.speed_factor(i);
                }
                out.copy_width(path, i);
            }
        }
        out.push_from(path, i, 0.0, 0);
        new_index.push(out.points.len() - 1);
    }
    out.copy_events(path, &|i| new_index.get(i).copied());
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
        let p = vase_spiral(&w, settings(0.6, 0.3, 40.0), true, MAX_PATH_POINTS).unwrap();
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
    fn an_open_top_ends_the_revolved_spiral_on_the_rim_at_full_flow() {
        // A cylinder 20 mm round and 6 mm high, 2 mm a turn: 3 turns of 24 points.
        let profile = Curve::new(vec![[20.0, 0.0, 0.0], [20.0, 0.0, 6.0]], false);
        let (rise, width, flow) = (|_| Ok(2.0), |_| Ok(3.0), |_| Ok(0.0));
        let per_turn = PerTurn {
            rise: &rise,
            width: &width,
            flow: &flow,
        };
        let options = |flat_top| Revolve {
            speed: 40.0,
            bead_layer: 0.0,
            tolerance: 1.0,
            seam: 90.0,
            flat_top,
        };
        let flat = vase_revolve(&profile, &per_turn, &options(true), MAX_PATH_POINTS).unwrap();
        let open = vase_revolve(&profile, &per_turn, &options(false), MAX_PATH_POINTS).unwrap();
        assert_eq!((flat.layers, open.layers), (3, 3));
        // The same spiral, without the closing turn's 24 points.
        assert_eq!(flat.points.len(), 1 + 4 * 24);
        assert_eq!(open.points.len(), 1 + 3 * 24);
        assert_eq!(open.points[..], flat.points[..open.points.len()]);
        assert_eq!(open.flow[..], flat.flow[..open.points.len()]);
        // It ends on the seam at the top, still at full flow.
        let end = open.points[open.points.len() - 1];
        assert!(end[0].abs() < 1e-9 && (end[1] - 20.0).abs() < 1e-9);
        assert!((end[2] - 6.0).abs() < 1e-9);
        assert_eq!(open.flow.last().copied(), Some(1.0));
        // The closing turn ramped down to nothing.
        assert!((flat.flow[flat.flow.len() - 1] - 1.0 / 48.0).abs() < 1e-12);
    }

    #[test]
    fn ramps_average_each_segment_like_easygcode() {
        let p = vase_spiral(
            &wall(1.0, 0.0),
            settings(0.6, 0.3, 40.0),
            true,
            MAX_PATH_POINTS,
        )
        .unwrap();
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
            vase_spiral(
                &wall(top, 0.0),
                settings(0.6, 0.3, 40.0),
                true,
                MAX_PATH_POINTS,
            )
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
            vase_spiral(&flat, ok, true, MAX_PATH_POINTS)
                .unwrap_err()
                .contains("same height")
        );
        let mut upside_down = w.clone();
        upside_down.sections.reverse();
        assert!(
            vase_spiral(&upside_down, ok, true, MAX_PATH_POINTS)
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
            vase_spiral(&tall, settings(0.6, 0.02, 40.0), true, MAX_PATH_POINTS)
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
        let p = stack(&one, 3, 1.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(p.points.len(), 9 * 3 + 2 * 2);
        assert_eq!(p.layer_starts, vec![0, 9, 20]);
        // Lift one layer above the next layer, over to the start, then down onto it.
        let near = |a: P3, b: P3| dist3(a, b) < 1e-9;
        assert!(near(p.points[9], [10.0, 0.0, 1.8]));
        assert!(near(p.points[10], [10.0, 0.0, 1.8]));
        assert!(near(p.points[11], [10.0, 0.0, 1.2]));
        assert!(p.travel[9] && p.travel[10] && p.travel[11] && !p.travel[12]);
        assert_eq!(p.layers, 3);

        // Lift 0 (EasyGCode's spoon): up to the new layer where the last one ended, then
        // straight over to the start; no move down.
        let open = Curve::new(vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 5.0, 0.0]], false);
        let one = from_curves(&[&open], settings(2.0, 0.6, 20.0), MAX_PATH_POINTS).unwrap();
        let p = stack(&one, 2, 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(p.points.len(), 3 + 2 + 2);
        assert!(near(p.points[3], [10.0, 5.0, 1.2]) && near(p.points[4], [0.0, 0.0, 1.2]));
        assert!(p.travel[3] && p.travel[4] && !p.travel[5]);
        assert!(near(p.points[5], [10.0, 0.0, 1.2]));
        assert_eq!(p.layer_starts, vec![0, 3]);
        assert!(stack(&one, 2, -1.0, 0.0, MAX_PATH_POINTS).is_err());
    }

    #[test]
    fn joins_hop_clear_or_carry_straight_on() {
        let near = |a: P3, b: P3| dist3(a, b) < 1e-9;
        let flat = |pts: Vec<P3>, layer: f64| {
            from_curves(
                &[&Curve::new(pts, false)],
                settings(1.0, layer, 20.0),
                MAX_PATH_POINTS,
            )
            .unwrap()
        };
        let a = flat(vec![[0.0; 3], [10.0, 0.0, 0.0]], 0.8);
        let b = flat(vec![[20.0, 0.0, -0.8], [30.0, 0.0, -0.8]], 1.25); // printed at z = 0.45
        // An absolute travel height: up to 1.6, over, down (EasyGCode's 2 × brim layer).
        let j = join(&[&a, &b], 1.6, 0.0, MAX_PATH_POINTS).unwrap();
        assert!(near(j.points[2], [10.0, 0.0, 1.6]) && near(j.points[3], [20.0, 0.0, 1.6]));
        assert!(near(j.points[4], [20.0, 0.0, 0.45]) && j.travel[4] && !j.travel[5]);
        // Automatic: one layer of the next part above the higher end.
        let j = join(&[&a, &b], 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert!(near(j.points[2], [10.0, 0.0, 0.8 + 1.25]));
        // A part that starts where the last one ended carries straight on, no travel.
        let c = flat(vec![[10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], 0.8);
        let j = join(&[&a, &c], 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(j.points.len(), 3);
        assert_eq!(j.travel, vec![true, false, false]);
        assert_eq!(j.part, vec![0, 0, 1]);
        // Empty parts are skipped.
        let none = from_curves(&[], settings(1.0, 0.8, 20.0), MAX_PATH_POINTS).unwrap();
        assert!(none.points.is_empty());
        let j = join(&[&none, &a, &none], 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!((j.points.len(), j.parts.len()), (2, 1));
        assert!(join(&[&a], -1.0, 0.0, MAX_PATH_POINTS).is_err());
    }

    #[test]
    fn a_brim_and_the_helix_inside_it_are_one_first_layer() {
        // EasyGCode turns the fan on after the helix's first turn, not after the brim.
        let ring = Curve::circle(12.0, 24);
        let brim = from_curves(&[&ring], settings(2.0, 1.25, 20.0), MAX_PATH_POINTS).unwrap();
        let h = helix(
            &Curve::circle(10.0, 24),
            1.25,
            5.0,
            settings(3.0, 1.25, 20.0),
            MAX_PATH_POINTS,
        )
        .unwrap();
        assert_eq!(h.layer_starts, vec![0, 25]);
        let j = join(&[&brim, &h], 2.5, 0.0, MAX_PATH_POINTS).unwrap();
        // The brim (25 points), the hop (up, over), then the helix from its first point.
        assert_eq!(j.layer_starts, vec![0, 25 + 2 + 25]);
        // Several bottom layers keep their own first layer.
        let two = stack(&brim, 2, 1.0, 0.0, MAX_PATH_POINTS).unwrap();
        let j = join(&[&two, &h], 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(j.layer_starts[..2], [0, 25]);
    }

    #[test]
    fn a_primer_after_the_first_layer_belongs_to_it_and_wipes_go_straight_over() {
        let ring = Curve::circle(10.0, 12);
        let layer = |z: f64| {
            from_curves(
                &[&ring.transformed(&Xform::translate(0.0, 0.0, z))],
                settings(1.0, 0.5, 20.0),
                MAX_PATH_POINTS,
            )
            .unwrap()
        };
        let primer = from_curves(
            &[&Curve::new(
                vec![[-30.0, -30.0, 0.0], [-20.0, -30.0, 0.0]],
                false,
            )],
            settings(1.0, 0.3, 10.0),
            MAX_PATH_POINTS,
        )
        .unwrap();
        // The primer (z = 0.3) is no higher than layer 1 (z = 0.5); layer 2 (z = 1) is.
        let j = join(
            &[&layer(0.0), &primer, &layer(0.5)],
            1.0,
            0.0,
            MAX_PATH_POINTS,
        )
        .unwrap();
        // Layer 1 (13 points), the hop to the primer (up, over, down) and its line, the hop to
        // layer 2 (up, over): layer 2 starts with the move down onto it.
        assert_eq!(j.layer_starts, vec![0, 13 + 3 + 1 + 2]);
        assert_eq!(j.layers, 2);
        // A wipe: one straight move to the wipe height above the next start, then down.
        let w = join(&[&layer(0.0), &layer(0.5)], 0.0, 1.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(w.points[13], [10.0, 0.0, 2.0]);
        assert_eq!(w.points[14], [10.0, 0.0, 1.0]);
        assert!(w.travel[13] && w.travel[14] && !w.travel[15]);
        let s = stack(&layer(0.0), 3, 1.0, 1.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(s.points[13], [10.0, 0.0, 2.0]);
        assert_eq!(s.points[14], [10.0, 0.0, 1.0]);
        assert_eq!(s.layer_starts, vec![0, 13, 13 + 14]);
        // No copies: an empty toolpath, which `join` skips.
        assert!(
            stack(&layer(0.0), 0, 1.0, 0.0, MAX_PATH_POINTS)
                .unwrap()
                .points
                .is_empty()
        );
    }

    #[test]
    fn helices_rise_exactly_one_pitch_per_turn() {
        let c = Curve::circle(10.0, 36).transformed(&Xform::rotate_z(90.0));
        let h = helix(&c, 1.25, 20.0, settings(3.0, 1.25, 20.0), MAX_PATH_POINTS).unwrap();
        // 16 turns: the travel, the ramp turn, 15 more turns, the closing turn.
        assert_eq!(h.points.len(), 1 + 36 + 15 * 36 + 36);
        let near = |a: P3, b: P3| dist3(a, b) < 1e-9;
        assert!(near(h.points[0], [0.0, 10.0, 0.0]) && h.travel[0]);
        assert!(near(h.points[36], [0.0, 10.0, 1.25]));
        assert!(near(h.points[36 + 15 * 36], [0.0, 10.0, 20.0]));
        assert!(h.points.iter().all(|p| p[2] <= 20.0 + 1e-12));
        assert!((h.flow[1] - 1.0 / 72.0).abs() < 1e-12); // midpoint of 0 → 1/36
        assert!(h.flow.last().is_some_and(|f| *f < 0.02));
        // A fraction of a turn is spread evenly over the moves it needs.
        let f = helix(&c, 1.25, 20.5, settings(3.0, 1.25, 20.0), MAX_PATH_POINTS).unwrap();
        let rest = ((20.5 / 1.25 - 1.0) * 36.0_f64).ceil() as usize;
        assert_eq!(f.points.len(), 1 + 36 + rest + 36);
        assert!(near(f.points[36 + rest], {
            let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * 20.5 / 1.25;
            [10.0 * a.cos(), 10.0 * a.sin(), 20.5]
        }));
        // Clockwise circles make left-hand helices.
        let left = helix(
            &c.flipped(),
            1.25,
            5.0,
            settings(3.0, 1.25, 20.0),
            MAX_PATH_POINTS,
        );
        assert!(
            left.is_err(),
            "a flipped circle is no longer an exact circle"
        );
        let cw = Curve {
            circle: c.circle,
            ..c.flipped()
        };
        let left = helix(&cw, 1.25, 5.0, settings(3.0, 1.25, 20.0), MAX_PATH_POINTS).unwrap();
        assert!(left.points[1][0] > 0.0, "{:?}", left.points[1]); // clockwise from +Y
        assert!(
            helix(
                &Curve::polygon(6, 10.0),
                1.0,
                5.0,
                settings(3.0, 1.0, 20.0),
                100
            )
            .is_err()
        );
        assert!(helix(&c, 1.25, 1.0, settings(3.0, 1.25, 20.0), MAX_PATH_POINTS).is_err());
        assert!(helix(&c, 0.0, 10.0, settings(3.0, 1.25, 20.0), MAX_PATH_POINTS).is_err());
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
        let j = join(&[&a, &b], 0.0, 0.0, MAX_PATH_POINTS).unwrap();
        assert_eq!(j.parts.len(), 2);
        assert_eq!(j.settings(j.points.len() - 1).width, 1.0);
        assert_eq!(j.settings(3).width, 2.0);
        // Two one-layer parts are one layer.
        assert_eq!((j.layer_starts.clone(), j.layers), (vec![0], 1));
    }

    #[test]
    fn whole_waves_stack_and_half_waves_alternate() {
        let c = Curve::circle(40.0, 96);
        let top = c.transformed(&Xform::translate(0.0, 0.0, 30.0));
        let w = Surface::loft(&[&c, &top]).unwrap();
        let p = vase_spiral(&w, settings(0.8, 0.5, 30.0), true, MAX_PATH_POINTS).unwrap();
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
