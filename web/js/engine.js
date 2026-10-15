// Engine client: the Rust engine runs as WebAssembly in a Web Worker, so the UI never freezes.
//
// Every request carries an id and gets one answer. For runs only the newest matters (like a game
// dropping stale frames): while the worker is busy, a new run replaces any queued one, and the
// replaced run resolves with `null`. If the engine ever crashes (a bug), the worker is replaced
// by a fresh one: the request that hit it fails with a message, and the others are asked again.

/**
 * @typedef {{ source: string, project: string | null, printer: string | null, notes: string[],
 *   params: any[], steps: any[], diagnostics: { line: number, col: number, severity: string, message: string }[],
 *   print: { stats: Record<string, any>, printer: string | null } | null,
 *   exports: { file: string, kind: 'stl' | 'gcode', line: number }[],
 *   bounds: { min: number[], max: number[] } | null,
 *   mesh: { positions: Float32Array, indices: Uint32Array },
 *   curves: { points: Float32Array, starts: Uint32Array },
 *   paths: { points: Float32Array, starts: Uint32Array },
 *   highlight: { mesh: { positions: Float32Array, indices: Uint32Array },
 *     curves: { points: Float32Array, starts: Uint32Array }, paths: { points: Float32Array, starts: Uint32Array } },
 *   ms: number }} RunResult
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

  let nextId = 1;
  /** Requests the worker hasn't answered yet, by id. @type {Map<number, { msg: object, resolve: (r: any) => void, reject: (e: Error) => void }>} */
  const pending = new Map();

  /** @typedef {{ source: string, values: object, quality: string, highlight: string, resolve: (r: RunResult | null) => void, reject: (e: Error) => void }} Job */
  /** @type {Job | null} */
  let queued = null;
  let running = false;
  /** @type {(() => void)[]} */
  let idleWaiters = [];

  function start() {
    worker = new Worker(new URL('./engine.worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = onMessage;
    worker.onerror = (e) => {
      if (!info) fail(new Error(e.message || 'The engine worker could not start.'));
    };
  }

  /** The engine can't start: everything waiting on it fails. */
  function fail(err) {
    onFail(err);
    for (const r of pending.values()) r.reject(err);
    pending.clear();
  }

  /** @param {{ type: string } & Record<string, any>} msg */
  function send(msg) {
    return new Promise((resolve, reject) => {
      const withId = { ...msg, id: nextId++ };
      pending.set(withId.id, { msg: withId, resolve, reject });
      worker.postMessage(withId);
    });
  }

  /** @param {MessageEvent} e */
  function onMessage(e) {
    const msg = e.data;
    if (msg.type === 'ready') {
      info = { version: msg.version, loadMs: msg.loadMs };
      onReady(info);
      return;
    }
    if (msg.type === 'fatal') {
      fail(new Error(msg.error));
      return;
    }
    const request = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.type === 'crashed') {
      worker.terminate();
      start();
      // Whatever the old worker never answered is asked again.
      for (const r of pending.values()) worker.postMessage(r.msg);
    }
    if (!request) return;
    if (msg.type === 'result') request.resolve(msg.result);
    else request.reject(new Error(msg.type === 'crashed' ? `The engine hit an internal error and restarted (${msg.error}).` : msg.error));
  }

  function pump() {
    if (running) return;
    const job = queued;
    if (!job) {
      idleWaiters.splice(0).forEach((w) => w());
      return;
    }
    queued = null;
    running = true;
    send({ type: 'evaluate', source: job.source, values: job.values, quality: job.quality, highlight: job.highlight })
      .then(job.resolve, job.reject)
      .finally(() => {
        running = false;
        pump();
      });
  }

  start();

  return {
    ready,

    /**
     * Runs a recipe, with setting values written into it first.
     * @param {string} source recipe text
     * @param {Record<string, number | boolean | string>} values setting values, each in its own unit
     * @param {'preview' | 'final'} quality
     * @param {string} [highlight] a step to draw on its own too (the graph's selected component)
     * @returns {Promise<RunResult | null>} null when a newer run replaced this one
     */
    evaluate(source, values, quality, highlight = '') {
      return new Promise((resolve, reject) => {
        if (queued) queued.resolve(null);
        queued = { source, values, quality, highlight, resolve, reject };
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

    /**
     * Makes a file to save: the printed toolpath as G-code (`which` = 'print'), or the file of
     * one of the recipe's `export` lines (`which` = its index in `exports`). Fails with a
     * readable message when there's nothing to export or the toolpath isn't safe to print.
     * @param {string} source recipe text
     * @param {Record<string, number | boolean | string>} values setting values
     * @param {string} which
     * @param {object} profile the printer (see print.js)
     * @returns {Promise<{ name: string, data: Uint8Array }>}
     */
    exportFile(source, values, which, profile) {
      return send({ type: 'export', source, values, which, profile });
    },

    /** The node catalog (every step a recipe can use), as text. @returns {Promise<string>} */
    catalog() {
      return send({ type: 'catalog' });
    },

    /** Every step with its inputs, for the graph's palette. @returns {Promise<any[]>} */
    palette() {
      return send({ type: 'palette' });
    },

    /**
     * The recipe as components and wires (see easycad_core::graph). Fails with the syntax
     * error when the recipe doesn't parse.
     * @param {string} source
     * @returns {Promise<any>}
     */
    graph(source) {
      return send({ type: 'graph', source });
    },

    /**
     * Runs a graph command on the recipe text and returns `{ source, name }`: the edited
     * recipe, and the name of anything added. Fails with a readable message.
     * @param {string} source
     * @param {Record<string, any>} command e.g. `{ op: 'setInput', step, stage, input, text }`
     * @returns {Promise<{ source: string, name: string | null }>}
     */
    editGraph(source, command) {
      return send({ type: 'edit', source, command });
    },
  };
}
