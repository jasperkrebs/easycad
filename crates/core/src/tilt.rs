//! The nozzle tilted along the wall, for a robot arm (`tilt`).
//!
//! A Cartesian printer slices a leaning wall by height and makes up for the lean with thinner
//! layers (overhang compensation by layer height): each turn rises the layer height × cos(lean),
//! so it steps one layer height along the wall. A robot arm can hold the nozzle along the wall
//! instead, so each bead is laid on the one before it rather than hanging off it. The path stays
//! where it is; what changes is the nozzle's direction at each point and the bead each move
//! gets:
//!
//! - **Direction:** up the wall (`wall::up_the_wall`: from the bead underneath to this point,
//!   square to the travel), averaged over `smooth` mm of path so the wrist turns gently (a line
//!   that runs from the wall in to the wings and back changes its lean within a layer), then
//!   held to `max` degrees from upright, and near the bed to less: the most grows from 0 on the
//!   bed to `max` at `bed` mm up, so the tool's fans clear the bed.
//! - **Bead:** a tilted nozzle lays the bead across the gap along its own axis. The gap between
//!   a bead and the one underneath is the move's layer height ÷ cos(lean) along the wall; along
//!   a nozzle tilted `t` it is that × cos(lean − t). So each move's layer height becomes
//!   h · (u·n) / u_z (u up the wall, n the nozzle): the full along-the-wall step where the
//!   nozzle follows the wall, unchanged where it stays upright, at most 3 × h and never past
//!   the part's layer height (a full bead) unless it was already. That's for moves
//!   extruded for their rise (squeezed by compensation, as `thread_shade`'s wall); a toolpath
//!   already extruded for the full bead (`vase_revolve`'s `bead_layer`) keeps it with `bead`
//!   off.

use crate::geom::{P3, dot3, unit3};
use crate::toolpath::Toolpath;
use crate::wall::up_the_wall;

const UP: P3 = [0.0, 0.0, 1.0];
/// The most a move's layer height may grow (a wall that runs almost flat).
const MAX_BEAD_GROWTH: f64 = 3.0;

/// The toolpath with the nozzle tilted along the wall (see the module). `max` 0 is upright
/// everywhere: the toolpath as it was, with no nozzle direction.
pub fn tilt(
    path: &Toolpath,
    max: f64,
    smooth: f64,
    bed: f64,
    bead: bool,
) -> Result<Toolpath, String> {
    if !(max.is_finite() && (0.0..=80.0).contains(&max)) {
        return Err(format!("`max` must be between 0 and 80°, got {max}"));
    }
    if !(smooth.is_finite() && (0.0..=1000.0).contains(&smooth)) {
        return Err(format!(
            "`smooth` must be between 0 and 1000 mm, got {smooth}"
        ));
    }
    if !(bed.is_finite() && (0.0..=1000.0).contains(&bed)) {
        return Err(format!("`bed` must be between 0 and 1000 mm, got {bed}"));
    }
    let mut out = path.clone();
    out.axis.clear();
    let n = path.points.len();
    if max == 0.0 || n == 0 {
        return Ok(out);
    }
    let ups = up_the_wall(path);
    let travel = |i: usize| i == 0 || path.travel.get(i).copied().unwrap_or(true);
    let smoothed = average_along(path, &ups, smooth);
    let floor = (0..n)
        .filter(|&i| !travel(i))
        .map(|i| path.points[i][2])
        .fold(f64::INFINITY, f64::min);
    let floor = if floor.is_finite() { floor } else { 0.0 };
    let mut axis = Vec::with_capacity(n);
    for (i, v) in smoothed.into_iter().enumerate() {
        let up = path.points[i][2] - floor;
        let most = if bed > 0.0 {
            max * (up / bed).clamp(0.0, 1.0)
        } else {
            max
        };
        axis.push(capped(v, most));
    }
    // Each printed move's bead along its nozzle (unless its bead is already full: `bead` off).
    for i in 0..n {
        if !bead || travel(i) {
            continue;
        }
        let u = ups[i];
        let growth = if u[2] > 1e-6 {
            (dot3(u, axis[i]) / u[2]).clamp(1.0, MAX_BEAD_GROWTH)
        } else {
            MAX_BEAD_GROWTH
        };
        if growth > 1.0 + 1e-12 {
            // Never thicker than a full layer of its part (a bead already that thick, as a
            // wing's for the gap it's in, stays as it is).
            let h = path.bead_of(i).1;
            let full = path.settings(i).layer.max(h);
            if let Some(slot) = out.height.get_mut(i) {
                *slot = (h * growth).min(full).min(10.0);
            }
        }
    }
    out.axis = axis;
    Ok(out)
}

/// `v` turned towards upright until it's at most `most` degrees from it.
fn capped(v: P3, most: f64) -> P3 {
    let lean = v[2].clamp(-1.0, 1.0).acos().to_degrees();
    if lean <= most {
        return v;
    }
    let across = v[0].hypot(v[1]);
    if across < 1e-12 {
        return UP;
    }
    let (s, c) = most.to_radians().sin_cos();
    [v[0] / across * s, v[1] / across * s, c]
}

/// Each direction averaged with its neighbours within `window` mm of path on either side (half
/// the window each way), not across travels; unit length again.
fn average_along(path: &Toolpath, v: &[P3], window: f64) -> Vec<P3> {
    let n = v.len();
    if window <= 0.0 || n < 2 {
        return v.to_vec();
    }
    let half = window / 2.0;
    // Distance along the path, and which run (between travels) each point is in.
    let mut s = vec![0.0; n];
    let mut run = vec![0usize; n];
    for i in 1..n {
        let p = path.points[i];
        let q = path.points[i - 1];
        s[i] = s[i - 1]
            + ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
        run[i] = run[i - 1] + usize::from(path.travel.get(i).copied().unwrap_or(false));
    }
    // Prefix sums of the vectors.
    let mut sum = vec![[0.0; 3]; n + 1];
    for i in 0..n {
        for k in 0..3 {
            sum[i + 1][k] = sum[i][k] + v[i][k];
        }
    }
    let (mut lo, mut hi) = (0usize, 0usize);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        while lo < i && (s[i] - s[lo] > half || run[lo] != run[i]) {
            lo += 1;
        }
        hi = hi.max(i);
        while hi + 1 < n && s[hi + 1] - s[i] <= half && run[hi + 1] == run[i] {
            hi += 1;
        }
        let m = [
            sum[hi + 1][0] - sum[lo][0],
            sum[hi + 1][1] - sum[lo][1],
            sum[hi + 1][2] - sum[lo][2],
        ];
        out.push(unit3(m).unwrap_or(v[i]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::toolpath::{Settings, from_curves};

    /// A cone wall leaning `lean` degrees out, built as one line of rings: each ring one turn,
    /// rising h·cos(lean)... as overhang compensation lays it (h along the wall).
    fn cone(lean: f64, turns: usize) -> Toolpath {
        let h = 2.5;
        let (s, c) = lean.to_radians().sin_cos();
        let per = 120;
        let mut pts = Vec::new();
        for k in 0..=turns * per {
            let t = k as f64 / per as f64;
            let a = t * std::f64::consts::TAU;
            let r = 60.0 + t * h * s;
            pts.push([r * a.cos(), r * a.sin(), t * h * c]);
        }
        let bead: Vec<[f64; 2]> = pts.iter().map(|_| [5.0, h * c]).collect();
        let curve = Curve {
            points: pts,
            closed: false,
            circle: None,
            winding: None,
            bead: Some(bead),
        };
        from_curves(&[&curve], Settings::new(5.0, h, 20.0).unwrap(), 1_000_000).unwrap()
    }

    fn lean_of(v: P3) -> f64 {
        v[2].clamp(-1.0, 1.0).acos().to_degrees()
    }

    #[test]
    fn upright_when_max_is_zero() {
        let p = cone(30.0, 4);
        let t = tilt(&p, 0.0, 20.0, 0.0, true).unwrap();
        assert!(t.axes().is_none());
        assert_eq!(t.height, p.height);
    }

    #[test]
    fn follows_a_leaning_wall_and_lays_the_full_bead() {
        let p = cone(30.0, 12);
        let t = tilt(&p, 45.0, 20.0, 0.0, true).unwrap();
        let axes = t.axes().unwrap();
        // Well up the wall (past the bed's easing): along the wall, 30° out.
        let i = p.points.len() - 200;
        assert!(
            (lean_of(axes[i]) - 30.0).abs() < 1.5,
            "{}",
            lean_of(axes[i])
        );
        let radial = [p.points[i][0], p.points[i][1]];
        assert!(
            axes[i][0] * radial[0] + axes[i][1] * radial[1] > 0.0,
            "leans out"
        );
        // The bead: the full layer along the wall (2.5 mm), not the squeezed 2.5·cos 30°.
        let h = t.bead_of(i).1;
        assert!((h - 2.5).abs() < 0.1, "{h}");
    }

    #[test]
    fn held_to_the_most_and_upright_on_the_bed() {
        let p = cone(60.0, 12);
        let t = tilt(&p, 45.0, 20.0, 10.0, true).unwrap();
        let axes = t.axes().unwrap();
        for (i, v) in axes.iter().enumerate() {
            let z = p.points[i][2] - p.points[1][2];
            let most = 45.0 * (z / 10.0).clamp(0.0, 1.0);
            assert!(
                lean_of(*v) <= most + 1e-6,
                "point {i}: {} > {most}",
                lean_of(*v)
            );
        }
        let i = p.points.len() - 200;
        assert!((lean_of(axes[i]) - 45.0).abs() < 1e-6);
        // The bead along a nozzle 15° short of the wall: h/cos 60° × cos 15° = h_wall × cos 15°.
        let h = t.bead_of(i).1;
        assert!((h - 2.5 * 15f64.to_radians().cos()).abs() < 0.15, "{h}");
    }

    #[test]
    fn keeps_a_full_bead_when_asked() {
        let p = cone(30.0, 12);
        let t = tilt(&p, 45.0, 20.0, 0.0, false).unwrap();
        assert!(t.axes().is_some());
        assert_eq!(t.height, p.height);
    }

    #[test]
    fn a_recipe_tilts_its_toolpath() {
        let recipe = "easycad 0.1\nc = circle(r: 40)\np = vase_spiral(extrude(c, 30), layer: 2, width: 4, speed: 10)\nprint tilt(p, max: 30 deg, bead: false)\n";
        let e = crate::eval::evaluate(recipe, crate::eval::Quality::Final);
        assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
        let path = &e.prints[0].path;
        assert!(path.axes().is_some());
        // An upright wall: the nozzle stays (almost) upright.
        assert!(
            e.prints[0].stats.max_tilt_deg < 2.0,
            "{}",
            e.prints[0].stats.max_tilt_deg
        );
    }

    #[test]
    fn refuses_nonsense() {
        let p = cone(10.0, 2);
        assert!(tilt(&p, 90.0, 20.0, 0.0, true).is_err());
        assert!(tilt(&p, f64::NAN, 20.0, 0.0, true).is_err());
        assert!(tilt(&p, 45.0, -1.0, 0.0, true).is_err());
    }
}
