// EasyCAD: a recipe (.easycad text) → Rust engine (WebAssembly, in a worker) → 3D view, a settings
// window generated from the recipe, a print estimate, and files to save (G-code, STL).
//
// The recipe text is the project and the single source of truth: moving a slider writes the new
// value into the text (the engine does that, see crates/wasm), and saving a project saves exactly
// that text.

import { createEngine } from './engine.js';
import { createViewport } from './viewport.js';
import { $, copyText, download, el, fmt, load, loadJSON, parseNumber, save, saveJSON, slug } from './dom.js';
import { layoutKey, renderSettings } from './settings.js';
import { fileNote, fingerprint, projectOf, withPartsOf, withoutFileNote, withoutValues } from './templates.js';
import { PRINTERS, applyMaterial, changeSettings, describePrinter, loadProfile, printerFor, renderPrint, renderPrinterSettings } from './print.js';
import { aiPrompt, renderProblems, renderSteps, selectLine } from './recipe.js';
import { createChat } from './chat.js';
import { askNewPart, newPartName, renderPlan } from './parts.js';
import { beadExtremes, renderBeads } from './beads.js';

const TEMPLATES = [
  { id: 'lampshade', name: 'Lampshade' },
  { id: 'lamp', name: 'Lamp' },
  { id: 'vessel', name: 'Vessel' },
  { id: 'salt-shaker', name: 'Salt & pepper shaker' },
  { id: 'bend-stool', name: 'Bend stool' },
  { id: 'thread-connector', name: 'Thread connector' },
  { id: 'spiral-disc', name: 'Spiral disc' },
];
const STORAGE_KEY = 'easycad.project';
/** Parallel or perspective 3D view. */
const VIEW_KEY = 'easycad.view';
/** The toolpath as lines or as solid beads. */
const BEADS_KEY = 'easycad.beads';
/** Whether the beads are simulated (how the plastic actually lies). */
const SIM_KEY = 'easycad.simulate';
/** How solid the beads are drawn (10–100 %). */
const OPACITY_KEY = 'easycad.beadOpacity';
/** Which way the nozzle and beads stand: upright, tilted (≤45°) or along the wall. */
const NOZZLE_KEY = 'easycad.nozzle';
/** Custom colours (the "Custom" colour mode): per project, each part's colour and opacity, and the light. */
const CUSTOM_KEY = 'easycad.customColors';
/** What the toolpath is coloured by. */
const COLOR_KEY = 'easycad.colorMode';
/** How fast the print plays back. */
const SPEED_KEY = 'easycad.playSpeed';
const MAX_FILE_BYTES = 256 * 1024;
const TABS = ['settings', 'chat', 'recipe', 'print', 'about'];

const state = {
  /** The recipe text: the project. */
  source: '',
  /** Whether the beads are simulated (how the plastic actually lies): full-quality runs ask
   * the engine for it. */
  simulate: false,
  /** Setting values as last opened, for "Reset settings". */
  openedValues: /** @type {Record<string, any>} */ ({}),
  /** Current setting values (each in its own unit); written into the text on every run. */
  values: /** @type {Record<string, any>} */ ({}),
  /** The settings window's layout, so it's rebuilt only when the recipe's settings change. */
  layout: '',
  printer: /** @type {keyof typeof PRINTERS} */ ('p1p'),
  /** The printer's machine settings (print.js); G-code is written for these. */
  profile: loadProfile('p1p'),
  /** Changed since it was opened or saved (asks before a template replaces it). */
  unsaved: false,
  /** @type {import('./engine.js').RunResult | null} */
  last: null,
  engineInfo: { version: '?', loadMs: 0 },
  catalog: '',
  /**
   * The template the project was started from, and that template's text as it was then (values
   * left out), to tell when the template has been improved since.
   * @type {{ id: string, base: string } | null}
   */
  template: null,
};
const SEEN_KEY = 'easycad.templateSeen';

const engine = createEngine();
const viewport = createViewport(/** @type {HTMLCanvasElement} */ ($('view')));
const editor = /** @type {HTMLTextAreaElement} */ ($('recipe'));
let refitNext = true;
// ---------------------------------------------------------------------------------------------
// Running the recipe
// ---------------------------------------------------------------------------------------------

/**
 * Runs the recipe.
 * @param {'preview' | 'final'} quality
 * @param {{ fresh?: boolean, opened?: boolean }} [how] fresh: the text itself changed (values come
 *   from the text, not the sliders); opened: a new project was opened (also frames the view)
 */
async function run(quality, how = {}) {
  if (how.fresh) state.values = {};
  if (how.opened) refitNext = true;
  let result;
  try {
    // The simulation only on full-quality runs: a slider being dragged stays quick.
    const simulate = state.simulate && quality === 'final' ? state.profile.multiplier || 1 : null;
    result = await engine.evaluate(state.source, state.values, quality, simulate);
  } catch (err) {
    setStatus('Engine error', 'bad');
    showSettingsError(String(err.message || err));
    return null;
  }
  if (!result) return null; // a newer run replaced this one
  state.last = result;
  state.source = result.source;
  for (const p of result.params) if (!(p.name in state.values)) state.values[p.name] = p.value;
  if (how.opened) state.openedValues = { ...state.values };
  // The recipe's `printer` line decides the printer (choosing one in the Print tab rewrites it).
  if (result.printer && printerFor(result.printer) !== state.printer) setPrinter(printerFor(result.printer));

  const errors = result.diagnostics.filter((d) => d.severity === 'error').length;
  // The ports there are (with room) change what a part can be mounted on.
  const ports = (result.ports || []).map((p) => `${p.name}:${p.value[3] > 0}`).join();
  const layout = layoutKey(result.params) + ports;
  if (how.fresh || layout !== state.layout) {
    state.layout = layout;
    renderSettings($('settings'), result.params, state.values, onSettingChange, {
      broken: errors > 0,
      ports: result.ports,
      components: result.components,
      onAddPart: () => addPart(),
      onRemovePart: (name) => removePart(name),
    });
  }
  viewport.show(result, { refit: refitNext });
  refitNext = false;
  syncScrub();
  renderLegend();
  renderPartToggles(result);
  renderCustomColors();
  renderPlanView(result);

  renderProject(result);
  renderPrintTab();
  renderExports(result);
  renderProblems($('problems'), result.diagnostics, (line) => selectLine(editor, line));
  renderSteps($('steps'), result.steps);
  showSource(result.source);
  renderHud(result);
  renderAbout();

  showSettingsError(errors ? `The recipe has ${errors === 1 ? 'a problem' : `${errors} problems`}: see the Recipe tab.` : '');
  if (errors) setStatus(`${errors} recipe ${errors === 1 ? 'problem' : 'problems'}`, 'bad');
  else setStatus(`Engine ready · ${fmt(result.ms)} ms`, 'ok');
  autosave();
  return result;
}

function onSettingChange(name, value, quality) {
  state.values[name] = value;
  state.unsaved = true;
  run(quality);
}

// ---------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------

function renderProject(result) {
  const name = result.project || 'Untitled project';
  $('projectSub').textContent = name;
  $('projectName').textContent = name;
  document.title = `${name} · EasyCAD`;
  /** @type {HTMLInputElement} */ ($('saveName')).placeholder = slug(name);
}

function renderHud(result) {
  const prints = printsOf(result);
  const st = prints[0]?.stats;
  const parts = !st
    ? ['no toolpath']
    : prints.length > 1
      ? [`${prints.length} parts`, `${fmt(prints.reduce((n, p) => n + p.stats.pathPoints, 0))} path points`]
      : [`${fmt(st.layers)} ${st.layers === 1 ? 'layer' : 'layers'}`, `${fmt(st.pathPoints)} path points`];
  hudText = [...parts, `${fmt(result.ms)} ms`].join(' · ');
  $('hud').textContent = hudText;
  const empty = !result.mesh.indices.length && result.curves.starts.length < 2 && result.paths.starts.length < 2;
  const msg = $('viewMessage');
  msg.hidden = !empty;
  msg.textContent = result.diagnostics.some((d) => d.severity === 'error')
    ? 'Nothing to draw: the recipe has a problem (see the Recipe tab).'
    : 'Nothing to draw yet: add a line like  show name  to the recipe.';
}

/** Printed parts hidden from the 3D view, by name (they stay hidden while the recipe changes). */
const hiddenParts = new Set();

/**
 * The 3D view's part toggles, like layers: one per printed part when there are several, each
 * showing or hiding that part's toolpath and the shapes only it is made from.
 */
function renderPartToggles(result) {
  const prints = printsOf(result);
  const host = $('viewParts');
  host.hidden = prints.length < 2;
  host.replaceChildren(
    ...prints.map((p) => {
      const shown = !hiddenParts.has(p.name);
      const b = el('button', { type: 'button', 'aria-pressed': String(shown), title: `${shown ? 'Hide' : 'Show'} ${p.name}`, text: p.name });
      b.addEventListener('click', () => {
        if (hiddenParts.has(p.name)) hiddenParts.delete(p.name);
        else hiddenParts.add(p.name);
        renderPartToggles(state.last);
      });
      return b;
    }),
  );
  viewport.setHiddenParts(prints.flatMap((p, i) => (hiddenParts.has(p.name) ? [i] : [])));
}

/** The parts the recipe prints (`print` lines), each with its statistics. */
function printsOf(result) {
  if (!result) return [];
  return result.prints?.length ? result.prints : result.print ? [result.print] : [];
}

function renderPrintTab() {
  renderPrint($('stats'), $('checks'), printsOf(state.last), state.printer, state.profile);
  renderBeads($('beads'), beadGroups(state.last));
  renderSimulationChecks();
}

/** What the bead simulation found, as print checks (while it's on). */
function renderSimulationChecks() {
  const sim = state.simulate ? state.last?.simulation : null;
  if (!sim || !sim.moves) return;
  const pct = (n) => `${fmt((100 * n) / sim.moves, n * 100 < sim.moves ? 1 : 0)} %`;
  const items = [];
  if (sim.wide) items.push(['warn', `Simulated: ${fmt(sim.wide)} moves (${pct(sim.wide)}) come out over 1.5× their planned width, up to ${fmt(sim.maxRatio, 1)}×: the layer below is too close for their plastic (colour by Squish to see where)`]);
  if (sim.unsupported) items.push(['warn', `Simulated: ${fmt(sim.unsupported)} moves (${pct(sim.unsupported)}) have nothing under them for well over a layer: they hang (a bridge or a steep overhang)`]);
  if (sim.collisions) items.push(['bad', `Simulated: at ${fmt(sim.collisions)} moves (${pct(sim.collisions)}) the nozzle runs into plastic already printed`]);
  if (!items.length) items.push(['ok', `Simulated: every bead lies as planned (${fmt(sim.minRatio, 2)}–${fmt(sim.maxRatio, 2)}× its planned width)`]);
  const host = $('checks');
  for (const [kind, text] of items) host.append(el('li', {}, [el('span', { className: `dot ${kind === 'ok' ? '' : kind}` }), el('span', { text })]));
}

/** Simulates the beads (how the plastic actually lies), or stops; runs the recipe again. */
function setSimulate(on) {
  state.simulate = on;
  /** @type {HTMLElement} */ ($('viewSim')).setAttribute('aria-pressed', String(on));
  save(SIM_KEY, on ? 'on' : 'off');
  viewport.setSimulated(on);
  if (on && !state.last?.simulation) run('final');
  else renderPrintTab();
}

/** Each printed part's beads (or, with no parts named, all the toolpaths'), for the Print tab. */
function beadGroups(result) {
  const paths = result?.paths;
  if (!paths?.widths || !paths?.heights) return [];
  const prints = printsOf(result);
  const owner = result.parts?.paths || new Int32Array(0);
  /** @type {Map<number, { widths: number[], heights: number[] }>} */
  const byPart = new Map();
  for (let i = 0; i + 1 < paths.starts.length; i++) {
    const part = prints.length > 1 ? (owner[i] ?? -1) : 0;
    const into = byPart.get(part) || { widths: [], heights: [] };
    for (let k = paths.starts[i]; k < paths.starts[i + 1]; k++) {
      into.widths.push(paths.widths[k]);
      into.heights.push(paths.heights[k]);
    }
    byPart.set(part, into);
  }
  return [...byPart.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([part, b]) => ({ name: prints[part]?.name || 'other', ...beadExtremes(b.widths, b.heights) }));
}

/** Buttons for the files the recipe makes: G-code of what it prints, and its `export` lines. */
function renderExports(result) {
  const host = $('exportButtons');
  host.replaceChildren();
  // A broken recipe has nothing to export (the problem is explained above).
  const broken = result.diagnostics.some((d) => d.severity === 'error');
  const prints = printsOf(result);
  $('exportEmpty').hidden = broken || Boolean(prints.length || result.exports.length);
  if (broken) return;
  // One G-code file per part: the project's name, and the part's when there are several.
  prints.forEach((part, i) => {
    const several = prints.length > 1;
    const text = `G-code for the ${PRINTERS[state.printer].name}${several ? `: ${part.name}` : ''}`;
    const b = el('button', { className: 'btn primary', type: 'button', text });
    const name = `${slug(projectName())}${several ? `-${slug(part.name)}` : ''}.gcode`;
    b.addEventListener('click', () => saveFile(i ? `print:${i}` : 'print', name, true));
    host.append(b);
  });
  result.exports.forEach((x, i) => {
    const b = el('button', { className: 'btn', type: 'button', text: x.file });
    b.addEventListener('click', () => saveFile(String(i), x.file));
    host.append(b);
  });
}

/**
 * Makes a file in the engine and offers it as a download.
 * @param {string} which 'print', 'print:1' (a part) or an export's index
 * @param {string} fallback the file name when the engine doesn't give one
 * @param {boolean} [ours] always use `fallback` (G-code of a print, named after the project)
 */
async function saveFile(which, fallback, ours = false) {
  exportNote('');
  try {
    const file = await engine.exportFile(state.source, state.values, which, state.profile);
    const name = (!ours && file.name) || fallback;
    download(name, file.data, 'application/octet-stream');
    exportNote(`Saved ${name} (${fmt(file.data.length / 1024, 1)} kB).`);
  } catch (err) {
    exportNote(`Couldn't make the file: ${err.message || err}.`, true);
  }
}

function exportNote(text, bad = false) {
  const n = $('exportNote');
  n.textContent = text;
  n.className = bad ? 'error' : 'hint';
  n.hidden = !text;
}

function renderAbout() {
  const steps = (state.catalog.match(/^\w+\(/gm) || []).length;
  const rows = [
    ['Engine', `Rust → WebAssembly, v${state.engineInfo.version}`],
    ['Engine start', `${fmt(state.engineInfo.loadMs)} ms`],
    ['Last run', state.last ? `${fmt(state.last.ms, 1)} ms (in a background worker)` : '…'],
    ['Recipe language', `easycad 0.1 · ${steps || '…'} steps`],
    ['G-code', 'EasyGCode’s dialect, golden-tested against it'],
    ['3D view', viewport.rendererInfo()],
    ['CPU threads', String(navigator.hardwareConcurrency || '?')],
    ['Multi-thread ready', self.crossOriginIsolated ? 'yes' : 'not yet (needs isolation headers)'],
  ];
  const about = $('about');
  about.replaceChildren(...rows.flatMap(([k, v]) => [el('dt', { text: k }), el('dd', { text: v })]));
  const link = el('a', {
    href: 'https://github.com/jasperkrebs/easycad/blob/main/docs/roadmap.md',
    target: '_blank',
    rel: 'noopener',
    text: 'Roadmap',
  });
  about.append(el('dt', { text: 'Docs' }), el('dd', {}, [link]));
}

/** The colour legend under the 3D view: what the toolpath is coloured by, low to high. Its
 * ends can be typed in (the full colour scale then runs between them); empty is automatic. */
function renderLegend() {
  const legend = viewport.legend();
  const box = $('viewLegend');
  // The custom colours have their own controls instead.
  box.hidden = !state.last?.paths?.starts || state.last.paths.starts.length < 2 || legend.mode === 'custom';
  $('legendLabel').textContent = `${legend.label} (${legend.unit})`;
  $('legendBar').style.background = `linear-gradient(to right, ${legend.colors.join(', ')})`;
  for (const [id, v] of [
    ['legendMin', legend.min],
    ['legendMax', legend.max],
  ]) {
    const input = /** @type {HTMLInputElement} */ ($(id));
    if (document.activeElement !== input) input.value = fmt(v, legend.digits);
  }
  $('legendAuto').hidden = !legend.set;
}

/** Colours for parts with none chosen yet (the plan's, in order). */
const PART_COLORS = ['#4f9dff', '#2bd9a0', '#ffb454', '#b58cff', '#3fd0d4', '#ff7ab6'];

/** This project's custom colours: `{ parts: { [name]: { color, opacity } }, light, shadow }`. */
function customColors() {
  const all = loadJSON(CUSTOM_KEY, {});
  const mine = (all && typeof all === 'object' && all[projectName()]) || {};
  return { parts: mine.parts || {}, light: mine.light ?? 0.5, shadow: mine.shadow ?? 0.5 };
}

function saveCustomColors(colors) {
  const all = loadJSON(CUSTOM_KEY, {});
  saveJSON(CUSTOM_KEY, { ...(all && typeof all === 'object' ? all : {}), [projectName()]: colors });
}

/** A part's custom colour and opacity (a colour of its own, solid, until chosen). */
function partLook(colors, name, i) {
  return colors.parts[name] || { color: PART_COLORS[i % PART_COLORS.length], opacity: 1 };
}

/** Hands the custom colours to the view, by the parts' places among the prints. */
function applyCustomColors() {
  const colors = customColors();
  viewport.setCustomColors({ parts: printsOf(state.last).map((p, i) => partLook(colors, p.name, i)), light: colors.light, shadow: colors.shadow });
}

/** The custom colours' controls (colour mode "Custom"): each part's colour and opacity, the light. */
let customLayout = '';
function renderCustomColors() {
  const host = $('viewColors');
  const on = /** @type {HTMLSelectElement} */ ($('viewColor')).value === 'custom';
  host.hidden = !on;
  applyCustomColors();
  if (!on) return;
  const prints = printsOf(state.last);
  const layout = `${projectName()}|${prints.map((p) => p.name).join()}`;
  if (layout === customLayout && host.childElementCount) return;
  customLayout = layout;
  const colors = customColors();
  const change = (update) => {
    const next = customColors();
    update(next);
    saveCustomColors(next);
    applyCustomColors();
    if (state.last && !$('plan').hidden) renderPlanView(state.last);
  };
  const slider = (label, value, onInput) => {
    const input = /** @type {HTMLInputElement} */ (el('input', { type: 'range', min: '0', max: '100', step: '1', value: String(Math.round(value * 100)), 'aria-label': label, title: label }));
    input.addEventListener('input', () => onInput(Number(input.value) / 100));
    return input;
  };
  host.replaceChildren(
    ...prints.map((p, i) => {
      const look = partLook(colors, p.name, i);
      const color = /** @type {HTMLInputElement} */ (el('input', { type: 'color', value: look.color, 'aria-label': `${p.name}: colour`, title: `${p.name}: colour` }));
      color.addEventListener('input', () => change((c) => (c.parts[p.name] = { ...partLook(c, p.name, i), color: color.value })));
      const solid = slider(`${p.name}: see-through ↔ solid`, look.opacity, (v) => change((c) => (c.parts[p.name] = { ...partLook(c, p.name, i), opacity: Math.max(0.05, v) })));
      return el('div', { className: 'color-row' }, [el('span', { className: 'color-name', text: p.name }), color, solid]);
    }),
    el('div', { className: 'color-row' }, [el('span', { className: 'color-name', text: 'Highlights' }), slider('Highlights: how much white', colors.light, (v) => change((c) => (c.light = v)))]),
    el('div', { className: 'color-row' }, [el('span', { className: 'color-name', text: 'Shadows' }), slider('Shadows: how much black', colors.shadow, (v) => change((c) => (c.shadow = v)))]),
  );
}

/** Applies colour range ends typed into the legend (empty or unreadable: automatic). */
function setLegendRange() {
  const legend = viewport.legend();
  const read = (id, auto) => {
    const v = parseNumber(/** @type {HTMLInputElement} */ ($(id)).value);
    return Number.isFinite(v) && Math.abs(v - auto) > 1e-9 ? v : null;
  };
  viewport.setColorRange(legend.mode, read('legendMin', legend.auto[0]), read('legendMax', legend.auto[1]));
  renderLegend();
}

/** The stats line under the 3D view, while the whole print is shown. */
let hudText = '';
/** The part of the print shown, as fractions of its moves: from, to (0 and 1: all of it). */
let scrubFrom = 0;
let scrubAt = 1;

/** When each move of the toolpaths is reached, in seconds (made when first needed). */
let moveTimes = /** @type {Float64Array | null} */ (null);
const timesOf = () => (moveTimes ||= viewport.moveTimes(state.profile.travelSpeed || 100));

/** "1 h 05 min", "12 min 30 s", "40 s". */
function duration(s) {
  const t = Math.round(s);
  if (t >= 3600) return `${Math.floor(t / 3600)} h ${String(Math.floor((t % 3600) / 60)).padStart(2, '0')} min`;
  if (t >= 60) return `${Math.floor(t / 60)} min ${String(t % 60).padStart(2, '0')} s`;
  return `${t} s`;
}

/** Playing the print back: the animation frame, or 0 while it isn't playing. */
let playing = 0;
/** While playing, how much of the move after the end handle is done (0..1): the nozzle glides
 * along each move instead of waiting at its start and jumping to its end. */
let playPart = 0;

function stopPlaying() {
  if (playing) cancelAnimationFrame(playing);
  playing = 0;
  playPart = 0;
  $('scrubPlay').textContent = '▶';
  $('scrubPlay').setAttribute('aria-label', 'Play the print');
}

/**
 * Plays the print back like a video: the end handle moves at the chosen multiple of the real
 * print time (each move at its own speed), from the start handle, or on from where it is.
 */
function play() {
  if (playing) return stopPlaying();
  const lo = /** @type {HTMLInputElement} */ ($('scrubFrom'));
  const hi = /** @type {HTMLInputElement} */ ($('scrub'));
  const max = Number(hi.max) || 1;
  if (max < 2) return;
  const times = timesOf();
  if (Number(hi.value) >= max) hi.value = lo.value;
  let t = times[Math.min(times.length - 1, Number(hi.value))] || 0;
  let before = performance.now();
  $('scrubPlay').textContent = '❚❚';
  $('scrubPlay').setAttribute('aria-label', 'Pause');
  const frame = (now) => {
    t += ((now - before) / 1000) * Number(/** @type {HTMLSelectElement} */ ($('scrubSpeed')).value);
    before = now;
    // The last move reached by now (binary search over the times).
    let [a, b] = [0, times.length - 1];
    while (a < b) {
      const m = (a + b + 1) >> 1;
      if (times[m] <= t) a = m;
      else b = m - 1;
    }
    hi.value = String(Math.min(max, a + 1));
    const span = (times[a + 1] ?? times[a]) - times[a];
    playPart = span > 0 ? Math.min(1, (t - times[a]) / span) : 0;
    showProgress();
    if (a + 1 >= max) {
      stopPlaying();
      return;
    }
    playing = requestAnimationFrame(frame);
  };
  playing = requestAnimationFrame(frame);
}

/** The progress slider: one step per move of the toolpaths, kept at the same fractions. */
function syncScrub() {
  moveTimes = null;
  const total = Math.max(1, viewport.pathPoints);
  for (const [id, f] of [
    ['scrubFrom', scrubFrom],
    ['scrub', scrubAt],
  ]) {
    const slider = /** @type {HTMLInputElement} */ ($(id));
    slider.max = String(total);
    slider.value = String(Math.round(f * total));
    slider.disabled = viewport.pathPoints < 2;
  }
  showProgress();
}

/**
 * Shows the print between the slider's two handles. Moving the end handle back shows a nozzle
 * where it's got to, and that move's height, speed, flow and bead in the stats line; moving the
 * start handle on hides what's printed before (to look at the top layers on their own).
 * @param {Event} [e] the input that moved: it pushes the other handle along rather than cross it
 */
function showProgress(e) {
  const lo = /** @type {HTMLInputElement} */ ($('scrubFrom'));
  const hi = /** @type {HTMLInputElement} */ ($('scrub'));
  const max = Number(hi.max) || 1;
  if (Number(lo.value) > Number(hi.value)) {
    if (e?.target === lo) hi.value = lo.value;
    else lo.value = hi.value;
  }
  scrubFrom = Number(lo.value) / max;
  scrubAt = Number(hi.value) / max;
  const range = /** @type {HTMLElement} */ ($('scrubRange'));
  range.style.setProperty('--lo', `${scrubFrom * 100}%`);
  range.style.setProperty('--hi', `${scrubAt * 100}%`);
  const at = viewport.setProgress(scrubAt, scrubFrom, playing ? playPart : 0);
  let part = scrubFrom > 0 || scrubAt < 1 ? `${fmt(scrubFrom * 100)}–${fmt(scrubAt * 100)} %` : '';
  if (at) {
    const times = timesOf();
    const i = Math.max(0, Math.min(times.length - 1, Number(hi.value) - 1));
    const now = times[i] + (playing ? playPart * ((times[i + 1] ?? times[i]) - times[i]) : 0);
    part += ` · ${duration(now)} of ${duration(times[times.length - 1] || 0)}`;
  }
  $('hud').textContent = at
    ? at.travel
      ? `${part} · Z ${fmt(at.z, 2)} mm · travel`
      : `${part} · Z ${fmt(at.z, 2)} mm · ${fmt(at.speed, 1)} mm/s · ${fmt(at.flow, 1)} mm³/s · ${fmt(at.width, 2)} × ${fmt(at.layer, 2)} mm` +
        (state.simulate && at.simWidth ? ` (lies ${fmt(at.simWidth, 2)} × ${fmt(at.simLayer, 2)})` : '')
    : part || hudText;
}

function setStatus(text, kind) {
  const s = $('status');
  s.textContent = text;
  s.className = `status ${kind || ''}`;
}

function showSettingsError(message) {
  const e = $('settingsError');
  e.textContent = message;
  e.hidden = !message;
}

// ---------------------------------------------------------------------------------------------
// The recipe editor
// ---------------------------------------------------------------------------------------------

let editorDirty = false;

/** Shows the latest recipe text, unless you're in the middle of editing it. */
function showSource(text) {
  if (!editorDirty && editor.value !== text) editor.value = text;
}

function setDirty(dirty) {
  editorDirty = dirty;
  $('recipeDirty').hidden = !dirty;
}

function runEditor() {
  state.source = editor.value;
  state.unsaved = true;
  setDirty(false);
  run('final', { fresh: true });
}

function note(text) {
  const n = $('copyNote');
  n.textContent = text;
  n.hidden = false;
  setTimeout(() => (n.hidden = true), 6000);
}

// ---------------------------------------------------------------------------------------------
// The agent (Chat tab): it changes the project only through the recipe text, like everything else
// ---------------------------------------------------------------------------------------------

/** Replaces the recipe (an agent edit), runs it at full quality and returns the result. */
async function applyRecipe(text) {
  state.source = text;
  state.unsaved = true;
  setDirty(false);
  editor.value = text;
  await run('final', { fresh: true });
  await engine.idle();
  return state.last;
}

/** Moves settings (an agent edit), runs and returns the result. */
async function applyValues(values) {
  Object.assign(state.values, values);
  state.unsaved = true;
  state.layout = ''; // rebuild the window so the sliders show the new values
  await run('final');
  await engine.idle();
  return state.last;
}

const chat = createChat({
  project: () => ({
    source: state.source,
    last: state.last,
    printer: describePrinter(state.printer, state.profile),
    values: state.values,
    editorDirty,
  }),
  applyRecipe,
  applyValues,
  setPrinter: async (id, settings, material) => {
    if (id) await choosePrinter(id);
    const notes = [
      ...(material ? applyMaterial(state.printer, state.profile, material).notes : []),
      ...(settings ? changeSettings(state.printer, state.profile, settings).notes : []),
    ];
    if (settings || material) {
      renderPrinterSettings($('printerSettings'), state.printer, state.profile, onProfileChange);
      onProfileChange();
    }
    return { printer: describePrinter(state.printer, state.profile), notes };
  },
  look: (show) => viewport.snapshot(/** @type {any} */ (show)),
  catalog: async () => state.catalog || (state.catalog = await engine.catalog()),
  templateOffer: async () => {
    // A check can start while we wait (the first one, after the project is restored).
    let waited;
    do {
      waited = templateChecked;
      await waited;
    } while (waited !== templateChecked);
    return templateOffer;
  },
});

// ---------------------------------------------------------------------------------------------
// Projects: templates, open, save, autosave
// ---------------------------------------------------------------------------------------------

/**
 * Opens recipe text as the current project.
 * @param {string} text
 * @param {'opened' | 'updated'} how a new project, or this one updated to its template
 */
async function openSource(text, how = 'opened') {
  state.template = null;
  $('templateUpdate').hidden = true;
  templateOffer = null;
  chat?.projectReplaced(how);
  state.source = text;
  setDirty(false);
  editor.value = text;
  await run('final', { fresh: true, opened: true });
  state.unsaved = false;
}

async function openTemplate(id) {
  const response = await fetch(`templates/${id}.easycad`);
  if (!response.ok) throw new Error(`template ${id}: ${response.status}`);
  const text = await response.text();
  await openSource(text);
  setSaveName('');
  state.template = { id, base: withoutValues(text) };
  autosave();
}

/** The templates' texts as they are now, fetched once each. */
const templateTexts = new Map();
async function templateText(id) {
  if (!templateTexts.has(id)) {
    templateTexts.set(
      id,
      fetch(`templates/${id}.easycad`)
        .then((r) => (r.ok ? r.text() : null))
        .catch(() => null),
    );
  }
  return templateTexts.get(id);
}

/**
 * The update to the project's template on offer, while the user hasn't answered; the chat waits
 * for the answer before carrying on with a finished feature request.
 * @type {{ update: () => Promise<void>, keep: () => void } | null}
 */
let templateOffer = null;
/** @type {(v?: any) => void} */
let firstCheckDone = () => {};
/** Settles once the first check after starting (and each later one) is done. */
let templateChecked = new Promise((resolve) => (firstCheckDone = resolve));

/**
 * A project started from a template that has been improved since (or, for an older project or
 * a file, one that differs from its template): offers to bring it up to date, keeping its
 * setting values. Saying no is remembered until the template changes again.
 */
function checkTemplate() {
  templateChecked = findTemplateUpdate();
  firstCheckDone();
  return templateChecked;
}

async function findTemplateUpdate() {
  templateOffer = null;
  const project = state.last?.project;
  if (!project) return;
  const mine = withoutValues(state.source);
  /** @type {{ id: string, text: string } | null} */
  let newer = null;
  if (state.template) {
    const text = await templateText(state.template.id);
    if (text && withoutValues(text) !== state.template.base) newer = { id: state.template.id, text };
  } else {
    for (const t of TEMPLATES) {
      const text = await templateText(t.id);
      if (!text || projectOf(text) !== project) continue;
      if (withoutValues(text) !== mine) newer = { id: t.id, text };
      break;
    }
  }
  if (!newer) return;
  const seen = loadJSON(SEEN_KEY, {});
  const print = fingerprint(withoutValues(newer.text));
  if (seen[newer.id] === print) return;
  const edited = state.template ? mine !== state.template.base : true;
  $('templateUpdateText').textContent =
    (state.template
      ? `The “${project}” template has been improved since you started this project. Update it to the new version?`
      : `This project differs from the current “${project}” template, which may have been improved since. Update it to the current version?`) +
    ` Your settings carry over${edited ? '; changes you made to the recipe text itself would be replaced' : ''}.`;
  const box = $('templateUpdate');
  box.hidden = false;
  const offer = {
    update: async () => {
      if (templateOffer !== offer) return;
      await updateToTemplate(newer.id, newer.text);
      chat.templateAnswered(true);
    },
    keep: () => {
      if (templateOffer !== offer) return;
      box.hidden = true;
      templateOffer = null;
      saveJSON(SEEN_KEY, { ...seen, [newer.id]: print });
      chat.templateAnswered(false);
    },
  };
  templateOffer = offer;
  $('templateUpdateYes').onclick = offer.update;
  $('templateUpdateNo').onclick = offer.keep;
}

/** Opens the template's current version with the project's setting values carried over. */
async function updateToTemplate(id, text) {
  $('templateUpdate').hidden = true;
  const keep = { ...state.values };
  const kind = (v) => (Array.isArray(v) ? 'list' : typeof v);
  // A project's own parts (its shades) carry over, like its values.
  await openSource(withPartsOf(text, state.source), 'updated');
  state.template = { id, base: withoutValues(text) };
  let kept = 0;
  for (const [k, v] of Object.entries(keep)) {
    if (k in state.values && kind(v) === kind(state.values[k])) {
      state.values[k] = v;
      kept++;
    }
  }
  state.layout = ''; // rebuild the window so every slider shows its value
  await run('final');
  state.openedValues = { ...state.values };
  state.unsaved = true;
  autosave();
  const done = $('templateUpdated');
  done.textContent = `Updated to the current template; ${kept} ${kept === 1 ? 'setting' : 'settings'} carried over.`;
  done.hidden = false;
  setTimeout(() => (done.hidden = true), 8000);
}

let saveTimer = 0;
function autosave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveJSON(STORAGE_KEY, {
      source: state.source,
      openedValues: state.openedValues,
      printer: state.printer,
      unsaved: state.unsaved,
      saveName: /** @type {HTMLInputElement} */ ($('saveName')).value,
      template: state.template,
    });
  }, 300);
}

function restore() {
  const saved = loadJSON(STORAGE_KEY, null);
  if (saved && typeof saved.source === 'string' && saved.source.trim()) {
    return saved;
  }
  return null;
}

/** The project, with every pending slider value written in. */
async function currentSource() {
  await engine.idle();
  return state.source;
}

/**
 * The name files are saved under: the one typed by Save, else the project's own (made safe for
 * a file name). The settings file is this name with "-settings".
 */
function saveName() {
  const typed = /** @type {HTMLInputElement} */ ($('saveName'))
    .value.replace(/\.easycad$/i, '')
    .replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '-')
    .trim();
  return typed || slug(projectName());
}

/** Puts a name in the Save field (empty: the project's own, shown as a hint). */
function setSaveName(name) {
  const input = /** @type {HTMLInputElement} */ ($('saveName'));
  input.value = name;
  input.placeholder = slug(projectName()) || 'Name';
}

function projectName() {
  return (state.last && state.last.project) || 'project';
}

function setPrinter(id) {
  state.printer = id;
  state.profile = loadProfile(id);
  /** @type {HTMLSelectElement} */ ($('printer')).value = id;
  renderPrinterSettings($('printerSettings'), id, state.profile, onProfileChange);
  onProfileChange();
}

/**
 * Chooses the printer (from the Print tab or the agent), and names it in the recipe's `printer`
 * line (an engine command), so the project opens with it next time.
 * @param {keyof typeof PRINTERS} id
 */
async function choosePrinter(id) {
  setPrinter(id);
  autosave();
  await engine.idle();
  const named = state.last?.printer;
  if (editorDirty || (named && printerFor(named) === id)) return;
  try {
    const source = await engine.setPrinter(state.source, PRINTERS[id].recipeName);
    if (source !== state.source) await applyRecipe(source);
  } catch {
    // A recipe with a problem keeps its line; the printer is still chosen for now.
  }
}

/** The printer or its settings changed: the bed, the checks and the export buttons follow. */
function onProfileChange() {
  moveTimes = null; // the travel speed counts in the playback's times
  // The extrusion multiplier counts in the simulation.
  if (state.simulate && state.last) run('final');
  viewport.setBed(state.profile.bed, state.profile.center);
  renderPrintTab();
  if (state.last) renderExports(state.last);
}

// ---------------------------------------------------------------------------------------------
// Parts (the lamp builder): the Plan, adding and removing parts, all through engine commands
// ---------------------------------------------------------------------------------------------

/** The part last tapped in the plan (drawn highlighted). */
let selectedPart = /** @type {string | null} */ (null);

/** The Plan button shows for recipes with components; the plan redraws with every result. */
function renderPlanView(result) {
  const has = (result.components || []).length > 0;
  const button = $('viewPlan');
  button.hidden = !has;
  const on = has && button.getAttribute('aria-pressed') === 'true';
  $('plan').hidden = !on;
  const colors = customColors();
  const prints = printsOf(result);
  const colorOf = (name) => partLook(colors, name, Math.max(0, prints.findIndex((p) => p.name === name))).color;
  if (on) renderPlan($('plan'), result, { onPort: (port) => addPart(port), onPart: (name) => openPart(name), selected: selectedPart, colorOf });
}

/** Opens a part's fold-out in the settings and brings it into sight. */
function openPart(name) {
  selectedPart = name;
  selectTab('settings');
  const id = `set-${name}.mount`;
  const details = document.getElementById(id)?.closest('details');
  if (details) {
    /** @type {HTMLDetailsElement} */ (details).open = true;
    details.scrollIntoView({ block: 'start', behavior: 'smooth' });
  }
  if (state.last) renderPlanView(state.last);
}

/**
 * Asks where a new part goes and what it is (on a port tapped in the plan, or any), then adds
 * it with the engine's `addPart` command.
 * @param {any} [port]
 */
async function addPart(port) {
  const result = state.last;
  if (!result) return;
  const answer = await askNewPart($('addPart'), result, port);
  if (!answer) return;
  const taken = [...(result.partList || []).map((p) => p.name), ...result.params.map((p) => p.name), ...result.steps.map((s) => s.name)];
  const name = newPartName(String(answer.values.shape ?? 'part'), taken);
  const component = result.components.find((c) => c.name === answer.component);
  const input = component?.inputs[0];
  try {
    await engine.idle();
    const source = await engine.addPart(state.source, answer.component, name, { ...answer.values, ...(input ? { [input]: answer.port } : {}) });
    selectedPart = name;
    refitNext = true;
    await applyRecipe(source);
    openPart(name);
  } catch (err) {
    showSettingsError(String(err.message || err));
  }
}

/** Removes a part (and, after asking, the parts mounted on it) with the engine's command. */
async function removePart(name) {
  const result = state.last;
  if (!result) return;
  const mounted = (part) =>
    (result.partList || [])
      .map((p) => p.name)
      .filter((p) => String(state.values[`${p}.mount`] ?? '').startsWith(`${part}.`));
  const gone = [];
  for (let next = mounted(name); next.length; next = next.flatMap(mounted)) gone.push(...next);
  if (gone.length && !window.confirm(`Remove ${name} and what's mounted on it (${gone.join(', ')})?`)) return;
  try {
    await engine.idle();
    const source = await engine.removePart(state.source, name);
    if (selectedPart === name || gone.includes(String(selectedPart))) selectedPart = null;
    refitNext = true;
    await applyRecipe(source);
  } catch (err) {
    showSettingsError(String(err.message || err));
  }
}

// ---------------------------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------------------------

function selectTab(name) {
  for (const t of TABS) {
    $(`tab-${t}`).setAttribute('aria-selected', String(t === name));
    $(`panel-${t}`).hidden = t !== name;
  }
  document.body.dataset.tab = name;
}

/** Reads a picked file as text, refusing anything too big to be a recipe or settings file. */
async function readPicked(input) {
  const file = input.files && input.files[0];
  input.value = '';
  if (!file) return null;
  if (file.size > MAX_FILE_BYTES) throw new Error(`${file.name} is too big for a project file (${fmt(file.size / 1024)} kB).`);
  return file.text();
}

function wire() {
  for (const t of TABS) $(`tab-${t}`).addEventListener('click', () => selectTab(t));

  document.querySelectorAll('[data-mode]').forEach((b) =>
    b.addEventListener('click', () => {
      document.querySelectorAll('[data-mode]').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
      $('viewPlan').setAttribute('aria-pressed', 'false');
      viewport.setMode(/** @type {any} */ (/** @type {HTMLElement} */ (b).dataset.mode));
      if (state.last) renderPlanView(state.last);
    }),
  );
  // The plan: the parts cut through their axis, over the 3D view.
  $('viewPlan').addEventListener('click', () => {
    const on = $('viewPlan').getAttribute('aria-pressed') !== 'true';
    $('viewPlan').setAttribute('aria-pressed', String(on));
    document.querySelectorAll('[data-mode]').forEach((x) => x.setAttribute('aria-pressed', String(!on && /** @type {HTMLElement} */ (x).dataset.mode === 'both')));
    if (!on) viewport.setMode('both');
    if (state.last) renderPlanView(state.last);
  });
  // Fit what's shown of the toolpath and orbit round its centre: the Fit button, a double-click,
  // or on a phone a double-tap (touch doesn't make a dblclick everywhere).
  $('viewFit').addEventListener('click', () => viewport.focus());
  $('view').addEventListener('dblclick', () => viewport.focus());
  let lastTap = { t: 0, x: 0, y: 0 };
  let touches = 0;
  $('view').addEventListener('pointerdown', (e) => {
    if (e.pointerType === 'touch') touches++;
  });
  $('view').addEventListener('pointerup', (e) => {
    if (e.pointerType !== 'touch') return;
    const several = touches > 1;
    touches = Math.max(0, touches - 1);
    if (several) {
      lastTap = { t: 0, x: 0, y: 0 };
      return;
    }
    const now = performance.now();
    if (now - lastTap.t < 320 && Math.hypot(e.clientX - lastTap.x, e.clientY - lastTap.y) < 30) {
      viewport.focus();
      lastTap = { t: 0, x: 0, y: 0 };
    } else {
      lastTap = { t: now, x: e.clientX, y: e.clientY };
    }
  });
  $('view').addEventListener('pointercancel', () => (touches = 0));
  // Set views, and parallel or perspective (remembered on this device).
  document.querySelectorAll('[data-view]').forEach((b) =>
    b.addEventListener('click', () => viewport.view(/** @type {any} */ (/** @type {HTMLElement} */ (b).dataset.view))),
  );
  const parallel = $('viewParallel');
  const setParallel = (on) => {
    viewport.setParallel(on);
    parallel.setAttribute('aria-pressed', String(on));
    save(VIEW_KEY, on ? 'parallel' : 'perspective');
  };
  setParallel(load(VIEW_KEY) !== 'perspective');
  parallel.addEventListener('click', () => setParallel(!viewport.parallel));
  // The toolpath as solid beads at their real line width and layer height, as slicers draw it
  // (remembered on this device).
  const widthButton = $('viewWidth');
  const opacity = /** @type {HTMLInputElement} */ ($('viewBeadOpacity'));
  const showBeads = (on) => {
    widthButton.setAttribute('aria-pressed', String(on));
    opacity.hidden = !on;
    viewport.setBeads(on);
    save(BEADS_KEY, on ? 'on' : 'off');
  };
  // How see-through the beads are (remembered on this device).
  const storedOpacity = Number(load(OPACITY_KEY));
  if (storedOpacity >= 10 && storedOpacity <= 100) opacity.value = String(storedOpacity);
  viewport.setBeadOpacity(Number(opacity.value) / 100);
  opacity.addEventListener('input', () => {
    viewport.setBeadOpacity(Number(opacity.value) / 100);
    save(OPACITY_KEY, opacity.value);
  });
  showBeads(load(BEADS_KEY) === 'on');
  // Simulated beads: each fills the gap under the nozzle (remembered on this device). Turning it
  // on shows the beads.
  state.simulate = load(SIM_KEY) === 'on';
  $('viewSim').setAttribute('aria-pressed', String(state.simulate));
  viewport.setSimulated(state.simulate);
  $('viewSim').addEventListener('click', () => {
    const on = !state.simulate;
    if (on && widthButton.getAttribute('aria-pressed') !== 'true') widthButton.click();
    setSimulate(on);
  });
  widthButton.addEventListener('click', () => {
    const on = widthButton.getAttribute('aria-pressed') !== 'true';
    // Beads are the toolpath: show it if only the shape is shown.
    if (on && document.querySelector('[data-mode="shape"]')?.getAttribute('aria-pressed') === 'true') {
      /** @type {HTMLElement} */ (document.querySelector('[data-mode="both"]')).click();
    }
    showBeads(on);
  });

  // Which way the nozzle and beads stand (remembered on this device): preview only.
  const nozzleSelect = /** @type {HTMLSelectElement} */ ($('viewNozzle'));
  const storedNozzle = load(NOZZLE_KEY);
  if (storedNozzle && [...nozzleSelect.options].some((o) => o.value === storedNozzle)) nozzleSelect.value = storedNozzle;
  viewport.setOrientation(/** @type {any} */ (nozzleSelect.value));
  nozzleSelect.addEventListener('change', () => {
    viewport.setOrientation(/** @type {any} */ (nozzleSelect.value));
    save(NOZZLE_KEY, nozzleSelect.value);
  });

  // What the toolpath is coloured by (remembered on this device), with its legend.
  const colorSelect = /** @type {HTMLSelectElement} */ ($('viewColor'));
  for (const [id, m] of Object.entries(viewport.colorModes)) colorSelect.append(el('option', { value: id, text: m.label }));
  const storedColor = load(COLOR_KEY);
  if (storedColor && storedColor in viewport.colorModes) colorSelect.value = storedColor;
  viewport.setColorMode(/** @type {any} */ (colorSelect.value));
  colorSelect.addEventListener('change', () => {
    // The simulated colours need the simulation.
    if ((colorSelect.value === 'simWidth' || colorSelect.value === 'squish') && !state.simulate) setSimulate(true);
    viewport.setColorMode(/** @type {any} */ (colorSelect.value));
    renderLegend();
    renderCustomColors();
    save(COLOR_KEY, colorSelect.value);
  });
  // Print progress: drag either handle, or scroll over the track (the handle nearer the
  // pointer moves; a step per 1/500 of the print, faster with Shift).
  for (const id of ['scrubFrom', 'scrub'])
    $(id).addEventListener('input', (e) => {
      stopPlaying();
      showProgress(e);
    });
  $('scrubPlay').addEventListener('click', play);
  const speed = /** @type {HTMLSelectElement} */ ($('scrubSpeed'));
  const storedSpeed = load(SPEED_KEY);
  if (storedSpeed && [...speed.options].some((o) => o.value === storedSpeed)) speed.value = storedSpeed;
  speed.addEventListener('change', () => save(SPEED_KEY, speed.value));
  const range = $('scrubRange');
  range.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      stopPlaying();
      const lo = /** @type {HTMLInputElement} */ ($('scrubFrom'));
      const hi = /** @type {HTMLInputElement} */ ($('scrub'));
      const max = Number(hi.max) || 1;
      const box = range.getBoundingClientRect();
      const at = ((e.clientX - box.left) / Math.max(1, box.width)) * max;
      const handle = Math.abs(at - Number(lo.value)) < Math.abs(at - Number(hi.value)) ? lo : hi;
      const step = Math.max(1, Math.round(max / (e.shiftKey ? 50 : 500)));
      handle.value = String(Math.min(max, Math.max(0, Number(handle.value) + (e.deltaY > 0 ? -step : step))));
      showProgress(/** @type {any} */ ({ target: handle }));
    },
    { passive: false },
  );
  // The legend's ends: type a value, Enter or leave the field; ↺ back to automatic.
  for (const id of ['legendMin', 'legendMax']) {
    const input = $(id);
    input.addEventListener('change', setLegendRange);
    input.addEventListener('keydown', (e) => {
      if (/** @type {KeyboardEvent} */ (e).key === 'Enter') /** @type {HTMLInputElement} */ (input).blur();
    });
  }
  $('legendAuto').addEventListener('click', () => {
    viewport.setColorRange(viewport.legend().mode, null, null);
    renderLegend();
  });

  const templates = /** @type {HTMLSelectElement} */ ($('template'));
  for (const t of TEMPLATES) templates.append(el('option', { value: t.id, text: t.name }));
  const open = async (id) => {
    $('replaceConfirm').hidden = true;
    try {
      await openTemplate(id);
    } catch (err) {
      showSettingsError(`Could not open the template: ${err.message || err}`);
    }
  };
  // Replacing a changed project asks first, inside the page (no browser dialog).
  let pending = '';
  templates.addEventListener('change', () => {
    const id = templates.value;
    templates.value = '';
    if (!id) return;
    if (!state.unsaved) {
      open(id);
      return;
    }
    pending = id;
    const name = TEMPLATES.find((t) => t.id === id)?.name || id;
    $('replaceText').textContent = `Replace “${projectName()}” with a new “${name}”? Changes since you last saved it are lost.`;
    $('replaceConfirm').hidden = false;
  });
  $('replaceYes').addEventListener('click', () => open(pending));
  $('replaceNo').addEventListener('click', () => ($('replaceConfirm').hidden = true));

  $('openProject').addEventListener('click', () => $('projectFile').click());
  $('projectFile').addEventListener('change', async (e) => {
    try {
      const picked = /** @type {HTMLInputElement} */ (e.target).files?.[0];
      const text = await readPicked(e.target);
      if (text !== null) {
        await openSource(withoutFileNote(text));
        setSaveName((picked?.name || '').replace(/\.easycad$/i, ''));
        autosave();
        checkTemplate();
      }
    } catch (err) {
      showSettingsError(`Could not open the project: ${err.message || err}`);
    }
  });
  $('saveName').addEventListener('input', () => autosave());
  $('saveProject').addEventListener('click', async () => {
    const site = new URL('.', location.href).href;
    download(`${saveName()}.easycad`, `${fileNote(site)}\n\n${await currentSource()}`);
    state.openedValues = { ...state.values };
    state.unsaved = false;
    autosave();
  });

  $('printer').addEventListener('change', (e) => {
    choosePrinter(/** @type {any} */ (/** @type {HTMLSelectElement} */ (e.target).value));
  });

  $('resetSettings').addEventListener('click', () => {
    Object.assign(state.values, state.openedValues);
    state.layout = ''; // rebuild the window so every slider shows its reset value
    run('final');
  });
  $('saveSettings').addEventListener('click', async () => {
    await engine.idle();
    const data = {
      app: 'EasyCAD',
      kind: 'settings',
      version: 2,
      about:
        "Only the settings' values (Settings, Load settings puts them back into the open project). The recipe isn't in here: to change what the project does, or to have an AI change it, use the project file (Settings, Save), which holds the recipe and these values together.",
      project: projectName(),
      printer: state.printer,
      values: state.values,
    };
    download(`${saveName()}-settings.json`, JSON.stringify(data, null, 2), 'application/json');
  });
  $('loadSettings').addEventListener('click', () => $('settingsFile').click());
  $('settingsFile').addEventListener('change', async (e) => {
    try {
      const text = await readPicked(e.target);
      if (text === null) return;
      const data = JSON.parse(text);
      if (!data || typeof data.values !== 'object') throw new Error('this file has no EasyCAD settings in it.');
      // The printer first: naming it in the recipe runs it again, from the values in the text.
      if (typeof data.printer === 'string' && data.printer in PRINTERS) await choosePrinter(/** @type {any} */ (data.printer));
      let used = 0;
      for (const [k, v] of Object.entries(data.values)) {
        if (k in state.values && typeof v === typeof state.values[k]) {
          state.values[k] = v;
          used++;
        }
      }
      if (!used) throw new Error('none of its settings belong to this project.');
      state.unsaved = true;
      state.layout = '';
      run('final');
    } catch (err) {
      showSettingsError(`Could not load settings: ${err.message || err}`);
    }
  });

  editor.addEventListener('input', () => setDirty(editor.value !== state.source));
  editor.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      runEditor();
    }
  });
  $('runRecipe').addEventListener('click', runEditor);
  $('revertRecipe').addEventListener('click', () => {
    setDirty(false);
    editor.value = state.source;
  });
  $('copyRecipe').addEventListener('click', async () => {
    note((await copyText(editor.value)) ? 'Recipe copied.' : 'Copying was blocked by the browser: select the text and copy it.');
  });
  // Copy right inside the tap: some browsers (Safari) refuse clipboard writes after a wait.
  $('copyForAI').addEventListener('click', async () => {
    const ok = await copyText(aiPrompt(state.source, state.catalog));
    note(ok ? 'Copied. Paste it into a Claude chat and describe what you want.' : 'Copying was blocked by the browser.');
  });
}

async function start() {
  wire();
  chat.start();
  const saved = restore();
  setPrinter(state.printer);
  renderAbout();
  try {
    state.engineInfo = await engine.ready;
    engine.catalog().then((text) => {
      state.catalog = text;
      $('catalog').textContent = text;
      renderAbout();
    });
    if (saved) {
      editor.value = saved.source;
      state.source = saved.source;
      await run('final', { fresh: true, opened: true });
      // Back to how it was left: the printer chosen (when the recipe doesn't name one), and
      // "Reset" still meaning "as opened".
      if (!state.last?.printer && saved.printer in PRINTERS) setPrinter(saved.printer);
      if (saved.openedValues && typeof saved.openedValues === 'object') state.openedValues = saved.openedValues;
      state.unsaved = saved.unsaved === true;
      setSaveName(typeof saved.saveName === 'string' ? saved.saveName : '');
      const t = saved.template;
      state.template = t && typeof t.id === 'string' && typeof t.base === 'string' ? { id: t.id, base: t.base } : null;
      checkTemplate();
    } else {
      await openTemplate(TEMPLATES[0].id);
    }
  } catch (err) {
    setStatus('Engine failed to start', 'bad');
    const msg = $('viewMessage');
    msg.hidden = false;
    msg.textContent = `The geometry engine could not start: ${err.message || err}`;
  }
  // No project to check (a fresh start, or the engine failed): nothing for the chat to wait for.
  firstCheckDone();
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('./sw.js', { updateViaCache: 'none' }).catch(() => {});
  }
}

start();
