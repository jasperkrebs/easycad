//! WebAssembly facade for the web app (`web/js/engine.worker.js` loads it in a Web Worker).
//!
//! Deliberately thin: convert JavaScript values to engine types, call the engine, and convert
//! the results back. Big arrays are returned as typed arrays so the worker can hand them to the
//! UI without copying.

use easycad_core::eval::{Drawing, Evaluation, MAX_SOURCE_BYTES, Quality};
use easycad_core::files::{self, Which};
use easycad_core::gcode::Profile;
use easycad_core::geom::P3;
use easycad_core::nodes;
use easycad_lang::{Literal, set_values};
use js_sys::{Float32Array, Int32Array, Object, Reflect, Uint8Array, Uint32Array};
use serde::Serialize;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// Engine version, shown in the app's About panel.
#[wasm_bindgen(js_name = engineVersion)]
pub fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The node catalog as text: every step a recipe can use, with its inputs.
#[wasm_bindgen(js_name = nodeCatalog)]
pub fn node_catalog() -> String {
    nodes::catalog()
}

/// Runs a recipe.
///
/// `values` (optional) holds setting values to write into the recipe first, each in its own
/// unit as shown in the settings window: `{ height: 200, depth: 18 }`. Slider moves become edits
/// to the recipe text, so the text stays the single source of truth; the edited text comes
/// back as `source`. `quality` is `"preview"` or `"final"`. `simulate` (the printer's extrusion
/// multiplier) adds the bead simulation; `wall` adds the way up the wall at each toolpath point
/// (for a view that stands the nozzle along it).
///
/// Returns `{ source, project, printer, notes, params, steps, diagnostics, print, exports,
/// bounds, mesh: { positions, indices }, curves: { points, starts },
/// paths: { points, starts, widths, heights, speeds, flows, ups } (each point's line width and
/// layer height in mm, speed in mm/s and flow in mm³/s: the move that ends there; 0 for travel;
/// with `wall`, `ups`: x,y,z of the way up the wall there, `easycad_core::wall`), parts: { mesh, curves, paths } (each
/// piece's printed part) }`. A broken recipe doesn't throw: its problems are in
/// `diagnostics`.
#[wasm_bindgen]
pub fn evaluate(
    source: &str,
    values: JsValue,
    quality: &str,
    simulate: Option<f64>,
    wall: Option<bool>,
) -> Result<Object, JsError> {
    let quality = if quality == "final" {
        Quality::Final
    } else {
        Quality::Preview
    };
    let source = with_values(source, values)?;
    let e = easycad_core::evaluate(&source, quality);
    let out = to_js(&source, &e, wall.unwrap_or(false))?;
    if let Some(multiplier) = simulate {
        simulation(&out, &e, multiplier)?;
    }
    Ok(out)
}

/// The beads as the plastic would actually lie (`easycad_core::deposit`), added to the result:
/// `paths.simWidths`, `paths.simHeights` and `paths.simFlags` per point (packed like `widths`),
/// and `simulation`, what it found. A simulation that can't run is left out, not an error.
fn simulation(out: &Object, e: &Evaluation, multiplier: f64) -> Result<(), JsError> {
    let paths: Vec<&easycad_core::toolpath::Toolpath> =
        e.drawing.paths.iter().map(|p| p.as_ref()).collect();
    let Ok((beads, summary)) =
        easycad_core::deposit::simulate(&paths, &e.drawing.path_parts, multiplier)
    else {
        return Ok(());
    };
    let packed = Reflect::get(out, &JsValue::from_str("paths"))
        .map_err(|_| JsError::new("internal: no paths"))?;
    let packed: Object = packed.into();
    let widths: Vec<f32> = beads
        .iter()
        .flat_map(|b| b.widths.iter().copied())
        .collect();
    let heights: Vec<f32> = beads
        .iter()
        .flat_map(|b| b.heights.iter().copied())
        .collect();
    let flags: Vec<u8> = beads.iter().flat_map(|b| b.flags.iter().copied()).collect();
    set(&packed, "simWidths", &Float32Array::from(widths.as_slice()))?;
    set(
        &packed,
        "simHeights",
        &Float32Array::from(heights.as_slice()),
    )?;
    set(&packed, "simFlags", &Uint8Array::from(flags.as_slice()))?;
    set(out, "simulation", &plain(&summary)?)?;
    Ok(())
}

/// Names the project's printer in the recipe (its `printer "…"` line, added if missing) and
/// returns the new recipe text. Throws a readable message if it can't.
#[wasm_bindgen(js_name = setPrinter)]
pub fn set_printer(source: &str, name: &str) -> Result<String, JsError> {
    easycad_lang::set_printer(source, name).map_err(|m| JsError::new(&m))
}

/// Adds a part made from `component`, named `name`, with `values` (`{ setting: value }`; an
/// input takes a port's name, like `"main.end_out"`), and returns the new recipe text. Throws a
/// readable message if it can't.
#[wasm_bindgen(js_name = addPart)]
pub fn add_part(
    source: &str,
    component: &str,
    name: &str,
    values: JsValue,
) -> Result<String, JsError> {
    let values: BTreeMap<String, Literal> = if values.is_undefined() || values.is_null() {
        BTreeMap::new()
    } else {
        serde_wasm_bindgen::from_value(values)
            .map_err(|e| JsError::new(&format!("settings: {e}")))?
    };
    let pairs: Vec<(String, Literal)> = values.into_iter().collect();
    easycad_lang::add_part(source, component, name, &pairs).map_err(|m| JsError::new(&m))
}

/// Removes the part `name` and every part mounted on it; returns the new recipe text.
#[wasm_bindgen(js_name = removePart)]
pub fn remove_part(source: &str, name: &str) -> Result<String, JsError> {
    easycad_lang::remove_part(source, name).map_err(|m| JsError::new(&m))
}

/// Makes a file to save: `which` is `"print"` for the printed toolpath as G-code, or the index
/// of an `export` statement (STL for shapes, G-code for a toolpath). `profile` is the printer
/// (see `easycad_core::gcode::Profile`; missing fields use EasyGCode's defaults). Returns
/// `{ name, data }` where `data` is the file's bytes (`name` is empty for the printed G-code).
/// Throws a readable message if the recipe has nothing to export or the toolpath isn't safe to
/// print.
#[wasm_bindgen(js_name = exportFile)]
pub fn export_file(
    source: &str,
    values: JsValue,
    which: &str,
    profile: JsValue,
) -> Result<Object, JsError> {
    let source = with_values(source, values)?;
    let profile: Profile = if profile.is_undefined() || profile.is_null() {
        Profile::default()
    } else {
        serde_wasm_bindgen::from_value(profile)
            .map_err(|e| JsError::new(&format!("printer settings: {e}")))?
    };
    // "print" (the first part), "print:1" (another part), or an export's index.
    let which = match which.strip_prefix("print") {
        Some("") => Which::Print(0),
        Some(part) => Which::Print(
            part.strip_prefix(':')
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| JsError::new("unknown part"))?,
        ),
        None => Which::Export(which.parse().map_err(|_| JsError::new("unknown export"))?),
    };
    let e = easycad_core::evaluate(&source, Quality::Final);
    let file = files::make(&e, which, &profile).map_err(|m| JsError::new(&m))?;
    let out = Object::new();
    set(&out, "name", &JsValue::from_str(&file.name))?;
    set(&out, "data", &Uint8Array::from(file.bytes.as_slice()))?;
    Ok(out)
}

/// The recipe with setting values written in (the text stays the single source of truth).
fn with_values(source: &str, values: JsValue) -> Result<String, JsError> {
    let values: BTreeMap<String, Literal> = if values.is_undefined() || values.is_null() {
        BTreeMap::new()
    } else {
        serde_wasm_bindgen::from_value(values)
            .map_err(|e| JsError::new(&format!("settings: {e}")))?
    };
    let pairs: Vec<(String, Literal)> = values.into_iter().collect();
    Ok(if pairs.is_empty() || source.len() > MAX_SOURCE_BYTES {
        source.to_string()
    } else {
        // A recipe that doesn't parse keeps its text; the evaluation reports the error.
        set_values(source, &pairs).unwrap_or_else(|_| source.to_string())
    })
}

fn to_js(source: &str, e: &Evaluation, wall: bool) -> Result<Object, JsError> {
    let out = Object::new();
    set(&out, "source", &JsValue::from_str(source))?;
    set(&out, "project", &plain(&e.project)?)?;
    set(&out, "printer", &plain(&e.printer)?)?;
    set(&out, "notes", &plain(&e.notes)?)?;
    set(&out, "params", &plain(&e.params)?)?;
    set(&out, "steps", &plain(&e.steps)?)?;
    set(&out, "diagnostics", &plain(&e.diagnostics)?)?;
    set(&out, "print", &plain(&e.print)?)?;
    set(&out, "prints", &plain(&e.prints)?)?;
    set(&out, "exports", &plain(&e.exports)?)?;
    set(&out, "partList", &plain(&e.parts)?)?;
    set(&out, "ports", &plain(&e.ports)?)?;
    set(&out, "components", &plain(&e.components)?)?;
    let bounds = &e.drawing.bounds;
    set(
        &out,
        "bounds",
        &if bounds.is_empty() {
            JsValue::NULL
        } else {
            plain(bounds)?
        },
    )?;

    let mesh = Object::new();
    set(
        &mesh,
        "positions",
        &Float32Array::from(e.drawing.mesh.positions.as_slice()),
    )?;
    set(
        &mesh,
        "indices",
        &Uint32Array::from(e.drawing.mesh.indices.as_slice()),
    )?;
    set(&out, "mesh", &mesh)?;
    let curves = polylines(e.drawing.curves.iter().map(Vec::as_slice))?;
    set(&out, "curves", &curves)?;
    // Placed parts where they're shown (the G-code keeps them where they print).
    let shown: Vec<std::borrow::Cow<[P3]>> = e
        .drawing
        .paths
        .iter()
        .map(|p| match &p.view {
            Some(x) => std::borrow::Cow::Owned(p.points.iter().map(|q| x.apply(*q)).collect()),
            None => std::borrow::Cow::Borrowed(p.points.as_slice()),
        })
        .collect();
    let paths = polylines(shown.iter().map(|p| p.as_ref()))?;
    let [widths, heights, speeds, flows] = beads(&e.drawing);
    set(&paths, "widths", &widths)?;
    set(&paths, "heights", &heights)?;
    set(&paths, "speeds", &speeds)?;
    set(&paths, "flows", &flows)?;
    // Only when the view stands the nozzle along the wall: for a print of many rings it takes
    // several times as long as the recipe itself.
    if wall {
        let ups: Vec<f32> = e
            .drawing
            .paths
            .iter()
            .flat_map(|p| {
                let ups = easycad_core::wall::up_the_wall(p);
                match &p.view {
                    Some(x) => ups.into_iter().map(|v| x.apply_vector(v)).collect(),
                    None => ups,
                }
            })
            .flat_map(|v| v.map(|c| c as f32))
            .collect();
        set(&paths, "ups", &Float32Array::from(ups.as_slice()))?;
    }
    set(&out, "paths", &paths)?;
    let parts = parts(&e.drawing)?;
    set(&out, "parts", &parts)?;
    Ok(out)
}

/// Which printed part each piece of the drawing belongs to (-1: none), so the view can hide a
/// part: `{ mesh: [end, part, end, part, …] (runs over mesh.indices), curves, paths }`.
fn parts(d: &Drawing) -> Result<Object, JsError> {
    let part = |p: Option<usize>| p.map_or(-1, |p| p.min(i32::MAX as usize) as i32);
    let mesh: Vec<i32> = d
        .mesh_parts
        .iter()
        .flat_map(|(end, p)| [(*end).min(i32::MAX as usize) as i32, part(*p)])
        .collect();
    let curves: Vec<i32> = d.curve_parts.iter().map(|p| part(*p)).collect();
    let paths: Vec<i32> = d.path_parts.iter().map(|p| part(*p)).collect();
    let out = Object::new();
    set(&out, "mesh", &Int32Array::from(mesh.as_slice()))?;
    set(&out, "curves", &Int32Array::from(curves.as_slice()))?;
    set(&out, "paths", &Int32Array::from(paths.as_slice()))?;
    Ok(out)
}

/// Several polylines as one `points` array (x,y,z floats) plus `starts`: where each polyline
/// begins, counted in points, with the total count at the end.
/// Each toolpath point's bead, line width and layer height (mm): the bead of the move that ends
/// there, 0 for a travel. A path's first point takes its first move's, so a bead starts full size.
fn beads(d: &Drawing) -> [Float32Array; 4] {
    let mut out: [Vec<f32>; 4] = Default::default();
    for p in &d.paths {
        // Each move's line width and layer height, its speed in mm/s as the G-code writes it
        // (before the printer's travel-speed cap) and its volumetric flow in mm³/s; 0 for travel.
        let bead = |i: usize| {
            if p.travel.get(i).copied().unwrap_or(false) {
                [0.0; 4]
            } else {
                let (w, h) = p.bead_of(i);
                let area = p.area(i);
                let speed = p.settings(i).feed_for(area) / 60.0 * p.speed_factor(i);
                let flow = area * speed * p.flow.get(i).copied().unwrap_or(1.0);
                [w as f32, h as f32, speed as f32, flow as f32]
            }
        };
        for i in 0..p.points.len() {
            let b = bead(if i == 0 && p.points.len() > 1 { 1 } else { i });
            for (list, v) in out.iter_mut().zip(b) {
                list.push(v);
            }
        }
    }
    out.map(|v| Float32Array::from(v.as_slice()))
}

fn polylines<'a>(lines: impl Iterator<Item = &'a [P3]>) -> Result<Object, JsError> {
    let mut points: Vec<f32> = Vec::new();
    let mut starts: Vec<u32> = vec![0];
    for line in lines {
        points.extend(line.iter().flat_map(|p| p.map(|c| c as f32)));
        starts.push((points.len() / 3) as u32);
    }
    let out = Object::new();
    set(&out, "points", &Float32Array::from(points.as_slice()))?;
    set(&out, "starts", &Uint32Array::from(starts.as_slice()))?;
    Ok(out)
}

/// Converts to plain JavaScript objects and arrays (`null` for missing values).
fn plain<T: Serialize + ?Sized>(value: &T) -> Result<JsValue, JsError> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true);
    value
        .serialize(&serializer)
        .map_err(|e| JsError::new(&format!("internal: {e}")))
}

fn set(target: &Object, key: &str, value: &JsValue) -> Result<(), JsError> {
    Reflect::set(target, &JsValue::from_str(key), value)
        .map(|_| ())
        .map_err(|_| JsError::new("internal: could not build the result object"))
}
