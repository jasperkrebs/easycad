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

/// A literal as it would be typed: `160`, `0.8`, `-12.5`, `true`, `"text"`.
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
