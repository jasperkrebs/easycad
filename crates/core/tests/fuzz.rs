//! "Never crash": recipes nobody would write on purpose, run through everything the app runs.
//!
//! Every template and test recipe is damaged in small ways (numbers swapped for awkward ones,
//! lines dropped, doubled or reordered, steps called with whatever is at hand, stray characters)
//! and then evaluated, simulated and written as G-code and STL. Any outcome is fine except a
//! panic or a run that takes far too long. Seeded, so a failure names a case that fails again.
//!
//! `cargo test -p easycad-core --test fuzz` runs a short round (40 cases). A long one:
//! `FUZZ_CASES=20000 cargo test --release -p easycad-core --test fuzz -- --nocapture`
//! (`FUZZ_SEED=n` starts elsewhere; `FUZZ_ONLY=n` runs one case and prints its recipe).

// A test file: it may unwrap and index (integration tests don't get clippy.toml's test exemption).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use easycad_core::eval::{Evaluation, Quality};
use easycad_core::files::{self, Which};
use easycad_core::gcode::{Mode, Profile};
use easycad_core::{deposit, evaluate, nodes, wall};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

/// A run slower than this counts as a hang (the slowest template takes well under a second).
const TOO_SLOW: Duration = Duration::from_secs(20);

/// xorshift64*: small, seeded, the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

const NUMBERS: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "-1",
    "0.5",
    "-0.5",
    "0.001",
    "0.000000001",
    "7",
    "100",
    "360",
    "1000",
    "100000",
    "1000000000",
    "1e300",
    "-1e300",
    "0.3333333",
    "2.5",
    "99.999",
    "-7",
    "1e-300",
    "65536",
    "4294967296",
];

fn recipes() -> Vec<String> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut out = Vec::new();
    for dir in ["web/templates", "fixtures/recipes"] {
        let mut names: Vec<_> = std::fs::read_dir(format!("{root}/{dir}"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "easycad"))
            .collect();
        names.sort();
        for path in names {
            out.push(std::fs::read_to_string(path).unwrap());
        }
    }
    assert!(out.len() >= 10, "the recipes are missing");
    out
}

/// Where the numbers are in a recipe: (start, end) of each literal outside strings and comments.
fn number_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut in_string) = (0, false);
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\n' {
            in_string = false;
        } else if c == b'"' {
            in_string = !in_string;
        } else if c == b'#' && !in_string {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        } else if !in_string
            && c.is_ascii_digit()
            && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
        {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                // `5..150` is a range, not a number with two dots.
                if bytes[i] == b'.' && bytes.get(i + 1) == Some(&b'.') {
                    break;
                }
                i += 1;
            }
            out.push((start, i));
            continue;
        }
        i += 1;
    }
    out
}

/// The names a recipe binds (settings and steps), for made-up calls to read.
fn names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let l = l.strip_prefix("param ").unwrap_or(l);
            let (name, rest) = l.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()
                && !rest.starts_with('=')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .then(|| name.to_string())
        })
        .collect()
}

fn argument(rng: &mut Rng, names: &[String]) -> String {
    match rng.below(10) {
        0..=3 if !names.is_empty() => rng.pick(names).clone(),
        4 => format!(
            "[{}, {}, {}]",
            rng.pick(NUMBERS),
            rng.pick(NUMBERS),
            rng.pick(NUMBERS)
        ),
        5 => "[]".into(),
        6 => format!("circle({})", rng.pick(NUMBERS)),
        7 => format!(
            "range({}, {}, count: {})",
            rng.pick(NUMBERS),
            rng.pick(NUMBERS),
            rng.pick(&["0", "1", "2", "5", "40", "100000"])
        ),
        8 => (*rng.pick(&["true", "false", "\"x\"", "x", "y", "z"])).to_string(),
        _ => (*rng.pick(NUMBERS)).to_string(),
    }
}

/// A line calling some step with whatever is at hand, shown or printed.
fn made_up_step(rng: &mut Rng, names: &[String], k: usize) -> String {
    let node = rng.pick(nodes::NODES);
    let mut args = Vec::new();
    for input in node.inputs {
        if input.required() || rng.below(3) == 0 {
            let value = argument(rng, names);
            args.push(if rng.below(2) == 0 {
                value
            } else {
                format!("{}: {value}", input.name)
            });
        }
    }
    let verb = *rng.pick(&["show", "show", "print", "export"]);
    let tail = if verb == "export" {
        format!(" as \"fuzz{k}.{}\"", rng.pick(&["stl", "gcode"]))
    } else {
        String::new()
    };
    format!(
        "fuzz{k} = {}({})\n{verb} fuzz{k}{tail}\n",
        node.name,
        args.join(", ")
    )
}

fn damage(rng: &mut Rng, source: &str, case: usize) -> String {
    let mut text = source.to_string();
    for k in 0..1 + rng.below(4) {
        match rng.below(12) {
            // An awkward number in place of one that's there (the most telling damage).
            0..=4 => {
                let spans = number_spans(&text);
                if !spans.is_empty() {
                    let (a, b) = spans[rng.below(spans.len())];
                    text.replace_range(a..b, rng.pick(NUMBERS));
                }
            }
            5 | 6 => {
                let bound = names(&text);
                text.push('\n');
                text.push_str(&made_up_step(rng, &bound, case * 10 + k));
            }
            // A line dropped, doubled, or moved.
            7..=9 => {
                let mut lines: Vec<&str> = text.lines().collect();
                if lines.len() > 3 {
                    let i = 1 + rng.below(lines.len() - 1);
                    match rng.below(3) {
                        0 => {
                            lines.remove(i);
                        }
                        1 => lines.insert(i, lines[i]),
                        _ => {
                            let line = lines.remove(i);
                            lines.insert(1 + rng.below(lines.len() - 1), line);
                        }
                    }
                    text = lines.join("\n") + "\n";
                }
            }
            // A name swapped for another the recipe binds.
            10 => {
                let bound = names(&text);
                if bound.len() > 2 {
                    let (from, to) = (rng.pick(&bound).clone(), rng.pick(&bound).clone());
                    if let Some(at) = text.rfind(&from) {
                        text.replace_range(at..at + from.len(), &to);
                    }
                }
            }
            // A stray character.
            _ => {
                let mut at = rng.below(text.len().max(1));
                while !text.is_char_boundary(at) {
                    at -= 1;
                }
                let stray = *rng.pick(&[
                    "(", ")", "[", "]", "\"", ",", "|>", "-", "^", "/", "é", "\n", ":", "..", "=",
                    "end", "%",
                ]);
                text.insert_str(at, stray);
            }
        }
    }
    text
}

fn profiles() -> Vec<Profile> {
    let roomy = Profile {
        center: [500.0, 500.0],
        ..Profile::default()
    };
    vec![
        roomy.clone(),
        Profile {
            mode: Mode::Filament,
            include_start_end: false,
            ..roomy
        },
    ]
}

/// Everything the app does with a recipe.
fn run(source: &str, profiles: &[Profile]) {
    for quality in [Quality::Preview, Quality::Final] {
        let e: Evaluation = evaluate(source, quality);
        let paths: Vec<_> = e.drawing.paths.iter().map(|p| p.as_ref()).collect();
        for path in &paths {
            let _ = wall::up_the_wall(path);
            let _ = path.stats();
        }
        if quality == Quality::Preview {
            continue;
        }
        let _ = deposit::simulate(&paths, &e.drawing.path_parts, 1.0);
        for profile in profiles {
            for i in 0..e.prints.len().max(1) {
                let _ = files::make(&e, Which::Print(i), profile);
            }
            for i in 0..e.exports.len() {
                let _ = files::make(&e, Which::Export(i), profile);
            }
        }
    }
    let _ = easycad_lang::set_printer(source, "Ginger G1");
    let _ = easycad_lang::remove_part(source, "main");
}

fn env(name: &str) -> Option<usize> {
    std::env::var(name).ok()?.parse().ok()
}

#[test]
fn damaged_recipes_never_crash() {
    let recipes = recipes();
    let profiles = profiles();
    let cases = env("FUZZ_CASES").unwrap_or(40);
    let seed = env("FUZZ_SEED").unwrap_or(1);
    let only = env("FUZZ_ONLY");
    let quiet = std::panic::take_hook();
    // Quiet, but remembering where: the message alone ("attempt to add with overflow") doesn't say.
    static WHERE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    std::panic::set_hook(Box::new(|info| {
        if let (Some(at), Ok(mut slot)) = (info.location(), WHERE.lock()) {
            *slot = format!("{}:{}", at.file(), at.line());
        }
    }));
    let mut failures = Vec::new();
    for case in only.map_or(0..cases, |n| n..n + 1) {
        let mut rng = Rng((seed as u64) << 32 | (case as u64 + 1));
        for _ in 0..4 {
            rng.next();
        }
        let source = damage(&mut rng, rng_recipe(&recipes, case), case);
        if only.is_some() {
            eprintln!("{source}");
        }
        let started = Instant::now();
        let outcome = catch_unwind(AssertUnwindSafe(|| run(&source, &profiles)));
        let took = started.elapsed();
        if let Err(panic) = outcome {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            let at = WHERE.lock().map(|s| s.clone()).unwrap_or_default();
            failures.push(format!(
                "case {case} (seed {seed}) panicked at {at}: {message}"
            ));
        } else if took > TOO_SLOW {
            failures.push(format!("case {case} (seed {seed}) took {took:?}"));
        }
    }
    std::panic::set_hook(quiet);
    assert!(
        failures.is_empty(),
        "{} of {cases} damaged recipes went wrong (FUZZ_ONLY=n shows one):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn rng_recipe(recipes: &[String], case: usize) -> &str {
    &recipes[case % recipes.len()]
}
