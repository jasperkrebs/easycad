//! Evaluator tests: the templates, Phase 0 parity, diagnostics, and hostile recipes.

use crate::eval::{Evaluation, Quality, Severity, evaluate};
use easycad_lang::{Literal, set_values};

const TEMPLATES: [(&str, &str); 3] = [
    (
        "twisted-vase",
        include_str!("../../../web/templates/twisted-vase.easycad"),
    ),
    (
        "ribbed-cylinder",
        include_str!("../../../web/templates/ribbed-cylinder.easycad"),
    ),
    (
        "star-bowl",
        include_str!("../../../web/templates/star-bowl.easycad"),
    ),
];

fn run(src: &str) -> Evaluation {
    evaluate(src, Quality::Preview)
}

/// `(line, col, message)` of every diagnostic with this severity.
fn diags(e: &Evaluation, severity: Severity) -> Vec<(u32, u32, String)> {
    e.diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| (d.line, d.col, d.message.clone()))
        .collect()
}

fn status(e: &Evaluation, name: &str) -> &'static str {
    e.steps
        .iter()
        .find(|s| s.name == name)
        .map_or("missing", |s| s.status)
}

fn summary(e: &Evaluation, name: &str) -> String {
    e.steps
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.summary.clone())
        .unwrap_or_default()
}

fn with(src: &str, name: &str, value: f64) -> String {
    set_values(src, &[(name.to_string(), Literal::Number(value))]).unwrap()
}

#[test]
fn templates_run_cleanly() {
    for (name, src) in TEMPLATES {
        for quality in [Quality::Preview, Quality::Final] {
            let e = evaluate(src, quality);
            assert!(e.diagnostics.is_empty(), "{name}: {:?}", e.diagnostics);
            assert!(e.print.is_some(), "{name}: nothing to print");
            assert!(
                e.project.is_some() && !e.params.is_empty() && !e.notes.is_empty(),
                "{name}"
            );
            assert!(!e.drawing.paths.is_empty(), "{name}");
            let s = &e.print.as_ref().unwrap().stats;
            assert!(
                s.max_overhang_deg < 45.0,
                "{name}: overhang {}",
                s.max_overhang_deg
            );
        }
    }
}

/// The twisted vase recipe must print like the Phase 0 demo, whose numbers were recorded
/// before the demo was rewritten as a recipe (same settings, final quality).
#[test]
fn twisted_vase_recipe_matches_phase_0() {
    let src = TEMPLATES[0].1;
    let close = |got: f64, want: f64, tol: f64| (got - want).abs() <= tol * want.abs();
    let cases = [
        // (twist, top scale, path length, volume, overhang)
        (90.0, 0.8, 135_442.76, 21_723.00, 11.40),
        (180.0, 0.8, 135_494.75, 21_731.34, 24.00),
        (0.0, 2.0, 225_839.03, 36_221.10, 15.72),
    ];
    for (twist, top, length, volume, overhang) in cases {
        let recipe = with(&with(src, "twist", twist), "top_scale", top);
        let e = evaluate(&recipe, Quality::Final);
        let s = &e.print.as_ref().unwrap().stats;
        assert_eq!(s.layers, 533);
        assert!((s.printed_height_mm - 159.9).abs() < 1e-9);
        assert_eq!((s.samples_per_turn, s.path_points), (224, 119_617));
        assert!(
            close(s.path_length_mm, length, 0.0002),
            "twist {twist}: length {} vs {length}",
            s.path_length_mm
        );
        assert!(
            close(s.volume_mm3, volume, 0.0002),
            "twist {twist}: volume {} vs {volume}",
            s.volume_mm3
        );
        assert!(
            (s.max_overhang_deg - overhang).abs() < 0.5,
            "twist {twist}: overhang {} vs {overhang}",
            s.max_overhang_deg
        );
    }
}

#[test]
fn settings_window_comes_from_the_params() {
    let e = run(TEMPLATES[0].1);
    let names: Vec<&str> = e.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "height",
            "radius",
            "points",
            "depth",
            "twist",
            "top_scale",
            "layer",
            "line",
            "speed"
        ]
    );
    let depth = &e.params[3];
    assert_eq!(
        (depth.kind, depth.unit, depth.min, depth.max),
        ("number", "%", Some(0.0), Some(60.0))
    );
    assert_eq!(
        (depth.label.as_str(), depth.group.as_deref(), depth.line),
        ("Star depth", Some("Shape"), 10)
    );
    assert_eq!(e.params[2].step, Some(1.0));
    assert_eq!(e.params[8].group.as_deref(), Some("Print"));

    let e = run(
        "param flag = true\nparam mode = \"a\" of [a, b]\nparam caption = \"hi\"\nparam top_scale = 2",
    );
    let kinds: Vec<&str> = e.params.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, ["toggle", "choice", "text", "number"]);
    assert_eq!(e.params[3].label, "Top scale");
}

#[test]
fn units_become_base_values() {
    let e = run("param d = 18 %\nparam w = 2 cm\nx = d * 100 + w");
    assert_eq!(summary(&e, "x"), "38");
}

#[test]
fn out_of_range_and_unknown_choices_warn() {
    let e = run("param h = 500 mm in 10..300\nparam m = \"c\" of [a, b]");
    assert_eq!(
        diags(&e, Severity::Warning),
        [
            (1, 1, "`h` is 500, outside its range 10..300".to_string()),
            (2, 1, "`m` is not one of its choices".to_string())
        ]
    );
}

#[test]
fn errors_belong_to_their_step_and_the_rest_still_runs() {
    let src = "param r = 10 mm
good  = circle(r)
bad   = circle(-r)
later = bad |> move(z: 5)
other = good |> move(z: 5)
show other";
    let e = run(src);
    assert_eq!(
        diags(&e, Severity::Error),
        [(3, 9, "circle: `r` must be more than 0, got -10".to_string())]
    );
    assert_eq!(
        [
            status(&e, "good"),
            status(&e, "bad"),
            status(&e, "later"),
            status(&e, "other")
        ],
        ["ok", "error", "skipped", "ok"]
    );
    assert_eq!(e.drawing.curves.len(), 1);
}

#[test]
fn typos_get_suggestions() {
    let e = run(
        "param height = 10\nx = hieght * 2\ny = lofft(x)\nz = circle(rr: 3)\nw = vase_spiral(x, layr: 1, width: 1)",
    );
    assert_eq!(
        diags(&e, Severity::Error),
        [
            (2, 5, "unknown name `hieght`; did you mean `height`?".to_string()),
            (3, 5, "there's no step called `lofft`; did you mean `loft`?".to_string()),
            (4, 16, "circle: has no input called `rr`; did you mean `r`? (inputs: r)".to_string()),
            (5, 26, "vase_spiral: has no input called `layr`; did you mean `layer`? (inputs: wall, layer, width, speed)".to_string()),
        ]
    );
    let e = run("x = zzz");
    assert_eq!(
        diags(&e, Severity::Error)[0].2,
        "unknown name `zzz`; define it with `param zzz = …` or `zzz = …`"
    );
}

#[test]
fn loops_are_reported_once_per_step_in_them() {
    let e = run("a = b + 1\nb = a * 2\nc = a\nd = 4\nself = self");
    let errors = diags(&e, Severity::Error);
    assert_eq!(
        errors,
        [
            (1, 1, "`a` depends on itself (a → b → a)".to_string()),
            (2, 1, "`b` depends on itself (a → b → a)".to_string()),
            (5, 1, "`self` depends on itself (self → self)".to_string()),
        ]
    );
    assert_eq!([status(&e, "c"), status(&e, "d")], ["skipped", "ok"]);
}

#[test]
fn names_can_only_be_defined_once() {
    let e = run("param x = 1\nparam x = 2\nx = 3\ny = 4\ny = 5");
    let errors: Vec<String> = diags(&e, Severity::Error)
        .into_iter()
        .map(|d| d.2)
        .collect();
    assert_eq!(
        errors,
        [
            "`x` is defined twice (first on line 1)",
            "`x` is defined twice (first on line 1)",
            "`y` is defined twice (first on line 4)"
        ]
    );
    assert_eq!(summary(&e, "y"), "4");
}

#[test]
fn arguments_are_checked() {
    let msgs = |src: &str| -> Vec<String> {
        diags(&run(src), Severity::Error)
            .into_iter()
            .map(|d| d.2)
            .collect()
    };
    assert_eq!(
        msgs("x = circle(1, 2)"),
        ["circle: too many values; it takes 1 (circle(r: Length) -> Curve)"]
    );
    assert_eq!(
        msgs("x = star(5, outer: 3)"),
        ["star: missing `inner` (star(points: Count, outer: Length, inner: Length) -> Curve)"]
    );
    assert_eq!(msgs("x = circle(1, r: 2)"), ["circle: `r` is given twice"]);
    assert_eq!(
        msgs("x = circle(\"big\")"),
        ["circle: `r` must be a number, got text"]
    );
}

#[test]
fn operators_work_on_lists_item_by_item() {
    let e = run(
        "t = range(0, 1, count: 3)\nu = t * 2 + 1\nv = -t\nw = [1, 2] + [10, 20, 30]\npi2 = pi * 2",
    );
    assert_eq!(summary(&e, "u"), "[1, 2, 3]");
    assert_eq!(summary(&e, "v"), "[0, -0.5, -1]");
    assert_eq!(summary(&e, "w"), "[11, 22, 32]");
    assert_eq!(summary(&e, "pi2"), "6.283185");
}

#[test]
fn arithmetic_mistakes_are_explained() {
    let msgs = |src: &str| -> Vec<String> {
        diags(&run(src), Severity::Error)
            .into_iter()
            .map(|d| d.2)
            .collect()
    };
    assert_eq!(msgs("x = 1 / 0"), ["division by zero"]);
    assert_eq!(
        msgs("x = circle(5) * 2"),
        ["can't multiply a curve and a number; to resize geometry use `scale(…, factor)`"]
    );
    assert_eq!(
        msgs("x = circle(5) + 2"),
        ["can't add a curve and a number; to move geometry use `move(…, x: …, y: …, z: …)`"]
    );
    assert_eq!(msgs("x = 10 ^ 400"), ["10 ^ 400 is not a finite number"]);
    assert_eq!(
        msgs("x = -circle(1)"),
        ["can't put a minus in front of a curve"]
    );
}

#[test]
fn outputs() {
    let e = run("x = 3\nshow x");
    assert_eq!(
        diags(&e, Severity::Warning),
        [(2, 6, "`x` is a number; there's nothing to draw".to_string())]
    );

    let e = run("c = circle(5)\nprint c");
    assert_eq!(
        diags(&e, Severity::Error),
        [(
            2,
            1,
            "`print` needs a toolpath (from vase_spiral), got a curve".to_string()
        )]
    );

    let e = run("c = circle(5)\nexport c as \"c.stl\"");
    assert_eq!(diags(&e, Severity::Warning).len(), 1);

    let e = run("c = circle(5)\nd = 2");
    assert_eq!(e.drawing.curves.len(), 1);
    assert_eq!(diags(&e, Severity::Info).len(), 1);

    let src = "printer \"Klipper pellet\"\nw = loft([circle(10), circle(10) |> move(z: 3)])\np = vase_spiral(w, layer: 1, width: 2)\nprint p\nprint p with \"Other\"\nshow p";
    let e = run(src);
    let print = e.print.as_ref().unwrap();
    assert_eq!(print.printer.as_deref(), Some("Klipper pellet"));
    assert_eq!(print.stats.layers, 3);
    assert_eq!(e.drawing.paths.len(), 1, "the same toolpath is drawn once");
    assert_eq!(
        diags(&e, Severity::Warning),
        [(5, 1, "only the first `print` is used for now".to_string())]
    );
    let e = run(&src.replace("print p\nprint p with \"Other\"", "print p with \"Other\""));
    assert_eq!(e.print.unwrap().printer.as_deref(), Some("Other"));
}

#[test]
fn parse_errors_and_versions() {
    let e = run("x = (1");
    assert_eq!(
        diags(&e, Severity::Error),
        [(1, 7, "expected `)`, found the end of the file".to_string())]
    );
    let e = run("easycad 0.2\nx = 1");
    assert_eq!(
        diags(&e, Severity::Warning),
        [(
            1,
            1,
            "this recipe says `easycad 0.2`; this app reads version 0.1".to_string()
        )]
    );
    let e = run(&"x = 1\n".repeat(60_000));
    assert!(
        diags(&e, Severity::Error)[0]
            .2
            .starts_with("the recipe is too long")
    );
}

#[test]
fn too_much_geometry_is_refused_with_a_message() {
    let e = run("t = range(0, 1, count: 100000)\nc = circle(t + 1)");
    assert!(
        diags(&e, Severity::Error)[0]
            .2
            .contains("too much geometry"),
        "{:?}",
        e.diagnostics
    );
    let e = run(
        "w = loft([circle(10), circle(10) |> move(z: 2000)])\np = vase_spiral(w, layer: 0.02, width: 0.4)",
    );
    assert!(
        diags(&e, Severity::Error)[0].2.contains("too many points"),
        "{:?}",
        e.diagnostics
    );
}

#[test]
fn same_recipe_same_result() {
    let a = evaluate(TEMPLATES[2].1, Quality::Final);
    let b = evaluate(TEMPLATES[2].1, Quality::Final);
    assert_eq!(a, b);
}

#[test]
fn every_node_is_documented_in_the_language_doc() {
    let doc = include_str!("../../../docs/definition-language.md");
    for node in crate::nodes::NODES {
        assert!(
            doc.contains(&format!("`{}(", node.name)),
            "docs/definition-language.md doesn't mention `{}(`",
            node.name
        );
    }
}

/// Random recipes built from real syntax and real node names: whatever they do, evaluating
/// them must return (an error is fine, a panic is not).
#[test]
fn random_recipes_never_crash() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move |n: usize| -> usize {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let numbers = [
        "0", "1", "-1", "0.5", "3", "7", "16", "100", "2 cm", "45 deg", "1e6", "-1e9", "1e300",
        "0.001", "18 %",
    ];
    let nodes: Vec<&str> = crate::nodes::NODES.iter().map(|n| n.name).collect();
    for _ in 0..400 {
        let mut src = String::from("param a = 3\nparam b = 0.5 mm\n");
        let steps = 1 + next(6);
        for i in 0..steps {
            let mut expr = String::new();
            gen_expr(&mut next, &numbers, &nodes, steps, 3, &mut expr);
            src.push_str(&format!("s{i} = {expr}\n"));
        }
        src.push_str(&format!("show s{}\nprint s{}\n", next(steps), next(steps)));
        let _ = evaluate(&src, Quality::Preview);
    }
}

fn gen_expr(
    next: &mut impl FnMut(usize) -> usize,
    numbers: &[&str],
    nodes: &[&str],
    steps: usize,
    depth: usize,
    out: &mut String,
) {
    let pick = if depth == 0 { next(3) } else { next(8) };
    match pick {
        0 => out.push_str(numbers[next(numbers.len())]),
        1 => out.push_str(["a", "b", "pi"][next(3)]),
        2 => out.push_str(&format!("s{}", next(steps))),
        3 | 4 => {
            out.push_str(nodes[next(nodes.len())]);
            out.push('(');
            for k in 0..next(5) {
                if k > 0 {
                    out.push_str(", ");
                }
                gen_expr(next, numbers, nodes, steps, depth - 1, out);
            }
            out.push(')');
        }
        5 => {
            out.push('[');
            for k in 0..next(4) {
                if k > 0 {
                    out.push_str(", ");
                }
                gen_expr(next, numbers, nodes, steps, depth - 1, out);
            }
            out.push(']');
        }
        6 => {
            gen_expr(next, numbers, nodes, steps, depth - 1, out);
            out.push_str([" + ", " - ", " * ", " / ", " ^ "][next(5)]);
            gen_expr(next, numbers, nodes, steps, depth - 1, out);
        }
        _ => {
            gen_expr(next, numbers, nodes, steps, depth - 1, out);
            out.push_str(" |> ");
            out.push_str(nodes[next(nodes.len())]);
            out.push('(');
            gen_expr(next, numbers, nodes, steps, depth - 1, out);
            out.push(')');
        }
    }
}
