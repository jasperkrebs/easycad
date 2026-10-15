// Engine client: the Rust engine runs as WebAssembly in a Web Worker, so the UI never freezes.
//
// Only the newest run matters (like a game dropping stale frames): while the worker is busy, a
// new run replaces any queued one, and the replaced run resolves with `null`. If the engine ever
// crashes (a bug), the worker is replaced by a fresh one and the run that hit it fails with a
// message.

/**
 * @typedef {{ source: string, project: string | null, printer: string | null, notes: string[],
 *   params: any[], steps: any[], diagnostics: { line: number, col: number, severity: string, message: string }[],
 *   print: { stats: Record<string, any>, printer: string | null } | null,
 *   bounds: { min: number[], max: number[] } | null,
 *   mesh: { positions: Float32Array, indices: Uint32Array },
 *   curves: { points: Float32Array, starts: Uint32Array },
 *   paths: { points: Float32Array, starts: Uint32Array }, ms: number }} RunResult
 */

export function createEngine() {
  /** @type {Worker} */
  let worker;
  /** @type {{ version: string, loadMs: number } | null} */
  let info = null;
  /** @type {(info: { version: string, loadMs: number }) => void} */
  let onReady = () => {};
  /** @type {(err: Error) => void} */
  let onFail = () => {};
  const ready = new Promise((resolve, reject) => {
    onReady = resolve;
    onFail = reject;
  });

  /** @typedef {{ source: string, values: object, quality: string, resolve: (r: RunResult | null) => void, reject: (e: Error) => void }} Job */
  /** @type {Job | null} */
  let queued = null;
  /** @type {Job | null} */
  let running = null;
  /** @type {(() => void)[]} */
  let idleWaiters = [];
  /** @type {((text: string) => void)[]} */
  let catalogWaiters = [];

  function start() {
    worker = new Worker(new URL('./engine.worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = onMessage;
    worker.onerror = (e) => {
      if (!info) onFail(new Error(e.message || 'The engine worker could not start.'));
    };
  }

  /** @param {MessageEvent} e */
  function onMessage(e) {
    const msg = e.data;
    if (msg.type === 'ready') {
      info = { version: msg.version, loadMs: msg.loadMs };
      onReady(info);
    } else if (msg.type === 'fatal') {
      onFail(new Error(msg.error));
    } else if (msg.type === 'catalog') {
      catalogWaiters.splice(0).forEach((w) => w(msg.text));
    } else if (msg.type === 'result' || msg.type === 'error' || msg.type === 'crashed') {
      const job = running;
      running = null;
      if (msg.type === 'crashed') {
        worker.terminate();
        start();
        // A catalog request the old worker never answered is asked again.
        if (catalogWaiters.length) worker.postMessage({ type: 'catalog' });
      }
      if (job) {
        if (msg.type === 'result') job.resolve(msg.result);
        else job.reject(new Error(msg.type === 'crashed' ? `The engine hit an internal error and restarted (${msg.error}).` : msg.error));
      }
      pump();
    }
  }

  function pump() {
    if (running) return;
    if (!queued) {
      idleWaiters.splice(0).forEach((w) => w());
      return;
    }
    running = queued;
    queued = null;
    worker.postMessage({ type: 'evaluate', source: running.source, values: running.values, quality: running.quality });
  }

  start();

  return {
    ready,

    /**
     * Runs a recipe, with setting values written into it first.
     * @param {string} source recipe text
     * @param {Record<string, number | boolean | string>} values setting values, each in its own unit
     * @param {'preview' | 'final'} quality
     * @returns {Promise<RunResult | null>} null when a newer run replaced this one
     */
    evaluate(source, values, quality) {
      return new Promise((resolve, reject) => {
        if (queued) queued.resolve(null);
        queued = { source, values, quality, resolve, reject };
        pump();
      });
    },

    /** Resolves once no run is waiting or in progress. */
    idle() {
      return new Promise((resolve) => {
        idleWaiters.push(resolve);
        if (!running && !queued) pump();
      });
    },

    /** The node catalog (every step a recipe can use), as text. */
    catalog() {
      return new Promise((resolve) => {
        catalogWaiters.push(resolve);
        worker.postMessage({ type: 'catalog' });
      });
    },
  };
}
