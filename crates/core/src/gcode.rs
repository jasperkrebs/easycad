//! G-code for a toolpath, in EasyGCode's dialect.
//!
//! G-code drives real machines, so this follows EasyGCode's proven output exactly (`gcode.js`):
//! absolute positioning (G90) with relative extrusion (M83); every `G1 E` is the segment's
//! volume (bead area × length × flow) in mm³ for the pellet printer or in mm of filament for
//! the filament printer; coordinates with 3 decimals and E with 5, rounded like JavaScript's
//! `toFixed`; F only when it changes; the part fan on after the first layer; the start and end
//! blocks copied verbatim from EasyGCode; and the toolpath's machine events (fan levels, dwells,
//! temperatures and waits for them, speed and extrusion overrides) in the commands EasyGCode
//! uses for them. Golden tests compare the output with G-code produced by EasyGCode itself
//! (`fixtures/easygcode/`), and hand-checked fixtures cover what EasyGCode has no project for.
//!
//! Before writing anything, the toolpath is checked: every point must be on the bed and inside
//! the build height, and every extrusion amount sane.

use crate::geom::dist3;
use crate::toolpath::{EventKind, Toolpath};
use serde::Deserialize;
use std::f64::consts::PI;
use std::fmt::Write as _;

/// Which kind of machine, and so which start/end blocks and E units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Klipper pellet printer: E in mm³.
    #[default]
    Pellet,
    /// Marlin-style filament printer: E in mm of filament.
    Filament,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Filament {
    pub diameter: f64,
    pub nozzle: f64,
    pub bed: f64,
    /// Part fan, 0–100 %.
    pub fan: f64,
}

impl Default for Filament {
    fn default() -> Self {
        // EasyGCode's defaults.
        Filament {
            diameter: 1.75,
            nozzle: 205.0,
            bed: 60.0,
            fan: 100.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Pellet {
    /// Upper, middle and lower extruder zone temperatures.
    pub up: f64,
    pub mid: f64,
    pub down: f64,
    pub bed: f64,
    /// Pressure advance.
    pub pa: f64,
    /// Purge amount.
    pub purge: f64,
    /// Part fan, 0–100 %.
    pub fan: f64,
}

impl Default for Pellet {
    fn default() -> Self {
        // EasyGCode's defaults.
        Pellet {
            up: 210.0,
            mid: 210.0,
            down: 210.0,
            bed: 50.0,
            pa: 0.3,
            purge: 5000.0,
            fan: 0.0,
        }
    }
}

/// A printer: the machine settings G-code needs. Defaults are EasyGCode's.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Profile {
    pub mode: Mode,
    /// Extrusion multiplier.
    pub multiplier: f64,
    /// Include the start and end G-code blocks.
    pub include_start_end: bool,
    /// How far above the print the nozzle lifts at the end, in mm.
    pub end_lift: f64,
    /// Travel speed in mm/s.
    pub travel_speed: f64,
    /// Where the recipe's origin sits on the bed (x, y), in mm.
    pub center: [f64; 2],
    /// Bed width, depth and build height, in mm: the toolpath must fit inside.
    pub bed: [f64; 3],
    pub filament: Filament,
    pub pellet: Pellet,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            mode: Mode::Pellet,
            multiplier: 1.0,
            include_start_end: true,
            end_lift: 50.0,
            travel_speed: 100.0,
            center: [125.0, 125.0],
            bed: [400.0, 400.0, 400.0],
            filament: Filament::default(),
            pellet: Pellet::default(),
        }
    }
}

/// Numbers as JavaScript's `Number.prototype.toFixed` writes them: rounded half away from zero
/// on the exact value (Rust rounds exact ties to even), and never "-0.000".
pub fn fixed(v: f64, digits: usize) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let x = v.abs();
    let scaled = x * 10f64.powi(digits as i32);
    let mut out = if (scaled - scaled.floor() - 0.5).abs() > 1e-6 {
        format!("{x:.digits$}")
    } else {
        // Possibly an exact tie: decide on the exact decimal expansion.
        let exact = format!("{x:.80}");
        let (int, frac) = exact.split_once('.').unwrap_or((exact.as_str(), ""));
        let keep = frac.get(..digits).unwrap_or(frac);
        let round_up = frac.as_bytes().get(digits).is_some_and(|d| *d >= b'5');
        let mut digits_str: Vec<u8> = format!("{int}{keep}").into_bytes();
        if round_up {
            let mut i = digits_str.len();
            loop {
                if i == 0 {
                    digits_str.insert(0, b'1');
                    break;
                }
                i -= 1;
                if digits_str[i] == b'9' {
                    digits_str[i] = b'0';
                } else {
                    digits_str[i] += 1;
                    break;
                }
            }
        }
        let s = String::from_utf8(digits_str).unwrap_or_default();
        let split = s.len() - digits;
        if digits == 0 {
            s
        } else {
            format!("{}.{}", &s[..split], &s[split..])
        }
    };
    // JavaScript keeps the sign of any negative number, even one that rounds to zero
    // ((-0.0001).toFixed(3) is "-0.000"), but not of negative zero.
    if v < 0.0 {
        out.insert(0, '-');
    }
    out
}

fn f3(v: f64) -> String {
    fixed(v, 3)
}

fn f5(v: f64) -> String {
    fixed(v, 5)
}

/// JavaScript's `Math.round` (half up).
fn round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// EasyGCode's filament (Marlin) start block.
fn marlin_start(f: &Filament) -> Vec<String> {
    let (bed, nozzle) = (num(f.bed), num(f.nozzle));
    vec![
        "; --- start G-code (filament / Marlin) ---".into(),
        format!("M140 S{bed} ; set bed temp"),
        format!("M104 S{nozzle} ; set hotend temp (heats during bed wait + homing)"),
        format!("M190 S{bed} ; wait for bed temp"),
        "G28 ; home all (incl. mesh bed level)".into(),
        "G90 ; absolute coordinates".into(),
        "G21 ; millimeter units".into(),
        "M83 ; relative extrusion".into(),
        "G0 F3000 X10.0 Y10.0 ; park".into(),
        format!("M109 S{nozzle} ; wait for hotend temp"),
        "G1 F250 E20.788 ; load / prime nozzle".into(),
        "G0 F8000 Z0.3".into(),
        "M220 S100 ; reset speed factor".into(),
        "M221 S100 ; reset extrude factor".into(),
        "; primer lines".into(),
        "G0 F8000 X50.0 Y14.0 Z0.2".into(),
        "G1 F500 X110.0 E12".into(),
        "G1 Y12.0 E0.5".into(),
        "G1 X50.0 E12".into(),
        "G1 Y14.0 E0.5".into(),
        "G1 Y16.0 E0.5".into(),
        "G1 X80.0 E4".into(),
        "G1 Y45.2 E3".into(),
        "M106 S0 ; fan off for the ramp loop".into(),
        "; --- end of start G-code ---".into(),
    ]
}

fn marlin_end(z_lift: f64) -> Vec<String> {
    vec![
        "; --- end G-code (filament / Marlin) ---".into(),
        "M83".into(),
        "G1 E-0.8 F3000 ; retract".into(),
        "G90 ; absolute coordinates".into(),
        format!(
            "G0 Z{} F8000 ; lift clear of the print for finishing",
            f3(z_lift)
        ),
        "M106 S0 ; fan off".into(),
        "M140 S0 ; bed off".into(),
        "M104 S0 ; hotend off".into(),
        "M221 S100 ; reset flow".into(),
        "M900 K0 ; reset linear advance".into(),
        "M84 ; disable steppers".into(),
    ]
}

/// EasyGCode's pellet (Klipper) start block.
fn klipper_start(p: &Pellet) -> Vec<String> {
    // Bed wait window derived from the target (reproduces 40/90 at bed 50).
    let bed_min = round(p.bed - 10.0).max(0);
    let bed_max = round(p.bed + 40.0);
    let (up, mid, down) = (num(p.up), num(p.mid), num(p.down));
    vec![
        "; --- start G-code (pellet / Klipper) ---".into(),
        "SET_PRESSURE_ADVANCE EXTRUDER=extruder SMOOTH_TIME=0.04".into(),
        "SET_PRESSURE_ADVANCE EXTRUDER=extruder ADVANCE=0.0".into(),
        "_GINGER_BUZZER_TONE_INITIAL".into(),
        format!("_GINGER_BED_HEATING BED_TEMPERATURE={}", num(p.bed)),
        format!("_GINGER_EXTRUDER_SET_UP S={up}"),
        format!("_GINGER_EXTRUDER_SET_MID S={mid}"),
        format!("_GINGER_EXTRUDER_SET_DOWN S={down}"),
        "G28 ; home".into(),
        "BED_MESH_PROFILE LOAD=global".into(),
        "_GINGER_PURGE_PARKING PURGE_LAYER_HEIGHT=2 PURGE_PARKING_SPEED=10000".into(),
        format!("_GINGER_EXTRUDER_WAIT_UP S={up}"),
        format!("_GINGER_EXTRUDER_WAIT_MID S={mid}"),
        format!("_GINGER_EXTRUDER_WAIT_DOWN S={down}"),
        format!("_GINGER_BED_WAIT BED_TEMPERATURE_MIN={bed_min} BED_TEMPERATURE_MAX={bed_max}"),
        "_GINGER_EXTRUDER_MIXING_MULTIPLIER S=1".into(),
        "SET_EXTRUDER_ROTATION_DISTANCE EXTRUDER=extruder DISTANCE=456".into(),
        "SET_EXTRUDER_ROTATION_DISTANCE EXTRUDER=mixing_stepper DISTANCE=8000".into(),
        // PURGE_LENGHT [sic]: the parameter name must match the printer macro.
        format!(
            "_GINGER_PURGE PURGE_LENGHT=400 PURGE_SPEED=500 PURGE_MATERIAL_QUANTITY={}",
            num(p.purge)
        ),
        "G90 ; absolute coordinates".into(),
        "G92 E0".into(),
        "M83 ; relative extrusion".into(),
        "M220 S100 ; reset speed factor".into(),
        "M221 S100 ; reset extrude factor".into(),
        format!(
            "SET_PRESSURE_ADVANCE EXTRUDER=extruder ADVANCE={}",
            num(p.pa)
        ),
        "SET_PRESSURE_ADVANCE EXTRUDER=extruder SMOOTH_TIME=0.5".into(),
        "M106 S0 ; fan off for the ramp loop".into(),
        "_GINGER_BUZZER_TONE_INITIAL".into(),
        "; --- end of start G-code ---".into(),
    ]
}

fn klipper_end(z_lift: f64) -> Vec<String> {
    vec![
        "; --- end G-code (pellet / Klipper) ---".into(),
        "M83".into(),
        "G90 ; absolute coordinates".into(),
        format!(
            "G0 Z{} F3000 ; lift clear of the print for finishing",
            f3(z_lift)
        ),
        "TURN_OFF_HEATERS ; zones + bed off".into(),
        "M106 S0 ; fan off".into(),
        "M84 ; disable steppers".into(),
    ]
}

/// A setting as JavaScript prints a number in a string (`'S' + 210` is "S210", 0.3 is "0.3").
fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// What a G-code file contains, for the app.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub text: String,
    pub lines: usize,
}

/// Writes the toolpath as G-code for `profile`. `title` names the project in the header.
pub fn write(path: &Toolpath, profile: &Profile, title: &str) -> Result<Output, String> {
    check(path, profile)?;
    let mode = profile.mode;
    let mult = if profile.multiplier > 0.0 {
        profile.multiplier
    } else {
        1.0
    };
    let dia = if profile.filament.diameter > 0.0 {
        profile.filament.diameter
    } else {
        1.75
    };
    let e_factor = mult
        / if mode == Mode::Filament {
            PI * (dia / 2.0) * (dia / 2.0)
        } else {
            1.0
        };
    let fan = if mode == Mode::Filament {
        profile.filament.fan
    } else {
        profile.pellet.fan
    };
    let fan_pwm = round(fan.clamp(0.0, 100.0) * 2.55);
    let travel_feed = profile.travel_speed * 60.0;
    let [cx, cy] = profile.center;
    let own_zones = [profile.pellet.up, profile.pellet.mid, profile.pellet.down];

    let mut lines: Vec<String> = Vec::with_capacity(path.points.len() + 64);
    let stats = path.stats();
    lines.push(format!("; EasyCAD — {}", title.replace(['\n', '\r'], " ")));
    lines.push(format!(
        "; {} layer(s), {} mm of extrusion, {} mm3",
        stats.layers,
        fixed(stats.path_length_mm, 1),
        fixed(stats.volume_mm3, 1)
    ));
    for (i, s) in path.parts.iter().enumerate() {
        let speed = match s.flow {
            Some(q) => format!("flow {} mm3/s", num(q)),
            None => format!("feed {} mm/min", round(s.speed * 60.0)),
        };
        lines.push(format!(
            "; part {}: lineWidth={} layerHeight={} {speed} (bead area {} mm2)",
            i + 1,
            num(s.width),
            num(s.layer),
            fixed(s.area(), 3)
        ));
    }
    lines.push(match mode {
        Mode::Filament => format!(
            "; printer=filament multiplier={} filamentDiameter={} (E in mm of filament)",
            num(mult),
            num(dia)
        ),
        Mode::Pellet => format!(
            "; printer=pellet multiplier={} (E in mm^3, volumetric)",
            num(mult)
        ),
    });
    if profile.include_start_end {
        lines.extend(match mode {
            Mode::Filament => marlin_start(&profile.filament),
            Mode::Pellet => klipper_start(&profile.pellet),
        });
    }
    lines.push("G90 ; absolute positioning".into());
    lines.push("M83 ; relative extrusion".into());

    let first_layer_end = path
        .layer_starts
        .get(1)
        .copied()
        .unwrap_or(path.points.len());
    // A toolpath that sets its own fan levels doesn't get the automatic one.
    let own_fan = path
        .events
        .iter()
        .any(|e| matches!(e.kind, EventKind::Fan(_)));
    let mut sorted = path.events.clone();
    sorted.sort_by_key(|e| e.after); // stable: events at one point keep their order
    let mut events = sorted.iter().peekable();
    let mut prev: Option<[f64; 3]> = None;
    let mut last_feed: Option<i64> = None;
    let mut first_extrude = true;
    let mut max_z = 0.0f64;
    let layers = path.layer_starts.len().max(1);
    let mut layer = 0;
    for (i, p) in path.points.iter().enumerate() {
        if path.layer_starts.get(layer) == Some(&i) {
            layer += 1;
            lines.push(format!("; --- layer {layer} of {layers} ---"));
        }
        let cur = [p[0] + cx, p[1] + cy, p[2]];
        let travel = i == 0 || path.travel.get(i).copied().unwrap_or(false);
        if travel {
            let feed = round(travel_feed);
            lines.push(format!(
                "G0 X{} Y{} Z{} F{feed}",
                f3(cur[0]),
                f3(cur[1]),
                f3(cur[2])
            ));
            last_feed = Some(feed);
            prev = Some(cur);
        } else if let Some(from) = prev {
            let seg = dist3(from, cur);
            prev = Some(cur);
            if seg >= 1e-7 {
                let s = path.settings(i);
                let area = path.area(i);
                let volume = area * seg * path.flow.get(i).copied().unwrap_or(1.0);
                let mut line = format!(
                    "G1 X{} Y{} Z{} E{}",
                    f3(cur[0]),
                    f3(cur[1]),
                    f3(cur[2]),
                    f5(volume * e_factor)
                );
                let feed = round(s.feed_for(area) * path.speed_factor(i));
                if last_feed != Some(feed) || first_extrude {
                    let _ = write!(line, " F{feed}");
                    last_feed = Some(feed);
                }
                lines.push(line);
                first_extrude = false;
                max_z = max_z.max(cur[2]);
            }
        }
        if i + 1 == first_layer_end && profile.include_start_end && fan_pwm > 0 && !own_fan {
            lines.push(format!(
                "M106 S{fan_pwm} ; part cooling fan on after first layer"
            ));
        }
        while let Some(e) = events.next_if(|e| e.after <= i) {
            match e.kind {
                // Like the automatic fan, only with start/end blocks (they turn it off again).
                EventKind::Fan(level) if profile.include_start_end => {
                    let pwm = round(level.clamp(0.0, 100.0) * 2.55);
                    lines.push(if pwm > 0 {
                        format!("M106 S{pwm} ; part fan {} %", num(level))
                    } else {
                        "M107 ; part fan off".into()
                    });
                }
                EventKind::Fan(_) => {}
                EventKind::Dwell(seconds) => {
                    lines.push(format!("G4 P{} ; dwell", round(seconds * 1000.0)));
                }
                // EasyGCode's own commands for these (its bend stool's foaming): the pellet
                // printer's zone macros and tolerant waits, and the filament printer's M104/M109.
                EventKind::Heat(t) => match mode {
                    Mode::Pellet => {
                        let [up, mid, down] = t.zones_or(own_zones);
                        lines.push(format!("_GINGER_EXTRUDER_SET_UP S={}", num(up)));
                        lines.push(format!("_GINGER_EXTRUDER_SET_MID S={}", num(mid)));
                        lines.push(format!("_GINGER_EXTRUDER_SET_DOWN S={}", num(down)));
                    }
                    Mode::Filament => lines.push(format!(
                        "M104 S{} ; set hotend temp",
                        num(t.nozzle_or(profile.filament.nozzle))
                    )),
                },
                EventKind::WaitHeat { temps, cool } => match (mode, cool) {
                    // Exact-match waits can hang on zones that settle near the target, never on
                    // it: wait for the zone at the nozzle to be within 2 °C (EasyGCode).
                    (Mode::Pellet, false) => {
                        let [_, _, down] = temps.zones_or(own_zones);
                        lines.push(format!(
                            "TEMPERATURE_WAIT SENSOR=extruder2 MINIMUM={}",
                            num(down - 2.0)
                        ));
                    }
                    (Mode::Pellet, true) => {
                        let zones = temps.zones_or(own_zones);
                        for (sensor, t) in ["extruder", "extruder1", "extruder2"].iter().zip(zones)
                        {
                            lines.push(format!(
                                "TEMPERATURE_WAIT SENSOR={sensor} MAXIMUM={}",
                                num(t + 2.0)
                            ));
                        }
                    }
                    (Mode::Filament, _) => lines.push(format!(
                        "M109 S{} ; wait for hotend temp",
                        num(temps.nozzle_or(profile.filament.nozzle))
                    )),
                },
                EventKind::SpeedOverride(pct) => {
                    lines.push(format!("M220 S{} ; speed override", round(pct)));
                }
                EventKind::ExtrusionOverride(pct) => {
                    lines.push(format!("M221 S{} ; extrusion override", round(pct)));
                }
            }
        }
    }
    if profile.include_start_end {
        // Lift clear of the print, but never past the build height (Klipper refuses such a move).
        let lift = (max_z + profile.end_lift.max(0.0)).min(profile.bed[2].max(max_z));
        lines.extend(match mode {
            Mode::Filament => marlin_end(lift),
            Mode::Pellet => klipper_end(lift),
        });
    }
    let count = lines.len();
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(Output { text, lines: count })
}

/// The most a move may extrude, in beads (overhang compensation goes up to about 2.5).
const MAX_EXTRUSION: f64 = 4.0;
/// The most a move's speed may be scaled up.
const MAX_SPEED_FACTOR: f64 = 20.0;
/// The hottest a heater may be set, in °C (the printer settings allow the same).
const MAX_TEMPERATURE: f64 = 350.0;
/// The range of the printer's speed and extrusion overrides, in %.
const MIN_OVERRIDE: f64 = 1.0;
const MAX_OVERRIDE: f64 = 1000.0;

/// Refuses toolpaths a machine shouldn't run: off the bed, out of reach, or with nonsense
/// extrusion.
fn check(path: &Toolpath, profile: &Profile) -> Result<(), String> {
    if path.points.is_empty() {
        return Err("the toolpath is empty".into());
    }
    let p = profile;
    let finite = [
        p.multiplier,
        p.end_lift,
        p.travel_speed,
        p.center[0],
        p.center[1],
        p.bed[0],
        p.bed[1],
        p.bed[2],
    ]
    .iter()
    .chain(
        [
            p.filament.diameter,
            p.filament.nozzle,
            p.filament.bed,
            p.filament.fan,
        ]
        .iter(),
    )
    .chain(
        [
            p.pellet.up,
            p.pellet.mid,
            p.pellet.down,
            p.pellet.bed,
            p.pellet.pa,
            p.pellet.purge,
            p.pellet.fan,
        ]
        .iter(),
    )
    .all(|v| v.is_finite());
    if !finite {
        return Err("a printer setting is not a number".into());
    }
    if !(p.travel_speed > 0.0 && p.travel_speed <= 2000.0) {
        return Err(format!(
            "the travel speed must be between 0 and 2000 mm/s, got {}",
            p.travel_speed
        ));
    }
    if !(p.multiplier > 0.0 && p.multiplier <= 5.0) {
        return Err(format!(
            "the extrusion multiplier must be between 0 and 5, got {}",
            p.multiplier
        ));
    }
    if p.mode == Mode::Filament && !(p.filament.diameter > 0.5 && p.filament.diameter < 5.0) {
        return Err(format!(
            "the filament diameter must be between 0.5 and 5 mm, got {}",
            p.filament.diameter
        ));
    }
    let [bx, by, bz] = p.bed;
    for (i, q) in path.points.iter().enumerate() {
        let (x, y, z) = (q[0] + p.center[0], q[1] + p.center[1], q[2]);
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err("the toolpath has a point that is not a number".into());
        }
        if x < 0.0 || x > bx || y < 0.0 || y > by {
            return Err(format!(
                "the toolpath leaves the bed at X{} Y{} (the bed is {} × {} mm, the print's centre at X{} Y{}); move it or make it smaller",
                f3(x),
                f3(y),
                num(bx),
                num(by),
                num(p.center[0]),
                num(p.center[1])
            ));
        }
        if z < 0.0 || z > bz {
            return Err(format!(
                "the toolpath goes to Z{} (the build height is {} mm)",
                f3(z),
                num(bz)
            ));
        }
        let flow = path.flow.get(i).copied().unwrap_or(0.0);
        if !(0.0..=MAX_EXTRUSION).contains(&flow) {
            return Err(format!(
                "move {i} extrudes {flow} times a bead; it must be between 0 and {MAX_EXTRUSION}"
            ));
        }
        let speed = path.speed_factor(i);
        if !(speed.is_finite() && speed > 0.0 && speed <= MAX_SPEED_FACTOR) {
            return Err(format!(
                "move {i} has a speed factor of {speed}; it must be more than 0 and at most {MAX_SPEED_FACTOR}"
            ));
        }
    }
    let own_zones = [p.pellet.up, p.pellet.mid, p.pellet.down];
    for e in &path.events {
        match e.kind {
            EventKind::Dwell(s) if !(s.is_finite() && (0.0..=600.0).contains(&s)) => {
                return Err(format!("a dwell of {s} s; it must be between 0 and 600 s"));
            }
            EventKind::Fan(f) if !(f.is_finite() && (0.0..=100.0).contains(&f)) => {
                return Err(format!(
                    "a fan level of {f} %; it must be between 0 and 100"
                ));
            }
            EventKind::Heat(t) | EventKind::WaitHeat { temps: t, .. } => {
                let used = match p.mode {
                    Mode::Pellet => t.zones_or(own_zones).to_vec(),
                    Mode::Filament => vec![t.nozzle_or(p.filament.nozzle)],
                };
                if let Some(bad) = used
                    .iter()
                    .find(|t| !(t.is_finite() && **t > 0.0 && **t <= MAX_TEMPERATURE))
                {
                    return Err(format!(
                        "a temperature of {} °C; it must be more than 0 and at most {MAX_TEMPERATURE} °C",
                        num(*bad)
                    ));
                }
                if p.mode == Mode::Filament
                    && matches!(e.kind, EventKind::WaitHeat { cool: true, .. })
                {
                    return Err("the filament printer can't wait for its nozzle to cool (only the pellet printer's zones can); wait until hot, or leave the wait out".into());
                }
            }
            EventKind::SpeedOverride(v) | EventKind::ExtrusionOverride(v)
                if !(v.is_finite() && (MIN_OVERRIDE..=MAX_OVERRIDE).contains(&v)) =>
            {
                return Err(format!(
                    "an override of {} %; it must be between {MIN_OVERRIDE} and {MAX_OVERRIDE} %",
                    num(v)
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::toolpath::{self, MAX_PATH_POINTS, Settings};

    #[test]
    fn numbers_round_like_javascript_to_fixed() {
        // Expected strings from Node.js (`v.toFixed(3)`).
        let cases = [
            (125.0, "125.000"),
            (0.0625, "0.063"), // an exact tie: JavaScript rounds up, Rust's {:.3} rounds to even
            (-0.0625, "-0.063"),
            (1.0005, "1.000"), // really 1.000499999…
            (0.9995, "1.000"), // really 0.999500000…055: up, with a carry
            (9.9995, "9.999"),
            (185.8235, "185.823"),
            (1.4995, "1.500"),
            (-0.0001, "-0.000"),
            (-0.0, "0.000"),
        ];
        for (v, want) in cases {
            assert_eq!(fixed(v, 3), want, "{v}");
        }
        assert_eq!(fixed(2.675, 2), "2.67");
        assert_eq!(fixed(2.5, 0), "3");
        assert_eq!(fixed(0.00668123, 5), "0.00668");
    }

    #[test]
    fn a_ring_prints_with_volumetric_e_and_travel() {
        let ring = Curve::circle(10.0, 4);
        let path = toolpath::from_curves(
            &[&ring],
            Settings::new(2.0, 0.6, 20.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let profile = Profile {
            include_start_end: false,
            ..Profile::default()
        };
        let out = write(&path, &profile, "ring").unwrap().text;
        let body: Vec<&str> = out.lines().filter(|l| !l.starts_with(';')).collect();
        assert_eq!(body[0], "G90 ; absolute positioning");
        assert_eq!(body[2], "G0 X135.000 Y125.000 Z0.600 F6000");
        // A side of the square: 10·√2 mm × bead area (2 − 0.6)·0.6 + π·0.3².
        let e = 10.0 * 2f64.sqrt() * ((2.0 - 0.6) * 0.6 + PI * 0.3 * 0.3);
        assert_eq!(
            body[3],
            format!("G1 X125.000 Y135.000 Z0.600 E{} F1200", fixed(e, 5))
        );
        assert!(!body[4].contains('F'));
        assert_eq!(body.len(), 3 + 4);
    }

    #[test]
    fn filament_e_is_in_mm_of_filament() {
        let ring = Curve::circle(10.0, 4);
        let path = toolpath::from_curves(
            &[&ring],
            Settings::new(0.6, 0.3, 30.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let profile = Profile {
            mode: Mode::Filament,
            include_start_end: false,
            ..Profile::default()
        };
        let out = write(&path, &profile, "ring").unwrap().text;
        let g1 = out.lines().find(|l| l.starts_with("G1")).unwrap();
        let area = (0.6 - 0.3) * 0.3 + PI * 0.15 * 0.15;
        let e = area * 10.0 * 2f64.sqrt() / (PI * 0.875 * 0.875);
        assert!(g1.contains(&format!("E{}", fixed(e, 5))), "{g1}");
    }

    #[test]
    fn unsafe_toolpaths_are_refused() {
        let big = Curve::circle(300.0, 8);
        let path = toolpath::from_curves(
            &[&big],
            Settings::new(2.0, 0.6, 20.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let err = write(&path, &Profile::default(), "big").unwrap_err();
        assert!(err.contains("leaves the bed"), "{err}");
        let small = toolpath::from_curves(
            &[&Curve::circle(10.0, 8)],
            Settings::new(2.0, 0.6, 20.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let low = Profile {
            bed: [400.0, 400.0, 0.5],
            ..Profile::default()
        };
        assert!(
            write(&small, &low, "low")
                .unwrap_err()
                .contains("build height")
        );
        let bad = Profile {
            travel_speed: f64::NAN,
            ..Profile::default()
        };
        assert!(write(&small, &bad, "x").is_err());
    }

    #[test]
    fn start_and_end_blocks_are_easygcodes() {
        let ring = Curve::circle(10.0, 4);
        let path = toolpath::from_curves(
            &[&ring],
            Settings::new(2.0, 0.6, 20.0).unwrap(),
            MAX_PATH_POINTS,
        )
        .unwrap();
        let pellet = write(&path, &Profile::default(), "x").unwrap().text;
        assert!(
            pellet.contains("_GINGER_BED_WAIT BED_TEMPERATURE_MIN=40 BED_TEMPERATURE_MAX=90\n")
        );
        assert!(pellet.ends_with("G0 Z50.600 F3000 ; lift clear of the print for finishing\nTURN_OFF_HEATERS ; zones + bed off\nM106 S0 ; fan off\nM84 ; disable steppers\n"));
        let filament = Profile {
            mode: Mode::Filament,
            ..Profile::default()
        };
        let text = write(&path, &filament, "x").unwrap().text;
        assert!(text.contains("M104 S205 ; set hotend temp (heats during bed wait + homing)\n"));
        assert!(text.contains("M106 S255 ; part cooling fan on after first layer\n"));
        // The end lift stops at the build height.
        let short = Profile {
            bed: [400.0, 400.0, 30.0],
            ..Profile::default()
        };
        let text = write(&path, &short, "x").unwrap().text;
        assert!(text.contains("G0 Z30.000 F3000 ; lift clear of the print for finishing\n"));
    }
}
