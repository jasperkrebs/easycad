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
}
