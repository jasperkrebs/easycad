//! WebAssembly facade for the web app (`web/js/engine.worker.js` loads it in a Web Worker).
//!
//! Deliberately thin: convert JavaScript values to engine types, call the engine, and convert
//! the results back. Big arrays are returned as typed arrays so the worker can hand them to the
//! UI without copying.

use easycad_core::vase::{self, VaseParams};
use js_sys::{Float32Array, Object, Reflect, Uint32Array};
use wasm_bindgen::prelude::*;

/// Engine version, shown in the app's About panel.
#[wasm_bindgen(js_name = engineVersion)]
pub fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Evaluates the Phase 0 demo (the twisted star vase).
///
/// `params` is a plain object with any of the `VaseParams` fields in camelCase (missing ones
/// use the defaults). Returns `{ positions, indices, path, stats }`, or throws an `Error` with
/// a readable message for invalid settings.
#[wasm_bindgen(js_name = twistedVase)]
pub fn twisted_vase(params: JsValue) -> Result<Object, JsError> {
    let p: VaseParams = serde_wasm_bindgen::from_value(params)
        .map_err(|e| JsError::new(&format!("settings: {e}")))?;
    let out = vase::build(&p).map_err(|e| JsError::new(&e.to_string()))?;

    let result = Object::new();
    set(
        &result,
        "positions",
        &Float32Array::from(out.mesh.positions.as_slice()),
    )?;
    set(
        &result,
        "indices",
        &Uint32Array::from(out.mesh.indices.as_slice()),
    )?;
    set(&result, "path", &Float32Array::from(out.path.as_slice()))?;
    let stats = serde_wasm_bindgen::to_value(&out.stats)
        .map_err(|e| JsError::new(&format!("stats: {e}")))?;
    set(&result, "stats", &stats)?;
    Ok(result)
}

fn set(target: &Object, key: &str, value: &JsValue) -> Result<(), JsError> {
    Reflect::set(target, &JsValue::from_str(key), value)
        .map(|_| ())
        .map_err(|_| JsError::new("internal: could not build the result object"))
}
