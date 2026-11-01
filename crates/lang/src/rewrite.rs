//! Writing new setting values back into recipe text, touching nothing else. This is how
//! "Save project" stores what you set with the sliders.

use crate::ast::Literal;
use crate::error::ParseError;
use crate::parser::parse;

/// Returns `src` with the given settings' values replaced. Comments, layout and every other
/// line stay exactly as they were. Unknown names are ignored.
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
