//! The Nutcracker template (web/templates/nutcracker.easycad): a threaded cap and a key that
//! screws into it, the key flattening above the cap into a broad handle.

use crate::eval::{Evaluation, Quality, evaluate};
use crate::geom::P3;
use easycad_lang::{Literal, set_values};

const TEMPLATE: &str = include_str!("../../../web/templates/nutcracker.easycad");

fn nutcracker(values: &[(&str, f64)]) -> Evaluation {
    let v: Vec<(String, Literal)> = values
        .iter()
        .map(|(k, x)| (k.to_string(), Literal::Number(*x)))
        .collect();
    evaluate(&set_values(TEMPLATE, &v).unwrap(), Quality::Final)
}

fn radius(p: &P3) -> f64 {
    p[0].hypot(p[1])
}

/// The print named `name`.
fn print<'a>(e: &'a Evaluation, name: &str) -> &'a [P3] {
    &e.prints
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no print {name}"))
        .path
        .points
}

#[test]
fn a_cap_and_a_key_as_it_ships() {
    let e = nutcracker(&[]);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    assert_eq!(e.project.as_deref(), Some("Nutcracker"));
    assert_eq!(e.prints.len(), 2);
    for p in &e.prints {
        let g =
            crate::gcode::write(&p.path, &crate::gcode::Profile::default(), "Nutcracker").unwrap();
        assert!(
            g.text.lines().filter(|l| l.starts_with("G1 X")).count() > 1000,
            "{}",
            p.name
        );
    }
    // The key's wall half a line outside the 80 mm thread, the cap's one line further out:
    // the key's widest round turn and the cap's straight wall.
    let (cap, key) = (print(&e, "cap"), print(&e, "key"));
    let key_r = 40.0 + 3.0;
    let cap_wall: Vec<f64> = cap.iter().filter(|p| p[2] > 20.0).map(radius).collect();
    assert!(
        cap_wall.iter().all(|r| (r - (key_r + 6.0)).abs() < 0.05),
        "{:?}",
        cap_wall.iter().fold(0.0, |a: f64, b| a.max(*b))
    );
    // The threaded part (above the fillet, below the flattening, 90 mm) is round.
    let thread: Vec<f64> = key
        .iter()
        .filter(|p| p[2] > 20.0 && p[2] < 85.0)
        .map(radius)
        .collect();
    assert!(thread.len() > 1000 && thread.iter().all(|r| (r - key_r).abs() < 0.05));
}

/// Above the cap it flattens into the handle: as long round as the tube by default (a stadium
/// whose length round its bead's middle is the tube's), two lines thick: one solid blade.
#[test]
fn the_key_flattens_into_a_handle() {
    let e = nutcracker(&[]);
    let key = print(&e, "key");
    let top = key.iter().map(|p| p[2]).fold(0.0, f64::max);
    let last: Vec<&P3> = key.iter().filter(|p| p[2] > top - 2.0).collect();
    let (xs, ys): (Vec<f64>, Vec<f64>) = last.iter().map(|p| (p[1].abs(), p[0].abs())).unzip();
    let (rx, ry) = (
        xs.iter().fold(0.0_f64, |a, b| a.max(*b)),
        ys.iter().fold(0.0_f64, |a, b| a.max(*b)),
    );
    // Turned with its seam to the back (+Y), the handle runs along Y; its thickness along X.
    let key_r = 43.0;
    let want_rx =
        (2.0 * std::f64::consts::PI * key_r - 2.0 * std::f64::consts::PI * 3.0) / 4.0 + 3.0;
    assert!((rx - want_rx).abs() < 0.5, "{rx} {want_rx}");
    assert!((ry - 3.0).abs() < 0.3, "{ry}");
    // A width of its own, and a thicker handle with a hollow inside.
    let e = nutcracker(&[("handle_w", 100.0), ("handle_t", 24.0)]);
    let key = print(&e, "key");
    let top = key.iter().map(|p| p[2]).fold(0.0, f64::max);
    let last: Vec<&P3> = key.iter().filter(|p| p[2] > top - 2.0).collect();
    let rx = last.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
    let ry = last.iter().map(|p| p[0].abs()).fold(0.0, f64::max);
    assert!(
        (rx - 47.0).abs() < 0.5 && (ry - 9.0).abs() < 0.3,
        "{rx} {ry}"
    );
}

#[test]
fn a_key_too_short_for_the_cap_says_so() {
    let e = nutcracker(&[("key_thread", 40.0)]);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.message.contains("shorter than the cap")),
        "{:?}",
        e.diagnostics
    );
}

/// Both threads exactly a pitch a turn (cap and key alike, whatever their fillets): every point
/// of the straight walls is on one true helix. (`contour` with a varying layer fits its steps to
/// the top, which put the threads 1 % off; they're sliced on their own now.)
#[test]
fn both_threads_are_a_pitch_a_turn() {
    let e = nutcracker(&[]);
    for (name, r) in [("cap", 49.0), ("key", 43.0)] {
        let wall: Vec<&P3> = print(&e, name)
            .iter()
            .filter(|p| (radius(p) - r).abs() < 0.01 && p[2] > 25.0 && p[2] < 75.0)
            .collect();
        assert!(wall.len() > 1000, "{name}: {}", wall.len());
        let phase = |p: &P3| {
            let t = p[2] / 2.5 - p[1].atan2(p[0]) / std::f64::consts::TAU;
            t - t.floor()
        };
        let p0 = phase(wall[0]);
        let worst = wall
            .iter()
            .map(|p| {
                let d = (phase(p) - p0).abs();
                d.min(1.0 - d)
            })
            .fold(0.0, f64::max);
        assert!(worst < 1e-3, "{name}: {worst}");
    }
}

/// The cross: as soon as the wall is a layer up from the bottom, four spokes in to the middle
/// and back, flat on the bottom (a layer above it), a quarter turn apart; none when set to 0.
#[test]
fn a_cross_ties_each_bottom_to_its_wall() {
    let e = nutcracker(&[]);
    for name in ["cap", "key"] {
        let pts = print(&e, name);
        // Inside the fillet's foot and above the bottom's spiral (at 2.5): only the spokes.
        let cross: Vec<&P3> = pts
            .iter()
            .filter(|p| radius(p) < 30.0 && p[2] > 3.0 && p[2] < 10.0)
            .collect();
        assert!(cross.len() >= 36, "{name}: {}", cross.len());
        assert!(cross.iter().all(|p| (p[2] - 5.0).abs() < 1e-9), "{name}");
        let nearest = cross
            .iter()
            .map(|p| radius(p))
            .fold(f64::INFINITY, f64::min);
        // Four spokes: as close as keeps neighbours a bead apart (6 / sin 45° from the axis, less
        // the turn's half a line).
        let tip = 6.0 / std::f64::consts::FRAC_PI_4.sin() - 3.0;
        assert!((nearest - tip).abs() < 0.05, "{name}: {nearest}");
        let mut tips: Vec<f64> = cross
            .iter()
            .filter(|p| (radius(p) - nearest).abs() < 0.05)
            .map(|p| p[1].atan2(p[0]).to_degrees())
            .collect();
        tips.sort_by(f64::total_cmp);
        assert_eq!(tips.len(), 4, "{name}: {tips:?}");
        assert!(
            tips.windows(2).all(|w| (w[1] - w[0] - 90.0).abs() < 1.0),
            "{name}: {tips:?}"
        );
    }
    let none = nutcracker(&[("cap_spokes", 0.0)]);
    assert!(
        print(&none, "cap")
            .iter()
            .all(|p| radius(p) > 30.0 || p[2] < 3.0)
    );
}

/// Each part its own number of spokes, and crosses stacked on each other, each a layer up, its
/// spokes over the ones below.
#[test]
fn crosses_stack_and_each_part_has_its_own() {
    let e = nutcracker(&[
        ("cap_spokes", 3.0),
        ("key_spokes", 6.0),
        ("key_crosses", 3.0),
    ]);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    // Their tips: as close to the axis as keeps neighbours a bead apart.
    let tip = |n: f64| 6.0 / (std::f64::consts::PI / n).sin() - 3.0;
    let tips = |name: &str, z: f64| -> Vec<f64> {
        let r = tip(if name == "cap" { 3.0 } else { 6.0 });
        let mut t: Vec<f64> = print(&e, name)
            .iter()
            .filter(|p| (radius(p) - r).abs() < 0.05 && (p[2] - z).abs() < 1e-9)
            .map(|p| p[1].atan2(p[0]).to_degrees().rem_euclid(360.0))
            .collect();
        t.sort_by(f64::total_cmp);
        t
    };
    assert_eq!(tips("cap", 5.0).len(), 3);
    assert!(tips("cap", 7.5).is_empty());
    let low = tips("key", 5.0);
    assert_eq!(low.len(), 6, "{low:?}");
    for z in [7.5, 10.0] {
        let up = tips("key", z);
        assert_eq!(up.len(), 6, "{z}: {up:?}");
        assert!(
            low.iter().zip(&up).all(|(a, b)| (a - b).abs() < 1.0),
            "{z}: {low:?} {up:?}"
        );
    }
    assert!(tips("key", 12.5).is_empty());
}

/// The fillets lean, so their layers come closer (overhang compensation by layer height); off,
/// they're the pitch too.
#[test]
fn the_fillets_are_compensated() {
    let rises = |e: &Evaluation| -> Vec<f64> {
        // The cap's wall where it crosses the back (+Y), turn by turn, up the fillet (above the
        // bottom and its cross, below 15 mm).
        let pts = print(e, "cap");
        let mut z: Vec<f64> = pts
            .windows(2)
            .filter(|w| {
                let r = radius(&w[1]);
                r > 40.0 && w[0][0] > 0.0 && w[1][0] <= 0.0 && w[1][1] > 0.0 && w[1][2] < 15.0
            })
            .map(|w| w[1][2])
            .collect();
        z.dedup_by(|a, b| (*a - *b).abs() < 0.05);
        z.windows(2).map(|w| w[1] - w[0]).collect()
    };
    let on = rises(&nutcracker(&[]));
    assert!(on.iter().any(|r| *r < 1.0), "{on:?}");
    let e = evaluate(
        &set_values(
            TEMPLATE,
            &[("compensation".into(), Literal::Text("off".into()))],
        )
        .unwrap(),
        Quality::Final,
    );
    let off = rises(&e);
    // (Fitted to the fillet's height: four steps of 2 mm.)
    assert!(off.iter().all(|r| *r > 1.9), "{off:?}");
}

/// Found by the settings sweep: a handle thicker than the tube is round (it can't be narrower
/// than it's thick), and a flattening too short for a big tube (lengthened to stay under about
/// 75°, or its layers couldn't stack): both still print, saying what's steep.
#[test]
fn extreme_handles_still_print() {
    for values in [
        vec![("thread_d", 20.0), ("handle_t", 130.0)],
        vec![("thread_d", 131.0), ("flatten", 10.0), ("cap_fillet", 1.0)],
    ] {
        let e = nutcracker(&values);
        assert!(
            e.diagnostics
                .iter()
                .all(|d| d.severity == crate::eval::Severity::Warning),
            "{values:?}: {:?}",
            e.diagnostics
        );
        assert_eq!(e.prints.len(), 2, "{values:?}");
    }
}

/// One spoke runs right across to the opposite wall and back; two meet at the axis, tip to tip.
#[test]
fn one_spoke_runs_across_two_meet_in_the_middle() {
    let flat = |e: &Evaluation| -> Vec<P3> {
        print(e, "cap")
            .iter()
            .filter(|p| (p[2] - 5.0).abs() < 1e-9)
            .copied()
            .collect()
    };
    // The cap's wall at 5 mm (part way up its fillet), from the drawing of it.
    let one = flat(&nutcracker(&[("cap_spokes", 1.0)]));
    // (Its first point flat at the height is a line in from the wall.)
    let wall_r = radius(&one[0]) + 6.0;
    // In at the back (+Y), round a bead off the wall at the front.
    let front = one.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    assert!(
        (front - -(wall_r - 1.5 * 6.0 + 3.0)).abs() < 0.1,
        "{front} {wall_r}"
    );
    let two = flat(&nutcracker(&[("cap_spokes", 2.0)]));
    let nearest = two.iter().map(radius).fold(f64::INFINITY, f64::min);
    assert!((nearest - 3.0).abs() < 0.05, "{nearest}");
}

/// Instead of spokes, a second bottom in one line: in to the middle against the way the wall
/// goes, rings a line apart, round in the middle and back out between them, into the wall; flat
/// on the first bottom, nowhere more than a line from the next ring, from the wall to the
/// middle. Two of them stack.
#[test]
fn a_spiral_ties_a_second_bottom_to_the_wall() {
    let v = [("cap_tie".to_string(), Literal::Text("spiral".into()))];
    let e = evaluate(&set_values(TEMPLATE, &v).unwrap(), Quality::Final);
    assert!(e.diagnostics.is_empty(), "{:?}", e.diagnostics);
    let pts = print(&e, "cap");
    let i0 = pts.iter().position(|p| (p[2] - 5.0).abs() < 1e-9).unwrap();
    let n = pts[i0..]
        .iter()
        .take_while(|p| (p[2] - 5.0).abs() < 1e-9)
        .count();
    let layer = &pts[i0..i0 + n];
    let wall_r = radius(&layer[0]);
    let mut radii: Vec<f64> = layer.iter().map(radius).collect();
    radii.sort_by(f64::total_cmp);
    assert!(radii[0] < 3.5, "{}", radii[0]);
    let gap = radii.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
    assert!(gap <= 6.0 + 1e-6, "{gap}");
    // The way in turns against the wall's way (the wall goes counter-clockwise).
    let turn = |a: &P3, b: &P3| {
        let d = b[1].atan2(b[0]) - a[1].atan2(a[0]);
        (d + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
    };
    let wall_way = turn(&pts[i0 - 2], &pts[i0 - 1]).signum();
    assert!(turn(&layer[2], &layer[10]).signum() == -wall_way);
    // Out at the wall's radius, then on up the wall, the wall's way.
    assert!(
        (radius(&layer[n - 1]) - wall_r).abs() < 0.2,
        "{}",
        radius(&layer[n - 1])
    );
    assert!(pts[i0 + n][2] > 5.0 && turn(&layer[n - 1], &pts[i0 + n + 2]).signum() == wall_way);
    // Two: the second a layer up, on the first.
    let v = [
        ("cap_tie".to_string(), Literal::Text("spiral".into())),
        ("cap_crosses".to_string(), Literal::Number(2.0)),
    ];
    let e = evaluate(&set_values(TEMPLATE, &v).unwrap(), Quality::Final);
    let up = print(&e, "cap")
        .iter()
        .filter(|p| (p[2] - 7.5).abs() < 1e-9 && radius(p) < 10.0)
        .count();
    assert!(up > 10, "{up}");
}
