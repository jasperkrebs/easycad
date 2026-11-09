//! Every setting of every template, at both ends of its range and through each of its choices:
//! what does the recipe say? Run by hand:
//! `cargo test --release -p easycad-core --test sweep -- --ignored --nocapture`
//! (`SWEEP_RANDOM=n` adds n random combinations per template; `SWEEP_ONLY=lampshade` one template).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use easycad_core::eval::{Evaluation, Quality, Severity};
use easycad_core::files::{self, Which};
use easycad_core::gcode::Profile;
use easycad_core::{ParamInfo, evaluate};
use easycad_lang::{Literal, set_values};
use std::collections::BTreeMap;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn candidates(p: &ParamInfo) -> Vec<Literal> {
    match p.kind {
        "toggle" => vec![Literal::Bool(true), Literal::Bool(false)],
        "choice" => p.choices.clone(),
        "number" => {
            let mut out = Vec::new();
            if let (Some(lo), Some(hi)) = (p.min, p.max) {
                out.push(Literal::Number(lo));
                out.push(Literal::Number(hi));
                out.push(Literal::Number(lo + (hi - lo) * 0.1));
            }
            out
        }
        _ => Vec::new(),
    }
}

fn random_value(p: &ParamInfo, rng: &mut Rng) -> Option<Literal> {
    match p.kind {
        "toggle" => Some(Literal::Bool(rng.next() % 2 == 0)),
        "choice" if !p.choices.is_empty() => {
            Some(p.choices[(rng.next() % p.choices.len() as u64) as usize].clone())
        }
        "number" => {
            let (lo, hi) = (p.min?, p.max?);
            let mut v = match rng.next() % 6 {
                0 => lo,
                1 => hi,
                _ => lo + (hi - lo) * rng.unit(),
            };
            if let Some(step) = p.step.filter(|s| *s > 0.0) {
                v = (lo + ((v - lo) / step).round() * step).clamp(lo, hi);
            }
            Some(Literal::Number(v))
        }
        _ => None,
    }
}

/// What went wrong with a run: the recipe's errors, else what the G-code writer refuses.
fn verdict(e: &Evaluation, profile: &Profile) -> (Vec<String>, Vec<String>) {
    let mut problems: Vec<String> = e
        .errors()
        .map(|d| format!("error line {}: {}", d.line, d.message))
        .collect();
    let warnings: Vec<String> = e
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Error)
        .map(|d| format!("warning line {}: {}", d.line, d.message))
        .collect();
    if problems.is_empty() {
        for i in 0..e.prints.len() {
            if let Err(m) = files::make(e, Which::Print(i), profile) {
                problems.push(format!("gcode: {m}"));
            }
        }
        if e.prints.is_empty() {
            problems.push("prints nothing".into());
        }
    }
    (problems, warnings)
}

/// Numbers out of a message, so the same complaint at different values counts once.
fn shape_of(message: &str) -> String {
    let mut out = String::new();
    let mut in_number = false;
    for c in message.chars() {
        if c.is_ascii_digit() || (in_number && (c == '.' || c == 'e' || c == '-')) {
            if !in_number {
                out.push('#');
            }
            in_number = true;
        } else {
            in_number = false;
            out.push(c);
        }
    }
    out
}

#[test]
#[ignore = "a survey to read, not a check"]
fn every_setting_at_its_ends() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/templates");
    let only = std::env::var("SWEEP_ONLY").ok();
    let random: usize = std::env::var("SWEEP_RANDOM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let profile = Profile {
        center: [500.0, 500.0],
        ..Profile::default()
    };
    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "easycad"))
        .collect();
    names.sort();
    for path in names {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
            continue;
        }
        let mut source = std::fs::read_to_string(&path).unwrap();
        // `SWEEP_SET="shape=dome,opening=316"`: start from these instead of the shipped values.
        if let Ok(set) = std::env::var("SWEEP_SET") {
            let values: Vec<(String, Literal)> = set
                .split(',')
                .filter_map(|pair| pair.split_once('='))
                .map(|(k, v)| {
                    let value = match (v.parse::<f64>(), v) {
                        (Ok(n), _) => Literal::Number(n),
                        (_, "true") => Literal::Bool(true),
                        (_, "false") => Literal::Bool(false),
                        _ => Literal::Text(v.to_string()),
                    };
                    (k.trim().to_string(), value)
                })
                .collect();
            source = set_values(&source, &values).unwrap();
        }
        let base = evaluate(&source, Quality::Final);
        let (problems, warnings) = verdict(&base, &profile);
        println!("\n=== {name}: {} settings", base.params.len());
        for p in problems.iter().chain(&warnings) {
            println!("  as shipped: {p}");
        }
        // complaint → (how often, an example of the settings that cause it)
        let mut seen: BTreeMap<String, (usize, String)> = BTreeMap::new();
        let mut note = |what: &str, problems: Vec<String>, warnings: Vec<String>| {
            for m in problems.into_iter().chain(warnings) {
                let slot = seen
                    .entry(shape_of(&m))
                    .or_insert((0, format!("{what} → {m}")));
                slot.0 += 1;
            }
        };
        let mut runs = 0;
        for p in &base.params {
            for value in candidates(p) {
                if value == p.value {
                    continue;
                }
                let Ok(text) = set_values(&source, &[(p.name.clone(), value.clone())]) else {
                    continue;
                };
                let e = evaluate(&text, Quality::Final);
                let (problems, warnings) = verdict(&e, &profile);
                note(&format!("{} = {:?}", p.name, value), problems, warnings);
                runs += 1;
            }
        }
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..random {
            let values: Vec<(String, Literal)> = base
                .params
                .iter()
                .filter_map(|p| {
                    if rng.next() % 3 != 0 {
                        return None;
                    }
                    Some((p.name.clone(), random_value(p, &mut rng)?))
                })
                .collect();
            let Ok(text) = set_values(&source, &values) else {
                continue;
            };
            let e = evaluate(&text, Quality::Final);
            let (problems, warnings) = verdict(&e, &profile);
            let what = values
                .iter()
                .map(|(k, v)| format!("{k}={v:?}"))
                .collect::<Vec<_>>()
                .join(" ");
            note(&what, problems, warnings);
            runs += 1;
        }
        println!("  {runs} runs, {} kinds of complaint:", seen.len());
        for (count, example) in seen.values() {
            println!("  [{count}×] {example}");
        }
    }
}
