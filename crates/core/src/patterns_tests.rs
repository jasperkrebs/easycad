//! Patterns on the Lamp's and the Lampshade's sections (docs/patterns.md).

use crate::eval::{Evaluation, Quality, evaluate};
use crate::geom::P3;
use easycad_lang::{Literal, set_values};

const LAMPSHADE: &str = include_str!("../../../web/templates/lampshade.easycad");
const LAMP: &str = include_str!("../../../web/templates/lamp.easycad");

fn lampshade(values: &[(&str, Literal)]) -> Evaluation {
    let v: Vec<(String, Literal)> = values
        .iter()
        .map(|(k, x)| (k.to_string(), x.clone()))
        .collect();
    evaluate(&set_values(LAMPSHADE, &v).unwrap(), Quality::Final)
}

fn radius(p: &P3) -> f64 {
    p[0].hypot(p[1])
}

/// How far each point is pushed in or out from the plain print (its radius at that height), the
/// most in each band of height `band`. (The plain print's points are on the wall, the waved
/// print's between them on its chords, up to about 0.1 mm inside: so a wave of 2 mm reads 2 to
/// 2.1.)
fn depth(pts: &[P3], plain: &[P3], band: f64) -> Vec<(f64, f64)> {
    let r_at = |z: f64| {
        let i = plain
            .partition_point(|p| p[2] < z)
            .clamp(1, plain.len() - 1);
        let (a, b) = (plain[i - 1], plain[i]);
        let t = if b[2] > a[2] {
            (z - a[2]) / (b[2] - a[2])
        } else {
            0.0
        };
        radius(&a) + (radius(&b) - radius(&a)) * t.clamp(0.0, 1.0)
    };
    let top = pts.iter().map(|p| p[2]).fold(0.0, f64::max);
    let mut most = vec![0.0_f64; (top / band) as usize + 1];
    for p in pts {
        let i = (p[2] / band) as usize;
        most[i] = most[i].max((radius(p) - r_at(p[2])).abs());
    }
    most.into_iter()
        .enumerate()
        .map(|(i, d)| (i as f64 * band, d))
        .collect()
}

/// How well the push in and out at each point (from `lo` to `hi` high) follows one fixed wave
/// of `n` round the axis, a·sin(nθ) + b·cos(nθ): what's left over, for each unit of the wave's size.
fn off_rails(pts: &[P3], plain: &[P3], n: f64, lo: f64, hi: f64) -> f64 {
    let r_at = |z: f64| {
        let i = plain
            .partition_point(|p| p[2] < z)
            .clamp(1, plain.len() - 1);
        let (a, b) = (plain[i - 1], plain[i]);
        let t = if b[2] > a[2] {
            (z - a[2]) / (b[2] - a[2])
        } else {
            0.0
        };
        radius(&a) + (radius(&b) - radius(&a)) * t.clamp(0.0, 1.0)
    };
    let mut rows = vec![];
    for p in pts.iter().filter(|p| p[2] > lo && p[2] < hi) {
        let th = p[1].atan2(p[0]);
        rows.push(((n * th).sin(), (n * th).cos(), radius(p) - r_at(p[2])));
    }
    let (mut ss, mut sc, mut cc, mut sd, mut cd) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (s, c, d) in &rows {
        ss += s * s;
        sc += s * c;
        cc += c * c;
        sd += s * d;
        cd += c * d;
    }
    let det = ss * cc - sc * sc;
    let a = (sd * cc - cd * sc) / det;
    let b = (cd * ss - sd * sc) / det;
    let size = a.hypot(b).max(1e-9);
    let left = rows
        .iter()
        .map(|(s, c, d)| (d - a * s - b * c).powi(2))
        .sum::<f64>()
        / rows.len() as f64;
    left.sqrt() / size.max(0.1)
}

/// As it ships, nothing is patterned: a depth and a count set, but no section picked, change
/// nothing.
#[test]
fn nothing_is_patterned_until_a_section_is_picked() {
    use Literal::Number;
    let plain = lampshade(&[]);
    let set = lampshade(&[
        ("pattern_depth", Number(5.0)),
        ("pattern_waves", Number(7.0)),
    ]);
    assert_eq!(plain.prints[0].path.points, set.prints[0].path.points);
    assert!(
        plain
            .marks
            .iter()
            .all(|m| m.label.as_deref() != Some("Pattern"))
    );
}

/// On the shade alone (the cone, from about 25 mm, where the fillet ends, to the opening at
/// 75 mm): in and out by the depth, fading in and out over 20 mm along the surface; the throat
/// untouched, so it still screws on.
#[test]
fn a_pattern_on_the_shade_fades_in_and_out() {
    use Literal::Bool;
    let plain = lampshade(&[]);
    let e = lampshade(&[("pattern_shade", Bool(true))]);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    let d = depth(&e.prints[0].path.points, &plain.prints[0].path.points, 2.5);
    let at = |z: f64| d.iter().find(|(lo, _)| (lo - z).abs() < 1e-9).unwrap().1;
    for z in [0.0, 5.0, 10.0, 15.0, 20.0] {
        assert!(at(z) < 1e-9, "{z}: {d:?}");
    }
    for z in [40.0, 45.0, 50.0, 55.0, 60.0] {
        assert!(at(z) > 1.99 && at(z) < 2.12, "{z}: {d:?}");
    }
    // Fading: in between, part of the way, growing in and shrinking out.
    assert!(
        at(30.0) > 0.2 && at(30.0) < 1.5 && at(32.5) > at(30.0),
        "{d:?}"
    );
    assert!(
        at(70.0) > 0.2 && at(70.0) < 1.5 && at(72.5) < at(70.0),
        "{d:?}"
    );
    // Its mark, beside the shade, leads to its settings.
    let mark = e
        .marks
        .iter()
        .find(|m| m.label.as_deref() == Some("Pattern"));
    assert_eq!(
        mark.unwrap().settings,
        [
            "pattern_waves",
            "pattern_stack",
            "pattern_depth",
            "pattern_fade_in",
            "pattern_fade_out"
        ]
    );
}

/// Picked sections next to each other are one run: the wave fades in where the run starts and
/// out where it ends, not at the sections' joins. On the throat it takes the thread: it says so.
#[test]
fn picked_sections_run_into_each_other() {
    use Literal::{Bool, Number};
    let plain = lampshade(&[]);
    let e = lampshade(&[
        ("pattern_start", Bool(true)),
        ("pattern_fillet", Bool(true)),
        ("pattern_shade", Bool(true)),
        ("pattern_fade_in", Number(5.0)),
        ("pattern_fade_out", Number(5.0)),
    ]);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.message.contains("won't screw on")),
        "{:?}",
        e.diagnostics
    );
    let d = depth(&e.prints[0].path.points, &plain.prints[0].path.points, 2.5);
    assert!(
        d.iter()
            .filter(|(z, _)| *z >= 7.5 && *z <= 65.0)
            .all(|(_, x)| *x > 1.99 && *x < 2.12),
        "{d:?}"
    );
    assert!(d[0].1 < 2.0 && d.last().unwrap().1 < 1.0, "{d:?}");
    // The throat and the shade, not the fillet between: two runs, faded out and in again there.
    let two = lampshade(&[
        ("pattern_start", Bool(true)),
        ("pattern_shade", Bool(true)),
        ("pattern_fade_in", Number(5.0)),
        ("pattern_fade_out", Number(5.0)),
    ]);
    let d = depth(
        &two.prints[0].path.points,
        &plain.prints[0].path.points,
        2.5,
    );
    assert!(
        d.iter().any(|(z, x)| *z > 15.0 && *z < 30.0 && *x < 0.05),
        "{d:?}"
    );
}

/// The fade in and the fade out are set on their own: in over 20 mm and none out, the shade's
/// pattern grows in above the fillet and runs at its full depth right to the opening; the other
/// way round, it starts at its full depth and fades out.
#[test]
fn fade_in_and_out_are_set_on_their_own() {
    use Literal::{Bool, Number};
    let plain = lampshade(&[]);
    let at = |fade_in: f64, fade_out: f64| {
        let e = lampshade(&[
            ("pattern_shade", Bool(true)),
            ("pattern_fade_in", Number(fade_in)),
            ("pattern_fade_out", Number(fade_out)),
        ]);
        depth(&e.prints[0].path.points, &plain.prints[0].path.points, 2.5)
    };
    let band = |d: &[(f64, f64)], z: f64| d.iter().find(|(lo, _)| (lo - z).abs() < 1e-9).unwrap().1;
    // (The cone's shade from about 25 mm to its opening at 75.)
    let only_in = at(20.0, 0.0);
    assert!(band(&only_in, 30.0) < 1.5, "{only_in:?}");
    assert!(band(&only_in, 72.5) > 1.99, "{only_in:?}");
    let only_out = at(0.0, 20.0);
    assert!(band(&only_out, 27.5) > 1.99, "{only_out:?}");
    assert!(band(&only_out, 70.0) < 1.5, "{only_out:?}");
}

/// Whole waves a turn stack into rails: every layer's wave where the one below it is. Woven, each
/// layer's is shifted half a wave.
#[test]
fn rails_stack_and_woven_shifts_half_a_wave() {
    use Literal::{Bool, Number, Text};
    let plain = lampshade(&[]);
    let base = [
        ("pattern_shade", Bool(true)),
        ("pattern_waves", Number(12.0)),
    ];
    let rails = lampshade(&base);
    let woven = lampshade(&[
        base[0].clone(),
        base[1].clone(),
        ("pattern_stack", Text("woven".into())),
    ]);
    let p = &plain.prints[0].path.points;
    let r = off_rails(&rails.prints[0].path.points, p, 12.0, 40.0, 60.0);
    let w = off_rails(&woven.prints[0].path.points, p, 12.0, 40.0, 60.0);
    assert!(r < 0.05, "rails: {r}");
    assert!(w > 0.7, "woven: {w}");
}

/// The Lamp's shades take patterns the same way, each its own: the sphere as it ships, patterned
/// from its fillet to its straight end's fillet, keeps its start's thread and its end's; on the
/// straight end, it says nothing screws on there any more. Each section's mark leads to its
/// pattern switch.
#[test]
fn a_lamp_shade_takes_a_pattern_too() {
    use Literal::Bool;
    let set = |values: &[(&str, Literal)]| {
        let v: Vec<(String, Literal)> = values
            .iter()
            .map(|(k, x)| (format!("main.{k}"), x.clone()))
            .collect();
        evaluate(&set_values(LAMP, &v).unwrap(), Quality::Final)
    };
    let plain = set(&[]);
    let mark = |e: &Evaluation, label: &str| {
        e.marks
            .iter()
            .find(|m| m.label.as_deref() == Some(label))
            .map(|m| m.settings.clone())
    };
    assert_eq!(
        mark(&plain, "Fillet").unwrap(),
        ["main.fillet", "main.pattern_fillet"]
    );
    assert!(
        mark(&plain, "Straight end")
            .unwrap()
            .contains(&"main.pattern_end".to_string())
    );
    assert!(mark(&plain, "Pattern").is_none());
    let e = set(&[
        ("pattern_fillet", Bool(true)),
        ("pattern_shade", Bool(true)),
        ("pattern_end_fillet", Bool(true)),
    ]);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    assert!(mark(&e, "Pattern").is_some());
    let d = depth(&e.prints[0].path.points, &plain.prints[0].path.points, 5.0);
    let most = d.iter().map(|(_, x)| *x).fold(0.0, f64::max);
    assert!(most > 1.99 && most < 2.12, "{d:?}");
    // Both straight sections untouched: the first 20 mm and the last.
    let top = d.last().unwrap().0;
    assert!(
        d.iter()
            .filter(|(z, _)| *z < 15.0 || *z > top - 15.0)
            .all(|(_, x)| *x < 1e-9),
        "{d:?}"
    );
    let end = set(&[("pattern_end", Bool(true))]);
    assert!(
        end.diagnostics
            .iter()
            .any(|d| d.message.contains("nothing screws on there")),
        "{:?}",
        end.diagnostics
    );
}
