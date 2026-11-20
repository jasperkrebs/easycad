//! A KUKA robot program (KRL) for a toolpath: the KR50 R2500 pellet cell of the SPHR handover
//! (`SPHR_Full_Print_Handover-2/`, KRC5, KSS 8.7).
//!
//! A robot program drives a real machine, so this follows the handover's own programs
//! (`Globe_E27_30_Layers/SPHR.SRC`, the 30-layer test) line for line where they do the same
//! thing: the start (frames, load, heaters and fans over EthernetKRL, a halt, the move to the
//! purge point, a second halt, the purge), the move onto the print, the spline restrictions and
//! acceleration, extruder changes as triggers at the start of a spline segment calling a small
//! subroutine each, and the end (extruder and heaters off, lift, nozzle upright, fans off, the
//! spline settings put back). What's EasyCAD's own:
//!
//! - **Where:** each point is the recipe origin's world position (`origin`, its z the bed) plus
//!   the toolpath point, written in the base frame `base` (as the handover subtracts its
//!   TR_BASE). The nozzle points along the toolpath's `axis` (`tilt`), upright without one; the
//!   tool's roll keeps world −Y across the nozzle (the handover's "smoothed roll"). The bed is
//!   the printer's bed setting around the origin (`BED`, `CENTER`: the handover's bed).
//! - **How fast:** each move at its own speed (at a set flow: the flow ÷ its bead), at most
//!   `max_speed` and slow enough that the wrist turns at most `max_turn` °/s; written to 0.1
//!   mm/s, rounded down.
//! - **The extruder:** a screw command of `screw` units per mm³/s of what the move lays down
//!   (bead × extrusion × speed × multiplier), changed only when it moves more than `step` %.
//! - **Fewer points:** within a stretch of the same speed and command, a point is left out where
//!   the straight line past it stays within `tolerance` mm of it and the nozzle within 0.5° (the
//!   spline then runs smoothly through the rest); `tolerance` 0 keeps every point.
//! - **Blocks:** at most `block` segments per SPLINE block, blocks joined with `C_SPL`.
//! - **Joints:** every pose is worked out on the handover's kinematics (`kuka`): where a joint
//!   would come within `wrist_margin` of its limit, the tool rolls about the nozzle (smoothly,
//!   at most `roll_rate` °/mm); where even that leaves a joint within 3° of its limit, the tilt
//!   is eased back there. A program with a pose out of reach or still that close is refused.
//!
//! Before writing, the toolpath is checked: one continuous line (no travels, no machine
//! events: not on the robot yet), every point above the bed and inside the bed's world
//! rectangle and height, the nozzle at most 60° from upright, and settings that make sense.

use crate::gcode::fixed;
use crate::geom::{P3, dot3};
use crate::kuka;
use crate::toolpath::Toolpath;
use serde::Deserialize;

/// The robot cell: everything the program needs besides the toolpath. Defaults are the SPHR
/// handover's (20 Nov 2026).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Robot {
    /// The program's name (the SRC/DAT files and `DEF`), letters, digits and `_`.
    pub program: String,
    /// The EthernetKRL channel the extruder listens on.
    pub channel: String,
    /// Where the recipe's origin is, in world mm; its z is the bed's surface.
    pub origin: [f64; 3],
    /// The base frame (X, Y, Z mm, A, B, C °) the program moves in (TR_BASE).
    pub base: [f64; 6],
    /// The tool frame (TR_TOOL).
    pub tool: [f64; 6],
    /// LOAD_DATA entry whose inertia the tool reuses, its mass (kg) and centre of mass (mm).
    pub load_data: u32,
    pub mass: f64,
    pub com: [f64; 3],
    /// Heater targets T0, T1, T2 (°C).
    pub heat: [f64; 3],
    /// Part fan command (0–255) from the second layer, and the firecap fan's (FAN2).
    pub part_fan: f64,
    pub firecap_fan: f64,
    /// The purge: where (world mm), how long (s), at which screw command.
    pub purge_at: [f64; 3],
    pub purge_seconds: f64,
    pub purge_command: f64,
    /// From the purge to above the print, and down onto it (mm/s).
    pub transfer_speed: f64,
    pub approach_speed: f64,
    /// Screw command units per mm³/s.
    pub screw: f64,
    /// The extruder command changes only when it moves more than this (%).
    pub step: f64,
    /// Fastest print move (mm/s) and fastest wrist turn while printing (°/s).
    pub max_speed: f64,
    pub max_turn: f64,
    /// How far a left-out point may be from the line past it (mm); 0 keeps every point.
    pub tolerance: f64,
    /// Most segments in one SPLINE block.
    pub block: usize,
    /// Stop printing this high above the bed (mm), then lift and finish: a test print. 0: all.
    pub stop_at: f64,
    /// The lift at the end (mm), and how fast the nozzle turns upright there (°/s).
    pub lift: f64,
    pub upright_speed: f64,
    /// Keep every joint (A1–A5) at least this far inside its limits (°), rolling the tool about
    /// the nozzle where the wrist would come closer.
    pub wrist_margin: f64,
    /// How fast that roll may change along the path (°/mm).
    pub roll_rate: f64,
}

impl Default for Robot {
    fn default() -> Self {
        Robot {
            program: "EASYCAD".into(),
            channel: "EthernetKRL_Config".into(),
            origin: [1088.304936, -115.143158, -659.0],
            base: [0.0, 50.0, -0.3, 0.0, 0.0, 0.0],
            tool: [-508.0, 3.5, 139.34, -180.0, -90.0, 0.0],
            load_data: 6,
            mass: 18.0,
            com: [-90.65, 5.95, 164.92],
            heat: [220.0, 190.0, 70.0],
            part_fan: 255.0,
            firecap_fan: 5.0,
            purge_at: [1088.304936, 134.856842, -590.0],
            purge_seconds: 20.0,
            purge_command: 12000.0,
            transfer_speed: 100.0,
            approach_speed: 20.0,
            // The handover's wall: 300 units per mm/s with a 4.5 × 2.5 mm bead (9.91 mm²).
            screw: 30.0,
            step: 2.0,
            max_speed: 25.0,
            max_turn: 30.0,
            tolerance: 0.2,
            block: 1000,
            stop_at: 0.0,
            lift: 50.0,
            upright_speed: 30.0,
            wrist_margin: 10.0,
            roll_rate: 1.5,
        }
    }
}

/// The program's two files.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub name: String,
    pub src: String,
    pub dat: String,
    /// Segments, extruder changes and the print's time (s), for the app.
    pub segments: usize,
    pub seconds: f64,
    /// The joints' least distance to their limits over the program (°), and which (1: A1).
    pub joint_margin: f64,
    pub joint: usize,
    /// How many points the tool is rolled at to keep the wrist inside its limits, and the most.
    pub rolled: usize,
    pub max_roll: f64,
    /// How many points have their tilt eased back to keep the joints inside their limits.
    pub relieved: usize,
}

/// The most the nozzle may lean from upright (°).
const MAX_TILT: f64 = 60.0;
const UP: P3 = [0.0, 0.0, 1.0];
/// The handover's orientation limits while moving and its spline acceleration (CP in m/s²).
const ORI_ACC: &str = "150.0";
const ORI_VEL: &str = "200.0";
const SPLINE_ACC: &str = "CP 0.050";
/// The approach starts this high above the first point (mm), as the handover's.
const APPROACH_HEIGHT: f64 = 5.0;
/// To the purge point and the lift at the end (mm/s), as the handover's.
const MOVE_SPEED: f64 = 50.0;
/// Left-out points: at most this many in a row, and the nozzle within this of a straight turn.
const MAX_SKIP: usize = 32;
const ANGLE_TOLERANCE: f64 = 0.5;
/// Tilt relief: how far along the path each way (mm), how much tilt is kept at the point, and
/// how many times it may be applied.
const RELIEF: f64 = 40.0;
const RELIEF_FACTOR: f64 = 0.75;
const MAX_RELIEF: usize = 30;
/// The least any joint may come to its limit anywhere in the program (°): closer, the tilt is
/// eased back there.
const MIN_MARGIN: f64 = 3.0;

/// Writes the KRL program for `path` (`multiplier`: the printer's extrusion multiplier).
/// The bed of the SPHR cell (1200 × 2800 mm, 1000 mm of build height assumed) and where the
/// recipe origin sits on it, as the printer settings give a bed: the handover's print centre.
pub const BED: [f64; 3] = [1200.0, 2800.0, 1000.0];
pub const CENTER: [f64; 2] = [375.0, 1750.0];

/// Writes the KRL program for `path`. `bed` and `center` are the printer's bed (width, depth,
/// height) and where the recipe origin sits on it, as for G-code: every point must be over the
/// bed and under its height. `multiplier`: the printer's extrusion multiplier.
pub fn write(
    path: &Toolpath,
    robot: &Robot,
    bed: [f64; 3],
    center: [f64; 2],
    multiplier: f64,
    title: &str,
) -> Result<Program, String> {
    check_settings(robot, multiplier)?;
    if path.points.len() < 2 {
        return Err("there's nothing to print".into());
    }
    if !path.axis.is_empty() && path.axes().is_none() {
        return Err("the nozzle's tilt was lost by a step after `tilt`: put `tilt` last".into());
    }
    if path
        .points
        .iter()
        .skip(1)
        .enumerate()
        .any(|(k, _)| path.travel.get(k + 1).copied().unwrap_or(false))
    {
        return Err(
            "the robot prints one continuous line for now: this toolpath has travel moves".into(),
        );
    }
    if !path.events.is_empty() {
        return Err("the robot doesn't take machine events (fans, pauses, temperatures) from the recipe yet".into());
    }
    let axes: Vec<P3> = match path.axes() {
        Some(a) => a.to_vec(),
        None => vec![UP; path.points.len()],
    };
    // A test print stops at the first point above `stop_at`.
    let end = if robot.stop_at > 0.0 {
        path.points
            .iter()
            .position(|p| p[2] > robot.stop_at)
            .unwrap_or(path.points.len())
    } else {
        path.points.len()
    };
    if end < 2 {
        return Err(format!(
            "nothing is printed below {} mm",
            fixed(robot.stop_at, 1)
        ));
    }
    let o = robot.origin;
    let world = |i: usize| {
        let p = path.points[i];
        [o[0] + p[0], o[1] + p[1], o[2] + p[2]]
    };
    for (i, axis) in axes.iter().enumerate().take(end) {
        let p = path.points[i];
        let w = world(i);
        if !(p.iter().all(|v| v.is_finite())) || p[2] < -1e-9 {
            return Err(format!(
                "point {} is below the bed (z {})",
                i + 1,
                fixed(p[2], 2)
            ));
        }
        let (bx, by) = (p[0] + center[0], p[1] + center[1]);
        if !(0.0..=bed[0]).contains(&bx) || !(0.0..=bed[1]).contains(&by) || p[2] > bed[2] {
            return Err(format!(
                "point {} is off the bed (world X {} Y {} Z {})",
                i + 1,
                fixed(w[0], 1),
                fixed(w[1], 1),
                fixed(w[2], 1)
            ));
        }
        let lean = axis[2].clamp(-1.0, 1.0).acos().to_degrees();
        if !lean.is_finite() || lean > MAX_TILT + 1e-9 {
            return Err(format!(
                "point {}: the nozzle leans {}° (at most {MAX_TILT}°)",
                i + 1,
                fixed(lean, 1)
            ));
        }
    }

    // The motion: where a pose would be past a joint limit even with the tool rolled, the tilt
    // is eased back around it (to 75 % of itself at the point, less with distance, over
    // `RELIEF` mm of path each way) and the motion worked out again.
    let mut axes = axes;
    let mut relieved = vec![false; end];
    let mut attempts = 0;
    let Plan {
        rolls,
        speed,
        held,
        fan_at,
        keep,
        least,
    } = loop {
        match plan(path, &axes, end, robot, &world, multiplier) {
            Ok(p) => break p,
            Err(failures) => {
                attempts += 1;
                if attempts > MAX_RELIEF {
                    let first = failures.into_iter().next().map(|f| f.1);
                    return Err(first.unwrap_or_default());
                }
                // Each failing point once (several poses between two kept points fail together).
                let mut done = std::collections::BTreeSet::new();
                for (at, _) in failures {
                    if done.insert(at) {
                        relieve(path, &mut axes, &mut relieved, at, end);
                    }
                }
            }
        }
    };
    let orientation = |i: usize| tool_rotation(axes[i], rolls[i]);
    let purge_pose = (robot.purge_at, tool_rotation(UP, 0.0));
    let first = world(0);
    let above = [first[0], first[1], first[2] + APPROACH_HEIGHT];
    let last = world(end - 1);
    let lifted = [last[0], last[1], last[2] + robot.lift];

    let prog = robot.program.as_str();
    let ch = robot.channel.as_str();
    let send = |payload: String| format!("RET = EKI_SEND(\"{ch}\", \"{payload}#\")");
    let set = |c: i64| send(format!("CHANGE_SPEED1 {c}"));
    let pose = |i: usize| frame(robot, world(i), &orientation(i));
    let mut src: Vec<String> = Vec::with_capacity(keep.len() * 2 + 100);
    src.push("&ACCESS RVP".into());
    src.push(format!("DEF {prog}()"));
    // KRL files are ASCII: anything else in the title becomes "?".
    let title: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '?'
            }
        })
        .collect();
    src.push(format!("; EasyCAD: {title}"));
    src.push("DECL EKI_STATUS RET".into());
    src.push("DECL SPL_VEL_MODE EC_MODE".into());
    src.push("DECL SPL_VEL_RESTR_STRUC EC_SAVED, EC_RESTR".into());
    src.push("BAS (#INITMOV,0)".into());
    src.push(format!("$ACC.ORI1 = {ORI_ACC}"));
    src.push(format!("$ACC.ORI2 = {ORI_ACC}"));
    src.push(format!("$VEL.ORI1 = {ORI_VEL}"));
    src.push(format!("$VEL.ORI2 = {ORI_VEL}"));
    src.push("$ADVANCE = 3".into());
    src.push(send("STOP_EXTRUDE".into()));
    src.push("$TOOL = TR_TOOL".into());
    src.push(format!(
        "; Reuse tool {} inertia and inertia-axis orientation; override mass and COM XYZ.",
        robot.load_data
    ));
    src.push(format!("$LOAD = LOAD_DATA[{}]", robot.load_data));
    src.push("$LOAD.M = TR_MASS".into());
    src.push("$LOAD.CM.X = TR_COM.X".into());
    src.push("$LOAD.CM.Y = TR_COM.Y".into());
    src.push("$LOAD.CM.Z = TR_COM.Z".into());
    src.push("$BASE = TR_BASE".into());
    src.push("$IPO_MODE = #BASE".into());
    src.push("$ORI_TYPE = #VAR".into());
    src.push("; SIMULATION ONLY - verify cell, cables and controller.".into());
    src.push("PTP $AXIS_ACT".into());
    for (k, t) in robot.heat.iter().enumerate() {
        src.push(send(format!("SET_TEMP T{k} {}", int(*t))));
    }
    src.push(send("SET_FAN1 0".into()));
    src.push(send(format!("SET_FAN2 {}", int(robot.firecap_fan))));
    src.push("HALT".into());
    // The purge, upright; then onto the print, the extruder running at the first command.
    src.push(format!("$VEL.CP = {}", mps(MOVE_SPEED)));
    src.push(format!("LIN {}", frame(robot, purge_pose.0, &purge_pose.1)));
    src.push("HALT".into());
    src.push("WAIT SEC 0".into());
    src.push(set(int(robot.purge_command)));
    src.push(send("START_EXTRUDE".into()));
    src.push(format!("WAIT SEC {}", int(robot.purge_seconds)));
    src.push(set(held[1]));
    src.push(format!("$VEL.CP = {}", mps(robot.transfer_speed)));
    src.push(format!("LIN {}", frame(robot, above, &orientation(0))));
    src.push(format!("$VEL.CP = {}", mps(robot.approach_speed)));
    src.push(format!("LIN {}", pose(0)));
    src.push("EC_MODE = $SPL_VEL_MODE".into());
    src.push("EC_SAVED = $SPL_VEL_RESTR".into());
    src.push("EC_RESTR = EC_SAVED".into());
    src.push("$SPL_VEL_MODE = #OPT".into());
    src.push("EC_RESTR.ORI_VEL = #ON".into());
    src.push("EC_RESTR.ORI_ACC = #ON".into());
    src.push("EC_RESTR.CART_ACC = #ON".into());
    src.push("$SPL_VEL_RESTR = EC_RESTR".into());
    let spline = format!("SPLINE WITH $ACC={{{SPLINE_ACC}, ORI1 {ORI_ACC}, ORI2 {ORI_ACC}}}");
    src.push(spline.clone());

    // Subroutines for the triggers, one per distinct instruction.
    let mut helpers: Vec<String> = Vec::new();
    let mut trigger = |src: &mut Vec<String>, instruction: String| {
        let n = match helpers.iter().position(|h| *h == instruction) {
            Some(n) => n + 1,
            None => {
                helpers.push(instruction);
                helpers.len()
            }
        };
        src.push(format!(
            "  TRIGGER WHEN PATH=0 ONSTART DELAY=0 DO {prog}_EVT_{n}() PRIO=-1"
        ));
    };
    let mut command_now = held[1];
    let mut prev = 0;
    let mut in_block = 0;
    let mut seconds = 0.0;
    let mut fan_done = robot.part_fan <= 0.0;
    for &k in &keep {
        if in_block == robot.block {
            src.push("ENDSPLINE C_SPL".into());
            src.push(spline.clone());
            in_block = 0;
        }
        // This segment runs over moves prev+1 ..= k (all one speed and command).
        if held[prev + 1] != command_now {
            command_now = held[prev + 1];
            trigger(&mut src, set(command_now));
        }
        if !fan_done && fan_at.is_some_and(|f| f <= prev + 1) {
            fan_done = true;
            trigger(&mut src, send(format!("SET_FAN1 {}", int(robot.part_fan))));
        }
        let v = speed[prev + 1];
        src.push(format!("  SPL {} WITH $VEL.CP={}", pose(k), mps(v)));
        seconds += (prev + 1..=k)
            .map(|i| dist(path.points[i - 1], path.points[i]))
            .sum::<f64>()
            / v;
        in_block += 1;
        prev = k;
    }
    src.push("ENDSPLINE".into());
    src.push(";END PRINT".into());
    src.push("WAIT SEC 0".into());
    src.push(send("STOP_EXTRUDE".into()));
    src.push(set(0));
    for k in 0..3 {
        src.push(send(format!("SET_TEMP T{k} 0")));
    }
    src.push(";LIFT".into());
    src.push(format!("$VEL.CP = {}", mps(MOVE_SPEED)));
    src.push(format!(
        "LIN {}",
        frame(robot, lifted, &orientation(end - 1))
    ));
    src.push(";UPRIGHT".into());
    src.push(format!("$VEL.ORI1 = {}", fixed(robot.upright_speed, 1)));
    src.push(format!("$VEL.ORI2 = {}", fixed(robot.upright_speed, 1)));
    src.push(format!(
        "LIN {}",
        frame(robot, lifted, &tool_rotation(UP, 0.0))
    ));
    src.push("WAIT SEC 0".into());
    src.push(send("SET_FAN1 0".into()));
    src.push(send("SET_FAN2 0".into()));
    src.push("$SPL_VEL_RESTR = EC_SAVED".into());
    src.push("$SPL_VEL_MODE = EC_MODE".into());
    src.push("END".into());
    for (n, h) in helpers.iter().enumerate() {
        src.push(String::new());
        src.push(format!("DEF {prog}_EVT_{}()", n + 1));
        src.push("DECL EKI_STATUS RET".into());
        src.push(h.clone());
        src.push("END".into());
    }
    let dat = vec![
        "&ACCESS RVP".to_string(),
        format!("DEFDAT {prog} PUBLIC"),
        format!("DECL FRAME TR_TOOL={}", frame6(robot.tool)),
        format!("DECL FRAME TR_BASE={}", frame6(robot.base)),
        "; Estimated uniform-density volume centroid, relative to flange.".into(),
        format!("DECL REAL TR_MASS={}", fixed(robot.mass, 1)),
        format!(
            "DECL FRAME TR_COM={}",
            frame6([robot.com[0], robot.com[1], robot.com[2], 0.0, 0.0, 0.0])
        ),
        "ENDDAT".into(),
    ];
    let crlf = |lines: Vec<String>| {
        let mut t = lines.join("\r\n");
        t.push_str("\r\n");
        t
    };
    Ok(Program {
        name: prog.to_string(),
        src: crlf(src),
        dat: crlf(dat),
        segments: keep.len(),
        seconds,
        joint_margin: least.0,
        joint: least.1 + 1,
        rolled: rolls.iter().filter(|r| r.abs() > 1e-9).count(),
        max_roll: rolls.iter().fold(0.0, |m: f64, r| m.max(r.abs())),
        relieved: relieved.iter().filter(|r| **r).count(),
    })
}

fn check_settings(r: &Robot, multiplier: f64) -> Result<(), String> {
    let name_ok = !r.program.is_empty()
        && r.program.len() <= 20
        && r.program
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && r.program
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !name_ok {
        return Err(
            "the program's name must start with a letter and have up to 20 letters, digits or _"
                .into(),
        );
    }
    if r.channel.is_empty()
        || !r
            .channel
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("the EthernetKRL channel's name must be letters, digits or _".into());
    }
    let finite = r
        .origin
        .iter()
        .chain(&r.base)
        .chain(&r.tool)
        .chain(&r.com)
        .chain(&r.purge_at)
        .all(|v| v.is_finite());
    if !finite {
        return Err("the robot's positions must be numbers".into());
    }
    let within = |name: &str, v: f64, lo: f64, hi: f64| {
        if v.is_finite() && (lo..=hi).contains(&v) {
            Ok(())
        } else {
            Err(format!(
                "the robot's {name} must be between {lo} and {hi}, got {v}"
            ))
        }
    };
    for (k, t) in r.heat.iter().enumerate() {
        within(&format!("heater T{k}"), *t, 0.0, 300.0)?;
    }
    within("part fan", r.part_fan, 0.0, 255.0)?;
    within("firecap fan", r.firecap_fan, 0.0, 255.0)?;
    within("purge time", r.purge_seconds, 0.0, 600.0)?;
    within("purge command", r.purge_command, 0.0, 20000.0)?;
    within("transfer speed", r.transfer_speed, 1.0, 250.0)?;
    within("approach speed", r.approach_speed, 1.0, 100.0)?;
    within("screw units per mm³/s", r.screw, 0.01, 1000.0)?;
    within("command step", r.step, 0.0, 50.0)?;
    within("fastest print move", r.max_speed, 0.5, 250.0)?;
    within("fastest wrist turn", r.max_turn, 1.0, 200.0)?;
    within("tolerance", r.tolerance, 0.0, 5.0)?;
    within("stop height", r.stop_at, 0.0, 5000.0)?;
    within("lift", r.lift, 0.0, 500.0)?;
    within("upright turn speed", r.upright_speed, 1.0, 200.0)?;
    within("joint margin", r.wrist_margin, 0.0, 45.0)?;
    within("roll rate", r.roll_rate, 0.1, 30.0)?;
    within("mass", r.mass, 0.0, 50.0)?;
    within("extrusion multiplier", multiplier, 0.01, 5.0)?;
    if r.load_data == 0 || r.load_data > 16 {
        return Err("the LOAD_DATA entry must be 1 to 16".into());
    }
    if r.block < 2 || r.block > 10_000 {
        return Err("a SPLINE block takes 2 to 10000 segments".into());
    }
    Ok(())
}

/// Whether the straight segment from point a to point b (and the nozzle turning evenly between
/// them) passes within `tol` mm of every point between (and within 0.5° of its nozzle).
fn covers(path: &Toolpath, axes: &[P3], rolls: &[f64], a: usize, b: usize, tol: f64) -> bool {
    let (pa, pb) = (path.points[a], path.points[b]);
    let ab = sub(pb, pa);
    let len2 = dot3(ab, ab);
    for k in a + 1..b {
        let p = path.points[k];
        let t = if len2 > 1e-18 {
            (dot3(sub(p, pa), ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = [pa[0] + ab[0] * t, pa[1] + ab[1] * t, pa[2] + ab[2] * t];
        if dist(p, q) > tol {
            return false;
        }
        let n = [
            axes[a][0] + (axes[b][0] - axes[a][0]) * t,
            axes[a][1] + (axes[b][1] - axes[a][1]) * t,
            axes[a][2] + (axes[b][2] - axes[a][2]) * t,
        ];
        if angle(n, axes[k]) > ANGLE_TOLERANCE
            || (rolls[a] + (rolls[b] - rolls[a]) * t - rolls[k]).abs() > ANGLE_TOLERANCE
        {
            return false;
        }
    }
    true
}

/// What `plan` works out: each point's roll, each move's speed and held screw command, where the
/// part fan comes on, the points kept, and the joints' least margin (degrees, joint index).
struct Plan {
    rolls: Vec<f64>,
    speed: Vec<f64>,
    held: Vec<i64>,
    fan_at: Option<usize>,
    keep: Vec<usize>,
    least: (f64, usize),
}

/// The motion for the nozzle directions `axes`: rolls, speeds, commands, the points kept, and
/// every pose checked on the robot's kinematics. Fails with every point where a pose is out of
/// reach or past a joint limit.
fn plan(
    path: &Toolpath,
    axes: &[P3],
    end: usize,
    robot: &Robot,
    world: &dyn Fn(usize) -> P3,
    multiplier: f64,
) -> Result<Plan, Vec<(usize, String)>> {
    // The tool's roll about the nozzle at each point (0: world −Y across it), where the wrist
    // would otherwise come closer than `wrist_margin` to a joint limit.
    let rolls = wrist_rolls(path, axes, end, robot, world);
    let orientation = |i: usize| tool_rotation(axes[i], rolls[i]);
    let purge_pose = (robot.purge_at, tool_rotation(UP, 0.0));
    let first = world(0);
    let above = [first[0], first[1], first[2] + APPROACH_HEIGHT];
    let last = world(end - 1);
    let lifted = [last[0], last[1], last[2] + robot.lift];

    // Each move's speed and extruder command.
    let mut speed = vec![0.0; end];
    let mut command = vec![0i64; end];
    for i in 1..end {
        let s = path.settings(i);
        let area = path.area(i);
        let seg = dist(path.points[i - 1], path.points[i]);
        let mut v = (s.feed_for(area) / 60.0 * path.speed_factor(i)).min(robot.max_speed);
        let turn = turn_between(&orientation(i - 1), &orientation(i));
        if turn > 1e-9 {
            v = v.min(robot.max_turn * seg / turn);
        }
        // To 0.1 mm/s, rounded down (never faster than allowed), at least 0.1.
        let v = ((v * 10.0 + 1e-9).floor() / 10.0).max(0.1);
        speed[i] = v;
        let volume = area * path.flow.get(i).copied().unwrap_or(1.0) * v * multiplier;
        command[i] = (volume * robot.screw + 0.5).floor() as i64;
    }
    // The extruder changes only when the command moves more than `step` %.
    let mut held = vec![0i64; end];
    let mut cur = command[1];
    for i in 1..end {
        if (command[i] - cur).abs() as f64 > cur as f64 * robot.step / 100.0 {
            cur = command[i];
        }
        held[i] = cur;
    }
    // The part fan comes on with the second layer: its first point, or (one line climbing
    // through layers) the first point a layer above the first.
    let fan_at = path
        .layer_starts
        .get(1)
        .copied()
        .filter(|&k| k > 0 && k < end)
        .or_else(|| {
            let z0 = path.points[1][2];
            (1..end).find(|&i| path.points[i][2] >= z0 + path.settings(i).layer - 1e-9)
        });

    // The points the program keeps: every change of speed or command, the fan, the end, and
    // wherever leaving one out would move the line more than `tolerance`.
    let mut keep: Vec<usize> = Vec::new();
    let mut a = 0;
    while a + 1 < end {
        let mut j = a + 1;
        if robot.tolerance > 0.0 {
            while j + 1 < end
                && j + 1 - a <= MAX_SKIP
                && speed[j + 1] == speed[a + 1]
                && held[j + 1] == held[a + 1]
                && fan_at != Some(j)
                && covers(path, axes, &rolls, a, j + 1, robot.tolerance)
            {
                j += 1;
            }
        }
        keep.push(j);
        a = j;
    }
    if keep.last() != Some(&(end - 1)) {
        keep.push(end - 1);
    }

    // Every pose the robot takes, in order, on the handover's kinematics: the purge, the
    // approach, each kept point and three between each two (the nozzle and its roll turning
    // evenly), the lift and the turn upright. Each must be reachable and inside the limits,
    // with A1, A4 and A6 counted on from the pose before (the robot turns them continuously),
    // the first from the handover's start pose.
    let mut least = (f64::INFINITY, 0usize);
    let mut joints = kuka::SEED;
    let mut failures: Vec<(usize, String)> = Vec::new();
    let mut check = |at: usize, what: &dyn Fn() -> String, p: P3, r: &kuka::M3| {
        let Some(j) = kuka::inverse(p, r, &robot.tool) else {
            failures.push((at, format!("{} is out of the robot's reach", what())));
            return;
        };
        joints = kuka::unwrap_near(j, &joints);
        let (m, axis) = kuka::margin(&joints);
        if m < MIN_MARGIN {
            failures.push((
                at,
                format!(
                    "at {} A{} would be {}° from its limit (at least {MIN_MARGIN}°), even with the tool rolled and the tilt eased: lower the tilt there",
                    what(),
                    axis + 1,
                    fixed(m, 1)
                ),
            ));
        }
        if m < least.0 {
            least = (m, axis);
        }
    };
    let at = |i: usize| move || format!("point {} ({} mm up)", i + 1, fixed(path.points[i][2], 1));
    check(0, &|| "the purge point".into(), purge_pose.0, &purge_pose.1);
    check(
        0,
        &|| "above the first point".into(),
        above,
        &orientation(0),
    );
    check(0, &at(0), world(0), &orientation(0));
    let mut from = 0;
    for &k in &keep {
        for f in [0.25, 0.5, 0.75] {
            let (a, b) = (world(from), world(k));
            let p = [
                a[0] + (b[0] - a[0]) * f,
                a[1] + (b[1] - a[1]) * f,
                a[2] + (b[2] - a[2]) * f,
            ];
            let (na, nb) = (axes[from], axes[k]);
            let n = [
                na[0] + (nb[0] - na[0]) * f,
                na[1] + (nb[1] - na[1]) * f,
                na[2] + (nb[2] - na[2]) * f,
            ];
            let roll = rolls[from] + (rolls[k] - rolls[from]) * f;
            check(k, &at(k), p, &tool_rotation(n, roll));
        }
        check(k, &at(k), world(k), &orientation(k));
        from = k;
    }
    check(
        end - 1,
        &|| "the lift at the end".into(),
        lifted,
        &orientation(end - 1),
    );
    check(
        end - 1,
        &|| "upright at the end".into(),
        lifted,
        &tool_rotation(UP, 0.0),
    );
    if !failures.is_empty() {
        return Err(failures);
    }
    Ok(Plan {
        rolls,
        speed,
        held,
        fan_at,
        keep,
        least,
    })
}

/// Eases the tilt back around point `at`: each point within `RELIEF` mm of path leans
/// `RELIEF_FACTOR` of its tilt at `at`, blending to all of it at `RELIEF` mm (a cosine).
fn relieve(path: &Toolpath, axes: &mut [P3], relieved: &mut [bool], at: usize, end: usize) {
    let at = at.min(end - 1);
    let mut reach = |range: &mut dyn Iterator<Item = usize>, step: &dyn Fn(usize) -> usize| {
        let mut s = 0.0;
        for i in range {
            if i != at {
                s += dist(path.points[i], path.points[step(i)]);
            }
            if s > RELIEF {
                break;
            }
            let t = 0.5 - 0.5 * (std::f64::consts::PI * s / RELIEF).cos(); // 0 at `at`, 1 at RELIEF
            let keep = RELIEF_FACTOR + (1.0 - RELIEF_FACTOR) * t;
            let n = axes[i];
            let lean = n[2].clamp(-1.0, 1.0).acos() * keep;
            let across = n[0].hypot(n[1]);
            if across > 1e-12 {
                let (sl, cl) = lean.sin_cos();
                axes[i] = [n[0] / across * sl, n[1] / across * sl, cl];
                relieved[i] = true;
            }
        }
    };
    reach(&mut (at..end), &|i| i.saturating_sub(1).max(at));
    reach(&mut (0..at).rev(), &|i| i + 1);
}

/// The tool's world rotation (columns x, y, z) pointing along `n` (from the tip into the tool):
/// world −Y made square to the nozzle is x (the handover's roll), turned `roll` degrees about
/// the nozzle.
fn tool_rotation(n: P3, roll: f64) -> kuka::M3 {
    let z = unit(n).unwrap_or(UP);
    let y_down = [0.0, -1.0, 0.0];
    let x0 = unit(sub(y_down, scale(z, dot3(z, y_down)))).unwrap_or([1.0, 0.0, 0.0]);
    let y0 = cross(z, x0);
    let (s, c) = roll.to_radians().sin_cos();
    let x = [
        x0[0] * c + y0[0] * s,
        x0[1] * c + y0[1] * s,
        x0[2] * c + y0[2] * s,
    ];
    let y = cross(z, x);
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

/// The angle between two orientations (degrees).
fn turn_between(a: &kuka::M3, b: &kuka::M3) -> f64 {
    let trace: f64 = (0..3)
        .map(|i| (0..3).map(|k| a[k][i] * b[k][i]).sum::<f64>())
        .sum();
    ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The tool's roll (degrees) at each of the first `end` points: as close to 0 (the handover's)
/// as it can be while every joint stays at least `wrist_margin` inside its limits, changing at
/// most `roll_rate` °/mm of path.
///
/// Each point gets the rolls (in 5° steps from −90° to 90°) that keep the margin: the run of
/// them around 0, or, where 0 doesn't, the run nearest 0. A pass from the end back narrows each
/// point's range to what the points after it can still reach at `roll_rate`; a pass forward
/// then takes the roll nearest 0 within it and within `roll_rate` of the point before. Where no
/// roll keeps the margin, the one with the most room (`write` checks the limits themselves).
fn wrist_rolls(
    path: &Toolpath,
    axes: &[P3],
    end: usize,
    robot: &Robot,
    world: &dyn Fn(usize) -> P3,
) -> Vec<f64> {
    const STEP: f64 = 5.0;
    const STEPS: i32 = 18; // ±90°
    // The joints at each point without a roll, A1, A4 and A6 counted on from the point before
    // (from the purge point's, counted from the handover's start pose): a roll's joints are
    // counted from these.
    let ik = |p: P3, n: P3, roll: f64| kuka::inverse(p, &tool_rotation(n, roll), &robot.tool);
    let mut near = ik(robot.purge_at, UP, 0.0)
        .map(|j| kuka::unwrap_near(j, &kuka::SEED))
        .unwrap_or(kuka::SEED);
    let mut plain = Vec::with_capacity(end);
    for (i, &n) in axes.iter().enumerate().take(end) {
        if let Some(j) = ik(world(i), n, 0.0) {
            near = kuka::unwrap_near(j, &near);
        }
        plain.push(near);
    }
    let room = |i: usize, roll: f64| {
        ik(world(i), axes[i], roll)
            .map(|j| kuka::margin(&kuka::unwrap_near(j, &plain[i])).0)
            .unwrap_or(f64::NEG_INFINITY)
    };
    // Each point's range of rolls.
    let mut range = Vec::with_capacity(end);
    for i in 0..end {
        let rolls: Vec<f64> = (-STEPS..=STEPS).map(|k| f64::from(k) * STEP).collect();
        let rooms: Vec<f64> = rolls.iter().map(|&r| room(i, r)).collect();
        let ok: Vec<bool> = rooms.iter().map(|&m| m >= robot.wrist_margin).collect();
        // Runs of rolls that keep the margin; the one holding 0, else the nearest 0.
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut k = 0;
        while k < ok.len() {
            if ok[k] {
                let first = k;
                while k + 1 < ok.len() && ok[k + 1] {
                    k += 1;
                }
                runs.push((first, k));
            }
            k += 1;
        }
        let distance = |&(a, b): &(usize, usize)| {
            let (lo, hi) = (rolls[a], rolls[b]);
            if lo <= 0.0 && hi >= 0.0 {
                0.0
            } else {
                lo.abs().min(hi.abs())
            }
        };
        let r = match runs
            .iter()
            .min_by(|x, y| distance(x).total_cmp(&distance(y)))
        {
            Some(&(a, b)) => [rolls[a], rolls[b]],
            None => {
                let best = (0..rolls.len())
                    .max_by(|&x, &y| rooms[x].total_cmp(&rooms[y]))
                    .unwrap_or(STEPS as usize);
                [rolls[best], rolls[best]]
            }
        };
        range.push(r);
    }
    let reach = |i: usize| robot.roll_rate * dist(path.points[i - 1], path.points[i]);
    // Back from the end: what each point may take so the points after it stay reachable.
    let mut allowed = range.clone();
    for i in (0..end.saturating_sub(1)).rev() {
        let r = reach(i + 1);
        let (lo, hi) = (allowed[i + 1][0] - r, allowed[i + 1][1] + r);
        let narrowed = [allowed[i][0].max(lo), allowed[i][1].min(hi)];
        // Where the two can't both be met, the point's own range (it then turns faster there,
        // and the move slows for it).
        if narrowed[0] <= narrowed[1] {
            allowed[i] = narrowed;
        }
    }
    // Forward: nearest 0 within the allowed range and within reach of the point before.
    let mut rolls = vec![0.0; end];
    for i in 0..end {
        let [mut lo, mut hi] = allowed[i];
        if i > 0 {
            let r = reach(i);
            let (plo, phi) = (rolls[i - 1] - r, rolls[i - 1] + r);
            if plo.max(lo) <= phi.min(hi) {
                lo = lo.max(plo);
                hi = hi.min(phi);
            }
        }
        rolls[i] = 0.0f64.clamp(lo, hi);
    }
    rolls
}

/// The pose `{X …,Y …,Z …,A …,B …,C …}` in the base frame for the nozzle tip at world `p` with
/// the tool's world rotation `world` (`tool_rotation`).
fn frame(robot: &Robot, p: P3, world: &kuka::M3) -> String {
    let b = rot(robot.base[3], robot.base[4], robot.base[5]);
    let d = sub(p, [robot.base[0], robot.base[1], robot.base[2]]);
    // Rᵀ·d and Rᵀ·R_world.
    let pos = [
        b[0][0] * d[0] + b[1][0] * d[1] + b[2][0] * d[2],
        b[0][1] * d[0] + b[1][1] * d[1] + b[2][1] * d[2],
        b[0][2] * d[0] + b[1][2] * d[1] + b[2][2] * d[2],
    ];
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| b[k][i] * world[k][j]).sum();
        }
    }
    let a = r[1][0].atan2(r[0][0]).to_degrees();
    let bb = (-r[2][0]).atan2(r[0][0].hypot(r[1][0])).to_degrees();
    let c = r[2][1].atan2(r[2][2]).to_degrees();
    format!(
        "{{X {},Y {},Z {},A {},B {},C {}}}",
        two(pos[0]),
        two(pos[1]),
        two(pos[2]),
        two(a),
        two(bb),
        two(c)
    )
}

/// A frame's six numbers as KRL writes them.
fn frame6(f: [f64; 6]) -> String {
    format!(
        "{{X {},Y {},Z {},A {},B {},C {}}}",
        two(f[0]),
        two(f[1]),
        two(f[2]),
        two(f[3]),
        two(f[4]),
        two(f[5])
    )
}

/// KUKA's ZYX angles (A about Z, then B about Y, then C about X), in degrees, as a matrix.
fn rot(a: f64, b: f64, c: f64) -> [[f64; 3]; 3] {
    let (sa, ca) = a.to_radians().sin_cos();
    let (sb, cb) = b.to_radians().sin_cos();
    let (sc, cc) = c.to_radians().sin_cos();
    [
        [ca * cb, ca * sb * sc - sa * cc, ca * sb * cc + sa * sc],
        [sa * cb, sa * sb * sc + ca * cc, sa * sb * cc - ca * sc],
        [-sb, cb * sc, cb * cc],
    ]
}

/// Two decimals, half away from zero, never "-0.00".
fn two(v: f64) -> String {
    let s = fixed(v, 2);
    if s == "-0.00" { "0.00".into() } else { s }
}

/// A speed in mm/s as $VEL.CP's m/s, five decimals.
fn mps(v: f64) -> String {
    fixed(v / 1000.0, 5)
}

/// A whole number for a command.
fn int(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dist(a: P3, b: P3) -> f64 {
    let d = sub(a, b);
    dot3(d, d).sqrt()
}
fn unit(a: P3) -> Option<P3> {
    let l = dot3(a, a).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| scale(a, 1.0 / l))
}
/// The angle between two directions, in degrees.
fn angle(a: P3, b: P3) -> f64 {
    match (unit(a), unit(b)) {
        (Some(a), Some(b)) => dot3(a, b).clamp(-1.0, 1.0).acos().to_degrees(),
        _ => 0.0,
    }
}

/// The program as a ZIP of its two files (stored, not compressed), for the controller.
pub fn zip(program: &Program) -> Vec<u8> {
    let files = [
        (format!("{}.SRC", program.name), program.src.as_bytes()),
        (format!("{}.DAT", program.name), program.dat.as_bytes()),
    ];
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in &files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // made by
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // needed to extract
            h.extend_from_slice(&0u16.to_le_bytes()); // flags
            h.extend_from_slice(&0u16.to_le_bytes()); // stored
            h.extend_from_slice(&0u16.to_le_bytes()); // time
            h.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&size.to_le_bytes());
            h.extend_from_slice(&size.to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central {
                h.extend_from_slice(&0u16.to_le_bytes()); // comment
                h.extend_from_slice(&0u16.to_le_bytes()); // disk
                h.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
                h.extend_from_slice(&0u32.to_le_bytes()); // external attributes
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(header(0x0201_4b50, true));
    }
    let start = out.len() as u32;
    let size = central.len() as u32;
    out.extend(central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::toolpath::{Settings, from_curves};

    /// The hand-checked fixture's toolpath (scripts/hand-checked-fixtures.py, `robot_square`).
    fn square() -> Toolpath {
        let h = 2.5;
        let drawn = [
            [0.0, 0.0, 0.0],
            [20.0, 0.0, 0.0],
            [20.0, 20.0, 0.0],
            [0.0, 20.0, 0.0],
            [0.0, 0.0, h],
            [20.0, 0.0, h],
            [20.0, 4.1, h],
        ];
        let curve = Curve {
            points: drawn.to_vec(),
            closed: false,
            circle: None,
            winding: None,
            bead: None,
        };
        let mut path = from_curves(&[&curve], Settings::new(5.0, h, 10.0).unwrap(), 1000).unwrap();
        let (s30, c30) = 30f64.to_radians().sin_cos();
        let (s20, c20) = 20f64.to_radians().sin_cos();
        path.axis = vec![
            UP,
            UP,
            UP,
            UP,
            [s30, 0.0, c30],
            [0.0, s30, c30],
            [0.0, -s20, c20],
        ];
        path
    }

    fn robot() -> Robot {
        Robot {
            tolerance: 0.0,
            ..Robot::default()
        }
    }

    fn program(path: &Toolpath, r: &Robot) -> Result<Program, String> {
        write(path, r, BED, CENTER, 1.0, "Robot square")
    }

    #[test]
    fn matches_the_hand_checked_fixture() {
        let fixture = include_str!("../../../fixtures/hand-checked/robot-square.src");
        let ours = program(&square(), &robot()).unwrap();
        let expected: Vec<&str> = fixture.lines().filter(|l| !l.starts_with(";;")).collect();
        let got: Vec<&str> = ours.src.split("\r\n").collect();
        // The program ends with a line break (CRLF): one empty piece after it.
        assert_eq!(got.last(), Some(&""));
        let got = &got[..got.len() - 1];
        for (k, (a, b)) in got.iter().zip(&expected).enumerate() {
            assert_eq!(a, b, "line {}", k + 1);
        }
        assert_eq!(got.len(), expected.len());
        assert!(ours.src.is_ascii() && ours.dat.is_ascii());
    }

    #[test]
    fn the_dat_holds_the_frames_and_the_load() {
        let ours = program(&square(), &robot()).unwrap();
        assert_eq!(
            ours.dat,
            "&ACCESS RVP\r\nDEFDAT EASYCAD PUBLIC\r\n\
             DECL FRAME TR_TOOL={X -508.00,Y 3.50,Z 139.34,A -180.00,B -90.00,C 0.00}\r\n\
             DECL FRAME TR_BASE={X 0.00,Y 50.00,Z -0.30,A 0.00,B 0.00,C 0.00}\r\n\
             ; Estimated uniform-density volume centroid, relative to flange.\r\n\
             DECL REAL TR_MASS=18.0\r\n\
             DECL FRAME TR_COM={X -90.65,Y 5.95,Z 164.92,A 0.00,B 0.00,C 0.00}\r\nENDDAT\r\n"
        );
    }

    #[test]
    fn a_test_print_stops_at_its_height() {
        let r = Robot {
            stop_at: 3.0,
            ..robot()
        };
        let ours = program(&square(), &r).unwrap();
        // The first layer only (nozzle at 2.5 mm): three moves, no fan, upright at the end.
        assert_eq!(ours.segments, 3);
        assert!(!ours.src.contains("SET_FAN1 255"));
        assert!(
            ours.src
                .contains("LIN {X 1088.30,Y -145.14,Z -606.20,A -90.00,B 0.00,C 0.00}")
        );
        let r = Robot {
            stop_at: 1.0,
            ..robot()
        };
        assert!(program(&square(), &r).is_err());
    }

    #[test]
    fn leaves_out_points_on_a_straight_line() {
        // Ten points along one side: only the ends are needed.
        let pts: Vec<P3> = (0..=10).map(|k| [k as f64 * 2.0, 0.0, 0.0]).collect();
        let curve = Curve {
            points: pts,
            closed: false,
            circle: None,
            winding: None,
            bead: None,
        };
        let path = from_curves(&[&curve], Settings::new(5.0, 2.5, 10.0).unwrap(), 1000).unwrap();
        let ours = program(&path, &Robot::default()).unwrap();
        assert_eq!(ours.segments, 1);
        assert!((ours.seconds - 2.0).abs() < 1e-9);
        // Long programs come in blocks.
        let r = Robot {
            block: 3,
            ..robot()
        };
        let ours = program(&path, &r).unwrap();
        assert_eq!(ours.src.matches("ENDSPLINE C_SPL").count(), 3);
    }

    #[test]
    fn refuses_what_it_cannot_print() {
        let mut lost = square();
        lost.axis.pop();
        assert!(
            program(&lost, &robot())
                .unwrap_err()
                .contains("put `tilt` last")
        );
        let mut tilted = square();
        tilted.axis[6] = [0.0, -(70f64.to_radians().sin()), 70f64.to_radians().cos()];
        assert!(program(&tilted, &robot()).unwrap_err().contains("leans"));
        let mut low = square();
        low.points[3][2] = -0.5;
        assert!(
            program(&low, &robot())
                .unwrap_err()
                .contains("below the bed")
        );
        let mut off = square();
        off.points[2][0] = 900.0;
        assert!(program(&off, &robot()).unwrap_err().contains("off the bed"));
        let mut hop = square();
        hop.travel[4] = true;
        assert!(program(&hop, &robot()).unwrap_err().contains("travel"));
        let bad = Robot {
            program: "1st".into(),
            ..robot()
        };
        assert!(program(&square(), &bad).is_err());
        let bad = Robot {
            heat: [400.0, 0.0, 0.0],
            ..robot()
        };
        assert!(program(&square(), &bad).is_err());
        let bad = Robot {
            screw: f64::NAN,
            ..robot()
        };
        assert!(program(&square(), &bad).is_err());
    }

    /// A ring 200 mm up, the nozzle tilted 45° in towards its centre all the way round (the
    /// Sphere's closing top): with the handover's roll alone the wrist (A5) goes past its limit
    /// on one side. The tool rolls there, and where that's not enough the tilt eases back; every
    /// pose then keeps 3° from every limit.
    #[test]
    fn rolls_the_tool_to_keep_the_wrist_inside_its_limits() {
        let n = 360;
        let pts: Vec<P3> = (0..=n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                [150.0 * a.cos(), 150.0 * a.sin(), 200.0]
            })
            .collect();
        let axis: Vec<P3> = pts
            .iter()
            .map(|p| {
                let r = p[0].hypot(p[1]);
                let s = 45f64.to_radians().sin();
                [-p[0] / r * s, -p[1] / r * s, 45f64.to_radians().cos()]
            })
            .collect();
        let curve = Curve {
            points: pts,
            closed: false,
            circle: None,
            winding: None,
            bead: None,
        };
        let mut path =
            from_curves(&[&curve], Settings::new(5.0, 2.5, 10.0).unwrap(), 1000).unwrap();
        path.axis = axis;
        // The handover's roll alone: past A5's limit somewhere on the ring.
        let tool = Robot::default().tool;
        let o = Robot::default().origin;
        let worst = path
            .points
            .iter()
            .zip(&path.axis)
            .map(|(p, n)| {
                let j = kuka::inverse(
                    [o[0] + p[0], o[1] + p[1], o[2] + p[2]],
                    &tool_rotation(*n, 0.0),
                    &tool,
                )
                .unwrap();
                kuka::margin(&j).0
            })
            .fold(f64::INFINITY, f64::min);
        assert!(worst < 0.0, "{worst}");
        let ours = program(&path, &Robot::default()).unwrap();
        assert!(
            ours.joint_margin >= MIN_MARGIN - 1e-9,
            "{}",
            ours.joint_margin
        );
        assert!(
            ours.rolled > 0 && ours.max_roll > 10.0,
            "{} {}",
            ours.rolled,
            ours.max_roll
        );
    }

    #[test]
    fn the_zip_holds_both_files() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let ours = program(&square(), &robot()).unwrap();
        let z = zip(&ours);
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        let text = String::from_utf8_lossy(&z);
        assert!(text.contains("EASYCAD.SRC") && text.contains("EASYCAD.DAT"));
        assert!(text.contains(&ours.src) && text.contains(&ours.dat));
        assert_eq!(&z[z.len() - 22..z.len() - 18], &[0x50, 0x4b, 0x05, 0x06]);
    }
}
