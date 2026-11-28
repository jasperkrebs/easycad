// EasyCAD: a recipe (.easycad text) → Rust engine (WebAssembly, in a worker) → 3D view, a settings
// window generated from the recipe, a print estimate, and files to save (G-code, STL).
//
// The recipe text is the project and the single source of truth: moving a slider writes the new
// value into the text (the engine does that, see crates/wasm), and saving a project saves exactly
// that text.

import { createEngine } from './engine.js';
import { createViewport } from './viewport.js';
import { $, copyText, download, el, fmt, load, loadJSON, parseNumber, save, saveJSON, slug } from './dom.js';
import { foldout, layoutKey, renderSettings, settingRow } from './settings.js';
import { batchPlanLine, colorsLine, fileNote, fingerprint, printerLine, projectOf, withPartsOf, withoutBatch, withoutColors, withoutFileNote, withoutPrinter, withoutValues } from './templates.js';
import { PRINTERS, applyMaterial, isRobot, changeSettings, describePrinter, loadProfile, newProfile, printerFor, profileFrom, renderPrint, renderPrinterSettings, settingsOf } from './print.js';
import { aiPrompt, renderProblems, renderSteps, selectLine } from './recipe.js';
import { createChat } from './chat.js';
import { assemblyMotion, assemblySteps, capturePlan, fitPlan, markKey, markToggles, newPartName, renderPlan, sections, shadeDefaults, shapeSwitch } from './parts.js';
import { CAMERA_VIEWS, cleanRender, fixtureOf, fixtureTakenOut, partsExtent } from './fixture.js';
import { PLASTICS, cleanMaterial, materialValues } from './materials.js';
import {
  addBatchSetup,
  batchFileName,
  batchLine,
  batchPictures,
  batchPlanOf,
  batchSetups,
  batchStamp,
  clearBatchPictures,
  keepBatchPicture,
  moveBatchSetup,
  nextBatchName,
  removeBatchSetup,
  setupsOfPlan,
  zipFiles,
} from './batch.js';
import { beadExtremes, renderBeads } from './beads.js';
import { SKETCHER_TEMPLATE, isSketcherSettings, sketcherToValues, valuesToSketcher } from './sketcher.js';

const TEMPLATES = [
  { id: 'lampshade', name: 'Lampshade' },
  { id: 'lamp', name: 'Lamp' },
  { id: 'thread-pendant', name: 'Thread pendant' },
  { id: 'vessel', name: 'Vessel' },
  { id: 'salt-shaker', name: 'Salt & pepper shaker' },
  { id: 'nutcracker', name: 'Nutcracker' },
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
/** Whether the plan shows the lamp's fittings (socket, bulb, cable): 'on' (or unset) or 'off'. */
const PLAN_FITTINGS_KEY = 'easycad.planFittings';
/** What the toolpath is coloured by. */
const COLOR_KEY = 'easycad.colorMode';
/** How fast the print plays back. */
const SPEED_KEY = 'easycad.playSpeed';
const MAX_FILE_BYTES = 256 * 1024;
const TABS = ['settings', 'chat', 'recipe', 'print', 'render', 'about'];

const state = {
  /** The recipe text: the project. */
  source: '',
  /** Whether the beads are simulated (how the plastic actually lies): full-quality runs ask
   * the engine for it. */
  simulate: false,
  /** Whether the nozzle and beads stand along the wall (the view's Nozzle picker): then runs ask
   * the engine for the way up the wall, which takes a while on a print of many rings. */
  alongWall: false,
  /** Setting values as last opened, for "Reset settings". */
  openedValues: /** @type {Record<string, any>} */ ({}),
  /** Current setting values (each in its own unit); written into the text on every run. */
  values: /** @type {Record<string, any>} */ ({}),
  /** The settings window's layout, so it's rebuilt only when the recipe's settings change. */
  layout: '',
  printer: /** @type {keyof typeof PRINTERS} */ ('p1p'),
  /**
   * The printer's machine settings (print.js); G-code is written for these. They're the
   * project's: saved in its file (and with the project on the device), never per device.
   */
  profile: newProfile('p1p'),
  /** The project's settings for each printer it has been set to (its file keeps the chosen one's). */
  profiles: /** @type {Record<string, any>} */ ({}),
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
// The 3D view's GPU context taken away and not given back (a phone short of memory): it's
// rebuilt, after freeing the Render view's path tracer, the other big user of the GPU; while the
// Render view is open, when it closes.
const viewport = createViewport(/** @type {HTMLCanvasElement} */ ($('view')), {
  onLost: () => {
    if (renderOn()) return false;
    freeRender();
    return true;
  },
});
viewport.onFailed(() => {
  const msg = $('viewMessage');
  msg.hidden = false;
  msg.textContent = 'The 3D view stopped: this device ran out of graphics memory. Reload the page.';
});
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
    result = await engine.evaluate(state.source, changedValues(), quality, simulate, state.alongWall);
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
      onAddPart: () => showPlan(),
      onRemovePart: (name) => removePart(name),
    });
  }
  $('saveSketcher').hidden = result.project !== 'Thread pendant';
  viewport.show(result, { refit: refitNext });
  viewport.setFixture(fixtureFor(result));
  markRenderStale();
  refitNext = false;
  syncScrub();
  renderLegend();
  renderPartToggles(result);
  applyAssembly(result);
  renderPrintsView(result);
  renderCustomColors();
  renderPlanView(result);
  // The Render tab's materials follow the parts.
  if (renderOn() && renderPanelKey !== `${projectName()}|${printsOf(result).map((p) => p.name).join()}`) renderPanel();

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

/**
 * The values that differ from what the recipe says (its last run): only those are written into
 * the text, so a part's line gains just the settings moved, not every one it has.
 */
function changedValues() {
  const was = new Map((state.last?.params || []).map((p) => [p.name, JSON.stringify(p.value)]));
  return Object.fromEntries(Object.entries(state.values).filter(([k, v]) => was.get(k) !== JSON.stringify(v)));
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
        if (state.last) renderPlanView(state.last);
        markRenderStale();
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
    const kind = isRobot(state.printer) ? 'Robot program' : 'G-code';
    const text = `${kind} for the ${PRINTERS[state.printer].name}${several ? `: ${part.name}` : ''}`;
    const b = el('button', { className: 'btn primary', type: 'button', text });
    const name = `${slug(projectName())}${several ? `-${slug(part.name)}` : ''}${printExtension()}`;
    b.addEventListener('click', () => saveFile(i ? `print:${i}` : 'print', name, true));
    host.append(b);
  });
  result.exports.forEach((x, i) => {
    const b = el('button', { className: 'btn', type: 'button', text: x.file });
    b.addEventListener('click', () => saveFile(String(i), x.file));
    host.append(b);
  });
}

/** The printed file's extension: G-code, or a robot program's ZIP (its .SRC and .DAT). */
function printExtension() {
  return isRobot(state.printer) ? '.zip' : '.gcode';
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
  return { parts: mine.parts || {}, light: mine.light ?? 0.5, shadow: mine.shadow ?? 0.5, hang: mine.hang === 'up' ? 'up' : 'down', render: cleanRender(mine.render) };
}

/**
 * The view's colours as a file keeps them: this project's custom colours, and whether the view
 * is in them. null when none were chosen.
 */
function colorsToSave() {
  const all = loadJSON(CUSTOM_KEY, {});
  const mine = all && typeof all === 'object' ? all[projectName()] : null;
  const custom = /** @type {HTMLSelectElement} */ ($('viewColor')).value === 'custom';
  if (!mine && !custom) return null;
  // Every part's colour as shown (chosen, or its own until chosen).
  const colors = customColors();
  printsOf(state.last).forEach((p, i) => (colors.parts[p.name] = partLook(colors, p.name, i)));
  return { ...colors, custom };
}

/** Puts colours from a file back (and the view in them, when it was saved in them). */
function restoreColors(colors) {
  if (!colors || typeof colors !== 'object') return;
  const parts = colors.parts && typeof colors.parts === 'object' ? colors.parts : {};
  const clean = {};
  for (const [name, look] of Object.entries(parts)) {
    if (look && /^#[0-9a-f]{6}$/i.test(String(look.color))) {
      clean[name] = { color: String(look.color), opacity: Math.min(1, Math.max(0.05, Number(look.opacity) || 1)) };
      if (look.material) clean[name].material = cleanMaterial(look.material);
    }
  }
  const unit = (v) => (Number.isFinite(Number(v)) ? Math.min(1, Math.max(0, Number(v))) : 0.5);
  saveCustomColors({ parts: clean, light: unit(colors.light), shadow: unit(colors.shadow), hang: colors.hang === 'up' ? 'up' : 'down', render: cleanRender(colors.render) });
  customLayout = '';
  if (colors.custom) {
    const select = /** @type {HTMLSelectElement} */ ($('viewColor'));
    select.value = 'custom';
    select.dispatchEvent(new Event('change'));
  }
  renderCustomColors();
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
    if (state.last) renderView.api?.setLooks(renderLooks(state.last));
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
      renderPrinterSettings($('printerSettings'), state.printer, state.profile, onPrinterEdit);
      onPrinterEdit();
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
  // A new project: the printer's own defaults.
  state.profiles = {};
  await openSource(text);
  setPrinter(state.printer);
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
  saveTimer = setTimeout(saveNow, 300);
}

/** Saves the project on the device now (what autosave does a moment after the last change). */
function saveNow() {
  clearTimeout(saveTimer);
  saveTimer = 0;
  saveJSON(STORAGE_KEY, {
    source: state.source,
    openedValues: state.openedValues,
    printer: state.printer,
    printerSettings: { printer: state.printer, settings: settingsOf(state.printer, state.profile) },
    unsaved: state.unsaved,
    saveName: /** @type {HTMLInputElement} */ ($('saveName')).value,
    template: state.template,
  });
}

// A reload, or the tab closed, within the moment autosave waits: saved now, not lost.
window.addEventListener('pagehide', () => {
  if (saveTimer) saveNow();
});

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
  state.profile = state.profiles[id] ||= newProfile(id);
  /** @type {HTMLSelectElement} */ ($('printer')).value = id;
  renderPrinterSettings($('printerSettings'), id, state.profile, onPrinterEdit);
  onProfileChange();
}

/** The project's printer settings from a file's (or the saved project's) `{ printer, settings }`. */
function printerSettingsOf(saved) {
  return saved && saved.printer in PRINTERS ? { [saved.printer]: profileFrom(/** @type {keyof typeof PRINTERS} */ (saved.printer), saved.settings) } : {};
}

/** A printer setting changed (Print tab, agent): the project changed, so it's kept. */
function onPrinterEdit() {
  state.unsaved = true;
  onProfileChange();
  autosave();
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

/** The part selected in the plan (drawn highlighted, its bar showing). */
let selectedPart = /** @type {string | null} */ (null);
/** The mark tapped in the plan (its `markKey`), whose bar shows; null: none. */
let selectedMark = /** @type {string | null} */ (null);
/**
 * The part the plan just added, while nothing else has changed: switching its shape then adds it
 * afresh under its new shape's name (from the recipe as it was before), so a `cone` that becomes
 * a tube is called `tube`.
 * @type {{ name: string, before: string, after: string, values: Record<string, unknown>, port: any } | null}
 */
let freshPart = null;

/** Whether the plan is the view shown. */
const planOn = () => $('viewPlan').getAttribute('aria-pressed') === 'true';

/** The Plan button shows for recipes with parts or marks; the plan redraws with every result. */
function renderPlanView(result) {
  const has = (result.components || []).length > 0 || (result.marks || []).length > 0;
  const button = $('viewPlan');
  button.hidden = !has;
  const on = has && planOn();
  $('plan').hidden = !on;
  // The plan has no colours and no print to play: the view's foot gives its room to the plan.
  document.body.toggleAttribute('data-plan', on);
  if (selectedPart &&!(result.partList || []).some((p) => p.name === selectedPart)) selectedPart = null;
  if (!on) return;
  const colors = customColors();
  const prints = printsOf(result);
  const colorOf = (name) => partLook(colors, name, Math.max(0, prints.findIndex((p) => p.name === name))).color;
  renderPlan($('plan'), result, {
    selected: selectedPart,
    hidden: hiddenParts,
    colorOf,
    onSelect: (name) => {
      selectedMark = null;
      selectPart(name);
    },
    selectedMark,
    // A piece of the shape tapped: with on/off settings (a pattern on it, …), its bar of them in
    // the plan; without, its settings.
    onMark: (mark, part) => {
      if (markToggles(result, mark).length) {
        selectedMark = markKey(mark);
        selectedPart = part;
        renderPlanView(result);
      } else {
        if (part) selectPart(part, false);
        openSettings(mark.settings);
      }
    },
    onMarkSettings: (mark) => openSettings(mark.settings),
    onMarkClose: () => {
      selectedMark = null;
      renderPlanView(result);
    },
    onSetValues: (values) => applyValues(values),
    onAdd: (add) => addPartAt(add.port, add.values),
    onSet: (name, values) => setPartValues(name, values),
    onOpen: (name) => openPart(name),
    onRemove: (name) => removePart(name),
    offsets: assemblyOffsets,
    fixture: fixtureFor(result),
    fittingsShown: load(PLAN_FITTINGS_KEY) !== 'off',
    onFittings: (shown) => {
      save(PLAN_FITTINGS_KEY, shown ? 'on' : 'off');
      renderPlanView(result);
    },
  });
}

/** Shows the plan (the Plan button pressed). */
function showPlan() {
  if (!planOn()) $('viewPlan').click();
  // Its add buttons are the selected part's: the last part, unless one is selected.
  const parts = state.last?.partList || [];
  if (!selectedPart && parts.length) selectPart(parts[parts.length - 1].name, false);
}

/**
 * Selects a part in the plan (null: none). With the Settings tab showing, its fold-out comes
 * into sight too.
 * @param {string | null} name
 * @param {boolean} [follow] bring its settings into sight (when they're showing)
 */
function selectPart(name, follow = true) {
  selectedPart = name;
  if (name && follow && !$('panel-settings').hidden) showPartSettings(name);
  if (state.last) renderPlanView(state.last);
}

/**
 * Brings settings into sight (a mark tapped in the plan): the first that shows now, its fold-outs
 * opened; every one that shows lights up for a moment.
 * @param {string[]} names
 */
function openSettings(names) {
  selectTab('settings');
  const rows = names
    .map((n) => document.getElementById(`set-${n}`)?.closest('.setting'))
    .filter((row) => row && !row.hidden && !row.closest('[hidden]'));
  if (!rows.length) return;
  for (const row of rows) {
    for (let d = row.closest('details'); d; d = d.parentElement?.closest('details') ?? null) /** @type {HTMLDetailsElement} */ (d).open = true;
    row.classList.remove('flash');
    void (/** @type {HTMLElement} */ (row).offsetWidth); // restart the animation
    row.classList.add('flash');
  }
  rows[0].scrollIntoView({ block: 'center', behavior: 'smooth' });
}

/** A part's fold-out in the settings: opened and brought into sight. */
function showPartSettings(name) {
  const details = document.getElementById(`set-${name}.mount`)?.closest('details');
  if (!details) return;
  /** @type {HTMLDetailsElement} */ (details).open = true;
  details.scrollIntoView({ block: 'start', behavior: 'smooth' });
}

/** Opens a part's fold-out in the Settings tab (and selects it in the plan). */
function openPart(name) {
  selectedPart = name;
  selectTab('settings');
  showPartSettings(name);
  if (state.last) renderPlanView(state.last);
}

/** Names a new part can't have: the parts', settings' and steps' names. */
function takenNames(result) {
  return [...(result.partList || []).map((p) => p.name), ...result.params.map((p) => p.name), ...result.steps.map((s) => s.name)];
}

/**
 * Adds a part where an add button in the plan says (a port, or the default for the first part)
 * with the engine's `addPart` command: its component's `@ask` settings at their defaults unless
 * given (the button's way, up or down), its sizes scaled to where it goes. It arrives selected.
 * @param {any | null} port
 * @param {Record<string, unknown>} values
 * @param {string} [from] the recipe to add it to (the current one; the one before, when a part
 *   just added changes shape)
 */
async function addPartAt(port, values, from) {
  const result = state.last;
  const component = result?.components?.[0];
  if (!result || !component) return;
  const base = { ...Object.fromEntries(component.ask.map((p) => [p.name, p.value])), ...values };
  /** @type {Record<string, unknown>} */
  const sized = port ? shadeDefaults(String(base.shape ?? ''), port.value) : {};
  delete sized.direction;
  const all = { ...sized, ...base };
  const before = from ?? state.source;
  const replaced = from ? freshPart?.name : null;
  const taken = takenNames(result).filter((n) => !replaced || (n !== replaced && !n.startsWith(`${replaced}.`)));
  const name = newPartName(String(all.shape ?? component.name), taken);
  const input = component.inputs[0];
  try {
    await engine.idle();
    const source = await engine.addPart(before, component.name, name, { ...all, ...(input && port ? { [input]: port.name } : {}) });
    selectedPart = name;
    if (replaced && hiddenParts.delete(replaced)) hiddenParts.add(name);
    refitNext = true;
    fitPlan();
    await applyRecipe(source);
    freshPart = { name, before, after: state.source, values: base, port };
    if (!$('panel-settings').hidden) showPartSettings(name);
  } catch (err) {
    showSettingsError(String(err.message || err));
  }
}

/**
 * Changes a part's settings from the plan's bar (its shape, which way it goes, a switch). A part
 * just added, while nothing else has changed, is added afresh with its new `@ask` values (under
 * the new shape's name, its sizes scaled for it); otherwise the values are set, and a new shape
 * brings its sizes for the settings the old one didn't use.
 * @param {string} name
 * @param {Record<string, unknown>} values the part's own settings (`shape`, not `main.shape`)
 */
async function setPartValues(name, values) {
  const result = state.last;
  if (!result) return;
  const asked = new Set((result.components?.[0]?.ask || []).map((p) => p.name));
  if (freshPart && freshPart.name === name && freshPart.after === state.source && Object.keys(values).every((k) => asked.has(k))) {
    await addPartAt(freshPart.port, { ...freshPart.values, ...values }, freshPart.before);
    return;
  }
  if (values.shape !== undefined) {
    const old = String(state.values[`${name}.shape`] ?? '');
    const mount = (result.ports || []).find((p) => p.name === String(state.values[`${name}.mount`] ?? ''));
    values = { ...shapeSwitch(old, String(values.shape), mount ? mount.value : null), ...values };
  }
  freshPart = null;
  await applyValues(Object.fromEntries(Object.entries(values).map(([k, v]) => [`${name}.${k}`, v])));
}

// ---- The prints overview: every part laid as it prints, each on its own bed (like a slicer's
// plates); per part, which way up it prints and its G-code.

/** Shows the Prints button (for a project of several parts) and, while it's on, the plates. */
function renderPrintsView(result) {
  const prints = printsOf(result);
  const button = $('viewPrints');
  button.hidden = prints.length < 2;
  const on = !button.hidden && button.getAttribute('aria-pressed') === 'true';
  $('printsStrip').hidden = !on;
  if (!on) return;
  const value = (key) => result.params.find((p) => p.name === key)?.value;
  const strip = $('printsStrip');
  strip.replaceChildren(
    ...prints.map((p, i) => {
      const card = el('div', { className: 'print-card' }, [el('span', { className: 'print-name', text: p.name })]);
      // Which way up: a shade's wide edge down, or (closed) its disc down, always.
      if (value(`${p.name}.closed`) === true) {
        card.append(el('button', { type: 'button', text: 'Disc down', disabled: true, title: 'A closed end prints its disc first' }));
      } else if (typeof value(`${p.name}.wide_down`) === 'boolean') {
        const down = value(`${p.name}.wide_down`) === true;
        const flip = el('button', { type: 'button', text: '↕ Flip', 'aria-pressed': String(down), title: down ? 'Printed wide edge down: print it start down' : 'Printed start down: print it wide edge down' });
        flip.addEventListener('click', () => setPartValues(p.name, { wide_down: !down }));
        card.append(flip);
      }
      const several = prints.length > 1;
      const file = `${slug(projectName())}${several ? `-${slug(p.name)}` : ''}${printExtension()}`;
      const kind = isRobot(state.printer) ? 'Program' : 'G-code';
      const gcode = el('button', { type: 'button', className: 'print-gcode', text: kind, title: `${kind} for the ${PRINTERS[state.printer].name}: ${p.name}` });
      gcode.addEventListener('click', () => saveFile(i ? `print:${i}` : 'print', file, true));
      card.append(gcode);
      return card;
    }),
  );
}

/** Turns the prints overview on or off (off: the assembly as it was). */
function showPrints(on) {
  $('viewPrints').setAttribute('aria-pressed', String(on));
  viewport.setLayout(on ? { bed: state.profile.bed, center: state.profile.center } : null);
  if (state.last) renderPrintsView(state.last);
}

// ---- The assembly taken apart (the view's assembly row): a shade at a time, furthest out
// first, each turned off its thread and moved on by a gap as long as the thread, the shades on it
// riding along; at the end, the exploded view. A view only: nothing in the recipe changes.

/** How far it's taken apart (0: assembled, 1: exploded), and the animation playing it. */
const assembly = { u: 0, frame: 0, steps: /** @type {ReturnType<typeof assemblySteps>} */ ([]) };

/** Seconds per shade unscrewed, while it plays. */
const SECONDS_PER_STEP = 2.2;

/** The offsets now (by part name), for the plan; the 3D view gets them by print. */
let assemblyOffsets = /** @type {Map<string, number>} */ (new Map());

/** Works out the steps for this result and shows the parts where `assembly.u` puts them. */
function applyAssembly(result = state.last) {
  assembly.steps = result ? assemblySteps(result) : [];
  $('viewAssembly').hidden = !assembly.steps.length;
  if (!assembly.steps.length) assembly.u = 0;
  const motion = assemblyMotion(assembly.steps, assembly.u);
  const prints = printsOf(result);
  viewport.setPartMotion(new Map([...motion].map(([name, m]) => [prints.findIndex((p) => p.name === name), m]).filter(([i]) => i >= 0)));
  assemblyOffsets = new Map([...motion].map(([name, m]) => [name, m.dz]));
  /** @type {HTMLInputElement} */ ($('asmRange')).value = String(Math.round(assembly.u * 1000));
  $('asmExplode').setAttribute('aria-pressed', String(assembly.u >= 1));
  $('asmPlay').textContent = assembly.frame ? '❚❚' : '▶';
  // The fittings come out with them (in the 3D view; the plan's redrawn by the caller).
  if (result) viewport.setFixture(fixtureFor(result));
  // The render takes the parts where this puts them: a prepared one is out of date.
  markRenderStale();
}

/** Sets how far it's taken apart, and shows it (the plan too). */
function setAssembly(u) {
  assembly.u = Math.min(1, Math.max(0, u));
  applyAssembly();
  if (state.last && !$('plan').hidden) renderPlanView(state.last);
}

function stopAssembly() {
  cancelAnimationFrame(assembly.frame);
  assembly.frame = 0;
  $('asmPlay').textContent = '▶';
}

/** Plays it: apart, or (taken apart already) back together. */
function playAssembly() {
  if (assembly.frame) return stopAssembly();
  const way = assembly.u >= 1 ? -1 : 1;
  const total = Math.max(1, assembly.steps.length) * SECONDS_PER_STEP;
  let last = performance.now();
  const tick = (now) => {
    const u = assembly.u + (way * (now - last)) / 1000 / total;
    last = now;
    const done = way > 0 ? u >= 1 : u <= 0;
    assembly.frame = done ? 0 : requestAnimationFrame(tick);
    setAssembly(u);
  };
  assembly.frame = requestAnimationFrame(tick);
  $('asmPlay').textContent = '❚❚';
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
    freshPart = null;
    refitNext = true;
    fitPlan();
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

/**
 * Opens a Thread Lampshade Sketcher settings file: a new Thread pendant with its values.
 * @param {any} data the parsed file
 * @param {string} name the file's name
 */
async function openSketcher(data, name) {
  const { values, notes } = sketcherToValues(data);
  await openTemplate(SKETCHER_TEMPLATE);
  Object.assign(state.values, values);
  state.unsaved = true;
  state.layout = '';
  await run('final');
  setSaveName(name.replace(/\.json$/i, ''));
  autosave();
  note(['Opened the sketcher\'s settings as a Thread pendant.', ...notes].join(' '));
}

// ---- The Render view: the lamp path traced (render.js, loaded when first opened). Prepare
// takes a snapshot of the project as it is (its toolpaths swept into solid beads, the fittings,
// the custom colours); changing the project afterwards changes nothing in the render until it's
// prepared again. The light and the picture's settings only restart the sampling.

/**
 * The fittings for a result (holder, bulb, cable), as the project hangs; null without a socket.
 * Taken apart (the assembly row), they come apart too: the socket up past the highest shade,
 * the bulb down past the lowest.
 */
function fixtureFor(result, ctx = null) {
  if (!result) return null;
  const parts = sections(result);
  const fx = fixtureOf(result.params, parts, ctx ? ctx.colors.hang : customColors().hang);
  const steps = ctx ? assemblySteps(result) : assembly.steps;
  const u = ctx ? ctx.u : assembly.u;
  if (!fx || !steps.length || !(u > 0)) return fx;
  // Where the shades are, taken apart as far as they are.
  const offsets = new Map([...assemblyMotion(steps, u)].map(([name, m]) => [name, m.dz]));
  return fixtureTakenOut(fx, u, partsExtent(parts, offsets));
}

/**
 * What a render's picture is made of besides the recipe: the parts' looks and the way it hangs
 * (the custom colours' parts and hang), the parts hidden, how far it's taken apart. Now, or (a
 * batch setup's) as saved.
 * @returns {{ colors: { parts: Record<string, any>, hang?: string }, hidden: string[], u: number }}
 */
function renderContext() {
  const c = customColors();
  return { colors: { parts: c.parts, hang: c.hang }, hidden: [...hiddenParts], u: assembly.u };
}

const renderView = {
  /** @type {Awaited<ReturnType<typeof import('./render.js').createRenderer>> | null} */
  api: null,
  /** @type {Promise<any> | null} */
  loading: null,
  /** What was prepared (the project, its values, the way it hangs and hidden parts), to tell when it's out of date. */
  key: '',
  busy: false,
  /** What was prepared: its size and how long it took. */
  info: '',
  /** Whoever waits on the renderer (the batch): called with each progress, and `{ state: 'lost' }` on a GPU reset. */
  listeners: /** @type {Set<(p: any) => void>} */ (new Set()),
};

/**
 * The picture the Render view holds: how long it has rendered and whether it's saved (a long
 * one not saved is asked about before it's let go), and how the view shows it (moved, zoomed).
 */
const picture = { seconds: 0, saved: false, zoom: 1, x: 0, y: 0 };

/** The batch while it runs: where it is, to stop it. */
const batch = {
  running: false,
  stopping: false,
  /** "Batch 2/5 · name": put before the status line. */
  label: '',
  /** The batch's start ("batch-2026-11-28-2205"), and the folder its pictures go into (Chrome on a computer: chosen at the start), or null: downloads. */
  stamp: '',
  /** @type {any} */
  folder: null,
};

const renderOn = () => $('viewRender').getAttribute('aria-pressed') === 'true';

/** What a render is prepared from: changes to any of it make the prepared one out of date. */
function renderKey() {
  const c = customColors();
  return JSON.stringify([state.source, state.values, c.hang, [...hiddenParts].sort(), Math.round(assembly.u * 1000)]);
}

/** The Prepare button shows when the prepared render no longer matches the project. */
function markRenderStale() {
  const stale = Boolean(renderView.key) && renderView.key !== renderKey();
  $('renderPrepare').toggleAttribute('data-stale', stale || !renderView.key);
}

function renderStatus(text) {
  $('renderStatus').textContent = text;
}

/** Each printed part's look for the render: its custom colour, how solid, its material. */
function renderLooks(result) {
  const colors = customColors();
  return printsOf(result).map((p, i) => partLook(colors, p.name, i));
}

/**
 * The project as the render takes it: each printed part's toolpaths (print aids such as a brim
 * left out, hidden parts too) where the assembly row has them (taken apart: each turned about
 * the axis and moved along it, as the 3D view shows it; the socket and bulb stay), the parts'
 * looks and the fittings.
 */
function renderSnapshot(result, ctx = null) {
  const { points, starts, widths, heights, ups, aids } = result.paths;
  const owner = result.parts?.paths || [];
  const prints = printsOf(result);
  const hidden = ctx ? new Set(ctx.hidden) : hiddenParts;
  const motion = ctx ? assemblyMotion(assemblySteps(result), ctx.u) : assemblyMotion(assembly.steps, assembly.u);
  /** Turns (degrees, counter-clockwise from above) and lifts (mm) x, y, z triples in place. */
  const move = (xyz, angle, dz, lift) => {
    const [c, s] = [Math.cos((angle * Math.PI) / 180), Math.sin((angle * Math.PI) / 180)];
    for (let i = 0; i < xyz.length; i += 3) {
      const [x, y] = [xyz[i], xyz[i + 1]];
      xyz[i] = c * x - s * y;
      xyz[i + 1] = s * x + c * y;
      if (lift) xyz[i + 2] += dz;
    }
  };
  const paths = [];
  for (let k = 0; k + 1 < starts.length; k++) {
    const part = owner[k] ?? -1;
    if (part < 0 || !prints[part] || hidden.has(prints[part].name)) continue;
    const [a, b] = [starts[k], starts[k + 1]];
    if (b - a < 2) continue;
    const w = widths ? widths.slice(a, b) : new Float32Array(b - a).fill(1);
    const h = heights ? heights.slice(a, b) : new Float32Array(b - a).fill(0.5);
    if (aids) for (let i = a; i < b; i++) if (aids[i]) w[i - a] = 0;
    const pts = points.slice(3 * a, 3 * b);
    const up = ups ? ups.slice(3 * a, 3 * b) : null;
    const m = motion.get(prints[part].name);
    if (m && (m.angle || m.dz)) {
      move(pts, m.angle, m.dz, true);
      if (up) move(up, m.angle, 0, false);
    }
    paths.push({ points: pts, widths: w, heights: h, ups: up, part });
  }
  const looks = ctx ? prints.map((p, i) => partLook({ parts: {}, ...ctx.colors }, p.name, i)) : renderLooks(result);
  return { paths, looks, fixture: fixtureFor(result, ctx) };
}

/** "2 h 05 min", "1 min 05 s", "12 s". */
const renderTime = (s) =>
  s >= 3600
    ? `${Math.floor(s / 3600)} h ${String(Math.floor((s % 3600) / 60)).padStart(2, '0')} min`
    : s >= 60
      ? `${Math.floor(s / 60)} min ${String(Math.round(s % 60)).padStart(2, '0')} s`
      : `${Math.round(s)} s`;

/** The status line and the buttons from what the renderer is doing. */
function showRenderProgress(p) {
  const what = p.kind === 'final' ? 'Render' : 'Preview';
  const size = p.width ? ` · ${p.width}×${p.height}` : '';
  const by = { oidn: 'denoised', smooth: 'smoothed (no WebGPU for the denoiser)' };
  const denoised = p.denoised ? ` · ${p.showing ? by[p.denoised] : 'as rendered'}` : p.denoiseError ? ` · not denoised: ${p.denoiseError}` : '';
  const texts = {
    empty: 'Prepare to render',
    denoising: `${what} · ${p.samples} samples${size} · denoising…`,
    draft: `Draft${renderView.info ? ` · ${renderView.info}` : ''}`,
    compiling: `${what} · compiling the shaders…`,
    rendering: `${what} · ${p.samples} / ${p.target}${size} · ${renderTime(p.seconds || 0)}`,
    done: `${what} done · ${p.samples} samples${size} · ${renderTime(p.seconds || 0)}${denoised}`,
    stopped: `${what} stopped · ${p.samples} samples${size}${denoised}`,
  };
  // A picture held: settings changed meanwhile wait for the next Preview or Render.
  const holding = ['compiling', 'rendering', 'done', 'stopped', 'denoising'].includes(p.state);
  renderStatus(`${batch.label}${texts[p.state] || ''}${holding && p.pending ? ' · settings changed' : ''}`);
  if (p.state === 'compiling') picture.saved = false;
  if (holding && p.seconds) picture.seconds = p.seconds;
  if (!holding) {
    picture.seconds = 0;
    picture.saved = false;
    showPictureAt(1, 0, 0);
  }
  $('render').toggleAttribute('data-held', holding);
  $('renderNewView').toggleAttribute('disabled', !holding);
  $('renderPreview').toggleAttribute('data-stale', holding && Boolean(p.pending));
  $('renderFinal').toggleAttribute('data-stale', holding && Boolean(p.pending));
  const running = p.state === 'compiling' || p.state === 'rendering';
  const prepared = Boolean(renderView.api?.prepared);
  $('renderStop').toggleAttribute('disabled', !running);
  $('renderPreview').toggleAttribute('disabled', !prepared);
  $('renderFinal').toggleAttribute('disabled', !prepared);
  $('renderSave').toggleAttribute('disabled', !(p.state === 'done' || p.state === 'stopped' || p.state === 'rendering'));
  // Denoise (a finished or stopped picture); once denoised, it switches between the denoised
  // picture and the one as rendered.
  const finished = p.state === 'done' || p.state === 'stopped';
  $('renderDenoised').toggleAttribute('disabled', !(finished || p.denoised));
  $('renderDenoised').dataset.denoised = p.denoised ? '1' : '';
  $('renderDenoised').textContent = p.denoised ? 'Denoised' : 'Denoise';
  $('renderDenoised').setAttribute('aria-pressed', String(Boolean(p.denoised && p.showing)));
  $('renderPreview').setAttribute('aria-pressed', String(running && p.kind === 'preview'));
  $('renderFinal').setAttribute('aria-pressed', String(running && p.kind === 'final'));
}

/** The held picture, moved and zoomed in the view (1, 0, 0: as rendered, whole). */
function showPictureAt(zoom, x, y) {
  picture.zoom = zoom;
  [picture.x, picture.y] = zoom === 1 ? [0, 0] : [x, y];
  const canvas = /** @type {HTMLElement | null} */ (document.getElementById('renderCanvas'));
  if (canvas) canvas.style.transform = zoom === 1 ? '' : `translate(${picture.x}px, ${picture.y}px) scale(${zoom})`;
}

/**
 * Whether the held picture may go (for a new one, or the camera free again): yes when there's
 * none, it's saved, or it took under a minute; else asked (Save PNG first, Throw away, Keep it).
 * @returns {Promise<boolean>}
 */
async function letPictureGo() {
  if (!renderView.api?.held || picture.saved || picture.seconds < 60) return true;
  $('renderConfirmText').textContent = `Throw away this picture? ${renderTime(picture.seconds)} of rendering, not saved.`;
  $('renderConfirm').hidden = false;
  const answer = await new Promise((resolve) => {
    const pick = (a) => () => resolve(a);
    $('renderConfirmSave').onclick = pick('save');
    $('renderConfirmDrop').onclick = pick('drop');
    $('renderConfirmKeep').onclick = pick('keep');
  });
  $('renderConfirm').hidden = true;
  if (answer === 'keep') return false;
  if (answer === 'save') await saveRenderPng();
  return true;
}

/** The picture as it shows, as a PNG. */
async function saveRenderPng() {
  if (!renderView.api?.rendered) return;
  const blob = await (await fetch(renderView.api.capture())).blob();
  download(`${saveName()}-render.png`, blob, 'image/png');
  picture.saved = true;
}

/** Back to the scene: the picture let go (asked first when it's worth keeping). */
async function newRenderView() {
  if (!(await letPictureGo())) return false;
  renderView.api?.discard();
  return true;
}

/**
 * Moving and zooming the held picture in the view: drag, the wheel, two fingers; a double click
 * shows it whole again. (While no picture is held, the view orbits the camera instead.)
 */
function wirePictureView() {
  const host = $('render');
  const holding = () => Boolean(renderView.api?.held);
  /** @type {Map<number, { x: number, y: number }>} */
  const pointers = new Map();
  /** Zooms by `f` about (cx, cy), a point in the view. */
  const zoomAbout = (f, cx, cy) => {
    const zoom = Math.min(16, Math.max(1, picture.zoom * f));
    const k = zoom / picture.zoom;
    showPictureAt(zoom, cx - (cx - picture.x) * k, cy - (cy - picture.y) * k);
  };
  const at = (e) => {
    const r = host.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };
  host.addEventListener(
    'wheel',
    (e) => {
      if (!holding()) return;
      e.preventDefault();
      const [cx, cy] = at(e);
      zoomAbout(Math.exp(-e.deltaY * 0.0015), cx, cy);
    },
    { passive: false },
  );
  host.addEventListener('pointerdown', (e) => {
    if (!holding() || /** @type {HTMLElement} */ (e.target).closest('.confirm')) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    host.setPointerCapture?.(e.pointerId);
  });
  host.addEventListener('pointermove', (e) => {
    const was = pointers.get(e.pointerId);
    if (!was || !holding()) return;
    const now = { x: e.clientX, y: e.clientY };
    if (pointers.size === 1) {
      showPictureAt(picture.zoom, picture.x + now.x - was.x, picture.y + now.y - was.y);
    } else if (pointers.size === 2) {
      const other = [...pointers].find(([id]) => id !== e.pointerId)?.[1];
      if (other) {
        const before = Math.hypot(was.x - other.x, was.y - other.y);
        const after = Math.hypot(now.x - other.x, now.y - other.y);
        const [cx, cy] = at({ clientX: (now.x + other.x) / 2, clientY: (now.y + other.y) / 2 });
        if (before > 0) zoomAbout(after / before, cx, cy);
      }
    }
    pointers.set(e.pointerId, now);
  });
  const up = (e) => pointers.delete(e.pointerId);
  host.addEventListener('pointerup', up);
  host.addEventListener('pointercancel', up);
  host.addEventListener('dblclick', () => holding() && showPictureAt(1, 0, 0));
}

/** Loads the renderer (and the path tracer with it) the first time the view opens. */
async function renderApi() {
  if (renderView.api) return renderView.api;
  renderView.loading ||= import('./render.js').then(({ createRenderer }) =>
    createRenderer(/** @type {HTMLCanvasElement} */ ($('renderCanvas')), {
      onProgress: (p) => {
        showRenderProgress(p);
        for (const f of [...renderView.listeners]) f(p);
      },
      // Where the camera was left is kept with the project (nothing else changes).
      onCamera: (cam) => !batch.running && saveRender({ camera: { ...customColors().render.camera, ...cam } }),
      // The GPU was reset under the render: start afresh (Prepare again).
      onLost: (doing) => resetRender(`The graphics card was reset (while ${doing}): Prepare again (a smaller picture or fewer bounces is lighter on it)`),
    }),
  );
  try {
    renderView.api = await renderView.loading;
  } catch (err) {
    // Not kept: the next Prepare tries again, on a fresh canvas (a failed one can stay unusable).
    renderView.loading = null;
    freshRenderCanvas();
    throw err;
  }
  renderView.api.setSettings(customColors().render);
  return renderView.api;
}

/** The render's canvas replaced by a new one (its WebGL context gone with the old). */
function freshRenderCanvas() {
  const old = $('renderCanvas');
  old.replaceWith(old.cloneNode(false));
}

/**
 * Starts the Render view afresh after its GPU context was lost: whatever was prepared is gone,
 * the canvas is new, and the status says what happened.
 * @param {string} message
 */
function resetRender(message) {
  const api = renderView.api;
  renderView.api = null;
  renderView.loading = null;
  renderView.busy = false;
  renderView.key = '';
  renderView.info = '';
  try {
    api?.dispose();
  } catch {
    // its context is gone already
  }
  freshRenderCanvas();
  showRenderProgress({ state: 'empty' });
  renderStatus(message);
  $('renderPrepare').removeAttribute('disabled');
  markRenderStale();
  for (const f of [...renderView.listeners]) f({ state: 'lost' });
}

/** What the status line says when the render can't start or go on. */
function renderTrouble(err) {
  return /** @type {any} */ (err)?.code === 'no-gl'
    ? 'No graphics for the render: the browser turned them off for this page after the graphics card was reset (Safari keeps them off until it is quit and opened again). Restart the browser'
    : `Can't render here: ${/** @type {any} */ (err)?.message || err}`;
}

/**
 * Gives the path tracer's GPU memory back (the 3D view needs it): the render is no longer
 * prepared, and its canvas is a fresh one for the next.
 */
function freeRender() {
  if (!renderView.api || renderView.busy) return;
  renderView.api.dispose();
  renderView.api = null;
  renderView.loading = null;
  renderView.key = '';
  renderView.info = '';
  freshRenderCanvas();
  showRenderProgress({ state: 'empty' });
  markRenderStale();
}

/** Opens or closes the Render view (closing stops a render; it stays prepared). */
async function showRender(on) {
  if (on === renderOn()) return;
  $('viewRender').setAttribute('aria-pressed', String(on));
  $('render').hidden = !on;
  $('viewRenderBar').hidden = !on;
  document.body.toggleAttribute('data-render', on);
  if (!on) {
    renderView.api?.pause();
    $('tab-render').hidden = true;
    if ($('tab-render').getAttribute('aria-selected') === 'true') selectTab('settings');
    if (viewport.lost) {
      freeRender();
      viewport.rebuild();
    }
    return;
  }
  // The render is a view of its own: not the plan, not the prints laid out.
  if (planOn()) $('viewPlan').click();
  if ($('viewPrints').getAttribute('aria-pressed') === 'true') showPrints(false);
  document.querySelectorAll('[data-mode]').forEach((x) => x.setAttribute('aria-pressed', 'false'));
  // Its settings: the side panel's Render tab, shown while the view is.
  renderPanel();
  $('tab-render').hidden = false;
  selectTab('render');
  markRenderStale();
  if (!renderView.api?.prepared) showRenderProgress({ state: 'empty' });
  try {
    const api = await renderApi();
    if (renderOn()) api.resume();
    if (!api.running) api.setSettings(customColors().render);
  } catch (err) {
    renderStatus(renderTrouble(err));
  }
}

/** Prepares the project as it is now for rendering. */
async function prepareRender() {
  if (!state.last || renderView.busy) return;
  renderView.busy = true;
  $('renderPrepare').setAttribute('disabled', '');
  const t0 = performance.now();
  try {
    const api = await renderApi();
    const key = renderKey();
    renderView.info = '';
    // Taken apart, the lamp is shown, not lit: the light goes off (it can be turned on again).
    const exploded = assembly.u > 0 && assembly.steps.length > 0;
    if (exploded && customColors().render.light) {
      saveRender({ light: false });
      if (renderOn()) renderPanel();
    }
    // This project's settings (another project may have been open since).
    api.setSettings(customColors().render);
    const { triangles } = await api.prepare(renderSnapshot(state.last), (what) => renderStatus(`Preparing · ${what}`));
    renderView.key = key;
    renderView.info = `${exploded ? 'taken apart · ' : ''}${(triangles / 1e6).toFixed(2)} M triangles, prepared in ${fmt((performance.now() - t0) / 1000)} s`;
    showRenderProgress({ state: 'draft' });
  } catch (err) {
    renderStatus(renderTrouble(err));
  } finally {
    renderView.busy = false;
    $('renderPrepare').removeAttribute('disabled');
    markRenderStale();
  }
}

/** Keeps a change to the render's settings with the project's colours (nothing else). */
function saveRender(update) {
  const colors = customColors();
  colors.render = cleanRender({ ...colors.render, ...update });
  saveCustomColors(colors);
  return colors.render;
}

/** Changes the render's settings: kept, shown, and the renderer stops (Preview or Render again). */
function changeRender(update) {
  const next = saveRender(update);
  // A batch running: the setting is kept for later, the batch's render goes on.
  if (!batch.running) renderView.api?.setSettings(next);
}

/** The parts' looks changed (colour, solid, material): the 3D view and the render follow. */
function changeLook(name, i, update) {
  const colors = customColors();
  colors.parts[name] = { ...partLook(colors, name, i), ...update };
  saveCustomColors(colors);
  applyCustomColors();
  if (state.last && !batch.running) renderView.api?.setLooks(renderLooks(state.last));
}

/** A setting row as the settings window draws it (number field, unit, slider; switch; list). */
// (Named render-…, so their ids never meet a project setting's.)
const renderParam = (name, label, kind, extra = {}) => ({ name: `render-${name}`, label, kind, attrs: [], unit: '', ...extra });

/** A row of its own: a label and whatever control. */
const renderRow = (label, control) => el('div', { className: 'setting text' }, [el('label', { text: label }), control]);

/** What the Render tab was last built for (its parts), so it's rebuilt only when they change. */
let renderPanelKey = '';

/** The denoiser's networks: [key, label]. */
const DENOISERS = [['detailed', 'Detailed (slower)'], ['fast', 'Fast']];

/** Where the Render's wall distance is measured from: [key, label]. */
const WALL_FROM = [['axis', "the lamp's axis"], ['back', "the lamp's back"]];

/**
 * The Render tab: the render's settings as the settings window has them (a number to type, a
 * slider beside it), in fold-outs: light, camera, room, each part's material, quality. Changes
 * stop the render; Preview or Render again.
 */
function renderPanel() {
  const host = $('renderSettings');
  const colors = customColors();
  const r = colors.render;
  const prints = printsOf(state.last);
  renderPanelKey = `${projectName()}|${prints.map((p) => p.name).join()}`;
  const views = Object.entries(CAMERA_VIEWS);
  const plastics = Object.entries(PLASTICS);
  const pct = (x) => Math.round(x * 1000) / 10;

  /** @type {[string, string, any[]][]} group name, remembered key, rows: [param, value] or a node */
  const groups = [
    ['Light', 'render:light', [
      [renderParam('light', 'Light on', 'toggle'), r.light],
      [renderParam('hang', 'Shades hang (the socket points down)', 'choice', { choices: ['down', 'up'] }), colors.hang],
      [renderParam('lumens', 'Bulb', 'number', { unit: 'lm', min: 50, max: 5000, step: 10 }), r.lumens],
      [renderParam('kelvin', 'Colour temperature', 'number', { unit: 'K', min: 1800, max: 6500, step: 50 }), r.kelvin],
      [renderParam('lightSize', 'Light size (of the bulb\'s glass; 0: a point)', 'number', { unit: '%', min: 0, max: 100, step: 5 }), pct(r.lightSize)],
      [renderParam('room', 'Room light (bulb on)', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(r.room)],
      [renderParam('exposure', 'Exposure', 'number', { unit: 'stops', min: -6, max: 6, step: 0.25 }), r.exposure],
    ]],
    ['Camera', 'render:camera', [
      [renderParam('view', 'View (Fit frames it)', 'choice', { choices: views.map(([, v]) => v.label) }), CAMERA_VIEWS[r.camera.view].label],
      [renderParam('lens', 'Lens (full-frame)', 'number', { unit: 'mm', min: 10, max: 300, step: 1 }), r.camera.lens],
      renderRow('Frame the lamp', Object.assign(el('button', { type: 'button', text: 'Fit', title: 'Frame the lamp from the view' }), { onclick: async () => !batch.running && (await newRenderView()) && renderView.api?.fit() })),
    ]],
    ['Room', 'render:room', [
      [renderParam('wall', 'Wall behind', 'toggle'), r.wall.on],
      [renderParam('wallFrom', 'Wall measured from', 'choice', { choices: WALL_FROM.map(([, label]) => label) }), WALL_FROM.find(([k]) => k === r.wall.from)?.[1]],
      [renderParam('wallD', 'Wall distance (0: through the axis, or touching the back)', 'number', { unit: 'mm', min: -500, max: 5000, step: 5 }), r.wall.distance],
      [renderParam('floor', 'Floor or table under', 'toggle'), r.floor.on],
      [renderParam('floorD', 'Floor: below the lamp', 'number', { unit: 'mm', min: 0, max: 5000, step: 10 }), r.floor.distance],
      [renderParam('ceiling', 'Ceiling above', 'toggle'), r.ceiling.on],
      [renderParam('ceilingD', 'Ceiling: above the socket', 'number', { unit: 'mm', min: 0, max: 5000, step: 10 }), r.ceiling.distance],
      renderRow('Colour', (() => {
        const input = /** @type {HTMLInputElement} */ (el('input', { type: 'color', value: r.surface, 'aria-label': 'Wall, floor and ceiling colour' }));
        input.addEventListener('input', () => changeRender({ surface: input.value }));
        return input;
      })()),
    ]],
    ...prints.map((p, i) => {
      const look = partLook(colors, p.name, i);
      const mat = cleanMaterial(look.material);
      const v = materialValues(mat);
      const color = /** @type {HTMLInputElement} */ (el('input', { type: 'color', value: look.color, 'aria-label': `${p.name}: colour` }));
      color.addEventListener('input', () => changeLook(p.name, i, { color: color.value }));
      const reset = el('button', { type: 'button', text: '↺ The plastic\'s values', title: `${p.name}: the plastic's own values` });
      reset.addEventListener('click', () => {
        changeLook(p.name, i, { material: { plastic: mat.plastic } });
        renderPanel();
      });
      const m = (key) => `mat:${i}:${key}`;
      return /** @type {[string, string, any[]]} */ ([`Material · ${p.name}`, `render:material:${p.name}`, [
        renderRow('Colour', color),
        [renderParam(m('opacity'), 'Solid (less: see-through)', 'number', { unit: '%', min: 5, max: 100, step: 1 }), pct(look.opacity)],
        [renderParam(m('plastic'), 'Plastic', 'choice', { choices: plastics.map(([, pl]) => pl.label) }), PLASTICS[mat.plastic].label],
        [renderParam(m('roughness'), 'Matte', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(v.roughness)],
        [renderParam(m('coat'), 'Gloss coat', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(v.coat)],
        [renderParam(m('sheen'), 'Sheen', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(v.sheen)],
        [renderParam(m('metal'), 'Silk shine', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(v.metal)],
        [renderParam(m('depth'), 'Colour depth', 'number', { unit: 'mm', min: 0.5, max: 100, step: 0.5 }), v.depth],
        [renderParam(m('ior'), 'Refractive index', 'number', { min: 1.3, max: 1.7, step: 0.01 }), v.ior],
        renderRow('', reset),
      ]]);
    }),
    ['Quality', 'render:quality', [
      [renderParam('bounces', 'Light bounces', 'number', { min: 1, max: 32, step: 1 }), r.bounces],
      [renderParam('denoise', 'Denoise by itself (when done or stopped)', 'toggle'), r.denoise],
      [renderParam('denoiseStrength', 'Denoise strength', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(r.denoiseStrength)],
      [renderParam('denoiseDetail', 'Denoise: detail kept', 'number', { unit: '%', min: 0, max: 100, step: 1 }), pct(r.denoiseDetail)],
      [renderParam('denoiseModel', 'Denoiser', 'choice', { choices: DENOISERS.map(([, label]) => label) }), DENOISERS.find(([k]) => k === r.denoiseModel)?.[1]],
      [renderParam('previewSize', 'Preview: longer side', 'number', { unit: 'px', min: 160, max: 4096, step: 1 }), r.preview.size],
      [renderParam('previewSamples', 'Preview: samples', 'number', { unit: 'spp', min: 1, max: 4096, step: 1 }), r.preview.samples],
      [renderParam('finalSize', 'Render: longer side', 'number', { unit: 'px', min: 160, max: 4096, step: 1 }), r.final.size],
      [renderParam('finalSamples', 'Render: samples', 'number', { unit: 'spp', min: 1, max: 10000, step: 1 }), r.final.samples],
    ]],
    ['Batch', 'render:batch', batchRows()],
  ];
  host.replaceChildren(
    ...groups.map(([name, key, rows], g) => {
      const f = foldout(name, g === 0, key);
      const body = /** @type {HTMLElement} */ (f.querySelector('.group-body'));
      for (const row of rows) body.append(row instanceof Node ? row : settingRow(row[0], row[1], onRenderSetting));
      /** @type {HTMLElement} */ (f.querySelector('.group-count')).textContent = String(body.childElementCount);
      return f;
    }),
  );
}

// ---- The batch: setups saved one by one (batch.js), rendered one after another overnight.

/** The Batch group's rows: a name and Save, the saved setups, Batch render. */
function batchRows() {
  const name = /** @type {HTMLInputElement} */ (el('input', { type: 'text', id: 'batchName', placeholder: 'Name', 'aria-label': 'Name for the batch setup' }));
  const saveSetup = el('button', { type: 'button', className: 'btn', id: 'batchSave', text: 'Save current settings for batch', title: 'The render settings, the camera and the project as it is now, as one line of the batch' });
  saveSetup.addEventListener('click', () => saveBatchSetup());
  const run = el('button', { type: 'button', className: 'btn primary', id: 'batchRun', text: batch.running ? 'Stop batch' : 'Batch render', title: 'Render every setup in turn; each picture is downloaded as rendered and denoised' });
  run.addEventListener('click', () => runBatch());
  const list = el('div', { className: 'batch-list', id: 'batchList' });
  const ask = el('div', { className: 'confirm', id: 'batchConfirm', role: 'alertdialog', hidden: true });
  const all = el('button', { type: 'button', className: 'btn', text: 'Download all', title: 'Every picture the batch made, as one zip (a folder per batch)' });
  all.addEventListener('click', () => downloadBatchPictures());
  const forget = el('button', { type: 'button', className: 'btn', text: 'Forget', title: 'Forget the pictures kept (the files saved stay)' });
  forget.addEventListener('click', async () => {
    const answer = await askBatch('Forget the pictures the batch kept in this browser? Files already saved stay.', [
      ['forget', 'Forget them'],
      ['keep', 'Keep them'],
    ]);
    if (answer !== 'forget') return;
    await clearBatchPictures().catch(() => {});
    refreshBatchPictures();
  });
  const pictures = el('div', { className: 'setting text batch-pictures', id: 'batchPictures', hidden: true }, [el('label', { className: 'batch-count' }), el('div', { className: 'batch-buttons' }, [all, forget])]);
  refreshBatch();
  refreshBatchPictures();
  return [renderRow('Name', name), renderRow('', saveSetup), ask, list, renderRow('', run), pictures];
}

/**
 * A question in the Batch group, its answer the key of the button pressed.
 * @param {string} text
 * @param {[string, string][]} choices [key, label], the first the main one
 * @returns {Promise<string>}
 */
function askBatch(text, choices) {
  const box = $('batchConfirm');
  return new Promise((resolve) => {
    const buttons = choices.map(([key, label], k) => {
      const b = el('button', { type: 'button', className: k === 0 ? 'btn primary' : 'btn', text: label });
      b.addEventListener('click', () => {
        box.hidden = true;
        resolve(key);
      });
      return b;
    });
    box.replaceChildren(el('p', { text }), el('div', { className: 'buttons' }, buttons));
    box.hidden = false;
    box.scrollIntoView?.({ block: 'nearest' });
  });
}

/**
 * Opens a saved setup to change it: its project (the recipe with its values), the parts' looks,
 * the parts hidden, how far apart, and its render settings with the camera. Its name goes in
 * the name field, so saving it again overwrites it (asked).
 * @param {import('./batch.js').BatchSetup} setup
 */
async function openBatchSetup(setup) {
  if (batch.running) return;
  const lost = state.unsaved ? ' Changes to the project since you last saved it are lost.' : '';
  const answer = await askBatch(`Open “${setup.name}”? Its project and render settings replace the ones now.${lost}`, [
    ['open', 'Open'],
    ['keep', 'Keep editing'],
  ]);
  if (answer !== 'open' || !(await letPictureGo())) return;
  renderView.api?.discard();
  const r = setup.recipe;
  if (r) {
    const nozzle = /** @type {HTMLSelectElement} */ ($('viewNozzle'));
    if (Boolean(r.alongWall) !== Boolean(state.alongWall)) {
      nozzle.value = r.alongWall ? 'wall' : 'upright';
      viewport.setOrientation(/** @type {any} */ (nozzle.value));
      save(NOZZLE_KEY, nozzle.value);
      state.alongWall = Boolean(r.alongWall);
    }
    hiddenParts.clear();
    for (const name of r.hidden || []) hiddenParts.add(name);
    await openSource(r.source, 'opened');
    const colors = customColors();
    saveCustomColors({ ...colors, parts: r.colors?.parts || {}, hang: r.colors?.hang === 'up' ? 'up' : 'down', render: cleanRender(setup.render) });
    applyCustomColors();
    renderPartToggles(state.last);
    setAssembly(r.u || 0);
    autosave();
  } else {
    // Saved before setups kept their recipe: the render settings only.
    saveRender(setup.render);
  }
  renderView.api?.setSettings(customColors().render);
  renderPanel();
  /** @type {HTMLInputElement} */ ($('batchName')).value = setup.name;
  markRenderStale();
  renderStatus(`Opened “${setup.name}”${r ? '' : ' (its render settings: saved before setups kept their project)'}`);
}

/** The saved setups, listed (each with a button to take it out), and the name field's next name. */
async function refreshBatch() {
  let setups = [];
  try {
    setups = await batchSetups();
  } catch {
    // no IndexedDB (a private window): the batch stays empty
  }
  const list = document.getElementById('batchList');
  if (!list) return;
  list.replaceChildren(
    ...setups.map((setup) => {
      const remove = el('button', { type: 'button', className: 'btn tiny batch-remove', text: '×', title: `Take ${setup.name} out of the batch`, 'aria-label': `Take ${setup.name} out of the batch` });
      remove.addEventListener('click', async () => {
        if (batch.running) return;
        await removeBatchSetup(setup.id);
        refreshBatch();
      });
      const move = (way, text, label) => {
        const b = el('button', { type: 'button', className: 'btn tiny batch-move', text, title: `${label}: ${setup.name}`, 'aria-label': `${label}: ${setup.name}` });
        b.addEventListener('click', async () => {
          if (batch.running) return;
          await moveBatchSetup(setup.id, way);
          refreshBatch();
        });
        return b;
      };
      const line = el('div', { className: 'batch-setup', title: `Open ${setup.name} to change it`, tabIndex: 0, role: 'button' }, [
        el('strong', { text: setup.name }),
        el('span', { text: batchLine(setup, CAMERA_VIEWS) }),
        el('div', { className: 'batch-buttons' }, [move(-1, '↑', 'Earlier'), move(1, '↓', 'Later'), remove]),
      ]);
      line.addEventListener('click', (e) => {
        if (!(/** @type {HTMLElement} */ (e.target).closest('button'))) openBatchSetup(setup);
      });
      line.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') openBatchSetup(setup);
      });
      return line;
    }),
  );
  const name = /** @type {HTMLInputElement | null} */ (document.getElementById('batchName'));
  if (name && !name.value) name.placeholder = nextBatchName(setups);
  document.getElementById('batchRun')?.toggleAttribute('disabled', !setups.length && !batch.running);
}

/** A project file's batch plan, added to the batch (the setups it has already left out). */
async function addBatchPlan(plan) {
  try {
    const fresh = setupsOfPlan(plan, await batchSetups(), Date.now());
    for (const setup of fresh) await addBatchSetup(setup);
    if (fresh.length) renderStatus(`The file's batch plan: ${fresh.length} ${fresh.length === 1 ? 'setup' : 'setups'} added`);
  } catch {
    // no IndexedDB here: the plan stays in the file
  }
  refreshBatch();
}

/** Saves the render's settings (the camera too) and the project as it is now as a batch setup. */
async function saveBatchSetup() {
  if (!state.last) return;
  const field = /** @type {HTMLInputElement} */ ($('batchName'));
  try {
    const setups = await batchSetups();
    let name = field.value.trim() || nextBatchName(setups);
    let id = Date.now();
    /** @type {number | undefined} */
    let order;
    // A name the batch has: overwrite that setup (in its place), or keep both.
    const same = setups.find((x) => x.name === name);
    if (same) {
      const answer = await askBatch(`“${name}” is in the batch already.`, [
        ['over', `Overwrite “${name}”`],
        ['new', 'Save as new'],
        ['cancel', 'Cancel'],
      ]);
      if (answer === 'cancel') return;
      if (answer === 'over') {
        id = same.id;
        order = same.order;
      }
      else {
        let k = 2;
        while (setups.some((x) => x.name === `${name} ${k}`)) k++;
        name = `${name} ${k}`;
      }
    }
    const recipe = { source: await currentSource(), alongWall: Boolean(state.alongWall), ...renderContext() };
    await addBatchSetup({ id, name, project: projectName(), render: customColors().render, snapshot: renderSnapshot(state.last), saved: Date.now(), recipe, ...(order === undefined ? {} : { order }) });
    field.value = '';
    renderStatus(`Saved for the batch: ${name}`);
  } catch (err) {
    renderStatus(`Couldn't save for the batch: ${/** @type {any} */ (err)?.message || err}`);
  }
  refreshBatch();
}

/**
 * The next progress the test accepts (or a GPU reset, `{ state: 'lost' }`).
 * @param {(p: any) => boolean} test
 * @returns {Promise<any>}
 */
function nextRender(test) {
  return new Promise((resolve) => {
    const f = (p) => {
      if (p.state !== 'lost' && !test(p)) return;
      renderView.listeners.delete(f);
      resolve(p);
    };
    renderView.listeners.add(f);
  });
}

/**
 * The picture on the render's canvas, a batch's: kept in the browser (whatever happens to the
 * download), then written into the batch's folder, or downloaded. (A browser lets a page download
 * one file by itself; for more, Chrome asks once, and an answer not given, or No, blocks the rest:
 * the folder needs no such leave, and the kept pictures can be downloaded as one zip.)
 */
async function downloadRender(name) {
  const api = renderView.api;
  if (!api) return;
  const blob = await (await fetch(api.capture())).blob();
  try {
    await keepBatchPicture({ key: `${batch.stamp}/${name}`, batch: batch.stamp, file: name, blob, at: Date.now() });
  } catch {
    // no IndexedDB, or full: the download still goes
  }
  let written = false;
  if (batch.folder) {
    try {
      const handle = await batch.folder.getFileHandle(name, { create: true });
      const out = await handle.createWritable();
      await out.write(blob);
      await out.close();
      written = true;
    } catch {
      // the folder went away, or its permission: a download instead
    }
  }
  if (!written) download(name, blob, 'image/png');
  picture.saved = true;
  refreshBatchPictures();
}

/** The kept pictures' row in the Batch group: how many, Download all (one zip), Forget. */
async function refreshBatchPictures() {
  const row = document.getElementById('batchPictures');
  if (!row) return;
  const kept = await batchPictures().catch(() => []);
  row.hidden = !kept.length;
  const count = row.querySelector('.batch-count');
  if (count) count.textContent = `${kept.length} ${kept.length === 1 ? 'picture' : 'pictures'} kept`;
}

/** Every kept batch picture as one zip (a folder per batch): one download. */
async function downloadBatchPictures() {
  const kept = await batchPictures().catch(() => []);
  if (!kept.length) return;
  const files = await Promise.all(kept.map(async (k) => ({ path: `${k.batch}/${k.file}`, bytes: new Uint8Array(await k.blob.arrayBuffer()) })));
  const last = kept[kept.length - 1].batch;
  download(`${last || 'batch'}.zip`, zipFiles(files), 'application/zip');
}

/**
 * One setup: prepared, rendered at its Render size and samples, downloaded as rendered, then
 * denoised and downloaded again. 'done', 'stopped', or 'lost' (the GPU reset).
 * @param {import('./batch.js').BatchSetup} setup
 * @param {Date} started
 */
async function renderSetup(setup, started) {
  // From a project file (another computer's plan): its beads made from its recipe here.
  let snapshot = setup.snapshot;
  if (!snapshot && setup.recipe) {
    renderStatus(`${batch.label}Making the project`);
    let result = null;
    // (A run of the project's own can replace this one: tried again.)
    for (let k = 0; k < 5 && !result; k++) result = await engine.evaluate(setup.recipe.source, {}, 'final', null, setup.recipe.alongWall);
    if (!result) throw new Error('the engine was busy');
    if (result.diagnostics.some((d) => d.severity === 'error')) throw new Error('its recipe has a problem');
    snapshot = renderSnapshot(result, setup.recipe);
  }
  if (!snapshot) throw new Error('nothing to render');
  const api = await renderApi();
  renderView.busy = true;
  try {
    // Denoised below, after the picture as rendered is saved.
    api.setSettings({ ...setup.render, denoise: false });
    await api.prepare(snapshot, (what) => renderStatus(`${batch.label}Preparing · ${what}`));
  } finally {
    renderView.busy = false;
  }
  if (batch.stopping) return 'stopped';
  const finished = nextRender((p) => p.state === 'done' || p.state === 'stopped');
  api.start('final');
  const p = await finished;
  if (p.state !== 'done') return p.state;
  await downloadRender(batchFileName(started, setup.name, 'rendered'));
  const denoised = nextRender((q) => Boolean(q.denoised) || Boolean(q.denoiseError));
  api.denoise();
  const q = await denoised;
  if (q.state === 'lost') return 'lost';
  if (q.denoised) await downloadRender(batchFileName(started, setup.name, 'denoised'));
  return 'done';
}

/** Renders every saved setup in turn (or stops the batch when it's running). */
async function runBatch() {
  if (batch.running) {
    batch.stopping = true;
    renderView.api?.stop();
    return;
  }
  const started = new Date();
  // Chrome on a computer: a folder for the pictures, asked first (while the click still counts);
  // in it, a folder for this batch. Cancelled, or another browser: downloads.
  batch.stamp = batchStamp(started);
  batch.folder = null;
  const pick = /** @type {any} */ (window).showDirectoryPicker;
  if (typeof pick === 'function' && !renderView.busy) {
    try {
      const parent = await pick.call(window, { id: 'easycad-batch', mode: 'readwrite' });
      batch.folder = await parent.getDirectoryHandle(batch.stamp, { create: true });
    } catch {
      batch.folder = null;
    }
  }
  const setups = await batchSetups().catch(() => []);
  if (!setups.length || renderView.busy || !(await letPictureGo())) return;
  batch.running = true;
  batch.stopping = false;
  $('batchRun').textContent = 'Stop batch';
  // The screen kept on: a sleeping screen stops the page's drawing, and the render with it.
  /** @type {any} */
  let lock = null;
  try {
    lock = await /** @type {any} */ (navigator).wakeLock?.request('screen');
  } catch {
    // not offered: keep the screen on by hand
  }
  const failed = [];
  let done = 0;
  try {
    for (const [k, setup] of setups.entries()) {
      if (batch.stopping) break;
      batch.label = `Batch ${k + 1}/${setups.length} · ${setup.name} · `;
      // A GPU reset: tried once more (a Mac takes smaller pieces after one).
      let result = 'lost';
      for (let attempt = 0; attempt < 2 && result === 'lost' && !batch.stopping; attempt++) {
        try {
          result = await renderSetup(setup, started);
        } catch (err) {
          result = /** @type {any} */ (err)?.code === 'no-gl' ? 'lost' : 'failed';
          if (result === 'failed') break;
        }
      }
      if (result === 'done') done++;
      else if (!batch.stopping) failed.push(setup.name);
    }
  } finally {
    batch.running = false;
    batch.label = '';
    lock?.release?.().catch?.(() => {});
    // The project as it is (the batch's last setup was prepared): Prepare again.
    renderView.key = '';
    renderView.api?.setSettings(customColors().render);
    markRenderStale();
    const run = document.getElementById('batchRun');
    if (run) run.textContent = 'Batch render';
    const stopped = batch.stopping ? ' · stopped' : '';
    const where = batch.folder ? ` · in the folder ${batch.stamp}` : '';
    renderStatus(`Batch: ${done} of ${setups.length} rendered${failed.length ? ` · not rendered: ${failed.join(', ')}` : ''}${stopped}${where}`);
    batch.folder = null;
    batch.stopping = false;
  }
}

/** A Render tab setting changed (dragged, typed or picked). */
function onRenderSetting(setting, value) {
  const name = setting.replace(/^render-/, '');
  const r = customColors().render;
  const mat = /^mat:(\d+):(\w+)$/.exec(name);
  if (mat) {
    const i = Number(mat[1]);
    const part = printsOf(state.last)[i];
    if (!part) return;
    const key = mat[2];
    if (key === 'opacity') return changeLook(part.name, i, { opacity: Math.max(0.05, value / 100) });
    if (key === 'plastic') {
      const plastic = Object.entries(PLASTICS).find(([, pl]) => pl.label === value)?.[0] ?? 'pla';
      // A new plastic brings its own values (the part's own go).
      changeLook(part.name, i, { material: { plastic } });
      renderPanel();
      return;
    }
    const scaled = ['roughness', 'coat', 'sheen', 'metal'].includes(key) ? value / 100 : value;
    const own = cleanMaterial(customColors().parts[part.name]?.material);
    changeLook(part.name, i, { material: cleanMaterial({ ...own, [key]: scaled }) });
    return;
  }
  switch (name) {
    case 'light':
      return changeRender({ light: value });
    case 'hang': {
      const colors = customColors();
      colors.hang = value === 'up' ? 'up' : 'down';
      saveCustomColors(colors);
      if (state.last) {
        viewport.setFixture(fixtureFor(state.last));
        renderPlanView(state.last);
      }
      markRenderStale();
      return;
    }
    case 'lumens':
    case 'kelvin':
    case 'exposure':
    case 'bounces':
      return changeRender({ [name]: value });
    case 'room':
      return changeRender({ room: value / 100 });
    case 'lightSize':
      return changeRender({ lightSize: value / 100 });
    case 'view': {
      const view = Object.entries(CAMERA_VIEWS).find(([, v]) => v.label === value)?.[0] ?? 'front';
      return changeRender({ camera: { ...r.camera, view, position: null, target: null } });
    }
    case 'lens':
      return changeRender({ camera: { ...r.camera, lens: value } });
    case 'wall':
    case 'floor':
    case 'ceiling':
      return changeRender({ [name]: { ...r[name], on: value } });
    case 'wallFrom': {
      // From the back: just behind the lamp (20 mm), the shade's light falling across the wall.
      const from = WALL_FROM.find(([, label]) => label === value)?.[0] ?? 'axis';
      if (from === r.wall.from) return;
      changeRender({ wall: { on: true, from, distance: from === 'back' ? 20 : 0 } });
      renderPanel(); // the distance field shows the new distance
      return;
    }
    case 'wallD':
    case 'floorD':
    case 'ceilingD': {
      const k = name.slice(0, -1);
      return changeRender({ [k]: { ...r[k], on: true, distance: value } });
    }
    case 'denoise':
      return changeRender({ denoise: value });
    case 'denoiseStrength':
      return changeRender({ denoiseStrength: value / 100 });
    case 'denoiseDetail':
      return changeRender({ denoiseDetail: value / 100 });
    case 'denoiseModel':
      return changeRender({ denoiseModel: DENOISERS.find(([, label]) => label === value)?.[0] ?? 'detailed' });
    case 'previewSize':
      return changeRender({ preview: { ...r.preview, size: value } });
    case 'previewSamples':
      return changeRender({ preview: { ...r.preview, samples: value } });
    case 'finalSize':
      return changeRender({ final: { ...r.final, size: value } });
    case 'finalSamples':
      return changeRender({ final: { ...r.final, samples: value } });
  }
}

function wireRender() {
  $('viewRender').addEventListener('click', () => showRender(!renderOn()));
  // While a batch runs, these leave its render alone (Stop stops the batch). A picture held is
  // let go only by these, asked first when it's worth keeping.
  const fresh = (go) => async () => {
    if (batch.running || !(await letPictureGo())) return;
    showPictureAt(1, 0, 0);
    go();
  };
  $('renderPrepare').addEventListener('click', fresh(() => prepareRender()));
  $('renderPreview').addEventListener('click', fresh(() => renderView.api?.start('preview')));
  $('renderFinal').addEventListener('click', fresh(() => renderView.api?.start('final')));
  $('renderNewView').addEventListener('click', () => !batch.running && newRenderView());
  $('renderStop').addEventListener('click', () => (batch.running ? runBatch() : renderView.api?.stop()));
  wirePictureView();
  $('renderDenoised').addEventListener('click', () => {
    const api = renderView.api;
    if (!api) return;
    if ($('renderDenoised').dataset.denoised) api.showDenoised($('renderDenoised').getAttribute('aria-pressed') !== 'true');
    else api.denoise();
  });
  $('renderSave').addEventListener('click', () => saveRenderPng());
}

function wire() {
  for (const t of TABS) $(`tab-${t}`).addEventListener('click', () => selectTab(t));

  document.querySelectorAll('[data-mode]').forEach((b) =>
    b.addEventListener('click', () => {
      showRender(false);
      document.querySelectorAll('[data-mode]').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
      $('viewPlan').setAttribute('aria-pressed', 'false');
      if ($('viewPrints').getAttribute('aria-pressed') === 'true') showPrints(false);
      viewport.setMode(/** @type {any} */ (/** @type {HTMLElement} */ (b).dataset.mode));
      if (state.last) renderPlanView(state.last);
    }),
  );
  // The assembly row: play it apart (or together), scrub it, or jump to the exploded view.
  $('asmPlay').addEventListener('click', () => playAssembly());
  $('asmRange').addEventListener('input', () => {
    stopAssembly();
    setAssembly(Number(/** @type {HTMLInputElement} */ ($('asmRange')).value) / 1000);
  });
  $('asmExplode').addEventListener('click', () => {
    stopAssembly();
    setAssembly(assembly.u >= 1 ? 0 : 1);
  });
  // The plan: the parts cut through their axis, over the 3D view.
  $('viewPrints').addEventListener('click', () => {
    showRender(false);
    const on = $('viewPrints').getAttribute('aria-pressed') !== 'true';
    if (on && $('viewPlan').getAttribute('aria-pressed') === 'true') $('viewPlan').click();
    showPrints(on);
  });
  $('viewPlan').addEventListener('click', () => {
    showRender(false);
    if ($('viewPrints').getAttribute('aria-pressed') === 'true') showPrints(false);
    const on = $('viewPlan').getAttribute('aria-pressed') !== 'true';
    $('viewPlan').setAttribute('aria-pressed', String(on));
    document.querySelectorAll('[data-mode]').forEach((x) => x.setAttribute('aria-pressed', String(!on && /** @type {HTMLElement} */ (x).dataset.mode === 'both')));
    if (!on) viewport.setMode('both');
    if (state.last) renderPlanView(state.last);
  });
  // Fit what's shown of the toolpath and orbit round its centre: the Fit button, a double-click,
  // or on a phone a double-tap (touch doesn't make a dblclick everywhere).
  const renderFit = async () => !batch.running && (await newRenderView()) && renderView.api?.fit();
  $('viewFit').addEventListener('click', () => (renderOn() ? renderFit() : planOn() ? fitPlan() : viewport.focus()));
  // The view as a picture, sharper than the screen.
  // What's shown: the plan when it's up, else the 3D view (taken apart, when it is).
  $('viewSnap').addEventListener('click', async () => {
    const url = $('plan').hidden ? viewport.capture(2) : await capturePlan($('plan'), 2);
    const blob = await (await fetch(url)).blob();
    download(`${saveName()}.png`, blob, 'image/png');
  });
  wireRender();
  viewport.listen('dblclick', () => viewport.focus());
  let lastTap = { t: 0, x: 0, y: 0 };
  let touches = 0;
  viewport.listen('pointerdown', (e) => {
    if (/** @type {PointerEvent} */ (e).pointerType === 'touch') touches++;
  });
  viewport.listen('pointerup', (ev) => {
    const e = /** @type {PointerEvent} */ (ev);
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
  viewport.listen('pointercancel', () => (touches = 0));
  // Set views, and parallel or perspective (remembered on this device).
  // In the Render view they set the render's camera: snapped to that view and framed.
  const RENDER_VIEWS = { iso: 'three-quarter', front: 'front', side: 'side', top: 'top' };
  document.querySelectorAll('[data-view]').forEach((b) =>
    b.addEventListener('click', () => {
      const view = /** @type {HTMLElement} */ (b).dataset.view || 'iso';
      if (renderOn()) {
        if (batch.running) return;
        // A new camera: the picture held is let go (asked first when it's worth keeping).
        newRenderView().then((ok) => {
          if (!ok) return;
          changeRender({ camera: { ...customColors().render.camera, view: RENDER_VIEWS[view] ?? 'front', position: null, target: null } });
          renderView.api?.fit();
          renderPanel();
        });
        return;
      }
      viewport.view(/** @type {any} */ (view));
    }),
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
  state.alongWall = nozzleSelect.value !== 'upright';
  nozzleSelect.addEventListener('change', () => {
    viewport.setOrientation(/** @type {any} */ (nozzleSelect.value));
    save(NOZZLE_KEY, nozzleSelect.value);
    state.alongWall = nozzleSelect.value !== 'upright';
    // The engine only works the wall's direction out when asked: run again for it.
    if (state.alongWall && state.last && !state.last.paths.ups) run('final');
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
      const sketch = text !== null && /^\s*\{/.test(text) ? JSON.parse(text) : null;
      if (sketch && isSketcherSettings(sketch)) {
        await openSketcher(sketch, picked?.name || 'thread-pendant');
      } else if (text !== null) {
        const { text: withBatch, colors } = withoutColors(withoutFileNote(text));
        const { text: withPrinter, plan } = withoutBatch(withBatch);
        const { text: recipe, printer } = withoutPrinter(withPrinter);
        // The printer's settings are the file's (a file without them: the printer's defaults).
        state.profiles = printerSettingsOf(printer);
        await openSource(recipe);
        setPrinter(state.printer);
        restoreColors(colors);
        setSaveName((picked?.name || '').replace(/\.easycad$/i, ''));
        autosave();
        checkTemplate();
        if (plan) await addBatchPlan(plan);
      }
    } catch (err) {
      showSettingsError(`Could not open the project: ${err.message || err}`);
    }
  });
  $('saveName').addEventListener('input', () => autosave());
  $('saveProject').addEventListener('click', async () => {
    const site = new URL('.', location.href).href;
    const colors = colorsToSave();
    const printer = printerLine(state.printer, settingsOf(state.printer, state.profile));
    // The Render's batch plan goes with it (this project's setups), to render on another computer.
    const plan = batchPlanOf(await batchSetups().catch(() => []), projectName());
    download(`${saveName()}.easycad`, `${fileNote(site)}\n\n${printer}\n${colors ? `${colorsLine(colors)}\n` : ''}${plan ? `${batchPlanLine(plan)}\n` : ''}\n${await currentSource()}`);
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
      ...(colorsToSave() ? { colors: colorsToSave() } : {}),
    };
    download(`${saveName()}-settings.json`, JSON.stringify(data, null, 2), 'application/json');
  });
  $('saveSketcher').addEventListener('click', async () => {
    await engine.idle();
    download(`${saveName()}.json`, JSON.stringify(valuesToSketcher(state.values), null, 2), 'application/json');
  });
  $('loadSettings').addEventListener('click', () => $('settingsFile').click());
  $('settingsFile').addEventListener('change', async (e) => {
    try {
      const text = await readPicked(e.target);
      if (text === null) return;
      const data = JSON.parse(text);
      if (isSketcherSettings(data)) {
        await openSketcher(data, 'thread-pendant');
        return;
      }
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
      restoreColors(data.colors);
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
  // The project left open keeps its printer settings; one left open from before they went into
  // the project (23 Nov 2026) takes the ones this device had then.
  if (saved?.printerSettings) state.profiles = printerSettingsOf(saved.printerSettings);
  else if (saved) for (const id of Object.keys(PRINTERS)) state.profiles[id] = loadProfile(/** @type {keyof typeof PRINTERS} */ (id));
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
