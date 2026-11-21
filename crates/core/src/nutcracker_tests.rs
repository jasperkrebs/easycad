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
