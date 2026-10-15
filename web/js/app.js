// EasyCAD: a recipe (.easycad text) → Rust engine (WebAssembly, in a worker) → 3D view, a settings
// window generated from the recipe, and a print estimate.
//
// The recipe text is the project and the single source of truth: moving a slider writes the new
// value into the text (the engine does that, see crates/wasm), and saving a project saves exactly
// that text.

import { createEngine } from './engine.js';
import { createViewport } from './viewport.js';
import { $, copyText, download, el, fmt, slug } from './dom.js';
import { layoutKey, renderSettings } from './settings.js';
import { PRINTERS, printerFor, renderPrint } from './print.js';
import { aiPrompt, renderProblems, renderSteps, selectLine } from './recipe.js';

const TEMPLATES = [
  { id: 'twisted-vase', name: 'Twisted star vase' },
  { id: 'ribbed-cylinder', name: 'Ribbed cylinder' },
  { id: 'star-bowl', name: 'Star bowl' },
];
const STORAGE_KEY = 'easycad.project';
const MAX_FILE_BYTES = 256 * 1024;
const TABS = ['settings', 'recipe', 'print', 'about'];

const state = {
  /** The recipe text: the project. */
  source: '',
  /** Setting values as last opened, for "Reset settings". */
  openedValues: /** @type {Record<string, any>} */ ({}),
  /** Current setting values (each in its own unit); written into the text on every run. */
  values: /** @type {Record<string, any>} */ ({}),
  /** The settings window's layout, so it's rebuilt only when the recipe's settings change. */
  layout: '',
  printer: /** @type {keyof typeof PRINTERS} */ ('p1p'),
  /** Changed since it was opened or saved (asks before a template replaces it). */
  unsaved: false,
  /** @type {import('./engine.js').RunResult | null} */
  last: null,
  engineInfo: { version: '?', loadMs: 0 },
  catalog: '',
};

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
    result = await engine.evaluate(state.source, state.values, quality);
  } catch (err) {
    setStatus('Engine error', 'bad');
    showSettingsError(String(err.message || err));
    return;
  }
  if (!result) return; // a newer run replaced this one
  state.last = result;
  state.source = result.source;
  for (const p of result.params) if (!(p.name in state.values)) state.values[p.name] = p.value;
  if (how.opened) {
    state.openedValues = { ...state.values };
    if (result.printer) setPrinter(printerFor(result.printer));
  }

  const layout = layoutKey(result.params);
  if (how.fresh || layout !== state.layout) {
    state.layout = layout;
    renderSettings($('settings'), result.params, state.values, onSettingChange);
  }
  viewport.show(result, { refit: refitNext });
  refitNext = false;

  renderProject(result);
  renderPrint($('stats'), $('checks'), result.print, state.printer);
  renderProblems($('problems'), result.diagnostics, (line) => selectLine(editor, line));
  renderSteps($('steps'), result.steps);
  showSource(result.source);
  renderHud(result);
  renderAbout();

  const errors = result.diagnostics.filter((d) => d.severity === 'error').length;
  showSettingsError(errors ? `The recipe has ${errors === 1 ? 'a problem' : `${errors} problems`}: see the Recipe tab.` : '');
  if (errors) setStatus(`${errors} recipe ${errors === 1 ? 'problem' : 'problems'}`, 'bad');
  else setStatus(`Engine ready · ${fmt(result.ms)} ms`, 'ok');
  autosave();
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
  $('projectNotes').textContent = result.notes.join(' ');
}

function renderHud(result) {
  const st = result.print && result.print.stats;
  const parts = st ? [`${fmt(st.layers)} layers`, `${fmt(st.pathPoints)} path points`] : ['no toolpath'];
  $('hud').textContent = [...parts, `${fmt(result.ms)} ms`].join(' · ');
  const empty = !result.mesh.indices.length && result.curves.starts.length < 2 && result.paths.starts.length < 2;
  const msg = $('viewMessage');
  msg.hidden = !empty;
  msg.textContent = result.diagnostics.some((d) => d.severity === 'error')
    ? 'Nothing to draw: the recipe has a problem (see the Recipe tab).'
    : 'Nothing to draw yet: add a line like  show name  to the recipe.';
}

function renderAbout() {
  const steps = (state.catalog.match(/^\w+\(/gm) || []).length;
  const rows = [
    ['Engine', `Rust → WebAssembly, v${state.engineInfo.version}`],
    ['Engine start', `${fmt(state.engineInfo.loadMs)} ms`],
    ['Last run', state.last ? `${fmt(state.last.ms, 1)} ms (in a background worker)` : '…'],
    ['Recipe language', `easycad 0.1 · ${steps || '…'} steps`],
    ['3D view', viewport.rendererInfo()],
    ['CPU threads', String(navigator.hardwareConcurrency || '?')],
    ['Multi-thread ready', self.crossOriginIsolated ? 'yes' : 'not yet (needs isolation headers)'],
  ];
  const about = $('about');
  about.replaceChildren(...rows.flatMap(([k, v]) => [el('dt', { text: k }), el('dd', { text: v })]));
  const link = el('a', {
    href: 'https://github.com/jasperkrebs/easycad/blob/claude/quirky-gates-1ykve2/docs/roadmap.md',
    target: '_blank',
    rel: 'noopener',
    text: 'Roadmap',
  });
  about.append(el('dt', { text: 'Docs' }), el('dd', {}, [link]));
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
// Projects: templates, open, save, autosave
// ---------------------------------------------------------------------------------------------

/** Opens recipe text as the current project. */
async function openSource(text) {
  state.source = text;
  setDirty(false);
  editor.value = text;
  await run('final', { fresh: true, opened: true });
  state.unsaved = false;
}

async function openTemplate(id) {
  const response = await fetch(`templates/${id}.easycad`);
  if (!response.ok) throw new Error(`template ${id}: ${response.status}`);
  await openSource(await response.text());
}

let saveTimer = 0;
function autosave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ source: state.source, openedValues: state.openedValues, printer: state.printer, unsaved: state.unsaved }),
      );
    } catch {
      // Storage can be unavailable (private browsing); the app works without it.
    }
  }, 300);
}

function restore() {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || 'null');
    if (saved && typeof saved.source === 'string' && saved.source.trim()) {
      return saved;
    }
  } catch {
    // Unreadable storage: start from a template.
  }
  return null;
}

/** The project, with every pending slider value written in. */
async function currentSource() {
  await engine.idle();
  return state.source;
}

function projectName() {
  return (state.last && state.last.project) || 'project';
}

function setPrinter(id) {
  state.printer = id;
  /** @type {HTMLSelectElement} */ ($('printer')).value = id;
  viewport.setBed(PRINTERS[id].bed);
}

// ---------------------------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------------------------

function selectTab(name) {
  for (const t of TABS) {
    $(`tab-${t}`).setAttribute('aria-selected', String(t === name));
    $(`panel-${t}`).hidden = t !== name;
  }
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
      viewport.setMode(/** @type {any} */ (/** @type {HTMLElement} */ (b).dataset.mode));
    }),
  );
  $('view').addEventListener('dblclick', () => viewport.refit());

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
      const text = await readPicked(e.target);
      if (text !== null) await openSource(text);
    } catch (err) {
      showSettingsError(`Could not open the project: ${err.message || err}`);
    }
  });
  $('saveProject').addEventListener('click', async () => {
    download(`${slug(projectName())}.easycad`, await currentSource());
    state.openedValues = { ...state.values };
    state.unsaved = false;
    autosave();
  });

  $('printer').addEventListener('change', (e) => {
    setPrinter(/** @type {any} */ (/** @type {HTMLSelectElement} */ (e.target).value));
    if (state.last) renderPrint($('stats'), $('checks'), state.last.print, state.printer);
    autosave();
  });

  $('resetSettings').addEventListener('click', () => {
    Object.assign(state.values, state.openedValues);
    state.layout = ''; // rebuild the window so every slider shows its reset value
    run('final');
  });
  $('saveSettings').addEventListener('click', async () => {
    await engine.idle();
    const data = { app: 'EasyCAD', kind: 'settings', version: 2, project: projectName(), printer: state.printer, values: state.values };
    download(`${slug(projectName())}-settings.json`, JSON.stringify(data, null, 2), 'application/json');
  });
  $('loadSettings').addEventListener('click', () => $('settingsFile').click());
  $('settingsFile').addEventListener('change', async (e) => {
    try {
      const text = await readPicked(e.target);
      if (text === null) return;
      const data = JSON.parse(text);
      if (!data || typeof data.values !== 'object') throw new Error('this file has no EasyCAD settings in it.');
      let used = 0;
      for (const [k, v] of Object.entries(data.values)) {
        if (k in state.values && typeof v === typeof state.values[k]) {
          state.values[k] = v;
          used++;
        }
      }
      if (typeof data.printer === 'string' && data.printer in PRINTERS) setPrinter(/** @type {any} */ (data.printer));
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
      // Back to how it was left: the printer chosen, and "Reset" still meaning "as opened".
      setPrinter(saved.printer in PRINTERS ? saved.printer : state.printer);
      if (saved.openedValues && typeof saved.openedValues === 'object') state.openedValues = saved.openedValues;
      state.unsaved = saved.unsaved === true;
      if (state.last) renderPrint($('stats'), $('checks'), state.last.print, state.printer);
    } else {
      await openTemplate(TEMPLATES[0].id);
    }
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
