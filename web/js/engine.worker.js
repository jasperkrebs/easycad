// Web Worker that hosts the Rust engine (WebAssembly). Generated glue lives in ../pkg/
// (scripts/build-web.sh). Big arrays go back to the page as transferables: moved, not copied.

import init, { engineVersion, twistedVase } from '../pkg/easycad_wasm.js';

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
  const { type, params } = e.data;
  if (type !== 'vase') return;
  try {
    await loading;
    const t0 = performance.now();
    const r = twistedVase(params);
    const ms = performance.now() - t0;
    postMessage(
      { type: 'result', result: { positions: r.positions, indices: r.indices, path: r.path, stats: r.stats, ms } },
      [r.positions.buffer, r.indices.buffer, r.path.buffer],
    );
  } catch (err) {
    postMessage({ type: 'error', error: String((err && err.message) || err) });
  }
};
