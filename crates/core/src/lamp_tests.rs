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
    assert!(!gone.contains("part "));
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
    let broken = lampshade.replace("mark mark_fillet for fillet", "mark mark_fillet for filet");
    let e = run(&broken);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.message == "`mark … for filet`: there's no setting `filet`")
    );
}
