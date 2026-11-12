//! The lamp builder (`docs/lamp-builder.md`): shades screwed into each other.

use crate::eval::{Evaluation, Quality, Severity, evaluate};
use easycad_lang::{Literal, add_part, set_values};

const LAMP: &str = include_str!("../../../web/templates/lamp.easycad");

fn run(src: &str) -> Evaluation {
    evaluate(src, Quality::Final)
}

fn with_part(src: &str, name: &str, values: &[(&str, Literal)]) -> String {
    let values: Vec<(String, Literal)> = values
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    add_part(src, "shade", name, &values).unwrap()
}

fn warnings(e: &Evaluation) -> Vec<&str> {
    e.diagnostics
        .iter()
        .map(|d| {
            assert_ne!(d.severity, Severity::Error, "{:?}", e.diagnostics);
            d.message.as_str()
        })
        .collect()
}

fn port<'e>(e: &'e Evaluation, name: &str) -> &'e [f64] {
    &e.ports.iter().find(|p| p.name == name).unwrap().value
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// As it ships: one shade, a sphere with a straight end, on an E27 socket. Its throat runs half
/// a line outside the socket's 40 mm thread; its ports: the throat's outside (the socket fills
/// its inside), and both faces of its straight end, a line plus the fit away.
#[test]
fn the_lamp_starts_with_one_shade_on_the_socket() {
    let e = run(LAMP);
    assert!(warnings(&e).is_empty());
    assert_eq!(e.prints.len(), 1);
    assert_eq!(e.prints[0].name, "main");
    assert_eq!(port(&e, "main.start_out")[..4], [29.0, 0.0, 1.0, 20.0]);
    assert_eq!(port(&e, "main.start_in")[3], 0.0, "the socket's");
    let end = port(&e, "main.end_out");
    assert!(
        close(end[0], 75.0 + 6.0) && end[2] == -1.0 && end[3] == 20.0,
        "{end:?}"
    );
    assert!(close(port(&e, "main.end_in")[0], 75.0 - 6.0));
    // The fit between shades moves every joint.
    let looser = set_values(LAMP, &[("joint_fit".into(), Literal::Number(0.3))]).unwrap();
    assert!(close(port(&run(&looser), "main.end_out")[0], 81.3));
}

/// A tube screwed onto the sphere's straight end, going up: it runs a line outside the end,
/// its start (the holder) along the end's last 20 mm; printed on the bed as any shade, shown in
/// its place. A cone screwed onto the throat, going down, hangs below it, turned over. Nothing
/// runs into anything.
#[test]
fn shades_screw_onto_each_other() {
    use Literal::{Number, Text};
    let src = with_part(
        LAMP,
        "tube",
        &[
            ("mount", Text("main.end_out".into())),
            ("shape", Text("tube".into())),
            ("length", Number(150.0)),
        ],
    );
    let src = with_part(
        &src,
        "skirt",
        &[
            ("mount", Text("main.start_out".into())),
            ("direction", Text("down".into())),
            ("opening", Number(120.0)),
            ("transition", Number(40.0)),
        ],
    );
    let e = run(&src);
    assert!(warnings(&e).is_empty(), "{:?}", e.diagnostics);
    assert_eq!(
        e.prints.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["main", "tube", "skirt"]
    );
    let end = port(&e, "main.end_out").to_vec();
    let tube = &e.prints[1].path;
    let r = tube.points[5][0].hypot(tube.points[5][1]);
    assert!(close(r, end[0]), "{r}");
    // Printed from the bed up, shown with its start 20 mm below the sphere's end.
    assert!(tube.points[0][2] < 3.0);
    let placed = tube.view.unwrap().apply(tube.points[0]);
    assert!((placed[2] - (end[1] - 20.0)).abs() < 3.0, "{placed:?}");
    // Its own ports run its whole length, both ends.
    let start = port(&e, "tube.start_out");
    assert!(close(start[0], end[0] + 6.0) && start[3] == 150.0);
    // The skirt: turned over, below the throat.
    let skirt = &e.prints[2].path;
    let lowest = skirt
        .points
        .iter()
        .map(|q| skirt.view.unwrap().apply(*q)[2])
        .fold(f64::MAX, f64::min);
    assert!(lowest < -30.0, "{lowest}");
    // Each prints: G-code for every shade.
    for p in &e.prints {
        crate::gcode::write(&p.path, &crate::gcode::Profile::default(), "Lamp").unwrap();
    }
    // Taking the sphere away takes everything mounted on it.
    let gone = easycad_lang::remove_part(&src, "main").unwrap();
    assert!(!gone.lines().any(|l| l.starts_with("part ")));
}

/// A cone on the outside of the throat, going up, would cut through the sphere: a warning names
/// both and the height.
#[test]
fn shades_that_run_into_each_other_are_reported() {
    let src = with_part(
        LAMP,
        "bad",
        &[("mount", Literal::Text("main.start_out".into()))],
    );
    let e = run(&src);
    let w = warnings(&e);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].starts_with("`main` and `bad` run into each other at "),
        "{w:?}"
    );
}

/// The new shapes: a bell and a cone that open out to their widest and close in again, and a
/// tube; each ends straight (more shades screw on there) at its end diameter.
#[test]
fn shades_open_out_and_close_in_again() {
    use Literal::{Bool, Number, Text};
    for (shape, widest) in [
        ("bell in-out", 100.0),
        ("cone in-out", 100.0),
        ("tube", 23.0),
    ] {
        let src = set_values(
            LAMP,
            &[
                ("main.shape".into(), Text(shape.into())),
                ("main.end_d".into(), Number(80.0)),
                ("main.end_straight".into(), Bool(true)),
            ],
        )
        .unwrap();
        let e = run(&src);
        assert!(warnings(&e).is_empty(), "{shape}: {:?}", e.diagnostics);
        let p = &e.prints[0].path;
        let r = |q: &[f64; 3]| q[0].hypot(q[1]);
        let rmax = p.points.iter().map(r).fold(0.0, f64::max);
        assert!((rmax - widest).abs() < 0.5, "{shape}: {rmax}");
        let end = port(&e, "main.end_out");
        let end_r = if shape == "tube" { 23.0 } else { 40.0 };
        assert!(close(end[0], end_r + 6.0), "{shape}: {end:?}");
        assert!(end[3] > 0.0);
        // Straight at the end: the last turns all at the end radius.
        let last = p.points.len() - 1;
        assert!((r(&p.points[last]) - end_r).abs() < 0.01, "{shape}");
        assert!(
            (r(&p.points[last - 3 * p.samples_per_turn.max(24)]) - end_r).abs() < 0.01,
            "{shape}"
        );
    }
}

/// Each shade's surface (shown inside the component, not made from its print) belongs to its
/// part, so the view can colour it, see through it or hide it with its shade.
#[test]
fn each_shades_surface_belongs_to_its_shade() {
    let src = with_part(
        LAMP,
        "tube",
        &[
            ("mount", Literal::Text("main.end_out".into())),
            ("shape", Literal::Text("tube".into())),
        ],
    );
    let e = run(&src);
    let owners: Vec<Option<usize>> = e.drawing.mesh_parts.iter().map(|(_, p)| *p).collect();
    assert_eq!(owners, [Some(0), Some(1)]);
}

/// A shade that opens out into its straight end (a bell flat at its rim, a cone) rounds into it
/// with the fillet, as a sphere closing in does: the wall's angle comes down to vertical in
/// small steps, with no corner, and the straight end then carries on at the opening's radius.
#[test]
fn shades_that_open_out_round_into_their_straight_end() {
    use Literal::{Bool, Number, Text};
    for shape in ["bell", "cone", "sphere"] {
        let src = set_values(
            LAMP,
            &[
                ("main.shape".into(), Text(shape.into())),
                ("main.end_straight".into(), Bool(true)),
                ("main.end_len".into(), Number(60.0)),
                ("main.fillet".into(), Number(100.0)),
            ],
        )
        .unwrap();
        let (e, v) = crate::eval::step_values(&src, Quality::Final);
        assert!(e.errors().next().is_none(), "{shape}: {:?}", e.diagnostics);
        let list = |n: &str| match v.get(n) {
            Some(crate::value::Value::List(l)) => l
                .items
                .iter()
                .map(|x| match x {
                    crate::value::Value::Number(k) => *k,
                    _ => f64::NAN,
                })
                .collect::<Vec<f64>>(),
            _ => panic!("{n}"),
        };
        let (r, l) = (list("main.full_r"), list("main.full_l"));
        let n = r.len();
        assert_eq!(l[n - 1], 0.0, "{shape}: straight at the end");
        assert!(
            (r[n - 1] - r[n - 2]).abs() < 1e-9,
            "{shape}: the straight end"
        );
        // From the shade into the straight end: no step in the wall's angle over 6°.
        let worst = l
            .windows(2)
            .skip(n / 2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        assert!(worst < 6.0, "{shape}: a corner of {worst}°");
    }
}

/// The Plan's marks lead to the settings that drive each piece: a shade's are its own (inside
/// the component, `fillet` is `main.fillet`), placed where it goes; a straight end only has one
/// when it's there. A mark naming no setting is a warning.
#[test]
fn marks_lead_to_each_shades_own_settings() {
    let e = run(LAMP);
    let mark = |label: &str| e.marks.iter().find(|m| m.label.as_deref() == Some(label));
    assert_eq!(mark("Fillet").unwrap().settings, ["main.fillet"]);
    assert!(mark("Straight end").is_some(), "the sphere ends straight");
    let start = &mark("Start").unwrap().lines[0];
    assert_eq!((start[0][0], start[0][2]), (23.0, 0.0));
    let tube = with_part(
        LAMP,
        "tube",
        &[
            ("mount", Literal::Text("main.end_out".into())),
            ("shape", Literal::Text("tube".into())),
        ],
    );
    let e = run(&tube);
    let tube_marks: Vec<&str> = e
        .marks
        .iter()
        .filter(|m| m.settings.iter().all(|s| s.starts_with("tube.")))
        .filter_map(|m| m.label.as_deref())
        .collect();
    assert_eq!(tube_marks, ["Start"], "a tube is one straight piece");
    let lampshade = include_str!("../../../web/templates/lampshade.easycad");
    let e = run(lampshade);
    assert!(e.marks.iter().any(|m| m.settings == ["fillet"]));
    assert!(
        e.marks.iter().all(|m| m.label.as_deref() != Some("Lip")),
        "no lip on a cone"
    );
    let broken = lampshade.replace(
        "mark pick(not_tube, mark_fillet, []) for fillet",
        "mark pick(not_tube, mark_fillet, []) for filet",
    );
    let e = run(&broken);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.message == "`mark … for filet`: there's no setting `filet`")
    );
}

/// A bell or dome wider than it is tall says how to fix it instead of failing deep inside.
#[test]
fn a_shade_too_wide_for_its_height_says_so() {
    use Literal::{Number, Text};
    for (shape, words) in [
        ("bell", "This bell is wider"),
        ("dome", "This dome is wider"),
    ] {
        let src = set_values(
            LAMP,
            &[
                ("main.shape".into(), Text(shape.into())),
                ("main.opening".into(), Number(300.0)),
            ],
        )
        .unwrap();
        let e = run(&src);
        let errors: Vec<_> = e.errors().map(|d| d.message.as_str()).collect();
        assert_eq!(errors.len(), 1, "{shape}: {:?}", e.diagnostics);
        assert!(errors[0].starts_with(words), "{}", errors[0]);
    }
    let src = set_values(
        LAMP,
        &[
            ("main.shape".into(), Text("bell".into())),
            ("main.opening".into(), Number(300.0)),
            ("main.max_angle".into(), Number(60.0)),
        ],
    )
    .unwrap();
    let e = run(&src);
    assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
}

/// A closed end: the shade's end shut by a flat disc that rises through a fillet into the wall,
/// printed disc down. Its print starts on the bed with the disc (rings in a spiral, out to where
/// the fillet starts), then the shade as any shade prints, from there in one spiral to the
/// start's thread, compensated where it leans (its fillet leaves the disc flat) and a pitch a turn
/// where it's straight; shown where it goes (its disc at the top of a shade going up), its
/// end with no ports. The same shade open prints as before.
#[test]
fn a_shade_with_a_closed_end_prints_its_disc_first() {
    use Literal::{Bool, Number, Text};
    let with = |cap: bool| {
        set_values(
            LAMP,
            &[
                ("main.shape".into(), Text("bell in-out".into())),
                ("main.end_d".into(), Number(120.0)),
                ("main.closed".into(), Bool(cap)),
                ("main.closed_fillet".into(), Number(15.0)),
            ],
        )
        .unwrap()
    };
    let e = run(&with(true));
    assert!(warnings(&e).is_empty(), "{:?}", e.diagnostics);
    let p = &e.prints[0].path;
    // On the bed first, at the disc's layer (the pitch), round the middle.
    let first = p.points.iter().position(|q| q[2] > 0.0).unwrap();
    assert!(
        (p.points[first][2] - 2.5).abs() < 1e-6,
        "{:?}",
        p.points[first]
    );
    let r = |q: &[f64; 3]| q[0].hypot(q[1]);
    let disc: Vec<f64> = p
        .points
        .iter()
        .filter(|q| (q[2] - 2.5).abs() < 1e-6)
        .map(r)
        .collect();
    assert!(
        disc.iter().cloned().fold(f64::MAX, f64::min) < 10.0,
        "the disc fills the middle"
    );
    // Up to the thread at the start's radius (23 mm on the E27), the last turns a pitch apart.
    let last = p.points.len() - 1;
    assert!(
        (r(&p.points[last]) - 23.0).abs() < 0.5,
        "{}",
        r(&p.points[last])
    );
    let top = p.points[last][2];
    let a_turn_down = p.points.iter().rev().find(|q| q[2] < top - 2.4).unwrap();
    assert!(
        (top - a_turn_down[2] - 2.5).abs() < 0.1,
        "{} {}",
        top,
        a_turn_down[2]
    );
    // The fillet is compensated as every shade is (layer height, the default): where it leaves
    // the disc, nearly flat, its turns rise far less than a pitch.
    let starts: Vec<f64> = (0..p.points.len())
        .filter(|&i| p.turn[i] >= 1.0 && p.turn[i].fract() == 0.0 && !p.travel[i])
        .map(|i| p.points[i][2])
        .collect();
    // One travel, to the disc's start: from the disc into the wall it carries straight on.
    assert_eq!(p.travel.iter().filter(|t| **t).count(), 1, "travels");
    let first_rise = starts[1] - starts[0];
    assert!(
        first_rise < 1.0,
        "the fillet's first turn rises {first_rise}"
    );
    // Shown going up from the socket: the disc at the top, the thread at the bottom.
    let x = p.view.unwrap();
    let shown_first = x.apply(p.points[first]);
    let shown_last = x.apply(p.points[last]);
    assert!(
        shown_first[2] > shown_last[2] + 100.0,
        "{shown_first:?} {shown_last:?}"
    );
    assert!(shown_last[2] < 25.0);
    assert_eq!(
        port(&e, "main.end_out")[3],
        0.0,
        "closed: nothing mounts on its end"
    );
    assert!(e.marks.iter().any(|m| m.settings == ["main.closed_fillet"]));
    crate::gcode::write(p, &crate::gcode::Profile::default(), "Lamp").unwrap();
    // Open, it's the vase as before.
    let open = run(&with(false));
    assert!(open.prints[0].path.points[1][2] < 2.5);
    assert!(
        port(&open, "main.end_out")[3] > 0.0,
        "open, its straight end takes shades again"
    );
}

/// A straight start 122–126 mm long met its fillet a rounding error lower (124.99999999999999
/// after 125), and vase_revolve took that for the profile going down. The owner's shade: a cone
/// with a 125 mm start, a 20 mm fillet, opening 195 mm over 107 mm, a 53 mm straight end.
#[test]
fn a_straight_start_of_any_length_meets_its_fillet() {
    use Literal::{Bool, Number, Text};
    for start in [122.0, 123.0, 124.0, 125.0, 126.0] {
        let src = set_values(
            LAMP,
            &[
                ("main.shape".into(), Text("cone".into())),
                ("main.start_len".into(), Number(start)),
                ("main.fillet".into(), Number(20.0)),
                ("main.opening".into(), Number(195.0)),
                ("main.transition".into(), Number(107.0)),
                ("main.end_straight".into(), Bool(true)),
                ("main.end_len".into(), Number(53.0)),
            ],
        )
        .unwrap();
        let e = run(&src);
        assert!(e.errors().next().is_none(), "{start}: {:?}", e.diagnostics);
    }
}

/// The closed end's disc runs straight on into the wall: its spiral ends on the wall's first
/// point (no step out onto the fillet), whatever the slicing or the compensation, in the lamp
/// and in the single lampshade. The owner's
/// shade (12 Nov 2026): a cone opening to 195 mm with an 18 mm closed fillet stepped 1.4 mm out
/// from the disc's edge to the wall's first turn.
#[test]
fn a_closed_end_runs_from_its_disc_into_the_wall_without_a_step() {
    use Literal::{Bool, Number, Text};
    // The lamp's shade, and the single lampshade (the same arithmetic, its own names).
    let lampshade = include_str!("../../../web/templates/lampshade.easycad");
    for ((slicing, comp, layer), (template, part)) in [
        ("height", "layer height", 0.0),
        ("height", "width", 0.0),
        ("height", "off", 1.5),
        ("surface", "layer height", 0.0),
    ]
    .into_iter()
    .flat_map(|c| [(c, (LAMP, "main.")), (c, (lampshade, ""))])
    {
        let own = |name: &str| format!("{part}{name}");
        let src = set_values(
            template,
            &[
                (own("shape"), Text("cone".into())),
                (own("opening"), Number(195.0)),
                (own("transition"), Number(107.0)),
                (own("fillet"), Number(20.0)),
                (own("closed"), Bool(true)),
                (own("closed_fillet"), Number(18.0)),
                (own("closed_layer"), Number(layer)),
                ("slicing".into(), Text(slicing.into())),
                ("compensation".into(), Text(comp.into())),
            ],
        )
        .unwrap();
        let e = run(&src);
        assert!(warnings(&e).is_empty(), "{:?}", e.diagnostics);
        let p = &e.prints[0].path;
        let disc_z = p.points.iter().find(|q| q[2] > 0.0).unwrap()[2];
        let r = |q: [f64; 3]| q[0].hypot(q[1]);
        let ang = |q: [f64; 3]| q[1].atan2(q[0]);
        // Every move on the disc (past its middle) steps out about as fast as its spiral does (a
        // line a turn), the last one onto the wall's first point too.
        let on_disc: Vec<usize> = (1..p.points.len())
            .filter(|&i| p.points[i][2] == disc_z && p.points[i - 1][2] == disc_z)
            .filter(|&i| r(p.points[i - 1]) > 12.0)
            .collect();
        assert!(on_disc.len() > 50);
        for &i in &on_disc {
            let (a, b) = (p.points[i - 1], p.points[i]);
            let turned = (ang(b) - ang(a)).rem_euclid(std::f64::consts::TAU);
            let allowed = 1.1 * 6.0 * turned / std::f64::consts::TAU + 0.05;
            assert!(
                (r(b) - r(a)).abs() <= allowed,
                "{slicing} {comp}: a step out from r {} to {} at {a:?}",
                r(a),
                r(b)
            );
        }
        // Then the wall rises from there, round the same circle.
        let last = *on_disc.last().unwrap();
        let next = p.points[last + 1];
        assert!(next[2] > disc_z && (r(next) - r(p.points[last])).abs() < 1e-6);
        // The disc fills the middle, a line apart.
        assert!(p.points.iter().any(|q| q[2] == disc_z && r(*q) < 3.0));
    }
}

/// A closed end's fillet meets a curved wall tangent: on a bell in-out (an S-curve where it
/// ends), the profile's lean never jumps where the fillet ends, in the lamp or the lampshade.
#[test]
fn a_closed_ends_fillet_meets_a_curved_wall_tangent() {
    use Literal::{Bool, Number, Text};
    let lampshade = include_str!("../../../web/templates/lampshade.easycad");
    for (template, part) in [(LAMP, "main."), (lampshade, "")] {
        let own = |name: &str| format!("{part}{name}");
        let src = set_values(
            template,
            &[
                (own("shape"), Text("bell in-out".into())),
                (own("end_d"), Number(120.0)),
                (own("closed"), Bool(true)),
                (own("closed_fillet"), Number(25.0)),
            ],
        )
        .unwrap();
        let (e, v) = crate::eval::step_values(&src, Quality::Final);
        assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
        let Some(crate::value::Value::List(lean)) = v.get(&own("print_l")) else {
            panic!("no lean list");
        };
        let lean: Vec<f64> = lean
            .items
            .iter()
            .map(|x| match x {
                crate::value::Value::Number(n) => *n,
                _ => f64::NAN,
            })
            .collect();
        let worst = lean
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f64::max);
        assert!(worst < 12.0, "{part}: the lean jumps {worst}° ({lean:?})");
    }
}

/// A brim is a print aid (`aid`): it prints, but it isn't part of the shade, so a shade screwed
/// onto the throat right where the brim lies on the bed (a short tube round it) doesn't run into
/// it. Unmarked, it would.
#[test]
fn a_brim_is_a_print_aid_that_nothing_runs_into() {
    use Literal::{Bool, Number, Text};
    let with = |src: &str| {
        let src = set_values(src, &[("brim".into(), Bool(true))]).unwrap();
        with_part(
            &src,
            "cone",
            &[
                ("mount", Text("main.start_out".into())),
                ("along", Number(0.0)),
                ("shape", Text("tube".into())),
                ("length", Number(15.0)),
            ],
        )
    };
    let e = run(&with(LAMP));
    assert!(warnings(&e).is_empty(), "{:?}", e.diagnostics);
    // The brim still prints, round the main shade's first turn, and only it is an aid.
    let main = &e.prints.iter().find(|p| p.name == "main").unwrap().path;
    let aids = (0..main.points.len()).filter(|&i| main.is_aid(i)).count();
    assert!(aids > 100 && aids < main.points.len() / 2, "{aids}");
    let unmarked = LAMP.replace(
        "aid(toolpath(graft(brim_rings)",
        "(toolpath(graft(brim_rings)",
    );
    assert_ne!(unmarked, LAMP);
    let e = run(&with(&unmarked));
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.message.contains("run into each other")),
        "{:?}",
        e.diagnostics
    );
}
