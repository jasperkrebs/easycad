//! The threaded wing pendant: a lampshade printed as one line, whose wings run in from the wall
//! and close into rings that screw onto the lampholder's thread. A port of the "Thread Lampshade
//! Sketcher" (a friend of the owner's, 16 Nov 2026), pendant with the wings threading onto the
//! socket, so its settings files make the same shade here; with what printing needs on top:
//! a first layer that sits on the bed, overhang compensation by layer height (on the outer wall,
//! never on the thread: the bridge in between climbs from one to the other) and a line width of
//! your own. See docs/thread-pendant.md.
//!
//! Each layer is one turn: the outer wall (the profile, followed continuously as the layer
//! rises), with `entries` places where the line turns in. At first that's a crease (a radial
//! slot, getting deeper by at most the overhang a layer); once the slot reaches the wings'
//! radius, two wings run off it, one each way, as a loop: out along the middle ring, round a
//! half circle onto the inner ring, back past the slot, round onto the middle ring and back to
//! the slot. The inner rings, all the entries' together, are the thread: in the grip zone they
//! sit on the socket's barrel. Every point's height comes from its angle round the axis ("z by
//! angle"), so every ring is a true helix one pitch a turn and the slots take up the difference.

use crate::geom::P3;
use std::f64::consts::{PI, TAU};

/// A point of the profile, the wing curve or the path curve: where (0..1) and the value there.
#[derive(Clone, Copy, Debug)]
pub struct Handle {
    pub u: f64,
    pub v: f64,
    /// A corner: the curve is smooth on either side of it but not through it (profile only).
    pub sharp: bool,
}

/// The shade's settings (the sketcher's names in brackets).
#[derive(Clone, Debug)]
pub struct Settings {
    /// Radius of the outer wall's bead centre against height 0..1 (`profile`).
    pub profile: Vec<Handle>,
    /// Height (`H`) and the radius's scale (`scale`).
    pub height: f64,
    pub scale: f64,
    /// Wing length 0..1 over the wing zone, bottom to top (`wing`).
    pub wing: Vec<Handle>,
    /// How the wings go from the socket (0) out to the wall (1), down from the grip over
    /// `path_len` (`path`, `pathLen`); empty: straight out at the overhang.
    pub path: Vec<Handle>,
    pub path_len: f64,
    /// Entry points (`N`), each sending out two wings.
    pub entries: usize,
    /// How far down from the top the wings reach (`wingLen`).
    pub wing_zone: f64,
    /// The steepest overhang the wings grow or lean at, degrees from vertical (`maxOver`).
    pub max_overhang: f64,
    pub tip_gap: f64,
    pub neck_gap: f64,
    pub wall_gap: f64,
    pub pin_gap: f64,
    /// Turns the entries round, degrees (`wingRot`).
    pub rotation: f64,
    /// The socket barrel's diameter, how far the thread digs into it, the grip's height and
    /// how far below the top it sits (`sockD`, `eSock`, `grip`, `hide`).
    pub socket_d: f64,
    pub engagement: f64,
    pub grip: f64,
    pub hide: f64,
    /// The socket body's height, from the grip's bottom up (`sockH`): it should stay hidden.
    pub socket_h: f64,
    /// Above the grip: only a crease of this depth (true, `topMode` "crease") or full wings.
    pub crease_top: bool,
    pub crease: f64,
    /// How the layers rise: flat, with only the thread a helix (the default); every point at its
    /// angle's height, every ring a helix (the sketcher's `zMode` "angle"); or by the length so
    /// far (its "length", a slicer's vase mode).
    pub layers: Layers,
    /// Line width (the sketcher's 5 mm bead) and layer height (the thread's pitch, `LH`).
    pub line: f64,
    pub layer: f64,
    /// Longest step along an arc (the sketcher's draft 2.5, fine 1.2).
    pub step: f64,
    /// Overhang compensation by layer height on the outer wall (0: off), as the Lampshade's; the
    /// thread keeps its pitch.
    pub strength: f64,
    pub steep: f64,
    pub pivot: f64,
}

/// How the layers rise (see [`Settings::layers`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layers {
    Flat,
    Angle,
    Length,
}

/// Layers over which the thread eases in below the grip and out above it (all exactly a pitch).
const FADE: f64 = 4.0;

/// What came out, besides the line.
#[derive(Clone, Debug, Default)]
pub struct Info {
    /// The top of the print (the sketcher's `zEnd`).
    pub top: f64,
    pub layers: usize,
    /// The outer wall had to be pushed out to make room for the wings.
    pub clamped: bool,
    /// Too many entries for the diameter: the wings can only be stubs.
    pub too_many: bool,
    /// The wing curve grows faster than the overhang allows: slowed down.
    pub limited: bool,
    /// The wing path is steeper than the overhang allows: followed as fast as allowed.
    pub path_limited: bool,
    /// The steepest the outer wall leans, degrees from vertical.
    pub max_lean: f64,
    /// The grip zone (bottom, top) and the inner rings' radius there.
    pub grip_bottom: f64,
    pub grip_top: f64,
    pub grip_r: f64,
    /// Every layer of the grip has full wings on the socket.
    pub grip_full: bool,
    /// How far the socket body sticks out above the top (0: hidden).
    pub socket_shows: f64,
    /// Room between the wings above the grip and the socket body (mm).
    pub socket_room: f64,
    /// Room between the socket and the top opening, for fingers (mm).
    pub top_room: f64,
    /// The wing zone is shorter than grip depth + grip + path height.
    pub zone_short: bool,
    /// Length of the line (mm).
    pub length: f64,
    /// Widest and bottom outer diameter (outside of the bead).
    pub max_d: f64,
    pub bottom_d: f64,
    /// Thinnest layer (overhang compensation).
    pub min_rise: f64,
    /// The most the wing rings move in or out from one layer to the next (mm).
    pub max_wing_step: f64,
    /// Each layer's outer radius and the height of its top (for the wall shown).
    pub wall: Vec<(f64, f64)>,
}

/// A monotone cubic through the points (Fritsch–Carlson, never overshooting), flat outside.
struct Spline {
    xs: Vec<f64>,
    ys: Vec<f64>,
    m: Vec<f64>,
}

impl Spline {
    fn new(xs: Vec<f64>, ys: Vec<f64>) -> Spline {
        let n = xs.len();
        let mut m = vec![0.0; n];
        if n >= 2 {
            let d: Vec<f64> = (0..n - 1)
                .map(|i| (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]).max(1e-9))
                .collect();
            m[0] = d[0];
            m[n - 1] = d[n - 2];
            for i in 1..n - 1 {
                m[i] = if d[i - 1] * d[i] <= 0.0 {
                    0.0
                } else {
                    (d[i - 1] + d[i]) / 2.0
                };
            }
            for i in 0..n - 1 {
                if d[i] == 0.0 {
                    m[i] = 0.0;
                    m[i + 1] = 0.0;
                    continue;
                }
                let (a, b) = (m[i] / d[i], m[i + 1] / d[i]);
                let s = a * a + b * b;
                if s > 9.0 {
                    let t = 3.0 / s.sqrt();
                    m[i] = t * a * d[i];
                    m[i + 1] = t * b * d[i];
                }
            }
        }
        Spline { xs, ys, m }
    }

    fn at(&self, x: f64) -> f64 {
        let (xs, ys, m) = (&self.xs, &self.ys, &self.m);
        let n = xs.len();
        match n {
            0 => 0.0,
            1 => ys[0],
            _ if x <= xs[0] => ys[0],
            _ if x >= xs[n - 1] => ys[n - 1],
            _ => {
                let mut i = 0;
                while i + 2 < n && x > xs[i + 1] {
                    i += 1;
                }
                let h = xs[i + 1] - xs[i];
                let t = (x - xs[i]) / h.max(1e-12);
                let (t2, t3) = (t * t, t * t * t);
                (2.0 * t3 - 3.0 * t2 + 1.0) * ys[i]
                    + (t3 - 2.0 * t2 + t) * h * m[i]
                    + (-2.0 * t3 + 3.0 * t2) * ys[i + 1]
                    + (t3 - t2) * h * m[i + 1]
            }
        }
    }
}

fn sorted(h: &[Handle]) -> Vec<Handle> {
    let mut p = h.to_vec();
    p.sort_by(|a, b| a.u.total_cmp(&b.u));
    p
}

/// The outer wall's radius against height: a monotone cubic through the handles, with a
/// corner at each sharp one (the pieces between corners are separate curves).
struct Radius {
    pieces: Vec<(f64, Spline)>,
}

impl Radius {
    fn new(profile: &[Handle], height: f64, scale: f64) -> Radius {
        let p = sorted(profile);
        let mut groups: Vec<Vec<Handle>> = Vec::new();
        let mut cur: Vec<Handle> = Vec::new();
        for (i, h) in p.iter().enumerate() {
            cur.push(*h);
            if h.sharp && i > 0 && i + 1 < p.len() {
                groups.push(std::mem::take(&mut cur));
                cur.push(*h);
            }
        }
        groups.push(cur);
        let pieces = groups
            .into_iter()
            .map(|g| {
                let z1 = g.last().map_or(0.0, |h| h.u * height);
                let s = Spline::new(
                    g.iter().map(|h| h.u * height).collect(),
                    g.iter().map(|h| h.v * scale).collect(),
                );
                (z1, s)
            })
            .collect();
        Radius { pieces }
    }

    fn at(&self, z: f64) -> f64 {
        for (z1, s) in &self.pieces {
            if z <= *z1 {
                return s.at(z);
            }
        }
        self.pieces.last().map_or(0.0, |(_, s)| s.at(z))
    }
}

/// A curve over 0..1 (the wing length or the path), clamped to 0..1.
fn unit_curve(h: &[Handle]) -> Spline {
    let p = sorted(h);
    Spline::new(
        p.iter().map(|q| q.u).collect(),
        p.iter().map(|q| q.v).collect(),
    )
}

/// One turn being built: its points and, for those that have one, the angle their height comes
/// from (NaN: in between, by length).
struct Rev {
    xs: Vec<f64>,
    ys: Vec<f64>,
    za: Vec<f64>,
    /// The thread: points whose height follows the helix in flat layers.
    helix: Vec<bool>,
    step: f64,
    /// Flat layers: the turns between the rings take their heights from their ends.
    flat: bool,
    on_thread: bool,
}

impl Rev {
    fn new(step: f64, flat: bool) -> Rev {
        Rev {
            xs: Vec::new(),
            ys: Vec::new(),
            za: Vec::new(),
            helix: Vec::new(),
            step,
            flat,
            on_thread: false,
        }
    }

    fn p(&mut self, x: f64, y: f64, a: f64) {
        if let (Some(&px), Some(&py)) = (self.xs.last(), self.ys.last())
            && (x - px).hypot(y - py) < 0.05
        {
            if !a.is_nan()
                && let (Some(last), Some(h)) = (self.za.last_mut(), self.helix.last_mut())
            {
                *last = a;
                *h = self.on_thread;
            }
            return;
        }
        self.xs.push(x);
        self.ys.push(y);
        self.za.push(a);
        self.helix.push(self.on_thread);
    }

    fn arc(&mut self, r: f64, a0: f64, a1: f64) {
        let n = ((a1 - a0).abs() * r / self.step).ceil().clamp(1.0, 1e6) as usize;
        for j in 0..=n {
            let a = a0 + (a1 - a0) * j as f64 / n as f64;
            self.p(r * a.cos(), r * a.sin(), a);
        }
    }

    /// A point in an entry's own frame: `lr` from the axis along its direction `al`, `lt`
    /// sideways (counter-clockwise positive).
    fn loc(&mut self, al: f64, lr: f64, lt: f64, anchored: bool) {
        let (c, s) = (al.cos(), al.sin());
        let a = if anchored { ang(al, lr, lt) } else { f64::NAN };
        self.p(lr * c - lt * s, lr * s + lt * c, a);
    }
}

fn ang(al: f64, lr: f64, lt: f64) -> f64 {
    al + lt.atan2(lr)
}

/// Along an entry's own axis, how far out a point `t` to the side sits on radius `r`: the
/// slots are drawn straight in their own frame, so their ends land on the wall and the rings
/// exactly (the sketcher's land a hair outside: a little step where each ring starts, most
/// near the socket).
fn on_radius(r: f64, t: f64) -> f64 {
    (r * r - t * t).max(0.0).sqrt()
}

/// The slot in from the wall: a quarter turn of radius `q` off the outer wall (radius `ro`),
/// straight in at `-dn`, then (with `qb`) a quarter turn onto radius `r1` going clockwise.
fn neck_pts(ro: f64, r1: f64, dn: f64, step: f64, q: f64, qb: f64) -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let ro = on_radius(ro, dn + q);
    let r1 = if qb > 0.05 {
        on_radius(r1, dn + qb)
    } else {
        on_radius(r1, dn)
    };
    for j in 0..=4 {
        let s = j as f64 / 4.0 * PI / 2.0;
        pts.push((ro - q + q * s.cos(), -dn - q + q * s.sin()));
    }
    let len = (ro - q) - (r1 + qb);
    let n = (len / step).ceil().max(0.0) as usize;
    for j in 1..n {
        pts.push((ro - q - len * j as f64 / n as f64, -dn));
    }
    if qb > 0.05 {
        for j in 0..=4 {
            let s = j as f64 / 4.0 * PI / 2.0;
            pts.push((r1 + qb - qb * s.sin(), -dn - qb + qb * s.cos()));
        }
    } else {
        pts.push((r1, -dn));
    }
    pts
}

/// A crease `d` deep at entry `al`: a slot with a round end, or a shallow dip while it's less
/// than a slot deep. Starts and ends on the outer wall; `cur` is where the wall got to.
#[allow(clippy::too_many_arguments)]
fn feat_spoke(rv: &mut Rev, ro: f64, al: f64, d: f64, dn: f64, q: f64, cur: f64) -> f64 {
    let step = rv.step;
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let r_wall = ro;
    if d >= dn + q + 0.05 {
        let ro = on_radius(r_wall, dn + q);
        for j in 0..=4 {
            let s = j as f64 / 4.0 * PI / 2.0;
            pts.push((ro - q + q * s.cos(), -dn - q + q * s.sin()));
        }
        let ru = ro - d + dn;
        let len = (ro - q) - ru;
        let n = (len / step).ceil().max(0.0) as usize;
        for j in 1..n {
            pts.push((ro - q - len * j as f64 / n as f64, -dn));
        }
        for j in 0..=8 {
            let s = j as f64 / 8.0 * PI;
            pts.push((ru - dn * s.sin(), -dn * s.cos()));
        }
        for j in (1..n).rev() {
            pts.push((ro - q - len * j as f64 / n as f64, dn));
        }
        for j in 0..=4 {
            let s = j as f64 / 4.0 * PI / 2.0;
            pts.push((ro - q + q * s.sin(), dn + q - q * s.cos()));
        }
    } else {
        let w = dn + q;
        for j in 0..=12 {
            let t = -w + 2.0 * w * j as f64 / 12.0;
            pts.push((on_radius(ro, t) - d * 0.5 * (1.0 + (PI * t / w).cos()), t));
        }
    }
    let Some(&(r0, t0)) = pts.first() else {
        return cur;
    };
    rv.arc(r_wall, cur, ang(al, r0, t0));
    for &(r, t) in &pts {
        rv.loc(al, r, t, true);
    }
    pts.last().map_or(cur, |&(r, t)| ang(al, r, t))
}

/// The two wings of entry `al`: in through the slot, out along the middle ring (`rm`) to one
/// tip, round onto the inner ring (`ri`) and along it past the slot to the other tip, round onto
/// the middle ring, back to the slot and out. `l` is each wing's length along its rings.
#[allow(clippy::too_many_arguments)]
fn feat_wings(
    rv: &mut Rev,
    ro: f64,
    rm: f64,
    ri: f64,
    al: f64,
    l: f64,
    dn: f64,
    q: f64,
    cur: f64,
) -> f64 {
    let step = rv.step;
    let rt = (rm + ri) / 2.0;
    let rho2 = (rm - ri) / 2.0;
    // Short wings: the slot turns straight into the wing's tip.
    let qb = q.min((l - dn).max(0.0));
    let nin = neck_pts(ro, rm, dn, step, q, qb);
    let nout: Vec<(f64, f64)> = nin.iter().rev().map(|&(r, t)| (r, -t)).collect();
    let Some(&(r0, t0)) = nin.first() else {
        return cur;
    };
    rv.arc(ro, cur, ang(al, r0, t0));
    for &(r, t) in &nin {
        rv.loc(al, r, t, true);
    }
    let (ba, bb) = (al - l / rt, al + l / rt);
    let a_in = nin.last().map_or(al, |&(r, t)| ang(al, r, t));
    let a_out = nout.first().map_or(al, |&(r, t)| ang(al, r, t));
    // In flat layers the turns between the rings take their heights from their ends (the
    // middle ring flat, the inner one on the thread).
    let turns_anchored = !rv.flat;
    rv.arc(rm, a_in, ba);
    for j in 0..=8 {
        let s = j as f64 / 8.0 * PI;
        rv.loc(
            ba,
            rt + rho2 * s.cos(),
            -rho2 * s.sin(),
            turns_anchored || j == 0,
        );
    }
    rv.on_thread = true;
    rv.arc(ri, ba, bb);
    rv.on_thread = false;
    for j in 0..=8 {
        let s = j as f64 / 8.0 * PI;
        rv.loc(
            bb,
            rt - rho2 * s.cos(),
            rho2 * s.sin(),
            turns_anchored || j == 8,
        );
    }
    rv.arc(rm, bb, a_out);
    for &(r, t) in &nout {
        rv.loc(al, r, t, true);
    }
    nout.last().map_or(cur, |&(r, t)| ang(al, r, t))
}

/// One layer of the plan: where the outer wall's turn starts and how much it rises, and how far
/// the thread (the inner ring) sits above or below the wall at its start.
#[derive(Clone, Copy, Debug)]
struct Layer {
    z0: f64,
    rise: f64,
    /// The thread's height less the wall's, at the layer's start (0 without compensation).
    d: f64,
}

/// The layers. The outer wall rises a pitch a layer, less where it leans (layer-height
/// compensation, as the Lampshade's: pitch × (1 − strength × (lean / pivot)^steep × (1 − cos
/// lean))), all the way up. The thread keeps its own heights: where its rings sit on the socket
/// (and the layer under them) they rise exactly a pitch, so the grip stays a true helix. Over
/// `FADE` layers below and above they ease from the wall's rise into a pitch and back; below
/// that they rise a little less than the wall, to start low enough that the thread meets the
/// wall again at the top. In each layer the bridge (the slot and the wing rings) takes the
/// height from one to the other (see `bridge`).
fn layer_plan(s: &Settings, radius: &Radius) -> Vec<Layer> {
    let lh = s.layer;
    let top_target = (s.height / lh).round().max(3.0) * lh;
    let mut out: Vec<Layer> = Vec::new();
    let mut z = 0.0;
    let comp = s.strength > 0.0;
    while out.len() < 100_000 {
        let rise = if comp {
            let (a, b) = (z, z + lh);
            let lean = (radius.at(b) - radius.at(a)).abs().atan2(lh).to_degrees();
            let curve = s.strength * (lean.min(89.0) / s.pivot.max(1.0)).powf(s.steep);
            lh * (1.0 - curve * (1.0 - lean.to_radians().cos())).max(0.05)
        } else {
            lh
        };
        // The sketcher's layers: their tops one pitch apart up to a pitch below the top.
        if z + lh > top_target - lh + 1e-6 && !out.is_empty() {
            break;
        }
        out.push(Layer {
            z0: z,
            rise,
            d: 0.0,
        });
        z += rise;
    }
    if comp {
        thread_offsets(s, &mut out);
    }
    out
}

/// The thread's offsets from the wall (`Layer::d`), worked down from the top, where it is 0.
fn thread_offsets(s: &Settings, plan: &mut [Layer]) {
    let lh = s.layer;
    let n = plan.len();
    let Some(last) = plan.last() else {
        return;
    };
    let z_end = last.z0 + 2.0 * lh;
    let z_gt = z_end - s.hide;
    let z_gb = z_gt - s.grip;
    let fade = FADE as usize;
    // The thread's rise in each layer; d[k] = d[k + 1] − (its rise − the wall's).
    let mut d = vec![0.0; n + 1];
    // Above the grip (a layer's clearance): easing from a pitch back to the wall's rise.
    let k_b = plan.iter().position(|l| l.z0 + lh > z_gt + lh).unwrap_or(n);
    for k in (k_b..n).rev() {
        let i = k - k_b;
        let w = if i < fade {
            1.0 - (i + 1) as f64 / (fade + 1) as f64
        } else {
            0.0
        };
        let r = plan[k].rise + w * (lh - plan[k].rise);
        d[k] = d[k + 1] - (r - plan[k].rise);
    }
    // The grip and the layer under it: exactly a pitch.
    let mut k_a = k_b;
    while k_a > 0 {
        let k = k_a - 1;
        let dk = d[k + 1] - (lh - plan[k].rise);
        if plan[k].z0 + dk + lh < z_gb - 2.0 * lh {
            break;
        }
        d[k] = dk;
        k_a = k;
    }
    // Below: easing from the wall's rise into a pitch over `fade` layers, and, before that, a
    // little less than the wall's, to start the thread where it has to be. The less is spread
    // over enough layers to stay within a tenth of a pitch a layer, when there's room.
    let avail = k_a.saturating_sub(1);
    if avail > 0 {
        below_the_grip(plan, &mut d, k_a, avail, lh);
    }
    for (l, dk) in plan.iter_mut().zip(&d) {
        l.d = *dk;
    }
}

/// The thread's rise below the exact part (`k_a` up), over the `avail` layers under it.
fn below_the_grip(plan: &[Layer], d: &mut [f64], k_a: usize, avail: usize, lh: f64) {
    let fade = FADE as usize;
    let need = -d[k_a];
    let ramp = |i: usize| -> f64 {
        if i <= fade {
            (fade + 1 - i) as f64 / (fade + 1) as f64
        } else {
            0.0
        }
    };
    // How much of the "less" layer i (1: the one under k_a) takes, with `m` full layers.
    let catch = |i: usize, m: usize| -> f64 {
        if i <= fade {
            1.0 - ramp(i)
        } else if i <= fade + m {
            1.0
        } else if i <= 2 * fade + m {
            1.0 - (i - fade - m) as f64 / (fade + 1) as f64
        } else {
            0.0
        }
    };
    let gain: f64 = (1..=avail.min(fade))
        .map(|i| ramp(i) * (lh - plan[k_a - i].rise))
        .sum();
    let mut m = 0;
    let mut delta;
    loop {
        let total: f64 = (1..=avail).map(|i| catch(i, m)).sum();
        delta = (need + gain) / total.max(1e-9);
        if delta.abs() <= 0.1 * lh || 2 * fade + m >= avail {
            break;
        }
        m += 1;
    }
    for i in 1..=avail {
        let k = k_a - i;
        let wall = plan[k].rise;
        let r = (wall + ramp(i) * (lh - wall) - delta * catch(i, m)).max(0.05 * lh);
        d[k] = d[k + 1] - (r - wall);
    }
}

/// How much of the thread's offset a point at radius `r` takes: all of it on the inner ring in
/// the grip (`lo`), none from a line inside the outer wall outwards (`hi`), smoothly between.
fn bridge(r: f64, lo: f64, hi: f64) -> f64 {
    let t = ((hi - r) / (hi - lo).max(1e-9)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The line, as points (x, y, z) with z one layer below the nozzle (as `toolpath` wants it:
/// it prints a layer above its curve), and what came out.
pub fn thread_shade(s: &Settings) -> Result<(Vec<P3>, Info), String> {
    check(s)?;
    let lh = s.layer;
    let bw = s.line;
    let q = bw / 2.0;
    let radius = Radius::new(&s.profile, s.height, s.scale);
    let wing_fn = unit_curve(&s.wing);
    let path_fn = (!s.path.is_empty()).then(|| unit_curve(&s.path));
    let plan = layer_plan(s, &radius);
    let n = s.entries;
    let step = s.step;
    let phi0 = PI / 2.0 + s.rotation.to_radians();
    let th_s = phi0 - PI / n as f64;
    let dn = (s.neck_gap + bw) / 2.0;
    let tan_over = s.max_overhang.to_radians().tan();
    let mut info = Info {
        grip_full: true,
        min_rise: lh,
        ..Info::default()
    };
    let mut out: Vec<P3> = Vec::new();
    let (mut d_prev, mut l_prev, mut rt_prev, mut ri_prev): (f64, f64, f64, f64) =
        (0.0, 0.0, 0.0, 0.0);
    // Heights in the sketcher's terms: layer k's `zc` is the top of its start (its base + a
    // layer); the top of the print one layer above the last.
    let last_zc = plan.last().map_or(lh, |l| l.z0 + lh);
    let z_end = last_zc + lh;
    let z_start = z_end - s.wing_zone;
    info.top = z_end;
    let r_grip = s.socket_d / 2.0 - s.engagement + bw / 2.0;
    let z_gt = z_end - s.hide;
    let z_gb = z_gt - s.grip;
    info.grip_bottom = z_gb;
    info.grip_top = z_gt;
    info.grip_r = r_grip;
    let wall_ri_of = |ro: f64| ro - bw - s.wall_gap - bw - s.pin_gap;
    // Per layer, for the checks: its height, outer radius, the innermost face, whether it has
    // wings on the socket.
    let mut checks: Vec<(f64, f64, f64, bool)> = Vec::new();
    let mut prev_ro: Option<(f64, f64)> = None;
    let d_at = |k: usize| plan.get(k).map_or(0.0, |l| l.d);
    for (k, layer) in plan.iter().enumerate() {
        let (z0, rise) = (layer.z0, layer.rise);
        let zc = z0 + lh;
        // Where the thread is: the rings' heights (the wall's, plus the offset).
        let zc_r = zc + layer.d;
        let rise_next = plan.get(k + 1).map_or(0.0, |p| p.rise);
        // The thread: a helix in the grip, eased in below it and out above it.
        let thread = (1.0 - (z_gb - zc_r).max(zc_r - z_gt).max(0.0) / (FADE * lh)).clamp(0.0, 1.0);
        let grow = rise * tan_over;
        let ring_rise = rise + d_at(k + 1) - layer.d;
        info.min_rise = info.min_rise.min(rise).min(ring_rise);
        let mut ro = radius.at(zc);
        if let Some((pz, pr)) = prev_ro {
            let lean = (ro - pr).abs().atan2(zc - pz).to_degrees();
            info.max_lean = info.max_lean.max(lean);
        }
        prev_ro = Some((zc, ro));
        let in_zone = n > 0 && zc >= z_start && zc <= z_end;
        let (mut rm, mut ri) = (0.0, 0.0);
        let mut ri_wanted: Option<f64> = None;
        if in_zone {
            let wall_ri = wall_ri_of(ro);
            ri = if zc_r >= z_gb && zc_r <= z_gt {
                r_grip
            } else if zc_r < z_gb && s.path_len > 0.0 {
                // Below the grip: the path from the socket (0) out to the wall (1).
                let t = (z_gb - zc_r) / s.path_len;
                match &path_fn {
                    Some(pf) if t < 1.0 => r_grip + pf.at(t).clamp(0.0, 1.0) * (wall_ri - r_grip),
                    _ => wall_ri,
                }
            } else {
                r_grip + (z_gb - zc_r).max(zc_r - z_gt).max(0.0) * tan_over
            };
            ri = ri.min(wall_ri);
            ri_wanted = Some(ri);
            rm = ri + bw + s.pin_gap;
            let req = rm + bw + s.wall_gap;
            if ro < req - 1e-6 {
                ro = req;
                info.clamped = true;
            }
        }
        if in_zone && l_prev > 0.0 && ri_prev > 0.0 {
            let wall_ri = wall_ri_of(ro);
            ri = wall_ri.min((ri_prev - grow).max((ri_prev + grow).min(ri)));
            rm = ri + bw + s.pin_gap;
            if let Some(w) = ri_wanted
                && (ri - w).abs() > 0.5
            {
                info.path_limited = true;
            }
        }
        let mut rv = Rev::new(step, s.layers == Layers::Flat);
        // 0: nothing, 1: a crease, 2: wings.
        let mut kind = 0;
        let (mut d, mut l) = (0.0, 0.0);
        let crease_top = s.crease_top && zc_r > z_gt && in_zone;
        if crease_top {
            d = s.crease.min(ro - bw);
            kind = usize::from(d > 0.05);
        } else if in_zone {
            let d_full = ro - ri;
            d = if l_prev > 0.0 {
                d_full
            } else {
                d_full.min(d_prev + grow)
            };
            let rt = (rm + ri) / 2.0;
            let rho2 = (rm - ri) / 2.0;
            let l_min = dn + 0.01;
            let mut l_max = PI * rt / n as f64 - (s.tip_gap + bw) / 2.0 - rho2;
            if l_max < l_min {
                info.too_many = true;
                l_max = l_min;
            }
            let u = (zc - z_start) / (z_end - z_start).max(1e-6);
            let wv = wing_fn.at(u).clamp(0.0, 1.0);
            if d >= d_full - 1e-6 && wv > 0.002 {
                let target = l_min + wv * (l_max - l_min);
                l = if l_prev > 0.0 {
                    target.min(l_prev * rt / rt_prev.max(1e-6) + grow)
                } else {
                    l_min
                };
                if l < target - 3.0 * grow {
                    info.limited = true;
                }
                kind = 2;
            } else {
                kind = usize::from(d > 0.05);
                l = 0.0;
            }
        }
        if kind == 2 && ri_prev > 0.0 {
            info.max_wing_step = info.max_wing_step.max((ri - ri_prev).abs());
        }
        d_prev = if in_zone { d } else { 0.0 };
        l_prev = if kind == 2 { l } else { 0.0 };
        rt_prev = (rm + ri) / 2.0;
        ri_prev = if kind == 2 { ri } else { 0.0 };
        let mut cur = th_s;
        if kind > 0 {
            for i in 0..n {
                let al = phi0 + i as f64 * TAU / n as f64;
                cur = if kind == 2 {
                    feat_wings(&mut rv, ro, rm, ri, al, l, dn, q, cur)
                } else {
                    feat_spoke(&mut rv, ro, al, d, dn, q, cur)
                };
            }
        }
        rv.arc(ro, cur, th_s + TAU);
        // The profile followed continuously as the layer rises (not one radius a layer), so a
        // changing shape leaves no step at the seam; the rings only move with it where they
        // follow the wall.
        let clamped_layer = (ro - radius.at(zc)).abs() > 1e-9;
        let wall_ri_l = wall_ri_of(ro);
        let follow_all = kind == 0
            || (kind == 2 && (ri - wall_ri_l).abs() < 1e-6 && (ri - r_grip).abs() >= 1e-6);
        let r_in = match kind {
            2 => rm,
            1 => ro - d,
            _ => ro,
        };
        let inner = if kind == 2 {
            ri - bw / 2.0
        } else {
            ro - d - bw / 2.0
        };
        checks.push((zc_r, ro, inner, kind == 2 && (ri - r_grip).abs() < 0.01));
        info.wall.push((ro, zc));
        let base = z0 + lh / 2.0;
        info.length += finish(
            &rv,
            Heights {
                base,
                rise,
                rise_next,
                th_s,
                thread,
                ramp: 2.0 * bw,
                layers: s.layers,
            },
            Thread {
                d: layer.d,
                d_next: d_at(k + 1),
                d_after: d_at(k + 2),
                // The bridge: the thread's offset fades out from the grip's inner ring to a
                // line inside this layer's outer wall, so the wall stays on its own
                // (compensated) layers. The wall moves in or out a little from layer to layer,
                // and the bridge with it: a point sits a hair off the same point below (the
                // offset × that share, a tenth of a mm or so).
                lo: r_grip,
                hi: (radius.at(zc) - bw).max(r_grip + bw),
            },
            !out.is_empty(),
            &mut out,
            |x, y, z| {
                if clamped_layer {
                    return (x, y);
                }
                let r = x.hypot(y);
                if r < 1e-6 {
                    return (x, y);
                }
                let w = if follow_all {
                    1.0
                } else {
                    ((r - r_in) / (ro - r_in).max(1e-6)).clamp(0.0, 1.0)
                };
                if w <= 0.0 {
                    return (x, y);
                }
                let dr = (radius.at(z.clamp(0.0, s.height)) - ro) * w;
                let k = (r + dr) / r;
                (x * k, y * k)
            },
        );
        if out.len() > 3_000_000 {
            return Err(
                "thread_shade: this shade makes too many points: a larger step or a smaller shade"
                    .into(),
            );
        }
    }
    info.layers = plan.len();
    // The checks the sketcher makes.
    let grip_layers: Vec<&(f64, f64, f64, bool)> = checks
        .iter()
        .filter(|c| c.0 <= z_gt && c.0 >= z_gt - s.grip)
        .collect();
    info.grip_full = !grip_layers.is_empty() && grip_layers.iter().all(|c| c.3);
    let face = z_gb;
    let socket_h = s.socket_h;
    info.socket_shows = (face + socket_h - z_end).max(0.0);
    info.socket_room = checks
        .iter()
        .filter(|c| c.0 > z_gt && c.0 <= face + socket_h)
        .map(|c| c.2 - s.socket_d / 2.0)
        .fold(99.0, f64::min);
    info.top_room = checks.last().map_or(0.0, |c| c.2 - s.socket_d / 2.0);
    info.zone_short = s.wing_zone < s.hide + s.grip + s.path_len;
    info.max_d = checks.iter().map(|c| c.1).fold(0.0, f64::max) * 2.0 + bw;
    info.bottom_d = checks.first().map_or(0.0, |c| c.1) * 2.0 + bw;
    // Heights: the sketcher's are bead centres; the nozzle runs half a layer above, and the
    // curve is drawn a layer below the nozzle.
    // (Never below the bed: the first layer's start can round a hair under it.)
    for p in &mut out {
        p[2] = (p[2] - 1.5 * lh).max(-lh);
    }
    Ok((out, info))
}

/// How a turn's points get their heights.
#[derive(Clone, Copy)]
struct Heights {
    /// The turn's start (the sketcher's bead centre), its rise and the next turn's.
    base: f64,
    rise: f64,
    rise_next: f64,
    /// Where each turn starts (the seam), and how much of a helix the thread is (0..1).
    th_s: f64,
    thread: f64,
    /// Flat layers: the step up to the next layer, along the end of the turn.
    ramp: f64,
    layers: Layers,
}

/// The thread's offset from the wall at the turn's start, the next's and the one after (for
/// the step up at its end), and the bridge's radii (see `bridge`).
struct Thread {
    d: f64,
    d_next: f64,
    d_after: f64,
    lo: f64,
    hi: f64,
}

/// A turn's points into the line, each at its height; returns the turn's length. The wall's
/// heights and the thread's (the same, from its own start and rise) are blended across the
/// bridge: the wall, out by the outer wall, on its own; the inner ring in the grip on the
/// thread's.
///
/// - Flat: every point at the layer's top, except the thread (the inner ring in and near the
///   grip), a helix one pitch a turn: a pitch below the top at the seam, rising to it a turn on;
///   the turns into and out of it in between, by length. The last `ramp` mm step up to the next
///   layer.
/// - By angle: every point at the height its angle round the axis gives (the sketcher's).
/// - By length: rising by the length so far.
fn finish(
    rv: &Rev,
    h: Heights,
    t: Thread,
    skip_first: bool,
    out: &mut Vec<P3>,
    adj: impl Fn(f64, f64, f64) -> (f64, f64),
) -> f64 {
    let (z, len) = heights(rv, &h);
    let z = if t.d.abs() > 1e-12 || t.d_next.abs() > 1e-12 || t.d_after.abs() > 1e-12 {
        let ring = Heights {
            base: h.base + t.d,
            rise: h.rise + t.d_next - t.d,
            rise_next: if h.rise_next > 0.0 {
                h.rise_next + t.d_after - t.d_next
            } else {
                0.0
            },
            ..h
        };
        let (zr, _) = heights(rv, &ring);
        z.iter()
            .zip(&zr)
            .zip(rv.xs.iter().zip(&rv.ys))
            .map(|((&zw, &zt), (&x, &y))| zw + (zt - zw) * bridge(x.hypot(y), t.lo, t.hi))
            .collect()
    } else {
        z
    };
    for ((&x, &y), &zj) in rv
        .xs
        .iter()
        .zip(&rv.ys)
        .zip(&z)
        .skip(usize::from(skip_first))
    {
        let (x, y) = adj(x, y, zj);
        out.push([x, y, zj]);
    }
    len
}

/// A turn's points' heights (see `finish`), and its length.
fn heights(rv: &Rev, h: &Heights) -> (Vec<f64>, f64) {
    let n = rv.xs.len();
    let mut cum = vec![0.0; n];
    for j in 1..n {
        cum[j] = cum[j - 1] + (rv.xs[j] - rv.xs[j - 1]).hypot(rv.ys[j] - rv.ys[j - 1]);
    }
    let len = cum.last().copied().unwrap_or(0.0);
    let (base, rise) = (h.base, h.rise);
    let mut z = vec![base; n];
    if h.layers == Layers::Length || len <= 0.0 {
        for j in 0..n {
            z[j] = base + rise * cum[j] / len.max(1e-9);
        }
    } else {
        let top = base + rise;
        let at = |j: usize, a: f64| -> f64 {
            let frac = ((a - h.th_s) / TAU).clamp(0.0, 1.0);
            match h.layers {
                Layers::Flat if rv.helix.get(j) == Some(&true) => {
                    top - rise * (1.0 - frac) * h.thread
                }
                Layers::Flat => top,
                _ => base + rise * frac,
            }
        };
        let mut last: Option<usize> = None;
        for j in 0..n {
            let a = rv.za[j];
            if a.is_nan() {
                continue;
            }
            z[j] = at(j, a);
            if let Some(l) = last {
                for m in l + 1..j {
                    let f = (cum[m] - cum[l]) / (cum[j] - cum[l]).max(1e-12);
                    z[m] = z[l] + (z[j] - z[l]) * f;
                }
            } else {
                let zj = z[j];
                for zm in z.iter_mut().take(j) {
                    *zm = zj;
                }
            }
            last = Some(j);
        }
        if let Some(l) = last {
            let zl = z[l];
            for zm in z.iter_mut().skip(l + 1) {
                *zm = zl;
            }
        }
        if h.layers == Layers::Flat && h.rise_next > 0.0 {
            let ramp = h.ramp.min(len / 4.0).max(1e-6);
            for (zj, &c) in z.iter_mut().zip(&cum) {
                let t = (c - (len - ramp)) / ramp;
                if t > 0.0 {
                    *zj += h.rise_next * t.min(1.0);
                }
            }
        }
    }
    (z, len)
}

fn check(s: &Settings) -> Result<(), String> {
    let finite = [
        s.height,
        s.scale,
        s.path_len,
        s.wing_zone,
        s.max_overhang,
        s.tip_gap,
        s.neck_gap,
        s.wall_gap,
        s.pin_gap,
        s.rotation,
        s.socket_d,
        s.engagement,
        s.grip,
        s.hide,
        s.socket_h,
        s.crease,
        s.line,
        s.layer,
        s.step,
        s.strength,
        s.steep,
        s.pivot,
    ];
    if finite.iter().any(|v| !v.is_finite()) {
        return Err("thread_shade: every setting must be a number".into());
    }
    if s.profile.len() < 2 {
        return Err(
            "thread_shade: the profile needs at least two points [height 0..1, radius]".into(),
        );
    }
    if s.profile
        .iter()
        .chain(&s.wing)
        .chain(&s.path)
        .any(|h| !h.u.is_finite() || !h.v.is_finite())
    {
        return Err("thread_shade: the curves' points must be numbers".into());
    }
    if s.profile.iter().any(|h| h.v <= 0.0) {
        return Err("thread_shade: the profile's radii must be more than 0".into());
    }
    if s.wing.is_empty() {
        return Err("thread_shade: the wing curve needs at least one point".into());
    }
    if !(1..=8).contains(&s.entries) {
        return Err("thread_shade: `entries` must be 1 to 8".into());
    }
    if s.layer < 0.1 || s.line < 0.2 || s.height < 3.0 * s.layer || s.height > 5000.0 {
        return Err("thread_shade: the height must be at least three layers (and at most 5 m), the layer at least 0.1 mm, the line at least 0.2 mm".into());
    }
    if s.step < 0.2 {
        return Err("thread_shade: `step` must be at least 0.2 mm".into());
    }
    if !(1.0..=89.0).contains(&s.max_overhang) {
        return Err("thread_shade: `max_overhang` must be 1° to 89°".into());
    }
    if s.socket_d <= 0.0 || s.scale <= 0.0 {
        return Err("thread_shade: the socket's diameter and the scale must be more than 0".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(u: f64, v: f64) -> Handle {
        Handle { u, v, sharp: false }
    }

    /// The sketcher's "straight" settings file (a 150 mm cylinder, three entries).
    pub fn straight() -> Settings {
        Settings {
            profile: (0..15)
                .map(|i| h(i as f64 / 14.0, 72.49999999999999))
                .collect(),
            height: 250.0,
            scale: 1.0,
            wing: vec![
                h(0.0, 0.0),
                h(0.4241898148148148, 1.0),
                h(0.8069058641975309, 1.0),
                h(1.0, 1.0),
            ],
            path: (0..7)
                .map(|i| {
                    let u = i as f64 / 6.0;
                    h(u, (u * PI / 2.0).sin())
                })
                .collect(),
            path_len: 80.0,
            entries: 3,
            wing_zone: 202.0,
            max_overhang: 50.0,
            tip_gap: 0.0,
            neck_gap: 0.0,
            wall_gap: 5.0,
            pin_gap: 5.0,
            rotation: 0.0,
            socket_d: 40.0,
            engagement: 0.0,
            grip: 30.0,
            hide: 20.0,
            socket_h: 50.0,
            crease_top: true,
            crease: 14.0,
            layers: Layers::Angle,
            line: 5.0,
            layer: 2.5,
            step: 2.5,
            strength: 0.0,
            steep: 0.0,
            pivot: 45.0,
        }
    }

    /// The sketcher's "Sphere" settings file (a globe 367 mm across, five entries).
    fn sphere() -> Settings {
        let r = [
            90.5,
            115.25891768488509,
            136.82123593769808,
            154.58894381346775,
            168.0692703748495,
            176.88835123810793,
            180.80159735586057,
            179.70047846798673,
            173.61553308796823,
            162.7155215454755,
            147.30274557477932,
            127.80466425587272,
            104.7620388324187,
            78.813935199027,
            50.68000000000006,
        ];
        let u = [
            0.0,
            0.052018875501840194,
            0.11574843957283734,
            0.18942121172291324,
            0.2709939457864398,
            0.3582042974640113,
            0.4486335683482259,
            0.5397737862859743,
            0.6290972616654624,
            0.7141266905487044,
            0.7925038604039742,
            0.8620550529483045,
            0.9208513302127038,
            0.9672620318518694,
            1.0,
        ];
        Settings {
            profile: u.iter().zip(r).map(|(&u, r)| h(u, r)).collect(),
            height: 330.0,
            wing: vec![
                h(0.0, 0.0),
                h(0.2991898148148148, 0.9307692307692308),
                h(0.5275848765432098, 1.0),
                h(1.0, 1.0),
            ],
            path: vec![h(0.0, 0.0), h(0.5, 0.5), h(1.0, 1.0)],
            path_len: 125.0,
            entries: 5,
            wing_zone: 202.5,
            max_overhang: 70.0,
            grip: 28.5,
            hide: 35.0,
            socket_h: 55.0,
            crease: 15.0,
            engagement: 1.0,
            ..straight()
        }
    }

    /// Point for point what the sketcher itself draws for its own settings files: every 2500th
    /// point (bead centres), from its generator run in Node on the files (16 Nov 2026).
    #[test]
    fn the_same_points_as_the_sketcher() {
        let cases: [(Settings, usize, &[[f64; 3]]); 2] = [
            (
                straight(),
                38493,
                &[
                    [62.7868, 36.2500, 1.2500],
                    [-2.4888, -72.4573, 35.4030],
                    [-42.0129, 59.0861, 64.4126],
                    [-44.2191, -44.1692, 82.6039],
                    [-4.0433, 72.3097, 96.6889],
                    [-61.8718, -32.8349, 109.9858],
                    [-46.8357, 41.1371, 122.0049],
                    [48.5969, 17.3342, 133.6780],
                    [-2.5241, -54.1260, 145.3981],
                    [-12.3972, 71.4322, 156.7350],
                    [20.9338, 33.0939, 168.9422],
                    [8.3177, 31.5201, 181.5640],
                    [-72.4336, -3.1027, 194.8087],
                    [25.6458, -20.5801, 208.2726],
                    [-11.4466, 30.4175, 221.8099],
                    [50.8489, 51.6782, 238.8574],
                ],
            ),
            (
                sphere(),
                96813,
                &[
                    [54.2646, 74.6888, 1.2500],
                    [-23.8417, 120.8903, 24.0775],
                    [31.2140, -137.2386, 42.8390],
                    [-148.1370, -37.6488, 59.7240],
                    [93.2798, -132.2133, 75.4945],
                    [104.8230, -131.9314, 90.5171],
                    [-92.6954, -146.6698, 105.0257],
                    [-80.2911, 157.8550, 119.1872],
                    [-135.5525, 117.6257, 131.8406],
                    [118.2382, -115.9014, 140.5665],
                    [-60.0063, -170.5522, 147.6154],
                    [-47.7885, 141.0263, 154.1300],
                    [-105.4168, -140.8405, 159.9943],
                    [93.6832, -154.3871, 165.4670],
                    [109.9071, -66.9834, 170.6572],
                    [103.3528, -83.5322, 175.6045],
                    [67.7563, -95.3640, 180.4958],
                    [81.6856, -158.9536, 185.4389],
                    [-71.2913, -102.3773, 190.0080],
                    [-85.0557, -68.0751, 194.8936],
                    [-171.4486, -38.7741, 199.7135],
                    [-94.4325, 27.8541, 204.5109],
                    [-80.5978, 6.8281, 209.5914],
                    [-62.6947, 41.0888, 214.3942],
                    [-62.0902, 30.3346, 219.4442],
                    [-84.7061, 24.8931, 224.5113],
                    [-152.1551, -65.0259, 229.7857],
                    [-14.0696, -50.1933, 235.1413],
                    [53.0643, -77.2858, 240.4894],
                    [122.2533, 37.1021, 245.9922],
                    [4.9302, 33.6715, 251.4422],
                    [-144.3997, -41.6951, 257.2368],
                    [104.6831, -102.0293, 263.0676],
                    [-132.8120, 48.4259, 269.4859],
                    [102.0426, -90.1936, 275.5870],
                    [-120.2249, -49.4561, 282.2803],
                    [-2.5214, 115.6061, 289.0087],
                    [-32.3407, 109.7336, 296.6140],
                    [72.6594, 24.6141, 313.5050],
                ],
            ),
        ];
        for (s, count, samples) in cases {
            let (pts, _) = thread_shade(&s).unwrap();
            // The slots' ends land on the rings here (the sketcher's a hair outside), so the
            // count differs a little and the sketcher's points sit within half a millimetre of
            // the line, everywhere.
            assert!(
                (pts.len() as f64 / count as f64 - 1.0).abs() < 0.03,
                "{}",
                pts.len()
            );
            for (i, want) in samples.iter().enumerate() {
                let near = pts
                    .iter()
                    .map(|p| {
                        let got = [p[0], p[1], p[2] + 1.5 * s.layer];
                        (0..3)
                            .map(|k| (got[k] - want[k]).powi(2))
                            .sum::<f64>()
                            .sqrt()
                    })
                    .fold(f64::INFINITY, f64::min);
                assert!(near < 0.5, "{i}: {want:?} {near}");
            }
        }
    }

    /// The slots' ends land on the wall and the rings: no zigzag where a ring starts (the
    /// sketcher's landed a hair outside, 0.2–0.4 mm, most near the socket: 876 of them in this
    /// shade, the owner's photos of 17 Nov). A zigzag: two sharp turns one way and back within a
    /// millimetre.
    #[test]
    fn no_zigzags_where_the_rings_start() {
        for layers in [Layers::Flat, Layers::Angle] {
            let s = Settings {
                layers,
                ..straight()
            };
            let (pts, _) = thread_shade(&s).unwrap();
            let mut segs: Vec<((f64, f64), f64)> = Vec::new();
            let mut last = pts[0];
            for p in &pts[1..] {
                let (dx, dy) = (p[0] - last[0], p[1] - last[1]);
                let l = dx.hypot(dy);
                if l >= 0.02 {
                    segs.push(((dx / l, dy / l), l));
                    last = *p;
                }
            }
            let turn = |a: (f64, f64), b: (f64, f64)| {
                (a.0 * b.1 - a.1 * b.0)
                    .atan2(a.0 * b.0 + a.1 * b.1)
                    .to_degrees()
            };
            let zigzags = segs
                .windows(3)
                .filter(|w| {
                    let (t1, t2) = (turn(w[0].0, w[1].0), turn(w[1].0, w[2].0));
                    t1.abs() > 30.0 && t2.abs() > 30.0 && t1.signum() != t2.signum() && w[1].1 < 1.0
                })
                .count();
            assert_eq!(zigzags, 0, "{layers:?}");
        }
    }

    /// Flat layers: the outer wall is level, a layer up each turn, except where it steps up to
    /// the next layer (the last two line widths of each turn, at the seam); only the thread
    /// (the inner rings in the grip) is a helix.
    #[test]
    fn flat_layers_except_the_thread() {
        let s = Settings {
            layers: Layers::Flat,
            ..straight()
        };
        let (pts, info) = thread_shade(&s).unwrap();
        let th_s = PI / 2.0 - PI / 3.0;
        let mut wall = 0;
        for p in &pts {
            let r = p[0].hypot(p[1]);
            if (r - 72.5).abs() > 0.01 {
                continue;
            }
            // How far before the seam, along the wall.
            let back = (th_s - p[1].atan2(p[0])).rem_euclid(TAU) * r;
            let nozzle = p[2] + s.layer;
            if back > 2.0 * s.line + 0.5 {
                assert!(
                    (nozzle / s.layer - (nozzle / s.layer).round()).abs() < 1e-9,
                    "{p:?}"
                );
                wall += 1;
            }
        }
        assert!(wall > 10_000, "{wall}");
        // The thread: on the barrel, a helix (its height less its angle's share of a pitch the
        // same at every point, whole pitches apart).
        let ring: Vec<&P3> = pts
            .iter()
            .filter(|p| (p[0].hypot(p[1]) - info.grip_r).abs() < 1e-6)
            .collect();
        assert!(ring.len() > 200);
        let phase = |p: &P3| {
            let t = (p[2] + s.layer) / s.layer - (p[1].atan2(p[0]) - th_s).rem_euclid(TAU) / TAU;
            t - t.round()
        };
        assert!(
            ring.iter().all(|p| phase(p).abs() < 1e-6),
            "{:?}",
            ring.iter().map(|p| phase(p)).fold(0.0, f64::max)
        );
    }

    #[test]
    fn the_sketchers_straight_shade() {
        let (pts, info) = thread_shade(&straight()).unwrap();
        // The sketcher: 99 layers (H 250 / 2.5 − 1), top at 250.
        assert_eq!(info.layers, 99);
        assert!((info.top - 250.0).abs() < 1e-9, "{}", info.top);
        assert!(info.grip_full);
        assert!((info.max_d - 150.0).abs() < 0.01, "{}", info.max_d);
        assert!(pts.len() > 20_000, "{}", pts.len());
        assert!(!info.clamped && !info.too_many);
    }

    /// In the grip the inner rings are a true helix on the socket: at any angle, a point on the
    /// inner ring sits at the grip radius, one pitch above the one below it.
    #[test]
    fn the_grip_is_a_true_helix_on_the_socket() {
        let s = straight();
        let (pts, info) = thread_shade(&s).unwrap();
        let r = info.grip_r;
        let on_ring: Vec<&P3> = pts
            .iter()
            .filter(|p| (p[0].hypot(p[1]) - r).abs() < 1e-6)
            .filter(|p| {
                p[2] + 1.5 * s.layer > info.grip_bottom + s.layer
                    && p[2] + 1.5 * s.layer < info.grip_top - s.layer
            })
            .collect();
        assert!(on_ring.len() > 200, "{}", on_ring.len());
        for p in &on_ring {
            let a = p[1].atan2(p[0]);
            let turns = (p[2] + 1.5 * s.layer - s.layer / 2.0) / s.layer
                - (a - (PI / 2.0 - PI / 3.0)) / TAU;
            assert!((turns - turns.round()).abs() < 1e-6, "{p:?} {turns}");
        }
    }

    /// A leaning wall: compensated in every layer, the grip's included; the thread on the
    /// socket still a true helix a pitch a turn; the two meet again at the top, and the bridge
    /// between them climbs gently.
    fn leaning() -> Settings {
        let mut s = straight();
        s.profile = vec![
            Handle {
                u: 0.0,
                v: 120.0,
                sharp: true,
            },
            h(1.0, 50.0),
        ];
        s.strength = 1.0;
        s
    }

    #[test]
    fn overhang_compensation_on_the_wall_not_the_thread() {
        for layers in [Layers::Angle, Layers::Flat] {
            let s = Settings {
                layers,
                ..leaning()
            };
            let radius = Radius::new(&s.profile, s.height, s.scale);
            let plan = layer_plan(&s, &radius);
            // The wall: compensated all the way up.
            assert!(plan.iter().all(|l| l.rise < s.layer - 0.05), "{plan:?}");
            // The thread meets the wall at the top and is below it in the grip.
            assert!(plan.last().unwrap().d.abs() < 1e-12);
            assert!(plan.iter().any(|l| l.d < -1.0), "{plan:?}");
            // Its own rise never jumps: from layer to layer it changes by at most a fifth of a
            // pitch, and never gets thin.
            let ring: Vec<f64> = plan
                .windows(2)
                .map(|w| w[0].rise + w[1].d - w[0].d)
                .collect();
            assert!(ring.iter().all(|&r| r > 0.5 * s.layer), "{ring:?}");
            assert!(
                ring.windows(2).all(|w| (w[1] - w[0]).abs() < 0.2 * s.layer),
                "{ring:?}"
            );
            let (pts, info) = thread_shade(&s).unwrap();
            assert!(info.grip_full);
            // On the barrel in the grip: a true helix (its height less its angle's share of a
            // pitch the same at every point, whole pitches apart).
            let th_s = PI / 2.0 - PI / 3.0;
            let ring: Vec<&P3> = pts
                .iter()
                .filter(|p| (p[0].hypot(p[1]) - info.grip_r).abs() < 1e-6)
                .filter(|p| {
                    let zc = p[2] + 1.5 * s.layer;
                    zc > info.grip_bottom && zc < info.grip_top
                })
                .collect();
            assert!(ring.len() > 200, "{}", ring.len());
            let phase = |p: &P3| {
                let a = (p[1].atan2(p[0]) - th_s).rem_euclid(TAU) / TAU;
                p[2] / s.layer
                    - if layers == Layers::Flat {
                        a.min(1.0)
                    } else {
                        a
                    }
            };
            let p0 = phase(ring[0]);
            for p in &ring {
                let t = phase(p) - p0;
                assert!((t - t.round()).abs() < 1e-6, "{layers:?} {p:?} {t}");
            }
            // The outer wall: on its own layers (each turn's wall a layer's rise above the last).
            let wall: Vec<&P3> = pts
                .iter()
                .filter(|p| {
                    let r = p[0].hypot(p[1]);
                    (r - radius.at(p[2] + 1.5 * s.layer)).abs() < 1e-6
                })
                .collect();
            assert!(wall.len() > 1000);
            // The bridge: no steeper than 1 in 4 anywhere off the seam's step.
            let mut steepest: f64 = 0.0;
            for w in pts.windows(2) {
                let run = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                let r = w[0][0].hypot(w[0][1]);
                if run > 0.2 && r < radius.at(w[0][2] + 1.5 * s.layer) - s.line {
                    steepest = steepest.max((w[1][2] - w[0][2]).abs() / run);
                }
            }
            assert!(steepest < 0.25, "{layers:?} {steepest}");
        }
    }

    /// Without compensation, nothing moves: the thread and the wall on the same layers.
    #[test]
    fn no_offset_without_compensation() {
        let s = Settings {
            strength: 0.0,
            ..leaning()
        };
        let plan = layer_plan(&s, &Radius::new(&s.profile, s.height, s.scale));
        assert!(plan.iter().all(|l| l.d == 0.0 && l.rise == s.layer));
    }

    /// Found by the settings sweep: a custom 3.45 mm pitch rounded the first point under the bed.
    #[test]
    fn never_below_the_bed() {
        let mut s = straight();
        s.layer = 3.45;
        s.hide = 85.0;
        s.height = 2370.0;
        s.path_len = 200.0;
        s.tip_gap = 16.5;
        s.strength = 1.0;
        s.steep = 0.7;
        s.pivot = 57.0;
        let (pts, _) = thread_shade(&s).unwrap();
        assert!(
            pts.iter().all(|p| p[2] >= -s.layer),
            "{:?}",
            pts.iter().map(|p| p[2]).fold(0.0, f64::min)
        );
    }

    #[test]
    fn bad_settings_are_errors() {
        let mut s = straight();
        s.entries = 0;
        assert!(thread_shade(&s).is_err());
        let mut s = straight();
        s.profile.truncate(1);
        assert!(thread_shade(&s).is_err());
        let mut s = straight();
        s.height = f64::NAN;
        assert!(thread_shade(&s).is_err());
    }
}
