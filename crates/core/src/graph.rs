//! The graph view: the recipe as components and wires, and the commands that edit it.
//!
//! Every edit made in the graph (an input typed or wired, a component added, removed or
//! renamed, a preview switched on, a slider added) is one of these commands, and each one is an
//! edit of the recipe text: the definition stays the single source of truth. A command touches
//! only its own piece of the text (comments and layout elsewhere stay as they were), and its
//! result is parsed again before it's returned, so the graph can't break a recipe.

use crate::nodes::{self, Access, Input, NODES, Node};
use easycad_lang::edit;
use easycad_lang::outline::{Lines, Outline, OutputKind, Stage, StepLine, outline};
use easycad_lang::{RESERVED, format_number, parse};
use serde::{Deserialize, Serialize};

/// Names that are already taken by the language itself.
const BUILTINS: [&str; 4] = ["pi", "x", "y", "z"];

/// The recipe as the graph view draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub params: Vec<GraphParam>,
    pub steps: Vec<GraphStep>,
    pub outputs: Vec<GraphOutput>,
    /// The steps named in `show` statements.
    pub shown: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphParam {
    pub name: String,
    pub line: u32,
}

/// One component: a named step, with its pipe stages in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStep {
    pub name: String,
    pub line: u32,
    pub end_line: u32,
    /// The expression as written.
    pub text: String,
    pub stages: Vec<GraphStage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStage {
    /// The step it calls; `None` for a plain expression (`a * (1 + b)`).
    pub node: Option<String>,
    /// Whether the engine has a step by that name.
    pub known: bool,
    pub group: Option<&'static str>,
    pub output: Option<&'static str>,
    pub doc: Option<&'static str>,
    pub text: String,
    /// Fed by the stage before it (its first input).
    pub piped: bool,
    pub inputs: Vec<GraphInput>,
    /// The names a plain expression reads.
    pub refs: Vec<String>,
}

/// One input of a stage, with what's written for it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphInput {
    pub name: String,
    pub kind: String,
    pub required: bool,
    /// The default as text (`1`, `[0, 0, 1]`, `false`).
    pub default: Option<String>,
    pub doc: String,
    /// The value as written, if one is.
    pub text: Option<String>,
    /// The names that value reads: settings and steps (wires), or built-ins like `x`.
    pub refs: Vec<String>,
    /// Fed by the stage before it.
    pub piped: bool,
    /// Takes a whole list.
    pub list: bool,
    /// Written, but not one of the step's inputs (a misspelt name, or one value too many).
    pub unknown: bool,
}

/// A `print` or `export` statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphOutput {
    pub kind: OutputKind,
    pub line: u32,
    pub text: String,
    pub refs: Vec<String>,
    pub file: Option<String>,
}

/// A step the palette offers.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteNode {
    pub name: &'static str,
    pub group: &'static str,
    pub output: &'static str,
    pub doc: &'static str,
    pub inputs: Vec<GraphInput>,
}

/// An edit made in the graph view.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Command {
    /// Sets one input of a stage to an expression (a name wires it); empty text removes it.
    SetInput {
        step: String,
        stage: usize,
        input: String,
        text: String,
    },
    /// Replaces a stage that is a plain expression (`a * (1 + b)`), or with `stage` 0 and a
    /// one-stage step, the whole expression.
    SetExpr {
        step: String,
        stage: usize,
        text: String,
    },
    /// Adds a component calling `node`. With `from`, that step or setting feeds `input` (by
    /// default the first required one) and the new step goes right after it.
    AddStep {
        node: String,
        from: Option<String>,
        input: Option<String>,
    },
    /// Appends `|> node(…)` to a step.
    AddStage { step: String, node: String },
    /// Removes stage `stage` (1 or more) of a step.
    RemoveStage { step: String, stage: usize },
    /// Removes a step or setting. Inputs wired only to it are emptied, and it leaves `show`.
    Remove { name: String },
    /// Renames a step or setting everywhere.
    Rename { name: String, to: String },
    /// Shows a step in the 3D view, or stops showing it.
    SetShown { step: String, shown: bool },
    /// Adds a number slider.
    AddParam { value: f64, min: f64, max: f64 },
    /// Changes a number slider: value, range, step and label.
    SetParam {
        name: String,
        value: f64,
        min: f64,
        max: f64,
        step: Option<f64>,
        label: Option<String>,
    },
    /// Names the printer the project is for: the `printer "…"` line, added after the title if
    /// the recipe has none. The app opens the project with that printer.
    SetPrinter { name: String },
}

/// The recipe after a command, and the name of anything it added.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Applied {
    pub source: String,
    pub name: Option<String>,
}

/// The graph of a recipe. A recipe with a syntax error has none (the message says why).
pub fn describe(src: &str) -> Result<Graph, String> {
    let o = outline(src).map_err(|e| e.to_string())?;
    let text = |r: (usize, usize)| src.get(r.0..r.1).unwrap_or_default().to_string();
    Ok(Graph {
        params: o
            .params
            .iter()
            .map(|p| GraphParam {
                name: p.name.clone(),
                line: p.lines.line,
            })
            .collect(),
        steps: o
            .steps
            .iter()
            .map(|s| GraphStep {
                name: s.name.clone(),
                line: s.lines.line,
                end_line: s.lines.end_line,
                text: text(s.expr),
                stages: s.stages.iter().map(|st| stage_info(src, st)).collect(),
            })
            .collect(),
        outputs: o
            .outputs
            .iter()
            .filter(|x| x.kind != OutputKind::Show)
            .map(|x| GraphOutput {
                kind: x.kind,
                line: x.lines.line,
                text: x.expr.map(text).unwrap_or_default(),
                refs: x.refs.clone(),
                file: x.file.clone(),
            })
            .collect(),
        shown: o
            .outputs
            .iter()
            .flat_map(|x| x.names.iter().map(|n| n.0.clone()))
            .collect(),
    })
}

/// Every step a recipe can use, for the palette.
pub fn palette() -> Vec<PaletteNode> {
    NODES
        .iter()
        .map(|n| PaletteNode {
            name: n.name,
            group: n.group,
            output: n.output,
            doc: n.doc,
            inputs: n
                .inputs
                .iter()
                .map(|i| input_info(i, None, false, false))
                .collect(),
        })
        .collect()
}

fn input_info(
    i: &Input,
    arg: Option<(String, Vec<String>)>,
    piped: bool,
    unknown: bool,
) -> GraphInput {
    let (text, refs) = arg.map_or((None, Vec::new()), |(t, r)| (Some(t), r));
    GraphInput {
        name: i.name.to_string(),
        kind: i.kind.to_string(),
        required: i.required(),
        default: default_text(i),
        doc: i.doc.to_string(),
        text,
        refs,
        piped,
        list: i.access == Access::List || i.access == Access::Tree,
        unknown,
    }
}

/// An input's default as it would be written.
fn default_text(i: &Input) -> Option<String> {
    match (i.point, i.default) {
        (Some(p), _) => Some(format!(
            "[{}, {}, {}]",
            format_number(p[0]),
            format_number(p[1]),
            format_number(p[2])
        )),
        (None, Some(d)) if i.kind == "Bool" => Some((d != 0.0).to_string()),
        (None, Some(d)) => Some(format_number(d)),
        (None, None) => None,
    }
}

fn stage_info(src: &str, st: &Stage) -> GraphStage {
    let text = |r: (usize, usize)| src.get(r.0..r.1).unwrap_or_default().to_string();
    let node = st.call.as_deref().and_then(nodes::find);
    let mut inputs: Vec<GraphInput> = Vec::new();
    if let Some(node) = node {
        for (k, input) in node.inputs.iter().enumerate() {
            let arg = edit::find_arg(st, k, input.name).map(|a| (text(a.value), a.refs.clone()));
            inputs.push(input_info(input, arg, st.piped && k == 0, false));
        }
    }
    // Values that don't belong to any input are shown too, so nothing written is hidden.
    let offset = usize::from(st.piped);
    let mut positional = 0;
    for (k, arg) in st.args.iter().enumerate() {
        let known = match (&arg.name, node) {
            (None, Some(node)) => {
                positional += 1;
                positional + offset <= node.inputs.len()
            }
            (Some(name), Some(node)) => node.inputs.iter().any(|i| i.name == name),
            (_, None) => false,
        };
        if !known {
            inputs.push(GraphInput {
                name: arg
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("value {}", k + 1)),
                kind: "?".into(),
                required: false,
                default: None,
                doc: String::new(),
                text: Some(text(arg.value)),
                refs: arg.refs.clone(),
                piped: false,
                list: false,
                unknown: true,
            });
        }
    }
    GraphStage {
        node: st.call.clone(),
        known: node.is_some(),
        group: node.map(|n| n.group),
        output: node.map(|n| n.output),
        doc: node.map(|n| n.doc),
        text: text(st.range),
        piped: st.piped,
        inputs,
        refs: st.refs.clone(),
    }
}

/// Runs a graph command on a recipe.
pub fn apply(src: &str, cmd: &Command) -> Result<Applied, String> {
    let o = outline(src).map_err(|e| {
        format!(
            "the recipe has a problem the graph can't edit around ({e}); fix it in the Recipe tab"
        )
    })?;
    let mut name = None;
    let out = match cmd {
        Command::SetInput {
            step,
            stage,
            input,
            text,
        } => set_input(src, &o, step, *stage, input, text)?,
        Command::SetExpr { step, stage, text } => {
            let s = find_step(&o, step)?;
            let st = s
                .stages
                .get(*stage)
                .ok_or_else(|| format!("`{step}` has no stage {}", stage + 1))?;
            let text = one_line(text);
            if text.is_empty() {
                return Err("an expression can't be empty; remove the component instead".into());
            }
            single_value(&text)?;
            let range = if s.stages.len() == 1 {
                s.expr
            } else {
                st.range
            };
            if st.call.is_some() && s.stages.len() > 1 {
                return Err("this stage is a step; set its inputs instead".into());
            }
            edit::replace(src, range, &text)?
        }
        Command::AddStep { node, from, input } => {
            let (out, added) = add_step(src, &o, node, from.as_deref(), input.as_deref())?;
            name = Some(added);
            out
        }
        Command::AddStage { step, node } => {
            let s = find_step(&o, step)?;
            let node = find_node(node)?;
            let args = new_args(node, Some(0), None);
            let call = if args.is_empty() {
                node.name.to_string()
            } else {
                format!("{}({args})", node.name)
            };
            edit::insert(src, s.expr.1, &format!(" |> {call}"))?
        }
        Command::RemoveStage { step, stage } => {
            let s = find_step(&o, step)?;
            let (Some(before), Some(this)) = (
                stage.checked_sub(1).and_then(|k| s.stages.get(k)),
                s.stages.get(*stage),
            ) else {
                return Err("only a stage after the first can be removed; remove the whole component instead".into());
            };
            edit::replace(src, (before.range.1, this.range.1), "")?
        }
        Command::Remove { name } => remove(src, name)?,
        Command::Rename { name, to } => {
            if o.step(name).is_none() && o.param(name).is_none() {
                return Err(format!("there's no `{name}` to rename"));
            }
            check_new_name(&o, to)?;
            edit::rename(src, name, to)?
        }
        Command::SetShown { step, shown } => set_shown(src, &o, step, *shown)?,
        Command::AddParam { value, min, max } => {
            let (lo, hi) = (min.min(*max), min.max(*max));
            if ![*value, lo, hi].iter().all(|v| v.is_finite()) {
                return Err("the slider needs real numbers".into());
            }
            let added = fresh_name(&o, "slider");
            let k = added.trim_start_matches("slider");
            let line = format!(
                "param {added} = {} in {}..{} \"Slider {k}\"",
                format_number(value.clamp(lo, hi)),
                format_number(lo),
                format_number(hi)
            );
            name = Some(added);
            edit::insert_line(src, param_slot(&o), &line)?
        }
        Command::SetParam { .. } => set_param(src, &o, cmd)?,
        Command::SetPrinter { name } => {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 80 || name.contains(['\n', '\r']) {
                return Err("a printer's name is one line of text (80 characters at most)".into());
            }
            let text = easycad_lang::format_literal(&easycad_lang::Literal::Text(name.to_string()));
            match o.printer {
                Some(range) => edit::replace(src, range, &text)?,
                None => edit::insert_line(src, o.title_end, &format!("printer {text}"))?,
            }
        }
    };
    parse(&out).map_err(|e| format!("that would break the recipe ({})", e.message))?;
    Ok(Applied { source: out, name })
}

fn find_step<'a>(o: &'a Outline, name: &str) -> Result<&'a StepLine, String> {
    o.step(name)
        .ok_or_else(|| format!("there's no step `{name}`"))
}

fn find_node(name: &str) -> Result<&'static Node, String> {
    nodes::find(name).ok_or_else(|| format!("`{name}` isn't a step the engine knows"))
}

/// Text typed into one field: one line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Checks that `text` is exactly one value (so an input edit can't reach into the rest of the
/// line).
fn single_value(text: &str) -> Result<(), String> {
    let probe = format!("probe = f({text})\n");
    let o =
        outline(&probe).map_err(|e| format!("`{text}` isn't a complete value: {}", e.message))?;
    let args = o
        .steps
        .first()
        .filter(|s| s.stages.len() == 1)
        .and_then(|s| s.stages.first())
        .map(|st| st.args.len());
    match args {
        Some(1) => Ok(()),
        _ => Err(format!("`{text}` isn't one value")),
    }
}

fn set_input(
    src: &str,
    o: &Outline,
    step: &str,
    stage: usize,
    input: &str,
    text: &str,
) -> Result<String, String> {
    let s = find_step(o, step)?;
    let st = s
        .stages
        .get(stage)
        .ok_or_else(|| format!("`{step}` has no stage {}", stage + 1))?;
    let call = st
        .call
        .as_deref()
        .ok_or("this part is an expression; edit it as a whole")?;
    let node = find_node(call)?;
    let index = node
        .inputs
        .iter()
        .position(|i| i.name == input)
        .ok_or_else(|| format!("`{call}` has no input `{input}`"))?;
    if st.piped && index == 0 {
        return Err(format!(
            "`{input}` comes from the stage before (the `|>`); change that stage instead"
        ));
    }
    let names: Vec<&str> = node.inputs.iter().map(|i| i.name).collect();
    let text = one_line(text);
    if text.is_empty() {
        return edit::remove_arg(src, st, index, &names);
    }
    single_value(&text)?;
    edit::set_arg(src, st, index, input, &text)
}

/// A value to start a new component's required input with, when the kind has an obvious one.
fn placeholder(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Length" | "Length | Field" => "10",
        "Count" => "4",
        "Number" | "Number | Field" => "1",
        "Angle" | "Angle | Field" => "0",
        "Speed" => "40",
        "Bool" => "false",
        "Point" | "Vector" => "[0, 0, 0]",
        _ => return None,
    })
}

/// The argument list for a new call of `node`: the wired input gets `from`, other required
/// inputs a placeholder where there's an obvious one (the rest stay missing until wired, like
/// a Grasshopper component waiting for input). `skip` is an input fed by a pipe.
fn new_args(node: &Node, skip: Option<usize>, wire: Option<(usize, &str)>) -> String {
    let mut parts = Vec::new();
    let mut positional = true;
    for (k, input) in node.inputs.iter().enumerate() {
        if Some(k) == skip {
            continue;
        }
        let value = match wire {
            Some((w, from)) if w == k => Some(from.to_string()),
            _ if input.required() => placeholder(input.kind).map(str::to_string),
            _ => None,
        };
        match value {
            Some(v) if positional => parts.push(v),
            Some(v) => parts.push(format!("{}: {v}", input.name)),
            None => positional = false,
        }
    }
    parts.join(", ")
}

/// A name not yet used: `base1`, `base2`, …
fn fresh_name(o: &Outline, base: &str) -> String {
    let taken = |n: &str| {
        o.step(n).is_some()
            || o.param(n).is_some()
            || BUILTINS.contains(&n)
            || RESERVED.contains(&n)
            || nodes::find(n).is_some()
    };
    (1..)
        .map(|k| format!("{base}{k}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| format!("{base}_new"))
}

fn check_new_name(o: &Outline, to: &str) -> Result<(), String> {
    if !edit::is_name(to) {
        return Err(format!(
            "`{to}` can't be a name: use letters, digits and _, starting with a letter (and not a word like `show`)"
        ));
    }
    if BUILTINS.contains(&to) {
        return Err(format!("`{to}` is a built-in name; pick another"));
    }
    if o.step(to).is_some() || o.param(to).is_some() {
        return Err(format!("`{to}` is already used"));
    }
    Ok(())
}

/// Where new steps go: after the last step, or the settings, or the header.
fn step_slot(o: &Outline) -> usize {
    o.steps
        .last()
        .map(|s| s.lines.range.1)
        .or_else(|| o.params.last().map(|p| p.lines.range.1))
        .unwrap_or(o.header_end)
}

/// Where new settings go: after the last one, or the header, or before the first step.
fn param_slot(o: &Outline) -> usize {
    o.params
        .last()
        .map(|p| p.lines.range.1)
        .or((o.header_end > 0).then_some(o.header_end))
        .or_else(|| o.steps.first().map(|s| s.lines.range.0))
        .unwrap_or(0)
}

fn add_step(
    src: &str,
    o: &Outline,
    node: &str,
    from: Option<&str>,
    input: Option<&str>,
) -> Result<(String, String), String> {
    let node = find_node(node)?;
    let wire = match (from, input) {
        (Some(f), Some(i)) => {
            let k = node
                .inputs
                .iter()
                .position(|x| x.name == i)
                .ok_or_else(|| format!("`{}` has no input `{i}`", node.name))?;
            Some((k, f))
        }
        (Some(f), None) => Some((node.inputs.iter().position(Input::required).unwrap_or(0), f)),
        (None, _) => None,
    };
    if let Some(f) = from
        && o.step(f).is_none()
        && o.param(f).is_none()
    {
        return Err(format!("there's no `{f}` to connect"));
    }
    let name = fresh_name(o, node.name);
    let line = format!("{name} = {}({})", node.name, new_args(node, None, wire));
    let at = from
        .and_then(|f| o.step(f))
        .map_or_else(|| step_slot(o), |s| s.lines.range.1);
    Ok((edit::insert_line(src, at, &line)?, name))
}

/// Removes a step or setting, one text edit at a time (each needs a fresh outline).
fn remove(src: &str, name: &str) -> Result<String, String> {
    let mut out = src.to_string();
    // Inputs fed only by it are emptied. Bounded: each round removes one argument.
    for _ in 0..10_000 {
        let o = outline(&out).map_err(|e| e.to_string())?;
        let hit = o.steps.iter().find_map(|s| {
            s.stages.iter().find_map(|st| {
                let node = st.call.as_deref().and_then(nodes::find)?;
                let names: Vec<&str> = node.inputs.iter().map(|i| i.name).collect();
                (0..names.len()).find_map(|k| {
                    let arg = edit::find_arg(st, k, names.get(k)?)?;
                    (out.get(arg.value.0..arg.value.1) == Some(name))
                        .then(|| (st.clone(), k, names.clone()))
                })
            })
        });
        let Some((st, k, names)) = hit else {
            break;
        };
        out = edit::remove_arg(&out, &st, k, &names)?;
    }
    let o = outline(&out).map_err(|e| e.to_string())?;
    if o.outputs
        .iter()
        .any(|x| x.names.iter().any(|n| n.0 == name))
    {
        out = set_shown(&out, &o, name, false)?;
    }
    let o = outline(&out).map_err(|e| e.to_string())?;
    let lines: Lines = o
        .step(name)
        .map(|s| s.lines)
        .or_else(|| o.param(name).map(|p| p.lines))
        .ok_or_else(|| format!("there's no `{name}` to remove"))?;
    edit::replace(&out, lines.range, "")
}

fn set_shown(src: &str, o: &Outline, step: &str, shown: bool) -> Result<String, String> {
    let shows: Vec<_> = o
        .outputs
        .iter()
        .filter(|x| x.kind == OutputKind::Show)
        .collect();
    let is_shown = shows.iter().any(|x| x.names.iter().any(|n| n.0 == step));
    if shown == is_shown {
        return Ok(src.to_string());
    }
    if shown {
        if o.step(step).is_none() {
            return Err(format!("there's no step `{step}`"));
        }
        return match shows.first().and_then(|x| x.names.last()) {
            Some((_, range)) => edit::insert(src, range.1, &format!(", {step}")),
            None => {
                let at = o
                    .outputs
                    .first()
                    .map_or_else(|| step_slot(o), |x| x.lines.range.0);
                edit::insert_line(src, at, &format!("show {step}"))
            }
        };
    }
    // Stop showing it: drop it from its list, or the whole line if it's the only name.
    let Some(show) = shows.iter().find(|x| x.names.iter().any(|n| n.0 == step)) else {
        return Ok(src.to_string());
    };
    if show.names.len() == 1 {
        return edit::replace(src, show.lines.range, "");
    }
    let k = show
        .names
        .iter()
        .position(|n| n.0 == step)
        .unwrap_or_default();
    let this = show.names.get(k).map_or((0, 0), |n| n.1);
    let range = match (
        k.checked_sub(1).and_then(|p| show.names.get(p)),
        show.names.get(k + 1),
    ) {
        (Some(prev), _) => (prev.1.1, this.1),
        (None, Some(next)) => (this.0, next.1.0),
        (None, None) => this,
    };
    edit::replace(src, range, "")
}

fn set_param(src: &str, o: &Outline, cmd: &Command) -> Result<String, String> {
    let Command::SetParam {
        name,
        value,
        min,
        max,
        step,
        label,
    } = cmd
    else {
        return Err("internal: not a slider change".into());
    };
    let (name, value, min, max, step) = (name.as_str(), *value, *min, *max, *step);
    let line = o
        .param(name)
        .ok_or_else(|| format!("there's no setting `{name}`"))?;
    let program = parse(src).map_err(|e| e.to_string())?;
    let p = program
        .params
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("there's no setting `{name}`"))?;
    if !matches!(p.value, easycad_lang::Literal::Number(_)) || p.choices.is_some() {
        return Err(format!(
            "`{name}` isn't a number slider; change it in the Recipe tab"
        ));
    }
    let (lo, hi) = (min.min(max), min.max(max));
    if ![value, lo, hi].iter().all(|v| v.is_finite())
        || step.is_some_and(|s| !(s > 0.0 && s.is_finite()))
    {
        return Err("the slider needs real numbers (and a step above 0)".into());
    }
    let unit = p.unit.word();
    let mut text = format!("param {name} = {}", format_number(value.clamp(lo, hi)));
    if !unit.is_empty() {
        text.push_str(&format!(" {unit}"));
    }
    text.push_str(&format!(" in {}..{}", format_number(lo), format_number(hi)));
    if let Some(s) = step {
        text.push_str(&format!(" step {}", format_number(s)));
    }
    if let Some(l) = label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .or(p.label.as_deref())
    {
        text.push_str(&format!(
            " {}",
            easycad_lang::format_literal(&easycad_lang::Literal::Text(l.to_string()))
        ));
    }
    for a in &p.attrs {
        text.push_str(&format!(" @{a}"));
    }
    edit::replace(src, line.range, &text)
}
