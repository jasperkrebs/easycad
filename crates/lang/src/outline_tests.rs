//! Outline and graph-edit tests: byte ranges, references, and edits that touch only their
//! piece of the text.

use crate::edit;
use crate::outline::{OutputKind, outline};
use crate::*;

const RECIPE: &str = "easycad 0.1
project \"Test\"
note \"n\"

group \"Shape\"
param height = 160 mm in 40..300 \"Height\"
param twist  = 90 deg  # turns
param points = 7 in 3..16 step 1

# One star, copied and turned.
outline  = star(points, outer: 45, inner: 36) |> smooth
t        = range(0, 1, count: 16)
sections = outline
           |> scale(lerp(1, 0.8, t))
           |> rotate(twist * t)   # comment
           |> move(z: height * t)
wall     = loft(sections)
spacing  = 2 mm * (1 + t)
path     = vase_spiral(wall, layer: 0.3, width: 0.6, flow: 8)

show wall, outline
print path
export cap(wall) as \"vase.stl\"
";

fn text(src: &str, r: (usize, usize)) -> &str {
    &src[r.0..r.1]
}

#[test]
fn outlines_every_statement_with_its_text() {
    let o = outline(RECIPE).unwrap();
    let names: Vec<&str> = o.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["height", "twist", "points"]);
    assert_eq!(
        text(RECIPE, o.params[1].lines.range),
        "param twist  = 90 deg  # turns\n"
    );
    assert_eq!(text(RECIPE, o.params[1].range), "param twist  = 90 deg");

    let outline_step = o.step("outline").unwrap();
    assert_eq!(outline_step.stages.len(), 2);
    let star = &outline_step.stages[0];
    assert_eq!(star.call.as_deref(), Some("star"));
    assert!(!star.piped);
    assert_eq!(star.args.len(), 3);
    assert_eq!(star.args[0].name, None);
    assert_eq!(star.args[0].refs, ["points"]);
    assert_eq!(star.args[1].name.as_deref(), Some("outer"));
    assert_eq!(text(RECIPE, star.args[1].value), "45");
    assert_eq!(text(RECIPE, star.args[1].full), "outer: 45");
    let smooth = &outline_step.stages[1];
    assert_eq!(
        (smooth.call.as_deref(), smooth.piped, smooth.close),
        (Some("smooth"), true, None)
    );

    let sections = o.step("sections").unwrap();
    assert_eq!((sections.lines.line, sections.lines.end_line), (13, 16));
    assert_eq!(sections.stages.len(), 4);
    assert_eq!(sections.stages[0].call, None);
    assert_eq!(sections.stages[0].refs, ["outline"]);
    assert_eq!(text(RECIPE, sections.stages[2].args[0].value), "twist * t");
    assert_eq!(sections.stages[2].args[0].refs, ["twist", "t"]);
    assert_eq!(sections.stages[3].args[0].name.as_deref(), Some("z"));
    assert!(text(RECIPE, sections.lines.range).ends_with("move(z: height * t)\n"));

    // Units after numbers aren't names.
    assert_eq!(o.step("spacing").unwrap().stages[0].refs, ["t"]);

    let kinds: Vec<OutputKind> = o.outputs.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [OutputKind::Show, OutputKind::Print, OutputKind::Export]
    );
    assert_eq!(
        o.outputs[0]
            .names
            .iter()
            .map(|n| n.0.as_str())
            .collect::<Vec<_>>(),
        ["wall", "outline"]
    );
    assert_eq!(o.outputs[2].refs, ["wall"]);
    assert_eq!(o.outputs[2].file.as_deref(), Some("vase.stl"));
    assert_eq!(
        text(RECIPE, (0, o.header_end)),
        "easycad 0.1\nproject \"Test\"\nnote \"n\"\n"
    );
}

/// The outline reads the same names as the parser, step by step, in every template.
#[test]
fn outline_references_match_the_parser() {
    let templates = [
        include_str!("../../../web/templates/twisted-vase.easycad"),
        include_str!("../../../web/templates/ribbed-cylinder.easycad"),
        include_str!("../../../web/templates/star-bowl.easycad"),
        include_str!("../../../web/templates/spiral-disc.easycad"),
        include_str!("../../../web/templates/cut-cylinder.easycad"),
        include_str!("../../../web/templates/turned-vase.easycad"),
        RECIPE,
    ];
    for src in templates {
        let program = parse(src).unwrap();
        let o = outline(src).unwrap();
        assert_eq!(o.steps.len(), program.bindings.len());
        for (b, s) in program.bindings.iter().zip(&o.steps) {
            assert_eq!(b.name, s.name);
            let mut want = Vec::new();
            b.expr.references(&mut want);
            let mut want: Vec<String> = want.into_iter().map(|(n, _)| n).collect();
            let mut got: Vec<String> = s
                .stages
                .iter()
                .flat_map(|st| st.refs.iter().chain(st.args.iter().flat_map(|a| &a.refs)))
                .cloned()
                .collect();
            for list in [&mut want, &mut got] {
                list.sort();
                list.dedup();
            }
            assert_eq!(got, want, "step {}", b.name);
        }
    }
}

#[test]
fn setting_inputs_replaces_or_adds_arguments() {
    let o = outline(RECIPE).unwrap();
    let star = &o.step("outline").unwrap().stages[0];
    // Positional input 0 and named input `inner`.
    let s = edit::set_arg(RECIPE, star, 0, "points", "5").unwrap();
    assert!(s.contains("star(5, outer: 45, inner: 36)"));
    let s = edit::set_arg(RECIPE, star, 2, "inner", "outer_r * 0.8").unwrap();
    assert!(s.contains("star(points, outer: 45, inner: outer_r * 0.8)"));
    // A call written without brackets gets them.
    let smooth = &o.step("outline").unwrap().stages[1];
    let s = edit::set_arg(RECIPE, smooth, 1, "tension", "2").unwrap();
    assert!(s.contains("|> smooth(tension: 2)"));
    // A missing input is added by name; a piped stage's positional values start at input 1.
    let scale = &o.step("sections").unwrap().stages[1];
    assert_eq!(
        edit::find_arg(scale, 1, "factor").map(|a| text(RECIPE, a.value)),
        Some("lerp(1, 0.8, t)")
    );
    let s = edit::set_arg(RECIPE, scale, 5, "center", "[0, 0, 10]").unwrap();
    assert!(s.contains("|> scale(lerp(1, 0.8, t), center: [0, 0, 10])"));
    // Everything else is untouched.
    let changed: Vec<(&str, &str)> = RECIPE
        .lines()
        .zip(s.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 1);
    parse(&s).unwrap();
}

#[test]
fn removing_inputs_keeps_the_others_on_their_inputs() {
    let src = "a = star(7, 45, 36)\nb = circle(r: 3)\nc = a |> move(1, 2, z: 3)\n";
    let o = outline(src).unwrap();
    let star = &o.step("a").unwrap().stages[0];
    let inputs = ["points", "outer", "inner"];
    // The last one just goes.
    let s = edit::remove_arg(src, star, 2, &inputs).unwrap();
    assert!(s.starts_with("a = star(7, 45)\n"), "{s}");
    // One in the middle: the one after it is named, so it still feeds `inner`.
    let s = edit::remove_arg(src, star, 1, &inputs).unwrap();
    assert!(s.starts_with("a = star(7, inner: 36)\n"), "{s}");
    let s = edit::remove_arg(src, star, 0, &inputs).unwrap();
    assert!(s.starts_with("a = star(outer: 45, inner: 36)\n"), "{s}");
    // The only argument.
    let circle = &o.step("b").unwrap().stages[0];
    let s = edit::remove_arg(src, circle, 0, &["r"]).unwrap();
    assert!(s.contains("b = circle()\n"), "{s}");
    // In a piped stage: input 1 is the first value in the brackets.
    let mv = &o.step("c").unwrap().stages[1];
    let s = edit::remove_arg(src, mv, 1, &["geometry", "x", "y", "z"]).unwrap();
    assert!(s.ends_with("c = a |> move(y: 2, z: 3)\n"), "{s}");
    let s = edit::remove_arg(src, mv, 3, &["geometry", "x", "y", "z"]).unwrap();
    assert!(s.ends_with("c = a |> move(1, 2)\n"), "{s}");
    // An input without an argument: nothing to do.
    assert_eq!(edit::remove_arg(src, circle, 5, &["r"]).unwrap(), src);
}

#[test]
fn renaming_changes_names_not_calls_or_labels() {
    let src = "param r = 5\ncircle = circle(r: r)\nbig = circle |> scale(2)\nshow circle, big\nprint path(circle)\n";
    let s = edit::rename(src, "circle", "ring").unwrap();
    assert_eq!(
        s,
        "param r = 5\nring = circle(r: r)\nbig = ring |> scale(2)\nshow ring, big\nprint path(ring)\n"
    );
    let s = edit::rename(src, "r", "radius").unwrap();
    assert!(
        s.starts_with("param radius = 5\ncircle = circle(r: radius)\n"),
        "{s}"
    );
    // `2 m` is two metres, not a step named m.
    let s = edit::rename("m = 1\nx = 2 m + m\n", "m", "k").unwrap();
    assert_eq!(s, "k = 1\nx = 2 m + k\n");
    assert!(edit::is_name("ring_2") && !edit::is_name("2ring") && !edit::is_name("show"));
}

#[test]
fn new_lines_go_where_asked() {
    assert_eq!(
        edit::insert_line("a = 1", 5, "b = 2").unwrap(),
        "a = 1\nb = 2\n"
    );
    assert_eq!(
        edit::insert_line("a = 1\n", 6, "b = 2").unwrap(),
        "a = 1\nb = 2\n"
    );
    assert_eq!(
        edit::insert_line("a = 1\nc = 3\n", 6, "b = 2").unwrap(),
        "a = 1\nb = 2\nc = 3\n"
    );
    assert!(edit::replace("abc", (2, 9), "x").is_err());
}
