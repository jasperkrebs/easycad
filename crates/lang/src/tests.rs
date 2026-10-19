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
    assert_eq!(err("param x = 1 2"), (1, 13, "unexpected the number 2 in this setting (expected `in a..b`, `step s`, `of [...]`, a \"Label\" or @attribute)".into()));
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
