//! Spirals from the centre out whose spacing can vary over space, and interlocking wiggles that
//! close the extra gaps between their turns.
//!
//! Ported from EasyGCode's spiral disc (`spiralDiscSpec` in its gcode.js) and made general: the
//! spacing is any field, so the attractor, band and fades are written in the recipe, and the
//! wiggles work on any spiral. The numerics follow EasyGCode step by step (RK4 with 720 steps
//! per turn, snapping to the nearest whole turn, chord-tolerance sampling, and a least-squares
//! amplitude solve per angle), so a recipe can reproduce its spiral disc G-code.

use crate::curve::{Curve, Winding};
use crate::field::Field;
use crate::geom::P3;
use std::f64::consts::TAU;

/// The most turns a spiral may make.
pub const MAX_TURNS: usize = 2000;
/// Integration steps per turn (as EasyGCode).
const SUB: usize = 720;

/// A flat spiral (in the XY plane, around the origin) from the centre out to about `outer`.
/// Each turn sits one local `spacing` outside the one before (dr/dθ = spacing / 2π), and the
/// spiral ends on the whole number of turns whose end is nearest `outer`. Points are spaced
/// evenly in angle: enough per turn to keep within `tolerance` of the true curve, at least 32,
/// and at least `min_samples`.
pub fn spiral(
    outer: f64,
    spacing: &Field,
    tolerance: f64,
    min_samples: usize,
    max_points: usize,
) -> Result<Curve, String> {
    if !outer.is_finite() || outer <= 0.0 {
        return Err(format!("`outer` must be more than 0, got {outer}"));
    }
    if !tolerance.is_finite() || !(0.001..=10.0).contains(&tolerance) {
        return Err(format!(
            "`tolerance` must be between 0.001 and 10 mm, got {tolerance}"
        ));
    }
    let h = TAU / SUB as f64;
    let drdt = |th: f64, r: f64| -> Result<f64, String> {
        let s = spacing.at([r * th.cos(), r * th.sin(), 0.0]);
        if !(s.is_finite() && s > 0.0) {
            return Err(format!(
                "the spacing must be more than 0, but it is {s} at r = {r:.2} mm"
            ));
        }
        Ok(s / TAU)
    };

    // Integrate r(θ) with RK4, turn by turn, until a turn ends past `outer`.
    let mut rs: Vec<f64> = vec![0.0];
    let mut turn_r: Vec<f64> = vec![0.0];
    let mut n = 0;
    loop {
        if n >= MAX_TURNS {
            return Err(format!(
                "the spiral would need more than {MAX_TURNS} turns to reach r = {outer} mm; use a larger spacing"
            ));
        }
        for i in 0..SUB {
            let r = rs.last().copied().unwrap_or(0.0);
            let th = h * (n * SUB + i) as f64;
            let k1 = drdt(th, r)?;
            let k2 = drdt(th + h / 2.0, r + (h / 2.0) * k1)?;
            let k3 = drdt(th + h / 2.0, r + (h / 2.0) * k2)?;
            let k4 = drdt(th + h, r + h * k3)?;
            rs.push(r + (h / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4));
        }
        n += 1;
        let end = rs.last().copied().unwrap_or(0.0);
        turn_r.push(end);
        if end >= outer {
            break;
        }
    }
    // Snap: the whole number of turns ending closest to the requested radius.
    let at = |k: usize| turn_r.get(k).copied().unwrap_or(0.0);
    let mut turns = n;
    if n > 1 && (at(n - 1) - outer).abs() <= (at(n) - outer).abs() {
        turns = n - 1;
    }
    let turns = turns.max(1);
    let th_end = TAU * turns as f64;
    let last = turns * SUB;
    let r_at = |th: f64| -> f64 {
        let x = (th / h).clamp(0.0, last as f64);
        let i = (x.floor() as usize).min(last - 1);
        let (a, b) = (
            rs.get(i).copied().unwrap_or(0.0),
            rs.get(i + 1).copied().unwrap_or(0.0),
        );
        a + (b - a) * (x - i as f64)
    };

    let r_end = at(turns);
    let start_spacing = spacing.at([0.0; 3]);
    let mut dth = 2.0
        * (1.0 - tolerance / r_end.max(start_spacing))
            .max(-1.0)
            .acos();
    if !dth.is_finite() || dth <= 0.0 {
        dth = 0.2;
    }
    let per_turn = 32.max((TAU / dth).ceil() as usize).max(min_samples);
    let total = turns.saturating_mul(per_turn);
    if total.saturating_add(1) > max_points {
        return Err(format!(
            "this spiral needs too many points ({turns} turns × {per_turn}); use a larger spacing or tolerance"
        ));
    }
    let mut points: Vec<P3> = Vec::with_capacity(total + 1);
    let mut turn: Vec<f64> = Vec::with_capacity(total + 1);
    for i in 0..=total {
        let th = (i as f64 / total as f64) * th_end;
        let r = r_at(th);
        points.push([r * th.cos(), r * th.sin(), 0.0]);
        turn.push((i as f64 / total as f64) * turns as f64);
    }
    Ok(Curve {
        winding: Some(Winding {
            turn,
            per_turn,
            center: [0.0; 2],
        }),
        ..Curve::new(points, false)
    })
}

/// The spiral stretched, every radius by the same factor, so that it ends exactly `outer` from
/// its centre: a spiral snapped to whole turns made to run straight into a wall that starts
/// there (each turn moves out by at most about half a spacing over `outer`).
pub fn end_exactly_at(curve: Curve, outer: f64) -> Curve {
    let c = curve.winding.as_ref().map_or([0.0; 2], |w| w.center);
    let end = curve
        .points
        .last()
        .map_or(0.0, |p| (p[0] - c[0]).hypot(p[1] - c[1]));
    if !(end > 0.0 && outer > 0.0) {
        return curve;
    }
    let k = outer / end;
    let points = curve
        .points
        .iter()
        .map(|p| [c[0] + (p[0] - c[0]) * k, c[1] + (p[1] - c[1]) * k, p[2]])
        .collect();
    Curve { points, ..curve }
}

/// A flat spiral (in the XY plane, around the origin) of exactly `turns` turns (any fraction
/// too) from radius `start`, each turn one local `spacing` outside the one before. With a
/// constant spacing it's the exact Archimedean spiral r = start + θ/2π · spacing, sampled as
/// EasyGCode samples its spoon (`spoonPath` in its geometry.js): points evenly spaced in angle
/// over the whole spiral, as many as keep within `tolerance` of the true curve at its outer
/// end, at least 8 (and at least `min_samples` per turn). A field spacing is integrated like
/// [`spiral`]'s.
pub fn spiral_turns(
    start: f64,
    turns: f64,
    spacing: &Field,
    constant: Option<f64>,
    tolerance: f64,
    min_samples: usize,
    max_points: usize,
) -> Result<Curve, String> {
    if !start.is_finite() || start < 0.0 {
        return Err(format!("`start` must be 0 or more, got {start}"));
    }
    if !turns.is_finite() || turns <= 0.0 || turns > MAX_TURNS as f64 {
        return Err(format!(
            "`turns` must be more than 0 and at most {MAX_TURNS}, got {turns}"
        ));
    }
    if !tolerance.is_finite() || !(0.001..=10.0).contains(&tolerance) {
        return Err(format!(
            "`tolerance` must be between 0.001 and 10 mm, got {tolerance}"
        ));
    }
    let total = turns * TAU;
    // r(θ) at any angle, and the radius the spiral ends at.
    let r_at: Box<dyn Fn(f64) -> f64> = match constant {
        Some(s) => {
            if !(s.is_finite() && s > 0.0) {
                return Err(format!("`spacing` must be more than 0, got {s}"));
            }
            // As EasyGCode writes it, so the numbers match to the last digit.
            Box::new(move |th: f64| start + (th / TAU) * s)
        }
        None => {
            let n = ((turns * SUB as f64).ceil() as usize).max(1);
            let h = total / n as f64;
            let drdt = |th: f64, r: f64| -> Result<f64, String> {
                let s = spacing.at([r * th.cos(), r * th.sin(), 0.0]);
                if !(s.is_finite() && s > 0.0) {
                    return Err(format!(
                        "the spacing must be more than 0, but it is {s} at r = {r:.2} mm"
                    ));
                }
                Ok(s / TAU)
            };
            let mut rs = Vec::with_capacity(n + 1);
            rs.push(start);
            for i in 0..n {
                let r = rs.last().copied().unwrap_or(start);
                let th = h * i as f64;
                let k1 = drdt(th, r)?;
                let k2 = drdt(th + h / 2.0, r + (h / 2.0) * k1)?;
                let k3 = drdt(th + h / 2.0, r + (h / 2.0) * k2)?;
                let k4 = drdt(th + h, r + h * k3)?;
                rs.push(r + (h / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4));
            }
            Box::new(move |th: f64| {
                let x = (th / h).clamp(0.0, n as f64);
                let i = (x.floor() as usize).min(n - 1);
                let (a, b) = (
                    rs.get(i).copied().unwrap_or(0.0),
                    rs.get(i + 1).copied().unwrap_or(0.0),
                );
                a + (b - a) * (x - i as f64)
            })
        }
    };
    let r_end = match constant {
        Some(s) => start + turns * s,
        None => r_at(total),
    };
    let start_spacing = constant.unwrap_or_else(|| spacing.at([start, 0.0, 0.0]));
    let mut dth = 2.0
        * (1.0 - tolerance / r_end.max(start_spacing))
            .max(-1.0)
            .acos();
    if !dth.is_finite() || dth <= 0.0 {
        dth = 0.2;
    }
    let steps = 8
        .max((total / dth).ceil() as usize)
        .max((min_samples as f64 * turns).ceil() as usize);
    if steps.saturating_add(1) > max_points {
        return Err(format!(
            "this spiral needs too many points ({steps}); use fewer turns or a larger tolerance"
        ));
    }
    let mut points: Vec<P3> = Vec::with_capacity(steps + 1);
    let mut turn: Vec<f64> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let th = (i as f64 / steps as f64) * total;
        let r = r_at(th);
        points.push([r * th.cos(), r * th.sin(), 0.0]);
        turn.push((i as f64 / steps as f64) * turns);
    }
    Ok(Curve {
        winding: Some(Winding {
            turn,
            // Only exact when the turns divide the points evenly; `interlock` checks.
            per_turn: ((steps as f64 / turns).round() as usize).max(1),
            center: [0.0; 2],
        }),
        ..Curve::new(points, false)
    })
}

/// What `interlock` did, for messages.
#[derive(Debug, Clone, PartialEq)]
pub struct Interlock {
    /// Bumps per turn actually used (always odd).
    pub bumps: usize,
    pub max_amplitude: f64,
    /// How far the closest meeting points of neighbouring turns come inside one line width
    /// (`overlap`) or stay outside it (`gap`), over the turns that spread apart.
    pub overlap: f64,
    pub gap: f64,
}

/// Wiggles a spiral's turns in and out, `A · sin(B·θ/2)`, so that where turns are spread more
/// than `width` apart, each turn's outward peak meets the next turn's inward dip with exactly
/// one line width between their centrelines. `B` (bumps per turn, each outward and each inward
/// bump counted) must be odd so a peak always faces a dip; an even count is rounded up.
///
/// At each angle the turns form a chain where `A_k + A_(k+1)` must equal the extra gap `g_k`;
/// that is solved in the least-squares sense (a tridiagonal Thomas solve per angle, with a hair
/// of damping), holding turns whose neighbouring gaps are all closed at zero. Exactly as in
/// EasyGCode's spiral disc.
pub fn interlock(curve: &Curve, bumps: usize, width: f64) -> Result<(Curve, Interlock), String> {
    let w = curve
        .winding
        .as_ref()
        .ok_or("needs a spiral (a curve made of turns, like `spiral(…)`)".to_string())?;
    if !(width.is_finite() && width > 0.0) {
        return Err(format!("`width` must be more than 0, got {width}"));
    }
    let spt = w.per_turn;
    let total = curve.points.len().saturating_sub(1);
    if spt == 0 || total == 0 || total % spt != 0 || w.turn.len() != curve.points.len() {
        return Err("needs a spiral whose points are spaced evenly in turns".into());
    }
    let b = if bumps % 2 == 1 { bumps } else { bumps + 1 };
    let c = w.center;
    let radius = |p: P3| (p[0] - c[0]).hypot(p[1] - c[1]);
    let radii: Vec<f64> = curve.points.iter().map(|p| radius(*p)).collect();
    let r = |i: usize| radii.get(i).copied().unwrap_or(0.0);

    let mut amps = vec![0.0; total + 1];
    for j in 0..spt {
        let idx: Vec<usize> = (j..=total).step_by(spt).collect();
        let k_count = idx.len();
        if k_count < 2 {
            continue;
        }
        let g: Vec<f64> = (0..k_count - 1)
            .map(|k| (r(idx[k + 1]) - r(idx[k]) - width).max(0.0))
            .collect();
        let gap = |k: usize| g.get(k).copied().unwrap_or(0.0);
        let eps = width * 1e-4;
        let free: Vec<usize> = (0..k_count)
            .filter(|&k| (k > 0 && gap(k - 1) > eps) || (k < k_count - 1 && gap(k) > eps))
            .collect();
        if free.is_empty() {
            continue;
        }
        let mut is_free = vec![false; k_count];
        for &k in &free {
            is_free[k] = true;
        }
        let m = free.len();
        let (mut dg, mut off, mut rhs) = (vec![0.0; m], vec![0.0; m], vec![0.0; m]);
        for (n2, &k) in free.iter().enumerate() {
            dg[n2] = 1e-6;
            if k > 0 {
                dg[n2] += 1.0;
                rhs[n2] += gap(k - 1);
            }
            if k < k_count - 1 {
                dg[n2] += 1.0;
                rhs[n2] += gap(k);
                if is_free.get(k + 1).copied().unwrap_or(false) {
                    off[n2] = 1.0;
                }
            }
        }
        // Thomas algorithm on the symmetric tridiagonal (dg, off).
        let (mut cp, mut dp) = (vec![0.0; m], vec![0.0; m]);
        cp[0] = off[0] / dg[0];
        dp[0] = rhs[0] / dg[0];
        for n2 in 1..m {
            let den = dg[n2] - off[n2 - 1] * cp[n2 - 1];
            cp[n2] = off[n2] / den;
            dp[n2] = (rhs[n2] - off[n2 - 1] * dp[n2 - 1]) / den;
        }
        let mut sol = vec![0.0; m];
        sol[m - 1] = dp[m - 1];
        for n2 in (0..m - 1).rev() {
            sol[n2] = dp[n2] - cp[n2] * sol[n2 + 1];
        }
        for (n2, &k) in free.iter().enumerate() {
            if let Some(a) = amps.get_mut(idx[k]) {
                *a = sol[n2].max(0.0);
            }
        }
    }

    let mut points = Vec::with_capacity(total + 1);
    let mut max_amplitude: f64 = 0.0;
    for (i, p) in curve.points.iter().enumerate() {
        let a = amps.get(i).copied().unwrap_or(0.0);
        max_amplitude = max_amplitude.max(a);
        let th = w.turn.get(i).copied().unwrap_or(0.0) * TAU;
        let ri = r(i);
        let rr = (ri + a * ((b as f64 * th) / 2.0).sin()).max(0.0);
        // Turns that don't wiggle keep their exact points.
        points.push(if a > 0.0 && ri > 0.0 {
            [
                c[0] + (p[0] - c[0]) / ri * rr,
                c[1] + (p[1] - c[1]) / ri * rr,
                p[2],
            ]
        } else {
            *p
        });
    }
    // How well the wiggles meet: at each angle, the closest centreline distance from a turn to
    // the next one, against one line width, over the turns that spread apart.
    let (mut overlap, mut gap) = (0.0f64, 0.0f64);
    let turns = total / spt;
    for k in 0..turns.saturating_sub(1) {
        let (mut mn, mut mx) = (f64::INFINITY, f64::NEG_INFINITY);
        for j in 0..spt {
            let i = k * spt + j;
            if let (Some(a), Some(b)) = (points.get(i), points.get(i + spt)) {
                let d = radius(*b) - radius(*a);
                mn = mn.min(d);
                mx = mx.max(d);
            }
        }
        if mx > width * 1.02 {
            overlap = overlap.max(width - mn);
            gap = gap.max(mn - width);
        }
    }
    let info = Interlock {
        bumps: b,
        max_amplitude,
        overlap: overlap.max(0.0),
        gap: gap.max(0.0),
    };
    Ok((
        Curve {
            points,
            circle: None,
            ..curve.clone()
        },
        info,
    ))
}

/// A C¹ ramp from 0: 0 up to 0, `x` from `soft` on, and a cubic between that leaves 0 level and
/// meets the line `x` with its slope, so wiggles grow from nothing without a corner (leaving the
/// turns at most 0.15 × `soft` short of meeting while they do).
fn soft_ramp(x: f64, soft: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else if x >= soft {
        x
    } else {
        let t = x / soft;
        x * t * (2.0 - t)
    }
}

/// `interlock`, smoothed: the wiggles change evenly all along the spiral, so they ease in where
/// the turns start to spread and out where they close again, and run on through the seam
/// without a step. `touch` is how far apart neighbouring turns are where their bumps meet, as a
/// fraction of `width` (1: one line width; 0.5: half, the beads pushed into each other); it eases
/// in with the spread, so turns that touch stay straight.
///
/// Turn k's amplitude `A` at each point is solved from the gap to the turn inside (`g_in`) and
/// outside (`g_out`), the point one turn back and on: `A_k + A_(k+1) = g` wherever a peak meets a
/// dip. Starting from `(g_in + g_out) / 4` and correcting by the same rule a few times converges on
/// that where the gaps change gradually, without the alternating big/small solutions the system
/// also allows, and every step only looks one turn in and out along the spiral itself, so no angle
/// is solved apart from its neighbours (the per-angle solve jumps at the seam: the chain of turns at
/// angle 0 has one turn more, the spiral's start, than the chain just before it).
pub fn interlock_smooth(
    curve: &Curve,
    bumps: usize,
    width: f64,
    touch: f64,
) -> Result<(Curve, Interlock), String> {
    let w = curve
        .winding
        .as_ref()
        .ok_or("needs a spiral (a curve made of turns, like `spiral(…)`)".to_string())?;
    if !(width.is_finite() && width > 0.0) {
        return Err(format!("`width` must be more than 0, got {width}"));
    }
    if !(touch.is_finite() && touch > 0.0 && touch <= 2.0) {
        return Err(format!(
            "`touch` must be more than 0 and at most 2 (200 %), got {touch}"
        ));
    }
    let spt = w.per_turn;
    let total = curve.points.len().saturating_sub(1);
    if spt == 0 || total == 0 || total % spt != 0 || w.turn.len() != curve.points.len() {
        return Err("needs a spiral whose points are spaced evenly in turns".into());
    }
    let b = if bumps % 2 == 1 { bumps } else { bumps + 1 };
    let c = w.center;
    let radius = |p: P3| (p[0] - c[0]).hypot(p[1] - c[1]);
    let radii: Vec<f64> = curve.points.iter().map(|p| radius(*p)).collect();
    let r = |i: usize| radii.get(i).copied().unwrap_or(0.0);
    let n = total + 1;
    if total < spt {
        return Ok((
            curve.clone(),
            Interlock {
                bumps: b,
                max_amplitude: 0.0,
                overlap: 0.0,
                gap: 0.0,
            },
        ));
    }
    // The gap a point and its neighbour one turn out (or in) should close: past one line width,
    // eased in, plus the extra overlap `touch` asks for, in full once the turns are clearly apart
    // (0.3 line widths) and eased in before that. Inside the first
    // turn the neighbour is the spiral's start, outside the last its end, so the gaps run on
    // through the seam (and close towards the outer end, where a wall takes over).
    let soft = 0.3 * width;
    let close = |inner: f64, outer: f64| {
        let x = outer - inner - width;
        let spread = (x / soft).clamp(0.0, 1.0);
        let ease = spread * spread * (3.0 - 2.0 * spread);
        soft_ramp(x, soft) + (1.0 - touch) * width * ease
    };
    let (r_start, r_end) = (r(0), r(total));
    let g_out: Vec<f64> = (0..n)
        .map(|i| close(r(i), if i + spt <= total { r(i + spt) } else { r_end }))
        .collect();
    let g_in: Vec<f64> = (0..n)
        .map(|i| close(if i >= spt { r(i - spt) } else { r_start }, r(i)))
        .collect();
    let at = |v: &Vec<f64>, i: usize| v.get(i).copied().unwrap_or(0.0);
    // Each turn takes half of each gap beside it (half of a half, as both turns of a pair share it).
    let mut amps: Vec<f64> = (0..n)
        .map(|i| (at(&g_in, i) + at(&g_out, i)) / 4.0)
        .collect();
    // That first guess is exact where the gaps hold from turn to turn and misses a little where
    // they change fast. Correct it a few times by the same rule, only where the turns spread
    // (closed turns stay straight). Solving each angle's chain exactly instead brings back the
    // alternating big/small amplitudes (with the spiral's start and end both held, a chain has one
    // more gap than turns), and more passes creep towards them too.
    let open: Vec<f64> = (0..n)
        .map(|i| {
            let t = ((at(&g_in, i) + at(&g_out, i)) / (0.2 * width)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect();
    for _ in 0..4 {
        // The neighbour one turn in or out, or the spiral's start or end where there is none
        // (the same points the gaps were measured to).
        let miss = |i: usize, j: usize, g: f64| g - at(&amps, i) - at(&amps, j);
        let fix: Vec<f64> = (0..n)
            .map(|i| {
                let outer = miss(
                    i,
                    if i + spt <= total { i + spt } else { total },
                    at(&g_out, i),
                );
                let inner = miss(i, i.saturating_sub(spt), at(&g_in, i));
                at(&open, i) * (inner + outer) / 4.0
            })
            .collect();
        for (a, f) in amps.iter_mut().zip(fix) {
            *a += f;
        }
    }
    let mut points = Vec::with_capacity(n);
    let mut max_amplitude: f64 = 0.0;
    for (i, p) in curve.points.iter().enumerate() {
        let a = at(&amps, i);
        max_amplitude = max_amplitude.max(a.abs());
        let th = w.turn.get(i).copied().unwrap_or(0.0) * TAU;
        let ri = r(i);
        let rr = (ri + a * ((b as f64 * th) / 2.0).sin()).max(0.0);
        // Turns that don't wiggle keep their exact points.
        points.push(if a.abs() > width * 1e-9 && ri > 0.0 {
            [
                c[0] + (p[0] - c[0]) / ri * rr,
                c[1] + (p[1] - c[1]) / ri * rr,
                p[2],
            ]
        } else {
            *p
        });
    }
    // How well the wiggles meet: at each angle, the closest centreline distance from a turn to the
    // next one, against where it should be, over the turns that spread apart.
    let (mut overlap, mut gap_left) = (0.0f64, 0.0f64);
    let turns = total / spt;
    for k in 0..turns.saturating_sub(1) {
        let (mut mn, mut mx, mut aim) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY);
        for j in 0..spt {
            let i = k * spt + j;
            if let (Some(a), Some(b)) = (points.get(i), points.get(i + spt)) {
                let d = radius(*b) - radius(*a);
                mn = mn.min(d);
                mx = mx.max(d);
                aim = aim.min(r(i + spt) - r(i) - at(&g_out, i));
            }
        }
        if mx > width * 1.02 {
            overlap = overlap.max(aim - mn);
            gap_left = gap_left.max(mn - aim);
        }
    }
    let info = Interlock {
        bumps: b,
        max_amplitude,
        overlap: overlap.max(0.0),
        gap: gap_left.max(0.0),
    };
    Ok((
        Curve {
            points,
            circle: None,
            ..curve.clone()
        },
        info,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::fade;
    use easycad_lang::BinOp;
    use std::rc::Rc;

    fn rc(f: Field) -> Rc<Field> {
        Rc::new(f)
    }

    /// EasyGCode's spiral disc spacing: `line * (1 + (open - 1) * fade(distance to the ring))`.
    fn disc_spacing(
        line: f64,
        ring: f64,
        open: f64,
        fade_in: f64,
        fade_out: f64,
        band: f64,
    ) -> Field {
        let d = rc(Field::distance(&Curve::circle(ring, 8)));
        let k = |v: f64| rc(Field::constant(v));
        let near = rc(Field::map(fade, vec![d, k(fade_in), k(fade_out), k(band)]).unwrap());
        let grow = rc(Field::binary(BinOp::Mul, k(open - 1.0), near).unwrap());
        let mult = rc(Field::binary(BinOp::Add, k(1.0), grow).unwrap());
        Field::binary(BinOp::Mul, k(line), mult).unwrap()
    }

    #[test]
    fn an_exact_spiral_ends_right_on_its_radius() {
        let radius = |p: &P3| p[0].hypot(p[1]);
        // 10.4 snaps to 10 whole turns; stretched, every turn moves out alike to end on 10.4.
        let s = spiral(10.4, &Field::constant(1.0), 0.05, 0, 1_000_000).unwrap();
        assert!((radius(s.points.last().unwrap()) - 10.0).abs() < 1e-6);
        let e = end_exactly_at(s.clone(), 10.4);
        let end = e.points.last().unwrap();
        assert!(
            (radius(end) - 10.4).abs() < 1e-9 && end[1].abs() < 1e-9,
            "on +X: {end:?}"
        );
        assert_eq!(e.winding, s.winding, "still a spiral of the same turns");
        let w = e.winding.as_ref().unwrap();
        let (a, b) = (e.points[3 * w.per_turn], e.points[4 * w.per_turn]);
        assert!(
            (radius(&b) - radius(&a) - 1.04).abs() < 1e-6,
            "turns 1.04 apart"
        );
        // Nothing to stretch from the centre.
        let dot = Curve::new(vec![[0.0; 3]], false);
        assert_eq!(end_exactly_at(dot.clone(), 5.0).points, dot.points);
    }

    #[test]
    fn even_spacing_is_an_archimedean_spiral() {
        let s = spiral(10.0, &Field::constant(1.0), 0.05, 0, 1_000_000).unwrap();
        assert_eq!(s.turns(), 10.0);
        let w = s.winding.as_ref().unwrap();
        // Each turn is one spacing further out.
        let end = s.points.last().unwrap();
        assert!((end[0] - 10.0).abs() < 1e-9 && end[1].abs() < 1e-9);
        let quarter = s.points[w.per_turn * 3 + w.per_turn / 4];
        assert!((quarter[0].hypot(quarter[1]) - 3.25).abs() < 1e-6);
    }

    #[test]
    fn the_end_snaps_to_the_nearest_whole_turn() {
        assert_eq!(
            spiral(10.4, &Field::constant(1.0), 0.05, 0, 1_000_000)
                .unwrap()
                .turns(),
            10.0
        );
        assert_eq!(
            spiral(10.6, &Field::constant(1.0), 0.05, 0, 1_000_000)
                .unwrap()
                .turns(),
            11.0
        );
        assert_eq!(
            spiral(0.1, &Field::constant(1.0), 0.05, 0, 1_000_000)
                .unwrap()
                .turns(),
            1.0
        );
    }

    #[test]
    fn bad_spirals_are_errors() {
        assert!(spiral(0.0, &Field::constant(1.0), 0.05, 0, 1_000_000).is_err());
        assert!(
            spiral(10.0, &Field::constant(0.0), 0.05, 0, 1_000_000)
                .unwrap_err()
                .contains("spacing")
        );
        assert!(spiral(10.0, &Field::constant(1.0), 0.0, 0, 1_000_000).is_err());
        assert!(
            spiral(1e6, &Field::constant(0.1), 0.05, 0, 1_000_000)
                .unwrap_err()
                .contains("turns")
        );
        assert!(
            spiral(100.0, &Field::constant(1.0), 0.05, 0, 1000)
                .unwrap_err()
                .contains("too many points")
        );
    }

    #[test]
    fn spacing_opens_up_around_the_ring() {
        let s = spiral(
            60.0,
            &disc_spacing(2.0, 35.0, 2.0, 15.0, 15.0, 0.0),
            0.05,
            336,
            1_000_000,
        )
        .unwrap();
        // EasyGCode's defaults give 26 turns ending at r = 60.823 (its fixture header).
        assert_eq!(s.turns(), 26.0);
        let end = s.points.last().unwrap();
        assert!((end[0].hypot(end[1]) - 60.823).abs() < 5e-4, "{end:?}");
        assert_eq!(s.winding.as_ref().unwrap().per_turn, 336);
    }

    #[test]
    fn interlocking_closes_the_gaps_like_easygcode() {
        let s = spiral(
            60.0,
            &disc_spacing(2.0, 35.0, 2.0, 15.0, 15.0, 0.0),
            0.05,
            21 * 16,
            1_000_000,
        )
        .unwrap();
        let (w, info) = interlock(&s, 21, 2.0).unwrap();
        // EasyGCode's fixture header: max amplitude 1.034 mm, peaks meet dips within
        // −0.005/+0.000 mm of one line width.
        assert!((info.max_amplitude - 1.034).abs() < 5e-4, "{info:?}");
        assert!(
            (info.overlap - 0.005).abs() < 5e-4 && info.gap < 5e-4,
            "{info:?}"
        );
        assert_eq!(w.points.len(), s.points.len());
        // Solid parts don't wiggle: the first turns are untouched.
        assert!(w.points[..336].iter().zip(&s.points).all(|(a, b)| a == b));
        let (_, even) = interlock(&s, 20, 2.0).unwrap();
        assert_eq!(even.bumps, 21);
        assert!(interlock(&Curve::circle(5.0, 8), 21, 2.0).is_err());
    }

    /// The sharpest kink in a curve: how far a point's turning departs from its neighbours'
    /// (degrees). A smooth wiggle changes its turning gradually.
    fn sharpest_kink(c: &Curve) -> f64 {
        let turning: Vec<f64> = c
            .points
            .windows(3)
            .map(|w| {
                let (u, v) = (
                    [w[1][0] - w[0][0], w[1][1] - w[0][1]],
                    [w[2][0] - w[1][0], w[2][1] - w[1][1]],
                );
                (u[0] * v[1] - u[1] * v[0])
                    .atan2(u[0] * v[0] + u[1] * v[1])
                    .to_degrees()
            })
            .collect();
        turning
            .windows(3)
            .map(|t| (t[1] - (t[0] + t[2]) / 2.0).abs())
            .fold(0.0, f64::max)
    }

    /// The shaker lid's spiral disc: 3 mm lines spreading to 157 % around a ring 12 mm out, so
    /// the turns near the centre spread too.
    fn lid_disc(ring: f64) -> Curve {
        spiral(
            32.0,
            &disc_spacing(3.0, ring, 1.57, 9.4, 5.0, 8.4),
            0.05,
            15 * 16,
            1_000_000,
        )
        .unwrap()
    }

    #[test]
    fn smooth_wiggles_have_no_kink_at_the_seam_or_where_they_start() {
        let s = lid_disc(12.0);
        // EasyGCode's per-angle solve: the chain of turns at angle 0 has one turn more (the
        // spiral's start) than the chain just before it, and when the turns near the centre
        // spread the two solve differently: every turn steps at the seam.
        let (old, _) = interlock(&s, 15, 3.0).unwrap();
        assert!(sharpest_kink(&old) > 5.0, "{}", sharpest_kink(&old));
        let (new, info) = interlock_smooth(&s, 15, 3.0, 1.0).unwrap();
        assert!(sharpest_kink(&new) < 1.5, "{}", sharpest_kink(&new));
        assert!(info.max_amplitude > 0.5, "{info:?}");
        // Turns that touch stay exactly as they were: with the ring further out, the centre.
        let far = lid_disc(20.0);
        let (w, _) = interlock_smooth(&far, 15, 3.0, 1.0).unwrap();
        let moved = w.points[..2 * 240]
            .iter()
            .zip(&far.points)
            .map(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]))
            .fold(0.0, f64::max);
        assert!(moved < 0.01, "{moved}");
        assert!(sharpest_kink(&w) < 1.5, "{}", sharpest_kink(&w));
    }

    #[test]
    fn smooth_wiggles_meet_one_line_width_apart_or_closer_with_touch() {
        // Spread to 250 % in a 70 mm band around a ring, so several turns in a row open more
        // than a line width.
        let s = spiral(
            110.0,
            &disc_spacing(3.0, 50.0, 2.5, 9.4, 9.4, 70.0),
            0.05,
            15 * 16,
            1_000_000,
        )
        .unwrap();
        let spt = s.winding.as_ref().unwrap().per_turn;
        let radius = |p: &P3| p[0].hypot(p[1]);
        for touch in [1.0, 0.5] {
            let (w, _) = interlock_smooth(&s, 15, 3.0, touch).unwrap();
            // Turns spread at least a line width wider than touching: where their bumps meet,
            // `touch` line widths apart; within 2 % inside the band, where the spread holds two
            // turns either side, and within 10 % next to where it changes fast (a little more
            // overlap there rather than wiggles alternating big and small).
            let turns = (s.points.len() - 1) / spt;
            let spread = |k: usize| {
                (0..spt)
                    .map(|j| radius(&s.points[k * spt + spt + j]) - radius(&s.points[k * spt + j]))
                    .fold(f64::INFINITY, f64::min)
            };
            let (mut inside, mut edge) = (0, 0);
            for k in 2..turns - 3 {
                if spread(k) < 2.0 * 3.0 {
                    continue;
                }
                let meet = (0..spt)
                    .map(|j| radius(&w.points[k * spt + spt + j]) - radius(&w.points[k * spt + j]))
                    .fold(f64::INFINITY, f64::min);
                let steady = (k - 2..=k + 2).all(|k| spread(k) >= 2.0 * 3.0);
                let off = (meet - touch * 3.0).abs() / 3.0;
                if steady {
                    assert!(off < 0.02, "turn {k}, touch {touch}: {meet}");
                    inside += 1;
                } else {
                    assert!(off < 0.1, "turn {k}, touch {touch}: {meet}");
                    edge += 1;
                }
            }
            assert!(inside >= 1 && edge >= 1, "{inside} {edge}");
        }
        assert!(interlock_smooth(&s, 15, 3.0, 0.0).is_err());
    }
}
