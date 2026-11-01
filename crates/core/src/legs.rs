//! Concentric rings with legs: a disc of rings one line apart whose outer rings run out along
//! straight legs and back (hairpins, with tangent fillets where they leave the ring and a round
//! cap at the tip), linked into one line by a staircase or zipper seam. EasyGCode's bend stool
//! seat, ported point for point from its `Geo.stoolLoop` and `discLoops` (geometry.js, gcode.js)
//! so the G-code matches; useful for anything round with arms (stools, tables, trivets).
//!
//! The optional *spread* widens the gaps between a leg's hairpins near where it bends away from
//! the seat (EasyGCode's bend-zone attractor): within `r1` of an attractor point every point
//! moves outward by its hairpin's share of the spread, easing to nothing at `r2`. Each point's
//! z holds how far it was spread there (0 elsewhere), so a recipe can sink the spread parts of
//! a layer (EasyGCode's overhang drop) with a field of z.

use crate::curve::Curve;
use crate::geom::P3;

/// One ring's legs.
#[derive(Debug, Clone, Copy)]
struct Leg {
    /// Half-width of this ring's hairpin (from the leg's spine to this line).
    d: f64,
    /// Fillet radius where this ring leaves the circle for the leg.
    f: f64,
    /// Distance of the tip cap's centre from the middle.
    tip: f64,
}

/// The spread of one ring.
#[derive(Debug, Clone)]
struct Spread {
    points: Vec<[f64; 2]>,
    r1: f64,
    r2: f64,
    /// Full displacement of this ring inside r1.
    d: f64,
    /// Pull back along the overhang slope (0: none).
    slope: f64,
}

/// What `leg_rings` builds.
#[derive(Debug, Clone)]
pub struct LegRings {
    /// Rings from the centre out. Each starts where the one before it ended (its first point is
    /// the step out to it), except the first.
    pub rings: Vec<Curve>,
    /// The outermost ring as one closed outline (no seam gap): for a brim, or to fit the bed.
    pub outline: Curve,
}

/// The settings, in mm and radians.
#[derive(Debug, Clone)]
pub struct LegSpec {
    /// How many rings.
    pub count: usize,
    /// Line width: ring i's radius is spacing / 2 + i * spacing.
    pub spacing: f64,
    /// Directions of the legs (radians).
    pub angles: Vec<f64>,
    /// How many outer rings make up each leg (hairpin pairs); 0: no legs.
    pub pairs: usize,
    /// Distance of the tip caps' centre from the middle.
    pub tip: f64,
    /// Fillet radius of the outermost ring's junctions (inner ones add a line width each).
    pub fillet: f64,
    /// Zipper seam (every other ring reversed, the seam fixed) instead of a staircase.
    pub zipper: bool,
    /// Spread: attractor distance from the middle, inner and outer radius, the gap added
    /// between hairpins (in line widths), the share of it applied (0..1), and the overhang drop.
    pub spread_at: f64,
    pub r1: f64,
    pub r2: f64,
    pub gap: f64,
    pub amount: f64,
    pub drop: f64,
    pub tolerance: f64,
    pub max_points: usize,
}

const FINE: f64 = 1.2;

/// Points of one loop, with each point's spread kept in z.
struct Loop<'a> {
    pts: Vec<P3>,
    tol: f64,
    r: f64,
    spread: Option<&'a Spread>,
    max: usize,
}

impl Loop<'_> {
    fn push(&mut self, x: f64, y: f64, w: f64) -> Result<(), String> {
        if let Some(last) = self.pts.last()
            && (last[0] - x).abs() < 1e-9
            && (last[1] - y).abs() < 1e-9
        {
            return Ok(());
        }
        if self.pts.len() >= self.max {
            return Err("too many points: raise the tolerance".into());
        }
        self.pts.push([x, y, w]);
        Ok(())
    }

    fn dist_a(s: &Spread, x: f64, y: f64) -> f64 {
        s.points
            .iter()
            .map(|q| (x - q[0]).hypot(y - q[1]))
            .fold(f64::INFINITY, f64::min)
    }

    fn k_at(s: &Spread, x: f64, y: f64) -> f64 {
        let dd = Self::dist_a(s, x, y);
        if dd <= s.r1 {
            return 1.0;
        }
        if dd >= s.r2 {
            return 0.0;
        }
        let t = (dd - s.r1) / (s.r2 - s.r1);
        1.0 - t * t * (3.0 - 2.0 * t)
    }

    fn near(s: &Spread, x: f64, y: f64) -> bool {
        Self::dist_a(s, x, y) < s.r2 + FINE * 2.0
    }

    fn eff(s: &Spread, g: f64) -> f64 {
        s.d * g * (1.0 - g * s.slope)
    }

    fn arc_steps(&self, radius: f64, sweep: f64) -> usize {
        let mut dth = 2.0 * (1.0 - self.tol / radius.max(1e-6)).max(-1.0).acos();
        if !dth.is_finite() || dth <= 0.0 {
            dth = 0.2;
        }
        steps_of(sweep.abs() / dth, 2)
    }

    /// The ring's own circle from a0 to a1; the spread pushes it straight out.
    fn ring_arc(&mut self, a0: f64, a1: f64) -> Result<(), String> {
        let r = self.r;
        let n = self.arc_steps(r, a1 - a0);
        let Some(s) = self.spread else {
            for i in 1..=n {
                let a = a0 + ((a1 - a0) * i as f64) / n as f64;
                self.push(r * a.cos(), r * a.sin(), 0.0)?;
            }
            return Ok(());
        };
        let mut prev = a0;
        for i in 1..=n {
            let a = a0 + ((a1 - a0) * i as f64) / n as f64;
            let fine = Self::near(s, r * prev.cos(), r * prev.sin())
                || Self::near(s, r * a.cos(), r * a.sin());
            let m = if fine {
                steps_of(((a - prev).abs() * r) / FINE, 1)
            } else {
                1
            };
            for j in 1..=m {
                let aa = prev + ((a - prev) * j as f64) / m as f64;
                let k = Self::k_at(s, r * aa.cos(), r * aa.sin());
                let rr = r + Self::eff(s, k);
                self.push(rr * aa.cos(), rr * aa.sin(), s.d * k)?;
            }
            prev = a;
        }
        Ok(())
    }

    /// An arc around (cx, cy). sign +1: the spread grows it (a tip cap); -1: shrinks it toward
    /// its centre, never past it (a fillet).
    fn arc_around(
        &mut self,
        cx: f64,
        cy: f64,
        radius: f64,
        a0: f64,
        a1: f64,
        sign: f64,
    ) -> Result<(), String> {
        if radius <= 1e-9 || (a1 - a0).abs() < 1e-9 {
            return Ok(());
        }
        let n = self.arc_steps(radius, a1 - a0);
        let Some(s) = self.spread else {
            for i in 1..=n {
                let a = a0 + ((a1 - a0) * i as f64) / n as f64;
                self.push(cx + radius * a.cos(), cy + radius * a.sin(), 0.0)?;
            }
            return Ok(());
        };
        let mut prev = a0;
        for i in 1..=n {
            let a = a0 + ((a1 - a0) * i as f64) / n as f64;
            let fine = Self::near(s, cx + radius * prev.cos(), cy + radius * prev.sin())
                || Self::near(s, cx + radius * a.cos(), cy + radius * a.sin());
            let m = if fine {
                steps_of(((a - prev).abs() * radius) / FINE, 1)
            } else {
                1
            };
            for j in 1..=m {
                let aa = prev + ((a - prev) * j as f64) / m as f64;
                let k = Self::k_at(s, cx + radius * aa.cos(), cy + radius * aa.sin());
                let rr = (radius + sign * Self::eff(s, k)).max(0.0);
                self.push(cx + rr * aa.cos(), cy + rr * aa.sin(), s.d * k)?;
            }
            prev = a;
        }
        Ok(())
    }
}

/// ceil(x), at least `min`, as a count (a NaN or huge x gives `min` or a large but finite count).
fn steps_of(x: f64, min: usize) -> usize {
    let c = x.ceil();
    if c.is_finite() && c > min as f64 {
        c.min(1e7) as usize
    } else {
        min
    }
}

/// What every loop of one set of rings shares.
struct Common<'a> {
    tol: f64,
    angles: &'a [f64],
    max: usize,
}

/// One loop of radius `r` from `a_start`, counter-clockwise, `gap` radians short of a full
/// turn, out along each leg and back (EasyGCode's `Geo.stoolLoop`).
fn stool_loop(
    c: &Common,
    r: f64,
    a_start: f64,
    gap: f64,
    leg: Option<Leg>,
    spread: Option<&Spread>,
) -> Result<Vec<P3>, String> {
    let (tol, angles, max) = (c.tol, c.angles, c.max);
    let spread = spread.filter(|s| s.d > 0.0 && !s.points.is_empty());
    let mut lp = Loop {
        pts: Vec::new(),
        tol: if tol > 0.0 { tol } else { 0.05 },
        r,
        spread,
        max,
    };
    let a_end = a_start + 2.0 * std::f64::consts::PI - gap;
    match spread {
        Some(s) => {
            let (sx, sy) = (r * a_start.cos(), r * a_start.sin());
            let k0 = Loop::k_at(s, sx, sy);
            let rr0 = r + Loop::eff(s, k0);
            lp.push(rr0 * a_start.cos(), rr0 * a_start.sin(), s.d * k0)?;
        }
        None => lp.pts.push([r * a_start.cos(), r * a_start.sin(), 0.0]),
    }
    let Some(leg) = leg else {
        lp.ring_arc(a_start, a_end)?;
        return Ok(lp.pts);
    };
    let (d, f, tip) = (leg.d, leg.f.max(0.0), leg.tip);
    let t = ((r + f) * (r + f) - (d + f) * (d + f)).max(0.0).sqrt();
    let beta = (d + f).atan2(t);
    let turn = std::f64::consts::FRAC_PI_2 - beta;
    let tau = 2.0 * std::f64::consts::PI;
    let mut angs: Vec<f64> = angles
        .iter()
        .map(|&p| {
            let mut a = p;
            let mut guard = 0;
            while a <= a_start + 1e-12 && guard < 10_000 {
                a += tau;
                guard += 1;
            }
            a
        })
        .collect();
    angs.sort_by(f64::total_cmp);
    let mut cur = a_start;
    for p in angs {
        if p - beta <= cur + 1e-9 || p + beta >= a_end - 1e-9 {
            continue; // no room
        }
        let u = [p.cos(), p.sin()];
        let v = [-u[1], u[0]];
        let at = |tu: f64, sv: f64| [u[0] * tu + v[0] * sv, u[1] * tu + v[1] * sv];

        // A straight side of the leg from t_from (already there) to t_to, at s_off from the
        // spine; the spread shifts it sideways, finely sampled only where it acts.
        let leg_line = |lp: &mut Loop, t_from: f64, t_to: f64, s_off: f64| -> Result<(), String> {
            let Some(s) = lp.spread else {
                let e = at(t_to, s_off);
                return lp.push(e[0], e[1], 0.0);
            };
            let sgn = if s_off >= 0.0 { 1.0 } else { -1.0 };
            let emit = |lp: &mut Loop, tt: f64| {
                let b = at(tt, s_off);
                let k = Loop::k_at(s, b[0], b[1]);
                let e = Loop::eff(s, k);
                lp.push(b[0] + sgn * v[0] * e, b[1] + sgn * v[1] * e, s.d * k)
            };
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for q in &s.points {
                let tq = q[0] * u[0] + q[1] * u[1];
                let sq = q[0] * v[0] + q[1] * v[1];
                let perp = (sq - s_off).abs();
                let rr = s.r2 + FINE;
                if perp < rr {
                    let half = (rr * rr - perp * perp).sqrt();
                    lo = lo.min(tq - half);
                    hi = hi.max(tq + half);
                }
            }
            lo = lo.max(t_from.min(t_to));
            hi = hi.min(t_from.max(t_to));
            if lo < hi {
                let dir = if t_to >= t_from { 1.0 } else { -1.0 };
                let (ta, tb) = if dir > 0.0 { (lo, hi) } else { (hi, lo) };
                if (ta - t_from) * dir > 1e-9 {
                    emit(lp, ta)?;
                }
                let m = steps_of((tb - ta).abs() / FINE, 1);
                for j in 1..=m {
                    emit(lp, ta + ((tb - ta) * j as f64) / m as f64)?;
                }
                if (t_to - tb) * dir > 1e-9 {
                    emit(lp, t_to)?;
                }
                Ok(())
            } else {
                emit(lp, t_to)
            }
        };

        // A fillet the spread has used up becomes the true offset corner: where the spread
        // circle meets the spread leg side. None while the fillet survives.
        let fillet_corner = |lp: &Loop,
                             fc: [f64; 2],
                             a_a: f64,
                             a_b: f64,
                             s_sign: f64|
         -> Option<(f64, [f64; 3], f64)> {
            let s = lp.spread?;
            let pb = at(t, s_sign * d);
            let kc = Loop::k_at(s, pb[0], pb[1]);
            let dk = Loop::eff(s, kc);
            if f > 1e-9 {
                let m = steps_of(((a_b - a_a).abs() * f) / FINE, 6);
                let mut r_min = f64::INFINITY;
                for i in 0..=m {
                    let aa = a_a + ((a_b - a_a) * i as f64) / m as f64;
                    let rr =
                        f - Loop::eff(s, Loop::k_at(s, fc[0] + f * aa.cos(), fc[1] + f * aa.sin()));
                    r_min = r_min.min(rr);
                }
                if r_min > 0.02 {
                    return None;
                }
            } else if dk <= 1e-9 {
                return None;
            }
            let rr = lp.r + dk;
            let dd2 = d + dk;
            let tc = (rr * rr - dd2 * dd2).max(0.0).sqrt();
            let c = at(tc, s_sign * dd2);
            Some((tc, [c[0], c[1], s.d * kc], dd2.atan2(tc)))
        };

        let f1 = at(t, -(d + f));
        let a1 = (-f1[1]).atan2(-f1[0]);
        let c1 = fillet_corner(&lp, f1, a1, a1 - turn, -1.0);
        let f2 = at(t, d + f);
        let half_pi = std::f64::consts::FRAC_PI_2;
        let c2 = fillet_corner(&lp, f2, p - half_pi, p - half_pi - turn, 1.0);

        lp.ring_arc(cur, p - c1.map_or(beta, |c| c.2))?;
        match c1 {
            Some((tc, c, _)) => {
                lp.push(c[0], c[1], c[2])?;
                leg_line(&mut lp, tc, tip, -d)?;
            }
            None => {
                lp.arc_around(f1[0], f1[1], f, a1, a1 - turn, -1.0)?;
                leg_line(&mut lp, t, tip, -d)?;
            }
        }
        lp.arc_around(u[0] * tip, u[1] * tip, d, p - half_pi, p + half_pi, 1.0)?;
        match c2 {
            Some((tc, c, _)) => {
                leg_line(&mut lp, tip, tc, d)?;
                lp.push(c[0], c[1], c[2])?;
            }
            None => {
                leg_line(&mut lp, tip, t, d)?;
                lp.arc_around(f2[0], f2[1], f, p - half_pi, p - half_pi - turn, -1.0)?;
            }
        }
        cur = p + c2.map_or(beta, |c| c.2);
    }
    lp.ring_arc(cur, a_end)?;
    Ok(lp.pts)
}

/// Builds the rings (EasyGCode's `discLoops`, and its plain staircase rings without legs).
pub fn leg_rings(s: &LegSpec) -> Result<LegRings, String> {
    let n = s.count;
    let lw = s.spacing;
    if n == 0 {
        return Err("needs at least one ring".into());
    }
    if lw.is_nan() || lw <= 0.0 || !lw.is_finite() {
        return Err("`spacing` must be more than 0".into());
    }
    if s.tolerance.is_nan() || s.tolerance <= 0.0 || !s.tolerance.is_finite() {
        return Err("`tolerance` must be more than 0".into());
    }
    let tol = s.tolerance;
    let radii: Vec<f64> = (0..n).map(|i| lw / 2.0 + i as f64 * lw).collect();
    let pi = std::f64::consts::PI;
    let legs = s.pairs > 0;
    if s.pairs >= n {
        return Err(format!(
            "{} rings can make legs of at most {} rings (one must stay round)",
            n,
            n.saturating_sub(1)
        ));
    }
    let m = s.pairs;
    let leg_for = |i: usize| -> Option<Leg> {
        if !legs || i < n - m {
            return None;
        }
        let h = (n - 1 - i) as f64;
        Some(Leg {
            d: (m as f64 - h) * lw - lw / 2.0,
            f: s.fillet + h * lw,
            tip: s.tip,
        })
    };
    if legs {
        let r0 = radii[n - 1];
        let (d0, f0) = (m as f64 * lw - lw / 2.0, s.fillet.max(0.0));
        let t0 = ((r0 + f0) * (r0 + f0) - (d0 + f0) * (d0 + f0))
            .max(0.0)
            .sqrt();
        if s.tip <= t0 + 1e-6 {
            return Err("the legs are too short for their width and fillet".into());
        }
        let spacing_between = if s.angles.len() > 1 {
            2.0 * pi / s.angles.len() as f64
        } else {
            2.0 * pi
        };
        if (d0 + f0).atan2(t0) > spacing_between / 2.0 - 0.02 {
            return Err(
                "the legs are too wide (or their fillets too big) for how close together they are"
                    .into(),
            );
        }
    }
    let spread_on = legs && s.r1 > 0.0 && s.r2 > s.r1 && s.gap > 0.0;
    let attr_points: Vec<[f64; 2]> = if spread_on {
        let a = n as f64 * lw + s.spread_at;
        s.angles
            .iter()
            .map(|p| [a * p.cos(), a * p.sin()])
            .collect()
    } else {
        Vec::new()
    };
    let drop = s.drop.clamp(0.0, 1.0);
    let spread_for = |i: usize| -> Option<Spread> {
        if !spread_on || s.amount <= 0.0 || i < n - m {
            return None;
        }
        let q = (i - (n - m)) as f64;
        let full = ((2.0 * q + 1.0) * s.gap * lw) / 2.0;
        let slope = if drop > 0.0 {
            (drop * (2.0 * q + 1.0)) / (2.0 * m as f64 - 1.0)
        } else {
            0.0
        };
        Some(Spread {
            points: attr_points.clone(),
            r1: s.r1,
            r2: s.r2,
            d: full * s.amount,
            slope,
        })
    };
    let max = s.max_points;
    let common = Common {
        tol,
        angles: &s.angles,
        max,
    };
    let mut used = 0usize;
    let mut rings: Vec<Curve> = Vec::with_capacity(n);
    let mut take = |pts: Vec<P3>, rings: &mut Vec<Curve>| -> Result<(), String> {
        used += pts.len();
        if used > max {
            return Err("too many points: raise the tolerance".into());
        }
        rings.push(Curve::new(pts, false));
        Ok(())
    };

    if s.zipper {
        // The seam is a straight slot, half a line width either side of the seam ray: each
        // ring turns back where it crosses it, every other ring reversed.
        let s0 = if legs { 0.0 } else { pi / 2.0 };
        for (i, &r) in radii.iter().enumerate() {
            let del = (lw / 2.0 / r).min(1.0).asin();
            let sp = spread_for(i);
            let mut pts = stool_loop(&common, r, s0 + del, 2.0 * del, leg_for(i), sp.as_ref())?;
            if i % 2 == 1 {
                pts.reverse();
            }
            take(pts, &mut rings)?;
        }
    } else if legs {
        // Staircase anchored at the outermost ring, drifting back a line width per ring inward.
        let mut starts = vec![0.0; n];
        for i in (0..n - 1).rev() {
            starts[i] = starts[i + 1] + lw / radii[i];
        }
        for i in 0..n {
            let sp = spread_for(i);
            let pts = stool_loop(
                &common,
                radii[i],
                starts[i],
                lw / radii[i],
                leg_for(i),
                sp.as_ref(),
            )?;
            take(pts, &mut rings)?;
        }
    } else {
        // Plain rings: from 90°, each a line width of arc short of a full turn (at least 12
        // segments), the next starting where it stops.
        let mut a = pi / 2.0;
        for &r in &radii {
            let sweep = 2.0 * pi - lw / r;
            let mut dth = 2.0 * (1.0 - tol / r).max(-1.0).acos();
            if !dth.is_finite() || dth <= 0.0 {
                dth = 0.2;
            }
            let steps = steps_of(sweep / dth, 12);
            let mut pts = Vec::with_capacity(steps + 1);
            pts.push([r * a.cos(), r * a.sin(), 0.0]);
            for k in 1..=steps {
                let b = a + (sweep * k as f64) / steps as f64;
                pts.push([r * b.cos(), r * b.sin(), 0.0]);
            }
            a = (a + sweep) % (2.0 * pi);
            take(pts, &mut rings)?;
        }
    }

    // The closed outline of the outermost ring.
    let outline = if legs {
        let sp = spread_for(n - 1);
        let mut pts = stool_loop(&common, radii[n - 1], 0.0, 0.0, leg_for(n - 1), sp.as_ref())?;
        if pts.len() > 1 {
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            if (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6 {
                pts.pop();
            }
        }
        Curve::new(pts, true)
    } else {
        let r = radii[n - 1];
        let mut dth = 2.0 * (1.0 - tol / r).max(-1.0).acos();
        if !dth.is_finite() || dth <= 0.0 {
            dth = 0.2;
        }
        let steps = steps_of(2.0 * pi / dth, 24);
        let pts = (0..steps)
            .map(|k| {
                let a = 2.0 * pi * k as f64 / steps as f64;
                [r * a.cos(), r * a.sin(), 0.0]
            })
            .collect();
        Curve::new(pts, true)
    };
    Ok(LegRings { rings, outline })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> LegSpec {
        LegSpec {
            count: 21,
            spacing: 7.0,
            angles: vec![
                180f64.to_radians(),
                60f64.to_radians(),
                (-60f64).to_radians(),
            ],
            pairs: 5,
            tip: 532.0,
            fillet: 120.0,
            zipper: false,
            spread_at: 0.0,
            r1: 25.0,
            r2: 125.0,
            gap: 1.55,
            amount: 1.0,
            drop: 0.2,
            tolerance: 0.02,
            max_points: 10_000_000,
        }
    }

    /// Each ring starts where the one before it stopped, a line width further out (the step
    /// out), and only the leg rings reach past the seat.
    #[test]
    fn rings_chain_and_only_the_outer_ones_make_legs() {
        let r = leg_rings(&LegSpec {
            amount: 0.0,
            ..spec()
        })
        .unwrap();
        assert_eq!(r.rings.len(), 21);
        let reach = |c: &Curve| {
            c.points
                .iter()
                .map(|p| p[0].hypot(p[1]))
                .fold(0.0, f64::max)
        };
        for (i, ring) in r.rings.iter().enumerate() {
            let radius = 3.5 + 7.0 * i as f64;
            if i < 16 {
                assert!((reach(ring) - radius).abs() < 1e-9, "ring {i}");
            } else {
                assert!(reach(ring) > 500.0, "ring {i} should run out into the legs");
            }
            assert!(ring.points.iter().all(|p| p[2] == 0.0), "no spread, no z");
        }
        // The outermost ring's tip: the cap's centre plus its half-width (5 pairs of 7 mm),
        // within the chord tolerance.
        assert!((reach(&r.rings[20]) - (532.0 + 31.5)).abs() < 0.02);
        assert!(r.outline.closed);
    }

    /// The spread moves points only in the leg rings, keeps how far in z, and scales with the
    /// amount; the innermost hairpin (half a gap each side) is fully spread near the bend.
    #[test]
    fn the_spread_is_kept_in_z_and_scales_with_the_amount() {
        let spread = |amount: f64| {
            let r = leg_rings(&LegSpec { amount, ..spec() }).unwrap();
            r.rings
                .iter()
                .map(|c| c.points.iter().map(|p| p[2]).fold(0.0, f64::max))
                .collect::<Vec<f64>>()
        };
        let (full, half) = (spread(1.0), spread(0.5));
        assert!(
            full[..16].iter().all(|&w| w == 0.0),
            "the round rings don't spread"
        );
        assert!((full[16] - 1.55 * 7.0 / 2.0).abs() < 1e-9, "{}", full[16]);
        for (a, b) in full.iter().zip(&half) {
            assert!((a / 2.0 - b).abs() < 1e-9);
        }
        assert!(spread(0.0).iter().all(|&w| w == 0.0));
    }

    #[test]
    fn impossible_legs_are_refused() {
        assert!(
            leg_rings(&LegSpec {
                tip: 100.0,
                ..spec()
            })
            .is_err(),
            "too short"
        );
        assert!(
            leg_rings(&LegSpec {
                pairs: 21,
                ..spec()
            })
            .is_err(),
            "no round ring left"
        );
        assert!(
            leg_rings(&LegSpec {
                fillet: 2000.0,
                ..spec()
            })
            .is_err(),
            "fillets collide"
        );
        assert!(leg_rings(&LegSpec { count: 0, ..spec() }).is_err());
        assert!(
            leg_rings(&LegSpec {
                tolerance: 0.0,
                ..spec()
            })
            .is_err()
        );
        assert!(
            leg_rings(&LegSpec {
                max_points: 100,
                ..spec()
            })
            .is_err(),
            "too many points"
        );
    }
}
