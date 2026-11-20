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
    /// The gap between the wing lines closes again before the grip (as it opens from none where
    /// the wings start): none all over the socket, so each gap is a closed pocket.
    pub pockets: bool,
    /// The wings as drawn (the "leaf"): their width from `wing` exactly, a straight path from
    /// where they start (at the wall, `wing_zone` down from the top) to the socket, nothing
    /// limited by `max_overhang` (and `path`, `path_len` unused). Off: the sketcher's wings.
    pub leaf: bool,
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
    /// How the layers rise: every point at its angle's height, every layer rising a layer a turn
    /// with no seam and every ring a helix (the default, the sketcher's `zMode` "angle"); flat,
    /// with only the thread a helix and each layer stepping up at a seam; or by the length so
    /// far (its "length", a slicer's vase mode).
    pub layers: Layers,
    /// Line width (the sketcher's 5 mm bead) and layer height (the thread's pitch, `LH`).
    pub line: f64,
    pub layer: f64,
    /// The outer wall's and the wings' own line width and layer height (0: the thread's `line`
    /// and the pitch): thinner everywhere but the thread, which grows into its own over the
    /// layers before the grip.
    pub wall_line: f64,
    pub wall_layer: f64,
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

/// How much slower than the wall the wing rings rise at most, where the thread starts below the
/// wall or the wall catches up with it.
const SLOW: f64 = 0.25;

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
    /// The steepest the thread's lead makes the line climb (the bridge from the wall to the
    /// thread, on top of the layer's own heights), degrees.
    pub max_bridge_slope: f64,
    /// The thinnest the wings and the arm get (mm), and the same for their layer's rise (1: as
    /// planned): where they move in or out while the lead changes, a point sits a little low on
    /// the one below.
    pub min_ring_gap: f64,
    pub min_ring_share: f64,
    /// The thickest they get for their layer's rise (where the wings move in far below the
    /// wall's heights, the arm just outside them sits a little high on the one below).
    pub max_ring_share: f64,
    /// Each point's bead: the line width and layer height to extrude the move to it for.
    pub bead: Vec<[f64; 2]>,
    /// Each layer with wings: the thread's height there, the inner and middle rings' radii and
    /// the gap between them.
    pub rings: Vec<[f64; 4]>,
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
/// the thread (the inner ring) sits above the wall at its start.
#[derive(Clone, Copy, Debug)]
struct Layer {
    z0: f64,
    rise: f64,
    /// The thread's height less the wall's, at the layer's start.
    d: f64,
    /// How much of a thread this layer's inner ring is, 0..1: 0 a wall layer (the wall's
    /// height, width and bead), 1 the thread (a pitch, the thread's width and bead).
    f: f64,
    /// The layer height the wall is extruded for: its own (squeezed where it leans), less where
    /// it gives the wings layers (see `layer_plan`).
    bead: f64,
    /// Of `d`, the thread's part (the rest the wings' own, from rising their own way).
    t: f64,
}

/// The wall's own layer height: `wall_layer` (0: the pitch), never more than the pitch.
fn wall_layer(s: &Settings) -> f64 {
    if s.wall_layer > 0.0 {
        s.wall_layer.min(s.layer)
    } else {
        s.layer
    }
}

/// The wall's (and wings') own line width: `wall_line` (0: the thread's, `line`).
fn wall_line(s: &Settings) -> f64 {
    if s.wall_line > 0.0 {
        s.wall_line
    } else {
        s.line
    }
}

/// The wings' shape, as a function of height: where the inner ring wants to be, how wide the
/// leaf is, how far the wings lean. Shared by the layer plan (the wings' own compensation) and
/// the line itself.
struct Geo<'a> {
    s: &'a Settings,
    radius: &'a Radius,
    lh: f64,
    lw: f64,
    bw: f64,
    r_grip: f64,
    z_end: f64,
    z_start: f64,
    z_gt: f64,
    z_gb: f64,
    tan_over: f64,
    wing: Spline,
    path: Option<Spline>,
}

impl<'a> Geo<'a> {
    fn new(s: &'a Settings, radius: &'a Radius, z_end: f64) -> Geo<'a> {
        let z_gt = z_end - s.hide;
        Geo {
            s,
            radius,
            lh: s.layer,
            lw: wall_layer(s),
            bw: wall_line(s),
            r_grip: s.socket_d / 2.0 - s.engagement + s.line / 2.0,
            z_end,
            z_start: z_end - s.wing_zone,
            z_gt,
            z_gb: z_gt - s.grip,
            tan_over: s.max_overhang.to_radians().tan(),
            wing: unit_curve(&s.wing),
            path: (!s.path.is_empty()).then(|| unit_curve(&s.path)),
        }
    }

    /// The inner ring furthest out: inside the wall `ro`, its gap, the middle ring and its gap
    /// (`wi`, `wm` the inner and middle rings' widths, `gap` between them).
    fn wall_ri(&self, ro: f64, wi: f64, wm: f64, gap: f64) -> f64 {
        ro - (self.bw + wm) / 2.0 - self.s.wall_gap - (wm + wi) / 2.0 - gap
    }

    /// The same at height `z`, the rings a wall line wide with the whole gap.
    fn wall_ri_at(&self, z: f64) -> f64 {
        let bw = self.bw;
        self.wall_ri(self.radius.at(z), bw, bw, self.s.pin_gap.max(0.0))
    }

    /// Where the inner ring wants to be at the rings' height `z`: on the socket in the grip.
    /// The leaf: below it a straight line from the wall where the wings start to the socket,
    /// above it (full wings) one from the socket back to the wall at the top. The sketcher's:
    /// below the grip along its path from the socket (0) out to the wall (1), above it (or with
    /// no path) leaning out at the max overhang. Never further out than `wall_ri`.
    fn target(&self, z: f64, wall_ri: f64) -> f64 {
        let (z_gb, z_gt, r_grip) = (self.z_gb, self.z_gt, self.r_grip);
        let r = if z >= z_gb && z <= z_gt {
            r_grip
        } else if self.s.leaf {
            let (from, to) = if z < z_gb {
                (self.z_start, z_gb)
            } else {
                (self.z_end, z_gt)
            };
            let span = (to - from).abs();
            if span < 1e-6 {
                r_grip
            } else {
                let t = ((to - z) / (to - from)).clamp(0.0, 1.0);
                r_grip + t * (self.wall_ri_at(from) - r_grip)
            }
        } else if z < z_gb && self.s.path_len > 0.0 {
            let t = (z_gb - z) / self.s.path_len;
            match &self.path {
                Some(pf) if t < 1.0 => r_grip + pf.at(t).clamp(0.0, 1.0) * (wall_ri - r_grip),
                _ => wall_ri,
            }
        } else {
            r_grip + (z_gb - z).max(z - z_gt).max(0.0) * self.tan_over
        };
        r.min(wall_ri)
    }

    /// The leaf's middle ring at the rings' height `z` (off the socket): a straight line from
    /// the wall where the wings start to its place on the socket (two thread lines and the gap
    /// there out from the barrel) below the grip, and (full wings) another back out to the wall
    /// at the top above it; never further out than `rm_wall`.
    fn middle(&self, z: f64, rm_wall: f64) -> f64 {
        let s = self.s;
        let at_wall = |z: f64| self.radius.at(z) - self.bw - s.wall_gap;
        let on_socket = self.r_grip + s.line + if s.pockets { 0.0 } else { s.pin_gap.max(0.0) };
        let (from, to) = if z < self.z_gb {
            (self.z_start, self.z_gb)
        } else {
            (self.z_end, self.z_gt)
        };
        let r = if (to - from).abs() < 1e-6 {
            on_socket
        } else {
            let t = ((to - z) / (to - from)).clamp(0.0, 1.0);
            on_socket + t * (at_wall(from) - on_socket)
        };
        r.min(rm_wall)
    }

    /// Where along the wing zone the height `z` is, 0..1 (for the leaf's width).
    fn along(&self, z: f64) -> f64 {
        (z - self.z_start) / (self.z_end - self.z_start).max(1e-6)
    }

    /// Whether the rings are there at height `z`, off the socket: wings, below the grip or
    /// (full wings) above it.
    fn rings_at(&self, z: f64) -> bool {
        let wv = self.wing.at(self.along(z).clamp(0.0, 1.0));
        // (The sketcher's: once the slot, cutting in at the max overhang, has reached them.)
        let reached = self.s.leaf
            || (z - self.z_start) * self.tan_over
                >= self.radius.at(z) - self.target(z, self.wall_ri_at(z));
        z >= self.z_start
            && z <= self.z_end
            && !(z >= self.z_gb - self.lh && z <= self.z_gt)
            && !(self.s.crease_top && z > self.z_gt)
            && wv > 0.002
            && reached
    }

    /// How far the wings lean at the rings' height `z`, degrees from vertical: the rings coming
    /// in towards the socket (or going out), not the leaf's edges growing along them. The
    /// sketcher's wings never lean past the max overhang.
    fn lean(&self, z: f64) -> f64 {
        let h = 1.0;
        let r = |z: f64| self.target(z, self.wall_ri_at(z));
        let lean = (r(z + h) - r(z - h)).abs().atan2(2.0 * h).to_degrees();
        if self.s.leaf {
            lean
        } else {
            lean.min(self.s.max_overhang)
        }
    }

    /// A layer's rise where it leans `lean` degrees: layer-height compensation, as the
    /// Lampshade's (× (1 − strength × (lean / pivot)^steep × (1 − cos lean))).
    fn compensated(&self, lean: f64) -> f64 {
        let s = self.s;
        if s.strength <= 0.0 {
            return self.lw;
        }
        let curve = s.strength * (lean.min(89.0) / s.pivot.max(1.0)).powf(s.steep);
        self.lw * (1.0 - curve * (1.0 - lean.to_radians().cos())).max(0.05)
    }
}

/// The wall's layers: its own layer height, less where it leans (layer-height compensation).
/// Each layer's bead is the wall's own layer (squeezed as compensation squeezes it).
fn wall_layers(s: &Settings, radius: &Radius) -> Vec<Layer> {
    let lh = s.layer;
    let lw = wall_layer(s);
    let top_target = (s.height / lh).round().max(3.0) * lh;
    let mut out: Vec<Layer> = Vec::new();
    let mut z = 0.0;
    let comp = s.strength > 0.0;
    while out.len() < 100_000 {
        let rise = if comp {
            let (a, b) = (z, z + lw);
            let lean = (radius.at(b) - radius.at(a)).abs().atan2(lw).to_degrees();
            let curve = s.strength * (lean.min(89.0) / s.pivot.max(1.0)).powf(s.steep);
            lw * (1.0 - curve * (1.0 - lean.to_radians().cos())).max(0.05)
        } else {
            lw
        };
        // The sketcher's layers: their tops up to a pitch below the top.
        if z + lw > top_target - lh + 1e-6 && !out.is_empty() {
            break;
        }
        out.push(Layer {
            z0: z,
            rise,
            d: 0.0,
            f: 0.0,
            bead: lw,
            t: 0.0,
        });
        z += rise;
    }
    out
}

/// The layers. The outer wall rises its own layer height (`wall_layer`, the pitch unless set),
/// less where it leans (layer-height compensation, see `Geo::compensated`); the wings rise
/// their own, compensated for how far they lean in towards the socket. Each layer is one line,
/// so where one rises more than the other the wings' heights part from the wall's (falling
/// behind where the wings lean more, catching up where the wall does) and the arm between them
/// takes up the difference, its angle changing layer by layer. The thread keeps its own heights
/// (see `thread_offsets`): a pitch a layer on the socket, so the grip stays a true helix.
///
/// Both have the same number of layers, so if the wings fell so far behind that they couldn't
/// finish the grip before the wall reaches the top, the difference is scaled back (each wing
/// layer between its own and the wall's), as little as it takes.
fn layer_plan(s: &Settings, radius: &Radius) -> Vec<Layer> {
    let lh = s.layer;
    let comp = s.strength > 0.0;
    let wall = wall_layers(s, radius);
    let geo = Geo::new(s, radius, top_of(&wall, s));
    // The wings' own rise each layer, `share` of the way from the wall's to their own, at their
    // own heights (the wall's, and how far they've parted).
    let own_of = |share: f64| -> Vec<f64> {
        let mut d = 0.0;
        wall.iter()
            .map(|l| {
                let z = l.z0 + lh + d;
                let o = if comp && geo.rings_at(z) {
                    l.rise + share * (geo.compensated(geo.lean(z)) - l.rise)
                } else {
                    l.rise
                };
                d += o - l.rise;
                o
            })
            .collect()
    };
    let plan_with = |own: &[f64]| -> Vec<Layer> {
        let mut plan = wall.clone();
        // (How much of the lead the crease above the grip takes at a height: its depth's share of
        // the arm there, from the middle ring on the socket out to inside the wall.)
        let crease_share = |z: f64| -> f64 {
            let bw = wall_line(s);
            let arm = radius.at(z) - bw - (geo.r_grip + s.line);
            ((s.crease - bw).max(0.0) / arm.max(1.0)).clamp(0.05, 1.0)
        };
        thread_offsets(s, &mut plan, own, &crease_share);
        let mut drift = 0.0;
        for (l, o) in plan.iter_mut().zip(own) {
            l.t = l.d - drift;
            drift += o - l.rise;
        }
        plan
    };
    // Whether the thread gets through the grip: its rings up past the grip's top by the end.
    let z_gt = top_of(&wall, s) - s.hide;
    let finishes =
        |plan: &[Layer]| !comp || plan.iter().any(|l| l.z0 + lh + l.d > z_gt + lh - 1e-6);
    let full = own_of(1.0);
    let plan = plan_with(&full);
    if finishes(&plan) {
        return plan;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if finishes(&plan_with(&own_of(mid))) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    plan_with(&own_of(lo))
}

/// The top of the print in the sketcher's terms (its `zEnd`): a pitch over the last layer's top.
fn top_of(plan: &[Layer], s: &Settings) -> f64 {
    plan.last()
        .map_or(2.0 * s.layer, |l| l.z0 + wall_layer(s) + s.layer)
}

/// The rings' heights against the wall's, given each layer's own rise for the wings (`own`:
/// compensated for their lean; the wall's where there are none).
///
/// - Below the grip the rings rise their own: where they lean more than the wall they fall
///   behind it, where less they pull ahead.
/// - Over the layers before the grip the inner ring grows into the thread, its rise a step at a
///   time (a tenth of a pitch, over at least `FADE` layers) from its own to a pitch, its width
///   and bead with it.
/// - On the socket, and the layer under it, it rises exactly a pitch: the grip a true helix.
/// - Above the grip it eases back over as many layers.
///
/// Where the thread rises faster than the rings would, it pulls ahead: by the grip's top, by the
/// difference over every layer it took. That lead can be shared: the thread starts half (or all)
/// of it lower, whichever keeps the rings closest to the wall overall, so the arm between them
/// climbs least. To start lower, the rings rise less for a while below the grip; above it, they
/// rise less or more until the wall has caught up (as far as there are layers). Never more than
/// `SLOW` of their own rise less or more, easing in and out of that by a twentieth of it a layer.
fn thread_offsets(
    s: &Settings,
    plan: &mut [Layer],
    own: &[f64],
    crease_share: &dyn Fn(f64) -> f64,
) {
    let lh = s.layer;
    let lw = wall_layer(s);
    let n = plan.len();
    if n == 0 {
        return;
    }
    let z_end = top_of(plan, s);
    let z_gt = z_end - s.hide;
    let z_gb = z_gt - s.grip;
    let own_v: Vec<f64> = plan
        .iter()
        .enumerate()
        .map(|(k, l)| own.get(k).copied().unwrap_or(l.rise))
        .collect();
    let own_at = |k: usize| -> f64 { own_v.get(k).copied().unwrap_or(lw) };
    let least = (0..n).map(own_at).fold(lw, f64::min);
    let fade = (FADE as usize).max(((lh - least) / (0.1 * lh)).ceil().clamp(0.0, 1e4) as usize);
    let ramp_in = |i: usize| (i + 1) as f64 / (fade + 1) as f64;
    // The rings' lead from rising their own way, up to each layer.
    let mut drift = vec![0.0; n + 1];
    for k in 0..n {
        drift[k + 1] = drift[k] + own_at(k) - plan[k].rise;
    }
    // Where the rings are after easing in from layer `k`, `pre` further from their own way.
    let reach = |k: usize, pre: f64| -> Option<f64> {
        plan.get(k + fade)?;
        let climb: f64 = (0..fade)
            .map(|i| {
                let o = own_at(k + i);
                o + ramp_in(i) * (lh - o)
            })
            .sum();
        Some(plan[k].z0 + lh + drift[k] + pre + climb)
    };
    // Where they start easing in: the first layer from which, eased in, the thread reaches the
    // layer under the grip.
    let start = |pre: f64| {
        (0..n)
            .find(|&k| reach(k, pre).is_some_and(|z| z >= z_gb - 2.0 * lh))
            .unwrap_or(n)
    };
    // From easing in at `k_s`, `d0` from the wall: each layer's (lead at its start, thread
    // fraction, the rings' rise), up to where it has eased out.
    let rising = |k_s: usize, d0: f64| -> Vec<(f64, f64, f64)> {
        let mut out = Vec::new();
        let (mut d, mut phase, mut i) = (d0, 1, 0);
        for (k, l) in plan.iter().enumerate().skip(k_s) {
            let (wall, o) = (l.rise, own_at(k));
            if phase == 1 && i >= fade {
                phase = 2;
            }
            if phase == 2 && l.z0 + lh + d > z_gt + lh {
                phase = 3;
                i = 0;
            }
            if phase == 3 && i >= fade {
                break;
            }
            let (f, rise) = match phase {
                1 => (ramp_in(i), o + ramp_in(i) * (lh - o)),
                2 => (1.0, lh),
                _ => {
                    let t = 1.0 - ramp_in(i);
                    (t, o + t * (lh - o))
                }
            };
            out.push((d, f, rise));
            d += rise - wall;
            i += 1;
        }
        out
    };
    // The thread's own lead, from easing in at `k_s`.
    let lead_of = |k_s: usize, pre: f64| -> f64 {
        rising(k_s, drift.get(k_s).copied().unwrap_or(0.0) + pre)
            .iter()
            .enumerate()
            .map(|(i, r)| r.2 - own_at(k_s + i))
            .sum()
    };
    // How far the rings get from the wall with the thread started `pre` lower: the furthest.
    let furthest = |k_s: usize, pre: f64| -> f64 {
        let lead = lead_of(k_s, pre);
        let dk = drift.get(k_s).copied().unwrap_or(0.0) + pre;
        drift
            .iter()
            .take(k_s + 1)
            .fold(0.0_f64, |m, d| m.max(d.abs()))
            .max(dk.abs())
            .max((dk + lead).abs())
    };
    // Full wings move back out above the grip, and would lose a lead they carried: all of it
    // below. With a crease: none, half or all below, whichever stays closest (a few rounds, as
    // starting lower moves the start and so, on a changing wall, the lead).
    let shares: &[f64] = if s.crease_top {
        &[0.0, 0.5, 1.0]
    } else {
        &[1.0]
    };
    let mut best: Option<((f64, f64), f64, usize)> = None;
    for &share in shares {
        let mut pre = 0.0;
        let mut k_s = start(pre);
        for _ in 0..4 {
            pre = -lead_of(k_s, pre) * share;
            k_s = start(pre);
        }
        // (Then, as far as the wings' own drift allows, the thread's lead itself shared.)
        let dk = drift.get(k_s).copied().unwrap_or(0.0) + pre;
        let own_far = dk.abs().max((dk + lead_of(k_s, pre)).abs());
        let far = (furthest(k_s, pre), own_far);
        if best
            .is_none_or(|b| far.0 < b.0.0 - 1e-6 || (far.0 < b.0.0 + 1e-6 && far.1 < b.0.1 - 1e-6))
        {
            best = Some((far, pre, k_s));
        }
    }
    let (_, pre, k_s) = best.unwrap_or(((0.0, 0.0), 0.0, n));
    // Below: the rings rise less than their own to start `pre` lower at `k_s`, the less easing
    // in and out (a trapezoid over as few layers as it takes, before the easing in).
    let need = (-pre).max(0.0);
    let mut cut = vec![0.0; n];
    // (With no room below, the thread starts where the rings are.)
    if need > 1e-9 && k_s > 3 {
        let avail = k_s - 1;
        let trapezoid = |m: usize, cut: &mut [f64]| -> f64 {
            let mut sum = 0.0;
            for j in 0..m {
                let k = k_s - m + j;
                let w = own_at(k);
                // (Back to none the layer before the easing in.)
                let c = (SLOW * w)
                    .min(0.05 * w * (j + 1) as f64)
                    .min(0.05 * w * (m - 1 - j) as f64);
                cut[k] = c;
                sum += c;
            }
            sum
        };
        // As few layers as it takes (the sum grows with them: a binary search).
        let (mut lo, mut hi) = (2, avail.max(2));
        while lo < hi {
            let mid = (lo + hi) / 2;
            cut.iter_mut().for_each(|c| *c = 0.0);
            if trapezoid(mid, &mut cut) >= need {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        cut.iter_mut().for_each(|c| *c = 0.0);
        let sum = trapezoid(lo, &mut cut);
        if sum > need {
            for c in cut.iter_mut() {
                *c *= need / sum;
            }
        }
    }
    let mut below = Vec::with_capacity(k_s.min(n));
    let mut d = 0.0;
    for (k, c) in cut.iter().enumerate().take(k_s.min(n)) {
        below.push(d);
        d += own_at(k) - c - plan[k].rise;
    }
    let up = rising(k_s, d);
    for (l, &dk) in plan.iter_mut().zip(&below) {
        l.d = dk;
    }
    let mut k = k_s;
    for &(dk, f, rise) in &up {
        if let Some(l) = plan.get_mut(k) {
            l.d = dk;
            l.f = f;
            d = dk + rise - l.rise;
        }
        k += 1;
    }
    // Above: the wall catches up (from a layer level with it), whichever side the rings are.
    // With a crease there are no rings left, only the crease by the wall, taking its share of the
    // lead: it closes as fast as leaves each crease layer at least 75 % of its height (the rest
    // left for the wall moving in or out under it).
    let mut c = -0.05 * own_at(k.min(n.saturating_sub(1)));
    for (j, l) in plan.iter_mut().enumerate().skip(k) {
        l.d = d;
        let w = own.get(j).copied().unwrap_or(l.rise);
        let most = if s.crease_top {
            (0.25 / crease_share(l.z0 + lh)).clamp(SLOW, 0.8)
        } else {
            SLOW
        };
        let mag = d.abs();
        // Easing in, and out again in time to stop at none (with a crease, where there are no
        // rings left to step, four times as fast).
        let step = if s.crease_top { 0.2 } else { 0.05 } * w;
        c = if mag > 1e-9 {
            let tail = c * (c / step + 1.0) / 2.0;
            let next = if mag <= tail {
                (c - step).max(step.min(mag))
            } else {
                (c + step).min(most * w)
            };
            next.min(mag)
        } else {
            0.0
        };
        // The rings rise their own, less or more by `c`: the lead goes towards none.
        d += w - l.rise - d.signum() * c;
    }
}

/// How much of the thread's offset (and bead) a point at radius `r` takes: all of it from `lo`
/// in (the middle ring, with wings), none from a line inside the outer wall outwards (`hi`),
/// evenly between: the arm a straight climb, its angle no steeper anywhere than on average.
fn bridge(r: f64, lo: f64, hi: f64) -> f64 {
    ((hi - r) / (hi - lo).max(1e-9)).clamp(0.0, 1.0)
}

/// The line, as points (x, y, z) with z one layer below the nozzle (as `toolpath` wants it:
/// it prints a layer above its curve), and what came out.
pub fn thread_shade(s: &Settings) -> Result<(Vec<P3>, Info), String> {
    check(s)?;
    let lh = s.layer;
    // The wall's (and wings') line width, and the thread's.
    let bw = wall_line(s);
    let tw = s.line;
    let q = bw / 2.0;
    let radius = Radius::new(&s.profile, s.height, s.scale);
    let plan = layer_plan(s, &radius);
    // Too many points, said before making them: the outer wall alone, a turn a layer.
    let wall_points: f64 = plan
        .iter()
        .map(|l| TAU * radius.at(l.z0 + s.layer).abs() / s.step)
        .sum();
    if !wall_points.is_finite() || wall_points > 3_000_000.0 {
        return Err(
            "thread_shade: this shade makes too many points: a larger step or a smaller shade"
                .into(),
        );
    }
    let n = s.entries;
    let step = s.step;
    let phi0 = PI / 2.0 + s.rotation.to_radians();
    let th_s = phi0 - PI / n as f64;
    let dn = (s.neck_gap + bw) / 2.0;
    let tan_over = s.max_overhang.to_radians().tan();
    let mut info = Info {
        grip_full: true,
        min_rise: lh,
        min_ring_gap: lh,
        min_ring_share: 1.0,
        max_ring_share: 1.0,
        ..Info::default()
    };
    let mut out: Vec<P3> = Vec::new();
    let (mut d_prev, mut l_prev, mut rt_prev, mut ri_prev): (f64, f64, f64, f64) =
        (0.0, 0.0, 0.0, 0.0);
    // Heights in the sketcher's terms: layer k's `zc` is the top of its start (its base + a
    // layer); the top of the print one layer above the last.
    let z_end = top_of(&plan, s);
    let z_start = z_end - s.wing_zone;
    info.top = z_end;
    let r_grip = s.socket_d / 2.0 - s.engagement + tw / 2.0;
    let z_gt = z_end - s.hide;
    let z_gb = z_gt - s.grip;
    info.grip_bottom = z_gb;
    info.grip_top = z_gt;
    info.grip_r = r_grip;
    // The inner ring furthest out: inside the wall, its gap, the middle ring and its gap (`wi`,
    // `wm` the inner and middle rings' widths).
    let geo = Geo::new(s, &radius, z_end);
    let wall_ri_of = |ro: f64, wi: f64, wm: f64, gap: f64| geo.wall_ri(ro, wi, wm, gap);
    let wing_fn = &geo.wing;
    // The inner and middle rings' widths for a layer: both the thread's bead, as far as the
    // layer is a thread. The change from the wall's bead happens along the straight arm (the
    // slot) only, so every ring of the wings (and the turns at their tips) prints the same bead.
    let widths = |l: &Layer, _gap: f64| -> (f64, f64) {
        let w = bw + (tw - bw) * l.f;
        (w, w)
    };
    // Where the inner ring wants to be, at the thread's height `zc_r`: on the socket in the
    // grip; below it along the path from the socket (0) out to the wall (1); above it (or with
    // no path) leaning out at the max overhang; never further out than `wall_ri`.
    let ring_target = |zc_r: f64, wall_ri: f64| geo.target(zc_r, wall_ri);
    // The gap between the wing lines (`pin_gap`): none where the wings start, so the first
    // turn at each tip is a line wide and hangs at most half a line off the slot under it. It
    // opens a layer at a time, up to the setting, by what's left of a ring's step (at most half a
    // line, and what the max overhang allows) after the inner ring's own move that layer, so the
    // middle ring never steps out further than that. With `pockets` it closes again before the
    // grip, as fast as the max overhang allows on top of the inner ring's move: none all over
    // the socket, the rings there solid. It only opens as far as it can close in time.
    // (The leaf: by eye, no max overhang; half a line a layer, closing too.)
    let over = |l: &Layer| {
        if s.leaf {
            bw / 2.0
        } else {
            (l.rise * tan_over).max(0.0)
        }
    };
    let held: Vec<f64> = plan
        .iter()
        .map(|l| {
            let zc = l.z0 + lh;
            let (wi, wm) = widths(l, s.pin_gap);
            ring_target(zc + l.d, wall_ri_of(radius.at(zc), wi, wm, s.pin_gap))
        })
        .collect();
    // What's left for the gap each layer, opening (`.0`) and closing (`.1`). (A tenth kept back:
    // `held` is where the ring would be with the whole gap, and with less it sits a little
    // differently.)
    let budget: Vec<(f64, f64)> = plan
        .iter()
        .enumerate()
        .map(|(k, l)| {
            let moved = match (k.checked_sub(1).and_then(|j| held.get(j)), held.get(k)) {
                (Some(a), Some(b)) => (b - a).abs(),
                _ => 0.0,
            };
            (
                0.9 * (over(l).min(bw / 2.0) - moved).max(0.0),
                if s.leaf {
                    0.9 * over(l)
                } else {
                    0.9 * (over(l) - moved).max(0.0)
                },
            )
        })
        .collect();
    let mut gap_cap = vec![f64::INFINITY; plan.len()];
    if s.pockets {
        // On the socket (the inner ring's height in the grip, and the layer under it): none;
        // below it, no more than it can close by on the way up.
        let on_socket = |l: &Layer| {
            let zc_r = l.z0 + lh + l.d;
            zc_r >= z_gb - lh && zc_r <= z_gt
        };
        if let Some(first) = plan.iter().position(on_socket) {
            let mut c = 0.0;
            for k in (0..first).rev() {
                c += budget.get(k + 1).map_or(0.0, |b| b.1);
                if let Some(slot) = gap_cap.get_mut(k) {
                    *slot = c;
                }
            }
            for (slot, l) in gap_cap.iter_mut().zip(plan.iter()).skip(first) {
                if on_socket(l) {
                    *slot = 0.0;
                }
            }
        }
    }
    let mut gap_prev = 0.0;
    let mut rm_prev = 0.0;
    // The bridge: where it starts (the middle ring, see below) and ends, last layer.
    let mut lo_prev = r_grip;
    let mut below: Option<(f64, f64, f64, f64, f64)> = None;
    // The thread's part of the lead fades from the middle ring on the socket.
    let rm_thread = r_grip + tw + if s.pockets { 0.0 } else { s.pin_gap.max(0.0) };
    let t_at = |k: usize| plan.get(k).or(plan.last()).map_or(0.0, |l| l.t);
    // Per layer, for the checks: its height, outer radius, the innermost face, whether it has
    // wings on the socket.
    let mut checks: Vec<(f64, f64, f64, bool)> = Vec::new();
    let mut prev_ro: Option<(f64, f64)> = None;
    // (Past the last layer the lead carries on as it was.)
    let d_at = |k: usize| plan.get(k).or(plan.last()).map_or(0.0, |l| l.d);
    for (k, layer) in plan.iter().enumerate() {
        let (z0, rise) = (layer.z0, layer.rise);
        let zc = z0 + lh;
        // Where the thread is: the rings' heights (the wall's, plus the offset).
        let zc_r = zc + layer.d;
        let rise_next = plan.get(k + 1).map_or(0.0, |p| p.rise);
        // The thread: a helix in the grip, eased in below it and out above it.
        let thread = (1.0 - (z_gb - zc_r).max(zc_r - z_gt).max(0.0) / (FADE * lh)).clamp(0.0, 1.0);
        let ring_rise = rise + d_at(k + 1) - layer.d;
        // (The sketcher's pace: the slot by the wall's rise, the wings by their own.)
        let grow = rise * tan_over;
        let grow_wing = ring_rise.max(0.0) * tan_over;
        info.min_rise = info.min_rise.min(rise).min(ring_rise);
        let mut ro = radius.at(zc);
        if let Some((pz, pr)) = prev_ro {
            let lean = (ro - pr).abs().atan2(zc - pz).to_degrees();
            info.max_lean = info.max_lean.max(lean);
        }
        prev_ro = Some((zc, ro));
        let in_zone = n > 0 && zc >= z_start && zc <= z_end;
        let hi_l = (radius.at(zc) - bw).max(r_grip + bw);
        let gap = if l_prev > 0.0 {
            (gap_prev + budget.get(k).map_or(0.0, |b| b.0))
                .min(s.pin_gap)
                .min(gap_cap.get(k).copied().unwrap_or(0.0))
                .max(0.0)
        } else {
            0.0
        };
        let (wi, wm) = widths(layer, gap);
        let (mut rm, mut ri) = (0.0, 0.0);
        let mut ri_wanted: Option<f64> = None;
        if in_zone {
            if s.leaf && !(zc_r >= z_gb && zc_r <= z_gt) {
                // The leaf: the middle ring (the one the arm runs into) on its straight line,
                // the inner ring the gap inside it, so the gap opens and closes by the inner ring
                // moving (closing before the grip, it comes in more slowly, not the middle ring
                // swinging in onto it).
                let rm_wall = ro - (bw + wm) / 2.0 - s.wall_gap;
                rm = geo.middle(zc_r, rm_wall);
                ri = rm - (wi + wm) / 2.0 - gap;
            } else {
                let wall_ri = wall_ri_of(ro, wi, wm, gap);
                ri = ring_target(zc_r, wall_ri);
                ri_wanted = Some(ri);
                rm = ri + (wi + wm) / 2.0 + gap;
            }
            let req = rm + (wm + bw) / 2.0 + s.wall_gap;
            if ro < req - 1e-6 {
                ro = req;
                info.clamped = true;
            }
        }
        if in_zone && l_prev > 0.0 && ri_prev > 0.0 && !s.leaf {
            // The rings move by at most the overhang over their own rise (the thread's, where
            // it rises faster or slower than the wall).
            let own = k
                .checked_sub(1)
                .and_then(|j| plan.get(j))
                .map_or(rise, |b| b.rise + layer.d - b.d);
            let grow_r = own.max(0.0) * tan_over;
            let wall_ri = wall_ri_of(ro, wi, wm, gap);
            ri = wall_ri.min((ri_prev - grow_r).max((ri_prev + grow_r).min(ri)));
            rm = ri + (wi + wm) / 2.0 + gap;
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
            d = if l_prev > 0.0 || s.leaf {
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
            // (The leaf: along the wing zone at the rings' own height.)
            let u = if s.leaf {
                geo.along(zc_r)
            } else {
                (zc - z_start) / (z_end - z_start).max(1e-6)
            };
            let wv = wing_fn.at(u).clamp(0.0, 1.0);
            if d >= d_full - 1e-6 && wv > 0.002 {
                let target = l_min + wv * (l_max - l_min);
                l = if s.leaf {
                    target
                } else if l_prev > 0.0 {
                    target.min(l_prev * rt / rt_prev.max(1e-6) + grow_wing)
                } else {
                    l_min
                };
                // (Slowed: more than three pitches' worth behind.)
                if l < target - 3.0 * lh * tan_over {
                    info.limited = true;
                }
                kind = 2;
            } else {
                kind = usize::from(d > 0.05);
                l = 0.0;
            }
        }
        if kind == 2 && ri_prev > 0.0 {
            info.max_wing_step = info
                .max_wing_step
                .max((ri - ri_prev).abs())
                .max((rm - rm_prev).abs());
        }
        d_prev = if in_zone { d } else { 0.0 };
        l_prev = if kind == 2 { l } else { 0.0 };
        rt_prev = (rm + ri) / 2.0;
        ri_prev = if kind == 2 { ri } else { 0.0 };
        rm_prev = rm;
        gap_prev = if kind == 2 { gap } else { 0.0 };
        if kind == 2 {
            info.rings.push([zc_r, ri, rm, gap]);
        }
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
        let wall_ri_l = wall_ri_of(ro, wi, wm, gap);
        let follow_all = kind == 0
            || (kind == 2 && (ri - wall_ri_l).abs() < 1e-6 && (ri - r_grip).abs() >= 1e-6);
        let r_in = match kind {
            2 => rm,
            1 => ro - d,
            _ => ro,
        };
        let inner = if kind == 2 {
            ri - wi / 2.0
        } else {
            ro - d - bw / 2.0
        };
        checks.push((zc_r, ro, inner, kind == 2 && (ri - r_grip).abs() < 0.01));
        info.wall.push((ro, zc));
        let base = z0 + lh / 2.0;
        // (The slot, cutting in below the grip before the wings form, fades from where their
        // middle ring will be; a crease above the grip from where it was.)
        let lo_l = if kind == 2 || (kind == 1 && !crease_top && rm > 0.0) {
            rm.min(hi_l - 1.0)
        } else {
            lo_prev
        };
        let (len, slope, thin) = finish(
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
                // The bridge: along the straight arm only, from the middle ring (the rings, the
                // turns at their tips and everything in from them on their own heights and,
                // as far as the layer is a thread, the thread's bead) out to a line inside this
                // layer's outer wall (the wall on its own, compensated layers). Where the rings
                // move the arm moves with them, and its points don't sit exactly on the arm
                // below: each is extruded for the gap it's in.
                lo: lo_l,
                hi: hi_l,
                t: [layer.t, t_at(k + 1), t_at(k + 2)],
                lo_thread: rm_thread.min(hi_l - 1.0),
                below,
                bead: Bead {
                    f: layer.f,
                    wall: [bw, layer.bead],
                    thread: tw,
                },
            },
            !out.is_empty(),
            &mut out,
            &mut info.bead,
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
        info.length += len;
        info.max_bridge_slope = info.max_bridge_slope.max(slope);
        info.min_ring_gap = info.min_ring_gap.min(thin[0]);
        info.min_ring_share = info.min_ring_share.min(thin[1]);
        info.max_ring_share = info.max_ring_share.max(thin[2]);
        lo_prev = lo_l;
        below = Some((layer.d, layer.t, lo_l, hi_l, rise));
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
    // (The leaf: room for a few layers coming in below the grip; the sketcher's: its path.)
    info.zone_short = if s.leaf {
        s.wing_zone < s.hide + s.grip + 4.0 * lh
    } else {
        s.wing_zone < s.hide + s.grip + s.path_len
    };
    info.max_d = checks.iter().map(|c| c.1).fold(0.0, f64::max) * 2.0 + bw;
    info.bottom_d = checks.first().map_or(0.0, |c| c.1) * 2.0 + bw;
    // Heights: the sketcher's are bead centres; the nozzle runs half a layer above, and the
    // curve is drawn a layer below the nozzle.
    // (Never below the bed: the first layer's start can round a hair under it.)
    // (On the first layer each move is extruded for the gap over the bed, as a vase's first
    // turn: the nozzle a layer above the curve.)
    for (p, b) in out.iter_mut().zip(info.bead.iter_mut()) {
        p[2] = (p[2] - 1.5 * lh).max(-lh);
        b[1] = b[1].min((p[2] + lh).max(0.05));
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

/// What each point is extruded for: the wall's bead (width, layer) out by the wall; the wings'
/// (as far as the layer is a thread, `f`, the thread's width; else the wall's) for the gap
/// they're in; blending along the arm, as the heights.
struct Bead {
    f: f64,
    wall: [f64; 2],
    thread: f64,
}

/// The thread's offset from the wall at the turn's start, the next's and the one after (for
/// the step up at its end), and the bridge's radii (see `bridge`).
struct Thread {
    d: f64,
    d_next: f64,
    d_after: f64,
    lo: f64,
    hi: f64,
    /// Of `d`, `d_next` and `d_after`, the thread's part: it fades from `lo_thread` (the
    /// middle ring on the socket) rather than the layer's own middle ring.
    t: [f64; 3],
    lo_thread: f64,
    /// The layer below's lead at its start, its thread part, its bridge (`lo`, `hi`) and its
    /// rise, for the gap under each point.
    below: Option<(f64, f64, f64, f64, f64)>,
    /// What each point is extruded for.
    bead: Bead,
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
    beads: &mut Vec<[f64; 2]>,
    adj: impl Fn(f64, f64, f64) -> (f64, f64),
) -> (f64, f64, [f64; 3]) {
    let (z, len) = heights(rv, &h);
    // Each point's share of the thread's lead (0 where there's none).
    let mut lead = vec![0.0; z.len()];
    // The rings' heights with the whole lead, and with only the wings' own part of it.
    let ring_at = |d: [f64; 3]| -> Vec<f64> {
        let ring = Heights {
            base: h.base + d[0],
            rise: h.rise + d[1] - d[0],
            rise_next: if h.rise_next > 0.0 {
                h.rise_next + d[2] - d[1]
            } else {
                0.0
            },
            ..h
        };
        heights(rv, &ring).0
    };
    let whole = [t.d, t.d_next, t.d_after];
    let z = if whole.iter().any(|v| v.abs() > 1e-12) {
        let zr = ring_at(whole);
        let own = [t.d - t.t[0], t.d_next - t.t[1], t.d_after - t.t[2]];
        let zo = ring_at(own);
        for ((((l, &zw), &zt), &zd), (&x, &y)) in lead
            .iter_mut()
            .zip(&z)
            .zip(&zr)
            .zip(&zo)
            .zip(rv.xs.iter().zip(&rv.ys))
        {
            let r = x.hypot(y);
            *l = (zd - zw) * bridge(r, t.lo, t.hi) + (zt - zd) * bridge(r, t.lo_thread, t.hi);
        }
        z.iter().zip(&lead).map(|(&zw, &l)| zw + l).collect()
    } else {
        z
    };
    let skip = usize::from(skip_first);
    // The steepest the lead makes the line climb (the layer's own heights left out: the flat
    // layers' turns onto the helix, the step up at the seam).
    let mut slope: f64 = 0.0;
    for j in 1..rv.xs.len() {
        let run = (rv.xs[j] - rv.xs[j - 1]).hypot(rv.ys[j] - rv.ys[j - 1]);
        if run > 0.3 {
            slope = slope.max((lead[j] - lead[j - 1]).abs().atan2(run).to_degrees());
        }
    }
    // The thinnest gap under the bridge (the wings and the arm): in mm, and for its layer's rise.
    let mut thin = [f64::INFINITY, f64::INFINITY, 0.0];
    for ((&x, &y), &zj) in rv.xs.iter().zip(&rv.ys).zip(&z).skip(skip) {
        let b = &t.bead;
        let r = x.hypot(y);
        let tall = bridge(r, t.lo, t.hi);
        let tall_t = bridge(r, t.lo_thread, t.hi);
        // The gap under the point (half way round its turn): the wall's rise, and the change
        // in the lead it takes over the one below (each layer its own bridge).
        let share = |own: f64, thread: f64, lo: f64, hi: f64| {
            bridge(r, lo, hi) * own + bridge(r, t.lo_thread, hi) * thread
        };
        let mine = share(
            (t.d - t.t[0] + t.d_next - t.t[1]) / 2.0,
            (t.t[0] + t.t[1]) / 2.0,
            t.lo,
            t.hi,
        );
        let under = t.below.map_or(mine, |(dp, tp, lo, hi, _)| {
            share((dp - tp + t.d - t.t[0]) / 2.0, (tp + t.t[0]) / 2.0, lo, hi)
        });
        // (Half way round, the wall's gap is half this layer's rise and half the one below's.)
        let wall_gap = t.below.map_or(h.rise, |b| (b.4 + h.rise) / 2.0);
        let tall = tall.max(tall_t);
        let gap = (wall_gap + mine - under).clamp(0.05, 10.0);
        if tall > 0.01 {
            thin[0] = thin[0].min(gap);
            thin[1] = thin[1].min(gap / wall_gap.max(1e-9));
            // (Against the layer it should be: the wall's, blending into the wings' own.)
            let planned = wall_gap + tall * (t.d_next - t.below.map_or(t.d, |b| b.0)) / 2.0;
            thin[2] = f64::max(thin[2], gap / planned.max(0.05 * wall_gap));
        }
        // The wall's bead out by the wall (its layer, squeezed where it leans); the wings' for
        // the gap they're in (theirs aren't squeezed), blending along the arm.
        beads.push([
            b.wall[0] + (b.thread - b.wall[0]) * b.f * tall,
            b.wall[1] + (gap - b.wall[1]) * tall,
        ]);
        let (x, y) = adj(x, y, zj);
        out.push([x, y, zj]);
    }
    (len, slope, thin)
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
        s.wall_line,
        s.wall_layer,
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
    if s.wall_line < 0.0 || (s.wall_line > 0.0 && s.wall_line < 0.2) || s.wall_layer < 0.0 {
        return Err("thread_shade: `wall_line` must be 0 (the thread's) or at least 0.2 mm, `wall_layer` 0 (the pitch) or more".into());
    }
    // (Found by the fuzz test: a 4 × 10⁹ mm gap sent the rings far off and the line with them.)
    let gaps = [s.tip_gap, s.neck_gap, s.wall_gap, s.pin_gap, s.crease];
    if gaps.iter().any(|g| !(0.0..=1000.0).contains(g)) {
        return Err("thread_shade: the gaps and the crease must be 0 to 1000 mm".into());
    }
    if s.wall_line > 20.0 || s.line > 20.0 {
        return Err("thread_shade: `line` and `wall_line` must be at most 20 mm".into());
    }
    if s.wall_layer > 0.0 && s.wall_layer < 0.1 {
        return Err("thread_shade: `wall_layer` must be 0 (the pitch) or at least 0.1 mm".into());
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
            pockets: false,
            leaf: false,
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
            wall_line: 0.0,
            wall_layer: 0.0,
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

    /// The gaps under the bridge (the slot and the wing rings, inside the outer wall), from each
    /// point down to the nearest point of an earlier turn under it (within 0.6 mm sideways):
    /// the thinnest and the thickest. (The thickest is where the wings grow or move, as they
    /// always have: a new tip over nothing.)
    fn bridge_gaps(pts: &[P3], s: &Settings, radius: &Radius, from: f64) -> (f64, f64) {
        use std::collections::HashMap;
        let cell = |p: &P3| ((p[0] / 0.6).floor() as i64, (p[1] / 0.6).floor() as i64);
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in pts.iter().enumerate() {
            grid.entry(cell(p)).or_default().push(i);
        }
        let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
        for (i, p) in pts.iter().enumerate() {
            let r = p[0].hypot(p[1]);
            if p[2] < from || r > radius.at(p[2] + 1.5 * s.layer) - wall_line(s) - 1.0 {
                continue;
            }
            let (cx, cy) = cell(p);
            let mut best: Option<f64> = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for &j in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                        let q = &pts[j];
                        let gap = p[2] - q[2];
                        // Earlier in the line, under it, not the same turn.
                        if j + 50 < i && gap > 0.05 && (p[0] - q[0]).hypot(p[1] - q[1]) < 0.6 {
                            best = Some(best.map_or(gap, |b: f64| b.min(gap)));
                        }
                    }
                }
            }
            if let Some(g) = best {
                lo = lo.min(g);
                hi = hi.max(g);
            }
        }
        (lo, hi)
    }

    /// A leaning wall: compensated in every layer, the grip's included; the thread on the
    /// socket still a true helix a pitch a turn, starting level with the wall below the grip and
    /// growing into the pitch, the bridge between them climbing gently.
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

    /// The thread grows into its pitch a step at a time: over the layers where it's partly a
    /// thread, the rings' rise changes by at most a tenth of a pitch a layer (with a hair for the
    /// wings' own rise changing under it).
    fn assert_thread_eases_in(plan: &[Layer], s: &Settings) {
        for w in plan.windows(3) {
            if !(w[0].f > 0.0 && w[1].f > 0.0 && w[0].f < 1.0 && w[1].f < 1.0) {
                continue;
            }
            let r0 = w[0].rise + w[1].d - w[0].d;
            let r1 = w[1].rise + w[2].d - w[1].d;
            assert!((r1 - r0).abs() < 0.1 * s.layer + 0.05, "{r0} {r1}");
        }
    }

    /// On the socket in the grip: a true helix (its height less its angle's share of a pitch the
    /// same at every point, whole pitches apart).
    fn assert_true_helix(pts: &[P3], s: &Settings, info: &Info) {
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
                - if s.layers == Layers::Flat {
                    a.min(1.0)
                } else {
                    a
                }
        };
        let p0 = phase(ring[0]);
        for p in &ring {
            let t = phase(p) - p0;
            assert!((t - t.round()).abs() < 1e-6, "{:?} {p:?} {t}", s.layers);
        }
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
            // The wings keep to the wall's heights until they start, then rise their own (and
            // the thread its pitch): never further from the wall than the arm can climb.
            let z_start = top_of(&plan, &s) - s.wing_zone;
            assert!(
                plan.iter()
                    .filter(|l| l.z0 + s.layer < z_start)
                    .all(|l| l.d == 0.0)
            );
            assert!(plan.iter().any(|l| l.d.abs() > 0.5), "{plan:?}");
            assert_thread_eases_in(&plan, &s);
            let (pts, info) = thread_shade(&s).unwrap();
            assert!(info.grip_full);
            assert_true_helix(&pts, &s, &info);
            // No bead in the bridge is pressed into the one under it: every gap at least half
            // the wall's layer.
            let (lo, _) = bridge_gaps(&pts, &s, &radius, 2.0 * s.layer);
            assert!(lo > 0.5 * plan[0].rise, "{layers:?} {lo}");
            // (A narrow cone with its wings leaning in steeply: the arm climbs; the check says.)
            assert!(info.max_bridge_slope < 30.0, "{}", info.max_bridge_slope);
        }
    }

    /// A thinner wall and wings (3 mm lines, 1.5 mm layers) than the thread (5 mm, a 2.5 mm
    /// pitch): the thread on the socket still a true helix, extruded for its own bead; the wall
    /// for its own; the thread grows into it before the grip, a tenth of a pitch a layer.
    #[test]
    fn a_thinner_wall_than_the_thread() {
        for (layers, crease_top, strength) in [
            (Layers::Angle, true, 0.0),
            (Layers::Flat, true, 0.0),
            (Layers::Angle, true, 1.0),
            (Layers::Angle, false, 0.0),
        ] {
            let s = Settings {
                layers,
                crease_top,
                strength,
                wall_line: 3.0,
                wall_layer: 1.5,
                ..leaning()
            };
            let radius = Radius::new(&s.profile, s.height, s.scale);
            let plan = layer_plan(&s, &radius);
            assert!(plan.iter().all(|l| l.rise <= 1.5 + 1e-9));
            assert!(plan.len() > 160, "{}", plan.len());
            let ring: Vec<f64> = plan
                .windows(2)
                .map(|w| w[0].rise + w[1].d - w[0].d)
                .collect();
            assert!(ring.iter().any(|&r| (r - 2.5).abs() < 1e-12));
            assert_thread_eases_in(&plan, &s);
            let (pts, info) = thread_shade(&s).unwrap();
            assert_eq!(pts.len(), info.bead.len());
            assert!(info.grip_full);
            assert_true_helix(&pts, &s, &info);
            // The beads: the thread's on the socket in the grip, the wall's on the wall.
            for (p, b) in pts.iter().zip(&info.bead) {
                let r = p[0].hypot(p[1]);
                let zc = p[2] + 1.5 * s.layer;
                if (r - info.grip_r).abs() < 1e-6 && zc > info.grip_bottom && zc < info.grip_top {
                    assert!(
                        (b[0] - 5.0).abs() < 1e-9 && (b[1] - 2.5).abs() < 1e-9,
                        "{b:?}"
                    );
                }
                // (Its own layer, or less where it gives the wings layers.)
                if (r - radius.at(zc)).abs() < 1e-6 && p[2] > 0.0 {
                    assert!(
                        (b[0] - 3.0).abs() < 1e-9 && b[1] <= 1.5 + 1e-9 && b[1] > 0.5,
                        "{b:?}"
                    );
                }
                assert!(
                    b[0] >= 3.0 - 1e-9 && b[0] <= 5.0 + 1e-9 && b[1] <= 1.2 * 2.5,
                    "{b:?}"
                );
            }
            // The inner face on the socket is the thread's: half its width inside the grip
            // radius (the barrel less the engagement).
            assert!((info.grip_r - (20.0 - s.engagement + 2.5)).abs() < 1e-9);
            let (lo, hi) = bridge_gaps(&pts, &s, &radius, 2.0 * s.layer);
            // (With full wings the rings move back out above the grip while the wall catches
            // up: a little closer.)
            // (Against the thinnest wall layer it has, compensation squeezing them.)
            let least = if crease_top { 0.5 } else { 0.45 };
            let thinnest = plan.iter().map(|l| l.rise).fold(f64::INFINITY, f64::min);
            assert!(
                lo > least * thinnest,
                "{layers:?} {crease_top} {lo} {hi} {thinnest}"
            );
            assert!(info.min_ring_share > 0.5, "{}", info.min_ring_share);
            // A narrow shade (a 47 mm wall round a 22.5 mm thread at the grip's top, the arm
            // between them under 20 mm) climbs steeply; the check in the recipe says so. (The
            // owner's straight shade, a 72.5 mm wall: 19°.)
            assert!(info.max_bridge_slope < 50.0, "{}", info.max_bridge_slope);
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

    /// The owner's photos: flat layers step up at one place, a seam all the way up the wall. By
    /// angle (the default) the outer wall rises a layer a turn and never steps: between any two
    /// points along it, the height changes by at most its layer's rise × the turn between them,
    /// and a whole turn rises exactly that layer's rise; the thread on the socket stays a true
    /// helix, with a thinner wall too.
    #[test]
    fn the_wall_rises_a_layer_a_turn_with_no_seam() {
        for (wall_line, wall_layer, strength) in [(0.0, 0.0, 1.0), (3.0, 1.5, 1.0), (3.0, 1.5, 0.0)]
        {
            let s = Settings {
                layers: Layers::Angle,
                wall_line,
                wall_layer,
                strength,
                ..leaning()
            };
            let radius = Radius::new(&s.profile, s.height, s.scale);
            let plan = layer_plan(&s, &radius);
            let most = plan.iter().map(|l| l.rise).fold(0.0, f64::max);
            let (pts, info) = thread_shade(&s).unwrap();
            let on_wall =
                |p: &P3| (p[0].hypot(p[1]) - radius.at(p[2] + 1.5 * s.layer)).abs() < 1e-6;
            let mut pairs = 0;
            for w in pts.windows(2) {
                if !(on_wall(&w[0]) && on_wall(&w[1])) || w[0][2] < 0.0 {
                    continue;
                }
                let turn = (w[1][1].atan2(w[1][0]) - w[0][1].atan2(w[0][0])).rem_euclid(TAU);
                let turn = turn.min(TAU - turn) / TAU;
                assert!(
                    (w[1][2] - w[0][2]).abs() <= most * turn + 1e-9,
                    "a step on the wall: {:?} {:?}",
                    w[0],
                    w[1]
                );
                pairs += 1;
            }
            assert!(pairs > 5_000, "{pairs}");
            // A whole turn: from the seam round to the seam, the layer's rise.
            let th_s = PI / 2.0 - PI / 3.0;
            let seam: Vec<f64> = pts
                .iter()
                .filter(|p| {
                    on_wall(p)
                        && ((p[1].atan2(p[0]) - th_s).rem_euclid(TAU))
                            .min(TAU - (p[1].atan2(p[0]) - th_s).rem_euclid(TAU))
                            < 1e-9
                })
                .map(|p| p[2])
                .collect();
            assert!(seam.len() > 50, "{}", seam.len());
            // (A seam point is its layer's start, a layer below the curve: `z0 − layer`.)
            for w in seam.windows(2) {
                let Some(l) = plan.iter().find(|l| (l.z0 - s.layer - w[0]).abs() < 1e-6) else {
                    continue;
                };
                assert!(
                    (w[1] - w[0] - l.rise).abs() < 1e-9,
                    "{} {}",
                    w[1] - w[0],
                    l.rise
                );
            }
            assert_true_helix(&pts, &s, &info);
        }
    }

    /// The owner's photo: the first turn at each wing tip hung a whole line off the slot under
    /// it (with a 5 mm gap it was 10 mm across). The gap now opens from none: the first turn is a
    /// line across; it grows by at most half a line a layer, and the middle ring never steps
    /// further than the inner ring or half a line. With pockets (the default) it closes again
    /// before the grip: none on any layer on the socket.
    #[test]
    fn the_wing_gap_opens_from_none_and_closes_on_the_socket() {
        for (pockets, line, max_overhang) in [
            (true, 0.0, 45.0),
            (true, 3.2, 45.0),
            (false, 0.0, 45.0),
            (true, 0.0, 70.0),
        ] {
            let s = Settings {
                pockets,
                wall_line: line,
                wall_layer: if line > 0.0 { 1.5 } else { 0.0 },
                max_overhang,
                pin_gap: 5.0,
                ..leaning()
            };
            let w = wall_line(&s);
            let (_, info) = thread_shade(&s).unwrap();
            let rings = &info.rings;
            assert!(rings.len() > 20, "{}", rings.len());
            // The first: no gap, the turn a line across.
            let [_, ri, rm, gap] = rings[0];
            assert!(gap == 0.0 && (rm - ri - w).abs() < 1e-9, "{:?}", rings[0]);
            // It opens to the setting.
            assert!(rings.iter().any(|r| (r[3] - 5.0).abs() < 1e-9), "{rings:?}");
            // Opening: at most half a line a layer, the middle ring stepping out no further
            // than the inner ring or that. Closing: as fast as the max overhang allows (over the
            // tallest layer), the middle ring no further than the inner ring or that. (The plan
            // is from where the rings would be with the whole gap: a hair over.)
            let over = s.layer * s.max_overhang.to_radians().tan();
            for p in rings.windows(2) {
                let (a, b) = (p[0], p[1]);
                let (di, dm) = ((b[1] - a[1]).abs(), (b[2] - a[2]).abs());
                if b[3] > a[3] {
                    assert!(b[3] - a[3] <= w / 2.0 + 1e-9, "{a:?} {b:?}");
                    assert!(dm <= di.max(w / 2.0) * 1.03 + 1e-6, "{a:?} {b:?}");
                } else {
                    assert!(a[3] - b[3] <= over + 1e-9, "{a:?} {b:?}");
                    assert!(dm <= di.max(over) * 1.03 + 1e-6, "{a:?} {b:?}");
                }
            }
            // On the socket.
            let on: Vec<&[f64; 4]> = rings
                .iter()
                .filter(|r| r[0] >= info.grip_bottom && r[0] <= info.grip_top)
                .collect();
            assert!(on.len() > 5, "{}", on.len());
            for r in on {
                if pockets {
                    assert!(r[3] == 0.0, "{r:?}");
                } else {
                    assert!((r[3] - 5.0).abs() < 1e-9, "{r:?}");
                }
            }
        }
    }

    /// A sphere (the owner's), the leaf's wings.
    fn owners_sphere() -> Settings {
        let prof = [
            [0.0, 90.5],
            [0.115748, 136.821236],
            [0.270994, 168.06927],
            [0.448634, 180.801597],
            [0.629097, 173.615533],
            [0.792504, 147.302746],
            [0.920851, 104.762039],
            [1.0, 50.68],
        ];
        Settings {
            profile: prof.iter().map(|p| h(p[0], p[1])).collect(),
            height: 330.0,
            entries: 5,
            wing_zone: 202.5,
            wing: vec![h(0.0, 0.0), h(0.215, 0.995), h(0.54, 1.0), h(1.0, 1.0)],
            wall_gap: 5.0,
            pin_gap: 0.0,
            grip: 28.5,
            hide: 35.0,
            crease: 15.0,
            layers: Layers::Angle,
            strength: 1.0,
            pockets: true,
            leaf: true,
            ..straight()
        }
    }

    /// The leaf: the wings come in from the wall in a straight line to the socket, as wide as
    /// the leaf says, nothing slowed by a max overhang.
    #[test]
    fn the_leaf_is_drawn_as_set() {
        let s = Settings {
            strength: 0.0,
            ..owners_sphere()
        };
        let (pts, info) = thread_shade(&s).unwrap();
        assert!(info.grip_full && !info.limited && !info.path_limited);
        assert_true_helix(&pts, &s, &info);
        // Below the grip, the inner ring on a straight line from where the wings start (by the
        // wall) to the socket: its radius against height a line.
        let below: Vec<&[f64; 4]> = info
            .rings
            .iter()
            .filter(|r| r[0] < info.grip_bottom - 3.0 * s.layer)
            .collect();
        assert!(below.len() > 20, "{}", below.len());
        let slope = |a: &[f64; 4], b: &[f64; 4]| (b[1] - a[1]) / (b[0] - a[0]);
        let (a, b) = (below[below.len() / 3], below[2 * below.len() / 3]);
        let whole = slope(a, b);
        assert!(whole < 0.0);
        for w in below.windows(2).skip(below.len() / 4) {
            assert!(
                (slope(w[0], w[1]) - whole).abs() < 0.02,
                "{} {whole}",
                slope(w[0], w[1])
            );
        }
    }

    /// The owner's Sphere20test: the wall and the wings compensated each at its own rate, the
    /// arm between them taking up the difference. At the equator the wall hardly leans and keeps
    /// its own layers while the wings, leaning in, take thinner ones and fall behind; the thread
    /// still a true helix through the whole grip, the arm under 15°.
    #[test]
    fn the_wings_and_the_wall_compensated_each_their_own() {
        let prof = [
            [0.0, 90.5],
            [0.115748, 136.821236],
            [0.270994, 168.06927],
            [0.448634, 180.801597],
            [0.629097, 173.615533],
            [0.792504, 147.302746],
            [0.920851, 104.762039],
            [1.0, 50.68],
        ];
        let s = Settings {
            profile: prof.iter().map(|p| h(p[0], p[1])).collect(),
            height: 330.0,
            entries: 5,
            wing_zone: 202.5,
            wing: vec![
                h(0.0, 0.0),
                h(0.29919, 0.930769),
                h(0.527585, 1.0),
                h(1.0, 1.0),
            ],
            wall_gap: 5.0,
            pin_gap: 0.0,
            grip: 28.5,
            hide: 35.0,
            crease: 15.0,
            layers: Layers::Angle,
            strength: 0.83,
            steep: 0.8,
            pivot: 73.0,
            pockets: true,
            leaf: true,
            ..straight()
        };
        let radius = Radius::new(&s.profile, s.height, s.scale);
        let plan = layer_plan(&s, &radius);
        let (pts, info) = thread_shade(&s).unwrap();
        assert!(info.grip_full);
        assert_true_helix(&pts, &s, &info);
        // At the equator (the wall upright there), the wall its own: a whole layer.
        let equator = plan
            .iter()
            .find(|l| (l.z0 + s.layer - 150.0).abs() < 1.5)
            .unwrap();
        assert!((equator.rise - s.layer).abs() < 0.02, "{}", equator.rise);
        // The wings their own there: thinner, so they fall behind the wall (their own part of
        // the lead, `d − t`, below zero), by up to some 20 mm before the socket.
        let k = plan.iter().position(|l| std::ptr::eq(l, equator)).unwrap();
        let own = equator.rise + (plan[k + 1].d - plan[k + 1].t) - (equator.d - equator.t);
        assert!(own < s.layer - 0.3, "{own}");
        let behind = plan.iter().map(|l| l.d - l.t).fold(0.0, f64::min);
        assert!(behind < -15.0 && behind > -30.0, "{behind}");
        assert!(info.max_bridge_slope < 15.0, "{}", info.max_bridge_slope);
        assert!(info.min_ring_share > 0.5, "{}", info.min_ring_share);
    }

    /// The owner's half sphere: with a gap closing before the socket, the leaf's middle ring
    /// (the one the arm runs into) keeps to its straight line and the inner ring moves out to it,
    /// coming in more slowly (the middle ring swung in onto it, 5.6 mm a layer).
    #[test]
    fn the_leaf_closes_its_gap_with_the_inner_ring() {
        let s = Settings {
            pin_gap: 5.0,
            ..owners_sphere()
        };
        let (_, info) = thread_shade(&s).unwrap();
        let mut closing = 0;
        for w in info.rings.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b[3] < a[3] - 1e-9 && b[0] < info.grip_bottom {
                closing += 1;
                let (di, dm) = ((b[1] - a[1]).abs(), (b[2] - a[2]).abs());
                assert!(di < dm, "{a:?} {b:?}");
            }
        }
        assert!(closing > 0);
        assert!(
            info.rings
                .iter()
                .filter(|r| r[0] >= info.grip_bottom && r[0] <= info.grip_top)
                .all(|r| r[3] == 0.0)
        );
    }

    /// Found by the fuzz test: a radius of 4 × 10⁹ mm in the profile took a minute to say it
    /// was too big (it made millions of points first); now it says so at once.
    #[test]
    fn too_big_is_said_at_once() {
        let mut s = straight();
        s.profile[1].v = 4_294_967_296.0;
        let t = std::time::Instant::now();
        assert!(thread_shade(&s).is_err_and(|e| e.contains("too many points")));
        assert!(t.elapsed().as_secs_f64() < 1.0);
    }

    /// Found by the fuzz test: a wall gap of 4 × 10⁹ mm ran out of memory; now it's said.
    #[test]
    fn huge_gaps_are_errors() {
        for leaf in [false, true] {
            let s = Settings {
                wall_gap: 4_294_967_296.0,
                leaf,
                ..straight()
            };
            assert!(thread_shade(&s).is_err_and(|e| e.contains("gaps")));
        }
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
