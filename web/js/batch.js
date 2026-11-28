// The Render's batch: setups saved one by one (a name, the render's settings with the camera,
// and the project as prepared: its beads, the parts' looks, the fittings), then rendered one
// after another, each picture downloaded as rendered and denoised. Kept in the browser's
// IndexedDB (the beads are too big for localStorage), so a batch survives a reload. The file
// names and the list's lines are pure helpers, for the node tests.

const DB = 'easycad-batch';
const STORE = 'setups';
/** The batch's pictures, kept as they're made (whether or not the browser let them download). */
const PICTURES = 'pictures';

/**
 * @typedef {{
 *   id: number,
 *   name: string,
 *   project: string,
 *   render: any,
 *   snapshot: any,
 *   saved: number,
 *   order?: number,
 *   recipe?: { source: string, alongWall: boolean, hidden: string[], u: number, colors: any },
 * }} BatchSetup
 * `snapshot`: the beads as saved (on this computer); `recipe`: what makes them (the project
 * file's plan carries only this), with the parts' looks and the way it hangs.
 */

/** @returns {Promise<IDBDatabase>} */
function open() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 2);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE, { keyPath: 'id' });
      if (!db.objectStoreNames.contains(PICTURES)) db.createObjectStore(PICTURES, { keyPath: 'key' });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/**
 * One request in a transaction, its result.
 * @param {'readonly' | 'readwrite'} mode
 * @param {(store: IDBObjectStore) => IDBRequest} make
 */
async function request(mode, make, store = STORE) {
  const db = await open();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(store, mode);
      const req = make(tx.objectStore(store));
      tx.oncomplete = () => resolve(req.result);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Where a setup stands in the batch: its place when moved, else when it was saved. */
const place = (s) => (Number.isFinite(s.order) ? /** @type {number} */ (s.order) : s.id);

/** The saved setups, in the batch's order. @returns {Promise<BatchSetup[]>} */
export async function batchSetups() {
  const all = /** @type {BatchSetup[]} */ (await request('readonly', (s) => s.getAll()));
  return inBatchOrder(all);
}

/**
 * Setups in the batch's order (their place, else when saved).
 * @template {{ id: number, order?: number }} T
 * @param {T[]} setups
 */
export function inBatchOrder(setups) {
  return [...setups].sort((a, b) => place(a) - place(b) || a.id - b.id);
}

/**
 * The setups with one moved a place up (-1) or down (+1): each given its place (`order`) anew.
 * @template {{ id: number, order?: number }} T
 * @param {T[]} setups in the batch's order
 * @param {number} id
 * @param {-1 | 1} way
 * @returns {T[]} those whose place changed
 */
export function movedInBatch(setups, id, way) {
  const list = inBatchOrder(setups);
  const k = list.findIndex((s) => s.id === id);
  const j = k + way;
  if (k < 0 || j < 0 || j >= list.length) return [];
  [list[k], list[j]] = [list[j], list[k]];
  // Every setup gets its place (0, 1, …), so the order holds; one saved later has none yet and
  // stands at its save time (its id, in ms): after them.
  return list.map((s, n) => ({ ...s, order: n })).filter((s, n) => setups.find((x) => x.id === s.id)?.order !== n);
}

/**
 * Moves a setup a place up or down in the stored batch.
 * @param {number} id
 * @param {-1 | 1} way
 */
export async function moveBatchSetup(id, way) {
  for (const s of movedInBatch(await batchSetups(), id, way)) await addBatchSetup(s);
}

/** @param {BatchSetup} setup */
export async function addBatchSetup(setup) {
  await request('readwrite', (s) => s.put(setup));
}

/** @param {number} id */
export async function removeBatchSetup(id) {
  await request('readwrite', (s) => s.delete(id));
}

/**
 * The name the next setup gets unless one is typed: the lowest number not taken yet.
 * @param {{ name: string }[]} setups
 */
export function nextBatchName(setups) {
  const taken = new Set(setups.map((s) => s.name));
  let n = 1;
  while (taken.has(String(n))) n++;
  return String(n);
}

/**
 * A batch's file name: the batch's start (so its pictures sort together in the downloads),
 * the setup's name, and which picture.
 * @param {Date} started
 * @param {string} name
 * @param {'rendered' | 'denoised'} which
 */
export function batchFileName(started, name, which) {
  const two = (n) => String(n).padStart(2, '0');
  const stamp = `${started.getFullYear()}-${two(started.getMonth() + 1)}-${two(started.getDate())}-${two(started.getHours())}${two(started.getMinutes())}`;
  const safe = String(name).replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '-').trim().slice(0, 80) || 'render';
  return `batch-${stamp}-${safe}${which === 'denoised' ? '-denoised' : ''}.png`;
}

/**
 * What a setup's line says about it: project, size and samples, light, view.
 * @param {{ project: string, render: any }} setup
 * @param {Record<string, { label: string }>} views
 */
export function batchLine(setup, views) {
  const r = setup.render || {};
  const parts = [
    setup.project,
    r.final ? `${r.final.size} px · ${r.final.samples} spp` : '',
    r.light === false ? 'light off' : 'light on',
    views[r.camera?.view]?.label ?? '',
  ];
  return parts.filter(Boolean).join(' · ');
}

/**
 * The batch plan a project file keeps: its setups (those saved from this project) without their
 * beads, each recipe written once.
 * @param {BatchSetup[]} setups
 * @param {string} project
 */
export function batchPlanOf(setups, project) {
  /** @type {string[]} */
  const sources = [];
  const planned = [];
  for (const s of setups) {
    if (s.project !== project || !s.recipe || typeof s.recipe.source !== 'string') continue;
    let k = sources.indexOf(s.recipe.source);
    if (k < 0) k = sources.push(s.recipe.source) - 1;
    const { source, ...rest } = s.recipe;
    planned.push({ name: s.name, project: s.project, render: s.render, recipe: { ...rest, source: k } });
  }
  return planned.length ? { sources, setups: planned } : null;
}

/**
 * The setups of a file's batch plan, ready to add (no beads: the batch makes them from the
 * recipe), each with an id after `firstId`. Setups the batch has already (same name, project,
 * settings and recipe) are left out. Anything unreadable is left out.
 * @param {any} plan
 * @param {BatchSetup[]} have
 * @param {number} firstId
 * @returns {BatchSetup[]}
 */
export function setupsOfPlan(plan, have, firstId) {
  if (!plan || !Array.isArray(plan.setups) || !Array.isArray(plan.sources)) return [];
  const same = (a, b) => a.name === b.name && a.project === b.project && JSON.stringify(a.render) === JSON.stringify(b.render) && JSON.stringify(a.recipe) === JSON.stringify(b.recipe);
  const out = [];
  for (const p of plan.setups) {
    const source = plan.sources[p?.recipe?.source];
    if (typeof source !== 'string' || typeof p.name !== 'string' || !p.render || typeof p.render !== 'object') continue;
    const r = p.recipe;
    const setup = {
      id: firstId + out.length,
      name: p.name,
      project: typeof p.project === 'string' ? p.project : '',
      render: p.render,
      snapshot: null,
      saved: Date.now(),
      recipe: {
        source,
        alongWall: Boolean(r.alongWall),
        hidden: Array.isArray(r.hidden) ? r.hidden.filter((x) => typeof x === 'string') : [],
        u: Number.isFinite(Number(r.u)) ? Math.min(1, Math.max(0, Number(r.u))) : 0,
        colors: r.colors && typeof r.colors === 'object' ? r.colors : { parts: {} },
      },
    };
    if (!have.some((h) => same(h, setup)) && !out.some((o) => same(o, setup))) out.push(setup);
  }
  return out;
}

/**
 * A batch picture as kept: which batch (its start, as in the file names), its file name, the PNG.
 * @typedef {{ key: string, batch: string, file: string, blob: Blob, at: number }} BatchPicture
 */

/** @param {BatchPicture} picture */
export async function keepBatchPicture(picture) {
  await request('readwrite', (s) => s.put(picture), PICTURES);
}

/** The pictures kept, oldest first. @returns {Promise<BatchPicture[]>} */
export async function batchPictures() {
  const all = /** @type {BatchPicture[]} */ (await request('readonly', (s) => s.getAll(), PICTURES));
  return all.sort((a, b) => a.at - b.at);
}

/** Forgets the pictures kept. */
export async function clearBatchPictures() {
  await request('readwrite', (s) => s.clear(), PICTURES);
}

/** The batch's part of a file name: "batch-2026-11-28-2205" (its start). */
export function batchStamp(started) {
  return batchFileName(started, 'x', 'rendered').replace(/-x\.png$/, '');
}

// ---- A zip (stored, not compressed: PNGs are compressed already) of files in folders.

/** CRC-32 (the zip's checksum) of bytes. */
const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
export function crc32(bytes) {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/**
 * A zip file of `files` (paths may hold folders: "batch-…/front.png"), stored.
 * @param {{ path: string, bytes: Uint8Array }[]} files
 * @param {Date} [when] the files' time
 * @returns {Uint8Array}
 */
export function zipFiles(files, when = new Date()) {
  const enc = new TextEncoder();
  const time = (when.getHours() << 11) | (when.getMinutes() << 5) | (when.getSeconds() >> 1);
  const date = ((when.getFullYear() - 1980) << 9) | ((when.getMonth() + 1) << 5) | when.getDate();
  /** @type {Uint8Array[]} */
  const parts = [];
  /** @type {Uint8Array[]} */
  const central = [];
  let offset = 0;
  for (const f of files) {
    const name = enc.encode(f.path);
    const crc = crc32(f.bytes);
    const size = f.bytes.length;
    const local = new DataView(new ArrayBuffer(30));
    // Local header: signature, version 2.0, UTF-8 names (bit 11), stored, time, date, CRC, sizes, name length.
    [[0, 0x04034b50, 4], [4, 20, 2], [6, 0x0800, 2], [8, 0, 2], [10, time, 2], [12, date, 2], [14, crc, 4], [18, size, 4], [22, size, 4], [26, name.length, 2], [28, 0, 2]].forEach(([at, v, n]) =>
      n === 4 ? local.setUint32(at, v, true) : local.setUint16(at, v, true),
    );
    parts.push(new Uint8Array(local.buffer), name, f.bytes);
    const head = new DataView(new ArrayBuffer(46));
    [[0, 0x02014b50, 4], [4, 20, 2], [6, 20, 2], [8, 0x0800, 2], [10, 0, 2], [12, time, 2], [14, date, 2], [16, crc, 4], [20, size, 4], [24, size, 4], [28, name.length, 2], [30, 0, 2], [32, 0, 2], [34, 0, 2], [36, 0, 2], [38, 0, 4], [42, offset, 4]].forEach(([at, v, n]) =>
      n === 4 ? head.setUint32(at, v, true) : head.setUint16(at, v, true),
    );
    central.push(new Uint8Array(head.buffer), name);
    offset += 30 + name.length + size;
  }
  const centralSize = central.reduce((n, b) => n + b.length, 0);
  const end = new DataView(new ArrayBuffer(22));
  [[0, 0x06054b50, 4], [4, 0, 2], [6, 0, 2], [8, files.length, 2], [10, files.length, 2], [12, centralSize, 4], [16, offset, 4], [20, 0, 2]].forEach(([at, v, n]) =>
    n === 4 ? end.setUint32(at, v, true) : end.setUint16(at, v, true),
  );
  const all = [...parts, ...central, new Uint8Array(end.buffer)];
  const out = new Uint8Array(all.reduce((n, b) => n + b.length, 0));
  let at = 0;
  for (const b of all) {
    out.set(b, at);
    at += b.length;
  }
  return out;
}
