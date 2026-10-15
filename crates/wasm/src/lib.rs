//! WebAssembly facade for the web app (`web/js/engine.worker.js` loads it in a Web Worker).
//!
//! Deliberately thin: convert JavaScript values to engine types, call the engine, and convert
//! the results back. Big arrays are returned as typed arrays so the worker can hand them to the
//! UI without copying.

use easycad_core::eval::{Evaluation, MAX_SOURCE_BYTES, Quality};
use easycad_core::files::{self, Which};
use easycad_core::gcode::Profile;
use easycad_core::geom::P3;
use easycad_core::nodes;
use easycad_lang::{Literal, set_values};
use js_sys::{Float32Array, Object, Reflect, Uint8Array, Uint32Array};
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
/// back as `source`. `quality` is `"preview"` or `"final"`.
///
/// Returns `{ source, project, printer, notes, params, steps, diagnostics, print, exports,
/// bounds, mesh: { positions, indices }, curves: { points, starts }, paths: { points, starts } }`.
/// A broken recipe doesn't throw: its problems are in `diagnostics`.
#[wasm_bindgen]
pub fn evaluate(source: &str, values: JsValue, quality: &str) -> Result<Object, JsError> {
    let quality = if quality == "final" {
        Quality::Final
    } else {
        Quality::Preview
    };
    let source = with_values(source, values)?;
    let e = easycad_core::evaluate(&source, quality);
    to_js(&source, &e)
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
    let which = match which {
        "print" => Which::Print,
        index => Which::Export(index.parse().map_err(|_| JsError::new("unknown export"))?),
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

fn to_js(source: &str, e: &Evaluation) -> Result<Object, JsError> {
    let out = Object::new();
    set(&out, "source", &JsValue::from_str(source))?;
    set(&out, "project", &plain(&e.project)?)?;
    set(&out, "printer", &plain(&e.printer)?)?;
    set(&out, "notes", &plain(&e.notes)?)?;
    set(&out, "params", &plain(&e.params)?)?;
    set(&out, "steps", &plain(&e.steps)?)?;
    set(&out, "diagnostics", &plain(&e.diagnostics)?)?;
    set(&out, "print", &plain(&e.print)?)?;
    set(&out, "exports", &plain(&e.exports)?)?;
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
    let paths = polylines(e.drawing.paths.iter().map(|p| p.points.as_slice()))?;
    set(&out, "paths", &paths)?;
    Ok(out)
}

/// Several polylines as one `points` array (x,y,z floats) plus `starts`: where each polyline
/// begins, counted in points, with the total count at the end.
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
