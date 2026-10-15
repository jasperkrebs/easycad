// Web Worker that hosts the Rust engine (WebAssembly). Generated glue lives in ../pkg/
// (scripts/build-web.sh). Big arrays go back to the page as transferables: moved, not copied.

import init, { engineVersion, evaluate, nodeCatalog } from '../pkg/easycad_wasm.js';

const started = performance.now();
const loading = init()
  .then(() => {
    postMessage({ type: 'ready', version: engineVersion(), loadMs: performance.now() - started });
  })
  .catch((err) => {
    postMessage({ type: 'fatal', error: String((err && err.message) || err) });
    throw err;
  });

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    await loading;
  } catch {
    return; // already reported as fatal
  }
  if (msg.type === 'catalog') {
    postMessage({ type: 'catalog', text: nodeCatalog() });
    return;
  }
  if (msg.type !== 'evaluate') return;
  try {
    const t0 = performance.now();
    const r = evaluate(msg.source, msg.values, msg.quality);
    const ms = performance.now() - t0;
    const transfer = [r.mesh.positions, r.mesh.indices, r.curves.points, r.curves.starts, r.paths.points, r.paths.starts].map(
      (a) => a.buffer,
    );
    postMessage({ type: 'result', result: { ...r, ms } }, transfer);
  } catch (err) {
    // A trap inside the WebAssembly is an engine bug; the instance can't be trusted after it,
    // so the page starts a fresh worker. Anything else is an ordinary error message.
    const crashed = err instanceof WebAssembly.RuntimeError;
    postMessage({ type: crashed ? 'crashed' : 'error', error: String((err && err.message) || err) });
  }
};
