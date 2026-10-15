//! Graph view tests: the recipe as components and wires, and every command as a text edit
//! that changes only its own piece and leaves a recipe that runs.

use crate::eval::{Quality, evaluate, evaluate_with};
use crate::graph::{Command, apply, describe, palette};

const VASE: &str = include_str!("../../../web/templates/twisted-vase.easycad");

/// Runs a command that must work, and checks that the result still runs without errors.
fn ok(src: &str, cmd: Command) -> (String, Option<String>) {
    let applied = apply(src, &cmd).unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    let e = evaluate(&applied.source, Quality::Preview);
    assert!(
        e.errors().next().is_none(),
        "{cmd:?}: {:?}\n{}",
        e.diagnostics,
        applied.source
    );
    (applied.source, applied.name)
}

/// The lines that differ between two texts.
fn changed<'a>(a: &'a str, b: &'a str) -> Vec<&'a str> {
    b.lines().filter(|l| !a.lines().any(|m| m == *l)).collect()
}

fn set(step: &str, stage: usize, input: &str, text: &str) -> Command {
    Command::SetInput {
        step: step.into(),
        stage,
        input: input.into(),
        text: text.into(),
    }
}

#[test]
fn describes_components_inputs_and_wires() {
    let g = describe(VASE).unwrap();
    let params: Vec<&str> = g.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(params[..2], ["height", "radius"]);
    let sections = g.steps.iter().find(|s| s.name == "sections").unwrap();
    let nodes: Vec<Option<&str>> = sections.stages.iter().map(|s| s.node.as_deref()).collect();
    assert_eq!(nodes, [None, Some("scale"), Some("rotate"), Some("move")]);
    assert_eq!(sections.stages[0].refs, ["outline"]);
    let scale = &sections.stages[1];
    assert!(scale.piped && scale.known && scale.inputs[0].piped);
    assert_eq!(scale.inputs[1].name, "factor");
    assert_eq!(
        scale.inputs[1].text.as_deref(),
        Some("lerp(1, top_scale, t)")
    );
    assert_eq!(scale.inputs[1].refs, ["top_scale", "t"]);
    assert_eq!(scale.inputs[5].default.as_deref(), Some("[0, 0, 0]"));
    let mv = &sections.stages[3];
    assert_eq!(mv.inputs[3].name, "z");
    assert_eq!(mv.inputs[3].text.as_deref(), Some("height * t"));
    let path = g.steps.iter().find(|s| s.name == "path").unwrap();
    let wall = &path.stages[0].inputs[0];
    assert_eq!(
        (wall.name.as_str(), wall.required, wall.text.as_deref()),
        ("wall", true, Some("wall"))
    );
    assert_eq!(g.shown, ["vase"]);
    assert_eq!(g.outputs.len(), 2);
    assert_eq!(g.outputs[1].file.as_deref(), Some("twisted-vase.stl"));
    // Misspelt and extra values are listed, not hidden.
    let g = describe("a = circle(5, 6, rr: 2)\nb = circl(3)\n").unwrap();
    let names: Vec<(&str, bool)> = g.steps[0].stages[0]
        .inputs
        .iter()
        .map(|i| (i.name.as_str(), i.unknown))
        .collect();
    assert_eq!(names, [("r", false), ("value 2", true), ("rr", true)]);
    assert!(!g.steps[1].stages[0].known);
    // The palette lists every step with its inputs.
    let p = palette();
    let spiralize = p.iter().find(|n| n.name == "spiralize").unwrap();
    assert_eq!(spiralize.group, "Toolpaths");
    assert!(spiralize.inputs[0].list && spiralize.inputs[0].required);
    assert!(describe("a = (").is_err());
}

#[test]
fn inputs_are_set_wired_and_cleared_in_place() {
    // A typed value: only that line changes.
    let (s, _) = ok(VASE, set("outline", 0, "inner", "radius * 0.5"));
    assert_eq!(
        changed(VASE, &s),
        ["outline  = star(points, outer: radius, inner: radius * 0.5) |> smooth"]
    );
    // A wire is a name; an input that wasn't written is added by name.
    let (s, _) = ok(VASE, set("sections", 2, "center", "[0, 0, height]"));
    assert!(
        s.contains("|> rotate(twist * t, center: [0, 0, height])"),
        "{s}"
    );
    // Clearing an optional input takes it back to its default.
    let (s, _) = ok(VASE, set("path", 0, "flow", ""));
    assert!(
        s.contains("path     = vase_spiral(wall, layer: layer, width: line)"),
        "{s}"
    );
    // Refused: values that aren't one value, broken text, and the input a pipe feeds.
    for (cmd, msg) in [
        (set("outline", 0, "inner", "1, 2"), "isn't one value"),
        (set("outline", 0, "inner", "1 +"), "isn't a complete value"),
        (set("outline", 0, "inner", "1) |> move(z: 3"), "isn't"),
        (
            set("sections", 1, "geometry", "outline"),
            "comes from the stage before",
        ),
        (set("sections", 1, "nope", "1"), "has no input"),
        (set("nope", 0, "r", "1"), "no step"),
    ] {
        let e = apply(VASE, &cmd).unwrap_err();
        assert!(e.contains(msg), "{cmd:?}: {e}");
    }
}

#[test]
fn components_are_added_after_their_source() {
    let (s, name) = ok(
        VASE,
        Command::AddStep {
            node: "offset".into(),
            from: Some("outline".into()),
            input: None,
        },
    );
    assert_eq!(name.as_deref(), Some("offset1"));
    let lines: Vec<&str> = s.lines().collect();
    let at = lines.iter().position(|l| l.starts_with("outline")).unwrap();
    assert_eq!(lines[at + 1], "offset1 = offset(outline, 10)");
    // Without a source: at the end of the steps, required inputs waiting to be wired.
    let applied = apply(
        VASE,
        &Command::AddStep {
            node: "loft".into(),
            from: None,
            input: None,
        },
    )
    .unwrap();
    assert!(applied.source.contains(
        "path     = vase_spiral(wall, layer: layer, width: line, flow: flow)\nloft1 = loft()\n"
    ));
    let e = evaluate(&applied.source, Quality::Preview);
    assert!(
        e.errors().any(|d| d.message.contains("missing")),
        "{:?}",
        e.diagnostics
    );
    // Into a chosen input; the geometry still waits for a wire.
    let s = apply(
        VASE,
        &Command::AddStep {
            node: "scale".into(),
            from: Some("top_scale".into()),
            input: Some("factor".into()),
        },
    )
    .unwrap()
    .source;
    assert!(s.contains("scale1 = scale(factor: top_scale)"), "{s}");
    let e = evaluate(&s, Quality::Preview);
    assert!(e.errors().any(|d| d.message.contains("geometry")));
    // A stage piped onto a step, and taken off again.
    let (s, _) = ok(
        VASE,
        Command::AddStage {
            step: "outline".into(),
            node: "offset".into(),
        },
    );
    assert!(s.contains("outline  = star(points, outer: radius, inner: radius * (1 - depth)) |> smooth |> offset(10)"));
    let (s, _) = ok(
        &s,
        Command::RemoveStage {
            step: "outline".into(),
            stage: 2,
        },
    );
    assert_eq!(s, VASE);
    let (s, _) = ok(
        VASE,
        Command::RemoveStage {
            step: "sections".into(),
            stage: 2,
        },
    );
    assert!(
        s.contains("|> scale(lerp(1, top_scale, t))\n           |> move(z: height * t)"),
        "{s}"
    );
    assert!(
        apply(
            VASE,
            &Command::RemoveStage {
                step: "sections".into(),
                stage: 0
            }
        )
        .is_err()
    );
}

#[test]
fn expressions_are_replaced_whole_or_by_stage() {
    let (s, _) = ok(
        VASE,
        Command::SetExpr {
            step: "sections".into(),
            stage: 0,
            text: "outline |> flip".into(),
        },
    );
    assert!(
        s.contains("sections = outline |> flip\n           |> scale("),
        "{s}"
    );
    let (s, _) = ok(
        VASE,
        Command::SetExpr {
            step: "vase".into(),
            stage: 0,
            text: "wall".into(),
        },
    );
    assert!(s.contains("vase     = wall\n"), "{s}");
    for (stage, text) in [(1, "x"), (0, ""), (0, "a b")] {
        let cmd = Command::SetExpr {
            step: "sections".into(),
            stage,
            text: text.into(),
        };
        assert!(apply(VASE, &cmd).is_err(), "{cmd:?}");
    }
}

#[test]
fn removing_renaming_and_showing() {
    let src = "easycad 0.1
param r = 10 in 1..50 \"Radius\"
base = circle(r)
ring = base |> offset(2)
both = loft([base, ring |> move(z: 5)])
show base, ring
";
    // Removing a step empties the inputs wired only to it and takes it out of `show`.
    let applied = apply(
        src,
        &Command::Remove {
            name: "ring".into(),
        },
    )
    .unwrap();
    assert_eq!(
        applied.source,
        "easycad 0.1
param r = 10 in 1..50 \"Radius\"
base = circle(r)
both = loft([base, ring |> move(z: 5)])
show base
"
    );
    let applied = apply(src, &Command::Remove { name: "r".into() }).unwrap();
    assert!(applied.source.contains("base = circle()\n") && !applied.source.contains("param"));
    // Renaming reaches every use.
    let (s, _) = ok(
        src,
        Command::Rename {
            name: "base".into(),
            to: "profile".into(),
        },
    );
    assert!(
        s.contains("profile = circle(r)\nring = profile |> offset(2)\nboth = loft([profile, ring")
    );
    assert!(s.ends_with("show profile, ring\n"));
    for to in ["ring", "2a", "show", "x"] {
        assert!(
            apply(
                src,
                &Command::Rename {
                    name: "base".into(),
                    to: to.into()
                }
            )
            .is_err(),
            "{to}"
        );
    }
    // Previews.
    let shown = |s: &str, step: &str, on: bool| {
        apply(
            s,
            &Command::SetShown {
                step: step.into(),
                shown: on,
            },
        )
        .unwrap()
        .source
    };
    assert!(shown(src, "both", true).ends_with("show base, ring, both\n"));
    assert!(shown(src, "base", false).ends_with("show ring\n"));
    assert!(shown(src, "ring", false).ends_with("show base\n"));
    assert!(!shown(&shown(src, "ring", false), "base", false).contains("show"));
    assert_eq!(shown(src, "base", true), src);
    let bare = "a = circle(5)\nprint p\n";
    assert_eq!(shown(bare, "a", true), "a = circle(5)\nshow a\nprint p\n");
}

#[test]
fn sliders_are_added_and_changed() {
    let (s, name) = ok(
        VASE,
        Command::AddParam {
            value: 5.0,
            min: 0.0,
            max: 10.0,
        },
    );
    assert_eq!(name.as_deref(), Some("slider1"));
    assert!(s.contains("param flow      = 8       in 1..25            \"Flow (mm³/s)\"\nparam slider1 = 5 in 0..10 \"Slider 1\"\n"), "{s}");
    let (s, _) = ok(
        VASE,
        Command::SetParam {
            name: "height".into(),
            value: 120.0,
            min: 50.0,
            max: 250.0,
            step: Some(5.0),
            label: None,
        },
    );
    assert!(
        s.contains("param height = 120 mm in 50..250 step 5 \"Height\"\n"),
        "{s}"
    );
    let src = "param r = 5\nc = circle(r)\n";
    let (s, name) = ok(
        src,
        Command::AddParam {
            value: 50.0,
            min: 10.0,
            max: 0.0,
        },
    );
    assert_eq!(
        (name.as_deref(), s.as_str()),
        (
            Some("slider1"),
            "param r = 5\nparam slider1 = 10 in 0..10 \"Slider 1\"\nc = circle(r)\n"
        )
    );
    let e = apply(
        VASE,
        &Command::SetParam {
            name: "height".into(),
            value: f64::NAN,
            min: 0.0,
            max: 1.0,
            step: None,
            label: None,
        },
    );
    assert!(e.is_err());
}

#[test]
fn the_selected_step_is_drawn_on_its_own() {
    let e = evaluate_with(VASE, Quality::Preview, Some("outline"));
    assert_eq!(e.highlight.curves.len(), 1);
    assert!(e.highlight.mesh.indices.is_empty() && e.highlight.paths.is_empty());
    // The main drawing is unchanged.
    assert_eq!(e.drawing, evaluate(VASE, Quality::Preview).drawing);
    let e = evaluate_with(VASE, Quality::Preview, Some("path"));
    assert_eq!(e.highlight.paths.len(), 1);
    let e = evaluate_with(VASE, Quality::Preview, Some("height"));
    assert!(e.highlight.curves.is_empty());
}

/// Every command on every component of every template: each either gives a recipe that parses
/// (`apply` checks) or a message. Nothing panics.
#[test]
fn every_command_on_every_template_parses_or_says_why() {
    let templates = [
        VASE,
        include_str!("../../../web/templates/ribbed-cylinder.easycad"),
        include_str!("../../../web/templates/star-bowl.easycad"),
        include_str!("../../../web/templates/spiral-disc.easycad"),
        include_str!("../../../web/templates/cut-cylinder.easycad"),
        include_str!("../../../web/templates/turned-vase.easycad"),
    ];
    let mut worked = 0;
    for src in templates {
        let g = describe(src).unwrap();
        let first = g.steps.first().map(|s| s.name.clone()).unwrap_or_default();
        let mut try_it = |cmd: Command| {
            if apply(src, &cmd).is_ok() {
                worked += 1;
            }
        };
        for step in &g.steps {
            for (k, stage) in step.stages.iter().enumerate() {
                for input in &stage.inputs {
                    for text in [
                        "",
                        "1",
                        "x * 2",
                        first.as_str(),
                        "[1, 2]",
                        "(",
                        "a, b",
                        "\n",
                    ] {
                        try_it(set(&step.name, k, &input.name, text));
                    }
                }
                try_it(Command::RemoveStage {
                    step: step.name.clone(),
                    stage: k,
                });
            }
            for stage in 0..step.stages.len() {
                try_it(Command::SetExpr {
                    step: step.name.clone(),
                    stage,
                    text: "circle(3)".into(),
                });
            }
            try_it(Command::Remove {
                name: step.name.clone(),
            });
            try_it(Command::Rename {
                name: step.name.clone(),
                to: format!("{}_2", step.name),
            });
            for shown in [true, false] {
                try_it(Command::SetShown {
                    step: step.name.clone(),
                    shown,
                });
            }
        }
        for p in &g.params {
            try_it(Command::Remove {
                name: p.name.clone(),
            });
            try_it(Command::SetParam {
                name: p.name.clone(),
                value: 3.0,
                min: 1.0,
                max: 5.0,
                step: None,
                label: Some("Changed".into()),
            });
        }
        for node in palette() {
            try_it(Command::AddStep {
                node: node.name.into(),
                from: Some(first.clone()),
                input: None,
            });
            try_it(Command::AddStage {
                step: first.clone(),
                node: node.name.into(),
            });
        }
    }
    assert!(worked > 1000, "{worked}");
}
