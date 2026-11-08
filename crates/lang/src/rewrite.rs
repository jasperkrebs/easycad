//! Writing new setting values back into recipe text, touching nothing else. This is how
//! "Save project" stores what you set with the sliders.

use crate::ast::{Literal, Unit};
use crate::error::ParseError;
use crate::parser::parse;

/// Returns `src` with the given settings' values replaced. Comments, layout and every other
/// line stay exactly as they were. Unknown names are ignored.
///
/// A part's setting (`tube.shape`) is written into the part's line: its value replaced there,
/// or, when the line doesn't give it yet, added at the end of the line's list. An input's
/// value is a name (`main.end_out`), written as it is.
pub fn set_values(src: &str, values: &[(String, Literal)]) -> Result<String, ParseError> {
    let program = parse(src)?;
    let mut edits: Vec<(usize, usize, String)> = program
        .params
        .iter()
        .filter_map(|p| {
            values
                .iter()
                .find(|(name, _)| *name == p.name)
                .map(|(_, v)| (p.value_range.0, p.value_range.1, format_literal(v)))
        })
        .collect();
    // Parts' settings: replaced in place, or new ones added before each part's `)`.
    for part in &program.parts {
        let Some(c) = program.components.iter().find(|c| c.name == part.component) else {
            continue;
        };
        let mut added: Vec<String> = Vec::new();
        for (name, v) in values {
            let Some(own) = name
                .strip_prefix(part.name.as_str())
                .and_then(|n| n.strip_prefix('.'))
            else {
                continue;
            };
            let text = if let Some(q) = c.body.params.iter().find(|q| q.name == own) {
                with_unit(v, q.unit)
            } else if c.body.inputs.iter().any(|i| i.name == own) {
                match v {
                    Literal::Text(t) if is_name(t) => t.clone(),
                    _ => continue,
                }
            } else {
                continue;
            };
            match part.args.iter().find(|a| a.name == own) {
                Some(a) => edits.push((a.range.0, a.range.1, text)),
                None => added.push(format!("{own}: {text}")),
            }
        }
        if !added.is_empty() {
            let before = src.get(..part.close).unwrap_or_default().trim_end();
            let sep = if before.ends_with('(') || before.ends_with(',') {
                ""
            } else {
                ", "
            };
            let lead = if before.ends_with(',') { " " } else { "" };
            // Inserted right after the last non-space before `)`.
            let at = before.len();
            edits.push((at, at, format!("{sep}{lead}{}", added.join(", "))));
        }
    }
    // Apply from the end so earlier byte offsets stay valid.
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = src.to_string();
    for (start, end, text) in edits {
        if start <= end
            && end <= out.len()
            && out.is_char_boundary(start)
            && out.is_char_boundary(end)
        {
            out.replace_range(start..end, &text);
        }
    }
    Ok(out)
}

/// A setting's value as typed in a part's line, with the setting's unit (`300 mm`, `50 %`).
fn with_unit(v: &Literal, unit: Unit) -> String {
    match v {
        Literal::Number(_) if unit != Unit::None => {
            format!("{} {}", format_literal(v), unit.word())
        }
        _ => format_literal(v),
    }
}

/// A name a recipe can read: letters, digits and `_`, parts joined by dots (`main.end_out`).
fn is_name(t: &str) -> bool {
    !t.is_empty()
        && t.split('.').all(|w| {
            w.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
                && w.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
}

/// Returns `src` with a new part: `part name = component(setting: value, …)`, after the last
/// part (or at the end). Values are written like `set_values` writes them. Fails, with a
/// reason, if the name is taken or not a name, or the component or a setting doesn't exist.
pub fn add_part(
    src: &str,
    component: &str,
    name: &str,
    values: &[(String, Literal)],
) -> Result<String, String> {
    let program = parse(src).map_err(|e| e.message)?;
    if !is_name(name) || name.contains('.') || crate::RESERVED.contains(&name) {
        return Err(format!(
            "`{name}` can't name a part (letters, digits and _)"
        ));
    }
    let taken = program.params.iter().any(|p| p.name == name)
        || program.bindings.iter().any(|b| b.name == name)
        || program.components.iter().any(|c| c.name == name)
        || program.parts.iter().any(|p| p.name == name);
    if taken {
        return Err(format!("there's already something called `{name}`"));
    }
    let c = program
        .components
        .iter()
        .find(|c| c.name == component)
        .ok_or_else(|| format!("there's no component `{component}`"))?;
    let mut args = Vec::new();
    for (k, v) in values {
        if let Some(q) = c.body.params.iter().find(|q| q.name == *k) {
            args.push(format!("{k}: {}", with_unit(v, q.unit)));
        } else if c.body.inputs.iter().any(|i| i.name == *k) {
            match v {
                Literal::Text(t) if is_name(t) => args.push(format!("{k}: {t}")),
                _ => return Err(format!("`{k}` takes a name, like `main.end_out`")),
            }
        } else {
            return Err(format!("the component `{component}` has no setting `{k}`"));
        }
    }
    let line = format!("part {name} = {component}({})\n", args.join(", "));
    let at = program.parts.iter().map(|p| p.range.1).max();
    let out = match at {
        Some(at) => {
            let (before, after) = (
                src.get(..at).unwrap_or_default(),
                src.get(at..).unwrap_or_default(),
            );
            let gap = if before.ends_with('\n') { "" } else { "\n" };
            format!("{before}{gap}{line}{after}")
        }
        None => {
            let gap = if src.is_empty() || src.ends_with("\n\n") {
                ""
            } else if src.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{src}{gap}{line}")
        }
    };
    parse(&out).map_err(|e| format!("that would break the recipe ({})", e.message))?;
    Ok(out)
}

/// Returns `src` without the part `name`, and without every part mounted on it (whose line
/// reads one of its steps, `name.…`), and theirs in turn.
pub fn remove_part(src: &str, name: &str) -> Result<String, String> {
    let program = parse(src).map_err(|e| e.message)?;
    if !program.parts.iter().any(|p| p.name == name) {
        return Err(format!("there's no part `{name}`"));
    }
    let mut gone: Vec<&str> = vec![name];
    loop {
        let more: Vec<&str> = program
            .parts
            .iter()
            .filter(|p| !gone.contains(&p.name.as_str()))
            .filter(|p| {
                p.args.iter().any(|a| match &a.value {
                    crate::ast::PartValue::Expr(e) => {
                        let mut refs = Vec::new();
                        e.references(&mut refs);
                        refs.iter().any(|(r, _)| {
                            gone.iter().any(|g| {
                                r.strip_prefix(g).is_some_and(|rest| rest.starts_with('.'))
                            })
                        })
                    }
                    _ => false,
                })
            })
            .map(|p| p.name.as_str())
            .collect();
        if more.is_empty() {
            break;
        }
        gone.extend(more);
    }
    let mut ranges: Vec<(usize, usize)> = program
        .parts
        .iter()
        .filter(|p| gone.contains(&p.name.as_str()))
        .map(|p| p.range)
        .collect();
    ranges.sort_by_key(|r| std::cmp::Reverse(r.0));
    let mut out = src.to_string();
    for (a, b) in ranges {
        if a <= b && b <= out.len() && out.is_char_boundary(a) && out.is_char_boundary(b) {
            out.replace_range(a..b, "");
        }
    }
    parse(&out).map_err(|e| format!("that would break the recipe ({})", e.message))?;
    Ok(out)
}

/// A literal as it would be typed: `160`, `0.8`, `-12.5`, `true`, `"text"`, `[[0, 30], [1, 28]]`.
pub fn format_literal(value: &Literal) -> String {
    match value {
        Literal::Number(n) => format_number(*n),
        Literal::Bool(b) => b.to_string(),
        Literal::Text(s) => {
            format!(
                "\"{}\"",
                s.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
            )
        }
        Literal::List(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_literal)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Shortest readable form of a number: no trailing zeros, at most 6 decimals.
pub fn format_number(n: f64) -> String {
    if !n.is_finite() {
        return "0".into();
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    let s = format!("{n:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// Returns `src` naming `name` as the project's printer: the name in the `printer "…"` line
/// changes, or, with no such line, one is added after the `easycad` and `project` lines.
/// Everything else stays exactly as it was. Choosing a printer in the app does this, so the
/// project opens with that printer next time.
pub fn set_printer(src: &str, name: &str) -> Result<String, String> {
    use crate::lexer::{Tok, lex};
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.contains(['\n', '\r']) {
        return Err("a printer's name is one line of text (80 characters at most)".into());
    }
    parse(src).map_err(|e| e.message)?;
    let tokens = lex(src).map_err(|e| e.message)?;
    let text = format_literal(&Literal::Text(name.to_string()));
    // Statements start at the first token of a line; header lines are one line each.
    let mut title_end = 0;
    let mut line_start = true;
    let mut existing = None;
    for (i, t) in tokens.iter().enumerate() {
        match &t.tok {
            Tok::Newline => {
                line_start = true;
                continue;
            }
            Tok::Ident(word) if line_start => {
                let line_end = src
                    .get(t.start..)
                    .and_then(|s| s.find('\n'))
                    .map_or(src.len(), |p| t.start + p + 1);
                match word.as_str() {
                    "easycad" | "project" => title_end = title_end.max(line_end),
                    "printer" => {
                        if let Some(n) = tokens.get(i + 1).filter(|n| matches!(n.tok, Tok::Str(_)))
                        {
                            existing = Some((n.start, n.end));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        line_start = false;
    }
    let out = match existing {
        Some((a, b)) => match (src.get(..a), src.get(b..)) {
            (Some(before), Some(after)) => format!("{before}{text}{after}"),
            _ => return Err("internal: the printer line is outside the recipe".into()),
        },
        None => {
            let (before, after) = (
                src.get(..title_end).unwrap_or_default(),
                src.get(title_end..).unwrap_or_default(),
            );
            let gap = if !before.is_empty() && !before.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            format!("{before}{gap}printer {text}\n{after}")
        }
    };
    parse(&out).map_err(|e| format!("that would break the recipe ({})", e.message))?;
    Ok(out)
}
