// Web Worker that hosts the Rust engine (WebAssembly). Generated glue lives in ../pkg/
// (scripts/build-web.sh). Big arrays go back to the page as transferables: moved, not copied.
// Every request carries an id, and its answer (result, error or crash) carries the same id.

import init, { engineVersion, evaluate, exportFile, nodeCatalog } from '../pkg/easycad_wasm.js';

const started = performance.now();
const loading = init()
  .then(() => {
    postMessage({ type: 'ready', version: engineVersion(), loadMs: performance.now() - started });
  })
  .catch((err) => {
    postMessage({ type: 'fatal', error: String((err && err.message) || err) });
    throw err;
  });

/** Answers one request: the result, and the buffers to move rather than copy. */
function answer(msg) {
  if (msg.type === 'catalog') return { result: nodeCatalog(), transfer: [] };
  if (msg.type === 'export') {
    const file = exportFile(msg.source, msg.values, msg.which, msg.profile);
    return { result: { name: file.name, data: file.data }, transfer: [file.data.buffer] };
  }
  if (msg.type === 'evaluate') {
    const t0 = performance.now();
    const r = evaluate(msg.source, msg.values, msg.quality);
    const ms = performance.now() - t0;
    const arrays = [r.mesh.positions, r.mesh.indices, r.curves.points, r.curves.starts, r.paths.points, r.paths.starts];
    return { result: { ...r, ms }, transfer: arrays.map((a) => a.buffer) };
  }
  throw new Error(`unknown request: ${msg.type}`);
}

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    await loading;
  } catch {
    return; // already reported as fatal
  }
  try {
    const { result, transfer } = answer(msg);
    postMessage({ type: 'result', id: msg.id, result }, transfer);
  } catch (err) {
    // A trap inside the WebAssembly is an engine bug; the instance can't be trusted after it,
    // so the page starts a fresh worker. Anything else is an ordinary error message.
    const crashed = err instanceof WebAssembly.RuntimeError;
    postMessage({ type: crashed ? 'crashed' : 'error', id: msg.id, error: String((err && err.message) || err) });
  }
};
