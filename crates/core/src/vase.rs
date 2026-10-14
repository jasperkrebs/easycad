//! Phase 0 demo project: a twisted star vase.
//!
//! Built the way a Grasshopper definition would build it:
//! star corners → smooth closed NURBS curve through them (`curvo`) → sections that scale and
//! twist with height. One construction drives three outputs:
//!
//! - a **preview mesh** (rings of sections plus flat caps),
//! - a **continuous vase-mode toolpath**: one helix that rises one layer per revolution, with
//!   EasyGCode's conventions (the first revolution climbs from z = 0 while extrusion ramps
//!   0 → 100 %, the height snaps to whole layers, and a flat cap revolution ramps extrusion
//!   back to 0),
//! - **print statistics** from the bead model (length, volume, mass, time, flow, overhang).
//!
//! No G-code yet: G-code drives real machines and only arrives once it is golden-tested
//! against EasyGCode (Phase 1).

use crate::bead::bead_area;
use crate::error::{Error, check_range};
use crate::geom::{P2, P3, dist3, signed_distance_to_ring};
use curvo::prelude::*;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};

/// PLA, for the mass estimate.
const PLA_DENSITY_G_PER_MM3: f64 = 1.24e-3;
/// Above this the toolpath gets too heavy to compute and draw interactively.
const MAX_PATH_POINTS: usize = 3_000_000;

/// How finely curves are sampled: coarse while dragging a slider, fine on release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Preview,
    Final,
}

impl Quality {
    /// Samples per star corner along the outline.
    fn samples_per_corner(self) -> usize {
        match self {
            Quality::Preview => 8,
            Quality::Final => 16,
        }
    }
}

/// The demo's settings, one per slider in the web app. Lengths in mm, angles in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct VaseParams {
    /// Total height of the vase.
    pub height: f64,
    /// Radius of the star tips at the bottom.
    pub radius: f64,
    /// Number of star tips.
    pub points: u32,
    /// How far the inner corners pull in, as a fraction of the radius (0 = circle).
    pub depth: f64,
    /// Rotation of the top section relative to the bottom.
    pub twist: f64,
    /// Size of the top section relative to the bottom.
    pub top_scale: f64,
    /// Layer height: in vase mode, the rise per revolution.
    pub layer: f64,
    /// Extrusion line width.
    pub line: f64,
    /// Print speed in mm/s, for the time and flow estimates.
    pub speed: f64,
    pub quality: Quality,
}

impl Default for VaseParams {
    fn default() -> Self {
        Self {
            height: 160.0,
            radius: 45.0,
            points: 7,
            depth: 0.18,
            twist: 90.0,
            top_scale: 0.8,
            layer: 0.3,
            line: 0.6,
            speed: 40.0,
            quality: Quality::Preview,
        }
    }
}

impl VaseParams {
    /// Rejects values the engine can't handle. The UI's sliders stay well inside these limits;
    /// these protect against bad files and agent mistakes.
    pub fn validate(&self) -> Result<(), Error> {
        check_range("height", self.height, 1.0, 2000.0)?;
        check_range("radius", self.radius, 1.0, 1000.0)?;
        check_range("points", f64::from(self.points), 2.0, 64.0)?;
        check_range("depth", self.depth, 0.0, 0.95)?;
        check_range("twist", self.twist, -3600.0, 3600.0)?;
        check_range("topScale", self.top_scale, 0.05, 10.0)?;
        check_range("layer", self.layer, 0.02, 10.0)?;
        check_range("line", self.line, 0.05, 20.0)?;
        check_range("speed", self.speed, 0.1, 1000.0)?;
        Ok(())
    }
}

/// Triangle mesh for the 3D view: `positions` as x,y,z triples, `indices` as triangles.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mesh {
    pub positions: Vec<f32>,
    pub indices: Vec<u32>,
}

/// Print statistics shown in the app (serialized with camelCase names for JavaScript).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaseStats {
    /// Spiral revolutions (= layers), not counting the flat cap revolution.
    pub revolutions: usize,
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
    pub samples_per_revolution: usize,
    pub path_points: usize,
    pub triangles: usize,
}

/// Everything the demo produces in one evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct VaseOutput {
    pub mesh: Mesh,
    /// The toolpath as x,y,z triples, in print order.
    pub path: Vec<f32>,
    pub stats: VaseStats,
}

/// Evaluates the demo: outline → mesh, toolpath and statistics.
pub fn build(p: &VaseParams) -> Result<VaseOutput, Error> {
    p.validate()?;
    let outline = star_outline(p)?;

    let revolutions = (p.height / p.layer).round().max(1.0) as usize;
    let printed_height = revolutions as f64 * p.layer;
    let points_needed = (revolutions + 1) * outline.len() + 1;
    if points_needed > MAX_PATH_POINTS {
        return Err(Error::TooLarge(format!(
            "{revolutions} layers × {} points per layer; use a larger layer height",
            outline.len()
        )));
    }

    let mesh = build_mesh(p, &outline);
    let (path, path_length, extruded_length) = build_path(p, &outline, revolutions);
    let area = bead_area(p.line, p.layer);
    let volume = extruded_length * area;

    let stats = VaseStats {
        revolutions,
        printed_height_mm: printed_height,
        path_length_mm: path_length,
        bead_area_mm2: area,
        volume_mm3: volume,
        mass_g_pla: volume * PLA_DENSITY_G_PER_MM3,
        time_s: path_length / p.speed,
        flow_mm3_s: p.speed * area,
        max_overhang_deg: max_overhang_deg(p, &outline, revolutions),
        samples_per_revolution: outline.len(),
        path_points: path.len() / 3,
        triangles: mesh.indices.len() / 3,
    };
    Ok(VaseOutput { mesh, path, stats })
}

/// The star's corners, alternating tip and inner corner, counter-clockwise from +X, with a
/// smooth closed NURBS curve through them, sampled evenly in curve parameter.
fn star_outline(p: &VaseParams) -> Result<Vec<P2>, Error> {
    let n = p.points as usize;
    let inner = p.radius * (1.0 - p.depth);
    let corners: Vec<Point2<f64>> = (0..2 * n)
        .map(|i| {
            let a = i as f64 * std::f64::consts::PI / n as f64;
            let r = if i % 2 == 0 { p.radius } else { inner };
            Point2::new(r * a.cos(), r * a.sin())
        })
        .collect();
    let curve = NurbsCurve2D::<f64>::interpolate_periodic(&corners, 3, KnotStyle::Centripetal)
        .map_err(|e| Error::Geometry(format!("star outline: {e}")))?;

    let samples = 2 * n * p.quality.samples_per_corner();
    let (start, end) = curve.knots_domain();
    let span = end - start;
    Ok((0..samples)
        .map(|j| {
            let q = curve.point_at(start + span * j as f64 / samples as f64);
            [q.x, q.y]
        })
        .collect())
}

/// Scale, rotation and height of the section at height `z`, as a function of z / `total`.
#[derive(Debug, Clone, Copy)]
struct Section {
    cos: f64,
    sin: f64,
    scale: f64,
    z: f64,
}

fn section(p: &VaseParams, z: f64, total: f64) -> Section {
    let t = if total > 0.0 {
        (z / total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let angle = (p.twist * t).to_radians();
    Section {
        cos: angle.cos(),
        sin: angle.sin(),
        scale: 1.0 + (p.top_scale - 1.0) * t,
        z,
    }
}

fn place(s: Section, q: P2) -> P3 {
    [
        (q[0] * s.cos - q[1] * s.sin) * s.scale,
        (q[0] * s.sin + q[1] * s.cos) * s.scale,
        s.z,
    ]
}

/// Rings of sections with flat caps. Ring count follows the twist so twisted walls stay smooth.
fn build_mesh(p: &VaseParams, outline: &[P2]) -> Mesh {
    let m = outline.len();
    let base_rings = 24 + (p.twist.abs() / 5.0) as usize;
    let rings = match p.quality {
        Quality::Preview => base_rings,
        Quality::Final => base_rings * 2,
    }
    .clamp(2, 512);

    let mut positions = Vec::with_capacity((rings * m + 2) * 3);
    for i in 0..rings {
        let s = section(p, p.height * i as f64 / (rings - 1) as f64, p.height);
        for q in outline {
            positions.extend(place(s, *q).map(|v| v as f32));
        }
    }
    let bottom_center = (rings * m) as u32;
    positions.extend([0.0, 0.0, 0.0]);
    let top_center = bottom_center + 1;
    positions.extend([0.0, 0.0, p.height as f32]);

    let mut indices = Vec::with_capacity(((rings - 1) * m * 2 + 2 * m) * 3);
    let at = |ring: usize, j: usize| (ring * m + j % m) as u32;
    for i in 0..rings - 1 {
        for j in 0..m {
            let (a, b) = (at(i, j), at(i, j + 1));
            let (c, d) = (at(i + 1, j + 1), at(i + 1, j));
            // Counter-clockwise seen from outside, so normals point outward.
            indices.extend([a, b, c, a, c, d]);
        }
    }
    for j in 0..m {
        indices.extend([bottom_center, at(0, j + 1), at(0, j)]);
        indices.extend([top_center, at(rings - 1, j), at(rings - 1, j + 1)]);
    }
    Mesh { positions, indices }
}

/// The vase-mode spiral. Returns (points, path length, extruded length), where the extruded
/// length counts each segment at its flow fraction (the start and cap ramps are partial).
fn build_path(p: &VaseParams, outline: &[P2], revolutions: usize) -> (Vec<f32>, f64, f64) {
    let m = outline.len();
    let total = revolutions as f64 * p.layer;
    let mut path = Vec::with_capacity(((revolutions + 1) * m + 1) * 3);
    let mut length = 0.0;
    let mut extruded = 0.0;
    let mut prev: Option<P3> = None;
    let mut push = |pt: P3, flow: f64, path: &mut Vec<f32>| {
        if let Some(q) = prev {
            let d = dist3(q, pt);
            length += d;
            extruded += d * flow;
        }
        prev = Some(pt);
        path.extend(pt.map(|v| v as f32));
    };

    // Spiral revolutions: z rises one layer per revolution, starting at z = 0.
    for k in 0..revolutions {
        for (j, q) in outline.iter().enumerate() {
            let u = j as f64 / m as f64;
            let z = p.layer * (k as f64 + u);
            let flow = if k == 0 { u } else { 1.0 };
            push(place(section(p, z, total), *q), flow, &mut path);
        }
    }
    // Flat cap: one more revolution at the top height while extrusion ramps back to zero,
    // ending on the seam so the rim closes cleanly.
    let top = section(p, total, total);
    for (j, q) in outline.iter().enumerate() {
        let u = j as f64 / m as f64;
        push(place(top, *q), 1.0 - u, &mut path);
    }
    if let Some(first) = outline.first() {
        push(place(top, *first), 0.0, &mut path);
    }
    (path, length, extruded)
}

/// Steepest overhang: how far a point on one layer sticks out past the layer below, turned
/// into an angle from vertical. Sampled on up to 24 evenly spaced layer pairs.
fn max_overhang_deg(p: &VaseParams, outline: &[P2], revolutions: usize) -> f64 {
    if revolutions < 2 {
        return 0.0;
    }
    let total = revolutions as f64 * p.layer;
    let pairs = revolutions.min(24);
    let mut worst: f64 = 0.0;
    for s in 0..pairs {
        let k = s * (revolutions - 1) / pairs;
        let z0 = k as f64 * p.layer;
        let lower: Vec<P2> = outline
            .iter()
            .map(|q| {
                let v = place(section(p, z0, total), *q);
                [v[0], v[1]]
            })
            .collect();
        let upper = section(p, z0 + p.layer, total);
        for q in outline {
            let v = place(upper, *q);
            if let Some(d) = signed_distance_to_ring([v[0], v[1]], &lower) {
                if d > 0.0 {
                    worst = worst.max(d.atan2(p.layer).to_degrees());
                }
            }
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z_values(path: &[f32]) -> Vec<f32> {
        path.chunks_exact(3).map(|c| c[2]).collect()
    }

    #[test]
    fn default_vase_builds() {
        let out = build(&VaseParams::default()).unwrap();
        // 160 / 0.3 = 533.3 → 533 whole layers, like EasyGCode's height snapping.
        assert_eq!(out.stats.revolutions, 533);
        assert!((out.stats.printed_height_mm - 159.9).abs() < 1e-9);
        assert!(out.stats.triangles > 1000);
        assert_eq!(out.stats.path_points * 3, out.path.len());
    }

    #[test]
    fn path_starts_on_the_bed_rises_monotonically_and_ends_at_the_top() {
        let out = build(&VaseParams::default()).unwrap();
        let z = z_values(&out.path);
        assert_eq!(z.first().copied(), Some(0.0));
        assert!(z.windows(2).all(|w| w[1] >= w[0]));
        let top = z.last().copied().unwrap();
        assert!((f64::from(top) - out.stats.printed_height_mm).abs() < 1e-3);
    }

    #[test]
    fn ramps_make_volume_less_than_full_flow() {
        let out = build(&VaseParams::default()).unwrap();
        let full = out.stats.path_length_mm * out.stats.bead_area_mm2;
        assert!(out.stats.volume_mm3 < full);
        assert!(out.stats.volume_mm3 > 0.95 * full);
    }

    #[test]
    fn mesh_indices_are_in_range() {
        let out = build(&VaseParams::default()).unwrap();
        let verts = (out.mesh.positions.len() / 3) as u32;
        assert!(out.mesh.indices.iter().all(|&i| i < verts));
        assert_eq!(out.mesh.indices.len() % 3, 0);
    }

    #[test]
    fn straight_cylinder_has_no_overhang() {
        let p = VaseParams {
            twist: 0.0,
            top_scale: 1.0,
            depth: 0.0,
            ..VaseParams::default()
        };
        assert!(build(&p).unwrap().stats.max_overhang_deg < 0.5);
    }

    #[test]
    fn flaring_out_overhangs_tapering_in_does_not() {
        let flare = VaseParams {
            twist: 0.0,
            top_scale: 2.0,
            ..VaseParams::default()
        };
        let taper = VaseParams {
            twist: 0.0,
            top_scale: 0.5,
            ..VaseParams::default()
        };
        let flare_deg = build(&flare).unwrap().stats.max_overhang_deg;
        // Radius grows 45 mm over 160 mm of height: about atan(45/160) ≈ 15.7° at the tips.
        assert!(flare_deg > 10.0 && flare_deg < 25.0, "{flare_deg}");
        assert!(build(&taper).unwrap().stats.max_overhang_deg < 0.5);
    }

    #[test]
    fn same_settings_give_the_same_result() {
        let p = VaseParams {
            quality: Quality::Final,
            ..VaseParams::default()
        };
        assert_eq!(build(&p).unwrap(), build(&p).unwrap());
    }

    #[test]
    fn bad_settings_are_errors_not_crashes() {
        let nan = VaseParams {
            height: f64::NAN,
            ..VaseParams::default()
        };
        assert!(matches!(
            build(&nan),
            Err(Error::InvalidParam { name: "height", .. })
        ));
        let zero_layer = VaseParams {
            layer: 0.0,
            ..VaseParams::default()
        };
        assert!(matches!(
            build(&zero_layer),
            Err(Error::InvalidParam { name: "layer", .. })
        ));
        let too_many_tips = VaseParams {
            points: 1000,
            ..VaseParams::default()
        };
        assert!(build(&too_many_tips).is_err());
    }

    #[test]
    fn huge_toolpaths_are_refused_with_a_message() {
        let p = VaseParams {
            height: 2000.0,
            layer: 0.02,
            points: 64,
            quality: Quality::Final,
            ..VaseParams::default()
        };
        assert!(matches!(build(&p), Err(Error::TooLarge(_))));
    }
}
