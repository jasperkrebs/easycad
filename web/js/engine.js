// Engine client: the Rust engine runs as WebAssembly in a Web Worker, so the UI never freezes.
//
// Only the newest request matters (like a game dropping stale frames): while the worker is busy,
// a new request replaces any queued one, and the replaced request resolves with `null`.

/**
 * @typedef {{ positions: Float32Array, indices: Uint32Array, path: Float32Array,
 *             stats: Record<string, number>, ms: number }} VaseResult
 */

export function createEngine() {
  const worker = new Worker(new URL('./engine.worker.js', import.meta.url), { type: 'module' });

  /** @type {(info: { version: string, loadMs: number }) => void} */
  let onReady = () => {};
  /** @type {(err: Error) => void} */
  let onFail = () => {};
  const ready = new Promise((resolve, reject) => {
    onReady = resolve;
    onFail = reject;
  });

  /** @type {{ params: object, resolve: (r: VaseResult | null) => void, reject: (e: Error) => void } | null} */
  let queued = null;
  /** @type {typeof queued} */
  let running = null;

  worker.onmessage = (e) => {
    const msg = e.data;
    if (msg.type === 'ready') {
      onReady({ version: msg.version, loadMs: msg.loadMs });
    } else if (msg.type === 'fatal') {
      onFail(new Error(msg.error));
    } else if (msg.type === 'result' || msg.type === 'error') {
      const job = running;
      running = null;
      if (job) {
        if (msg.type === 'result') job.resolve(msg.result);
        else job.reject(new Error(msg.error));
      }
      pump();
    }
  };
  worker.onerror = (e) => {
    onFail(new Error(e.message || 'The engine worker could not start.'));
  };

  function pump() {
    if (running || !queued) return;
    running = queued;
    queued = null;
    worker.postMessage({ type: 'vase', params: running.params });
  }

  return {
    ready,
    /**
     * Evaluates the twisted vase demo.
     * @param {object} params camelCase VaseParams fields (see crates/core/src/vase.rs)
     * @returns {Promise<VaseResult | null>} null when a newer request replaced this one
     */
    vase(params) {
      return new Promise((resolve, reject) => {
        if (queued) queued.resolve(null);
        queued = { params, resolve, reject };
        pump();
      });
    },
  };
}
