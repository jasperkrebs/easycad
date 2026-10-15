//! Golden tests: EasyCAD's G-code against G-code produced by EasyGCode itself.
//!
//! The fixtures in `fixtures/easygcode/` come from running EasyGCode's own `gcode.js` headlessly
//! (`scripts/easygcode-fixtures.cjs`; the commit is in `SOURCE`). Each case runs a *recipe*
//! built only from general steps with the same settings, and compares the G-code command by
//! command (comments aside): every command must be byte-identical, and the start and end blocks
//! too, comments included.
//!
//! - The spiral disc template (`web/templates/spiral-disc.easycad`) against EasyGCode's spiral
//!   disc: its defaults in volumetric flow mode, and three cases at a set speed.
//! - `vase_spiral` on a cylinder against EasyGCode's lampshade generator with a straight
//!   "cone" whose ends are the same size: the same vase mode (ramped first turn, whole layers,
//!   flat closing turn), at a set speed and in flow mode.

use crate::eval::{Quality, evaluate};
use crate::gcode::{self, Filament, Mode, Pellet, Profile};
use easycad_lang::{Literal, set_values};

const SPIRAL_DISC: &str = include_str!("../../../web/templates/spiral-disc.easycad");

struct Case {
    fixture: &'static str,
    values: &'static [(&'static str, f64)],
    /// Print at this speed (mm/s) instead of the template's volumetric flow.
    speed: Option<f64>,
    profile: fn() -> Profile,
}

const CASES: [Case; 4] = [
    // The template exactly as it ships: EasyGCode's defaults in flow mode (20 mm³/s).
    Case {
        fixture: include_str!("../../../fixtures/easygcode/sdisc-flow.gcode"),
        values: &[],
        speed: None,
        profile: Profile::default,
    },
    Case {
        fixture: include_str!("../../../fixtures/easygcode/sdisc-default.gcode"),
        values: &[],
        speed: Some(20.0),
        profile: Profile::default,
    },
    Case {
        fixture: include_str!("../../../fixtures/easygcode/sdisc-filament.gcode"),
        values: &[
            ("outer", 16.0),
            ("ring", 9.0),
            ("open", 250.0),
            ("band", 2.0),
            ("fade_in", 5.0),
            ("fade_out", 3.0),
            ("bumps", 15.0),
            ("layers", 2.0),
            ("layer", 0.3),
            ("line", 0.6),
        ],
        speed: Some(30.0),
        profile: || Profile {
            mode: Mode::Filament,
            multiplier: 0.95,
            end_lift: 30.0,
            travel_speed: 150.0,
            center: [128.0, 128.0],
            filament: Filament {
                diameter: 1.75,
                nozzle: 210.0,
                bed: 65.0,
                fan: 80.0,
            },
            ..Profile::default()
        },
    },
    Case {
        fixture: include_str!("../../../fixtures/easygcode/sdisc-bare.gcode"),
        values: &[
            ("outer", 20.0),
            ("ring", 10.0),
            ("open", 300.0),
            ("band", 0.0),
            ("fade_in", 20.0),
            ("fade_out", 0.0),
            ("bumps", 8.0),
            ("layers", 3.0),
            ("layer", 0.4),
            ("line", 1.2),
        ],
        speed: Some(15.0),
        profile: || Profile {
            include_start_end: false,
            center: [100.0, 100.0],
            pellet: Pellet::default(),
            ..Profile::default()
        },
    },
];

/// The commands of a G-code file: comments and blank lines removed.
fn commands(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| l.split(';').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Compares two command lines: same words, numbers equal to the last printed digit.
fn same_command(ours: &str, theirs: &str) -> Result<bool, String> {
    if ours == theirs {
        return Ok(true);
    }
    let (a, b): (Vec<&str>, Vec<&str>) = (
        ours.split_whitespace().collect(),
        theirs.split_whitespace().collect(),
    );
    if a.len() != b.len() {
        return Err(format!("`{ours}` vs EasyGCode's `{theirs}`"));
    }
    for (x, y) in a.iter().zip(&b) {
        if x == y {
            continue;
        }
        let (lx, nx) = x.split_at(1);
        let (ly, ny) = y.split_at(1);
        let tolerance = match lx {
            "E" => 1.01e-5,
            "X" | "Y" | "Z" => 1.01e-3,
            _ => 0.0,
        };
        match (lx == ly, nx.parse::<f64>(), ny.parse::<f64>()) {
            (true, Ok(vx), Ok(vy)) if (vx - vy).abs() <= tolerance => {}
            _ => return Err(format!("`{ours}` vs EasyGCode's `{theirs}`")),
        }
    }
    Ok(false)
}

/// The lines from the one starting with `from` through the one starting with `to`.
fn block<'a>(text: &'a str, from: &str, to: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.starts_with(from));
    let end = lines.iter().rposition(|l| l.starts_with(to));
    match (start, end) {
        (Some(s), Some(e)) if s <= e => lines[s..=e].to_vec(),
        _ => Vec::new(),
    }
}

/// Checks our G-code against EasyGCode's: every command identical, and the start and end blocks
/// identical comments included.
fn assert_matches(ours: &str, fixture: &str) {
    let (a, b) = (commands(ours), commands(fixture));
    assert_eq!(a.len(), b.len(), "command count differs from EasyGCode's");
    let mut identical = 0;
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        match same_command(x, y) {
            Ok(true) => identical += 1,
            Ok(false) => {}
            Err(msg) => panic!("command {}: {msg}", i + 1),
        }
    }
    // Every command is byte-identical today. (`same_command` already allows a difference in the
    // last printed digit, so a failure here means only that: two programs' floating-point
    // arithmetic rounding a hair differently. Investigate before loosening.)
    assert_eq!(
        identical,
        a.len(),
        "{} of {} commands differ in the last digit",
        a.len() - identical,
        a.len()
    );
    for (from, to) in [
        ("; --- start G-code", "; --- end of start G-code ---"),
        ("; --- end G-code", "M84"),
    ] {
        assert_eq!(block(ours, from, to), block(fixture, from, to));
    }
}

#[test]
fn spiral_disc_recipe_matches_easygcode() {
    for case in &CASES {
        let values: Vec<(String, Literal)> = case
            .values
            .iter()
            .map(|(k, v)| (k.to_string(), Literal::Number(*v)))
            .collect();
        let template = match case.speed {
            Some(v) => SPIRAL_DISC.replace("flow: flow", &format!("speed: {v}")),
            None => SPIRAL_DISC.to_string(),
        };
        let recipe = set_values(&template, &values).unwrap();
        let e = evaluate(&recipe, Quality::Final);
        assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
        let path = &e.print.as_ref().unwrap().path;
        let ours = gcode::write(path, &(case.profile)(), "Spiral disc")
            .unwrap()
            .text;
        assert_matches(&ours, case.fixture);
    }
}

/// EasyGCode's lampshade with both ends the same size is a cylinder in vase mode: 40 turns of
/// 0.5 mm at radius 20.4 mm, 192 points per turn, the seam at the back (+Y). Once at a set
/// speed, once in volumetric flow mode.
#[test]
fn vase_spiral_matches_easygcodes_vase_mode() {
    for (speed, fixture) in [
        (
            "speed: 20",
            include_str!("../../../fixtures/easygcode/lamp-cylinder.gcode"),
        ),
        (
            "flow: 8",
            include_str!("../../../fixtures/easygcode/lamp-cylinder-flow.gcode"),
        ),
    ] {
        let recipe = format!(
            "easycad 0.1
base = circle(20.4) |> rotate(90)
wall = loft([base, base |> move(z: 20)])
path = vase_spiral(wall, layer: 0.5, width: 0.8, {speed})
print path
"
        );
        let e = evaluate(&recipe, Quality::Final);
        assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
        let path = &e.print.as_ref().unwrap().path;
        assert_eq!((path.samples_per_turn, path.layers), (192, 40));
        let ours = gcode::write(path, &Profile::default(), "Cylinder")
            .unwrap()
            .text;
        assert_matches(&ours, fixture);
    }
}
