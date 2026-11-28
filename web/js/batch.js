// The Render's batch: setups saved one by one (a name, the render's settings with the camera,
// and the project as prepared: its beads, the parts' looks, the fittings), then rendered one
// after another, each picture downloaded as rendered and denoised. Kept in the browser's
// IndexedDB (the beads are too big for localStorage), so a batch survives a reload. The file
// names and the list's lines are pure helpers, for the node tests.

const DB = 'easycad-batch';
const STORE = 'setups';

/**
 * @typedef {{
 *   id: number,
 *   name: string,
 *   project: string,
 *   render: any,
 *   snapshot: any,
 *   saved: number,
 *   recipe?: { source: string, alongWall: boolean, hidden: string[], u: number, colors: any },
 * }} BatchSetup
 * `snapshot`: the beads as saved (on this computer); `recipe`: what makes them (the project
 * file's plan carries only this), with the parts' looks and the way it hangs.
 */

/** @returns {Promise<IDBDatabase>} */
function open() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE, { keyPath: 'id' });
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/**
 * One request in a transaction, its result.
 * @param {'readonly' | 'readwrite'} mode
 * @param {(store: IDBObjectStore) => IDBRequest} make
 */
async function request(mode, make) {
  const db = await open();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(STORE, mode);
      const req = make(tx.objectStore(STORE));
      tx.oncomplete = () => resolve(req.result);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** The saved setups, in the order they were saved. @returns {Promise<BatchSetup[]>} */
export async function batchSetups() {
  const all = /** @type {BatchSetup[]} */ (await request('readonly', (s) => s.getAll()));
  return all.sort((a, b) => a.id - b.id);
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
