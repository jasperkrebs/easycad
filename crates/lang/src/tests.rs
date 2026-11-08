//! Parser and rewrite tests: the whole syntax surface, error positions, and round trips.

use crate::*;

/// Evaluates a pure-number expression, to check precedence without a geometry engine.
fn calc(e: &Expr) -> f64 {
    match e {
        Expr::Number { value, .. } => *value,
        Expr::Neg { expr, .. } => -calc(expr),
        Expr::Binary { op, lhs, rhs, .. } => {
            let (a, b) = (calc(lhs), calc(rhs));
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => a / b,
                BinOp::Pow => a.powf(b),
            }
        }
        other => panic!("not a number expression: {other:?}"),
    }
}

fn binding(src: &str) -> Expr {
    parse(src).unwrap().bindings.remove(0).expr
}

fn err(src: &str) -> (u32, u32, String) {
    let e = parse(src).unwrap_err();
    (e.line, e.col, e.message)
}

fn call_name(e: &Expr) -> &str {
    match e {
        Expr::Call { name, .. } => name,
        other => panic!("not a call: {other:?}"),
    }
}

fn call_args(e: &Expr) -> &[Arg] {
    match e {
        Expr::Call { args, .. } => args,
        other => panic!("not a call: {other:?}"),
    }
}

#[test]
fn header_settings_and_groups() {
    let src = r#"easycad 0.1
project "Twisted vase"
printer "Bambu P1P 0.4"
note "first note"
note "second note"

param loose = 3
group "Shape"
param height = 160 mm in 60..300 step 5 "Height" @assumed @locked
param depth  = 18 % in 0..40 "Star depth"   # a comment
group "Print"
param lean   = -12.5 deg in -45..45
param flip   = true "Flip"
param mode   = "spiral" of [spiral, rings, "two words", 3] "Mode"
"#;
    let p = parse(src).unwrap();
    assert_eq!(p.version.as_deref(), Some("0.1"));
    assert_eq!(p.project.as_deref(), Some("Twisted vase"));
    assert_eq!(p.printer.as_deref(), Some("Bambu P1P 0.4"));
    assert_eq!(p.notes, vec!["first note", "second note"]);
    assert_eq!(p.params.len(), 6);

    let loose = &p.params[0];
    assert_eq!(
        (loose.name.as_str(), loose.group.as_deref()),
        ("loose", None)
    );

    let h = &p.params[1];
    assert_eq!(h.name, "height");
    assert_eq!(h.value, Literal::Number(160.0));
    assert_eq!(h.unit, Unit::Mm);
    assert_eq!(h.range, Some((60.0, 300.0)));
    assert_eq!(h.step, Some(5.0));
    assert_eq!(h.label.as_deref(), Some("Height"));
    assert_eq!(h.group.as_deref(), Some("Shape"));
    assert_eq!(h.attrs, vec!["assumed", "locked"]);
    assert_eq!(h.span, Span { line: 9, col: 1 });
    assert_eq!(&src[h.value_range.0..h.value_range.1], "160");

    let d = &p.params[2];
    assert_eq!(
        (d.unit, d.range, d.label.as_deref()),
        (Unit::Percent, Some((0.0, 40.0)), Some("Star depth"))
    );

    let lean = &p.params[3];
    assert_eq!(
        (lean.value.clone(), lean.unit, lean.range),
        (Literal::Number(-12.5), Unit::Deg, Some((-45.0, 45.0)))
    );
    assert_eq!(lean.group.as_deref(), Some("Print"));
    assert_eq!(&src[lean.value_range.0..lean.value_range.1], "-12.5");

    assert_eq!(p.params[4].value, Literal::Bool(true));
    let mode = &p.params[5];
    assert_eq!(mode.value, Literal::Text("spiral".into()));
    assert_eq!(
        mode.choices,
        Some(vec![
            Literal::Text("spiral".into()),
            Literal::Text("rings".into()),
            Literal::Text("two words".into()),
            Literal::Number(3.0),
        ])
    );
}

#[test]
fn numbers_convert_to_base_units() {
    let cases = [
        ("x = 2 cm", 20.0),
        ("x = 1.5 m", 1500.0),
        ("x = 1 inch", 25.4),
        ("x = 18 %", 0.18),
        ("x = 90 deg", 90.0),
        ("x = 7 mm", 7.0),
        ("x = 2e3", 2000.0),
        ("x = .5", 0.5),
    ];
    for (src, want) in cases {
        let got = calc(&binding(src));
        assert!((got - want).abs() < 1e-9, "{src}: {got} != {want}");
    }
    let rad = calc(&binding("x = 3.141592653589793 rad"));
    assert!((rad - 180.0).abs() < 1e-9);
}

#[test]
fn precedence_and_associativity() {
    let cases = [
        ("x = 1 + 2 * 3", 7.0),
        ("x = (1 + 2) * 3", 9.0),
        ("x = 2 ^ 3 ^ 2", 512.0),
        ("x = -2 ^ 2", -4.0),
        ("x = 2 ^ -1", 0.5),
        ("x = 8 / 4 / 2", 1.0),
        ("x = 10 - 4 - 3", 3.0),
        ("x = - - 3", 3.0),
        ("x = 1 + 2 * 3 ^ 2 - -4", 23.0),
        ("x = 10 mm + 1 cm", 20.0),
    ];
    for (src, want) in cases {
        assert_eq!(calc(&binding(src)), want, "{src}");
    }
}

#[test]
fn pipes_become_calls_and_may_wrap_lines() {
    let src = "sections = base\n           |> scale(lerp(1, top, t))   # comment\n\n           |> rotate(twist * t)\n           |> cap\nnext = 1";
    let p = parse(src).unwrap();
    assert_eq!(p.bindings.len(), 2);
    let e = &p.bindings[0].expr;
    assert_eq!(call_name(e), "cap");
    let rotate = &call_args(e)[0].value;
    assert_eq!(call_name(rotate), "rotate");
    let scale = &call_args(rotate)[0].value;
    assert_eq!(call_name(scale), "scale");
    let args = call_args(scale);
    assert_eq!(args.len(), 2);
    assert_eq!(
        args[0].value,
        Expr::Ident {
            name: "base".into(),
            span: Span { line: 1, col: 12 }
        }
    );
    assert_eq!(call_name(&args[1].value), "lerp");
    assert_eq!(p.bindings[1].name, "next");
}

#[test]
fn calls_lists_and_named_arguments() {
    let e = binding("base = star(points, outer: radius, inner: radius * (1 - depth),)");
    let args = call_args(&e);
    assert_eq!(args.len(), 3);
    assert_eq!(args[0].name, None);
    assert_eq!(args[1].name.as_deref(), Some("outer"));
    assert_eq!(args[2].name.as_deref(), Some("inner"));

    let e = binding("pts = [1, [2, 3], f(\n  4,\n  5\n), \"text\", true, []]");
    let Expr::List { items, .. } = e else {
        panic!("not a list")
    };
    assert_eq!(items.len(), 6);
    assert!(matches!(&items[1], Expr::List { items, .. } if items.len() == 2));
    assert!(matches!(&items[4], Expr::Bool { value: true, .. }));
    assert!(matches!(&items[5], Expr::List { items, .. } if items.is_empty()));

    let e = binding("x = f()");
    assert!(call_args(&e).is_empty());
}

#[test]
fn references_list_every_name_read() {
    let e = binding("x = f(a, b: [c, -d ^ e]) |> g(h + i)");
    let mut refs = Vec::new();
    e.references(&mut refs);
    let names: Vec<_> = refs.into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, vec!["a", "c", "d", "e", "h", "i"]);
}

#[test]
fn outputs() {
    let p = parse(
        "show vase, path\nexport vase as \"vase.stl\"\nprint path with printer\nprint path\n",
    )
    .unwrap();
    assert_eq!(p.outputs.len(), 4);
    match &p.outputs[0] {
        Output::Show { names, .. } => {
            assert_eq!(
                names.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
                vec!["vase", "path"]
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(&p.outputs[1], Output::Export { file, .. } if file == "vase.stl"));
    assert!(matches!(
        &p.outputs[2],
        Output::Print {
            printer: Some(_),
            ..
        }
    ));
    assert!(matches!(&p.outputs[3], Output::Print { printer: None, .. }));
}

#[test]
fn empty_and_comment_only_files_are_fine() {
    assert_eq!(parse("").unwrap(), Program::default());
    assert_eq!(
        parse("\n\n# just a comment\n   \n").unwrap(),
        Program::default()
    );
    assert_eq!(parse("x = 1\r\ny = 2\r\n").unwrap().bindings.len(), 2);
}

#[test]
fn errors_point_at_the_problem() {
    assert_eq!(
        err("x = (1 + 2"),
        (1, 11, "expected `)`, found the end of the file".into())
    );
    assert_eq!(
        err("param h = mm"),
        (
            1,
            11,
            "a setting's value must be a number, true/false, text or a list, found `mm`".into()
        )
    );
    assert_eq!(
        err("wall = loft(s))"),
        (1, 15, "expected the end of the line, found `)`".into())
    );
    assert_eq!(
        err("a = 1\nin = 3"),
        (2, 1, "`in` is a reserved word; pick another name".into())
    );
    assert_eq!(
        err("cube 3"),
        (
            1,
            1,
            "`cube` isn't a statement; to define a step write `cube = …`".into()
        )
    );
    assert_eq!(
        err("param r = 5 in 10..1"),
        (1, 13, "the range 10..1 runs backwards".into())
    );
    assert_eq!(
        err("easycad\nx = 1"),
        (
            1,
            8,
            "expected a version like `easycad 0.1`, found the end of the line".into()
        )
    );
    assert_eq!(
        err("x = f(a: 1, 2)"),
        (
            1,
            13,
            "unnamed values must come before named ones (`name: value`)".into()
        )
    );
    assert_eq!(
        err("x = 1 +"),
        (1, 8, "expected a value, found the end of the file".into())
    );
    assert_eq!(
        err("export v \"f\""),
        (
            1,
            10,
            "expected `as \"file name\"`, found the text \"f\"".into()
        )
    );
    assert_eq!(err("param x = 1 2"), (1, 13, "unexpected the number 2 in this setting (expected `in a..b`, `step s`, `of [...]`, `when …`, a \"Label\" or @attribute)".into()));
    assert_eq!(
        err("show"),
        (
            1,
            5,
            "expected a step name to show, found the end of the file".into()
        )
    );
    assert_eq!(
        err("= 3"),
        (
            1,
            1,
            "expected a step like `name = …` or a setting `param …`, found `=`".into()
        )
    );
    assert_eq!(
        err("x = |> f"),
        (1, 5, "expected a value, found `|>`".into())
    );
}

#[test]
fn hostile_input_is_an_error_not_a_crash() {
    let deep = format!("x = {}1{}", "(".repeat(100_000), ")".repeat(100_000));
    assert!(err(&deep).2.contains("nested too deeply"));
    let negs = format!("x = {}1", "-".repeat(100_000));
    assert!(err(&negs).2.contains("nested too deeply"));
    let sum = format!("x = 1{}", " + 1".repeat(100_000));
    assert!(err(&sum).2.contains("too long"));
    let pipes = format!("x = 1{}", " |> f".repeat(100_000));
    assert!(err(&pipes).2.contains("too long"));
    let powers = format!("x = 2{}", " ^ 2".repeat(100_000));
    assert!(err(&powers).2.contains("nested too deeply"));
    // Long but flat is fine.
    let list = format!("x = [{}]", "1, ".repeat(100_000));
    assert!(parse(&list).is_ok());
    for src in [
        "\"",
        "@",
        "x = \"\\",
        "param",
        "param x",
        "param x =",
        "x = [1, 2",
        "x = f(",
        "é = 1",
        "x = 1..2",
    ] {
        let _ = parse(src); // must return, whatever it returns
    }
}

#[test]
fn set_values_rewrites_only_the_values() {
    let src = r#"easycad 0.1
param height = 160 mm in 60..300 "Height"   # comment stays
param depth = 18 % in 0..40
param lean = -12 deg
param label = "old" "Label"
param flip = false
x = height * 2
"#;
    let values = vec![
        ("height".to_string(), Literal::Number(212.5)),
        ("depth".to_string(), Literal::Number(20.0)),
        ("lean".to_string(), Literal::Number(7.0)),
        ("label".to_string(), Literal::Text("new \"q\"".into())),
        ("flip".to_string(), Literal::Bool(true)),
        ("unknown".to_string(), Literal::Number(1.0)),
    ];
    let out = set_values(src, &values).unwrap();
    assert_eq!(
        out,
        r#"easycad 0.1
param height = 212.5 mm in 60..300 "Height"   # comment stays
param depth = 20 % in 0..40
param lean = 7 deg
param label = "new \"q\"" "Label"
param flip = true
x = height * 2
"#
    );
    let back = parse(&out).unwrap();
    let got: Vec<_> = back
        .params
        .iter()
        .map(|p| (p.name.clone(), p.value.clone()))
        .collect();
    assert_eq!(got, values[..5].to_vec());
    assert_eq!(set_values(src, &[]).unwrap(), src);
    assert!(set_values("x = (", &values).is_err());
}

#[test]
fn list_settings_hold_numbers_or_rows() {
    let src = "param body = [[0, 30], [0.5, -36.5],\n  [1, 28]] \"Body radii\" @assumed\nparam steps = [1, 2, 3]\nparam none = []\nx = 1";
    let p = parse(src).unwrap();
    let row = |a: f64, b: f64| Literal::List(vec![Literal::Number(a), Literal::Number(b)]);
    assert_eq!(
        p.params[0].value,
        Literal::List(vec![row(0.0, 30.0), row(0.5, -36.5), row(1.0, 28.0)])
    );
    assert_eq!(p.params[0].label.as_deref(), Some("Body radii"));
    assert_eq!(p.params[0].attrs, ["assumed"]);
    assert_eq!(p.params[0].unit, Unit::None);
    assert_eq!(
        p.params[1].value,
        Literal::List(vec![
            Literal::Number(1.0),
            Literal::Number(2.0),
            Literal::Number(3.0)
        ])
    );
    assert_eq!(p.params[2].value, Literal::List(vec![]));
    // Written back as typed, on one line; it reads back the same.
    let value = Literal::List(vec![row(0.0, 30.0), row(0.25, 41.5)]);
    assert_eq!(format_literal(&value), "[[0, 30], [0.25, 41.5]]");
    let out = set_values(src, &[("body".into(), value.clone())]).unwrap();
    assert!(out.starts_with("param body = [[0, 30], [0.25, 41.5]] \"Body radii\" @assumed\n"));
    assert_eq!(parse(&out).unwrap().params[0].value, value);
    // What a list setting can't be.
    for (bad, why) in [
        ("param a = [1, [2]]", "not both"),
        ("param a = [[1], []]", "needs a number"),
        ("param a = [[[1]]]", "expected a number"),
        ("param a = [\"x\"]", "numbers or rows of numbers"),
        ("param a = [1, 2] in 0..3", "not `in`"),
        ("param a = [1, 2] of [1]", "not `of`"),
        ("param a = [1, 2] mm", "unexpected `mm`"),
    ] {
        let err = parse(bad).unwrap_err().to_string();
        assert!(err.contains(why), "{bad}: {err}");
    }
}

#[test]
fn numbers_are_written_readably() {
    assert_eq!(format_number(160.0), "160");
    assert_eq!(format_number(0.1 + 0.2), "0.3");
    assert_eq!(format_number(-12.5), "-12.5");
    assert_eq!(format_number(-0.0), "0");
    assert_eq!(format_number(1.0 / 3.0), "0.333333");
    assert_eq!(format_number(-1e-9), "0");
    assert_eq!(format_number(f64::NAN), "0");
    assert_eq!(format_number(f64::INFINITY), "0");
    assert_eq!(format_literal(&Literal::Text("a\\b".into())), "\"a\\\\b\"");
    // A line break is written as \n, so the value stays on its line and reads back the same.
    let out = set_values(
        "param t = \"x\"",
        &[("t".into(), Literal::Text("two\nlines".into()))],
    )
    .unwrap();
    assert_eq!(out, "param t = \"two\\nlines\"");
    assert_eq!(
        parse(&out).unwrap().params[0].value,
        Literal::Text("two\nlines".into())
    );
}

#[test]
fn set_printer_writes_the_printer_line() {
    let src = "easycad 0.1\nproject \"Vase\"\nprinter \"Bambu P1P 0.4\"\nnote \"A note\"\n\nparam h = 10 mm\nshow h\n";
    // An existing line: only its name changes.
    let out = set_printer(src, "Klipper pellet").unwrap();
    assert_eq!(out, src.replace("Bambu P1P 0.4", "Klipper pellet"));
    assert_eq!(
        parse(&out).unwrap().printer.as_deref(),
        Some("Klipper pellet")
    );
    // No line yet: one is added after the title, before the notes.
    let bare = src.replace("printer \"Bambu P1P 0.4\"\n", "");
    let out = set_printer(&bare, "Klipper pellet").unwrap();
    let lines: Vec<&str> = out.lines().take(4).collect();
    assert_eq!(
        lines,
        [
            "easycad 0.1",
            "project \"Vase\"",
            "printer \"Klipper pellet\"",
            "note \"A note\""
        ]
    );
    // No title at all: it goes first.
    let out = set_printer("param h = 10 mm\n", "P").unwrap();
    assert_eq!(out, "printer \"P\"\nparam h = 10 mm\n");
    // Quotes are written so they read back; a name is one line; a broken recipe is refused.
    let out = set_printer(src, "My \"big\" one").unwrap();
    assert_eq!(
        parse(&out).unwrap().printer.as_deref(),
        Some("My \"big\" one")
    );
    assert!(set_printer(src, "two\nlines").is_err());
    assert!(set_printer(src, "  ").is_err());
    assert!(set_printer("x = (", "P").is_err());
}

#[test]
fn settings_can_apply_only_when_another_has_a_value() {
    let src = "param shape = \"cone\" of [\"cone\", \"own\"] \"Shape\"\n\
               param own_r = 50 mm in 5..250 \"Own radius\" when shape = \"own\"\n\
               param sphere = 200 mm when shape = [\"sphere\", \"flat sphere\"]\n\
               param lip = false when not shape = \"cone\"\n\
               param lip_length = 10 mm when lip\n\
               param n = 3 when count = -2\n";
    let p = parse(src).unwrap();
    let w = |i: usize| p.params[i].when.clone();
    assert_eq!(w(0), None);
    let own = w(1).unwrap();
    assert_eq!((own.name.as_str(), own.not), ("shape", false));
    assert_eq!(own.values, [Literal::Text("own".into())]);
    assert_eq!(p.params[1].label.as_deref(), Some("Own radius"));
    assert_eq!(w(2).unwrap().values.len(), 2);
    assert!(w(3).unwrap().not);
    let lip = w(4).unwrap();
    assert!(lip.values.is_empty() && !lip.not);
    assert_eq!(w(5).unwrap().values, [Literal::Number(-2.0)]);
    // Writing a value back leaves the condition as it was.
    let out = set_values(src, &[("own_r".into(), Literal::Number(60.0))]).unwrap();
    assert!(out.contains("param own_r = 60 mm in 5..250 \"Own radius\" when shape = \"own\""));
    for bad in [
        "param a = 1 when",
        "param a = 1 when b =",
        "param a = 1 when b when c",
        "param a = 1 when 3",
    ] {
        assert!(parse(bad).is_err(), "{bad}");
    }
}

// ---- Components and parts (docs/lamp-builder.md)

const RINGS: &str = r#"easycad 0.1
param line = 6 mm in 1..12 "Line width"
base = [20, 0]

component ring "Ring"
  group "Shape"
  param r = 30 mm in 5..100 "Radius"
  param how = "plain" of ["plain", "wavy"] "How"
  param wave = 2 mm "Wave"  when how = "wavy"
  input mount = base "Mounted on"
  size = r + item(mount, 0) + line
  port out = [size + line, 0] "Outside"
  print circle(size)
  show size
end

part a = ring()
part b = ring(mount: a.out, r: 40 mm)
"#;

#[test]
fn parts_expand_into_their_own_names() {
    let p = parse(RINGS).unwrap();
    assert_eq!(p.components.len(), 1);
    assert_eq!(p.parts.len(), 2);
    let e = crate::expand(&p);
    assert!(e.errors.is_empty(), "{:?}", e.errors);
    let names: Vec<&str> = e.program.params.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(
        names,
        ["line", "a.r", "a.how", "a.wave", "b.r", "b.how", "b.wave"]
    );
    let b_r = e.program.params.iter().find(|q| q.name == "b.r").unwrap();
    assert_eq!(b_r.value, Literal::Number(40.0));
    assert_eq!(b_r.part.as_deref(), Some("b"));
    assert_eq!(b_r.group.as_deref(), Some("Shape"));
    // `when` follows the part's own setting.
    let wave = e
        .program
        .params
        .iter()
        .find(|q| q.name == "b.wave")
        .unwrap();
    assert_eq!(wave.when.as_ref().unwrap().name, "b.how");
    // Steps read the part's own names, and shared ones as they are.
    let size = e
        .program
        .bindings
        .iter()
        .find(|x| x.name == "b.size")
        .unwrap();
    let mut refs = Vec::new();
    size.expr.references(&mut refs);
    let refs: Vec<&str> = refs.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(refs, ["b.r", "b.mount", "line"]);
    // Inputs: the default, or the part's own.
    let names: Vec<(&str, &str, bool)> = e
        .inputs
        .iter()
        .map(|i| (i.name.as_str(), i.value.as_str(), i.given))
        .collect();
    assert_eq!(
        names,
        [("a.mount", "base", false), ("b.mount", "a.out", true)]
    );
    assert_eq!(e.inputs[0].group.as_deref(), Some("Shape"));
    let ports: Vec<&str> = e.ports.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(ports, ["a.out", "b.out"]);
    // Each part prints under its own name.
    let prints: Vec<Option<&str>> = e
        .program
        .outputs
        .iter()
        .filter_map(|o| match o {
            Output::Print { part, .. } => Some(part.as_deref()),
            _ => None,
        })
        .collect();
    assert_eq!(prints, [Some("a"), Some("b")]);
}

#[test]
fn parts_explain_what_is_wrong() {
    let errors = |src: &str| -> Vec<String> {
        crate::expand(&parse(src).unwrap())
            .errors
            .into_iter()
            .map(|(_, m)| m)
            .collect()
    };
    let src = RINGS.replace(
        "part b = ring(mount: a.out, r: 40 mm)",
        "part b = ring(size: 3)",
    );
    assert_eq!(errors(&src), ["the component `ring` has no setting `size`"]);
    let src = RINGS.replace(
        "part b = ring(mount: a.out, r: 40 mm)",
        "part b = ring(r: line * 2)",
    );
    assert_eq!(
        errors(&src),
        ["`r` is a setting: give it a value, like `r: 10`"]
    );
    let src = RINGS.replace("part b = ring(mount: a.out, r: 40 mm)", "part b = hoop()");
    assert_eq!(errors(&src), ["there's no component `hoop`"]);
    let src = RINGS.replace("part b = ring(mount: a.out, r: 40 mm)", "part a = ring()");
    assert_eq!(errors(&src), ["`a` is defined twice"]);
    // Statements that can't be inside a component, or outside one.
    assert!(parse("easycad 0.1\ncomponent c\n  part x = c()\nend\n").is_err());
    assert!(parse("easycad 0.1\nport p = 1\n").is_err());
    assert!(parse("easycad 0.1\ncomponent c\n  x = 1\n").is_err());
    assert!(parse("easycad 0.1\na.b = 1\n").is_err());
}

#[test]
fn a_parts_settings_are_written_into_its_own_line() {
    let values = [
        ("b.r".to_string(), Literal::Number(45.0)),
        ("a.how".to_string(), Literal::Text("wavy".into())),
        ("a.r".to_string(), Literal::Number(12.5)),
        ("b.mount".to_string(), Literal::Text("base".into())),
        ("line".to_string(), Literal::Number(5.0)),
    ];
    let out = crate::set_values(RINGS, &values).unwrap();
    assert!(
        out.contains("part a = ring(how: \"wavy\", r: 12.5 mm)\n"),
        "{out}"
    );
    assert!(
        out.contains("part b = ring(mount: base, r: 45 mm)\n"),
        "{out}"
    );
    assert!(out.contains("param line = 5 mm"), "{out}");
    // The component itself is untouched.
    assert!(out.contains("  param r = 30 mm in 5..100 \"Radius\""));
    // Over several lines, with a trailing comma: added after it.
    let multi = RINGS.replace(
        "part b = ring(mount: a.out, r: 40 mm)",
        "part b = ring(\n  mount: a.out,\n  r: 40 mm,\n)",
    );
    let out = crate::set_values(
        &multi,
        &[("b.how".to_string(), Literal::Text("wavy".into()))],
    )
    .unwrap();
    assert!(out.contains("  r: 40 mm, how: \"wavy\"\n)"), "{out}");
    assert!(parse(&out).is_ok());
}

#[test]
fn parts_are_added_and_removed_with_what_is_mounted_on_them() {
    let out = crate::add_part(
        RINGS,
        "ring",
        "c",
        &[
            ("mount".to_string(), Literal::Text("b.out".into())),
            ("r".to_string(), Literal::Number(25.0)),
        ],
    )
    .unwrap();
    assert!(
        out.ends_with(
            "part b = ring(mount: a.out, r: 40 mm)\npart c = ring(mount: b.out, r: 25 mm)\n"
        ),
        "{out}"
    );
    assert!(crate::add_part(RINGS, "ring", "a", &[]).is_err());
    assert!(crate::add_part(RINGS, "ring", "line", &[]).is_err());
    assert!(crate::add_part(RINGS, "hoop", "c", &[]).is_err());
    assert!(
        crate::add_part(
            RINGS,
            "ring",
            "c",
            &[("size".to_string(), Literal::Number(1.0))]
        )
        .is_err()
    );
    assert!(crate::add_part(RINGS, "ring", "c.d", &[]).is_err());
    // Removing `b` takes `c` (mounted on it) along; removing `a` takes them all.
    let gone = crate::remove_part(&out, "b").unwrap();
    assert!(gone.ends_with("part a = ring()\n"), "{gone}");
    let gone = crate::remove_part(&out, "a").unwrap();
    assert!(!gone.contains("part "), "{gone}");
    assert!(crate::remove_part(&out, "z").is_err());
}
