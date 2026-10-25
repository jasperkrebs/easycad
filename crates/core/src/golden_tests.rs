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
//!   flat closing turn), at a set speed and in flow mode. `spiralize` through the same
//!   cylinder's flat layers must print exactly the same.
//! - The thread connector template against EasyGCode's thread connector (`helix`, a brim and
//!   the hop over it), and the spoon template against its spoon (`spiral` with exact turns,
//!   `center`, `bead`, continuous joins, `stack` straight over): three cases each.
//!
//! `spiralize` through tilted layers and the toolpath tuning steps (`adjust`, `accelerate`,
//! `fan`, `dwell`) have no EasyGCode counterpart to compare with directly, so they're compared
//! with hand-checked fixtures instead (`fixtures/hand-checked/`, worked out from the geometry
//! in exact arithmetic by `scripts/hand-checked-fixtures.py`, every move annotated for checking
//! by hand).

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

/// The same cylinder as layer curves instead of a wall: `spiralize` through flat, evenly spaced
/// layers is EasyGCode's vase mode exactly.
#[test]
fn spiralize_through_flat_layers_matches_easygcodes_vase_mode() {
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
layers = base |> move(z: range(0, 20, count: 41))
path = spiralize(layers, width: 0.8, {speed})
print path
"
        );
        let e = evaluate(&recipe, Quality::Final);
        assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
        let path = &e.print.as_ref().unwrap().path;
        assert_eq!((path.samples_per_turn, path.layers), (192, 40));
        let ours = gcode::write(path, &Profile::default(), "Cylinder")
            .unwrap()
            .text;
        assert_matches(&ours, fixture);
    }
}

/// A diamond whose top is cut by a tilted plane, three layers tweened in between: every move is
/// extruded (and, at a set flow, sped up or slowed down) for its own layer height.
#[test]
fn spiralize_through_tilted_layers_matches_the_hand_checked_fixture() {
    let fixture = include_str!("../../../fixtures/hand-checked/spiralize-tilted.gcode");
    // The recipe is quoted in the fixture's header.
    let recipe: String = fixture
        .lines()
        .filter_map(|l| l.strip_prefix(";   "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(recipe.starts_with("easycad 0.1\n") && recipe.contains("spiralize("));
    let e = evaluate(&recipe, Quality::Final);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    let path = &e.print.as_ref().unwrap().path;
    let profile = Profile {
        include_start_end: false,
        ..Profile::default()
    };
    let ours = gcode::write(path, &profile, "Tilted diamond").unwrap().text;
    assert_matches(&ours, fixture);
}

/// Runs a template with some settings changed and compares its G-code with a fixture.
fn template_matches(template: &str, values: &[(&str, Literal)], profile: &Profile, fixture: &str) {
    let values: Vec<(String, Literal)> = values
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let recipe = set_values(template, &values).unwrap();
    let e = evaluate(&recipe, Quality::Final);
    assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
    let path = &e.print.as_ref().unwrap().path;
    let ours = gcode::write(path, profile, "Golden").unwrap().text;
    assert_matches(&ours, fixture);
}

/// The lampshade's "Steep curve" scales the overhang compensation by (angle / pivot)^steep: at 0
/// it's the compensation EasyGCode's lampshade uses (golden above); above 0 a bell, which leans
/// from upright to almost flat, gets narrower beads where it leans a little and wider ones where
/// it leans a lot (reaching the max width sooner), the same width where it's upright.
#[test]
fn the_lampshade_compensates_steep_overhangs_more_on_a_curve() {
    use Literal::{Number, Text};
    let widths_at = |steep: f64, pivot: f64| {
        let values = [
            ("shape".to_string(), Text("bell".into())),
            ("max_width".to_string(), Number(3.0)),
            ("steep".to_string(), Number(steep)),
            ("pivot".to_string(), Number(pivot)),
        ];
        let e = evaluate(&set_values(LAMPSHADE, &values).unwrap(), Quality::Final);
        assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
        let path = &e.print.as_ref().unwrap().path;
        (0..path.points.len())
            .filter(|&i| !path.travel[i])
            .map(|i| path.bead_of(i).0)
            .collect::<Vec<f64>>()
    };
    let widths = |steep: f64| widths_at(steep, 45.0);
    let (even, curved) = (widths(0.0), widths(2.0));
    assert_eq!(even.len(), curved.len());
    let line = 6.0;
    let (mut less, mut more) = (0, 0);
    for (a, b) in even.iter().zip(&curved) {
        assert!(*b <= 3.0 * line + 1e-9);
        if (a - line).abs() < 1e-12 {
            assert!((b - line).abs() < 1e-12, "upright, yet {b} wide");
        } else if b < a {
            less += 1;
        } else if b > a {
            more += 1;
        }
    }
    assert!(less > 0 && more > 0, "{less} narrower, {more} wider");
    // Steep enough, both reach the max width; on the curve it gets there lower down.
    let capped = |w: &[f64]| w.iter().filter(|w| (*w - 3.0 * line).abs() < 1e-9).count();
    // A higher pivot: as now at a steeper angle, so never wider than at 45°, and narrower where
    // it leans.
    let later = widths_at(2.0, 70.0);
    assert!(later.iter().zip(&curved).all(|(a, b)| a <= b));
    assert!(later.iter().zip(&curved).any(|(a, b)| a < b));
    assert!(
        capped(&curved) > capped(&even),
        "{} capped, was {}",
        capped(&curved),
        capped(&even)
    );
}

const THREAD: &str = include_str!("../../../web/templates/thread-connector.easycad");

/// The thread connector template against EasyGCode's thread connector: its defaults (M8,
/// 20 mm, no brim); an M12 with a brim of its own layer height on the filament printer (the hop
/// over the brim at twice its layer height, the fan after the helix's first turn, a fraction of
/// a turn at the top, a tighter fit); and an M3 with a set line width, a brim of another width
/// and no start/end blocks.
#[test]
fn thread_connector_recipe_matches_easygcode() {
    use Literal::{Bool, Number, Text};
    template_matches(
        THREAD,
        &[],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/thread-default.gcode"),
    );
    template_matches(
        THREAD,
        &[
            ("size", Text("M12".into())),
            ("height", Number(15.5)),
            ("fit", Number(-0.1)),
            ("flow", Number(15.0)),
            ("brim", Bool(true)),
            ("brim_lines", Number(3.0)),
            ("brim_layer", Number(0.8)),
            ("brim_flow", Number(10.0)),
        ],
        &Profile {
            mode: Mode::Filament,
            end_lift: 30.0,
            travel_speed: 150.0,
            filament: Filament {
                diameter: 1.75,
                nozzle: 215.0,
                bed: 65.0,
                fan: 100.0,
            },
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/thread-brim.gcode"),
    );
    template_matches(
        THREAD,
        &[
            ("size", Text("M3".into())),
            ("height", Number(6.0)),
            ("fit", Number(0.05)),
            ("line", Number(1.0)),
            ("flow", Number(5.0)),
            ("tolerance", Number(0.02)),
            ("brim", Bool(true)),
            ("brim_lines", Number(2.0)),
            ("brim_line", Number(1.2)),
            ("brim_flow", Number(5.0)),
        ],
        &Profile {
            include_start_end: false,
            center: [100.0, 110.0],
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/thread-bare.gcode"),
    );
}

const SPOON: &str = include_str!("../../../web/templates/spoon.easycad");

/// The spoon template against EasyGCode's spoon: its defaults in volumetric flow mode; one
/// printed from the stick's tip with a thicker stick bead on the filament printer; and one with
/// no stick, one layer and no start/end blocks.
#[test]
fn spoon_recipe_matches_easygcode() {
    use Literal::{Bool, Number};
    template_matches(
        SPOON,
        &[],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/spoon-flow.gcode"),
    );
    template_matches(
        SPOON,
        &[
            ("turns", Number(4.5)),
            ("start_radius", Number(5.0)),
            ("stick", Number(30.0)),
            ("layers", Number(2.0)),
            ("from_stick", Bool(true)),
            ("stick_line", Number(3.0)),
            ("stick_layer", Number(0.8)),
            ("layer", Number(0.4)),
            ("line", Number(1.2)),
            ("flow", Number(12.0)),
        ],
        &Profile {
            mode: Mode::Filament,
            end_lift: 30.0,
            travel_speed: 150.0,
            center: [128.0, 128.0],
            filament: Filament {
                diameter: 1.75,
                nozzle: 210.0,
                bed: 60.0,
                fan: 80.0,
            },
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/spoon-stick.gcode"),
    );
    template_matches(
        SPOON,
        &[
            ("turns", Number(3.0)),
            ("start_radius", Number(2.0)),
            ("stick", Number(0.0)),
            ("layers", Number(1.0)),
            ("layer", Number(0.5)),
            ("line", Number(1.5)),
            ("flow", Number(10.0)),
        ],
        &Profile {
            include_start_end: false,
            center: [110.0, 120.0],
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/spoon-bare.gcode"),
    );
}

const STOOL: &str = include_str!("../../../web/templates/bend-stool.easycad");

/// The bend stool template against EasyGCode's bend stool (its seat, without legs): as it
/// ships, one layer in volumetric flow mode; foaming over four layers at EasyGCode's set feed
/// (the heat-up after the first layer, the waits on primer lines at the bed's corner, the
/// overrides, the hops at twice the tallest print so far); foaming with a cool-down below the
/// printer's own temperatures and a reheat, a zigzag cool-down primer, primers at a set flow,
/// another size and the print centred elsewhere; and three plain layers without start/end
/// blocks (the wipe between layers).
#[test]
fn bend_stool_recipe_matches_easygcode() {
    use Literal::{Bool, Number};
    template_matches(
        STOOL,
        &[],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/stool-flow.gcode"),
    );
    template_matches(
        &STOOL.replace("flow: flow", "speed: 30"),
        &[("layers", Number(4.0)), ("foam", Bool(true))],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/stool-foam.gcode"),
    );
    template_matches(
        STOOL,
        &[
            ("diameter", Number(61.0)),
            ("layers", Number(5.0)),
            ("layer", Number(0.5)),
            ("line", Number(1.5)),
            ("flow", Number(15.0)),
            ("tolerance", Number(0.04)),
            ("foam", Bool(true)),
            ("foam_up", Number(240.0)),
            ("foam_mid", Number(235.0)),
            ("foam_down", Number(230.0)),
            ("foam_extrusion", Number(45.0)),
            ("exit_cool", Number(185.0)),
            ("primer_x", Number(-150.0)),
            ("primer_y", Number(-140.0)),
            ("enter_length", Number(30.0)),
            ("enter_width", Number(1.8)),
            ("enter_layer", Number(0.4)),
            ("enter_flow", Number(5.0)),
            ("exit_length", Number(50.0)),
            ("exit_lines", Number(3.0)),
            ("exit_width", Number(1.6)),
            ("exit_layer", Number(0.35)),
            ("exit_flow", Number(4.0)),
        ],
        &Profile {
            center: [150.0, 140.0],
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/stool-cool.gcode"),
    );
    template_matches(
        &STOOL.replace("flow: flow", "speed: 25"),
        &[
            ("diameter", Number(30.0)),
            ("layers", Number(3.0)),
            ("layer", Number(0.4)),
            ("line", Number(1.2)),
        ],
        &Profile {
            include_start_end: false,
            center: [100.0, 110.0],
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/stool-bare.gcode"),
    );
}

/// The toolpath tuning steps (`adjust`, `accelerate`, `fan`, `dwell`) write speeds, fan levels
/// and pauses that EasyGCode has no single project for, so they're compared with a hand-checked
/// fixture: the moves between the printer's start and end blocks (those are EasyGCode's, and
/// golden-tested above).
#[test]
fn toolpath_tuning_matches_the_hand_checked_fixture() {
    let fixture = include_str!("../../../fixtures/hand-checked/machine-events.gcode");
    let recipe: String = fixture
        .lines()
        .filter_map(|l| l.strip_prefix(";   "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(recipe.starts_with("easycad 0.1\n") && recipe.contains("accelerate("));
    let e = evaluate(&recipe, Quality::Final);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    let path = &e.print.as_ref().unwrap().path;
    let ours = gcode::write(path, &Profile::default(), "Tuned square")
        .unwrap()
        .text;
    let body: String = ours
        .lines()
        .skip_while(|l| !l.starts_with("; --- end of start G-code"))
        .skip(1)
        .take_while(|l| !l.starts_with("; --- end G-code"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_matches(&body, fixture);
}

const LAMPSHADE: &str = include_str!("../../../web/templates/lampshade.easycad");

/// The lampshade template against EasyGCode's lampshade (`vase_revolve` and the shades as
/// profiles): as it ships (E27, a cone with a fillet, width compensation, volumetric flow); a
/// capped bell on an E14 with layer-height compensation and a brim at a set feed; a sphere with
/// a lip, wide edge down, its flow ramping over the fillet; and a dome on a custom thread
/// without compensation, with a set ramp height and no start/end blocks.
#[test]
fn lampshade_recipe_matches_easygcode() {
    use Literal::{Bool, Number, Text};
    template_matches(
        LAMPSHADE,
        &[],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/lampshade-flow.gcode"),
    );
    template_matches(
        &LAMPSHADE.replace("flow: flow_at", "speed: 20"),
        &[
            ("socket", Text("E14".into())),
            ("fit", Number(-0.2)),
            ("shape", Text("bell".into())),
            ("throat", Number(15.0)),
            ("opening", Number(180.0)),
            ("transition", Number(60.0)),
            ("max_angle", Number(55.0)),
            ("compensation", Text("layer height".into())),
            ("strength", Number(80.0)),
            ("brim", Bool(true)),
            ("brim_lines", Number(3.0)),
            ("brim_width", Number(5.0)),
            ("brim_layer", Number(2.0)),
        ],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/lampshade-bell.gcode"),
    );
    template_matches(
        LAMPSHADE,
        &[
            ("shape", Text("sphere".into())),
            ("throat", Number(12.0)),
            ("opening", Number(120.0)),
            ("sphere", Number(220.0)),
            ("lip", Bool(true)),
            ("lip_length", Number(8.0)),
            ("wide_down", Bool(true)),
            ("max_width", Number(2.0)),
            ("flow", Number(18.0)),
            ("shade_flow", Number(30.0)),
        ],
        &Profile::default(),
        include_str!("../../../fixtures/easygcode/lampshade-sphere.gcode"),
    );
    template_matches(
        LAMPSHADE,
        &[
            ("socket", Text("custom".into())),
            ("thread_d", Number(35.0)),
            ("thread_pitch", Number(3.0)),
            ("shape", Text("dome".into())),
            ("throat", Number(10.0)),
            ("fillet", Number(25.0)),
            ("opening", Number(160.0)),
            ("transition", Number(70.0)),
            ("compensation", Text("off".into())),
            ("flow", Number(15.0)),
            ("shade_flow", Number(25.0)),
            ("flow_ramp", Number(30.0)),
            ("line", Number(5.0)),
            ("tolerance", Number(0.08)),
        ],
        &Profile {
            include_start_end: false,
            center: [150.0, 150.0],
            ..Profile::default()
        },
        include_str!("../../../fixtures/easygcode/lampshade-dome.gcode"),
    );
}

/// The lampshade with an open top (`flat_top: false`) prints exactly EasyGCode's lampshade
/// without its closing turn: the spiral stops on the rim at full flow, and the end block follows.
/// As it ships (width compensation, volumetric flow) and the capped bell (layer-height
/// compensation, a brim, a set feed).
#[test]
fn an_open_lampshade_is_easygcodes_without_the_closing_turn() {
    use Literal::{Bool, Number, Text};
    let bell = [
        ("socket", Text("E14".into())),
        ("fit", Number(-0.2)),
        ("shape", Text("bell".into())),
        ("throat", Number(15.0)),
        ("opening", Number(180.0)),
        ("transition", Number(60.0)),
        ("max_angle", Number(55.0)),
        ("compensation", Text("layer height".into())),
        ("strength", Number(80.0)),
        ("brim", Bool(true)),
        ("brim_lines", Number(3.0)),
        ("brim_width", Number(5.0)),
        ("brim_layer", Number(2.0)),
    ];
    type Values<'a> = &'a [(&'a str, Literal)];
    let cases: [(String, Values, &str); 2] = [
        (
            LAMPSHADE.to_string(),
            &[],
            include_str!("../../../fixtures/easygcode/lampshade-flow.gcode"),
        ),
        (
            LAMPSHADE.replace("flow: flow_at", "speed: 20"),
            &bell,
            include_str!("../../../fixtures/easygcode/lampshade-bell.gcode"),
        ),
    ];
    for (template, values, fixture) in cases {
        let run = |flat_top: bool| {
            let mut values: Vec<(String, Literal)> = values
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            values.push(("flat_top".into(), Bool(flat_top)));
            let recipe = set_values(&template, &values).unwrap();
            let e = evaluate(&recipe, Quality::Final);
            assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
            e.print.unwrap().path
        };
        let (flat, open) = (run(true), run(false));
        let cap = flat.points.len() - open.points.len();
        assert!(cap >= 24, "the closing turn has at least 24 points");
        // EasyGCode's G-code without the closing turn: the last `cap` moves before the end block.
        let lines: Vec<&str> = fixture.lines().collect();
        let end = lines
            .iter()
            .position(|l| l.starts_with("; --- end G-code"))
            .unwrap();
        let mut dropped = 0;
        let mut kept: Vec<&str> = Vec::new();
        for (i, l) in lines.iter().enumerate().rev() {
            if i < end && dropped < cap && l.starts_with("G1 ") {
                dropped += 1;
            } else {
                kept.push(l);
            }
        }
        assert_eq!(dropped, cap);
        kept.reverse();
        let expected: String = kept.iter().map(|l| format!("{l}\n")).collect();
        let ours = gcode::write(&open, &Profile::default(), "Golden")
            .unwrap()
            .text;
        assert_matches(&ours, &expected);
    }
}

/// The machine events on the filament printer (`M104`, `M109`, `M220`, `M221`, back to the
/// printer's own temperature) have no EasyGCode counterpart (its foaming is for the pellet
/// printer, golden-tested above), so they're compared with a hand-checked fixture. The moves
/// between the printer's start and end blocks, as for the tuning steps.
#[test]
fn machine_events_on_the_filament_printer_match_the_hand_checked_fixture() {
    let fixture = include_str!("../../../fixtures/hand-checked/filament-machine.gcode");
    let recipe: String = fixture
        .lines()
        .filter_map(|l| l.strip_prefix(";   "))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(recipe.starts_with("easycad 0.1\n") && recipe.contains("wait_temperature("));
    let e = evaluate(&recipe, Quality::Final);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    let path = &e.print.as_ref().unwrap().path;
    let profile = Profile {
        mode: Mode::Filament,
        bed: [256.0, 256.0, 256.0],
        filament: Filament {
            nozzle: 210.0,
            ..Filament::default()
        },
        ..Profile::default()
    };
    let ours = gcode::write(path, &profile, "Filament events")
        .unwrap()
        .text;
    let body: String = ours
        .lines()
        .skip_while(|l| !l.starts_with("; --- end of start G-code"))
        .skip(1)
        .take_while(|l| !l.starts_with("; --- end G-code"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_matches(&body, fixture);
    // The stats count the overrides: layer 1 (80 mm) and the primer (10 mm) as written, layer 2
    // (80 mm) at 80 % extrusion.
    let st = path.stats();
    let full = 2.0 * 80.0 + 10.0;
    let counted = 80.0 + 10.0 + 0.8 * 80.0;
    assert!((st.volume_mm3 / path.parts[0].area() - counted).abs() < 1e-9 * full);
}
