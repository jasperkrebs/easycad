//! Tests of the steps added so EasyGCode's projects can be built from general pieces
//! (`nodes/curve_tools.rs`, `nodes/printing.rs`), run through recipes. The thread and spoon
//! templates built from them are golden-tested in `golden_tests.rs`; the new machine commands
//! (fan levels, dwells, speed and extrusion factors) against a hand-checked fixture there too.

use crate::eval::{Quality, Severity, step_values};
use crate::geom::{P3, dist3};
use crate::toolpath::{EventKind, Toolpath};
use crate::value::Value;
use std::collections::HashMap;

fn run(body: &str) -> HashMap<String, Value> {
    let (e, values) = step_values(&format!("easycad 0.1\n{body}\n"), Quality::Final);
    assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
    values
}

fn messages(body: &str, severity: Severity) -> Vec<String> {
    let (e, _) = step_values(&format!("easycad 0.1\n{body}\n"), Quality::Final);
    e.diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| d.message.clone())
        .collect()
}

fn error(body: &str) -> String {
    messages(body, Severity::Error).join("; ")
}

fn num(v: &HashMap<String, Value>, name: &str) -> f64 {
    match v.get(name) {
        Some(Value::Number(n)) => *n,
        other => panic!("{name} is not a number: {other:?}"),
    }
}

fn nums(v: &HashMap<String, Value>, name: &str) -> Vec<f64> {
    match v.get(name) {
        Some(Value::List(l)) => l
            .items
            .iter()
            .map(|x| match x {
                Value::Number(n) => *n,
                other => panic!("{name} has {other:?}"),
            })
            .collect(),
        other => panic!("{name} is not a list: {other:?}"),
    }
}

fn point(v: &HashMap<String, Value>, name: &str) -> P3 {
    match v.get(name) {
        Some(Value::Point(p)) => *p,
        other => panic!("{name} is not a point: {other:?}"),
    }
}

fn curve_points(v: &HashMap<String, Value>, name: &str) -> (Vec<P3>, bool) {
    match v.get(name) {
        Some(Value::Curve(c)) => (c.points.clone(), c.closed),
        other => panic!("{name} is not a curve: {other:?}"),
    }
}

fn curves(v: &HashMap<String, Value>, name: &str) -> Vec<Vec<P3>> {
    match v.get(name) {
        Some(Value::List(l)) => l
            .items
            .iter()
            .map(|x| match x {
                Value::Curve(c) => c.points.clone(),
                other => panic!("{name} has {other:?}"),
            })
            .collect(),
        other => panic!("{name} is not a list: {other:?}"),
    }
}

fn path<'a>(v: &'a HashMap<String, Value>, name: &str) -> &'a Toolpath {
    match v.get(name) {
        Some(Value::Toolpath(p)) => p,
        other => panic!("{name} is not a toolpath: {other:?}"),
    }
}

#[track_caller]
fn assert_near(a: P3, b: P3) {
    assert!(dist3(a, b) < 1e-9, "{a:?} vs {b:?}");
}

#[test]
fn choosing_looking_up_and_adding_up() {
    let v = run(r#"param size = "M3" of ["M2", "M3", "M4"]
auto = pick(0, 7, 2.4 * 2)
set = pick(1.5, 1.5, 9)
toggled = pick(true, [1, 2], [3])
row = index_of(["M2", "M3", "M4"], size)
d = item([2, 3, 4], index_of(["M2", "M3", "M4"], size))
found = index_of([0.1, 0.2, 0.30000000000000004], 0.3)
total = sum([1, 2, 3.5])
running = accumulate([1, 2, 3.5])
up = greater([1, 2, 3], 2)
down = less([1, 2, 3], 2)
masked = sample(greater(z, 2) * 10, [0, 0, 5])"#);
    assert_eq!(num(&v, "auto"), 4.8);
    assert_eq!(num(&v, "set"), 1.5);
    assert_eq!(nums(&v, "toggled"), [1.0, 2.0]);
    assert_eq!(num(&v, "row"), 1.0);
    assert_eq!(num(&v, "d"), 3.0);
    assert_eq!(num(&v, "found"), 2.0);
    assert_eq!(num(&v, "total"), 6.5);
    assert_eq!(nums(&v, "running"), [1.0, 3.0, 6.5]);
    assert_eq!(nums(&v, "up"), [0.0, 0.0, 1.0]);
    assert_eq!(nums(&v, "down"), [1.0, 0.0, 0.0]);
    assert_eq!(num(&v, "masked"), 10.0);
    // A field test picks point by point.
    let v = run("f = pick(greater(x, 0), 5, -5)\na = sample(f, [1, 0])\nb = sample(f, [-1, 0])");
    assert_eq!((num(&v, "a"), num(&v, "b")), (5.0, -5.0));
    assert!(error(r#"i = index_of(["a"], "b")"#).contains("not in the list"));
    assert!(error(r#"s = sum([1, "two"])"#).contains("not a number"));
}

#[test]
fn circles_by_chord_tolerance() {
    // EasyGCode: max(24, ceil(2π / (2·acos(1 − tol/r)))).
    let v = run("a = circle(5.18, tolerance: 0.01)\nb = circle(1, tolerance: 1)");
    let (a, closed) = curve_points(&v, "a");
    assert!(closed);
    let expected = (std::f64::consts::TAU / (2.0 * (1.0 - 0.01 / 5.18f64).acos())).ceil() as usize;
    assert_eq!(a.len(), expected);
    assert_eq!(curve_points(&v, "b").0.len(), 24);
    assert!(error("c = circle(5, tolerance: -1)").contains("tolerance"));
}

#[test]
fn curve_tools() {
    let v = run(
        "sq = polyline([[0, 0], [10, 0], [10, 10], [0, 10]], closed: true)
round = fillet(sq, 2)
piece = subcurve(sq, 0.125, 0.375)
joined = join_curves([line([0, 0], [10, 0]), line([10, 10], [10, 0]), line([10, 10], [0, 0])])
dense = circle(10, tolerance: 0.001)
lean = simplify(dense, 0.05)
spline = catmull_rom([[0, 0], [10, 0], [10, 10]], samples: 4)
squircle = superellipse(10, 10, 4)
t = tangent(sq, 0.1)
n = normal(sq, 0.1)
c = closest(sq, [5, -3])
mid = centroid(sq)",
    );
    let (round, closed) = curve_points(&v, "round");
    assert!(closed && round.len() > 8);
    assert_eq!(
        curve_points(&v, "piece").0,
        vec![[5.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 5.0, 0.0]]
    );
    let (joined, closed) = curve_points(&v, "joined");
    assert!(closed && joined.len() == 3);
    assert!(curve_points(&v, "lean").0.len() < curve_points(&v, "dense").0.len() / 4);
    assert_eq!(curve_points(&v, "spline").0.len(), 2 * 4 + 1);
    assert_eq!(curve_points(&v, "squircle").0.len(), 720);
    assert_near(point(&v, "t"), [1.0, 0.0, 0.0]);
    assert_near(point(&v, "n"), [0.0, -1.0, 0.0]);
    assert!((num(&v, "c") - 0.125).abs() < 1e-12);
    assert_near(point(&v, "mid"), [5.0, 5.0, 0.0]);
    assert!(
        error("j = join_curves([line([0, 0], [1, 0]), line([5, 5], [6, 5])])")
            .contains("doesn't touch")
    );
}

#[test]
fn centring_moves_everything_together() {
    let v = run(
        "both = center([line([0, 0], [10, 0]), line([10, 0], [10, 20])])
one = center(circle(5) |> move(x: 100, y: 50))
p = center(point(3, 4, 5))",
    );
    let both = curves(&v, "both");
    assert_near(both[0][0], [-5.0, -10.0, 0.0]);
    assert_near(both[1][1], [5.0, 10.0, 0.0]);
    let (one, _) = curve_points(&v, "one");
    assert!((one[0][0] - 5.0).abs() < 1e-9 && one[0][1].abs() < 1e-9);
    assert_near(point(&v, "p"), [0.0, 0.0, 5.0]);
}

#[test]
fn spirals_with_exact_turns() {
    let v = run("s = spiral(spacing: 2, start: 5, turns: 4.5)");
    let (pts, _) = curve_points(&v, "s");
    assert_near(pts[0], [5.0, 0.0, 0.0]);
    let end = pts.last().copied().unwrap();
    assert!((end[0].hypot(end[1]) - 14.0).abs() < 1e-9, "{end:?}");
    assert!(error("s = spiral(spacing: 2)").contains("give `outer`"));
    assert!(error("s = spiral(10, spacing: 2, start: 3)").contains("`start` needs `turns`"));
    assert!(!messages("s = spiral(10, spacing: 2, turns: 3)", Severity::Warning).is_empty());
}

#[test]
fn ring_fills() {
    let v = run(r#"outline = circle(20)
r = rings(outline, 2)
inside = rings(polyline([[0, 0], [20, 0], [20, 10], [0, 10]], closed: true), 1, inset: true)
stairs = link_rings(reverse(r), "staircase", 2)
zip = link_rings(reverse(r), "zipper", 2)
swirl = link_rings(reverse(r), "spiral", 2)
bottom = toolpath(swirl, width: 2, layer: 0.5, flow: 10)"#);
    assert_eq!(curves(&v, "r").len(), 10);
    assert_eq!(curves(&v, "inside").len(), 5);
    for name in ["stairs", "zip", "swirl"] {
        let (pts, closed) = curve_points(&v, name);
        assert!(!closed && pts.len() > 100, "{name}");
    }
    let p = path(&v, "bottom");
    assert_eq!(
        p.travel.iter().filter(|t| **t).count(),
        1,
        "one continuous line"
    );
    assert!(error(r#"l = link_rings(rings(circle(5), 1), "swirl", 1)"#).contains("staircase"));
}

#[test]
fn scattered_points_and_the_lean_of_a_wall() {
    let v = run("pts = scatter(100, 40, 25, seed: 3)
wall = loft([circle(20), circle(30) |> move(z: 10)])
out = sample(lean(wall), [20, 0, 1])
straight = sample(lean(loft([circle(20), circle(20) |> move(z: 10)])), [20, 0, 1])");
    assert!(matches!(v.get("pts"), Some(Value::List(l)) if l.items.len() == 25));
    assert!((num(&v, "out") - 45.0).abs() < 0.5, "{}", num(&v, "out"));
    assert!(num(&v, "straight").abs() < 1e-6);
}

#[test]
fn paced_contours_and_open_tops() {
    let v = run("cone = loft([circle(20), circle(40) |> move(z: 20)])
even = contour(cone, 1)
paced = contour(cone, 1, pace: 1)
flat = vase_spiral(cone, layer: 1, width: 2)
open = vase_spiral(cone, layer: 1, width: 2, flat_top: false)
open2 = spiralize(contour(cone, 1), width: 2, flat_top: false)");
    let (even, paced) = (curves(&v, "even"), curves(&v, "paced"));
    assert_eq!(even.len(), 21);
    // 45° everywhere: steps of cos 45° · 1 mm, fitted to end at 20 mm.
    assert_eq!(
        paced.len(),
        (20.0 / 45f64.to_radians().cos()).ceil() as usize + 1
    );
    assert!((paced.last().unwrap()[0][2] - 20.0).abs() < 1e-9);
    let (flat, open) = (path(&v, "flat"), path(&v, "open"));
    assert_eq!(flat.points.len() - open.points.len(), flat.samples_per_turn);
    assert!((open.points.last().unwrap()[2] - 20.0).abs() < 1e-9);
    assert_eq!(open.flow.last().copied(), Some(1.0));
    assert_eq!(path(&v, "open2").points.len(), open.points.len());
    assert!(
        error("c = contour(loft([circle(5), circle(5) |> move(z: 3)]), 1, pace: 2)")
            .contains("pace")
    );
}

#[test]
fn tuning_a_toolpath() {
    let v = run(r#"wall = loft([circle(20), circle(20) |> move(z: 10)])
base = vase_spiral(wall, layer: 1, width: 2, flow: 10)
slow = adjust(base, speed: 1 - 0.5 * greater(z, 5), extrude: 1 + 0.2 * greater(z, 8))
ramped = accelerate(slow, 500)
cooled = fan(base, level: 100 * greater(z, 1.5))
paused = dwell(base, [[0, 20, 5]], 2)
spiky = spikes(base, [[20, 0, 5], [-20, 0, 7]], width: 3, length: 4, angle: 20, dwell: 1)"#);
    let slow = path(&v, "slow");
    assert!(slow.speed.contains(&0.5) && slow.flow.iter().any(|f| (*f - 1.2).abs() < 1e-12));
    let ramped = path(&v, "ramped");
    assert!(ramped.points.len() >= slow.points.len());
    let cooled = path(&v, "cooled");
    let fans: Vec<f64> = cooled
        .events
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Fan(l) => Some(l),
            _ => None,
        })
        .collect();
    assert_eq!(fans, vec![100.0]);
    assert_eq!(path(&v, "paused").events.len(), 1);
    let spiky = path(&v, "spiky");
    assert_eq!(spiky.events.len(), 2, "a dwell at each tip");
    let b = spiky.stats().bounds;
    assert!(b.max[0] > 23.5, "spikes stick out: {:?}", b.max);
    assert!(error("p = adjust(vase_spiral(loft([circle(5), circle(5) |> move(z: 3)]), layer: 1, width: 2), speed: 0)").contains("more than 0"));
}

#[test]
fn beads_change_extrusion_but_not_layers() {
    let v = run(
        "a = toolpath([line([0, 0], [10, 0])], width: 1, layer: 0.5, flow: 5)
b = bead(a, width: 2, layer: 0.8)
stacked = stack(b, 3)",
    );
    let (a, b) = (path(&v, "a"), path(&v, "b"));
    assert_eq!(a.points, b.points);
    assert!(b.area(1) > a.area(1));
    assert_eq!(b.parts[0].width, 2.0);
    assert_eq!(b.parts[0].layer, 0.5, "the layer for stacking stays");
    let stacked = path(&v, "stacked");
    assert!((stacked.points.last().unwrap()[2] - 1.5).abs() < 1e-9);
}

#[test]
fn rings_with_radial_steps_and_machine_events() {
    let v = run(r#"radii = series(1, 2, 3)
sweeps = 360 - 2 / radii * 180 / pi
starts = mod(90 + accumulate(sweeps) - sweeps, 360)
rings = arc(radii, start: starts, end: starts + sweeps, tolerance: 0.05)
seat = polyline(flatten(vertices(rings)))
count_points = count(vertices(seat))
layer = toolpath(seat, width: 2, layer: 0.6, speed: 30)
heated = layer |> temperature(up: 230, mid: 230, down: 230, at_end: true)
waited = heated |> wait_temperature(down: 230)
sped = waited |> speed_override(263.2, at_end: true)
layers = stack(layer, 3, wipe: 1.2)
none = stack(layer, 0)"#);
    // Three rings, each with its start point (the step out from the ring before) and at least
    // 13 points; strung together in order.
    let rings = curves(&v, "rings");
    assert_eq!(rings.len(), 3);
    let total: usize = rings.iter().map(|r| r.len()).sum();
    assert!(rings.iter().all(|r| r.len() >= 13));
    assert_eq!(num(&v, "count_points") as usize, total);
    // Ring 2 starts where ring 1 stops, one line further out.
    let end1 = rings[0][rings[0].len() - 1];
    let start2 = rings[1][0];
    assert!((start2[1].atan2(start2[0]) - end1[1].atan2(end1[0])).abs() < 1e-9);
    assert!((start2[0].hypot(start2[1]) - 3.0).abs() < 1e-9);
    // The events: in order, at the start or the end; the override in whole percent.
    let sped = path(&v, "sped");
    let n = sped.points.len();
    let kinds: Vec<(usize, String)> = sped
        .events
        .iter()
        .map(|e| (e.after, format!("{:?}", e.kind)))
        .collect();
    assert_eq!(kinds.len(), 3);
    assert_eq!(kinds[0].0, 0);
    assert!(kinds[0].1.starts_with("WaitHeat"));
    assert_eq!((kinds[1].0, kinds[2].0), (n - 1, n - 1));
    assert!(matches!(sped.events[2].kind, EventKind::SpeedOverride(p) if p == 263.0));
    // Layers that wipe across: three copies, two wipes of two moves each.
    assert_eq!(path(&v, "layers").points.len(), 3 * n + 2);
    assert!(path(&v, "none").points.is_empty());
    // Bad values are explained.
    assert!(
        error(
            "p = toolpath(line([0, 0], [9, 0]), width: 1, layer: 0.5) |> temperature(nozzle: 999)"
        )
        .contains("350")
    );
    assert!(
        error("p = toolpath(line([0, 0], [9, 0]), width: 1, layer: 0.5) |> speed_override(0)")
            .contains("percent")
    );
}

#[test]
fn every_lampshade_shade_prints_both_ways_up() {
    use easycad_lang::{Literal, set_values};
    let template = include_str!("../../../web/templates/lampshade.easycad");
    for shape in ["cone", "bell", "dome", "sphere", "flat sphere", "own"] {
        for (wide, lip, brim, comp) in [
            (false, false, false, "width"),
            (true, true, true, "layer height"),
            (false, true, false, "off"),
        ] {
            let values: Vec<(String, Literal)> = vec![
                ("shape".into(), Literal::Text(shape.into())),
                ("wide_down".into(), Literal::Bool(wide)),
                ("lip".into(), Literal::Bool(lip)),
                ("brim".into(), Literal::Bool(brim)),
                ("compensation".into(), Literal::Text(comp.into())),
            ];
            let recipe = set_values(template, &values).unwrap();
            let (e, v) = step_values(&recipe, Quality::Final);
            assert!(
                e.errors().next().is_none(),
                "{shape} {wide} {lip}: {:?}",
                e.diagnostics
            );
            // The profile always rises, from the bed to the top.
            let (pts, _) = curve_points(&v, "profile");
            assert!(pts.windows(2).all(|w| w[1][2] >= w[0][2] - 1e-9), "{shape}");
            assert!(
                pts[0][2].abs() < 1e-9 && pts.iter().all(|p| p[0] > 0.0),
                "{shape}"
            );
            let p = e.print.as_ref().unwrap();
            assert!(p.path.points.len() > 100 && p.stats.layers > 10, "{shape}");
        }
    }
}

#[test]
fn graphs_bounds_and_grafts() {
    let v = run(r#"g = graph([0, 10, 10, 20], [1, 2, 5, 6], 15)
below = graph([0, 10], [1, 2], -5)
above = graph([0, 10], [1, 2], 50)
field = graph([0, 10], [0, 100], z)
at = sample(field, [0, 0, 2.5])
b = bounds([3, -1, 7, 2])
leaves = graft([1, 2, 3])
widths = toolpath(graft([line([0, 0], [10, 0]), line([0, 5], [10, 5])]), width: 1, layer: 0.5)"#);
    // Between the second 10 and 20: from 5 to 6.
    assert!((num(&v, "g") - 5.5).abs() < 1e-12);
    assert_eq!((num(&v, "below"), num(&v, "above")), (1.0, 2.0));
    assert!((num(&v, "at") - 25.0).abs() < 1e-12);
    assert_eq!(nums(&v, "b"), vec![-1.0, 7.0]);
    match v.get("leaves") {
        Some(Value::List(l)) => assert!(
            l.items
                .iter()
                .all(|x| matches!(x, Value::List(i) if i.items.len() == 1))
        ),
        other => panic!("{other:?}"),
    }
    // One toolpath per grafted curve.
    match v.get("widths") {
        Some(Value::List(l)) => assert_eq!(l.items.len(), 2),
        other => panic!("{other:?}"),
    }
    assert!(error("g = graph([0, 1], [1], 0.5)").contains("as many"));
    assert!(error("g = graph([1, 0], [1, 2], 0.5)").contains("decrease"));
}

#[test]
fn vase_revolve_follows_the_profile_turn_by_turn() {
    let v = run(
        r#"profile = polyline([[20, 0, 0], [20, 0, 10], [40, 0, 30]])
path = vase_revolve(profile, layer: 2, width: 3, flow: 10, tolerance: 0.1)
wide = vase_revolve(profile, layer: 2, width: 3 + greater(z, 10), tolerance: 0.1)
squeezed = vase_revolve(profile, layer: 2 - greater(z, 10), width: 3, bead_layer: 2, tolerance: 0.1)"#,
    );
    let p = path(&v, "path");
    // 15 turns of 2 mm to 30 mm, then the closing turn; the first holds radius 20 at the back.
    assert_eq!(p.layers, 15);
    assert_near(p.points[0], [20.0 * 90f64.to_radians().cos(), 20.0, 0.0]);
    let top = p.points[p.points.len() - 1];
    assert!((top[2] - 30.0).abs() < 1e-9 && (top[0].hypot(top[1]) - 40.0).abs() < 1e-9);
    // Every point sits on the profile: radius 20 up to 10 mm, then 20 + (z - 10).
    for q in &p.points[1..] {
        let want = if q[2] <= 10.0 {
            20.0
        } else {
            20.0 + (q[2] - 10.0)
        };
        assert!((q[0].hypot(q[1]) - want).abs() < 1e-9, "{q:?}");
    }
    // Wider beads above 10 mm: two widths (one part each, the closing turn as the last).
    let w = path(&v, "wide");
    let widths: Vec<f64> = w.parts.iter().map(|s| s.width).collect();
    assert_eq!(widths.first(), Some(&3.0));
    assert_eq!(widths.last(), Some(&4.0));
    // A shorter rise above 10 mm, each turn still extruded for the full 2 mm.
    let s = path(&v, "squeezed");
    assert_eq!(s.layers, 5 + 20);
    assert!(s.parts.iter().all(|p| p.layer == 2.0));
    assert!(
        error(
            "p = vase_revolve(polyline([[20, 0, 0], [20, 0, 10], [30, 0, 5]]), layer: 2, width: 3)"
        )
        .contains("rise")
    );
    assert!(
        error("p = vase_revolve(polyline([[0, 0, 0], [20, 0, 10]]), layer: 2, width: 3)")
            .contains("radius")
    );
}
