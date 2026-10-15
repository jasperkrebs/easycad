//! Text edits for the graph view. Each one changes exactly one piece of a recipe's text (an
//! argument, a statement, a name) and leaves everything else as it was. The engine's graph
//! commands (`easycad_core::graph`) build on these and parse every result again before using
//! it.

use crate::ast::Unit;
use crate::lexer::{Tok, lex};
use crate::outline::{ArgText, Range, Stage};

/// `src` with `range` replaced by `text`.
pub fn replace(src: &str, range: Range, text: &str) -> Result<String, String> {
    let (a, b) = range;
    match (src.get(..a), src.get(b..)) {
        (Some(before), Some(after)) if a <= b => Ok(format!("{before}{text}{after}")),
        _ => Err("internal: an edit pointed outside the recipe".into()),
    }
}

/// `src` with `text` inserted at byte offset `at`.
pub fn insert(src: &str, at: usize, text: &str) -> Result<String, String> {
    replace(src, (at, at), text)
}

/// `src` with a new line inserted at `at` (the start of a line, or the end of the text).
pub fn insert_line(src: &str, at: usize, line: &str) -> Result<String, String> {
    let needs_break = at > 0 && src.get(..at).is_some_and(|s| !s.ends_with('\n'));
    let text = format!("{}{line}\n", if needs_break { "\n" } else { "" });
    insert(src, at, &text)
}

/// The argument that feeds input number `index` (named `name`) of a stage: the positional
/// argument at that place, or the named one. A piped stage's first input is the stage before
/// it, so its positional arguments start at input 1.
pub fn find_arg<'a>(stage: &'a Stage, index: usize, name: &str) -> Option<&'a ArgText> {
    let offset = usize::from(stage.piped);
    let mut positional = 0;
    for arg in &stage.args {
        match &arg.name {
            None => {
                if positional + offset == index {
                    return Some(arg);
                }
                positional += 1;
            }
            Some(n) if n == name => return Some(arg),
            Some(_) => {}
        }
    }
    None
}

/// Sets input number `index` (named `name`) of a stage to `text`: replaces its argument's
/// value, or adds `name: text`.
pub fn set_arg(
    src: &str,
    stage: &Stage,
    index: usize,
    name: &str,
    text: &str,
) -> Result<String, String> {
    match find_arg(stage, index, name) {
        Some(arg) => replace(src, arg.value, text),
        None => add_arg(src, stage, &format!("{name}: {text}")),
    }
}

/// Adds an argument (`text` is `name: value`) after the stage's last one.
pub fn add_arg(src: &str, stage: &Stage, text: &str) -> Result<String, String> {
    if let Some(last) = stage.args.last() {
        return insert(src, last.full.1, &format!(", {text}"));
    }
    match (stage.close, stage.name_range) {
        (Some(close), _) => insert(src, close, text),
        (None, Some((_, end))) => insert(src, end, &format!("({text})")),
        _ => Err("only a step's inputs can be set here".into()),
    }
}

/// Removes the argument for input number `index`, so the input takes its default (or is
/// missing). Positional arguments after it become named ones, so they keep feeding the same
/// inputs. `inputs` are the stage's input names in order.
pub fn remove_arg(
    src: &str,
    stage: &Stage,
    index: usize,
    inputs: &[&str],
) -> Result<String, String> {
    let name = inputs.get(index).copied().unwrap_or_default();
    let Some(target) = find_arg(stage, index, name) else {
        return Ok(src.to_string());
    };
    let at = stage
        .args
        .iter()
        .position(|a| a == target)
        .unwrap_or_default();
    let later = stage.args.get(at + 1..).unwrap_or_default();
    let shifts = target.name.is_none() && later.iter().any(|a| a.name.is_none());
    if !shifts {
        let range = match (
            at.checked_sub(1).and_then(|p| stage.args.get(p)),
            later.first(),
        ) {
            (Some(prev), _) => (prev.full.1, target.full.1),
            (None, Some(next)) => (target.full.0, next.full.0),
            (None, None) => target.full,
        };
        return replace(src, range, "");
    }
    // Name every argument after it, positional ones by the input they feed.
    let offset = usize::from(stage.piped);
    let before = stage.args.get(..at).unwrap_or_default();
    let mut slot = offset + before.iter().filter(|a| a.name.is_none()).count() + 1;
    let mut parts = Vec::new();
    for arg in later {
        let value = src.get(arg.value.0..arg.value.1).unwrap_or_default();
        match &arg.name {
            Some(n) => parts.push(format!("{n}: {value}")),
            None => {
                let input = inputs
                    .get(slot)
                    .ok_or("there are more values than this step has inputs")?;
                parts.push(format!("{input}: {value}"));
                slot += 1;
            }
        }
    }
    let end = later.last().map_or(target.full.1, |a| a.full.1);
    replace(src, (target.full.0, end), &parts.join(", "))
}

/// Renames a setting or step everywhere: its definition, every reference, `show` lists and
/// outputs. Calls of a step with the same name as a node, argument names, and units are left
/// alone.
pub fn rename(src: &str, from: &str, to: &str) -> Result<String, String> {
    let tokens = lex(src).map_err(|e| e.to_string())?;
    let mut ranges: Vec<Range> = Vec::new();
    for (k, t) in tokens.iter().enumerate() {
        if !matches!(&t.tok, Tok::Ident(n) if n == from) {
            continue;
        }
        let next = tokens.get(k + 1).map(|t| &t.tok);
        let prev = k.checked_sub(1).and_then(|p| tokens.get(p)).map(|t| &t.tok);
        let call_or_label = matches!(next, Some(Tok::LParen | Tok::Colon));
        let after_pipe = matches!(prev, Some(Tok::Pipe));
        let unit = matches!(prev, Some(Tok::Number(_))) && Unit::from_word(from).is_some();
        if !(call_or_label || after_pipe || unit) {
            ranges.push((t.start, t.end));
        }
    }
    let mut out = src.to_string();
    for range in ranges.into_iter().rev() {
        out = replace(&out, range, to)?;
    }
    Ok(out)
}

/// Whether `name` can name a setting or step: a letter or `_`, then letters, digits or `_`.
pub fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !crate::parser::RESERVED.contains(&name)
}
