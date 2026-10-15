//! Tests of the Grasshopper-style toolbox (`nodes/toolbox.rs`), run through recipes: list
//! tools, points and planes, curve operations, transforms by fields, surfaces from curves and
//! `spiralize`'s checks. (Its G-code is golden-tested in `golden_tests.rs`.)

use crate::eval::{Quality, Severity, step_values};
use crate::geom::P3;
use crate::value::Value;
use std::collections::HashMap;

/// Runs a recipe that must work without errors and returns its steps' values.
fn run(body: &str) -> HashMap<String, Value> {
    let (e, values) = step_values(&format!("easycad 0.1\n{body}\n"), Quality::Final);
    assert!(e.errors().next().is_none(), "{:?}", e.diagnostics);
    values
}

/// The messages a recipe gets with this severity.
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

/// A curve's points, or a list of points.
fn points(v: &HashMap<String, Value>, name: &str) -> Vec<P3> {
    match v.get(name) {
        Some(Value::Curve(c)) => c.points.clone(),
        Some(Value::List(l)) => l
            .items
            .iter()
            .map(|x| match x {
                Value::Point(p) => *p,
                other => panic!("{name} has {other:?}"),
            })
            .collect(),
        other => panic!("{name} is not a curve or points: {other:?}"),
    }
}

fn near(a: P3, b: P3, tolerance: f64) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= tolerance)
}

#[track_caller]
fn assert_near(a: P3, b: P3) {
    assert!(near(a, b, 1e-9), "{a:?} vs {b:?}");
}

#[test]
fn list_tools_work_like_grasshoppers() {
    let v = run("s = series(0, 2, 4)
r1 = random(5, 10, 20, seed: 3)
r2 = random(5, 10, 20, seed: 3)
r3 = random(5, 10, 20, seed: 4)
last = item(s, -1)
wrapped = item(s, 5)
picked = item(s, [0, 2])
rev = reverse(s)
shifted = shift(s, 1)
flat = flatten([[1, 2], [3, [4, 5]]])
counts = count([[1, 2], [3]])
mapped = remap(5, 0, 10, 0, 100)");
    assert_eq!(nums(&v, "s"), [0.0, 2.0, 4.0, 6.0]);
    assert_eq!(nums(&v, "r1"), nums(&v, "r2"), "same seed, same numbers");
    assert_ne!(nums(&v, "r1"), nums(&v, "r3"));
    assert!(nums(&v, "r1").iter().all(|x| (10.0..20.0).contains(x)));
    assert_eq!(num(&v, "last"), 6.0);
    assert_eq!(num(&v, "wrapped"), 2.0);
    assert_eq!(nums(&v, "picked"), [0.0, 4.0]);
    assert_eq!(nums(&v, "rev"), [6.0, 4.0, 2.0, 0.0]);
    assert_eq!(nums(&v, "shifted"), [2.0, 4.0, 6.0, 0.0]);
    assert_eq!(nums(&v, "flat"), [1.0, 2.0, 3.0, 4.0, 5.0]);
    // A list of lists is like a Grasshopper tree: list steps run once per branch.
    assert_eq!(nums(&v, "counts"), [2.0, 1.0]);
    assert_eq!(num(&v, "mapped"), 50.0);
    assert!(error("e = item([], 0)").contains("the list is empty"));
    assert!(error("m = remap(1, 2, 2)").contains("must differ"));
}

#[test]
fn points_planes_and_coordinate_fields() {
    let v = run("p = point(1, 2, 3)
moved = p + vector(1, 0, 0)
doubled = p * 2
height = sample(z, p)
mixed = sample(x * 2 + y, p)
row = point([0, 1, 2], 5)
pl = plane([0, 0, 5], normal: [0, 0, 2])
lifted = p |> move(z: 1)
x2 = 5
own = x2 * 2");
    assert_eq!(point(&v, "p"), [1.0, 2.0, 3.0]);
    assert_eq!(point(&v, "moved"), [2.0, 2.0, 3.0]);
    assert_eq!(point(&v, "doubled"), [2.0, 4.0, 6.0]);
    assert_eq!(num(&v, "height"), 3.0);
    assert_eq!(num(&v, "mixed"), 4.0);
    assert_eq!(
        points(&v, "row"),
        [[0.0, 5.0, 0.0], [1.0, 5.0, 0.0], [2.0, 5.0, 0.0]]
    );
    let Some(Value::Plane(pl)) = v.get("pl") else {
        panic!("not a plane")
    };
    assert_eq!((pl.origin, pl.z), ([0.0, 0.0, 5.0], [0.0, 0.0, 1.0]));
    assert_eq!(point(&v, "lifted"), [1.0, 2.0, 4.0]);
    assert_eq!(num(&v, "own"), 10.0);
    // A setting or step named like a coordinate wins over the built-in field.
    let v = run("param x = 5\ntwice = x * 2");
    assert_eq!(num(&v, "twice"), 10.0);
    assert!(error("pl = plane(normal: [0, 0, 0])").contains("`normal` must have a length"));
    assert!(error("p = line([0, 0, 0], [1, 2, 3, 4])").contains("must be a point"));
}

#[test]
fn curve_tools() {
    let v = run("r = rectangle(20, 10)
rl = length(r)
r0 = evaluate(r, 0)
rounded = length(rectangle(20, 10, radius: 2))
e0 = evaluate(ellipse(20, 10), 0.25)
a = arc(10, 0, 90)
a0 = evaluate(a, 0)
a1 = evaluate(a, 1)
ln = line([0, 0, 0], [3, 4, 0])
lnl = length(ln)
mid = evaluate(ln, 0.5)
pieces = divide(ln, 5)
quarters = divide(circle(10), 4)
open = polyline([[0, 0], [10, 0], [10, 10]])
closed = polyline([[0, 0], [10, 0], [10, 10]], closed: true)
ol = length(open)
cl = length(closed)
through = interpolate([[0, 0], [10, 5], [20, 0]])
flipped = flip(ln)
seamed = seam(circle(10), 0.25)
halfway = tween(circle(10), circle(20) |> move(z: 10), 0.5)
several = tween(circle(10), circle(20), [0, 0.5, 1])
wider = offset(circle(10), 2)
fewer = resample(ln, 4)");
    assert!((num(&v, "rl") - 60.0).abs() < 1e-9);
    // Starts in the middle of its +X side and runs counter-clockwise.
    assert_near(point(&v, "r0"), [10.0, 0.0, 0.0]);
    let rounded = num(&v, "rounded");
    assert!(
        (rounded - (44.0 + 4.0 * std::f64::consts::PI)).abs() < 0.05,
        "{rounded}"
    );
    assert!(near(point(&v, "e0"), [0.0, 10.0, 0.0], 0.5));
    assert_near(point(&v, "a0"), [10.0, 0.0, 0.0]);
    assert!(near(point(&v, "a1"), [0.0, 10.0, 0.0], 1e-9));
    assert!((num(&v, "lnl") - 5.0).abs() < 1e-12);
    assert_near(point(&v, "mid"), [1.5, 2.0, 0.0]);
    let pieces = points(&v, "pieces");
    assert_eq!(pieces.len(), 6, "an open curve: count + 1 points");
    assert_near(pieces[1], [0.6, 0.8, 0.0]);
    let quarters = points(&v, "quarters");
    assert_eq!(quarters.len(), 4, "a closed curve: count points");
    assert!(near(quarters[1], [0.0, 10.0, 0.0], 1e-9));
    assert!((num(&v, "ol") - 20.0).abs() < 1e-12);
    assert!((num(&v, "cl") - (20.0 + 200f64.sqrt())).abs() < 1e-9);
    let through = points(&v, "through");
    assert_near(through[0], [0.0, 0.0, 0.0]);
    assert_near(*through.last().unwrap(), [20.0, 0.0, 0.0]);
    assert!(through.iter().any(|p| near(*p, [10.0, 5.0, 0.0], 1e-6)));
    assert_near(points(&v, "flipped")[0], [3.0, 4.0, 0.0]);
    assert!(near(points(&v, "seamed")[0], [0.0, 10.0, 0.0], 1e-9));
    assert!(near(points(&v, "halfway")[0], [15.0, 0.0, 5.0], 1e-9));
    let Some(Value::List(several)) = v.get("several") else {
        panic!("not a list")
    };
    assert_eq!(several.items.len(), 3);
    let wider = points(&v, "wider");
    assert!(
        wider.iter().all(|p| (p[0].hypot(p[1]) - 12.0).abs() < 2e-3),
        "outward by 2"
    );
    assert_eq!(points(&v, "fewer").len(), 4);
    assert!(error("s = seam(line([0, 0], [1, 0]), 0.5)").contains("closed"));
    assert!(error("t = tween(circle(10), line([0, 0], [1, 0]), 0.5)").contains("both"));
}

#[test]
fn transforms_take_fields_axes_and_centres() {
    let v = run("c = polyline([[10, 0], [0, 10], [-10, 0]])
slope = c |> move(z: 0.1 * x)
lift = c |> move(z: 5)
tilt = c |> rotate(90, axis: [0, 1, 0])
twist = c |> rotate(9 * x)
about = c |> rotate(90, center: [10, 0, 0])
bigger = c |> scale(2, center: [10, 0, 0])
flare = c |> scale(1 + 0.1 * x)
mirrored = c |> mirror(plane(normal: [1, 0, 0]))
cut = plane([0, 0, 5]) |> rotate(45, axis: [0, 1, 0], center: [0, 0, 5])
projected = c |> project_along(cut)
slanted = c |> project_along(plane([0, 0, 5]), direction: [1, 0, 1])");
    let z: Vec<f64> = points(&v, "slope").iter().map(|p| p[2]).collect();
    assert_eq!(z, [1.0, 0.0, -1.0]);
    assert!(points(&v, "lift").iter().all(|p| p[2] == 5.0));
    // Counter-clockwise looking against the axis: +X turns to -Z about +Y.
    assert_near(points(&v, "tilt")[0], [0.0, 0.0, -10.0]);
    let twist = points(&v, "twist");
    assert_near(twist[0], [0.0, 10.0, 0.0]);
    assert_near(twist[1], [0.0, 10.0, 0.0]);
    assert_near(twist[2], [0.0, 10.0, 0.0]);
    assert_near(points(&v, "about")[1], [0.0, -10.0, 0.0]);
    assert_near(points(&v, "bigger")[1], [-10.0, 20.0, 0.0]);
    let flare = points(&v, "flare");
    assert_near(flare[0], [20.0, 0.0, 0.0]);
    assert_near(flare[1], [0.0, 10.0, 0.0]);
    assert_near(flare[2], [0.0, 0.0, 0.0]);
    assert_near(points(&v, "mirrored")[0], [-10.0, 0.0, 0.0]);
    // The plane z = 5 - x: each point dropped straight onto it.
    let projected = points(&v, "projected");
    assert_near(projected[0], [10.0, 0.0, -5.0]);
    assert_near(projected[1], [0.0, 10.0, 5.0]);
    assert_near(projected[2], [-10.0, 0.0, 15.0]);
    assert_near(points(&v, "slanted")[0], [15.0, 0.0, 5.0]);

    let path = "path = toolpath([circle(10)], width: 0.6, layer: 0.3)";
    assert!(error(&format!("{path}\nm = path |> move(z: x)")).contains("can't move a toolpath"));
    assert!(
        error(&format!("{path}\nr = path |> rotate(10, axis: [1, 0, 0])"))
            .contains("can't tilt a toolpath")
    );
    assert!(
        error(&format!(
            "{path}\nm = path |> mirror(plane(normal: [0, 0, 1]))"
        ))
        .contains("upright plane")
    );
    let v = run(&format!("{path}\nturned = path |> rotate(30)"));
    assert!(matches!(v.get("turned"), Some(Value::Toolpath(_))));
    assert!(
        error("p = circle(10) |> project_along(plane(), direction: [1, 0, 0])")
            .contains("parallel")
    );
    assert!(error("r = circle(10) |> rotate(10, axis: [0, 0, 0])").contains("`axis`"));
}

#[test]
fn surfaces_from_curves() {
    let v = run("tube = extrude(circle(10), 20)
turned = revolve(polyline([[10, 0, 0], [12, 0, 10], [8, 0, 20]]))
layers = contour(tube, 0.5)");
    let Some(Value::Surface(tube)) = v.get("tube") else {
        panic!("not a surface")
    };
    assert_eq!(tube.sections.len(), 2);
    assert!(tube.sections[1].iter().all(|p| p[2] == 20.0));
    let Some(Value::Surface(turned)) = v.get("turned") else {
        panic!("not a surface")
    };
    let radii: Vec<f64> = turned.sections.iter().map(|s| s[0][0]).collect();
    assert_eq!(radii, [10.0, 12.0, 8.0]);
    assert!(turned.closed);
    let Some(Value::List(layers)) = v.get("layers") else {
        panic!("not a list")
    };
    assert_eq!(layers.items.len(), 41);
    let Some(Value::Curve(top)) = layers.items.last() else {
        panic!("not a curve")
    };
    assert!(top.points.iter().all(|p| (p[2] - 20.0).abs() < 1e-9));
    assert!(error("s = extrude(circle(10), 0)").contains("can't be 0"));
    assert!(error("s = revolve(circle(10))").contains("open curve"));
    assert!(error("s = revolve(line([-1, 0, 0], [1, 0, 5]))").contains("x ≥ 0"));
}

#[test]
fn spiralize_checks_its_layers() {
    assert!(error("p = spiralize([circle(10)], width: 0.6)").contains("at least 2 layer"));
    assert!(
        error("p = spiralize([line([0, 0], [1, 0]), line([0, 0, 1], [1, 0, 1])], width: 0.6)")
            .contains("open curve")
    );
    assert!(
        error("p = spiralize([circle(10), circle(10)], width: 0.6)")
            .contains("isn't above layer 1")
    );
    let thick = messages(
        "p = spiralize(circle(10) |> move(z: [0, 1]), width: 0.6)",
        Severity::Warning,
    );
    assert!(thick.iter().any(|m| m.contains("1 mm thick")), "{thick:?}");
    let mixed = messages(
        "p = spiralize([circle(10), polygon(6, 10) |> move(z: 0.3)], width: 0.6)",
        Severity::Warning,
    );
    assert!(mixed.iter().any(|m| m.contains("resampled")), "{mixed:?}");
    // A tilted stack: every move knows its own layer height.
    let v = run("base = circle(20)
layers = tween(base, base |> move(z: 10 + 0.1 * x), range(0, 1, count: 41))
path = spiralize(layers, width: 0.8, flow: 6)");
    let Some(Value::Toolpath(path)) = v.get("path") else {
        panic!("not a toolpath")
    };
    let heights: Vec<f64> = path.height.iter().copied().filter(|h| *h > 0.0).collect();
    let (lo, hi) = heights.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), h| {
        (lo.min(*h), hi.max(*h))
    });
    // The top runs from 8 mm (at -X) to 12 mm (at +X): 40 layers of 0.2 to 0.3 mm.
    assert!(
        (lo - 0.2).abs() < 2e-3 && (hi - 0.3).abs() < 2e-3,
        "{lo}..{hi}"
    );
    let stats = path.stats();
    assert!(stats.by_flow && (stats.flow_mm3_s - 6.0).abs() < 1e-9);
}

/// Degenerate and hostile inputs to the new steps: each one must end in a value or a message.
#[test]
fn hostile_inputs_give_messages_not_crashes() {
    let cases = [
        "c = rectangle(10, 10, radius: 100)",
        "c = rectangle(10, 10, radius: -1)",
        "c = interpolate([[0, 0], [0, 0], [0, 0]])",
        "c = interpolate([[0, 0]])",
        "c = polyline([[1, 2]], closed: true)",
        "c = polyline([])",
        "c = seam(polyline([[0, 0], [1, 0]], closed: true), 0.5)",
        "c = seam(circle(10), 1e300)",
        "c = evaluate(line([0, 0], [0, 0]), 0.5)",
        "c = divide(line([0, 0], [0, 0]), 4)",
        "c = divide(circle(1), 1e12)",
        "c = resample(circle(1), 1)",
        "c = offset(circle(1), 1e300)",
        "c = tween(circle(1), circle(2), 1e300)",
        "c = arc(10, 0, 1e9)",
        "c = ellipse(10, 0)",
        "c = item([1, 2], 1e300)",
        "c = shift([1, 2], -1e300)",
        "c = random(1e9)",
        "c = series(0, 1, 1e9)",
        "c = flatten(1)",
        "c = remap(1, 0, 0)",
        "c = sample(z, [1, 2, 3, 4])",
        "c = point(1e308, 1e308, 1e308) * 10",
        "c = circle(10) |> move(z: 1e308 * x)",
        "c = circle(10) |> scale(1e308 * x)",
        "c = circle(10) |> rotate(1e300 * x)",
        "c = circle(10) |> rotate(45, axis: [1e-320, 0, 0])",
        "c = circle(10) |> mirror(plane(normal: [1e-320, 0, 0]))",
        "c = circle(10) |> project_along(plane(normal: [0, 0, 1]), direction: [0, 0, 1e-320])",
        "c = extrude(line([0, 0], [0, 0]), 5)",
        "c = revolve(line([0, 0, 0], [0, 0, 5]))",
        "c = revolve(polyline([[1, 0, 0], [1, 0, 0]]))",
        "c = contour(extrude(circle(5), 5), 1e-9)",
        "c = contour(loft([circle(5), circle(5)]), 0.5)",
        "c = spiralize([circle(5), circle(5) |> move(z: 1e308)], width: 0.6)",
        "c = spiralize(circle(5) |> move(z: range(0, 1, count: 100000)), width: 0.6)",
        "c = spiralize([polygon(3, 1), circle(5) |> move(z: 0.2)], width: 0)",
        "c = spiralize([circle(5), circle(5) |> move(z: 0.2 * x)], width: 0.6)",
        "c = spiralize([circle(5), circle(5) |> move(z: 0.3)], width: 0.6, flow: -1)",
        "c = spiralize([circle(5), circle(5) |> move(z: 0.3)], width: 0.6, flow: 1e9)",
    ];
    for body in cases {
        let (e, _) = step_values(&format!("easycad 0.1\n{body}\nshow c\n"), Quality::Final);
        // Reaching this line is the test; most of these are errors, a few are fine shapes.
        let _ = e.diagnostics.len();
    }
}
