// EasyCAD Phase 0: settings → Rust engine (WebAssembly, in a worker) → 3D view + print estimate.
// In Phase 1 the settings list below is generated from the project file instead of hard-coded.

import { createEngine } from './engine.js';
import { createViewport } from './viewport.js';

const PRINTERS = {
  p1p: { name: 'Bambu P1P', bed: 256, nozzle: 0.4, defaults: { layer: 0.3, line: 0.6, speed: 40 } },
  // Example values from EasyGCode's pellet defaults (0.6 mm layers, 2.0 mm lines, 1800 mm/min).
  pellet: { name: 'Klipper pellet', bed: 400, nozzle: null, defaults: { layer: 0.6, line: 2.0, speed: 30 } },
};

/**
 * The demo's settings. `toEngine` converts what you see into what the engine expects.
 * @type {{ group: string, key: string, label: string, unit: string, min: number, max: number,
 *          step: number, value: number, toEngine?: (v: number) => number }[]}
 */
const SETTINGS = [
  { group: 'Shape', key: 'height', label: 'Height', unit: 'mm', min: 40, max: 300, step: 1, value: 160 },
  { group: 'Shape', key: 'radius', label: 'Tip radius', unit: 'mm', min: 10, max: 150, step: 0.5, value: 45 },
  { group: 'Shape', key: 'points', label: 'Star points', unit: '', min: 3, max: 16, step: 1, value: 7 },
  { group: 'Shape', key: 'depth', label: 'Star depth', unit: '%', min: 0, max: 60, step: 1, value: 18, toEngine: (v) => v / 100 },
  { group: 'Shape', key: 'twist', label: 'Twist', unit: '°', min: -360, max: 360, step: 1, value: 90 },
  { group: 'Shape', key: 'topScale', label: 'Top scale', unit: '×', min: 0.3, max: 2, step: 0.01, value: 0.8 },
  { group: 'Print', key: 'layer', label: 'Layer height', unit: 'mm', min: 0.08, max: 3, step: 0.01, value: 0.3 },
  { group: 'Print', key: 'line', label: 'Line width', unit: 'mm', min: 0.2, max: 10, step: 0.05, value: 0.6 },
  { group: 'Print', key: 'speed', label: 'Speed', unit: 'mm/s', min: 2, max: 300, step: 1, value: 40 },
];

const STORAGE_KEY = 'easycad.phase0.settings';
const $ = (id) => /** @type {HTMLElement} */ (document.getElementById(id));

const state = {
  printer: 'p1p',
  /** @type {Record<string, number>} */
  values: Object.fromEntries(SETTINGS.map((s) => [s.key, s.value])),
  engineInfo: { version: '?', loadMs: 0 },
  lastMs: 0,
};

// ---------------------------------------------------------------------------------------------
// Settings panel
// ---------------------------------------------------------------------------------------------

function clampToSetting(s, v) {
  const n = Number(v);
  if (!Number.isFinite(n)) return s.value;
  return Math.min(s.max, Math.max(s.min, n));
}

function renderSettings() {
  const host = $('settings');
  host.innerHTML = '';
  const groups = new Map();
  for (const s of SETTINGS) {
    if (!groups.has(s.group)) {
      const g = document.createElement('div');
      g.className = 'group';
      g.innerHTML = `<h2>${s.group}</h2>`;
      groups.set(s.group, g);
      host.appendChild(g);
    }
    const row = document.createElement('div');
    row.className = 'setting';
    row.innerHTML = `
      <label for="n-${s.key}">${s.label}</label>
      <span class="num"><input type="number" id="n-${s.key}" inputmode="decimal" min="${s.min}" max="${s.max}" step="${s.step}"><span class="unit">${s.unit}</span></span>
      <input type="range" id="r-${s.key}" min="${s.min}" max="${s.max}" step="${s.step}" aria-label="${s.label}">`;
    groups.get(s.group).appendChild(row);

    const num = /** @type {HTMLInputElement} */ (row.querySelector(`#n-${s.key}`));
    const range = /** @type {HTMLInputElement} */ (row.querySelector(`#r-${s.key}`));
    // Dragging a slider asks for quick previews; letting go asks for the full-quality result.
    range.addEventListener('input', () => setValue(s.key, range.value, 'preview'));
    range.addEventListener('change', () => setValue(s.key, range.value, 'final'));
    num.addEventListener('change', () => setValue(s.key, num.value, 'final'));
  }
  syncInputs();
}

function syncInputs() {
  for (const s of SETTINGS) {
    const v = state.values[s.key];
    /** @type {HTMLInputElement} */ ($(`n-${s.key}`)).value = String(v);
    /** @type {HTMLInputElement} */ ($(`r-${s.key}`)).value = String(v);
  }
  /** @type {HTMLSelectElement} */ ($('printer')).value = state.printer;
}

function setValue(key, raw, quality) {
  const s = SETTINGS.find((x) => x.key === key);
  if (!s) return;
  state.values[key] = clampToSetting(s, raw);
  /** @type {HTMLInputElement} */ ($(`n-${key}`)).value = String(state.values[key]);
  if (quality === 'final') /** @type {HTMLInputElement} */ ($(`r-${key}`)).value = String(state.values[key]);
  save();
  compute(quality);
}

// ---------------------------------------------------------------------------------------------
// Persistence: remembered on this device, plus "Save/Load settings" files (like EasyGCode).
// ---------------------------------------------------------------------------------------------

function save() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ printer: state.printer, values: state.values }));
  } catch {
    // Storage can be unavailable (private browsing); the app works without it.
  }
}

/** @param {any} data */
function applySaved(data) {
  if (!data || typeof data !== 'object') throw new Error('This file does not contain EasyCAD settings.');
  if (typeof data.printer === 'string' && data.printer in PRINTERS) state.printer = data.printer;
  const values = data.values && typeof data.values === 'object' ? data.values : {};
  for (const s of SETTINGS) {
    if (s.key in values) state.values[s.key] = clampToSetting(s, values[s.key]);
  }
}

function restore() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) applySaved(JSON.parse(raw));
  } catch {
    // Ignore unreadable storage and start from the defaults.
  }
}

function downloadSettings() {
  const data = { app: 'EasyCAD', kind: 'settings', version: 1, demo: 'twisted-vase', printer: state.printer, values: state.values };
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'easycad-settings.json';
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

function showSettingsError(message) {
  const el = $('settingsError');
  el.textContent = message;
  el.hidden = !message;
}

// ---------------------------------------------------------------------------------------------
// Engine → view and stats
// ---------------------------------------------------------------------------------------------

const engine = createEngine();
const viewport = createViewport(/** @type {HTMLCanvasElement} */ ($('view')));
let refitNext = false;

function engineParams(quality) {
  const p = { quality };
  for (const s of SETTINGS) p[s.key] = s.toEngine ? s.toEngine(state.values[s.key]) : state.values[s.key];
  return p;
}

async function compute(quality) {
  try {
    const result = await engine.vase(engineParams(quality));
    if (!result) return; // a newer request replaced this one
    state.lastMs = result.ms;
    viewport.show(result, {
      radius: state.values.radius * Math.max(1, state.values.topScale),
      height: state.values.height,
      refit: refitNext,
    });
    refitNext = false;
    renderStats(result.stats);
    renderHud(result.stats, result.ms);
    renderAbout();
    showSettingsError('');
    setStatus(`Engine ready · ${result.ms.toFixed(0)} ms`, 'ok');
  } catch (err) {
    showSettingsError(String(err.message || err));
    setStatus('Engine error', 'bad');
  }
}

function setStatus(text, kind) {
  const el = $('status');
  el.textContent = text;
  el.className = `status ${kind || ''}`;
}

const fmt = (n, digits = 0) => n.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });

function duration(seconds) {
  const m = Math.round(seconds / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')}`;
}

function renderStats(st) {
  const printer = PRINTERS[state.printer];
  $('stats').innerHTML = [
    [fmt(st.revolutions), 'layers, one continuous spiral'],
    [`${fmt(st.printedHeightMm, 1)} mm`, 'printed height (whole layers)'],
    [duration(st.timeS), `at ${fmt(state.values.speed)} mm/s`],
    [`${fmt(st.massGPla)} g`, `PLA · ${fmt(st.volumeMm3 / 1000, 1)} cm³`],
    [`${fmt(st.pathLengthMm / 1000, 1)} m`, 'toolpath length'],
    [`${fmt(st.flowMm3S, 1)} mm³/s`, 'volumetric flow'],
  ]
    .map(([b, s]) => `<div class="stat"><b>${b}</b><span>${s}</span></div>`)
    .join('');

  const overhang = st.maxOverhangDeg;
  const diameter = 2 * state.values.radius * Math.max(1, state.values.topScale);
  const checks = [
    [overhang < 40 ? 'ok' : overhang < 55 ? 'warn' : 'bad', `Steepest overhang ${fmt(overhang, 1)}° (under ~45° prints cleanly)`],
    [diameter <= printer.bed - 10 ? 'ok' : 'bad', `About ${fmt(diameter)} mm wide on a ${printer.bed} mm bed`],
  ];
  if (printer.nozzle) {
    const ratio = state.values.line / printer.nozzle;
    checks.push([ratio >= 1 && ratio <= 2 ? 'ok' : 'warn', `Line width is ${fmt(ratio, 1)}× the ${printer.nozzle} mm nozzle`]);
  }
  $('checks').innerHTML = checks
    .map(([kind, text]) => `<li><span class="dot ${kind === 'ok' ? '' : kind}"></span><span>${text}</span></li>`)
    .join('');
}

function renderHud(st, ms) {
  $('hud').textContent = `${fmt(st.revolutions)} layers · ${fmt(st.pathPoints)} path points · ${fmt(ms)} ms`;
}

function renderAbout() {
  const rows = [
    ['Engine', `Rust → WebAssembly, v${state.engineInfo.version}`],
    ['Engine start', `${fmt(state.engineInfo.loadMs)} ms`],
    ['Last evaluation', `${fmt(state.lastMs, 1)} ms (in a background worker)`],
    ['3D view', viewport.rendererInfo()],
    ['CPU threads', String(navigator.hardwareConcurrency || '?')],
    ['Multi-thread ready', self.crossOriginIsolated ? 'yes' : 'not yet (needs isolation headers)'],
    ['Docs', '<a href="https://github.com/jasperkrebs/easycad/blob/claude/quirky-gates-1ykve2/docs/roadmap.md" target="_blank" rel="noopener">Roadmap</a>'],
  ];
  $('about').innerHTML = rows.map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join('');
}

// ---------------------------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------------------------

function selectTab(name) {
  for (const t of ['settings', 'print', 'about']) {
    $(`tab-${t}`).setAttribute('aria-selected', String(t === name));
    $(`panel-${t}`).hidden = t !== name;
  }
}

function wire() {
  for (const t of ['settings', 'print', 'about']) $(`tab-${t}`).addEventListener('click', () => selectTab(t));

  document.querySelectorAll('[data-mode]').forEach((b) =>
    b.addEventListener('click', () => {
      const mode = /** @type {HTMLElement} */ (b).dataset.mode;
      document.querySelectorAll('[data-mode]').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
      viewport.setMode(/** @type {any} */ (mode));
    }),
  );
  $('view').addEventListener('dblclick', () => {
    viewport.fit(state.values.radius * Math.max(1, state.values.topScale), state.values.height);
  });

  $('printer').addEventListener('change', (e) => {
    state.printer = /** @type {HTMLSelectElement} */ (e.target).value;
    const p = PRINTERS[state.printer];
    Object.assign(state.values, p.defaults);
    viewport.setBed(p.bed);
    syncInputs();
    save();
    compute('final');
  });

  $('saveSettings').addEventListener('click', downloadSettings);
  $('loadSettings').addEventListener('click', () => $('settingsFile').click());
  $('settingsFile').addEventListener('change', async (e) => {
    const input = /** @type {HTMLInputElement} */ (e.target);
    const file = input.files && input.files[0];
    input.value = '';
    if (!file) return;
    try {
      applySaved(JSON.parse(await file.text()));
      viewport.setBed(PRINTERS[state.printer].bed);
      syncInputs();
      save();
      refitNext = true;
      compute('final');
    } catch (err) {
      showSettingsError(`Could not load settings: ${err.message || err}`);
    }
  });
  $('resetSettings').addEventListener('click', () => {
    state.printer = 'p1p';
    for (const s of SETTINGS) state.values[s.key] = s.value;
    viewport.setBed(PRINTERS.p1p.bed);
    syncInputs();
    save();
    refitNext = true;
    compute('final');
  });
}

async function start() {
  restore();
  renderSettings();
  wire();
  viewport.setBed(PRINTERS[state.printer].bed);
  renderAbout();
  try {
    state.engineInfo = await engine.ready;
    await compute('final');
  } catch (err) {
    setStatus('Engine failed to start', 'bad');
    const msg = $('viewMessage');
    msg.hidden = false;
    msg.textContent = `The geometry engine could not start: ${err.message || err}`;
  }
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('./sw.js', { updateViaCache: 'none' }).catch(() => {});
  }
}

start();
