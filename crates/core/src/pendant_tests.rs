//! The Thread pendant template (web/templates/thread-pendant.easycad): the Thread Lampshade
//! Sketcher's pendant, made printable. The step itself is tested in `pendant.rs`.

use crate::eval::{Evaluation, Quality, Severity, evaluate};
use crate::geom::P3;
use easycad_lang::{Literal, set_values};

const TEMPLATE: &str = include_str!("../../../web/templates/thread-pendant.easycad");

fn num(v: f64) -> Literal {
    Literal::Number(v)
}
fn text(v: &str) -> Literal {
    Literal::Text(v.into())
}
fn rows(r: &[&[f64]]) -> Literal {
    Literal::List(
        r.iter()
            .map(|row| Literal::List(row.iter().map(|&x| num(x)).collect()))
            .collect(),
    )
}

fn pendant(values: &[(&str, Literal)]) -> Evaluation {
    let v: Vec<(String, Literal)> = values
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    evaluate(&set_values(TEMPLATE, &v).unwrap(), Quality::Final)
}

fn warnings(e: &Evaluation) -> Vec<String> {
    e.diagnostics.iter().map(|d| d.message.clone()).collect()
}

fn points(e: &Evaluation) -> Vec<P3> {
    assert!(
        e.diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning),
        "{:?}",
        e.diagnostics
    );
    e.prints[0].path.points.clone()
}

/// The sketcher's "Angled" file: a 500 mm cone with a corner at the opening, five entries.
fn angled() -> Vec<(&'static str, Literal)> {
    vec![
        ("wing_shape", text("sketcher")),
        ("height", num(300.0)),
        (
            "profile",
            rows(&[&[0.0, 247.5, 1.0], &[1.0, 63.31022832652452]]),
        ),
        ("entries", num(5.0)),
        ("wing_zone", num(250.0)),
        (
            "wing_curve",
            rows(&[
                &[0.0, 0.0],
                &[0.27449845679012347, 0.8346153846153846],
                &[0.5584490740740741, 1.0],
                &[1.0, 1.0],
            ]),
        ),
        ("max_overhang", num(70.0)),
        ("tip_gap", num(0.0)),
        ("wall_gap", num(5.0)),
        ("pin_gap", num(5.0)),
        ("grip", num(28.5)),
        ("crease", num(15.0)),
        (
            "path",
            rows(&[
                &[0.0, 0.0],
                &[0.10184151785714286, 0.19673352345083853],
                &[0.19707961309523808, 0.3656962746710856],
                &[0.35184151785714285, 0.5606532953098323],
                &[0.5780319940476191, 0.7599426941849956],
                &[0.8042224702380952, 0.8985787977503266],
                &[1.0, 1.0],
            ]),
        ),
        ("path_len", num(80.0)),
    ]
}

#[test]
fn the_template_prints_as_it_ships() {
    let e = pendant(&[]);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    assert_eq!(e.project.as_deref(), Some("Thread pendant"));
    let p = &e.prints[0];
    // 79 layers (200 mm at 2.5 mm; the leaf's wings lean in only 14°, hardly thinner), rising
    // a layer a turn (no seam): the first starts on the bed and climbs a layer as it goes
    // round, each move extruded for the gap over the bed.
    assert_eq!(p.stats.layers, 79);
    let first = p.path.points[1];
    assert!(first[2] < 0.1, "{first:?}");
    for i in 1..p.path.points.len() {
        let z = p.path.points[i][2];
        if z < 2.5 && !p.path.travel[i] {
            assert!(p.path.bead_of(i).1 <= z.max(0.05) + 1e-9, "{i} {z}");
        }
    }
    let top = p.path.points.iter().map(|q| q[2]).fold(0.0, f64::max);
    assert!((top - 197.5).abs() < 2.5 + 1e-6, "{top}");
    // G-code the printers take.
    let g =
        crate::gcode::write(&p.path, &crate::gcode::Profile::default(), "Thread pendant").unwrap();
    assert!(g.text.lines().filter(|l| l.starts_with("G1 X")).count() > 10_000);
}

/// The sketcher's "Angled" cone: it prints, its grip is full and on the socket, and the cone's
/// lean (31°) is within its max overhang (70°).
#[test]
fn the_sketchers_angled_cone() {
    // At its 70° max overhang the wings step in up to 6.9 mm a layer: more than a line, nothing
    // under the new ring. It says so. At 45° they step half a line at most, but then this wide
    // cone needs a longer wing zone and path to bring them in to the socket: it says that.
    // Its wings, compensated for their own 70° lean, fall far behind the wall's heights (the
    // cone leans 31°): the arm climbs steeply, and where the wings move in fast its layers just
    // outside them get thick. It says those too.
    let e = pendant(&angled());
    assert_eq!(warnings(&e).len(), 3, "{:?}", warnings(&e));
    assert!(warnings(&e)[0].contains("half a line"));
    assert!(warnings(&e)[1].contains("steeper than 15°"));
    assert!(warnings(&e)[2].contains("over 1.5 times"));
    let mut v = angled();
    v.retain(|(k, _)| *k != "max_overhang");
    v.push(("max_overhang", num(45.0)));
    let slow = pendant(&v);
    assert!(!warnings(&slow).iter().any(|w| w.contains("half a line")));
    assert!(
        warnings(&slow)
            .iter()
            .any(|w| w.contains("aren't fully formed"))
    );
    let pts = points(&e);
    // The widest: the 500 mm opening on the bed.
    let widest = pts.iter().map(|q| q[0].hypot(q[1])).fold(0.0, f64::max);
    assert!((widest - 247.5).abs() < 2.5, "{widest}");
}

/// Overhang compensation squeezes the layers where the wall leans (the half sphere's bottom
/// leans 71°), but never inside the thread: there every ring is a true helix, one pitch a turn.
#[test]
fn compensation_stays_out_of_the_thread() {
    // The sketcher's "halfsphere" file: a 500 mm half globe.
    let v = vec![
        ("wing_shape", text("sketcher")),
        ("height", num(237.5)),
        (
            "profile",
            rows(&[
                &[0.0, 247.5],
                &[0.09562427560010185, 246.45493847944672],
                &[0.19044100977910877, 243.3285794012757],
                &[0.2836494807560514, 238.14732468526094],
                &[0.37446254843874693, 230.95492972479147],
                &[0.462113301775894, 221.81213387513034],
                &[0.5458615352760975, 210.79614751310788],
                &[0.6249999999999999, 198.00000000000003],
                &[0.6988603762362595, 183.53175405400694],
                &[0.7668189174224678, 167.51359316691094],
                &[0.8283017176484295, 150.08078977162612],
                &[0.8827895582580468, 131.38056287440543],
                &[0.9298222926205395, 111.57083479893276],
                &[0.9690027320418575, 90.81889754152589],
                &[1.0, 69.30000000000007],
            ]),
        ),
        ("entries", num(5.0)),
        ("wing_zone", num(215.0)),
        (
            "wing_curve",
            rows(&[
                &[0.0, 0.0],
                &[0.36709104938271603, 0.9653846153846154],
                &[0.6510416666666666, 1.0],
                &[1.0, 1.0],
            ]),
        ),
        ("max_overhang", num(65.0)),
        ("tip_gap", num(0.0)),
        ("wall_gap", num(5.0)),
        ("pin_gap", num(5.0)),
        ("grip", num(28.5)),
        ("crease", num(15.0)),
        (
            "path",
            rows(&[
                &[0.0, 0.0],
                &[0.3567405523255814, 0.36923076923076925],
                &[1.0, 1.0],
            ]),
        ),
        ("path_len", num(100.0)),
    ];
    let on = pendant(&v);
    let mut off_v = v.clone();
    off_v.push(("compensation", text("off")));
    let off = pendant(&off_v);
    // Without it, the wall leans past the max overhang: it says so.
    assert!(
        warnings(&off)
            .iter()
            .any(|w| w.contains("leans more than the max overhang")),
        "{:?}",
        warnings(&off)
    );
    assert!(
        !warnings(&on).iter().any(|w| w.contains("leans")),
        "{:?}",
        warnings(&on)
    );
    let (a, b) = (points(&on), points(&off));
    assert!(on.prints[0].stats.layers > off.prints[0].stats.layers);
    // In the grip (the inner rings on the E27 barrel: radius 20 − 1 + 2.5), every point is on
    // one true helix, a pitch a turn: its height less its angle's share of a pitch is the same
    // whole number of pitches apart from any other's.
    for pts in [&a, &b] {
        let ring: Vec<&P3> = pts
            .iter()
            .filter(|q| (q[0].hypot(q[1]) - 21.5).abs() < 1e-6)
            .collect();
        assert!(ring.len() > 100, "{}", ring.len());
        let phase = |q: &P3| {
            let t = q[2] / 2.5 - q[1].atan2(q[0]) / std::f64::consts::TAU;
            t - t.floor()
        };
        let p0 = phase(ring[0]);
        assert!(ring.iter().all(|q| {
            let d = (phase(q) - p0).abs();
            d.min(1.0 - d) < 1e-6
        }));
    }
}

#[test]
fn the_checks_say_what_is_wrong() {
    // A grip deeper than the wing zone reaches: the wings can't form there.
    let e = pendant(&[("wing_zone", num(40.0))]);
    assert!(
        warnings(&e)
            .iter()
            .any(|w| w.contains("aren't fully formed")),
        "{:?}",
        warnings(&e)
    );
    // The socket sticks out above a grip that's too close to the top.
    let e = pendant(&[("hide", num(5.0))]);
    assert!(
        warnings(&e).iter().any(|w| w.contains("sticks out")),
        "{:?}",
        warnings(&e)
    );
    // Heights by length: not a true helix.
    let e = pendant(&[("z_by", text("length"))]);
    assert!(
        warnings(&e).iter().any(|w| w.contains("true helix")),
        "{:?}",
        warnings(&e)
    );
    // E14 and a custom socket run too.
    for s in ["E14", "custom"] {
        let e = pendant(&[("socket", text(s))]);
        assert!(!e.prints.is_empty(), "{s}: {:?}", e.diagnostics);
    }
}

/// The owner's Angled cone with a thinner wall and wings (3.2 mm lines, 1.5 mm layers) than the
/// thread (5 mm, the E27's 2.5 mm pitch): about a third less plastic; the thread on the socket
/// still one true helix, each move extruded for its own bead.
#[test]
fn a_thinner_wall_on_the_angled_cone() {
    let mut v = angled();
    v.extend([
        ("grip", num(28.5)),
        ("hide", num(35.0)),
        ("crease", num(15.0)),
        ("z_by", text("angle")),
    ]);
    let thick = pendant(&v);
    v.extend([("wall_line", num(3.2)), ("wall_layer", num(1.5))]);
    let thin = pendant(&v);
    // What the thick one says (the wings stepping more than half a line at 70°, and the arm
    // climbing steeply: the 70° wings far from the wall's heights), and nothing more.
    assert_eq!(warnings(&thin), warnings(&thick));
    let (a, b) = (&thin.prints[0].stats, &thick.prints[0].stats);
    assert!(
        a.volume_mm3 < 0.7 * b.volume_mm3,
        "{} {}",
        a.volume_mm3,
        b.volume_mm3
    );
    assert!(a.layers > b.layers);
    let path = &thin.prints[0].path;
    let pts = &path.points;
    // On the barrel (radius 20 − 1 + 2.5): one helix, a pitch a turn, each move there extruded
    // for the thread's bead.
    let on: Vec<usize> = (1..pts.len())
        .filter(|&i| {
            let r = |p: &P3| p[0].hypot(p[1]);
            (r(&pts[i]) - 21.5).abs() < 1e-6 && (r(&pts[i - 1]) - 21.5).abs() < 1e-6
        })
        .collect();
    assert!(on.len() > 100, "{}", on.len());
    let phase = |q: &P3| {
        let t = q[2] / 2.5 - q[1].atan2(q[0]) / std::f64::consts::TAU;
        t - t.floor()
    };
    let p0 = phase(&pts[on[0]]);
    for &i in &on {
        let d = (phase(&pts[i]) - p0).abs();
        assert!(d.min(1.0 - d) < 1e-6, "{:?}", pts[i]);
        let (w, h) = path.bead_of(i);
        assert!((w - 5.0).abs() < 1e-9 && (h - 2.5).abs() < 1e-9, "{w} {h}");
    }
    // Everywhere: between the wall's bead and the thread's; most of it the wall's.
    let mut walls = 0;
    for i in 1..pts.len() {
        if path.travel[i] {
            continue;
        }
        let (w, h) = path.bead_of(i);
        assert!(
            // (The arm extruded for the gap it's in: up to some 1.6 pitches just outside the
            // middle ring, where the 70° wings move in 3 mm a layer far below the wall's heights;
            // the recipe warns over 1.5 times a layer.)
            (3.2 - 1e-9..=5.0 + 1e-9).contains(&w) && h <= 2.0 * 2.5,
            "{i} {w} {h}"
        );
        // (The wall's own bead: its layer, or less where it gives the wings layers.)
        if (w - 3.2).abs() < 1e-9 && h <= 1.5 + 1e-9 {
            walls += 1;
        }
    }
    assert!(walls * 10 > pts.len() * 6, "{walls} of {}", pts.len());
}

/// The owner's Sphere (a 330 mm pendant, the opening 181 mm across on the bed) on the SPHR
/// handover's KUKA arm: the nozzle follows the wall, at most 45° and upright on the bed, and the
/// program for a short test (the first 40 mm) writes. `fixtures/recipes/sphere-robot.easycad`.
#[test]
fn the_sphere_on_the_robot() {
    use crate::krl;
    let recipe = include_str!("../../../fixtures/recipes/sphere-robot.easycad");
    let e = evaluate(recipe, Quality::Final);
    assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
    let print = &e.prints[0];
    let path = &print.path;
    let axes = path.axes().expect("the nozzle tilts");
    let lean = |v: &P3| v[2].clamp(-1.0, 1.0).acos().to_degrees();
    let floor = path.points[1][2];
    for (p, v) in path.points.iter().zip(axes) {
        // At most 45°, growing from upright on the bed over the first 45 mm.
        let most = 45.0 * ((p[2] - floor) / 45.0).clamp(0.0, 1.0);
        assert!(lean(v) <= most + 1e-6, "{} at z {}", lean(v), p[2]);
    }
    assert!((print.stats.max_tilt_deg - 45.0).abs() < 1e-6);
    // Leaning out at the bottom: well up the first stretch the nozzle leans out (radially).
    let i = path.points.iter().position(|p| p[2] > 50.0).unwrap();
    let (p, v) = (path.points[i], axes[i]);
    assert!(lean(&v) > 20.0 && p[0] * v[0] + p[1] * v[1] > 0.0);
    // The test program: the first 40 mm.
    let robot = krl::Robot {
        stop_at: 40.0,
        ..krl::Robot::default()
    };
    let program = krl::write(path, &robot, krl::BED, krl::CENTER, 1.0, "Sphere").unwrap();
    assert!(program.segments > 500, "{}", program.segments);
    assert!(program.src.contains("SET_FAN1 255"));
    assert!(program.src.ends_with("END\r\n"));
    // Without the robot: no tilt, the same toolpath as the Ginger G1 prints.
    let flat = evaluate(
        &set_values(recipe, &[("robot".to_string(), Literal::Bool(false))]).unwrap(),
        Quality::Final,
    );
    let flat_path = &flat.prints[0].path;
    assert!(flat_path.axes().is_none());
    assert_eq!(flat_path.points, path.points);
    // Tilting gives the squeezed wall its bead back, never more than a full layer (2.5 mm).
    let mut grown = 0;
    for i in 1..path.points.len() {
        let (before, after) = (flat_path.bead_of(i).1, path.bead_of(i).1);
        assert!(
            after >= before - 1e-9 && after <= before.max(2.5) + 1e-9,
            "{i}: {before} → {after}"
        );
        grown += usize::from(after > before + 1e-6);
    }
    assert!(grown > path.points.len() / 4, "{grown}");
}
